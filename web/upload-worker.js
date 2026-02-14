"use strict";
/**
 * deadrop — Upload Encryption Web Worker
 *
 * Encrypts files in a background thread using WASM before uploading
 * to the receive-mode server. The encryption key comes from the URL
 * fragment — same zero-knowledge model as download mode.
 *
 * Flow:
 * 1. Main thread reads file + extracts #key from URL
 * 2. Worker encrypts file via WASM (XChaCha20-Poly1305, 64KB chunks)
 * 3. Worker uploads encrypted blob to /api/upload
 * 4. Server decrypts with same key and saves to disk
 */

let wasmModule = null;

async function initWasm() {
    if (wasmModule) return wasmModule;
    const wasm = await import("/wasm/deadrop_wasm.js");
    await wasm.default();
    wasmModule = wasm;
    return wasm;
}

function formatBytes(bytes) {
    if (bytes === 0) return "0 B";
    const units = ["B", "KB", "MB", "GB", "TB"];
    const i = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
    return (bytes / Math.pow(1024, i)).toFixed(i > 0 ? 1 : 0) + " " + units[i];
}

async function encryptAndUpload(data, filename, mime, keyBase64) {
    const wasm = await initWasm();

    postMessage({ type: "status", message: "Encrypting file..." });
    postMessage({ type: "progress", percent: 5 });

    // ─── Encrypt via WASM using the key from URL fragment ───
    const plaintext = new Uint8Array(data);
    let encrypted;
    try {
        encrypted = wasm.encrypt_blob(plaintext, keyBase64);
    } catch (e) {
        throw new Error("Encryption failed: " + String(e));
    }

    postMessage({ type: "progress", percent: 50 });
    postMessage({
        type: "status",
        message: "Encrypted " + formatBytes(plaintext.length) + " — uploading..."
    });

    // ─── Upload encrypted blob ───
    const response = await fetch("/api/upload", {
        method: "POST",
        headers: {
            "Content-Type": "application/octet-stream",
            "X-Filename": encodeURIComponent(filename),
            "X-Mime": encodeURIComponent(mime),
            "X-Original-Size": plaintext.length.toString(),
        },
        body: encrypted,
    });

    if (!response.ok) {
        const errText = await response.text().catch(() => "Unknown error");
        throw new Error("Upload failed (HTTP " + response.status + "): " + errText);
    }

    postMessage({ type: "progress", percent: 95 });

    const result = await response.json();

    postMessage({
        type: "complete",
        savedAs: result.saved_as,
        size: result.size,
    });
}

// ─── Worker message handler ───
self.onmessage = async function (e) {
    if (!e.data || typeof e.data !== "object") return;
    const { action, data, filename, mime, key } = e.data;

    if (action === "encrypt_and_upload") {
        if (!data) {
            postMessage({ type: "error", message: "No file data provided" });
            return;
        }
        if (!key) {
            postMessage({ type: "error", message: "No encryption key — was the URL fragment missing?" });
            return;
        }
        try {
            await encryptAndUpload(data, filename, mime, key);
        } catch (err) {
            postMessage({ type: "error", message: err.message || String(err) });
        }
    }
};
