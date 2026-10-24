mod files;
mod share;
mod trash;

pub use files::FileService;
pub use share::ShareService;
pub use trash::TrashFileService;

use crate::{
    activity::{
        Activity, ActivityData, ActivityQueue, ActivityQueueHandle, queue::ProcessActivity,
    },
    error::AppError,
    file::{
        UserRootContext,
        format::{DocFormat, FileType},
        model::{FileEntry, FileKind},
        storage::Storage,
        tree::{FileEntryTree, FileTree},
    },
};
use async_zip::{Compression, ZipEntryBuilder, tokio::write::ZipFileWriter};
use futures::io::copy;
use std::{collections::HashMap, path::PathBuf};
use tokio::io::DuplexStream;
use tokio_util::compat::TokioAsyncReadCompatExt;
use uuid::Uuid;

/// Resolve the on-disk path and MIME for inline preview content.
///
/// Prefer `preview_blob` when present (derived payloads such as DOCX → PDF).
/// Otherwise return the original blob when the type can be previewed as-is.
pub(crate) fn preview_content_for(
    storage: &Storage,
    file: &FileEntry,
) -> Result<(PathBuf, &'static str), AppError> {
    if file.kind != FileKind::File {
        return Err(AppError::Unprocessable(
            "Directories cannot be previewed".into(),
        ));
    }

    if let Some(hash) = file.metadata.preview_blob.as_deref() {
        let mime = match file.file_type.as_ref() {
            Some(FileType::Doc(DocFormat::Docx)) => "application/pdf",
            Some(file_type) => file_type.mime(),
            None => "application/octet-stream",
        };
        return Ok((storage.preview_path(hash), mime));
    }

    let file_type = file
        .file_type
        .as_ref()
        .ok_or_else(|| AppError::Unprocessable("File type is required for preview".into()))?;

    if matches!(file_type, FileType::Doc(DocFormat::Docx)) {
        return Err(AppError::ResourceMissing("Preview does not exist".into()));
    }

    if !file.previewable_as_original() {
        return Err(AppError::Unprocessable(
            "File type cannot be previewed".into(),
        ));
    }

    let hash = file.hash.as_deref().ok_or(AppError::Internal)?;
    Ok((storage.path(hash), file_type.mime()))
}

/// HTTP download payload: a file on disk, or a zip streamed as it is built.
#[derive(Debug)]
pub enum DownloadBody {
    File { path: PathBuf, mime: &'static str },
    Zip { reader: DuplexStream },
}

/// Resolve on-disk blob paths for file rows in `children`. Missing blobs are skipped.
pub(crate) async fn blob_paths_for(
    storage: &Storage,
    children: &[FileEntryTree],
) -> Result<HashMap<Uuid, PathBuf>, AppError> {
    let mut blob_paths = HashMap::new();

    for child in children.iter() {
        if child.kind != FileKind::File {
            continue;
        }

        let Some(hash) = child.hash.as_deref() else {
            tracing::error!(
                id = %child.id,
                name = %child.name,
                "file row has no hash, will not be downloaded"
            );
            continue;
        };

        let path = storage.path(hash);

        if !tokio::fs::try_exists(&path).await? {
            tracing::error!(
                id = %child.id,
                name = %child.name,
                hash,
                "file row has no blob on disk, will not be downloaded"
            );
            continue;
        }

        blob_paths.insert(child.id, path);
    }

    Ok(blob_paths)
}

/// Stream `tree` as a zip archive over a duplex pipe.
pub(crate) fn zip_directory_stream(tree: FileTree, blobs: HashMap<Uuid, PathBuf>) -> DownloadBody {
    let (writer, reader) = tokio::io::duplex(256 * 1024);

    tokio::spawn(async move {
        if let Err(error) = write_zip(writer, tree, blobs).await {
            tracing::error!(%error, "directory zip stream failed");
        }
    });

    DownloadBody::Zip { reader }
}

async fn write_zip(
    writer: DuplexStream,
    tree: FileTree,
    blobs: HashMap<Uuid, PathBuf>,
) -> Result<(), AppError> {
    let mut zip = ZipFileWriter::with_tokio(writer);
    let mut seen = HashMap::new();

    zip_tree(&mut zip, &tree, "", &blobs, &mut seen).await?;

    zip.close().await.map_err(|error| {
        tracing::error!(%error, "failed to finish zip archive");
        AppError::Internal
    })?;

    Ok(())
}

async fn zip_tree(
    zip: &mut ZipFileWriter<DuplexStream>,
    node: &FileTree,
    prefix: &str,
    blob_paths: &HashMap<Uuid, PathBuf>,
    seen: &mut HashMap<String, usize>,
) -> Result<(), AppError> {
    let mut path = if prefix.is_empty() {
        node.name.clone()
    } else {
        format!("{prefix}/{}", node.name)
    };

    let count = match seen.get_mut(&path) {
        Some(count) => {
            *count += 1;
            *count
        }
        None => {
            seen.insert(path.clone(), 0);
            0
        }
    };

    if count > 0 {
        path = if let Some((prefix, ext)) = path.rsplit_once('.') {
            format!("{prefix} ({count}).{ext}")
        } else {
            format!("{path} ({count})")
        };
    }

    match node.kind {
        FileKind::Directory => {
            let entry = ZipEntryBuilder::new(format!("{path}/").into(), Compression::Stored);

            zip.write_entry_whole(entry, &[]).await.map_err(|error| {
                tracing::error!(%error, "failed to add directory to zip");
                AppError::Internal
            })?;

            for child in node.children.iter() {
                Box::pin(zip_tree(zip, child, &path, blob_paths, seen)).await?;
            }
        }
        FileKind::File => {
            let source = blob_paths.get(&node.id).ok_or(AppError::Internal)?;

            let compression = match node.file_type {
                // Usually already compressed
                Some(
                    FileType::Image(_)
                    | FileType::Video(_)
                    | FileType::Audio(_)
                    | FileType::Archive(_)
                    | FileType::Book(_), // epub is zip-based
                ) => Compression::Stored,
                // Deflate usually wins on size
                Some(
                    FileType::Text(_)
                    | FileType::Doc(_)
                    | FileType::Font(_)
                    | FileType::App(_)
                    | FileType::Other(_),
                )
                | None => Compression::Deflate,
            };

            let entry = ZipEntryBuilder::new(path.into(), compression);

            let mut entry_writer = zip.write_entry_stream(entry).await.map_err(|error| {
                tracing::error!(%error, "failed to start zip file entry");
                AppError::Internal
            })?;

            let file = tokio::fs::File::open(source).await?;

            copy(&mut file.compat(), &mut entry_writer)
                .await
                .map_err(|error| {
                    tracing::error!(%error, "failed to write zip file entry");
                    AppError::Internal
                })?;

            entry_writer.close().await.map_err(|error| {
                tracing::error!(%error, "failed to close zip file entry");
                AppError::Internal
            })?;
        }
    }

    Ok(())
}

pub(super) async fn activity(
    handle: &ActivityQueueHandle,
    ctx: &UserRootContext,
    data: ActivityData,
) {
    if let Err(e) = ActivityQueue::submit(
        handle,
        ProcessActivity::Process(Activity::new(ctx.root.id, ctx.user.username.clone(), data)),
    )
    .await
    {
        tracing::error!(error = %e, "unable to submit activity to queue");
    };
}
