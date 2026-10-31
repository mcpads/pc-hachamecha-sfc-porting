use crate::Result;
use crate::sfc_b1_text::{
    BUFFER, COLUMNS, GLYPH, GLYPH_BYTES, INK_COLOR, LAST_LINE_Y, LINE_Y, PEN_X, ROWS, TEXT_DP,
    ink_index,
};
use sfc_builder::cartridge::{CartridgeLayout, Section};
use w65c816::{AddressingMode as A, CpuMode, Mnemonic as M, Operand as O};
const CODE_SECTION: &str = "b1_text_draw";
const ENTRY: &str = "text_glyph";
const ENDING_ENTRY: &str = "text_ending_glyph";
pub(crate) const FIRST_AT: usize = 30;
const _: () = assert!(FIRST_AT + 2 == GLYPH_BYTES);
const COLORS: u8 = 6;
fn table_labels() -> Vec<(&'static str, Vec<String>)> {
    let routines = |suffix: &str| -> Vec<String> {
        ["", "n"]
            .iter()
            .flat_map(|w| (0..8).map(move |s| format!("text_shift_{s}{w}{suffix}")))
            .collect()
    };
    let writers = |slot: &str| -> Vec<String> {
        ["", "n"]
            .iter()
            .flat_map(|w| {
                std::iter::once(String::new())
                    .chain((1..=COLORS).map(move |c| format!("text_write_{c}{slot}{w}")))
                    .chain(std::iter::once(String::new()))
            })
            .collect()
    };
    vec![
        ("text_shifts", routines("")),
        ("text_returns_a", routines("_back_a")),
        ("text_returns_b", routines("_back_b")),
        ("text_writers_a", writers("a")),
        ("text_writers_b", writers("b")),
        (
            "text_ending_shifts",
            (0..8).map(|s| format!("text_ending_shift_{s}")).collect(),
        ),
        (
            "text_ending_writers",
            ENDING_WRITERS.iter().map(|w| w.to_string()).collect(),
        ),
    ]
}
const ENDING_WRITERS: [&str; 4] = [
    "text_ending_write_a1",
    "text_ending_write_a2",
    "text_ending_write_b1",
    "text_ending_write_b2",
];
const IA: u8 = 0x0c;
const MA: u8 = 0x0f;
const IB: u8 = 0x12;
const MB: u8 = 0x15;
const NM2: u8 = 0x18;
const NMW: u8 = 0x2e;
const COUNT: u8 = 0x19;
const WA: u16 = TEXT_DP + 0x1a;
const WB: u16 = TEXT_DP + 0x1c;
const RA: u16 = TEXT_DP + 0x22;
const RB: u16 = TEXT_DP + 0x24;
const GPTR: u8 = 0x1e;
const FIRST: u8 = 0x20;
const _: () = assert!(FIRST + 2 <= 0x22);
pub fn append(sections: &mut Vec<Section>, space: &mut crate::sfc_rom::Space) -> Result<()> {
    let origin = w65c816::CodeLocation::new(0xc0, 0);
    let bytes = assemble(origin, 0xc0_0000, None)?.bytes().len();
    let offset = space.reserve_fast(bytes)?;
    sections.push(Section {
        name: CODE_SECTION.into(),
        offset,
        bytes: vec![0; bytes],
    });
    Ok(())
}
pub fn place_code(sections: &mut [Section], layout: &CartridgeLayout) -> Result<()> {
    let bytes = code(layout)?.bytes().to_vec();
    let section = sections
        .iter_mut()
        .find(|s| s.name == CODE_SECTION)
        .ok_or("Missing B1 text drawing section")?;
    if section.bytes.len() != bytes.len() {
        return Err("B1 text drawing changed length once placed".into());
    }
    section.bytes = bytes;
    Ok(())
}
pub fn entry(layout: &CartridgeLayout) -> Result<u32> {
    let program = code(layout)?;
    let placed = &layout.section(CODE_SECTION)?.bytes;
    if placed[..program.bytes().len()] != *program.bytes() {
        return Err("B1 text drawing differs from its placed section".into());
    }
    Ok(program
        .label_location(ENTRY)
        .ok_or("Missing B1 text drawing entry")?
        .to_u32())
}
pub fn ending_entry(layout: &CartridgeLayout) -> Result<u32> {
    Ok(code(layout)?
        .label_location(ENDING_ENTRY)
        .ok_or("Missing ending glyph drawing entry")?
        .to_u32())
}
pub fn code_bytes(layout: &CartridgeLayout) -> Result<usize> {
    Ok(code(layout)?.bytes().len())
}
fn code(layout: &CartridgeLayout) -> Result<w65c816::AssembledProgram> {
    let at = layout.cpu_address(layout.section(CODE_SECTION)?.offset)?;
    let glyphs = layout.cpu_address(layout.section("b1_text_glyphs")?.offset)?;
    let origin = w65c816::CodeLocation::new((at >> 16) as u8, at as u16);
    let draft = assemble(origin, glyphs, None)?;
    let label = |name: &str| -> Result<u16> {
        Ok(draft
            .label_location(name)
            .ok_or_else(|| format!("Missing label {name}"))?
            .to_u32() as u16)
    };
    let mut words = Vec::new();
    for (_, names) in table_labels() {
        for name in names {
            let at = if name.is_empty() { 0 } else { label(&name)? };
            words.extend(at.to_le_bytes());
        }
    }
    assemble(origin, glyphs, Some(words))
}
fn assemble(
    origin: w65c816::CodeLocation,
    glyphs: u32,
    tables: Option<Vec<u8>>,
) -> Result<w65c816::AssembledProgram> {
    use crate::sfc::{emit, implied};
    let mut p = w65c816::Assembler::new();
    let p = &mut p;
    let dp = |p: &mut w65c816::Assembler, m, at: u8| emit(p, m, A::DirectPage, O::Byte(at));
    let imm8 = |p: &mut w65c816::Assembler, m, v: u8| emit(p, m, A::Immediate, O::Byte(v));
    let imm16 = |p: &mut w65c816::Assembler, m, v: u16| emit(p, m, A::Immediate, O::Word(v));
    let branch = |p: &mut w65c816::Assembler, m, label: &str| {
        p.emit_label_ref(m, A::Relative8, label);
    };
    let m16 = |p: &mut w65c816::Assembler| emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    let m8 = |p: &mut w65c816::Assembler| emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    let bank = glyphs & 0xff_0000;
    let row_ink = || O::Long(bank + 2);
    p.label(ENTRY).assume_mode(CpuMode::NATIVE_M16_X16);
    dp(p, M::Lda, LINE_Y);
    imm16(p, M::Cmp, LAST_LINE_Y as u16 + 1);
    branch(p, M::Bcs, "text_glyph_out");
    dp(p, M::Lda, PEN_X);
    imm16(p, M::Cmp, (COLUMNS * 8 - 16 + 1) as u16);
    branch(p, M::Bcc, "text_glyph_in");
    emit(p, M::Lda, A::DirectPageIndirectLong, O::Byte(GLYPH));
    implied(p, M::Xba);
    imm16(p, M::And, 0xff);
    implied(p, M::Clc);
    dp(p, M::Adc, PEN_X);
    imm16(p, M::Cmp, (COLUMNS * 8 + 1) as u16);
    branch(p, M::Bcc, "text_glyph_in");
    p.label("text_glyph_out")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    implied(p, M::Rtl);
    p.label("text_glyph_in")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    dp(p, M::Ldx, GLYPH);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(bank + FIRST_AT as u32));
    implied(p, M::Tay);
    imm16(p, M::And, 0xff);
    dp(p, M::Sta, FIRST);
    implied(p, M::Tya);
    implied(p, M::Xba);
    imm16(p, M::And, 0xff);
    branch(p, M::Beq, "text_glyph_out");
    dp(p, M::Sta, COUNT);
    implied(p, M::Txa);
    implied(p, M::Clc);
    dp(p, M::Adc, FIRST);
    dp(p, M::Sta, GPTR);
    let t0 = (WA - TEXT_DP) as u8;
    dp(p, M::Lda, LINE_Y);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Clc);
    dp(p, M::Adc, FIRST);
    implied(p, M::Tax);
    p.emit_label_ref(M::Lda, A::AbsoluteLongX, "text_lines");
    dp(p, M::Sta, t0);
    dp(p, M::Lda, PEN_X);
    imm16(p, M::And, 0xfff8);
    emit(p, M::Asl, A::Accumulator, O::None);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Clc);
    dp(p, M::Adc, t0);
    implied(p, M::Tay);
    dp(p, M::Lda, PEN_X);
    imm16(p, M::And, 7);
    emit(p, M::Asl, A::Accumulator, O::None);
    dp(p, M::Sta, FIRST);
    dp(p, M::Ldx, GLYPH);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(bank));
    implied(p, M::Xba);
    imm16(p, M::And, 0xff);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Clc);
    dp(p, M::Adc, FIRST);
    imm16(p, M::Cmp, 33);
    branch(p, M::Bcs, "text_glyph_wide");
    dp(p, M::Lda, FIRST);
    imm16(p, M::Ora, 16);
    dp(p, M::Sta, FIRST);
    p.label("text_glyph_wide")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    dp(p, M::Ldx, FIRST);
    p.emit_label_ref(M::Lda, A::AbsoluteLongX, "text_returns_a");
    dp(p, M::Sta, (RA - TEXT_DP) as u8);
    p.emit_label_ref(M::Lda, A::AbsoluteLongX, "text_returns_b");
    dp(p, M::Sta, (RB - TEXT_DP) as u8);
    dp(p, M::Lda, FIRST);
    imm16(p, M::And, 16);
    implied(p, M::Clc);
    dp(p, M::Adc, INK_COLOR);
    dp(p, M::Adc, INK_COLOR);
    implied(p, M::Tax);
    p.emit_label_ref(M::Lda, A::AbsoluteLongX, "text_writers_a");
    dp(p, M::Sta, (WA - TEXT_DP) as u8);
    p.emit_label_ref(M::Lda, A::AbsoluteLongX, "text_writers_b");
    dp(p, M::Sta, (WB - TEXT_DP) as u8);
    dp(p, M::Stz, IB);
    dp(p, M::Stz, IB + 1);
    dp(p, M::Ldx, FIRST);
    p.emit_label_ref(M::Jmp, A::AbsoluteIndexedIndirectX, "text_shifts");
    for narrow in [false, true] {
        for s in 0u8..8 {
            let name = format!("text_shift_{s}{}", if narrow { "n" } else { "" });
            p.label(&name).assume_mode(CpuMode::NATIVE_M16_X16);
            dp(p, M::Ldx, GPTR);
            let top = format!("{name}_row");
            let done = format!("{name}_done");
            p.label(&top).assume_mode(CpuMode::NATIVE_M16_X16);
            for (cur, mcur, prev, writer, slot) in [(IA, MA, IB, WA, "a"), (IB, MB, IA, WB, "b")] {
                if s == 0 && !narrow {
                    dp(p, M::Stz, cur);
                }
                emit(p, M::Lda, A::AbsoluteLongX, row_ink());
                for _ in 0..s {
                    emit(p, M::Lsr, A::Accumulator, O::None);
                }
                dp(p, M::Sta, cur + 1);
                if s != 0 && !narrow {
                    emit(p, M::Lda, A::AbsoluteLongX, row_ink());
                    for _ in s..8 {
                        emit(p, M::Asl, A::Accumulator, O::None);
                    }
                    dp(p, M::Sta, cur);
                }
                dp(p, M::Lda, prev + 1);
                emit(p, M::Lsr, A::Accumulator, O::None);
                dp(p, M::Ora, cur + 1);
                dp(p, M::Sta, mcur + 1);
                imm16(p, M::Eor, 0xffff);
                dp(p, M::Sta, NMW);
                m8(p);
                if !narrow {
                    dp(p, M::Lda, prev);
                    emit(p, M::Ror, A::Accumulator, O::None);
                    dp(p, M::Ora, cur);
                    dp(p, M::Sta, mcur);
                    imm8(p, M::Eor, 0xff);
                    dp(p, M::Sta, NM2);
                }
                implied(p, M::Inx);
                implied(p, M::Inx);
                emit(p, M::Jmp, A::AbsoluteIndirect, O::Word(writer));
                p.label(format!("{name}_back_{slot}"))
                    .assume_mode(CpuMode::NATIVE_M8_X16);
                dp(p, M::Dec, COUNT);
                branch(p, M::Beq, &done);
                m16(p);
                implied(p, M::Iny);
                implied(p, M::Iny);
                implied(p, M::Tya);
                imm16(p, M::And, 0x000f);
                let same = format!("{name}_{cur}_same");
                branch(p, M::Bne, &same);
                implied(p, M::Tya);
                implied(p, M::Clc);
                imm16(p, M::Adc, (COLUMNS * 32 - 16) as u16);
                implied(p, M::Tay);
                p.label(&same).assume_mode(CpuMode::NATIVE_M16_X16);
            }
            p.emit_label_ref(M::Jmp, A::Absolute, &top);
            p.label(&done).assume_mode(CpuMode::NATIVE_M8_X16);
            m16(p);
            implied(p, M::Rtl);
        }
    }
    for (slot, i, m, back) in [("a", IA, MA, RA), ("b", IB, MB, RB)] {
        for color in 1..=COLORS {
            let ink = ink_index(color);
            p.label(format!("text_write_{color}{slot}"))
                .assume_mode(CpuMode::NATIVE_M8_X16);
            for k in [2u8, 1, 0] {
                if k == 1 {
                    p.label(format!("text_write_{color}{slot}n"))
                        .assume_mode(CpuMode::NATIVE_M8_X16);
                }
                let (ik, mk, nm) = (i + 2 - k, m + 2 - k, [NMW + 1, NMW, NM2][usize::from(k)]);
                let skip = format!("text_write_{color}{slot}_{k}");
                dp(p, M::Lda, mk);
                branch(p, M::Beq, &skip);
                for (plane, bit, shadow) in [
                    (0u16, 1u8, true),
                    (1, 2, false),
                    (16, 4, true),
                    (17, 8, false),
                ] {
                    let at = (BUFFER as u16).wrapping_add(u16::from(k) * 32 + plane);
                    emit(p, M::Lda, A::AbsoluteY, O::Word(at));
                    match (ink & bit != 0, shadow) {
                        (true, true) => dp(p, M::Ora, mk),
                        (false, true) => {
                            dp(p, M::Ora, mk);
                            dp(p, M::Eor, ik);
                        }
                        (true, false) => {
                            dp(p, M::And, nm);
                            dp(p, M::Ora, ik);
                        }
                        (false, false) => dp(p, M::And, nm),
                    }
                    emit(p, M::Sta, A::AbsoluteY, O::Word(at));
                }
                p.label(&skip).assume_mode(CpuMode::NATIVE_M8_X16);
            }
            emit(p, M::Jmp, A::AbsoluteIndirect, O::Word(back));
        }
    }
    emit_ending(p, bank);
    let mut words = tables.map(|t| t.into_iter());
    for (name, names) in table_labels() {
        let bytes = 2 * names.len();
        let data = match &mut words {
            Some(w) => w.by_ref().take(bytes).collect(),
            None => vec![0; bytes],
        };
        p.label(name).data(data);
    }
    let lines: Vec<u8> = (0..ROWS * 8)
        .flat_map(|y| ((y / 8 * COLUMNS * 32 + y % 8 * 2) as u16).to_le_bytes())
        .collect();
    p.label("text_lines").data(lines);
    crate::sfc_width_check::assemble(p, origin, CpuMode::NATIVE_M16_X16)
}
const E_PREV: u8 = 0x30;
const E_CUR: u8 = 0x32;
const E_NEXT: u8 = 0x34;
const E_V: u8 = 0x36;
const E_T: u8 = 0x38;
const E_I: u8 = 0x3a;
const E_D: u8 = 0x3d;
const E_COUNT: u8 = 0x41;
const E_GPTR: u8 = 0x46;
const E_WRITER: u8 = 0x48;
const _: () =
    assert!(E_COUNT + 2 <= crate::sfc_ending::TILE && E_GPTR >= crate::sfc_ending::TILE + 2);
fn emit_ending(p: &mut w65c816::Assembler, bank: u32) {
    use crate::sfc::{emit, implied};
    use crate::sfc_ending::{DP, GLYPH, INK, LINE_Y, PEN_X, TEXT_BUFFER, TILE};
    let dp = |p: &mut w65c816::Assembler, m, at: u8| emit(p, m, A::DirectPage, O::Byte(at));
    let imm8 = |p: &mut w65c816::Assembler, m, v: u8| emit(p, m, A::Immediate, O::Byte(v));
    let imm16 = |p: &mut w65c816::Assembler, m, v: u16| emit(p, m, A::Immediate, O::Word(v));
    let acc = |p: &mut w65c816::Assembler, m| emit(p, m, A::Accumulator, O::None);
    let branch = |p: &mut w65c816::Assembler, m, label: &str| {
        p.emit_label_ref(m, A::Relative8, label);
    };
    let m16 = |p: &mut w65c816::Assembler| emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    let m8 = |p: &mut w65c816::Assembler| emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    p.label(ENDING_ENTRY).assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    dp(p, M::Ldx, GLYPH);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(bank + FIRST_AT as u32));
    implied(p, M::Tay);
    imm16(p, M::And, 0xff);
    dp(p, M::Sta, E_V);
    implied(p, M::Tya);
    implied(p, M::Xba);
    imm16(p, M::And, 0xff);
    branch(p, M::Bne, "text_ending_rows");
    m8(p);
    implied(p, M::Rtl);
    p.label("text_ending_rows")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    dp(p, M::Sta, E_COUNT);
    implied(p, M::Txa);
    implied(p, M::Clc);
    dp(p, M::Adc, E_V);
    dp(p, M::Sta, E_GPTR);
    dp(p, M::Lda, E_V);
    acc(p, M::Lsr);
    implied(p, M::Clc);
    dp(p, M::Adc, LINE_Y);
    acc(p, M::Dec);
    dp(p, M::Sta, E_T);
    imm16(p, M::And, 0xfff8);
    acc(p, M::Asl);
    acc(p, M::Asl);
    dp(p, M::Sta, E_NEXT);
    dp(p, M::Lda, PEN_X);
    for _ in 0..3 {
        acc(p, M::Lsr);
    }
    acc(p, M::Dec);
    implied(p, M::Clc);
    dp(p, M::Adc, E_NEXT);
    for _ in 0..4 {
        acc(p, M::Asl);
    }
    dp(p, M::Sta, E_NEXT);
    dp(p, M::Lda, TILE);
    imm16(p, M::Cmp, 32);
    dp(p, M::Lda, E_NEXT);
    branch(p, M::Bcc, "text_ending_tile16");
    acc(p, M::Asl);
    p.label("text_ending_tile16")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    dp(p, M::Sta, E_NEXT);
    dp(p, M::Lda, E_T);
    imm16(p, M::And, 7);
    acc(p, M::Asl);
    implied(p, M::Clc);
    dp(p, M::Adc, E_NEXT);
    implied(p, M::Tay);
    imm16(p, M::Lda, 32);
    implied(p, M::Sec);
    dp(p, M::Sbc, TILE);
    acc(p, M::Lsr);
    acc(p, M::Lsr);
    dp(p, M::Sta, E_T);
    dp(p, M::Lda, INK);
    imm16(p, M::And, 0xff);
    acc(p, M::Asl);
    implied(p, M::Clc);
    dp(p, M::Adc, E_T);
    acc(p, M::Dec);
    acc(p, M::Dec);
    implied(p, M::Tax);
    p.emit_label_ref(M::Lda, A::AbsoluteLongX, "text_ending_writers");
    dp(p, M::Sta, E_WRITER);
    dp(p, M::Stz, E_PREV);
    dp(p, M::Stz, E_CUR);
    dp(p, M::Stz, E_D + 2);
    dp(p, M::Lda, PEN_X);
    imm16(p, M::And, 7);
    acc(p, M::Asl);
    implied(p, M::Tax);
    p.emit_label_ref(M::Jmp, A::AbsoluteIndexedIndirectX, "text_ending_shifts");
    for s in 0u8..8 {
        let name = format!("text_ending_shift_{s}");
        let row = format!("{name}_one");
        p.label(&name).assume_mode(CpuMode::NATIVE_M16_X16);
        dp(p, M::Ldx, E_GPTR);
        p.label(format!("{name}_loop"))
            .assume_mode(CpuMode::NATIVE_M16_X16);
        emit(p, M::Lda, A::AbsoluteLongX, O::Long(bank + 2));
        p.emit_label_ref(M::Jsr, A::Absolute, &row);
        implied(p, M::Inx);
        implied(p, M::Inx);
        dp(p, M::Dec, E_COUNT);
        branch(p, M::Bne, &format!("{name}_loop"));
        imm16(p, M::Lda, 0);
        p.emit_label_ref(M::Jsr, A::Absolute, &row);
        m8(p);
        implied(p, M::Rtl);
        p.label(&row).assume_mode(CpuMode::NATIVE_M16_X16);
        dp(p, M::Sta, E_NEXT);
        dp(p, M::Ora, E_CUR);
        dp(p, M::Ora, E_PREV);
        dp(p, M::Sta, E_V);
        acc(p, M::Asl);
        dp(p, M::Ora, E_V);
        dp(p, M::Sta, E_T);
        dp(p, M::Lda, E_V);
        acc(p, M::Lsr);
        dp(p, M::Ora, E_T);
        dp(p, M::Sta, E_T);
        if s == 0 {
            implied(p, M::Xba);
            imm16(p, M::And, 0xff00);
            dp(p, M::Sta, E_D);
            dp(p, M::Lda, E_T);
            dp(p, M::Sta, E_D + 1);
        } else {
            dp(p, M::Lda, E_V);
            acc(p, M::Asl);
            dp(p, M::Lda, E_T);
            acc(p, M::Ror);
            for _ in 1..s {
                acc(p, M::Lsr);
            }
            dp(p, M::Sta, E_D + 1);
            dp(p, M::Lda, E_T);
            for _ in s..8 {
                acc(p, M::Asl);
            }
            dp(p, M::Sta, E_D);
        }
        dp(p, M::Lda, E_CUR);
        if s == 0 {
            implied(p, M::Xba);
            imm16(p, M::And, 0xff00);
            dp(p, M::Sta, E_I);
            dp(p, M::Lda, E_CUR);
            dp(p, M::Sta, E_I + 1);
        } else {
            for _ in 0..s {
                acc(p, M::Lsr);
            }
            dp(p, M::Sta, E_I + 1);
            dp(p, M::Lda, E_CUR);
            for _ in s..8 {
                acc(p, M::Asl);
            }
            dp(p, M::Sta, E_I);
        }
        dp(p, M::Lda, E_CUR);
        dp(p, M::Sta, E_PREV);
        dp(p, M::Lda, E_NEXT);
        dp(p, M::Sta, E_CUR);
        m8(p);
        if s == 0 {
            dp(p, M::Lda, E_V + 1);
            acc(p, M::Asl);
            imm8(p, M::Lda, 0);
            acc(p, M::Rol);
            dp(p, M::Sta, E_D + 3);
        }
        emit(
            p,
            M::Jmp,
            A::AbsoluteIndirect,
            O::Word(DP + u16::from(E_WRITER)),
        );
    }
    for (name, tile, ink) in [
        (ENDING_WRITERS[0], 32u16, 1u8),
        (ENDING_WRITERS[1], 32, 2),
        (ENDING_WRITERS[2], 16, 1),
        (ENDING_WRITERS[3], 16, 2),
    ] {
        p.label(name).assume_mode(CpuMode::NATIVE_M8_X16);
        let (t0, t1) = (E_T, E_T + 1);
        for k in 0..4u8 {
            let d = E_D + 3 - k;
            let i = (k > 0).then(|| E_I + 3 - k);
            let at = (TEXT_BUFFER as u16).wrapping_add(u16::from(k) * tile);
            let skip = format!("{name}_{k}");
            dp(p, M::Lda, d);
            branch(p, M::Beq, &skip);
            emit(p, M::Lda, A::AbsoluteY, O::Word(at + 1));
            dp(p, M::Sta, t1);
            emit(p, M::Lda, A::AbsoluteY, O::Word(at));
            dp(p, M::Sta, t0);
            let (lack, has) = if ink == 1 { (at + 1, at) } else { (at, at + 1) };
            let (lack_own, has_own) = if ink == 1 { (t1, t0) } else { (t0, t1) };
            let (lack_other, has_other) = if ink == 1 { (t0, t1) } else { (t1, t0) };
            for (target, own, other) in [(lack, lack_own, lack_other), (has, has_own, has_other)] {
                dp(p, M::Lda, other);
                imm8(p, M::Eor, 0xff);
                dp(p, M::And, d);
                dp(p, M::Ora, own);
                if let Some(i) = i {
                    dp(p, M::Ora, i);
                    if target == lack {
                        dp(p, M::Eor, i);
                    }
                }
                emit(p, M::Sta, A::AbsoluteY, O::Word(target));
            }
            p.label(&skip).assume_mode(CpuMode::NATIVE_M8_X16);
        }
        m16(p);
        implied(p, M::Iny);
        implied(p, M::Iny);
        implied(p, M::Tya);
        imm16(p, M::And, 0x000f);
        let same = format!("{name}_same");
        branch(p, M::Bne, &same);
        implied(p, M::Tya);
        implied(p, M::Clc);
        imm16(p, M::Adc, 32 * tile - 16);
        implied(p, M::Tay);
        p.label(&same).assume_mode(CpuMode::NATIVE_M16_X16);
        implied(p, M::Rts);
    }
}
