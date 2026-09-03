const SCREEN_WIDTH: usize = 240;
const SCREEN_HEIGHT: usize = 160;
const FRAME_SIZE: usize = SCREEN_WIDTH * SCREEN_HEIGHT * 3;

/// BLDCNT's target bits are ordered BG0, BG1, BG2, BG3, OBJ, BD.
const LAYER_OBJ: usize = 4;
const LAYER_BD: usize = 5;

pub struct Ppu {
    frame_buffer: [u8; FRAME_SIZE],
    pub scanline: u16,
    pub mode: u8,
    pub vblank: bool,
    pub hblank: bool,
    vblank_irq_pending: bool,
    hblank_irq_pending: bool,
    vcount_irq_pending: bool,
    vcount_match: bool,
    /// The sprite pixel on the current scanline, as (colour, priority), and
    /// `None` where no sprite drew. Per-scanline scratch, so it is
    /// deliberately not part of a save state.
    ///
    /// Sprites used to be painted straight into the frame buffer before the
    /// backgrounds, which meant any opaque background pixel covered them
    /// whatever their priority said. Holding them here instead lets the
    /// compositor order them against the backgrounds properly.
    /// Colour, priority, and whether the sprite was semi-transparent.
    obj_pixel: [Option<((u8, u8, u8), u8, bool)>; SCREEN_WIDTH],
    /// Pixels covered by an OBJ-window sprite (OAM mode 2). Those sprites are
    /// never drawn; their non-transparent dots shape a window instead.
    obj_window: [bool; SCREEN_WIDTH],
    /// Which layers each pixel of this scanline may show, from WININ/WINOUT.
    window: [u8; SCREEN_WIDTH],
    /// Which layer wrote the pixel currently in `frame_buffer` at each x of
    /// this scanline, one of the `LAYER_*` constants (or a BG index 0-3).
    /// Only modes 1-5 use this - mode 0 already knows the top layer per pixel
    /// from its own compositing stack and gates `blend()` on it directly.
    /// Without this, `apply_brightness_inc`/`apply_brightness_dec` brightened
    /// or darkened the whole scanline whenever BLDCNT selected the effect,
    /// with no regard for which layers BLDCNT actually marked as a 1st
    /// target - so a game pulsing BLDY to highlight one layer flashed the
    /// entire screen instead.
    scanline_layer: [usize; SCREEN_WIDTH],
    /// The pixel each `scanline_layer` entry was drawn *over*, and which layer
    /// that was. Modes 1-5 composite by overdraw rather than by sorting, so
    /// this is the only record of the 2nd blend target once the top pixel has
    /// been written; without it `apply_alpha_blend` had nothing to mix with.
    scanline_second: [(u8, u8, u8); SCREEN_WIDTH],
    scanline_second_layer: [usize; SCREEN_WIDTH],
    cycle_counter: u32,
    pub bg_mode: u8,
    // Display control
    pub dispcnt: u16,
    force_blank: bool,
    bg0_enable: bool,
    bg1_enable: bool,
    bg2_enable: bool,
    bg3_enable: bool,
    obj_enable: bool,
    // DISPSTAT IRQ enable bits (read from game writes to DISPSTAT)
    dispstat_vblank_ie: bool,
    dispstat_hblank_ie: bool,
    dispstat_vcount_ie: bool,
    // BG control registers
    pub bg0cnt: u16,
    pub bg1cnt: u16,
    pub bg2cnt: u16,
    pub bg3cnt: u16,
    // BG scroll registers
    pub bg0hofs: u16,
    pub bg0vofs: u16,
    pub bg1hofs: u16,
    pub bg1vofs: u16,
    pub bg2hofs: u16,
    pub bg2vofs: u16,
    pub bg3hofs: u16,
    pub bg3vofs: u16,
    // Affine BG reference point registers
    pub bg2x: i32,
    pub bg2y: i32,
    pub bg3x: i32,
    pub bg3y: i32,
    // Affine BG rotation/scaling parameters
    pub bg2pa: i16,
    pub bg2pb: i16,
    pub bg2pc: i16,
    pub bg2pd: i16,
    pub bg3pa: i16,
    pub bg3pb: i16,
    pub bg3pc: i16,
    pub bg3pd: i16,
    // Color effects
    pub bldcnt: u16,
    pub bldalpha: u16,
    pub bldy: u8,
    // Mosaic
    mosaic_bg_hsize: u8,
    mosaic_bg_vsize: u8,
    mosaic_obj_hsize: u8,
    mosaic_obj_vsize: u8,
    mosaic_obj_enabled: bool,
    // Windows
    win0h_left: u8,
    win0h_right: u8,
    win0v_top: u8,
    win0v_bottom: u8,
    win1h_left: u8,
    win1h_right: u8,
    win1v_top: u8,
    win1v_bottom: u8,
    winin: u16,
    winout: u16,
    // Internal affine reference point latches
    bg2x_internal: i32,
    bg2y_internal: i32,
    bg3x_internal: i32,
    bg3y_internal: i32,
}

impl Ppu {
    pub fn new() -> Self {
        Self {
            frame_buffer: [0; FRAME_SIZE],
            scanline: 0,
            mode: 0,
            vblank: false,
            hblank: false,
            vblank_irq_pending: false,
            hblank_irq_pending: false,
            vcount_irq_pending: false,
            vcount_match: false,
            obj_pixel: [None; SCREEN_WIDTH],
            obj_window: [false; SCREEN_WIDTH],
            window: [0x3F; SCREEN_WIDTH],
            scanline_layer: [LAYER_BD; SCREEN_WIDTH],
            scanline_second: [(0, 0, 0); SCREEN_WIDTH],
            scanline_second_layer: [LAYER_BD; SCREEN_WIDTH],
            cycle_counter: 0,
            dispcnt: 0,
            force_blank: false,
            bg_mode: 0,
            dispstat_vblank_ie: false,
            dispstat_hblank_ie: false,
            dispstat_vcount_ie: false,
            bg0_enable: false,
            bg1_enable: false,
            bg2_enable: false,
            bg3_enable: false,
            obj_enable: false,
            bg0cnt: 0,
            bg1cnt: 0,
            bg2cnt: 0,
            bg3cnt: 0,
            bg0hofs: 0,
            bg0vofs: 0,
            bg1hofs: 0,
            bg1vofs: 0,
            bg2hofs: 0,
            bg2vofs: 0,
            bg3hofs: 0,
            bg3vofs: 0,
            bg2x: 0,
            bg2y: 0,
            bg3x: 0,
            bg3y: 0,
            bg2pa: 0,
            bg2pb: 0,
            bg2pc: 0,
            bg2pd: 0,
            bg3pa: 0,
            bg3pb: 0,
            bg3pc: 0,
            bg3pd: 0,
            bldcnt: 0,
            bldalpha: 0,
            bldy: 0,
            mosaic_bg_hsize: 0,
            mosaic_bg_vsize: 0,
            mosaic_obj_hsize: 0,
            mosaic_obj_vsize: 0,
            mosaic_obj_enabled: false,
            win0h_left: 0,
            win0h_right: 0,
            win0v_top: 0,
            win0v_bottom: 0,
            win1h_left: 0,
            win1h_right: 0,
            win1v_top: 0,
            win1v_bottom: 0,
            winin: 0,
            winout: 0,
            bg2x_internal: 0,
            bg2y_internal: 0,
            bg3x_internal: 0,
            bg3y_internal: 0,
        }
    }

    pub fn tick(
        &mut self,
        cycles: u32,
        bus: &mut super::memory::MemoryBus,
        dma: &mut super::dma::Dma,
    ) {
        self.cycle_counter += cycles;

        // GBA timing: 1232 cycles per scanline
        if self.cycle_counter >= 1232 {
            self.cycle_counter -= 1232;
            self.sync_from_bus(bus);
            self.render_scanline(bus);
            // The affine reference point is *accumulated* down the frame: each
            // visible line adds PB/PD to it. Reloading it from BGxX/BGxY every
            // line, as this used to, throws PB and PD away entirely, so a
            // rotated background has no vertical component and a scrolling one
            // never moves.
            if self.scanline < 160 {
                self.bg2x_internal = self.bg2x_internal.wrapping_add(self.bg2pb as i32);
                self.bg2y_internal = self.bg2y_internal.wrapping_add(self.bg2pd as i32);
                self.bg3x_internal = self.bg3x_internal.wrapping_add(self.bg3pb as i32);
                self.bg3y_internal = self.bg3y_internal.wrapping_add(self.bg3pd as i32);
            }
            self.scanline += 1;

            // VBlank start
            if self.scanline == 160 && !self.vblank {
                self.vblank = true;
                if self.dispstat_vblank_ie {
                    self.vblank_irq_pending = true;
                }
                dma.on_vblank(bus);
            }

            if self.scanline >= 228 {
                self.scanline = 0;
                self.vblank = false;
                dma.on_vcounter(bus);
            }

            // A new frame reloads the reference point from the registers.
            if self.scanline == 0 {
                self.reload_affine_reference();
            }
        }

        // HBlank start at cycle 960
        let new_hblank = self.cycle_counter >= 960;
        if new_hblank && !self.hblank {
            self.hblank = true;
            if self.dispstat_hblank_ie {
                self.hblank_irq_pending = true;
            }
            // GBATEK, GBA DMA Transfers: HBlank DMA is not performed during
            // VBlank. Without this an enabled repeating HBlank channel fires
            // 68 extra times a frame and runs off the end of its buffer.
            if self.scanline < 160 {
                dma.on_hblank(bus);
            }
        } else if !new_hblank {
            self.hblank = false;
        }

        // DISPSTAT bits 3-15 belong to the game: the three IRQ enables and the
        // VCount setting it compares against. They have to be re-read here,
        // every tick, and written straight back. Caching them once per
        // scanline and composing the register from the cache silently erased
        // any DISPSTAT write the game made mid-scanline - which is how Yggdra
        // Union enabled the VBlank IRQ and then waited forever for it.
        let game_bits = bus.read16(0x0400_0004) & 0xFF38;
        self.dispstat_vblank_ie = game_bits & 0x0008 != 0;
        self.dispstat_hblank_ie = game_bits & 0x0010 != 0;
        self.dispstat_vcount_ie = game_bits & 0x0020 != 0;

        // Bit 2 is the VCounter match against DISPSTAT bits 8-15, not a
        // second VBlank flag.
        let vcount_match = self.scanline == (game_bits >> 8);
        if vcount_match && !self.vcount_match && self.dispstat_vcount_ie {
            self.vcount_irq_pending = true;
        }
        self.vcount_match = vcount_match;

        let dispstat = game_bits
            | (self.vblank as u16)
            | ((self.hblank as u16) << 1)
            | ((vcount_match as u16) << 2);
        bus.write16(0x0400_0004, dispstat);

        // Write VCOUNT
        bus.write16(0x0400_0006, self.scanline);
    }

    fn sync_from_bus(&mut self, bus: &mut super::memory::MemoryBus) {
        self.dispcnt = bus.read16(0x0400_0000);
        // Bit 7 is the only blanking control DISPCNT has. There used also to
        // be a `display_off` read from bit 15 - which is the OBJ Window
        // enable, not a blank - so any game that turned the OBJ window on had
        // its entire screen blacked out.
        self.force_blank = self.dispcnt & 0x0080 != 0;
        self.bg_mode = (self.dispcnt & 0x0007) as u8;
        self.bg0_enable = self.dispcnt & 0x0100 != 0;
        self.bg1_enable = self.dispcnt & 0x0200 != 0;
        self.bg2_enable = self.dispcnt & 0x0400 != 0;
        self.bg3_enable = self.dispcnt & 0x0800 != 0;
        self.obj_enable = self.dispcnt & 0x1000 != 0;

        // BG control registers
        self.bg0cnt = bus.read16(0x0400_0008);
        self.bg1cnt = bus.read16(0x0400_000A);
        self.bg2cnt = bus.read16(0x0400_000C);
        self.bg3cnt = bus.read16(0x0400_000E);

        // BG scroll registers
        self.bg0hofs = bus.read16(0x0400_0010);
        self.bg0vofs = bus.read16(0x0400_0012);
        self.bg1hofs = bus.read16(0x0400_0014);
        self.bg1vofs = bus.read16(0x0400_0016);
        self.bg2hofs = bus.read16(0x0400_0018);
        self.bg2vofs = bus.read16(0x0400_001A);
        self.bg3hofs = bus.read16(0x0400_001C);
        self.bg3vofs = bus.read16(0x0400_001E);

        // Affine reference points. GBATEK, LCD I/O BG Rotation/Scaling: these
        // are 28 bits - 19 integer, 8 fractional, sign in bit 27. Taking all
        // 32 bits left the sign unextended, so any negative reference point
        // came out as a huge positive one.
        let signed28 = |lo: u16, hi: u16| -> i32 {
            let raw = ((hi as u32) << 16) | lo as u32;
            ((raw << 4) as i32) >> 4
        };
        let (bg2x, bg2y) = (
            signed28(bus.read16(0x0400_0028), bus.read16(0x0400_002A)),
            signed28(bus.read16(0x0400_002C), bus.read16(0x0400_002E)),
        );
        let (bg3x, bg3y) = (
            signed28(bus.read16(0x0400_0038), bus.read16(0x0400_003A)),
            signed28(bus.read16(0x0400_003C), bus.read16(0x0400_003E)),
        );
        // Writing BGxX or BGxY mid-frame reloads the internal accumulator on
        // hardware, which is how a game restarts an effect part-way down the
        // screen.
        let changed = (bg2x, bg2y, bg3x, bg3y) != (self.bg2x, self.bg2y, self.bg3x, self.bg3y);
        self.bg2x = bg2x;
        self.bg2y = bg2y;
        self.bg3x = bg3x;
        self.bg3y = bg3y;
        if changed {
            self.reload_affine_reference();
        }

        // Affine parameters
        self.bg2pa = bus.read16(0x0400_0020) as i16;
        self.bg2pb = bus.read16(0x0400_0022) as i16;
        self.bg2pc = bus.read16(0x0400_0024) as i16;
        self.bg2pd = bus.read16(0x0400_0026) as i16;
        self.bg3pa = bus.read16(0x0400_0030) as i16;
        self.bg3pb = bus.read16(0x0400_0032) as u16 as i16;
        self.bg3pc = bus.read16(0x0400_0034) as i16;
        self.bg3pd = bus.read16(0x0400_0036) as i16;

        // Color effects
        self.bldcnt = bus.read16(0x0400_0050);
        self.bldalpha = bus.read16(0x0400_0052);
        self.bldy = (bus.read8(0x0400_0054) & 0x1F) as u8;

        // Mosaic
        let mosaic = bus.read16(0x0400_004C);
        self.mosaic_bg_hsize = (mosaic & 0x000F) as u8;
        self.mosaic_bg_vsize = ((mosaic >> 4) & 0x000F) as u8;
        self.mosaic_obj_hsize = ((mosaic >> 8) & 0x000F) as u8;
        self.mosaic_obj_vsize = ((mosaic >> 12) & 0x000F) as u8;

        // Mosaic enable flags (from BLDCNT/MOSAIC interaction)
        // Mosaic is per layer, and DISPCNT bit 6 is OBJ character mapping -
        // not a mosaic enable. Reading it here mosaiced every background
        // whenever a game set 1D sprite mapping, which is most of them.
        self.mosaic_obj_enabled = true;

        // Window registers. GBATEK, LCD I/O Window Feature: WINxH holds X1 -
        // the *left* edge - in bits 8-15 and X2 in bits 0-7, so the low byte
        // is the right edge. This used to read them the other way round.
        self.win0h_right = bus.read8(0x0400_0040);
        self.win0h_left = bus.read8(0x0400_0041);
        self.win1h_right = bus.read8(0x0400_0042);
        self.win1h_left = bus.read8(0x0400_0043);
        self.win0v_bottom = bus.read8(0x0400_0044);
        self.win0v_top = bus.read8(0x0400_0045);
        self.win1v_bottom = bus.read8(0x0400_0046);
        self.win1v_top = bus.read8(0x0400_0047);
        self.winin = bus.read16(0x0400_0048);
        self.winout = bus.read16(0x0400_004A);

        self.mode = self.bg_mode;
    }

    // ========================================================================
    // Windows
    // ========================================================================

    /// Which layers are enabled at each pixel of this scanline.
    ///
    /// Each entry holds WININ/WINOUT's low six bits: BG0-3, OBJ, and bit 5
    /// for the colour special effect. With no window enabled everything is on,
    /// which is the common case and costs one branch.
    ///
    /// This replaces `get_window` and `window_layer_visible`, which were dead
    /// code - `render_scanline` computed its window flags and threw them away,
    /// so windows had no effect on anything.
    fn window_mask(&self, y: usize) -> [u8; SCREEN_WIDTH] {
        let win0_en = self.dispcnt & 0x2000 != 0;
        let win1_en = self.dispcnt & 0x4000 != 0;
        let objwin_en = self.dispcnt & 0x8000 != 0;
        if !win0_en && !win1_en && !objwin_en {
            return [0x3F; SCREEN_WIDTH];
        }

        // GBATEK: X2 beyond 240, or X1 > X2, is read as X2 = 240; the same for
        // Y against 160. A window whose top is below its bottom is empty.
        let rows = |top: u8, bottom: u8| -> (usize, usize) {
            let bottom = if bottom as usize > SCREEN_HEIGHT || top > bottom {
                SCREEN_HEIGHT
            } else {
                bottom as usize
            };
            (top as usize, bottom)
        };
        let cols = |left: u8, right: u8| -> (usize, usize) {
            let right = if right as usize > SCREEN_WIDTH || left > right {
                SCREEN_WIDTH
            } else {
                right as usize
            };
            (left as usize, right)
        };

        let (w0t, w0b) = rows(self.win0v_top, self.win0v_bottom);
        let (w1t, w1b) = rows(self.win1v_top, self.win1v_bottom);
        let (w0l, w0r) = cols(self.win0h_left, self.win0h_right);
        let (w1l, w1r) = cols(self.win1h_left, self.win1h_right);

        let win0_row = win0_en && y >= w0t && y < w0b;
        let win1_row = win1_en && y >= w1t && y < w1b;

        let inside0 = (self.winin & 0x3F) as u8;
        let inside1 = ((self.winin >> 8) & 0x3F) as u8;
        let outside = (self.winout & 0x3F) as u8;
        let objwin = ((self.winout >> 8) & 0x3F) as u8;

        let mut mask = [outside; SCREEN_WIDTH];
        for (x, m) in mask.iter_mut().enumerate() {
            // WIN0 outranks WIN1, which outranks the OBJ window, which
            // outranks everything outside.
            *m = if win0_row && x >= w0l && x < w0r {
                inside0
            } else if win1_row && x >= w1l && x < w1r {
                inside1
            } else if objwin_en && self.obj_window[x] {
                objwin
            } else {
                outside
            };
        }
        mask
    }

    // ========================================================================
    // Mosaic
    // ========================================================================

    /// Apply mosaic to an (x, y) coordinate for BG tiles.
    fn mosaic_bg(&self, bg: usize, x: usize, y: usize) -> (usize, usize) {
        let cnt = match bg {
            0 => self.bg0cnt,
            1 => self.bg1cnt,
            2 => self.bg2cnt,
            _ => self.bg3cnt,
        };
        // BGxCNT bit 6 is this background's own mosaic enable.
        if cnt & 0x0040 == 0 || (self.mosaic_bg_hsize == 0 && self.mosaic_bg_vsize == 0) {
            return (x, y);
        }
        let h = self.mosaic_bg_hsize as usize + 1;
        let v = self.mosaic_bg_vsize as usize + 1;
        let mx = (x / h) * h;
        let my = (y / v) * v;
        (mx, my)
    }

    /// Apply mosaic to an (x, y) coordinate for OBJ sprites.
    fn mosaic_obj(&self, x: usize, y: usize) -> (usize, usize) {
        if !self.mosaic_obj_enabled || (self.mosaic_obj_hsize == 0 && self.mosaic_obj_vsize == 0) {
            return (x, y);
        }
        let h = self.mosaic_obj_hsize as usize + 1;
        let v = self.mosaic_obj_vsize as usize + 1;
        let mx = (x / h) * h;
        let my = (y / v) * v;
        (mx, my)
    }

    fn render_scanline(&mut self, bus: &mut super::memory::MemoryBus) {
        if self.force_blank {
            for x in 0..SCREEN_WIDTH {
                let idx = (self.scanline as usize * SCREEN_WIDTH + x) * 3;
                self.frame_buffer[idx] = 0xFF;
                self.frame_buffer[idx + 1] = 0xFF;
                self.frame_buffer[idx + 2] = 0xFF;
            }
            return;
        }

        let y = self.scanline as usize;
        if y >= SCREEN_HEIGHT {
            return;
        }

        // Clear the scanline to the backdrop, which is palette entry 0 - not
        // white. Clearing to white made every unrendered pixel look like a
        // deliberately bright background and hid the fact that mode 0 was
        // drawing nothing at all.
        let backdrop = bus.read16(0x0500_0000);
        let (br, bg_, bb) = (
            ((backdrop & 0x1F) as u8) << 3,
            (((backdrop >> 5) & 0x1F) as u8) << 3,
            (((backdrop >> 10) & 0x1F) as u8) << 3,
        );
        for x in 0..SCREEN_WIDTH {
            let idx = (y * SCREEN_WIDTH + x) * 3;
            self.frame_buffer[idx] = br;
            self.frame_buffer[idx + 1] = bg_;
            self.frame_buffer[idx + 2] = bb;
        }
        self.scanline_layer = [LAYER_BD; SCREEN_WIDTH];
        self.scanline_second = [(br, bg_, bb); SCREEN_WIDTH];
        self.scanline_second_layer = [LAYER_BD; SCREEN_WIDTH];

        // The sprite pass comes first for every mode: mode 0 composites from
        // its result, the others overlay it, and OBJ-window sprites have to be
        // decoded before the window mask can be built at all.
        if self.obj_enable {
            self.render_obj_scanline(y, bus);
        } else {
            self.obj_pixel = [None; SCREEN_WIDTH];
            self.obj_window = [false; SCREEN_WIDTH];
        }
        self.window = self.window_mask(y);

        match self.bg_mode {
            0 => self.render_mode0_scanline(y, bus),
            1 => self.render_mode1_scanline(y, bus),
            2 => self.render_mode2_scanline(y, bus),
            3 => self.render_mode3_scanline(y, bus),
            4 => self.render_mode4_scanline(y, bus),
            5 => self.render_mode5_scanline(y, bus),
            _ => {}
        }

        // Mode 0 orders sprites against its backgrounds by priority while it
        // composites. The other modes get them laid on top instead - wrong
        // when a background outranks the sprite, but the same approximation
        // their colour effects already make, and it is what makes sprites
        // appear in the bitmap modes at all: those three never ran the sprite
        // pass before.
        if self.bg_mode != 0 && self.obj_enable {
            for x in 0..SCREEN_WIDTH {
                if self.window[x] & 0x10 == 0 {
                    continue;
                }
                if let Some((rgb, _, _)) = self.obj_pixel[x] {
                    self.put_pixel(y, x, rgb, LAYER_OBJ);
                }
            }
        }

        // Windowing itself is done by `window_mask` before the mode renderers
        // run; the loop that used to sit here recomputed the WINOUT bits per
        // pixel and threw them all away.

        // Mode 0 applies colour effects per pixel while it composites, which
        // is the only way to know which layer is under the top one. The other
        // modes still use the scanline-wide approximation below, now gated
        // per-pixel on `scanline_layer` inside `apply_brightness_inc/dec`.
        //
        // This used to also gate the call itself on `bldcnt & 0x0020` - bit 5
        // is BLDCNT's *backdrop* 1st-target-select bit, not a "some effect is
        // active" flag, so the whole effect silently turned on and off with
        // whatever unrelated thing the game did to the backdrop's target bit.
        // `apply_color_effects` already no-ops on effect 0, so no bail-out is
        // needed here.
        if self.bg_mode != 0 {
            self.apply_color_effects(y);
        }
    }

    // ========================================================================
    // Mode 0: 4 tiled backgrounds (BG0-BG3)
    // ========================================================================

    fn render_mode0_scanline(&mut self, y: usize, bus: &mut super::memory::MemoryBus) {
        let mut bg_list: Vec<(u8, usize)> = Vec::new();
        if self.bg0_enable {
            bg_list.push(((self.bg0cnt & 3) as u8, 0));
        }
        if self.bg1_enable {
            bg_list.push(((self.bg1cnt & 3) as u8, 1));
        }
        if self.bg2_enable {
            bg_list.push(((self.bg2cnt & 3) as u8, 2));
        }
        if self.bg3_enable {
            bg_list.push(((self.bg3cnt & 3) as u8, 3));
        }
        // A stable sort on priority alone leaves equal priorities in BG-number
        // order, and the lower-numbered background is the one in front.
        bg_list.sort_by_key(|&(p, _)| p);

        let backdrop = {
            let idx = (y * SCREEN_WIDTH) * 3;
            (
                self.frame_buffer[idx],
                self.frame_buffer[idx + 1],
                self.frame_buffer[idx + 2],
            )
        };

        for x in 0..SCREEN_WIDTH {
            let idx = (y * SCREEN_WIDTH + x) * 3;

            // Colour effects need the layer *below* the top one, so collect
            // the first two opaque backgrounds rather than stopping at one.
            let mut hit: [Option<((u8, u8, u8), usize, u8)>; 2] = [None, None];
            let mut found = 0usize;
            let allow = self.window[x];
            for &(bg_priority, bg) in &bg_list {
                if allow & (1 << bg) == 0 {
                    continue;
                }
                let (mx, my) = self.mosaic_bg(bg, x, y);
                let (tile_local_x, tile_local_y, screen_entry, char_base, palette_bank, is_8bpp) =
                    self.get_bg_pixel(bg, mx, my, bus);

                let color = if !is_8bpp {
                    let tile_data_addr = char_base + screen_entry as usize * 32;
                    let byte_offset = tile_local_y * 4 + tile_local_x / 2;
                    let byte = bus.read8((tile_data_addr + byte_offset) as u32);
                    let color_index = if tile_local_x % 2 == 0 {
                        byte & 0x0F
                    } else {
                        (byte >> 4) & 0x0F
                    };
                    if color_index == 0 {
                        continue;
                    }
                    let entry = palette_bank * 16 + color_index as usize;
                    bus.read16((0x0500_0000 + entry * 2) as u32)
                } else {
                    let tile_data_addr = char_base + screen_entry as usize * 64;
                    let byte_offset = tile_local_y * 8 + tile_local_x;
                    let color_index = bus.read8((tile_data_addr + byte_offset) as u32);
                    if color_index == 0 {
                        continue;
                    }
                    bus.read16((0x0500_0000 + color_index as usize * 2) as u32)
                };

                let rgb = (
                    ((color & 0x001F) as u8) << 3,
                    (((color >> 5) & 0x001F) as u8) << 3,
                    (((color >> 10) & 0x001F) as u8) << 3,
                );
                hit[found] = Some((rgb, bg, bg_priority));
                found += 1;
                if found == 2 {
                    break;
                }
            }

            // Merge the sprite into the ordering at its own priority. GBATEK,
            // LCD OBJ - OAM Attributes: "In case that the Priority relative to
            // BG is the same than the priority of one of the background
            // layers, then the OBJ becomes higher priority" - so the sprite
            // goes ahead of the first background it ties with.
            //
            // A fixed array, not a `Vec`: this runs once per pixel, so a
            // heap allocation here is 38,400 of them a frame.
            let mut stack: [((u8, u8, u8), usize); 4] = [((0, 0, 0), LAYER_BD); 4];
            let mut depth = 0usize;
            let mut push = |entry: ((u8, u8, u8), usize)| {
                if depth < stack.len() {
                    stack[depth] = entry;
                    depth += 1;
                }
            };

            let obj = if allow & 0x10 != 0 {
                self.obj_pixel[x]
            } else {
                None
            };
            let mut obj_placed = obj.is_none();
            for entry in hit.iter().flatten() {
                let &(rgb, bg, bg_priority) = entry;
                if let Some((obj_rgb, obj_priority, _)) = obj {
                    if !obj_placed && obj_priority <= bg_priority {
                        push((obj_rgb, LAYER_OBJ));
                        obj_placed = true;
                    }
                }
                push((rgb, bg));
            }
            if let Some((obj_rgb, _, _)) = obj {
                if !obj_placed {
                    push((obj_rgb, LAYER_OBJ));
                }
            }
            push((backdrop, LAYER_BD));
            drop(push);

            let (top, top_layer) = stack[0];
            // With nothing but the backdrop there is no second layer; the
            // backdrop stands in for itself, which is what hardware blends.
            let (second, second_layer) = if depth > 1 { stack[1] } else { stack[0] };

            // WININ/WINOUT bit 5 is the colour special effect's own enable
            // for that region.
            // GBATEK, Color Special Effects: a semi-transparent OBJ is
            // always a 1st target and always alpha-blends, whatever BLDCNT
            // bits 4 and 6-7 say. The 2nd-target bits still decide what it
            // blends with, and a layer that is not one means no blend at all.
            let semi = matches!(obj, Some((_, _, true))) && top_layer == LAYER_OBJ;
            let out = if semi {
                self.alpha_blend(top, second, second_layer)
            } else if allow & 0x20 != 0 {
                self.blend(top, top_layer, second, second_layer)
            } else {
                top
            };
            self.frame_buffer[idx] = out.0;
            self.frame_buffer[idx + 1] = out.1;
            self.frame_buffer[idx + 2] = out.2;
        }
    }

    /// Apply BLDCNT's colour special effect to one composited pixel.
    ///
    /// `apply_alpha_blend` used to be an empty stub, and the effect was gated
    /// on BLDCNT bit 5 - which selects the *backdrop* as a first target, not
    /// whether an effect runs at all. A game that alpha-blended a layer got it
    /// drawn flat and opaque instead: in Yggdra Union that is the grey bar
    /// across the title screen, which should be a translucent band.
    /// GBATEK: `I = min(31, I1st*EVA/16 + I2nd*EVB/16)` per channel, with EVA
    /// and EVB capped at 16. A second layer that is not a selected 2nd target
    /// means the top pixel is shown unblended.
    fn alpha_blend(
        &self,
        top: (u8, u8, u8),
        second: (u8, u8, u8),
        second_layer: usize,
    ) -> (u8, u8, u8) {
        if self.bldcnt & (0x0100 << second_layer) == 0 {
            return top;
        }
        let eva = (self.bldalpha & 0x1F).min(16) as u32;
        let evb = ((self.bldalpha >> 8) & 0x1F).min(16) as u32;
        let mix = |a: u8, b: u8| ((a as u32 * eva + b as u32 * evb) / 16).min(255) as u8;
        (
            mix(top.0, second.0),
            mix(top.1, second.1),
            mix(top.2, second.2),
        )
    }

    fn blend(
        &self,
        top: (u8, u8, u8),
        top_layer: usize,
        second: (u8, u8, u8),
        second_layer: usize,
    ) -> (u8, u8, u8) {
        let effect = (self.bldcnt >> 6) & 3;
        if effect == 0 || self.bldcnt & (1 << top_layer) == 0 {
            return top;
        }
        match effect {
            1 => self.alpha_blend(top, second, second_layer),
            2 => {
                let ey = (self.bldy & 0x1F).min(16) as u32;
                let up = |a: u8| (a as u32 + (255 - a as u32) * ey / 16).min(255) as u8;
                (up(top.0), up(top.1), up(top.2))
            }
            3 => {
                let ey = (self.bldy & 0x1F).min(16) as u32;
                let down = |a: u8| (a as u32 - a as u32 * ey / 16) as u8;
                (down(top.0), down(top.1), down(top.2))
            }
            _ => top,
        }
    }

    // ========================================================================
    // Mode 1: BG0 + BG1 tiled, BG2 affine
    // ========================================================================

    fn render_mode1_scanline(&mut self, y: usize, bus: &mut super::memory::MemoryBus) {
        // BG0 and BG1: standard tiled (like Mode 0)
        let mut bg_list: Vec<(u8, usize)> = Vec::new();
        if self.bg0_enable {
            bg_list.push(((self.bg0cnt & 3) as u8, 0));
        }
        if self.bg1_enable {
            bg_list.push(((self.bg1cnt & 3) as u8, 1));
        }
        bg_list.sort_by_key(|&(p, _)| p);

        for x in 0..SCREEN_WIDTH {
            for &(_, bg) in &bg_list {
                if self.window[x] & (1 << bg) == 0 {
                    continue;
                }
                let (mx, my) = self.mosaic_bg(bg, x, y);
                let (tile_local_x, tile_local_y, screen_entry, char_base, palette_bank, is_8bpp) =
                    self.get_bg_pixel(bg, mx, my, bus);

                let color = if !is_8bpp {
                    let tile_data_addr = char_base + screen_entry as usize * 32;
                    let byte_offset = tile_local_y * 4 + tile_local_x / 2;
                    let byte = bus.read8((tile_data_addr + byte_offset) as u32);
                    let color_index = if tile_local_x % 2 == 0 {
                        byte & 0x0F
                    } else {
                        (byte >> 4) & 0x0F
                    };
                    if color_index == 0 {
                        continue;
                    }
                    let entry = palette_bank * 16 + color_index as usize;
                    bus.read16((0x0500_0000 + entry * 2) as u32)
                } else {
                    let tile_data_addr = char_base + screen_entry as usize * 64;
                    let byte_offset = tile_local_y * 8 + tile_local_x;
                    let color_index = bus.read8((tile_data_addr + byte_offset) as u32);
                    if color_index == 0 {
                        continue;
                    }
                    bus.read16((0x0500_0000 + color_index as usize * 2) as u32)
                };

                let r = ((color & 0x001F) as u8) << 3;
                let g = (((color >> 5) & 0x001F) as u8) << 3;
                let b = (((color >> 10) & 0x001F) as u8) << 3;
                self.put_pixel(y, x, (r, g, b), bg);
                break;
            }

            // BG2 affine
            if self.bg2_enable && self.window[x] & 0x04 != 0 {
                self.render_affine_bg_pixel(2, x, y, bus);
            }
        }
    }

    // ========================================================================
    // Mode 2: BG2 + BG3 affine
    // ========================================================================

    fn render_mode2_scanline(&mut self, y: usize, bus: &mut super::memory::MemoryBus) {
        for x in 0..SCREEN_WIDTH {
            if self.bg2_enable && self.window[x] & 0x04 != 0 {
                self.render_affine_bg_pixel(2, x, y, bus);
            }
            if self.bg3_enable && self.window[x] & 0x08 != 0 {
                self.render_affine_bg_pixel(3, x, y, bus);
            }
        }
    }

    // ========================================================================
    // Affine BG renderer (used by Mode 1 BG2, Mode 2 BG2/BG3)
    // ========================================================================

    /// One pixel of an affine (rotation/scaling) background.
    ///
    /// This used to never read the tilemap at all: it took the map *index* as
    /// the tile number and multiplied it by 8, so the background came out as a
    /// linear walk through character memory - a striped pattern that looks
    /// deliberate at a glance. It also skipped the whole layer unless BGxCNT
    /// bit 7 was set, though affine backgrounds are always 256-colour and that
    /// bit means nothing for them.
    fn reload_affine_reference(&mut self) {
        self.bg2x_internal = self.bg2x;
        self.bg2y_internal = self.bg2y;
        self.bg3x_internal = self.bg3x;
        self.bg3y_internal = self.bg3y;
    }

    fn render_affine_bg_pixel(
        &mut self,
        bg: usize,
        screen_x: usize,
        y: usize,
        bus: &mut super::memory::MemoryBus,
    ) {
        let (cnt, ref_x, ref_y, pa, pb, pc, pd) = match bg {
            2 => (
                self.bg2cnt,
                self.bg2x_internal,
                self.bg2y_internal,
                self.bg2pa as i32,
                self.bg2pb as i32,
                self.bg2pc as i32,
                self.bg2pd as i32,
            ),
            3 => (
                self.bg3cnt,
                self.bg3x_internal,
                self.bg3y_internal,
                self.bg3pa as i32,
                self.bg3pb as i32,
                self.bg3pc as i32,
                self.bg3pd as i32,
            ),
            _ => return,
        };

        // Screen size 0-3 is 16x16, 32x32, 64x64 or 128x128 tiles.
        let map_pixels: i32 = 128 << ((cnt >> 14) & 3);
        let map_tiles = (map_pixels / 8) as usize;

        // The reference point is per-scanline; pa/pc step it across the line.
        let mut tx = (ref_x + pa * screen_x as i32) >> 8;
        let mut ty = (ref_y + pc * screen_x as i32) >> 8;
        let _ = (pb, pd); // applied when the reference point advances per line

        // BGxCNT bit 13 is Display Area Overflow: 0 leaves the area outside
        // the map transparent, 1 wraps it.
        if cnt & 0x2000 != 0 {
            tx = tx.rem_euclid(map_pixels);
            ty = ty.rem_euclid(map_pixels);
        } else if tx < 0 || tx >= map_pixels || ty < 0 || ty >= map_pixels {
            return;
        }

        let screen_base = ((cnt >> 8) & 0x1F) as usize * 0x800;
        let char_base = ((cnt >> 2) & 3) as usize * 0x4000;

        // An affine map entry is a single byte: the tile number, with no flip
        // or palette bits.
        let map_index = (ty as usize / 8) * map_tiles + (tx as usize / 8);
        let tile = bus.read8((0x0600_0000 + screen_base + map_index) as u32) as usize;

        let addr = 0x0600_0000 + char_base + tile * 64 + (ty as usize % 8) * 8 + (tx as usize % 8);
        let color_index = bus.read8(addr as u32);
        if color_index == 0 {
            return;
        }

        let color = bus.read16(0x0500_0000 + color_index as u32 * 2);
        let rgb = (
            ((color & 0x001F) as u8) << 3,
            (((color >> 5) & 0x001F) as u8) << 3,
            (((color >> 10) & 0x001F) as u8) << 3,
        );
        self.put_pixel(y, screen_x, rgb, bg);
    }

    // ========================================================================
    // OBJ sprite renderer
    // ========================================================================

    /// Address of an OBJ tile.
    ///
    /// OBJ tile data starts at 0x06010000, not at 0x06000000 - that is BG
    /// VRAM, and reading sprites from it textured every sprite with whatever
    /// background happened to live there. In the bitmap modes the first
    /// 0x4000 of OBJ VRAM belongs to the frame buffer, so tiles below 512 are
    /// not displayed at all.
    ///
    /// Tile numbers are always counted in 32-byte units. A 256-colour tile is
    /// 64 bytes, so it occupies two of them - which is why an 8bpp sprite's
    /// tiles step by two, in both mapping modes.
    fn obj_tile_addr(&self, tile_index: usize) -> Option<usize> {
        let index = tile_index & 0x3FF;
        if self.bg_mode >= 3 && index < 512 {
            return None;
        }
        Some(0x0601_0000 + index * 32)
    }

    /// Offset, in 32-byte tile units, of the (col, row) tile of a sprite.
    fn obj_tile_offset(&self, col: usize, row: usize, width: usize, is_8bpp: bool) -> usize {
        let mapping_1d = self.dispcnt & 0x0040 != 0;
        let step = if is_8bpp { 2 } else { 1 };
        if mapping_1d {
            (row * (width / 8) + col) * step
        } else {
            // 2D mapping is a fixed 32-unit-wide grid, so a row always
            // advances 32 units whatever the sprite's own width.
            row * 32 + col * step
        }
    }

    fn render_obj_scanline(&mut self, y: usize, bus: &mut super::memory::MemoryBus) {
        self.obj_pixel = [None; SCREEN_WIDTH];
        self.obj_window = [false; SCREEN_WIDTH];
        for sprite in 0..128u32 {
            let oam_addr = 0x0700_0000 + sprite * 8;
            let attr0 = bus.read16(oam_addr);
            let attr1 = bus.read16(oam_addr + 2);
            let attr2 = bus.read16(oam_addr + 4);

            let is_affine = attr0 & 0x0100 != 0;
            let is_double_size = attr0 & 0x0200 != 0;
            if !is_affine && attr0 & 0x0200 != 0 {
                continue; // disabled
            }
            // GBATEK, OBJ Attribute 0: bit 13 is the colour depth and bits
            // 10-11 the OBJ mode. The depth used to be read from bit 7, which
            // belongs to the Y coordinate, so any sprite at Y >= 128 was
            // decoded as 256-colour.
            let is_8bpp = attr0 & 0x2000 != 0;
            // Mode 2 is the OBJ window: the sprite is not drawn, but its
            // non-transparent dots still have to be decoded because they are
            // what shapes the window.
            let is_obj_window = (attr0 >> 10) & 3 == 2;
            let is_semi_transparent = (attr0 >> 10) & 3 == 1;

            let shape = (attr0 >> 14) & 3;
            let size = (attr1 >> 14) & 3;
            let (width, height) = match shape {
                0b00 => match size {
                    0 => (8, 8),
                    1 => (16, 16),
                    2 => (32, 32),
                    _ => (64, 64),
                },
                0b01 => match size {
                    0 => (16, 8),
                    1 => (32, 8),
                    2 => (32, 16),
                    _ => (64, 32),
                },
                0b10 => match size {
                    0 => (8, 16),
                    1 => (8, 32),
                    2 => (16, 32),
                    _ => (32, 64),
                },
                _ => continue,
            };

            // Y is 8 bits and X is 9, both unsigned, and both wrap - a sprite
            // near the bottom reappears at the top. Sign-extending them
            // instead put every sprite at Y >= 128 off the top of the screen.
            let sy = (attr0 & 0x00FF) as i32;
            let sx = (attr1 & 0x01FF) as i32;
            let (box_w, box_h) = if is_affine && is_double_size {
                (width * 2, height * 2)
            } else {
                (width, height)
            };
            let row = (y as i32 - sy).rem_euclid(256);
            if row >= box_h as i32 {
                continue;
            }

            let palette = ((attr2 >> 12) & 0xF) as usize;
            let tile_num = (attr2 & 0x03FF) as usize;
            let priority = ((attr2 >> 10) & 3) as u8;

            // Affine sprites take their transform from one of 32 parameter
            // groups interleaved through OAM at +6, +14, +22, +30.
            let (pa, pb, pc, pd) = if is_affine {
                let base = 0x0700_0000 + ((attr1 >> 9) & 0x1F) as u32 * 32 + 6;
                (
                    bus.read16(base) as i16 as i32,
                    bus.read16(base + 8) as i16 as i32,
                    bus.read16(base + 16) as i16 as i32,
                    bus.read16(base + 24) as i16 as i32,
                )
            } else {
                (256, 0, 0, 256)
            };
            let hflip = !is_affine && attr1 & 0x1000 != 0;
            let vflip = !is_affine && attr1 & 0x2000 != 0;

            for col in 0..box_w {
                let screen_x = (sx + col as i32).rem_euclid(512);
                if screen_x >= SCREEN_WIDTH as i32 {
                    continue;
                }
                let screen_x = screen_x as usize;
                if !is_obj_window && self.obj_pixel[screen_x].is_some() {
                    // Between two overlapping sprites the OAM index decides,
                    // on its own - a sprite's priority field is only ever
                    // compared against the backgrounds, never against another
                    // sprite. GBATEK's "Caution" example under OAM Attributes
                    // spells this out, and it was confirmed on hardware in
                    // VisualBoyAdvance bug #130. TONC's regobj page says the
                    // opposite in passing; it is the outlier.
                    continue;
                }

                let (tex_x, tex_y) = if is_affine {
                    let rel_x = col as i32 - box_w as i32 / 2;
                    let rel_y = row - box_h as i32 / 2;
                    (
                        (pa * rel_x + pb * rel_y) / 256 + width as i32 / 2,
                        (pc * rel_x + pd * rel_y) / 256 + height as i32 / 2,
                    )
                } else {
                    let x = if hflip {
                        width as i32 - 1 - col as i32
                    } else {
                        col as i32
                    };
                    let y = if vflip { height as i32 - 1 - row } else { row };
                    (x, y)
                };
                if tex_x < 0 || tex_x >= width as i32 || tex_y < 0 || tex_y >= height as i32 {
                    continue;
                }
                let (tex_x, tex_y) = (tex_x as usize, tex_y as usize);

                let offset = self.obj_tile_offset(tex_x / 8, tex_y / 8, width, is_8bpp);
                let Some(tile_addr) = self.obj_tile_addr(tile_num + offset) else {
                    continue;
                };

                let color_index = if is_8bpp {
                    bus.read8((tile_addr + (tex_y % 8) * 8 + (tex_x % 8)) as u32)
                } else {
                    let byte = bus.read8((tile_addr + (tex_y % 8) * 4 + (tex_x % 8) / 2) as u32);
                    if tex_x % 2 == 0 {
                        byte & 0x0F
                    } else {
                        (byte >> 4) & 0x0F
                    }
                };
                if color_index == 0 {
                    continue;
                }

                let palette_addr = if is_8bpp {
                    0x0500_0200 + color_index as usize * 2
                } else {
                    0x0500_0200 + palette * 32 + color_index as usize * 2
                };
                if is_obj_window {
                    self.obj_window[screen_x] = true;
                    continue;
                }

                let color = bus.read16(palette_addr as u32);
                self.obj_pixel[screen_x] = Some((
                    (
                        ((color & 0x001F) as u8) << 3,
                        (((color >> 5) & 0x001F) as u8) << 3,
                        (((color >> 10) & 0x001F) as u8) << 3,
                    ),
                    priority,
                    is_semi_transparent,
                ));
            }
        }
    }

    // ========================================================================
    // BG pixel lookup (standard tiled)
    // ========================================================================

    fn get_bg_pixel(
        &self,
        bg: usize,
        screen_x: usize,
        screen_y: usize,
        bus: &mut super::memory::MemoryBus,
    ) -> (usize, usize, u16, usize, usize, bool) {
        let (cnt, hofs, vofs) = match bg {
            0 => (self.bg0cnt, self.bg0hofs, self.bg0vofs),
            1 => (self.bg1cnt, self.bg1hofs, self.bg1vofs),
            2 => (self.bg2cnt, self.bg2hofs, self.bg2vofs),
            3 => (self.bg3cnt, self.bg3hofs, self.bg3vofs),
            _ => unreachable!(),
        };

        let scrolled_x = (screen_x + hofs as usize) & 0x1FF;
        let scrolled_y = (screen_y + vofs as usize) & 0x1FF;
        let screen_size = (cnt >> 14) & 3;
        let is_8bpp = cnt & 0x0080 != 0;
        let tile_x = scrolled_x / 8;
        let tile_y = scrolled_y / 8;

        let screen_block_offset = match screen_size {
            0 => 0,
            1 => {
                if tile_x >= 32 {
                    1024
                } else {
                    0
                }
            }
            2 => {
                if tile_y >= 32 {
                    1024
                } else {
                    0
                }
            }
            3 => ((tile_x / 32) + (tile_y / 32) * 2) * 1024,
            _ => 0,
        };

        let tile_index_in_block = (tile_y % 32) * 32 + (tile_x % 32);
        let screen_entry = screen_block_offset + tile_index_in_block;
        let screen_base = ((cnt >> 8) & 0x1F) as usize * 0x800;
        let screen_entry_addr = 0x0600_0000 + screen_base + screen_entry * 2;
        let screen_entry_value = bus.read16(screen_entry_addr as u32);
        let tile_number = screen_entry_value & 0x03FF;
        // Absolute address, not a VRAM offset: the caller reads through the bus.
        let char_base = 0x0600_0000 + ((cnt >> 2) & 3) as usize * 0x4000;

        // Screen entry bit 10 flips horizontally, bit 11 vertically.
        let mut local_x = scrolled_x % 8;
        let mut local_y = scrolled_y % 8;
        if screen_entry_value & 0x0400 != 0 {
            local_x = 7 - local_x;
        }
        if screen_entry_value & 0x0800 != 0 {
            local_y = 7 - local_y;
        }

        // Bits 12-15 select one of sixteen 16-colour palettes, 4bpp only.
        let palette_bank = ((screen_entry_value >> 12) & 0xF) as usize;

        (
            local_x,
            local_y,
            tile_number,
            char_base,
            palette_bank,
            is_8bpp,
        )
    }

    // ========================================================================
    // Mode 3: Bitmap 16bpp
    // ========================================================================

    fn render_mode3_scanline(&mut self, y: usize, bus: &mut super::memory::MemoryBus) {
        let base = 0x0600_0000 + y * 480;
        for x in 0..SCREEN_WIDTH {
            let color = bus.read16((base + x * 2) as u32);
            let r = ((color & 0x001F) as u8) << 3;
            let g = (((color >> 5) & 0x001F) as u8) << 3;
            let b = (((color >> 10) & 0x001F) as u8) << 3;
            // Bitmap modes are BG2 top to bottom - no transparency to fall
            // through to the backdrop, which stays the 2nd blend target.
            self.put_pixel(y, x, (r, g, b), 2);
        }
    }

    // ========================================================================
    // Mode 4: Bitmap 8bpp (2 framebuffers)
    // ========================================================================

    fn render_mode4_scanline(&mut self, y: usize, bus: &mut super::memory::MemoryBus) {
        let page = if self.dispcnt & 0x0010 != 0 {
            0xA000
        } else {
            0x0000
        };
        let base = 0x0600_0000 + page + y * 240;
        for x in 0..SCREEN_WIDTH {
            let color_index = bus.read8((base + x) as u32);
            let color = bus.read16((0x0500_0000 + color_index as usize * 2) as u32);
            let r = ((color & 0x001F) as u8) << 3;
            let g = (((color >> 5) & 0x001F) as u8) << 3;
            let b = (((color >> 10) & 0x001F) as u8) << 3;
            self.put_pixel(y, x, (r, g, b), 2);
        }
    }

    // ========================================================================
    // Mode 5: Bitmap 16bpp (2 framebuffers, 160x128 each)
    // ========================================================================

    fn render_mode5_scanline(&mut self, y: usize, bus: &mut super::memory::MemoryBus) {
        // Mode 5: two 160x128 16bpp framebuffers
        // Page select from DISPCNT bit 4
        let page = if self.dispcnt & 0x0010 != 0 {
            0xA000
        } else {
            0x0000
        };

        // Only 128 lines per page; lines 128-159 show garbage (use last line)
        let fb_y = if y >= 128 { 127 } else { y };

        // Each line is 160 pixels * 2 bytes = 320 bytes
        let base = 0x0600_0000 + page + fb_y * 320;

        for x in 0..SCREEN_WIDTH {
            // Only 160 pixels wide; beyond that, show black
            if x >= 160 {
                self.put_pixel(y, x, (0, 0, 0), 2);
                continue;
            }

            let color = bus.read16((base + x * 2) as u32);
            let r = ((color & 0x001F) as u8) << 3;
            let g = (((color >> 5) & 0x001F) as u8) << 3;
            let b = (((color >> 10) & 0x001F) as u8) << 3;
            self.put_pixel(y, x, (r, g, b), 2);
        }
    }

    // ========================================================================
    // Color effects (alpha blend, brighten, darken)
    // ========================================================================

    /// Write one composited pixel, pushing whatever was there down into the
    /// 2nd-target buffers. Every overdraw path in modes 1-5 goes through here
    /// so that `apply_alpha_blend` has a layer underneath to mix with.
    ///
    /// ponytail: the 2nd target here is whatever was painted over, i.e. draw
    /// order, not the runner-up by BGxCNT priority the way mode 0's `blend`
    /// finds it. The two agree unless a game gives a background a priority
    /// that should put it under one drawn earlier. Upgrade path is to give
    /// modes 1-5 mode 0's per-pixel priority sort - the same missing sort
    /// that makes sprites always land on top in those modes - at which point
    /// this pair of buffers is replaced by the sort's runner-up. Recorded in
    /// ROADMAP.md's "Known accuracy gaps".
    fn put_pixel(&mut self, y: usize, x: usize, rgb: (u8, u8, u8), layer: usize) {
        let idx = (y * SCREEN_WIDTH + x) * 3;
        self.scanline_second[x] = (
            self.frame_buffer[idx],
            self.frame_buffer[idx + 1],
            self.frame_buffer[idx + 2],
        );
        self.scanline_second_layer[x] = self.scanline_layer[x];
        self.frame_buffer[idx] = rgb.0;
        self.frame_buffer[idx + 1] = rgb.1;
        self.frame_buffer[idx + 2] = rgb.2;
        self.scanline_layer[x] = layer;
    }

    fn apply_color_effects(&mut self, y: usize) {
        let effect = (self.bldcnt >> 6) & 3;
        match effect {
            0 => {} // None
            1 => self.apply_alpha_blend(y),
            2 => self.apply_brightness_inc(y),
            3 => self.apply_brightness_dec(y),
            _ => {}
        }
    }

    fn apply_alpha_blend(&mut self, y: usize) {
        for x in 0..SCREEN_WIDTH {
            // BLDCNT bits 0-5 select the 1st target, one bit per layer.
            if self.bldcnt & (1 << self.scanline_layer[x]) == 0 {
                continue;
            }
            let idx = (y * SCREEN_WIDTH + x) * 3;
            let top = (
                self.frame_buffer[idx],
                self.frame_buffer[idx + 1],
                self.frame_buffer[idx + 2],
            );
            let out = self.alpha_blend(top, self.scanline_second[x], self.scanline_second_layer[x]);
            self.frame_buffer[idx] = out.0;
            self.frame_buffer[idx + 1] = out.1;
            self.frame_buffer[idx + 2] = out.2;
        }
    }

    fn apply_brightness_inc(&mut self, y: usize) {
        // GBATEK caps EVY at 16 even though BLDY holds 5 bits. Without the
        // cap a game writing BLDY > 16 over-brightens here and *panics* in
        // `apply_brightness_dec`, where the same uncapped EY underflows the
        // u32 subtraction. `blend()` already caps the mode-0 path.
        let ey = (self.bldy as u32).min(16);
        if ey == 0 {
            return;
        }
        for x in 0..SCREEN_WIDTH {
            // BLDCNT bits 0-5 are the 1st-target select, one bit per layer
            // (BG0-3, OBJ, BD) - the same gate `blend()` uses for mode 0.
            // Without it, this brightened every pixel on the scanline
            // whenever BLDCNT selected the effect at all, so a game pulsing
            // BLDY to highlight one layer flashed the whole screen instead.
            if self.bldcnt & (1 << self.scanline_layer[x]) == 0 {
                continue;
            }
            let idx = (y * SCREEN_WIDTH + x) * 3;
            let r = self.frame_buffer[idx] as u32;
            let g = self.frame_buffer[idx + 1] as u32;
            let b = self.frame_buffer[idx + 2] as u32;
            self.frame_buffer[idx] = (r + (0xFF - r) * ey / 16).min(255) as u8;
            self.frame_buffer[idx + 1] = (g + (0xFF - g) * ey / 16).min(255) as u8;
            self.frame_buffer[idx + 2] = (b + (0xFF - b) * ey / 16).min(255) as u8;
        }
    }

    fn apply_brightness_dec(&mut self, y: usize) {
        let ey = (self.bldy as u32).min(16);
        if ey == 0 {
            return;
        }
        for x in 0..SCREEN_WIDTH {
            if self.bldcnt & (1 << self.scanline_layer[x]) == 0 {
                continue;
            }
            let idx = (y * SCREEN_WIDTH + x) * 3;
            let r = self.frame_buffer[idx] as u32;
            let g = self.frame_buffer[idx + 1] as u32;
            let b = self.frame_buffer[idx + 2] as u32;
            self.frame_buffer[idx] = (r - r * ey / 16) as u8;
            self.frame_buffer[idx + 1] = (g - g * ey / 16) as u8;
            self.frame_buffer[idx + 2] = (b - b * ey / 16) as u8;
        }
    }

    // ========================================================================
    // Interrupt helpers
    // ========================================================================

    pub fn vblank_pending(&mut self) -> bool {
        if self.vblank_irq_pending {
            self.vblank_irq_pending = false;
            true
        } else {
            false
        }
    }

    pub fn hblank_pending(&mut self) -> bool {
        if self.hblank_irq_pending {
            self.hblank_irq_pending = false;
            true
        } else {
            false
        }
    }

    /// Cycles until the next scanline boundary or HBlank start, whichever
    /// comes first. A halted CPU has to be advanced in these steps: ticking a
    /// whole scanline at once walks straight past HBlank, so an enabled
    /// HBlank IRQ never fires while the game is waiting in HALT - which is
    /// most of every frame.
    pub fn cycles_to_next_event(&self) -> u32 {
        if self.cycle_counter < 960 {
            960 - self.cycle_counter
        } else {
            1232 - self.cycle_counter.min(1232)
        }
        .max(1)
    }

    pub fn vcount_pending(&mut self) -> bool {
        if self.vcount_irq_pending {
            self.vcount_irq_pending = false;
            true
        } else {
            false
        }
    }

    pub fn frame_buffer(&self) -> &[u8; FRAME_SIZE] {
        &self.frame_buffer
    }

    pub fn frame_buffer_mut(&mut self) -> &mut [u8; FRAME_SIZE] {
        &mut self.frame_buffer
    }
}
