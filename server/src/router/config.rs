use crate::{
    auth::session::Session,
    config::{ConfigUpdate, Configuration},
    error::AppError,
    state::AppState,
};
use axum::{Json, extract::State, http::StatusCode};

pub(crate) async fn get_config(
    _session: Session,
    State(state): State<AppState>,
) -> Result<Json<Configuration>, AppError> {
    Ok(Json(state.services.config.get().await))
}

pub(crate) async fn update_config(
    _session: Session,
    State(state): State<AppState>,
    Json(update): Json<ConfigUpdate>,
) -> Result<Json<Configuration>, AppError> {
    Ok(Json(state.services.config.update(update).await?))
}

/// Send a test message to the admin's account email using current SMTP settings.
pub(crate) async fn send_test_email(
    session: Session,
    State(state): State<AppState>,
) -> Result<StatusCode, AppError> {
    let Some(email) = session
        .user
        .email
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Err(AppError::Unprocessable(
            "Set an email on your account to receive the test message".into(),
        ));
    };

    state.services.config.send_test_email(email).await?;
    Ok(StatusCode::NO_CONTENT)
}
