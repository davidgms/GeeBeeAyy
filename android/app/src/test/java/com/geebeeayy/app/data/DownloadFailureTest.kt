package com.geebeeayy.app.data

import org.junit.Assert.assertEquals
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
}
