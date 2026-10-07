use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    message::{Mailbox, header::ContentType},
    transport::smtp::authentication::Credentials,
};
use uuid::Uuid;

use crate::error::AppError;

/// Gmail SMTP sender. An empty app password disables delivery and logs instead.
#[derive(Clone)]
pub struct EmailSender {
    mailer: Option<AsyncSmtpTransport<Tokio1Executor>>,
    from: Mailbox,
    frontend_url: String,
}

impl EmailSender {
    pub fn new(
        address: String,
        app_password: String,
        from_name: String,
        frontend_url: String,
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

        Self {
            mailer,
            from,
            frontend_url: frontend_url.trim_end_matches('/').to_string(),
        }
    }

    pub async fn send_account_created(&self, to: &str, username: &str) -> Result<(), AppError> {
        let username = escape(username);
        let html = format!(
            "<p>An administrator created a Nimbus account for <strong>{username}</strong>.</p>\
             <p>Sign in and change the password you were given.</p>"
        );
        self.send(to, "Your Nimbus account", &html).await
    }

    pub async fn send_password_reset(
        &self,
        to: &str,
        username: &str,
        token: Uuid,
    ) -> Result<(), AppError> {
        let link = format!("{}/reset/{token}", self.frontend_url);
        let username = escape(username);
        let html = format!(
            "<p>{username}, a password reset was requested for your Nimbus account.</p>\
             <p><a href=\"{link}\">Choose a new password</a></p>\
             <p>If you did not ask for this, you can ignore the message.</p>"
        );
        if self.mailer.is_none() {
            tracing::info!(%link, "gmail is not configured; password reset link");
        }
        self.send(to, "Reset your Nimbus password", &html).await
    }

    pub async fn send_password_changed(&self, to: &str, username: &str) -> Result<(), AppError> {
        let username = escape(username);
        let html = format!("<p>{username}, your Nimbus password was changed.</p>");
        self.send(to, "Nimbus password changed", &html).await
    }

    async fn send(&self, to: &str, subject: &str, html: &str) -> Result<(), AppError> {
        let Some(mailer) = &self.mailer else {
            tracing::info!(to, subject, "gmail is not configured; email was not sent");
            return Ok(());
        };

        let to = to.parse::<Mailbox>().map_err(|error| {
            tracing::error!(%error, "invalid recipient address");
            AppError::Unprocessable("invalid email address".into())
        })?;

        let message = Message::builder()
            .from(self.from.clone())
            .to(to)
            .subject(subject)
            .header(ContentType::TEXT_HTML)
            .body(html.to_string())
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

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
