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
    let Some(counts) = render("stripes") else {
        return;
    };

    // The ROM writes 0x560B as the backdrop and 0x6290 as colour 1, then fills
    // alternating rows. Both must appear, and nothing else.
    let backdrop = rgb(0x560B);
    let stripe = rgb(0x6290);

    assert_eq!(
        counts.len(),
        2,
        "expected exactly two colours, got {counts:?}"
    );
    assert!(counts.contains_key(&backdrop), "backdrop 0x560B is missing");
    assert!(counts.contains_key(&stripe), "colour 1 (0x6290) is missing");
    assert_eq!(
        counts[&backdrop], counts[&stripe],
        "the stripes should split the screen evenly"
    );
}

#[test]
fn mode0_renders_a_full_palette_of_shades() {
    let Some(counts) = render("shades") else {
        return;
    };

    // The ROM steps the blue channel through sixteen values, so every colour on
    // screen must be pure blue and there must be many distinct ones. A single
    // colour here means the backgrounds are not drawing at all.
    assert!(counts.len() >= 15, "expected ~16 shades, got {counts:?}");
    for &(r, g, _b) in counts.keys() {
        assert_eq!(
            (r, g),
            (0, 0),
            "a shade of blue should have no red or green"
        );
    }
}

#[test]
fn text_renders_over_the_backdrop() {
    let Some(counts) = render("hello") else {
        return;
    };

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
    assert_eq!(
        flat,
        vec![0xF8, 0x00, 0x00],
        "BG0 should draw its own colour"
    );

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

/// Build a machine with BG0 drawing solid colour 1 everywhere and one 8x8
/// sprite at the origin drawing colour 1 from OBJ palette 0.
fn bg_and_sprite(bg_priority: u16, obj_priority: u16) -> Gba {
    let mut gba = Gba::new();
    gba.load_rom(&vec![0u8; 0x200]).expect("ROM should load");

    // BG tile 0: colour index 1. Sprite tile 0 in OBJ VRAM: colour index 1.
    for i in (0..32u32).step_by(2) {
        gba.bus.write16(0x0600_0000 + i, 0x1111);
        gba.bus.write16(0x0601_0000 + i, 0x1111);
    }
    for i in 0..32u32 * 32 {
        gba.bus.write16(0x0600_F800 + i * 2, 0);
    }
    gba.bus.write16(0x0500_0002, 0x001F); // BG palette 0, colour 1: red
    gba.bus.write16(0x0500_0202, 0x03E0); // OBJ palette 0, colour 1: green

    // BG0: screen base 0x1F, char base 0, 16-colour, given priority.
    gba.bus.write16(0x0400_0008, 0x1F00 | bg_priority);

    gba.bus.write16(0x0700_0000, 0x0000); // y = 0, 8x8, 4bpp
    gba.bus.write16(0x0700_0002, 0x0000); // x = 0
    gba.bus.write16(0x0700_0004, obj_priority << 10);
    for s in 1..128u32 {
        gba.bus.write16(0x0700_0000 + s * 8, 0x0200); // disabled
    }

    gba.bus.write16(0x0400_0000, 0x1140); // mode 0, BG0 + OBJ, 1D mapping
    gba
}

/// OAM attribute 2 bits 10-11 are the sprite's priority, and they were never
/// read: sprites were composited before the backgrounds, so any opaque
/// background pixel covered them whatever their priority said.
#[test]
fn a_low_priority_sprite_goes_behind_the_background() {
    let mut gba = bg_and_sprite(0, 3);
    gba.run_frame();
    assert_eq!(
        &gba.frame_buffer()[0..3],
        [0xF8, 0x00, 0x00],
        "a priority-3 sprite must not cover a priority-0 background"
    );
}

#[test]
fn a_high_priority_sprite_goes_in_front_of_the_background() {
    let mut gba = bg_and_sprite(3, 0);
    gba.run_frame();
    assert_eq!(
        &gba.frame_buffer()[0..3],
        [0x00, 0xF8, 0x00],
        "a priority-0 sprite must cover a priority-3 background"
    );
}

/// GBATEK: sprites are drawn in front of a background of the *same* priority.
#[test]
fn a_sprite_wins_a_priority_tie_with_a_background() {
    let mut gba = bg_and_sprite(1, 1);
    gba.run_frame();
    assert_eq!(
        &gba.frame_buffer()[0..3],
        [0x00, 0xF8, 0x00],
        "an equal-priority sprite must be in front of the background"
    );
}

/// Between two overlapping sprites the OAM index decides on its own: a
/// sprite's priority field is only ever compared against the backgrounds.
/// GBATEK's "Caution" example under OAM Attributes spells this out, and it was
/// confirmed on hardware in VisualBoyAdvance bug #130 - so sprite 0 wins even
/// when sprite 1 carries the better priority.
#[test]
fn between_two_sprites_the_oam_index_decides_not_the_priority() {
    let mut gba = Gba::new();
    gba.load_rom(&vec![0u8; 0x200]).expect("ROM should load");

    // Two OBJ tiles: tile 0 is colour 1, tile 1 is colour 2.
    for i in (0..32u32).step_by(2) {
        gba.bus.write16(0x0601_0000 + i, 0x1111);
        gba.bus.write16(0x0601_0020 + i, 0x2222);
    }
    gba.bus.write16(0x0500_0202, 0x001F); // colour 1: red
    gba.bus.write16(0x0500_0204, 0x03E0); // colour 2: green

    // Sprite 0 uses tile 0 and the *worse* priority; sprite 1 uses tile 1 and
    // the better one. They sit on top of each other.
    gba.bus.write16(0x0700_0000, 0x0000);
    gba.bus.write16(0x0700_0002, 0x0000);
    gba.bus.write16(0x0700_0004, (3 << 10) | 0);
    gba.bus.write16(0x0700_0008, 0x0000);
    gba.bus.write16(0x0700_000A, 0x0000);
    gba.bus.write16(0x0700_000C, (0 << 10) | 1);
    for s in 2..128u32 {
        gba.bus.write16(0x0700_0000 + s * 8, 0x0200);
    }

    gba.bus.write16(0x0400_0000, 0x1040); // mode 0, OBJ on, 1D mapping
    gba.run_frame();

    assert_eq!(
        &gba.frame_buffer()[0..3],
        [0xF8, 0x00, 0x00],
        "sprite 0 must win the overlap even though sprite 1 has a better priority"
    );
}

/// Affine backgrounds are always 256-colour and their map entry is a single
/// byte holding the tile number. The renderer used to take the map *index* as
/// the tile number and multiply it by 8 - never reading the map at all - so
/// the layer came out as a linear walk through character memory. It also
/// skipped the layer entirely unless BGxCNT bit 7 was set, which means nothing
/// for an affine background.
#[test]
fn an_affine_background_reads_its_tilemap() {
    let mut gba = Gba::new();
    gba.load_rom(&vec![0u8; 0x200]).expect("ROM should load");

    // Tile 5 is solid colour index 3; every other tile is blank.
    for i in (0..64u32).step_by(2) {
        gba.bus.write16(0x0600_0000 + 5 * 64 + i, 0x0303);
    }
    gba.bus.write16(0x0500_0006, 0x001F); // colour 3: red

    // A 16x16-tile map at screen base 0x10, every entry pointing at tile 5.
    for i in (0..16 * 16u32).step_by(2) {
        gba.bus.write16(0x0600_8000 + i, 0x0505);
    }

    // BG2: char base 0, screen base 0x10, size 0 (16x16 tiles), wraparound on.
    gba.bus.write16(0x0400_000C, 0x2000 | (0x10 << 8));
    // Identity matrix, reference point at the origin.
    gba.bus.write16(0x0400_0020, 0x0100); // PA = 1.0
    gba.bus.write16(0x0400_0022, 0x0000);
    gba.bus.write16(0x0400_0024, 0x0000);
    gba.bus.write16(0x0400_0026, 0x0100); // PD = 1.0
    gba.bus.write32(0x0400_0028, 0);
    gba.bus.write32(0x0400_002C, 0);

    gba.bus.write16(0x0400_0000, 0x0402); // mode 2, BG2 on
    gba.run_frame();

    assert_eq!(
        &gba.frame_buffer()[0..3],
        [0xF8, 0x00, 0x00],
        "the affine background did not read tile 5 out of its map"
    );
}

/// BGxCNT bit 13 is Display Area Overflow: clear means the area outside the
/// map is transparent, set means it wraps. The renderer used to wrap
/// unconditionally.
#[test]
fn an_affine_background_without_overflow_is_transparent_outside_its_map() {
    let mut gba = Gba::new();
    gba.load_rom(&vec![0u8; 0x200]).expect("ROM should load");

    for i in (0..64u32).step_by(2) {
        gba.bus.write16(0x0600_0000 + 5 * 64 + i, 0x0303);
    }
    gba.bus.write16(0x0500_0000, 0x03E0); // backdrop: green
    gba.bus.write16(0x0500_0006, 0x001F); // colour 3: red
    for i in (0..16 * 16u32).step_by(2) {
        gba.bus.write16(0x0600_8000 + i, 0x0505);
    }

    // Same as above but with overflow off, and the reference point pushed one
    // whole map (128 pixels) to the right so screen x=0 lands outside it.
    gba.bus.write16(0x0400_000C, 0x10 << 8);
    gba.bus.write16(0x0400_0020, 0x0100);
    gba.bus.write16(0x0400_0022, 0x0000);
    gba.bus.write16(0x0400_0024, 0x0000);
    gba.bus.write16(0x0400_0026, 0x0100);
    gba.bus.write32(0x0400_0028, 128 << 8);
    gba.bus.write32(0x0400_002C, 0);

    gba.bus.write16(0x0400_0000, 0x0402);
    gba.run_frame();

    assert_eq!(
        &gba.frame_buffer()[0..3],
        [0x00, 0xF8, 0x00],
        "outside the map with overflow off the backdrop must show through"
    );
}

/// Build a machine with BG0 drawing solid red everywhere over a green
/// backdrop, ready for a window to cut a hole in it.
fn bg_over_backdrop() -> Gba {
    let mut gba = Gba::new();
    gba.load_rom(&vec![0u8; 0x200]).expect("ROM should load");
    for i in (0..32u32).step_by(2) {
        gba.bus.write16(0x0600_0000 + i, 0x1111);
    }
    for i in 0..32u32 * 32 {
        gba.bus.write16(0x0600_F800 + i * 2, 0);
    }
    gba.bus.write16(0x0500_0000, 0x03E0); // backdrop: green
    gba.bus.write16(0x0500_0002, 0x001F); // BG colour 1: red
    gba.bus.write16(0x0400_0008, 0x1F00); // BG0: screen base 0x1F
    gba
}

/// DISPCNT bit 15 is the OBJ Window enable. It used to be read as a
/// "display off" flag, so any game that turned the OBJ window on had its
/// entire screen blanked - which is why `fantasy-knight.gba` was black.
#[test]
fn dispcnt_bit_15_is_the_obj_window_not_a_display_off() {
    let mut gba = bg_over_backdrop();
    gba.bus.write16(0x0400_004A, 0x3F3F); // everything visible in/out
    gba.bus.write16(0x0400_0000, 0x8100); // mode 0, BG0 on, OBJ window on
    gba.run_frame();
    assert_eq!(
        &gba.frame_buffer()[0..3],
        [0xF8, 0x00, 0x00],
        "enabling the OBJ window blanked the screen"
    );
}

/// WININ and WINOUT decide which layers show inside and outside a window. The
/// whole windowing path used to be dead code: `render_scanline` computed its
/// flags and discarded them.
#[test]
fn a_window_hides_the_layers_winout_leaves_out() {
    let mut gba = bg_over_backdrop();
    // WIN0 covers x 0..64, y 0..32. GBATEK: X1/Y1 - the left and top edges -
    // live in bits 8-15, so the *high* byte is the near edge.
    gba.bus.write16(0x0400_0040, (0 << 8) | 64);
    gba.bus.write16(0x0400_0044, (0 << 8) | 32);
    gba.bus.write16(0x0400_0048, 0x0001); // inside WIN0: BG0 only
    gba.bus.write16(0x0400_004A, 0x0000); // outside: nothing
    gba.bus.write16(0x0400_0000, 0x2100); // mode 0, BG0 on, WIN0 on
    gba.run_frame();

    let at = |x: usize, y: usize| {
        let i = (y * 240 + x) * 3;
        gba.frame_buffer()[i..i + 3].to_vec()
    };
    assert_eq!(
        at(10, 10),
        vec![0xF8, 0x00, 0x00],
        "inside the window BG0 shows"
    );
    assert_eq!(
        at(100, 10),
        vec![0x00, 0xF8, 0x00],
        "right of the window it does not"
    );
    assert_eq!(
        at(10, 100),
        vec![0x00, 0xF8, 0x00],
        "below the window it does not"
    );
}

/// WIN0 outranks WIN1 where they overlap.
#[test]
fn win0_takes_precedence_over_win1() {
    let mut gba = bg_over_backdrop();
    gba.bus.write16(0x0400_0040, (0 << 8) | 64); // WIN0: x 0..64
    gba.bus.write16(0x0400_0044, (0 << 8) | 64); // WIN0: y 0..64
    gba.bus.write16(0x0400_0042, (0 << 8) | 240); // WIN1: the whole screen
    gba.bus.write16(0x0400_0046, (0 << 8) | 160);
    gba.bus.write16(0x0400_0048, 0x0100); // WIN0: nothing, WIN1: BG0
    gba.bus.write16(0x0400_004A, 0x0000);
    gba.bus.write16(0x0400_0000, 0x6100); // BG0 on, WIN0 + WIN1 on
    gba.run_frame();

    let at = |x: usize, y: usize| {
        let i = (y * 240 + x) * 3;
        gba.frame_buffer()[i..i + 3].to_vec()
    };
    assert_eq!(
        at(10, 10),
        vec![0x00, 0xF8, 0x00],
        "WIN0 must win the overlap"
    );
    assert_eq!(at(100, 100), vec![0xF8, 0x00, 0x00], "WIN1 alone shows BG0");
}

/// GBATEK, Color Special Effects: "OBJs defined as Semi-Transparent in OAM
/// memory are always selected as 1st Target (regardless of BLDCNT Bit 4), and
/// are always using Alpha Blending mode (regardless of BLDCNT Bit 6-7)." Only
/// the 2nd-target bits still matter, and a layer that is not one means the
/// sprite is drawn unblended.
#[test]
fn a_semi_transparent_sprite_blends_without_bldcnt_asking() {
    let mut gba = bg_over_backdrop();
    for i in (0..32u32).step_by(2) {
        gba.bus.write16(0x0601_0000 + i, 0x1111);
    }
    gba.bus.write16(0x0500_0202, 0x7C00); // OBJ colour 1: blue

    // Sprite 0, 8x8 at the origin, OBJ mode 1 (semi-transparent).
    gba.bus.write16(0x0700_0000, 0x0400);
    gba.bus.write16(0x0700_0002, 0x0000);
    gba.bus.write16(0x0700_0004, 0x0000);
    for s in 1..128u32 {
        gba.bus.write16(0x0700_0000 + s * 8, 0x0200);
    }

    // BLDCNT selects no first target and no effect at all - only BG0 as a
    // second target. The sprite must still blend with the red background.
    gba.bus.write16(0x0400_0050, 0x0100);
    gba.bus.write16(0x0400_0052, 0x0808); // half and half
    gba.bus.write16(0x0400_0000, 0x1100); // mode 0, BG0 + OBJ

    gba.run_frame();
    let px = &gba.frame_buffer()[0..3];
    assert!(
        px[0] > 0x40 && px[2] > 0x40,
        "expected a mix of the red background and the blue sprite, got {px:02X?}"
    );
}

/// Outside mode 0, colour effects used to run as a scanline-wide
/// post-process with no idea which layer produced each pixel: brighten/darken
/// applied to every pixel on the line whenever BLDCNT selected the effect at
/// all, with no regard for BLDCNT's 1st-target-select bits. A game pulsing
/// BLDY to highlight one layer flashed the whole screen instead - reported
/// from a device as Yggdra Union's dialog panels strobing between normal and
/// washed-out every other frame.
#[test]
fn brighten_in_a_bitmap_mode_only_affects_the_selected_1st_target_layer() {
    let mut gba = Gba::new();
    gba.load_rom(&vec![0u8; 0x200]).expect("ROM should load");

    // BG2's mid-grey, at bitmap pixel (0, 0).
    gba.bus.write16(0x0500_0002, 0x4210);
    gba.bus.write8(0x0600_0000, 1);

    // An 8x8 sprite at x=8 using the same mid-grey, so it does not overlap
    // the bitmap pixel above. Bitmap modes reserve OBJ tiles below 512 for
    // the frame buffer, so this one has to live at tile 512 (0x06014000).
    gba.bus.write16(0x0500_0202, 0x4210);
    for i in (0..32u32).step_by(2) {
        gba.bus.write16(0x0601_4000 + i, 0x1111);
    }
    gba.bus.write16(0x0700_0000, 0x0000);
    gba.bus.write16(0x0700_0002, 0x0008);
    gba.bus.write16(0x0700_0004, 0x0200); // tile 512
    for s in 1..128u32 {
        gba.bus.write16(0x0700_0000 + s * 8, 0x0200); // disabled
    }

    // Mode 4, BG2 + OBJ on. Brighten (effect 2), BG2 the only 1st target.
    gba.bus.write16(0x0400_0000, 0x1404);
    gba.bus.write16(0x0400_0050, 0x0084); // effect 2, 1st target BG2 (bit 2)
    gba.bus.write16(0x0400_0054, 16); // EVY = 16 (max)

    gba.run_frame();
    let fb = gba.frame_buffer();
    let bg2_px = &fb[0..3];
    let obj_px = &fb[8 * 3..8 * 3 + 3];

    assert_eq!(
        bg2_px,
        [0xFF, 0xFF, 0xFF],
        "BG2, the selected 1st target, should be brightened to white: {bg2_px:02X?}"
    );
    assert_eq!(
        obj_px,
        [0x80, 0x80, 0x80],
        "OBJ is not a 1st target and must be left at its original grey: {obj_px:02X?}"
    );
}

/// The same sprite over a layer that is *not* a selected 2nd target is drawn
/// at full strength.
#[test]
fn a_semi_transparent_sprite_over_a_non_target_is_not_blended() {
    let mut gba = bg_over_backdrop();
    for i in (0..32u32).step_by(2) {
        gba.bus.write16(0x0601_0000 + i, 0x1111);
    }
    gba.bus.write16(0x0500_0202, 0x7C00);
    gba.bus.write16(0x0700_0000, 0x0400);
    gba.bus.write16(0x0700_0002, 0x0000);
    gba.bus.write16(0x0700_0004, 0x0000);
    for s in 1..128u32 {
        gba.bus.write16(0x0700_0000 + s * 8, 0x0200);
    }

    gba.bus.write16(0x0400_0050, 0x0000); // nothing is a second target
    gba.bus.write16(0x0400_0052, 0x0808);
    gba.bus.write16(0x0400_0000, 0x1100);

    gba.run_frame();
    assert_eq!(
        &gba.frame_buffer()[0..3],
        [0x00, 0x00, 0xF8],
        "with no second target the sprite must be drawn unblended"
    );
}

/// BLDY is a 5-bit field but EVY saturates at 16. Darkening with an
/// out-of-range BLDY used to underflow a u32 subtraction and panic, taking the
/// whole process down through JNI.
#[test]
fn a_bldy_above_16_darkens_to_black_instead_of_panicking() {
    let mut gba = Gba::new();
    gba.load_rom(&vec![0u8; 0x200]).expect("ROM should load");
    gba.bus.write16(0x0500_0002, 0x4210); // BG2 colour 1: mid-grey
    gba.bus.write8(0x0600_0000, 1);
    gba.bus.write16(0x0400_0000, 0x0404); // mode 4, BG2 on
    gba.bus.write16(0x0400_0050, 0x00C4); // effect 3 (darken), 1st target BG2
    gba.bus.write16(0x0400_0054, 0x001F); // BLDY = 31, above the EVY cap

    gba.run_frame();
    assert_eq!(
        &gba.frame_buffer()[0..3],
        [0x00, 0x00, 0x00],
        "EVY caps at 16, so a full darken must reach black"
    );
}

/// The same, brightening: EVY over the cap used to over-brighten rather than
/// saturating at the documented maximum.
#[test]
fn a_bldy_above_16_brightens_to_white() {
    let mut gba = Gba::new();
    gba.load_rom(&vec![0u8; 0x200]).expect("ROM should load");
    gba.bus.write16(0x0500_0002, 0x4210);
    gba.bus.write8(0x0600_0000, 1);
    gba.bus.write16(0x0400_0000, 0x0404);
    gba.bus.write16(0x0400_0050, 0x0084); // effect 2 (brighten), 1st target BG2
    gba.bus.write16(0x0400_0054, 0x001F);

    gba.run_frame();
    assert_eq!(&gba.frame_buffer()[0..3], [0xFF, 0xFF, 0xFF]);
}

/// Alpha blending outside mode 0 was a stub: `apply_alpha_blend` computed EVA
/// and EVB and then threw them away, so a translucent layer in any affine or
/// bitmap mode drew flat and opaque.
#[test]
fn alpha_blending_works_in_a_bitmap_mode() {
    let mut gba = Gba::new();
    gba.load_rom(&vec![0u8; 0x200]).expect("ROM should load");
    gba.bus.write16(0x0500_0000, 0x001F); // backdrop: red
    gba.bus.write16(0x0500_0002, 0x7C00); // BG2 colour 1: blue
    gba.bus.write8(0x0600_0000, 1);
    gba.bus.write16(0x0400_0000, 0x0404); // mode 4, BG2 on
                                          // Effect 1 (alpha), 1st target BG2 (bit 2), 2nd target backdrop (bit 13).
    gba.bus.write16(0x0400_0050, 0x2044);
    gba.bus.write16(0x0400_0052, 0x0808); // EVA = EVB = 8, an even mix

    gba.run_frame();
    let px = &gba.frame_buffer()[0..3];
    assert_eq!(
        px,
        [0x7C, 0x00, 0x7C],
        "half blue over half red should come out purple, not flat blue: {px:02X?}"
    );
}

/// OBJ mosaic is per sprite: OAM attr0 bit 12 turns it on, and MOSAIC bits
/// 8-15 give the block size. `mosaic_obj` existed but had no callers, and was
/// gated on a field `sync_from_bus` hardcoded to true, so sprite mosaic was a
/// silent no-op.
#[test]
fn obj_mosaic_only_applies_to_the_sprites_that_asked_for_it() {
    /// One 8x8 sprite at (0, 0) whose first dot is red and whose remaining
    /// seven are green. A 4-wide mosaic makes dots 1-3 sample the red one, so
    /// the red block widens - something no tile boundary can imitate.
    fn first_row(mosaic: bool) -> Vec<(u8, u8, u8)> {
        let mut gba = Gba::new();
        gba.load_rom(&vec![0u8; 0x200]).expect("ROM should load");

        // OBJ palette: 1 = red, 2 = green.
        gba.bus.write16(0x0500_0202, 0x001F);
        gba.bus.write16(0x0500_0204, 0x03E0);

        // 4bpp: one byte holds two dots, low nibble first. Dot 0 = colour 1,
        // dots 1-7 = colour 2. write16, since a byte store into VRAM is
        // duplicated across the halfword rather than doing what it looks like.
        for row in 0..8u32 {
            let base = 0x0601_0000 + row * 4;
            gba.bus.write16(base, 0x2221);
            gba.bus.write16(base + 2, 0x2222);
        }

        // attr0: y=0, normal, 4bpp, square; bit 12 is the mosaic enable.
        gba.bus
            .write16(0x0700_0000, if mosaic { 0x1000 } else { 0x0000 });
        gba.bus.write16(0x0700_0002, 0x0000); // attr1: x=0, size 0 -> 8x8
        gba.bus.write16(0x0700_0004, 0x0000); // attr2: tile 0, palette 0
        for s in 1..128u32 {
            gba.bus.write16(0x0700_0000 + s * 8, 0x0200); // disabled
        }

        // MOSAIC: OBJ H-size 3 -> blocks of 4. Deliberately not 8, so the
        // block cannot coincide with a tile edge.
        gba.bus.write16(0x0400_004C, 0x0300);
        gba.bus.write16(0x0400_0000, 0x1040); // mode 0, OBJ on, 1D mapping

        gba.run_frame();
        let fb = gba.frame_buffer();
        (0..8)
            .map(|x| {
                let i = x * 3;
                (fb[i], fb[i + 1], fb[i + 2])
            })
            .collect()
    }

    let red = (0xF8, 0x00, 0x00);
    let green = (0x00, 0xF8, 0x00);

    let plain = first_row(false);
    assert_eq!(plain[0], red, "dot 0 should be red: {plain:02X?}");
    assert_eq!(plain[1], green, "dot 1 should be green without mosaic");

    let mosaiced = first_row(true);
    assert_eq!(
        mosaiced[0], red,
        "the block's own dot is unchanged: {mosaiced:02X?}"
    );
    assert_eq!(
        mosaiced[1], red,
        "a 4-wide mosaic makes dot 1 sample dot 0, so it must be red too"
    );
    assert_eq!(mosaiced[3], red, "still inside the first 4-wide block");
    assert_eq!(
        mosaiced[4], green,
        "the next block samples dot 4, which is green"
    );
}
