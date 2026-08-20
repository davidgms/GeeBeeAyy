#!/bin/bash
# build-mobile.sh — Build Rust core for Android and iOS targets
#
# Usage:
#   ./build-mobile.sh android-arm64
#   ./build-mobile.sh android-arm
#   ./build-mobile.sh ios-arm64
#   ./build-mobile.sh ios-sim

set -euo pipefail
cd "$(dirname "$0")"

source "$HOME/.cargo/env"

TARGET="${1:-help}"

case "$TARGET" in
  android-arm64)
    echo "Building for Android arm64-v8a..."
    rustup target add aarch64-linux-android 2>/dev/null || true
    cargo build --target aarch64-linux-android --release -p geebee-core
    echo "Output: core/target/aarch64-linux-android/release/libgeebee_core.so"
    ;;

  android-arm)
    echo "Building for Android armeabi-v7a..."
    rustup target add armv7-linux-androideabi 2>/dev/null || true
    cargo build --target armv7-linux-androideabi --release -p geebee-core
    echo "Output: core/target/armv7-linux-androideabi/release/libgeebee_core.so"
    ;;

  ios-arm64)
    echo "Building for iOS arm64 (device)..."
    rustup target add aarch64-apple-ios 2>/dev/null || true
    cargo build --target aarch64-apple-ios --release -p geebee-core
    echo "Output: core/target/aarch64-apple-ios/release/libgeebee_core.a"
    ;;

  ios-sim)
    echo "Building for iOS Simulator (x86_64)..."
    rustup target add x86_64-apple-ios 2>/dev/null || true
    cargo build --target x86_64-apple-ios --release -p geebee-core
    echo "Output: core/target/x86_64-apple-ios/release/libgeebee_core.a"
    ;;

  help|*)
    echo "Usage: $0 <target>"
    echo ""
    echo "Targets:"
    echo "  android-arm64   Android arm64-v8a (most modern devices)"
    echo "  android-arm     Android armeabi-v7a (older devices)"
    echo "  ios-arm64       iOS device (arm64)"
    echo "  ios-sim         iOS Simulator (x86_64)"
    ;;
esac
