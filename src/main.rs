#![allow(dead_code)]

mod archive;
mod config;
mod crypto;
mod progress;
mod qr;
mod server;
mod store;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "ded",
    bin_name = "ded",
    about = "🔒 Encrypted dead drop. One command. One link. Gone.",
    version,
    author,
    arg_required_else_help = true
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// File or folder path to share (shorthand for `ded send <PATH>`)
    #[arg(value_name = "PATH")]
    path: Option<std::path::PathBuf>,

    /// Port to listen on
    #[arg(short = 'p', long, default_value_t = 8080, global = true)]
    port: u16,

    /// Auto-expire after duration (e.g. 30s, 10m, 1h, 7d)
    #[arg(short = 'e', long, default_value = "1h", global = true)]
    expire: String,

    /// Max downloads before auto-delete (0 = unlimited)
    #[arg(short = 'n', long, default_value_t = 1, global = true)]
    downloads: u32,

    /// Require password for decryption (key derived via Argon2id)
    #[arg(long = "pw", global = true)]
    password: Option<String>,

    /// Bind address
    #[arg(short = 'b', long, default_value = "0.0.0.0", global = true)]
    bind: String,

    /// Don't display QR code
    #[arg(long, global = true)]
    no_qr: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Send a file or folder (same as `ded <PATH>`)
    Send {
        /// File or folder to share
        #[arg(value_name = "PATH")]
        path: std::path::PathBuf,
    },
    /// Receive a file from another device (phone → PC)
    Receive {
        /// Directory to save received files
        #[arg(short = 'o', long, default_value = ".")]
        output: std::path::PathBuf,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("deadrop=info".parse().unwrap()),
        )
        .with_target(false)
        .without_time()
        .init();

    let cli = Cli::parse();

    match cli.command {
        // `ded receive` — start upload server
        Some(Commands::Receive { output }) => {
            let receive_config = config::ReceiveConfig::new(
                output,
                cli.port,
                cli.expire,
                cli.bind,
                cli.no_qr,
            )?;
            server::start_receive(receive_config).await?;
        }
        // `ded send <PATH>` — explicit send
        Some(Commands::Send { path }) => {
            let config = config::DropConfig::new(
                path,
                cli.port,
                cli.expire,
                cli.downloads,
                cli.password,
                cli.bind,
                cli.no_qr,
            )?;
            server::start(config).await?;
        }
        // `ded <PATH>` — legacy shorthand for send
        None => {
            let path = cli.path.expect("PATH is required");
            let config = config::DropConfig::new(
                path,
                cli.port,
                cli.expire,
                cli.downloads,
                cli.password,
                cli.bind,
                cli.no_qr,
            )?;
            server::start(config).await?;
        }
    }

    Ok(())
}
