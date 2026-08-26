#!/bin/bash
# setup-android-wsl.sh — Install Android SDK + NDK in WSL for cross-compilation
#
# Run this once before using build-mobile.sh

set -euo pipefail

ANDROID_HOME="$HOME/android-sdk"
CMDLINE_TOOLS_URL="https://dl.google.com/android/repository/commandlinetools-linux-11076708_latest.zip"

echo "=== GeeBeeAyy! Android SDK Setup (WSL) ==="
echo ""

# Step 1: Install dependencies
echo "[1/4] Installing dependencies..."
sudo apt-get update -qq
sudo apt-get install -y -qq openjdk-17-jdk wget unzip > /dev/null 2>&1
echo "  Java: $(java -version 2>&1 | head -1)"

# Step 2: Download Android command-line tools
echo "[2/4] Downloading Android command-line tools..."
mkdir -p "$ANDROID_HOME/cmdline-tools"
cd /tmp
if [ ! -f commandlinetools-linux.zip ]; then
    wget -q "$CMDLINE_TOOLS_URL" -O commandlinetools-linux.zip
fi
unzip -q -o commandlinetools-linux.zip -d /tmp/cmdline-tools-tmp
rm -rf "$ANDROID_HOME/cmdline-tools/latest"
mv /tmp/cmdline-tools-tmp/cmdline-tools "$ANDROID_HOME/cmdline-tools/latest"
rm -rf /tmp/cmdline-tools-tmp
echo "  Installed to $ANDROID_HOME/cmdline-tools/latest"

# Step 3: Accept licenses and install NDK
echo "[3/4] Installing NDK (this may take a few minutes)..."
export PATH="$ANDROID_HOME/cmdline-tools/latest/bin:$PATH"
yes | sdkmanager --licenses > /dev/null 2>&1 || true
sdkmanager --install "ndk;27.0.12077973" > /dev/null 2>&1
echo "  NDK installed"

# Step 4: Set environment variables
echo "[4/4] Configuring environment..."
SHELL_RC="$HOME/.bashrc"
grep -q "ANDROID_HOME" "$SHELL_RC" 2>/dev/null || {
    echo "" >> "$SHELL_RC"
    echo "# Android SDK (GeeBeeAyy)" >> "$SHELL_RC"
    echo "export ANDROID_HOME=\"$ANDROID_HOME\"" >> "$SHELL_RC"
    echo "export NDK_HOME=\"$ANDROID_HOME/ndk/27.0.12077973\"" >> "$SHELL_RC"
    echo "export PATH=\"\$ANDROID_HOME/cmdline-tools/latest/bin:\$PATH\"" >> "$SHELL_RC"
}

export ANDROID_HOME="$ANDROID_HOME"
export NDK_HOME="$ANDROID_HOME/ndk/27.0.12077973"

echo ""
echo "=== Setup complete! ==="
echo "NDK: $NDK_HOME"
echo ""
echo "Now run: source ~/.bashrc && ./build-mobile.sh android-all"
