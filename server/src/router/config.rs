use crate::{
    auth::session::Session,
    config::{ConfigUpdate, Configuration},
    error::AppError,
    state::AppState,
};
use axum::{Json, extract::State};

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
