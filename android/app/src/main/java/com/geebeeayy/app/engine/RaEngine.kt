package com.geebeeayy.app.engine

import android.content.Context
import android.util.Log
import com.geebeeayy.app.data.readCapped
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
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
    private const val MAX_RESPONSE_BYTES = 16L * 1024 * 1024

    /** `rc_client` event ids, from `rc_client.h`. Only the ones acted on. */
    private const val EVENT_ACHIEVEMENT_TRIGGERED = 1
    private const val EVENT_LEADERBOARD_STARTED = 2
    private const val EVENT_LEADERBOARD_FAILED = 3
    private const val EVENT_LEADERBOARD_SUBMITTED = 4
    private const val EVENT_LEADERBOARD_TRACKER_SHOW = 10
    private const val EVENT_LEADERBOARD_TRACKER_HIDE = 11
    private const val EVENT_LEADERBOARD_TRACKER_UPDATE = 12
    private const val EVENT_GAME_COMPLETED = 15

    // Test seam: the hooks below are internal so a JVM test can replace the
    // native library, the network and the scope. Production never touches them.
    internal var available: Boolean = runCatching {
        System.loadLibrary("geebeeayy_ra")
    }.isSuccess

    internal var scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    /** The HTTP call rcheevos asked for: status and body. */
    internal var http: suspend (url: String, post: String?) -> Pair<Int, ByteArray> = ::fetch

    internal var native: RaNative = object : RaNative {
        override fun nativeInit() = RaEngine.nativeInit()
        override fun nativeSetEmulator(handle: Long) = RaEngine.nativeSetEmulator(handle)
        override fun nativeLogin(username: String, password: String) =
            RaEngine.nativeLogin(username, password)
        override fun nativeLoginWithToken(username: String, token: String) =
            RaEngine.nativeLoginWithToken(username, token)
        override fun nativeLogout() = RaEngine.nativeLogout()
        override fun nativeUsername() = RaEngine.nativeUsername()
        override fun nativeToken() = RaEngine.nativeToken()
        override fun nativeLoadGame(hash: String) = RaEngine.nativeLoadGame(hash)
        override fun nativeUnloadGame() = RaEngine.nativeUnloadGame()
        override fun nativeDoFrame() = RaEngine.nativeDoFrame()
        override fun nativeIdle() = RaEngine.nativeIdle()
        override fun nativeServerResponse(slot: Int, status: Int, body: ByteArray) =
            RaEngine.nativeServerResponse(slot, status, body)
        override fun nativeSerializeProgress() = RaEngine.nativeSerializeProgress()
        override fun nativeDeserializeProgress(data: ByteArray) =
            RaEngine.nativeDeserializeProgress(data)
        override fun nativeAchievements() = RaEngine.nativeAchievements()
        override fun nativeLeaderboards() = RaEngine.nativeLeaderboards()
        override fun nativeHashRom(data: ByteArray) = RaEngine.nativeHashRom(data)
        override fun nativeVersion() = RaEngine.nativeVersion()
    }

    internal var storeFactory: (Context) -> RaStore = { RaCredentials(it) }

    /**
     * THE lock around the emulator. Every rcheevos call that can reach
     * `ra_read_memory` runs under it, and so does every thread that steps or
     * reads the core. Shared, not per ViewModel: the C side holds one global
     * emulator pointer and ViewModels are per navigation entry.
     */
    internal val coreLock = Mutex()

    private val startLock = Any()
    private var store: RaStore? = null

    @Volatile
    private var started = false
    private var tokenLoginSent = false

    private val _user = MutableStateFlow<String?>(null)
    /** The signed-in player's display name, or null. */
    val user: StateFlow<String?> = _user.asStateFlow()

    private val _game = MutableStateFlow<RaGame?>(null)
    /** The game this session is tracking, once the server has answered. */
    val game: StateFlow<RaGame?> = _game.asStateFlow()

    private val _unlocks = MutableSharedFlow<RaUnlock>(extraBufferCapacity = 8)
    /** Achievements earned during this session, as they happen. */
    val unlocks = _unlocks.asSharedFlow()

    private val _trackers = MutableStateFlow<Map<Int, String>>(emptyMap())
    /**
     * Leaderboard attempts in progress, id to the value to show.
     *
     * A tracker is the live readout of a run - a lap timer, a running score.
     * rcheevos formats the string itself, which is why this is text and not a
     * number: only it knows whether a leaderboard counts in seconds, frames
     * or points.
     */
    val trackers: StateFlow<Map<Int, String>> = _trackers.asStateFlow()

    private val _leaderboardNotice = MutableSharedFlow<String>(extraBufferCapacity = 4)
    /** "Attempt started", "score submitted" - worth a word, not a screen. */
    val leaderboardNotice = _leaderboardNotice.asSharedFlow()

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
    fun start(context: Context): Boolean = synchronized(startLock) {
        if (!available) return false
        if (!started) {
            if (!runCatching { native.nativeInit() }.getOrDefault(false)) return false
            store = storeFactory(context)
            started = true
        }
        val saved = store?.read()
        if (saved != null && !tokenLoginSent && _user.value == null) {
            tokenLoginSent = true
            runCatching { native.nativeLoginWithToken(saved.first, saved.second) }
        }
        true
    }

    /**
     * Tell the client which emulator to read memory from, or 0 to detach.
     *
     * Detach **before** that emulator is destroyed. The C side keeps the raw
     * handle, and a handle outliving its machine is a read of freed memory
     * the next time anything asks rcheevos about memory.
     */
    fun attachEmulator(handle: Long) {
        check(coreLock.isLocked) { "attachEmulator needs coreLock" }
        if (available && started) runCatching { native.nativeSetEmulator(handle) }
    }

    /**
     * Sign in with the player's own retroachievements.org account.
     *
     * The password is used once and never stored; what is kept is the token
     * the server returns.
     */
    fun login(username: String, password: String) {
        if (available && started) runCatching { native.nativeLogin(username, password) }
    }

    fun logout() {
        if (available && started) runCatching { native.nativeLogout() }
        store?.clear()
        _user.value = null
        _game.value = null
    }

    /** Ask the server what this ROM is, and start a session for it. Caller holds [coreLock]. */
    fun loadGame(hash: String) {
        check(coreLock.isLocked) { "loadGame needs coreLock" }
        Log.i(TAG, "load game $hash (started=$started)")
        if (available && started) runCatching { native.nativeLoadGame(hash) }
    }

    /** Caller holds [coreLock]. */
    fun unloadGame() {
        check(coreLock.isLocked) { "unloadGame needs coreLock" }
        if (available && started) runCatching { native.nativeUnloadGame() }
        _game.value = null
    }

    /** Once per emulated frame, on the emulation thread. Caller holds [coreLock]. */
    fun doFrame() {
        check(coreLock.isLocked) { "doFrame needs coreLock" }
        if (!available || !started) return
        native.nativeDoFrame()
    }

    /** While paused, so the session stays alive without evaluating. */
    fun idle() {
        if (available && started) runCatching { native.nativeIdle() }
    }

    /**
     * The achievement runtime's own state, to store beside a save state.
     *
     * An achievement is a **transition** - a condition that was false becoming
     * true while the runtime watches. That memory lives in rcheevos, not in
     * the emulator, so a save state that does not carry it restores the game
     * to one moment and leaves the achievement logic in another.
     */
    fun serializeProgress(): ByteArray? {
        check(coreLock.isLocked) { "serializeProgress needs coreLock" }
        return if (available && started) runCatching { native.nativeSerializeProgress() }.getOrNull() else null
    }

    fun restoreProgress(data: ByteArray): Boolean {
        check(coreLock.isLocked) { "restoreProgress needs coreLock" }
        if (!available || !started) return false
        val ok = runCatching { native.nativeDeserializeProgress(data) }.getOrDefault(false)
        // A bad sidecar makes rcheevos reset the set; the state still loads.
        if (!ok) Log.w(TAG, "achievement progress sidecar rejected")
        return ok
    }

    fun hashRom(romData: ByteArray): String? =
        if (available) runCatching { native.nativeHashRom(romData) }.getOrNull() else null

    /**
     * Every achievement in the loaded game, locked ones included.
     *
     * Empty when no game is loaded or nobody is signed in. Returned flat from
     * the C side, [ACHIEVEMENT_STRIDE] strings each, because a set runs to
     * hundreds and building objects across JNI costs a call each. Deliberately
     * not under [coreLock]: it reads rcheevos data only, and Compose must not
     * block on a frame.
     */
    fun achievements(): List<RaAchievement> {
        if (!available || !started) return emptyList()
        val f = runCatching { native.nativeAchievements() }.getOrNull() ?: return emptyList()
        return (0 until f.size / ACHIEVEMENT_STRIDE).mapNotNull { n ->
            val o = n * ACHIEVEMENT_STRIDE
            RaAchievement(
                id = f[o].toIntOrNull() ?: return@mapNotNull null,
                title = f[o + 1],
                description = f[o + 2],
                points = f[o + 3].toIntOrNull() ?: 0,
                unlocked = f[o + 4] != "0",
                progress = f[o + 5],
                badgeUrl = f[o + 6],
            )
        }
    }

    /** Every leaderboard in the loaded game, [LEADERBOARD_STRIDE] strings each. Not under [coreLock]. */
    fun leaderboards(): List<RaLeaderboard> {
        if (!available || !started) return emptyList()
        val f = runCatching { native.nativeLeaderboards() }.getOrNull() ?: return emptyList()
        return (0 until f.size / LEADERBOARD_STRIDE).mapNotNull { n ->
            val o = n * LEADERBOARD_STRIDE
            RaLeaderboard(
                id = f[o].toIntOrNull() ?: return@mapNotNull null,
                title = f[o + 1],
                description = f[o + 2],
                active = f[o + 3] == "2",
                value = f[o + 4],
            )
        }
    }

    fun version(): String? =
        if (available) runCatching { native.nativeVersion() }.getOrNull() else null

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
    private fun onServerRequest(slot: Int, url: String, postData: String?) =
        handleServerRequest(slot, url, postData)

    internal fun handleServerRequest(slot: Int, url: String, postData: String?) {
        scope.launch {
            val (status, body) = runCatching { http(url, postData) }.getOrElse {
                Log.w(TAG, "request failed: ${it.message}")
                // -1 is rcheevos' own "the client could not send this",
                // which it reports as a network error rather than a server
                // one. Leaving the slot unanswered would hang the session.
                -1 to ByteArray(0)
            }
            // Always delivered, to its own slot: a slot is freed only by its
            // answer. Under coreLock because rcheevos may read emulated memory
            // while handling it (address validation on game load).
            coreLock.withLock {
                runCatching { native.nativeServerResponse(slot, status, body) }
            }
        }
    }

    private suspend fun fetch(url: String, postData: String?): Pair<Int, ByteArray> {
        // Credentials ride on these requests; never over cleartext. The caller treats this like any I/O error.
        if (!com.geebeeayy.app.data.isHttps(url)) {
            Log.w(TAG, "refusing non-https request")
            throw java.io.IOException("non-https URL refused")
        }
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
                setRequestProperty("Content-Type", "application/x-www-form-urlencoded")
                outputStream.use { it.write(postData.toByteArray()) }
            }
        }
        try {
            val status = connection.responseCode
            val stream = if (status in 200..299) connection.inputStream else connection.errorStream
            // The largest achievement set is a few MB of JSON; past the cap the
            // request fails like a dropped connection rather than filling memory.
            val body = stream?.use { it.readCapped(MAX_RESPONSE_BYTES) } ?: ByteArray(0)
            return status to body
        } finally {
            connection.disconnect()
        }
    }

    @JvmStatic
    private fun onEvent(type: Int, id: Int, title: String?, description: String?, points: Int) =
        handleEvent(type, id, title, description, points)

    internal fun handleEvent(type: Int, id: Int, title: String?, description: String?, points: Int) {
        when (type) {
            EVENT_ACHIEVEMENT_TRIGGERED ->
                _unlocks.tryEmit(RaUnlock(id, title.orEmpty(), description.orEmpty(), points))
            EVENT_LEADERBOARD_TRACKER_SHOW,
            EVENT_LEADERBOARD_TRACKER_UPDATE ->
                _trackers.update { it + (id to title.orEmpty()) }
            EVENT_LEADERBOARD_TRACKER_HIDE ->
                _trackers.update { it - id }
            EVENT_LEADERBOARD_STARTED ->
                _leaderboardNotice.tryEmit("Leaderboard attempt started: ${title.orEmpty()}")
            EVENT_LEADERBOARD_SUBMITTED ->
                _leaderboardNotice.tryEmit("Leaderboard score submitted: ${title.orEmpty()}")
            // Deliberately quiet: a failed attempt is the normal end of most
            // runs, and saying so every time would be nagging.
            EVENT_LEADERBOARD_FAILED -> Log.i(TAG, "leaderboard attempt failed")
            EVENT_GAME_COMPLETED -> Log.i(TAG, "game completed")
        }
    }

    @JvmStatic
    private fun onLogin(result: Int, errorMessage: String?) = handleLogin(result, errorMessage)

    internal fun handleLogin(result: Int, errorMessage: String?) {
        // The sign-in was superseded; rc_client handles that itself.
        if (result == RESULT_ABORTED) return
        if (result == RESULT_OK) {
            Log.i(TAG, "signed in")
            val name = runCatching { native.nativeUsername() }.getOrNull()
            val token = runCatching { native.nativeToken() }.getOrNull()
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
    private fun onGameLoaded(result: Int, errorMessage: String?, title: String?, achievements: Int) =
        handleGameLoaded(result, errorMessage, title, achievements)

    internal fun handleGameLoaded(
        result: Int,
        errorMessage: String?,
        title: String?,
        achievements: Int,
    ) {
        // The old game's abort must not clear the new game's state.
        if (result == RESULT_ABORTED) return
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

    private const val ACHIEVEMENT_STRIDE = 7
    private const val LEADERBOARD_STRIDE = 5

    /** `RC_ABORTED`: the session this answer belonged to has ended. */
    private const val RESULT_ABORTED = -31

    /** `RC_OK`. */
    private const val RESULT_OK = 0

    /** Back to a fresh process, for tests. */
    internal fun resetForTest() {
        started = false
        tokenLoginSent = false
        store = null
        _user.value = null
        _game.value = null
        _trackers.value = emptyMap()
    }

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
    @JvmStatic private external fun nativeLeaderboards(): Array<String>?
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
    /**
     * The badge for the state this achievement is in - greyed while locked,
     * full colour once earned. Empty when rcheevos could not build a URL.
     */
    val badgeUrl: String = "",
)

/** One leaderboard in the loaded game's set. */
data class RaLeaderboard(
    val id: Int,
    val title: String,
    val description: String,
    /** True while an attempt is being tracked right now. */
    val active: Boolean,
    /** The current or last value, already formatted by rcheevos. */
    val value: String,
)

/** The `external` functions of [RaEngine], so a test can stand in for the library. */
internal interface RaNative {
    fun nativeInit(): Boolean
    fun nativeSetEmulator(handle: Long)
    fun nativeLogin(username: String, password: String)
    fun nativeLoginWithToken(username: String, token: String)
    fun nativeLogout()
    fun nativeUsername(): String?
    fun nativeToken(): String?
    fun nativeLoadGame(hash: String)
    fun nativeUnloadGame()
    fun nativeDoFrame()
    fun nativeIdle()
    fun nativeServerResponse(slot: Int, status: Int, body: ByteArray)
    fun nativeSerializeProgress(): ByteArray?
    fun nativeDeserializeProgress(data: ByteArray): Boolean
    fun nativeAchievements(): Array<String>?
    fun nativeLeaderboards(): Array<String>?
    fun nativeHashRom(data: ByteArray): String?
    fun nativeVersion(): String?
}
