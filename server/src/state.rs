#[cfg(debug_assertions)]
use std::sync::Arc;

use tracing_error::ErrorLayer;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt};

use crate::{
    activity::ActivityQueue,
    auth::{login_attempt::LoginAttemptStore, service::AuthenticationService, token::TokenStore},
    cache::Cache,
    config::{ConfigurationService, NimbusConfig, StartArgs},
    db::{self, Repository, Transactor},
    email::EmailSender,
    file::{
        queue::{DeletionQueue, DeletionQueueHandle, ProcessingQueue},
        service::{FileService, ShareService, TrashFileService},
        storage::Storage,
    },
    user::service::UserService,
};

#[derive(Clone)]
pub struct AppState {
    pub services: ServiceState,
    pub queue: DeletionQueueHandle,
    #[cfg(debug_assertions)]
    pub http_config: HttpConfiguration,
}

#[derive(Clone)]
pub struct ServiceState {
    pub auth: AuthenticationService,
    pub users: UserService,
    pub files: FileService,
    pub share: ShareService,
    pub trash: TrashFileService,
    pub config: ConfigurationService,
}

#[cfg(debug_assertions)]
#[derive(Debug, Clone)]
pub struct HttpConfiguration {
    pub cors_origins: Arc<[String]>,
    pub cors_headers: Arc<[String]>,
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

        let valkey_url = args.valkey_url();

        let cache = if valkey_url.is_empty() {
            tracing::info!("VALKEY_URL unset; using in-memory cache");
            Cache::new()
        } else {
            Cache::connect(&valkey_url).expect("unable to create the valkey pool")
        };

        let upload_dir = args.upload_dir();

        let config = NimbusConfig::load(
            args,
            std::path::PathBuf::from(&upload_dir).join("config.json"),
        )
        .await;

        let storage = Storage::new(upload_dir.into(), config.clone());

        let email = EmailSender::new(config.clone()).await;

        let repository = Repository::new(db.clone());
        let tokens = TokenStore::new(cache.clone());
        let login_attempts = LoginAttemptStore::new(cache);

        let transactor = Transactor::new(db);

        let auth = AuthenticationService::new(
            repository.clone(),
            tokens,
            login_attempts,
            email.clone(),
            config.clone(),
        );

        let user_service = UserService::new(
            repository.users.clone(),
            repository.files.clone(),
            repository.trash.clone(),
            repository.folders.clone(),
            repository.sessions.clone(),
            email,
            transactor.clone(),
        );

        let processing = ProcessingQueue::start(storage.clone(), repository.files.clone());
        let activity = ActivityQueue::start(repository.clone());

        let file_service = FileService::new(
            repository.clone(),
            storage.clone(),
            processing,
            activity.clone(),
            transactor.clone(),
        );

        let trash_service = TrashFileService::new(
            repository.trash.clone(),
            repository.files.clone(),
            activity,
            transactor,
        );

        let share_service = ShareService::new(repository.share.clone(), storage.clone());

        file_service
            .prepare()
            .await
            .expect("unable to prepare file storage");

        user_service
            .ensure_initial_admin()
            .await
            .expect("unable to seed the initial admin");

        #[cfg(debug_assertions)]
        let http_config = HttpConfiguration {
            cors_origins: Arc::from(args.allowed_origins()),
            cors_headers: Arc::from(args.allowed_headers()),
        };

        let config_service = ConfigurationService::new(config.clone());

        let services = ServiceState {
            auth,
            users: user_service,
            files: file_service,
            share: share_service,
            trash: trash_service,
            config: config_service,
        };

        // Expired trash is swept every five minutes.
        let queue = DeletionQueue::start(
            repository,
            storage,
            std::time::Duration::from_secs(5 * 60),
            config,
        );

        Self {
            services,
            queue,
            #[cfg(debug_assertions)]
            http_config,
        }
    }
}

#[cfg(all(test, debug_assertions))]
impl Default for HttpConfiguration {
    fn default() -> Self {
        HttpConfiguration {
            cors_origins: Arc::from([String::from("http://localhost:5173")]),
            cors_headers: Arc::from([String::from("Authorization"), String::from("Content-Type")]),
        }
    }
}
