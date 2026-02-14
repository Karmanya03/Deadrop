use axum::{
    Router,
    body::Body,
    extract::{ConnectInfo, Path, State},
    http::{StatusCode, header, HeaderMap},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json,
};
use rust_embed::Embed;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Notify;
use tokio_util::io::ReaderStream;
use axum::middleware;
use axum::http::HeaderValue;
use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};
use axum_server::tls_rustls::RustlsConfig;
use rcgen::{generate_simple_self_signed, CertifiedKey};
use crate::{config::{DropConfig, ReceiveConfig}, crypto, progress, store::BlobStore};

// Embed web assets into the binary at compile time
#[derive(Embed)]
#[folder = "web/"]
struct WebAssets;

// Also embed the top-level assets/ folder (images, etc.)
#[derive(Embed)]
#[folder = "assets/"]
struct StaticAssets;

pub struct AppState {
    pub store: BlobStore,
    pub shutdown: Arc<Notify>,
}

/// State for receive mode
pub struct ReceiveState {
    pub key: crypto::EncryptionKey,
    pub output_dir: std::path::PathBuf,
    pub shutdown: Arc<Notify>,
    pub received: std::sync::atomic::AtomicBool,
}

/// Threshold: files larger than 50MB use disk-backed streaming
const DISK_THRESHOLD: u64 = 50 * 1024 * 1024;

// ─── Self-signed TLS cert helper ─────────────────────────────────────────────

async fn generate_tls_config(local_ip: &std::net::IpAddr) -> anyhow::Result<RustlsConfig> {
    let san = vec![
        local_ip.to_string(),
        "localhost".to_string(),
        "127.0.0.1".to_string(),
    ];
    let CertifiedKey { cert, key_pair } = generate_simple_self_signed(san)
        .expect("failed to generate self-signed cert");

    let tls_config = RustlsConfig::from_pem(
        cert.pem().into(),
        key_pair.serialize_pem().into(),
    )
    .await?;

    Ok(tls_config)
}

// ─── Security headers middleware ─────────────────────────────────────────────

async fn security_headers(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    // Prevent iframe embedding (clickjacking)
    headers.insert("X-Frame-Options", HeaderValue::from_static("DENY"));
    // Prevent MIME-type sniffing
    headers.insert("X-Content-Type-Options", HeaderValue::from_static("nosniff"));
    // Don't leak URL to other sites via Referer header
    headers.insert("Referrer-Policy", HeaderValue::from_static("no-referrer"));
    // Block unnecessary browser permissions
    headers.insert(
        "Permissions-Policy",
        HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
    );
    // CSP: only allow own origin + WASM execution
    headers.insert(
        "Content-Security-Policy",
        HeaderValue::from_static(
            "default-src 'self'; \
             script-src 'self' 'wasm-unsafe-eval'; \
             style-src 'self' 'unsafe-inline'; \
             img-src 'self' data:; \
             connect-src 'self'; \
             frame-ancestors 'none';"
        ),
    );
    // Prevent caching of any response
    headers.insert("Cache-Control", HeaderValue::from_static("no-store, no-cache, must-revalidate"));
    headers.insert("Pragma", HeaderValue::from_static("no-cache"));
    response
}

// ═══════════════════════════════════════════════════════════════════════════════
// SEND MODE (existing — `ded ./file` or `ded send ./file`)
// ═══════════════════════════════════════════════════════════════════════════════

pub async fn start(config: DropConfig) -> anyhow::Result<()> {
    let shutdown = Arc::new(Notify::new());
    let shutdown_clone = shutdown.clone();

    // Setup blob store with expiry callback
    let store = BlobStore::new(move || {
        progress::print_expired();
    });
    store.spawn_reaper();

    // --- Generate encryption key ---
    let (key, password_salt) = match &config.password {
        Some(pw) => {
            let mut salt = [0u8; 16];
            rand::fill(&mut salt);
            let k = crypto::EncryptionKey::from_password(pw, &salt)?;
            (k, Some(salt))
        }
        None => (crypto::EncryptionKey::generate(), None),
    };

    // --- Prepare file or folder ---
    let file_size: u64;
    let filename: String;
    let encrypted_path: Option<std::path::PathBuf>;
    let ciphertext: Option<Vec<u8>>;
    let encrypted_size: u64;

    if config.file.is_dir() {
        // FOLDER MODE: compress to .tar.gz first, then encrypt
        let pm = progress::ProgressManager::new();
        let archive_bar = pm.create_encrypt_bar(0);
        archive_bar.set_style(
            indicatif::ProgressStyle::with_template(
                " {spinner:.green} Archiving [{bar:40.yellow/dark_gray}] {pos}/{len} files"
            )
            .unwrap()
            .progress_chars("━╸─")
        );

        let (archive_bytes, archive_name): (Vec<u8>, String) = crate::archive::compress_folder(
            &config.file,
            &archive_bar,
        )?;

        file_size = archive_bytes.len() as u64;
        filename = archive_name;

        let pm2 = progress::ProgressManager::new();
        let encrypt_bar = pm2.create_encrypt_bar(file_size);

        if file_size > DISK_THRESHOLD {
            let mut cursor = std::io::Cursor::new(&archive_bytes);
            let info = crypto::encrypt_file_to_disk(
                &mut cursor,
                &key,
                file_size,
                |bytes| encrypt_bar.set_position(bytes),
            )?;
            encrypted_size = info.total_size;
            encrypted_path = Some(info.path);
            ciphertext = None;
        } else {
            let mut cursor = std::io::Cursor::new(&archive_bytes);
            let ct = crypto::encrypt_file_streaming(
                &mut cursor,
                &key,
                file_size,
                |bytes| encrypt_bar.set_position(bytes),
            )?;
            encrypted_size = ct.len() as u64;
            ciphertext = Some(ct);
            encrypted_path = None;
        }
        encrypt_bar.finish_and_clear();
    } else {
        // FILE MODE
        file_size = std::fs::metadata(&config.file)?.len();
        filename = config.file.file_name().unwrap().to_string_lossy().to_string();

        let pm = progress::ProgressManager::new();
        let encrypt_bar = pm.create_encrypt_bar(file_size);

        if file_size > DISK_THRESHOLD {
            let mut file = std::fs::File::open(&config.file)?;
            let info = crypto::encrypt_file_to_disk(
                &mut file,
                &key,
                file_size,
                |bytes| encrypt_bar.set_position(bytes),
            )?;
            encrypted_size = info.total_size;
            encrypted_path = Some(info.path);
            ciphertext = None;
        } else {
            let mut file = std::fs::File::open(&config.file)?;
            let ct = crypto::encrypt_file_streaming(
                &mut file,
                &key,
                file_size,
                |bytes| encrypt_bar.set_position(bytes),
            )?;
            encrypted_size = ct.len() as u64;
            ciphertext = Some(ct);
            encrypted_path = None;
        }
        encrypt_bar.finish_and_clear();
    };

    let mime = mime_guess::from_path(&config.file)
        .first_or_octet_stream()
        .to_string();

    // Generate 16-char drop ID (64-bit entropy — brute-force infeasible)
    let drop_id = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple().to_string()[..8].to_string(),
        uuid::Uuid::new_v4().simple().to_string()[..8].to_string(),
    );

    let drop = crate::store::Drop {
        id: drop_id.clone(),
        encrypted_path,
        ciphertext,
        encrypted_size,
        filename: filename.clone(),
        mime_type: mime,
        file_size,
        created_at: chrono::Utc::now(),
        expires_at: chrono::Utc::now() + config.expiry_duration,
        max_downloads: config.max_downloads,
        download_count: std::sync::atomic::AtomicU32::new(0),
        has_password: password_salt.is_some(),
        pinned_ip: std::sync::Mutex::new(None),
    };

    store.insert(drop);

    let state = Arc::new(AppState {
        store,
        shutdown: shutdown_clone,
    });

    // ─── Rate limiter: 2 req/sec per IP, burst of 5 ───
    let governor_conf = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(2)
            .burst_size(5)
            .finish()
            .unwrap(),
    );

    // Build router — rate-limit only API/page routes, not static assets
    let rate_limited = Router::new()
        .route("/d/{id}", get(serve_download_page))
        .route("/api/blob/{id}", get(serve_blob))
        .route("/api/meta/{id}", get(serve_meta))
        .layer(GovernorLayer::new(governor_conf));

    let app = rate_limited
        .route("/assets/{*path}", get(serve_web_asset))
        .route("/wasm/{*path}", get(serve_wasm_asset))
        .layer(middleware::from_fn(security_headers))
        .with_state(state.clone());

    // Determine URL
    let key_fragment = key.to_url_safe();
    let local_ip = local_ip_address::local_ip().unwrap_or("127.0.0.1".parse().unwrap());

    // --- Generate self-signed TLS cert ---
    let tls_config = generate_tls_config(&local_ip).await?;

    let url = format!(
        "https://{}:{}/d/{}#{}",
        local_ip, config.port, drop_id, key_fragment
    );
    let localhost_url = format!(
        "https://localhost:{}/d/{}#{}",
        config.port, drop_id, key_fragment
    );

    // Print banner
    progress::print_banner(
        &url,
        &config.expire,
        config.max_downloads,
        file_size,
        &filename,
        config.password.is_some(),
    );

    // Print QR code
    if !config.no_qr {
        crate::qr::print_qr(&url);
    }

    eprintln!(
        " {} Also available at: {}",
        console::style("ℹ").blue(),
        console::style(&localhost_url).dim()
    );
    eprintln!(
        " {} Self-signed TLS — browser will show a warning (safe to proceed)",
        console::style("🔒").yellow(),
    );
    eprintln!(
        " {} Waiting for downloads... (Ctrl+C to abort)",
        console::style("⏳").dim()
    );
    eprintln!();

    // Start HTTPS server with graceful shutdown
   let addr: SocketAddr = format!("{}:{}", config.bind, config.port).parse()?;
    let handle = axum_server::Handle::new();

    // Spawn shutdown listener
    let handle_clone = handle.clone();
    let shutdown_signal = shutdown.clone();
    tokio::spawn(async move {
        tokio::select! {
            _ = shutdown_signal.notified() => {},
            _ = tokio::signal::ctrl_c() => {
                eprintln!("\n {} Shutting down...", console::style("🛑").bold());
            }
        }
        handle_clone.graceful_shutdown(Some(std::time::Duration::from_secs(5)));
    });

    axum_server::bind_rustls(addr, tls_config)
        .handle(handle)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await?;

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// RECEIVE MODE (new — `ded receive`)
// ═══════════════════════════════════════════════════════════════════════════════

pub async fn start_receive(config: ReceiveConfig) -> anyhow::Result<()> {
    let shutdown = Arc::new(Notify::new());

    // Generate encryption key — receiver (PC) creates it, shares via QR/URL fragment
    let key = crypto::EncryptionKey::generate();

    // Ensure output directory exists
    std::fs::create_dir_all(&config.output_dir)?;

    let state = Arc::new(ReceiveState {
        key: crypto::EncryptionKey(key.0), // clone the key bytes for state
        output_dir: config.output_dir.clone(),
        shutdown: shutdown.clone(),
        received: std::sync::atomic::AtomicBool::new(false),
    });

    // ─── Rate limiter for upload endpoint ───
    let governor_conf = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(2)
            .burst_size(5)
            .finish()
            .unwrap(),
    );

    // Build receive-mode router
    let rate_limited = Router::new()
        .route("/api/upload", post(receive_upload))
        .layer(GovernorLayer::new(governor_conf));

    let app = rate_limited
        .route("/", get(serve_upload_page))
        .route("/assets/{*path}", get(serve_web_asset_receive))
        .route("/wasm/{*path}", get(serve_wasm_asset))
        .layer(middleware::from_fn(security_headers))
        .with_state(state.clone());

    // Determine URL with key fragment
    let key_fragment = key.to_url_safe();
    let local_ip = local_ip_address::local_ip().unwrap_or("127.0.0.1".parse().unwrap());

    // --- Generate self-signed TLS cert ---
    let tls_config = generate_tls_config(&local_ip).await?;

    let url = format!(
        "https://{}:{}/#{}", 
        local_ip, config.port, key_fragment
    );
    let localhost_url = format!(
        "https://localhost:{}/#{}", 
        config.port, key_fragment
    );

    // Print receive banner
    print_receive_banner(&url, &config.output_dir);

    // Print QR code
    if !config.no_qr {
        crate::qr::print_qr(&url);
    }

    eprintln!(
        " {} Also available at: {}",
        console::style("ℹ").blue(),
        console::style(&localhost_url).dim()
    );
    eprintln!(
        " {} Self-signed TLS — browser will show a warning (safe to proceed)",
        console::style("🔒").yellow(),
    );
    eprintln!(
        " {} Waiting for upload... (Ctrl+C to abort)",
        console::style("⏳").dim()
    );
    eprintln!();

    // Start HTTPS server
    let addr: SocketAddr = format!("{}:{}", config.bind, config.port).parse()?;
    let handle = axum_server::Handle::new();

    // Spawn shutdown listener
    let handle_clone = handle.clone();
    let shutdown_signal = shutdown.clone();
    tokio::spawn(async move {
        tokio::select! {
            _ = shutdown_signal.notified() => {},
            _ = tokio::signal::ctrl_c() => {
                eprintln!("\n {} Shutting down...", console::style("🛑").bold());
            }
        }
        handle_clone.graceful_shutdown(Some(std::time::Duration::from_secs(5)));
    });

    axum_server::bind_rustls(addr, tls_config)
        .handle(handle)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await?;

    Ok(())
}

fn print_receive_banner(url: &str, output_dir: &std::path::Path) {
    eprintln!();
    eprintln!("{}", console::style(" ██████╗ ███████╗ █████╗ ██████╗ ██████╗ ██████╗ ██████╗ ").cyan().bold());
    eprintln!("{}", console::style(" ██╔══██╗██╔════╝██╔══██╗██╔══██╗██╔══██╗██╔═══██╗██╔══██╗").cyan().bold());
    eprintln!("{}", console::style(" ██║  ██║█████╗  ███████║██║  ██║██████╔╝██║   ██║██████╔╝").cyan().bold());
    eprintln!("{}", console::style(" ██║  ██║██╔══╝  ██╔══██║██║  ██║██╔══██╗██║   ██║██╔═══╝ ").cyan().bold());
    eprintln!("{}", console::style(" ██████╔╝███████╗██║  ██║██████╔╝██║  ██║╚██████╔╝██║     ").cyan().bold());
    eprintln!("{}", console::style(" ╚═════╝ ╚══════╝╚═╝  ╚═╝╚═════╝ ╚═╝  ╚═╝ ╚═════╝ ╚═╝     ").cyan().bold());
    eprintln!();
    eprintln!(" {} {}", console::style("📥").bold(), console::style("RECEIVE MODE").green().bold());
    eprintln!();
    eprintln!(" ┌──────────────────────────────────────────────────┐");
    eprintln!(" │ {} {}",
        console::style("URL").bold(),
        console::style(url).green()
    );
    eprintln!(" │");
    eprintln!(" │ ├─ {} {}",
        console::style("Mode").dim(),
        "Receive (phone → PC)"
    );
    eprintln!(" │ ├─ {} {}",
        console::style("Save to").dim(),
        output_dir.display()
    );
    eprintln!(" │ └─ {} {}",
        console::style("Crypto").dim(),
        "XChaCha20-Poly1305"
    );
    eprintln!(" └──────────────────────────────────────────────────┘");
    eprintln!();
}

// ─── Receive mode handlers ───────────────────────────────────────────────────

/// Serve the upload page
async fn serve_upload_page() -> impl IntoResponse {
    match WebAssets::get("upload.html") {
        Some(content) => {
            Html(String::from_utf8_lossy(content.data.as_ref()).to_string()).into_response()
        }
        None => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// Receive encrypted upload from browser, decrypt with server key, save to disk
async fn receive_upload(
    State(state): State<Arc<ReceiveState>>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    // Only allow one upload
    if state.received.load(std::sync::atomic::Ordering::SeqCst) {
        return (
            StatusCode::GONE,
            "Already received a file — server is shutting down",
        ).into_response();
    }

    // Extract metadata from headers
    let filename = headers
        .get("X-Filename")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| urlencoding::decode(v).ok())
        .map(|v| v.to_string())
        .unwrap_or_else(|| "received_file".to_string());

    // Sanitize filename — prevent path traversal
    let safe_filename: String = filename
        .replace("..", "")
        .replace('/', "")
        .replace('\\', "");

    let safe_filename = if safe_filename.is_empty() {
        "received_file".to_string()
    } else {
        safe_filename
    };

    eprintln!(
        " {} Receiving encrypted upload: {} ({} bytes)",
        console::style("📥").bold(),
        safe_filename,
        body.len()
    );

    // Decrypt the blob using the server's key
    let _key_base64 = state.key.to_url_safe();

    // Use the same decrypt logic as the WASM/server crypto module
    match decrypt_uploaded_blob(&body, &state.key) {
        Ok(plaintext) => {
            let output_path = state.output_dir.join(&safe_filename);
            match std::fs::write(&output_path, &plaintext) {
                Ok(_) => {
                    state.received.store(true, std::sync::atomic::Ordering::SeqCst);
                    eprintln!(
                        " {} Saved: {} ({})",
                        console::style("✅").bold(),
                        console::style(&safe_filename).green(),
                        console::style(bytesize::ByteSize::b(plaintext.len() as u64).to_string()).dim()
                    );
                    eprintln!(
                        " {} File saved to: {}",
                        console::style("📁").bold(),
                        console::style(output_path.display()).green()
                    );

                    // Schedule shutdown
                    let shutdown = state.shutdown.clone();
                    tokio::spawn(async move {
                        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                        eprintln!(
                            "\n {} Transfer complete — self-destructing.",
                            console::style("💥").bold()
                        );
                        shutdown.notify_one();
                    });

                    Json(serde_json::json!({
                        "status": "ok",
                        "saved_as": safe_filename,
                        "size": plaintext.len()
                    })).into_response()
                }
                Err(e) => {
                    eprintln!(
                        " {} Failed to save file: {}",
                        console::style("❌").bold(),
                        e
                    );
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Failed to save: {}", e),
                    ).into_response()
                }
            }
        }
        Err(e) => {
            eprintln!(
                " {} Decryption failed: {}",
                console::style("❌").bold(),
                e
            );
            (
                StatusCode::BAD_REQUEST,
                format!("Decryption failed: {}", e),
            ).into_response()
        }
    }
}

/// Decrypt an uploaded blob (same format as crypto module: [header][chunks])
fn decrypt_uploaded_blob(data: &[u8], key: &crypto::EncryptionKey) -> anyhow::Result<Vec<u8>> {
    use chacha20poly1305::{aead::{Aead, KeyInit}, XChaCha20Poly1305};

    const HEADER_SIZE: usize = 40;
    if data.len() < HEADER_SIZE {
        anyhow::bail!("Data too short for header");
    }

    let mut nonce_bytes = [0u8; 24];
    nonce_bytes.copy_from_slice(&data[..24]);
    let total_chunks = u64::from_le_bytes(data[24..32].try_into()?);
    let original_size = u64::from_le_bytes(data[32..40].try_into()?);

    let cipher = XChaCha20Poly1305::new_from_slice(&key.0)
        .map_err(|_| anyhow::anyhow!("Failed to init cipher"))?;

    let chunk_data = &data[HEADER_SIZE..];
    let mut plaintext = Vec::with_capacity(original_size as usize);
    let mut offset = 0;

    for chunk_index in 0..total_chunks {
        if offset + 4 > chunk_data.len() {
            anyhow::bail!("Truncated chunk length at chunk {}", chunk_index);
        }

        let chunk_len = u32::from_le_bytes(
            chunk_data[offset..offset + 4].try_into()?
        ) as usize;
        offset += 4;

        if offset + chunk_len > chunk_data.len() {
            anyhow::bail!("Truncated chunk data at chunk {}", chunk_index);
        }

        let encrypted_chunk = &chunk_data[offset..offset + chunk_len];
        offset += chunk_len;

        // Derive per-chunk nonce: base XOR chunk_index
        let mut chunk_nonce = nonce_bytes;
        let idx_bytes = chunk_index.to_le_bytes();
        for i in 0..8 {
            chunk_nonce[i] ^= idx_bytes[i];
        }

        let decrypted = cipher
            .decrypt(chunk_nonce.as_slice().into(), encrypted_chunk)
            .map_err(|_| anyhow::anyhow!(
                "Decryption failed at chunk {} — wrong key or corrupted", chunk_index
            ))?;

        plaintext.extend_from_slice(&decrypted);
    }

    Ok(plaintext)
}

/// Serve web assets in receive mode (shares the same handler)
async fn serve_web_asset_receive(Path(path): Path<String>) -> Response {
    match WebAssets::get(&path) {
        Some(content) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, mime.as_ref())],
                content.data.to_vec(),
            ).into_response()
        }
        None => match StaticAssets::get(&path) {
            Some(content) => {
                let mime = mime_guess::from_path(&path).first_or_octet_stream();
                (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, mime.as_ref())],
                    content.data.to_vec(),
                ).into_response()
            }
            None => StatusCode::NOT_FOUND.into_response(),
        },
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// SEND MODE HANDLERS (existing)
// ═══════════════════════════════════════════════════════════════════════════════

/// Serve the download HTML page (embedded in binary)
async fn serve_download_page() -> impl IntoResponse {
    match WebAssets::get("index.html") {
        Some(content) => {
            Html(String::from_utf8_lossy(content.data.as_ref()).to_string()).into_response()
        }
        None => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// Serve encrypted blob — streams from disk for large files, in-memory for small
async fn serve_blob(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> Response {
    let Some(drop) = state.store.get(&id) else {
        // Constant-time 404: random delay prevents timing-based ID enumeration
        let delay = 50 + rand::random::<u64>() % 150;
        tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
        return (StatusCode::NOT_FOUND, "Drop not found or already destroyed").into_response();
    };

    // ── IP pinning: lock download to first IP that connects ──
    {
        let mut pinned = drop.pinned_ip.lock().unwrap();
        let client_ip = addr.ip().to_string();
        match pinned.as_ref() {
            None => *pinned = Some(client_ip), // First request — pin this IP
            Some(ip) if ip == &client_ip => {} // Same IP — allowed
            Some(_) => {
                eprintln!(
                    " {} Blocked download attempt from {} (pinned to different IP)",
                    console::style("🛡").red(),
                    addr
                );
                return (
                    StatusCode::FORBIDDEN,
                    "Access denied — this drop is locked to another device",
                )
                .into_response();
            }
        }
    }

    // Record download
    let Some((count, should_delete)) = state.store.record_download(&id) else {
        return StatusCode::NOT_FOUND.into_response();
    };

    progress::print_download_event(count, drop.max_downloads, &addr.to_string());

    let encrypted_size = drop.encrypted_size;

    // Build response body depending on storage mode
    let body = if let Some(ref path) = drop.encrypted_path {
        // DISK MODE: stream from temp file (constant memory)
        match tokio::fs::File::open(path).await {
            Ok(file) => {
                let stream = ReaderStream::new(file);
                Body::from_stream(stream)
            }
            Err(e) => {
                eprintln!(
                    " {} Failed to open encrypted file {}: {}",
                    console::style("⚠").yellow(),
                    path.display(),
                    e
                );
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        }
    } else if let Some(ref data) = drop.ciphertext {
        // MEMORY MODE: serve from Vec
        Body::from(data.clone())
    } else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };

    if should_delete {
        let store = state.store.clone();
        let shutdown = state.shutdown.clone();
        let id_clone = id.clone();
        tokio::spawn(async move {
            // Delay to let the stream finish sending
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            store.remove(&id_clone);
            progress::print_self_destruct();
            if store.is_empty() {
                shutdown.notify_one();
            }
        });
    }

    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/octet-stream"),
            (header::CACHE_CONTROL, "no-store, no-cache, must-revalidate"),
            (header::CONTENT_LENGTH, &encrypted_size.to_string()),
        ],
        body,
    )
    .into_response()
}

/// Serve metadata (filename, size, mime) — no sensitive data
async fn serve_meta(Path(id): Path<String>, State(state): State<Arc<AppState>>) -> Response {
    // Check if this drop was already consumed (burned)
    if state.store.is_burned(&id) {
        return (
            StatusCode::GONE,
            [(header::CONTENT_TYPE, "application/json")],
            r#"{"burned":true}"#,
        )
        .into_response();
    }

    let Some(drop) = state.store.get(&id) else {
        // Constant-time 404: random delay prevents timing-based ID enumeration
        let delay = 50 + rand::random::<u64>() % 150;
        tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
        return (StatusCode::NOT_FOUND, "{}").into_response();
    };

    let meta = serde_json::json!({
        "filename": drop.filename,
        "size": bytesize::ByteSize::b(drop.file_size).to_string(),
        "size_bytes": drop.file_size,
        "mime": drop.mime_type,
        "expires_at": drop.expires_at.to_rfc3339(),
        "downloads_remaining": if drop.max_downloads == 0 {
            "unlimited".to_string()
        } else {
            let remaining = drop.max_downloads.saturating_sub(
                drop.download_count.load(std::sync::atomic::Ordering::SeqCst)
            );
            remaining.to_string()
        },
        "has_password": drop.has_password,
    });

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        serde_json::to_string(&meta).unwrap(),
    )
    .into_response()
}

/// Serve embedded web assets (CSS, etc.)
async fn serve_web_asset(Path(path): Path<String>) -> Response {
    // Try the web/ embedded assets first, then fall back to assets/ folder
    match WebAssets::get(&path) {
        Some(content) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, mime.as_ref())],
                content.data.to_vec(),
            )
            .into_response()
        }
        None => match StaticAssets::get(&path) {
            Some(content) => {
                let mime = mime_guess::from_path(&path).first_or_octet_stream();
                (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, mime.as_ref())],
                    content.data.to_vec(),
                )
                .into_response()
            }
            None => StatusCode::NOT_FOUND.into_response(),
        },
    }
}

/// Serve WASM assets
async fn serve_wasm_asset(Path(path): Path<String>) -> Response {
    let full_path = format!("wasm/{}", path);
    serve_embedded::<WebAssets>(&full_path)
}

/// Helper to serve embedded files with correct MIME type
fn serve_embedded<T: Embed>(path: &str) -> Response {
    match T::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, mime.as_ref())],
                content.data.to_vec(),
            )
            .into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
