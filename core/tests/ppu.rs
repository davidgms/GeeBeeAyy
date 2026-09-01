//! PPU rendering, checked against jsmolka/gba-tests' visual ROMs.
//!
//! These ROMs carry no pass/fail register - they draw and then spin - so the
//! assertions here are structural: the colours that come out must be the ones
//! the ROM wrote into the palette, in the proportions its tile map implies.
//! That is weaker than a reference image and much stronger than nothing, and
//! it is what caught mode 0 reading tile data from the wrong address entirely.
//!
//! ROMs are never committed. Fetch them into `temp/roms/`:
//!
//! ```text
//! for r in hello stripes shades; do
//!   curl -sSLO "https://raw.githubusercontent.com/jsmolka/gba-tests/master/ppu/$r.gba"
//! done
//!
//! # Freely distributable homebrew, for the parts jsmolka's suite does not
//! # reach - both of these are sprite-heavy, which is how the OBJ renderer's
//! # wrong tile base finally showed up.
//! curl -sSL -o celeste.gba \
//!   https://github.com/JeffRuLz/Celeste-Classic-GBA/releases/download/v1.2/Celeste.Classic.v1.2.Homebrew.gba
//! curl -sSL -o 240p.gba \
//!   https://github.com/pinobatch/240p-test-mini/releases/download/v0.23/240pee_mb.gba
//! ```

use geebeeayy_core::Gba;
use std::collections::HashMap;

/// Run a ppu ROM for a few frames and tally the frame buffer's colours.
fn render(name: &str) -> Option<HashMap<(u8, u8, u8), usize>> {
    let path = format!("{}/../temp/roms/{}.gba", env!("CARGO_MANIFEST_DIR"), name);
    let Ok(rom) = std::fs::read(&path) else {
        eprintln!("skipping ppu '{name}': {path} not present");
        return None;
    };
    let mut gba = Gba::new();
    gba.load_rom(&rom).expect("ppu ROM should load");
    for _ in 0..8 {
        gba.run_frame();
    }
    let mut counts = HashMap::new();
    for px in gba.frame_buffer().chunks(3) {
        *counts.entry((px[0], px[1], px[2])).or_insert(0) += 1;
    }
    Some(counts)
}

/// BGR555 to the RGB888 the frame buffer holds.
fn rgb(bgr555: u16) -> (u8, u8, u8) {
    (
        ((bgr555 & 0x1F) as u8) << 3,
        (((bgr555 >> 5) & 0x1F) as u8) << 3,
        (((bgr555 >> 10) & 0x1F) as u8) << 3,
    )
}

#[test]
fn mode0_tiled_background_uses_the_palette_the_rom_wrote() {
    let Some(counts) = render("stripes") else { return };

    // The ROM writes 0x560B as the backdrop and 0x6290 as colour 1, then fills
    // alternating rows. Both must appear, and nothing else.
    let backdrop = rgb(0x560B);
    let stripe = rgb(0x6290);

    assert_eq!(counts.len(), 2, "expected exactly two colours, got {counts:?}");
    assert!(counts.contains_key(&backdrop), "backdrop 0x560B is missing");
    assert!(counts.contains_key(&stripe), "colour 1 (0x6290) is missing");
    assert_eq!(
        counts[&backdrop], counts[&stripe],
        "the stripes should split the screen evenly"
    );
}

#[test]
fn mode0_renders_a_full_palette_of_shades() {
    let Some(counts) = render("shades") else { return };

    // The ROM steps the blue channel through sixteen values, so every colour on
    // screen must be pure blue and there must be many distinct ones. A single
    // colour here means the backgrounds are not drawing at all.
    assert!(counts.len() >= 15, "expected ~16 shades, got {counts:?}");
    for &(r, g, _b) in counts.keys() {
        assert_eq!((r, g), (0, 0), "a shade of blue should have no red or green");
    }
}

#[test]
fn text_renders_over_the_backdrop() {
    let Some(counts) = render("hello") else { return };

    // "Hello world!" in white on a black backdrop: mostly backdrop, a little
    // text, nothing in between.
    let text: usize = counts
        .iter()
        .filter(|(&(r, g, b), _)| r > 200 && g > 200 && b > 200)
        .map(|(_, &n)| n)
        .sum();
    assert!(text > 0, "no text pixels were drawn");
    assert!(
        text < 240 * 160 / 4,
        "text should cover a small part of the screen, covered {text} pixels"
    );
}

/// The PPU composes DISPSTAT every tick. It used to build the whole register
/// from cached copies of the IRQ-enable bits refreshed once per scanline, so a
/// game that enabled the VBlank IRQ mid-scanline had its write erased a few
/// cycles later and then waited on an interrupt that could never fire.
#[test]
fn dispstat_keeps_the_irq_enable_bits_the_game_wrote() {
    let mut gba = Gba::new();
    // VBlank + HBlank + VCount IRQ enabled, VCount setting 0x50.
    gba.bus.write16(0x0400_0004, 0x5038);
    gba.run_frame();
    let dispstat = gba.bus.read16(0x0400_0004);
    assert_eq!(
        dispstat & 0xFF38,
        0x5038,
        "the PPU overwrote the game's DISPSTAT control bits (got {dispstat:04X})"
    );
}

/// DISPSTAT bit 2 is the VCounter match against bits 8-15, and a match with
/// bit 5 set raises IRQ 0x0004. It used to be hard-wired to the VBlank range,
/// so bit 2 read back as a second VBlank flag and the IRQ never fired.
#[test]
fn vcount_match_sets_bit_2_and_requests_its_interrupt() {
    let mut gba = Gba::new();
    gba.bus.write16(0x0400_0004, 0x2020); // VCount IRQ on, setting = 0x20
    gba.bus.write16(0x0400_0200, 0x0004); // IE: VCounter
    while gba.bus.read16(0x0400_0006) != 0x20 {
        gba.step();
    }
    assert_ne!(
        gba.bus.read16(0x0400_0004) & 0x0004,
        0,
        "VCounter match flag not set at VCOUNT == the DISPSTAT setting"
    );
    assert_ne!(
        gba.bus.read16(0x0400_0202) & 0x0004,
        0,
        "VCounter match did not request its interrupt"
    );
}

/// BLDCNT's alpha blend used to be an empty stub, and the whole colour-effect
/// path was gated on bit 5 - which selects the backdrop as a first target, not
/// whether an effect runs. A layer the game asked to be blended came out flat
/// and opaque: in Yggdra Union, a grey bar across the title screen.
#[test]
fn alpha_blend_mixes_the_top_layer_with_the_one_under_it() {
    let mut gba = Gba::new();
    gba.load_rom(&vec![0u8; 0x200]).expect("ROM should load");

    // Backdrop white, BG0's colour 1 pure red.
    gba.bus.write16(0x0500_0000, 0x7FFF);
    gba.bus.write16(0x0500_0002, 0x001F);
    // One tile of solid colour 1, and a map that uses it everywhere.
    for i in 0..32u32 {
        gba.bus.write8(0x0600_0000 + i, 0x11);
    }
    for i in 0..32u32 * 32 {
        gba.bus.write16(0x0600_F800 + i * 2, 0);
    }
    // BG0: char base 0, screen base 0x1F, 16-colour.
    gba.bus.write16(0x0400_0008, 0x1F00);
    gba.bus.write16(0x0400_0000, 0x0100); // mode 0, BG0 on

    // No effect yet: the pixel is the flat red.
    gba.run_frame();
    let flat = gba.frame_buffer()[0..3].to_vec();
    assert_eq!(flat, vec![0xF8, 0x00, 0x00], "BG0 should draw its own colour");

    // Alpha blend BG0 (1st target) over the backdrop (2nd target), half each.
    gba.bus.write16(0x0400_0050, 0x2041); // effect 1 (bits 6-7), 1st = BG0, 2nd = BD
    gba.bus.write16(0x0400_0052, 0x0808); // EVA = EVB = 8/16
    gba.run_frame();
    let blended = gba.frame_buffer()[0..3].to_vec();
    assert_ne!(blended, flat, "the blend never ran");
    assert!(
        blended[1] > 0x60 && blended[2] > 0x60,
        "the white backdrop should have bled into the red: {blended:02X?}"
    );
}

/// Mosaic is per layer: BGxCNT bit 6 enables it for that background. It used
/// to be read from DISPCNT bit 6, which is OBJ character mapping - so any game
/// using 1D sprite mapping, which is most of them, had every background
/// mosaiced the moment it set a non-zero MOSAIC for one layer.
#[test]
fn mosaic_only_applies_to_the_backgrounds_that_asked_for_it() {
    let mut gba = Gba::new();
    gba.load_rom(&vec![0u8; 0x200]).expect("ROM should load");

    // Two colours, and a tile whose left half is colour 1 and right half 2.
    gba.bus.write16(0x0500_0002, 0x001F); // red
    gba.bus.write16(0x0500_0004, 0x03E0); // green
    for row in 0..8u32 {
        // 4bpp: two pixels per byte, four bytes per row.
        gba.bus.write8(0x0600_0000 + row * 4, 0x11);
        gba.bus.write8(0x0600_0000 + row * 4 + 1, 0x11);
        gba.bus.write8(0x0600_0000 + row * 4 + 2, 0x22);
        gba.bus.write8(0x0600_0000 + row * 4 + 3, 0x22);
    }
    for i in 0..32u32 * 32 {
        gba.bus.write16(0x0600_F800 + i * 2, 0);
    }

    // BG0 at screen base 0x1F, mosaic *off*, and a large mosaic size set.
    gba.bus.write16(0x0400_0008, 0x1F00);
    gba.bus.write16(0x0400_004C, 0x0077); // 8x8 mosaic
    gba.bus.write16(0x0400_0000, 0x0140); // mode 0, BG0 on, OBJ 1D mapping

    gba.run_frame();
    // Pixel 4 is the second half of the tile: green, unless a mosaic it never
    // asked for smeared pixel 0's red across the whole 8-pixel block.
    let px = &gba.frame_buffer()[4 * 3..4 * 3 + 3];
    assert_eq!(
        px,
        [0x00, 0xF8, 0x00],
        "BG0 was mosaiced without BGxCNT bit 6 set"
    );
}

/// Sprite tile data starts at 0x06010000. The OBJ renderer read it from
/// 0x06000000 - BG VRAM - so every sprite was textured with whatever
/// background tiles happened to sit there. It is what turned Yggdra Union's
/// opening, which is drawn entirely with sprites, into bands of noise.
#[test]
fn sprites_read_their_tiles_from_obj_vram() {
    let mut gba = Gba::new();
    gba.load_rom(&vec![0u8; 0x200]).expect("ROM should load");

    // BG VRAM at tile 0: colour index 1 everywhere. OBJ VRAM at tile 0:
    // colour index 2. A sprite must pick up the second.
    // Byte writes to OBJ VRAM are ignored on hardware, so these go in as
    // halfwords.
    for i in (0..32u32).step_by(2) {
        gba.bus.write16(0x0600_0000 + i, 0x1111);
        gba.bus.write16(0x0601_0000 + i, 0x2222);
    }
    gba.bus.write16(0x0500_0202, 0x001F); // OBJ palette 0, colour 1: red
    gba.bus.write16(0x0500_0204, 0x03E0); // colour 2: green

    // Sprite 0: 8x8, 4bpp, at (0, 0), tile 0.
    gba.bus.write16(0x0700_0000, 0x0000);
    gba.bus.write16(0x0700_0002, 0x0000);
    gba.bus.write16(0x0700_0004, 0x0000);
    // Every other sprite off.
    for s in 1..128u32 {
        gba.bus.write16(0x0700_0000 + s * 8, 0x0200);
    }

    gba.bus.write16(0x0400_0000, 0x1040); // mode 0, OBJ on, 1D mapping
    gba.run_frame();

    let px = &gba.frame_buffer()[0..3];
    assert_eq!(
        px,
        [0x00, 0xF8, 0x00],
        "the sprite drew BG VRAM's colour instead of OBJ VRAM's"
    );
}

/// OBJ attribute 0 bit 13 is the colour depth. It used to be read from bit 7,
/// which belongs to the Y coordinate: a sprite at any Y of 128 or more was
/// silently treated as 256-colour.
#[test]
fn sprite_colour_depth_comes_from_attr0_bit_13() {
    let mut gba = Gba::new();
    gba.load_rom(&vec![0u8; 0x200]).expect("ROM should load");

    // 4bpp tile 0 in OBJ VRAM: colour index 1.
    for i in (0..32u32).step_by(2) {
        gba.bus.write16(0x0601_0000 + i, 0x1111);
    }
    gba.bus.write16(0x0500_0202, 0x001F); // 16-colour palette 0, colour 1

    // Sprite 0 at y = 0x80, which sets bit 7 of attr0 and used to be read as
    // "256 colours". 4bpp is what it actually is.
    gba.bus.write16(0x0700_0000, 0x0080);
    gba.bus.write16(0x0700_0002, 0x0000);
    gba.bus.write16(0x0700_0004, 0x0000);
    for s in 1..128u32 {
        gba.bus.write16(0x0700_0000 + s * 8, 0x0200);
    }

    gba.bus.write16(0x0400_0000, 0x1040);
    gba.run_frame();

    let idx = (128 * 240) * 3;
    let px = &gba.frame_buffer()[idx..idx + 3];
    assert_eq!(
        px,
        [0xF8, 0x00, 0x00],
        "a sprite at y=0x80 was decoded as 256-colour"
    );
}

/// Celeste Classic gets past its title into a level, which is a sprite-heavy
/// screen: the player, the snow and the particles are all OBJ. A blank or
/// single-colour frame here means the sprite path is broken again.
#[test]
fn celeste_renders_its_title_screen() {
    let Some(counts) = render("celeste") else {
        return;
    };
    assert!(
        counts.len() >= 4,
        "Celeste's title screen should have several colours, got {}",
        counts.len()
    );
}

/// The 240p Test Suite's front page is LZ77-compressed into VRAM. When the
/// decompressor wrote bytes instead of halfwords the text came out as noise,
/// which shows up here as a much shorter colour list.
#[test]
fn the_240p_suite_draws_its_front_page() {
    let Some(counts) = render("240p") else {
        return;
    };
    assert!(
        counts.len() >= 4,
        "the 240p suite's front page should have several colours, got {}",
        counts.len()
    );
}
