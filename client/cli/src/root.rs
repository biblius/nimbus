use crate::{api::Api, config::Context};
use anyhow::{Result, bail};
use uuid::Uuid;

pub enum Action {
    Get,
    Set { id: Uuid },
    Clear,
}

pub async fn run(ctx: &mut Context, server_arg: Option<&str>, action: Action) -> Result<()> {
    match action {
        Action::Get => {
            let Some(id) = ctx.root_id else {
                println!("(no active root)");
                return Ok(());
            };

            let server = ctx.resolve_server(server_arg)?;
            let session = ctx.require_session()?;
            let api = Api::new(&server, Some(&session.to_string()))?;
            let roots = api.roots().await?;

            let Some(root) = roots.into_iter().find(|root| root.id == id) else {
                bail!("active root {id} was not found among accessible roots");
            };

            println!("{:<38}  {:<24}  {:>12}  ACCESS", "ID", "NAME", "QUOTA");
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
        Action::Set { id } => {
            ctx.root_id = Some(id);
            ctx.save()?;
            println!("active root set to {id}");
        }
        Action::Clear => {
            ctx.root_id = None;
            ctx.save()?;
            println!("active root cleared");
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
