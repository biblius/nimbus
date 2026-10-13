use crate::{
    db::MetadataRepository,
    error::AppError,
    file::{format::FileType, model::FileEntry, storage::Storage},
};
use image::imageops::FilterType;
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tokio::sync::mpsc;
use tracing::instrument;

/// Nextcloud's smallest square preview size used for list/icon views.
const PREVIEW_SIZE: u32 = 64;

/// A committed file entry ready for background processing.
#[derive(Debug)]
pub struct ProcessingRequest {
    pub file: FileEntry,
}

pub type ProcessingQueueHandle = mpsc::Sender<ProcessingRequest>;

/// Background worker that derives artifacts (previews, etc.) from uploaded files.
pub struct ProcessingQueue {
    requests: mpsc::Receiver<ProcessingRequest>,
    storage: Storage,
    metadata: MetadataRepository,
}

impl ProcessingQueue {
    pub fn start(storage: Storage, metadata: MetadataRepository) -> ProcessingQueueHandle {
        let (tx, rx) = mpsc::channel(128);
        ProcessingQueue::new(storage, metadata, rx)._start();
        tx
    }

    fn new(
        storage: Storage,
        metadata: MetadataRepository,
        requests: mpsc::Receiver<ProcessingRequest>,
    ) -> Self {
        Self {
            requests,
            storage,
            metadata,
        }
    }

    /// Enqueue a committed file entry for processing.
    pub async fn submit(handle: &ProcessingQueueHandle, file: FileEntry) -> Result<(), AppError> {
        handle
            .send(ProcessingRequest { file })
            .await
            .map_err(|_| AppError::Internal)
    }

    #[instrument(skip(self))]
    fn _start(mut self) {
        tokio::spawn(async move {
            while let Some(request) = self.requests.recv().await {
                if let Err(e) = self.process(request.file).await {
                    tracing::error!(error=%e, "error processing file");
                }
            }

            tracing::info!("Processing request channel closed, shutting down worker");
        });
    }

    #[instrument(skip(self, file), fields(file_id = %file.id, name = %file.name), err(Debug))]
    async fn process(&self, file: FileEntry) -> Result<(), AppError> {
        match file.file_type {
            Some(FileType::Image(_)) => self.process_image(file).await,
            _ => Ok(()),
        }
    }

    /// Resize an image to a 64×64 WebP icon and store it content-addressed under the preview dir.
    #[instrument(skip(self, file), fields(file_id = %file.id), err(Debug))]
    async fn process_image(&self, file: FileEntry) -> Result<(), AppError> {
        let hash = file.hash.as_deref().ok_or_else(|| {
            tracing::error!(file=%file.id, "image file has no hash");
            AppError::Internal
        })?;

        let source = self.storage.path(hash);
        let source_path = source.clone();
        let webp = tokio::task::spawn_blocking(move || encode_preview_webp(&source_path))
            .await
            .map_err(|e| {
                tracing::error!(error=%e, "preview encoding task failed");
                AppError::Internal
            })??;

        let preview_hash = hex::encode(Sha256::digest(&webp));

        self.storage.persist_preview(&preview_hash, &webp).await?;
        self.metadata.set_preview(file.id, &preview_hash).await?;

        tracing::info!(
            file=%file.id,
            preview=%preview_hash,
            "stored image preview"
        );

        Ok(())
    }
}

/// Decode `path`, crop/resize to [`PREVIEW_SIZE`], and encode as WebP.
fn encode_preview_webp(path: &std::path::Path) -> Result<Vec<u8>, AppError> {
    // Blobs are stored by content hash with no extension; sniff magic bytes.
    let img = image::ImageReader::open(path)
        .map_err(|e| {
            tracing::error!(error=%e, path=%path.display(), "failed to open image");
            AppError::Internal
        })?
        .with_guessed_format()
        .map_err(|e| {
            tracing::error!(error=%e, path=%path.display(), "failed to guess image format");
            AppError::Internal
        })?
        .decode()
        .map_err(|e| {
            tracing::error!(error=%e, path=%path.display(), "failed to decode image");
            AppError::Internal
        })?;

    let icon = img.resize_to_fill(PREVIEW_SIZE, PREVIEW_SIZE, FilterType::Triangle);

    let mut buf = Vec::new();

    icon.write_to(&mut Cursor::new(&mut buf), image::ImageFormat::WebP)
        .map_err(|e| {
            tracing::error!(error=%e, "failed to encode webp preview");
            AppError::Internal
        })?;

    Ok(buf)
}
