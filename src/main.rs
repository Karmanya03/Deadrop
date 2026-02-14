#![allow(dead_code)]

mod archive;
mod config;
mod crypto;
mod progress;
mod qr;
mod server;
mod store;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    name = "dd",
    bin_name = "dd",
    about = "🔒 Encrypted dead drop. One command. One link. Gone.",
    version,
    author,
    arg_required_else_help = true
)]
struct Cli {
    /// File or folder path to share
    #[arg(value_name = "PATH")]
    path: std::path::PathBuf,

    /// Port to listen on
    #[arg(short = 'p', long, default_value_t = 8080)]
    port: u16,

    /// Auto-expire after duration (e.g. 30s, 10m, 1h, 7d)
    #[arg(short = 'e', long, default_value = "1h")]
    expire: String,

    /// Max downloads before auto-delete (0 = unlimited)
    #[arg(short = 'n', long, default_value_t = 1)]
    downloads: u32,

    /// Require password for decryption (key derived via Argon2id)
    #[arg(long = "pw")]
    password: Option<String>,

    /// Bind address
    #[arg(short = 'b', long, default_value = "0.0.0.0")]
    bind: String,

    /// Don't display QR code
    #[arg(long)]
    no_qr: bool,
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

    let config = config::DropConfig::new(
        cli.path,
        cli.port,
        cli.expire,
        cli.downloads,
        cli.password,
        cli.bind,
        cli.no_qr,
    )?;

    server::start(config).await?;
    Ok(())
}
