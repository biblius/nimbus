use super::{ConfigUpdate, Configuration, NimbusConfig};
use crate::{email::EmailSender, error::AppError};
use validify::Validify;

/// Admin-facing configuration API.
#[derive(Clone)]
pub struct ConfigurationService {
    config: NimbusConfig,
    email: EmailSender,
}

impl ConfigurationService {
    pub fn new(config: NimbusConfig, email: EmailSender) -> Self {
        Self { config, email }
    }

    #[cfg(test)]
    pub fn inner(&self) -> &NimbusConfig {
        &self.config
    }

    pub async fn get(&self) -> Configuration {
        self.config.get().await.redacted()
    }

    pub async fn update(&self, mut update: ConfigUpdate) -> Result<Configuration, AppError> {
        update.validify()?;

        if let Some(ref origin) = update.nimbus_origin {
            let trimmed = origin.trim_end_matches('/');
            if let Err(error) = webauthn_rs::prelude::Url::parse(trimmed) {
                tracing::warn!(%error, origin = trimmed, "invalid nimbus origin");
                return Err(AppError::Unprocessable("Invalid nimbus origin URL".into()));
            }
            update.nimbus_origin = Some(trimmed.to_string());
        }

        let updated = self.config.update(update).await?;
        self.email.reconfigure().await?;
        Ok(updated.redacted())
    }

    /// Deliver a test message to `to` using the current SMTP settings.
    pub async fn send_test_email(&self, to: &str) -> Result<(), AppError> {
        if !self.config.get().await.smtp_configured() {
            return Err(AppError::Unprocessable(
                "SMTP is not fully configured".into(),
            ));
        }

        self.email.send_smtp_test(to).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Configuration {
        Configuration {
            file_retention_days: 30,
            smtp_host: String::new(),
            smtp_port: 587,
            smtp_username: String::new(),
            smtp_password: String::new(),
            email_from: "Nimbus".into(),
            upload_max_bytes: 1024,
            nimbus_origin: "http://localhost:5173".into(),
        }
    }

    #[tokio::test]
    async fn configuration_service_redacts_password() {
        let mut config = sample();
        config.smtp_password = "secret".into();
        let nimbus = NimbusConfig::in_memory(config);
        let email = crate::email::EmailSender::new(nimbus.clone()).await;
        let service = ConfigurationService::new(nimbus, email);

        let view = service.get().await;
        assert_eq!(view.smtp_password, "<redacted>");

        let updated = service
            .update(ConfigUpdate {
                smtp_password: Some("new-secret".into()),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(updated.smtp_password, "<redacted>");
        assert_eq!(service.inner().get().await.smtp_password, "new-secret");
    }
}
