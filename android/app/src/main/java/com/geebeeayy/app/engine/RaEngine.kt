package com.geebeeayy.app.engine

import android.content.Context
import android.util.Log
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import java.io.ByteArrayOutputStream
import java.net.HttpURLConnection
import java.net.URL

/**
 * The RetroAchievements side of the house.
 *
 * Backed by `rcheevos`, the MIT-licensed C library RetroAchievements publishes
 * for emulator authors, built from the submodule at `src/main/cpp/rcheevos`.
 * It lives here rather than in the Rust core on purpose: the core has two
 * dependencies and no C, and deciding what an achievement means is not
 * emulation. See `docs/achievements.md`.
 *
 * **rcheevos does no networking.** It hands out a URL and a callback; this
 * object makes the request and hands the bytes back. That is why the HTTP
 * lives in Kotlin and not in the bridge.
 *
 * An object rather than a class because the C side holds one client and finds
 * its callbacks by class name. One session, one login, one game at a time.
 */
object RaEngine {

    private const val TAG = "GeeBeeAyy/RA"

    /** `rc_client` event ids, from `rc_client.h`. Only the ones acted on. */
    private const val EVENT_ACHIEVEMENT_TRIGGERED = 1
    private const val EVENT_GAME_COMPLETED = 15

    private val available: Boolean = runCatching {
        System.loadLibrary("geebeeayy_ra")
    }.isSuccess

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    private var store: RaCredentials? = null
    private var started = false

    private val _user = MutableStateFlow<String?>(null)
    /** The signed-in player's display name, or null. */
    val user: StateFlow<String?> = _user.asStateFlow()

    private val _game = MutableStateFlow<RaGame?>(null)
    /** The game this session is tracking, once the server has answered. */
    val game: StateFlow<RaGame?> = _game.asStateFlow()

    private val _unlocks = MutableSharedFlow<RaUnlock>(extraBufferCapacity = 8)
    /** Achievements earned during this session, as they happen. */
    val unlocks = _unlocks.asSharedFlow()

    private val _loginError = MutableSharedFlow<String>(extraBufferCapacity = 4)
    val loginError = _loginError.asSharedFlow()

    /** False when the native library is missing, so callers degrade quietly. */
    fun isAvailable(): Boolean = available

    /**
     * Wake the client up, and sign in again if a token was kept.
     *
     * Safe to call more than once. Returns false when the native side could
     * not start - a missing library, or a core that has not loaded yet.
     */
    fun start(context: Context): Boolean {
        if (!available) return false
        if (!started) {
            started = runCatching { nativeInit() }.getOrDefault(false)
            if (!started) return false
            store = RaCredentials(context)
        }
        val saved = store?.read()
        if (saved != null && _user.value == null) {
            runCatching { nativeLoginWithToken(saved.first, saved.second) }
        }
        return true
    }

    /** Tell the client which emulator to read memory from. */
    fun attachEmulator(handle: Long) {
        if (available && started) runCatching { nativeSetEmulator(handle) }
    }

    /**
     * Sign in with the player's own retroachievements.org account.
     *
     * The password is used once and never stored; what is kept is the token
     * the server returns.
     */
    fun login(username: String, password: String) {
        if (available && started) runCatching { nativeLogin(username, password) }
    }

    fun logout() {
        if (available && started) runCatching { nativeLogout() }
        store?.clear()
        _user.value = null
        _game.value = null
    }

    /** Ask the server what this ROM is, and start a session for it. */
    fun loadGame(hash: String) {
        Log.i(TAG, "load game $hash (started=$started)")
        if (available && started) runCatching { nativeLoadGame(hash) }
    }

    fun unloadGame() {
        if (available && started) runCatching { nativeUnloadGame() }
        _game.value = null
    }

    /** Once per emulated frame, on the emulation thread. */
    fun doFrame() {
        if (available && started) nativeDoFrame()
    }

    /** While paused, so the session stays alive without evaluating. */
    fun idle() {
        if (available && started) runCatching { nativeIdle() }
    }

    /**
     * The achievement runtime's own state, to store beside a save state.
     *
     * An achievement is a **transition** - a condition that was false becoming
     * true while the runtime watches. That memory lives in rcheevos, not in
     * the emulator, so a save state that does not carry it restores the game
     * to one moment and leaves the achievement logic in another.
     */
    fun serializeProgress(): ByteArray? =
        if (available && started) runCatching { nativeSerializeProgress() }.getOrNull() else null

    fun restoreProgress(data: ByteArray): Boolean =
        available && started && runCatching { nativeDeserializeProgress(data) }.getOrDefault(false)

    fun hashRom(romData: ByteArray): String? =
        if (available) runCatching { nativeHashRom(romData) }.getOrNull() else null

    /**
     * Every achievement in the loaded game, locked ones included.
     *
     * Empty when no game is loaded or nobody is signed in. Packed as
     * tab-separated lines on the C side because a set runs to hundreds and
     * building objects across JNI costs a call each.
     */
    fun achievements(): List<RaAchievement> {
        if (!available || !started) return emptyList()
        val lines = runCatching { nativeAchievements() }.getOrNull() ?: return emptyList()
        return lines.mapNotNull { line ->
            val parts = line.split('\t')
            if (parts.size < 6) return@mapNotNull null
            RaAchievement(
                id = parts[0].toIntOrNull() ?: return@mapNotNull null,
                title = parts[1],
                description = parts[2],
                points = parts[3].toIntOrNull() ?: 0,
                unlocked = parts[4] != "0",
                progress = parts[5],
            )
        }
    }

    fun version(): String? =
        if (available) runCatching { nativeVersion() }.getOrNull() else null

    // ---------------------------------------------------------------------
    // Called from C. Names and signatures are matched by string in
    // ra_bridge.c, so renaming one of these silently breaks the bridge.
    // ---------------------------------------------------------------------

    /**
     * rcheevos wants a URL fetched. [slot] is its place in a small table on
     * the C side; handing the id back is what lets a C function pointer
     * survive a round trip through Kotlin.
     */
    @JvmStatic
    private fun onServerRequest(slot: Int, url: String, postData: String?) {
        scope.launch {
            var status = 0
            var body = ByteArray(0)
            runCatching {
                val connection = (URL(url).openConnection() as HttpURLConnection).apply {
                    connectTimeout = 15_000
                    readTimeout = 30_000
                    // The format rcheevos asks for. Nothing here is optional:
                    // the emulator name and version identify the client to
                    // the server, and hardcore support is refused to clients
                    // it cannot recognise.
                    setRequestProperty("User-Agent", userAgent())
                    if (postData != null) {
                        requestMethod = "POST"
                        doOutput = true
                        setRequestProperty(
                            "Content-Type",
                            "application/x-www-form-urlencoded",
                        )
                        outputStream.use { it.write(postData.toByteArray()) }
                    }
                }
                status = connection.responseCode
                val stream = if (status in 200..299) {
                    connection.inputStream
                } else {
                    connection.errorStream
                }
                body = stream?.use { input ->
                    ByteArrayOutputStream().also { input.copyTo(it) }.toByteArray()
                } ?: ByteArray(0)
                connection.disconnect()
            }.onFailure {
                Log.w(TAG, "request failed: ${it.message}")
                // -1 is rcheevos' own "the client could not send this",
                // which it reports as a network error rather than a server
                // one. Leaving the slot unanswered would hang the session.
                status = -1
            }
            runCatching { nativeServerResponse(slot, status, body) }
        }
    }

    @JvmStatic
    private fun onEvent(type: Int, id: Int, title: String?, description: String?, points: Int) {
        when (type) {
            EVENT_ACHIEVEMENT_TRIGGERED ->
                _unlocks.tryEmit(RaUnlock(id, title.orEmpty(), description.orEmpty(), points))
            EVENT_GAME_COMPLETED -> Log.i(TAG, "game completed")
        }
    }

    @JvmStatic
    private fun onLogin(result: Int, errorMessage: String?) {
        if (result == RESULT_OK) {
            Log.i(TAG, "signed in")
            val name = runCatching { nativeUsername() }.getOrNull()
            val token = runCatching { nativeToken() }.getOrNull()
            _user.value = name
            if (name != null && token != null) store?.write(name, token)
        } else {
            // A stored token the server no longer accepts must not be retried
            // on every launch.
            store?.clear()
            _user.value = null
            _loginError.tryEmit(errorMessage?.takeIf { it.isNotBlank() } ?: "Sign-in failed")
        }
    }

    @JvmStatic
    private fun onGameLoaded(result: Int, errorMessage: String?, title: String?, achievements: Int) {
        _game.value = if (result == RESULT_OK && title != null) {
            Log.i(TAG, "session: $title, $achievements achievements")
            RaGame(title, achievements)
        } else {
            // Not an error worth showing: about two thirds of GBA games have
            // no achievement set, and a game with none simply has none.
            Log.i(TAG, "no achievements for this game: ${errorMessage.orEmpty()}")
            null
        }
    }

    /**
     * `EmulatorName/version (OS version)`, the shape RetroAchievements asks
     * for. Hardcore credit is refused to a client the server cannot place,
     * and the format is how it places one.
     */
    private fun userAgent(): String =
        "GeeBeeAyy/v${com.geebeeayy.app.BuildConfig.VERSION_NAME} " +
            "(Android ${android.os.Build.VERSION.RELEASE})"

    /** `RC_OK`. */
    private const val RESULT_OK = 0

    @JvmStatic private external fun nativeInit(): Boolean
    @JvmStatic private external fun nativeSetEmulator(handle: Long)
    @JvmStatic private external fun nativeLogin(username: String, password: String)
    @JvmStatic private external fun nativeLoginWithToken(username: String, token: String)
    @JvmStatic private external fun nativeLogout()
    @JvmStatic private external fun nativeUsername(): String?
    @JvmStatic private external fun nativeToken(): String?
    @JvmStatic private external fun nativeLoadGame(hash: String)
    @JvmStatic private external fun nativeUnloadGame()
    @JvmStatic private external fun nativeDoFrame()
    @JvmStatic private external fun nativeIdle()
    @JvmStatic private external fun nativeServerResponse(slot: Int, status: Int, body: ByteArray)
    @JvmStatic private external fun nativeSerializeProgress(): ByteArray?
    @JvmStatic private external fun nativeDeserializeProgress(data: ByteArray): Boolean
    @JvmStatic private external fun nativeAchievements(): Array<String>?
    @JvmStatic private external fun nativeHashRom(data: ByteArray): String?
    @JvmStatic private external fun nativeVersion(): String?
}

/** The game a session is tracking. */
data class RaGame(val title: String, val achievementCount: Int)

/** An achievement earned while playing. */
data class RaUnlock(val id: Int, val title: String, val description: String, val points: Int)

/** One achievement in the loaded game's set. */
data class RaAchievement(
    val id: Int,
    val title: String,
    val description: String,
    val points: Int,
    val unlocked: Boolean,
    /** How far along, for the ones that track it. Empty when they do not. */
    val progress: String,
)
