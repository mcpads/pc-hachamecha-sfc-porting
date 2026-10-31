use crate::{
    sfc::{emit, implied},
    sfc_object_pool::*,
};
use w65c816::{AddressingMode as A, Assembler, CpuMode, Mnemonic as M, Operand as O};
pub fn emit_routine(p: &mut Assembler, table: u32) {
    p.label("object_motion").assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, 0x411);
    byte(p, M::And, 0xf0);
    branch(p, M::Beq, "object_motion_mode_ok");
    jump(p, "object_unsupported");
    p.label("object_motion_mode_ok");
    for axis in 0..3u16 {
        let label = |suffix: &str| format!("object_motion_{axis}_{suffix}");
        abs(p, M::Lda, 0x411);
        byte(p, M::And, 1 << axis);
        branch(p, M::Bne, &label("active"));
        jump(p, &label("next"));
        p.label(label("active"));
        m16(p);
        abs(p, M::Ldx, 0x408);
        ix(p, M::Lda, 0x8c + axis * 2);
        implied(p, M::Clc);
        word(p, M::Adc, 8);
        for _ in 0..4 {
            emit(p, M::Lsr, A::Accumulator, O::None);
        }
        if axis != 0 {
            implied(p, M::Clc);
            word(p, M::Adc, 1024);
            word(p, M::And, 4095);
        }
        abs(p, M::Sta, 0x440);
        word(p, M::And, 1023);
        abs(p, M::Sta, 0x442);
        abs(p, M::Lda, 0x440);
        word(p, M::And, 1024);
        branch(p, M::Beq, &label("table"));
        abs(p, M::Lda, 0x442);
        branch(p, M::Beq, &label("unity"));
        word(p, M::Lda, 1024);
        implied(p, M::Sec);
        abs(p, M::Sbc, 0x442);
        abs(p, M::Sta, 0x442);
        p.label(label("table"));
        abs(p, M::Lda, 0x442);
        emit(p, M::Asl, A::Accumulator, O::None);
        implied(p, M::Tax);
        emit(p, M::Lda, A::AbsoluteLongX, O::Long(table));
        abs(p, M::Sta, 0x42c);
        abs(p, M::Stz, 0x42e);
        branch(p, M::Bra, &label("signed"));
        p.label(label("unity"));
        abs(p, M::Stz, 0x42c);
        word(p, M::Lda, 1);
        abs(p, M::Sta, 0x42e);
        p.label(label("signed"));
        abs(p, M::Lda, 0x440);
        word(p, M::And, 2048);
        branch(p, M::Beq, &label("amplitude"));
        abs(p, M::Lda, 0x42c);
        word(p, M::Eor, 0xffff);
        implied(p, M::Clc);
        word(p, M::Adc, 1);
        abs(p, M::Sta, 0x42c);
        abs(p, M::Lda, 0x42e);
        word(p, M::Eor, 0xffff);
        word(p, M::Adc, 0);
        abs(p, M::Sta, 0x42e);
        p.label(label("amplitude"));
        abs(p, M::Ldx, 0x408);
        ix(p, M::Lda, 0x92 + axis * 2);
        abs(p, M::Sta, 0x430);
        branch(p, M::Bpl, &label("positive"));
        word(p, M::Lda, 0xffff);
        abs(p, M::Sta, 0x432);
        branch(p, M::Bra, &label("multiply"));
        p.label(label("positive"));
        abs(p, M::Stz, 0x432);
        p.label(label("multiply"));
        call(p, "object_multiply_u32");
        abs(p, M::Ldx, 0x408);
        abs(p, M::Lda, 0x411);
        word(p, M::And, 8);
        branch(p, M::Beq, &label("relative"));
        for i in [0, 2] {
            ix(p, M::Lda, 0x80 + axis * 4 + i);
            abs(p, M::Sta, 0x444 + i);
        }
        branch(p, M::Bra, &label("apply"));
        p.label(label("relative"));
        for i in [0, 2] {
            ix(p, M::Lda, 0x1c + axis * 4 + i);
            abs(p, M::Sta, 0x444 + i);
        }
        p.label(label("apply"));
        implied(p, if axis == 1 { M::Sec } else { M::Clc });
        for i in [0, 2] {
            abs(p, M::Lda, 0x444 + i);
            abs(p, if axis == 1 { M::Sbc } else { M::Adc }, 0x42c + i);
            ix(p, M::Sta, 0x1c + axis * 4 + i);
        }
        m8(p);
        p.label(label("next")).assume_mode(CpuMode::NATIVE_M8_X16);
    }
    jump(p, "object_advance");
}
