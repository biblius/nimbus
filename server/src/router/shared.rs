use crate::{
    error::AppError,
    file::model::{PreviewSize, SharedFileView},
    router::chunked_download,
    state::AppState,
};
use axum::{
    Json,
    body::Body,
    extract::{Path, Query, State},
    http::header,
    response::Response,
};
use axum_extra::{TypedHeader, headers::Range};
use serde::Deserialize;
use tokio_util::io::ReaderStream;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct PreviewQuery {
    /// Square edge length in pixels: `64` (default) or `256`.
    size: Option<u32>,
}

pub(crate) async fn file(
    State(state): State<AppState>,
    Path(file_id): Path<Uuid>,
) -> Result<Json<SharedFileView>, AppError> {
    Ok(Json(state.services.share.get(file_id).await?))
}

pub(crate) async fn children(
    State(state): State<AppState>,
    Path(file_id): Path<Uuid>,
) -> Result<Json<Vec<SharedFileView>>, AppError> {
    Ok(Json(state.services.share.list(file_id).await?))
}

pub(crate) async fn download(
    State(state): State<AppState>,
    Path(file_id): Path<Uuid>,
) -> Result<Response, AppError> {
    let (name, body) = state.services.share.download(file_id).await?;

    body.into_response(name).await
}

pub(crate) async fn download_chunk(
    State(state): State<AppState>,
    Path(file_id): Path<Uuid>,
    range: Option<TypedHeader<Range>>,
) -> Result<Response, AppError> {
    let (entry, path, mime) = state.services.share.content(file_id).await?;

    chunked_download(&path, &entry.name, mime, range).await
}

pub(crate) async fn thumbnail(
    State(state): State<AppState>,
    Path(file_id): Path<Uuid>,
    Query(query): Query<PreviewQuery>,
) -> Result<Response, AppError> {
    let size = PreviewSize::from_px(query.size.unwrap_or(64))?;
    let thumb = state.services.share.thumbnail(file_id, size).await?;

    Response::builder()
        .header(header::CONTENT_TYPE, thumb.mime)
        .header(header::CONTENT_LENGTH, thumb.len.to_string())
        .header(header::CACHE_CONTROL, "public, max-age=86400")
        .body(Body::from_stream(ReaderStream::new(thumb.file)))
        .map_err(|error| {
            tracing::error!(%error, "building shared thumbnail response");
            AppError::Internal
        })
}

pub(crate) async fn content_for_preview(
    State(state): State<AppState>,
    Path(file_id): Path<Uuid>,
) -> Result<Response, AppError> {
    let (name, path, mime) = state.services.share.content_for_preview(file_id).await?;

    chunked_download(&path, &name, mime, None).await
}
