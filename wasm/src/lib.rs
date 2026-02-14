use wasm_bindgen::prelude::*;

use chacha20poly1305::{
    aead::{Aead, KeyInit},
    XChaCha20Poly1305,
};
use base64::engine::{general_purpose::URL_SAFE_NO_PAD, Engine};

const HEADER_SIZE: usize = 40; // 24 (nonce) + 8 (chunk_count) + 8 (original_size)
const CHUNK_SIZE: usize = 64 * 1024; // 64KB — must match server-side crypto::CHUNK_SIZE

// ─── ENCRYPTION (NEW — for receive/upload mode) ────────────────────────────

/// Encrypt an entire blob in-memory. Returns [header][chunk_len][chunk]...
/// Key is base64url-encoded (same format as URL fragment).
/// Called from upload-worker.js when phone sends file to PC.
#[wasm_bindgen]
pub fn encrypt_blob(
    plaintext: &[u8],
    key_base64: &str,
) -> Result<Vec<u8>, JsValue> {
    let key_bytes = URL_SAFE_NO_PAD
        .decode(key_base64)
        .map_err(|e| JsValue::from_str(&format!("Invalid key: {}", e)))?;

    if key_bytes.len() != 32 {
        return Err(JsValue::from_str(&format!(
            "Invalid key length: expected 32, got {}", key_bytes.len()
        )));
    }

    let cipher = XChaCha20Poly1305::new_from_slice(&key_bytes)
        .map_err(|_| JsValue::from_str("Failed to init cipher"))?;

    // Generate random nonce (24 bytes for XChaCha20)
    let mut nonce_bytes = [0u8; 24];
    getrandom::fill(&mut nonce_bytes)
        .map_err(|e| JsValue::from_str(&format!("RNG failed: {}", e)))?;

    // Calculate chunks
    let total_chunks = if plaintext.is_empty() {
        1u64
    } else {
        ((plaintext.len() as u64) + (CHUNK_SIZE as u64) - 1) / (CHUNK_SIZE as u64)
    };
    let original_size = plaintext.len() as u64;

    // Build header (40 bytes)
    let mut output = Vec::with_capacity(plaintext.len() + HEADER_SIZE + (total_chunks as usize * 20));
    output.extend_from_slice(&nonce_bytes);                    // 24 bytes
    output.extend_from_slice(&total_chunks.to_le_bytes());     // 8 bytes
    output.extend_from_slice(&original_size.to_le_bytes());    // 8 bytes

    // Encrypt each chunk
    for chunk_index in 0..total_chunks {
        let start = (chunk_index as usize) * CHUNK_SIZE;
        let end = std::cmp::min(start + CHUNK_SIZE, plaintext.len());
        let chunk_data = &plaintext[start..end];

        // Derive per-chunk nonce: base_nonce XOR chunk_index
        let mut chunk_nonce = nonce_bytes;
        let idx_bytes = chunk_index.to_le_bytes();
        for i in 0..8 {
            chunk_nonce[i] ^= idx_bytes[i];
        }

        let encrypted = cipher
            .encrypt(chunk_nonce.as_slice().into(), chunk_data)
            .map_err(|_| JsValue::from_str(&format!(
                "Encryption failed at chunk {}", chunk_index
            )))?;

        // Write chunk length (4 bytes LE) then ciphertext
        output.extend_from_slice(&(encrypted.len() as u32).to_le_bytes());
        output.extend_from_slice(&encrypted);
    }

    Ok(output)
}

/// Encrypt a single chunk — for streaming upload (future use)
#[wasm_bindgen]
pub fn encrypt_chunk(
    chunk_data: &[u8],
    key_base64: &str,
    nonce_bytes: &[u8],
    chunk_index: u64,
) -> Result<Vec<u8>, JsValue> {
    let key_bytes = URL_SAFE_NO_PAD
        .decode(key_base64)
        .map_err(|e| JsValue::from_str(&format!("Invalid key: {}", e)))?;

    if key_bytes.len() != 32 {
        return Err(JsValue::from_str(&format!(
            "Invalid key length: expected 32, got {}", key_bytes.len()
        )));
    }

    let cipher = XChaCha20Poly1305::new_from_slice(&key_bytes)
        .map_err(|_| JsValue::from_str("Failed to init cipher"))?;

    let mut chunk_nonce = [0u8; 24];
    chunk_nonce.copy_from_slice(&nonce_bytes[..24]);
    let idx_bytes = chunk_index.to_le_bytes();
    for i in 0..8 {
        chunk_nonce[i] ^= idx_bytes[i];
    }

    let encrypted = cipher
        .encrypt(chunk_nonce.as_slice().into(), chunk_data)
        .map_err(|_| JsValue::from_str("Encryption failed"))?;

    Ok(encrypted)
}

// ─── DECRYPTION (existing) ─────────────────────────────────────────────────

/// Decrypt a single chunk given its encrypted data, key, base nonce, and chunk index
/// Used by the streaming Web Worker to decrypt chunk-by-chunk
#[wasm_bindgen]
pub fn decrypt_chunk(
    encrypted_chunk: &[u8],
    key_base64: &str,
    nonce_bytes: &[u8],
    chunk_index: u64,
) -> Result<Vec<u8>, JsValue> {
    let key_bytes = URL_SAFE_NO_PAD
        .decode(key_base64)
        .map_err(|e| JsValue::from_str(&format!("Invalid key: {}", e)))?;

    if key_bytes.len() != 32 {
        return Err(JsValue::from_str(&format!(
            "Invalid key length: expected 32, got {}", key_bytes.len()
        )));
    }

    let cipher = XChaCha20Poly1305::new_from_slice(&key_bytes)
        .map_err(|_| JsValue::from_str("Failed to init cipher"))?;

    // Derive per-chunk nonce
    let mut chunk_nonce = [0u8; 24];
    chunk_nonce.copy_from_slice(&nonce_bytes[..24]);
    let idx_bytes = chunk_index.to_le_bytes();
    for i in 0..8 {
        chunk_nonce[i] ^= idx_bytes[i];
    }

    let decrypted = cipher
        .decrypt(chunk_nonce.as_slice().into(), encrypted_chunk)
        .map_err(|_| JsValue::from_str("Decryption failed — wrong key or data corrupted"))?;

    Ok(decrypted)
}

/// Parse the 40-byte header from the encrypted blob
/// Returns [nonce(24), total_chunks(8), original_size(8)] as a flat Uint8Array
#[wasm_bindgen]
pub fn parse_header(data: &[u8]) -> Result<Vec<u8>, JsValue> {
    if data.len() < HEADER_SIZE {
        return Err(JsValue::from_str("Data too short to contain header"));
    }
    Ok(data[..HEADER_SIZE].to_vec())
}

/// Full in-memory decryption (for small files or when streaming isn't available)
#[wasm_bindgen]
pub fn decrypt_blob(
    encrypted_data: &[u8],
    key_base64: &str,
) -> Result<Vec<u8>, JsValue> {
    if encrypted_data.len() < HEADER_SIZE {
        return Err(JsValue::from_str("Data too short"));
    }

    let key_bytes = URL_SAFE_NO_PAD
        .decode(key_base64)
        .map_err(|e| JsValue::from_str(&format!("Invalid key: {}", e)))?;

    if key_bytes.len() != 32 {
        return Err(JsValue::from_str("Invalid key length"));
    }

    // Parse header
    let mut nonce_bytes = [0u8; 24];
    nonce_bytes.copy_from_slice(&encrypted_data[..24]);

    let total_chunks = u64::from_le_bytes(
        encrypted_data[24..32].try_into().map_err(|_| JsValue::from_str("Bad header"))?
    );
    let original_size = u64::from_le_bytes(
        encrypted_data[32..40].try_into().map_err(|_| JsValue::from_str("Bad header"))?
    );

    let cipher = XChaCha20Poly1305::new_from_slice(&key_bytes)
        .map_err(|_| JsValue::from_str("Failed to init cipher"))?;

    let chunk_data = &encrypted_data[HEADER_SIZE..];
    let mut plaintext = Vec::with_capacity(original_size as usize);
    let mut offset = 0;

    for chunk_index in 0..total_chunks {
        if offset + 4 > chunk_data.len() {
            return Err(JsValue::from_str("Truncated chunk length"));
        }

        let chunk_len = u32::from_le_bytes(
            chunk_data[offset..offset + 4].try_into().unwrap()
        ) as usize;
        offset += 4;

        if offset + chunk_len > chunk_data.len() {
            return Err(JsValue::from_str("Truncated chunk data"));
        }

        let encrypted_chunk = &chunk_data[offset..offset + chunk_len];
        offset += chunk_len;

        // Derive per-chunk nonce
        let mut chunk_nonce = nonce_bytes;
        let idx_bytes = chunk_index.to_le_bytes();
        for i in 0..8 {
            chunk_nonce[i] ^= idx_bytes[i];
        }

        let decrypted = cipher
            .decrypt(chunk_nonce.as_slice().into(), encrypted_chunk)
            .map_err(|_| JsValue::from_str(&format!(
                "Decryption failed at chunk {} — wrong key or corrupted", chunk_index
            )))?;

        plaintext.extend_from_slice(&decrypted);
    }

    Ok(plaintext)
}
