use super::*;
pub(super) fn emit(p: &mut w65c816::Assembler, tables: &Tables) -> Result<()> {
    use crate::sfc::emit;
    let Tables { program, .. } = *tables;
    p.label("field_target").assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    long_x(p, M::Lda, program + 2);
    emit(p, M::And, A::Immediate, O::Word(255));
    abs(p, M::Sta, TEMP);
    m8(p);
    long_x(p, M::Lda, program + 1);
    emit(p, M::And, A::Immediate, O::Byte(0xc0));
    emit(p, M::Cmp, A::Immediate, O::Byte(0x40));
    branch(p, M::Bne, "field_target_reference");
    m16(p);
    abs(p, M::Lda, TEMP);
    emit(p, M::Asl, A::Accumulator, O::None);
    emit(p, M::Asl, A::Accumulator, O::None);
    emit(p, M::Adc, A::Immediate, O::Word(GLOBALS));
    branch(p, M::Bra, "field_target_done");
    p.label("field_target_reference")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, REFERENCE);
    emit(p, M::Cmp, A::Immediate, O::Byte(3));
    branch(p, M::Beq, "field_target_party");
    emit(p, M::Cmp, A::Immediate, O::Byte(1));
    m16(p);
    branch(p, M::Beq, "field_target_game");
    branch(p, M::Bcs, "field_target_picture");
    emit(p, M::Lda, A::Immediate, O::Word(INPUT));
    branch(p, M::Bra, "field_target_add");
    p.label("field_target_game");
    abs(p, M::Lda, TEMP);
    emit(p, M::Cmp, A::Immediate, O::Word(8));
    emit(p, M::Lda, A::Immediate, O::Word(GAME));
    branch(p, M::Bcc, "field_target_add");
    emit(
        p,
        M::Lda,
        A::Immediate,
        O::Word(crate::sfc_b1_menu::INVENTORY - 8),
    );
    branch(p, M::Bra, "field_target_add");
    p.label("field_target_picture");
    emit(p, M::Lda, A::Immediate, O::Word(PICTURE));
    branch(p, M::Bra, "field_target_add");
    p.label("field_target_party")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    emit(p, M::Lda, A::Immediate, O::Word(crate::sfc_battle::STATS));
    p.label("field_target_add");
    implied(p, M::Clc);
    abs(p, M::Adc, TEMP);
    p.label("field_target_done");
    implied(p, M::Tay);
    m8(p);
    implied(p, M::Rts);
    for (op, width, value, length) in [(0x10u8, 1u32, 3u32, 4u16), (0x11, 2, 4, 8), (0x12, 4, 4, 8)]
    {
        p.label(format!("field_op_{op:02x}"));
        call(p, "field_target");
        long_x(p, M::Lda, program + 1);
        emit(p, M::And, A::Immediate, O::Byte(15));
        branch(p, M::Bne, &format!("field_not_set_{op:02x}"));
        jump(p, &format!("field_set_{op:02x}"));
        p.label(format!("field_not_set_{op:02x}"));
        emit(p, M::Cmp, A::Immediate, O::Byte(1));
        branch(p, M::Beq, &format!("field_add_{op:02x}"));
        implied(p, M::Sec);
        for i in 0..width {
            emit(p, M::Lda, A::AbsoluteY, O::Word(0));
            long_x(p, M::Sbc, program + value + i);
            emit(p, M::Sta, A::AbsoluteY, O::Word(0));
            implied(p, M::Iny);
        }
        jump(p, &format!("field_next{length}"));
        p.label(format!("field_add_{op:02x}"));
        implied(p, M::Clc);
        for i in 0..width {
            emit(p, M::Lda, A::AbsoluteY, O::Word(0));
            long_x(p, M::Adc, program + value + i);
            emit(p, M::Sta, A::AbsoluteY, O::Word(0));
            implied(p, M::Iny);
        }
        if op == 0x12 {
            abs(p, M::Lda, REFERENCE);
            emit(p, M::Cmp, A::Immediate, O::Byte(2));
            branch(p, M::Bne, "field_slide_kept");
            for at in [PICTURE_X, PICTURE_X + 1, PICTURE_Y, PICTURE_Y + 1] {
                abs(p, M::Lda, at);
                branch(p, M::Bne, "field_slide_kept");
            }
            abs(p, M::Lda, PICTURE_X + 2);
            abs(p, M::Ora, PICTURE_Y + 2);
            emit(p, M::And, A::Immediate, O::Byte(SLIDE_PIXELS - 1));
            branch(p, M::Bne, "field_slide_kept");
            call(p, "field_picture_view");
            p.label("field_slide_kept");
        }
        jump(p, &format!("field_next{length}"));
        p.label(format!("field_set_{op:02x}"));
        for i in 0..width {
            long_x(p, M::Lda, program + value + i);
            emit(p, M::Sta, A::AbsoluteY, O::Word(0));
            implied(p, M::Iny);
        }
        jump(p, &format!("field_next{length}"));
    }
    for (op, width, value, branch_at, length) in [
        (0x13u8, 1u16, 3u32, 4u16, 8u16),
        (0x14, 2, 4, 8, 12),
        (0x15, 4, 4, 8, 12),
    ] {
        p.label(format!("field_op_{op:02x}"));
        call(p, "field_target");
        if op == 0x13 {
            long_x(p, M::Lda, program + 1);
            emit(p, M::And, A::Immediate, O::Byte(15));
            emit(p, M::Cmp, A::Immediate, O::Byte(2));
            branch(p, M::Bcc, "field_equality_13");
            jump(p, "field_order_13");
            p.label("field_equality_13");
        }
        for i in 0..width {
            emit(p, M::Lda, A::AbsoluteY, O::Word(i));
            long_x(p, M::Cmp, program + value + u32::from(i));
            branch(p, M::Bne, &format!("field_differ_{op:02x}"));
        }
        if width < 4 {
            long_x(p, M::Lda, program + 1);
            emit(p, M::And, A::Immediate, O::Byte(0xc0));
            emit(p, M::Cmp, A::Immediate, O::Byte(0x40));
            branch(p, M::Bne, &format!("field_equal_{op:02x}"));
            emit(p, M::Lda, A::AbsoluteY, O::Word(width));
            for i in width + 1..4 {
                emit(p, M::Ora, A::AbsoluteY, O::Word(i));
            }
            branch(p, M::Bne, &format!("field_differ_{op:02x}"));
        }
        p.label(format!("field_equal_{op:02x}"));
        long_x(p, M::Lda, program + 1);
        emit(p, M::And, A::Immediate, O::Byte(15));
        branch(p, M::Bne, &format!("field_compare_taken_{op:02x}"));
        jump(p, &format!("field_next{length}"));
        p.label(format!("field_differ_{op:02x}"));
        long_x(p, M::Lda, program + 1);
        emit(p, M::And, A::Immediate, O::Byte(15));
        branch(p, M::Beq, &format!("field_compare_taken_{op:02x}"));
        jump(p, &format!("field_next{length}"));
        p.label(format!("field_compare_taken_{op:02x}"));
        jump(p, &format!("field_goto{branch_at}"));
    }
    p.label("field_order_13")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Sta, MASK);
    emit(p, M::Lda, A::AbsoluteY, O::Word(0));
    long_x(p, M::Cmp, program + 3);
    branch(p, M::Beq, "field_order_equal");
    branch(p, M::Bcc, "field_order_less");
    emit(p, M::Lda, A::Immediate, O::Byte(4));
    branch(p, M::Bra, "field_order_outcome");
    p.label("field_order_equal");
    emit(p, M::Lda, A::Immediate, O::Byte(2));
    branch(p, M::Bra, "field_order_outcome");
    p.label("field_order_less");
    emit(p, M::Lda, A::Immediate, O::Byte(1));
    p.label("field_order_outcome");
    abs(p, M::Sta, TEMP);
    for (sub, set) in [(2u8, 4u8), (3, 1), (4, 6), (5, 3)] {
        let next = format!("field_order_not_{sub}");
        abs(p, M::Lda, MASK);
        emit(p, M::Cmp, A::Immediate, O::Byte(sub));
        branch(p, M::Bne, &next);
        emit(p, M::Lda, A::Immediate, O::Byte(set));
        branch(p, M::Bra, "field_order_set");
        p.label(next);
    }
    p.label("field_order_set");
    abs(p, M::And, TEMP);
    branch(p, M::Beq, "field_order_not_taken");
    jump(p, "field_goto4");
    p.label("field_order_not_taken");
    jump(p, "field_next8");
    for (op, value) in [(0x4bu8, 0u8), (0x6f, 1), (0x4c, 3)] {
        p.label(format!("field_op_{op:02x}"));
        write(p, REFERENCE, value);
        jump(p, "field_next4");
    }
    Ok(())
}
