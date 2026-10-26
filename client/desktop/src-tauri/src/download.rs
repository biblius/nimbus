//! Authenticated file download streamed straight to a user-chosen path.
//!
//! Webview `<a download>` / blob URLs do not save files in Tauri; this writes
//! via Rust so large zips never sit fully in JS memory.

use futures_util::TryStreamExt;
use serde::Serialize;
use std::path::PathBuf;
use tauri_plugin_http::reqwest;
use tokio::{
    fs::File,
    io::{AsyncWriteExt, BufWriter},
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadHttpResponse {
    status: u16,
    body: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum DownloadError {
    Io { message: String },
    Network { message: String },
}

impl DownloadError {
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

type DownloadResult<T> = Result<T, DownloadError>;

/// GET `url` and stream the body to `path`. Non-2xx responses leave no file.
#[tauri::command]
pub async fn download_to_path(
    url: String,
    // Bearer token; omit (`None`) for public shared downloads.
    token: Option<String>,
    path: PathBuf,
) -> DownloadResult<DownloadHttpResponse> {
    let client = reqwest::Client::new();
    let mut request = client.get(url);

    if let Some(token) = token {
        request = request.header(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {token}"),
        );
    }

    let response = request.send().await.map_err(DownloadError::network)?;
    let status = response.status().as_u16();

    if !response.status().is_success() {
        let text = response.text().await.map_err(DownloadError::network)?;
        let body = if text.is_empty() {
            None
        } else {
            match serde_json::from_str(&text) {
                Ok(value) => Some(value),
                Err(_) => Some(serde_json::Value::String(text)),
            }
        };
        return Ok(DownloadHttpResponse { status, body });
    }

    let mut stream = response.bytes_stream();
    let file = File::create(&path)
        .await
        .map_err(|error| DownloadError::io(format!("{}: {error}", path.display())))?;
    let mut writer = BufWriter::new(file);

    while let Some(chunk) = stream.try_next().await.map_err(DownloadError::network)? {
        writer
            .write_all(&chunk)
            .await
            .map_err(|error| DownloadError::io(format!("{}: {error}", path.display())))?;
    }

    writer
        .flush()
        .await
        .map_err(|error| DownloadError::io(format!("{}: {error}", path.display())))?;

    Ok(DownloadHttpResponse {
        status,
        body: None,
    })
}
