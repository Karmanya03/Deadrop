#!/usr/bin/env bash
# deadrop installer — detects OS/arch, downloads the right binary, adds to PATH
# Usage: curl -fsSL https://raw.githubusercontent.com/Karmanya03/Deadrop/main/install.sh | bash

set -euo pipefail

# ─── Colors ───
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

REPO="Karmanya03/Deadrop"
BINARY="ded"
INSTALL_DIR="$HOME/.local/bin"

echo ""
echo -e "${CYAN}${BOLD}"
echo "     ██████╗ ███████╗ █████╗ ██████╗ ██████╗  ██████╗ ██████╗ "
echo "     ██╔══██╗██╔════╝██╔══██╗██╔══██╗██╔══██╗██╔═══██╗██╔══██╗"
echo "     ██║  ██║█████╗  ███████║██║  ██║██████╔╝██║   ██║██████╔╝"
echo "     ██║  ██║██╔══╝  ██╔══██║██║  ██║██╔══██╗██║   ██║██╔═══╝ "
echo "     ██████╔╝███████╗██║  ██║██████╔╝██║  ██║╚██████╔╝██║     "
echo "     ╚═════╝ ╚══════╝╚═╝  ╚═╝╚═════╝ ╚═╝  ╚═╝ ╚═════╝ ╚═╝     "
echo -e "${NC}"
echo -e "${BOLD}  ☠  Zero-knowledge encrypted file sharing${NC}"
echo ""

# ─── Detect OS ───
OS="$(uname -s)"
case "$OS" in
    Linux*)   PLATFORM="linux" ;;
    Darwin*)  PLATFORM="macos" ;;
    MINGW*|MSYS*|CYGWIN*)
        echo -e "${RED}✗ Windows detected. Please download manually from:"
        echo -e "  https://github.com/${REPO}/releases/latest${NC}"
        exit 1
        ;;
    *)
        echo -e "${RED}✗ Unsupported OS: $OS${NC}"
        exit 1
        ;;
esac

# ─── Detect Architecture ───
ARCH="$(uname -m)"
case "$ARCH" in
    x86_64|amd64)   ARCH_NAME="x86_64" ;;
    aarch64|arm64)   ARCH_NAME="aarch64" ;;
    *)
        echo -e "${RED}✗ Unsupported architecture: $ARCH${NC}"
        exit 1
        ;;
esac

ASSET="${BINARY}-${PLATFORM}-${ARCH_NAME}"
DOWNLOAD_URL="https://github.com/${REPO}/releases/latest/download/${ASSET}"

echo -e "  ${BOLD}Platform:${NC}      $PLATFORM"
echo -e "  ${BOLD}Architecture:${NC}  $ARCH_NAME"
echo -e "  ${BOLD}Binary:${NC}        $ASSET"
echo -e "  ${BOLD}Install to:${NC}    $INSTALL_DIR"
echo ""

# ─── Download ───
echo -e "${YELLOW}⬇  Downloading ${ASSET}...${NC}"

TMPDIR="$(mktemp -d)"
TMPFILE="${TMPDIR}/${BINARY}"

if command -v curl &> /dev/null; then
    HTTP_CODE=$(curl -fSL -w '%{http_code}' -o "$TMPFILE" "$DOWNLOAD_URL" 2>/dev/null || true)
elif command -v wget &> /dev/null; then
    wget -q -O "$TMPFILE" "$DOWNLOAD_URL" 2>/dev/null && HTTP_CODE="200" || HTTP_CODE="404"
else
    echo -e "${RED}✗ Neither curl nor wget found. Please install one and retry.${NC}"
    rm -rf "$TMPDIR"
    exit 1
fi

if [ ! -f "$TMPFILE" ] || [ ! -s "$TMPFILE" ] || [ "$HTTP_CODE" != "200" ]; then
    echo -e "${RED}✗ Download failed (HTTP $HTTP_CODE). Release may not exist yet.${NC}"
    echo -e "${RED}  Check: https://github.com/${REPO}/releases${NC}"
    rm -rf "$TMPDIR"
    exit 1
fi

# ─── Install ───
chmod +x "$TMPFILE"
mkdir -p "$INSTALL_DIR"
mv "$TMPFILE" "${INSTALL_DIR}/${BINARY}"
rm -rf "$TMPDIR"

echo -e "${GREEN}✓  Installed to ${INSTALL_DIR}/${BINARY}${NC}"

# ─── PATH setup ───
if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    echo ""
    echo -e "${YELLOW}⚠  $INSTALL_DIR is not in your PATH.${NC}"

    # Detect shell config file
    SHELL_NAME="$(basename "$SHELL")"
    case "$SHELL_NAME" in
        zsh)   SHELL_RC="$HOME/.zshrc" ;;
        bash)
            if [ -f "$HOME/.bashrc" ]; then
                SHELL_RC="$HOME/.bashrc"
            else
                SHELL_RC="$HOME/.bash_profile"
            fi
            ;;
        fish)  SHELL_RC="$HOME/.config/fish/config.fish" ;;
        *)     SHELL_RC="$HOME/.profile" ;;
    esac

    # Add to PATH
    if [ "$SHELL_NAME" = "fish" ]; then
        PATH_LINE="fish_add_path $INSTALL_DIR"
    else
        PATH_LINE="export PATH=\"\$HOME/.local/bin:\$PATH\""
    fi

    # Check if already in rc file
    if [ -f "$SHELL_RC" ] && grep -qF ".local/bin" "$SHELL_RC" 2>/dev/null; then
        echo -e "${CYAN}   (PATH entry already exists in $SHELL_RC)${NC}"
    else
        echo "$PATH_LINE" >> "$SHELL_RC"
        echo -e "${GREEN}✓  Added to $SHELL_RC${NC}"
    fi

    echo -e "${CYAN}   Run: ${BOLD}source $SHELL_RC${NC} ${CYAN}or restart your terminal${NC}"
fi

# ─── Verify ───
echo ""
if command -v "$BINARY" &> /dev/null; then
    echo -e "${GREEN}${BOLD}✓  deadrop is ready!${NC}"
else
    echo -e "${GREEN}${BOLD}✓  deadrop installed!${NC}"
    echo -e "${CYAN}   Restart your terminal, then run:${NC}"
fi

echo ""
echo -e "  ${BOLD}Usage:${NC}  ded ./secret-file.pdf"
echo -e "  ${BOLD}Help:${NC}   ded --help"
echo ""
echo -e "  ${CYAN}☠  Drop files. Leave no trace.${NC}"
echo ""
