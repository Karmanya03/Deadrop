use dashmap::DashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use tokio::time::{Duration, interval};

pub struct Drop {
    pub id: String,
    pub encrypted_path: Option<PathBuf>,   // Some = disk-backed, None = in-memory
    pub ciphertext: Option<Vec<u8>>,       // Some = in-memory, None = disk-backed
    pub encrypted_size: u64,               // Total size of encrypted data
    pub filename: String,
    pub mime_type: String,
    pub file_size: u64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub max_downloads: u32,
    pub download_count: AtomicU32,
    pub has_password: bool,
    pub pinned_ip: Mutex<Option<String>>,  // IP pinning: first downloader gets locked
}

impl std::ops::Drop for Drop {
    fn drop(&mut self) {
        // Securely delete the encrypted temp file when the drop is removed
        if let Some(ref path) = self.encrypted_path {
            if path.exists() {
                // Best-effort: overwrite with zeros before removing
                if let Ok(file) = std::fs::OpenOptions::new().write(true).open(path) {
                    let size = file.metadata().map(|m| m.len()).unwrap_or(0);
                    let zeros = vec![0u8; 64 * 1024];
                    let mut writer = std::io::BufWriter::new(file);
                    let mut remaining = size;
                    while remaining > 0 {
                        let to_write = remaining.min(zeros.len() as u64) as usize;
                        if std::io::Write::write_all(&mut writer, &zeros[..to_write]).is_err() {
                            break;
                        }
                        remaining -= to_write as u64;
                    }
                    let _ = std::io::Write::flush(&mut writer);
                }
                let _ = std::fs::remove_file(path);
            }
        }
        // In-memory ciphertext is dropped automatically (Vec deallocated)
    }
}

#[derive(Clone)]
pub struct BlobStore {
    drops: Arc<DashMap<String, Arc<Drop>>>,
    burned: Arc<DashMap<String, chrono::DateTime<chrono::Utc>>>,
    on_expire: Arc<dyn Fn() + Send + Sync>,
}

impl BlobStore {
    pub fn new(on_expire: impl Fn() + Send + Sync + 'static) -> Self {
        Self {
            drops: Arc::new(DashMap::new()),
            burned: Arc::new(DashMap::new()),
            on_expire: Arc::new(on_expire),
        }
    }

    pub fn insert(&self, drop: Drop) -> String {
        let id = drop.id.clone();
        self.drops.insert(id.clone(), Arc::new(drop));
        id
    }

    pub fn get(&self, id: &str) -> Option<Arc<Drop>> {
        self.drops.get(id).map(|d| d.value().clone())
    }

    pub fn remove(&self, id: &str) -> bool {
        let removed = self.drops.remove(id).is_some();
        if removed {
            // Track burned drops so late visitors see "already downloaded"
            self.burned.insert(id.to_string(), chrono::Utc::now());
        }
        removed
    }

    /// Check if a drop was already downloaded and destroyed
    pub fn is_burned(&self, id: &str) -> bool {
        self.burned.contains_key(id)
    }

    pub fn is_empty(&self) -> bool {
        self.drops.is_empty()
    }

    /// Increment download count. Returns (current_count, should_delete)
    pub fn record_download(&self, id: &str) -> Option<(u32, bool)> {
        let drop = self.get(id)?;
        let count = drop.download_count.fetch_add(1, Ordering::SeqCst) + 1;
        let should_delete = drop.max_downloads > 0 && count >= drop.max_downloads;
        Some((count, should_delete))
    }

    /// Background task: evict expired drops every 5 seconds
    pub fn spawn_reaper(&self) {
        let drops = self.drops.clone();
        let burned = self.burned.clone();
        let on_expire = self.on_expire.clone();
        tokio::spawn(async move {
            let mut tick = interval(Duration::from_secs(5));
            loop {
                tick.tick().await;
                let now = chrono::Utc::now();
                let before = drops.len();
                drops.retain(|_, drop| drop.expires_at > now);
                if drops.len() < before {
                    (on_expire)();
                }
                // Also clean burned entries older than 1 hour (no need to keep forever)
                burned.retain(|_, burned_at| {
                    now.signed_duration_since(*burned_at).num_hours() < 1
                });
            }
        });
    }
}
