use anyhow::{Result, anyhow};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct DropConfig {
    pub file: PathBuf,
    pub port: u16,
    pub expire: String,
    pub expiry_duration: chrono::Duration,
    pub max_downloads: u32,
    pub password: Option<String>,
    pub bind: String,
    pub no_qr: bool,
}

impl DropConfig {
    pub fn new(
        file: PathBuf,
        port: u16,
        expire: String,
        max_downloads: u32,
        password: Option<String>,
        bind: String,
        no_qr: bool,
    ) -> Result<Self> {
        if !file.exists() {
            return Err(anyhow!("Path not found: {}", file.display()));
        }

        let expiry_duration = parse_duration(&expire)?;

        Ok(Self {
            file,
            port,
            expire,
            expiry_duration,
            max_downloads,
            password,
            bind,
            no_qr,
        })
    }
}

fn parse_duration(s: &str) -> Result<chrono::Duration> {
    let s = s.trim().to_lowercase();

    if let Some(num) = s.strip_suffix('s') {
        let n: i64 = num
            .parse()
            .map_err(|_| anyhow!("Invalid duration: {}", s))?;
        return Ok(chrono::Duration::seconds(n));
    }
    if let Some(num) = s.strip_suffix('m') {
        let n: i64 = num
            .parse()
            .map_err(|_| anyhow!("Invalid duration: {}", s))?;
        return Ok(chrono::Duration::minutes(n));
    }
    if let Some(num) = s.strip_suffix('h') {
        let n: i64 = num
            .parse()
            .map_err(|_| anyhow!("Invalid duration: {}", s))?;
        return Ok(chrono::Duration::hours(n));
    }
    if let Some(num) = s.strip_suffix('d') {
        let n: i64 = num
            .parse()
            .map_err(|_| anyhow!("Invalid duration: {}", s))?;
        return Ok(chrono::Duration::days(n));
    }

    // Default: try parsing as minutes
    let n: i64 = s
        .parse()
        .map_err(|_| anyhow!("Invalid duration '{}'. Use format: 30s, 10m, 1h, 7d", s))?;
    Ok(chrono::Duration::minutes(n))
}
pub struct ReceiveConfig {
    pub output_dir: std::path::PathBuf,
    pub port: u16,
    pub expiry_duration: chrono::Duration,
    pub bind: String,
    pub no_qr: bool,
}

impl ReceiveConfig {
    pub fn new(
        output: std::path::PathBuf,
        port: u16,
        expire: String,
        bind: String,
        no_qr: bool,
    ) -> anyhow::Result<Self> {
        let expiry_duration = parse_duration(&expire)?;
        Ok(Self { output_dir: output, port, expiry_duration, bind, no_qr })
    }
}
