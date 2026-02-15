#![allow(dead_code, unused_imports)]

use clap::{Parser, Subcommand, Args};
use std::path::PathBuf;
use deadrop::{archive, config, server, tor};

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
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Send file(s) — also works as `ded <files...>` without "send"
    #[command(alias = "s")]
    Send(SendArgs),

    /// Receive a file from another device (phone → PC)
    #[command(alias = "r")]
    Receive(ReceiveArgs),
}

#[derive(Args, Debug)]
struct SendArgs {
    /// File(s), folder(s) to share, or "-" for stdin/clipboard
    #[arg(value_name = "PATH", num_args = 1..)]
    paths: Vec<PathBuf>,

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

    /// Enable Tor hidden service (.onion address)
    #[arg(long)]
    tor: bool,
}

#[derive(Args, Debug)]
struct ReceiveArgs {
    /// Port to listen on
    #[arg(short = 'p', long, default_value_t = 8080)]
    port: u16,

    /// Output directory for received files
    #[arg(short = 'o', long, default_value = ".")]
    output: PathBuf,

    /// Bind address
    #[arg(short = 'b', long, default_value = "0.0.0.0")]
    bind: String,

    /// Don't display QR code
    #[arg(long)]
    no_qr: bool,

    /// Enable Tor hidden service (.onion address)
    #[arg(long)]
    tor: bool,
}

/// Preprocess CLI args so `ded ./file` works without typing "send"
fn preprocess_args() -> Vec<String> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        return args;
    }

    let first = &args[1];
    let known = ["send", "s", "receive", "r", "help", "--help", "-h", "--version", "-V"];
    if !known.contains(&first.as_str()) {
        let mut new_args = vec![args[0].clone(), "send".to_string()];
        new_args.extend_from_slice(&args[1..]);
        return new_args;
    }
    args
}

/// Parse a human-readable duration string (e.g. "30s", "10m", "1h", "7d")
fn parse_duration(s: &str) -> anyhow::Result<chrono::Duration> {
    let s = s.trim();
    if s.is_empty() {
        anyhow::bail!("Empty duration string");
    }
    let (num_str, unit) = s.split_at(s.len() - 1);
    let num: i64 = num_str
        .parse()
        .map_err(|_| anyhow::anyhow!("Invalid duration number: {}", num_str))?;
    match unit {
        "s" => Ok(chrono::Duration::seconds(num)),
        "m" => Ok(chrono::Duration::minutes(num)),
        "h" => Ok(chrono::Duration::hours(num)),
        "d" => Ok(chrono::Duration::days(num)),
        _ => anyhow::bail!("Unknown duration unit '{}' — use s/m/h/d", unit),
    }
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

    let cli = Cli::parse_from(preprocess_args());

    match cli.command {
        Commands::Send(args) => {
            // ── Handle stdin/clipboard mode ──
            let resolved_paths = if args.paths.len() == 1 && args.paths[0] == PathBuf::from("-") {
                use std::io::Read;
                let mut buffer = Vec::new();
                std::io::stdin().read_to_end(&mut buffer)?;

                if buffer.is_empty() {
                    anyhow::bail!("stdin is empty — nothing to send");
                }

                let tmp_dir = std::env::temp_dir().join("deadrop-stdin");
                std::fs::create_dir_all(&tmp_dir)?;
                let tmp_path = tmp_dir.join("clipboard.txt");
                std::fs::write(&tmp_path, &buffer)?;

                eprintln!(
                    " {} Read {} from stdin",
                    console::style("📋").bold(),
                    console::style(bytesize::ByteSize::b(buffer.len() as u64).to_string()).green()
                );

                vec![tmp_path]
            } else {
                for p in &args.paths {
                    if !p.exists() {
                        anyhow::bail!("Path not found: {}", p.display());
                    }
                }
                args.paths.clone()
            };

            // ── Handle multi-file: bundle into tar.gz ──
            let final_path = if resolved_paths.len() > 1 {
                let tmp_dir = std::env::temp_dir().join("deadrop-bundle");
                std::fs::create_dir_all(&tmp_dir)?;
                let bundle_path = tmp_dir.join("deadrop-bundle.tar.gz");

                eprintln!(
                    " {} Bundling {} files into archive...",
                    console::style("📦").bold(),
                    resolved_paths.len()
                );

                archive::bundle_files(&resolved_paths, &bundle_path)?;
                bundle_path
            } else {
                resolved_paths[0].clone()
            };

            let drop_config = config::DropConfig::new(
                final_path,
                args.port,
                args.expire,
                args.downloads,
                args.password,
                args.bind,
                args.no_qr,
            )?;

            // ── Optional Tor hidden service ──
            let tor_service = if args.tor {
                Some(tor::start_hidden_service(drop_config.port).await?)
            } else {
                None
            };

            server::start(drop_config, tor_service.as_ref()).await?;
        }

        Commands::Receive(args) => {
            let expiry_dur = parse_duration("1h")?;

            let recv_config = config::ReceiveConfig {
                port: args.port,
                output_dir: args.output,
                bind: args.bind,
                no_qr: args.no_qr,
                expiry_duration: expiry_dur,
            };

            // ── Optional Tor hidden service ──
            let tor_service = if args.tor {
                Some(tor::start_hidden_service(recv_config.port).await?)
            } else {
                None
            };

            server::start_receive(recv_config, tor_service.as_ref()).await?;
        }
    }

    Ok(())
}