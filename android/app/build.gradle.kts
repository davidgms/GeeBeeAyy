plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

android {
    namespace = "com.gbee.emulator"

    // android.util.Log returns defaults instead of throwing in JVM tests, so
    // RaEngine can run off-device.
    testOptions { unitTests.isReturnDefaultValues = true }
    compileSdk = 36

    defaultConfig {
        applicationId = "com.gbee.emulator"
        minSdk = 26
        targetSdk = 36
        // CI run number keeps it monotonic for Play; local builds get 1. It
        // counts per workflow file: renaming ci.yml restarts it, and a re-run
        // reuses it, so re-upload a commit from a fresh run.
        versionCode = System.getenv("GITHUB_RUN_NUMBER")?.toIntOrNull()
            ?: if (System.getenv("GBA_KEYSTORE") != null) error("A signed release needs GITHUB_RUN_NUMBER for its versionCode") else 1
        versionName = "0.1.0"

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        vectorDrawables {
            useSupportLibrary = true
        }

        ndk {
            abiFilters += listOf("arm64-v8a", "armeabi-v7a")
        }

        // rcheevos, the RetroAchievements library, built from the submodule
        // at src/main/cpp/rcheevos. It is C, which is why it lives here and
        // not in core/ - that crate has two Rust dependencies and no C, and
        // deciding what an achievement means is not emulation.
        externalNativeBuild {
            cmake {
                // Matches the C99 the library is written in, and keeps the
                // Windows-only RAIntegration path out.
                arguments += "-DANDROID_STL=none"
            }
        }
    }

    externalNativeBuild {
        cmake {
            path = file("src/main/cpp/CMakeLists.txt")
            version = "3.22.1"
        }
    }

    // The submodule is not checked out in a plain `git clone`, and a build
    // that fails a thousand lines into CMake is a bad way to find that out.
    tasks.register("checkRcheevosSubmodule") {
        doFirst {
            val marker = file("src/main/cpp/rcheevos/include/rc_hash.h")
            if (!marker.exists()) {
                throw GradleException(
                    "rcheevos is missing. Run: git submodule update --init --recursive"
                )
            }
        }
    }

    // Signing comes from the environment so no secret is in the repo. Without
    // GBA_KEYSTORE the release build stays unsigned and local builds work.
    // Debug is untouched: it keeps the default debug key.
    val releaseKeystore = System.getenv("GBA_KEYSTORE")
    if (releaseKeystore != null) {
        // A missing value would make AGP skip signing with only a warning.
        fun need(name: String) = System.getenv(name)?.takeIf { it.isNotEmpty() } ?: error("$name is not set")
        signingConfigs.create("release") {
            storeFile = file(releaseKeystore)
            storePassword = need("GBA_KEYSTORE_PASSWORD")
            keyAlias = need("GBA_KEY_ALIAS")
            keyPassword = need("GBA_KEY_PASSWORD")
        }
    }

    buildTypes {
        release {
            if (releaseKeystore != null) {
                signingConfig = signingConfigs.getByName("release")
            }
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
        }
        debug {
            isDebuggable = true
            isJniDebuggable = true
            // x86_64 as well, so a debug build installs on an emulator. Real
            // devices are ARM, so release stays ARM-only and ships nothing
            // extra. The .so only exists if cargo-ndk was asked for the
            // target, and a missing ABI is not an error here.
            ndk {
                abiFilters += "x86_64"
            }
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }

    buildFeatures {
        // For the version string in the RetroAchievements User-Agent, which
        // the server uses to recognise the client.
        buildConfig = true
        compose = true
    }

    packaging {
        resources {
            excludes += "/META-INF/{AL2.0,LGPL2.1}"
        }
    }
}

dependencies {
    // Pure-JVM unit tests. `Sai2x` is deliberately written against plain
    // IntArrays so its arithmetic can be checked here rather than only by
    // looking at a phone.
    testImplementation("junit:junit:4.13.2")

    // Compose BOM
    val composeBom = platform("androidx.compose:compose-bom:2024.06.00")
    implementation(composeBom)

    // Core
    implementation("androidx.core:core-ktx:1.13.1")
    implementation("androidx.lifecycle:lifecycle-runtime-ktx:2.8.4")
    implementation("androidx.activity:activity-compose:1.9.1")

    // Compose UI
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-graphics")
    implementation("androidx.compose.ui:ui-tooling-preview")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.material:material-icons-extended")

    // Material (for XML themes)
    implementation("com.google.android.material:material:1.12.0")

    // Navigation
    implementation("androidx.navigation:navigation-compose:2.7.7")

    // Debug
    debugImplementation("androidx.compose.ui:ui-tooling")
    debugImplementation("androidx.compose.ui:ui-test-manifest")
}
