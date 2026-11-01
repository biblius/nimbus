use crate::{
    api::{Api, FileKind},
    config,
};
use anyhow::Result;
use uuid::Uuid;

pub async fn run(server_arg: Option<&str>, root: Uuid, query: &str) -> Result<()> {
    let server = config::resolve_server(server_arg)?;
    let token = config::require_token()?;
    let api = Api::new(&server, Some(&token))?;

    let files = api.search(root, query).await?;
    if files.is_empty() {
        println!("(no matches)");
        return Ok(());
    }

    println!(
        "{:<38}  {:<4}  {:>12}  {:<38}  {}",
        "ID", "KIND", "SIZE", "PARENT", "NAME"
    );
    for entry in files {
        let file = entry.file;
        let (kind, size) = match file.kind {
            FileKind::Directory => ("dir", "-".to_owned()),
            FileKind::File => ("file", format_bytes(file.size_bytes)),
        };
        let parent = file
            .parent
            .map(|id| id.to_string())
            .unwrap_or_else(|| "-".into());
        println!(
            "{:<38}  {:<4}  {:>12}  {:<38}  {}",
            file.id, kind, size, parent, file.name
        );
    }

    Ok(())
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
