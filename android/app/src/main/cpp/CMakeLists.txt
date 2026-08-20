cmake_minimum_required(VERSION 3.22.1)
project("geebeeayyayy_core")

# Pre-built Rust library (built by cargo build --target aarch64-linux-android)
# Place the .so file in: android/app/src/main/jniLibs/arm64-v8a/libgeebeeayyayy_core.so
#
# Build with: ./build-mobile.sh android-arm64

# Add pre-built library
add_library(geebeeayyayy_core SHARED IMPORTED)
set_target_properties(geebeeayyayy_core PROPERTIES
    IMPORTED_LOCATION ${CMAKE_SOURCE_DIR}/../jniLibs/${ANDROID_ABI}/libgeebeeayyayy_core.so
)

# JNI bridge library
add_library(jni_bridge SHARED
    jni_bridge.c
)

target_link_libraries(jni_bridge
    geebeeayyayy_core
    android
    log
)
