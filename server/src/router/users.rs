use crate::{
    error::AppError,
    router::middleware::AdminSession,
    state::AppState,
    user::model::{CreateUser, CurrentUserView, UserView},
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
    responses((status = 201, description = "User and private folder created", body = CurrentUserView))
)]
pub(crate) async fn create_user(
    State(state): State<AppState>,
    _admin: AdminSession,
    Json(input): Json<CreateUser>,
) -> Result<(StatusCode, Json<CurrentUserView>), AppError> {
    let user = state.services.users.create_user(input).await?;
    Ok((StatusCode::CREATED, Json(user)))
}

#[utoipa::path(
    get,
    path = "/users",
    responses((status = 200, description = "Active users", body = Vec<CurrentUserView>))
)]
pub(crate) async fn list_users(
    State(state): State<AppState>,
    _admin: AdminSession,
) -> Result<Json<Vec<UserView>>, AppError> {
    Ok(Json(state.services.users.list_users().await?))
}

#[utoipa::path(
    delete,
    path = "/users/{id}",
    params(("id" = Uuid, Path, description = "User id")),
    responses((status = 204, description = "User soft-deleted"))
)]
pub(crate) async fn delete_user(
    State(state): State<AppState>,
    _admin: AdminSession,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    state.services.users.delete_user(id).await?;
    Ok(StatusCode::NO_CONTENT)
}
