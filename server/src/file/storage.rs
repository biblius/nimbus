use crate::error::AppError;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::{fs::File, io::AsyncWriteExt};
use uuid::Uuid;

const TMP_DIR: &str = "tmp";
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
    max_bytes: i64,
}

/// Per-file writer for uploads.
#[derive(Debug)]
pub struct UploadWriter {
    /// The initial file name obtained from the client.
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

    /// Used to control the Drop impl so as not to remove the file.
    committed: bool,
}

impl Storage {
    pub fn new(root: PathBuf, max_bytes: usize) -> Self {
        Self {
            root,
            max_bytes: max_bytes as i64,
        }
    }

    pub async fn prepare(&self) -> Result<(), AppError> {
        tokio::fs::create_dir_all(self.root.join(DATA_DIR)).await?;
        tokio::fs::create_dir_all(self.root.join(TMP_DIR)).await?;
        Ok(())
    }

    pub async fn writer(&self, file_name: String) -> Result<UploadWriter, AppError> {
        let temp = self.root.join(TMP_DIR).join(Uuid::new_v4().to_string());

        let file = File::create(&temp).await?;

        Ok(UploadWriter {
            name: file_name,
            file,
            hasher: Sha256::new(),
            size: 0,
            temp,
            max_bytes: self.max_bytes,
            committed: false,
        })
    }

    /// Move a temporary file into the data directory. Returns whether this hash was new.
    pub async fn persist(&self, tmp: &mut TempFile) -> Result<bool, AppError> {
        let dest = self.path(&tmp.hash);

        if tokio::fs::try_exists(&dest).await? {
            let _ = tokio::fs::remove_file(&tmp.temp).await;
            tmp.committed = true;
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
}

impl UploadWriter {
    pub async fn write(&mut self, chunk: &[u8]) -> Result<(), AppError> {
        let next = self.size.saturating_add(chunk.len() as i64);

        if next > self.max_bytes {
            return Err(AppError::PayloadTooLarge);
        }

        self.hasher.update(chunk);

        self.file.write_all(chunk).await?;

        self.size = next;

        Ok(())
    }

    /// Flush this writer's buffer and return the temporary file representation.
    pub async fn finish(&mut self) -> Result<TempFile, AppError> {
        self.file.flush().await?;

        self.committed = true;

        Ok(TempFile {
            name: self.name.clone(),
            temp: self.temp.clone(),
            hash: hex::encode(self.hasher.clone().finalize()),
            size: self.size,
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
