use crate::{
    db::MetadataRepository,
    error::AppError,
    file::{format::FileType, model::FileEntry, storage::Storage},
};
use image::{DynamicImage, imageops::FilterType};
use sha2::{Digest, Sha256};
use std::{io::Cursor, path::Path, process::Command};
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
            Some(FileType::Video(_)) => self.process_video(file).await,
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

        let source_path = self.storage.path(hash);

        let webp = tokio::task::spawn_blocking(move || encode_preview_webp(&source_path))
            .await
            .map_err(|e| {
                tracing::error!(error=%e, "preview encoding task failed");
                AppError::Internal
            })??;

        self.store_preview(file.id, &webp).await
    }

    /// Extract one video frame with system `ffmpeg`, then store the same 64×64 WebP preview.
    #[instrument(skip(self, file), fields(file_id = %file.id), err(Debug))]
    async fn process_video(&self, file: FileEntry) -> Result<(), AppError> {
        let hash = file.hash.as_deref().ok_or_else(|| {
            tracing::error!(file=%file.id, "video file has no hash");
            AppError::Internal
        })?;

        let source_path = self.storage.path(hash);

        let webp = tokio::task::spawn_blocking(move || encode_video_preview_webp(&source_path))
            .await
            .map_err(|e| {
                tracing::error!(error=%e, "video preview encoding task failed");
                AppError::Internal
            })??;

        self.store_preview(file.id, &webp).await
    }

    async fn store_preview(&self, file_id: uuid::Uuid, webp: &[u8]) -> Result<(), AppError> {
        let preview_hash = hex::encode(Sha256::digest(webp));

        self.storage.persist_preview(&preview_hash, webp).await?;
        self.metadata.set_preview(file_id, &preview_hash).await?;

        tracing::info!(
            file=%file_id,
            preview=%preview_hash,
            "stored file preview"
        );

        Ok(())
    }
}

/// Decode `path`, crop/resize to [`PREVIEW_SIZE`], and encode as WebP.
fn encode_preview_webp(path: &Path) -> Result<Vec<u8>, AppError> {
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

    resize_to_preview_webp(&img)
}

/// Pull one still frame from a video blob, then crop/resize to the shared WebP icon.
///
/// Asks system `ffmpeg` for a single PNG frame from `path`.
///
/// Tries seek `1` first; if that fails (clip shorter than 1 second), retries at the start.
fn encode_video_preview_webp(path: &Path) -> Result<Vec<u8>, AppError> {
    // Prefer a frame one second in so the thumbnail is
    // less likely to be a black/fade-in first frame.
    let png = match run_ffmpeg_frame(path, "1") {
        Ok(bytes) => bytes,
        Err(first) => {
            tracing::warn!(
                error=%first,
                path=%path.display(),
                seek="1",
                "ffmpeg seek failed; retrying at start of video"
            );
            run_ffmpeg_frame(path, "0")?
        }
    };
    let img = image::load_from_memory(&png).map_err(|e| {
        tracing::error!(error=%e, path=%path.display(), "failed to decode ffmpeg frame");
        AppError::Internal
    })?;
    resize_to_preview_webp(&img)
}

fn resize_to_preview_webp(img: &DynamicImage) -> Result<Vec<u8>, AppError> {
    let icon = img.resize_to_fill(PREVIEW_SIZE, PREVIEW_SIZE, FilterType::Triangle);

    let mut buf = Vec::new();

    icon.write_to(&mut Cursor::new(&mut buf), image::ImageFormat::WebP)
        .map_err(|e| {
            tracing::error!(error=%e, "failed to encode webp preview");
            AppError::Internal
        })?;

    Ok(buf)
}

/// Run ffmpeg to extract one PNG frame to stdout.
///
/// Equivalent to:
/// `ffmpeg -hide_banner -loglevel error -y -ss <seek> -i <path> -frames:v 1 -f image2pipe -vcodec png -`
fn run_ffmpeg_frame(path: &Path, seek_secs: &str) -> Result<Vec<u8>, AppError> {
    let mut cmd = Command::new("ffmpeg");

    // Omit the version/config banner from stderr.
    cmd.arg("-hide_banner");
    // Only print real errors (keeps stderr useful when the command fails).
    cmd.arg("-loglevel").arg("error");
    // Overwrite outputs without prompting (harmless when writing to stdout).
    cmd.arg("-y");
    // Seek to this timestamp *before* opening the input (fast keyframe seek).
    cmd.arg("-ss").arg(seek_secs);
    // Input file set to the uploaded file's path
    cmd.arg("-i").arg(path);
    // Decode/emit exactly one video frame.
    cmd.arg("-frames:v").arg("1");
    // Write a still-image bitstream to the pipe (not a media container).
    cmd.arg("-f").arg("image2pipe");
    // Encode that still as PNG.
    cmd.arg("-vcodec").arg("png");
    // Write the PNG bytes to stdout (captured below).
    cmd.arg("-");

    let output = cmd.output().map_err(|e| {
        tracing::error!(error=%e, "failed to spawn ffmpeg; is it installed?");
        AppError::Internal
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::error!(
            status=?output.status.code(),
            stderr=%stderr,
            path=%path.display(),
            seek=seek_secs,
            "ffmpeg failed to extract video frame"
        );
        return Err(AppError::Internal);
    }

    if output.stdout.is_empty() {
        tracing::error!(
            path=%path.display(),
            seek=seek_secs,
            "ffmpeg produced empty stdout for video frame"
        );
        return Err(AppError::Internal);
    }

    Ok(output.stdout)
}
