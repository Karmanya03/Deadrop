<p align="center">
  <img src="assets/deadrop-logo.png" width="220" alt="deadrop logo" />
</p>

<h1 align="center">deadrop</h1>

<p align="center">
  <b>Zero‑knowledge file drops that self‑destruct.</b><br/>
  One command. One link. Gone. Like it never happened.
</p>

<p align="center">
  <a href="https://crates.io/crates/deadrop"><img src="https://img.shields.io/crates/v/deadrop.svg?style=flat-square&color=00ff88" alt="crates.io" /></a>
  <a href="https://github.com/Karmanya03/Deadrop/releases"><img src="https://img.shields.io/github/v/release/Karmanya03/Deadrop?style=flat-square&color=00ff88" alt="release" /></a>
  <a href="https://github.com/Karmanya03/Deadrop/blob/main/LICENSE"><img src="https://img.shields.io/badge/license-MIT-00ff88?style=flat-square" alt="license" /></a>
  <img src="https://img.shields.io/badge/encryption-XChaCha20--Poly1305-00ff88?style=flat-square" alt="encryption" />
  <img src="https://img.shields.io/badge/written_in-Rust_🦀-00ff88?style=flat-square" alt="rust" />
</p>

<p align="center">
  <img src="https://img.shields.io/badge/server_knows-nothing_🤷-ff4444?style=flat-square" alt="zero knowledge" />
  <img src="https://img.shields.io/badge/after_download-💥_self_destructs-ff4444?style=flat-square" alt="self destruct" />
  <img src="https://img.shields.io/badge/dependencies-just_the_binary-blueviolet?style=flat-square" alt="single binary" />
</p>

---

## What is this?

Remember in spy movies when someone leaves a briefcase under a park bench, and someone else picks it up later? That's a **dead drop**.

This is that, but for files. And the briefcase is encrypted with military-grade cryptography. And the park bench self-destructs after pickup. And nobody — not even the bench — knows what's inside.

```
You                              Your friend
 │                                    │
 │  ded ./secret-plans.pdf            │
 │  ─────────────────────►            │
 │  here's a link + QR code          │
 │                                    │
 │          (sends link via Signal)   │
 │                                    │
 │                    opens link in browser
 │                    browser decrypts locally
 │                    downloads the file
 │                                    │
 │  💥 file self-destructs            │
 │  🛑 server shuts down              │
 │                                    │
 │  what file? there was no file.     │
```

## Features

### Core

| Feature | Description |
|---|---|
| 🔐 **End‑to‑end encrypted** | XChaCha20‑Poly1305. The server never sees the key. Ever. |
| 🔗 **Key in URL fragment** | The `#key` part never hits server logs, proxies, or HTTP headers |
| 💥 **Self‑destruct** | Expire by time, by download count, or both |
| 📱 **Works on phones** | Receiver only needs a browser. No app. No account. No signup. |
| 📁 **Send folders** | Directories auto‑pack to `.tar.gz` before encryption |
| ♾️ **Unlimited file size** | Streams from disk — your 50GB file won't eat your RAM |
| 🔑 **Optional password** | Argon2id key derivation (64MB memory‑hard, GPU‑resistant) |
| 📦 **Single binary** | No runtime, no Docker, no config files. Just one executable. |
| 📲 **QR code** | Because typing URLs is for people who still use fax machines |

### Security Hardening

| Feature | Description |
|---|---|
| 👻 **Fragment auto‑clear** | `#key` is stripped from the URL bar and browser history the instant the page loads |
| 🔒 **IP pinning** | Download is locked to the first IP that connects — anyone else gets HTTP 403 |
| 🛡 **Security headers** | CSP, `X-Frame-Options: DENY`, `no-referrer`, `no-cache`, anti‑clickjack |
| ⏱ **Rate limiting** | 2 req/sec per IP with burst of 5 — stops brute‑force ID enumeration |
| 🎯 **16‑char drop IDs** | ~2⁶⁴ possible IDs — statistically impossible to guess |
| 🕐 **Constant‑time 404s** | Random delay on not-found responses prevents timing‑based ID enumeration |
| 🔥 **Burn page** | Late visitors see "🔥 This drop was already downloaded and destroyed" instead of a generic 404 |
| ⏰ **Auto‑expire page** | If the tab stays open past expiry, the key is nuked from JS memory and the UI self‑destructs |
| 🧠 **Memory locking** | `mlock()` on Unix prevents the encryption key from being swapped to disk |
| 🗑 **Zero‑write deletion** | Encrypted temp files are overwritten with zeros before `rm` — no forensic recovery |
| 🧹 **Key zeroization** | Encryption key is wiped from RAM (via `zeroize`) on drop, both server‑side and in‑browser |

## Installation

### 🚀 One-line install (Linux/macOS)

```bash
curl -fsSL https://raw.githubusercontent.com/Karmanya03/Deadrop/main/install.sh | bash
```

> Detects your OS & architecture automatically, downloads the right binary, and adds it to your PATH.

### Download a binary

Grab the latest release for your platform from [**Releases**](https://github.com/Karmanya03/Deadrop/releases).

| Platform | Binary | Architecture |
|---|---|---|
| **Windows** | [`ded-windows-x86_64.exe`](https://github.com/Karmanya03/Deadrop/releases/latest/download/ded-windows-x86_64.exe) | x86_64 |
| **Linux** | [`ded-linux-x86_64`](https://github.com/Karmanya03/Deadrop/releases/latest/download/ded-linux-x86_64) | x86_64 (musl, static) |
| **Linux** | [`ded-linux-aarch64`](https://github.com/Karmanya03/Deadrop/releases/latest/download/ded-linux-aarch64) | ARM64 (Raspberry Pi, etc.) |
| **macOS** | [`ded-macos-x86_64`](https://github.com/Karmanya03/Deadrop/releases/latest/download/ded-macos-x86_64) | Intel |
| **macOS** | [`ded-macos-aarch64`](https://github.com/Karmanya03/Deadrop/releases/latest/download/ded-macos-aarch64) | Apple Silicon (M1/M2/M3/M4) |

**Quick install (Linux/macOS):**

```bash
# Linux x86_64
curl -L https://github.com/Karmanya03/Deadrop/releases/latest/download/ded-linux-x86_64 -o ded && chmod +x ded && sudo mv ded /usr/local/bin/

# macOS Apple Silicon
curl -L https://github.com/Karmanya03/Deadrop/releases/latest/download/ded-macos-aarch64 -o ded && chmod +x ded && sudo mv ded /usr/local/bin/
```

### Via cargo

```bash
cargo install deadrop
```

### Build from source

```bash
git clone https://github.com/Karmanya03/Deadrop.git
cd Deadrop
cargo build --release
# Binary at: target/release/ded
```

### 🔄 One-line update (Linux/macOS)

```bash
curl -fsSL https://raw.githubusercontent.com/Karmanya03/Deadrop/main/install.sh | bash
```

> Same as install — it overwrites the old binary with the latest release. Your PATH stays intact.

### 🗑 One-line uninstall (Linux/macOS)

```bash
rm -f ~/.local/bin/ded && echo "deadrop removed ☠"
```

> If you installed to `/usr/local/bin/` instead:

```bash
sudo rm -f /usr/local/bin/ded && echo "deadrop removed ☠"
```

### 🗑 Uninstall (cargo)

```bash
cargo uninstall deadrop
```

## Usage

### The basics

```bash
# Send a file
ded ./secret.pdf

# Send a folder
ded ./tax-returns-2025/

# That's it. That's the tool.
```

### The spicy options

```bash
# Self-destruct after 1 download, expire in 10 minutes
ded ./evidence.zip -n 1 -e 10m

# Password-protected (because you're paranoid, and that's ok)
ded ./passwords.csv --pw "correct-horse-battery-staple"

# Custom port
ded ./file.txt -p 4200

# No QR code (you hate fun)
ded ./file.txt --no-qr

# Go full Mission Impossible
ded ./plans.pdf -n 1 -e 30s --pw "this-message-will-self-destruct"
```

### What you see

```
     ██████╗ ███████╗ █████╗ ██████╗ ██████╗  ██████╗ ██████╗
     ██╔══██╗██╔════╝██╔══██╗██╔══██╗██╔══██╗██╔═══██╗██╔══██╗
     ██║  ██║█████╗  ███████║██║  ██║██████╔╝██║   ██║██████╔╝
     ██║  ██║██╔══╝  ██╔══██║██║  ██║██╔══██╗██║   ██║██╔═══╝
     ██████╔╝███████╗██║  ██║██████╔╝██║  ██║╚██████╔╝██║
     ╚═════╝ ╚══════╝╚═╝  ╚═╝╚═════╝ ╚═╝  ╚═╝ ╚═════╝ ╚═╝
          ⚡ zero-knowledge encrypted file sharing ⚡

  ┌──────────────────────────────────────────────────┐
  │  URL  http://192.168.1.42:8080/d/a3f9c1b2#xK9m  │
  │                                                   │
  │  ├─ File       secret.pdf                         │
  │  ├─ Size       4.2 MB                             │
  │  ├─ Expires    10m                                │
  │  ├─ Downloads  1                                  │
  │  └─ Crypto     XChaCha20-Poly1305                 │
  └──────────────────────────────────────────────────┘

  █▀▀▀▀▀█ ▀▀▀█▄█ █▀▀▀▀▀█     <- QR code appears here
  █ ███ █ █▀█ ▀▄  █ ███ █        scan with phone
  ...
```

### What the receiver sees

A clean, dark download page in their browser. Click **"Download & Decrypt"** → file decrypts locally in their browser via WebAssembly → downloads to their device. The server never touches the plaintext.

## Flags Cheat Sheet

| Flag | Short | Default | What it does |
|---|---|---|---|
| `--port` | `-p` | `8080` | Port to listen on |
| `--expire` | `-e` | `1h` | Auto‑expire (`30s`, `10m`, `1h`, `7d`) |
| `--downloads` | `-n` | `1` | Max downloads before self‑destruct (0 = ∞) |
| `--pw` | — | None | Require password (Argon2id derived) |
| `--bind` | `-b` | `0.0.0.0` | Bind address |
| `--no-qr` | — | `false` | Hide QR code |

## How It Works

```
Sender                          Server (your machine)                 Receiver
  │                                    │                                  │
  │  1. Encrypt file with random key   │                                  │
  │  2. Store ciphertext on disk       │                                  │
  │  3. Key goes in URL #fragment      │                                  │
  │  ──────────────────────────────►   │                                  │
  │                                    │   4. Receiver opens URL          │
  │                                    │   ◄──────────────────────────────│
  │                                    │   5. Serve encrypted blob        │
  │                                    │   ──────────────────────────────►│
  │                                    │                                  │
  │                                    │   6. Browser extracts #key       │
  │                                    │      (stripped from URL instantly)│
  │                                    │   7. WASM decrypts locally       │
  │                                    │   8. File downloads              │
  │                                    │   9. Key wiped from JS memory    │
  │                                    │                                  │
  │                                    │   💥 Self-destruct               │
  │                                    │   🔥 Drop marked as burned      │
  │                                    │   🛑 Server shuts down           │
```

**The critical insight**: the `#fragment` in a URL is **never sent to the server**. Not in HTTP requests, not in logs, not in referrer headers. The server literally cannot learn the key even if it tried.

## Security Architecture

### Defense in Depth

```
Layer 1: Encryption      XChaCha20-Poly1305 (256-bit key, AEAD)
Layer 2: Zero-knowledge  Key in URL fragment — server never sees it
Layer 3: Network         Security headers, CSP, no-referrer, no-cache
Layer 4: Access control  IP pinning + rate limiting + 16-char drop IDs
Layer 5: Anti-forensics  mlock() + zeroize + zero-write disk deletion
Layer 6: Browser         Fragment auto-clear + auto-expire + key wipe
Layer 7: Self-destruct   One download → burn page → server shutdown
```

### Threat Model

#### ✅ Protected against

| Threat | How |
|---|---|
| Server operator learning file contents | Zero‑knowledge — key never reaches server |
| Man‑in‑the‑middle reading the key | Key lives in `#fragment`, never transmitted over HTTP |
| Server logs leaking the key | Fragments aren't logged by any HTTP server or proxy |
| Brute force on encryption | XChaCha20-Poly1305 with 256‑bit keys |
| GPU attacks on passwords | Argon2id with 64MB memory cost |
| Drop ID enumeration | 16‑char IDs (~2⁶⁴) + rate limiting + constant‑time 404s |
| URL bar shoulder surfing | Fragment stripped from URL bar on page load |
| Browser history forensics | `history.replaceState()` removes the `#key` |
| Key persisting in RAM | `zeroize` on Rust side, `key = null` on JS side |
| Key swapped to disk (Unix) | `mlock()` pins key memory pages |
| Encrypted file recovery | Zero‑overwrite before deletion |
| Clickjacking / iframe embedding | `X-Frame-Options: DENY` + `frame-ancestors 'none'` |
| XSS injection | Content Security Policy — scripts only from `'self'` |
| Stale tab leaking key | Auto‑expire nukes key from memory when drop expires |
| Late visitor confusion | Burn page — "already downloaded and destroyed" |

#### ❌ NOT protected against

- Someone who has the full URL with the `#key` (that IS the key)
- Malware on sender/receiver device (keyloggers, screen capture)
- Your friend screenshotting the file and posting it on Twitter
- Rubber hose cryptanalysis (look it up, it's not pretty)
- Time travelers

## Technical Details

| Component | Choice | Why |
|---|---|---|
| Encryption | XChaCha20‑Poly1305 | 256‑bit, extended nonce, AEAD. Used by WireGuard, Cloudflare, etc. |
| KDF | Argon2id | Memory‑hard, GPU‑resistant. Winner of the Password Hashing Competition |
| Chunk size | 64KB | Balances streaming performance vs. auth tag overhead |
| Server | Axum (Rust) | Async, zero-copy, no garbage collector |
| Rate limiter | tower_governor | Token bucket per IP — prevents brute force |
| Browser crypto | WebAssembly | Same Rust code compiled to WASM, runs in-browser at near-native speed |
| Nonce derivation | base XOR chunk_index | Per-chunk unique nonces without storing them |
| Binary embedding | rust-embed | HTML, CSS, JS, WASM all baked into the single binary |
| Memory safety | mlock + zeroize | Key never hits swap, wiped from RAM on drop |

## Memory Usage

| File Size | Server RAM (Sender) | Browser RAM (Receiver) |
|---|---|---|
| 1 MB | ~5 MB | ~5 MB |
| 100 MB | ~5 MB | ~200 MB |
| 1 GB | ~5 MB | ~2 GB (desktop) |
| 10 GB | ~5 MB | Desktop only (streaming) |

The server uses constant memory regardless of file size. It streams encrypted chunks from disk.

## FAQ

**Q: Is this legal?**
A: It's a file sharing tool with encryption. Like Signal, or HTTPS, or putting a letter in an envelope. What you put inside is your business.

**Q: Can I use this at work?**
A: Your IT department will either love you or fire you. No in-between.

**Q: Why not just use Google Drive?**
A: Google Drive knows your files. Deadrop doesn't. That's the whole point.

**Q: What happens if I lose the URL?**
A: The file is gone. That's... the feature. It's a dead drop, not Google Photos.

**Q: Can the server operator see my files?**
A: No. The encryption key is in the URL fragment which never reaches the server. The server only holds meaningless encrypted bytes.

**Q: What if someone else tries to download with the link?**
A: They can't. The download is IP-pinned to the first device that connects. A second IP gets blocked with HTTP 403.

**Q: What if I visit a dead link?**
A: If the file was already downloaded, you'll see a burn page: "🔥 This drop was already downloaded and destroyed." If it expired, you get a standard not-found message.

**Q: Why Rust?**
A: Because we wanted the binary to be fast, safe, and have zero dependencies. Also because we enjoy fighting the borrow checker on Friday nights.

## Contributing

PRs welcome. Here's what's on the radar:

- [ ] Receiver‑side streaming decryption for huge files on mobile
- [ ] Built‑in HTTPS (rustls + auto‑generated certs)
- [ ] `ded receive` mode (pull instead of push)
- [ ] Clipboard mode (`echo "secret" | ded -`)
- [ ] Tor hidden service mode
- [ ] Multi‑file drops
- [ ] Web UI upload mode

## License

MIT — do whatever you want. Just don't blame us if your dead drop gets intercepted by actual spies.

---

<p align="center">
  <sub>Built with 🦀 and paranoia.</sub>
</p>
