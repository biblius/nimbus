mod files;
mod share;
mod trash;

pub use files::FileService;
pub use share::ShareService;
pub use trash::TrashFileService;

use crate::{
    db::user_folders::UserFolderRepository,
    error::AppError,
    file::{
        format::FileType,
        model::FileKind,
        storage::Storage,
        tree::{FileEntryTree, FileTree},
    },
    user::model::{Role, User},
};
use async_zip::{Compression, ZipEntryBuilder, tokio::write::ZipFileWriter};
use futures::io::copy;
use std::{collections::HashMap, path::PathBuf};
use tokio::io::DuplexStream;
use tokio_util::compat::TokioAsyncReadCompatExt;
use uuid::Uuid;

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

pub(crate) async fn accessible(
    folders: &UserFolderRepository,
    user: &User,
    root_id: Uuid,
) -> Result<(), AppError> {
    if user.role != Role::Admin && !folders.can_access(user.id, root_id).await? {
        return Err(AppError::Forbidden(
            "You do not have access to this folder".into(),
        ));
    }
    Ok(())
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
