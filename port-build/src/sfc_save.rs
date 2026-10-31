use crate::Result;
use sfc_builder::battery::CopyPlan;
use sfc_builder::cartridge::{BatteryRam, CartridgeLayout};
use sfc_builder::memory_map::{MemoryMap, Space};
use w65c816::{AddressingMode as A, CpuMode, Mnemonic as M, Operand as O};
pub const SLOTS: usize = 4;
const SLOT_BYTES: usize = 1024;
const SRAM_BYTES: usize = 8192;
const MAGIC: &[u8; 4] = b"MDMC";
const VERSION: u8 = 6;
const HEADER: usize = 8;
const RANGES: [(u16, u16); 11] = [
    (0x500, 1),
    (0x50c, 1),
    (crate::sfc_b1_menu::SETTINGS, 1),
    (
        crate::sfc_battle::STATS,
        crate::sfc_battle::STATS_BYTES as u16,
    ),
    (
        crate::sfc_b1_menu::INVENTORY,
        crate::sfc_b1_menu::ITEM_KINDS as u16,
    ),
    (0x7c0, 1),
    (0x800, 192),
    (0x970, 16),
    crate::sfc_b1_map::VISITS,
    (crate::sfc_b1_map::PATH, 8),
    (crate::sfc_shop::STOCK, crate::sfc_shop::RECORDS as u16),
];
const PAYLOAD: usize = {
    let mut sum = 0;
    let mut i = 0;
    while i < RANGES.len() {
        sum += RANGES[i].1 as usize;
        i += 1;
    }
    sum
};
const _: () = assert!(HEADER + PAYLOAD <= SLOT_BYTES);
const STAGING: u32 = 0x7f6000;
pub const STAGED_SETTINGS: u32 = {
    let mut at = HEADER;
    let mut i = 0;
    while RANGES[i].0 != crate::sfc_b1_menu::SETTINGS {
        at += RANGES[i].1 as usize;
        i += 1;
    }
    STAGING + at as u32
};
const SUM: u16 = 0xa34;
pub const LOADING: u16 = 0xa36;
pub fn battery(target: crate::sfc_rom::Target) -> Result<BatteryRam> {
    target.battery(SRAM_BYTES)
}
pub fn emit(p: &mut w65c816::Assembler, layout: &CartridgeLayout) -> Result<()> {
    use crate::sfc::{emit, implied};
    let abs = |p: &mut w65c816::Assembler, m, at| emit(p, m, A::Absolute, O::Word(at));
    let long_x = |p: &mut w65c816::Assembler, m, at: u32| emit(p, m, A::AbsoluteLongX, O::Long(at));
    let branch = |p: &mut w65c816::Assembler, m, label: &str| {
        p.emit_label_ref(m, A::Relative8, label);
    };
    let m16 = |p: &mut w65c816::Assembler| emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    let m8 = |p: &mut w65c816::Assembler| emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    let ram = layout.battery_ram()?;
    let mut memory = MemoryMap::new_for_mapping(layout.mapping(), layout.size(), SRAM_BYTES)?;
    memory.reserve(Space::Sram, "save_slots", 0, SLOTS * SLOT_BYTES)?;
    memory.reserve(
        Space::Wram,
        "save_staging",
        (STAGING - 0x7e0000) as usize,
        SLOT_BYTES,
    )?;
    p.label("save_checksum").assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    abs(p, M::Stz, SUM);
    emit(p, M::Ldx, A::Immediate, O::Word(HEADER as u16));
    p.label("save_checksum_byte");
    long_x(p, M::Lda, STAGING);
    emit(p, M::And, A::Immediate, O::Word(0xff));
    implied(p, M::Clc);
    abs(p, M::Adc, SUM);
    abs(p, M::Sta, SUM);
    implied(p, M::Inx);
    emit(p, M::Cpx, A::Immediate, O::Word((HEADER + PAYLOAD) as u16));
    branch(p, M::Bne, "save_checksum_byte");
    m8(p);
    implied(p, M::Rts);
    p.label("save_write").assume_mode(CpuMode::NATIVE_M8_X16);
    implied(p, M::Pha);
    emit(p, M::Ldx, A::Immediate, O::Word(HEADER as u16));
    for (i, &(at, bytes)) in RANGES.iter().enumerate() {
        emit(p, M::Ldy, A::Immediate, O::Word(0));
        p.label(format!("save_gather_{i}"));
        emit(p, M::Lda, A::AbsoluteY, O::Word(at));
        long_x(p, M::Sta, STAGING);
        implied(p, M::Inx);
        implied(p, M::Iny);
        emit(p, M::Cpy, A::Immediate, O::Word(bytes));
        branch(p, M::Bne, &format!("save_gather_{i}"));
    }
    for (i, b) in MAGIC.iter().chain([VERSION].iter()).enumerate() {
        emit(p, M::Lda, A::Immediate, O::Byte(*b));
        emit(p, M::Sta, A::AbsoluteLong, O::Long(STAGING + i as u32));
    }
    abs(p, M::Lda, 0x50c);
    emit(p, M::Sta, A::AbsoluteLong, O::Long(STAGING + 5));
    p.emit_label_ref(M::Jsr, A::Absolute, "save_checksum");
    m16(p);
    abs(p, M::Lda, SUM);
    emit(p, M::Sta, A::AbsoluteLong, O::Long(STAGING + 6));
    m8(p);
    implied(p, M::Pla);
    for slot in 0..SLOTS {
        let plan = CopyPlan::new(
            layout,
            &memory,
            slot * SLOT_BYTES,
            (STAGING - 0x7e0000) as usize,
            HEADER + PAYLOAD,
        )?;
        emit(p, M::Cmp, A::Immediate, O::Byte(slot as u8));
        branch(p, M::Bne, &format!("save_write_not_{slot}"));
        plan.emit_save(p, &format!("save_write_copy_{slot}"))?;
        implied(p, M::Rts);
        p.label(format!("save_write_not_{slot}"));
    }
    implied(p, M::Rts);
    p.label("save_read").assume_mode(CpuMode::NATIVE_M8_X16);
    for slot in 0..SLOTS {
        let plan = CopyPlan::new(
            layout,
            &memory,
            slot * SLOT_BYTES,
            (STAGING - 0x7e0000) as usize,
            HEADER + PAYLOAD,
        )?;
        emit(p, M::Cmp, A::Immediate, O::Byte(slot as u8));
        branch(p, M::Bne, &format!("save_read_not_{slot}"));
        plan.emit_load(p, &format!("save_read_copy_{slot}"))?;
        p.emit_label_ref(M::Bra, A::Relative8, "save_read_check");
        p.label(format!("save_read_not_{slot}"));
    }
    implied(p, M::Sec);
    implied(p, M::Rts);
    p.label("save_read_check");
    for (i, b) in MAGIC.iter().chain([VERSION].iter()).enumerate() {
        emit(p, M::Lda, A::AbsoluteLong, O::Long(STAGING + i as u32));
        emit(p, M::Cmp, A::Immediate, O::Byte(*b));
        branch(p, M::Bne, "save_read_bad");
    }
    p.emit_label_ref(M::Jsr, A::Absolute, "save_checksum");
    m16(p);
    abs(p, M::Lda, SUM);
    emit(p, M::Cmp, A::AbsoluteLong, O::Long(STAGING + 6));
    m8(p);
    branch(p, M::Bne, "save_read_bad");
    implied(p, M::Clc);
    implied(p, M::Rts);
    p.label("save_read_bad");
    implied(p, M::Sec);
    implied(p, M::Rts);
    p.label("save_scatter").assume_mode(CpuMode::NATIVE_M8_X16);
    emit(p, M::Ldx, A::Immediate, O::Word(HEADER as u16));
    for (i, &(at, bytes)) in RANGES.iter().enumerate() {
        emit(p, M::Ldy, A::Immediate, O::Word(0));
        p.label(format!("save_scatter_{i}"));
        long_x(p, M::Lda, STAGING);
        emit(p, M::Sta, A::AbsoluteY, O::Word(at));
        implied(p, M::Inx);
        implied(p, M::Iny);
        emit(p, M::Cpy, A::Immediate, O::Word(bytes));
        branch(p, M::Bne, &format!("save_scatter_{i}"));
    }
    implied(p, M::Rts);
    p.label("save_slot_map").assume_mode(CpuMode::NATIVE_M8_X16);
    for slot in 0..SLOTS {
        let base = ram.cpu_address(slot * SLOT_BYTES, HEADER)?;
        emit(p, M::Cmp, A::Immediate, O::Byte(slot as u8));
        branch(p, M::Bne, &format!("save_map_not_{slot}"));
        for (i, b) in MAGIC.iter().chain([VERSION].iter()).enumerate() {
            emit(p, M::Lda, A::AbsoluteLong, O::Long(base + i as u32));
            emit(p, M::Cmp, A::Immediate, O::Byte(*b));
            branch(p, M::Bne, &format!("save_map_none_{slot}"));
        }
        emit(p, M::Lda, A::AbsoluteLong, O::Long(base + 5));
        implied(p, M::Rts);
        p.label(format!("save_map_none_{slot}"));
        emit(p, M::Lda, A::Immediate, O::Byte(0));
        implied(p, M::Rts);
        p.label(format!("save_map_not_{slot}"));
    }
    emit(p, M::Lda, A::Immediate, O::Byte(0));
    implied(p, M::Rts);
    Ok(())
}
pub fn report() -> serde_json::Value {
    serde_json::json!(
        { "slots" : SLOTS, "slot_bytes" : SLOT_BYTES, "sram_bytes" : SRAM_BYTES, "magic"
        : String::from_utf8_lossy(MAGIC), "version" : VERSION, "header" : HEADER,
        "payload" : PAYLOAD, "ranges" : RANGES.iter().map(|& (a, b) |
        serde_json::json!([a, b])).collect::< Vec < _ >> (), "staging" : STAGING }
    )
}
