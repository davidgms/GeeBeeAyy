#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TOOLS_DIR="${SCRIPT_DIR}/.tools"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log() { echo -e "${BLUE}[GeeBeeAyy]${NC} $1"; }
success() { echo -e "${GREEN}[OK]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }

usage() {
    cat <<EOF
GeeBeeAyy! Local Tools Installer

These tools can't run in Docker because they need hardware access
(USB for ADB, display for GUI apps).

Usage: $0 <command>

Commands:
  install     Install all local tools
  adb         Install ADB (Android Debug Bridge)
  rust        Install Rust toolchain
  check       Verify all tools are installed
  uninstall   Remove .tools directory

Installed to: ${TOOLS_DIR}/

EOF
    exit 0
}

install_rust() {
    log "Installing Rust toolchain..."
    if command -v rustc &>/dev/null; then
        success "Rust already installed: $(rustc --version)"
    else
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
        source "${HOME}/.cargo/env"
        success "Rust installed: $(rustc --version)"
    fi

    log "Adding Android targets..."
    rustup target add aarch64-linux-android armv7-linux-androideabi 2>/dev/null || true
    success "Android targets added"
}

install_adb() {
    log "Installing ADB (repo-scoped)..."
    mkdir -p "${TOOLS_DIR}/adb"

    if [ -f "${TOOLS_DIR}/adb/adb" ]; then
        success "ADB already installed"
        return
    fi

    local ADB_VERSION="36.0.0"
    local ARCH=$(uname -m)

    if [ "$ARCH" = "x86_64" ]; then
        local ADB_URL="https://dl.google.com/android/repository/platform-tools-latest-linux.zip"
    else
        warn "Unsupported architecture for ADB: $ARCH"
        return 1
    fi

    cd /tmp
    wget -q "${ADB_URL}" -O platform-tools.zip
    unzip -q platform-tools.zip
    mv platform-tools/adb "${TOOLS_DIR}/adb/"
    mv platform-tools/fastboot "${TOOLS_DIR}/adb/" 2>/dev/null || true
    rm -rf platform-tools.zip platform-tools

    chmod +x "${TOOLS_DIR}/adb/adb"
    success "ADB installed to ${TOOLS_DIR}/adb/adb"
}

install_all() {
    install_rust
    install_adb
    echo ""
    success "All tools installed!"
    echo ""
    log "Add to your PATH:"
    echo "  export PATH=\"${TOOLS_DIR}/adb:\${PATH}\""
    echo ""
    log "Or run tools directly:"
    echo "  ${TOOLS_DIR}/adb/adb devices"
}

check_tools() {
    log "Checking installed tools..."
    local all_ok=true

    # Rust
    if command -v rustc &>/dev/null; then
        success "Rust: $(rustc --version)"
    else
        warn "Rust: NOT INSTALLED"
        all_ok=false
    fi

    # Cargo
    if command -v cargo &>/dev/null; then
        success "Cargo: $(cargo --version)"
    else
        warn "Cargo: NOT INSTALLED"
        all_ok=false
    fi

    # ADB
    if [ -f "${TOOLS_DIR}/adb/adb" ]; then
        success "ADB: ${TOOLS_DIR}/adb/adb"
    elif command -v adb &>/dev/null; then
        success "ADB: $(which adb)"
    else
        warn "ADB: NOT INSTALLED"
        all_ok=false
    fi

    # Docker
    if command -v docker &>/dev/null; then
        success "Docker: $(docker --version)"
    else
        warn "Docker: NOT INSTALLED"
        all_ok=false
    fi

    if [ "$all_ok" = true ]; then
        success "All tools ready!"
    else
        warn "Some tools missing. Run '$0 install' to install them."
    fi
}

uninstall() {
    log "Removing .tools directory..."
    rm -rf "${TOOLS_DIR}"
    success "Removed ${TOOLS_DIR}"
}

case "${1:-}" in
    install)    install_all ;;
    rust)       install_rust ;;
    adb)        install_adb ;;
    check)      check_tools ;;
    uninstall)  uninstall ;;
    help|--help|-h) usage ;;
    *)          echo "Unknown command: $1. Run '$0 help' for usage."; exit 1 ;;
esac
