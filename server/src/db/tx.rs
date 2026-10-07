use crate::{db::DbPool, error::AppError};
use sqlx::Transaction;

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

    pub async fn begin(&self) -> Result<Transaction<'static, Db>, AppError> {
        self.pool.begin().await.map_err(AppError::from)
    }
}
