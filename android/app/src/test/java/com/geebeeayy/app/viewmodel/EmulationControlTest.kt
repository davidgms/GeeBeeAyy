package com.geebeeayy.app.viewmodel

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The three pieces of EmulationViewModel that decide *whether* something
 * reaches the engine, pulled out so a JVM test can pin them.
 */
class EmulationControlTest {

    @Test
    fun `two requests before the loop drains are both delivered in order`() {
        val queue = CommandQueue<StateCommand>()
        queue.offer(StateCommand.Save(1))
        queue.offer(StateCommand.Save(2))
        val seen = mutableListOf<StateCommand>()
        queue.drain { seen += it }
        assertEquals(listOf<StateCommand>(StateCommand.Save(1), StateCommand.Save(2)), seen)
        queue.drain { seen += it }
        assertEquals(2, seen.size)
    }

    @Test
    fun `a rejected state leaves the achievement runtime alone`() {
        var restored = false
        val ok = applyStateLoad(byteArrayOf(1), byteArrayOf(9), { false }, { restored = true })
        assertFalse(ok)
        assertFalse(restored)
    }

    @Test
    fun `an accepted state restores the achievement sidecar`() {
        var restored: ByteArray? = null
        val ok = applyStateLoad(byteArrayOf(1), byteArrayOf(9), { true }, { restored = it })
        assertTrue(ok)
        assertEquals(9.toByte(), restored!![0])
    }

    @Test
    fun `a state with no sidecar loads without touching achievements`() {
        var restored = false
        assertTrue(applyStateLoad(byteArrayOf(1), null, { true }, { restored = true }))
        assertFalse(restored)
    }

    @Test
    fun `nothing starts before the screen is resumed`() {
        val gate = RunGate()
        assertFalse(gate.mayStart(romLoaded = true))
        assertFalse(gate.shouldAutoResume(romLoaded = true, running = false))
    }

    @Test
    fun `resuming starts a loaded rom the player had not paused`() {
        val gate = RunGate()
        gate.resumed = true
        assertTrue(gate.mayStart(romLoaded = true))
        assertTrue(gate.shouldAutoResume(romLoaded = true, running = false))
        assertFalse(gate.shouldAutoResume(romLoaded = true, running = true))
        assertFalse(gate.shouldAutoResume(romLoaded = false, running = false))
    }

    @Test
    fun `a player pause survives a background and resume`() {
        val gate = RunGate()
        gate.resumed = true
        gate.userPaused = true
        assertFalse(gate.shouldAutoResume(romLoaded = true, running = false))
    }
}
