use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    message::{Mailbox, header::ContentType},
    transport::smtp::authentication::Credentials,
};
use tokio::sync::{mpsc, oneshot};
use tracing::instrument;
use uuid::Uuid;

use crate::{
    config::{Configuration, NimbusConfig},
    error::AppError,
};

struct EmailRequest {
    to: String,
    subject: String,
    html: String,
    /// When set, the worker reports whether delivery succeeded (SMTP test).
    reply: Option<oneshot::Sender<Result<(), AppError>>>,
}

enum EmailCommand {
    Send(EmailRequest),
    /// Rebuild the transport from the current [`NimbusConfig`].
    Reconfigure(oneshot::Sender<()>),
}

/// SMTP connection settings used only while building a transport.
struct SmtpEndpoint {
    host: String,
    port: u16,
    username: String,
    password: String,
}

impl SmtpEndpoint {
    fn from_config(config: &Configuration) -> Option<Self> {
        if !config.smtp_configured() {
            return None;
        }
        Some(Self {
            host: config.smtp_host.clone(),
            port: config.smtp_port,
            username: config.smtp_username.clone(),
            password: config.smtp_password.clone(),
        })
    }
}

type EmailQueueHandle = mpsc::Sender<EmailCommand>;

/// Background worker that delivers queued outbound messages over SMTP.
struct EmailQueue {
    commands: mpsc::Receiver<EmailCommand>,
    config: NimbusConfig,
    mailer: AsyncSmtpTransport<Tokio1Executor>,
}

impl EmailQueue {
    fn start(config: NimbusConfig, mailer: AsyncSmtpTransport<Tokio1Executor>) -> EmailQueueHandle {
        let (tx, rx) = mpsc::channel(128);
        EmailQueue {
            commands: rx,
            config,
            mailer,
        }
        ._start();
        tx
    }

    #[instrument(skip(self))]
    fn _start(mut self) {
        tokio::spawn(async move {
            while let Some(command) = self.commands.recv().await {
                match command {
                    EmailCommand::Reconfigure(done) => {
                        self.reconfigure().await;
                        let _ = done.send(());
                    }
                    EmailCommand::Send(mut request) => {
                        let reply = request.reply.take();
                        let require_smtp = reply.is_some();
                        let result = self.deliver(request, require_smtp).await;

                        if let Some(reply) = reply {
                            let _ = reply.send(result);
                        } else if let Err(e) = result {
                            tracing::error!(error=%e, "error sending email");
                        }
                    }
                }
            }

            tracing::info!("Email request channel closed, shutting down worker");
        });
    }

    async fn reconfigure(&mut self) {
        let cfg = self.config.get().await;
        self.mailer = transport(&cfg);
    }

    #[instrument(skip(self, request), fields(to = %request.to, subject = %request.subject), err(Debug))]
    async fn deliver(&self, request: EmailRequest, require_smtp: bool) -> Result<(), AppError> {
        let cfg = self.config.get().await;

        if !cfg.smtp_configured() {
            if require_smtp {
                return Err(AppError::Unprocessable(
                    "SMTP is not fully configured".into(),
                ));
            }
            tracing::info!(
                to = %request.to,
                subject = %request.subject,
                "smtp is not configured; email was not sent"
            );
            return Ok(());
        }

        let from = build_from(&cfg.smtp_username, &cfg.email_from);

        let to = request.to.parse::<Mailbox>().map_err(|error| {
            tracing::error!(%error, "invalid recipient address");
            AppError::Unprocessable("Invalid email address".into())
        })?;

        let message = Message::builder()
            .from(from)
            .to(to)
            .subject(request.subject)
            .header(ContentType::TEXT_HTML)
            .body(request.html)
            .map_err(|error| {
                tracing::error!(%error, "building email");
                AppError::Internal
            })?;

        self.mailer.send(message).await.map_err(|error| {
            tracing::error!(%error, "smtp send failed");
            if require_smtp {
                AppError::Unprocessable("SMTP delivery failed".into())
            } else {
                AppError::Internal
            }
        })?;
        Ok(())
    }
}

fn build_from(username: &str, from_name: &str) -> Mailbox {
    let parsed = if username.is_empty() {
        "nimbus@localhost".parse()
    } else {
        username.parse()
    };

    match parsed {
        Ok(email) => Mailbox::new(Some(from_name.to_string()), email),
        Err(error) => {
            tracing::error!(%error, "invalid smtp from address; using fallback");
            Mailbox::new(None, "nimbus@localhost".parse().expect("static mailbox"))
        }
    }
}

fn transport(config: &Configuration) -> AsyncSmtpTransport<Tokio1Executor> {
    let Some(endpoint) = SmtpEndpoint::from_config(config) else {
        tracing::info!("smtp is not configured; using inert transport");
        return AsyncSmtpTransport::<Tokio1Executor>::unencrypted_localhost();
    };

    let transport = if endpoint.port == 465 {
        AsyncSmtpTransport::<Tokio1Executor>::relay(&endpoint.host)
    } else {
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&endpoint.host)
    };

    match transport {
        Ok(builder) => {
            tracing::info!(host = %endpoint.host, port = endpoint.port, "smtp transport ready");
            builder
                .port(endpoint.port)
                .credentials(Credentials::new(endpoint.username, endpoint.password))
                .build()
        }
        Err(error) => {
            tracing::error!(%error, "smtp transport could not be built; using inert transport");
            AsyncSmtpTransport::<Tokio1Executor>::unencrypted_localhost()
        }
    }
}

/// SMTP sender. Missing host, username, or password disables delivery and logs instead.
///
/// Delivery runs on a background queue so callers are not blocked on SMTP latency.
/// The queue owns a single SMTP transport; call [`EmailSender::reconfigure`] after
/// SMTP settings change.
#[derive(Clone)]
pub struct EmailSender {
    queue: EmailQueueHandle,
    config: NimbusConfig,
}

impl EmailSender {
    pub async fn new(config: NimbusConfig) -> Self {
        let mailer = transport(&config.get().await);
        let queue = EmailQueue::start(config.clone(), mailer);

        Self { queue, config }
    }

    /// Rebuild the queue's SMTP transport from the current config.
    pub async fn reconfigure(&self) -> Result<(), AppError> {
        let (tx, rx) = oneshot::channel();
        self.queue
            .send(EmailCommand::Reconfigure(tx))
            .await
            .map_err(|_| AppError::Internal)?;
        rx.await.map_err(|_| AppError::Internal)
    }

    pub async fn send_account_created(&self, to: &str, username: &str) -> Result<(), AppError> {
        let username = escape(username);
        let html = format!(
            "<p>An administrator created a Nimbus account for <strong>{username}</strong>.</p>\
             <p>Sign in and change the password you were given.</p>"
        );
        self.enqueue(to, "Your Nimbus account", html, None).await
    }

    pub async fn send_password_reset(
        &self,
        to: &str,
        username: &str,
        token: Uuid,
    ) -> Result<(), AppError> {
        let origin = self.config.nimbus_origin().await;
        let link = format!("{origin}/reset-password/{token}");

        let username = escape(username);

        let html = format!(
            "<p>{username}, a password reset was requested for your Nimbus account.</p>\
             <p><a href=\"{link}\">Choose a new password</a></p>\
             <p>If you did not ask for this, you can ignore the message.</p>"
        );

        #[cfg(debug_assertions)]
        if !self.config.get().await.smtp_configured() {
            tracing::info!(%link, "smtp is not configured; password reset link");
        }

        self.enqueue(to, "Reset your Nimbus password", html, None)
            .await
    }

    pub async fn send_password_changed(&self, to: &str, username: &str) -> Result<(), AppError> {
        let username = escape(username);
        let html = format!("<p>{username}, your Nimbus password was changed.</p>");
        self.enqueue(to, "Nimbus password changed", html, None)
            .await
    }

    /// Send a short message used to verify SMTP settings. Delivers via the shared
    /// queue transport and waits for the outcome so the caller learns whether the
    /// relay accepted the message.
    pub async fn send_smtp_test(&self, to: &str) -> Result<(), AppError> {
        let html = String::from(
            "<p>This is a test message from Nimbus.</p>\
             <p>If you received it, SMTP is configured correctly.</p>",
        );
        let (tx, rx) = oneshot::channel();
        self.enqueue(to, "Nimbus SMTP test", html, Some(tx)).await?;
        rx.await.map_err(|_| AppError::Internal)?
    }

    /// Validate the recipient and enqueue delivery on the background worker.
    async fn enqueue(
        &self,
        to: &str,
        subject: &str,
        html: String,
        reply: Option<oneshot::Sender<Result<(), AppError>>>,
    ) -> Result<(), AppError> {
        // Fail fast on bad addresses so API callers still see Unprocessable.
        let _ = to.parse::<Mailbox>().map_err(|error| {
            tracing::error!(%error, "invalid recipient address");
            AppError::Unprocessable("Invalid email address".into())
        })?;

        self.queue
            .send(EmailCommand::Send(EmailRequest {
                to: to.to_string(),
                subject: subject.to_string(),
                html,
                reply,
            }))
            .await
            .map_err(|_| AppError::Internal)
    }
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
