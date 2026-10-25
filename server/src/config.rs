use clap::Parser;
use std::fmt;

#[derive(Debug, Parser)]
#[command(
    name = "nimbus",
    author = "barrage",
    version = "0.1.0",
    about = "Self-hosted file sharing",
    long_about = None
)]
pub struct StartArgs {
    /// Address to listen on.
    #[arg(long, short)]
    address: Option<String>,

    /// RUST_LOG string used to configure the log filter.
    #[arg(long, short)]
    log: Option<String>,

    /// CORS allowed origins.
    #[arg(long)]
    cors_allowed_origins: Option<String>,

    /// CORS allowed headers.
    #[arg(long)]
    cors_allowed_headers: Option<String>,

    /// Database URL.
    #[arg(long)]
    db_url: Option<String>,

    /// Valkey URL. Valkey speaks the Redis protocol. Empty uses in-memory cache only.
    #[arg(long)]
    valkey_url: Option<String>,

    /// Upload directory where all files end up.
    #[arg(long)]
    upload_dir: Option<String>,

    /// Directory for generated file previews (content-addressed by hash).
    #[arg(long)]
    preview_dir: Option<String>,

    /// Directory for ephemeral files (uploads in progress, zip downloads).
    #[arg(long)]
    tmp_dir: Option<String>,

    /// Days a soft-deleted file stays recoverable.
    #[arg(long)]
    file_retention_days: Option<String>,

    /// How often the deletion worker sweeps expired trash, in seconds.
    #[arg(long)]
    file_deletion_interval_secs: Option<String>,

    /// Maximum accepted upload body, in bytes.
    #[arg(long)]
    upload_max_bytes: Option<String>,

    /// SMTP relay hostname. Empty disables sending.
    #[arg(long)]
    smtp_host: Option<String>,

    /// SMTP port. Defaults to 587 (STARTTLS). Use 465 for implicit TLS.
    #[arg(long)]
    smtp_port: Option<String>,

    /// SMTP username; also used as the From address.
    #[arg(long)]
    smtp_username: Option<String>,

    /// SMTP password. Empty disables sending.
    #[arg(long)]
    smtp_password: Option<String>,

    #[arg(long)]
    mail_from_name: Option<String>,

    /// Public UI origin (scheme + host + optional port). Used for email links,
    /// WebAuthn RP origin, and native passkey handoff. Not the `/api` prefix.
    #[arg(long)]
    nimbus_origin: Option<String>,

    /// Optional WebAuthn RP ID override. Defaults to the host of `NIMBUS_ORIGIN`.
    /// Must be an effective domain of that origin (e.g. `example.com` for
    /// `https://nimbus.example.com`).
    #[arg(long)]
    webauthn_rp_id: Option<String>,
}

/// Implement a getter method on [StartArgs], using the `$var` environment variable as a fallback
/// and either panic or default if neither the argument nor the environment variable is set.
macro_rules! arg {
    ($id:ident, $var:literal, panic $msg:literal) => {
        impl StartArgs {
            pub fn $id(&self) -> String {
                match &self.$id {
                    Some(val) => val.to_string(),
                    None => match std::env::var($var) {
                        Ok(val) => val,
                        Err(_) => panic!($msg),
                    },
                }
            }
        }
    };
    ($id:ident, $var:literal, default $value:expr) => {
        impl StartArgs {
            pub fn $id(&self) -> String {
                match &self.$id {
                    Some(val) => val.to_string(),
                    None => match std::env::var($var) {
                        Ok(val) => val,
                        Err(_) => $value,
                    },
                }
            }
        }
    };
    ($id:ident, $var:literal, default $value:expr, $t:ty) => {
        impl StartArgs {
            pub fn $id(&self) -> $t {
                match &self.$id {
                    Some(val) => val
                        .parse::<$t>()
                        .unwrap_or_else(|e| panic!("unable to parse value {}: {e}", $var)),
                    None => match std::env::var($var) {
                        Ok(val) => val
                            .parse::<$t>()
                            .unwrap_or_else(|e| panic!("unable to parse value {}: {e}", $var)),
                        Err(_) => $value,
                    },
                }
            }
        }
    };
}

impl StartArgs {
    pub fn allowed_origins(&self) -> Vec<String> {
        self.csv_list(
            &self.cors_allowed_origins,
            "CORS_ALLOWED_ORIGINS",
            &[
                "http://localhost:5173",
                "http://localhost:4173",
                "http://localhost:1420",
                "tauri://localhost",
            ],
        )
    }

    pub fn allowed_headers(&self) -> Vec<String> {
        self.csv_list(
            &self.cors_allowed_headers,
            "CORS_ALLOWED_HEADERS",
            &[
                "Authorization",
                "Content-Type",
                "Accept",
                "X-Requested-With",
            ],
        )
    }

    fn csv_list(&self, arg: &Option<String>, var: &str, default: &[&str]) -> Vec<String> {
        let raw = arg.clone().or_else(|| std::env::var(var).ok());
        match raw {
            Some(value) => value
                .split(',')
                .filter_map(|part| {
                    let part = part.trim();
                    (!part.is_empty()).then_some(part.to_string())
                })
                .collect(),
            None => default.iter().map(|value| (*value).to_string()).collect(),
        }
    }
}

arg!(log, "RUST_LOG", default "info".to_string());
arg!(address, "ADDRESS", default "0.0.0.0:8080".to_string());

#[cfg(feature = "postgres")]
arg!(db_url, "DATABASE_URL", panic "Database url not found; Pass --db-url or set DATABASE_URL");

#[cfg(feature = "sqlite")]
arg!(
    db_url,
    "DATABASE_URL",
    default "sqlite://./data/nimbus.db?mode=rwc".to_string()
);

arg!(valkey_url, "VALKEY_URL", default String::new());

arg!(upload_dir, "UPLOAD_DIR", default "./data".to_string());
arg!(preview_dir, "PREVIEW_DIR", default "./preview".to_string());
arg!(tmp_dir, "TMP_DIR", default "./tmp".to_string());
arg!(file_retention_days, "FILE_RETENTION_DAYS", default 30, u16);
arg!(
    file_deletion_interval_secs,
    "FILE_DELETION_INTERVAL_SECS",
    default 30,
    u64
);
arg!(
    upload_max_bytes,
    "UPLOAD_MAX_BYTES",
    default 1024usize.pow(3) * 10, // 10 GiB
    usize
);

arg!(smtp_host, "SMTP_HOST", default String::new());
arg!(smtp_port, "SMTP_PORT", default 587, u16);
arg!(smtp_username, "SMTP_USERNAME", default String::new());
arg!(smtp_password, "SMTP_PASSWORD", default String::new());
arg!(mail_from_name, "MAIL_FROM_NAME", default "Nimbus".to_string());
arg!(nimbus_origin, "NIMBUS_ORIGIN", default "http://localhost:5173".to_string());
arg!(webauthn_rp_id, "WEBAUTHN_RP_ID", default String::new());

/// Shows every resolved setting (CLI argument, environment variable, or default), so the
/// effective configuration can be logged on startup. Secrets are redacted.
impl fmt::Display for StartArgs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn redact(value: &str) -> &str {
            if value.is_empty() {
                "<unset>"
            } else {
                "<redacted>"
            }
        }

        fn show(value: &str) -> &str {
            if value.is_empty() { "<unset>" } else { value }
        }

        writeln!(f, "address: {}", self.address())?;
        writeln!(f, "log: {}", self.log())?;
        #[cfg(debug_assertions)]
        {
            writeln!(
                f,
                "cors_allowed_origins: {}",
                self.allowed_origins().join(", ")
            )?;
            writeln!(
                f,
                "cors_allowed_headers: {}",
                self.allowed_headers().join(", ")
            )?;
        }
        #[cfg(debug_assertions)]
        writeln!(f, "db_url: {}", self.db_url())?;
        writeln!(f, "valkey_url: {}", show(&self.valkey_url()))?;
        writeln!(f, "upload_dir: {}", self.upload_dir())?;
        writeln!(f, "preview_dir: {}", self.preview_dir())?;
        writeln!(f, "tmp_dir: {}", self.tmp_dir())?;
        writeln!(f, "file_retention_days: {}", self.file_retention_days())?;
        writeln!(
            f,
            "file_deletion_interval_secs: {}",
            self.file_deletion_interval_secs()
        )?;
        writeln!(
            f,
            "upload_max_bytes: {} GiB",
            self.upload_max_bytes() as f64 / 1024. / 1024. / 1024.
        )?;
        writeln!(f, "smtp_host: {}", show(&self.smtp_host()))?;
        writeln!(f, "smtp_port: {}", self.smtp_port())?;
        writeln!(f, "smtp_username: {}", show(&self.smtp_username()))?;
        writeln!(f, "smtp_password: {}", redact(&self.smtp_password()))?;
        writeln!(f, "mail_from_name: {}", self.mail_from_name())?;
        writeln!(f, "nimbus_origin: {}", self.nimbus_origin())?;
        let rp_id = self.webauthn_rp_id();
        write!(
            f,
            "webauthn_rp_id: {}",
            if rp_id.is_empty() {
                "<from nimbus_origin host>"
            } else {
                &rp_id
            }
        )
    }
}
