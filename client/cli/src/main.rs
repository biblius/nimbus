mod api;
mod config;
mod download;
mod list;
mod login;
mod root;
mod search;
mod upload;

use clap::{Parser, Subcommand};
use config::Context;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Parser)]
#[command(
    name = "nimbus",
    about = "Administrate a Nimbus server from the command line",
    version
)]
struct Cli {
    /// Server origin (e.g. https://nimbus.example.com).
    /// Defaults to NIMBUS_SERVER or ~/.nimbus/config.json.
    #[arg(long, short, global = true, env = "NIMBUS_SERVER")]
    server: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Sign in permanently and store the session in ~/.nimbus/config.json.
    Login,

    /// Get, set, or clear the active root.
    Root {
        #[command(subcommand)]
        action: RootCommands,
    },

    /// List accessible roots, or the contents of a root / directory.
    List {
        /// Root to list. Defaults to the active root; omit both to list roots.
        #[arg(long, short)]
        root: Option<Uuid>,

        /// Directory id within the root.
        #[arg(long, short)]
        dir: Option<Uuid>,
    },

    /// Upload files or directories into a root.
    Upload {
        /// Destination root id. Defaults to the active root.
        #[arg(long, short)]
        root: Option<Uuid>,

        /// Optional parent directory id within the root.
        #[arg(long, short)]
        dir: Option<Uuid>,

        /// Local file or directory paths to upload.
        #[arg(required = true)]
        paths: Vec<PathBuf>,
    },

    /// Search file and directory names within a root.
    Search {
        /// Root to search in. Defaults to the active root.
        #[arg(long, short)]
        root: Option<Uuid>,

        /// Search query (name full-text).
        query: String,
    },

    /// Download a single file.
    Download {
        /// Root containing the file. Defaults to the active root.
        #[arg(long, short)]
        root: Option<Uuid>,

        /// File id to download.
        file: Uuid,

        /// Output path (file or directory). Defaults to `./<filename>`.
        #[arg(long, short = 'o')]
        output: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
enum RootCommands {
    /// Print the active root id.
    Get,
    /// Set the active root id.
    Set {
        /// Root id to make active.
        id: Uuid,
    },
    /// Clear the active root.
    Clear,
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}

async fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let server = cli.server.as_deref();
    let mut ctx = Context::load()?;

    match cli.command {
        Commands::Login => login::run(&mut ctx, server).await,
        Commands::Root { action } => {
            let action = match action {
                RootCommands::Get => root::Action::Get,
                RootCommands::Set { id } => root::Action::Set { id },
                RootCommands::Clear => root::Action::Clear,
            };
            root::run(&mut ctx, server, action).await
        }
        Commands::List { root, dir } => list::run(&ctx, server, root, dir).await,
        Commands::Upload { root, dir, paths } => {
            upload::run(&ctx, server, root, dir, &paths).await
        }
        Commands::Search { root, query } => search::run(&ctx, server, root, &query).await,
        Commands::Download { root, file, output } => {
            download::run(&ctx, server, root, file, output.as_deref()).await
        }
    }
}
