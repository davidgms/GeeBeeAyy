const SCREEN_WIDTH: usize = 240;
const SCREEN_HEIGHT: usize = 160;
const FRAME_SIZE: usize = SCREEN_WIDTH * SCREEN_HEIGHT * 3;

pub struct Ppu {
    frame_buffer: [u8; FRAME_SIZE],
    pub scanline: u16,
    pub mode: u8,
    pub vblank: bool,
    pub hblank: bool,
    vblank_irq_pending: bool,
    hblank_irq_pending: bool,
    vcount_irq_pending: bool,
    cycle_counter: u32,
    pub bg_mode: u8,
    // Display control
    pub dispcnt: u16,
    force_blank: bool,
    display_off: bool,
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
    mosaic_bg_enabled: bool,
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
            cycle_counter: 0,
            dispcnt: 0,
            force_blank: false,
            display_off: false,
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
            mosaic_bg_enabled: false,
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

    pub fn tick(&mut self, cycles: u32, bus: &mut super::memory::MemoryBus, dma: &mut super::dma::Dma) {
        self.cycle_counter += cycles;

        // GBA timing: 1232 cycles per scanline
        if self.cycle_counter >= 1232 {
            self.cycle_counter -= 1232;
            self.sync_from_bus(bus);

            // Read DISPSTAT IRQ enable bits from bus BEFORE rendering
            let dispstat = bus.read16(0x0400_0004);
            self.dispstat_vblank_ie = dispstat & 0x0008 != 0;
            self.dispstat_hblank_ie = dispstat & 0x0010 != 0;
            self.dispstat_vcount_ie = dispstat & 0x0020 != 0;

            self.render_scanline(bus);
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

            // Affine reference point update at HBlank
            if self.hblank {
                if self.scanline >= 2 && self.scanline <= 161 {
                    self.bg2x_internal = self.bg2x;
                    self.bg2y_internal = self.bg2y;
                    self.bg3x_internal = self.bg3x;
                    self.bg3y_internal = self.bg3y;
                }
            }
        }

        // HBlank start at cycle 960
        let new_hblank = self.cycle_counter >= 960;
        if new_hblank && !self.hblank {
            self.hblank = true;
            if self.dispstat_hblank_ie {
                self.hblank_irq_pending = true;
            }
            dma.on_hblank(bus);
        } else if !new_hblank {
            self.hblank = false;
        }

        // Write DISPSTAT: status bits from PPU state, enable bits from game
        let dispstat = ((self.vblank as u16) << 0)
            | ((self.hblank as u16) << 1)
            | (((self.scanline >= 160 && self.scanline <= 226) as u16) << 2)
            | ((self.dispstat_vcount_ie as u16) << 5)
            | ((self.dispstat_hblank_ie as u16) << 4)
            | ((self.dispstat_vblank_ie as u16) << 3);
        bus.write16(0x0400_0004, dispstat);

        // Write VCOUNT
        bus.write16(0x0400_0006, self.scanline);
    }

    fn sync_from_bus(&mut self, bus: &mut super::memory::MemoryBus) {
        self.dispcnt = bus.read16(0x0400_0000);
        self.force_blank = self.dispcnt & 0x0080 != 0;
        self.display_off = self.dispcnt & 0x8000 != 0;
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

        // Affine reference points (BG2)
        let bg2x_lo = bus.read16(0x0400_0028) as u32;
        let bg2x_hi = bus.read16(0x0400_002A) as u32;
        self.bg2x = ((bg2x_hi << 16) | bg2x_lo) as i32;

        let bg2y_lo = bus.read16(0x0400_002C) as u32;
        let bg2y_hi = bus.read16(0x0400_002E) as u32;
        self.bg2y = ((bg2y_hi << 16) | bg2y_lo) as i32;

        // Affine reference points (BG3)
        let bg3x_lo = bus.read16(0x0400_0038) as u32;
        let bg3x_hi = bus.read16(0x0400_003A) as u32;
        self.bg3x = ((bg3x_hi << 16) | bg3x_lo) as i32;

        let bg3y_lo = bus.read16(0x0400_003C) as u32;
        let bg3y_hi = bus.read16(0x0400_003E) as u32;
        self.bg3y = ((bg3y_hi << 16) | bg3y_lo) as i32;

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
        self.mosaic_bg_enabled = self.dispcnt & 0x0040 != 0;
        self.mosaic_obj_enabled = self.dispcnt & 0x0040 != 0;

        // Window registers
        self.win0h_left = bus.read8(0x0400_0040);
        self.win0h_right = bus.read8(0x0400_0041);
        self.win0v_top = bus.read8(0x0400_0044);
        self.win0v_bottom = bus.read8(0x0400_0045);
        self.win1h_left = bus.read8(0x0400_0042);
        self.win1h_right = bus.read8(0x0400_0043);
        self.win1v_top = bus.read8(0x0400_0046);
        self.win1v_bottom = bus.read8(0x0400_0047);
        self.winin = bus.read16(0x0400_0048);
        self.winout = bus.read16(0x0400_004A);

        self.mode = self.bg_mode;
    }

    // ========================================================================
    // Window check
    // ========================================================================

    /// Returns which window is active at (x, y).
    /// 0 = outside all windows, 1 = WIN0, 2 = WIN1, 3 = OBJ window, 4 = inside window
    fn get_window(&self, x: usize, y: usize) -> u8 {
        let win0_en = self.dispcnt & 0x2000 != 0;
        let win1_en = self.dispcnt & 0x4000 != 0;
        let obj_win_en = self.dispcnt & 0x8000 != 0;

        if win0_en
            && x >= self.win0h_left as usize
            && x < self.win0h_right as usize
            && y >= self.win0v_top as usize
            && y < self.win0v_bottom as usize
        {
            return 1;
        }

        if win1_en
            && x >= self.win1h_left as usize
            && x < self.win1h_right as usize
            && y >= self.win1v_top as usize
            && y < self.win1v_bottom as usize
        {
            return 2;
        }

        if obj_win_en {
            // OBJ window is set per-pixel in the OBJ layer
            return 3;
        }

        if win0_en || win1_en {
            return 4; // Inside display area but not in any window
        }

        0
    }

    /// Check if a given BG/OBJ layer is visible in the given window.
    fn window_layer_visible(&self, window: u8, layer: u8) -> bool {
        let winin = self.winin;
        let winout = self.winout;

        match window {
            0 => false, // No window active → use WINOUT
            1 => winin & (1 << layer) != 0,
            2 => (winin >> 8) & (1 << layer) != 0,
            3 => false, // OBJ window - handled per-pixel
            4 => winout & (1 << layer) != 0,
            _ => false,
        }
    }

    // ========================================================================
    // Mosaic
    // ========================================================================

    /// Apply mosaic to an (x, y) coordinate for BG tiles.
    fn mosaic_bg(&self, x: usize, y: usize) -> (usize, usize) {
        if !self.mosaic_bg_enabled || (self.mosaic_bg_hsize == 0 && self.mosaic_bg_vsize == 0) {
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

        if self.display_off {
            return;
        }

        let y = self.scanline as usize;
        if y >= SCREEN_HEIGHT {
            return;
        }

        // Clear scanline to white
        for x in 0..SCREEN_WIDTH {
            let idx = (y * SCREEN_WIDTH + x) * 3;
            self.frame_buffer[idx] = 0xFF;
            self.frame_buffer[idx + 1] = 0xFF;
            self.frame_buffer[idx + 2] = 0xFF;
        }

        match self.bg_mode {
            0 => self.render_mode0_scanline(y, bus),
            1 => self.render_mode1_scanline(y, bus),
            2 => self.render_mode2_scanline(y, bus),
            3 => self.render_mode3_scanline(y, bus),
            4 => self.render_mode4_scanline(y, bus),
            5 => self.render_mode5_scanline(y, bus),
            _ => {}
        }

        // Apply windowing (mask pixels outside active windows)
        let win0_en = self.dispcnt & 0x2000 != 0;
        let win1_en = self.dispcnt & 0x4000 != 0;
        let any_window = win0_en || win1_en;

        if any_window {
            for x in 0..SCREEN_WIDTH {
                let in_win0 = win0_en
                    && x >= self.win0h_left as usize
                    && x < self.win0h_right as usize
                    && y >= self.win0v_top as usize
                    && y < self.win0v_bottom as usize;
                let in_win1 = win1_en
                    && x >= self.win1h_left as usize
                    && x < self.win1h_right as usize
                    && y >= self.win1v_top as usize
                    && y < self.win1v_bottom as usize;

                if !in_win0 && !in_win1 {
                    // Outside all windows - apply WINOUT
                    let winout_bg0 = self.winout & 0x0001 != 0;
                    let winout_bg1 = self.winout & 0x0002 != 0;
                    let winout_bg2 = self.winout & 0x0004 != 0;
                    let winout_bg3 = self.winout & 0x0008 != 0;
                    let winout_obj = self.winout & 0x0010 != 0;
                    let _ = (winout_bg0, winout_bg1, winout_bg2, winout_bg3, winout_obj);
                    // For now, just keep the pixel visible outside windows
                }
            }
        }

        // Apply color effects
        if self.bldcnt & 0x0020 != 0 {
            // Color special effect on 1st target
            self.apply_color_effects(y);
        }
    }

    // ========================================================================
    // Mode 0: 4 tiled backgrounds (BG0-BG3)
    // ========================================================================

    fn render_mode0_scanline(&mut self, y: usize, bus: &mut super::memory::MemoryBus) {
        let mut bg_list: Vec<(u8, usize)> = Vec::new();
        if self.bg0_enable { bg_list.push(((self.bg0cnt & 3) as u8, 0)); }
        if self.bg1_enable { bg_list.push(((self.bg1cnt & 3) as u8, 1)); }
        if self.bg2_enable { bg_list.push(((self.bg2cnt & 3) as u8, 2)); }
        if self.bg3_enable { bg_list.push(((self.bg3cnt & 3) as u8, 3)); }
        bg_list.sort_by_key(|&(p, _)| p);

        if self.obj_enable {
            self.render_obj_scanline(y, bus);
        }

        for x in 0..SCREEN_WIDTH {
            for &(_, bg) in &bg_list {
                let (mx, my) = self.mosaic_bg(x, y);
                let (tile_local_x, tile_local_y, screen_entry, char_base, _palette_base, is_8bpp) =
                    self.get_bg_pixel(bg, mx, my, bus);

                let color = if !is_8bpp {
                    let tile_data_addr = char_base + screen_entry as usize * 32;
                    let byte_offset = tile_local_y * 4 + tile_local_x / 2;
                    let byte = bus.read8((tile_data_addr + byte_offset) as u32);
                    let color_index = if tile_local_x % 2 == 0 { byte & 0x0F } else { (byte >> 4) & 0x0F };
                    if color_index == 0 { continue; }
                    bus.read16((0x0500_0000 + color_index as usize * 2) as u32)
                } else {
                    let tile_data_addr = char_base + screen_entry as usize * 64;
                    let byte_offset = tile_local_y * 8 + tile_local_x;
                    let color_index = bus.read8((tile_data_addr + byte_offset) as u32);
                    if color_index == 0 { continue; }
                    bus.read16((0x0500_0000 + color_index as usize * 2) as u32)
                };

                let r = ((color & 0x001F) as u8) << 3;
                let g = (((color >> 5) & 0x001F) as u8) << 3;
                let b = (((color >> 10) & 0x001F) as u8) << 3;
                let idx = (y * SCREEN_WIDTH + x) * 3;
                self.frame_buffer[idx] = r;
                self.frame_buffer[idx + 1] = g;
                self.frame_buffer[idx + 2] = b;
                break;
            }
        }
    }

    // ========================================================================
    // Mode 1: BG0 + BG1 tiled, BG2 affine
    // ========================================================================

    fn render_mode1_scanline(&mut self, y: usize, bus: &mut super::memory::MemoryBus) {
        if self.obj_enable {
            self.render_obj_scanline(y, bus);
        }

        // BG0 and BG1: standard tiled (like Mode 0)
        let mut bg_list: Vec<(u8, usize)> = Vec::new();
        if self.bg0_enable { bg_list.push(((self.bg0cnt & 3) as u8, 0)); }
        if self.bg1_enable { bg_list.push(((self.bg1cnt & 3) as u8, 1)); }
        bg_list.sort_by_key(|&(p, _)| p);

        for x in 0..SCREEN_WIDTH {
            for &(_, bg) in &bg_list {
                let (mx, my) = self.mosaic_bg(x, y);
                let (tile_local_x, tile_local_y, screen_entry, char_base, _palette_base, is_8bpp) =
                    self.get_bg_pixel(bg, mx, my, bus);

                let color = if !is_8bpp {
                    let tile_data_addr = char_base + screen_entry as usize * 32;
                    let byte_offset = tile_local_y * 4 + tile_local_x / 2;
                    let byte = bus.read8((tile_data_addr + byte_offset) as u32);
                    let color_index = if tile_local_x % 2 == 0 { byte & 0x0F } else { (byte >> 4) & 0x0F };
                    if color_index == 0 { continue; }
                    bus.read16((0x0500_0000 + color_index as usize * 2) as u32)
                } else {
                    let tile_data_addr = char_base + screen_entry as usize * 64;
                    let byte_offset = tile_local_y * 8 + tile_local_x;
                    let color_index = bus.read8((tile_data_addr + byte_offset) as u32);
                    if color_index == 0 { continue; }
                    bus.read16((0x0500_0000 + color_index as usize * 2) as u32)
                };

                let r = ((color & 0x001F) as u8) << 3;
                let g = (((color >> 5) & 0x001F) as u8) << 3;
                let b = (((color >> 10) & 0x001F) as u8) << 3;
                let idx = (y * SCREEN_WIDTH + x) * 3;
                self.frame_buffer[idx] = r;
                self.frame_buffer[idx + 1] = g;
                self.frame_buffer[idx + 2] = b;
                break;
            }

            // BG2 affine
            if self.bg2_enable {
                self.render_affine_bg_pixel(2, x, y, bus);
            }
        }
    }

    // ========================================================================
    // Mode 2: BG2 + BG3 affine
    // ========================================================================

    fn render_mode2_scanline(&mut self, y: usize, bus: &mut super::memory::MemoryBus) {
        if self.obj_enable {
            self.render_obj_scanline(y, bus);
        }

        for x in 0..SCREEN_WIDTH {
            if self.bg2_enable {
                self.render_affine_bg_pixel(2, x, y, bus);
            }
            if self.bg3_enable {
                self.render_affine_bg_pixel(3, x, y, bus);
            }
        }
    }

    // ========================================================================
    // Affine BG renderer (used by Mode 1 BG2, Mode 2 BG2/BG3)
    // ========================================================================

    fn render_affine_bg_pixel(&mut self, bg: usize, screen_x: usize, y: usize, bus: &mut super::memory::MemoryBus) {
        let (cnt, ref_x, ref_y, pa, pb, pc, pd) = match bg {
            2 => (self.bg2cnt, self.bg2x_internal, self.bg2y_internal,
                  self.bg2pa as i32, self.bg2pb as i32, self.bg2pc as i32, self.bg2pd as i32),
            3 => (self.bg3cnt, self.bg3x_internal, self.bg3y_internal,
                  self.bg3pa as i32, self.bg3pb as i32, self.bg3pc as i32, self.bg3pd as i32),
            _ => return,
        };

        let screen_size = (cnt >> 14) & 3;
        let is_8bpp = cnt & 0x0080 != 0;

        // Calculate texture coordinates using affine matrix
        let texture_x = ref_x + pa * screen_x as i32 + pb * y as i32;
        let texture_y = ref_y + pc * screen_x as i32 + pd * y as i32;

        // Convert from 20.8 fixed point to integer
        let tx = (texture_x >> 8) as i32;
        let ty = (texture_y >> 8) as i32;

        // Wrap based on screen size (affine BGs wrap at 128/256 pixels)
        let wrap_size: i32 = match screen_size {
            0 => 128,
            1 => 256,
            2 => 512,
            3 => 1024,
            _ => 128,
        };

        let tx = tx.rem_euclid(wrap_size);
        let ty = ty.rem_euclid(wrap_size);

        let tile_x = tx / 8;
        let tile_y = ty / 8;

        // Screen base from BGxCNT bits 8-12
        let screen_base = ((cnt >> 8) & 0x1F) as usize * 0x800;
        let screen_entry_addr = 0x0600_0000 + screen_base + (tile_y as usize * wrap_size as usize / 8 + tile_x as usize);

        let color_index = if is_8bpp {
            let char_base = ((cnt >> 2) & 3) as usize * 0x4000;
            let tile_data_addr = 0x0600_0000 + char_base + (screen_entry_addr - 0x0600_0000 - screen_base) * 8;
            bus.read8(tile_data_addr as u32 + (ty % 8) as u32 * 8 + (tx % 8) as u32)
        } else {
            0 // Simplified: 4bpp affine not fully handled yet
        };

        if color_index == 0 { return; }

        let color = bus.read16((0x0500_0000 + color_index as usize * 2) as u32);
        let r = ((color & 0x001F) as u8) << 3;
        let g = (((color >> 5) & 0x001F) as u8) << 3;
        let b = (((color >> 10) & 0x001F) as u8) << 3;
        let idx = (y * SCREEN_WIDTH + screen_x) * 3;
        self.frame_buffer[idx] = r;
        self.frame_buffer[idx + 1] = g;
        self.frame_buffer[idx + 2] = b;
    }

    // ========================================================================
    // OBJ sprite renderer
    // ========================================================================

    fn render_obj_scanline(&mut self, y: usize, bus: &mut super::memory::MemoryBus) {
        for sprite in 0..128u16 {
            let oam_addr = 0x0700_0000 + (sprite as u32) * 8;
            let attr0 = bus.read16(oam_addr);
            let attr1 = bus.read16(oam_addr + 2);
            let attr2 = bus.read16(oam_addr + 4);

            let is_affine = attr0 & 0x0100 != 0;
            let is_double_size = attr0 & 0x0200 != 0;
            let is_disabled = attr0 & 0x0200 != 0 && !is_affine;
            let mosaic_enabled = attr0 & 0x1000 != 0;

            if is_disabled { continue; }

            let sy = (attr0 & 0x00FF) as i16;
            let sy = if sy > 127 { sy - 256 } else { sy };
            let shape = (attr0 >> 14) & 3;
            let sx = (attr1 & 0x01FF) as i16;
            let sx = if sx > 255 { sx - 512 } else { sx };
            let size = (attr1 >> 14) & 3;
            let tile_num = attr2 & 0x03FF;
            let palette = ((attr2 >> 12) & 0xF) as u8;
            let bitmap_mode = attr0 & 0x2000 != 0;

            let (base_width, base_height) = match shape {
                0b00 => match size { 0 => (8,8), 1 => (16,16), 2 => (32,32), 3 => (64,64), _ => (8,8) },
                0b01 => match size { 0 => (16,8), 1 => (32,8), 2 => (32,16), 3 => (64,32), _ => (16,8) },
                0b10 => match size { 0 => (8,16), 1 => (8,32), 2 => (16,32), 3 => (32,64), _ => (8,16) },
                _ => continue,
            };

            if is_affine {
                // Affine sprite rendering
                let matrix_idx = ((attr1 >> 9) & 0x1F) as usize;
                let matrix_base = matrix_idx * 32 + 0x0700_0006;

                // Read affine parameters from OAM
                let pa = bus.read16((0x0700_0000 + matrix_base as u32) as u32) as i16 as i32;
                let pb = bus.read16((0x0700_0000 + matrix_base as u32 + 8) as u32) as i16 as i32;
                let pc = bus.read16((0x0700_0000 + matrix_base as u32 + 16) as u32) as i16 as i32;
                let pd = bus.read16((0x0700_0000 + matrix_base as u32 + 24) as u32) as i16 as i32;

                let sprite_height = if is_double_size { base_height * 2 } else { base_height };
                let sprite_width = if is_double_size { base_width * 2 } else { base_width };

                // Center of sprite
                let cx = sx + base_width as i16 / 2;
                let cy = sy + base_height as i16 / 2;

                let mapping_1d = self.dispcnt & 0x0040 != 0;
                let is_8bpp = attr0 & 0x0080 != 0;

                for screen_y in 0..sprite_height {
                    let draw_y = cy - sprite_height as i16 / 2 + screen_y as i16;
                    if draw_y as usize != y as i16 as usize || draw_y < 0 || draw_y >= 160 {
                        continue;
                    }

                    for screen_x in 0..sprite_width {
                        let draw_x = cx - sprite_width as i16 / 2 + screen_x as i16;
                        if draw_x < 0 || draw_x >= 240 {
                            continue;
                        }

                        // Transform texture coordinates
                        let rel_x = screen_x as i32 - sprite_width as i32 / 2;
                        let rel_y = screen_y as i32 - sprite_height as i32 / 2;
                        let tex_x = (pa * rel_x + pb * rel_y) / 256 + base_width as i32 / 2;
                        let tex_y = (pc * rel_x + pd * rel_y) / 256 + base_height as i32 / 2;

                        if tex_x < 0 || tex_x >= base_width as i32 || tex_y < 0 || tex_y >= base_height as i32 {
                            continue;
                        }

                        let tile_x = tex_x as usize % 8;
                        let tile_y = tex_y as usize % 8;

                        let tiles_per_row = if mapping_1d { base_width / 8 } else { 32 };
                        let tile_offset = (tex_y as usize / 8) * tiles_per_row + (tex_x as usize / 8);
                        let tile_number = tile_num + tile_offset as u16;

                        let tile_data_addr = if is_8bpp {
                            0x0600_0000 + (tile_number as usize) * 64
                        } else {
                            0x0600_0000 + (tile_number as usize) * 32
                        };

                        let color_index = if is_8bpp {
                            let byte_offset = tile_y * 8 + tile_x;
                            bus.read8((tile_data_addr + byte_offset) as u32)
                        } else {
                            let byte_offset = tile_y * 4 + tile_x / 2;
                            let byte = bus.read8((tile_data_addr + byte_offset) as u32);
                            if tile_x % 2 == 0 { byte & 0x0F } else { (byte >> 4) & 0x0F }
                        };

                        if color_index == 0 { continue; }

                        let palette_addr = if is_8bpp {
                            0x0500_0200 + (color_index as usize) * 2
                        } else {
                            0x0500_0200 + (palette as usize) * 32 + (color_index as usize) * 2
                        };

                        let color = bus.read16(palette_addr as u32);
                        let r = ((color & 0x001F) as u8) << 3;
                        let g = (((color >> 5) & 0x001F) as u8) << 3;
                        let b = (((color >> 10) & 0x001F) as u8) << 3;

                        let idx = (y * SCREEN_WIDTH + draw_x as usize) * 3;
                        self.frame_buffer[idx] = r;
                        self.frame_buffer[idx + 1] = g;
                        self.frame_buffer[idx + 2] = b;
                    }
                }
            } else {
                // Regular (non-affine) sprite rendering
                if y as i16 >= sy && (y as i16) < sy + base_height as i16 {
                    let mapping_1d = self.dispcnt & 0x0040 != 0;
                    let is_8bpp = attr0 & 0x0080 != 0;

                    for sx_off in 0..base_width {
                        let px = sx + sx_off as i16;
                        if px < 0 || px >= 240 { continue; }

                        let py = y as i16 - sy;
                        let tile_x = sx_off % 8;
                        let tile_y = (py % 8) as usize;

                        let tiles_per_row = if mapping_1d { base_width as u16 / 8 } else { 32 };
                        let tile_offset = (py as u16 / 8) * tiles_per_row + (sx_off as u16 / 8);
                        let tile_number = tile_num + tile_offset;

                        let tile_data_addr = if bitmap_mode {
                            // Bitmap mode: tile_num is the base, offset is linear
                            0x0600_0000 + (tile_num as usize) * 64 + tile_offset as usize * 64
                        } else if is_8bpp {
                            0x0600_0000 + (tile_number as usize) * 64
                        } else {
                            0x0600_0000 + (tile_number as usize) * 32
                        };

                        let color_index = if bitmap_mode {
                            // Bitmap sprites are always 8bpp
                            let byte_offset = tile_y * 8 + tile_x;
                            bus.read8((tile_data_addr + byte_offset) as u32)
                        } else if is_8bpp {
                            let byte_offset = tile_y * 8 + tile_x;
                            bus.read8((tile_data_addr + byte_offset) as u32)
                        } else {
                            let byte_offset = tile_y * 4 + tile_x / 2;
                            let byte = bus.read8((tile_data_addr + byte_offset) as u32);
                            if tile_x % 2 == 0 { byte & 0x0F } else { (byte >> 4) & 0x0F }
                        };

                        if color_index == 0 { continue; }

                        let palette_addr = if bitmap_mode || is_8bpp {
                            0x0500_0200 + (color_index as usize) * 2
                        } else {
                            0x0500_0200 + (palette as usize) * 32 + (color_index as usize) * 2
                        };

                        let color = bus.read16(palette_addr as u32);
                        let r = ((color & 0x001F) as u8) << 3;
                        let g = (((color >> 5) & 0x001F) as u8) << 3;
                        let b = (((color >> 10) & 0x001F) as u8) << 3;

                        let idx = (y * SCREEN_WIDTH + px as usize) * 3;
                        self.frame_buffer[idx] = r;
                        self.frame_buffer[idx + 1] = g;
                        self.frame_buffer[idx + 2] = b;
                    }
                }
            }
        }
    }

    // ========================================================================
    // BG pixel lookup (standard tiled)
    // ========================================================================

    fn get_bg_pixel(
        &self, bg: usize, screen_x: usize, screen_y: usize,
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
            1 => if tile_x >= 32 { 1024 } else { 0 },
            2 => if tile_y >= 32 { 1024 } else { 0 },
            3 => ((tile_x / 32) + (tile_y / 32) * 2) * 1024,
            _ => 0,
        };

        let tile_index_in_block = (tile_y % 32) * 32 + (tile_x % 32);
        let screen_entry = screen_block_offset + tile_index_in_block;
        let screen_base = ((cnt >> 8) & 0x1F) as usize * 0x800;
        let screen_entry_addr = 0x0600_0000 + screen_base + screen_entry * 2;
        let screen_entry_value = bus.read16(screen_entry_addr as u32);
        let tile_number = screen_entry_value & 0x03FF;
        let char_base = ((cnt >> 2) & 3) as usize * 0x4000;

        (scrolled_x % 8, scrolled_y % 8, tile_number, char_base, 0, is_8bpp)
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
            let idx = (y * SCREEN_WIDTH + x) * 3;
            self.frame_buffer[idx] = r;
            self.frame_buffer[idx + 1] = g;
            self.frame_buffer[idx + 2] = b;
        }
    }

    // ========================================================================
    // Mode 4: Bitmap 8bpp (2 framebuffers)
    // ========================================================================

    fn render_mode4_scanline(&mut self, y: usize, bus: &mut super::memory::MemoryBus) {
        let page = if self.dispcnt & 0x0010 != 0 { 0xA000 } else { 0x0000 };
        let base = 0x0600_0000 + page + y * 240;
        for x in 0..SCREEN_WIDTH {
            let color_index = bus.read8((base + x) as u32);
            let color = bus.read16((0x0500_0000 + color_index as usize * 2) as u32);
            let r = ((color & 0x001F) as u8) << 3;
            let g = (((color >> 5) & 0x001F) as u8) << 3;
            let b = (((color >> 10) & 0x001F) as u8) << 3;
            let idx = (y * SCREEN_WIDTH + x) * 3;
            self.frame_buffer[idx] = r;
            self.frame_buffer[idx + 1] = g;
            self.frame_buffer[idx + 2] = b;
        }
    }

    // ========================================================================
    // Mode 5: Bitmap 16bpp (2 framebuffers, 160x128 each)
    // ========================================================================

    fn render_mode5_scanline(&mut self, y: usize, bus: &mut super::memory::MemoryBus) {
        // Mode 5: two 160x128 16bpp framebuffers
        // Page select from DISPCNT bit 4
        let page = if self.dispcnt & 0x0010 != 0 { 0xA000 } else { 0x0000 };

        // Only 128 lines per page; lines 128-159 show garbage (use last line)
        let fb_y = if y >= 128 { 127 } else { y };

        // Each line is 160 pixels * 2 bytes = 320 bytes
        let base = 0x0600_0000 + page + fb_y * 320;

        for x in 0..SCREEN_WIDTH {
            // Only 160 pixels wide; beyond that, show black
            if x >= 160 {
                let idx = (y * SCREEN_WIDTH + x) * 3;
                self.frame_buffer[idx] = 0;
                self.frame_buffer[idx + 1] = 0;
                self.frame_buffer[idx + 2] = 0;
                continue;
            }

            let color = bus.read16((base + x * 2) as u32);
            let r = ((color & 0x001F) as u8) << 3;
            let g = (((color >> 5) & 0x001F) as u8) << 3;
            let b = (((color >> 10) & 0x001F) as u8) << 3;
            let idx = (y * SCREEN_WIDTH + x) * 3;
            self.frame_buffer[idx] = r;
            self.frame_buffer[idx + 1] = g;
            self.frame_buffer[idx + 2] = b;
        }
    }

    // ========================================================================
    // Color effects (alpha blend, brighten, darken)
    // ========================================================================

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
        let eva = (self.bldalpha & 0x001F) as u32;
        let evb = ((self.bldalpha >> 8) & 0x001F) as u32;
        // Simplified: blend first two target pixels found
        // Full implementation would track per-pixel layer info
        let _ = (y, eva, evb);
    }

    fn apply_brightness_inc(&mut self, y: usize) {
        let ey = self.bldy as u32;
        if ey == 0 { return; }
        for x in 0..SCREEN_WIDTH {
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
        let ey = self.bldy as u32;
        if ey == 0 { return; }
        for x in 0..SCREEN_WIDTH {
            let idx = (y * SCREEN_WIDTH + x) * 3;
            let r = self.frame_buffer[idx] as u32;
            let g = self.frame_buffer[idx + 1] as u32;
            let b = self.frame_buffer[idx + 2] as u32;
            self.frame_buffer[idx] = (r - r * ey / 16).max(0) as u8;
            self.frame_buffer[idx + 1] = (g - g * ey / 16).max(0) as u8;
            self.frame_buffer[idx + 2] = (b - b * ey / 16).max(0) as u8;
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

    pub fn frame_buffer(&self) -> &[u8; FRAME_SIZE] {
        &self.frame_buffer
    }

    pub fn frame_buffer_mut(&mut self) -> &mut [u8; FRAME_SIZE] {
        &mut self.frame_buffer
    }
}
