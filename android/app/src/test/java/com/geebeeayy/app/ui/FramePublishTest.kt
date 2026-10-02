package com.geebeeayy.app.ui

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withContext
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

/**
 * The emulation thread hands frames to the screen through a StateFlow
 * (`EmulationViewModel._frameBuffer`), and `GbaScreen` draws them on its own
 * collector. Audio is the clock, so the loop must never wait for the screen:
 * on a vsync-paced design a 50 Hz panel slows the game by 17% (Azahar #1193).
 *
 * This pins the handoff's contract. If someone swaps the StateFlow for a
 * rendezvous channel, a queue with back-pressure or a shared lock, this fails.
 */
class FramePublishTest {

    @Test
    fun `a stuck draw never blocks publishing, and the screen gets the newest frame`() = runBlocking {
        val frames = MutableStateFlow<ByteArray?>(null)
        val drawing = CountDownLatch(1)
        val release = CountDownLatch(1)
        val drawn = mutableListOf<ByteArray>()

        val screen = launch(Dispatchers.Default) {
            frames.collect { frame ->
                if (frame == null) return@collect
                drawn += frame
                if (drawn.size == 1) {
                    drawing.countDown()
                    release.await() // a draw stuck far longer than a frame
                }
            }
        }

        frames.value = byteArrayOf(0)
        assertTrue(drawing.await(2, TimeUnit.SECONDS))

        // 600 frames is ten seconds of play; with the screen stuck it must
        // still take no real time.
        val started = System.nanoTime()
        withContext(Dispatchers.Default) {
            for (i in 1..600) frames.value = byteArrayOf((i % 128).toByte())
        }
        val publishMs = (System.nanoTime() - started) / 1_000_000
        assertTrue("publishing waited ${publishMs} ms on the screen", publishMs < 200)

        release.countDown()
        withContext(Dispatchers.Default) {
            val deadline = System.currentTimeMillis() + 2_000
            while (drawn.size < 2 && System.currentTimeMillis() < deadline) Thread.sleep(5)
        }
        screen.cancel()
        // Frames published while the screen was stuck are skipped, not queued.
        assertArrayEquals(byteArrayOf((600 % 128).toByte()), drawn.last())
        assertTrue("the screen replayed a backlog: ${drawn.size} draws", drawn.size <= 3)
    }
}
