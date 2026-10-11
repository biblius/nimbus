use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

/// HTTP error body matching the cerebro API shape.
#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub message: Option<String>,
    pub code: usize,
    pub body: Option<serde_json::Value>,
}

/// All possible reasons authentication or authorization can fail.
#[derive(Debug)]
pub enum AuthFailure {
    SessionCookieNotFound,
    CsrfHeaderNotFound,
    SessionNotFound,
    InvalidCredentials,
}

impl std::fmt::Display for AuthFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("UUID parsing error: {0}")]
    UuidParse(#[from] uuid::Error),

    #[error("Authx error: {0}")]
    Auth(AuthFailure),

    #[error("{0}")]
    ResourceMissing(String),

    #[error("{0}")]
    ResourceConflict(String),

    #[error("Forbidden error: {0}")]
    Forbidden(String),

    #[error("{0}")]
    Unprocessable(String),

    #[error("Storage quota exceeded")]
    QuotaExceeded,

    #[error("Payload too large")]
    PayloadTooLarge,

    #[error("Validation error")]
    Validation(validify::ValidationErrors),

    #[error("Internal error")]
    Internal,
}

impl AppError {
    fn status(&self) -> StatusCode {
        match self {
            AppError::Auth(_) => StatusCode::UNAUTHORIZED,
            AppError::ResourceMissing(_) => StatusCode::NOT_FOUND,
            AppError::ResourceConflict(_) => StatusCode::CONFLICT,
            AppError::Forbidden(_) => StatusCode::FORBIDDEN,
            AppError::Unprocessable(_) | AppError::Validation(_) | AppError::UuidParse(_) => {
                StatusCode::UNPROCESSABLE_ENTITY
            }
            AppError::QuotaExceeded | AppError::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            AppError::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn into_body(self) -> ErrorBody {
        match self {
            AppError::Auth(cause) => ErrorBody {
                message: Some(cause.to_string()),
                code: 1,
                body: None,
            },
            AppError::Forbidden(cause) => ErrorBody {
                message: Some(cause),
                code: 2,
                body: None,
            },
            AppError::ResourceMissing(message) => ErrorBody {
                message: Some(message),
                code: 6,
                body: None,
            },
            AppError::ResourceConflict(message) => ErrorBody {
                message: Some(message),
                code: 7,
                body: None,
            },
            AppError::Validation(errors) => ErrorBody {
                message: Some("Validation error".to_string()),
                code: 4,
                body: serde_json::to_value(errors).ok(),
            },
            AppError::Unprocessable(message) => ErrorBody {
                message: Some(message),
                code: 4,
                body: None,
            },
            AppError::QuotaExceeded => ErrorBody {
                message: Some("Storage quota exceeded".to_string()),
                code: 8,
                body: None,
            },
            AppError::PayloadTooLarge => ErrorBody {
                message: Some("Payload too large".to_string()),
                code: 9,
                body: None,
            },
            AppError::UuidParse(_) | AppError::Internal => ErrorBody {
                message: Some("Internal server error".to_string()),
                code: 0,
                body: None,
            },
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (self.status(), Json(self.into_body())).into_response()
    }
}

impl From<validify::ValidationErrors> for AppError {
    fn from(errors: validify::ValidationErrors) -> Self {
        Self::Validation(errors)
    }
}

impl From<sqlx::Error> for AppError {
    fn from(error: sqlx::Error) -> Self {
        map_sqlx(error)
    }
}

pub fn map_sqlx(error: sqlx::Error) -> AppError {
    if let Some(db) = error.as_database_error() {
        if db.is_unique_violation() {
            return AppError::ResourceConflict("Entry already exists".into());
        }
        if db.is_foreign_key_violation() {
            return AppError::ResourceMissing("A related record does not exist".into());
        }
    }
    tracing::error!(%error, "database error");
    AppError::Internal
}

impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        tracing::error!(%error, "filesystem error");
        Self::Internal
    }
}
