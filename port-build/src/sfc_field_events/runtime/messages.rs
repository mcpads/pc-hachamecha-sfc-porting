use super::*;
pub(super) fn emit(p: &mut w65c816::Assembler, tables: &Tables) -> Result<()> {
    use crate::sfc::emit;
    let Tables {
        program,
        messages,
        bits,
        ..
    } = *tables;
    p.label("field_op_2f");
    m16(p);
    long_x(p, M::Lda, program + 4);
    for _ in 0..3 {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    implied(p, M::Tax);
    m8(p);
    call(p, "field_show");
    jump(p, "field_yield8");
    p.label("field_show").assume_mode(CpuMode::NATIVE_M8_X16);
    long_x(p, M::Lda, messages + 4);
    emit(p, M::Cmp, A::Immediate, O::Byte(MESSAGE_DISPATCH));
    branch(p, M::Bne, "field_show_entry");
    m16(p);
    long_x(p, M::Lda, messages + 5);
    emit(p, M::And, A::Immediate, O::Word(255));
    emit(p, M::Asl, A::Accumulator, O::None);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Tay);
    m8(p);
    emit(p, M::Lda, A::AbsoluteY, O::Word(GLOBALS));
    long_x(p, M::Cmp, messages + 2);
    branch(p, M::Bcc, "field_show_alternative");
    long_x(p, M::Lda, messages + 2);
    emit(p, M::Dec, A::Accumulator, O::None);
    p.label("field_show_alternative");
    m16(p);
    emit(p, M::And, A::Immediate, O::Word(255));
    implied(p, M::Clc);
    long_x(p, M::Adc, messages);
    for _ in 0..3 {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    implied(p, M::Tax);
    m8(p);
    branch(p, M::Bra, "field_show");
    p.label("field_show_entry");
    m16(p);
    long_x(p, M::Lda, messages);
    abs(p, M::Sta, 0x501);
    long_x(p, M::Lda, messages + 2);
    abs(p, M::Sta, 0x50d);
    m8(p);
    long_x(p, M::Lda, messages + 4);
    abs(p, M::Sta, MODE);
    call(p, "b1_page");
    abs(p, M::Lda, MODE);
    emit(p, M::Cmp, A::Immediate, O::Byte(MESSAGE_NOTICE));
    branch(p, M::Bne, "field_show_waits");
    crate::sfc::write_word(p, 0x501, 2);
    implied(p, M::Rts);
    p.label("field_show_waits");
    emit(p, M::Cmp, A::Immediate, O::Byte(MESSAGE_QUESTION));
    branch(p, M::Bne, "field_show_done");
    write(p, 0x511, 1);
    p.label("field_show_done");
    implied(p, M::Rts);
    p.label("field_flag").assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    long_x(p, M::Lda, program + 2);
    abs(p, M::Sta, TEMP);
    emit(p, M::Lsr, A::Accumulator, O::None);
    emit(p, M::Lsr, A::Accumulator, O::None);
    emit(p, M::Lsr, A::Accumulator, O::None);
    implied(p, M::Tay);
    abs(p, M::Lda, TEMP);
    emit(p, M::And, A::Immediate, O::Word(7));
    implied(p, M::Phx);
    implied(p, M::Tax);
    m8(p);
    long_x(p, M::Lda, bits);
    implied(p, M::Plx);
    implied(p, M::Rts);
    p.label("field_op_31");
    call(p, "field_flag");
    abs(p, M::Sta, MASK);
    long_x(p, M::Lda, program + 1);
    branch(p, M::Beq, "field_flag_clear");
    emit(p, M::Lda, A::AbsoluteY, O::Word(GLOBALS));
    abs(p, M::Ora, MASK);
    branch(p, M::Bra, "field_flag_store");
    p.label("field_flag_clear");
    abs(p, M::Lda, MASK);
    emit(p, M::Eor, A::Immediate, O::Byte(0xff));
    emit(p, M::And, A::AbsoluteY, O::Word(GLOBALS));
    p.label("field_flag_store");
    emit(p, M::Sta, A::AbsoluteY, O::Word(GLOBALS));
    jump(p, "field_next4");
    p.label("field_op_32");
    call(p, "field_flag");
    emit(p, M::And, A::AbsoluteY, O::Word(GLOBALS));
    branch(p, M::Beq, "field_test_clear");
    long_x(p, M::Lda, program + 1);
    branch(p, M::Bne, "field_test_taken");
    jump(p, "field_next8");
    p.label("field_test_clear");
    long_x(p, M::Lda, program + 1);
    branch(p, M::Beq, "field_test_taken");
    jump(p, "field_next8");
    p.label("field_test_taken");
    jump(p, "field_goto4");
    Ok(())
}
