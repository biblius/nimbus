use crate::{
    auth::session::Session,
    error::AppError,
    file::{
        model::{
            CopyFiles, CreateDirectory, CreateTextFile, DownloadFiles, FileEntry, FileView,
            MoveFiles, PreviewSize, RenameFile, RestoreFile, RootView, ShareFiles, SharedFileView,
            TrashFiles, UnshareFiles,
        },
        tree::FileTree,
    },
    router::{chunked_download, sse},
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
use std::collections::HashMap;
use std::convert::Infallible;
use tokio_util::io::ReaderStream;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct ParentQuery {
    parent: Option<Uuid>,
}

#[derive(Deserialize)]
pub struct PreviewQuery {
    /// Square edge length in pixels: `64` (default) or `256`.
    size: Option<u32>,
}

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

pub(crate) async fn trash(
    Path(root_id): Path<Uuid>,
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<FileTree>, AppError> {
    Ok(Json(
        state
            .services
            .trash
            .get_trash_tree(&session.user, root_id)
            .await?,
    ))
}

pub(crate) async fn delete_trash(
    Path((root_id, id)): Path<(Uuid, Uuid)>,
    State(state): State<AppState>,
    session: Session,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>> + Send>, AppError> {
    let rx = state
        .services
        .trash
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

pub(crate) async fn restore_file(
    Path((root_id, id)): Path<(Uuid, Uuid)>,
    State(state): State<AppState>,
    session: Session,
    Json(input): Json<RestoreFile>,
) -> Result<StatusCode, AppError> {
    state
        .services
        .trash
        .restore(&session.user, root_id, id, input)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn restore_all(
    Path(root_id): Path<Uuid>,
    State(state): State<AppState>,
    session: Session,
    Json(input): Json<RestoreFile>,
) -> Result<StatusCode, AppError> {
    state
        .services
        .trash
        .restore_all(&session.user, root_id, input)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

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

pub(crate) async fn create_text_file(
    Path(root_id): Path<Uuid>,
    State(state): State<AppState>,
    session: Session,
    Json(input): Json<CreateTextFile>,
) -> Result<(StatusCode, Json<FileEntry>), AppError> {
    let entry = state
        .services
        .files
        .create_text_file(&session.user, root_id, input)
        .await?;

    Ok((StatusCode::CREATED, Json(entry)))
}

pub(crate) async fn upload(
    Path(root_id): Path<Uuid>,
    State(state): State<AppState>,
    session: Session,
    mut multipart: Multipart,
) -> Result<Json<Vec<FileEntry>>, AppError> {
    state
        .services
        .files
        .accessible(&session.user, root_id)
        .await?;

    let mut parent = None;
    let mut staged = Vec::new();

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
                    .start_upload(&session.user, root_id, name)
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

                staged.push(file);
            }
            _ => {
                tracing::warn!("multipart field has no name: {:?}", field);
            }
        }
    }

    if staged.is_empty() {
        return Err(AppError::Unprocessable("File is required".into()));
    }

    // Set `parent` after parsing whole multipart as it can arrive after file fields
    let staged = staged.into_iter().map(|file| (file, parent)).collect();

    let files = state
        .services
        .files
        .finish_upload_all(&session.user, root_id, parent, vec![], staged)
        .await?;

    Ok(Json(files))
}

/// Upload a directory tree.
/// Each `file` field's filename is a relative path
/// (e.g. `Photos/2024/pic.jpg`, set by webkitdirectory).
/// Intermediate directories are created as needed.
pub(crate) async fn upload_dir(
    Path(root_id): Path<Uuid>,
    State(state): State<AppState>,
    session: Session,
    mut multipart: Multipart,
) -> Result<Json<Vec<FileEntry>>, AppError> {
    state
        .services
        .files
        .accessible(&session.user, root_id)
        .await?;

    let mut parent = None;
    let mut staged_dirs = Vec::new();
    let mut staged_files = Vec::new();

    // Directories that need to be created, prefix mapped to the UUID
    // under which to create them.
    let mut dirs: HashMap<String, Uuid> = HashMap::new();

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
                let Some(relative) = field.file_name().map(|name| name.to_string()) else {
                    tracing::warn!("multipart file field has no name: {:?}", field);
                    continue;
                };

                let (parts, name) = split_relative_path(&relative)?;

                let mut file_parent = parent;

                let mut prefix = String::new();

                for part in parts {
                    if !prefix.is_empty() {
                        prefix.push('/');
                    }

                    prefix.push_str(part);

                    if let Some(id) = dirs.get(&prefix) {
                        file_parent = Some(*id);
                        continue;
                    }

                    let id = Uuid::new_v4();

                    let name = prefix
                        .split('/')
                        .next_back()
                        .expect("split always returns at least one item");

                    dirs.insert(prefix.clone(), id);

                    staged_dirs.push(CreateDirectory::new(id, name.into(), file_parent));

                    file_parent = Some(id);
                }

                let mut writer = state
                    .services
                    .files
                    .start_upload(&session.user, root_id, name.to_owned())
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

                staged_files.push((file, file_parent));
            }
            _ => {
                tracing::warn!("multipart field has no name: {:?}", field);
            }
        }
    }

    if staged_files.is_empty() {
        return Err(AppError::Unprocessable("File is required".into()));
    }

    let entries = state
        .services
        .files
        .finish_upload_all(&session.user, root_id, parent, staged_dirs, staged_files)
        .await?;

    Ok(Json(entries))
}

/// Split `dir/sub/file.txt` into directory components and the leaf file name.
fn split_relative_path(path: &str) -> Result<(Vec<&str>, &str), AppError> {
    let path = path.trim().trim_matches('/');

    if path.is_empty() || path.contains('\\') {
        return Err(AppError::Unprocessable("Invalid upload path".into()));
    }

    let mut parts: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();

    if parts.is_empty() || parts.iter().any(|part| *part == "." || *part == "..") {
        return Err(AppError::Unprocessable("Invalid upload path".into()));
    }

    let name = parts.pop().expect("parts is non-empty");

    Ok((parts, name))
}

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

pub(crate) async fn overwrite(
    Path((root_id, file_id)): Path<(Uuid, Uuid)>,
    State(state): State<AppState>,
    session: Session,
    mut multipart: Multipart,
) -> Result<StatusCode, AppError> {
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

    state
        .services
        .files
        .finish_overwrite(&session.user, existing, file)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn download_root(
    State(state): State<AppState>,
    session: Session,
    Path(root_id): Path<Uuid>,
) -> Result<Response, AppError> {
    let (name, body) = state
        .services
        .files
        .content_for_root_download(&session.user, root_id)
        .await?;

    body.into_response(name).await
}

#[derive(Deserialize)]
pub struct DownloadFilesQuery {
    /// Comma-separated live file ids.
    ids: String,
}

pub(crate) async fn download_dir(
    State(state): State<AppState>,
    session: Session,
    Path(root_id): Path<Uuid>,
    Query(query): Query<DownloadFilesQuery>,
) -> Result<Response, AppError> {
    let ids = query
        .ids
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| part.parse::<Uuid>())
        .collect::<Result<Vec<_>, _>>()?;

    let (name, body) = state
        .services
        .files
        .download_dir(&session.user, root_id, DownloadFiles { ids })
        .await?;

    body.into_response(name).await
}

pub(crate) async fn download(
    State(state): State<AppState>,
    session: Session,
    Path((_root_id, file_id)): Path<(Uuid, Uuid)>,
) -> Result<Response, AppError> {
    let (name, body) = state
        .services
        .files
        .content_for_download(&session.user, file_id)
        .await?;

    body.into_response(name).await
}

pub(crate) async fn download_chunk(
    State(state): State<AppState>,
    session: Session,
    Path((_root_id, file_id)): Path<(Uuid, Uuid)>,
    range: Option<TypedHeader<Range>>,
) -> Result<Response, AppError> {
    let (name, path, mime) = state
        .services
        .files
        .content_for_stream(&session.user, file_id)
        .await?;

    chunked_download(&path, &name, mime, range).await
}

pub(crate) async fn preview(
    State(state): State<AppState>,
    session: Session,
    Path((_root_id, file_id)): Path<(Uuid, Uuid)>,
    Query(query): Query<PreviewQuery>,
) -> Result<Response, AppError> {
    let size = PreviewSize::from_px(query.size.unwrap_or(64))?;
    let preview = state
        .services
        .files
        .preview(&session.user, file_id, size)
        .await?;

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

pub(crate) async fn rename_file(
    Path((root_id, id)): Path<(Uuid, Uuid)>,
    State(state): State<AppState>,
    session: Session,
    Json(input): Json<RenameFile>,
) -> Result<StatusCode, AppError> {
    state
        .services
        .files
        .rename(&session.user, root_id, id, input)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn move_files(
    Path(root_id): Path<Uuid>,
    State(state): State<AppState>,
    session: Session,
    Json(input): Json<MoveFiles>,
) -> Result<StatusCode, AppError> {
    state
        .services
        .files
        .move_files(&session.user, root_id, input)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn copy_files(
    Path(root_id): Path<Uuid>,
    State(state): State<AppState>,
    session: Session,
    Json(input): Json<CopyFiles>,
) -> Result<StatusCode, AppError> {
    state
        .services
        .files
        .copy_files(&session.user, root_id, input)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn delete_file(
    State(state): State<AppState>,
    session: Session,
    Path((_root_id, file_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    state.services.files.delete(&session.user, file_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

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

pub(crate) async fn shared_file(
    State(state): State<AppState>,
    Path(file_id): Path<Uuid>,
) -> Result<Json<SharedFileView>, AppError> {
    Ok(Json(state.services.share.get(file_id).await?))
}

pub(crate) async fn shared_children(
    State(state): State<AppState>,
    Path(file_id): Path<Uuid>,
) -> Result<Json<Vec<SharedFileView>>, AppError> {
    Ok(Json(state.services.share.list(file_id).await?))
}

pub(crate) async fn shared_download(
    State(state): State<AppState>,
    Path(file_id): Path<Uuid>,
) -> Result<Response, AppError> {
    let (name, body) = state.services.share.download(file_id).await?;

    body.into_response(name).await
}

pub(crate) async fn shared_download_chunk(
    State(state): State<AppState>,
    Path(file_id): Path<Uuid>,
    range: Option<TypedHeader<Range>>,
) -> Result<Response, AppError> {
    let (entry, path, mime) = state.services.share.content(file_id).await?;

    chunked_download(&path, &entry.name, mime, range).await
}

pub(crate) async fn shared_preview(
    State(state): State<AppState>,
    Path(file_id): Path<Uuid>,
    Query(query): Query<PreviewQuery>,
) -> Result<Response, AppError> {
    let size = PreviewSize::from_px(query.size.unwrap_or(64))?;
    let preview = state.services.share.preview(file_id, size).await?;

    Response::builder()
        .header(header::CONTENT_TYPE, preview.mime)
        .header(header::CONTENT_LENGTH, preview.len.to_string())
        .header(header::CACHE_CONTROL, "public, max-age=86400")
        .body(Body::from_stream(ReaderStream::new(preview.file)))
        .map_err(|error| {
            tracing::error!(%error, "building shared preview response");
            AppError::Internal
        })
}

pub(crate) async fn share_files(
    Path(root_id): Path<Uuid>,
    State(state): State<AppState>,
    session: Session,
    Json(input): Json<ShareFiles>,
) -> Result<StatusCode, AppError> {
    state
        .services
        .files
        .share_files(&session.user, root_id, input)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn unshare_files(
    Path(root_id): Path<Uuid>,
    State(state): State<AppState>,
    session: Session,
    Json(input): Json<UnshareFiles>,
) -> Result<StatusCode, AppError> {
    state
        .services
        .files
        .unshare_files(&session.user, root_id, input)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}
