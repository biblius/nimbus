use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::sse::{Event, Sse},
};
use futures::Stream;
use std::convert::Infallible;
use uuid::Uuid;

use crate::{
    auth::session::Session,
    error::AppError,
    file::model::RootView,
    model::{List, PaginationSort},
    router::sse,
    state::AppState,
    user::model::{CreateSharedFolder, Member, SetQuota, UpdateRoot, UserSearch, UserSort},
};

pub(crate) async fn usage(
    State(state): State<AppState>,
    session: Session,
    Path(root_id): Path<Uuid>,
) -> Result<Json<i64>, AppError> {
    Ok(Json(
        state.services.files.usage(&session.user, root_id).await?,
    ))
}

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

pub(crate) async fn create_shared(
    State(state): State<AppState>,
    _admin: Session,
    Json(input): Json<CreateSharedFolder>,
) -> Result<(StatusCode, Json<RootView>), AppError> {
    let folder = state.services.users.create_shared(input).await?;
    Ok((StatusCode::CREATED, Json(folder)))
}

pub(crate) async fn list_private(
    State(state): State<AppState>,
    _admin: Session,
) -> Result<Json<Vec<RootView>>, AppError> {
    Ok(Json(state.services.users.list_private_roots().await?))
}

pub(crate) async fn list_shared(
    State(state): State<AppState>,
    _admin: Session,
) -> Result<Json<Vec<RootView>>, AppError> {
    Ok(Json(state.services.users.list_shared_roots().await?))
}

pub(crate) async fn get_root(
    State(state): State<AppState>,
    _admin: Session,
    Path(root_id): Path<Uuid>,
) -> Result<Json<RootView>, AppError> {
    Ok(Json(state.services.users.get_root(root_id).await?))
}

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

pub(crate) async fn set_quota(
    State(state): State<AppState>,
    _admin: Session,
    Path(root_id): Path<Uuid>,
    Json(input): Json<SetQuota>,
) -> Result<StatusCode, AppError> {
    state.services.users.set_quota(root_id, input).await?;
    Ok(StatusCode::OK)
}

pub(crate) async fn delete_root(
    State(state): State<AppState>,
    admin: Session,
    Path(root_id): Path<Uuid>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>> + Send>, AppError> {
    let rx = state
        .services
        .users
        .delete_root(&admin.user, root_id, state.queue)
        .await?;

    Ok(sse(rx, |result| {
        tracing::debug!(?result, "sending SSE event");
        match Event::default().json_data(result) {
            Ok(event) => event,
            Err(error) => {
                tracing::error!(%error,"error serializing SSE event");
                let err = format!("{error}").replace('\n', " ");
                Event::default().data(err)
            }
        }
    }))
}

pub(crate) async fn assign(
    State(state): State<AppState>,
    _admin: Session,
    Path((root_id, user_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    state.services.users.assign(root_id, user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn unassign(
    State(state): State<AppState>,
    _admin: Session,
    Path((root_id, user_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    state.services.users.unassign(root_id, user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}
