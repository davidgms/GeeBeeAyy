/**
 * JNI Bridge for GeeBeeAyy Rust Core
 *
 * This file provides JNI functions that bridge the Kotlin GbaEngine class
 * to the C FFI functions in libgeebeeayy_core.so
 *
 * Build the Rust core first:
 *   ./build-mobile.sh android-arm64
 *   cp core/target/aarch64-linux-android/release/libgeebeeayy_core.so \
 *      android/app/src/main/jniLibs/arm64-v8a/
 */

#include <jni.h>
#include <string.h>

// C FFI declarations from geebeeayy_core
extern void *geebeeayy_create(void);
extern void geebeeayy_destroy(void *ptr);
extern int geebeeayy_load_rom(void *ptr, const uint8_t *data, size_t len);
extern void geebeeayy_run_frame(void *ptr);
extern void geebeeayy_run_frames(void *ptr, uint32_t count);
extern void geebeeayy_frame_buffer_copy(void *ptr, uint8_t *out);
extern size_t geebeeayy_audio_copy(void *ptr, float *out, size_t max_samples);
extern void *geebeeayy_save_state_create(void *ptr);
extern int geebeeayy_load_state(void *ptr, void *state_ptr);
extern void geebeeayy_save_state_destroy(void *state_ptr);

// ─── JNI Methods ────────────────────────────────────────────────────────────

JNIEXPORT jlong JNICALL
Java_com_geebeeayy_app_engine_GbaEngine_nativeCreate(
    JNIEnv *env, jobject thiz) {
    return (jlong)geebeeayy_create();
}

JNIEXPORT void JNICALL
Java_com_geebeeayy_app_engine_GbaEngine_nativeDestroy(
    JNIEnv *env, jobject thiz, jlong handle) {
    if (handle != 0) {
        geebeeayy_destroy((void *)handle);
    }
}

JNIEXPORT jint JNICALL
Java_com_geebeeayy_app_engine_GbaEngine_nativeLoadRom(
    JNIEnv *env, jobject thiz, jlong handle, jbyteArray data, jint len) {
    jbyte *bytes = (*env)->GetByteArrayElements(env, data, NULL);
    if (bytes == NULL) return -1;

    int result = geebeeayy_load_rom((void *)handle, (const uint8_t *)bytes, (size_t)len);

    (*env)->ReleaseByteArrayElements(env, data, bytes, JNI_ABORT);
    return result;
}

JNIEXPORT void JNICALL
Java_com_geebeeayy_app_engine_GbaEngine_nativeRunFrame(
    JNIEnv *env, jobject thiz, jlong handle) {
    geebeeayy_run_frame((void *)handle);
}

JNIEXPORT void JNICALL
Java_com_geebeeayy_app_engine_GbaEngine_nativeRunFrames(
    JNIEnv *env, jobject thiz, jlong handle, jint count) {
    geebeeayy_run_frames((void *)handle, (uint32_t)count);
}

JNIEXPORT void JNICALL
Java_com_geebeeayy_app_engine_GbaEngine_nativeFrameBufferCopy(
    JNIEnv *env, jobject thiz, jlong handle, jbyteArray out) {
    jsize len = (*env)->GetArrayLength(env, out);
    if (len < 240 * 160 * 3) return;

    // Get direct pointer to write into
    jbyte *bytes = (*env)->GetByteArrayElements(env, out, NULL);
    if (bytes == NULL) return;

    geebeeayy_frame_buffer_copy((void *)handle, (uint8_t *)bytes);

    (*env)->ReleaseByteArrayElements(env, out, bytes, 0);
}

JNIEXPORT jint JNICALL
Java_com_geebeeayy_app_engine_GbaEngine_nativeAudioCopy(
    JNIEnv *env, jobject thiz, jlong handle, jfloatArray out, jint max_samples) {
    jfloat *buf = (*env)->GetFloatArrayElements(env, out, NULL);
    if (buf == NULL) return 0;

    size_t count = geebeeayy_audio_copy((void *)handle, buf, (size_t)max_samples);

    (*env)->ReleaseFloatArrayElements(env, out, buf, 0);
    return (jint)count;
}

JNIEXPORT jlong JNICALL
Java_com_geebeeayy_app_engine_GbaEngine_nativeSaveStateCreate(
    JNIEnv *env, jobject thiz, jlong handle) {
    return (jlong)geebeeayy_save_state_create((void *)handle);
}

JNIEXPORT jint JNICALL
Java_com_geebeeayy_app_engine_GbaEngine_nativeLoadState(
    JNIEnv *env, jobject thiz, jlong handle, jlong stateHandle) {
    return geebeeayy_load_state((void *)handle, (void *)stateHandle);
}

JNIEXPORT void JNICALL
Java_com_geebeeayy_app_engine_GbaEngine_nativeSaveStateDestroy(
    JNIEnv *env, jobject thiz, jlong stateHandle) {
    if (stateHandle != 0) {
        geebeeayy_save_state_destroy((void *)stateHandle);
    }
}
