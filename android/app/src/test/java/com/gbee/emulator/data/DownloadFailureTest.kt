package com.gbee.emulator.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.IOException
import java.net.SocketException
import java.net.SocketTimeoutException
import java.net.UnknownHostException

class DownloadFailureTest {

    private val offline = "Connection lost. Check your internet and try again."

    @Test
    fun `wifi dropped mid-download reads as a lost connection`() {
        // The device run showed "Software caused connection abort" verbatim.
        assertEquals(offline, HomebrewDownloader.failureMessage(SocketException("Software caused connection abort")))
    }

    @Test
    fun `no network at all reads as a lost connection`() {
        assertEquals(offline, HomebrewDownloader.failureMessage(UnknownHostException("github.com")))
    }

    @Test
    fun `a stalled server reads as too slow`() {
        assertEquals(
            "The server took too long to answer. Try again later.",
            HomebrewDownloader.failureMessage(SocketTimeoutException("timeout")),
        )
    }

    @Test
    fun `a disk error says the file could not be saved`() {
        assertEquals(
            "Could not save the file. Check that the folder has free space.",
            HomebrewDownloader.failureMessage(IOException("ENOSPC (No space left on device)")),
        )
    }

    @Test
    fun `a download longer than the catalogue says reads as a mismatch, not a lost connection`() {
        assertTrue(HomebrewDownloader.failureMessage(TooLargeException(324)).startsWith("The download is not the file"))
    }

    @Test
    fun `every catalogue entry is pinned by hash and never by branch`() {
        HomebrewCatalog.entries.forEach { entry ->
            assertTrue(entry.name, Regex("[0-9a-f]{64}").matches(entry.sha256))
            assertTrue(entry.name, entry.bytes > 0)
            assertFalse(entry.name, "/master/" in entry.url || "/main/" in entry.url)
        }
    }
}
