# GeeBeeAyy!

A cross-platform Game Boy Advance emulator with a pixel bee theme. Android (primary) + iOS.

<p align="center">
  <img src="docs/logo.png" width="200" alt="GeeBeeAyy! Logo">
</p>

## What is this?

GeeBeeAyy! is an open-source GBA emulator built for mobile devices. The goal is to provide a fast, accurate, and beautiful emulation experience with a distinctive pixel art bee aesthetic. Written primarily in Rust for the emulation core, with native Android and iOS frontends.

## Architecture

```
geebeeayy/
├── core/                    # Rust emulation core
│   ├── src/
│   │   ├── cpu/            # ARM7TDMI interpreter (ARM + THUMB)
│   │   ├── ppu/            # Scanline-based pixel processor
│   │   ├── apu/            # Audio (2x PCM + 4x PSG)
│   │   ├── memory/         # Memory bus, DMA (4ch), I/O regs
│   │   ├── timer/          # Hardware timers (4x)
│   │   └── cart/           # ROM loading, save types
│   └── Cargo.toml
├── android/                 # Kotlin + Jetpack Compose frontend
├── ios/                     # Swift + SwiftUI frontend
├── docs/                    # Documentation, assets, research
├── Dockerfile               # Build environment container
├── docker-compose.yml       # Container orchestration
├── build.sh                 # Build script (all platforms)
├── install-tools.sh         # Local tools installer
├── .devcontainer/           # VS Code dev container config
├── README.md
├── LICENSE                  # MIT
└── .gitignore
```

## Tech Stack

| Layer | Technology | Rationale |
|-------|-----------|-----------|
| Emulation Core | **Rust** | Memory-safe, no GC pauses, compiles to ARM natively, proven for emulation (rustboyadvance-ng, SkyEmu) |
| Android UI | Kotlin + Jetpack Compose | Native Android feel, modern declarative UI, Google Play ready |
| iOS UI | Swift + SwiftUI | Native iOS feel, App Store compliant, clean architecture |
| Audio (Android) | AAudio (NDK) | Lowest latency native audio API on Android |
| Audio (iOS) | CoreAudio / AVAudioEngine | Lowest latency native audio API on iOS |
| Graphics | OpenGL ES / Metal | Hardware-accelerated, supports per-pixel rendering |

### Why not C/C++ like mGBA?

Rust offers memory safety without garbage collection, which is critical for an emulator where buffer overflows cause crashes. The Rust emulation community has proven this works well (see rustboyadvance-ng, SkyEmu). Rust also compiles to native ARM for both Android and iOS.

### Why not Flutter/React Native?

Emulation requires direct hardware access (SIMD instructions, raw audio buffers, GPU rendering). Cross-platform UI frameworks add overhead and can't easily access low-level platform APIs. Native UIs perform better and feel right on each platform.

## GBA Hardware Spec

The hardware we need to emulate accurately:

| Component | Specification |
|-----------|--------------|
| **CPU** | ARM7TDMI @ 16.78 MHz |
| **Instruction Sets** | ARM (32-bit) + THUMB (16-bit) |
| **Display** | 240x160 pixels, 15-bit color |
| **Sprites** | 128 OAM entries, max 32 per scanline |
| **Backgrounds** | 4 layers, modes 0-5 (tiled + bitmap) |
| **Audio** | 2x 8-bit PCM channels + 4x PSG channels |
| **Sample Rate** | 44.1 kHz |
| **IWRAM** | 32 KB internal work RAM |
| **EWRAM** | 256 KB external work RAM |
| **VRAM** | 96 KB video RAM |
| **OAM** | 1 KB object attribute memory |
| **DMA** | 4 channels with varied timing |
| **Timers** | 4 hardware timers (cascade support) |
| **RTC** | Real-time clock (Pokemon, etc.) |
| **Save Types** | SRAM (32KB), Flash (64K/128K), EEPROM (512B/8KB) |

## Features

### Phase 1 - Core Emulation (MVP)

**[`ROADMAP.md`](ROADMAP.md) is the authoritative status.** This list is a
summary and has been wrong before - it claimed a complete CPU while every
second instruction was being skipped. A box here means a test proves it.

As of 2026-08-30 a commercial game boots: *Yggdra Union* reaches its title
screen on the host, and the homebrew `waimanu`, `jumpingbarnabe` and
`powerpig` render. No commercial game has run on a device yet.

- [x] ARM7TDMI interpreter - passes jsmolka's `arm`, `thumb` and `memory` suites
- [x] Memory bus + I/O register dispatch - region mirroring, 8-bit video and
      save-region write rules
- [x] DMA controller (4 channels) - raises IF bits 8-11
- [x] Timer system (4 timers) - raises IF bits 3-6
- [x] ROM loading (.gba format)
- [x] Save type detection, and SRAM/Flash/EEPROM wired to the bus - passes the
      `sram`, `flash64`, `flash128` and `none` suites. EEPROM is the real
      serial protocol over DMA - see `docs/save-data.md`
- [x] HLE BIOS - the common SWIs; `BgAffineSet`/`ObjAffineSet`/`BitUnPack` are
      still stubs
- [~] Scanline-based PPU - mode 0 verified against test ROMs and against
      *Yggdra Union*'s title screen; modes 1, 2, 4, 5, windows, mosaic and
      blending are unverified
- [~] Audio output - the PSG channels and FIFOs run and a channel can be
      triggered, but no game has ever driven them and no device has played them

### Phase 2 - Android Frontend

The Android app builds and runs: an APK has been installed on a Xiaomi Mi 10T
Pro (Android 12), all ten inputs reach the core, and homebrew ROMs render. No
commercial game has been tried on a device, and audio has not been verified
there.

- [x] Kotlin + Jetpack Compose UI
- [x] On-screen touch controls, wired to the core
- [x] Save states (10 slots per game) and battery saves persisted to disk
- [x] Fast forward (2x, 4x)
- [ ] Customizable on-screen touch controls (size, position, opacity)
- [ ] Bluetooth/USB controller support (Xbox, PS, Switch Pro)
- [ ] Screen scaling (1x, 2x, 3x, fit)
- [ ] Screen filters (2xSaI, CRT, pixel-perfect)
- [ ] ROM browser with cover art + metadata
- [ ] Google Play Store distribution

### Phase 3 - iOS Frontend
- [ ] Swift + SwiftUI UI
- [ ] MFi controller support
- [ ] Touch controls + gesture support
- [ ] Save states + iCloud sync
- [ ] App Store distribution
- [ ] Widget for quick game resume

### Phase 4 - Advanced Features
- [ ] JIT recompilation (ARM host only)
- [ ] Link cable emulation (local WiFi)
- [ ] Cheat codes (GameShark / CodeBreaker)
- [ ] Rewind support
- [ ] Screen recording / screenshots
- [ ] Lua scripting interface
- [ ] Debug tools (breakpoints, memory viewer, register inspector)

## What Users Want Most

Based on analysis of top emulators (mGBA, Delta, Pizza Boy GBA, RetroArch) and thousands of user reviews:

### Must-Have (Launch Blockers)

| Feature | Why |
|---------|-----|
| **Audio sync** | #1 complaint across ALL emulators. Crackle, drift, desync |
| **Input latency < 45ms** | Critical for action games, platformers |
| **Save state reliability** | Users lose progress to corruption |
| **Controller support** | Xbox, PS4/PS5, Switch Pro via Bluetooth HID |

### Should-Have (Day 1 Quality)

| Feature | Why |
|---------|-----|
| Customizable touch overlay | Every hand is different, every game is different |
| Fast forward | Pokemon grinding, text-heavy RPGs |
| Screen filters | Nostalgia factor, screen clarity on modern displays |
| ROM library with covers | Organization, visual appeal |

### Nice-to-Have (Post-Launch)

| Feature | Why |
|---------|-----|
| Cheat codes | GameShark/CodeBreaker support |
| Link cable | Pokemon trading, multiplayer |
| Cloud sync | Continue on any device |
| RetroAchievements | Achievement hunting community |

## Common Bugs to Avoid

These are the most frequently reported issues across mobile emulators, with prevention strategies:

| Issue | Root Cause | Prevention Strategy |
|-------|-----------|---------------------|
| **Audio crackling** | Buffer underrun, sync drift | Lock audio as timing master. Use platform-native low-latency APIs (AAudio/CoreAudio). Proper buffer sizing with back-pressure. |
| **Input lag** | Excessive buffering, wrong sync strategy | Minimize audio buffer size. Optional runahead (predict next frame). Never block input on audio. |
| **Save corruption** | Version mismatch, unflushed writes | Versioned save format with magic header. Atomic writes (write to temp, then rename). Migration tool for format upgrades. |
| **Frame stutter** | VSync mismatch, 120Hz displays | Detect display refresh rate. Adaptive frame pacing. Never assume 60Hz. |
| **Battery drain** | Busy-wait loops, no sleep | Proper vsync sleeping. Low-power audio backend. Pause emulation when backgrounded. |
| **Controller mapping** | Vendor-specific HID quirks | Use Android's built-in gamepad abstraction. Test on Xbox, PS4, PS5, Switch Pro, 8BitDo. |
| **Crash on ROM load** | Bad ROM, wrong format, missing BIOS | Graceful error handling. Validate ROM header checksum. HLE BIOS fallback. |

## Project References

These projects are excellent references for architecture, accuracy, and implementation:

| Project | What to Study |
|---------|--------------|
| [mGBA](https://github.com/mgba-emu/mgba) | Gold standard for accuracy, cycle-accurate timing, cross-platform architecture |
| [SkyEmu](https://github.com/skylersaleh/SkyEmu) | Per-pixel PPU implementation, passes AGS Aging Test |
| [rustboyadvance-ng](https://github.com/rustboyadvance-ng) | Rust-based GBA emulator, proves Rust works for this |
| [NanoBoyAdvance](https://github.com/nba-emu/NanoBoyAdvance) | Cycle-accurate design, excellent test suite |
| [Delta](https://github.com/rileytestut/Delta) | iOS UX done right, multi-core architecture, App Store success |
| [Pizza Boy GBA](https://github.com/libretro/Pizza-Boy-GBA-A) | Lightweight Android-native approach |
| [GBATEK](https://problemkaputt.de/gbatek.htm) | Primary GBA hardware reference (read this) |
| [TONC](https://www.coranac.com/tonc/text/toc.htm) | GBA programming tutorial (understand the hardware) |
| [ARM7TDMI TRM](https://developer.arm.com/documentation/ddi0029/) | CPU technical reference manual |

## Legal

- **License**: MIT - use it however you want
- **ROMs**: NOT included. You must provide legally obtained ROM files (your own cartridge dumps, or homebrew).
- **BIOS**: High-level emulation (HLE) is included. Original BIOS file is optional for improved accuracy on select titles.
- **Nintendo**: This project is not affiliated with, endorsed by, or connected to Nintendo Co., Ltd. in any way.
- **Game Boy Advance**: Is a trademark of Nintendo Co., Ltd.

### Is this legal?

Yes. Emulators themselves are legal (established by Sony v. Connectix, Sega v. Accolade). The legal risk comes from distributing copyrighted ROMs, not from the emulator software. Many open-source GBA emulators exist on GitHub (mGBA, VBA-M, etc.) under various licenses.

## Installation & Setup

### Option 1: Docker (Recommended)

Docker contains everything you need - no manual tool installation required.

```bash
# Clone the repo
git clone https://github.com/your-username/GeeBeeAyy.git
cd GeeBeeAyy

# Build the Docker image (one-time, ~5.5GB)
./build.sh docker-build

# Open interactive development shell
./build.sh docker-shell
```

Inside the container:
```bash
cd /workspace/core
cargo check              # Verify code compiles
cargo build --release    # Build optimized core
```

### Option 2: Local Installation

If you prefer native tools on your machine:

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Install repo-scoped tools (ADB for device testing)
./install-tools.sh install
```

### Prerequisites Summary

| Tool | Docker | Local | Purpose |
|------|--------|-------|---------|
| Rust 1.97+ | Included | `rustup` | Core emulation |
| Java 17 | Included | `openjdk-17-jdk` | Android builds |
| Android SDK 34 | Included | Android Studio | Android builds |
| Android NDK 27 | Included | Android Studio | Native ARM code |
| cargo-ndk | Included | `cargo install cargo-ndk` | Rust -> Android |
| ADB | N/A | `.tools/adb/adb` | Phone testing |

## Building

### Build Commands

```bash
# Using Docker (recommended)
./build.sh docker-core       # Build Rust core for Android
./build.sh docker-android    # Build full Android APK
./build.sh docker-check      # Run clippy + format checks

# Using local tools
./build.sh core              # Build Rust core
./build.sh android           # Build debug APK
./build.sh android-release   # Build release APK
./build.sh check             # Run checks
./build.sh test              # Run tests
./build.sh clean             # Clean build artifacts
```

### Build Outputs

```
core/target/release/
├── libgeebeeayy_core.a         # Static library (for linking)
└── libgeebeeayy_core.so        # Shared library (for Android)

android/app/build/outputs/apk/
├── debug/app-debug.apk      # Debug build
└── release/app-release.apk  # Release build (unsigned)
```

## Android Phone Testing

### 1. Enable Developer Options

On your Android phone:
1. Go to **Settings > About Phone**
2. Tap **Build Number** 7 times
3. Go back to **Settings > System > Developer Options**
4. Enable **USB Debugging**

### 2. Connect via ADB

```bash
# Using local ADB
./install-tools.sh adb       # Install ADB if not present
.export PATH="$PWD/.tools/adb:$PATH"

# Verify connection
adb devices

# Should show something like:
# List of devices attached
# XXXXXXXX    device
```

### 3. Install APK

```bash
# Build first
./build.sh docker-android    # or ./build.sh android

# Install to phone
adb install android/app/build/outputs/apk/debug/app-debug.apk

# Launch
adb shell am start -n com.geebeeayya/.MainActivity
```

### 4. View Logs

```bash
# Filter for GeeBeeAyy logs
adb logcat | grep -i "geebeeayy"

# Save full log
adb logcat > geebeeayy-debug.log
```

## Configuration

### Rust Core Configuration

Edit `core/Cargo.toml` to adjust build profiles:

```toml
[profile.release]
opt-level = 3          # Maximum optimization
lto = true             # Link-time optimization (smaller binary)
codegen-units = 1      # Better optimization, slower build
panic = "abort"        # Smaller binary, no unwinding

[profile.dev]
opt-level = 1          # Some optimization for debug builds
```

### Android Configuration

The Android app configuration is in `android/app/build.gradle.kts`:

```kotlin
android {
    defaultConfig {
        applicationId = "com.geebeeayya"
        minSdk = 26          // Android 8.0+
        targetSdk = 34       // Android 14
        versionCode = 1
        versionName = "0.1.0"

        ndk {
            abiFilters += listOf("arm64-v8a", "armeabi-v7a")
        }
    }
}
```

### Docker Configuration

Edit `docker-compose.yml` to customize the build environment:

```yaml
services:
  dev:
    volumes:
      - .:/workspace           # Project files
      - cargo-cache:/usr/local/cargo/registry  # Persist Rust downloads
      - gradle-cache:/root/.gradle             # Persist Gradle downloads
    environment:
      - CARGO_INCREMENTAL=1    # Faster rebuilds
```

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `ANDROID_HOME` | `/opt/android-sdk` | Android SDK path |
| `ANDROID_NDK_HOME` | `/opt/android-sdk/ndk/27.0.12077973` | NDK path |
| `JAVA_HOME` | `/usr/lib/jvm/java-17-openjdk-amd64` | Java path |
| `CARGO_HOME` | `/usr/local/cargo` | Rust/Cargo path |
| `CARGO_INCREMENTAL` | `1` | Enable incremental builds |

## Contributing

Contributions are welcome! Please read the contributing guidelines before submitting a PR.

## Roadmap

- [ ] Phase 1: Core emulation (CPU, PPU, APU, Memory)
- [ ] Phase 2: Android frontend with touch controls
- [ ] Phase 3: iOS frontend with SwiftUI
- [ ] Phase 4: Advanced features (JIT, link cable, cheats)
- [ ] Phase 5: Performance optimization + community features

## License

MIT License - see [LICENSE](LICENSE) for details.

---

<p align="center">
  Made with love and pixel art
  <br>
  <img src="docs/bee-icon.png" width="32" alt="Bee">
</p>
