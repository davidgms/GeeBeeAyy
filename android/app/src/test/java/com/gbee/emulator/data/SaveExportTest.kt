package com.gbee.emulator.data

import org.junit.Assert.assertEquals
import org.junit.Test

class SaveExportTest {

    @Test
    fun `a new save is named after the rom file`() {
        assertEquals("Hamtaro - Ham Ham Games.sav", exportTargetName("Hamtaro - Ham Ham Games.gba", emptyList()))
    }

    @Test
    fun `the name of the one file already there is reused exactly`() {
        val there = listOf("hamtaro ham ham games (U).srm", "other.sav")
        assertEquals("hamtaro ham ham games (U).srm", exportTargetName("Hamtaro - Ham Ham Games.gba", there))
    }

    @Test
    fun `two candidates or a non save file fall back to the rom name`() {
        assertEquals("Game (Rev1).sav", exportTargetName("Game (Rev1).gba", listOf("Game.sav", "Game (USA).srm")))
        assertEquals("Game.sav", exportTargetName("Game.gba", listOf("game.txt")))
    }

    @Test
    fun `a repeated key keeps only the first`() {
        assertEquals(listOf("a1", "b1"), dedupeBy(listOf("a1", "b1", "a2")) { it.take(1) })
    }
}
