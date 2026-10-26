//! Multipart file upload with byte progress, streamed from disk.
//!
//! Uses reqwest (via tauri-plugin-http) so we keep one HTTP stack and can
//! report progress that `@tauri-apps/plugin-http` fetch cannot.

use futures_util::TryStreamExt;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};
use tauri::{ipc::Channel, State};
use tauri_plugin_http::reqwest::{self, multipart, Body};
use tokio::{
    fs::File,
    sync::{oneshot, Mutex},
};
use tokio_util::codec::{BytesCodec, FramedRead};

/// In-flight uploads keyed by id so the frontend can abort via `cancel_upload`.
pub struct UploadRegistry {
    cancels: Mutex<HashMap<u32, oneshot::Sender<()>>>,
}

impl UploadRegistry {
    pub fn new() -> Self {
        Self {
            cancels: Mutex::new(HashMap::new()),
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadProgressEvent {
    loaded: u64,
    total: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadFilePart {
    path: PathBuf,
    /// Multipart filename: basename, or relative path for directory uploads.
    filename: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadHttpResponse {
    status: u16,
    body: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum UploadError {
    Aborted,
    Io { message: String },
    Network { message: String },
}

impl UploadError {
    fn io(error: impl ToString) -> Self {
        Self::Io {
            message: error.to_string(),
        }
    }

    fn network(error: impl ToString) -> Self {
        Self::Network {
            message: error.to_string(),
        }
    }
}

type UploadResult<T> = Result<T, UploadError>;

/// Stream files as `multipart/form-data` (`parent` + `file` fields) with progress.
#[tauri::command]
pub async fn upload_multipart(
    registry: State<'_, UploadRegistry>,
    upload_id: u32,
    url: String,
    token: String,
    parent: Option<String>,
    files: Vec<UploadFilePart>,
    on_progress: Channel<UploadProgressEvent>,
) -> UploadResult<UploadHttpResponse> {
    if files.is_empty() {
        return Err(UploadError::io("at least one file is required"));
    }

    let (cancel_tx, cancel_rx) = oneshot::channel();

    registry.cancels.lock().await.insert(upload_id, cancel_tx);

    let outcome = tokio::select! {
        biased;
        _ = cancel_rx => Err(UploadError::Aborted),
        result = send_multipart(url, token, parent, files, on_progress) => result,
    };

    registry.cancels.lock().await.remove(&upload_id);

    outcome
}

/// Abort an in-flight `upload_multipart` by id.
#[tauri::command]
pub async fn cancel_upload(
    registry: State<'_, UploadRegistry>,
    upload_id: u32,
) -> Result<(), String> {
    if let Some(tx) = registry.cancels.lock().await.remove(&upload_id) {
        let _ = tx.send(());
    }
    Ok(())
}

async fn send_multipart(
    url: String,
    token: String,
    parent: Option<String>,
    files: Vec<UploadFilePart>,
    on_progress: Channel<UploadProgressEvent>,
) -> UploadResult<UploadHttpResponse> {
    let mut total = 0u64;
    let mut opened = Vec::with_capacity(files.len());

    for part in files {
        let file = File::open(&part.path)
            .await
            .map_err(|error| UploadError::io(format!("{}: {error}", part.path.display())))?;

        let len = file
            .metadata()
            .await
            .map_err(|error| UploadError::io(format!("{}: {error}", part.path.display())))?
            .len();

        total = total.saturating_add(len);
        opened.push((part.filename, file, len));
    }

    let loaded = Arc::new(AtomicU64::new(0));
    let mut form = multipart::Form::new();

    if let Some(parent) = parent {
        form = form.text("parent", parent);
    }

    for (filename, file, len) in opened {
        let _loaded = loaded.clone();
        let _on_progress = on_progress.clone();

        let stream = FramedRead::new(file, BytesCodec::new()).map_ok(move |bytes| {
            let chunk = bytes.freeze();
            let n = chunk.len() as u64;
            let loaded = _loaded.fetch_add(n, Ordering::Relaxed) + n;
            let _ = _on_progress.send(UploadProgressEvent { loaded, total });
            chunk
        });

        let body = Body::wrap_stream(stream);

        let part = multipart::Part::stream_with_length(body, len)
            .file_name(filename)
            .mime_str("application/octet-stream")
            .map_err(UploadError::io)?;

        form = form.part("file", part);
    }

    // Emit a zero tick so the UI can show 0% before the first chunk.
    let _ = on_progress.send(UploadProgressEvent { loaded: 0, total });

    let client = reqwest::Client::new();

    let response = client
        .post(url)
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"))
        .multipart(form)
        .send()
        .await
        .map_err(UploadError::network)?;

    let status = response.status().as_u16();

    let text = response.text().await.map_err(UploadError::network)?;

    let body = if text.is_empty() {
        None
    } else {
        match serde_json::from_str(&text) {
            Ok(value) => Some(value),
            Err(_) => Some(serde_json::Value::String(text)),
        }
    };

    // Ensure the bar reaches 100% even if the last progress event was skipped.
    let _ = on_progress.send(UploadProgressEvent {
        loaded: total,
        total,
    });

    Ok(UploadHttpResponse { status, body })
}
