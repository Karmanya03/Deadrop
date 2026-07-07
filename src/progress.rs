use console::style;
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};
use std::time::Duration;

/// Style presets for deadrop
pub struct Styles;

impl Styles {
    pub fn encrypt_bar() -> ProgressStyle {
        ProgressStyle::with_template(
            "  {spinner:.green} Encrypting  [{bar:40.cyan/dark_gray}] {bytes}/{total_bytes} ({eta})"
        )
        .unwrap()
        .progress_chars("━╸─")
    }

    pub fn download_bar() -> ProgressStyle {
        ProgressStyle::with_template(
            "  {spinner:.magenta} Download    [{bar:40.magenta/dark_gray}] {bytes}/{total_bytes} ({bytes_per_sec})"
        )
        .unwrap()
        .progress_chars("━╸─")
    }

    pub fn spinner() -> ProgressStyle {
        ProgressStyle::with_template("  {spinner:.cyan} {msg}").unwrap()
    }
}

/// Main progress manager for the CLI
pub struct ProgressManager {
    multi: MultiProgress,
}

impl ProgressManager {
    pub fn new() -> Self {
        Self {
            multi: MultiProgress::with_draw_target(ProgressDrawTarget::stderr()),
        }
    }

    /// Create encryption progress bar
    pub fn create_encrypt_bar(&self, total_bytes: u64) -> ProgressBar {
        let pb = self.multi.add(ProgressBar::new(total_bytes));
        pb.set_style(Styles::encrypt_bar());
        pb.enable_steady_tick(Duration::from_millis(80));
        pb
    }

    /// Create download tracking bar (updated when clients download)
    pub fn create_download_bar(&self, total_bytes: u64) -> ProgressBar {
        let pb = self.multi.add(ProgressBar::new(total_bytes));
        pb.set_style(Styles::download_bar());
        pb.enable_steady_tick(Duration::from_millis(80));
        pb
    }

    /// Create a spinner for status messages
    pub fn create_spinner(&self, msg: &str) -> ProgressBar {
        let pb = self.multi.add(ProgressBar::new_spinner());
        pb.set_style(Styles::spinner());
        pb.set_message(msg.to_string());
        pb.enable_steady_tick(Duration::from_millis(80));
        pb
    }
}

/// Print the startup banner with drop info
pub fn print_banner(
    url: &str,
    expire: &str,
    max_downloads: u32,
    file_size: u64,
    filename: &str,
    has_password: bool,
) {
    let size_str = bytesize::ByteSize::b(file_size).to_string();
    let downloads_str = if max_downloads == 0 {
        "unlimited".to_string()
    } else {
        format!("{}", max_downloads)
    };

    eprintln!();
    eprintln!(
        "{}",
        style(r#"     ██████╗ ███████╗ █████╗ ██████╗ ██████╗  ██████╗ ██████╗ "#)
            .bold()
            .green()
    );
    eprintln!(
        "{}",
        style(r#"     ██╔══██╗██╔════╝██╔══██╗██╔══██╗██╔══██╗██╔═══██╗██╔══██╗"#)
            .bold()
            .green()
    );
    eprintln!(
        "{}",
        style(r#"     ██║  ██║█████╗  ███████║██║  ██║██████╔╝██║   ██║██████╔╝"#)
            .bold()
            .green()
    );
    eprintln!(
        "{}",
        style(r#"     ██║  ██║██╔══╝  ██╔══██║██║  ██║██╔══██╗██║   ██║██╔═══╝ "#)
            .bold()
            .green()
    );
    eprintln!(
        "{}",
        style(r#"     ██████╔╝███████╗██║  ██║██████╔╝██║  ██║╚██████╔╝██║     "#)
            .bold()
            .green()
    );
    eprintln!(
        "{}",
        style(r#"     ╚═════╝ ╚══════╝╚═╝  ╚═╝╚═════╝ ╚═╝  ╚═╝ ╚═════╝ ╚═╝     "#)
            .bold()
            .green()
    );
    eprintln!(
        "{}",
        style(r#"          ⚡ zero-knowledge encrypted file sharing ⚡"#).dim()
    );
    eprintln!();

    eprintln!("  {}", style("─".repeat(50)).dim());
    eprintln!();
    eprintln!(
        "  {}  {}",
        style("URL").bold().cyan(),
        style(url).underlined().cyan()
    );
    eprintln!();
    eprintln!(
        "  {}  {}",
        style("├─ File").dim(),
        style(filename).white().bold()
    );
    eprintln!("  {}  {}", style("├─ Size").dim(), style(&size_str).white());
    eprintln!(
        "  {}  {}",
        style("├─ Expires").dim(),
        style(expire).yellow()
    );
    eprintln!(
        "  {}  {}",
        style("├─ Downloads").dim(),
        style(&downloads_str).white()
    );
    if has_password {
        eprintln!(
            "  {}  {}",
            style("├─ Password").dim(),
            style("✓ protected").green()
        );
    }
    eprintln!(
        "  {}  {}",
        style("└─ Crypto").dim(),
        style("XChaCha20-Poly1305 + Argon2id").dim()
    );
    eprintln!();
}

/// Print when a download happens
pub fn print_download_event(download_num: u32, max_downloads: u32, remote_addr: &str) {
    let count_str = if max_downloads == 0 {
        format!("#{}", download_num)
    } else {
        format!("#{}/{}", download_num, max_downloads)
    };

    eprintln!(
        "  {} Download {} from {}",
        style("⬇").magenta().bold(),
        style(&count_str).bold(),
        style(remote_addr).dim()
    );
}

/// Print when the drop self-destructs
pub fn print_self_destruct() {
    eprintln!();
    eprintln!(
        "  {} {}",
        style("💥").bold(),
        style("Drop self-destructed. All data wiped from memory.")
            .red()
            .bold()
    );
    eprintln!();
}

/// Print when a drop expires
pub fn print_expired() {
    eprintln!();
    eprintln!(
        "  {} {}",
        style("⏰").bold(),
        style("Drop expired. Data wiped.").yellow()
    );
    eprintln!();
}
