use super::{ConfigUpdate, Configuration, NimbusConfig};
use crate::error::AppError;
use validify::Validify;

/// Admin-facing configuration API.
#[derive(Clone)]
pub struct ConfigurationService {
    config: NimbusConfig,
}

impl ConfigurationService {
    pub fn new(config: NimbusConfig) -> Self {
        Self { config }
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

        Ok(self.config.update(update).await?.redacted())
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
        let service = ConfigurationService::new(NimbusConfig::in_memory(config));

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
