mod api;
mod config;
mod list;
mod login;
mod search;
mod upload;

use clap::{Parser, Subcommand};
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
    /// Defaults to NIMBUS_SERVER or ~/.nimbus/server.
    #[arg(long, short, global = true, env = "NIMBUS_SERVER")]
    server: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Sign in permanently and store the session token in ~/.nimbus/token.
    Login,

    /// List accessible roots, or the contents of a root / directory.
    List {
        /// Root to list. Omit to list accessible roots.
        #[arg(long, short)]
        root: Option<Uuid>,

        /// Directory id within the root. Requires --root.
        #[arg(long, short)]
        dir: Option<Uuid>,
    },

    /// Upload files or directories into a root.
    Upload {
        /// Destination root id.
        #[arg(long, short)]
        root: Uuid,

        /// Optional parent directory id within the root.
        #[arg(long, short)]
        dir: Option<Uuid>,

        /// Local file or directory paths to upload.
        #[arg(required = true)]
        paths: Vec<PathBuf>,
    },

    /// Search file and directory names within a root.
    Search {
        /// Root to search in.
        #[arg(long, short)]
        root: Uuid,

        /// Search query (name full-text).
        query: String,
    },
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

    match cli.command {
        Commands::Login => login::run(server).await,
        Commands::List { root, dir } => list::run(server, root, dir).await,
        Commands::Upload { root, dir, paths } => upload::run(server, root, dir, &paths).await,
        Commands::Search { root, query } => search::run(server, root, &query).await,
    }
}
