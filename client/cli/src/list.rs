use crate::{
    api::{Api, FileKind},
    config::Context,
};
use anyhow::Result;
use uuid::Uuid;

pub async fn run(
    ctx: &Context,
    server_arg: Option<&str>,
    root: Option<Uuid>,
    dir: Option<Uuid>,
) -> Result<()> {
    let server = ctx.resolve_server(server_arg)?;
    let session = ctx.require_session()?;
    let api = Api::new(&server, Some(&session.to_string()))?;

    // No --root and no active root → list roots. Otherwise list that root's entries.
    let root = root.or(ctx.root_id);

    if dir.is_some() && root.is_none() {
        anyhow::bail!("--dir requires a root; pass `--root` or run `nimbus root set <id>`");
    }

    match root {
        None => {
            let roots = api.roots().await?;
            if roots.is_empty() {
                println!("(no roots)");
                return Ok(());
            }

            println!("{:<38}  {:<24}  {:>12}  ACCESS", "ID", "NAME", "QUOTA");
            for root in roots {
                let quota = if root.quota == 0 {
                    "unlimited".to_owned()
                } else {
                    format_bytes(root.quota)
                };
                let kind = if root.owner.is_some() {
                    "private"
                } else {
                    "shared"
                };
                println!(
                    "{:<38}  {:<24}  {:>12}  {} ({})",
                    root.id,
                    truncate(&root.name, 24),
                    quota,
                    format_access(root.access_mode),
                    kind
                );
            }
        }
        Some(root_id) => {
            let files = api.list_files(root_id, dir).await?;
            if files.is_empty() {
                println!("(empty)");
                return Ok(());
            }

            println!("{:<38}  {:<4}  {:>12}  NAME", "ID", "KIND", "SIZE");
            for entry in files {
                let file = entry.file;
                let (kind, size) = match file.kind {
                    FileKind::Directory => ("dir", "-".to_owned()),
                    FileKind::File => ("file", format_bytes(file.size_bytes)),
                };
                println!("{:<38}  {:<4}  {:>12}  {}", file.id, kind, size, file.name);
            }
        }
    }

    Ok(())
}

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        value.to_owned()
    } else {
        let mut out: String = value.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

fn format_access(mode: i64) -> String {
    let mut parts = Vec::new();
    if mode & 1 != 0 {
        parts.push("write");
    }
    if mode & 2 != 0 {
        parts.push("share");
    }
    if parts.is_empty() {
        "read".into()
    } else {
        parts.join("+")
    }
}

fn format_bytes(bytes: i64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    if bytes < 0 {
        return "0 B".into();
    }

    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }

    if unit == 0 {
        format!("{bytes} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}
