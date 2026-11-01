//! Persistent CLI state under `~/.nimbus/`.

use anyhow::{Context, Result, bail};
use std::{
    fs,
    path::{Path, PathBuf},
};

const DIR_NAME: &str = ".nimbus";
const TOKEN_FILE: &str = "token";
const SERVER_FILE: &str = "server";

fn nimbus_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().context("unable to resolve home directory")?;
    Ok(home.join(DIR_NAME))
}

fn ensure_dir() -> Result<PathBuf> {
    let dir = nimbus_dir()?;
    fs::create_dir_all(&dir).with_context(|| format!("unable to create {}", dir.display()))?;
    Ok(dir)
}

fn read_trimmed(path: &Path) -> Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(contents) => {
            let trimmed = contents.trim().to_owned();
            if trimmed.is_empty() {
                Ok(None)
            } else {
                Ok(Some(trimmed))
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("unable to read {}", path.display())),
    }
}

fn write_trimmed(path: &Path, value: &str) -> Result<()> {
    fs::write(path, format!("{}\n", value.trim()))
        .with_context(|| format!("unable to write {}", path.display()))
}

/// Normalize a server origin: strip trailing slash and optional `/api` suffix.
pub fn normalize_server(input: &str) -> Result<String> {
    let trimmed = input.trim().trim_end_matches('/');

    if trimmed.is_empty() {
        bail!("server URL cannot be empty");
    }

    let without_api = trimmed
        .strip_suffix("/api")
        .unwrap_or(trimmed)
        .trim_end_matches('/');

    if without_api.is_empty() {
        bail!("server URL cannot be empty");
    }

    let parsed = reqwest::Url::parse(without_api).context("invalid server URL")?;

    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        bail!("server URL must use http or https");
    }

    if parsed.host_str().is_none() {
        bail!("server URL must include a host");
    }

    // Origin only (no path/query), matching the desktop client's endpoint shape.
    let origin = format!(
        "{}://{}{}",
        parsed.scheme(),
        parsed.host_str().unwrap(),
        match parsed.port() {
            Some(port) => format!(":{port}"),
            None => String::new(),
        }
    );

    Ok(origin)
}

pub fn save_server(server: &str) -> Result<()> {
    let dir = ensure_dir()?;
    write_trimmed(&dir.join(SERVER_FILE), server)
}

pub fn load_server() -> Result<Option<String>> {
    let path = nimbus_dir()?.join(SERVER_FILE);
    read_trimmed(&path)
}

pub fn save_token(token: &str) -> Result<()> {
    let dir = ensure_dir()?;
    write_trimmed(&dir.join(TOKEN_FILE), token)
}

pub fn load_token() -> Result<Option<String>> {
    let path = nimbus_dir()?.join(TOKEN_FILE);
    read_trimmed(&path)
}

pub fn require_token() -> Result<String> {
    load_token()?.context("not logged in; run `nimbus login` first")
}

/// Resolve the server origin from CLI flag / env, falling back to the saved value.
pub fn resolve_server(cli_server: Option<&str>) -> Result<String> {
    if let Some(server) = cli_server {
        return normalize_server(server);
    }

    match load_server()? {
        Some(server) => normalize_server(&server),
        None => bail!(
            "server URL not set; pass `--server` (or set NIMBUS_SERVER), or run `nimbus login --server <url>`"
        ),
    }
}
