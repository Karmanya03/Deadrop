<p align="center">
  <img src="assets/deadrop-logo.png" width="220" alt="deadrop logo" />
</p>

<h1 align="center">deadrop</h1>

<p align="center">
  <b>Zero‑knowledge file drops that self‑destruct.</b><br/>
  One command. One link. Gone. Like it never happened. 🫠
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
  <img src="https://img.shields.io/badge/tor-🧅_hidden_service-blueviolet?style=flat-square" alt="tor" />
</p>

---

## What is this?

Remember in spy movies when someone leaves a briefcase under a park bench, and someone else picks it up later? That's a **dead drop**.

This is that, but for files. And the briefcase is encrypted with military-grade cryptography. And the park bench self-destructs after pickup. And nobody — not even the bench — knows what's inside. And now the bench can hide on the dark web. 🧅

```
    ┌─────────┐                                          ┌─────────┐
    │   You   │                                          │  Friend │
    └────┬────┘                                          └────┬────┘
         │                                                    │
         │  ded ./secret-plans.pdf                            │
         │  ━━━━━━━━━━━━━━━━━━━━━━━                           │
         │                                                    │
    ┌────┴────────────────────────────────────────────────────┐│
    │                  🔒 Your Machine                        ││
    │  ┌──────────┐    ┌──────────────┐    ┌──────────────┐  ││
    │  │ Encrypt  │───►│  Ciphertext  │───►│ HTTP Server  │  ││
    │  │ (Rust)   │    │  (on disk)   │    │ (Axum)       │  ││
    │  └──────────┘    └──────────────┘    └──────┬───────┘  ││
    │       🔑 Key goes in URL #fragment          │          ││
    └─────────────────────────────────────────────┼──────────┘│
         │                                        │           │
         │  📲 Sends link via Signal / QR scan    │           │
         │ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─►│           │
         │                                        │           │
         │                                  ┌─────┴───────┐   │
         │                                  │  Opens URL   │◄──┘
         │                                  │  in browser  │
         │                                  └─────┬───────┘
         │                                        │
         │                          ┌─────────────┴─────────────┐
         │                          │  📦 Browser fetches blob  │
         │                          │  🔑 Extracts #key         │
         │                          │  ⚡ WASM decrypts locally  │
         │                          │  💾 File downloads         │
         │                          └─────────────┬─────────────┘
         │                                        │
    ┌────┴──────────────────────────────┐         │
    │  💥 Self-destruct triggered       │         │
    │  🔥 Drop marked as burned         │         │
    │  🛑 Server shuts down             │         │
    └───────────────────────────────────┘         │
         │                                        │
         ▼                                        ▼
    What file? There was no file.           Got it. Thanks. 👍
```

## Features

### Core — the stuff that makes your IT department nervous

| Feature | Description |
|---|---|
| 🔐 **End‑to‑end encrypted** | XChaCha20‑Poly1305. The server never sees the key. Like a blind courier. |
| 🔗 **Key in URL fragment** | The `#key` part never hits server logs, proxies, or HTTP headers. It's a ghost. |
| 💥 **Self‑destruct** | Expire by time, by download count, or both. This message will self‑destruct in... |
| 📱 **Works on phones** | Receiver only needs a browser. No app. No account. No soul‑selling signup. |
| 📁 **Send folders** | Directories auto‑pack to `.tar.gz`. Your entire `homework/` folder, encrypted. 📚 |
| 📦 **Multi‑file drops** | `ded file1.txt file2.pdf photos/` — bundles everything into one encrypted drop. |
| 📋 **Stdin / clipboard** | `echo "secret" \| ded -` — pipe anything. Your terminal is the dead drop. |
| ♾️ **Unlimited file size** | Streams from disk — your 50GB file won't eat your RAM for breakfast. |
| 🔑 **Password protection** | Argon2id key derivation (64MB memory‑hard, GPU‑resistant). Receiver gets a password prompt in-browser — key is derived client-side. The server never sees the password OR the key. Fort Knox mode. 🏰 |
| 📲 **QR code** | Because typing URLs is for people who still use fax machines. |
| 📥 **Receive mode** | `ded receive` — phone‑to‑PC uploads. Your phone becomes the dead drop. |
| 🧅 **Tor hidden service** | `--tor` — generates a `.onion` address. The dark web called, it wants its files. |
| 📦 **Single binary** | No runtime, no Docker, no config files. Just one executable. Like a katana. 🗡️ |

### Security Hardening — because paranoia is a feature

| Feature | Description |
|---|---|
| 👻 **Fragment auto‑clear** | `#key` is stripped from the URL bar and history the instant the page loads. Poof. |
| 🔒 **IP pinning** | Download is locked to the first IP that connects — everyone else gets a 403 slap. |
| 🛡 **Security headers** | CSP, `X-Frame-Options: DENY`, `no-referrer`, `no-cache`. The whole paranoia buffet. |
| ⏱ **Rate limiting** | 2 req/sec per IP with burst of 5 — stops brute‑force nerds in their tracks. |
| 🎯 **16‑char drop IDs** | ~2⁶⁴ possible IDs — you'll win the lottery before guessing one. |
| 🕐 **Constant‑time 404s** | Random delay on not-found — prevents timing‑based detective work. |
| 🔥 **Burn page** | Late visitors see "🔥 This drop was already downloaded and destroyed." Savage. |
| ⏰ **Auto‑expire page** | Tab stays open past expiry? Key nuked from JS memory. The UI self‑destructs. |
| 🧠 **Memory locking** | `mlock()` on Unix prevents the key from being swapped to disk. It lives in RAM or dies. |
| 🗑 **Zero‑write deletion** | Encrypted temp files get overwritten with zeros before `rm`. CSI can't touch this. |
| 🧹 **Key zeroization** | Key wiped from RAM (via `zeroize`) on drop, both server and browser. Clean exit. |

## Installation

### 🚀 One-line install (Linux/macOS)

```bash
curl -fsSL https://raw.githubusercontent.com/Karmanya03/Deadrop/main/install.sh | bash
```

> Detects your OS & architecture automatically, downloads the right binary, and adds it to your PATH. Magic.

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

### Build from source (for the trust‑no‑one crowd)

```bash
git clone https://github.com/Karmanya03/Deadrop.git
cd Deadrop
cargo build --release
# Binary at: target/release/ded
```

### 🔄 Update

```bash
# Linux/macOS — same as install, overwrites the old binary
curl -fsSL https://raw.githubusercontent.com/Karmanya03/Deadrop/main/install.sh | bash

# Via cargo
cargo install deadrop --force
```

### 🗑 Uninstall

```bash
# If installed via script
rm -f ~/.local/bin/ded && echo "deadrop removed ☠"

# If installed to /usr/local/bin/
sudo rm -f /usr/local/bin/ded && echo "deadrop removed ☠"

# If installed via cargo
cargo uninstall deadrop
```

## Usage

### The basics — impress your friends in 3 seconds

```bash
# Send a file — that's it, that's the whole tool
ded secret.pdf

# Send a folder — auto-archives to .tar.gz
ded ./tax-returns-2025/

# Send multiple files — bundles into one drop
ded passwords.csv nudes.zip plans.pdf

# Pipe from stdin — your clipboard is a dead drop
echo "the password is swordfish" | ded -
cat ~/.ssh/id_rsa | ded -
```

### Receive mode — your phone becomes the dead drop

```bash
# Open upload page on your LAN — scan QR from phone
ded receive

# Save to a specific folder
ded receive -o ~/Downloads/

# Custom port, no QR
ded receive -p 9090 --no-qr
```

Scan the QR from your phone → pick a file → encrypted in-browser → sent to your PC → decrypted → saved. One upload, then 💥 server self-destructs.

### Password mode — true zero‑knowledge 🔑

```bash
# Share a file with a password
ded secret.pdf --pw "correct-horse-battery-staple"
```

**How it works:**
1. Server encrypts the file with a key derived from your password via **Argon2id** (64MB, 3 iterations)
2. The URL contains the **salt** (not the key) — so the link alone can't decrypt anything
3. Receiver opens the link → sees a 🔑 password prompt → enters the password
4. Browser derives the same key via **Argon2id in WASM** (same params, runs client-side)
5. File decrypts locally. Server never sees the password or the key. Ever.

> 💡 **Pro tip:** Send the link over Slack, tell them the password on a phone call. Two channels = maximum paranoia.

### The spicy options 🌶️

```bash
# Self-destruct after 1 download, expire in 10 minutes
ded evidence.zip -n 1 -e 10m

# 30-second self-destruct. Blink and it's gone.
ded confession.txt -e 30s

# Go full Mission Impossible
ded plans.pdf -n 1 -e 30s --pw "this-message-will-self-destruct"

# Go full dark web spy 🕵️
ded whistleblower-docs.pdf --tor

# Receive via Tor — your phone uploads through the shadow realm
ded receive --tor -o ~/secrets/
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
  │  ├─ 🔑 Password  yes (Argon2id)                   │
  │  └─ Crypto     XChaCha20-Poly1305                 │
  └──────────────────────────────────────────────────┘

  🧅 Tor: http://abc...xyz.onion/d/a3f9c1b2#pw:...   ← (with --tor)

  █▀▀▀▀▀█ ▀▀▀█▄█ █▀▀▀▀▀█     <- QR code appears here
  █ ███ █ █▀█ ▀▄  █ ███ █        scan with phone 📱
  ...
```

### What the receiver sees (password-protected drop)

```
  ┌──────────────────────────────────────────┐
  │  DEADROP  encrypted dead drop            │
  │                                          │
  │  📄 File       secret.pdf                │
  │  📦 Size       4.2 MB                    │
  │  ⏰ Expires    59m                       │
  │  🔐 Encryption XChaCha20-Poly1305        │
  │                                          │
  │  🔑 This drop requires a password        │
  │  ┌──────────────────────────────────┐    │
  │  │ Enter password...               █│    │
  │  └──────────────────────────────────┘    │
  │                                          │
  │  [ 🔓 Unlock & Download ]               │
  └──────────────────────────────────────────┘
```

After entering the correct password, Argon2id runs in WASM (~2-5 seconds on the derivation), then the file decrypts and downloads. Wrong password? Decryption fails gracefully — try again.

## 🚀 Demo Commands — Try Every Feature

> **Run one at a time** — each starts a server. Ctrl+C to stop, then run the next.

| # | Feature | Command | What happens |
|---|---|---|---|
| 1 | 📄 **Single file** | `ded secret.pdf` | Encrypts → serves link → browser decrypts → 💥 |
| 2 | 📁 **Folder** | `ded ./my-folder/` | Archives → encrypts → serves `.tar.gz` |
| 3 | 📦 **Multi-file** | `ded file1.txt file2.pdf pics/` | Bundles all → one encrypted archive |
| 4 | 📋 **Stdin pipe** | `echo "swordfish" \| ded -` | Reads stdin → drops as `clipboard.txt` |
| 5 | ⏱ **Custom expiry** | `ded file.txt -e 5m` | Auto-expires after 5 minutes |
| 6 | 🔢 **Download limit** | `ded file.txt -n 3` | Self-destructs after 3 downloads |
| 7 | 🚫 **No QR** | `ded file.txt --no-qr` | URL only, no QR code (you hate fun) |
| 8 | 🔑 **Password** | `ded file.txt --pw "hunter2"` | Receiver gets password prompt. Argon2id in-browser. |
| 9 | 🌐 **Custom port** | `ded file.txt -p 9090` | Listens on port 9090 |
| 10 | 🤯 **Full paranoia** | `ded file.txt -n 1 -e 30s --pw "yolo"` | 1 download, 30s, password. Gone. |
| 11 | 📥 **Receive mode** | `ded receive -o ~/Downloads/` | Upload page → phone sends file to PC |
| 12 | 📥🔧 **Receive custom** | `ded receive -p 9999 --no-qr` | Custom port receive, no QR |
| 13 | 🧅 **Tor send** | `ded secret.pdf --tor` | Generates `.onion` URL. Dark web drop. |
| 14 | 🧅📥 **Tor receive** | `ded receive --tor -o ~/secrets/` | Tor receive. Max stealth. |
| 15 | 🔒 **IP pinning test** | `ded file.txt -n 2` | Download on PC → try on phone → 403 blocked |
| 16 | ⏰ **Auto-expiry test** | `ded file.txt -e 30s` | Wait 30s → open URL → "Drop not found" |

## ⚡ Flags Cheat Sheet

### `ded [send]` — Send mode (the default)

> `send` is optional — `ded file.txt` and `ded send file.txt` are identical.

| Flag | Short | Default | What it does | Vibe |
|---|---|---|---|---|
| `<PATH>...` | — | — | File(s), folder(s), or `-` for stdin | 🎯 The stuff you're dropping |
| `--port` | `-p` | `8080` | Port to listen on | 🚪 Pick your door |
| `--expire` | `-e` | `1h` | Auto‑expire duration (`30s`, `10m`, `1h`, `7d`) | ⏰ The countdown timer |
| `--downloads` | `-n` | `1` | Max downloads before self‑destruct (0 = ∞) | 💣 How many pickups |
| `--pw` | — | None | Password-protect drop (Argon2id, 64MB memory‑hard). Receiver gets a password prompt in-browser. | 🏰 Fort Knox mode |
| `--bind` | `-b` | `0.0.0.0` | Bind address | 🌐 Which interface |
| `--no-qr` | — | `false` | Suppress QR code | 😐 You hate fun |
| `--tor` | — | `false` | Enable Tor hidden service | 🧅 Dark web activated |

### `ded receive` — Receive mode

| Flag | Short | Default | What it does | Vibe |
|---|---|---|---|---|
| `--port` | `-p` | `8080` | Port to listen on | 🚪 Pick your door |
| `--output` | `-o` | `.` | Save received files here | 📂 Where the loot goes |
| `--bind` | `-b` | `0.0.0.0` | Bind address | 🌐 Which interface |
| `--no-qr` | — | `false` | Suppress QR code | 😐 Still no fun |
| `--tor` | — | `false` | Enable Tor hidden service | 🧅 Receive from the shadow realm |

## How It Works

### Send flow

```
  ┌──────────┐          ┌────────────────────┐          ┌──────────┐
  │  Sender  │          │   Server (your PC) │          │ Receiver │
  └─────┬────┘          └─────────┬──────────┘          └─────┬────┘
        │                         │                           │
        │  1. Generate random     │                           │
        │     256-bit key         │                           │
        │     (or derive from pw) │                           │
        │                         │                           │
        │  2. Encrypt file        │                           │
        │     XChaCha20-Poly1305  │                           │
        │                         │                           │
        │  3. Store ciphertext ──►│                           │
        │                         │                           │
        │  4. Key → URL #fragment │                           │
        │     (or salt if --pw)   │                           │
        │                         │                           │
        │  5. Share link ─ ─ ─ ─ ─│─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─►│
        │     (Signal, QR, etc.)  │                           │
        │                         │                           │
        │                         │◄── 6. Open link ──────────│
        │                         │                           │
        │                         │─── 7. Serve encrypted ──►│
        │                         │       blob (HTTP)         │
        │                         │                           │
        │                         │    8. Browser extracts    │
        │                         │       #key (or prompts    │
        │                         │        for password)      │
        │                         │                           │
        │                         │    9. WASM decrypts       │
        │                         │       locally in browser  │
        │                         │                           │
        │                         │   10. File downloads      │
        │                         │       to device           │
        │                         │                           │
        │  ┌─────────────────────────────────────────┐        │
        │  │  💥 Self-destruct │ 🔥 Burned │ 🛑 Off │        │
        │  └─────────────────────────────────────────┘        │
        ▼                                                     ▼
```

### Password flow (zero-knowledge)

```
  ┌──────────┐          ┌────────────────────┐          ┌──────────┐
  │  Sender  │          │   Server (your PC) │          │ Receiver │
  └─────┬────┘          └─────────┬──────────┘          └─────┬────┘
        │                         │                           │
        │  ded file --pw "pass"   │                           │
        │                         │                           │
        │  1. Argon2id(pass,salt) │                           │
        │     → 256-bit key       │                           │
        │                         │                           │
        │  2. Encrypt with key    │                           │
        │  3. URL = #pw:<salt>    │                           │
        │     (NOT the key!)      │                           │
        │                         │                           │
        │  5. Share link ─ ─ ─ ─ ─│─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─►│
        │  6. Tell password  ─ ─ ─│─ ─ ─ ─ (phone call) ─ ─►│
        │                         │                           │
        │                         │◄── 7. Open link ──────────│
        │                         │                           │
        │                         │    8. Browser shows 🔑    │
        │                         │       password prompt     │
        │                         │                           │
        │                         │    9. Receiver types pw   │
        │                         │   10. WASM: Argon2id      │
        │                         │       (pw,salt) → key     │
        │                         │                           │
        │                         │◄──11. Fetch blob ─────────│
        │                         │──►12. Return ciphertext──►│
        │                         │                           │
        │                         │   13. WASM decrypts       │
        │                         │   14. File downloads      │
        │                         │                           │
        │    ┌──────────────────────────────────────┐         │
        │    │  Server never saw: password or key  │         │
        │    └──────────────────────────────────────┘         │
        ▼                                                     ▼
```

### Receive flow

```
  ┌──────────┐          ┌────────────────────┐          ┌──────────┐
  │ Receiver │          │   Server (your PC) │          │  Phone   │
  │   (PC)   │          │                    │          │ (sender) │
  └─────┬────┘          └─────────┬──────────┘          └─────┬────┘
        │                         │                           │
        │  ded receive            │                           │
        │  ━━━━━━━━━━━━           │                           │
        │                         │                           │
        │  1. Generate key ──────►│                           │
        │  2. Key → QR code       │                           │
        │                         │                           │
        │                         │◄── 3. Scan QR, open ─────│
        │                         │       upload page         │
        │                         │                           │
        │                         │    4. Pick file           │
        │                         │    5. WASM encrypts       │
        │                         │       in-browser          │
        │                         │                           │
        │                         │◄── 6. Upload ciphertext ─│
        │                         │                           │
        │  7. Server decrypts  ◄──│                           │
        │  8. Saves to disk       │                           │
        │                         │                           │
        │  ┌──────────────────────────────────────┐           │
        │  │  ✅ Saved │ 💥 Self-destruct │ 🛑 Off │          │
        │  └──────────────────────────────────────┘           │
        ▼                                                     ▼
```

**The critical insight**: the `#fragment` in a URL is **never sent to the server**. Not in HTTP requests, not in logs, not in referrer headers. The server literally cannot learn the key even if it wanted to. It's like trying to read a letter through a sealed envelope. While blindfolded. In the dark.

### Tor flow 🧅

```
  ┌──────────┐     ┌─────────────┐     ┌──────────────┐     ┌──────────┐
  │  Sender  │────►│  ded --tor  │────►│  Tor Network  │────►│ Receiver │
  │          │     │             │     │  (.onion)     │     │ (Tor     │
  │          │     │  Generates  │     │               │     │  Browser)│
  │          │     │  .onion URL │     │  3 relays     │     │          │
  └──────────┘     └─────────────┘     └──────────────┘     └──────────┘
                                                               │
                                        No IP. No trace.       │
                                        Just encrypted bytes.  │
                                                               ▼
                                                          File decrypts
                                                          in browser 🔓
```

## Security Architecture

### Defense in Depth

```
  ╔══════════════════════════════════════════════════════════════════╗
  ║  Layer 7 │ Self-destruct    One download → burn → server off    ║
  ╠══════════╪══════════════════════════════════════════════════════╣
  ║  Layer 6 │ Browser          Fragment auto-clear + auto-expire   ║
  ╠══════════╪══════════════════════════════════════════════════════╣
  ║  Layer 5 │ Anti-forensics   mlock() + zeroize + zero-write     ║
  ╠══════════╪══════════════════════════════════════════════════════╣
  ║  Layer 4 │ Access control   IP pinning + rate limit + 64-bit ID║
  ╠══════════╪══════════════════════════════════════════════════════╣
  ║  Layer 3 │ Network          HTTP + security headers             ║
  ╠══════════╪══════════════════════════════════════════════════════╣
  ║  Layer 2 │ Zero-knowledge   Key in URL #fragment only          ║
  ╠══════════╪══════════════════════════════════════════════════════╣
  ║  Layer 1 │ Encryption       XChaCha20-Poly1305 (256-bit, AEAD) ║
  ╠══════════╪══════════════════════════════════════════════════════╣
  ║  Layer 0 │ Anonymity        Tor hidden service (.onion) 🧅     ║
  ╚══════════╧══════════════════════════════════════════════════════╝
```

### Threat Model

#### ✅ Protected against

| Threat | How |
|---|---|
| Server operator learning file contents | Zero‑knowledge — key never reaches server |
| Man‑in‑the‑middle reading the key | Key lives in `#fragment`, never transmitted over HTTP |
| Someone intercepting the URL (with `--pw`) | URL contains salt, not key. They still need the password. |
| Network eavesdropping | Encryption at application layer (XChaCha20-Poly1305) |
| Server logs leaking the key | Fragments aren't logged by any HTTP server or proxy |
| Brute force on encryption | XChaCha20-Poly1305 with 256‑bit keys. See you in 10²⁰ years. |
| GPU attacks on passwords | Argon2id with 64MB memory cost. Your 4090 weeps. |
| Drop ID enumeration | 16‑char IDs (~2⁶⁴) + rate limiting + constant‑time 404s |
| URL bar shoulder surfing | Fragment stripped from URL bar on page load |
| Browser history forensics | `history.replaceState()` removes the `#key` |
| Key persisting in RAM | `zeroize` on Rust side, `key = null` on JS side |
| Key swapped to disk (Unix) | `mlock()` pins key memory pages |
| Encrypted file recovery | Zero‑overwrite before deletion |
| Clickjacking / iframe embedding | `X-Frame-Options: DENY` + `frame-ancestors 'none'` |
| XSS injection | Content Security Policy — scripts only from `'self'` |
| Stale tab leaking key | Auto‑expire nukes key from memory when drop expires |
| IP tracking | `--tor` hides both sender and receiver behind .onion |
| Late visitor confusion | Burn page — "already downloaded and destroyed" |

#### ❌ NOT protected against

- Someone who has the full URL with the `#key` (for non-password drops, that IS the key — guard it)
- Malware on sender/receiver device (keyloggers, screen capture)
- Your friend screenshotting the file and posting it on Twitter
- Rubber hose cryptanalysis (look it up, it's not pretty 🔨)
- Time travelers
- Your mom looking over your shoulder
- The NSA (just kidding... unless? 👀)

## Technical Details

| Component | Choice | Why |
|---|---|---|
| Encryption | XChaCha20‑Poly1305 | 256‑bit, extended nonce, AEAD. Used by WireGuard, Cloudflare |
| KDF | Argon2id | Memory‑hard, GPU‑resistant. 64MB cost, 3 iterations. Winner of Password Hashing Competition |
| Browser KDF | Argon2id (WASM) | Same Rust `argon2` crate compiled to WASM — identical params, runs client-side |
| Chunk size | 64KB | Balances streaming performance vs. auth tag overhead |
| Server | Axum (Rust) | Async, zero-copy, no garbage collector. Blazingly fast™ |
| Rate limiter | tower_governor | Token bucket per IP — stops brute force |
| Browser crypto | WebAssembly | Same Rust code compiled to WASM, near-native speed |
| Nonce derivation | base XOR chunk_index | Per-chunk unique nonces without storing them |
| Binary embedding | rust-embed | HTML, CSS, JS, WASM all baked into the single binary |
| Memory safety | mlock + zeroize | Key never hits swap, wiped from RAM on drop |
| Anonymity | Tor hidden service | `.onion` address via local `tor` daemon |
| Archive | tar + flate2 | Folder/multi-file bundling with gzip compression |

## Memory Usage

| File Size | Server RAM | Browser RAM | Notes |
|---|---|---|---|
| 1 MB | ~5 MB | ~5 MB | Smol file, smol memory |
| 100 MB | ~5 MB | ~200 MB | Still chill |
| 1 GB | ~5 MB | ~2 GB | Desktop territory |
| 10 GB | ~5 MB | Desktop only | Streaming mode. Server doesn't care. |

The server uses constant memory regardless of file size. It streams encrypted chunks from disk. Your 50GB Linux ISO is treated the same as a 1KB text file (memory-wise).

## FAQ

**Q: Is this legal?**
A: It's a file sharing tool with encryption. Like Signal, or HTTPS, or putting a letter in an envelope. What you put inside is your business.

**Q: Can I use this at work?**
A: Your IT department will either promote you or fire you. No in-between.

**Q: Why not just use Google Drive?**
A: Google Drive knows your files. Deadrop doesn't. That's the whole point. Also Google Drive doesn't self-destruct. Boring.

**Q: What happens if I lose the URL?**
A: The file is gone. That's... the feature. It's a dead drop, not Google Photos.

**Q: Can the server see my files?**
A: No. The encryption key is in the URL fragment which never reaches the server. The server holds meaningless encrypted bytes. It's like asking if your mailbox can read your letters.

**Q: What about password-protected drops?**
A: Even better. The URL only has the salt — the server never sees the password or the key. The receiver's browser derives the key locally via Argon2id in WASM. True zero-knowledge. The server is literally clueless.

**Q: What if someone intercepts my password drop URL?**
A: Without the password, the URL is useless. It only contains a random salt. They'd need to brute-force Argon2id (64MB memory × 3 iterations per guess). Good luck with that.

**Q: What if someone else tries the link?**
A: They can't. IP pinning locks the download to the first device. Second IP gets 403'd into oblivion.

**Q: What if I visit a dead link?**
A: Already downloaded → "🔥 This drop was already downloaded and destroyed." Expired → "Drop not found." Either way, it's gone. Like your ex's texts.

**Q: Why does `--tor` take so long?**
A: Tor needs ~30-60 seconds to generate a `.onion` address and establish circuits through 3 relays. Patience. Good anonymity takes time.

**Q: Can I send multiple files?**
A: Yes! `ded file1.txt file2.pdf folder/` bundles everything into one encrypted `.tar.gz` archive automatically.

**Q: Can I pipe from stdin?**
A: `echo "the password is swordfish" | ded -` — works like a charm. Serves it as `clipboard.txt`.

**Q: Why Rust?**
A: Because we wanted the binary to be fast, safe, and have zero runtime dependencies. Also because we enjoy fighting the borrow checker at 3 AM. It builds character.

## Contributing

PRs welcome. Here's the roadmap:

- [x] ~~End-to-end encryption (XChaCha20-Poly1305)~~
- [x] ~~QR code generation~~
- [x] ~~Self-destruct by time & download count~~
- [x] ~~IP pinning~~
- [x] ~~Folder support (.tar.gz)~~
- [x] ~~`ded receive` mode (phone → PC)~~
- [x] ~~Multi-file drops~~
- [x] ~~Stdin / clipboard mode~~
- [x] ~~Tor hidden service~~
- [x] ~~Password protection (Argon2id)~~
- [x] ~~In-browser password prompt with client-side Argon2id~~
- [ ] Receiver‑side streaming decryption for huge files on mobile
- [ ] Web UI drag-and-drop improvements
- [ ] Resume interrupted downloads
- [ ] Multi-recipient drops (different keys per recipient)

## Star History

If you've read this far, you're legally obligated to star the repo. It's in the fine print.

⭐ **[Star this repo](https://github.com/Karmanya03/Deadrop)** — it makes the self-destruct mechanism work better. (Not really, but it makes us happy.)

## License

MIT — do whatever you want. Just don't blame us if your dead drop gets intercepted by actual spies. Or if your friend screenshots the file. Or if time travelers get involved.

---

<p align="center">
  <sub>Built with 🦀 and an unreasonable amount of paranoia.</sub><br/>
  <sub>Remember: just because you're paranoid doesn't mean they're not after your files.</sub>
</p>
