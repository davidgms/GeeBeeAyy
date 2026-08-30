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
        0x09 => handle_arc_tan(cpu),
        0x0A => handle_arc_tan2(cpu),
        0x0B => handle_cpu_set(cpu, bus),
        0x0C => handle_cpu_fast_set(cpu, bus),
        0x0E => handle_bg_affine_set(cpu, bus),
        0x0F => handle_obj_affine_set(cpu, bus),
        0x10 => handle_bit_unpack(cpu, bus),
        0x11 => handle_lz77_uncomp_wram(cpu, bus),
        0x12 => handle_lz77_uncomp_vram(cpu, bus),
        0x14 => handle_rl_uncomp_wram(cpu, bus),
        0x15 => handle_rl_uncomp_vram(cpu, bus),
        0x16 => handle_diff8bit_unfilter_wram(cpu, bus),
        0x17 => handle_diff8bit_unfilter_vram(cpu, bus),
        0x18 => handle_diff16bit_unfilter(cpu, bus),
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
/// SWI 04h. GBATEK: "Continues to wait in Halt state until one (or more) of
/// the specified interrupt(s) do occur. **The function forcefully sets
/// IME=1.**"
fn handle_intr_wait(cpu: &mut Cpu, bus: &mut MemoryBus) -> bool {
    let discard = cpu.reg(0) != 0;
    let wanted = (cpu.reg(1) & 0xFFFF) as u16;

    // Forcing IME is not optional: a game may call this with interrupts
    // globally masked and rely on the BIOS to enable them.
    bus.io.ime = 1;

    if discard {
        bus.io.if_ &= !wanted;
    }
    if bus.io.if_ & wanted == 0 {
        bus.io.halt = true;
    }
    true
}

/// SWI 0x05: VBlankIntrWait — waits specifically for VBlank
/// SWI 05h. GBATEK: "sets r0=1, r1=1, and then calls IntrWait".
fn handle_vblank_intr_wait(cpu: &mut Cpu, bus: &mut MemoryBus) -> bool {
    cpu.set_reg(0, 1);      // discard old flags
    cpu.set_reg(1, 0x0001); // wait for VBlank
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

/// The BIOS arc tangent polynomial.
///
/// GBATEK, BIOS Arithmetic Functions: the argument is a 16bit fixed point tan
/// (1 sign bit, 1 integral bit, 14 fractional bits) and the result covers
/// -PI/2..PI/2 as C000h..4000h, i.e. a full turn is 10000h. GBATEK also warns
/// that "there is a problem in accuracy with THETA<-PI/4, PI/4<THETA": the
/// series below genuinely diverges past tan = 1.0, and that inaccuracy is part
/// of the observable hardware behaviour, so it is reproduced rather than fixed.
/// The coefficients are the ones in the BIOS ROM (cross-checked against mGBA's
/// `_ArcTan`); GBATEK documents only the interface, not the series.
fn arc_tan(tan: i32) -> i32 {
    let a = -((tan * tan) >> 14);
    let mut b = ((0xA9 * a) >> 14) + 0x390;
    b = ((b * a) >> 14) + 0x91C;
    b = ((b * a) >> 14) + 0xFB60;
    b = ((b * a) >> 14) + 0x16C9;
    b = ((b * a) >> 14) + 0x2081;
    b = ((b * a) >> 14) + 0x3B10;
    b = ((b * a) >> 14) + 0xA2F9;
    (tan * b) >> 16
}

/// SWI 0x09: ArcTan(r0 = tan) -> r0 = angle in C000h..4000h.
fn handle_arc_tan(cpu: &mut Cpu) -> bool {
    let tan = cpu.reg(0) as i16 as i32;
    cpu.set_reg(0, arc_tan(tan) as u32);
    true
}

/// SWI 0x0A: ArcTan2(r0 = x, r1 = y) -> r0 = 0000h..FFFFh for 0 <= THETA < 2PI.
///
/// GBATEK gives the interface only. The quadrant folding below is the BIOS
/// algorithm: it always feeds `arc_tan` the smaller of |y/x| and |x/y| so the
/// series stays inside its accurate range, then rotates the result into the
/// right quadrant.
fn handle_arc_tan2(cpu: &mut Cpu) -> bool {
    let x = cpu.reg(0) as i16 as i32;
    let y = cpu.reg(1) as i16 as i32;
    let result = arc_tan2(x, y);
    cpu.set_reg(0, (result as u32) & 0xFFFF);
    true
}

fn arc_tan2(x: i32, y: i32) -> i32 {
    if y == 0 {
        return if x >= 0 { 0x0000 } else { 0x8000 };
    }
    if x == 0 {
        return if y >= 0 { 0x4000 } else { 0xC000 };
    }
    if y >= 0 {
        if x >= 0 {
            if x >= y {
                return arc_tan((y << 14) / x);
            }
        } else if -x >= y {
            return arc_tan((y << 14) / x) + 0x8000;
        }
        0x4000 - arc_tan((x << 14) / y)
    } else {
        if x <= 0 {
            if -x > -y {
                return arc_tan((y << 14) / x) + 0x8000;
            }
        } else if x >= -y {
            return arc_tan((y << 14) / x) + 0x10000;
        }
        0xC000 - arc_tan((x << 14) / y)
    }
}

/// SWI 0x0B: CpuSet(r0 = src, r1 = dst, r2 = length/mode).
///
/// GBATEK, BIOS Memory Copy: bits 0-20 are the (half)word count, bit 24 fixes
/// the source address (0 = copy, 1 = fill), bit 26 is the unit size
/// (0 = 16bit, 1 = 32bit).
fn handle_cpu_set(cpu: &mut Cpu, bus: &mut MemoryBus) -> bool {
    let src = cpu.reg(0);
    let dst = cpu.reg(1);
    let cnt = cpu.reg(2);
    let count = cnt & 0x001F_FFFF;
    let fill = cnt & 0x0100_0000 != 0;
    let is_32bit = cnt & 0x0400_0000 != 0;
    if is_32bit {
        for i in 0..count {
            let src_addr = if fill { src } else { src.wrapping_add(i * 4) };
            let val = bus.read32(src_addr);
            bus.write32(dst.wrapping_add(i * 4), val);
        }
    } else {
        for i in 0..count {
            let src_addr = if fill { src } else { src.wrapping_add(i * 2) };
            let val = bus.read16(src_addr);
            bus.write16(dst.wrapping_add(i * 2), val);
        }
    }
    true
}

/// SWI 0x0C: CpuFastSet(r0 = src, r1 = dst, r2 = length/mode).
///
/// GBATEK, BIOS Memory Copy: word-only transfers in units of 32 bytes. Bits
/// 0-20 are the word count, "GBA: rounded-up to multiple of 8 words", and bit
/// 24 fixes the source address (0 = copy, 1 = fill by WORD[r0]).
fn handle_cpu_fast_set(cpu: &mut Cpu, bus: &mut MemoryBus) -> bool {
    let src = cpu.reg(0);
    let dst = cpu.reg(1);
    let cnt = cpu.reg(2);
    let count = (cnt & 0x001F_FFFF).wrapping_add(7) & !7;
    let fill = cnt & 0x0100_0000 != 0;
    for i in 0..count {
        let src_addr = if fill { src } else { src.wrapping_add(i * 4) };
        let val = bus.read32(src_addr);
        bus.write32(dst.wrapping_add(i * 4), val);
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

/// GBATEK, BIOS Decompression Functions: the header holds the reserved nibble
/// in bits 0-3, the type in bits 4-7 and the **decompressed size in bits 8-31**.
/// The size is the only thing that ends the stream - there is no terminator -
/// so an unbounded loop here runs until it has overwritten every mapped byte.
fn lz77_decompress(src: u32, dst: u32, bus: &mut MemoryBus) {
    let end = dst.wrapping_add(bus.read32(src) >> 8);
    let mut src_pos = src + 4;
    let mut dst_pos = dst;
    while dst_pos < end {
        let flags = bus.read8(src_pos);
        src_pos += 1;
        for bit in 0..8 {
            if dst_pos >= end {
                break;
            }
            if flags & (0x80 >> bit) != 0 {
                let byte1 = bus.read8(src_pos);
                let byte2 = bus.read8(src_pos + 1);
                src_pos += 2;
                let length = ((byte1 >> 4) & 0x0F) as u32 + 3;
                // Disp is the full 12 bits, and the +1 applies to the whole
                // displacement: `x | y + 1` would bind the +1 to y alone.
                let offset = ((((byte1 & 0x0F) as u32) << 8) | byte2 as u32) + 1;
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

/// GBATEK, BIOS Decompression Functions: RLUnComp is byte-oriented on both the
/// read and the write side - "Data Byte(s) - N uncompressed bytes, or 1 byte
/// repeated N times". The header carries no unit-size flag; bits 8-31 are the
/// decompressed size, which is what ends the stream.
fn rl_decompress(src: u32, dst: u32, bus: &mut MemoryBus) {
    let end = dst.wrapping_add(bus.read32(src) >> 8);
    let mut src_pos = src + 4;
    let mut dst_pos = dst;
    while dst_pos < end {
        let flag = bus.read8(src_pos);
        src_pos += 1;
        if flag & 0x80 != 0 {
            // Compressed run: one byte repeated N+3 times.
            let val = bus.read8(src_pos);
            src_pos += 1;
            for _ in 0..(flag & 0x7F) as u32 + 3 {
                if dst_pos >= end {
                    break;
                }
                bus.write8(dst_pos, val);
                dst_pos += 1;
            }
        } else {
            // Uncompressed run of N+1 bytes.
            for _ in 0..(flag & 0x7F) as u32 + 1 {
                if dst_pos >= end {
                    break;
                }
                let val = bus.read8(src_pos);
                src_pos += 1;
                bus.write8(dst_pos, val);
                dst_pos += 1;
            }
        }
    }
}

/// SWI 0x16: Diff8bitUnFilterWrite8bit ("Wram").
///
/// GBATEK, BIOS Decompression Functions: r0 = source (header word with the data
/// size in bits 0-3, the type in bits 4-7 and the decompressed byte count in
/// bits 8-31, followed by the units), r1 = destination. The first unit is the
/// original datum and every later one is the difference from its predecessor,
/// so a running sum reproduces the stream.
fn handle_diff8bit_unfilter_wram(cpu: &mut Cpu, bus: &mut MemoryBus) -> bool {
    let src = cpu.reg(0);
    let dst = cpu.reg(1);
    let size = bus.read32(src) >> 8;
    let mut sum = 0u8;
    for i in 0..size {
        sum = sum.wrapping_add(bus.read8(src + 4 + i));
        bus.write8(dst + i, sum);
    }
    true
}

/// SWI 0x17: Diff8bitUnFilterWrite16bit ("Vram").
///
/// The filter is the 8bit one; only the writes differ, since VRAM rejects byte
/// stores. Units are accumulated in pairs and written as halfwords. An odd
/// trailing byte still costs a halfword write, with the high byte zero.
fn handle_diff8bit_unfilter_vram(cpu: &mut Cpu, bus: &mut MemoryBus) -> bool {
    let src = cpu.reg(0);
    let dst = cpu.reg(1);
    let size = bus.read32(src) >> 8;
    let mut sum = 0u8;
    let mut pending = 0u16;
    for i in 0..size {
        sum = sum.wrapping_add(bus.read8(src + 4 + i));
        if i % 2 == 0 {
            pending = sum as u16;
        } else {
            bus.write16(dst + i - 1, pending | ((sum as u16) << 8));
        }
    }
    if size % 2 == 1 {
        bus.write16(dst + size - 1, pending);
    }
    true
}

/// SWI 0x18: Diff16bitUnFilter. Same filter over halfword units; the header
/// count stays in bytes, so it covers `size / 2` units.
fn handle_diff16bit_unfilter(cpu: &mut Cpu, bus: &mut MemoryBus) -> bool {
    let src = cpu.reg(0);
    let dst = cpu.reg(1);
    let size = bus.read32(src) >> 8;
    let mut sum = 0u16;
    for i in 0..(size / 2) {
        sum = sum.wrapping_add(bus.read16(src + 4 + i * 2));
        bus.write16(dst + i * 2, sum);
    }
    true
}

fn handle_sound_bias(_cpu: &mut Cpu, _bus: &mut MemoryBus) -> bool {
    true
}
