package com.gbee.emulator.engine

import android.content.ContextWrapper
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.sync.withLock
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Before
import org.junit.Test
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger

/**
 * RetroAchievements threading: the lock around everything that can read core
 * memory, delivery of server answers, and the shared state the callbacks touch.
 * No library, no network, no sleeps - [RaNative] and [RaEngine.http] are fakes.
 */
class RaEngineTest {

    private data class Call(val method: String, val args: List<Any?>, val thread: Thread, val locked: Boolean)

    private class FakeNative : RaNative {
        val calls = java.util.Collections.synchronizedList(mutableListOf<Call>())
        val inits = AtomicInteger()
        val tokenLogins = AtomicInteger()
        var onLoadGame: () -> Unit = {}
        var onInit: () -> Unit = {}
        var achievements: Array<String>? = null

        private fun rec(method: String, vararg args: Any?) {
            calls += Call(method, args.toList(), Thread.currentThread(), RaEngine.coreLock.isLocked)
        }

        fun named(method: String) = calls.filter { it.method == method }

        override fun nativeInit(): Boolean { rec("init"); inits.incrementAndGet(); onInit(); return true }
        override fun nativeSetEmulator(handle: Long) = rec("setEmulator", handle)
        override fun nativeLogin(username: String, password: String) = rec("login")
        override fun nativeLoginWithToken(username: String, token: String) {
            rec("loginWithToken"); tokenLogins.incrementAndGet()
        }
        override fun nativeLogout() = rec("logout")
        override fun nativeUsername(): String? = "me"
        override fun nativeToken(): String? = "tok"
        override fun nativeLoadGame(hash: String) { rec("loadGame", hash); onLoadGame() }
        override fun nativeUnloadGame() = rec("unloadGame")
        override fun nativeDoFrame() = rec("doFrame")
        override fun nativeIdle() = rec("idle")
        override fun nativeServerResponse(slot: Int, status: Int, body: ByteArray) =
            rec("serverResponse", slot, status, body.toList())
        override fun nativeSerializeProgress(): ByteArray? { rec("serialize"); return ByteArray(1) }
        override fun nativeDeserializeProgress(data: ByteArray): Boolean { rec("restore"); return true }
        override fun nativeAchievements(): Array<String>? = achievements
        override fun nativeLeaderboards(): Array<String>? = null
        override fun nativeHashRom(data: ByteArray): String? = null
        override fun nativeVersion(): String? = null
    }

    private class FakeStore(var saved: Pair<String, String>? = null) : RaStore {
        val clears = AtomicInteger()
        override fun read() = saved
        override fun write(user: String, token: String) { saved = user to token }
        override fun clear() { clears.incrementAndGet(); saved = null }
    }

    private lateinit var fake: FakeNative
    private lateinit var store: FakeStore

    @Before
    fun setUp() {
        RaEngine.resetForTest()
        fake = FakeNative()
        store = FakeStore()
        RaEngine.available = true
        RaEngine.native = fake
        RaEngine.storeFactory = { store }
        RaEngine.scope = CoroutineScope(Dispatchers.Unconfined)
    }

    @After
    fun tearDown() {
        RaEngine.resetForTest()
    }

    private fun started() = assertTrue(RaEngine.start(ContextWrapper(null)))

    // 1 ---------------------------------------------------------------

    @Test
    fun `memory reading calls run under the core lock`() = runBlocking {
        started()
        RaEngine.coreLock.withLock {
            RaEngine.attachEmulator(7)
            RaEngine.loadGame("hash")
            RaEngine.doFrame()
            RaEngine.serializeProgress()
            RaEngine.restoreProgress(ByteArray(1))
            RaEngine.unloadGame()
        }
        val names = listOf("setEmulator", "loadGame", "doFrame", "serialize", "restore", "unloadGame")
        for (n in names) {
            val c = fake.named(n)
            assertEquals("$n called once", 1, c.size)
            assertTrue("$n ran under coreLock", c.single().locked)
        }
    }

    @Test
    fun `a server answer is delivered under the core lock on the completing thread`() = runBlocking {
        started()
        val gate = CompletableDeferred<Pair<Int, ByteArray>>()
        RaEngine.http = { _, _ -> gate.await() }
        val caller = Thread { RaEngine.handleServerRequest(3, "u", null) }
        caller.start(); caller.join()

        gate.complete(200 to "X".toByteArray())

        val delivered = fake.named("serverResponse").single()
        assertEquals(listOf(3, 200, "X".toByteArray().toList()), delivered.args)
        assertTrue("delivered under coreLock", delivered.locked)
        assertEquals(Thread.currentThread(), delivered.thread)
        assertNotEquals(caller, delivered.thread)
    }

    @Test
    fun `doFrame without the core lock is refused`() {
        started()
        try {
            RaEngine.doFrame()
            fail("doFrame must require coreLock")
        } catch (_: IllegalStateException) {
        }
    }

    // 2 ---------------------------------------------------------------

    @Test
    fun `an answer arriving after detach still reaches its own slot under the lock`() = runBlocking {
        started()
        val gate = CompletableDeferred<Pair<Int, ByteArray>>()
        RaEngine.http = { _, _ -> gate.await() }
        fake.onLoadGame = { RaEngine.handleServerRequest(3, "u", null) }
        RaEngine.coreLock.withLock {
            RaEngine.attachEmulator(5)
            RaEngine.loadGame("hash")
            RaEngine.unloadGame()
            RaEngine.attachEmulator(0)
        }

        gate.complete(200 to "X".toByteArray())

        val delivered = fake.named("serverResponse").single()
        assertEquals(listOf(3, 200, "X".toByteArray().toList()), delivered.args)
        assertTrue("delivered under coreLock", delivered.locked)
    }

    // 3 / 4 -----------------------------------------------------------

    @Test
    fun `an aborted game load leaves the current game alone`() {
        RaEngine.handleGameLoaded(0, null, "Zelda", 12)
        RaEngine.handleGameLoaded(-31, "The requested game is no longer active", null, 0)
        assertEquals(RaGame("Zelda", 12), RaEngine.game.value)
    }

    @Test
    fun `an aborted login leaves the stored token alone`() {
        started()
        RaEngine.handleLogin(0, null)
        assertEquals("me" to "tok", store.saved)

        RaEngine.handleLogin(-31, "aborted")

        assertEquals(0, store.clears.get())
        assertEquals("me" to "tok", store.saved)
        assertEquals("me", RaEngine.user.value)
    }

    // 5 ---------------------------------------------------------------

    /** The load sequence the ViewModel runs; the lock block itself is checked in review. */
    @Test
    fun `detaching for a load ends with the emulator detached`() = runBlocking {
        started()
        RaEngine.coreLock.withLock {
            RaEngine.attachEmulator(5)
            RaEngine.loadGame("hash")
            RaEngine.unloadGame()
            RaEngine.attachEmulator(0)
        }
        val last = fake.calls.last { it.method == "setEmulator" }
        assertEquals(listOf<Any?>(0L), last.args)
        assertNull(RaEngine.game.value)
        assertTrue(fake.calls.indexOfLast { it.method == "unloadGame" } < fake.calls.indexOf(last))
    }

    // 6 ---------------------------------------------------------------

    @Test
    fun `concurrent start initialises the native client once`() {
        val bothIn = CountDownLatch(2)
        // A second entrant, if the race exists, gets here within the wait.
        fake.onInit = { bothIn.countDown(); bothIn.await(300, TimeUnit.MILLISECONDS) }
        store.saved = "me" to "tok"
        val go = CountDownLatch(1)
        val results = java.util.Collections.synchronizedList(mutableListOf<Boolean>())
        val threads = List(2) {
            Thread {
                go.await()
                results += RaEngine.start(ContextWrapper(null))
            }.also { it.start() }
        }
        go.countDown()
        threads.forEach { it.join() }

        assertEquals(listOf(true, true), results)
        assertEquals(1, fake.inits.get())
        assertTrue("token login sent at most once", fake.tokenLogins.get() <= 1)
    }

    // 7 ---------------------------------------------------------------

    @Test
    fun `parallel tracker events lose no tracker`() {
        val show = 10
        val hide = 11
        fun burst(type: Int) {
            val go = CountDownLatch(1)
            val threads = List(8) { t ->
                Thread {
                    go.await()
                    for (i in 0 until 200) RaEngine.handleEvent(type, t * 1000 + i, "v", null, 0)
                }.also { it.start() }
            }
            go.countDown()
            threads.forEach { it.join() }
        }
        burst(show)
        assertEquals(1600, RaEngine.trackers.value.size)
        burst(hide)
        assertEquals(0, RaEngine.trackers.value.size)
    }

    // 8 ---------------------------------------------------------------

    @Test
    fun `achievements are parsed by stride so a tab in a title survives`() {
        started()
        fake.achievements = arrayOf(
            "1", "Tab\tTitle", "desc one", "10", "1", "", "http://b/1.png",
            "2", "Plain", "desc two", "5", "0", "3/10", "http://b/2.png",
        )
        val list = RaEngine.achievements()
        assertEquals(listOf("Tab\tTitle", "Plain"), list.map { it.title })
        assertEquals(listOf(1, 2), list.map { it.id })
    }
}
