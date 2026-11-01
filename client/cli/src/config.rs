//! Persistent CLI context in `~/.nimbus/config.json`.

use anyhow::{Context as _, Result, bail};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};
use uuid::Uuid;

const DIR_NAME: &str = ".nimbus";
const CONFIG_FILE: &str = "config.json";

/// Active CLI session state shared across commands.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Context {
    /// Server origin (scheme + host[+port], no `/api`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,

    /// Permanent session id used as `Authorization: Bearer`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<Uuid>,

    /// Active root for commands that operate inside a root.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_id: Option<Uuid>,
}

impl Context {
    pub fn load() -> Result<Self> {
        let path = config_path()?;
        match fs::read_to_string(&path) {
            Ok(contents) => {
                let trimmed = contents.trim();
                if trimmed.is_empty() {
                    Ok(Self::default())
                } else {
                    serde_json::from_str(trimmed)
                        .with_context(|| format!("invalid config at {}", path.display()))
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => {
                Err(error).with_context(|| format!("unable to read {}", path.display()))
            }
        }
    }

    pub fn save(&self) -> Result<()> {
        let dir = nimbus_dir()?;
        fs::create_dir_all(&dir)
            .with_context(|| format!("unable to create {}", dir.display()))?;

        let path = dir.join(CONFIG_FILE);
        let body = serde_json::to_string_pretty(self).context("unable to serialize config")?;
        fs::write(&path, format!("{body}\n"))
            .with_context(|| format!("unable to write {}", path.display()))
    }

    /// Resolve the server origin, preferring a one-shot CLI/`NIMBUS_SERVER` override.
    pub fn resolve_server(&self, override_server: Option<&str>) -> Result<String> {
        if let Some(server) = override_server {
            return normalize_server(server);
        }

        match &self.server {
            Some(server) => normalize_server(server),
            None => bail!(
                "server URL not set; pass `--server` (or set NIMBUS_SERVER), or run `nimbus login --server <url>`"
            ),
        }
    }

    pub fn require_session(&self) -> Result<Uuid> {
        self.session_id
            .context("not logged in; run `nimbus login` first")
    }

    /// Prefer an explicit `--root`, otherwise the active root from config.
    pub fn resolve_root(&self, override_root: Option<Uuid>) -> Result<Uuid> {
        override_root
            .or(self.root_id)
            .context("no root selected; pass `--root` or run `nimbus root set <id>`")
    }
}

fn nimbus_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().context("unable to resolve home directory")?;
    Ok(home.join(DIR_NAME))
}

fn config_path() -> Result<PathBuf> {
    Ok(nimbus_dir()?.join(CONFIG_FILE))
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
