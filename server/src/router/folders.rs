use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use uuid::Uuid;

use crate::{
    auth::session::Session,
    error::AppError,
    file::model::RootView,
    model::{List, PaginationSort},
    state::AppState,
    user::model::{CreateSharedFolder, Member, SetQuota, UpdateRoot, UserSearch, UserSort},
};

#[utoipa::path(
    get,
    path = "/roots/{root_id}/usage",
    params(("root_id" = Uuid, Path, description = "Root id")),
    responses((status = 200, description = "Bytes stored in the root", body = i64))
)]
pub(crate) async fn usage(
    State(state): State<AppState>,
    session: Session,
    Path(root_id): Path<Uuid>,
) -> Result<Json<i64>, AppError> {
    Ok(Json(
        state.services.files.usage(&session.user, root_id).await?,
    ))
}

#[utoipa::path(
    get,
    path = "/roots/{root_id}/users",
    params(("root_id" = Uuid, Path, description = "Root id")),
    responses((status = 200, description = "Users assigned to the root", body = Vec<Member>))
)]
pub(crate) async fn list_members(
    State(state): State<AppState>,
    session: Session,
    Path(root_id): Path<Uuid>,
    Query(pagination): Query<PaginationSort<UserSort, UserSearch>>,
) -> Result<Json<List<Member>>, AppError> {
    Ok(Json(
        state
            .services
            .users
            .list_members(&session.user, root_id, pagination)
            .await?,
    ))
}

#[utoipa::path(
    get,
    path = "/roots/{root_id}/users/all",
    params(("root_id" = Uuid, Path, description = "Root id")),
    responses((status = 200, description = "Every user assigned to the root", body = Vec<Member>))
)]
pub(crate) async fn list_all_members(
    State(state): State<AppState>,
    session: Session,
    Path(root_id): Path<Uuid>,
) -> Result<Json<Vec<Member>>, AppError> {
    Ok(Json(
        state
            .services
            .users
            .list_all_members(&session.user, root_id)
            .await?,
    ))
}

#[utoipa::path(
    post,
    path = "/admin/roots/shared",
    request_body = CreateSharedFolder,
    responses((status = 201, description = "Shared root folder created", body = RootView))
)]
pub(crate) async fn create_shared(
    State(state): State<AppState>,
    _admin: Session,
    Json(input): Json<CreateSharedFolder>,
) -> Result<(StatusCode, Json<RootView>), AppError> {
    let folder = state.services.users.create_shared(input).await?;
    Ok((StatusCode::CREATED, Json(folder)))
}

#[utoipa::path(
    get,
    path = "/admin/roots/private",
    responses((status = 200, description = "Private root folders", body = Vec<RootView>))
)]
pub(crate) async fn list_private(
    State(state): State<AppState>,
    _admin: Session,
) -> Result<Json<Vec<RootView>>, AppError> {
    Ok(Json(state.services.users.list_private_roots().await?))
}

#[utoipa::path(
    get,
    path = "/admin/roots/shared",
    responses((status = 200, description = "Shared root folders", body = Vec<RootView>))
)]
pub(crate) async fn list_shared(
    State(state): State<AppState>,
    _admin: Session,
) -> Result<Json<Vec<RootView>>, AppError> {
    Ok(Json(state.services.users.list_shared_roots().await?))
}

#[utoipa::path(
    get,
    path = "/admin/roots/{root_id}",
    params(("root_id" = Uuid, Path, description = "Root id")),
    responses(
        (status = 200, description = "Root folder", body = RootView),
        (status = 404, description = "Folder does not exist")
    )
)]
pub(crate) async fn get_root(
    State(state): State<AppState>,
    _admin: Session,
    Path(root_id): Path<Uuid>,
) -> Result<Json<RootView>, AppError> {
    Ok(Json(state.services.users.get_root(root_id).await?))
}

#[utoipa::path(
    put,
    path = "/admin/roots/{root_id}",
    params(("root_id" = Uuid, Path, description = "Root id")),
    request_body = UpdateRoot,
    responses((status = 200, description = "Root name, quota, and members updated", body = RootView))
)]
pub(crate) async fn update_root(
    State(state): State<AppState>,
    _admin: Session,
    Path(root_id): Path<Uuid>,
    Json(input): Json<UpdateRoot>,
) -> Result<Json<RootView>, AppError> {
    Ok(Json(
        state.services.users.update_root(root_id, input).await?,
    ))
}

#[utoipa::path(
    put,
    path = "/admin/roots/{root_id}/quota",
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
    delete,
    path = "/admin/roots/{root_id}",
    params(("root_id" = Uuid, Path, description = "Root id")),
    responses((status = 204, description = "Shared root and its files removed"))
)]
pub(crate) async fn delete_root(
    State(state): State<AppState>,
    _admin: Session,
    Path(root_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    state.services.users.delete_root(root_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/admin/roots/{root_id}/users/{user_id}",
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
    path = "/admin/roots/{root_id}/users/{user_id}",
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
