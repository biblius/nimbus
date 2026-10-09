use crate::{
    auth::session::Session,
    error::AppError,
    state::AppState,
    user::model::{CreateUser, UpdateUser, UserView, UserViewFull},
};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use uuid::Uuid;

#[utoipa::path(
    post,
    path = "/users",
    request_body = CreateUser,
    responses((status = 201, description = "User and private folder created", body = UserViewFull))
)]
pub(crate) async fn create_user(
    _session: Session,
    State(state): State<AppState>,
    Json(input): Json<CreateUser>,
) -> Result<(StatusCode, Json<UserViewFull>), AppError> {
    let user = state.services.users.create_user(input).await?;
    Ok((StatusCode::CREATED, Json(user)))
}

#[utoipa::path(
    get,
    path = "/users",
    responses((status = 200, description = "Active users", body = Vec<UserViewFull>))
)]
pub(crate) async fn list_users(
    _session: Session,
    State(state): State<AppState>,
) -> Result<Json<Vec<UserViewFull>>, AppError> {
    Ok(Json(state.services.users.list_users().await?))
}

#[utoipa::path(
    patch,
    path = "/users/{id}",
    params(("id" = Uuid, Path, description = "User id")),
    request_body = UpdateUser,
    responses((status = 200, description = "User updated", body = UserView))
)]
pub(crate) async fn update_user(
    _session: Session,
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
    Json(input): Json<UpdateUser>,
) -> Result<Json<UserView>, AppError> {
    Ok(Json(
        state.services.users.update_user(user_id, input).await?,
    ))
}

#[utoipa::path(
    delete,
    path = "/users/{id}",
    params(("id" = Uuid, Path, description = "User id")),
    responses((status = 204, description = "User removed"))
)]
pub(crate) async fn delete_user(
    _session: Session,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    state.services.users.delete_user(id).await?;
    Ok(StatusCode::NO_CONTENT)
}
