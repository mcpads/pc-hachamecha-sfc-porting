use crate::{
    sfc::{emit, implied},
    sfc_object_pool::*,
};
use w65c816::{AddressingMode as A, Assembler, CpuMode, Mnemonic as M, Operand as O};
fn read_operand(p: &mut Assembler, output: u16, label: &str) {
    m16(p);
    iy(p, M::Lda, 0);
    abs(p, M::Sta, output);
    abs(p, M::Stz, output + 2);
    m8(p);
    abs(p, M::Lda, 0x426);
    byte(p, M::Cmp, 4);
    branch(p, M::Bne, label);
    m16(p);
    iy(p, M::Lda, 2);
    abs(p, M::Sta, output + 2);
    m8(p);
    p.label(label);
}
pub fn emit_routine(p: &mut Assembler) {
    p.label("object_arithmetic")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, 0x410);
    byte(p, M::Cmp, 17);
    branch(p, M::Beq, "object_word_width");
    byte(p, M::Lda, 4);
    branch(p, M::Bra, "object_arithmetic_width");
    p.label("object_word_width");
    byte(p, M::Lda, 2);
    p.label("object_arithmetic_width");
    abs(p, M::Sta, 0x426);
    abs(p, M::Sta, 0x425);
    abs(p, M::Lda, 0x411);
    byte(p, M::And, 15);
    abs(p, M::Sta, 0x427);
    byte(p, M::Cmp, 4);
    branch(p, M::Bcc, "object_arithmetic_operator_ok");
    byte(p, M::Cmp, 7);
    branch(p, M::Beq, "object_arithmetic_operator_ok");
    jump(p, "object_unsupported");
    p.label("object_arithmetic_operator_ok");
    abs(p, M::Lda, 0x411);
    for _ in 0..6 {
        emit(p, M::Lsr, A::Accumulator, O::None);
    }
    abs(p, M::Sta, 0x438);
    abs(p, M::Lda, 0x412);
    abs(p, M::Sta, 0x424);
    call(p, "object_operand_address");
    branch(p, M::Bcc, "object_left_address_ok");
    jump(p, "object_fault");
    p.label("object_left_address_ok");
    abs(p, M::Sty, 0x428);
    read_operand(p, 0x42c, "object_left_read");
    abs(p, M::Lda, 0x411);
    for _ in 0..4 {
        emit(p, M::Lsr, A::Accumulator, O::None);
    }
    byte(p, M::And, 3);
    abs(p, M::Sta, 0x438);
    branch(p, M::Beq, "object_right_immediate");
    abs(p, M::Lda, 0x413);
    abs(p, M::Sta, 0x424);
    call(p, "object_operand_address");
    branch(p, M::Bcc, "object_right_address_ok");
    jump(p, "object_fault");
    p.label("object_right_address_ok");
    read_operand(p, 0x430, "object_right_read");
    jump(p, "object_arithmetic_execute");
    p.label("object_right_immediate");
    m16(p);
    abs(p, M::Lda, 0x414);
    abs(p, M::Sta, 0x430);
    abs(p, M::Stz, 0x432);
    m8(p);
    abs(p, M::Lda, 0x426);
    byte(p, M::Cmp, 4);
    branch(p, M::Bne, "object_arithmetic_execute");
    m16(p);
    abs(p, M::Lda, 0x416);
    abs(p, M::Sta, 0x432);
    m8(p);
    p.label("object_arithmetic_execute");
    abs(p, M::Lda, 0x427);
    for (op, label) in [
        (0, "object_arithmetic_assign"),
        (1, "object_arithmetic_add"),
        (2, "object_arithmetic_sub"),
        (3, "object_arithmetic_mul"),
    ] {
        let next = format!("object_arithmetic_not_{op}");
        byte(p, M::Cmp, op);
        branch(p, M::Bne, &next);
        jump(p, label);
        p.label(next);
    }
    m16(p);
    for i in [0, 2] {
        abs(p, M::Lda, 0x42c + i);
        abs(p, M::Ora, 0x430 + i);
        abs(p, M::Sta, 0x42c + i);
    }
    jump(p, "object_arithmetic_store");
    p.label("object_arithmetic_assign")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    for i in [0, 2] {
        abs(p, M::Lda, 0x430 + i);
        abs(p, M::Sta, 0x42c + i);
    }
    jump(p, "object_arithmetic_store");
    for (label, carry, op) in [
        ("object_arithmetic_add", M::Clc, M::Adc),
        ("object_arithmetic_sub", M::Sec, M::Sbc),
    ] {
        p.label(label).assume_mode(CpuMode::NATIVE_M8_X16);
        m16(p);
        implied(p, carry);
        for i in [0, 2] {
            abs(p, M::Lda, 0x42c + i);
            abs(p, op, 0x430 + i);
            abs(p, M::Sta, 0x42c + i);
        }
        jump(p, "object_arithmetic_store");
    }
    p.label("object_arithmetic_mul")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    call(p, "object_multiply_u32");
    jump(p, "object_arithmetic_store");
    p.label("object_arithmetic_store")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    abs(p, M::Ldy, 0x428);
    abs(p, M::Lda, 0x42c);
    iy(p, M::Sta, 0);
    m8(p);
    abs(p, M::Lda, 0x426);
    byte(p, M::Cmp, 4);
    branch(p, M::Bne, "object_arithmetic_stored");
    m16(p);
    abs(p, M::Lda, 0x42e);
    iy(p, M::Sta, 2);
    m8(p);
    p.label("object_arithmetic_stored");
    jump(p, "object_advance");
    p.label("object_operand_address")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, 0x438);
    byte(p, M::Cmp, 2);
    branch(p, M::Beq, "object_operand_own");
    byte(p, M::Cmp, 3);
    branch(p, M::Bne, "object_operand_bad");
    jump(p, "object_reference_address");
    p.label("object_operand_bad");
    implied(p, M::Sec);
    implied(p, M::Rts);
    p.label("object_operand_own");
    m16(p);
    abs(p, M::Ldx, 0x408);
    word(p, M::Ldy, 230);
    jump(p, "object_reference_offset");
    p.label("object_multiply_u32")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    abs(p, M::Stz, 0x434);
    abs(p, M::Stz, 0x436);
    word(p, M::Ldx, 32);
    p.label("object_arithmetic_mul_bit");
    abs(p, M::Lsr, 0x432);
    abs(p, M::Ror, 0x430);
    branch(p, M::Bcc, "object_arithmetic_mul_shift");
    implied(p, M::Clc);
    for i in [0, 2] {
        abs(p, M::Lda, 0x434 + i);
        abs(p, M::Adc, 0x42c + i);
        abs(p, M::Sta, 0x434 + i);
    }
    p.label("object_arithmetic_mul_shift");
    abs(p, M::Asl, 0x42c);
    abs(p, M::Rol, 0x42e);
    implied(p, M::Dex);
    branch(p, M::Bne, "object_arithmetic_mul_bit");
    for i in [0, 2] {
        abs(p, M::Lda, 0x434 + i);
        abs(p, M::Sta, 0x42c + i);
    }
    implied(p, M::Rts);
}
