#!/usr/bin/env bash
# Installs the debug APK on a booted emulator and confirms the app survives
# being launched. Lives in a file rather than inline in ci.yml because
# reactivecircus/android-emulator-runner runs each LINE of its `script` input
# in a separate `sh -c`, so a variable assigned on one line is gone by the
# next - which is exactly how this check silently installed an empty path.
set -euo pipefail

apk_path=$(find apk -name '*.apk' | head -1)
if [ -z "${apk_path}" ]; then
  echo "No APK under apk/ - the download-artifact step produced nothing."
  ls -R apk || true
  exit 1
fi

echo "Installing ${apk_path}"
adb install -r "${apk_path}"

adb shell monkey -p com.geebeeayy.app -c android.intent.category.LAUNCHER 1
sleep 5

if ! adb shell pidof com.geebeeayy.app; then
  echo "com.geebeeayy.app is not running after launch - crashed or failed to start"
  adb logcat -d | tail -300
  exit 1
fi

echo "com.geebeeayy.app launched and is still running."
