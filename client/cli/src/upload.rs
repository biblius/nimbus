use crate::{api::Api, config};
use anyhow::{Context, Result, bail};
use indicatif::{ProgressBar, ProgressStyle};
use std::path::{Path, PathBuf};
use uuid::Uuid;
use walkdir::WalkDir;

pub async fn run(
    server_arg: Option<&str>,
    root: Uuid,
    dir: Option<Uuid>,
    paths: &[PathBuf],
) -> Result<()> {
    if paths.is_empty() {
        bail!("at least one path is required");
    }

    let server = config::resolve_server(server_arg)?;
    let token = config::require_token()?;
    let api = Api::new(&server, Some(&token))?;

    let mut files: Vec<(PathBuf, String)> = Vec::new();
    let mut directories: Vec<PathBuf> = Vec::new();

    for path in paths {
        let meta = path
            .metadata()
            .with_context(|| format!("unable to stat {}", path.display()))?;

        if meta.is_file() {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .with_context(|| format!("invalid file name: {}", path.display()))?
                .to_owned();

            files.push((path.clone(), name));
        } else if meta.is_dir() {
            directories.push(path.clone());
        } else {
            bail!("{} is neither a file nor a directory", path.display());
        }
    }

    if !files.is_empty() {
        let label = format!(
            "Uploading {} file{}",
            files.len(),
            if files.len() == 1 { "" } else { "s" }
        );

        let progress = upload_progress(&files, &label).await?;

        let result = api.upload_files(root, dir, &files, &progress).await;

        finish_progress(&progress, result.is_ok());

        let uploaded = result?;

        for entry in uploaded {
            println!("  + {} ({})", entry.name, entry.id);
        }
    }

    for directory in directories {
        let relative_files = collect_dir_files(&directory)?;

        if relative_files.is_empty() {
            eprintln!("skipping empty directory {}", directory.display());
            continue;
        }

        let label = format!("Uploading {}", directory.display());

        let progress = upload_progress(&relative_files, &label).await?;

        let result = api.upload_dir(root, dir, &relative_files, &progress).await;

        finish_progress(&progress, result.is_ok());

        let uploaded = result?;

        for entry in uploaded {
            println!("  + {} ({})", entry.name, entry.id);
        }
    }

    Ok(())
}

async fn upload_progress(files: &[(PathBuf, String)], message: &str) -> Result<ProgressBar> {
    let mut total = 0u64;

    for (path, _) in files {
        let len = tokio::fs::metadata(path)
            .await
            .with_context(|| format!("unable to read metadata for {}", path.display()))?
            .len();

        total = total.saturating_add(len);
    }

    let bar = ProgressBar::new(total);

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

fn finish_progress(progress: &ProgressBar, ok: bool) {
    if ok {
        progress.finish_with_message(format!("{} done", progress.message()));
    } else {
        progress.abandon_with_message(format!("{} failed", progress.message()));
    }
}

/// Walk `dir` and build multipart filenames that preserve the top-level folder name
/// (e.g. uploading `Photos/` yields `Photos/2024/pic.jpg`).
fn collect_dir_files(dir: &Path) -> Result<Vec<(PathBuf, String)>> {
    let top = dir
        .file_name()
        .and_then(|name| name.to_str())
        .with_context(|| format!("invalid directory name: {}", dir.display()))?;

    let mut files = Vec::new();

    for entry in WalkDir::new(dir).follow_links(false) {
        let entry = entry.with_context(|| format!("error walking {}", dir.display()))?;
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();
        let relative = path
            .strip_prefix(dir)
            .with_context(|| format!("{} is not under {}", path.display(), dir.display()))?;

        let mut parts = vec![top.to_owned()];
        for component in relative.components() {
            let std::path::Component::Normal(part) = component else {
                bail!("invalid path component in {}", path.display());
            };
            let part = part
                .to_str()
                .with_context(|| format!("non-utf8 path: {}", path.display()))?;
            if part == "." || part == ".." {
                bail!("invalid path component in {}", path.display());
            }
            parts.push(part.to_owned());
        }

        files.push((path.to_path_buf(), parts.join("/")));
    }

    Ok(files)
}
