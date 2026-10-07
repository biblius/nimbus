use std::{str::FromStr, sync::Arc};

use axum::http::HeaderName;
use tracing_error::ErrorLayer;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt};

use crate::{
    auth::{service::AuthenticationService, token::TokenStore},
    cache::Cache,
    config::StartArgs,
    db::{
        self, FileRepository, SessionRepository, Transactor, UserFolderRepository, UserRepository,
    },
    email::EmailSender,
    file::{service::FileService, storage::Storage},
    user::service::UserService,
};

#[derive(Clone)]
pub struct AppState {
    pub services: ServiceState,
    pub http_config: HttpConfiguration,
    pub upload_max_bytes: usize,
}

#[derive(Clone)]
pub struct ServiceState {
    pub auth: AuthenticationService,
    pub users: UserService,
    pub files: FileService,
}

#[derive(Debug, Clone)]
pub struct HttpConfiguration {
    pub cors_origins: Arc<[String]>,
    pub cors_headers: Arc<[String]>,
    pub cookie_domain: Arc<str>,
    pub cookie_name: Arc<str>,
    pub cookie_secure: bool,
    pub csrf_header: Arc<HeaderName>,
}

impl AppState {
    pub async fn new(args: &StartArgs) -> Self {
        let subscriber = tracing_subscriber::Registry::default()
            .with(EnvFilter::from(args.log()))
            .with(tracing_subscriber::fmt::layer())
            .with(ErrorLayer::default());

        if let Err(error) = tracing::subscriber::set_global_default(subscriber) {
            eprintln!("Unable to set tracing subscriber: {error}");
            eprintln!("Application will not contain logs!");
        }

        let db = db::connect_with_retry(&args.db_url()).await;
        db::migrate(&db)
            .await
            .expect("unable to migrate the database");

        #[cfg(feature = "valkey")]
        let cache = Cache::connect(&args.valkey_url()).expect("unable to create the valkey pool");
        #[cfg(feature = "memory-cache")]
        let cache = Cache::new();

        let storage = Storage::new(args.upload_dir().into(), args.upload_max_bytes());
        let email = EmailSender::new(
            args.gmail_address(),
            args.gmail_app_password(),
            args.mail_from_name(),
            args.frontend_url(),
        );

        let users = UserRepository::new(db.clone());
        let sessions = SessionRepository::new(db.clone());
        let folders = UserFolderRepository::new(db.clone());
        let files = FileRepository::new(db.clone());
        let tokens = TokenStore::new(cache);
        let transactor = Transactor::new(db);

        let auth =
            AuthenticationService::new(users.clone(), sessions.clone(), tokens, email.clone());

        let user_service = UserService::new(
            users,
            files.clone(),
            folders.clone(),
            sessions,
            email,
            transactor.clone(),
        );

        let file_service = FileService::new(
            files,
            folders,
            storage,
            args.file_retention_days(),
            transactor,
        );

        file_service
            .prepare()
            .await
            .expect("unable to prepare file storage");

        user_service
            .ensure_initial_admin()
            .await
            .expect("unable to seed the initial admin");

        file_service
            .purge_expired()
            .await
            .expect("unable to purge expired files");

        let http_config = HttpConfiguration {
            cors_origins: Arc::from(args.allowed_origins()),
            cors_headers: Arc::from(args.allowed_headers()),
            cookie_domain: args.cookie_domain().into(),
            cookie_name: args.cookie_name().into(),
            cookie_secure: args.cookie_secure(),
            csrf_header: Arc::new(
                HeaderName::from_str(&args.csrf_header())
                    .expect("invalid header name provider for CSRF header"),
            ),
        };

        Self {
            services: ServiceState {
                auth,
                users: user_service,
                files: file_service,
            },
            http_config,
            upload_max_bytes: args.upload_max_bytes(),
        }
    }
}

#[cfg(test)]
impl Default for HttpConfiguration {
    fn default() -> Self {
        HttpConfiguration {
            cors_origins: Arc::from([String::from("http://localhost:5173")]),
            cors_headers: Arc::from([
                String::from("Authorization"),
                String::from("Content-Type"),
                String::from("X-NIMBUS-CSRF"),
            ]),
            cookie_domain: "".into(),
            cookie_name: "nimbus_session".into(),
            cookie_secure: false,
            csrf_header: Arc::new(HeaderName::from_static("X-NIMBUS-CSRF")),
        }
    }
}
