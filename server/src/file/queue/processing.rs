use crate::{
    db::files::FileRepository,
    error::AppError,
    file::{
        format::FileType,
        model::{FileEntry, FileMetadata},
        storage::Storage,
    },
};
use image::{DynamicImage, imageops::FilterType};
use sha2::{Digest, Sha256};
use std::{io::Cursor, path::Path, process::Command};
use tokio::sync::mpsc;
use tracing::instrument;
use uuid::Uuid;

/// List / small-tile icon size.
const PREVIEW_ICON: u32 = 64;
/// Large-tile thumbnail size.
const PREVIEW_TILE: u32 = 256;

/// Kind of preview work to run for a committed file.
#[derive(Debug, Clone, Copy)]
enum ProcessingKind {
    Image,
    Video,
}

/// Minimal payload for background preview generation.
#[derive(Debug)]
pub struct ProcessingRequest {
    file_id: Uuid,
    hash: String,
    kind: ProcessingKind,
}

pub type ProcessingQueueHandle = mpsc::Sender<ProcessingRequest>;

/// Encoded WebP previews at both supported sizes.
struct PreviewPair {
    icon: Vec<u8>,
    tile: Vec<u8>,
}

/// Background worker that derives artifacts (previews, etc.) from uploaded files.
pub struct ProcessingQueue {
    requests: mpsc::Receiver<ProcessingRequest>,
    storage: Storage,
    files: FileRepository,
}

impl ProcessingQueue {
    pub fn start(storage: Storage, files: FileRepository) -> ProcessingQueueHandle {
        let (tx, rx) = mpsc::channel(128);
        ProcessingQueue::new(storage, files, rx)._start();
        tx
    }

    fn new(
        storage: Storage,
        files: FileRepository,
        requests: mpsc::Receiver<ProcessingRequest>,
    ) -> Self {
        Self {
            requests,
            storage,
            files,
        }
    }

    /// Enqueue a committed file for preview processing when its type needs it.
    pub async fn submit(handle: &ProcessingQueueHandle, file: &FileEntry) -> Result<(), AppError> {
        let kind = match file.file_type {
            Some(FileType::Image(_)) => ProcessingKind::Image,
            Some(FileType::Video(_)) => ProcessingKind::Video,
            _ => return Ok(()),
        };

        let Some(hash) = file.hash.as_ref() else {
            tracing::error!(file = %file.id, "file queued for processing has no hash");
            return Err(AppError::Internal);
        };

        handle
            .send(ProcessingRequest {
                file_id: file.id,
                hash: hash.clone(),
                kind,
            })
            .await
            .map_err(|_| AppError::Internal)
    }

    #[instrument(skip(self))]
    fn _start(mut self) {
        tokio::spawn(async move {
            while let Some(request) = self.requests.recv().await {
                if let Err(e) = self.process(request).await {
                    tracing::error!(error=%e, "error processing file");
                }
            }

            tracing::info!("Processing request channel closed, shutting down worker");
        });
    }

    #[instrument(skip(self, request), fields(file_id = %request.file_id), err(Debug))]
    async fn process(&self, request: ProcessingRequest) -> Result<(), AppError> {
        match request.kind {
            ProcessingKind::Image => self.process_image(request.file_id, request.hash).await,
            ProcessingKind::Video => self.process_video(request.file_id, request.hash).await,
        }
    }

    /// Resize an image to 64×64 and 256×256 WebP icons and store them content-addressed.
    #[instrument(skip(self, hash), fields(file_id = %file_id), err(Debug))]
    async fn process_image(&self, file_id: Uuid, hash: String) -> Result<(), AppError> {
        let source_path = self.storage.path(&hash);

        let pair = tokio::task::spawn_blocking(move || encode_preview_pair(&source_path))
            .await
            .map_err(|e| {
                tracing::error!(error=%e, "preview encoding task failed");
                AppError::Internal
            })??;

        self.store_previews(file_id, &pair).await
    }

    /// Extract one video frame with system `ffmpeg`, then store both WebP sizes.
    #[instrument(skip(self, hash), fields(file_id = %file_id), err(Debug))]
    async fn process_video(&self, file_id: Uuid, hash: String) -> Result<(), AppError> {
        let source_path = self.storage.path(&hash);

        let pair = tokio::task::spawn_blocking(move || encode_video_preview_pair(&source_path))
            .await
            .map_err(|e| {
                tracing::error!(error=%e, "video preview encoding task failed");
                AppError::Internal
            })??;

        self.store_previews(file_id, &pair).await
    }

    async fn store_previews(&self, file_id: uuid::Uuid, pair: &PreviewPair) -> Result<(), AppError> {
        let icon_hash = hex::encode(Sha256::digest(&pair.icon));
        let tile_hash = hex::encode(Sha256::digest(&pair.tile));

        self.storage.persist_preview(&icon_hash, &pair.icon).await?;
        self.storage.persist_preview(&tile_hash, &pair.tile).await?;

        let metadata = FileMetadata {
            preview_64: Some(icon_hash),
            preview_256: Some(tile_hash),
        };

        self.files.set_metadata(file_id, &metadata).await?;

        tracing::info!(
            file=%file_id,
            preview_64=%metadata.preview_64.as_deref().unwrap_or_default(),
            preview_256=%metadata.preview_256.as_deref().unwrap_or_default(),
            "stored file previews"
        );

        Ok(())
    }
}

/// Decode `path` and encode both preview sizes as WebP.
fn encode_preview_pair(path: &Path) -> Result<PreviewPair, AppError> {
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

    resize_to_preview_pair(&img)
}

/// Pull one still frame from a video blob, then crop/resize to both WebP sizes.
///
/// Asks system `ffmpeg` for a single PNG frame from `path`.
///
/// Tries seek `1` first; if that fails (clip shorter than 1 second), retries at the start.
fn encode_video_preview_pair(path: &Path) -> Result<PreviewPair, AppError> {
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
    resize_to_preview_pair(&img)
}

fn resize_to_preview_pair(img: &DynamicImage) -> Result<PreviewPair, AppError> {
    Ok(PreviewPair {
        icon: encode_webp_square(img, PREVIEW_ICON)?,
        tile: encode_webp_square(img, PREVIEW_TILE)?,
    })
}

fn encode_webp_square(img: &DynamicImage, size: u32) -> Result<Vec<u8>, AppError> {
    let icon = img.resize_to_fill(size, size, FilterType::Triangle);

    let mut buf = Vec::new();

    icon.write_to(&mut Cursor::new(&mut buf), image::ImageFormat::WebP)
        .map_err(|e| {
            tracing::error!(error=%e, size, "failed to encode webp preview");
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
