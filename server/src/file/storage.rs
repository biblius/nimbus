use super::format::{FileType, SNIFF_LEN};
use crate::error::AppError;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::{fs::File, io::AsyncWriteExt};
use uuid::Uuid;

const DATA_DIR: &str = "data";

/// Content-addressed bytes on the local filesystem.
///
/// Database rows point at these blobs by hash so identical uploads share one file.
#[derive(Clone)]
pub struct Storage {
    /// Root directory of all files stored in Nimbus.
    ///
    /// See [crate::config::StartArgs::upload_dir]
    root: PathBuf,

    /// Directory for generated preview blobs (content-addressed by hash).
    ///
    /// See [crate::config::StartArgs::preview_dir]
    preview: PathBuf,

    /// Ephemeral files (in-progress uploads, zip archives).
    ///
    /// See [crate::config::StartArgs::tmp_dir]
    tmp: PathBuf,

    max_bytes: i64,
}

/// Per-file writer for uploads. Used to hash the file as it is being written.
#[derive(Debug)]
pub struct UploadWriter {
    /// The file name obtained from the client.
    name: String,

    /// A handle to the file opened by this writer.
    file: File,

    /// Continuously updates the hash based on the received chunks.
    hasher: Sha256,

    /// The current upload size, i.e. the amount of bytes processed.
    size: i64,

    /// The path to the temporary [file][Self::file] opened by this writer.
    temp: PathBuf,

    /// The maximum amount of bytes this writer can receive before it errors.
    max_bytes: i64,

    /// Leading bytes kept for type detection. Capped at [`SNIFF_LEN`].
    header: Vec<u8>,

    /// Used to control the Drop impl so as not to remove the file.
    committed: bool,
}

/// A fully processed temporary file created by an [UploadWriter].
#[derive(Debug)]
pub struct TempFile {
    /// See [UploadWriter::name].
    pub name: String,

    /// See [UploadWriter::temp].
    pub temp: PathBuf,

    /// The final file hash.
    pub hash: String,

    /// The final file size.
    pub size: i64,

    /// Type decided from the upload header and the file name.
    pub file_type: FileType,

    /// Used to control the Drop impl so as not to remove the file.
    committed: bool,
}

impl Storage {
    pub fn new(root: PathBuf, preview: PathBuf, tmp: PathBuf, max_bytes: usize) -> Self {
        Self {
            root,
            preview,
            tmp,
            max_bytes: max_bytes as i64,
        }
    }

    pub async fn prepare(&self) -> Result<(), AppError> {
        let data = self.root.join(DATA_DIR);

        tracing::debug!(path = %data.display(), "creating storage data directory");
        tokio::fs::create_dir_all(&data).await?;

        tracing::debug!(path = %self.tmp.display(), "creating storage tmp directory");
        tokio::fs::create_dir_all(&self.tmp).await?;

        tracing::debug!(path = %self.preview.display(), "creating preview directory");
        tokio::fs::create_dir_all(&self.preview).await?;

        Ok(())
    }

    /// Fresh path under the configured tmp directory (caller creates/removes the file).
    pub fn tmp_path(&self) -> PathBuf {
        self.tmp.join(Uuid::new_v4().to_string())
    }

    pub async fn writer(&self, name: impl Into<String>) -> Result<UploadWriter, AppError> {
        let temp = self.tmp_path();

        let file = File::create(&temp).await?;

        Ok(UploadWriter {
            name: name.into(),
            file,
            hasher: Sha256::new(),
            size: 0,
            temp,
            max_bytes: self.max_bytes,
            header: Vec::with_capacity(SNIFF_LEN),
            committed: false,
        })
    }

    /// Move a temporary file into the data directory. Returns whether this hash was new.
    pub async fn persist(&self, tmp: &mut TempFile) -> Result<bool, AppError> {
        let dest = self.path(&tmp.hash);

        if tokio::fs::try_exists(&dest).await? {
            // TempFile drop impl handles removing the tmp file
            return Ok(false);
        }

        tokio::fs::rename(&tmp.temp, &dest).await?;

        tmp.committed = true;

        Ok(true)
    }

    pub async fn remove(&self, hash: &str) -> Result<(), AppError> {
        let path = self.path(hash);

        match tokio::fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    pub fn path(&self, hash: &str) -> PathBuf {
        self.root.join(DATA_DIR).join(hash)
    }

    pub fn preview_path(&self, hash: &str) -> PathBuf {
        self.preview.join(hash)
    }

    /// Write preview bytes under their content hash. Returns whether this hash was new.
    pub async fn persist_preview(&self, hash: &str, bytes: &[u8]) -> Result<bool, AppError> {
        let dest = self.preview_path(hash);

        if tokio::fs::try_exists(&dest).await? {
            return Ok(false);
        }

        // Same directory as `dest` so rename stays on one filesystem.
        let temp = self.preview.join(format!(".{hash}.tmp"));

        tokio::fs::write(&temp, bytes).await?;

        match tokio::fs::rename(&temp, &dest).await {
            Ok(()) => Ok(true),
            Err(error) => {
                let _ = tokio::fs::remove_file(&temp).await;
                // Another worker may have won the race.
                if tokio::fs::try_exists(&dest).await? {
                    return Ok(false);
                }
                Err(error.into())
            }
        }
    }

    pub async fn remove_preview(&self, hash: &str) -> Result<(), AppError> {
        let path = self.preview_path(hash);

        match tokio::fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

impl UploadWriter {
    pub async fn write(&mut self, chunk: &[u8]) -> Result<(), AppError> {
        let next = self.size.saturating_add(chunk.len() as i64);

        if next > self.max_bytes {
            return Err(AppError::PayloadTooLarge);
        }

        self.hasher.update(chunk);

        let room = SNIFF_LEN.saturating_sub(self.header.len());
        if room > 0 {
            let take = room.min(chunk.len());
            self.header.extend_from_slice(&chunk[..take]);
        }

        self.file.write_all(chunk).await?;

        self.size = next;

        Ok(())
    }

    /// Flush this writer's buffer and return the temporary file representation.
    pub async fn finish(&mut self) -> Result<TempFile, AppError> {
        self.file.flush().await?;

        self.committed = true;

        let name = std::mem::take(&mut self.name);
        let file_type = FileType::infer(&name, &self.header);

        Ok(TempFile {
            name,
            temp: std::mem::take(&mut self.temp),
            hash: hex::encode(std::mem::replace(&mut self.hasher, Sha256::new()).finalize()),
            size: self.size,
            file_type,
            committed: false,
        })
    }
}

impl Drop for UploadWriter {
    fn drop(&mut self) {
        if !self.committed
            && let Err(e) = std::fs::remove_file(&self.temp)
        {
            tracing::error!(error=%e, "tmp upload cleanup failed: {:?}", self)
        }
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        if !self.committed
            && let Err(e) = std::fs::remove_file(&self.temp)
        {
            tracing::error!(error=%e, "tmp upload cleanup failed: {:?}", self)
        }
    }
}

impl AsRef<Path> for Storage {
    fn as_ref(&self) -> &Path {
        &self.root
    }
}
