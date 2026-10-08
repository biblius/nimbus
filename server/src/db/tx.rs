use crate::{db::DbPool, error::AppError};
use sqlx::Transaction;
use tracing::instrument;

use super::Db;

/// Service level transaction handler. Dropping the returned transaction rolls it back.
#[derive(Clone)]
pub struct Transactor {
    pool: DbPool,
}

impl Transactor {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    #[instrument(skip(self), err(Debug))]
    pub async fn begin(&self) -> Result<Transaction<'static, Db>, AppError> {
        self.pool.begin().await.map_err(AppError::from)
    }
}
