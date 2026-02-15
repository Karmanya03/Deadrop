"use strict";

/**
 * deadrop — Streaming Decryption Web Worker (v2)
 *
 * Downloads and decrypts simultaneously — never holds the full
 * encrypted blob in memory. Peak RAM ≈ plaintext size + one chunk buffer.
 *
 * Protocol:
 *   [40-byte header][4-byte chunk_len][chunk_ciphertext]...[repeat]
 *
 * Header:
 *   bytes  0..24  = XChaCha20 nonce (192-bit)
 *   bytes 24..32  = total_chunks (u64 LE)
 *   bytes 32..40  = original_size (u64 LE)
 */

const HEADER_SIZE = 40;
const CHUNK_LEN_SIZE = 4;
const MAX_CHUNK_SIZE = 1024 * 1024; // 1MB sanity limit

let wasmModule = null;

async function initWasm() {
    if (wasmModule) return wasmModule;
    const wasm = await import("/wasm/deadrop_wasm.js");
    await wasm.default();
    wasmModule = wasm;
    return wasm;
}

function parseHeader(data) {
    if (data.length < HEADER_SIZE) {
        throw new Error("Encrypted data is too short to contain a valid header");
    }
    const view = new DataView(data.buffer, data.byteOffset, data.byteLength);
    const nonce = data.slice(0, 24);
    const totalChunksLow = view.getUint32(24, true);
    const totalChunksHigh = view.getUint32(28, true);
    const totalChunks = totalChunksLow + totalChunksHigh * 0x100000000;
    const originalSizeLow = view.getUint32(32, true);
    const originalSizeHigh = view.getUint32(36, true);
    const originalSize = originalSizeLow + originalSizeHigh * 0x100000000;
    return { nonce, totalChunks, originalSize };
}

function readU32LE(data, offset) {
    return (
        data[offset] |
        (data[offset + 1] << 8) |
        (data[offset + 2] << 16) |
        (data[offset + 3] << 24)
    ) >>> 0;
}

function formatBytes(bytes) {
    if (bytes === 0) return "0 B";
    const units = ["B", "KB", "MB", "GB", "TB"];
    const i = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
    return (bytes / Math.pow(1024, i)).toFixed(i > 0 ? 1 : 0) + " " + units[i];
}

function sanitizeString(str) {
    if (typeof str !== "string") return "";
    return str.replace(/[<>&"']/g, "");
}

/**
 * Append new data to existing buffer efficiently.
 * Returns a new Uint8Array with old + new data.
 */
function appendBuffer(existing, newData) {
    if (existing.length === 0) return new Uint8Array(newData);
    const merged = new Uint8Array(existing.length + newData.length);
    merged.set(existing);
    merged.set(newData, existing.length);
    return merged;
}

/**
 * Streaming decryption pipeline:
 *   1. Fetch encrypted blob as a ReadableStream
 *   2. Parse 40-byte header from first arriving bytes
 *   3. As network chunks arrive, extract and decrypt crypto-chunks immediately
 *   4. Never hold the full encrypted blob — only a sliding buffer
 *   5. Accumulate plaintext parts (or stream to File System Access API if available)
 */
async function decryptStream(dropId, key, filename, mime) {
    const wasm = await initWasm();

    postMessage({ type: "status", message: "Connecting..." });
    postMessage({ type: "progress", percent: 2 });

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
    const reader = response.body.getReader();

    // ── Streaming state ──
    let buffer = new Uint8Array(0); // sliding window of unprocessed bytes
    let headerParsed = false;
    let nonce = null;
    let totalChunks = 0;
    let originalSize = 0;
    let chunkIndex = 0;
    let receivedBytes = 0;
    let decryptedBytes = 0;

    // Plaintext accumulator — for final blob assembly
    const plainParts = [];

    // ── Read loop: process as data arrives ──
    while (true) {
        const { done, value } = await reader.read();

        if (value) {
            buffer = appendBuffer(buffer, value);
            receivedBytes += value.length;

            // Download progress
            if (contentLength > 0) {
                const dlPct = Math.min(45, Math.round((receivedBytes / contentLength) * 45));
                postMessage({ type: "progress", percent: 2 + dlPct });
                postMessage({
                    type: "status",
                    message: "Downloading & decrypting... " +
                        formatBytes(receivedBytes) + " / " + formatBytes(contentLength)
                });
            }
        }

        // ── Parse header from first 40 bytes ──
        if (!headerParsed && buffer.length >= HEADER_SIZE) {
            const header = parseHeader(buffer);
            nonce = header.nonce;
            totalChunks = header.totalChunks;
            originalSize = header.originalSize;
            buffer = buffer.slice(HEADER_SIZE); // consume header
            headerParsed = true;

            if (totalChunks === 0) {
                throw new Error("File appears empty \u2014 no encrypted chunks found");
            }
        }

        // ── Decrypt as many complete chunks as available ──
        if (headerParsed) {
            while (chunkIndex < totalChunks) {
                // Need at least 4 bytes for chunk length
                if (buffer.length < CHUNK_LEN_SIZE) break;

                const chunkLen = readU32LE(buffer, 0);

                // Sanity check
                if (chunkLen === 0 || chunkLen > MAX_CHUNK_SIZE) {
                    throw new Error("Invalid chunk length at chunk " + chunkIndex + ": " + chunkLen);
                }

                // Need full chunk data
                if (buffer.length < CHUNK_LEN_SIZE + chunkLen) break;

                // Extract encrypted chunk
                const encryptedChunk = buffer.slice(CHUNK_LEN_SIZE, CHUNK_LEN_SIZE + chunkLen);

                // Advance buffer — release encrypted memory immediately
                buffer = buffer.slice(CHUNK_LEN_SIZE + chunkLen);

                // Decrypt via WASM
                let plaintext;
                try {
                    plaintext = wasm.decrypt_chunk(encryptedChunk, key, nonce, BigInt(chunkIndex));
                } catch (e) {
                    throw new Error("Decryption failed at chunk " + chunkIndex + ": " + String(e));
                }

                plainParts.push(plaintext);
                decryptedBytes += plaintext.length;
                chunkIndex++;

                // Combined progress: download (0-47%) + decrypt (47-92%)
                const pct = Math.min(92, Math.round((chunkIndex / totalChunks) * 90) + 2);
                postMessage({ type: "progress", percent: pct });

                // Yield every 100 chunks to keep messages flowing
                if (chunkIndex % 100 === 0) {
                    postMessage({
                        type: "status",
                        message: "Decrypting... " + formatBytes(decryptedBytes) + " / " + formatBytes(originalSize)
                    });
                    await new Promise(function (r) { setTimeout(r, 0); });
                }
            }
        }

        if (done) break;
    }

    // ── Verify all chunks were decrypted ──
    if (chunkIndex < totalChunks) {
        throw new Error(
            "Incomplete download: got " + chunkIndex + "/" + totalChunks + " chunks. " +
            "Connection may have been interrupted."
        );
    }

    postMessage({ type: "progress", percent: 94 });
    postMessage({ type: "status", message: "Preparing download..." });

    // ── Merge plaintext parts ──
    const result = new Uint8Array(decryptedBytes);
    let offset = 0;
    for (const part of plainParts) {
        result.set(part, offset);
        offset += part.length;
    }

    postMessage({ type: "progress", percent: 98 });

    // Transfer ownership for zero-copy handoff
    postMessage({
        type: "complete",
        data: result.buffer,
        filename: sanitizeString(filename),
        mime: sanitizeString(mime),
        size: decryptedBytes
    }, [result.buffer]);
}

// ── Worker message handler ──
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
