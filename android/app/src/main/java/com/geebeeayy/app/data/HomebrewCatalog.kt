package com.geebeeayy.app.data

/**
 * A small catalogue of freely distributable GBA software the app can fetch.
 *
 * Every entry is homebrew or a test ROM published by its author for free
 * download, and every URL here was fetched and its size checked before being
 * added - a broken entry in a download list is worse than a missing one.
 * Each entry carries the size and SHA-256 of the file as it was checked, so a
 * release asset replaced upstream, or a tampered mirror, is refused rather
 * than dropped into the library.
 *
 * This is deliberately **not** a commercial ROM index. Those are the game
 * publishers' to distribute, and a "click to download any GBA game" list is a
 * piracy tool whatever it is labelled. Commercial ROMs go in the ROM folder by
 * the player's own hand, from their own cartridges or their own backups.
 */
data class HomebrewEntry(
    val name: String,
    val fileName: String,
    val description: String,
    /** Exact size of the file at [url], checked on download. */
    val bytes: Long,
    val url: String,
    /** SHA-256 of the file at [url], checked before it lands in the library. */
    val sha256: String,
    val source: String,
)

object HomebrewCatalog {

    // A commit, not `master`: a branch can change under a pinned hash and
    // turn every download into a mismatch.
    private const val JSMOLKA_COMMIT = "a7113b67e63f83a9b321696ddd7042ccfad6c881"

    val entries: List<HomebrewEntry> = listOf(
        HomebrewEntry(
            name = "Celeste Classic",
            fileName = "celeste.gba",
            description = "The original PICO-8 Celeste, ported to the GBA. A real game, sprite-heavy.",
            bytes = 5_418_424,
            url = "https://github.com/JeffRuLz/Celeste-Classic-GBA/releases/download/v1.2/" +
                "Celeste.Classic.v1.2.Homebrew.gba",
            sha256 = "4463620519f9c60a435ef0a6fa2cca123409380881fb0d00c63f2daed0c4f85b",
            source = "JeffRuLz/Celeste-Classic-GBA",
        ),
        HomebrewEntry(
            name = "240p Test Suite",
            fileName = "240p.gba",
            description = "Display and timing test patterns: scroll tests, sprite tests, colour bars.",
            bytes = 61_104,
            url = "https://github.com/pinobatch/240p-test-mini/releases/download/v0.23/240pee_mb.gba",
            sha256 = "47844f7140738a06f8f3bc09780da3ab095539a250b870f614feed561d9d6f34",
            source = "pinobatch/240p-test-mini",
        ),
        HomebrewEntry(
            name = "Fantasy Knight",
            fileName = "fantasyknight.gba",
            description = "A graphics demo: affine backgrounds, windows and a scaling sprite.",
            bytes = 570_052,
            url = "https://github.com/laqieer/gba-free-fonts/releases/download/v1.2/fantasy-knight.gba",
            sha256 = "e449a76d0cbb01427aa06b515e2447f42bbf46e3c4811dac5dc5e9a156d6157a",
            source = "laqieer/gba-free-fonts",
        ),
        HomebrewEntry(
            name = "gba-tests: ARM",
            fileName = "arm.gba",
            description = "jsmolka's ARM instruction suite. Prints \"All tests passed\" when the CPU is right.",
            bytes = 8_824,
            url = "https://raw.githubusercontent.com/jsmolka/gba-tests/$JSMOLKA_COMMIT/arm/arm.gba",
            sha256 = "77ee88662552bdc885c1080c0172ff119d54db791bd73b21808cf1ff1fe5b40e",
            source = "jsmolka/gba-tests",
        ),
        HomebrewEntry(
            name = "gba-tests: THUMB",
            fileName = "thumb.gba",
            description = "The THUMB half of the same suite.",
            bytes = 3_680,
            url = "https://raw.githubusercontent.com/jsmolka/gba-tests/$JSMOLKA_COMMIT/thumb/thumb.gba",
            sha256 = "b5cb2291df4ab314b31c598acd9bff2ccfa0b38efff29daadfe97422ce369b67",
            source = "jsmolka/gba-tests",
        ),
        HomebrewEntry(
            name = "gba-tests: Hello",
            fileName = "hello.gba",
            description = "Draws \"Hello world!\". The smallest thing that proves the whole pipeline works.",
            bytes = 1_300,
            url = "https://raw.githubusercontent.com/jsmolka/gba-tests/$JSMOLKA_COMMIT/ppu/hello.gba",
            sha256 = "38aed48b67bc0f701e8aa222b0c3334bd306bd29888707bb7224d81f5576c264",
            source = "jsmolka/gba-tests",
        ),
        HomebrewEntry(
            name = "gba-tests: Stripes",
            fileName = "stripes.gba",
            description = "A two-colour background pattern, for checking mode 0 tile addressing.",
            bytes = 324,
            url = "https://raw.githubusercontent.com/jsmolka/gba-tests/$JSMOLKA_COMMIT/ppu/stripes.gba",
            sha256 = "80d6cc05b5f289b9a2392621fc245270f08e9f0d77fd33982120cef240f5a504",
            source = "jsmolka/gba-tests",
        ),
        HomebrewEntry(
            name = "gba-tests: Shades",
            fileName = "shades.gba",
            description = "A full palette of shades, for checking colour conversion.",
            bytes = 352,
            url = "https://raw.githubusercontent.com/jsmolka/gba-tests/$JSMOLKA_COMMIT/ppu/shades.gba",
            sha256 = "127ce348f17e4d5fd8a0821a7af757eda6109c66c9f7b8f7d1e0fa31831b407d",
            source = "jsmolka/gba-tests",
        ),
    )
}
