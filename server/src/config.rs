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

    /// Name of the session cookie.
    #[arg(long)]
    cookie_name: Option<String>,

    /// Domain of the session cookie. Empty keeps the cookie host-only.
    #[arg(long)]
    cookie_domain: Option<String>,

    /// Set the session cookie Secure attribute.
    #[arg(long)]
    cookie_secure: Option<String>,

    /// Database URL.
    #[arg(long)]
    db_url: Option<String>,

    /// Valkey URL. Valkey speaks the Redis protocol.
    #[cfg(feature = "valkey")]
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

    /// Gmail address used as the SMTP username and From address.
    #[arg(long)]
    gmail_address: Option<String>,

    /// Gmail app password. Empty disables sending.
    #[arg(long)]
    gmail_app_password: Option<String>,

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
                "http://tauri.localhost",
                "https://tauri.localhost",
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

arg!(cookie_name, "COOKIE_NAME", default "nimbus_session".to_string());
arg!(cookie_domain, "COOKIE_DOMAIN", default String::new());
arg!(cookie_secure, "COOKIE_SECURE", default true, bool);

#[cfg(feature = "postgres")]
arg!(db_url, "DATABASE_URL", panic "Database url not found; Pass --db-url or set DATABASE_URL");

#[cfg(feature = "sqlite")]
arg!(
    db_url,
    "DATABASE_URL",
    default "sqlite://./data/nimbus.db?mode=rwc".to_string()
);

#[cfg(feature = "valkey")]
arg!(
    valkey_url,
    "VALKEY_URL",
    default "redis://127.0.0.1:6379".to_string()
);

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

arg!(gmail_address, "GMAIL_ADDRESS", default String::new());
arg!(gmail_app_password, "GMAIL_APP_PASSWORD", default String::new());
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
        writeln!(f, "cookie_name: {}", self.cookie_name())?;
        writeln!(f, "cookie_domain: {}", show(&self.cookie_domain()))?;
        writeln!(f, "cookie_secure: {}", self.cookie_secure())?;
        #[cfg(debug_assertions)]
        writeln!(f, "db_url: {}", self.db_url())?;
        #[cfg(feature = "valkey")]
        writeln!(f, "valkey_url: {}", self.valkey_url())?;
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
        writeln!(f, "gmail_address: {}", show(&self.gmail_address()))?;
        writeln!(
            f,
            "gmail_app_password: {}",
            redact(&self.gmail_app_password())
        )?;
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
