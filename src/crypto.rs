#![allow(dead_code, unused_imports)]

use chacha20poly1305::{
    XChaCha20Poly1305,
    aead::{Aead, KeyInit},
};
use rand::Rng;
use std::io::{Read, Write, Seek, SeekFrom, BufReader, BufWriter};
use std::path::{Path, PathBuf};
use zeroize::Zeroize;

pub const CHUNK_SIZE: usize = 64 * 1024; // 64KB chunks
const AUTH_TAG_SIZE: usize = 16;

#[derive(Clone)]
pub struct EncryptionKey(pub [u8; 32]);

impl Zeroize for EncryptionKey {
    fn zeroize(&mut self) {
        self.0.zeroize();
    }
}

impl Drop for EncryptionKey {
    fn drop(&mut self) {
        // Unlock memory before zeroizing (best-effort, ignore errors)
        #[cfg(unix)]
        unsafe {
            libc::munlock(
                self.0.as_ptr() as *const libc::c_void,
                self.0.len(),
            );
        }
        self.zeroize();
    }
}

impl EncryptionKey {
    /// Generate a cryptographically secure random 256-bit key
    pub fn generate() -> Self {
        let mut key = [0u8; 32];
        rand::rng().fill_bytes(&mut key);
        let k = Self(key);
        k.lock_memory();
        k
    }

    /// Derive key from password using Argon2id (memory-hard, GPU-resistant)
    ///
    /// Params: Argon2id v0x13, m=65536 (64 MB), t=3, p=1, output=32 bytes
    ///
    /// ⚠️  p=1 (not p=4) because the browser-side WASM runs single-threaded.
    ///     Both sides MUST use identical params or decryption will fail.
    pub fn from_password(password: &str, salt: &[u8; 16]) -> anyhow::Result<Self> {
        use argon2::{Algorithm, Argon2, Params, Version};

        let params = Params::new(65536, 3, 1, Some(32))
            .map_err(|e| anyhow::anyhow!("Argon2 params error: {}", e))?;
        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

        let mut key = [0u8; 32];
        argon2
            .hash_password_into(password.as_bytes(), salt, &mut key)
            .map_err(|e| anyhow::anyhow!("Argon2 hash error: {}", e))?;

        let k = Self(key);
        k.lock_memory();
        Ok(k)
    }

    /// Lock the key's memory page to prevent it from being swapped to disk.
    /// On Unix: uses mlock(). On Windows: this is a no-op (key is still
    /// zeroized on drop).
    fn lock_memory(&self) {
        #[cfg(unix)]
        unsafe {
            let ret = libc::mlock(
                self.0.as_ptr() as *const libc::c_void,
                self.0.len(),
            );
            if ret != 0 {
                // mlock can fail if user doesn't have CAP_IPC_LOCK or ulimit is too low.
                // Not fatal — key is still zeroized on drop either way.
                eprintln!(
                    " {} Could not lock key memory (mlock failed) — key may be swapped to disk",
                    console::style("⚠").yellow()
                );
            }
        }
    }

    /// Encode key as URL-safe base64 (no padding) for URL fragment
    pub fn to_url_safe(&self) -> String {
        use base64::engine::{Engine, general_purpose::URL_SAFE_NO_PAD};
        URL_SAFE_NO_PAD.encode(self.0)
    }

    /// Decode key from URL-safe base64
    pub fn from_url_safe(encoded: &str) -> anyhow::Result<Self> {
        use base64::engine::{Engine, general_purpose::URL_SAFE_NO_PAD};
        let bytes = URL_SAFE_NO_PAD.decode(encoded)?;
        if bytes.len() != 32 {
            return Err(anyhow::anyhow!(
                "Invalid key length: expected 32, got {}",
                bytes.len()
            ));
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(&bytes);
        let k = Self(key);
        k.lock_memory();
        Ok(k)
    }
}

/// Header written before encrypted data
#[derive(Debug)]
pub struct EncryptedHeader {
    pub nonce: [u8; 24],
    pub total_chunks: u64,
    pub original_size: u64,
}

impl EncryptedHeader {
    pub const SIZE: usize = 24 + 8 + 8; // nonce + chunk_count + original_size = 40 bytes

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(Self::SIZE);
        buf.extend_from_slice(&self.nonce);
        buf.extend_from_slice(&self.total_chunks.to_le_bytes());
        buf.extend_from_slice(&self.original_size.to_le_bytes());
        buf
    }

    pub fn from_bytes(data: &[u8]) -> anyhow::Result<Self> {
        if data.len() < Self::SIZE {
            return Err(anyhow::anyhow!("Header too short"));
        }
        let mut nonce = [0u8; 24];
        nonce.copy_from_slice(&data[..24]);
        let total_chunks = u64::from_le_bytes(data[24..32].try_into()?);
        let original_size = u64::from_le_bytes(data[32..40].try_into()?);
        Ok(Self {
            nonce,
            total_chunks,
            original_size,
        })
    }
}

/// Info about an encrypted file stored on disk
pub struct EncryptedFileInfo {
    pub path: PathBuf,
    pub total_size: u64,
    pub original_size: u64,
    pub total_chunks: u64,
}

/// Encrypt file streaming from disk → encrypted temp file on disk.
/// Memory usage: constant ~128KB regardless of file size.
pub fn encrypt_file_to_disk(
    input: &mut impl Read,
    key: &EncryptionKey,
    _original_size: u64,
    progress_callback: impl Fn(u64),
) -> anyhow::Result<EncryptedFileInfo> {
    let cipher = XChaCha20Poly1305::new_from_slice(&key.0)
        .map_err(|e| anyhow::anyhow!("Cipher init error: {}", e))?;

    let mut nonce_bytes = [0u8; 24];
    rand::rng().fill_bytes(&mut nonce_bytes);
    let nonce = *chachapoly1305_nonce_from_slice(&nonce_bytes);

    // Create temp file for encrypted output
    let temp_file = tempfile::NamedTempFile::new()?;
    let (file, temp_path) = temp_file.keep()
        .map_err(|e| anyhow::anyhow!("Failed to persist temp file: {}", e))?;
    let mut writer = BufWriter::with_capacity(CHUNK_SIZE * 2, file);

    // Write placeholder header (we'll update chunk count after)
    writer.write_all(&[0u8; EncryptedHeader::SIZE])?;

    let mut buf = vec![0u8; CHUNK_SIZE];
    let mut chunk_index: u64 = 0;
    let mut bytes_processed: u64 = 0;

    loop {
        let bytes_read = read_exact_or_eof(input, &mut buf)?;
        if bytes_read == 0 {
            break;
        }

        let chunk_nonce = derive_chunk_nonce(&nonce, chunk_index);
        let encrypted = cipher
            .encrypt(&chunk_nonce.into(), &buf[..bytes_read])
            .map_err(|e| anyhow::anyhow!("Encryption error at chunk {}: {}", chunk_index, e))?;

        // Write: [chunk_len (4 bytes LE)][encrypted_chunk_with_auth_tag]
        let len = (encrypted.len() as u32).to_le_bytes();
        writer.write_all(&len)?;
        writer.write_all(&encrypted)?;

        bytes_processed += bytes_read as u64;
        chunk_index += 1;
        progress_callback(bytes_processed);
    }

    writer.flush()?;

    // Seek back and write the real header with actual chunk count
    let mut file = writer.into_inner()?;
    file.seek(SeekFrom::Start(0))?;
    let header = EncryptedHeader {
        nonce: nonce.into(),
        total_chunks: chunk_index,
        original_size: bytes_processed,
    };
    file.write_all(&header.to_bytes())?;
    file.flush()?;

    let total_size = file.metadata()?.len();

    Ok(EncryptedFileInfo {
        path: temp_path,
        total_size,
        original_size: bytes_processed,
        total_chunks: chunk_index,
    })
}

/// Legacy in-memory encrypt (kept for backward compat / small files)
pub fn encrypt_file_streaming(
    reader: &mut impl Read,
    key: &EncryptionKey,
    file_size: u64,
    progress_callback: impl Fn(u64),
) -> anyhow::Result<Vec<u8>> {
    let cipher = XChaCha20Poly1305::new_from_slice(&key.0)
        .map_err(|e| anyhow::anyhow!("Cipher init error: {}", e))?;

    let mut nonce_bytes = [0u8; 24];
    rand::rng().fill_bytes(&mut nonce_bytes);
    let nonce = *chachapoly1305_nonce_from_slice(&nonce_bytes);

    let estimated_size = file_size as usize
        + (file_size as usize / CHUNK_SIZE + 1) * (AUTH_TAG_SIZE + 4)
        + EncryptedHeader::SIZE;
    let mut ciphertext = Vec::with_capacity(estimated_size);
    ciphertext.extend_from_slice(&[0u8; EncryptedHeader::SIZE]);

    let mut buf = vec![0u8; CHUNK_SIZE];
    let mut chunk_index: u64 = 0;
    let mut bytes_processed: u64 = 0;

    loop {
        let bytes_read = read_exact_or_eof(reader, &mut buf)?;
        if bytes_read == 0 {
            break;
        }

        let chunk_nonce = derive_chunk_nonce(&nonce, chunk_index);
        let encrypted = cipher
            .encrypt(&chunk_nonce.into(), &buf[..bytes_read])
            .map_err(|e| anyhow::anyhow!("Encryption error at chunk {}: {}", chunk_index, e))?;

        let len = (encrypted.len() as u32).to_le_bytes();
        ciphertext.extend_from_slice(&len);
        ciphertext.extend_from_slice(&encrypted);

        bytes_processed += bytes_read as u64;
        chunk_index += 1;
        progress_callback(bytes_processed);
    }

    let header = EncryptedHeader {
        nonce: nonce.into(),
        total_chunks: chunk_index,
        original_size: bytes_processed,
    };
    let header_bytes = header.to_bytes();
    ciphertext[..EncryptedHeader::SIZE].copy_from_slice(&header_bytes);

    Ok(ciphertext)
}

/// Read exactly buf.len() bytes or fewer if EOF
fn read_exact_or_eof(reader: &mut impl Read, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut total = 0;
    while total < buf.len() {
        match reader.read(&mut buf[total..])? {
            0 => break,
            n => total += n,
        }
    }
    Ok(total)
}

/// Derive per-chunk nonce by XORing base nonce with chunk index
fn derive_chunk_nonce(base_nonce: &chacha20poly1305::XNonce, chunk_index: u64) -> [u8; 24] {
    let mut nonce = [0u8; 24];
    nonce.copy_from_slice(base_nonce.as_slice());
    let idx_bytes = chunk_index.to_le_bytes();
    for i in 0..8 {
        nonce[i] ^= idx_bytes[i];
    }
    nonce
}

fn chachapoly1305_nonce_from_slice(slice: &[u8]) -> &chacha20poly1305::XNonce {
    chacha20poly1305::XNonce::from_slice(slice)
}
