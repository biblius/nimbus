use crate::{
    auth::session::Session,
    error::AppError,
    model::{List, PaginationSort},
    router::sse,
    state::AppState,
    user::model::{CreateUser, UpdateUser, UserSearch, UserSort, UserView},
};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::sse::{Event, Sse},
};
use futures::Stream;
use std::convert::Infallible;
use uuid::Uuid;

pub(crate) async fn create_user(
    _session: Session,
    State(state): State<AppState>,
    Json(input): Json<CreateUser>,
) -> Result<(StatusCode, Json<UserView>), AppError> {
    let user = state.services.users.create_user(input).await?;
    Ok((StatusCode::CREATED, Json(user)))
}

pub(crate) async fn list_users(
    _session: Session,
    State(state): State<AppState>,
    Query(pagination): Query<PaginationSort<UserSort, UserSearch>>,
) -> Result<Json<List<UserView>>, AppError> {
    Ok(Json(state.services.users.list_users(pagination).await?))
}

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

pub(crate) async fn delete_user(
    _session: Session,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>> + Send>, AppError> {
    let rx = state.services.users.delete_user(id, state.queue).await?;

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
