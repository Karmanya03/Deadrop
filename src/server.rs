use base64::Engine;

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
use crate::{config::{DropConfig, ReceiveConfig}, crypto, progress, store::BlobStore};

#[derive(Embed)]
#[folder = "web/"]
struct WebAssets;

#[derive(Embed)]
#[folder = "assets/"]
struct StaticAssets;

pub struct AppState {
    pub store: BlobStore,
    pub shutdown: Arc<Notify>,
}

pub struct ReceiveState {
    pub key: crypto::EncryptionKey,
    pub output_dir: std::path::PathBuf,
    pub shutdown: Arc<Notify>,
    pub received: std::sync::atomic::AtomicBool,
}

const DISK_THRESHOLD: u64 = 50 * 1024 * 1024;

async fn security_headers(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert("X-Frame-Options", HeaderValue::from_static("DENY"));
    headers.insert("X-Content-Type-Options", HeaderValue::from_static("nosniff"));
    headers.insert("Referrer-Policy", HeaderValue::from_static("no-referrer"));
    headers.insert(
        "Permissions-Policy",
        HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
    );
    headers.insert(
        "Content-Security-Policy",
        HeaderValue::from_static(
            "default-src 'self'; \
            script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval'; \
            style-src 'self' 'unsafe-inline'; \
            img-src 'self' data:; \
            connect-src 'self'; \
            frame-ancestors 'none';"
        ),
    );
    headers.insert("Cache-Control", HeaderValue::from_static("no-store, no-cache, must-revalidate"));
    headers.insert("Pragma", HeaderValue::from_static("no-cache"));
    response
}

// ===============================================================================
// FAVICON
// ===============================================================================

async fn serve_favicon() -> Response {
    match StaticAssets::get("favicon.ico") {
        Some(content) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "image/x-icon")],
            content.data.to_vec(),
        ).into_response(),
        None => StatusCode::NO_CONTENT.into_response(),
    }
}

// ===============================================================================
// SEND MODE
// ===============================================================================

pub async fn start(
    config: DropConfig,
    tor_service: Option<&crate::tor::TorHiddenService>,
) -> anyhow::Result<()> {
    let shutdown = Arc::new(Notify::new());
    let shutdown_clone = shutdown.clone();
    let store = BlobStore::new(move || {
        progress::print_expired();
    });
    store.spawn_reaper();

    // Generate encryption key (or derive from password)
    let (key, password_salt) = match &config.password {
        Some(pw) => {
            let mut salt = [0u8; 16];
            rand::fill(&mut salt);
            let k = crypto::EncryptionKey::from_password(pw, &salt)?;
            (k, Some(salt))
        }
        None => (crypto::EncryptionKey::generate(), None),
    };

    // Prepare file or folder
    let file_size: u64;
    let filename: String;
    let encrypted_path: Option<std::path::PathBuf>;
    let ciphertext: Option<Vec<u8>>;
    let encrypted_size: u64;

    if config.file.is_dir() {
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
                &mut cursor, &key, file_size,
                |bytes| encrypt_bar.set_position(bytes),
            )?;
            encrypted_size = info.total_size;
            encrypted_path = Some(info.path);
            ciphertext = None;
        } else {
            let mut cursor = std::io::Cursor::new(&archive_bytes);
            let ct = crypto::encrypt_file_streaming(
                &mut cursor, &key, file_size,
                |bytes| encrypt_bar.set_position(bytes),
            )?;
            encrypted_size = ct.len() as u64;
            ciphertext = Some(ct);
            encrypted_path = None;
        }
        encrypt_bar.finish_and_clear();
    } else {
        file_size = std::fs::metadata(&config.file)?.len();
        filename = config.file.file_name().unwrap().to_string_lossy().to_string();

        let pm = progress::ProgressManager::new();
        let encrypt_bar = pm.create_encrypt_bar(file_size);

        if file_size > DISK_THRESHOLD {
            let mut file = std::fs::File::open(&config.file)?;
            let info = crypto::encrypt_file_to_disk(
                &mut file, &key, file_size,
                |bytes| encrypt_bar.set_position(bytes),
            )?;
            encrypted_size = info.total_size;
            encrypted_path = Some(info.path);
            ciphertext = None;
        } else {
            let mut file = std::fs::File::open(&config.file)?;
            let ct = crypto::encrypt_file_streaming(
                &mut file, &key, file_size,
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

    let governor_conf = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(2)
            .burst_size(5)
            .finish()
            .unwrap(),
    );

    let rate_limited = Router::new()
        .route("/d/{id}", get(serve_download_page))
        .route("/api/blob/{id}", get(serve_blob))
        .route("/api/meta/{id}", get(serve_meta))
        .layer(GovernorLayer::new(governor_conf));

    let app = rate_limited
        .route("/assets/{*path}", get(serve_web_asset))
        .route("/wasm/{*path}", get(serve_wasm_asset))
        .route("/favicon.ico", get(serve_favicon))
        .layer(middleware::from_fn(security_headers))
        .with_state(state.clone());

    // Password drops: put salt in fragment. Normal drops: put key in fragment.
    let key_fragment = match &password_salt {
        Some(salt) => {
            let salt_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(salt);
            format!("pw:{}", salt_b64)
        }
        None => key.to_url_safe(),
    };

    let local_ip = local_ip_address::local_ip().unwrap_or("127.0.0.1".parse().unwrap());
    let url = format!(
        "http://{}:{}/d/{}#{}",
        local_ip, config.port, drop_id, key_fragment
    );
    let localhost_url = format!(
        "http://localhost:{}/d/{}#{}",
        config.port, drop_id, key_fragment
    );

    progress::print_banner(
        &url,
        &config.expire,
        config.max_downloads,
        file_size,
        &filename,
        config.password.is_some(),
    );

    if let Some(tor) = tor_service {
        let onion_url = tor.onion_url(&format!("/d/{}", drop_id), &key_fragment);
        eprintln!(
            " {} Tor: {}",
            console::style("🧅").bold(),
            console::style(&onion_url).green()
        );
        eprintln!();
    }

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

// ===============================================================================
// RECEIVE MODE
// ===============================================================================

pub async fn start_receive(
    config: ReceiveConfig,
    tor_service: Option<&crate::tor::TorHiddenService>,
) -> anyhow::Result<()> {
    let shutdown = Arc::new(Notify::new());
    let key = crypto::EncryptionKey::generate();

    std::fs::create_dir_all(&config.output_dir)?;

    let state = Arc::new(ReceiveState {
        key: crypto::EncryptionKey(key.0),
        output_dir: config.output_dir.clone(),
        shutdown: shutdown.clone(),
        received: std::sync::atomic::AtomicBool::new(false),
    });

    let governor_conf = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(2)
            .burst_size(5)
            .finish()
            .unwrap(),
    );

    let rate_limited = Router::new()
        .route("/api/upload", post(receive_upload))
        .layer(GovernorLayer::new(governor_conf));

    let app = rate_limited
        .route("/", get(serve_upload_page))
        .route("/assets/{*path}", get(serve_web_asset_receive))
        .route("/wasm/{*path}", get(serve_wasm_asset))
        .route("/favicon.ico", get(serve_favicon))
        .layer(middleware::from_fn(security_headers))
        .with_state(state.clone());

    let key_fragment = key.to_url_safe();
    let local_ip = local_ip_address::local_ip().unwrap_or("127.0.0.1".parse().unwrap());
    let url = format!("http://{}:{}/#{}", local_ip, config.port, key_fragment);
    let localhost_url = format!("http://localhost:{}/#{}", config.port, key_fragment);

    print_receive_banner(&url, &config.output_dir);

    if let Some(tor) = tor_service {
        let onion_url = tor.onion_url("/", &key_fragment);
        eprintln!(
            " {} Tor: {}",
            console::style("🧅").bold(),
            console::style(&onion_url).green()
        );
        eprintln!();
    }

    if !config.no_qr {
        crate::qr::print_qr(&url);
    }

    eprintln!(
        " {} Also available at: {}",
        console::style("ℹ").blue(),
        console::style(&localhost_url).dim()
    );
    eprintln!(
        " {} Waiting for upload... (Ctrl+C to abort)",
        console::style("⏳").dim()
    );
    eprintln!();

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

async fn serve_upload_page() -> impl IntoResponse {
    match WebAssets::get("upload.html") {
        Some(content) => {
            Html(String::from_utf8_lossy(content.data.as_ref()).to_string()).into_response()
        }
        None => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn receive_upload(
    State(state): State<Arc<ReceiveState>>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    if state.received.load(std::sync::atomic::Ordering::SeqCst) {
        return (
            StatusCode::GONE,
            "Already received a file — server is shutting down",
        ).into_response();
    }

    let filename = headers
        .get("X-Filename")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| urlencoding::decode(v).ok())
        .map(|v| v.to_string())
        .unwrap_or_else(|| "received_file".to_string());

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
                    eprintln!(" {} Failed to save file: {}", console::style("❌").bold(), e);
                    (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to save: {}", e)).into_response()
                }
            }
        }
        Err(e) => {
            eprintln!(" {} Decryption failed: {}", console::style("❌").bold(), e);
            (StatusCode::BAD_REQUEST, format!("Decryption failed: {}", e)).into_response()
        }
    }
}

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

async fn serve_web_asset_receive(Path(path): Path<String>) -> Response {
    match WebAssets::get(&path) {
        Some(content) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            (StatusCode::OK, [(header::CONTENT_TYPE, mime.as_ref())], content.data.to_vec()).into_response()
        }
        None => match StaticAssets::get(&path) {
            Some(content) => {
                let mime = mime_guess::from_path(&path).first_or_octet_stream();
                (StatusCode::OK, [(header::CONTENT_TYPE, mime.as_ref())], content.data.to_vec()).into_response()
            }
            None => StatusCode::NOT_FOUND.into_response(),
        },
    }
}

// ===============================================================================
// SEND MODE HANDLERS
// ===============================================================================

async fn serve_download_page() -> impl IntoResponse {
    match WebAssets::get("index.html") {
        Some(content) => {
            Html(String::from_utf8_lossy(content.data.as_ref()).to_string()).into_response()
        }
        None => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn serve_blob(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> Response {
    let Some(drop) = state.store.get(&id) else {
        let delay = 50 + rand::random::<u64>() % 150;
        tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
        return (StatusCode::NOT_FOUND, "Drop not found or already destroyed").into_response();
    };

    // Scope the MutexGuard so it drops before any .await below
    {
        let mut pinned = drop.pinned_ip.lock().unwrap();
        let client_ip = addr.ip().to_string();
        match pinned.as_ref() {
            None => *pinned = Some(client_ip),
            Some(ip) if ip == &client_ip => {}
            Some(_) => {
                eprintln!(
                    " {} Blocked download attempt from {} (pinned to different IP)",
                    console::style("🛡").red(), addr
                );
                return (
                    StatusCode::FORBIDDEN,
                    "Access denied — this drop is locked to another device",
                ).into_response();
            }
        }
    }

    let Some((count, should_delete)) = state.store.record_download(&id) else {
        return StatusCode::NOT_FOUND.into_response();
    };

    progress::print_download_event(count, drop.max_downloads, &addr.to_string());

    let encrypted_size = drop.encrypted_size;
    let body = if let Some(ref path) = drop.encrypted_path {
        match tokio::fs::File::open(path).await {
            Ok(file) => Body::from_stream(ReaderStream::new(file)),
            Err(e) => {
                eprintln!(" {} Failed to open encrypted file {}: {}",
                    console::style("⚠").yellow(), path.display(), e);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        }
    } else if let Some(ref data) = drop.ciphertext {
        Body::from(data.clone())
    } else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };

    if should_delete {
        let store = state.store.clone();
        let shutdown = state.shutdown.clone();
        let id_clone = id.clone();
        tokio::spawn(async move {
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
    ).into_response()
}

async fn serve_meta(Path(id): Path<String>, State(state): State<Arc<AppState>>) -> Response {
    if state.store.is_burned(&id) {
        return (
            StatusCode::GONE,
            [(header::CONTENT_TYPE, "application/json")],
            r#"{"burned":true}"#,
        ).into_response();
    }

    let Some(drop) = state.store.get(&id) else {
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
    ).into_response()
}

async fn serve_web_asset(Path(path): Path<String>) -> Response {
    match WebAssets::get(&path) {
        Some(content) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            (StatusCode::OK, [(header::CONTENT_TYPE, mime.as_ref())], content.data.to_vec()).into_response()
        }
        None => match StaticAssets::get(&path) {
            Some(content) => {
                let mime = mime_guess::from_path(&path).first_or_octet_stream();
                (StatusCode::OK, [(header::CONTENT_TYPE, mime.as_ref())], content.data.to_vec()).into_response()
            }
            None => StatusCode::NOT_FOUND.into_response(),
        },
    }
}

async fn serve_wasm_asset(Path(path): Path<String>) -> Response {
    let full_path = format!("wasm/{}", path);
    serve_embedded::<WebAssets>(&full_path)
}

fn serve_embedded<T: rust_embed::Embed>(path: &str) -> Response {
    match T::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            (StatusCode::OK, [(header::CONTENT_TYPE, mime.as_ref())], content.data.to_vec()).into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
