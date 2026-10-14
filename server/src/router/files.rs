use crate::{
    auth::session::Session,
    error::AppError,
    file::{
        model::{CreateDirectory, FileEntry, FileView, RestoreFile, RootView, TrashFiles},
        queue::DeletionResult,
        tree::FileTree,
    },
    router::sse,
    state::AppState,
};
use axum::{
    Json,
    body::Body,
    extract::{Multipart, Path, Query, State},
    http::{StatusCode, header},
    response::{Response, Sse, sse::Event},
};
use axum_extra::{TypedHeader, headers::Range};
use futures::Stream;
use serde::Deserialize;
use std::convert::Infallible;
use std::io::SeekFrom;
use std::ops::Bound;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::ReaderStream;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct ParentQuery {
    parent: Option<Uuid>,
}

#[utoipa::path(
    get,
    path = "/roots",
    responses((status = 200, description = "Roots the caller can use", body = Vec<RootView>))
)]
pub(crate) async fn roots(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<Vec<RootView>>, AppError> {
    Ok(Json(
        state
            .services
            .files
            .list_accessible_roots(&session.user)
            .await?,
    ))
}

#[utoipa::path(
    get,
    path = "/roots/{root_id}",
    params(
        ("root_id" = Uuid, Path, description = "Root id"),
        ("parent" = Option<Uuid>, Query, description = "Directory to list. Omit to list the root itself.")
    ),
    responses((status = 200, description = "Children of the directory", body = Vec<FileView>))
)]
pub(crate) async fn list(
    Path(root_id): Path<Uuid>,
    State(state): State<AppState>,
    session: Session,
    Query(query): Query<ParentQuery>,
) -> Result<Json<Vec<FileView>>, AppError> {
    Ok(Json(
        state
            .services
            .files
            .list(&session.user, root_id, query.parent)
            .await?,
    ))
}

#[utoipa::path(
    get,
    path = "/roots/{root_id}/tree",
    params(("root_id" = Uuid, Path, description = "Root id")),
    responses((status = 200, description = "Live file tree", body = FileTree))
)]
pub(crate) async fn tree(
    Path(root_id): Path<Uuid>,
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<FileTree>, AppError> {
    Ok(Json(
        state
            .services
            .files
            .get_tree(&session.user, root_id, None)
            .await?,
    ))
}

#[utoipa::path(
    get,
    path = "/roots/{root_id}/trash",
    params(("root_id" = Uuid, Path, description = "Root id")),
    responses((status = 200, description = "Trash tree", body = FileTree))
)]
pub(crate) async fn trash(
    Path(root_id): Path<Uuid>,
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<FileTree>, AppError> {
    Ok(Json(
        state
            .services
            .files
            .get_trash_tree(&session.user, root_id)
            .await?,
    ))
}

#[utoipa::path(
    delete,
    path = "/roots/{root_id}/trash/{id}",
    params(
        ("root_id" = Uuid, Path, description = "Root id"),
        ("id" = Uuid, Path, description = "Trashed file id")
    ),
    responses((status = 200, description = "Deletion progress", body = DeletionResult))
)]
pub(crate) async fn delete_trash(
    Path((root_id, id)): Path<(Uuid, Uuid)>,
    State(state): State<AppState>,
    session: Session,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>> + Send>, AppError> {
    let rx = state
        .services
        .files
        .delete_trash(&session.user, root_id, id, state.queue)
        .await?;

    Ok(sse(rx, |result| {
        tracing::debug!(?result, "sending SSE event");
        match Event::default().json_data(result) {
            Ok(event) => event,
            Err(error) => {
                tracing::error!(%error, "error serializing SSE event");
                Event::default().data(format!("{error}").replace('\n', " "))
            }
        }
    }))
}

#[utoipa::path(
    post,
    path = "/roots/{root_id}/trash/{id}/restore",
    params(
        ("root_id" = Uuid, Path, description = "Root id"),
        ("id" = Uuid, Path, description = "Trashed file id")
    ),
    request_body = RestoreFile,
    responses((status = 204, description = "File restored"))
)]
pub(crate) async fn restore_file(
    Path((root_id, id)): Path<(Uuid, Uuid)>,
    State(state): State<AppState>,
    session: Session,
    Json(input): Json<RestoreFile>,
) -> Result<StatusCode, AppError> {
    state
        .services
        .files
        .restore(&session.user, root_id, id, input)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/roots/{root_id}/directories",
    request_body = CreateDirectory,
    responses((status = 201, description = "Directory created", body = FileEntry))
)]
pub(crate) async fn create_directory(
    Path(root_id): Path<Uuid>,
    State(state): State<AppState>,
    session: Session,
    Json(input): Json<CreateDirectory>,
) -> Result<(StatusCode, Json<FileEntry>), AppError> {
    let entry = state
        .services
        .files
        .create_directory(&session.user, root_id, input)
        .await?;

    Ok((StatusCode::CREATED, Json(entry)))
}

#[utoipa::path(
    post,
    path = "/roots/{root_id}",
    params(("root_id" = Uuid, Path, description = "Root id")),
    responses((status = 200, description = "Stored files", body = Vec<FileEntry>))
)]
pub(crate) async fn upload(
    Path(root_id): Path<Uuid>,
    State(state): State<AppState>,
    session: Session,
    mut multipart: Multipart,
) -> Result<Json<Vec<FileEntry>>, AppError> {
    let mut parent = None;
    let mut uploaded = Vec::new();

    while let Some(mut field) = multipart.next_field().await.map_err(|error| {
        tracing::error!(%error, "multipart read failed");
        AppError::Unprocessable("Invalid upload".into())
    })? {
        match field.name() {
            Some("parent") => {
                let text = field
                    .text()
                    .await
                    .map_err(|_| AppError::Unprocessable("Parent must be text".into()))?;

                parent = Some(
                    text.parse::<Uuid>()
                        .map_err(|_| AppError::Unprocessable("Parent must be a uuid".into()))?,
                );
            }
            Some("file") => {
                let Some(name) = field.file_name().map(|name| name.to_string()) else {
                    tracing::warn!("multipart file field has no name: {:?}", field);
                    continue;
                };

                let mut writer = state
                    .services
                    .files
                    .start_upload(&session.user, root_id, parent, name)
                    .await?;

                while let Some(chunk) = field.chunk().await.map_err(|error| {
                    tracing::error!(%error, "multipart chunk failed");
                    AppError::Unprocessable("Invalid upload".into())
                })? {
                    if let Err(error) = writer.write(&chunk).await {
                        tracing::error!(%error, "unable to write to writer");
                        return Err(error);
                    }
                }

                let file = match writer.finish().await {
                    Ok(file) => file,
                    Err(error) => {
                        tracing::error!(%error, "unable to flush upload writer");
                        return Err(error);
                    }
                };

                uploaded.push(file);
            }
            _ => {
                tracing::warn!("multipart field has no name: {:?}", field);
            }
        }
    }

    if uploaded.is_empty() {
        return Err(AppError::Unprocessable("File is required".into()));
    }

    let mut stored = Vec::with_capacity(uploaded.len());

    for file in uploaded {
        stored.push(
            state
                .services
                .files
                .finish_upload(&session.user, root_id, parent, file)
                .await?,
        );
    }

    Ok(Json(stored))
}

#[utoipa::path(
    get,
    path = "/roots/{root_id}/files/{file_id}/ancestors",
    params(
        ("root_id" = Uuid, Path, description = "Root id"),
        ("file_id" = Uuid, Path, description = "File id")
    ),
    responses((
        status = 200,
        description = "Directories from the root down to this file, including this file",
        body = Vec<FileEntry>
    ))
)]
pub(crate) async fn ancestors(
    State(state): State<AppState>,
    session: Session,
    Path((root_id, file_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Vec<FileEntry>>, AppError> {
    Ok(Json(
        state
            .services
            .files
            .ancestors(&session.user, root_id, file_id)
            .await?,
    ))
}

#[utoipa::path(
    get,
    path = "/roots/{root_id}/files/{id}",
    params(
        ("root_id" = Uuid, Path, description = "Root id"),
        ("id" = Uuid, Path, description = "File id")
    ),
    responses((status = 200, description = "File metadata", body = FileEntry))
)]
pub(crate) async fn metadata(
    State(state): State<AppState>,
    session: Session,
    Path((_root_id, file_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<FileEntry>, AppError> {
    Ok(Json(
        state
            .services
            .files
            .metadata(&session.user, file_id)
            .await?,
    ))
}

#[utoipa::path(
    put,
    path = "/roots/{root_id}/files/{id}/overwrite",
    params(
        ("root_id" = Uuid, Path, description = "Root id"),
        ("id" = Uuid, Path, description = "File id")
    ),
    responses((status = 200, description = "Updated file", body = FileEntry))
)]
pub(crate) async fn overwrite(
    Path((root_id, file_id)): Path<(Uuid, Uuid)>,
    State(state): State<AppState>,
    session: Session,
    mut multipart: Multipart,
) -> Result<Json<FileEntry>, AppError> {
    let (existing, mut writer) = state
        .services
        .files
        .start_overwrite(&session.user, root_id, file_id)
        .await?;

    let mut saw_file = false;

    while let Some(mut field) = multipart.next_field().await.map_err(|error| {
        tracing::error!(%error, "multipart read failed");
        AppError::Unprocessable("Invalid upload".into())
    })? {
        if field.name() != Some("file") {
            tracing::warn!("multipart field has unexpected name: {:?}", field.name());
            continue;
        }

        saw_file = true;

        while let Some(chunk) = field.chunk().await.map_err(|error| {
            tracing::error!(%error, "multipart chunk failed");
            AppError::Unprocessable("Invalid upload".into())
        })? {
            if let Err(error) = writer.write(&chunk).await {
                tracing::error!(%error, "unable to write overwrite chunk");
                return Err(error);
            }
        }
    }

    if !saw_file {
        return Err(AppError::Unprocessable("File is required".into()));
    }

    let file = match writer.finish().await {
        Ok(file) => file,
        Err(error) => {
            tracing::error!(%error, "unable to flush overwrite writer");
            return Err(error);
        }
    };

    let new = state
        .services
        .files
        .finish_overwrite(&session.user, existing, file)
        .await?;

    Ok(Json(new))
}

#[utoipa::path(
    get,
    path = "/roots/{root_id}/files/{id}/blob/download",
    params(
        ("root_id" = Uuid, Path, description = "Root id"),
        ("id" = Uuid, Path, description = "File id")
    ),
    responses((status = 200, description = "Full file bytes"))
)]
pub(crate) async fn download(
    State(state): State<AppState>,
    session: Session,
    Path((_root_id, file_id)): Path<(Uuid, Uuid)>,
) -> Result<Response, AppError> {
    let (entry, path, mime) = state.services.files.content(&session.user, file_id).await?;

    let file = File::open(path).await?;

    Response::builder()
        .header(header::CONTENT_TYPE, mime)
        .header(
            header::CONTENT_DISPOSITION,
            content_disposition(&entry.name, false),
        )
        .header(header::CONTENT_LENGTH, entry.size_bytes.to_string())
        .body(Body::from_stream(ReaderStream::new(file)))
        .map_err(|error| {
            tracing::error!(%error, "building download response");
            AppError::Internal
        })
}

#[utoipa::path(
    get,
    path = "/roots/{root_id}/files/{id}/blob/chunk",
    params(
        ("root_id" = Uuid, Path, description = "Root id"),
        ("id" = Uuid, Path, description = "File id")
    ),
    responses(
        (status = 200, description = "Full file bytes (when no Range header)"),
        (status = 206, description = "Partial content for Range requests"),
    )
)]
pub(crate) async fn download_chunk(
    State(state): State<AppState>,
    session: Session,
    Path((_root_id, file_id)): Path<(Uuid, Uuid)>,
    range: Option<TypedHeader<Range>>,
) -> Result<Response, AppError> {
    let (entry, path, mime) = state.services.files.content(&session.user, file_id).await?;

    let mut file = File::open(&path).await?;

    let file_bytes = file.metadata().await?.len();

    // Inline so `<video>` / `<audio>` can play from this URL instead of downloading.
    let disposition = content_disposition(&entry.name, true);

    // No Range header: stream the whole file and advertise byte ranges for media players.
    let Some(TypedHeader(range)) = range else {
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::ACCEPT_RANGES, "bytes")
            .header(header::CONTENT_TYPE, mime)
            .header(header::CONTENT_DISPOSITION, &disposition)
            .header(header::CONTENT_LENGTH, file_bytes.to_string())
            .body(Body::from_stream(ReaderStream::new(file)))
            .map_err(|error| {
                tracing::error!(%error, "building chunk download response");
                AppError::Internal
            });
    };

    // Browsers request a single range for video seek/buffer; serve the first satisfiable one.
    let Some((start_bound, end_bound)) = range.satisfiable_ranges(file_bytes).next() else {
        return Response::builder()
            .status(StatusCode::RANGE_NOT_SATISFIABLE)
            .header(header::CONTENT_RANGE, format!("bytes */{file_bytes}"))
            .body(Body::empty())
            .map_err(|error| {
                tracing::error!(%error, "building range-not-satisfiable response");
                AppError::Internal
            });
    };

    let start = match start_bound {
        Bound::Included(value) => value,
        Bound::Excluded(value) => value.saturating_add(1),
        Bound::Unbounded => 0,
    };

    let end = match end_bound {
        Bound::Included(value) => value,
        Bound::Excluded(value) => value.saturating_sub(1),
        Bound::Unbounded => file_bytes.saturating_sub(1),
    };

    if start > end || start >= file_bytes {
        return Response::builder()
            .status(StatusCode::RANGE_NOT_SATISFIABLE)
            .header(header::CONTENT_RANGE, format!("bytes */{file_bytes}"))
            .body(Body::empty())
            .map_err(|error| {
                tracing::error!(%error, "building range-not-satisfiable response");
                AppError::Internal
            });
    }

    let len = end - start + 1;

    file.seek(SeekFrom::Start(start)).await?;

    let chunk = file.take(len);

    Response::builder()
        .status(StatusCode::PARTIAL_CONTENT)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_TYPE, mime)
        .header(header::CONTENT_DISPOSITION, &disposition)
        .header(header::CONTENT_LENGTH, len.to_string())
        .header(
            header::CONTENT_RANGE,
            format!("bytes {start}-{end}/{file_bytes}"),
        )
        .body(Body::from_stream(ReaderStream::new(chunk)))
        .map_err(|error| {
            tracing::error!(%error, "building partial chunk response");
            AppError::Internal
        })
}

#[utoipa::path(
    get,
    path = "/roots/{root_id}/files/{id}/preview",
    params(
        ("root_id" = Uuid, Path, description = "Root id"),
        ("id" = Uuid, Path, description = "File id")
    ),
    responses((status = 200, description = "Preview image bytes"))
)]
pub(crate) async fn preview(
    State(state): State<AppState>,
    session: Session,
    Path((_root_id, file_id)): Path<(Uuid, Uuid)>,
) -> Result<Response, AppError> {
    let preview = state.services.files.preview(&session.user, file_id).await?;

    Response::builder()
        .header(header::CONTENT_TYPE, preview.mime)
        .header(header::CONTENT_LENGTH, preview.len.to_string())
        .header(header::CACHE_CONTROL, "private, max-age=86400")
        .body(Body::from_stream(ReaderStream::new(preview.file)))
        .map_err(|error| {
            tracing::error!(%error, "building preview response");
            AppError::Internal
        })
}

#[utoipa::path(
    delete,
    path = "/roots/{root_id}/files/{id}",
    params(
        ("root_id" = Uuid, Path, description = "Root id"),
        ("id" = Uuid, Path, description = "File id")
    ),
    responses((status = 204, description = "File moved to trash"))
)]
pub(crate) async fn delete_file(
    State(state): State<AppState>,
    session: Session,
    Path((_root_id, file_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    state.services.files.delete(&session.user, file_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete,
    path = "/roots/{root_id}/files",
    params(("root_id" = Uuid, Path, description = "Root id")),
    request_body = TrashFiles,
    responses((status = 204, description = "Files moved to trash"))
)]
pub(crate) async fn delete_files(
    State(state): State<AppState>,
    session: Session,
    Path(root_id): Path<Uuid>,
    Json(input): Json<TrashFiles>,
) -> Result<StatusCode, AppError> {
    state
        .services
        .files
        .delete_many(&session.user, root_id, input)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

fn content_disposition(name: &str, inline: bool) -> String {
    let name: String = name
        .chars()
        .map(|character| {
            if character.is_ascii()
                && character != '"'
                && character != '\\'
                && !character.is_control()
            {
                character
            } else {
                '_'
            }
        })
        .collect();

    let kind = if inline { "inline" } else { "attachment" };

    format!("{kind}; filename=\"{name}\"")
}
