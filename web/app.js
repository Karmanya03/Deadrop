    // ─── SECURITY: Extract key and nuke from URL/history immediately ───
    const dropIdMatch = window.location.pathname.match(/\/d\/([^/]+)\/?$/);
    if (!dropIdMatch) {
        statusEl.className = 'error';
        statusEl.textContent = 'Invalid download link. Open the /d/<id> route from a shared drop URL.';
        throw new Error('Invalid drop route');
    }
    const dropId = decodeURIComponent(dropIdMatch[1]);

    let key = null;
    if (window.location.hash && window.location.hash.length > 1) {
        key = window.location.hash.substring(1);
        // Immediately strip the #key from URL bar, history, and back button
        history.replaceState(null, '', window.location.pathname + window.location.search);
    }

    // ─── DOM refs ───
    const statusEl = document.getElementById('status');
    const btnEl = document.getElementById('btn');
    const metaCard = document.getElementById('meta-card');
    const progressCont = document.getElementById('progress-container');
    const progressFill = document.getElementById('progress-fill');
    const progressText = document.getElementById('progress-text');
    const shieldEl = document.getElementById('shield');
    let fileMeta = null;

    // ─── Validate key presence ───
    if (!key) {
        statusEl.className = 'error';
        statusEl.textContent = 'No decryption key found in URL';
        throw new Error('No key');
    }

    // ─── Fetch metadata ───
    async function loadMeta() {
        try {
            const res = await fetch(`/api/meta/${dropId}`);

            // Handle burned drops (HTTP 410 Gone)
            if (res.status === 410) {
                statusEl.className = 'success';
                statusEl.textContent = '🔥 This drop was already downloaded and destroyed.';
                shieldEl.style.display = 'block';
                key = null;
                return;
            }

            if (!res.ok) {
                statusEl.className = 'error';
                statusEl.textContent = 'Drop not found — it may have expired or self-destructed';
                return;
            }

            fileMeta = await res.json();
            document.getElementById('meta-filename').textContent = fileMeta.filename;
            document.getElementById('meta-size').textContent = fileMeta.size;
            document.getElementById('meta-downloads').textContent = fileMeta.downloads_remaining;

            // Format expiry
            const expiresAt = new Date(fileMeta.expires_at);
            const diff = expiresAt - Date.now();
            if (diff > 0) {
                const mins = Math.round(diff / 60000);
                document.getElementById('meta-expires').textContent =
                    mins > 60 ? `${Math.round(mins / 60)}h ${mins % 60}m` : `${mins}m`;

                // ─── SECURITY: Auto-destruct the page when the drop expires ───
                setTimeout(() => {
                    key = null;
                    statusEl.className = 'error';
                    statusEl.textContent = '⏰ This drop has expired and self-destructed.';
                    btnEl.style.display = 'none';
                    metaCard.style.display = 'none';
                    progressCont.style.display = 'none';
                }, diff);
            } else {
                document.getElementById('meta-expires').textContent = 'Expired';
            }

            metaCard.style.display = 'block';
            shieldEl.style.display = 'block';
            btnEl.style.display = 'inline-block';
            statusEl.textContent = 'Ready to download';
        } catch (e) {
            statusEl.className = 'error';
            statusEl.textContent = 'Connection failed: ' + (e && e.message ? e.message : String(e));
        }
    }

    // ─── Trigger file save (universal) ───
    function saveFile(arrayBuffer, filename, mime) {
        const blob = new Blob([arrayBuffer], { type: mime });

        if ('showSaveFilePicker' in window) {
            showSaveFilePicker({ suggestedName: filename })
                .then(handle => handle.createWritable())
                .then(writable => writable.write(blob).then(() => writable.close()))
                .then(() => onSuccess())
                .catch(err => {
                    if (err.name !== 'AbortError') {
                        fallbackDownload(blob, filename);
                    }
                });
        } else {
            fallbackDownload(blob, filename);
        }
    }

    function fallbackDownload(blob, filename) {
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = filename;
        a.click();
        setTimeout(() => {
            URL.revokeObjectURL(url);
        }, 1000);
        onSuccess();
    }

    function onSuccess() {
        progressFill.style.width = '100%';
        progressFill.style.background = '#00ff88';
        statusEl.className = 'success';
        statusEl.textContent = '✅ Decrypted and saved! This drop has self-destructed on the server.';
        btnEl.style.display = 'none';
        progressText.textContent = 'Complete';

        // ─── SECURITY: Nuke the key from memory after successful download ───
        key = null;
    }

    // ─── Download & Decrypt via Web Worker ───
    async function startDecrypt() {
        btnEl.disabled = true;
        btnEl.textContent = 'Decrypting...';
        progressCont.style.display = 'block';

        if (typeof Worker !== 'undefined') {
            const worker = new Worker('/assets/worker.js', { type: 'module' });

            worker.onmessage = (e) => {
                const msg = e.data;
                switch (msg.type) {
                    case 'progress':
                        progressFill.style.width = msg.percent;
                        break;
                    case 'status':
                        progressText.textContent = msg.message;
                        break;
                    case 'complete':
                        saveFile(msg.data, msg.filename, msg.mime);
                        worker.terminate();
                        break;
                    case 'error':
                        statusEl.className = 'error';
                        statusEl.textContent = msg.message || 'Download failed';
                        progressFill.style.background = '#ff4444';
                        btnEl.textContent = 'Retry';
                        btnEl.disabled = false;
                        worker.terminate();
                        break;
                }
            };

            worker.onerror = (e) => {
                statusEl.className = 'error';
                statusEl.textContent = 'Worker error: ' + (e && e.message ? e.message : String(e));
                btnEl.textContent = 'Retry';
                btnEl.disabled = false;
            };

            worker.postMessage({
                action: 'decrypt',
                dropId: dropId,
                key: key,
                filename: fileMeta.filename,
                mime: fileMeta.mime,
            });

        } else {
            statusEl.textContent = 'Decrypting on main thread...';
            try {
                const wasm = await import('/wasm/deadrop_wasm.js');
                await wasm.default();
                const res = await fetch(`/api/blob/${dropId}`);
                const encrypted = new Uint8Array(await res.arrayBuffer());
                progressFill.style.width = '50%';
                const plaintext = wasm.decrypt_blob(encrypted, key);
                progressFill.style.width = '90%';
                saveFile(plaintext.buffer, fileMeta.filename, fileMeta.mime);
            } catch (e) {
                statusEl.className = 'error';
                statusEl.textContent = String((e && e.message) ? e.message : e);
                btnEl.textContent = 'Retry';
                btnEl.disabled = false;
            }
        }
    }

    // ─── Wire up button ───
    btnEl.addEventListener('click', startDecrypt);

    // ─── Init ───
    loadMeta();
