use crate::Result;
use sfc_builder::cartridge::{CartridgeLayout, Section};
use w65c816::{AddressingMode as A, CpuMode, Mnemonic as M, Operand as O};
pub const SEED: u16 = 0xa20;
pub const REQUEST: u16 = 0xa24;
pub const ENEMY: u16 = 0xa25;
pub const HITS: u16 = 0xa26;
const PRODUCT: u16 = 0xa2c;
const MULTIPLIER: u32 = 0x41c64e6d;
const INCREMENT: u16 = 0x3039;
pub const SKIP_MOST: usize = 40;
const ODDS: u8 = 20;
const ENABLED: u8 = 1;
const CANDIDATE_BYTES: usize = 8;
pub struct Floors {
    masks: Vec<u8>,
    candidates: Vec<u8>,
}
pub fn load(world: &serde_json::Value, count: u8) -> Result<Floors> {
    let (mut masks, mut candidates) = (Vec::new(), Vec::new());
    for id in 1..=count {
        let map = world["maps"]
            .as_array()
            .and_then(|m| m.iter().find(|m| m["id"] == id))
            .ok_or("Missing encounter map")?;
        let floor: Vec<u8> = serde_json::from_value(map["encounter_masks"].clone())?;
        let ids: Vec<u8> = serde_json::from_value(map["encounter_candidates"].clone())?;
        if floor.len() != 64 || ids.is_empty() || ids.len() >= CANDIDATE_BYTES {
            return Err(format!("Floor {id} encounter data out of range").into());
        }
        masks.extend(floor);
        let mut row = vec![ids.len() as u8];
        row.extend(&ids);
        row.resize(CANDIDATE_BYTES, 0);
        candidates.extend(row);
    }
    Ok(Floors { masks, candidates })
}
pub fn sections(
    sections: &mut Vec<Section>,
    space: &mut crate::sfc_rom::Space,
    floors: &Floors,
) -> Result<()> {
    crate::sfc_b1::add(
        sections,
        space,
        "encounter_masks".into(),
        floors.masks.clone(),
    )?;
    crate::sfc_b1::add(
        sections,
        space,
        "encounter_candidates".into(),
        floors.candidates.clone(),
    )?;
    Ok(())
}
pub fn emit(p: &mut w65c816::Assembler, layout: &CartridgeLayout) -> Result<()> {
    use crate::sfc::{emit, implied, write};
    let abs = |p: &mut w65c816::Assembler, m, at| emit(p, m, A::Absolute, O::Word(at));
    let branch = |p: &mut w65c816::Assembler, m, label: &str| {
        p.emit_label_ref(m, A::Relative8, label);
    };
    let m16 = |p: &mut w65c816::Assembler| emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    let m8 = |p: &mut w65c816::Assembler| emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    let section = |name: &str| -> Result<u32> { layout.cpu_address(layout.section(name)?.offset) };
    let masks = section("encounter_masks")?;
    let candidates = section("encounter_candidates")?;
    p.label("encounter_reset")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    abs(p, M::Stz, HITS);
    m8(p);
    abs(p, M::Stz, REQUEST);
    abs(p, M::Stz, ENEMY);
    implied(p, M::Rts);
    p.label("encounter_select")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, 0x50c);
    emit(
        p,
        M::Cmp,
        A::Immediate,
        O::Byte(crate::sfc_b1::BOUNDARY_FLOOR),
    );
    branch(p, M::Bcs, "encounter_none");
    abs(p, M::Lda, REQUEST);
    branch(p, M::Beq, "encounter_pick");
    abs(p, M::Stz, REQUEST);
    p.emit_label_ref(M::Jsr, A::Absolute, "battle_enter");
    m16(p);
    abs(p, M::Inc, HITS);
    m8(p);
    implied(p, M::Rts);
    p.label("encounter_pick");
    m16(p);
    abs(p, M::Lda, 0x50c);
    emit(p, M::And, A::Immediate, O::Word(0xff));
    emit(p, M::Dec, A::Accumulator, O::None);
    for _ in 0..CANDIDATE_BYTES.trailing_zeros() {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    implied(p, M::Tax);
    m8(p);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(candidates));
    implied(p, M::Phx);
    p.emit_label_ref(M::Jsr, A::Absolute, "encounter_rng");
    implied(p, M::Plx);
    m16(p);
    emit(p, M::And, A::Immediate, O::Word(0xff));
    emit(p, M::Inc, A::Accumulator, O::None);
    abs(p, M::Sta, 0xe0);
    implied(p, M::Txa);
    implied(p, M::Clc);
    abs(p, M::Adc, 0xe0);
    implied(p, M::Tax);
    m8(p);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(candidates));
    abs(p, M::Sta, ENEMY);
    p.label("encounter_none");
    implied(p, M::Rts);
    p.label("encounter_roll")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, 0x50c);
    emit(
        p,
        M::Cmp,
        A::Immediate,
        O::Byte(crate::sfc_b1::BOUNDARY_FLOOR),
    );
    branch(p, M::Bcs, "encounter_none");
    m16(p);
    abs(p, M::Lda, 0x50c);
    emit(p, M::And, A::Immediate, O::Word(0xff));
    emit(p, M::Dec, A::Accumulator, O::None);
    for _ in 0..6 {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    abs(p, M::Sta, 0xe0);
    abs(p, M::Lda, 0x500);
    emit(p, M::And, A::Immediate, O::Word(0xff));
    emit(p, M::Lsr, A::Accumulator, O::None);
    emit(p, M::Lsr, A::Accumulator, O::None);
    implied(p, M::Clc);
    abs(p, M::Adc, 0xe0);
    implied(p, M::Tax);
    m8(p);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(masks));
    abs(p, M::Sta, 0xe0);
    abs(p, M::Lda, 0x500);
    emit(p, M::And, A::Immediate, O::Byte(3));
    implied(p, M::Tax);
    emit(p, M::Lda, A::Immediate, O::Byte(1));
    p.label("encounter_bit");
    implied(p, M::Dex);
    branch(p, M::Bmi, "encounter_bit_done");
    emit(p, M::Asl, A::Accumulator, O::None);
    branch(p, M::Bra, "encounter_bit");
    p.label("encounter_bit_done");
    abs(p, M::And, 0xe0);
    branch(p, M::Bne, "encounter_none");
    emit(p, M::Lda, A::Immediate, O::Byte(ODDS));
    p.emit_label_ref(M::Jsr, A::Absolute, "encounter_rng");
    branch(p, M::Bne, "encounter_miss");
    write(p, REQUEST, ENABLED);
    implied(p, M::Rts);
    p.label("encounter_miss");
    abs(p, M::Stz, REQUEST);
    implied(p, M::Rts);
    Ok(())
}
pub fn emit_rng(p: &mut w65c816::Assembler) {
    use crate::sfc::{emit, implied};
    let abs = |p: &mut w65c816::Assembler, m, at| emit(p, m, A::Absolute, O::Word(at));
    let branch = |p: &mut w65c816::Assembler, m, label: &str| {
        p.emit_label_ref(m, A::Relative8, label);
    };
    let m16 = |p: &mut w65c816::Assembler| emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    let m8 = |p: &mut w65c816::Assembler| emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    let index = |p: &mut w65c816::Assembler, byte: u16| {
        m16(p);
        abs(p, M::Lda, SEED + byte);
        emit(p, M::And, A::Immediate, O::Word(0xff));
        emit(p, M::Asl, A::Accumulator, O::None);
        implied(p, M::Tax);
    };
    let low = |p: &mut w65c816::Assembler| {
        p.emit_label_ref(M::Lda, A::AbsoluteLongX, "encounter_rng_product_low");
    };
    let high = |p: &mut w65c816::Assembler| {
        p.emit_label_ref(M::Lda, A::AbsoluteLongX, "encounter_rng_product_high");
    };
    p.label("encounter_rng").assume_mode(CpuMode::NATIVE_M8_X16);
    implied(p, M::Pha);
    implied(p, M::Phx);
    index(p, 0);
    low(p);
    abs(p, M::Sta, PRODUCT);
    high(p);
    abs(p, M::Sta, PRODUCT + 2);
    index(p, 1);
    low(p);
    implied(p, M::Clc);
    abs(p, M::Adc, PRODUCT + 1);
    abs(p, M::Sta, PRODUCT + 1);
    m8(p);
    high(p);
    abs(p, M::Adc, PRODUCT + 3);
    abs(p, M::Sta, PRODUCT + 3);
    index(p, 2);
    low(p);
    implied(p, M::Clc);
    abs(p, M::Adc, PRODUCT + 2);
    abs(p, M::Sta, PRODUCT + 2);
    index(p, 3);
    m8(p);
    low(p);
    implied(p, M::Clc);
    abs(p, M::Adc, PRODUCT + 3);
    abs(p, M::Sta, PRODUCT + 3);
    implied(p, M::Plx);
    m16(p);
    abs(p, M::Lda, PRODUCT);
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Word(INCREMENT));
    abs(p, M::Sta, SEED);
    abs(p, M::Lda, PRODUCT + 2);
    emit(p, M::Adc, A::Immediate, O::Word(0));
    abs(p, M::Sta, SEED + 2);
    abs(p, M::Sta, 0x4204);
    m8(p);
    implied(p, M::Pla);
    branch(p, M::Beq, "encounter_rng_zero");
    abs(p, M::Sta, 0x4206);
    for _ in 0..8 {
        implied(p, M::Nop);
    }
    abs(p, M::Lda, 0x4216);
    implied(p, M::Rts);
    p.label("encounter_rng_zero");
    implied(p, M::Rts);
    for (name, shift) in [("low", 0), ("high", 16)] {
        p.label(format!("encounter_rng_product_{name}"));
        let plane: Vec<u8> = (0..256u32)
            .flat_map(|byte| ((byte.wrapping_mul(MULTIPLIER) >> shift) as u16).to_le_bytes())
            .collect();
        p.data(plane);
    }
    p.label("encounter_rng_skip")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    implied(p, M::Phx);
    m16(p);
    emit(p, M::And, A::Immediate, O::Word(0xff));
    implied(p, M::Tax);
    abs(p, M::Stz, PRODUCT);
    abs(p, M::Stz, PRODUCT + 2);
    m8(p);
    for i in 0..4u16 {
        abs(p, M::Lda, SEED + i);
        abs(p, M::Sta, 0x4202);
        for j in 0..4 - i {
            let at = PRODUCT + i + j;
            p.emit_label_ref(M::Lda, A::AbsoluteLongX, format!("encounter_rng_skip_a{j}"));
            abs(p, M::Sta, 0x4203);
            for _ in 0..4 {
                implied(p, M::Nop);
            }
            if i + j == 3 {
                abs(p, M::Lda, at);
                implied(p, M::Clc);
                abs(p, M::Adc, 0x4216);
                abs(p, M::Sta, at);
            } else {
                m16(p);
                abs(p, M::Lda, at);
                implied(p, M::Clc);
                abs(p, M::Adc, 0x4216);
                abs(p, M::Sta, at);
                m8(p);
                if i + j == 1 {
                    let no = format!("encounter_rng_skip_carry_{i}{j}");
                    branch(p, M::Bcc, &no);
                    abs(p, M::Inc, PRODUCT + 3);
                    p.label(&no);
                }
            }
        }
    }
    implied(p, M::Clc);
    for i in 0..4u16 {
        abs(p, M::Lda, PRODUCT + i);
        p.emit_label_ref(M::Adc, A::AbsoluteLongX, format!("encounter_rng_skip_c{i}"));
        abs(p, M::Sta, SEED + i);
    }
    implied(p, M::Plx);
    implied(p, M::Rts);
    let (mut a, mut c) = (vec![1u32], vec![0u32]);
    for k in 1..=SKIP_MOST {
        a.push(a[k - 1].wrapping_mul(MULTIPLIER));
        c.push(
            c[k - 1]
                .wrapping_mul(MULTIPLIER)
                .wrapping_add(u32::from(INCREMENT)),
        );
    }
    for (name, values) in [("a", &a), ("c", &c)] {
        for byte in 0..4 {
            p.label(format!("encounter_rng_skip_{name}{byte}"));
            let plane: Vec<u8> = values.iter().map(|v| v.to_le_bytes()[byte]).collect();
            p.data(plane);
        }
    }
}
