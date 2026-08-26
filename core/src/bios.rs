// GBA BIOS HLE (High-Level Emulation)
// Handles common SWI calls without requiring a real BIOS ROM.

use super::cpu::Cpu;
use super::memory::MemoryBus;

/// Handle a BIOS SWI call. Returns true if handled.
pub fn handle_swi(swi_num: u32, cpu: &mut Cpu, bus: &mut MemoryBus) -> bool {
    match swi_num {
        0x00 => handle_soft_reset(cpu, bus),
        0x01 => handle_register_ram_reset(cpu, bus),
        0x02 => handle_halt(cpu, bus),
        0x04 => handle_intr_wait(cpu, bus),
        0x05 => handle_vblank_intr_wait(cpu, bus),
        0x06 => handle_div(cpu),
        0x07 => handle_div_arm(cpu),
        0x08 => handle_sqrt(cpu),
        0x0B => handle_cpu_set(cpu, bus),
        0x0E => handle_bg_affine_set(cpu, bus),
        0x0F => handle_obj_affine_set(cpu, bus),
        0x10 => handle_bit_unpack(cpu, bus),
        0x11 => handle_lz77_uncomp_wram(cpu, bus),
        0x12 => handle_lz77_uncomp_vram(cpu, bus),
        0x14 => handle_rl_uncomp_wram(cpu, bus),
        0x15 => handle_rl_uncomp_vram(cpu, bus),
        0x19 => handle_sound_bias(cpu, bus),
        _ => false,
    }
}

fn handle_soft_reset(_cpu: &mut Cpu, _bus: &mut MemoryBus) -> bool {
    true
}

fn handle_register_ram_reset(cpu: &mut Cpu, bus: &mut MemoryBus) -> bool {
    let flags = cpu.reg(0);
    if flags & 0x01 != 0 { for b in bus.ewram_data_mut().iter_mut() { *b = 0; } }
    if flags & 0x02 != 0 { for b in bus.iwram_data_mut().iter_mut() { *b = 0; } }
    if flags & 0x04 != 0 { for b in bus.palette_data_mut().iter_mut() { *b = 0; } }
    if flags & 0x08 != 0 { for b in bus.vram_data_mut().iter_mut() { *b = 0; } }
    if flags & 0x10 != 0 { for b in bus.oam_data_mut().iter_mut() { *b = 0; } }
    true
}

/// SWI 0x02: Halt — enters low-power state until interrupt
fn handle_halt(_cpu: &mut Cpu, bus: &mut MemoryBus) -> bool {
    bus.io.halt = true;
    true
}

/// SWI 0x04: IntrWait(r1=discardOldFlags, r2=IEFlags)
fn handle_intr_wait(cpu: &mut Cpu, bus: &mut MemoryBus) -> bool {
    let discard = cpu.reg(0) != 0;
    let ie_flags = (cpu.reg(1) & 0xFFFF) as u16;
    if discard {
        bus.io.if_ &= !ie_flags;
    }
    // Spin until the requested interrupt fires
    if bus.io.if_ & ie_flags == 0 {
        bus.io.halt = true;
    }
    true
}

/// SWI 0x05: VBlankIntrWait — waits specifically for VBlank
fn handle_vblank_intr_wait(cpu: &mut Cpu, bus: &mut MemoryBus) -> bool {
    cpu.set_reg(0, 1); // r1 = 1 (discard old flags)
    cpu.set_reg(1, 0x0001); // r2 = VBlank IRQ bit
    handle_intr_wait(cpu, bus)
}

fn handle_div(cpu: &mut Cpu) -> bool {
    let numerator = cpu.reg(0) as i32;
    let denominator = cpu.reg(1) as i32;
    if denominator == 0 {
        cpu.set_reg(0, 0);
        cpu.set_reg(1, 0);
    } else {
        cpu.set_reg(0, (numerator / denominator) as u32);
        cpu.set_reg(1, (numerator % denominator) as u32);
    }
    true
}

fn handle_div_arm(cpu: &mut Cpu) -> bool {
    let denominator = cpu.reg(0) as i32;
    let numerator = cpu.reg(1) as i32;
    if denominator == 0 {
        cpu.set_reg(0, 0);
        cpu.set_reg(1, 0);
    } else {
        cpu.set_reg(0, (numerator / denominator) as u32);
        cpu.set_reg(1, (numerator % denominator) as u32);
    }
    true
}

fn handle_sqrt(cpu: &mut Cpu) -> bool {
    let input = cpu.reg(0);
    let result = (input as f64).sqrt() as u32;
    cpu.set_reg(0, result);
    true
}

fn handle_cpu_set(cpu: &mut Cpu, bus: &mut MemoryBus) -> bool {
    let src = cpu.reg(0);
    let dst = cpu.reg(1);
    let cnt = cpu.reg(2);
    let count = cnt & 0x001F_FFFF;
    let is_32bit = cnt & 0x0400_0000 != 0;
    if is_32bit {
        for i in 0..count {
            let addr = dst.wrapping_add(i * 4);
            let val = bus.read32(src.wrapping_add(i * 4));
            bus.write32(addr, val);
        }
    } else {
        for i in 0..count {
            let addr = dst.wrapping_add(i * 2);
            let val = bus.read16(src.wrapping_add(i * 2));
            bus.write16(addr, val);
        }
    }
    true
}

fn handle_bg_affine_set(cpu: &mut Cpu, bus: &mut MemoryBus) -> bool {
    let src = cpu.reg(0);
    let dst = cpu.reg(1);
    let count = cpu.reg(2);
    for i in 0..count {
        let src_off = src + i * 20;
        let dst_off = dst + i * 16;
        let _orig_x = bus.read32(src_off) as i32;
        let _orig_y = bus.read32(src_off + 4) as i32;
        let _aff_x = bus.read32(src_off + 8) as i32;
        let _aff_y = bus.read32(src_off + 12) as i32;
        let _theta = bus.read16(src_off + 16) as i16;
        bus.write16(dst_off, 0x0100);
        bus.write16(dst_off + 2, 0);
        bus.write16(dst_off + 4, 0);
        bus.write16(dst_off + 6, 0x0100);
        bus.write32(dst_off + 8, 0);
        bus.write32(dst_off + 12, 0);
    }
    true
}

fn handle_obj_affine_set(cpu: &mut Cpu, bus: &mut MemoryBus) -> bool {
    let src = cpu.reg(0);
    let dst = cpu.reg(1);
    let count = cpu.reg(2);
    let stride = cpu.reg(3) as i32;
    for i in 0..count {
        let _src_off = src + i * 4;
        let _sx = bus.read16(src + i * 4) as i16 as i32;
        let _sy = bus.read16(src + i * 4 + 2) as i16 as i32;
        let _theta = bus.read16(src + i * 4 + 4) as i16;
        let dst_off = (dst as i32 + stride * i as i32) as u32;
        bus.write16(dst_off, 0x0100);
        bus.write16(dst_off + 2, 0);
    }
    true
}

fn handle_bit_unpack(cpu: &mut Cpu, bus: &mut MemoryBus) -> bool {
    let src = cpu.reg(0);
    let dst = cpu.reg(1);
    let mut offset = 0u32;
    loop {
        let val = bus.read8(src + offset);
        if val == 0 { break; }
        bus.write8(dst + offset, val);
        offset += 1;
        if offset > 0x10000 { break; }
    }
    true
}

fn handle_lz77_uncomp_wram(cpu: &mut Cpu, bus: &mut MemoryBus) -> bool {
    let src = cpu.reg(0);
    let dst = cpu.reg(1);
    lz77_decompress(src, dst, bus);
    true
}

fn handle_lz77_uncomp_vram(cpu: &mut Cpu, bus: &mut MemoryBus) -> bool {
    let src = cpu.reg(0);
    let dst = cpu.reg(1);
    lz77_decompress(src, dst, bus);
    true
}

fn lz77_decompress(src: u32, dst: u32, bus: &mut MemoryBus) {
    let header = bus.read32(src);
    let _decompressed_size = header & 0x00FF_FFFF;
    let mut src_pos = src + 4;
    let mut dst_pos = dst;
    loop {
        let flags = bus.read8(src_pos);
        src_pos += 1;
        for bit in 0..8 {
            if flags & (0x80 >> bit) != 0 {
                let byte1 = bus.read8(src_pos);
                let byte2 = bus.read8(src_pos + 1);
                src_pos += 2;
                let length = ((byte1 >> 4) & 0x0F) as u32 + 3;
                let offset = (((byte1 & 0x0F) as u32) << 8) | (byte2 as u32) + 1;
                for _ in 0..length {
                    let val = bus.read8(dst_pos.wrapping_sub(offset));
                    bus.write8(dst_pos, val);
                    dst_pos += 1;
                }
            } else {
                let val = bus.read8(src_pos);
                src_pos += 1;
                bus.write8(dst_pos, val);
                dst_pos += 1;
            }
        }
    }
}

fn handle_rl_uncomp_wram(cpu: &mut Cpu, bus: &mut MemoryBus) -> bool {
    let src = cpu.reg(0);
    let dst = cpu.reg(1);
    rl_decompress(src, dst, bus);
    true
}

fn handle_rl_uncomp_vram(cpu: &mut Cpu, bus: &mut MemoryBus) -> bool {
    let src = cpu.reg(0);
    let dst = cpu.reg(1);
    rl_decompress(src, dst, bus);
    true
}

fn rl_decompress(src: u32, dst: u32, bus: &mut MemoryBus) {
    let header = bus.read32(src);
    let _decompressed_size = header & 0x00FF_FFFF;
    let is_8bit = header & 0x0800_0000 != 0;
    let mut src_pos = src + 4;
    let mut dst_pos = dst;
    loop {
        let data = bus.read8(src_pos);
        src_pos += 1;
        if data & 0x80 != 0 {
            let count = (data & 0x7F) + 3;
            if is_8bit {
                let val = bus.read8(src_pos);
                src_pos += 1;
                for _ in 0..count { bus.write8(dst_pos, val); dst_pos += 1; }
            } else {
                let val = bus.read16(src_pos);
                src_pos += 2;
                for _ in 0..count { bus.write16(dst_pos, val); dst_pos += 2; }
            }
        } else {
            let count = data + 1;
            for _ in 0..count {
                if is_8bit {
                    let val = bus.read8(src_pos);
                    src_pos += 1;
                    bus.write8(dst_pos, val); dst_pos += 1;
                } else {
                    let val = bus.read16(src_pos);
                    src_pos += 2;
                    bus.write16(dst_pos, val); dst_pos += 2;
                }
            }
        }
    }
}

fn handle_sound_bias(_cpu: &mut Cpu, _bus: &mut MemoryBus) -> bool {
    true
}
