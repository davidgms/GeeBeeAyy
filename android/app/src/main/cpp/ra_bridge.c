/* The door between Kotlin and rcheevos.
 *
 * rcheevos is MIT-licensed C from RetroAchievements. It evaluates achievement
 * logic over emulated memory and builds the server requests; **it performs no
 * networking itself**, which is the shape of this file: every HTTP call goes
 * back out to Kotlin, and the answer comes back in. See docs/achievements.md.
 *
 * Three things cross this boundary:
 *
 *   memory   C asks the Rust core for bytes, through `geebeeayy_peek_memory`
 *   network  C asks Kotlin to fetch a URL; Kotlin answers later, by id
 *   events   C tells Kotlin an achievement was earned
 */

#include <dlfcn.h>
#include <pthread.h>
#include <jni.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "rc_client.h"
#include "rc_hash.h"
#include "rc_version.h"

/* ------------------------------------------------------------------ state */

static JavaVM* g_vm = NULL;
static jclass g_engine = NULL; /* global ref to RaEngine */
static rc_client_t* g_client = NULL;

/* The emulator handle, as Kotlin knows it. Not owned here.
 *
 * No atomic needed: the only writer (nativeSetEmulator) and every reader
 * (ra_read_memory, reached from doFrame, load, progress and server-response
 * delivery) run under RaEngine.coreLock, which is also what orders them
 * against the emulation thread. volatile is only a free belt. */
static void* volatile g_gba = NULL;

/* Resolved from the already-loaded core rather than linked against it: the
 * core's .so is built by cargo-ndk outside Gradle, so linking would make this
 * build depend on a file Gradle does not produce. Both libraries live in the
 * same process, so the symbol is simply there once GbaEngine has loaded. */
typedef size_t (*peek_fn)(void*, uint32_t, uint8_t*, size_t);
static peek_fn g_peek = NULL;

static jmethodID g_on_request = NULL;
static jmethodID g_on_event = NULL;
static jmethodID g_on_login = NULL;
static jmethodID g_on_game = NULL;

/* ---------------------------------------------------------------- strings
 *
 * NewStringUTF takes *modified* UTF-8: a 4-byte sequence (an emoji in a
 * game title) or an invalid byte is undefined behaviour and aborts under
 * CheckJNI. Server text is neither guaranteed valid nor BMP-only, so it is
 * decoded here and handed over as UTF-16. Invalid or truncated input becomes
 * U+FFFD; NULL becomes "". */
static jstring ra_new_string(JNIEnv* env, const char* s) {
  const unsigned char* p = (const unsigned char*)(s ? s : "");
  size_t cap = strlen((const char*)p) + 1;
  jchar* out = (jchar*)malloc(cap * sizeof(jchar));
  size_t n = 0;
  jstring result;

  if (out == NULL) {
    return NULL;
  }
  while (*p) {
    uint32_t cp;
    int extra;
    int k;
    unsigned char c = *p;
    if (c < 0x80) { cp = c; extra = 0; }
    else if (c >= 0xC2 && c <= 0xDF) { cp = c & 0x1Fu; extra = 1; }
    else if (c >= 0xE0 && c <= 0xEF) { cp = c & 0x0Fu; extra = 2; }
    else if (c >= 0xF0 && c <= 0xF4) { cp = c & 0x07u; extra = 3; }
    else { out[n++] = 0xFFFD; p++; continue; }
    p++;
    for (k = 0; k < extra; k++) {
      if ((p[0] & 0xC0) != 0x80) { cp = 0xFFFD; break; }
      cp = (cp << 6) | (p[0] & 0x3Fu);
      p++;
    }
    /* Overlong forms, surrogates and out-of-range values are invalid. */
    if (cp != 0xFFFD &&
        ((extra == 2 && cp < 0x800u) || (extra == 3 && cp < 0x10000u) ||
         (cp >= 0xD800u && cp <= 0xDFFFu) || cp > 0x10FFFFu)) {
      cp = 0xFFFD;
    }
    if (cp >= 0x10000u) {
      cp -= 0x10000u;
      out[n++] = (jchar)(0xD800u + (cp >> 10));
      out[n++] = (jchar)(0xDC00u + (cp & 0x3FFu));
    } else {
      out[n++] = (jchar)cp;
    }
  }
  result = (*env)->NewString(env, out, (jsize)n);
  free(out);
  return result;
}

/* A Kotlin callback that threw must not leave the exception pending: the next
 * JNI call from this thread would abort the process. */
static void ra_clear_exception(JNIEnv* env) {
  if ((*env)->ExceptionCheck(env)) {
    (*env)->ExceptionClear(env);
  }
}

/* ------------------------------------------------------- memory, flat->real
 *
 * rcheevos addresses memory as one flat block per console, and an achievement
 * is authored against that. The GBA map is three regions (rcheevos'
 * consoleinfo.c, which cites GBATEK):
 *
 *   flat 0x000000-0x007FFF  ->  0x03000000  internal work RAM, 32 KB
 *   flat 0x008000-0x047FFF  ->  0x02000000  external work RAM, 256 KB
 *   flat 0x048000-0x057FFF  ->  0x0E000000  save memory, 64 KB
 *
 * Getting this wrong does not fail loudly - every achievement simply reads
 * the wrong bytes and never triggers.
 */
static int ra_translate(uint32_t flat, uint32_t* real) {
  if (flat < 0x008000u) {
    *real = 0x03000000u + flat;
  } else if (flat < 0x048000u) {
    *real = 0x02000000u + (flat - 0x008000u);
  } else if (flat < 0x058000u) {
    *real = 0x0E000000u + (flat - 0x048000u);
  } else {
    return 0;
  }
  return 1;
}

static uint32_t RC_CCONV ra_read_memory(uint32_t address, uint8_t* buffer,
                                        uint32_t num_bytes, rc_client_t* client) {
  uint32_t i;
  (void)client;

  /* Resolved on first use, not at init: the core's library is loaded when a
   * game is opened, and the client exists before that - signing in does not
   * need an emulator. */
  if (g_peek == NULL) {
    g_peek = (peek_fn)dlsym(RTLD_DEFAULT, "geebeeayy_peek_memory");
  }
  if (g_peek == NULL || g_gba == NULL) {
    return 0;
  }
  /* A byte at a time, because a read may straddle two regions that are next
   * to each other in the flat map and nowhere near each other in the real
   * one. Reads here are a handful of bytes, not a scan. */
  for (i = 0; i < num_bytes; i++) {
    uint32_t real;
    if (!ra_translate(address + i, &real)) {
      return i;
    }
    if (g_peek(g_gba, real, buffer + i, 1) != 1) {
      return i;
    }
  }
  return num_bytes;
}

/* ---------------------------------------------------------------- network
 *
 * rcheevos hands over a URL and a callback to invoke when the answer arrives.
 * The callback is a C pointer that cannot cross into Kotlin, so it is parked
 * in a slot and the slot's id makes the round trip instead.
 */
#define RA_MAX_PENDING 16

typedef struct {
  int in_use;
  rc_client_server_callback_t callback;
  void* callback_data;
} ra_pending_t;

static ra_pending_t g_pending[RA_MAX_PENDING];

/* The slot table is touched from more than one thread: a request is parked by
 * whichever thread called into rcheevos (the UI thread for a sign-in, the
 * emulation thread for everything a frame triggers), and an answer is
 * collected on the emulation thread. rcheevos guards its own state with its
 * own mutex; this table is ours, so it needs ours. */
static pthread_mutex_t g_pending_lock = PTHREAD_MUTEX_INITIALIZER;

static void ra_fail_request(rc_client_server_callback_t callback, void* callback_data) {
  /* Answering with a client error is the documented way to fail a request,
   * and it is the only way: leaving one unanswered makes rcheevos wait for a
   * reply that is never coming, which hangs the session. */
  rc_api_server_response_t response;
  memset(&response, 0, sizeof(response));
  response.http_status_code = RC_API_SERVER_RESPONSE_CLIENT_ERROR;
  callback(&response, callback_data);
}

static void RC_CCONV ra_server_call(const rc_api_request_t* request,
                                    rc_client_server_callback_t callback,
                                    void* callback_data, rc_client_t* client) {
  JNIEnv* env = NULL;
  int slot = -1;
  int i;
  jstring url;
  jstring post;
  (void)client;

  if (g_vm == NULL) {
    ra_fail_request(callback, callback_data);
    return;
  }

  pthread_mutex_lock(&g_pending_lock);
  for (i = 0; i < RA_MAX_PENDING; i++) {
    if (!g_pending[i].in_use) {
      slot = i;
      g_pending[i].in_use = 1;
      g_pending[i].callback = callback;
      g_pending[i].callback_data = callback_data;
      break;
    }
  }
  pthread_mutex_unlock(&g_pending_lock);

  if (slot < 0) {
    ra_fail_request(callback, callback_data);
    return;
  }

  (*g_vm)->GetEnv(g_vm, (void**)&env, JNI_VERSION_1_6);
  if (env == NULL) {
    /* Freed *and answered*. It used to be only freed, which left rcheevos
     * waiting on a request nobody would ever send. */
    pthread_mutex_lock(&g_pending_lock);
    g_pending[slot].in_use = 0;
    pthread_mutex_unlock(&g_pending_lock);
    ra_fail_request(callback, callback_data);
    return;
  }
  url = ra_new_string(env, request->url);
  post = request->post_data ? ra_new_string(env, request->post_data) : NULL;
  if (url == NULL || (request->post_data && post == NULL)) {
    ra_clear_exception(env);
    if (url) (*env)->DeleteLocalRef(env, url);
    if (post) (*env)->DeleteLocalRef(env, post);
    pthread_mutex_lock(&g_pending_lock);
    g_pending[slot].in_use = 0;
    pthread_mutex_unlock(&g_pending_lock);
    ra_fail_request(callback, callback_data);
    return;
  }
  (*env)->CallStaticVoidMethod(env, g_engine, g_on_request, (jint)slot, url, post);
  if ((*env)->ExceptionCheck(env)) {
    /* Kotlin never took the request, so no answer is coming: answer here. */
    (*env)->ExceptionClear(env);
    pthread_mutex_lock(&g_pending_lock);
    g_pending[slot].in_use = 0;
    pthread_mutex_unlock(&g_pending_lock);
    ra_fail_request(callback, callback_data);
  }
  (*env)->DeleteLocalRef(env, url);
  if (post) {
    (*env)->DeleteLocalRef(env, post);
  }
}

JNIEXPORT void JNICALL
Java_com_geebeeayy_app_engine_RaEngine_nativeServerResponse(
    JNIEnv* env, jclass clazz, jint slot, jint status, jbyteArray body) {
  rc_api_server_response_t response;
  jbyte* bytes = NULL;
  jsize length = 0;
  rc_client_server_callback_t callback;
  void* callback_data;
  (void)clazz;

  if (slot < 0 || slot >= RA_MAX_PENDING) {
    return;
  }
  pthread_mutex_lock(&g_pending_lock);
  if (!g_pending[slot].in_use) {
    pthread_mutex_unlock(&g_pending_lock);
    return;
  }
  callback = g_pending[slot].callback;
  callback_data = g_pending[slot].callback_data;
  g_pending[slot].in_use = 0;
  pthread_mutex_unlock(&g_pending_lock);

  memset(&response, 0, sizeof(response));
  response.http_status_code = status;
  if (body != NULL) {
    length = (*env)->GetArrayLength(env, body);
    bytes = (*env)->GetByteArrayElements(env, body, NULL);
    if (bytes == NULL) {
      /* Out of memory pinning the array. Handing rcheevos a NULL body with a
       * non-zero length would have it read through a null pointer. */
      ra_fail_request(callback, callback_data);
      return;
    }
    response.body = (const char*)bytes;
    response.body_length = (size_t)length;
  }
  callback(&response, callback_data);
  if (bytes != NULL) {
    (*env)->ReleaseByteArrayElements(env, body, bytes, JNI_ABORT);
  }
}

/* ----------------------------------------------------------------- events */

static void RC_CCONV ra_event_handler(const rc_client_event_t* event, rc_client_t* client) {
  JNIEnv* env = NULL;
  jstring title = NULL;
  jstring description = NULL;
  jint id = 0;
  jint points = 0;
  (void)client;

  if (g_vm == NULL) {
    return;
  }
  (*g_vm)->GetEnv(g_vm, (void**)&env, JNI_VERSION_1_6);
  if (env == NULL) {
    return;
  }
  if (event->achievement != NULL) {
    id = (jint)event->achievement->id;
    points = (jint)event->achievement->points;
    title = ra_new_string(env, event->achievement->title);
    description = ra_new_string(env, event->achievement->description);
  } else if (event->leaderboard_tracker != NULL) {
    /* The live readout of an attempt in progress - a lap timer, a running
     * score. `display` is already formatted for showing, which is the whole
     * reason the tracker exists as its own event rather than a raw value. */
    id = (jint)event->leaderboard_tracker->id;
    title = ra_new_string(env, event->leaderboard_tracker->display);
  } else if (event->leaderboard != NULL) {
    id = (jint)event->leaderboard->id;
    title = ra_new_string(env, event->leaderboard->title);
    description = ra_new_string(env, event->leaderboard->description);
  }
  (*env)->CallStaticVoidMethod(env, g_engine, g_on_event, (jint)event->type, id, title,
                               description, points);
  ra_clear_exception(env);
  if (title) {
    (*env)->DeleteLocalRef(env, title);
  }
  if (description) {
    (*env)->DeleteLocalRef(env, description);
  }
}

/* --------------------------------------------------------- async callbacks */

static void RC_CCONV ra_login_callback(int result, const char* error_message,
                                       rc_client_t* client, void* userdata) {
  JNIEnv* env = NULL;
  jstring message;
  (void)client;
  (void)userdata;

  if (g_vm == NULL) {
    return;
  }
  (*g_vm)->GetEnv(g_vm, (void**)&env, JNI_VERSION_1_6);
  if (env == NULL) {
    return;
  }
  message = ra_new_string(env, error_message);
  (*env)->CallStaticVoidMethod(env, g_engine, g_on_login, (jint)result, message);
  ra_clear_exception(env);
  if (message) {
    (*env)->DeleteLocalRef(env, message);
  }
}

static void RC_CCONV ra_load_callback(int result, const char* error_message,
                                      rc_client_t* client, void* userdata) {
  JNIEnv* env = NULL;
  jstring message;
  const rc_client_game_t* game;
  jstring name = NULL;
  jint achievements = 0;
  (void)userdata;

  if (g_vm == NULL) {
    return;
  }
  (*g_vm)->GetEnv(g_vm, (void**)&env, JNI_VERSION_1_6);
  if (env == NULL) {
    return;
  }
  message = ra_new_string(env, error_message);
  game = rc_client_get_game_info(client);
  if (game != NULL) {
    name = ra_new_string(env, game->title);
    {
      rc_client_achievement_list_t* list = rc_client_create_achievement_list(
          client, RC_CLIENT_ACHIEVEMENT_CATEGORY_PROMOTED_AND_UNPROMOTED,
          RC_CLIENT_ACHIEVEMENT_LIST_GROUPING_LOCK_STATE);
      if (list != NULL) {
        uint32_t b;
        for (b = 0; b < list->num_buckets; b++) {
          achievements += (jint)list->buckets[b].num_achievements;
        }
        rc_client_destroy_achievement_list(list);
      }
    }
  }
  (*env)->CallStaticVoidMethod(env, g_engine, g_on_game, (jint)result, message, name,
                               achievements);
  ra_clear_exception(env);
  if (message) {
    (*env)->DeleteLocalRef(env, message);
  }
  if (name) {
    (*env)->DeleteLocalRef(env, name);
  }
}

/* ----------------------------------------------------------------- exports */

JNIEXPORT jboolean JNICALL
Java_com_geebeeayy_app_engine_RaEngine_nativeInit(JNIEnv* env, jclass clazz) {
  jclass local;

  if (g_client != NULL) {
    return JNI_TRUE;
  }
  (*env)->GetJavaVM(env, &g_vm);
  local = (*env)->FindClass(env, "com/geebeeayy/app/engine/RaEngine");
  if (local == NULL) {
    return JNI_FALSE;
  }
  g_engine = (jclass)(*env)->NewGlobalRef(env, local);
  (*env)->DeleteLocalRef(env, local);
  if (g_engine == NULL) {
    return JNI_FALSE;
  }
  g_on_request = (*env)->GetStaticMethodID(env, g_engine, "onServerRequest",
                                           "(ILjava/lang/String;Ljava/lang/String;)V");
  g_on_event = (*env)->GetStaticMethodID(
      env, g_engine, "onEvent", "(IILjava/lang/String;Ljava/lang/String;I)V");
  g_on_login =
      (*env)->GetStaticMethodID(env, g_engine, "onLogin", "(ILjava/lang/String;)V");
  g_on_game = (*env)->GetStaticMethodID(
      env, g_engine, "onGameLoaded", "(ILjava/lang/String;Ljava/lang/String;I)V");
  if (!g_on_request || !g_on_event || !g_on_login || !g_on_game) {
    ra_clear_exception(env);
    return JNI_FALSE;
  }

  memset(g_pending, 0, sizeof(g_pending));
  g_client = rc_client_create(ra_read_memory, ra_server_call);
  if (g_client == NULL) {
    return JNI_FALSE;
  }
  rc_client_set_event_handler(g_client, ra_event_handler);
  /* Hardcore stays off. It requires approval and six months of public
   * availability, and it forbids save states, rewind and frame advance -
   * all of which this app has. See docs/achievements.md. */
  rc_client_set_hardcore_enabled(g_client, 0);
  (void)clazz;
  /* True once the client exists. It does **not** wait for an emulator: a
   * player signs in from Settings with no game open, and tying this to the
   * core being loaded is what silently swallowed the first sign-in attempt. */
  return JNI_TRUE;
}

JNIEXPORT void JNICALL
Java_com_geebeeayy_app_engine_RaEngine_nativeSetEmulator(JNIEnv* env, jclass clazz,
                                                         jlong handle) {
  (void)env;
  (void)clazz;
  g_gba = (void*)(intptr_t)handle;
}

JNIEXPORT void JNICALL
Java_com_geebeeayy_app_engine_RaEngine_nativeLogin(JNIEnv* env, jclass clazz,
                                                   jstring username, jstring password) {
  const char* user;
  const char* pass;
  (void)clazz;

  if (g_client == NULL) {
    return;
  }
  if (username == NULL || password == NULL) {
    return;
  }
  user = (*env)->GetStringUTFChars(env, username, NULL);
  if (user == NULL) {
    return;
  }
  pass = (*env)->GetStringUTFChars(env, password, NULL);
  if (pass == NULL) {
    (*env)->ReleaseStringUTFChars(env, username, user);
    return;
  }
  rc_client_begin_login_with_password(g_client, user, pass, ra_login_callback, NULL);
  (*env)->ReleaseStringUTFChars(env, username, user);
  (*env)->ReleaseStringUTFChars(env, password, pass);
}

JNIEXPORT void JNICALL
Java_com_geebeeayy_app_engine_RaEngine_nativeLoginWithToken(JNIEnv* env, jclass clazz,
                                                            jstring username, jstring token) {
  const char* user;
  const char* tok;
  (void)clazz;

  if (g_client == NULL) {
    return;
  }
  if (username == NULL || token == NULL) {
    return;
  }
  user = (*env)->GetStringUTFChars(env, username, NULL);
  if (user == NULL) {
    return;
  }
  tok = (*env)->GetStringUTFChars(env, token, NULL);
  if (tok == NULL) {
    (*env)->ReleaseStringUTFChars(env, username, user);
    return;
  }
  rc_client_begin_login_with_token(g_client, user, tok, ra_login_callback, NULL);
  (*env)->ReleaseStringUTFChars(env, username, user);
  (*env)->ReleaseStringUTFChars(env, token, tok);
}

/* The token, so the password is never stored. Null until a login succeeds. */
JNIEXPORT jstring JNICALL
Java_com_geebeeayy_app_engine_RaEngine_nativeToken(JNIEnv* env, jclass clazz) {
  const rc_client_user_t* user;
  (void)clazz;

  if (g_client == NULL) {
    return NULL;
  }
  user = rc_client_get_user_info(g_client);
  if (user == NULL || user->token == NULL) {
    return NULL;
  }
  return ra_new_string(env, user->token);
}

JNIEXPORT jstring JNICALL
Java_com_geebeeayy_app_engine_RaEngine_nativeUsername(JNIEnv* env, jclass clazz) {
  const rc_client_user_t* user;
  (void)clazz;

  if (g_client == NULL) {
    return NULL;
  }
  user = rc_client_get_user_info(g_client);
  if (user == NULL || user->display_name == NULL) {
    return NULL;
  }
  return ra_new_string(env, user->display_name);
}

JNIEXPORT void JNICALL
Java_com_geebeeayy_app_engine_RaEngine_nativeLogout(JNIEnv* env, jclass clazz) {
  (void)env;
  (void)clazz;
  if (g_client != NULL) {
    rc_client_logout(g_client);
  }
}

JNIEXPORT void JNICALL
Java_com_geebeeayy_app_engine_RaEngine_nativeLoadGame(JNIEnv* env, jclass clazz,
                                                      jstring hash) {
  const char* value;
  (void)clazz;

  if (g_client == NULL) {
    return;
  }
  if (hash == NULL) {
    return;
  }
  value = (*env)->GetStringUTFChars(env, hash, NULL);
  if (value == NULL) {
    return;
  }
  rc_client_begin_load_game(g_client, value, ra_load_callback, NULL);
  (*env)->ReleaseStringUTFChars(env, hash, value);
}

JNIEXPORT void JNICALL
Java_com_geebeeayy_app_engine_RaEngine_nativeUnloadGame(JNIEnv* env, jclass clazz) {
  (void)env;
  (void)clazz;
  if (g_client != NULL) {
    rc_client_unload_game(g_client);
  }
}

/* Called once per emulated frame, from the emulation thread. */
JNIEXPORT void JNICALL
Java_com_geebeeayy_app_engine_RaEngine_nativeDoFrame(JNIEnv* env, jclass clazz) {
  (void)env;
  (void)clazz;
  if (g_client != NULL) {
    rc_client_do_frame(g_client);
  }
}

/* Called while paused, so the session stays alive without evaluating. */
JNIEXPORT void JNICALL
Java_com_geebeeayy_app_engine_RaEngine_nativeIdle(JNIEnv* env, jclass clazz) {
  (void)env;
  (void)clazz;
  if (g_client != NULL) {
    rc_client_idle(g_client);
  }
}

/* Flat strings, 5 per leaderboard: id, title, description, state, current
 * value. No packing, so nothing is truncated and a tab in a title is just a
 * character. */
JNIEXPORT jobjectArray JNICALL
Java_com_geebeeayy_app_engine_RaEngine_nativeLeaderboards(JNIEnv* env, jclass clazz) {
  rc_client_leaderboard_list_t* list;
  jobjectArray out;
  jclass string_class;
  uint32_t total = 0;
  uint32_t b;
  uint32_t i;
  jsize index = 0;
  (void)clazz;

  if (g_client == NULL) {
    return NULL;
  }
  list = rc_client_create_leaderboard_list(g_client,
                                           RC_CLIENT_LEADERBOARD_LIST_GROUPING_NONE);
  if (list == NULL) {
    return NULL;
  }
  for (b = 0; b < list->num_buckets; b++) {
    total += list->buckets[b].num_leaderboards;
  }

  string_class = (*env)->FindClass(env, "java/lang/String");
  if (string_class == NULL) {
    rc_client_destroy_leaderboard_list(list);
    return NULL;
  }
  out = (*env)->NewObjectArray(env, (jsize)(total * 5), string_class, NULL);
  (*env)->DeleteLocalRef(env, string_class);
  if (out == NULL) {
    rc_client_destroy_leaderboard_list(list);
    return NULL;
  }
  for (b = 0; b < list->num_buckets; b++) {
    for (i = 0; i < list->buckets[b].num_leaderboards; i++) {
      const rc_client_leaderboard_t* l = list->buckets[b].leaderboards[i];
      char id[16];
      char state[16];
      const char* fields[5];
      int f;
      snprintf(id, sizeof(id), "%u", l->id);
      snprintf(state, sizeof(state), "%u", (unsigned)l->state);
      fields[0] = id;
      fields[1] = l->title;
      fields[2] = l->description;
      fields[3] = state;
      fields[4] = l->tracker_value;
      for (f = 0; f < 5; f++) {
        jstring value = ra_new_string(env, fields[f]);
        if (value == NULL) {
          ra_clear_exception(env);
          (*env)->DeleteLocalRef(env, out);
          rc_client_destroy_leaderboard_list(list);
          return NULL;
        }
        (*env)->SetObjectArrayElement(env, out, index++, value);
        (*env)->DeleteLocalRef(env, value);
      }
    }
  }
  rc_client_destroy_leaderboard_list(list);
  return out;
}

/* ------------------------------------------------- save states
 *
 * An achievement is a **transition**: a condition that was false becoming
 * true while the runtime is watching. That state lives inside rcheevos, not
 * in the emulator, so a save state that does not carry it restores the game
 * to one moment and leaves the achievement runtime in another. What follows
 * is rcheevos' own answer to that, stored beside our state file.
 */
JNIEXPORT jbyteArray JNICALL
Java_com_geebeeayy_app_engine_RaEngine_nativeSerializeProgress(JNIEnv* env, jclass clazz) {
  int size;
  jbyteArray out;
  jbyte* bytes;
  (void)clazz;

  if (g_client == NULL) {
    return NULL;
  }
  size = rc_client_progress_size(g_client);
  if (size <= 0) {
    return NULL;
  }
  out = (*env)->NewByteArray(env, (jsize)size);
  if (out == NULL) {
    return NULL;
  }
  bytes = (*env)->GetByteArrayElements(env, out, NULL);
  if (bytes == NULL) {
    return NULL;
  }
  if (rc_client_serialize_progress_sized(g_client, (uint8_t*)bytes, (size_t)size) != RC_OK) {
    (*env)->ReleaseByteArrayElements(env, out, bytes, JNI_ABORT);
    return NULL;
  }
  (*env)->ReleaseByteArrayElements(env, out, bytes, 0);
  return out;
}

JNIEXPORT jboolean JNICALL
Java_com_geebeeayy_app_engine_RaEngine_nativeDeserializeProgress(JNIEnv* env, jclass clazz,
                                                                 jbyteArray data) {
  jbyte* bytes;
  jsize size;
  int result;
  (void)clazz;

  if (g_client == NULL || data == NULL) {
    return JNI_FALSE;
  }
  size = (*env)->GetArrayLength(env, data);
  bytes = (*env)->GetByteArrayElements(env, data, NULL);
  if (bytes == NULL) {
    return JNI_FALSE;
  }
  result = rc_client_deserialize_progress_sized(g_client, (const uint8_t*)bytes, (size_t)size);
  (*env)->ReleaseByteArrayElements(env, data, bytes, JNI_ABORT);
  return result == RC_OK ? JNI_TRUE : JNI_FALSE;
}

/* ----------------------------------------------------------- the list
 *
 * Flat strings, 7 per achievement: id, title, description, points, unlocked,
 * progress, badge url. Flat rather than built as objects because the
 * alternative is a dozen JNI calls per achievement, and a set runs to
 * hundreds; flat rather than tab-packed so a long or tabbed title survives.
 */
JNIEXPORT jobjectArray JNICALL
Java_com_geebeeayy_app_engine_RaEngine_nativeAchievements(JNIEnv* env, jclass clazz) {
  rc_client_achievement_list_t* list;
  jobjectArray out;
  jclass string_class;
  uint32_t total = 0;
  uint32_t b;
  uint32_t i;
  jsize index = 0;
  (void)clazz;

  if (g_client == NULL) {
    return NULL;
  }
  list = rc_client_create_achievement_list(
      g_client, RC_CLIENT_ACHIEVEMENT_CATEGORY_PROMOTED_AND_UNPROMOTED,
      RC_CLIENT_ACHIEVEMENT_LIST_GROUPING_LOCK_STATE);
  if (list == NULL) {
    return NULL;
  }
  for (b = 0; b < list->num_buckets; b++) {
    total += list->buckets[b].num_achievements;
  }

  string_class = (*env)->FindClass(env, "java/lang/String");
  if (string_class == NULL) {
    rc_client_destroy_achievement_list(list);
    return NULL;
  }
  out = (*env)->NewObjectArray(env, (jsize)(total * 7), string_class, NULL);
  (*env)->DeleteLocalRef(env, string_class);
  if (out == NULL) {
    rc_client_destroy_achievement_list(list);
    return NULL;
  }

  for (b = 0; b < list->num_buckets; b++) {
    for (i = 0; i < list->buckets[b].num_achievements; i++) {
      const rc_client_achievement_t* a = list->buckets[b].achievements[i];
      char id[16];
      char points[16];
      char unlocked[4];
      char badge[256];
      const char* fields[7];
      int f;
      /* The badge for the state it is actually in: a locked achievement gets
       * the greyed art, an earned one the full-colour one. Asking for the
       * wrong state would show every row as earned. */
      if (rc_client_achievement_get_image_url(
              a, a->state, badge, sizeof(badge)) != RC_OK) {
        badge[0] = '\0';
      }
      snprintf(id, sizeof(id), "%u", a->id);
      snprintf(points, sizeof(points), "%u", a->points);
      snprintf(unlocked, sizeof(unlocked), "%u", (unsigned)a->unlocked);
      fields[0] = id;
      fields[1] = a->title;
      fields[2] = a->description;
      fields[3] = points;
      fields[4] = unlocked;
      fields[5] = a->measured_progress;
      fields[6] = badge;
      for (f = 0; f < 7; f++) {
        jstring value = ra_new_string(env, fields[f]);
        if (value == NULL) {
          ra_clear_exception(env);
          (*env)->DeleteLocalRef(env, out);
          rc_client_destroy_achievement_list(list);
          return NULL;
        }
        (*env)->SetObjectArrayElement(env, out, index++, value);
        (*env)->DeleteLocalRef(env, value);
      }
    }
  }
  rc_client_destroy_achievement_list(list);
  return out;
}

/* -------------------------------------------------------- identification */

JNIEXPORT jstring JNICALL
Java_com_geebeeayy_app_engine_RaEngine_nativeHashRom(JNIEnv* env, jclass clazz,
                                                     jbyteArray data) {
  jsize size;
  jbyte* bytes;
  char hash[33];
  int ok;
  (void)clazz;

  if (data == NULL) {
    return NULL;
  }
  size = (*env)->GetArrayLength(env, data);
  bytes = (*env)->GetByteArrayElements(env, data, NULL);
  if (bytes == NULL) {
    return NULL;
  }
  /* RC_CONSOLE_GAMEBOY_ADVANCE is 5. Naming the console rather than letting
   * the iterator guess from a path keeps this honest: the app only runs GBA. */
  ok = rc_hash_generate_from_buffer(hash, 5, (const uint8_t*)bytes, (size_t)size);
  (*env)->ReleaseByteArrayElements(env, data, bytes, JNI_ABORT);
  return ok ? (*env)->NewStringUTF(env, hash) : NULL;
}

JNIEXPORT jstring JNICALL
Java_com_geebeeayy_app_engine_RaEngine_nativeVersion(JNIEnv* env, jclass clazz) {
  (void)clazz;
  return (*env)->NewStringUTF(env, RCHEEVOS_VERSION_STRING);
}
