use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    message::{Mailbox, header::ContentType},
    transport::smtp::authentication::Credentials,
};
use tokio::sync::mpsc;
use tracing::instrument;
use uuid::Uuid;

use crate::error::AppError;

struct EmailRequest {
    to: String,
    subject: String,
    html: String,
}

type EmailQueueHandle = mpsc::Sender<EmailRequest>;

/// Background worker that delivers queued outbound messages over SMTP.
struct EmailQueue {
    requests: mpsc::Receiver<EmailRequest>,
    mailer: Option<AsyncSmtpTransport<Tokio1Executor>>,
    from: Mailbox,
}

impl EmailQueue {
    fn start(
        mailer: Option<AsyncSmtpTransport<Tokio1Executor>>,
        from: Mailbox,
    ) -> EmailQueueHandle {
        let (tx, rx) = mpsc::channel(128);
        EmailQueue::new(mailer, from, rx)._start();
        tx
    }

    fn new(
        mailer: Option<AsyncSmtpTransport<Tokio1Executor>>,
        from: Mailbox,
        requests: mpsc::Receiver<EmailRequest>,
    ) -> Self {
        Self {
            requests,
            mailer,
            from,
        }
    }

    #[instrument(skip(self))]
    fn _start(mut self) {
        tokio::spawn(async move {
            while let Some(request) = self.requests.recv().await {
                if let Err(e) = self.deliver(request).await {
                    tracing::error!(error=%e, "error sending email");
                }
            }

            tracing::info!("Email request channel closed, shutting down worker");
        });
    }

    #[instrument(skip(self, request), fields(to = %request.to, subject = %request.subject), err(Debug))]
    async fn deliver(&self, request: EmailRequest) -> Result<(), AppError> {
        let Some(mailer) = &self.mailer else {
            tracing::info!(
                to = %request.to,
                subject = %request.subject,
                "gmail is not configured; email was not sent"
            );
            return Ok(());
        };

        let to = request.to.parse::<Mailbox>().map_err(|error| {
            tracing::error!(%error, "invalid recipient address");
            AppError::Unprocessable("Invalid email address".into())
        })?;

        let message = Message::builder()
            .from(self.from.clone())
            .to(to)
            .subject(request.subject)
            .header(ContentType::TEXT_HTML)
            .body(request.html)
            .map_err(|error| {
                tracing::error!(%error, "building email");
                AppError::Internal
            })?;

        mailer.send(message).await.map_err(|error| {
            tracing::error!(%error, "gmail smtp send failed");
            AppError::Internal
        })?;
        Ok(())
    }
}

/// Gmail SMTP sender. An empty app password disables delivery and logs instead.
///
/// Delivery runs on a background queue so callers are not blocked on SMTP latency.
#[derive(Clone)]
pub struct EmailSender {
    queue: EmailQueueHandle,
    nimbus_origin: String,
    #[cfg(debug_assertions)]
    mailer_configured: bool,
}

impl EmailSender {
    pub fn new(
        address: String,
        app_password: String,
        from_name: String,
        nimbus_origin: String,
    ) -> Self {
        let parsed = if address.is_empty() {
            "nimbus@localhost".parse()
        } else {
            address.parse()
        };

        let from = match parsed {
            Ok(email) => Mailbox::new(Some(from_name), email),
            Err(error) => {
                tracing::error!(%error, "invalid gmail from address; email is disabled");
                Mailbox::new(None, "nimbus@localhost".parse().expect("static mailbox"))
            }
        };

        let mailer = if address.is_empty() || app_password.is_empty() {
            tracing::info!("gmail is not configured; outbound email is disabled");
            None
        } else {
            match AsyncSmtpTransport::<Tokio1Executor>::starttls_relay("smtp.gmail.com") {
                Ok(transport) => Some(
                    transport
                        .credentials(Credentials::new(address, app_password))
                        .build(),
                ),
                Err(error) => {
                    tracing::error!(%error, "gmail transport could not be built; email is disabled");
                    None
                }
            }
        };

        #[cfg(debug_assertions)]
        let mailer_configured = mailer.is_some();

        let queue = EmailQueue::start(mailer, from);

        Self {
            queue,
            nimbus_origin: nimbus_origin.trim_end_matches('/').to_string(),
            #[cfg(debug_assertions)]
            mailer_configured,
        }
    }

    pub async fn send_account_created(&self, to: &str, username: &str) -> Result<(), AppError> {
        let username = escape(username);
        let html = format!(
            "<p>An administrator created a Nimbus account for <strong>{username}</strong>.</p>\
             <p>Sign in and change the password you were given.</p>"
        );
        self.enqueue(to, "Your Nimbus account", html).await
    }

    pub async fn send_password_reset(
        &self,
        to: &str,
        username: &str,
        token: Uuid,
    ) -> Result<(), AppError> {
        let link = format!("{}/reset-password/{token}", self.nimbus_origin);

        let username = escape(username);

        let html = format!(
            "<p>{username}, a password reset was requested for your Nimbus account.</p>\
             <p><a href=\"{link}\">Choose a new password</a></p>\
             <p>If you did not ask for this, you can ignore the message.</p>"
        );

        #[cfg(debug_assertions)]
        if !self.mailer_configured {
            tracing::info!(%link, "gmail is not configured; password reset link");
        }

        self.enqueue(to, "Reset your Nimbus password", html).await
    }

    pub async fn send_password_changed(&self, to: &str, username: &str) -> Result<(), AppError> {
        let username = escape(username);
        let html = format!("<p>{username}, your Nimbus password was changed.</p>");
        self.enqueue(to, "Nimbus password changed", html).await
    }

    /// Validate the recipient and enqueue delivery on the background worker.
    async fn enqueue(&self, to: &str, subject: &str, html: String) -> Result<(), AppError> {
        // Fail fast on bad addresses so API callers still see Unprocessable.
        let _ = to.parse::<Mailbox>().map_err(|error| {
            tracing::error!(%error, "invalid recipient address");
            AppError::Unprocessable("Invalid email address".into())
        })?;

        self.queue
            .send(EmailRequest {
                to: to.to_string(),
                subject: subject.to_string(),
                html,
            })
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
