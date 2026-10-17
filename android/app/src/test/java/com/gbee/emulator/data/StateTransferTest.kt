package com.gbee.emulator.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File

class StateTransferTest {

    @get:Rule
    val folder = TemporaryFolder()

    @Test
    fun `a state is named after the rom, slot and kind`() {
        assertEquals("Hamtaro.slot3.gbstate", stateExportName("Hamtaro.gba", 3, false, emptyList()))
        assertEquals("Hamtaro.slot0.gbra", stateExportName("Hamtaro.gba", 0, true, emptyList()))
    }

    @Test
    fun `the one state name already there for the same game and slot is reused`() {
        val there = listOf("hamtaro (U).slot3.gbstate", "hamtaro (U).slot4.gbstate", "hamtaro (U).slot3.gbra")
        assertEquals("hamtaro (U).slot3.gbstate", stateExportName("Hamtaro.gba", 3, false, there))
        assertEquals("hamtaro (U).slot3.gbra", stateExportName("Hamtaro.gba", 3, true, there))
        assertEquals("Hamtaro.slot5.gbstate", stateExportName("Hamtaro.gba", 5, false, there))
    }

    @Test
    fun `state names parse back to base, slot and kind`() {
        assertEquals(StateName("Mr. Driller", 2, false), parseStateName("Mr. Driller.slot2.gbstate"))
        assertEquals(StateName("Game", 0, true), parseStateName("Game.SLOT0.GBRA"))
        assertNull(parseStateName("Game.sav"))
        assertNull(parseStateName("Game.slotx.gbstate"))
        assertNull(parseStateName(".slot1.gbstate"))
    }

    @Test
    fun `the magic is GBAS`() {
        fun header(magic: String, version: Int) =
            magic.toByteArray() + byteArrayOf(version.toByte(), (version shr 8).toByte(), 0, 0) + ByteArray(8)
        assertTrue(isGbeeState(header("GBAS", 11)))
        assertTrue(isGbeeState(header("GBAS", 10)))
        assertTrue(isGbeeState(header("GBAS", 6)))
        assertFalse(isGbeeState(header("MGBA", 10)))
        // A newer core's state, or one older than the oldest this core restores.
        assertFalse(isGbeeState(header("GBAS", 12)))
        assertFalse(isGbeeState(header("GBAS", 5)))
        assertFalse(isGbeeState("GBAS".toByteArray()))
        assertFalse(isGbeeState(byteArrayOf(1, 2)))
    }

    @Test
    fun `importing over a state keeps the old one as bak1`() {
        val f = File(folder.root, "states/K_slot1.state")
        assertTrue(importStateFile(f, byteArrayOf(1)))
        assertTrue(importStateFile(f, byteArrayOf(2)))
        assertEquals(listOf<Byte>(2), f.readBytes().toList())
        assertEquals(listOf<Byte>(1), File(f.path + ".bak1").readBytes().toList())
    }
}
