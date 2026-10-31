use crate::Result;
use sfc_builder::cartridge::CartridgeLayout;
use std::collections::BTreeMap;
use w65c816::{AddressingMode as A, CpuMode, Mnemonic as M, Operand as O};
pub const CELLS: u32 = 0x7f2200;
const SLOTS: u16 = 0x9e0;
const KEY: u16 = 0x9f4;
pub(crate) const CX: u16 = 0x9fe;
pub(crate) const CY: u16 = 0x9ff;
pub(crate) const CD: u16 = 0xa00;
const COUNT: u16 = 0xa01;
const CELL: u16 = 0xa02;
const HASH: u16 = 0xa04;
const NIBBLE: u16 = 0xa06;
pub const BASE: u16 = 0xa07;
const TABLE: u8 = 0xec;
pub type Rays = Vec<(Vec<u8>, Vec<usize>)>;
pub const KEY_BYTES: usize = 10;
const ENTRY: usize = KEY_BYTES + 2;
pub fn key(slots: &[u8; 20], parity: u8) -> [u8; KEY_BYTES] {
    let mut key = [0; KEY_BYTES];
    for (i, byte) in key.iter_mut().enumerate().take(9) {
        *byte = slots[1 + 2 * i] | slots[2 + 2 * i] << 4;
    }
    key[9] = slots[19] | parity << 4;
    key
}
fn hash(key: &[u8; KEY_BYTES]) -> u16 {
    let mut h = 0u16;
    for pair in key.chunks(2) {
        h = h
            .rotate_left(7)
            .wrapping_add(u16::from_le_bytes([pair[0], pair[1]]));
    }
    h ^ (h >> 8)
}
fn first_entry(key: &[u8; KEY_BYTES], size: usize, mask: u16) -> usize {
    let at = usize::from(hash(key) & mask);
    if at >= size { at - size } else { at }
}
pub fn table(records: &BTreeMap<[u8; KEY_BYTES], u16>) -> Result<(Vec<u8>, u16)> {
    let mut size = (records.len() * 9).div_ceil(8) + 1;
    loop {
        let mask = u16::try_from(size.next_power_of_two() - 1)?;
        let mut entries = vec![[0xffu8; ENTRY]; size];
        for (key, &record) in records {
            let mut at = first_entry(key, size, mask);
            while entries[at][KEY_BYTES..] != [0xff, 0xff] {
                at = (at + 1) % size;
            }
            entries[at][..KEY_BYTES].copy_from_slice(key);
            entries[at][KEY_BYTES..].copy_from_slice(&record.to_le_bytes());
        }
        let filled = |i: usize| entries[i % size][KEY_BYTES..] != [0xff, 0xff];
        let start = (0..size)
            .find(|&i| !filled(i))
            .ok_or("View table is full")?;
        let (mut run, mut longest) = (0, 0);
        for i in start..start + size {
            run = if filled(i) { run + 1 } else { 0 };
            longest = longest.max(run);
        }
        if longest < MIN_REPEAT_RUN {
            return Ok((entries.concat(), mask));
        }
        size += 1;
    }
}
const MIN_REPEAT_RUN: usize = 64;
pub fn rays(rays: &[(Vec<u8>, Vec<usize>)]) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    for (turns, slots) in rays {
        bytes.push(u8::try_from(turns.len())?);
        let mut facing = 0u8;
        for (&t, &s) in turns.iter().zip(slots) {
            facing = (facing + t) & 3;
            bytes.extend([facing, u8::try_from(s)?]);
        }
    }
    bytes.push(0);
    Ok(bytes)
}
pub fn sections(
    sections: &mut Vec<sfc_builder::cartridge::Section>,
    space: &mut crate::sfc_rom::Space,
    cells: &[Vec<u16>],
    ray_bytes: Vec<u8>,
    tables: &[(Vec<u8>, u16)],
    slot_zero: u8,
) -> Result<()> {
    use crate::sfc_b1::add;
    let cell_bytes = cells
        .iter()
        .flatten()
        .flat_map(|c| c.to_le_bytes())
        .collect();
    add(sections, space, "field_cells".into(), cell_bytes)?;
    add(sections, space, "view_rays".into(), ray_bytes)?;
    add(sections, space, "view_slot_zero".into(), vec![slot_zero])?;
    add(
        sections,
        space,
        "view_deltas".into(),
        vec![0, 1, 0, 0xff, 0xff, 0, 1, 0],
    )?;
    let mut info = Vec::new();
    for (i, (bytes, mask)) in tables.iter().enumerate() {
        if bytes.len() > 0x8000 {
            return Err("View table exceeds a bank".into());
        }
        let at = add(sections, space, format!("view_table_{i}"), bytes.clone())?;
        info.extend(&at.to_le_bytes()[..3]);
        info.extend(mask.to_le_bytes());
        info.extend(u16::try_from(bytes.len() / ENTRY)?.to_le_bytes());
    }
    add(sections, space, "view_floors".into(), info)?;
    Ok(())
}
pub const VIEW_RECORD: u16 = 4;
pub fn emit(p: &mut w65c816::Assembler, layout: &CartridgeLayout) -> Result<()> {
    use crate::sfc::{emit, implied};
    let abs = |p: &mut w65c816::Assembler, m, at: u16| emit(p, m, A::Absolute, O::Word(at));
    let long_x = |p: &mut w65c816::Assembler, m, at: u32| emit(p, m, A::AbsoluteLongX, O::Long(at));
    let branch = |p: &mut w65c816::Assembler, m, label: &str| {
        p.emit_label_ref(m, A::Relative8, label);
    };
    let call = |p: &mut w65c816::Assembler, label: &str| {
        p.emit_label_ref(M::Jsr, A::Absolute, label);
    };
    let m16 = |p: &mut w65c816::Assembler| emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    let m8 = |p: &mut w65c816::Assembler| emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    let section = |name: &str| -> Result<u32> { layout.cpu_address(layout.section(name)?.offset) };
    let cells_len = u16::try_from(layout.section("field_cells")?.bytes.len())?;
    let slot_zero = layout.section("view_slot_zero")?.bytes[0];
    let (cells, rays, deltas, floors) = (
        section("field_cells")?,
        section("view_rays")?,
        section("view_deltas")?,
        section("view_floors")?,
    );
    p.label("floor_cells_reset")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    emit(p, M::Ldx, A::Immediate, O::Word(0));
    p.label("floor_cells_copy");
    long_x(p, M::Lda, cells);
    long_x(p, M::Sta, CELLS);
    implied(p, M::Inx);
    implied(p, M::Inx);
    emit(p, M::Cpx, A::Immediate, O::Word(cells_len));
    branch(p, M::Bne, "floor_cells_copy");
    m8(p);
    implied(p, M::Rts);
    p.label("floor_from_state")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, 0x500);
    emit(p, M::And, A::Immediate, O::Byte(3));
    abs(p, M::Sta, CD);
    abs(p, M::Lda, 0x500);
    emit(p, M::Lsr, A::Accumulator, O::None);
    emit(p, M::Lsr, A::Accumulator, O::None);
    emit(p, M::And, A::Immediate, O::Byte(7));
    abs(p, M::Sta, CX);
    abs(p, M::Lda, 0x500);
    for _ in 0..5 {
        emit(p, M::Lsr, A::Accumulator, O::None);
    }
    abs(p, M::Sta, CY);
    implied(p, M::Rts);
    p.label("floor_nibble").assume_mode(CpuMode::NATIVE_M8_X16);
    implied(p, M::Phx);
    m16(p);
    abs(p, M::Lda, 0x50c);
    emit(p, M::And, A::Immediate, O::Word(255));
    emit(p, M::Dec, A::Accumulator, O::None);
    for _ in 0..7 {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    abs(p, M::Sta, CELL);
    abs(p, M::Lda, CY);
    emit(p, M::And, A::Immediate, O::Word(7));
    for _ in 0..3 {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    abs(p, M::Ora, CX);
    emit(p, M::And, A::Immediate, O::Word(63));
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Clc);
    abs(p, M::Adc, CELL);
    implied(p, M::Tax);
    long_x(p, M::Lda, CELLS);
    abs(p, M::Sta, CELL);
    abs(p, M::Lda, CD);
    emit(p, M::And, A::Immediate, O::Word(3));
    implied(p, M::Tax);
    abs(p, M::Lda, CELL);
    p.label("floor_nibble_shift");
    emit(p, M::Cpx, A::Immediate, O::Word(0));
    branch(p, M::Beq, "floor_nibble_done");
    for _ in 0..4 {
        emit(p, M::Lsr, A::Accumulator, O::None);
    }
    implied(p, M::Dex);
    branch(p, M::Bra, "floor_nibble_shift");
    p.label("floor_nibble_done");
    m8(p);
    emit(p, M::And, A::Immediate, O::Byte(15));
    abs(p, M::Sta, NIBBLE);
    implied(p, M::Plx);
    implied(p, M::Rts);
    p.label("floor_edge").assume_mode(CpuMode::NATIVE_M8_X16);
    call(p, "floor_from_state");
    call(p, "floor_nibble");
    abs(p, M::Lda, NIBBLE);
    implied(p, M::Rts);
    p.label("floor_view_record")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    for i in 1..20 {
        abs(p, M::Stz, SLOTS + i);
    }
    emit(p, M::Lda, A::Immediate, O::Byte(slot_zero));
    abs(p, M::Sta, SLOTS);
    emit(p, M::Ldx, A::Immediate, O::Word(0));
    p.label("floor_ray");
    long_x(p, M::Lda, rays);
    branch(p, M::Beq, "floor_rays_done");
    abs(p, M::Sta, COUNT);
    implied(p, M::Inx);
    call(p, "floor_from_state");
    abs(p, M::Lda, CD);
    abs(p, M::Sta, HASH);
    p.label("floor_ray_step");
    long_x(p, M::Lda, rays);
    implied(p, M::Clc);
    abs(p, M::Adc, HASH);
    emit(p, M::And, A::Immediate, O::Byte(3));
    abs(p, M::Sta, CD);
    call(p, "floor_nibble");
    abs(p, M::Lda, NIBBLE);
    branch(p, M::Beq, "floor_ray_open");
    implied(p, M::Phx);
    long_x(p, M::Lda, rays + 1);
    m16(p);
    emit(p, M::And, A::Immediate, O::Word(31));
    implied(p, M::Tax);
    m8(p);
    abs(p, M::Lda, NIBBLE);
    emit(p, M::Sta, A::AbsoluteX, O::Word(SLOTS));
    implied(p, M::Plx);
    p.label("floor_ray_skip");
    implied(p, M::Inx);
    implied(p, M::Inx);
    abs(p, M::Dec, COUNT);
    branch(p, M::Bne, "floor_ray_skip");
    branch(p, M::Bra, "floor_ray");
    p.label("floor_ray_open");
    implied(p, M::Phx);
    m16(p);
    abs(p, M::Lda, CD);
    emit(p, M::And, A::Immediate, O::Word(3));
    implied(p, M::Tax);
    m8(p);
    long_x(p, M::Lda, deltas);
    implied(p, M::Clc);
    abs(p, M::Adc, CX);
    abs(p, M::Sta, CX);
    long_x(p, M::Lda, deltas + 4);
    implied(p, M::Clc);
    abs(p, M::Adc, CY);
    abs(p, M::Sta, CY);
    implied(p, M::Plx);
    implied(p, M::Inx);
    implied(p, M::Inx);
    abs(p, M::Dec, COUNT);
    branch(p, M::Bne, "floor_ray_step");
    branch(p, M::Bra, "floor_ray");
    p.label("floor_rays_done");
    for i in 0..9u16 {
        abs(p, M::Lda, SLOTS + 2 + 2 * i);
        for _ in 0..4 {
            emit(p, M::Asl, A::Accumulator, O::None);
        }
        abs(p, M::Ora, SLOTS + 1 + 2 * i);
        abs(p, M::Sta, KEY + i);
    }
    abs(p, M::Lda, 0x500);
    abs(p, M::Sta, HASH);
    emit(p, M::Lsr, A::Accumulator, O::None);
    emit(p, M::Lsr, A::Accumulator, O::None);
    abs(p, M::Eor, HASH);
    abs(p, M::Sta, HASH + 1);
    abs(p, M::Lda, HASH);
    for _ in 0..5 {
        emit(p, M::Lsr, A::Accumulator, O::None);
    }
    abs(p, M::Eor, HASH + 1);
    emit(p, M::And, A::Immediate, O::Byte(1));
    for _ in 0..4 {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    abs(p, M::Ora, SLOTS + 19);
    abs(p, M::Sta, KEY + 9);
    m16(p);
    abs(p, M::Stz, HASH);
    for i in 0..5u16 {
        abs(p, M::Lda, HASH);
        implied(p, M::Xba);
        emit(p, M::Lsr, A::Accumulator, O::None);
        branch(p, M::Bcc, &format!("floor_hash_{i}"));
        emit(p, M::Ora, A::Immediate, O::Word(0x8000));
        p.label(format!("floor_hash_{i}"));
        implied(p, M::Clc);
        abs(p, M::Adc, KEY + 2 * i);
        abs(p, M::Sta, HASH);
    }
    implied(p, M::Xba);
    emit(p, M::And, A::Immediate, O::Word(0xff));
    abs(p, M::Eor, HASH);
    abs(p, M::Sta, HASH);
    floor_info_x(p);
    long_x(p, M::Lda, floors);
    emit(p, M::Sta, A::DirectPage, O::Byte(TABLE));
    long_x(p, M::Lda, floors + 2);
    emit(p, M::Sta, A::DirectPage, O::Byte(TABLE + 2));
    long_x(p, M::Lda, floors + 3);
    abs(p, M::And, HASH);
    long_x(p, M::Cmp, floors + 5);
    branch(p, M::Bcc, "floor_first_entry");
    long_x(p, M::Sbc, floors + 5);
    p.label("floor_first_entry");
    abs(p, M::Sta, HASH);
    p.label("floor_match");
    abs(p, M::Lda, HASH);
    emit(p, M::Asl, A::Accumulator, O::None);
    emit(p, M::Asl, A::Accumulator, O::None);
    abs(p, M::Sta, CELL);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Clc);
    abs(p, M::Adc, CELL);
    implied(p, M::Tay);
    emit(p, M::Ldx, A::Immediate, O::Word(0));
    p.label("floor_match_word");
    emit(p, M::Lda, A::DirectPageIndirectLongIndexedY, O::Byte(TABLE));
    emit(p, M::Cmp, A::AbsoluteX, O::Word(KEY));
    branch(p, M::Bne, "floor_match_miss");
    implied(p, M::Iny);
    implied(p, M::Iny);
    implied(p, M::Inx);
    implied(p, M::Inx);
    emit(p, M::Cpx, A::Immediate, O::Word(KEY_BYTES as u16));
    branch(p, M::Bne, "floor_match_word");
    emit(p, M::Lda, A::DirectPageIndirectLongIndexedY, O::Byte(TABLE));
    branch(p, M::Bra, "floor_match_found");
    p.label("floor_match_miss");
    abs(p, M::Lda, HASH);
    emit(p, M::Asl, A::Accumulator, O::None);
    emit(p, M::Asl, A::Accumulator, O::None);
    abs(p, M::Sta, CELL);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Clc);
    abs(p, M::Adc, CELL);
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Word(KEY_BYTES as u16));
    implied(p, M::Tay);
    emit(p, M::Lda, A::DirectPageIndirectLongIndexedY, O::Byte(TABLE));
    emit(p, M::Cmp, A::Immediate, O::Word(0xffff));
    branch(p, M::Beq, "floor_match_found");
    abs(p, M::Lda, HASH);
    emit(p, M::Inc, A::Accumulator, O::None);
    implied(p, M::Pha);
    floor_info_x(p);
    implied(p, M::Pla);
    long_x(p, M::Cmp, floors + 5);
    branch(p, M::Bcc, "floor_next_entry");
    emit(p, M::Lda, A::Immediate, O::Word(0));
    p.label("floor_next_entry");
    abs(p, M::Sta, HASH);
    branch(p, M::Bra, "floor_match");
    p.label("floor_match_found")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    abs(p, M::Sta, BASE);
    for _ in 0..VIEW_RECORD.trailing_zeros() {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    abs(p, M::Sta, 0x51a);
    m8(p);
    implied(p, M::Rts);
    Ok(())
}
const FLOOR_INFO: u16 = 7;
fn floor_info_x(p: &mut w65c816::Assembler) {
    use crate::sfc::{emit, implied};
    emit(p, M::Lda, A::Absolute, O::Word(0x50c));
    emit(p, M::And, A::Immediate, O::Word(255));
    emit(p, M::Dec, A::Accumulator, O::None);
    emit(p, M::Sta, A::Absolute, O::Word(CELL));
    for _ in 0..3 {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    implied(p, M::Sec);
    emit(p, M::Sbc, A::Absolute, O::Word(CELL));
    implied(p, M::Tax);
    const _: () = assert!(FLOOR_INFO == 8 - 1);
}
pub struct Walls<'a> {
    pub rays: &'a [(Vec<u8>, Vec<usize>)],
    pub masks: [u16; 4],
    pub slot_zero: u8,
}
impl Walls<'_> {
    pub fn slots(&self, cells: &[u16], x: u8, y: u8, d: u8) -> Result<[u8; 20]> {
        const DELTAS: [(i16, i16); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];
        let mut slots = [0; 20];
        slots[0] = self.slot_zero;
        for (turns, ray) in self.rays {
            let (mut cx, mut cy, mut cd) = (i16::from(x), i16::from(y), d);
            for (&t, &slot) in turns.iter().zip(ray) {
                cd = (cd + t) & 3;
                if !(0..8).contains(&cx) || !(0..8).contains(&cy) {
                    return Err("View ray leaves the floor".into());
                }
                let cell = cells[(cy * 8 + cx) as usize];
                if cell & self.masks[usize::from(cd)] != 0 {
                    slots[slot] = ((cell >> (cd * 4)) & 15) as u8;
                    break;
                }
                cx += DELTAS[usize::from(cd)].0;
                cy += DELTAS[usize::from(cd)].1;
            }
        }
        Ok(slots)
    }
}
