/* The door between Kotlin and rcheevos.
 *
 * rcheevos is MIT-licensed C from RetroAchievements. It evaluates achievement
 * logic over emulated memory and builds the server requests; it performs no
 * networking itself, which is why this file exists rather than a thicker one:
 * everything that touches the network stays in Kotlin.
 *
 * What is here so far is identification only - working out *which* game a ROM
 * file is, by the same hash the RetroAchievements server keys on. That needs
 * no account, no network and no memory access, so it is the part that can be
 * proven on its own. See docs/achievements.md.
 */

#include <jni.h>
#include <stdlib.h>
#include <string.h>

#include "rc_hash.h"
#include "rc_version.h"

/* Identify a ROM the way the RetroAchievements server does.
 *
 * For Game Boy Advance that is an MD5 of the whole file, unmodified - so a
 * renamed ROM still matches and a trimmed or patched one does not. Returns the
 * 32-character hash, or null when the file cannot be read.
 */
JNIEXPORT jstring JNICALL
Java_com_geebeeayy_app_engine_RaEngine_nativeHashRom(
    JNIEnv* env, jclass clazz, jbyteArray data) {
  (void)clazz;
  jsize size;
  jbyte* bytes;
  char hash[33];
  int ok;

  if (data == NULL) {
    return NULL;
  }
  size = (*env)->GetArrayLength(env, data);
  bytes = (*env)->GetByteArrayElements(env, data, NULL);
  if (bytes == NULL) {
    return NULL;
  }

  /* RC_CONSOLE_GAMEBOY_ADVANCE is 5. Passing the console explicitly rather
   * than letting the iterator guess from a path keeps this honest: this app
   * only ever runs GBA. */
  ok = rc_hash_generate_from_buffer(hash, 5, (const uint8_t*)bytes, (size_t)size);

  (*env)->ReleaseByteArrayElements(env, data, bytes, JNI_ABORT);
  return ok ? (*env)->NewStringUTF(env, hash) : NULL;
}

/* The rcheevos version this build carries, so a bug report can name it. */
JNIEXPORT jstring JNICALL
Java_com_geebeeayy_app_engine_RaEngine_nativeVersion(JNIEnv* env, jclass clazz) {
  (void)clazz;
  return (*env)->NewStringUTF(env, RCHEEVOS_VERSION_STRING);
}
