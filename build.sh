#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="${SCRIPT_DIR}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log() { echo -e "${BLUE}[GeeBee-A]${NC} $1"; }
success() { echo -e "${GREEN}[OK]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }

usage() {
    cat <<EOF
GeeBee-A! Build Script

Usage: $0 <command> [options]

Commands:
  core            Build Rust emulation core
  android         Build Android APK (debug)
  android-release Build Android APK (release)
  check           Run cargo check + clippy
  test            Run tests
  clean           Clean build artifacts
  docker-build    Build Docker image
  docker-shell    Open shell in Docker container
  docker-check    Run checks in Docker
  docker-core     Build core in Docker
  docker-android  Build Android APK in Docker

Examples:
  $0 core                    # Build Rust core locally
  $0 android                 # Build debug APK
  $0 docker-android          # Build APK using Docker
  $0 docker-shell            # Interactive Docker shell

EOF
    exit 0
}

check_local_tools() {
    log "Checking local tools..."
    local missing=()

    command -v rustc &>/dev/null || missing+=("rustc")
    command -v cargo &>/dev/null || missing+=("cargo")
    command -v docker &>/dev/null || missing+=("docker")

    if [ ${#missing[@]} -gt 0 ]; then
        warn "Missing tools: ${missing[*]}"
        warn "Install with: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        return 1
    fi
    success "All local tools available"
    return 0
}

build_core() {
    log "Building Rust core..."
    cd "${PROJECT_DIR}/core"
    cargo build --release
    success "Core built: target/release/libgeebee_core.a"
}

build_android() {
    local release=false
    if [ "${1:-}" = "release" ]; then
        release=true
    fi

    log "Building Rust core for Android (arm64)..."
    cd "${PROJECT_DIR}/core"
    cargo ndk -t arm64-v8a -t armeabi-v7a build --release

    log "Building Android APK..."
    cd "${PROJECT_DIR}/android"
    if [ "$release" = true ]; then
        ./gradlew assembleRelease
        success "Release APK built"
    else
        ./gradlew assembleDebug
        success "Debug APK built"
    fi

    log "APK location:"
    find "${PROJECT_DIR}/android" -name "*.apk" -type f
}

run_check() {
    log "Running checks..."
    cd "${PROJECT_DIR}/core"
    cargo check
    cargo clippy -- -D warnings
    cargo fmt --check
    success "All checks passed"
}

run_test() {
    log "Running tests..."
    cd "${PROJECT_DIR}/core"
    cargo test
    success "All tests passed"
}

clean() {
    log "Cleaning build artifacts..."
    cd "${PROJECT_DIR}/core"
    cargo clean
    rm -rf "${PROJECT_DIR}/android/app/build"
    success "Cleaned"
}

docker_build() {
    log "Building Docker image..."
    cd "${PROJECT_DIR}"
    docker build -t geebee-a:latest .
    success "Docker image built: geebee-a:latest"
}

docker_shell() {
    log "Opening Docker shell..."
    cd "${PROJECT_DIR}"
    docker compose run --rm dev
}

docker_check() {
    log "Running checks in Docker..."
    cd "${PROJECT_DIR}"
    docker compose run --rm check
}

docker_core() {
    log "Building core in Docker..."
    cd "${PROJECT_DIR}"
    docker compose run --rm build-core
}

docker_android() {
    log "Building Android APK in Docker..."
    cd "${PROJECT_DIR}"
    docker compose run --rm build-android
}

# Main
case "${1:-}" in
    core)           build_core ;;
    android)        build_android ;;
    android-release) build_android release ;;
    check)          run_check ;;
    test)           run_test ;;
    clean)          clean ;;
    docker-build)   docker_build ;;
    docker-shell)   docker_shell ;;
    docker-check)   docker_check ;;
    docker-core)    docker_core ;;
    docker-android) docker_android ;;
    help|--help|-h) usage ;;
    *)              error "Unknown command: $1. Run '$0 help' for usage." ;;
esac
