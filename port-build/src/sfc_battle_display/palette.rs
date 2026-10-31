use super::*;
pub(super) fn emit_palette(p: &mut Assembler, blocks: u32, rgb: u32) {
    let long_x = |p: &mut Assembler, m, at: u32| emit(p, m, A::AbsoluteLongX, O::Long(at));
    let jmp = |p: &mut Assembler, label: &str| {
        p.emit_label_ref(M::Jmp, A::Absolute, label);
    };
    let slot = |s: u16| QUEUE + s * 5;
    let table = |p: &mut Assembler| {
        abs(p, M::Lda, crate::sfc_encounter::ENEMY);
        word(p, M::And, 0xff);
        for _ in 0..6 {
            emit(p, M::Asl, A::Accumulator, O::None);
        }
        abs(p, M::Sta, PALETTE_TABLE);
        abs(p, M::Stz, PALETTE_BLOCK);
    };
    let entry = |p: &mut Assembler| {
        abs(p, M::Lda, PALETTE_TABLE);
        implied(p, M::Clc);
        abs(p, M::Adc, PALETTE_BLOCK);
        implied(p, M::Tax);
        long_x(p, M::Lda, blocks);
        word(p, M::And, 0xff);
    };
    let next_block = |p: &mut Assembler, label: &str| {
        abs(p, M::Lda, PALETTE_BLOCK);
        implied(p, M::Clc);
        word(p, M::Adc, 4);
        abs(p, M::Sta, PALETTE_BLOCK);
        word(p, M::Cmp, 64);
        branch(p, M::Bne, label);
    };
    p.label("battle_display_palette_start")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    word(p, M::Lda, 248);
    abs(p, M::Sta, crate::sfc_ending::SCREEN_KEPT);
    abs(p, M::Stz, PALETTE_DIRTY);
    table(p);
    p.label("bdp_start_block");
    entry(p);
    branch(p, M::Beq, "bdp_start_next");
    long_x(p, M::Lda, blocks + 2);
    abs(p, M::Sta, CELL);
    abs(p, M::Lda, PALETTE_BLOCK);
    emit(p, M::Asl, A::Accumulator, O::None);
    emit(p, M::Asl, A::Accumulator, O::None);
    abs(p, M::Sta, ROW);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Clc);
    abs(p, M::Adc, ROW);
    abs(p, M::Sta, COLUMN);
    emit(p, M::Ldy, A::Immediate, O::Word(0));
    p.label("bdp_start_copy");
    implied(p, M::Tya);
    implied(p, M::Clc);
    abs(p, M::Adc, CELL);
    implied(p, M::Tax);
    long_x(p, M::Lda, rgb);
    implied(p, M::Pha);
    implied(p, M::Tya);
    implied(p, M::Clc);
    abs(p, M::Adc, COLUMN);
    implied(p, M::Tax);
    implied(p, M::Pla);
    long_x(p, M::Sta, PALETTE_CURRENT);
    long_x(p, M::Sta, PALETTE_KEPT);
    implied(p, M::Iny);
    implied(p, M::Iny);
    emit(p, M::Cpy, A::Immediate, O::Word(48));
    branch(p, M::Bne, "bdp_start_copy");
    p.label("bdp_start_next");
    next_block(p, "bdp_start_block");
    m8(p);
    implied(p, M::Rts);
    p.label("battle_display_palette")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    abs(p, M::Lda, PALETTE_DIRTY);
    branch(p, M::Bne, "bdp_some");
    m8(p);
    implied(p, M::Rts);
    p.label("bdp_some").assume_mode(CpuMode::NATIVE_M16_X16);
    abs(p, M::Lda, slot(PALETTE_SLOT) + 3);
    branch(p, M::Beq, "bdp_free");
    m8(p);
    implied(p, M::Rts);
    p.label("bdp_free").assume_mode(CpuMode::NATIVE_M16_X16);
    table(p);
    abs(p, M::Stz, PALETTE_SENT);
    p.label("bdp_block");
    emit(p, M::Lsr, A::Absolute, O::Word(PALETTE_DIRTY));
    branch(p, M::Bcs, "bdp_changed");
    jmp(p, "bdp_next");
    p.label("bdp_changed");
    entry(p);
    branch(p, M::Bne, "bdp_drawn");
    jmp(p, "bdp_next");
    p.label("bdp_drawn");
    abs(p, M::Sta, CELL);
    long_x(p, M::Lda, blocks + 1);
    word(p, M::And, 0xff);
    abs(p, M::Sta, AT_X);
    abs(p, M::Lda, PALETTE_BLOCK);
    emit(p, M::Asl, A::Accumulator, O::None);
    emit(p, M::Asl, A::Accumulator, O::None);
    abs(p, M::Sta, ROW);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Clc);
    abs(p, M::Adc, ROW);
    abs(p, M::Sta, AT_Y);
    abs(p, M::Lda, PALETTE_SENT);
    for _ in 0..5 {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    abs(p, M::Sta, ROW);
    emit(p, M::Ldy, A::Immediate, O::Word(0));
    p.label("bdp_color");
    implied(p, M::Tya);
    abs(p, M::Cmp, AT_X);
    branch(p, M::Bne, "bdp_not_key");
    word(p, M::Lda, 0);
    branch(p, M::Bra, "bdp_source");
    p.label("bdp_not_key");
    word(p, M::Cmp, 0);
    branch(p, M::Bne, "bdp_source");
    abs(p, M::Lda, AT_X);
    p.label("bdp_source");
    abs(p, M::Sta, COLUMN);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Clc);
    abs(p, M::Adc, COLUMN);
    implied(p, M::Clc);
    abs(p, M::Adc, AT_Y);
    implied(p, M::Tax);
    long_x(p, M::Lda, PALETTE_CURRENT);
    word(p, M::And, 0xf8);
    for _ in 0..3 {
        emit(p, M::Lsr, A::Accumulator, O::None);
    }
    abs(p, M::Sta, COLUMN);
    long_x(p, M::Lda, PALETTE_CURRENT + 1);
    word(p, M::And, 0xf8);
    for _ in 0..2 {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    abs(p, M::Ora, COLUMN);
    abs(p, M::Sta, COLUMN);
    long_x(p, M::Lda, PALETTE_CURRENT + 2);
    word(p, M::And, 0xf8);
    for _ in 0..7 {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    abs(p, M::Ora, COLUMN);
    abs(p, M::Sta, COLUMN);
    implied(p, M::Tya);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Clc);
    abs(p, M::Adc, ROW);
    implied(p, M::Tax);
    abs(p, M::Lda, COLUMN);
    long_x(p, M::Sta, PALETTE_BUFFER);
    implied(p, M::Iny);
    emit(p, M::Cpy, A::Immediate, O::Word(16));
    branch(p, M::Bne, "bdp_color");
    abs(p, M::Lda, PALETTE_SENT);
    for _ in 0..3 {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    implied(p, M::Tax);
    word(p, M::Lda, 1);
    long_x(p, M::Sta, PALETTE_LIST);
    abs(p, M::Lda, CELL);
    long_x(p, M::Sta, PALETTE_LIST + 1);
    word(p, M::Lda, 32);
    long_x(p, M::Sta, PALETTE_LIST + 3);
    abs(p, M::Lda, ROW);
    implied(p, M::Clc);
    word(p, M::Adc, PALETTE_BUFFER as u16);
    long_x(p, M::Sta, PALETTE_LIST + 5);
    word(p, M::Lda, (PALETTE_BUFFER >> 16) as u16);
    long_x(p, M::Sta, PALETTE_LIST + 7);
    abs(p, M::Inc, PALETTE_SENT);
    arle_effects::emit_palette_mirror(p);
    p.label("bdp_next");
    next_block(p, "bdp_far");
    abs(p, M::Lda, PALETTE_SENT);
    branch(p, M::Beq, "bdp_done");
    word(p, M::Lda, PALETTE_LIST as u16);
    abs(p, M::Sta, slot(PALETTE_SLOT));
    m8(p);
    byte(p, M::Lda, (PALETTE_LIST >> 16) as u8);
    abs(p, M::Sta, slot(PALETTE_SLOT) + 2);
    m16(p);
    abs(p, M::Lda, PALETTE_SENT);
    abs(p, M::Sta, slot(PALETTE_SLOT) + 3);
    p.label("bdp_done");
    m8(p);
    implied(p, M::Rts);
    p.label("bdp_far").assume_mode(CpuMode::NATIVE_M16_X16);
    jmp(p, "bdp_block");
}
