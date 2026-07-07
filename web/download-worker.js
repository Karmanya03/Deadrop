// Streaming download worker (ES module worker)
// Receives: { type: 'start', dropId, key }
// Posts: { type: 'meta', ... }, { type: 'progress', idx, total }, { type: 'chunk', index, data }, { type: 'done' }, { type: 'error', message }

self.onmessage = async (ev) => {
    const msg = ev.data;
    if (msg.type === 'start') {
        const dropId = msg.dropId;
        const key = msg.key; // URL-safe base64 key or password-derived key
        const recipient_priv = msg.recipient_priv || null;
        try {
            // Load WASM
            const wasmImport = await import('/wasm/deadrop_wasm.js');
            await wasmImport.default('/wasm/deadrop_wasm_bg.wasm');
            const wasm = wasmImport;

            // Fetch header metadata
            self.postMessage({ type: 'debug', message: `fetch /api/chunks/${dropId}` });
            const resp = await fetchWithTimeout(`/api/chunks/${encodeURIComponent(dropId)}`, 8000);
            if (!resp.ok) {
                const txt = await resp.text().catch(() => '');
                throw new Error(`Failed to fetch chunk metadata: ${resp.status} ${txt}`);
            }
            const meta = await resp.json();

            // Decode nonce
            const nonce_b64 = meta.nonce;
            const nonce_bytes = Uint8Array.from(atob(nonce_b64.replace(/_/g,'/').replace(/-/g,'+')), c => c.charCodeAt(0));
            // above base64 variant handling; server uses URL_SAFE_NO_PAD
            // Better: use atob after transforming URL-safe

            const total = meta.total_chunks;
            self.postMessage({ type: 'meta', filename: msg.filename || 'file', total_chunks: total, original_size: meta.original_size });
            // Open IndexedDB to resume progress
            const db = await openResumeDB();
            const last = await getLastIndex(db, dropId);
            let start_idx = (last !== null) ? (last + 1) : 0;

            for (let idx = start_idx; idx < total; idx++) {
                self.postMessage({ type: 'debug', message: `fetch /api/chunk/${dropId}/${idx}` });
                const chunkResp = await fetchWithTimeout(`/api/chunk/${encodeURIComponent(dropId)}/${idx}`, 10000);
                if (!chunkResp.ok) {
                    const txt = await chunkResp.text().catch(() => '');
                    throw new Error(`Failed to fetch chunk ${idx}: ${chunkResp.status} ${txt}`);
                }
                const encrypted = new Uint8Array(await chunkResp.arrayBuffer());

                // Decrypt chunk using wasm
                // pass chunk index as Number (wasm-bindgen expects a JS number for u64)
                const decrypted = wasm.decrypt_chunk(encrypted, key, nonce_bytes, BigInt(idx));

                // Send decrypted chunk as transferable
                self.postMessage({ type: 'chunk', index: idx, data: decrypted }, [decrypted.buffer]);
                // Update resume DB with last completed index
                await putLastIndex(db, dropId, idx);
                self.postMessage({ type: 'progress', index: idx + 1, total });
            }

            self.postMessage({ type: 'done' });
        } catch (e) {
            self.postMessage({ type: 'error', message: e.message || String(e) });
        }
    }
};

function fetchWithTimeout(url, timeoutMs) {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), timeoutMs);
    return fetch(url, { signal: controller.signal }).finally(() => clearTimeout(timeout));
}

// IndexedDB helpers for resume
function openResumeDB() {
    return new Promise((resolve, reject) => {
        const req = indexedDB.open('deadrop-resume', 1);
        req.onupgradeneeded = () => {
            req.result.createObjectStore('downloads', { keyPath: 'id' });
        };
        req.onsuccess = () => resolve(req.result);
        req.onerror = () => reject(req.error);
    });
}

function getLastIndex(db, id) {
    return new Promise((resolve, reject) => {
        const tx = db.transaction('downloads', 'readonly');
        const store = tx.objectStore('downloads');
        const rq = store.get(id);
        rq.onsuccess = () => {
            const val = rq.result;
            if (!val) resolve(null);
            else resolve(val.last_index);
        };
        rq.onerror = () => reject(rq.error);
    });
}

function putLastIndex(db, id, idx) {
    return new Promise((resolve, reject) => {
        const tx = db.transaction('downloads', 'readwrite');
        const store = tx.objectStore('downloads');
        const rq = store.put({ id, last_index: idx, updated_at: Date.now() });
        rq.onsuccess = () => resolve(true);
        rq.onerror = () => reject(rq.error);
    });
}
