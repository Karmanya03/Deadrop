use std::path::PathBuf;

/// Represents a running Tor hidden service
pub struct TorHiddenService {
    pub onion_hostname: String,
    pub data_dir: PathBuf,
    child: Option<tokio::process::Child>,
}

impl TorHiddenService {
    /// Build a full .onion URL with path and key fragment
    pub fn onion_url(&self, path: &str, key_fragment: &str) -> String {
        format!(
            "http://{}{}#{}",
            self.onion_hostname.trim(),
            path,
            key_fragment
        )
    }
}

impl Drop for TorHiddenService {
    fn drop(&mut self) {
        if let Some(ref mut child) = self.child {
            let _ = child.start_kill();
            eprintln!(
                " {} Tor hidden service stopped",
                console::style("🧅").dim()
            );
        }
    }
}

/// Start a Tor hidden service that forwards to the given local port.
/// Requires `tor` binary in PATH.
pub async fn start_hidden_service(local_port: u16) -> anyhow::Result<TorHiddenService> {
    eprintln!(
        " {} Starting Tor hidden service...",
        console::style("🧅").bold()
    );

    // Create a temp directory for this Tor instance
    let tor_dir = std::env::temp_dir()
        .join("deadrop-tor")
        .join(format!("{}", std::process::id()));
    std::fs::create_dir_all(&tor_dir)?;

    let hidden_service_dir = tor_dir.join("hidden_service");
    std::fs::create_dir_all(&hidden_service_dir)?;

    // Write torrc
    let torrc_path = tor_dir.join("torrc");
    let torrc_content = format!(
        "SocksPort 0\n\
         HiddenServiceDir {}\n\
         HiddenServicePort 80 127.0.0.1:{}\n\
         DataDirectory {}\n",
        hidden_service_dir.display(),
        local_port,
        tor_dir.join("data").display(),
    );
    std::fs::write(&torrc_path, &torrc_content)?;

    // Start tor process
    let child = tokio::process::Command::new("tor")
        .arg("-f")
        .arg(&torrc_path)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| {
            anyhow::anyhow!(
                "Failed to start tor — is it installed? ({})\n\
                 Install: sudo apt install tor / brew install tor / choco install tor",
                e
            )
        })?;

    // Wait for the hostname file to appear (Tor generates it)
    let hostname_path = hidden_service_dir.join("hostname");
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(60);

    loop {
        if hostname_path.exists() {
            let hostname = std::fs::read_to_string(&hostname_path)?;
            let hostname = hostname.trim().to_string();

            if !hostname.is_empty() {
                eprintln!(
                    " {} Tor hidden service ready: {}",
                    console::style("🧅").green(),
                    console::style(&hostname).magenta()
                );

                return Ok(TorHiddenService {
                    onion_hostname: hostname,
                    data_dir: tor_dir,
                    child: Some(child),
                });
            }
        }

        if start.elapsed() > timeout {
            anyhow::bail!(
                "Tor hidden service timed out after {}s — check tor logs in {}",
                timeout.as_secs(),
                tor_dir.display()
            );
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }
}
