"use strict";

/**
 * deadrop — Streaming Decryption Web Worker
 *
 * Decrypts XChaCha20-Poly1305 encrypted chunks in a Web Worker thread
 * so the main UI thread stays responsive. Supports both streaming
 * (File System Access API) and in-memory fallback for all browsers.
 *
 * Protocol:
 *   [40-byte header][4-byte chunk_len][chunk_ciphertext]...[4-byte chunk_len][chunk_ciphertext]
 *
 * Header:
 *   bytes 0..24  = XChaCha20 nonce (192-bit)
 *   bytes 24..32 = total_chunks (u64 LE)
 *   bytes 32..40 = original_size (u64 LE)
 */

const HEADER_SIZE = 40;
const CHUNK_LEN_SIZE = 4;
const MAX_CHUNK_SIZE = 1024 * 1024; // 1MB sanity limit per chunk

let wasmModule = null;

// Initialize WASM module
async function initWasm() {
    if (wasmModule) return wasmModule;

    const wasm = await import("/wasm/deadrop_wasm.js");
    await wasm.default();
    wasmModule = wasm;
    return wasm;
}

// Parse 40-byte header from encrypted blob — returns regular Numbers (not BigInt)
function parseHeader(data) {
    if (data.length < HEADER_SIZE) {
        throw new Error("Encrypted data is too short to contain a valid header");
    }

    const view = new DataView(data.buffer, data.byteOffset, data.byteLength);
    const nonce = data.slice(0, 24);

    // Read u64 as two u32s to avoid BigInt conversion issues
    const totalChunksLow = view.getUint32(24, true);
    const totalChunksHigh = view.getUint32(28, true);
    const totalChunks = totalChunksLow + totalChunksHigh * 0x100000000;

    const originalSizeLow = view.getUint32(32, true);
    const originalSizeHigh = view.getUint32(36, true);
    const originalSize = originalSizeLow + originalSizeHigh * 0x100000000;

    if (totalChunks < 0 || totalChunks > Number.MAX_SAFE_INTEGER) {
        throw new Error("Invalid chunk count in header");
    }
    if (originalSize < 0 || originalSize > Number.MAX_SAFE_INTEGER) {
        throw new Error("Invalid original size in header");
    }

    return { nonce, totalChunks, originalSize };
}

// Read a u32 (LE) from Uint8Array at offset
function readU32LE(data, offset) {
    if (offset + 4 > data.length) {
        throw new Error("Buffer underflow reading u32 at offset " + offset);
    }
    return (
        data[offset] |
        (data[offset + 1] << 8) |
        (data[offset + 2] << 16) |
        (data[offset + 3] << 24)
    ) >>> 0;
}

// Format bytes for display
function formatBytes(bytes) {
    if (bytes === 0) return "0 B";
    const units = ["B", "KB", "MB", "GB", "TB"];
    const i = Math.floor(Math.log(bytes) / Math.log(1024));
    const idx = Math.min(i, units.length - 1);
    return (bytes / Math.pow(1024, idx)).toFixed(idx > 0 ? 1 : 0) + " " + units[idx];
}

// Sanitize string to prevent injection
function sanitizeString(str) {
    if (typeof str !== "string") return "";
    return str.replace(/[<>&"']/g, "");
}

/**
 * Main decryption pipeline
 *
 * Steps:
 * 1. Fetch encrypted blob from server
 * 2. Read 40-byte header -> extract nonce, chunk count, original size
 * 3. For each chunk: read 4-byte length -> read ciphertext -> WASM decrypt
 * 4. Write plaintext to output
 */
async function decryptStream(dropId, key, filename, mime) {
    const wasm = await initWasm();

    // --- Step 1: Fetch the encrypted blob ---
    postMessage({ type: "status", message: "Downloading encrypted blob..." });
    postMessage({ type: "progress", percent: 5 });

    // Sanitize dropId to prevent path traversal
    const safeDropId = encodeURIComponent(dropId);

    const response = await fetch("/api/blob/" + safeDropId);
    if (!response.ok) {
        throw new Error(
            response.status === 404
                ? "Drop not found \u2014 it may have self-destructed or expired."
                : "Server error: " + response.status
        );
    }

    const contentLength = parseInt(response.headers.get("Content-Length") || "0", 10);

    // --- Step 2: Read entire response into buffer ---
    const reader = response.body.getReader();
    const downloadChunks = [];
    let receivedBytes = 0;

    while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        downloadChunks.push(value);
        receivedBytes += value.length;

        if (contentLength > 0) {
            const downloadPercent = Math.min(40, Math.round((receivedBytes / contentLength) * 40));
            postMessage({ type: "progress", percent: 5 + downloadPercent });
            postMessage({
                type: "status",
                message: "Downloading... " + formatBytes(receivedBytes) + " / " + formatBytes(contentLength)
            });
        }
    }

    // Merge into single Uint8Array
    const encrypted = new Uint8Array(receivedBytes);
    let writeOffset = 0;
    for (const chunk of downloadChunks) {
        encrypted.set(chunk, writeOffset);
        writeOffset += chunk.length;
    }

    postMessage({ type: "progress", percent: 45 });
    postMessage({ type: "status", message: "Decrypting..." });

    // --- Step 3: Parse header ---
    const header = parseHeader(encrypted);
    const totalChunks = header.totalChunks;
    const originalSize = header.originalSize;
    const nonceBytes = header.nonce;

    if (totalChunks === 0) {
        throw new Error("File appears empty \u2014 no encrypted chunks found");
    }

    // --- Step 4: Decrypt chunks ---
    const plainParts = [];
    let offset = HEADER_SIZE;
    let decryptedBytes = 0;

    for (let chunkIndex = 0; chunkIndex < totalChunks; chunkIndex++) {
        // Read chunk length (4 bytes LE)
        if (offset + CHUNK_LEN_SIZE > encrypted.length) {
            throw new Error("Truncated data at chunk " + chunkIndex);
        }

        const chunkLen = readU32LE(encrypted, offset);
        offset += CHUNK_LEN_SIZE;

        // Sanity check chunk length
        if (chunkLen === 0 || chunkLen > MAX_CHUNK_SIZE) {
            throw new Error("Invalid chunk length at chunk " + chunkIndex + ": " + chunkLen);
        }

        // Read encrypted chunk
        if (offset + chunkLen > encrypted.length) {
            throw new Error("Truncated chunk " + chunkIndex + ": expected " + chunkLen + " bytes");
        }

        const encryptedChunk = encrypted.slice(offset, offset + chunkLen);
        offset += chunkLen;

        // Decrypt via WASM — pass chunkIndex as BigInt (WASM u64 requires BigInt in JS)
        let plaintext;
        try {
            plaintext = wasm.decrypt_chunk(encryptedChunk, key, nonceBytes, BigInt(chunkIndex));
        } catch (e) {
            throw new Error("Decryption failed at chunk " + chunkIndex + ": " + String(e));
        }

        plainParts.push(plaintext);
        decryptedBytes += plaintext.length;

        // Update progress (45% to 90% range for decryption)
        const decryptPercent = 45 + Math.round(((chunkIndex + 1) / totalChunks) * 45);
        postMessage({ type: "progress", percent: decryptPercent });

        // Yield to prevent blocking (every 50 chunks)
        if (chunkIndex % 50 === 0) {
            postMessage({
                type: "status",
                message: "Decrypting... " + formatBytes(decryptedBytes) + " / " + formatBytes(originalSize)
            });
            await new Promise(function (r) { setTimeout(r, 0); });
        }
    }

    postMessage({ type: "progress", percent: 92 });
    postMessage({ type: "status", message: "Preparing download..." });

    // --- Step 5: Merge and deliver ---
    const result = new Uint8Array(decryptedBytes);
    let mergeOffset = 0;
    for (const part of plainParts) {
        result.set(part, mergeOffset);
        mergeOffset += part.length;
    }

    postMessage({ type: "progress", percent: 95 });

    // Return the decrypted blob (transfer ownership for zero-copy)
    postMessage({
        type: "complete",
        data: result.buffer,
        filename: sanitizeString(filename),
        mime: sanitizeString(mime),
        size: decryptedBytes
    }, [result.buffer]);
}

// Worker message handler
self.onmessage = async function (e) {
    if (!e.data || typeof e.data !== "object") return;

    const { action, dropId, key, filename, mime } = e.data;

    if (action === "decrypt") {
        if (!dropId || !key) {
            postMessage({ type: "error", message: "Missing dropId or decryption key" });
            return;
        }

        try {
            await decryptStream(dropId, key, filename, mime);
        } catch (err) {
            postMessage({ type: "error", message: err.message || String(err) });
        }
    }
};
