use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use uuid::Uuid;

use crate::{
    auth::session::Session,
    error::AppError,
    file::model::Root,
    state::AppState,
    user::model::{CreateSharedFolder, SetQuota, UpdateRoot, UserView},
};

#[utoipa::path(
    get,
    path = "/roots/{root_id}/users",
    params(("root_id" = Uuid, Path, description = "Root id")),
    responses((status = 200, description = "Users assigned to the root", body = Vec<UserView>))
)]
pub(crate) async fn list_members(
    State(state): State<AppState>,
    session: Session,
    Path(root_id): Path<Uuid>,
) -> Result<Json<Vec<UserView>>, AppError> {
    Ok(Json(
        state
            .services
            .users
            .list_members(&session.user, root_id)
            .await?,
    ))
}

#[utoipa::path(
    post,
    path = "/roots/shared",
    request_body = CreateSharedFolder,
    responses((status = 201, description = "Shared root folder created", body = Root))
)]
pub(crate) async fn create_shared(
    State(state): State<AppState>,
    _admin: Session,
    Json(input): Json<CreateSharedFolder>,
) -> Result<(StatusCode, Json<Root>), AppError> {
    let folder = state.services.users.create_shared(input).await?;
    Ok((StatusCode::CREATED, Json(folder)))
}

#[utoipa::path(
    get,
    path = "/admin/roots",
    responses((status = 200, description = "Every root folder", body = Vec<Root>))
)]
pub(crate) async fn list_all(
    State(state): State<AppState>,
    _admin: Session,
) -> Result<Json<Vec<Root>>, AppError> {
    Ok(Json(state.services.users.list_all_roots().await?))
}

#[utoipa::path(
    put,
    path = "/roots/{root_id}",
    params(("root_id" = Uuid, Path, description = "Root id")),
    request_body = UpdateRoot,
    responses((status = 200, description = "Root name and quota updated", body = Root))
)]
pub(crate) async fn update_root(
    State(state): State<AppState>,
    _admin: Session,
    Path(root_id): Path<Uuid>,
    Json(input): Json<UpdateRoot>,
) -> Result<Json<Root>, AppError> {
    Ok(Json(
        state.services.users.update_root(root_id, input).await?,
    ))
}

#[utoipa::path(
    put,
    path = "/roots/{root_id}/quota",
    params(("root_id" = Uuid, Path, description = "Root id")),
    request_body = SetQuota,
    responses((status = 200, description = "Quota updated"))
)]
pub(crate) async fn set_quota(
    State(state): State<AppState>,
    _admin: Session,
    Path(root_id): Path<Uuid>,
    Json(input): Json<SetQuota>,
) -> Result<StatusCode, AppError> {
    state.services.users.set_quota(root_id, input).await?;
    Ok(StatusCode::OK)
}

#[utoipa::path(
    post,
    path = "/roots/{root_id}/users/{user_id}",
    params(
        ("root_id" = Uuid, Path, description = "Root id"),
        ("user_id" = Uuid, Path, description = "User id")
    ),
    responses((status = 204, description = "User assigned to the shared folder"))
)]
pub(crate) async fn assign(
    State(state): State<AppState>,
    _admin: Session,
    Path((root_id, user_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    state.services.users.assign(root_id, user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete,
    path = "/roots/{root_id}/users/{user_id}",
    params(
        ("root_id" = Uuid, Path, description = "Root id"),
        ("user_id" = Uuid, Path, description = "User id")
    ),
    responses((status = 204, description = "User removed from the shared folder"))
)]
pub(crate) async fn unassign(
    State(state): State<AppState>,
    _admin: Session,
    Path((root_id, user_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    state.services.users.unassign(root_id, user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}
