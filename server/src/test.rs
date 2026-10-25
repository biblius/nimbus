//! Test suites and harness.
//!
//! Feature matrix:
//!
//! - `sqlite`: temp-file DB + in-memory cache (no Docker)
//! - `postgres`: Postgres + Valkey via Testcontainers
//!
//! Cache backends are covered by parity unit tests; suites exercise each path
//! once through this split.
//!
//! Keep [`TestState`] alive in suitest [`before_all`](suitest::before_all) so
//! containers are not dropped mid-suite.

mod auth;
mod files;
mod members;
mod users;

use crate::{
    activity::ActivityQueue,
    auth::{login_attempt::LoginAttemptStore, service::AuthenticationService, token::TokenStore},
    cache::Cache,
    db::{self, DbPool, Repository, Transactor},
    email::EmailSender,
    error::AppError,
    file::{
        model::FileEntry,
        queue::{DeletionQueue, DeletionQueueHandle, DeletionResult, ProcessingQueue},
        service::{FileService, TrashFileService},
        storage::Storage,
    },
    state::ServiceState,
    user::{model::User, service::UserService},
};
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[cfg(feature = "postgres")]
use testcontainers::{ContainerAsync, GenericImage, ImageExt, runners::AsyncRunner};

#[cfg(feature = "postgres")]
type PostgresContainer = ContainerAsync<GenericImage>;

#[cfg(feature = "postgres")]
type ValkeyContainer = ContainerAsync<GenericImage>;

/// Application services plus the resources that keep the suite's database alive.
pub struct TestState {
    pub services: ServiceState,
    pub repository: Repository,
    pub pool: DbPool,
    pub cache: Cache,
    storage_dir: PathBuf,
    #[cfg(feature = "postgres")]
    _postgres: PostgresContainer,
    #[cfg(feature = "postgres")]
    _valkey: ValkeyContainer,
    pub queue: DeletionQueueHandle,
}

impl TestState {
    pub async fn init() -> Self {
        // let _ = tracing_subscriber::fmt()
        //     .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        //     .with_test_writer()
        //     .try_init();

        let storage_dir =
            std::env::temp_dir().join(format!("nimbus-{uuid}", uuid = Uuid::new_v4()));
        tokio::fs::create_dir_all(&storage_dir).await.unwrap();

        #[cfg(feature = "sqlite")]
        let pool = init_sqlite(&storage_dir).await;

        #[cfg(feature = "postgres")]
        let (pool, postgres) = init_postgres().await;

        #[cfg(feature = "sqlite")]
        let cache = Cache::new();

        #[cfg(feature = "postgres")]
        let (cache, valkey) = init_valkey().await;

        let (services, repository, storage) =
            build_services(pool.clone(), cache.clone(), &storage_dir).await;

        // Short sweep so expired-trash integration tests finish quickly.
        let queue = DeletionQueue::start(
            repository.clone(),
            storage,
            std::time::Duration::from_secs(1),
            30,
        );

        Self {
            services,
            repository,
            pool,
            cache,
            storage_dir,
            #[cfg(feature = "postgres")]
            _postgres: postgres,
            #[cfg(feature = "postgres")]
            _valkey: valkey,
            queue,
        }
    }

    pub fn storage_dir(&self) -> &Path {
        &self.storage_dir
    }

    /// Drain a deletion SSE stream until the worker drops the sender (cleanup finished).
    pub async fn await_deletion(
        &self,
        mut rx: tokio::sync::mpsc::Receiver<DeletionResult>,
    ) -> Vec<DeletionResult> {
        let mut results = Vec::new();
        while let Some(result) = rx.recv().await {
            results.push(result);
        }
        results
    }
}

impl Drop for TestState {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.storage_dir);
    }
}

#[cfg(feature = "sqlite")]
async fn init_sqlite(dir: &Path) -> DbPool {
    tokio::fs::create_dir_all(dir.join("db")).await.unwrap();
    let url = format!(
        "sqlite://{}?mode=rwc",
        dir.join("db").join("nimbus.db").display()
    );
    let pool = db::connect(&url).await.unwrap();
    db::migrate(&pool).await.unwrap();
    pool
}

#[cfg(feature = "postgres")]
async fn init_postgres() -> (DbPool, PostgresContainer) {
    use testcontainers::core::{IntoContainerPort, WaitFor};

    let postgres = GenericImage::new("postgres", "17")
        .with_exposed_port(5432.tcp())
        .with_wait_for(WaitFor::message_on_stdout(
            "database system is ready to accept connections",
        ))
        .with_wait_for(WaitFor::message_on_stderr(
            "database system is ready to accept connections",
        ))
        .with_env_var("POSTGRES_DB", "nimbus")
        .with_env_var("POSTGRES_USER", "nimbus")
        .with_env_var("POSTGRES_PASSWORD", "nimbus")
        .start()
        .await
        .expect("postgres container error");

    let host = postgres.get_host().await.unwrap();
    let port = postgres.get_host_port_ipv4(5432).await.unwrap();
    let url = format!("postgres://nimbus:nimbus@{host}:{port}/nimbus");

    let pool = db::connect(&url).await.expect("test postgres connection");
    db::migrate(&pool).await.expect("test postgres migration");

    (pool, postgres)
}

#[cfg(feature = "postgres")]
async fn init_valkey() -> (Cache, ValkeyContainer) {
    use testcontainers::core::{IntoContainerPort, WaitFor};

    let valkey = GenericImage::new("valkey/valkey", "8")
        .with_exposed_port(6379.tcp())
        .with_wait_for(WaitFor::message_on_stdout("Ready to accept connections"))
        .start()
        .await
        .expect("valkey container error");

    let host = valkey.get_host().await.unwrap();
    let port = valkey.get_host_port_ipv4(6379).await.unwrap();
    let url = format!("redis://{host}:{port}");
    let cache = Cache::connect(&url).expect("test valkey connection");

    (cache, valkey)
}

async fn build_services(
    pool: DbPool,
    cache: Cache,
    storage_dir: &Path,
) -> (ServiceState, Repository, Storage) {
    let email = EmailSender::new(
        String::new(),
        0,
        String::new(),
        String::new(),
        "Nimbus".into(),
        "http://localhost",
    );

    let storage = Storage::new(storage_dir.join("storage"), 1024 * 1024);
    let repository = Repository::new(pool.clone());

    let rp_origin =
        webauthn_rs::prelude::Url::parse("http://localhost:5173").expect("test webauthn origin");
    let webauthn = webauthn_rs::prelude::WebauthnBuilder::new("localhost", &rp_origin)
        .expect("test webauthn builder")
        .rp_name("Nimbus")
        .build()
        .expect("test webauthn");

    let auth = AuthenticationService::new(
        repository.clone(),
        TokenStore::new(cache.clone()),
        LoginAttemptStore::new(cache.clone()),
        email.clone(),
        webauthn,
        "http://localhost:5173",
    );

    let user_service = UserService::new(
        repository.users.clone(),
        repository.files.clone(),
        repository.trash.clone(),
        repository.folders.clone(),
        repository.sessions.clone(),
        email,
        Transactor::new(pool.clone()),
    );

    let processing = ProcessingQueue::start(storage.clone(), repository.files.clone());
    let activity = ActivityQueue::start(repository.clone());

    let files = FileService::new(
        repository.clone(),
        storage.clone(),
        processing,
        activity.clone(),
        Transactor::new(pool.clone()),
    );
    let trash = TrashFileService::new(
        repository.trash.clone(),
        repository.files.clone(),
        activity,
        Transactor::new(pool),
    );
    let share = crate::file::service::ShareService::new(repository.share.clone(), storage.clone());
    files.prepare().await.unwrap();
    user_service.ensure_initial_admin().await.unwrap();

    (
        ServiceState {
            auth,
            users: user_service,
            files,
            share,
            trash,
        },
        repository,
        storage,
    )
}

pub async fn login(
    state: &TestState,
    username: &str,
    password: &str,
) -> crate::auth::session::Session {
    use crate::auth::model::{LoginCredentials, LoginResult};

    let login = state
        .services
        .auth
        .login(
            LoginCredentials {
                username: username.into(),
                password: password.into(),
                remember: false,
                permanent: false,
            },
            None,
            None,
        )
        .await
        .unwrap();
    let LoginResult::Session { session_id } = login else {
        panic!("expected a session");
    };
    state
        .services
        .auth
        .get_session_by_id(session_id)
        .await
        .unwrap()
}

pub async fn private_root(state: &TestState, user: &User) -> Uuid {
    state
        .services
        .files
        .list_accessible_roots(user)
        .await
        .unwrap()
        .into_iter()
        .find(|root| root.owner == Some(user.id))
        .expect("user has a private root")
        .id
}

pub async fn upload(
    state: &TestState,
    user: &User,
    root: Uuid,
    name: &str,
    bytes: &[u8],
) -> FileEntry {
    submit(state, user, root, vec![(name, bytes)])
        .await
        .into_iter()
        .next()
        .unwrap()
        .unwrap()
}

pub async fn submit(
    state: &TestState,
    user: &User,
    root: Uuid,
    files: Vec<(&str, &[u8])>,
) -> Vec<Result<FileEntry, AppError>> {
    let ctx = match state.services.files.accessible_root(user, root).await {
        Ok(ctx) => ctx,
        Err(error) => return vec![Err(error)],
    };
    if let Err(error) = ctx.require(crate::file::AccessMode::WRITE) {
        return vec![Err(error)];
    }

    let mut staged = Vec::with_capacity(files.len());

    for (name, bytes) in files {
        let mut writer = match state.services.files.start_upload(&ctx, name).await {
            Ok(writer) => writer,
            Err(error) => return vec![Err(error)],
        };
        writer.write(bytes).await.unwrap();
        staged.push((writer.finish().await.unwrap(), None));
    }

    match state
        .services
        .files
        .finish_upload(&ctx, None, vec![], staged)
        .await
    {
        Ok(entries) => entries.into_iter().map(Ok).collect(),
        Err(error) => vec![Err(error)],
    }
}

pub fn blob_count(dir: &Path) -> usize {
    let root = dir.join("storage").join("data");
    if !root.exists() {
        return 0;
    }
    walk(&root)
}

pub fn preview_count(dir: &Path) -> usize {
    let root = dir.join("preview");
    if !root.exists() {
        return 0;
    }
    walk(&root)
}

pub fn preview_path(dir: &Path, hash: &str) -> PathBuf {
    dir.join("preview").join(hash)
}

fn walk(path: &Path) -> usize {
    let mut count = 0;
    for entry in std::fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name();
        // Skip atomic-write temps (`.{hash}.tmp`).
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        if entry.file_type().unwrap().is_dir() {
            count += walk(&entry.path());
        } else {
            count += 1;
        }
    }
    count
}
