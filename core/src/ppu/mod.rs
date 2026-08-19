const SCREEN_WIDTH: usize = 240;
const SCREEN_HEIGHT: usize = 160;
const FRAME_SIZE: usize = SCREEN_WIDTH * SCREEN_HEIGHT * 3;

pub struct Ppu {
    frame_buffer: [u8; FRAME_SIZE],
    scanline: u16,
    mode: u8,
    vblank: bool,
    hblank: bool,
    cycle_counter: u32,
}

impl Ppu {
    pub fn new() -> Self {
        Self {
            frame_buffer: [0; FRAME_SIZE],
            scanline: 0,
            mode: 0,
            vblank: false,
            hblank: false,
            cycle_counter: 0,
        }
    }

    pub fn tick(&mut self, cycles: u32, bus: &mut super::memory::MemoryBus) {
        self.cycle_counter += cycles;

        // GBA timing: 1232 cycles per scanline
        if self.cycle_counter >= 1232 {
            self.cycle_counter -= 1232;
            self.render_scanline(bus);
            self.scanline += 1;

            if self.scanline >= 160 && !self.vblank {
                self.vblank = true;
                // TODO: Trigger VBlank IRQ
            }

            if self.scanline >= 228 {
                self.scanline = 0;
                self.vblank = false;
                // TODO: Copy internal buffer to display
            }
        }

        // HBlank detection
        self.hblank = self.cycle_counter >= 960;
    }

    fn render_scanline(&mut self, _bus: &mut super::memory::MemoryBus) {
        let _ = SCREEN_WIDTH;
        let _ = SCREEN_HEIGHT;
        // TODO: Implement scanline rendering based on BG mode
        // Modes 0-2: tiled backgrounds
        // Modes 3-5: bitmap modes
    }

    pub fn frame_buffer(&self) -> &[u8; FRAME_SIZE] {
        &self.frame_buffer
    }
}
