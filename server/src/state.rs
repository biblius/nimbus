use std::{str::FromStr, sync::Arc};

use axum::http::HeaderName;
use tracing_error::ErrorLayer;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt};

use crate::{
    auth::{service::AuthenticationService, token::TokenStore},
    cache::Cache,
    config::StartArgs,
    db::{self, Repository, Transactor},
    email::EmailSender,
    file::{
        queue::{self, DeletionQueue, DeletionQueueHandle, ProcessingQueue},
        service::FileService,
        storage::Storage,
    },
    user::service::UserService,
};

#[derive(Clone)]
pub struct AppState {
    pub services: ServiceState,
    pub queue: DeletionQueueHandle,
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

        let storage = Storage::new(
            args.upload_dir().into(),
            args.preview_dir().into(),
            args.upload_max_bytes(),
        );
        let email = EmailSender::new(
            args.gmail_address(),
            args.gmail_app_password(),
            args.mail_from_name(),
            args.frontend_url(),
        );

        let repository = Repository::new(db.clone());
        let tokens = TokenStore::new(cache);

        let transactor = Transactor::new(db);

        let auth = AuthenticationService::new(
            repository.users.clone(),
            repository.sessions.clone(),
            tokens,
            email.clone(),
        );

        let user_service = UserService::new(
            repository.users.clone(),
            repository.files.clone(),
            repository.folders.clone(),
            repository.sessions.clone(),
            email,
            transactor.clone(),
            repository.deletions.clone(),
        );

        let processing = ProcessingQueue::start(storage.clone(), repository.metadata.clone());

        let file_service = FileService::new(
            repository.files.clone(),
            repository.folders.clone(),
            repository.metadata.clone(),
            storage.clone(),
            processing,
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

        let services = ServiceState {
            auth,
            users: user_service,
            files: file_service,
        };

        let retention_days = args.file_retention_days();
        let expiry_files = repository.files.clone();
        let expiry_deletions = repository.deletions.clone();
        let queue = DeletionQueue::start(
            repository,
            storage,
            std::time::Duration::from_secs(args.file_deletion_interval_secs()),
        );
        tokio::spawn(queue::expiry::start(
            expiry_files,
            expiry_deletions,
            retention_days,
        ));

        Self {
            services,
            queue,
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
