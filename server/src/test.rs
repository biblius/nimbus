//! Test suites and harness.
//!
//! The database engine follows the crate features: `postgres` starts a Postgres
//! container, `sqlite` uses a temporary file. The cache follows `valkey` or
//! `memory-cache` the same way.
//!
//! Keep [`TestState`] alive in suitest [`before_all`](suitest::before_all) so
//! containers are not dropped mid-suite.

mod auth;
mod files;
mod members;
mod smoke;
mod users;

use crate::{
    auth::{service::AuthenticationService, token::TokenStore},
    cache::Cache,
    db::{self, DbPool, Repository, Transactor},
    email::EmailSender,
    error::AppError,
    file::{
        model::FileEntry,
        queue::{self, DeletionQueue, DeletionQueueHandle, DeletionResult},
        service::FileService,
        storage::Storage,
    },
    state::ServiceState,
    user::{model::User, service::UserService},
};
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[cfg(any(feature = "postgres", feature = "valkey"))]
use testcontainers::{ContainerAsync, GenericImage, ImageExt, runners::AsyncRunner};

#[cfg(feature = "postgres")]
type PostgresContainer = ContainerAsync<GenericImage>;

#[cfg(feature = "valkey")]
type ValkeyContainer = ContainerAsync<GenericImage>;

/// Application services plus the resources that keep the suite's database alive.
pub struct TestState {
    pub services: ServiceState,
    pub pool: DbPool,
    pub cache: Cache,
    storage_dir: PathBuf,
    #[cfg(feature = "postgres")]
    _postgres: PostgresContainer,
    #[cfg(feature = "valkey")]
    _valkey: ValkeyContainer,
    pub queue: DeletionQueueHandle,
}

impl TestState {
    pub async fn init() -> Self {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .with_test_writer()
            .try_init();

        let storage_dir =
            std::env::temp_dir().join(format!("nimbus-{uuid}", uuid = Uuid::new_v4()));
        tokio::fs::create_dir_all(&storage_dir).await.unwrap();

        #[cfg(feature = "sqlite")]
        let pool = init_sqlite(&storage_dir).await;

        #[cfg(feature = "postgres")]
        let (pool, postgres) = init_postgres().await;

        #[cfg(feature = "memory-cache")]
        let cache = Cache::new();

        #[cfg(feature = "valkey")]
        let (cache, valkey) = init_valkey().await;

        let (services, repository, storage) =
            build_services(pool.clone(), cache.clone(), &storage_dir).await;

        let expiry_files = repository.files.clone();
        let expiry_deletions = repository.deletions.clone();
        // Short sweep so SYSTEM-queue integration tests finish quickly.
        let queue = DeletionQueue::start(
            repository,
            storage,
            std::time::Duration::from_secs(1),
        );
        tokio::spawn(queue::expiry::start(
            expiry_files,
            expiry_deletions,
            30,
        ));

        Self {
            services,
            pool,
            cache,
            storage_dir,
            #[cfg(feature = "postgres")]
            _postgres: postgres,
            #[cfg(feature = "valkey")]
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

#[cfg(feature = "valkey")]
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
        String::new(),
        "Nimbus".into(),
        "http://localhost".into(),
    );

    let storage = Storage::new(storage_dir.join("storage"), 1024 * 1024);
    let repository = Repository::new(pool.clone());
    let auth = AuthenticationService::new(
        repository.users.clone(),
        repository.sessions.clone(),
        TokenStore::new(cache),
        email.clone(),
    );

    let user_service = UserService::new(
        repository.users.clone(),
        repository.files.clone(),
        repository.folders.clone(),
        repository.sessions.clone(),
        email,
        Transactor::new(pool.clone()),
        repository.deletions.clone(),
    );

    let files = FileService::new(
        repository.files.clone(),
        repository.folders.clone(),
        storage.clone(),
        30,
        Transactor::new(pool),
    );
    files.prepare().await.unwrap();
    user_service.ensure_initial_admin().await.unwrap();

    (
        ServiceState {
            auth,
            users: user_service,
            files,
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
            },
            None,
            None,
        )
        .await
        .unwrap();
    let LoginResult::Session { session } = login else {
        panic!("expected a session");
    };
    state
        .services
        .auth
        .get_session_by_id(session.id)
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
    let mut stored = Vec::with_capacity(files.len());
    for (name, bytes) in files {
        let mut writer = match state
            .services
            .files
            .start_upload(user, root, None, name.to_string())
            .await
        {
            Ok(writer) => writer,
            Err(error) => {
                stored.push(Err(error));
                continue;
            }
        };
        writer.write(bytes).await.unwrap();
        let file = writer.finish().await.unwrap();
        stored.push(
            state
                .services
                .files
                .finish_upload(user, root, None, file)
                .await,
        );
    }
    stored
}

pub fn blob_count(dir: &Path) -> usize {
    let root = dir.join("storage").join("data");
    if !root.exists() {
        return 0;
    }
    walk(&root)
}

fn walk(path: &Path) -> usize {
    let mut count = 0;
    for entry in std::fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            count += walk(&entry.path());
        } else {
            count += 1;
        }
    }
    count
}
