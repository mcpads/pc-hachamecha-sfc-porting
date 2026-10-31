use super::*;
pub(super) fn emit(p: &mut w65c816::Assembler, tables: &Tables) -> Result<()> {
    use crate::sfc::emit;
    let Tables {
        events,
        floors,
        lookup,
        counts,
        starts,
        cells,
        masks,
        ..
    } = *tables;
    p.label("field_reset").assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    emit(p, M::Ldx, A::Immediate, O::Word(0));
    p.label("field_reset_clear");
    emit(p, M::Stz, A::AbsoluteX, O::Word(GLOBALS));
    implied(p, M::Inx);
    implied(p, M::Inx);
    emit(p, M::Cpx, A::Immediate, O::Word(END - GLOBALS));
    branch(p, M::Bne, "field_reset_clear");
    emit(p, M::Lda, A::Immediate, O::Word(0xffff));
    abs(p, M::Sta, PC);
    abs(p, M::Sta, SELECTED);
    abs(p, M::Sta, LOOP_TARGET);
    for (i, half) in [START_GOLD as u16, (START_GOLD >> 16) as u16]
        .into_iter()
        .enumerate()
    {
        emit(p, M::Lda, A::Immediate, O::Word(half));
        abs(p, M::Sta, GAME + 4 + 2 * i as u16);
    }
    m8(p);
    write(p, EVENT, 255);
    for at in LIVE..KEY {
        abs(p, M::Stz, at);
    }
    for at in [
        DEPTH,
        VIEW_DIRTY,
        WAIT,
        WAIT + 1,
        PICTURE_X,
        PICTURE_X + 1,
        PICTURE_X + 2,
        PICTURE_X + 3,
        PICTURE_Y,
        PICTURE_Y + 1,
        PICTURE_Y + 2,
        PICTURE_Y + 3,
    ] {
        abs(p, M::Stz, at);
    }
    p.label("field_floor_bounds");
    m16(p);
    abs(p, M::Lda, 0x50c);
    emit(p, M::And, A::Immediate, O::Word(255));
    for _ in 0..3 {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    implied(p, M::Tax);
    long_x(p, M::Lda, floors);
    abs(p, M::Sta, EVENTS_FROM);
    long_x(p, M::Lda, floors + 2);
    abs(p, M::Sta, EVENTS_TO);
    m8(p);
    implied(p, M::Rts);
    for (label, offset) in [("field_floor_exit", 6), ("field_floor_entry", 4)] {
        p.label(label).assume_mode(CpuMode::NATIVE_M8_X16);
        m16(p);
        abs(p, M::Lda, 0x50c);
        emit(p, M::And, A::Immediate, O::Word(255));
        for _ in 0..3 {
            emit(p, M::Asl, A::Accumulator, O::None);
        }
        implied(p, M::Tax);
        long_x(p, M::Lda, floors + offset);
        emit(p, M::Cmp, A::Immediate, O::Word(0xffff));
        branch(p, M::Beq, &format!("{label}_none"));
        abs(p, M::Sta, PC);
        m8(p);
        if label == "field_floor_exit" {
            jump(p, "field_run_script");
            p.label(format!("{label}_none"))
                .assume_mode(CpuMode::NATIVE_M16_X16);
            m8(p);
            implied(p, M::Rts);
            continue;
        }
        call(p, "field_run_script");
        abs(p, M::Lda, STEPS);
        implied(p, M::Sec);
        emit(p, M::Sbc, A::Immediate, O::Byte(SCRIPT_SLACK));
        branch(p, M::Bcs, &format!("{label}_budget"));
        emit(p, M::Lda, A::Immediate, O::Byte(1));
        p.label(format!("{label}_budget"));
        abs(p, M::Sta, STEPS);
        m16(p);
        emit(p, M::Lda, A::Immediate, O::Word(0xffff));
        p.label(format!("{label}_none"))
            .assume_mode(CpuMode::NATIVE_M16_X16);
        abs(p, M::Sta, PC);
        m8(p);
        implied(p, M::Rts);
    }
    p.label("field_find").assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    abs(p, M::Lda, 0x500);
    emit(p, M::And, A::Immediate, O::Word(3));
    implied(p, M::Tax);
    m8(p);
    long_x(p, M::Lda, masks);
    abs(p, M::Sta, MASK);
    abs(p, M::Lda, 0x500);
    emit(p, M::Lsr, A::Accumulator, O::None);
    emit(p, M::Lsr, A::Accumulator, O::None);
    emit(p, M::And, A::Immediate, O::Byte(7));
    abs(p, M::Sta, TEMP);
    abs(p, M::Lda, 0x500);
    for _ in 0..5 {
        emit(p, M::Lsr, A::Accumulator, O::None);
    }
    abs(p, M::Sta, TEMP + 1);
    implied(p, M::Phy);
    m16(p);
    abs(p, M::Lda, 0x50c);
    emit(p, M::And, A::Immediate, O::Word(0xff));
    for _ in 0..3 {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    m8(p);
    abs(p, M::Ora, TEMP + 1);
    implied(p, M::Tax);
    long_x(p, M::Lda, lookup);
    implied(p, M::Pha);
    long_x(p, M::Lda, lookup + counts);
    implied(p, M::Tay);
    m16(p);
    abs(p, M::Lda, TEMP);
    emit(p, M::And, A::Immediate, O::Word(0xff));
    implied(p, M::Tax);
    m8(p);
    implied(p, M::Pla);
    p.label("field_find_left");
    emit(p, M::Cpx, A::Immediate, O::Word(0));
    branch(p, M::Beq, "field_find_cell");
    emit(p, M::Lsr, A::Accumulator, O::None);
    branch(p, M::Bcc, "field_find_left_next");
    implied(p, M::Iny);
    p.label("field_find_left_next");
    implied(p, M::Dex);
    branch(p, M::Bra, "field_find_left");
    p.label("field_find_cell");
    emit(p, M::Lsr, A::Accumulator, O::None);
    branch(p, M::Bcc, "field_find_none_y");
    m16(p);
    abs(p, M::Lda, 0x50c);
    emit(p, M::And, A::Immediate, O::Word(0xff));
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Tax);
    implied(p, M::Tya);
    implied(p, M::Clc);
    long_x(p, M::Adc, lookup + starts);
    implied(p, M::Tax);
    m8(p);
    long_x(p, M::Lda, lookup + cells);
    implied(p, M::Ply);
    implied(p, M::Pha);
    m16(p);
    emit(p, M::And, A::Immediate, O::Word(0x7f));
    for _ in 0..3 {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    implied(p, M::Clc);
    abs(p, M::Adc, EVENTS_FROM);
    implied(p, M::Tax);
    m8(p);
    implied(p, M::Pla);
    implied(p, M::Pha);
    long_x(p, M::Lda, events + 2);
    abs(p, M::And, MASK);
    branch(p, M::Bne, "field_find_first");
    implied(p, M::Pla);
    branch(p, M::Bpl, "field_find_none");
    m16(p);
    implied(p, M::Txa);
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Word(RECORD as u16));
    implied(p, M::Tax);
    m8(p);
    p.label("field_find_record");
    abs(p, M::Cpx, EVENTS_TO);
    branch(p, M::Beq, "field_find_none");
    long_x(p, M::Lda, events);
    abs(p, M::Cmp, TEMP);
    branch(p, M::Bne, "field_find_next");
    long_x(p, M::Lda, events + 1);
    abs(p, M::Cmp, TEMP + 1);
    branch(p, M::Bne, "field_find_next");
    long_x(p, M::Lda, events + 2);
    abs(p, M::And, MASK);
    branch(p, M::Bne, "field_find_found");
    p.label("field_find_next");
    m16(p);
    implied(p, M::Txa);
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Word(RECORD as u16));
    implied(p, M::Tax);
    m8(p);
    branch(p, M::Bra, "field_find_record");
    p.label("field_find_first");
    implied(p, M::Pla);
    p.label("field_find_found");
    implied(p, M::Clc);
    implied(p, M::Rts);
    p.label("field_find_none_y");
    implied(p, M::Ply);
    p.label("field_find_none");
    abs(p, M::Ldx, EVENTS_TO);
    implied(p, M::Sec);
    implied(p, M::Rts);
    p.label("field_select").assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Stz, KIND);
    call(p, "field_find");
    branch(p, M::Bcc, "field_select_found");
    write(p, EVENT, 255);
    p.label("field_select_none");
    m16(p);
    emit(p, M::Lda, A::Immediate, O::Word(0xffff));
    abs(p, M::Sta, SELECTED);
    m8(p);
    abs(p, M::Stz, CONTEXT);
    implied(p, M::Rts);
    p.label("field_select_found");
    long_x(p, M::Lda, events + 3);
    abs(p, M::Sta, KIND);
    long_x(p, M::Lda, events + 4);
    abs(p, M::Cmp, EVENT);
    branch(p, M::Beq, "field_select_same");
    abs(p, M::Sta, EVENT);
    m16(p);
    long_x(p, M::Lda, events + 5);
    abs(p, M::Sta, SELECTED);
    m8(p);
    abs(p, M::Lda, 0x500);
    emit(p, M::And, A::Immediate, O::Byte(3));
    abs(p, M::Sta, ORIGIN);
    abs(p, M::Stz, CONTEXT);
    implied(p, M::Rts);
    p.label("field_select_same");
    long_x(p, M::Lda, events + 2);
    emit(p, M::Cmp, A::Immediate, O::Byte(15));
    branch(p, M::Bne, "field_select_kind");
    abs(p, M::Lda, 0x500);
    emit(p, M::And, A::Immediate, O::Byte(3));
    abs(p, M::Cmp, ORIGIN);
    branch(p, M::Beq, "field_select_kind");
    abs(p, M::Stz, KIND);
    branch(p, M::Bra, "field_select_none");
    p.label("field_select_kind");
    abs(p, M::Lda, KIND);
    emit(p, M::Cmp, A::Immediate, O::Byte(2));
    branch(p, M::Bcc, "field_select_done");
    abs(p, M::Stz, KIND);
    p.label("field_select_done");
    implied(p, M::Rts);
    Ok(())
}
