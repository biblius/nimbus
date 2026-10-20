use std::sync::Arc;

use tracing_error::ErrorLayer;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt};

use crate::{
    auth::{login_attempt::LoginAttemptStore, service::AuthenticationService, token::TokenStore},
    cache::Cache,
    config::StartArgs,
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
    pub http_config: HttpConfiguration,
    pub upload_max_bytes: usize,
}

#[derive(Clone)]
pub struct ServiceState {
    pub auth: AuthenticationService,
    pub users: UserService,
    pub files: FileService,
    pub share: ShareService,
    pub trash: TrashFileService,
}

#[derive(Debug, Clone)]
pub struct HttpConfiguration {
    pub cors_origins: Arc<[String]>,
    pub cors_headers: Arc<[String]>,
    pub cookie_domain: Arc<str>,
    pub cookie_name: Arc<str>,
    pub cookie_secure: bool,
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

        let nimbus_origin: Arc<str> = args.nimbus_origin().trim_end_matches('/').into();
        let rp_origin = webauthn_rs::prelude::Url::parse(&nimbus_origin).unwrap_or_else(|error| {
            panic!("invalid NIMBUS_ORIGIN `{nimbus_origin}`: {error}")
        });

        let rp_id_override = args.webauthn_rp_id();
        let rp_id = if rp_id_override.is_empty() {
            rp_origin
                .host_str()
                .unwrap_or_else(|| panic!("NIMBUS_ORIGIN `{nimbus_origin}` has no host"))
                .to_string()
        } else {
            rp_id_override
        };

        let webauthn = webauthn_rs::prelude::WebauthnBuilder::new(&rp_id, &rp_origin)
            .unwrap_or_else(|error| {
                panic!(
                    "invalid WebAuthn configuration (RP_ID={rp_id}, RP_ORIGIN={rp_origin}): {error}"
                )
            })
            .rp_name("Nimbus")
            .build()
            .unwrap_or_else(|error| panic!("unable to build WebAuthn: {error}"));

        let storage = Storage::new(
            args.upload_dir().into(),
            args.preview_dir().into(),
            args.tmp_dir().into(),
            args.upload_max_bytes(),
        );
        let email = EmailSender::new(
            args.gmail_address(),
            args.gmail_app_password(),
            args.mail_from_name(),
            Arc::clone(&nimbus_origin),
        );

        let repository = Repository::new(db.clone());
        let tokens = TokenStore::new(cache.clone());
        let login_attempts = LoginAttemptStore::new(cache);

        let transactor = Transactor::new(db);

        let auth = AuthenticationService::new(
            repository.clone(),
            tokens,
            login_attempts,
            email.clone(),
            webauthn,
            nimbus_origin,
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

        let file_service = FileService::new(
            repository.files.clone(),
            repository.share.clone(),
            repository.trash.clone(),
            repository.folders.clone(),
            storage.clone(),
            processing,
            transactor.clone(),
        );

        let trash_service = TrashFileService::new(
            repository.trash.clone(),
            repository.files.clone(),
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

        let http_config = HttpConfiguration {
            cors_origins: Arc::from(args.allowed_origins()),
            cors_headers: Arc::from(args.allowed_headers()),
            cookie_domain: args.cookie_domain().into(),
            cookie_name: args.cookie_name().into(),
            cookie_secure: args.cookie_secure(),
        };

        let services = ServiceState {
            auth,
            users: user_service,
            files: file_service,
            share: share_service,
            trash: trash_service,
        };

        let queue = DeletionQueue::start(
            repository,
            storage,
            std::time::Duration::from_secs(args.file_deletion_interval_secs()),
            args.file_retention_days(),
        );

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
            cors_headers: Arc::from([String::from("Authorization"), String::from("Content-Type")]),
            cookie_domain: "".into(),
            cookie_name: "nimbus_session".into(),
            cookie_secure: false,
        }
    }
}
