// ─── deadrop upload — client-side encryption & upload ───

// ─── SECURITY: Extract key from URL fragment immediately ───
let key = null;
if (window.location.hash && window.location.hash.length > 1) {
    key = window.location.hash.substring(1);
    // Strip the #key from URL bar and history
    history.replaceState(null, '', window.location.pathname + window.location.search);
}

const dropZone = document.getElementById('drop-zone');
const fileInput = document.getElementById('file-input');
const browseLink = document.getElementById('browse-link');
const fileInfo = document.getElementById('file-info');
const fileName = document.getElementById('file-name');
const fileSize = document.getElementById('file-size');
const btn = document.getElementById('btn');
const statusEl = document.getElementById('status');
const progressCont = document.getElementById('progress-container');
const progressFill = document.getElementById('progress-fill');
const progressText = document.getElementById('progress-text');

let selectedFile = null;

// ─── Validate key presence ───
if (!key) {
    statusEl.innerHTML = '<span class="error">No encryption key found in URL — cannot encrypt</span>';
    dropZone.style.display = 'none';
}

// ─── Format bytes ───
function formatBytes(bytes) {
    if (bytes === 0) return '0 B';
    const units = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
    return (bytes / Math.pow(1024, i)).toFixed(i > 0 ? 1 : 0) + ' ' + units[i];
}

// ─── Drag & drop ───
dropZone.addEventListener('dragover', (e) => {
    e.preventDefault();
    dropZone.classList.add('dragover');
});

dropZone.addEventListener('dragleave', () => {
    dropZone.classList.remove('dragover');
});

dropZone.addEventListener('drop', (e) => {
    e.preventDefault();
    dropZone.classList.remove('dragover');
    if (e.dataTransfer.files.length > 0) {
        handleFile(e.dataTransfer.files[0]);
    }
});

// ─── Browse click ───
browseLink.addEventListener('click', (e) => {
    e.stopPropagation();
    fileInput.click();
});
dropZone.addEventListener('click', () => fileInput.click());
fileInput.addEventListener('change', () => {
    if (fileInput.files.length > 0) {
        handleFile(fileInput.files[0]);
    }
});

// ─── File selected ───
function handleFile(file) {
    selectedFile = file;
    fileName.textContent = file.name;
    fileSize.textContent = formatBytes(file.size);
    fileInfo.style.display = 'block';
    btn.style.display = 'block';
    statusEl.textContent = 'Ready to encrypt & send';
    dropZone.style.display = 'none';
}

// ─── Encrypt & Upload via Web Worker ───
btn.addEventListener('click', async () => {
    if (!selectedFile || !key) return;

    btn.disabled = true;
    btn.textContent = 'Encrypting...';
    progressCont.style.display = 'block';

    if (typeof Worker !== 'undefined') {
        const worker = new Worker('/assets/upload-worker.js', { type: 'module' });

        worker.onmessage = (e) => {
            const msg = e.data;
            switch (msg.type) {
                case 'progress':
                    progressFill.style.width = msg.percent + '%';
                    break;
                case 'status':
                    progressText.textContent = msg.message;
                    break;
                case 'complete':
                    progressFill.style.width = '100%';
                    progressFill.style.background = '#00ff88';
                    statusEl.innerHTML = '<span class="success">✅ File encrypted and sent successfully!</span>';
                    btn.style.display = 'none';
                    progressText.textContent = 'Complete — ' + formatBytes(msg.size);
                    // Nuke key from memory
                    key = null;
                    worker.terminate();
                    break;
                case 'error':
                    statusEl.innerHTML = '<span class="error">' + msg.message + '</span>';
                    progressFill.style.background = '#ff4444';
                    btn.textContent = 'Retry';
                    btn.disabled = false;
                    worker.terminate();
                    break;
            }
        };

        worker.onerror = (e) => {
            statusEl.innerHTML = '<span class="error">Worker error: ' + e.message + '</span>';
            btn.textContent = 'Retry';
            btn.disabled = false;
        };

        // Read file and send to worker with the encryption key
        const arrayBuffer = await selectedFile.arrayBuffer();
        worker.postMessage({
            action: 'encrypt_and_upload',
            data: arrayBuffer,
            filename: selectedFile.name,
            mime: selectedFile.type || 'application/octet-stream',
            key: key,
        }, [arrayBuffer]);

    } else {
        statusEl.innerHTML = '<span class="error">Web Workers not supported — please use a modern browser</span>';
    }
});
