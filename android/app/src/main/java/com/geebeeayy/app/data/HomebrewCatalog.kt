package com.geebeeayy.app.data

/**
 * A small catalogue of freely distributable GBA software the app can fetch.
 *
 * Every entry is homebrew or a test ROM published by its author for free
 * download, and every URL here was fetched and its size checked before being
 * added - a broken entry in a download list is worse than a missing one.
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
    val approxBytes: Long,
    val url: String,
    val source: String,
)

object HomebrewCatalog {

    val entries: List<HomebrewEntry> = listOf(
        HomebrewEntry(
            name = "Celeste Classic",
            fileName = "celeste.gba",
            description = "The original PICO-8 Celeste, ported to the GBA. A real game, sprite-heavy.",
            approxBytes = 5_418_424,
            url = "https://github.com/JeffRuLz/Celeste-Classic-GBA/releases/download/v1.2/" +
                "Celeste.Classic.v1.2.Homebrew.gba",
            source = "JeffRuLz/Celeste-Classic-GBA",
        ),
        HomebrewEntry(
            name = "240p Test Suite",
            fileName = "240p.gba",
            description = "Display and timing test patterns: scroll tests, sprite tests, colour bars.",
            approxBytes = 61_104,
            url = "https://github.com/pinobatch/240p-test-mini/releases/download/v0.23/240pee_mb.gba",
            source = "pinobatch/240p-test-mini",
        ),
        HomebrewEntry(
            name = "Fantasy Knight",
            fileName = "fantasyknight.gba",
            description = "A graphics demo: affine backgrounds, windows and a scaling sprite.",
            approxBytes = 570_052,
            url = "https://github.com/laqieer/gba-free-fonts/releases/download/v1.2/fantasy-knight.gba",
            source = "laqieer/gba-free-fonts",
        ),
        HomebrewEntry(
            name = "gba-tests: ARM",
            fileName = "arm.gba",
            description = "jsmolka's ARM instruction suite. Prints \"All tests passed\" when the CPU is right.",
            approxBytes = 8_824,
            url = "https://raw.githubusercontent.com/jsmolka/gba-tests/master/arm/arm.gba",
            source = "jsmolka/gba-tests",
        ),
        HomebrewEntry(
            name = "gba-tests: THUMB",
            fileName = "thumb.gba",
            description = "The THUMB half of the same suite.",
            approxBytes = 3_680,
            url = "https://raw.githubusercontent.com/jsmolka/gba-tests/master/thumb/thumb.gba",
            source = "jsmolka/gba-tests",
        ),
        HomebrewEntry(
            name = "gba-tests: Hello",
            fileName = "hello.gba",
            description = "Draws \"Hello world!\". The smallest thing that proves the whole pipeline works.",
            approxBytes = 1_300,
            url = "https://raw.githubusercontent.com/jsmolka/gba-tests/master/ppu/hello.gba",
            source = "jsmolka/gba-tests",
        ),
        HomebrewEntry(
            name = "gba-tests: Stripes",
            fileName = "stripes.gba",
            description = "A two-colour background pattern, for checking mode 0 tile addressing.",
            approxBytes = 324,
            url = "https://raw.githubusercontent.com/jsmolka/gba-tests/master/ppu/stripes.gba",
            source = "jsmolka/gba-tests",
        ),
        HomebrewEntry(
            name = "gba-tests: Shades",
            fileName = "shades.gba",
            description = "A full palette of shades, for checking colour conversion.",
            approxBytes = 352,
            url = "https://raw.githubusercontent.com/jsmolka/gba-tests/master/ppu/shades.gba",
            source = "jsmolka/gba-tests",
        ),
    )
}
