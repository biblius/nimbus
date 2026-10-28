mod service;
mod start;

pub use service::ConfigurationService;
pub use start::StartArgs;

use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::RwLock;
use validify::Validify;
use webauthn_rs::prelude::{Url, Webauthn, WebauthnBuilder};

struct ConfigInner {
    settings: Configuration,
    webauthn: Webauthn,
}

/// Shared mutable handle around [`Configuration`] and the live WebAuthn authenticator.
#[derive(Clone)]
pub struct NimbusConfig {
    inner: Arc<RwLock<ConfigInner>>,
    path: Arc<PathBuf>,
    /// Optional `WEBAUTHN_RP_ID` override from env/CLI (fixed for the process lifetime).
    webauthn_rp_id: Arc<str>,
}

impl NimbusConfig {
    /// Seed from start args, then overlay `config.json` when present.
    pub async fn load(args: &StartArgs, path: PathBuf) -> Self {
        let mut settings = Configuration::from_args(args);

        match tokio::fs::read_to_string(&path).await {
            Ok(raw) => match serde_json::from_str::<Configuration>(&raw) {
                Ok(admin) => settings.apply_admin_overrides(admin),
                Err(error) => {
                    tracing::error!(%error, path = %path.display(), "invalid config.json; using env/CLI defaults");
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                tracing::error!(%error, path = %path.display(), "unable to read config.json; using env/CLI defaults");
            }
        }

        let webauthn_rp_id = args.webauthn_rp_id();
        let webauthn =
            build_webauthn(&settings.nimbus_origin, &webauthn_rp_id).unwrap_or_else(|error| {
                panic!(
                    "invalid WebAuthn configuration (origin={}, rp_id={}): {error}",
                    settings.nimbus_origin,
                    if webauthn_rp_id.is_empty() {
                        "<from origin host>"
                    } else {
                        &webauthn_rp_id
                    }
                )
            });

        Self {
            inner: Arc::new(RwLock::new(ConfigInner { settings, webauthn })),
            path: Arc::new(path),
            webauthn_rp_id: Arc::from(webauthn_rp_id),
        }
    }

    /// In-memory only (tests).
    #[cfg(test)]
    pub fn in_memory(config: Configuration) -> Self {
        let webauthn = build_webauthn(&config.nimbus_origin, "")
            .unwrap_or_else(|error| panic!("invalid test WebAuthn configuration: {error}"));

        Self {
            inner: Arc::new(RwLock::new(ConfigInner {
                settings: config,
                webauthn,
            })),
            path: Arc::new(PathBuf::from("/dev/null")),
            webauthn_rp_id: Arc::from(""),
        }
    }

    pub async fn get(&self) -> Configuration {
        self.inner.read().await.settings.clone()
    }

    pub async fn file_retention_days(&self) -> u16 {
        self.inner.read().await.settings.file_retention_days
    }

    pub async fn upload_max_bytes(&self) -> usize {
        self.inner.read().await.settings.upload_max_bytes
    }

    pub async fn nimbus_origin(&self) -> String {
        self.inner.read().await.settings.nimbus_origin.clone()
    }

    pub async fn webauthn(&self) -> Webauthn {
        self.inner.read().await.webauthn.clone()
    }

    pub async fn update(&self, update: ConfigUpdate) -> Result<Configuration, AppError> {
        let snapshot = {
            let mut guard = self.inner.write().await;
            let mut next = guard.settings.clone();
            update.apply(&mut next);

            if next.nimbus_origin != guard.settings.nimbus_origin {
                guard.webauthn = build_webauthn(&next.nimbus_origin, &self.webauthn_rp_id)?;
                tracing::info!(
                    origin = %next.nimbus_origin,
                    "reinitialized WebAuthn after NIMBUS_ORIGIN change"
                );
            }

            guard.settings = next;
            guard.settings.clone()
        };
        self.persist(&snapshot).await?;
        Ok(snapshot)
    }

    async fn persist(&self, config: &Configuration) -> Result<(), AppError> {
        if self.path.as_os_str() == "/dev/null" {
            return Ok(());
        }

        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|error| {
                tracing::error!(%error, path = %parent.display(), "creating config directory");
                AppError::Internal
            })?;
        }

        let json = serde_json::to_string_pretty(config).map_err(|error| {
            tracing::error!(%error, "serializing config.json");
            AppError::Internal
        })?;

        tokio::fs::write(self.path.as_ref(), json)
            .await
            .map_err(|error| {
                tracing::error!(%error, path = %self.path.display(), "writing config.json");
                AppError::Internal
            })
    }
}

fn build_webauthn(origin: &str, rp_id_override: &str) -> Result<Webauthn, AppError> {
    let rp_origin = Url::parse(origin).map_err(|error| {
        tracing::warn!(%error, origin, "invalid nimbus origin for WebAuthn");
        AppError::Unprocessable("Invalid nimbus origin URL".into())
    })?;

    let rp_id = if rp_id_override.is_empty() {
        rp_origin
            .host_str()
            .ok_or_else(|| {
                tracing::warn!(origin, "nimbus origin has no host");
                AppError::Unprocessable("Nimbus origin must include a host".into())
            })?
            .to_string()
    } else {
        rp_id_override.to_string()
    };

    WebauthnBuilder::new(&rp_id, &rp_origin)
        .map_err(|error| {
            tracing::error!(%error, %rp_id, %rp_origin, "invalid WebAuthn builder config");
            AppError::Unprocessable("Invalid WebAuthn configuration".into())
        })?
        .rp_name("Nimbus")
        .build()
        .map_err(|error| {
            tracing::error!(%error, %rp_id, %rp_origin, "unable to build WebAuthn");
            AppError::Internal
        })
}

/// Runtime settings seeded from env/CLI and optionally overridden by `config.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Configuration {
    pub file_retention_days: u16,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_username: String,
    pub smtp_password: String,
    pub email_from: String,
    pub upload_max_bytes: usize,
    pub nimbus_origin: String,
}

impl Configuration {
    fn from_args(args: &StartArgs) -> Self {
        Self {
            file_retention_days: args.file_retention_days(),
            smtp_host: args.smtp_host(),
            smtp_port: args.smtp_port(),
            smtp_username: args.smtp_username(),
            smtp_password: args.smtp_password(),
            email_from: args.email_from(),
            upload_max_bytes: args.upload_max_bytes(),
            nimbus_origin: args.nimbus_origin().trim_end_matches('/').to_string(),
        }
    }

    /// Apply admin-persisted values, logging each overridden env variable name.
    pub fn apply_admin_overrides(&mut self, admin: Self) {
        if self.file_retention_days != admin.file_retention_days {
            tracing::info!("overriding FILE_RETENTION_DAYS from application config");
            self.file_retention_days = admin.file_retention_days;
        }
        if self.smtp_host != admin.smtp_host {
            tracing::info!("overriding SMTP_HOST from application config");
            self.smtp_host = admin.smtp_host;
        }
        if self.smtp_port != admin.smtp_port {
            tracing::info!("overriding SMTP_PORT from application config");
            self.smtp_port = admin.smtp_port;
        }
        if self.smtp_username != admin.smtp_username {
            tracing::info!("overriding SMTP_USERNAME from application config");
            self.smtp_username = admin.smtp_username;
        }
        if self.smtp_password != admin.smtp_password {
            tracing::info!("overriding SMTP_PASSWORD from application config");
            self.smtp_password = admin.smtp_password;
        }
        if self.email_from != admin.email_from {
            tracing::info!("overriding EMAIL_FROM from application config");
            self.email_from = admin.email_from;
        }
        if self.upload_max_bytes != admin.upload_max_bytes {
            tracing::info!("overriding UPLOAD_MAX_BYTES from application config");
            self.upload_max_bytes = admin.upload_max_bytes;
        }
        let admin_origin = admin.nimbus_origin.trim_end_matches('/').to_string();
        if self.nimbus_origin != admin_origin {
            tracing::info!("overriding NIMBUS_ORIGIN from application config");
            self.nimbus_origin = admin_origin;
        }
    }

    pub fn redacted(&self) -> Self {
        let mut clone = self.clone();
        if !clone.smtp_password.is_empty() {
            clone.smtp_password = String::from("<redacted>");
        }
        clone
    }

    pub fn smtp_configured(&self) -> bool {
        !self.smtp_host.is_empty()
            && !self.smtp_username.is_empty()
            && !self.smtp_password.is_empty()
    }
}

/// Partial update: only non-null fields replace the current value.
#[derive(Debug, Default, Clone, Deserialize, Validify)]
#[serde(rename_all = "camelCase")]
pub struct ConfigUpdate {
    #[validate(range(min = 1., message = "Retention days must be at least 1"))]
    pub file_retention_days: Option<u16>,

    pub smtp_host: Option<String>,

    pub smtp_port: Option<u16>,

    pub smtp_username: Option<String>,

    pub smtp_password: Option<String>,

    #[validate(length(min = 1, message = "Email from name cannot be empty"))]
    pub email_from: Option<String>,

    #[validate(range(min = 1., message = "Upload max bytes must be at least 1"))]
    pub upload_max_bytes: Option<usize>,

    #[validate(length(min = 1, message = "Nimbus origin cannot be empty"))]
    pub nimbus_origin: Option<String>,
}

impl ConfigUpdate {
    pub fn apply(self, config: &mut Configuration) {
        if let Some(value) = self.file_retention_days {
            config.file_retention_days = value;
        }
        if let Some(value) = self.smtp_host {
            config.smtp_host = value;
        }
        if let Some(value) = self.smtp_port {
            config.smtp_port = value;
        }
        if let Some(value) = self.smtp_username {
            config.smtp_username = value;
        }
        if let Some(value) = self.smtp_password {
            config.smtp_password = value;
        }
        if let Some(value) = self.email_from {
            config.email_from = value;
        }
        if let Some(value) = self.upload_max_bytes {
            config.upload_max_bytes = value;
        }
        if let Some(value) = self.nimbus_origin {
            config.nimbus_origin = value.trim_end_matches('/').to_string();
        }
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

    #[test]
    fn config_update_applies_only_set_fields() {
        let mut config = sample();
        ConfigUpdate {
            file_retention_days: Some(7),
            email_from: Some("Admin".into()),
            nimbus_origin: Some("https://files.example.com/".into()),
            ..Default::default()
        }
        .apply(&mut config);

        assert_eq!(config.file_retention_days, 7);
        assert_eq!(config.email_from, "Admin");
        assert_eq!(config.nimbus_origin, "https://files.example.com");
        assert_eq!(config.smtp_port, 587);
        assert_eq!(config.upload_max_bytes, 1024);
    }

    #[tokio::test]
    async fn nimbus_config_update_swaps_values() {
        let inner = NimbusConfig::in_memory(sample());
        assert_eq!(inner.file_retention_days().await, 30);

        inner
            .update(ConfigUpdate {
                file_retention_days: Some(3),
                upload_max_bytes: Some(4096),
                ..Default::default()
            })
            .await
            .unwrap();

        assert_eq!(inner.file_retention_days().await, 3);
        assert_eq!(inner.upload_max_bytes().await, 4096);
        assert_eq!(inner.get().await.email_from, "Nimbus");
    }

    #[tokio::test]
    async fn nimbus_config_reinitializes_webauthn_on_origin_change() {
        let config = NimbusConfig::in_memory(sample());
        let before = config.webauthn().await;
        assert_eq!(
            before.get_allowed_origins()[0].as_str(),
            "http://localhost:5173/"
        );

        config
            .update(ConfigUpdate {
                nimbus_origin: Some("http://localhost:4173".into()),
                ..Default::default()
            })
            .await
            .unwrap();

        let after = config.webauthn().await;
        assert_eq!(
            after.get_allowed_origins()[0].as_str(),
            "http://localhost:4173/"
        );
    }

    #[tokio::test]
    async fn nimbus_config_loads_and_persists_config_json() {
        let dir = std::env::temp_dir().join(format!("nimbus-config-{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let path = dir.join("config.json");

        let admin = Configuration {
            file_retention_days: 9,
            smtp_host: "mail.test".into(),
            smtp_port: 2525,
            smtp_username: "user@test".into(),
            smtp_password: "pw".into(),
            email_from: "Tester".into(),
            upload_max_bytes: 512,
            nimbus_origin: "http://config.test".into(),
        };
        tokio::fs::write(&path, serde_json::to_string_pretty(&admin).unwrap())
            .await
            .unwrap();

        let args = StartArgs {
            address: None,
            log: None,
            #[cfg(debug_assertions)]
            cors_allowed_origins: None,
            #[cfg(debug_assertions)]
            cors_allowed_headers: None,
            db_url: None,
            valkey_url: None,
            upload_dir: Some(dir.to_string_lossy().into()),
            file_retention_days: Some("30".into()),
            upload_max_bytes: Some("1024".into()),
            smtp_host: None,
            smtp_port: None,
            smtp_username: None,
            smtp_password: None,
            email_from: Some("Nimbus".into()),
            nimbus_origin: Some("http://localhost:5173".into()),
            webauthn_rp_id: None,
        };

        let loaded = NimbusConfig::load(&args, path.clone()).await;
        assert_eq!(loaded.get().await, admin);
        assert_eq!(
            loaded.webauthn().await.get_allowed_origins()[0].as_str(),
            "http://config.test/"
        );

        loaded
            .update(ConfigUpdate {
                file_retention_days: Some(11),
                ..Default::default()
            })
            .await
            .unwrap();

        let raw = tokio::fs::read_to_string(&path).await.unwrap();
        let persisted: Configuration = serde_json::from_str(&raw).unwrap();
        assert_eq!(persisted.file_retention_days, 11);
        assert_eq!(persisted.smtp_host, "mail.test");

        let _ = tokio::fs::remove_dir_all(&dir).await;
    }
}
