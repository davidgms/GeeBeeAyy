const SCREEN_WIDTH: usize = 240;
const SCREEN_HEIGHT: usize = 160;
const FRAME_SIZE: usize = SCREEN_WIDTH * SCREEN_HEIGHT * 3;

pub struct Ppu {
    frame_buffer: [u8; FRAME_SIZE],
    pub scanline: u16,
    pub mode: u8,
    vblank: bool,
    hblank: bool,
    vblank_irq: bool,
    cycle_counter: u32,
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
    // Display control
    pub dispcnt: u16,
    force_blank: bool,
    display_off: bool,
    bg_mode: u8,
    // BG enable bits (from DISPCNT)
    bg0_enable: bool,
    bg1_enable: bool,
    bg2_enable: bool,
    bg3_enable: bool,
    obj_enable: bool,
}

impl Ppu {
    pub fn new() -> Self {
        Self {
            frame_buffer: [0; FRAME_SIZE],
            scanline: 0,
            mode: 0,
            vblank: false,
            hblank: false,
            vblank_irq: false,
            cycle_counter: 0,
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
            dispcnt: 0,
            force_blank: false,
            display_off: false,
            bg_mode: 0,
            bg0_enable: false,
            bg1_enable: false,
            bg2_enable: false,
            bg3_enable: false,
            obj_enable: false,
        }
    }

    pub fn tick(&mut self, cycles: u32, bus: &mut super::memory::MemoryBus) {
        self.cycle_counter += cycles;

        // GBA timing: 1232 cycles per scanline
        if self.cycle_counter >= 1232 {
            self.cycle_counter -= 1232;
            self.sync_from_bus(bus);
            self.render_scanline(bus);
            self.scanline += 1;

            if self.scanline == 160 && !self.vblank {
                self.vblank = true;
                self.vblank_irq = true;
            }

            if self.scanline >= 228 {
                self.scanline = 0;
                self.vblank = false;
            }
        }

        // HBlank detection
        self.hblank = self.cycle_counter >= 960;

        // Write DISPSTAT
        let dispstat = ((self.vblank as u16) << 0)
            | ((self.hblank as u16) << 1)
            | (((self.scanline >= 160 && self.scanline <= 226) as u16) << 2);
        bus.write16(0x0400_0004, dispstat);

        // Write VCOUNT
        bus.write16(0x0400_0006, self.scanline);
    }

    fn sync_from_bus(&mut self, bus: &mut super::memory::MemoryBus) {
        // Read DISPCNT
        self.dispcnt = bus.read16(0x0400_0000);
        self.force_blank = self.dispcnt & 0x0080 != 0;
        self.display_off = self.dispcnt & 0x0080 == 0;
        self.bg_mode = (self.dispcnt & 0x0007) as u8;
        self.bg0_enable = self.dispcnt & 0x0100 != 0;
        self.bg1_enable = self.dispcnt & 0x0200 != 0;
        self.bg2_enable = self.dispcnt & 0x0400 != 0;
        self.bg3_enable = self.dispcnt & 0x0800 != 0;
        self.obj_enable = self.dispcnt & 0x1000 != 0;

        // Read BG control registers
        self.bg0cnt = bus.read16(0x0400_0008);
        self.bg1cnt = bus.read16(0x0400_000A);
        self.bg2cnt = bus.read16(0x0400_000C);
        self.bg3cnt = bus.read16(0x0400_000E);

        // Read BG scroll registers
        self.bg0hofs = bus.read16(0x0400_0010);
        self.bg0vofs = bus.read16(0x0400_0012);
        self.bg1hofs = bus.read16(0x0400_0014);
        self.bg1vofs = bus.read16(0x0400_0016);
        self.bg2hofs = bus.read16(0x0400_0018);
        self.bg2vofs = bus.read16(0x0400_001A);
        self.bg3hofs = bus.read16(0x0400_001C);
        self.bg3vofs = bus.read16(0x0400_001E);

        self.mode = self.bg_mode;
    }

    fn render_scanline(&mut self, bus: &mut super::memory::MemoryBus) {
        if self.force_blank || self.display_off {
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
            3 => self.render_mode3_scanline(y, bus),
            4 => self.render_mode4_scanline(y, bus),
            _ => {}
        }
    }

    /// Mode 0: 4 tiled backgrounds (BG0-BG3)
    fn render_mode0_scanline(&mut self, y: usize, bus: &mut super::memory::MemoryBus) {
        // Render backgrounds in priority order (BG3 lowest, BG0 highest)
        // For each pixel, we check which BG is enabled and has highest priority

        // Create a priority list: (priority, bg_index)
        let mut bg_list: Vec<(u8, usize)> = Vec::new();
        if self.bg0_enable { bg_list.push(((self.bg0cnt & 3) as u8, 0)); }
        if self.bg1_enable { bg_list.push(((self.bg1cnt & 3) as u8, 1)); }
        if self.bg2_enable { bg_list.push(((self.bg2cnt & 3) as u8, 2)); }
        if self.bg3_enable { bg_list.push(((self.bg3cnt & 3) as u8, 3)); }

        // Sort by priority (lower number = higher priority)
        bg_list.sort_by_key(|&(p, _)| p);

        // Render OBJ sprites if enabled
        if self.obj_enable {
            self.render_obj_scanline(y, bus);
        }

        // Render BGs with priority
        for x in 0..SCREEN_WIDTH {
            for &(_, bg) in &bg_list {
                let (tile_local_x, tile_local_y, screen_entry, char_base, palette_base, is_8bpp) =
                    self.get_bg_pixel(bg, x, y, bus);

                if !is_8bpp {
                    // 4bpp: 16 colors, 32 bytes per tile
                    let tile_data_addr = char_base + screen_entry as usize * 32;
                    let byte_offset = tile_local_y * 4 + tile_local_x / 2;
                    let byte = bus.read8((tile_data_addr + byte_offset) as u32);
                    let color_index = if tile_local_x % 2 == 0 {
                        byte & 0x0F
                    } else {
                        (byte >> 4) & 0x0F
                    };

                    if color_index == 0 {
                        continue; // Transparent
                    }

                    // Read color from palette (BG palette at 0x05000000)
                    let palette_addr = 0x0500_0000 + palette_base + (color_index as usize) * 2;
                    let color = bus.read16(palette_addr as u32);
                    let r = ((color & 0x001F) as u8) << 3;
                    let g = (((color >> 5) & 0x001F) as u8) << 3;
                    let b = (((color >> 10) & 0x001F) as u8) << 3;

                    let idx = (y * SCREEN_WIDTH + x) * 3;
                    self.frame_buffer[idx] = r;
                    self.frame_buffer[idx + 1] = g;
                    self.frame_buffer[idx + 2] = b;
                    break;
                } else {
                    // 8bpp: 256 colors, 64 bytes per tile
                    let tile_data_addr = char_base + screen_entry as usize * 64;
                    let byte_offset = tile_local_y * 8 + tile_local_x;
                    let color_index = bus.read8((tile_data_addr + byte_offset) as u32);

                    if color_index == 0 {
                        continue; // Transparent
                    }

                    // Read color from palette (BG palette at 0x05000000)
                    let palette_addr = 0x0500_0000 + (color_index as usize) * 2;
                    let color = bus.read16(palette_addr as u32);
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
    }

    /// Render OBJ sprites for the current scanline.
    fn render_obj_scanline(&mut self, y: usize, bus: &mut super::memory::MemoryBus) {
        // OAM is at 0x07000000, 128 sprites, 8 bytes each = 1024 bytes
        // Each sprite has 3 attribute words (Attr0, Attr1, Attr2)

        // Iterate through all 128 sprites
        for sprite in 0..128u16 {
            let oam_addr = 0x0700_0000 + (sprite as u32) * 8;

            // Read Attr0 (16-bit)
            let attr0 = bus.read16(oam_addr);
            // Read Attr1 (16-bit)
            let attr1 = bus.read16(oam_addr + 2);
            // Read Attr2 (16-bit)
            let attr2 = bus.read16(oam_addr + 4);

            // Check if sprite is disabled (Attr0 bit 9 = 1 means disabled)
            if attr0 & 0x0200 != 0 {
                continue;
            }

            // Y coordinate (Attr0 bits 0-7)
            let sy = (attr0 & 0x00FF) as i16;
            let sy = if sy > 127 { sy - 256 } else { sy };

            // Shape (Attr0 bits 14-15)
            let shape = (attr0 >> 14) & 3;

            // X coordinate (Attr1 bits 0-8)
            let sx = (attr1 & 0x01FF) as i16;
            let sx = if sx > 255 { sx - 512 } else { sx };

            // Size (Attr1 bits 14-15)
            let size = (attr1 >> 14) & 3;

            // Tile number (Attr2 bits 0-9)
            let tile_num = attr2 & 0x03FF;

            // Palette number (Attr2 bits 12-15) - only for 4bpp
            let palette = ((attr2 >> 12) & 0xF) as u8;

            // Determine sprite dimensions based on shape and size
            let (sprite_width, sprite_height) = match shape {
                0b00 => { // Square
                    match size {
                        0b00 => (8, 8),
                        0b01 => (16, 16),
                        0b10 => (32, 32),
                        0b11 => (64, 64),
                        _ => (8, 8),
                    }
                }
                0b01 => { // Horizontal rectangle
                    match size {
                        0b00 => (16, 8),
                        0b01 => (32, 8),
                        0b10 => (32, 16),
                        0b11 => (64, 32),
                        _ => (16, 8),
                    }
                }
                0b10 => { // Vertical rectangle
                    match size {
                        0b00 => (8, 16),
                        0b01 => (8, 32),
                        0b10 => (16, 32),
                        0b11 => (32, 64),
                        _ => (8, 16),
                    }
                }
                _ => continue, // Prohibited
            };

            // Check if sprite intersects current scanline
            if y as i16 >= sy && (y as i16) < sy + sprite_height as i16 {
                // Check 1D/2D mapping (DISPCNT bit 6)
                let mapping_1d = self.dispcnt & 0x0040 != 0;
                let is_8bpp = attr0 & 0x0080 != 0;

                for sx_off in 0..sprite_width {
                    let px = sx + sx_off as i16;
                    if px < 0 || px >= 240 {
                        continue;
                    }

                    let py = y as i16 - sy;
                    let tile_x = sx_off % 8;
                    let tile_y = (py % 8) as usize;

                    // Calculate tile offset
                    let tiles_per_row = if mapping_1d {
                        sprite_width as u16 / 8
                    } else {
                        32 // 2D mapping: 32 tiles per row in VRAM
                    };

                    let tile_offset = (py as u16 / 8) * tiles_per_row + (sx_off as u16 / 8);
                    let tile_number = tile_num + tile_offset;

                    // Calculate tile data address in VRAM
                    let tile_data_addr = if is_8bpp {
                        0x0600_0000 + (tile_number as usize) * 64
                    } else {
                        0x0600_0000 + (tile_number as usize) * 32
                    };

                    let color_index = if is_8bpp {
                        // 8bpp: 256 colors
                        let byte_offset = tile_y * 8 + tile_x;
                        bus.read8((tile_data_addr + byte_offset) as u32)
                    } else {
                        // 4bpp: 16 colors
                        let byte_offset = tile_y * 4 + tile_x / 2;
                        let byte = bus.read8((tile_data_addr + byte_offset) as u32);
                        if tile_x % 2 == 0 {
                            byte & 0x0F
                        } else {
                            (byte >> 4) & 0x0F
                        }
                    };

                    if color_index == 0 {
                        continue; // Transparent
                    }

                    // Read color from OBJ palette (0x05000200)
                    let palette_addr = if is_8bpp {
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

    /// Get pixel info for a BG at the given screen position.
    /// Returns (tile_x, tile_y, screen_entry_index, char_base_addr, palette_base, is_8bpp)
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

        // Scroll the coordinates
        let scrolled_x = (screen_x + hofs as usize) & 0x1FF; // 512 pixel width
        let scrolled_y = (screen_y + vofs as usize) & 0x1FF; // 512 pixel height

        // Determine tile size (16x16 or 32x32) from BGxCNT bit 14
        let screen_size = (cnt >> 14) & 3;
        let is_8bpp = cnt & 0x0080 != 0;

        // Calculate which screen block and tile we're in
        // For 32x32 tiles: 32 tiles per screen, 32 screens = 1024 tiles
        let tiles_per_row = 32;
        let tile_x = scrolled_x / 8;
        let tile_y = scrolled_y / 8;

        // Determine screen block offset based on screen_size
        let screen_block_offset = match screen_size {
            0 => 0, // 32x32
            1 => {
                // 64x32
                if tile_x >= 32 { 1024 } else { 0 }
            }
            2 => {
                // 32x64
                if tile_y >= 32 { 1024 } else { 0 }
            }
            3 => {
                // 64x64
                let block = (tile_x / 32) + (tile_y / 32) * 2;
                block as usize * 1024
            }
            _ => 0,
        };

        // Calculate tile index within screen block
        let tile_index_in_block = (tile_y % 32) * tiles_per_row + (tile_x % 32);
        let screen_entry = screen_block_offset + tile_index_in_block;

        // Screen Base Address from BGxCNT bits 8-12 (screen base / 2KB)
        let screen_base = ((cnt >> 8) & 0x1F) as usize * 0x800;
        let screen_entry_addr = 0x0600_0000 + screen_base + screen_entry * 2;

        // Read the 16-bit screen entry (tile number)
        let screen_entry_value = bus.read16(screen_entry_addr as u32);
        let tile_number = screen_entry_value & 0x03FF; // 10-bit tile number

        // Char Base Address from BGxCNT bits 2-3 (char base / 16KB)
        let char_base = ((cnt >> 2) & 3) as usize * 0x4000;

        // Tile local coordinates
        let tile_local_x = scrolled_x % 8;
        let tile_local_y = scrolled_y % 8;

        (tile_local_x, tile_local_y, tile_number, char_base, 0, is_8bpp)
    }

    /// Mode 3: Bitmap 16bpp (single framebuffer)
    fn render_mode3_scanline(&mut self, y: usize, bus: &mut super::memory::MemoryBus) {
        let base = 0x0600_0000 + y * 480;
        for x in 0..SCREEN_WIDTH {
            let addr = base + x * 2;
            let color = bus.read16(addr as u32);
            let r = ((color & 0x001F) as u8) << 3;
            let g = (((color >> 5) & 0x001F) as u8) << 3;
            let b = (((color >> 10) & 0x001F) as u8) << 3;

            let idx = (y * SCREEN_WIDTH + x) * 3;
            self.frame_buffer[idx] = r;
            self.frame_buffer[idx + 1] = g;
            self.frame_buffer[idx + 2] = b;
        }
    }

    /// Mode 4: Bitmap 8bpp (2 framebuffers)
    fn render_mode4_scanline(&mut self, y: usize, bus: &mut super::memory::MemoryBus) {
        let page = if self.dispcnt & 0x0010 != 0 { 0xA000 } else { 0x0000 };
        let base = 0x0600_0000 + page + y * 240;
        for x in 0..SCREEN_WIDTH {
            let color_index = bus.read8((base + x) as u32);
            // Read color from palette (BG palette at 0x05000000)
            let palette_addr = 0x0500_0000 + (color_index as usize) * 2;
            let color = bus.read16(palette_addr as u32);
            let r = ((color & 0x001F) as u8) << 3;
            let g = (((color >> 5) & 0x001F) as u8) << 3;
            let b = (((color >> 10) & 0x001F) as u8) << 3;

            let idx = (y * SCREEN_WIDTH + x) * 3;
            self.frame_buffer[idx] = r;
            self.frame_buffer[idx + 1] = g;
            self.frame_buffer[idx + 2] = b;
        }
    }

    /// Returns true if VBlank just occurred (edge-triggered).
    pub fn vblank_pending(&mut self) -> bool {
        if self.vblank_irq {
            self.vblank_irq = false;
            true
        } else {
            false
        }
    }

    pub fn frame_buffer(&self) -> &[u8; FRAME_SIZE] {
        &self.frame_buffer
    }
}
