pub(crate) mod deletion;
pub(crate) mod files;
pub(crate) mod metadata;
pub(crate) mod sessions;
pub(crate) mod tx;
pub(crate) mod user_folders;
pub(crate) mod users;

use crate::error::AppError;
use std::time::Duration;

pub use deletion::DeletionRepository;
pub use files::FileRepository;
pub use metadata::MetadataRepository;
pub use sessions::SessionRepository;
pub use tx::Transactor;
pub use user_folders::UserFolderRepository;
pub use users::UserRepository;

/// Every repository backed by the database pool.
#[derive(Clone)]
pub struct Repository {
    pub users: UserRepository,
    pub sessions: SessionRepository,
    pub folders: UserFolderRepository,
    pub files: FileRepository,
    pub deletions: DeletionRepository,
    pub metadata: MetadataRepository,
    pool: DbPool,
}

impl Repository {
    pub fn new(pool: DbPool) -> Self {
        Self {
            users: UserRepository::new(pool.clone()),
            sessions: SessionRepository::new(pool.clone()),
            folders: UserFolderRepository::new(pool.clone()),
            files: FileRepository::new(pool.clone()),
            deletions: DeletionRepository::new(pool.clone()),
            metadata: MetadataRepository::new(pool.clone()),
            pool,
        }
    }

    pub async fn start_tx(&self) -> Result<sqlx::Transaction<'static, Db>, AppError> {
        self.pool.begin().await.map_err(AppError::from)
    }
}

#[cfg(feature = "postgres")]
pub type Db = sqlx::Postgres;

#[cfg(feature = "sqlite")]
pub type Db = sqlx::Sqlite;

#[cfg(feature = "postgres")]
pub type DbPool = sqlx::PgPool;

#[cfg(feature = "sqlite")]
pub type DbPool = sqlx::SqlitePool;

pub async fn connect(url: &str) -> Result<DbPool, AppError> {
    #[cfg(feature = "sqlite")]
    ensure_sqlite_parent(url);

    #[cfg(feature = "postgres")]
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(20)
        .connect(url)
        .await?;

    #[cfg(feature = "sqlite")]
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(20)
        .after_connect(|conn, _| {
            Box::pin(async move {
                sqlx::query("PRAGMA foreign_keys = ON")
                    .execute(&mut *conn)
                    .await?;
                sqlx::query("PRAGMA busy_timeout = 5000")
                    .execute(&mut *conn)
                    .await?;
                sqlx::query("PRAGMA journal_mode = WAL")
                    .execute(&mut *conn)
                    .await?;
                Ok(())
            })
        })
        .connect(url)
        .await?;

    Ok(pool)
}

/// Retry while infrastructure is still starting.
pub async fn connect_with_retry(url: &str) -> DbPool {
    let mut last = None;
    for attempt in 1..=30 {
        match connect(url).await {
            Ok(pool) => return pool,
            Err(error) => {
                tracing::warn!(%error, attempt, "database is not ready");
                last = Some(error);
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }

    panic!(
        "unable to connect to the database: {}",
        last.map(|error| error.to_string())
            .unwrap_or_else(|| "unknown error".into())
    );
}

pub async fn migrate(pool: &DbPool) -> Result<(), AppError> {
    let result = {
        #[cfg(feature = "postgres")]
        {
            sqlx::migrate!("./migrations/postgres").run(pool).await
        }
        #[cfg(feature = "sqlite")]
        {
            sqlx::migrate!("./migrations/sqlite").run(pool).await
        }
    };

    result.map_err(|error| {
        tracing::error!(%error, "database migration failed");
        AppError::Internal
    })
}

#[cfg(feature = "sqlite")]
fn ensure_sqlite_parent(url: &str) {
    let path = url
        .trim_start_matches("sqlite://")
        .split('?')
        .next()
        .unwrap_or("");
    if let Some(parent) = std::path::Path::new(path).parent()
        && !parent.as_os_str().is_empty()
    {
        let _ = std::fs::create_dir_all(parent);
    }
}
