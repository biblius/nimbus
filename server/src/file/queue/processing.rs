use crate::{
    db::files::FileRepository,
    error::AppError,
    file::{
        format::{DocFormat, FileType},
        model::{FileEntry, FileMetadata},
        storage::Storage,
    },
};
use image::{DynamicImage, imageops::FilterType};
use sha2::{Digest, Sha256};
use std::{io::Cursor, path::Path, process::Command, sync::Arc};
use tokio::sync::{Semaphore, mpsc};
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
    Docx,
}

/// Minimal payload for background preview generation.
#[derive(Debug)]
pub struct ProcessingRequest {
    root_id: Uuid,
    file_id: Uuid,
    hash: String,
    kind: ProcessingKind,
}

pub type ProcessingQueueHandle = mpsc::Sender<ProcessingRequest>;

/// Background worker that derives artifacts (previews, etc.) from uploaded files.
pub struct ProcessingQueue {
    requests: mpsc::Receiver<ProcessingRequest>,
    worker: ProcessingWorker,
}

impl ProcessingQueue {
    pub fn start(storage: Storage, files: FileRepository) -> ProcessingQueueHandle {
        let (tx, rx) = mpsc::channel(128);
        ProcessingQueue {
            requests: rx,
            worker: ProcessingWorker { storage, files },
        }
        ._start();
        tx
    }

    /// Enqueue a committed file for preview processing when its type needs it.
    pub async fn submit(handle: &ProcessingQueueHandle, file: &FileEntry) -> Result<(), AppError> {
        let kind = match file.file_type {
            Some(FileType::Image(_)) => ProcessingKind::Image,
            Some(FileType::Video(_)) => ProcessingKind::Video,
            Some(FileType::Doc(DocFormat::Docx)) => ProcessingKind::Docx,
            _ => return Ok(()),
        };

        let Some(hash) = file.hash.as_ref() else {
            tracing::error!(file = %file.id, "file queued for processing has no hash");
            return Err(AppError::Internal);
        };

        handle
            .send(ProcessingRequest {
                root_id: file.root_id,
                file_id: file.id,
                hash: hash.clone(),
                kind,
            })
            .await
            .map_err(|_| AppError::Internal)
    }

    /// Drain the channel and run jobs concurrently (bounded).
    #[instrument(skip(self))]
    fn _start(mut self) {
        tokio::spawn(async move {
            // Cap if the queue gets flooded with a massive amount of jobs
            let max_jobs = std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4)
                .clamp(2, 16);

            let semaphore = Arc::new(Semaphore::new(max_jobs));

            while let Some(request) = self.requests.recv().await {
                let Ok(permit) = semaphore.clone().acquire_owned().await else {
                    break;
                };

                let worker = self.worker.clone();

                tokio::spawn(async move {
                    if let Err(e) = worker.process(request).await {
                        tracing::error!(error=%e, "error processing file");
                    }
                    drop(permit);
                });
            }

            tracing::info!("Processing request channel closed, shutting down worker");
        });
    }
}

/// Cloneable side of the queue that runs a single preview job.
#[derive(Clone)]
struct ProcessingWorker {
    storage: Storage,
    files: FileRepository,
}

impl ProcessingWorker {
    #[instrument(skip(self, request), fields(root_id = %request.root_id, file_id = %request.file_id), err(Debug))]
    async fn process(&self, request: ProcessingRequest) -> Result<(), AppError> {
        match request.kind {
            ProcessingKind::Image => {
                self.process_image(request.root_id, request.file_id, request.hash)
                    .await
            }
            ProcessingKind::Video => {
                self.process_video(request.root_id, request.file_id, request.hash)
                    .await
            }
            ProcessingKind::Docx => {
                self.process_docx(request.root_id, request.file_id, request.hash)
                    .await
            }
        }
    }

    /// Resize an image to 64×64 and 256×256 WebP icons and store them content-addressed.
    #[instrument(skip(self, hash), fields(root_id = %root_id, file_id = %file_id), err(Debug))]
    async fn process_image(
        &self,
        root_id: Uuid,
        file_id: Uuid,
        hash: String,
    ) -> Result<(), AppError> {
        let source_path = self.storage.path(&hash);

        let pair = tokio::task::spawn_blocking(move || encode_preview_pair(&source_path))
            .await
            .map_err(|e| {
                tracing::error!(error=%e, "preview encoding task failed");
                AppError::Internal
            })??;

        self.store_previews(root_id, file_id, &pair).await
    }

    /// Extract one video frame with system `ffmpeg`, then store both WebP sizes.
    #[instrument(skip(self, hash), fields(root_id = %root_id, file_id = %file_id), err(Debug))]
    async fn process_video(
        &self,
        root_id: Uuid,
        file_id: Uuid,
        hash: String,
    ) -> Result<(), AppError> {
        let source_path = self.storage.path(&hash);

        let pair = tokio::task::spawn_blocking(move || encode_video_preview_pair(&source_path))
            .await
            .map_err(|e| {
                tracing::error!(error=%e, "video preview encoding task failed");
                AppError::Internal
            })??;

        self.store_previews(root_id, file_id, &pair).await
    }

    /// Convert DOCX to PDF and store it as `preview_blob`.
    #[instrument(skip(self, hash), fields(root_id = %root_id, file_id = %file_id), err(Debug))]
    async fn process_docx(
        &self,
        root_id: Uuid,
        file_id: Uuid,
        hash: String,
    ) -> Result<(), AppError> {
        let source_path = self.storage.path(&hash);

        let pdf = tokio::task::spawn_blocking(move || convert_docx_to_pdf(&source_path))
            .await
            .map_err(|e| {
                tracing::error!(error=%e, "DOCX PDF conversion task failed");
                AppError::Internal
            })??;

        let pdf_hash = hex::encode(Sha256::digest(&pdf));
        self.storage.persist_preview(&pdf_hash, &pdf).await?;

        let metadata = FileMetadata {
            preview_blob: Some(pdf_hash),
            ..Default::default()
        };

        self.files.set_metadata(root_id, file_id, &metadata).await?;

        tracing::info!(
            file=%file_id,
            preview_blob=%metadata.preview_blob.as_deref().unwrap_or_default(),
            "stored DOCX PDF preview blob"
        );

        Ok(())
    }

    async fn store_previews(
        &self,
        root_id: Uuid,
        file_id: Uuid,
        pair: &PreviewPair,
    ) -> Result<(), AppError> {
        let icon_hash = hex::encode(Sha256::digest(&pair.icon));
        let tile_hash = hex::encode(Sha256::digest(&pair.tile));

        self.storage.persist_preview(&icon_hash, &pair.icon).await?;
        self.storage.persist_preview(&tile_hash, &pair.tile).await?;

        let metadata = FileMetadata {
            preview_64: Some(icon_hash),
            preview_256: Some(tile_hash),
            ..Default::default()
        };

        self.files.set_metadata(root_id, file_id, &metadata).await?;

        tracing::info!(
            file=%file_id,
            preview_64=%metadata.preview_64.as_deref().unwrap_or_default(),
            preview_256=%metadata.preview_256.as_deref().unwrap_or_default(),
            "stored file thumbnails"
        );

        Ok(())
    }
}

/// Encoded WebP previews at both supported sizes.
struct PreviewPair {
    icon: Vec<u8>,
    tile: Vec<u8>,
}

/// Read a DOCX blob and convert it to PDF bytes.
fn convert_docx_to_pdf(path: &Path) -> Result<Vec<u8>, AppError> {
    let docx = std::fs::read(path).map_err(|e| {
        tracing::error!(error=%e, path=%path.display(), "failed to read DOCX for PDF preview");
        AppError::Internal
    })?;

    docxide_pdf::convert_docx_bytes_to_pdf_bytes(&docx).map_err(|e| {
        tracing::error!(error=%e, path=%path.display(), "docxide-pdf conversion failed");
        AppError::Internal
    })
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
