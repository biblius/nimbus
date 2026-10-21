use chrono::{DateTime, Duration, Utc};
use tracing::instrument;

use crate::{
    cache::Cache,
    error::{AppError, AuthFailure},
};

/// How long failed-attempt counters are kept when not cleared by a success.
const ATTEMPTS_TTL_SECS: u64 = 24 * 60 * 60; // 1 day

/// Every time `total attempts % this == 0`, set a login backoff.
const ATTEMPT_MOD: u64 = 5;

/// Every [`ATTEMPT_MOD`] attempts, sets the backoff timer to
/// `(total attempts / ATTEMPT_MOD) * this` minutes.
const BACKOFF_MULTIPLER: u64 = 10;

/// Cache-backed login throttle keyed by username (password and OTP).
///
/// Values are plain strings: attempt counts as decimal, backoff as an RFC3339
/// unlock timestamp.
#[derive(Clone)]
pub struct LoginAttemptStore {
    cache: Cache,
}

impl LoginAttemptStore {
    pub fn new(cache: Cache) -> Self {
        Self { cache }
    }

    /// Reject login while `username` is inside an active backoff window.
    #[instrument(skip(self), fields(username), err(Debug))]
    pub async fn ensure_allowed(&self, username: &str) -> Result<(), AppError> {
        let Some(raw) = self.cache.get(&Self::backoff_key(username)).await? else {
            return Ok(());
        };

        let unlock_at = DateTime::parse_from_rfc3339(&raw)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|error| {
                tracing::error!(%error, username, "invalid login backoff timestamp");
                AppError::Internal
            })?;

        if unlock_at > Utc::now() {
            return Err(AppError::Auth(AuthFailure::LoginBackoff { unlock_at }));
        }

        Ok(())
    }

    /// Increment the failed-login counter (bad password or OTP); every 5th
    /// attempt sets a backoff of `(attempts / 10) * 10` minutes.
    #[instrument(skip(self), fields(username), err(Debug))]
    pub async fn record_failure(&self, username: &str) -> Result<(), AppError> {
        let count = self
            .cache
            .get(&Self::attempts_key(username))
            .await?
            .and_then(|raw| raw.parse::<u64>().ok())
            .unwrap_or(0)
            .saturating_add(1);

        self.cache
            .set_ex(
                &Self::attempts_key(username),
                &count.to_string(),
                ATTEMPTS_TTL_SECS,
            )
            .await?;

        if count % ATTEMPT_MOD == 0 {
            let minutes = (count / ATTEMPT_MOD) * BACKOFF_MULTIPLER;
            let unlock_at = Utc::now() + Duration::minutes(minutes as i64);
            let ttl_secs = minutes.saturating_mul(60);

            self.cache
                .set_ex(
                    &Self::backoff_key(username),
                    &unlock_at.to_rfc3339(),
                    ttl_secs,
                )
                .await?;
        }

        Ok(())
    }

    /// Clear attempt counter and backoff after a full successful login.
    #[instrument(skip(self), fields(username), err(Debug))]
    pub async fn clear(&self, username: &str) -> Result<(), AppError> {
        self.cache.del(&Self::attempts_key(username)).await?;
        self.cache.del(&Self::backoff_key(username)).await?;
        Ok(())
    }

    fn attempts_key(username: &str) -> String {
        format!("login:attempts:{username}")
    }

    fn backoff_key(username: &str) -> String {
        format!("login:backoff:{username}")
    }
}
