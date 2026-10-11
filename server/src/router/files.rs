use crate::{
    auth::session::Session,
    error::AppError,
    file::model::{CreateDirectory, FileEntry, RootView},
    state::AppState,
};
use axum::{
    Json,
    body::Body,
    extract::{Multipart, Path, Query, State},
    http::{StatusCode, header},
    response::Response,
};
use serde::Deserialize;
use tokio::fs::File;
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
    responses((status = 200, description = "Children of the directory", body = Vec<FileEntry>))
)]
pub(crate) async fn list(
    Path(root_id): Path<Uuid>,
    State(state): State<AppState>,
    session: Session,
    Query(query): Query<ParentQuery>,
) -> Result<Json<Vec<FileEntry>>, AppError> {
    Ok(Json(
        state
            .services
            .files
            .list(&session.user, root_id, query.parent)
            .await?,
    ))
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
    get,
    path = "/roots/{root_id}/files/{id}/download",
    params(
        ("root_id" = Uuid, Path, description = "Root id"),
        ("id" = Uuid, Path, description = "File id")
    ),
    responses((status = 200, description = "File bytes"))
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
            content_disposition(&entry.name),
        )
        .header(header::CONTENT_LENGTH, entry.size_bytes.to_string())
        .body(Body::from_stream(ReaderStream::new(file)))
        .map_err(|error| {
            tracing::error!(%error, "building download response");
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
    responses((status = 204, description = "File scheduled for deletion"))
)]
pub(crate) async fn delete_file(
    State(state): State<AppState>,
    session: Session,
    Path((_root_id, file_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    state.services.files.delete(&session.user, file_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

fn content_disposition(name: &str) -> String {
    let fallback: String = name
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
    format!("attachment; filename=\"{fallback}\"")
}
