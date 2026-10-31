use super::*;
pub(super) fn emit_order(p: &mut Assembler) {
    p.label("bdo_reverse").assume_mode(CpuMode::NATIVE_M16_X16);
    implied(p, M::Plb);
    abs(p, M::Lda, VISIBLE);
    word(p, M::Cmp, Z_FEW_OBJECTS);
    br(p, M::Bcc, "bdo_zgeneral");
    long(p, M::Lda, Z_TRY_COUNT);
    br(p, M::Bne, "bdo_zsort");
    p.label("bdo_zgeneral");
    jmp(p, "bdo_reverse_general");
    p.label("bdo_zsort");
    let list = VISIBLE_LIST as u16;
    let end = list + VISIBLE_MAX * 4;
    let short = |v: u32| v as u16;
    implied(p, M::Phb);
    m8(p);
    byte(p, M::Lda, (VISIBLE_LIST >> 16) as u8);
    implied(p, M::Pha);
    implied(p, M::Plb);
    m16(p);
    word(p, M::Lda, VISIBLE_MAX);
    implied(p, M::Sec);
    abs(p, M::Sbc, VISIBLE);
    emit(p, M::Asl, A::Accumulator, O::None);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Clc);
    word(p, M::Adc, list);
    abs(p, M::Sta, short(Z_FROM));
    implied(p, M::Tay);
    emit(p, M::Lda, A::AbsoluteY, O::Word(0));
    abs(p, M::Sta, short(Z_LOW));
    abs(p, M::Sta, short(Z_HIGH));
    abs(p, M::Sta, short(Z_PREVIOUS));
    abs(p, M::Stz, short(Z_DESCENTS));
    p.label("bdo_zrange");
    emit(p, M::Lda, A::AbsoluteY, O::Word(0));
    abs(p, M::Cmp, short(Z_PREVIOUS));
    br(p, M::Bcs, "bdo_zrange_up");
    abs(p, M::Inc, short(Z_DESCENTS));
    p.label("bdo_zrange_up");
    abs(p, M::Sta, short(Z_PREVIOUS));
    abs(p, M::Cmp, short(Z_LOW));
    br(p, M::Bcs, "bdo_zrange_low");
    abs(p, M::Sta, short(Z_LOW));
    br(p, M::Bra, "bdo_zrange_next");
    p.label("bdo_zrange_low");
    abs(p, M::Cmp, short(Z_HIGH));
    br(p, M::Bcc, "bdo_zrange_next");
    abs(p, M::Sta, short(Z_HIGH));
    p.label("bdo_zrange_next");
    for _ in 0..4 {
        implied(p, M::Iny);
    }
    emit(p, M::Cpy, A::Immediate, O::Word(end));
    br(p, M::Bne, "bdo_zrange");
    abs(p, M::Lda, short(Z_DESCENTS));
    word(p, M::Cmp, Z_FEW_DESCENTS);
    br(p, M::Bcs, "bdo_zspan");
    implied(p, M::Plb);
    jmp(p, "bdo_reverse_general");
    p.label("bdo_zspan");
    abs(p, M::Lda, short(Z_HIGH));
    implied(p, M::Sec);
    abs(p, M::Sbc, short(Z_LOW));
    word(p, M::Cmp, Z_SPAN);
    br(p, M::Bcc, "bdo_zcount");
    implied(p, M::Plb);
    jmp(p, "bdo_reverse_general");
    p.label("bdo_zcount");
    emit(p, M::Ldx, A::Immediate, O::Word(Z_SPAN * 2 - 2));
    p.label("bdo_zclear");
    emit(p, M::Stz, A::AbsoluteX, O::Word(short(Z_COUNTS)));
    implied(p, M::Dex);
    implied(p, M::Dex);
    br(p, M::Bpl, "bdo_zclear");
    abs(p, M::Ldy, short(Z_FROM));
    p.label("bdo_ztally");
    emit(p, M::Lda, A::AbsoluteY, O::Word(0));
    implied(p, M::Sec);
    abs(p, M::Sbc, short(Z_LOW));
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Tax);
    emit(p, M::Inc, A::AbsoluteX, O::Word(short(Z_COUNTS)));
    for _ in 0..4 {
        implied(p, M::Iny);
    }
    emit(p, M::Cpy, A::Immediate, O::Word(end));
    br(p, M::Bne, "bdo_ztally");
    emit(p, M::Ldy, A::Immediate, O::Word(short(SORTED)));
    emit(p, M::Ldx, A::Immediate, O::Word(0));
    p.label("bdo_zplaces");
    emit(p, M::Lda, A::AbsoluteX, O::Word(short(Z_COUNTS)));
    emit(p, M::Asl, A::Accumulator, O::None);
    abs(p, M::Sta, short(Z_HIGH));
    implied(p, M::Tya);
    emit(p, M::Sta, A::AbsoluteX, O::Word(short(Z_COUNTS)));
    implied(p, M::Clc);
    abs(p, M::Adc, short(Z_HIGH));
    implied(p, M::Tay);
    implied(p, M::Inx);
    implied(p, M::Inx);
    emit(p, M::Cpx, A::Immediate, O::Word(Z_SPAN * 2));
    br(p, M::Bne, "bdo_zplaces");
    abs(p, M::Ldy, short(Z_FROM));
    p.label("bdo_zplace");
    emit(p, M::Lda, A::AbsoluteY, O::Word(0));
    implied(p, M::Sec);
    abs(p, M::Sbc, short(Z_LOW));
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Tax);
    emit(p, M::Lda, A::AbsoluteX, O::Word(short(Z_COUNTS)));
    emit(p, M::Inc, A::Accumulator, O::None);
    emit(p, M::Inc, A::Accumulator, O::None);
    emit(p, M::Sta, A::AbsoluteX, O::Word(short(Z_COUNTS)));
    emit(p, M::Dec, A::Accumulator, O::None);
    emit(p, M::Dec, A::Accumulator, O::None);
    implied(p, M::Tax);
    emit(p, M::Lda, A::AbsoluteY, O::Word(2));
    emit(p, M::Sta, A::AbsoluteX, O::Word(0));
    for _ in 0..4 {
        implied(p, M::Iny);
    }
    emit(p, M::Cpy, A::Immediate, O::Word(end));
    br(p, M::Bne, "bdo_zplace");
    abs(p, M::Lda, VISIBLE);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Clc);
    word(p, M::Adc, short(SORTED));
    abs(p, M::Sta, short(Z_HIGH));
    emit(p, M::Ldx, A::Immediate, O::Word(short(SORTED)));
    emit(p, M::Ldy, A::Immediate, O::Word(list + 2));
    word(p, M::And, 6);
    br(p, M::Beq, "bdo_zback_four");
    p.label("bdo_zback_remainder");
    emit(p, M::Lda, A::AbsoluteX, O::Word(0));
    emit(p, M::Sta, A::AbsoluteY, O::Word(0));
    implied(p, M::Inx);
    implied(p, M::Inx);
    for _ in 0..4 {
        implied(p, M::Iny);
    }
    implied(p, M::Txa);
    abs(p, M::Eor, short(Z_HIGH));
    word(p, M::And, 6);
    br(p, M::Bne, "bdo_zback_remainder");
    p.label("bdo_zback_four");
    for k in 0..4 {
        emit(p, M::Lda, A::AbsoluteX, O::Word(k * 2));
        emit(p, M::Sta, A::AbsoluteY, O::Word(k * 4));
    }
    implied(p, M::Tya);
    implied(p, M::Clc);
    word(p, M::Adc, 16);
    implied(p, M::Tay);
    implied(p, M::Txa);
    implied(p, M::Clc);
    word(p, M::Adc, 8);
    implied(p, M::Tax);
    abs(p, M::Cpx, short(Z_HIGH));
    br(p, M::Bne, "bdo_zback_four");
    implied(p, M::Dex);
    implied(p, M::Dex);
    emit(p, M::Lda, A::AbsoluteX, O::Word(0));
    implied(p, M::Inx);
    implied(p, M::Inx);
    implied(p, M::Plb);
    jmp(p, "bdo_draw");
    p.label("bdo_reverse_general")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    abs(p, M::Lda, VISIBLE);
    br(p, M::Beq, "bdo_sort");
    emit(p, M::Asl, A::Accumulator, O::None);
    emit(p, M::Asl, A::Accumulator, O::None);
    abs(p, M::Sta, AT_X);
    word(
        p,
        M::Lda,
        (VISIBLE_LIST as u16).wrapping_add(VISIBLE_MAX * 4),
    );
    implied(p, M::Sec);
    abs(p, M::Sbc, AT_X);
    implied(p, M::Tax);
    emit(p, M::Ldy, A::Immediate, O::Word(VISIBLE_LIST as u16));
    abs(p, M::Lda, AT_X);
    emit(p, M::Dec, A::Accumulator, O::None);
    implied(p, M::Phb);
    let bank = (VISIBLE_LIST >> 16) as u8;
    emit(
        p,
        M::Mvn,
        A::BlockMove,
        O::BlockMove {
            destination_bank: bank,
            source_bank: bank,
        },
    );
    implied(p, M::Plb);
    p.label("bdo_sort").assume_mode(CpuMode::NATIVE_M16_X16);
    implied(p, M::Phb);
    m8(p);
    byte(p, M::Lda, (VISIBLE_LIST >> 16) as u8);
    implied(p, M::Pha);
    implied(p, M::Plb);
    m16(p);
    let list = VISIBLE_LIST as u16;
    const _: () = assert!(INDEX < 0x2000 && VISIBLE < 0x2000 && AT_X < 0x2000 && AT_Y < 0x2000);
    abs(p, M::Stz, Z_MOVES as u16);
    word(p, M::Lda, 1);
    abs(p, M::Sta, INDEX);
    p.label("bdo_sort_outer");
    abs(p, M::Lda, INDEX);
    abs(p, M::Cmp, VISIBLE);
    br(p, M::Bcc, "bdo_sort_take");
    abs(p, M::Lda, VISIBLE);
    implied(p, M::Clc);
    word(p, M::Adc, 16);
    abs(p, M::Cmp, Z_MOVES as u16);
    word(p, M::Lda, 0);
    emit(p, M::Rol, A::Accumulator, O::None);
    emit(p, M::Eor, A::Immediate, O::Word(1));
    abs(p, M::Sta, Z_TRY_COUNT as u16);
    implied(p, M::Plb);
    jmp(p, "bdo_draw");
    p.label("bdo_sort_take");
    emit(p, M::Asl, A::Accumulator, O::None);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Tax);
    emit(p, M::Lda, A::AbsoluteX, O::Word(list));
    abs(p, M::Sta, AT_X);
    emit(p, M::Lda, A::AbsoluteX, O::Word(list + 2));
    abs(p, M::Sta, AT_Y);
    p.label("bdo_sort_inner");
    implied(p, M::Txa);
    br(p, M::Beq, "bdo_sort_place");
    emit(p, M::Lda, A::AbsoluteX, O::Word(list - 4));
    abs(p, M::Cmp, AT_X);
    br(p, M::Bcc, "bdo_sort_place");
    br(p, M::Beq, "bdo_sort_place");
    emit(p, M::Sta, A::AbsoluteX, O::Word(list));
    emit(p, M::Lda, A::AbsoluteX, O::Word(list - 2));
    emit(p, M::Sta, A::AbsoluteX, O::Word(list + 2));
    abs(p, M::Inc, Z_MOVES as u16);
    implied(p, M::Dex);
    implied(p, M::Dex);
    implied(p, M::Dex);
    implied(p, M::Dex);
    br(p, M::Bra, "bdo_sort_inner");
    p.label("bdo_sort_place");
    abs(p, M::Lda, AT_X);
    emit(p, M::Sta, A::AbsoluteX, O::Word(list));
    abs(p, M::Lda, AT_Y);
    emit(p, M::Sta, A::AbsoluteX, O::Word(list + 2));
    abs(p, M::Inc, INDEX);
    jmp(p, "bdo_sort_outer");
}
