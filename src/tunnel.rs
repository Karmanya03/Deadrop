use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};

/// Represents a running Cloudflare Tunnel
pub struct CloudflareTunnel {
    pub public_url: String,
    child: Option<tokio::process::Child>,
}

impl CloudflareTunnel {
    /// Build a full public URL with path and key fragment
    pub fn tunnel_url(&self, path: &str, key_fragment: &str) -> String {
        format!(
            "{}{}#{}",
            self.public_url.trim_end_matches('/'),
            path,
            key_fragment
        )
    }
}

impl Drop for CloudflareTunnel {
    fn drop(&mut self) {
        if let Some(ref mut child) = self.child {
            let _ = child.start_kill();
            eprintln!(
                " {} Cloudflare tunnel stopped",
                console::style("☁").dim()
            );
        }
    }
}

/// Try to start a tunnel. Returns Some(tunnel) on success, None on failure (with a warning).
pub async fn try_start_tunnel(local_port: u16) -> Option<CloudflareTunnel> {
    match start_tunnel(local_port).await {
        Ok(t) => Some(t),
        Err(e) => {
            eprintln!(
                " {} Tunnel unavailable: {}",
                console::style("☁").yellow(),
                console::style(e.to_string()).dim()
            );
            eprintln!(
                " {} Falling back to local network only",
                console::style("ℹ").blue(),
            );
            eprintln!();
            None
        }
    }
}

/// Resolve the cloudflared binary path using a 3-tier fallback:
///   1. Bundled: look next to the ded executable
///   2. System PATH: check if `cloudflared` is available globally
///   3. Auto-download: fetch from GitHub to ~/.deadrop/bin/
async fn resolve_cloudflared() -> anyhow::Result<PathBuf> {
    let bin_name = if cfg!(windows) { "cloudflared.exe" } else { "cloudflared" };

    // ── Tier 1: Bundled next to ded executable ──
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let bundled = exe_dir.join(bin_name);
            if bundled.exists() {
                eprintln!(
                    " {} Using bundled cloudflared",
                    console::style("☁").dim()
                );
                return Ok(bundled);
            }
        }
    }

    // ── Tier 2: System PATH ──
    if is_in_path(bin_name).await {
        eprintln!(
            " {} Using system cloudflared",
            console::style("☁").dim()
        );
        return Ok(PathBuf::from(bin_name));
    }

    // ── Tier 3: Auto-download to ~/.deadrop/bin/ ──
    let data_dir = get_deadrop_bin_dir()?;
    let cached = data_dir.join(bin_name);

    if cached.exists() {
        eprintln!(
            " {} Using cached cloudflared from {}",
            console::style("☁").dim(),
            console::style(cached.display()).dim()
        );
        return Ok(cached);
    }

    eprintln!(
        " {} cloudflared not found — downloading automatically...",
        console::style("☁").bold()
    );

    download_cloudflared(&cached).await?;

    Ok(cached)
}

/// Check if a binary is available in PATH
async fn is_in_path(bin_name: &str) -> bool {
    let check = if cfg!(windows) {
        tokio::process::Command::new("where.exe")
            .arg(bin_name)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
    } else {
        tokio::process::Command::new("which")
            .arg(bin_name)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
    };

    check.map(|s| s.success()).unwrap_or(false)
}

/// Get or create ~/.deadrop/bin/ directory
fn get_deadrop_bin_dir() -> anyhow::Result<PathBuf> {
    let home = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?;
    let bin_dir = home.join(".deadrop").join("bin");
    std::fs::create_dir_all(&bin_dir)?;
    Ok(bin_dir)
}

/// Download cloudflared binary from GitHub releases
async fn download_cloudflared(dest: &Path) -> anyhow::Result<()> {
    let url = get_cloudflared_download_url()?;

    eprintln!(
        " {} Downloading from: {}",
        console::style("⬇").bold(),
        console::style(&url).dim()
    );

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()?;

    let response = client.get(&url).send().await
        .map_err(|e| anyhow::anyhow!("Download failed: {}", e))?;

    if !response.status().is_success() {
        anyhow::bail!(
            "Download failed with status {} — check your internet connection",
            response.status()
        );
    }

    let bytes = response.bytes().await
        .map_err(|e| anyhow::anyhow!("Failed to read download: {}", e))?;

    // Write to a temp file first, then rename (atomic-ish)
    let tmp_path = dest.with_extension("tmp");
    std::fs::write(&tmp_path, &bytes)?;

    // Make executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp_path, std::fs::Permissions::from_mode(0o755))?;
    }

    std::fs::rename(&tmp_path, dest)?;

    let size_str = bytesize::ByteSize::b(bytes.len() as u64).to_string();
    eprintln!(
        " {} Downloaded cloudflared ({}) to {}",
        console::style("✅").bold(),
        console::style(&size_str).green(),
        console::style(dest.display()).dim()
    );

    Ok(())
}

/// Get the correct download URL for the current platform
fn get_cloudflared_download_url() -> anyhow::Result<String> {
    let base = "https://github.com/cloudflare/cloudflared/releases/latest/download";

    let filename = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64")  => "cloudflared-windows-amd64.exe",
        ("windows", "aarch64") => "cloudflared-windows-amd64.exe",
        ("linux", "x86_64")    => "cloudflared-linux-amd64",
        ("linux", "aarch64")   => "cloudflared-linux-arm64",
        ("linux", "arm")       => "cloudflared-linux-arm",
        ("macos", "x86_64")    => "cloudflared-darwin-amd64.tgz",
        ("macos", "aarch64")   => "cloudflared-darwin-amd64.tgz",
        (os, arch) => anyhow::bail!(
            "Unsupported platform: {}-{} — install cloudflared manually",
            os, arch
        ),
    };

    Ok(format!("{}/{}", base, filename))
}

/// Start a Cloudflare quick tunnel that forwards to the given local port.
pub async fn start_tunnel(local_port: u16) -> anyhow::Result<CloudflareTunnel> {
    eprintln!(
        " {} Starting Cloudflare tunnel...",
        console::style("☁").bold()
    );

    // Resolve binary with 3-tier fallback
    let cloudflared_path = resolve_cloudflared().await?;

    let mut child = tokio::process::Command::new(&cloudflared_path)
        .arg("tunnel")
        .arg("--url")
        .arg(format!("http://localhost:{}", local_port))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            anyhow::anyhow!(
                "Failed to start cloudflared at '{}': {}",
                cloudflared_path.display(),
                e
            )
        })?;

    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow::anyhow!("Failed to capture cloudflared stderr"))?;

    let mut reader = BufReader::new(stderr).lines();

    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(30);

    loop {
        tokio::select! {
            line = reader.next_line() => {
                match line {
                    Ok(Some(line)) => {
                        if let Some(url) = extract_tunnel_url(&line) {
                            eprintln!(
                                " {} Cloudflare tunnel ready: {}",
                                console::style("☁").green(),
                                console::style(&url).magenta()
                            );

                            return Ok(CloudflareTunnel {
                                public_url: url,
                                child: Some(child),
                            });
                        }
                    }
                    Ok(None) => {
                        anyhow::bail!("cloudflared exited before providing a URL");
                    }
                    Err(e) => {
                        anyhow::bail!("Error reading cloudflared output: {}", e);
                    }
                }
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                if start.elapsed() > timeout {
                    let _ = child.start_kill();
                    anyhow::bail!(
                        "Cloudflare tunnel timed out after {}s — check your internet connection",
                        timeout.as_secs()
                    );
                }
            }
        }
    }
}

/// Extract the trycloudflare.com URL from a cloudflared log line
fn extract_tunnel_url(line: &str) -> Option<String> {
    for word in line.split_whitespace() {
        let trimmed = word.trim_matches(|c: char| !c.is_alphanumeric() && c != ':' && c != '/' && c != '.' && c != '-');
        if trimmed.starts_with("https://") && trimmed.contains(".trycloudflare.com") {
            return Some(trimmed.to_string());
        }
    }
    None
}
