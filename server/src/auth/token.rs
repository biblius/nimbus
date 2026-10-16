use std::fmt::Display;

use serde::{Serialize, de::DeserializeOwned};
use uuid::Uuid;

use crate::{cache::Cache, error::AppError};

/// Cache-backed single-use tokens with a TTL.
#[derive(Clone)]
pub struct TokenStore {
    cache: Cache,
}

impl TokenStore {
    pub fn new(cache: Cache) -> Self {
        Self { cache }
    }

    pub async fn store<T: Serialize>(
        &self,
        action: OneShotToken,
        key: Uuid,
        value: &T,
    ) -> Result<(), AppError> {
        let body = serde_json::to_string(value).map_err(|error| {
            tracing::error!(%error, "serializing one-shot token payload");
            AppError::Internal
        })?;
        self.cache
            .set_ex(&action.key(key), &body, action.ttl_secs())
            .await
    }

    #[allow(dead_code)]
    pub async fn delete(&self, action: OneShotToken, key: Uuid) -> Result<(), AppError> {
        self.cache.del(&action.key(key)).await
    }

    /// Read the token without consuming it. Missing or expired tokens are a 404.
    pub async fn get<T: DeserializeOwned>(
        &self,
        action: OneShotToken,
        key: Uuid,
    ) -> Result<T, AppError> {
        let Some(body) = self.cache.get(&action.key(key)).await? else {
            return Err(AppError::ResourceMissing(
                "Token is missing, used, or expired".into(),
            ));
        };
        serde_json::from_str(&body).map_err(|error| {
            tracing::error!(%error, "deserializing one-shot token payload");
            AppError::Internal
        })
    }

    /// Atomically read and delete the token. Missing or expired tokens are a 404.
    pub async fn consume<T: DeserializeOwned>(
        &self,
        action: OneShotToken,
        key: Uuid,
    ) -> Result<T, AppError> {
        let Some(body) = self.cache.get_del(&action.key(key)).await? else {
            return Err(AppError::ResourceMissing(
                "Token is missing, used, or expired".into(),
            ));
        };
        serde_json::from_str(&body).map_err(|error| {
            tracing::error!(%error, "deserializing one-shot token payload");
            AppError::Internal
        })
    }
}

/// A token stored in the cache required to complete
/// auth related operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OneShotToken {
    /// Used after password login when OTP verification is still required.
    PendingLogin,

    /// Used during OTP registration.
    OneTimePassword,

    /// Used when requesting password resets via email.
    PasswordReset,

    /// In-progress discoverable passkey login (or registration) ceremony.
    Passkey,

    /// Passkey verified in the browser; desktop exchanges this for a session.
    NativeSession,
}

impl OneShotToken {
    pub fn ttl_secs(self) -> u64 {
        match self {
            Self::PendingLogin => 300,
            Self::PasswordReset => 300,
            Self::OneTimePassword => 300,
            Self::Passkey => 300,
            Self::NativeSession => 300,
        }
    }

    fn key(self, key: Uuid) -> String {
        format!("token:{self}:{key}")
    }
}

impl Display for OneShotToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PendingLogin => write!(f, "login"),
            Self::PasswordReset => write!(f, "pwreset"),
            Self::OneTimePassword => write!(f, "otp"),
            Self::Passkey => write!(f, "passkey"),
            Self::NativeSession => write!(f, "native"),
        }
    }
}
