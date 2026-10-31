use super::*;
pub(super) fn emit_arle(
    p: &mut Assembler,
    records: u32,
    names: u32,
    indirect: impl Fn(&mut Assembler, M),
) -> Result<()> {
    let long_x = |p: &mut Assembler, m, at: u32| emit(p, m, A::AbsoluteLongX, O::Long(at));
    let slot = |s: u16| QUEUE + s * 5;
    p.label("battle_display_arle")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    abs(p, M::Lda, ARLE_FRAME);
    word(p, M::Cmp, 0xffff);
    branch(p, M::Bne, "battle_display_arle_some");
    m8(p);
    implied(p, M::Rts);
    p.label("battle_display_arle_some")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    abs(p, M::Lda, ARLE_FRAME);
    abs(p, M::Cmp, ARLE_SHOWN);
    branch(p, M::Bne, "battle_display_arle_changed");
    abs(p, M::Lda, ARLE_X);
    abs(p, M::Cmp, ARLE_SHOWN_X);
    branch(p, M::Bne, "battle_display_arle_changed");
    abs(p, M::Lda, ARLE_Y);
    abs(p, M::Cmp, ARLE_SHOWN_Y);
    branch(p, M::Bne, "battle_display_arle_changed");
    m8(p);
    implied(p, M::Rts);
    p.label("battle_display_arle_changed")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    abs(p, M::Lda, slot(ARLE_SLOT) + 3);
    branch(p, M::Beq, "battle_display_arle_free");
    m8(p);
    implied(p, M::Rts);
    p.label("battle_display_arle_free")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    abs(p, M::Lda, crate::sfc_b1_text::SHARED);
    branch(p, M::Beq, "battle_display_arle_unshared");
    abs(p, M::Lda, QUEUE + 5 + 3);
    branch(p, M::Beq, "battle_display_arle_unshared");
    m8(p);
    implied(p, M::Rts);
    p.label("battle_display_arle_unshared")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    abs(p, M::Lda, ARLE_BANK);
    word(p, M::Eor, 1);
    word(p, M::And, 1);
    abs(p, M::Sta, ARLE_BANK);
    branch(p, M::Beq, "battle_display_arle_base");
    word(p, M::Lda, ARLE_TILES as u16 + 1);
    p.label("battle_display_arle_base");
    abs(p, M::Sta, ARLE_BASE);
    abs(p, M::Lda, ARLE_FRAME);
    abs(p, M::Sta, ARLE_SHOWN);
    abs(p, M::Lda, ARLE_X);
    abs(p, M::Sta, ARLE_SHOWN_X);
    abs(p, M::Lda, ARLE_Y);
    abs(p, M::Sta, ARLE_SHOWN_Y);
    abs(p, M::Lda, ARLE_FRAME);
    implied(p, M::Clc);
    word(p, M::Adc, records as u16);
    emit(p, M::Sta, A::DirectPage, O::Byte(RECORD));
    m8(p);
    byte(p, M::Lda, (records >> 16) as u8);
    emit(p, M::Sta, A::DirectPage, O::Byte(RECORD + 2));
    m16(p);
    emit(
        p,
        M::Ldx,
        A::Immediate,
        O::Word(*PANEL_COLUMNS.start() as u16 * 2),
    );
    p.label("battle_display_arle_clear");
    word(p, M::Lda, BLANK);
    for c in 0..(PANEL_COLUMNS.end() - PANEL_COLUMNS.start() + 1) as u32 {
        long_x(p, M::Sta, SHADOW + c * 2);
    }
    implied(p, M::Txa);
    implied(p, M::Clc);
    word(p, M::Adc, 64);
    implied(p, M::Tax);
    emit(
        p,
        M::Cpx,
        A::Immediate,
        O::Word(16 * 64 + *PANEL_COLUMNS.start() as u16 * 2),
    );
    branch(p, M::Bne, "battle_display_arle_clear");
    for (from, to, k) in [(ARLE_X, AT_X, 12u16), (ARLE_Y, AT_Y, 13)] {
        emit(p, M::Ldy, A::Immediate, O::Word(k));
        indirect(p, M::Lda);
        word(p, M::And, 0xff);
        abs(p, M::Sta, DATA);
        abs(p, M::Lda, from);
        implied(p, M::Sec);
        abs(p, M::Sbc, DATA);
        implied(p, M::Clc);
        word(p, M::Adc, 4);
        for _ in 0..3 {
            emit(p, M::Cmp, A::Immediate, O::Word(0x8000));
            emit(p, M::Ror, A::Accumulator, O::None);
        }
        abs(p, M::Sta, to);
    }
    m8(p);
    emit(p, M::Ldy, A::Immediate, O::Word(10));
    indirect(p, M::Lda);
    abs(p, M::Sta, COUNT);
    emit(p, M::Ldy, A::Immediate, O::Word(11));
    indirect(p, M::Lda);
    abs(p, M::Sta, INDEX);
    m16(p);
    abs(p, M::Lda, AT_Y);
    abs(p, M::Sta, ROW);
    emit(p, M::Ldy, A::Immediate, O::Word(14));
    p.label("battle_display_arle_row");
    abs(p, M::Lda, AT_X);
    abs(p, M::Sta, COLUMN);
    m8(p);
    abs(p, M::Lda, COUNT);
    abs(p, M::Sta, HIGH);
    m16(p);
    p.label("battle_display_arle_cell");
    indirect(p, M::Lda);
    branch(p, M::Beq, "battle_display_arle_next");
    implied(p, M::Clc);
    abs(p, M::Adc, ARLE_BASE);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Tax);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(names));
    abs(p, M::Sta, CELL);
    abs(p, M::Lda, ROW);
    word(p, M::Cmp, *PANEL_ROWS.start() as u16);
    branch(p, M::Bmi, "battle_display_arle_next");
    word(p, M::Cmp, *PANEL_ROWS.end() as u16 + 1);
    branch(p, M::Bpl, "battle_display_arle_next");
    abs(p, M::Lda, COLUMN);
    word(p, M::Cmp, *PANEL_COLUMNS.start() as u16);
    branch(p, M::Bmi, "battle_display_arle_next");
    word(p, M::Cmp, *PANEL_COLUMNS.end() as u16 + 1);
    branch(p, M::Bpl, "battle_display_arle_next");
    abs(p, M::Lda, ROW);
    emit(p, M::Dec, A::Accumulator, O::None);
    for _ in 0..5 {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    implied(p, M::Clc);
    abs(p, M::Adc, COLUMN);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Tax);
    abs(p, M::Lda, CELL);
    long_x(p, M::Sta, SHADOW);
    p.label("battle_display_arle_next");
    implied(p, M::Iny);
    implied(p, M::Iny);
    abs(p, M::Inc, COLUMN);
    m8(p);
    abs(p, M::Dec, HIGH);
    m16(p);
    branch(p, M::Bne, "battle_display_arle_cell");
    abs(p, M::Inc, ROW);
    m8(p);
    abs(p, M::Dec, INDEX);
    m16(p);
    branch(p, M::Bne, "battle_display_arle_row");
    m8(p);
    abs(p, M::Lda, ARLE_BANK);
    branch(p, M::Bne, "battle_display_arle_job_1");
    for (bank, label) in [(0u16, None), (1, Some("battle_display_arle_job_1"))] {
        if let Some(label) = label {
            p.label(label).assume_mode(CpuMode::NATIVE_M8_X16);
        }
        for i in 0..5u16 {
            emit(p, M::Ldy, A::Immediate, O::Word(bank * 5 + i));
            indirect(p, M::Lda);
            abs(p, M::Sta, slot(ARLE_SLOT) + i);
        }
        implied(p, M::Rts);
    }
    Ok(())
}
