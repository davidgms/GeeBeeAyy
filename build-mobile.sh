#!/bin/bash
# build-mobile.sh — Build Rust core for Android and iOS targets
#
# Usage:
#   ./build-mobile.sh android-arm64
#   ./build-mobile.sh android-arm
#   ./build-mobile.sh android-all    (both arm64 + arm)
#   ./build-mobile.sh ios-arm64
#   ./build-mobile.sh ios-sim

set -euo pipefail
cd "$(dirname "$0")"

source "$HOME/.cargo/env"

TARGET="${1:-help}"
CORE_DIR="core"
ANDROID_JNI_DIR="android/app/src/main/jniLibs"

# Auto-detect Android NDK linker if not already configured
setup_android_ndk() {
    if [ -n "${NDK_HOME:-}" ] && [ -d "$NDK_HOME" ]; then
        echo "Using NDK from NDK_HOME=$NDK_HOME"
        return
    fi

    # Try common NDK locations
    local NDK_PATHS=(
        "$HOME/android-sdk/ndk"
        "$HOME/Android/Sdk/ndk"
        "${ANDROID_HOME:-}/ndk"
        "/usr/local/lib/android/sdk/ndk"
        # WSL: Windows Android Studio NDK
        "/mnt/c/Users/$USER/AppData/Local/Android/Sdk/ndk"
    )

    for ndk_base in "${NDK_PATHS[@]}"; do
        if [ -d "$ndk_base" ]; then
            # Find latest version
            local latest
            latest=$(ls -1 "$ndk_base" 2>/dev/null | sort -V | tail -1)
            if [ -n "$latest" ] && [ -d "$ndk_base/$latest" ]; then
                export NDK_HOME="$ndk_base/$latest"
                echo "Auto-detected NDK: $NDK_HOME"
                return
            fi
        fi
    done

    echo "ERROR: Android NDK not found."
    echo "Install it: Android Studio → SDK Manager → SDK Tools → NDK (Side by side)"
    echo "Or set NDK_HOME=/path/to/ndk/version"
    exit 1
}

# Configure Rust to use the NDK linker
configure_rust_android() {
    setup_android_ndk

    local TOOLCHAIN="$NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64"

    if [ ! -d "$TOOLCHAIN" ]; then
        echo "ERROR: NDK toolchain not found at $TOOLCHAIN"
        echo "If on WSL, NDK might only have Windows binaries."
        echo "Install Linux NDK: sdkmanager --install 'ndk;27.0.12077973'"
        exit 1
    fi

    local BIN="$TOOLCHAIN/bin"

    # Create cargo config for Android targets
    mkdir -p "$CORE_DIR/.cargo"
    cat > "$CORE_DIR/.cargo/config.toml" << EOF
[target.aarch64-linux-android]
linker = "$BIN/aarch64-linux-android35-clang"

[target.armv7-linux-androideabi]
linker = "$BIN/armv7a-linux-androideabi35-clang"
EOF

    echo "Configured Rust linker for Android NDK"
}

build_android_arm64() {
    echo "Building for Android arm64-v8a..."
    rustup target add aarch64-linux-android 2>/dev/null || true
    configure_rust_android

    cargo build --manifest-path "$CORE_DIR/Cargo.toml" --target aarch64-linux-android --release

    mkdir -p "$ANDROID_JNI_DIR/arm64-v8a"
    cp "$CORE_DIR/target/aarch64-linux-android/release/libgeebeeayy_core.so" \
       "$ANDROID_JNI_DIR/arm64-v8a/"

    echo "Installed: $ANDROID_JNI_DIR/arm64-v8a/libgeebeeayy_core.so"
}

build_android_arm() {
    echo "Building for Android armeabi-v7a..."
    rustup target add armv7-linux-androideabi 2>/dev/null || true
    configure_rust_android

    cargo build --manifest-path "$CORE_DIR/Cargo.toml" --target armv7-linux-androideabi --release

    mkdir -p "$ANDROID_JNI_DIR/armeabi-v7a"
    cp "$CORE_DIR/target/armv7-linux-androideabi/release/libgeebeeayy_core.so" \
       "$ANDROID_JNI_DIR/armeabi-v7a/"

    echo "Installed: $ANDROID_JNI_DIR/armeabi-v7a/libgeebeeayy_core.so"
}

case "$TARGET" in
  android-arm64)
    build_android_arm64
    ;;

  android-arm)
    build_android_arm
    ;;

  android-all)
    build_android_arm64
    build_android_arm
    echo ""
    echo "Both Android ABIs built successfully."
    ;;

  ios-arm64)
    echo "Building for iOS arm64 (device)..."
    rustup target add aarch64-apple-ios 2>/dev/null || true
    cargo build --manifest-path "$CORE_DIR/Cargo.toml" --target aarch64-apple-ios --release
    echo "Output: $CORE_DIR/target/aarch64-apple-ios/release/libgeebeeayy_core.a"
    ;;

  ios-sim)
    echo "Building for iOS Simulator (x86_64)..."
    rustup target add x86_64-apple-ios 2>/dev/null || true
    cargo build --manifest-path "$CORE_DIR/Cargo.toml" --target x86_64-apple-ios --release
    echo "Output: $CORE_DIR/target/x86_64-apple-ios/release/libgeebeeayy_core.a"
    ;;

  help|*)
    echo "Usage: $0 <target>"
    echo ""
    echo "Targets:"
    echo "  android-arm64   Android arm64-v8a (most modern devices)"
    echo "  android-arm     Android armeabi-v7a (older devices)"
    echo "  android-all     Both Android ABIs"
    echo "  ios-arm64       iOS device (arm64)"
    echo "  ios-sim         iOS Simulator (x86_64)"
    ;;
esac
