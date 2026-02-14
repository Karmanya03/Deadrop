use axum::{
    Router,
    body::Body,
    extract::{ConnectInfo, Path, State},
    http::{StatusCode, header},
    response::{Html, IntoResponse, Response},
    routing::get,
};
use rust_embed::Embed;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Notify;
use tokio_util::io::ReaderStream;
use axum::middleware;
use axum::http::HeaderValue;
use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};
use crate::{config::DropConfig, crypto, progress, store::BlobStore};

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

/// Threshold: files larger than 50MB use disk-backed streaming
const DISK_THRESHOLD: u64 = 50 * 1024 * 1024;

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
            // Large folder archive → encrypt to disk
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
            // Small folder archive → encrypt in memory
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
            // Large file → encrypt streaming to disk (constant ~128KB memory)
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
            // Small file → encrypt in memory (fast, zero disk I/O)
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

    // Build router with security layers
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
    let url = format!(
        "http://{}:{}/d/{}#{}",
        local_ip, config.port, drop_id, key_fragment
    );
    let localhost_url = format!(
        "http://localhost:{}/d/{}#{}",
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
        " {} Waiting for downloads... (Ctrl+C to abort)",
        console::style("⏳").dim()
    );
    eprintln!();

    // Start server with graceful shutdown
    let listener =
        tokio::net::TcpListener::bind(format!("{}:{}", config.bind, config.port)).await?;
    let shutdown_signal = shutdown.clone();
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        tokio::select! {
            _ = shutdown_signal.notified() => {},
            _ = tokio::signal::ctrl_c() => {
                eprintln!("\n {} Shutting down...", console::style("🛑").bold());
            }
        }
    })
    .await?;

    Ok(())
}

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
            None => *pinned = Some(client_ip),       // First request — pin this IP
            Some(ip) if ip == &client_ip => {}       // Same IP — allowed
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
        // MEMORY MODE: serve from Vec<u8>
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
