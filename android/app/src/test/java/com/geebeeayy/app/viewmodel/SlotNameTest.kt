package com.geebeeayy.app.viewmodel

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * The save/load messages have to name the slot the player tapped.
 *
 * They used to be built from `slot + 1`, so saving into the row labelled
 * "Slot 2" reported "Saved to slot 3" - the one visible symptom of an
 * off-by-one that ran through every save-state message.
 */
class SlotNameTest {

    @Test
    fun `slot zero is the quick save slot`() {
        assertEquals("quick save slot", EmulationViewModel.slotName(0))
    }

    @Test
    fun `every other slot is named by its own index`() {
        for (slot in 1 until EmulationViewModel.SLOT_COUNT) {
            assertEquals("slot $slot", EmulationViewModel.slotName(slot))
        }
    }
}
