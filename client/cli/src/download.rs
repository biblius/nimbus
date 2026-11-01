use crate::{api::Api, config};
use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use std::path::Path;
use uuid::Uuid;

pub async fn run(
    server_arg: Option<&str>,
    root: Uuid,
    file: Uuid,
    output: Option<&Path>,
) -> Result<()> {
    let server = config::resolve_server(server_arg)?;
    let token = config::require_token()?;
    let api = Api::new(&server, Some(&token))?;

    let dest = output.unwrap_or_else(|| Path::new("."));
    let progress = download_progress(&format!("Downloading {file}"))?;

    let result = api.download_file(root, file, dest, &progress).await;
    if result.is_ok() {
        progress.finish_with_message(format!("{} done", progress.message()));
    } else {
        progress.abandon_with_message(format!("{} failed", progress.message()));
    }

    let path = result?;
    println!("saved {}", path.display());
    Ok(())
}

fn download_progress(message: &str) -> Result<ProgressBar> {
    let bar = ProgressBar::new(0);
    bar.set_style(
        ProgressStyle::with_template(
            "{msg}  [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, ETA {eta})",
        )
        .context("invalid progress bar template")?
        .progress_chars("=>-"),
    );
    bar.set_message(message.to_owned());
    Ok(bar)
}
