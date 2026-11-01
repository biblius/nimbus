//! Thin HTTP client for the Nimbus `/api` surface used by the CLI.

use anyhow::{Context, Result, bail};
use futures_util::TryStreamExt;
use indicatif::ProgressBar;
use reqwest::{
    Body, Client, StatusCode,
    header::{AUTHORIZATION, CONTENT_DISPOSITION, CONTENT_LENGTH, HeaderMap, HeaderValue},
    multipart,
};
use serde::Deserialize;
use serde_json::Value;
use std::path::{Path, PathBuf};
use tokio::{
    fs::File,
    io::{AsyncWriteExt, BufWriter},
};
use tokio_util::codec::{BytesCodec, FramedRead};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorBody {
    pub message: Option<String>,
    #[allow(dead_code)]
    pub code: Option<usize>,
    #[allow(dead_code)]
    pub body: Option<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(
    tag = "status",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum LoginResult {
    Session { session_id: Uuid },
    TwoFactorRequired { token: String },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RootView {
    pub id: Uuid,
    pub name: String,
    pub quota: i64,
    pub owner: Option<Uuid>,
    pub access_mode: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileView {
    pub file: FileEntry,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub id: Uuid,
    pub kind: FileKind,
    pub name: String,
    pub size_bytes: i64,
    pub parent: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileKind {
    File,
    Directory,
}

pub struct Api {
    client: Client,
    base: String,
}

impl Api {
    pub fn new(server: &str, token: Option<&str>) -> Result<Self> {
        let mut headers = HeaderMap::new();
        if let Some(token) = token {
            let value = HeaderValue::from_str(&format!("Bearer {token}"))
                .context("invalid session token for Authorization header")?;
            headers.insert(AUTHORIZATION, value);
        }

        let client = Client::builder()
            .default_headers(headers)
            .build()
            .context("unable to build HTTP client")?;

        Ok(Self {
            client,
            base: format!("{}/api", server.trim_end_matches('/')),
        })
    }

    pub async fn login(
        &self,
        username: &str,
        password: &str,
        permanent: bool,
    ) -> Result<LoginResult> {
        let response = self
            .client
            .post(format!("{}/auth/login", self.base))
            .json(&serde_json::json!({
                "username": username,
                "password": password,
                "remember": false,
                "permanent": permanent,
            }))
            .send()
            .await
            .context("login request failed")?;

        Self::json(response).await
    }

    pub async fn login_otp(&self, token: &str, otp_code: &str) -> Result<LoginResult> {
        let response = self
            .client
            .post(format!("{}/auth/login/otp", self.base))
            .json(&serde_json::json!({
                "token": token,
                "otpCode": otp_code,
            }))
            .send()
            .await
            .context("OTP login request failed")?;

        Self::json(response).await
    }

    pub async fn roots(&self) -> Result<Vec<RootView>> {
        let response = self
            .client
            .get(format!("{}/roots", self.base))
            .send()
            .await
            .context("list roots request failed")?;

        Self::json(response).await
    }

    pub async fn list_files(&self, root_id: Uuid, parent: Option<Uuid>) -> Result<Vec<FileView>> {
        let mut request = self.client.get(format!("{}/roots/{root_id}", self.base));

        if let Some(parent) = parent {
            request = request.query(&[("parent", parent.to_string())]);
        }

        let response = request.send().await.context("list files request failed")?;

        Self::json(response).await
    }

    pub async fn search(&self, root_id: Uuid, q: &str) -> Result<Vec<FileView>> {
        let response = self
            .client
            .get(format!("{}/roots/{root_id}/search", self.base))
            .query(&[("q", q)])
            .send()
            .await
            .context("search request failed")?;

        Self::json(response).await
    }

    /// Download one file via `GET /roots/{root_id}/files/{file_id}/blob/download`.
    ///
    /// If `dest` is a directory (or omitted as `.`), the server filename is used.
    pub async fn download_file(
        &self,
        root_id: Uuid,
        file_id: Uuid,
        dest: &Path,
        progress: &ProgressBar,
    ) -> Result<PathBuf> {
        let response = self
            .client
            .get(format!(
                "{}/roots/{root_id}/files/{file_id}/blob/download",
                self.base
            ))
            .send()
            .await
            .context("download request failed")?;

        let status = response.status();

        if !status.is_success() {
            let bytes = response
                .bytes()
                .await
                .context("unable to read error response")?;
            bail!("{}", format_error(status, &bytes));
        }

        if let Some(len) = response
            .headers()
            .get(CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok())
        {
            progress.set_length(len);
        }

        let filename = filename_from_disposition(response.headers().get(CONTENT_DISPOSITION))
            .unwrap_or_else(|| file_id.to_string());

        let path = if dest.is_dir() {
            dest.join(&filename)
        } else {
            dest.to_path_buf()
        };

        let file = File::create(&path)
            .await
            .with_context(|| format!("unable to create {}", path.display()))?;

        let mut writer = BufWriter::new(file);
        let mut stream = response.bytes_stream();

        while let Some(chunk) = stream
            .try_next()
            .await
            .context("error reading download stream")?
        {
            progress.inc(chunk.len() as u64);
            writer
                .write_all(&chunk)
                .await
                .with_context(|| format!("unable to write {}", path.display()))?;
        }

        writer
            .flush()
            .await
            .with_context(|| format!("unable to flush {}", path.display()))?;

        Ok(path)
    }

    /// Upload one or more regular files to `POST /roots/{root_id}`.
    pub async fn upload_files(
        &self,
        root_id: Uuid,
        parent: Option<Uuid>,
        files: &[(PathBuf, String)],
        progress: &ProgressBar,
    ) -> Result<Vec<FileEntry>> {
        if files.is_empty() {
            bail!("at least one file is required");
        }

        let form = build_multipart(parent, files, progress).await?;
        let response = self
            .client
            .post(format!("{}/roots/{root_id}", self.base))
            .multipart(form)
            .send()
            .await
            .context("upload request failed")?;

        Self::json(response).await
    }

    /// Upload a directory tree to `POST /roots/{root_id}/upload/dir`.
    ///
    /// Each `filename` must be a relative path using `/` separators
    /// (e.g. `Photos/2024/pic.jpg`).
    pub async fn upload_dir(
        &self,
        root_id: Uuid,
        parent: Option<Uuid>,
        files: &[(PathBuf, String)],
        progress: &ProgressBar,
    ) -> Result<Vec<FileEntry>> {
        if files.is_empty() {
            bail!("at least one file is required");
        }

        let form = build_multipart(parent, files, progress).await?;

        let response = self
            .client
            .post(format!("{}/roots/{root_id}/upload/dir", self.base))
            .multipart(form)
            .send()
            .await
            .context("directory upload request failed")?;

        Self::json(response).await
    }

    async fn json<T: for<'de> Deserialize<'de>>(response: reqwest::Response) -> Result<T> {
        let status = response.status();

        let bytes = response
            .bytes()
            .await
            .context("unable to read response body")?;

        if status.is_success() {
            serde_json::from_slice(&bytes).with_context(|| {
                format!(
                    "unable to parse success response ({status}): {}",
                    String::from_utf8_lossy(&bytes)
                )
            })
        } else {
            bail!("{}", format_error(status, &bytes))
        }
    }
}

fn filename_from_disposition(header: Option<&HeaderValue>) -> Option<String> {
    let value = header?.to_str().ok()?;
    let marker = "filename=\"";
    let start = value.find(marker)? + marker.len();
    let end = value[start..].find('"')? + start;
    let name = &value[start..end];
    if name.is_empty() || name.contains('/') || name.contains('\\') {
        None
    } else {
        Some(name.to_owned())
    }
}

fn format_error(status: StatusCode, bytes: &[u8]) -> String {
    if let Ok(body) = serde_json::from_slice::<ErrorBody>(bytes)
        && let Some(message) = body.message.filter(|m| !m.is_empty())
    {
        return format!("{status}: {message}");
    }

    let text = String::from_utf8_lossy(bytes);

    if text.trim().is_empty() {
        format!("{status}")
    } else {
        format!("{status}: {text}")
    }
}

async fn build_multipart(
    parent: Option<Uuid>,
    files: &[(PathBuf, String)],
    progress: &ProgressBar,
) -> Result<multipart::Form> {
    let mut form = multipart::Form::new();

    if let Some(parent) = parent {
        form = form.text("parent", parent.to_string());
    }

    for (path, filename) in files {
        let path: &Path = path.as_path();

        let file = File::open(path)
            .await
            .with_context(|| format!("unable to open {}", path.display()))?;

        let len = file
            .metadata()
            .await
            .with_context(|| format!("unable to read metadata for {}", path.display()))?
            .len();

        let bar = progress.clone();

        let stream = FramedRead::new(file, BytesCodec::new()).map_ok(move |bytes| {
            let chunk = bytes.freeze();
            bar.inc(chunk.len() as u64);
            chunk
        });

        let body = Body::wrap_stream(stream);

        let part = multipart::Part::stream_with_length(body, len)
            .file_name(filename.clone())
            .mime_str("application/octet-stream")
            .context("unable to build multipart file part")?;

        form = form.part("file", part);
    }

    Ok(form)
}
