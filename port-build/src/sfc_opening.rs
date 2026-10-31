use crate::{
    sfc::{implied, write},
    sfc_object_pool::*,
};
use w65c816::{Assembler, CpuMode, Mnemonic as M};
pub fn reset(p: &mut Assembler) {
    write(p, 0x480, 0);
    write(p, 0x481, 16);
    write(p, 0x310, 2);
    for (display, slot) in [(0, 0), (1, 1), (2, 2)] {
        write(p, 0x320 + display, slot);
    }
    write(p, 0x331, 2);
    write(p, 0x341, 1);
    write(p, 0x330, 1);
    write(p, 0x340, 1);
    write(p, 0x332, 0);
    write(p, 0x316, 1);
}
pub fn emit_routine(p: &mut Assembler, pressed: u16) {
    p.label("opening_tick").assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, 0x480);
    for (phase, label) in [
        (0, "opening_delay"),
        (1, "opening_first"),
        (2, "opening_move"),
        (3, "opening_second"),
    ] {
        byte(p, M::Cmp, phase);
        branch(p, M::Bne, &format!("opening_not_{phase}"));
        jump(p, label);
        p.label(format!("opening_not_{phase}"));
    }
    abs(p, M::Lda, pressed + 1);
    byte(p, M::And, 0x10);
    branch(p, M::Beq, "opening_done");
    write(p, 0x333, 0);
    write(p, 0x343, 0);
    for at in [0x4008, 0x4009, 0x4068, 0x4069, 0x40c8, 0x40c9, 0x5008] {
        crate::sfc::emit(
            p,
            M::Lda,
            w65c816::AddressingMode::Immediate,
            w65c816::Operand::Byte(0),
        );
        crate::sfc::emit(
            p,
            M::Sta,
            w65c816::AddressingMode::AbsoluteLong,
            w65c816::Operand::Long(0x7e0000 + at),
        );
    }
    reset(p);
    p.label("opening_done");
    implied(p, M::Rts);
    p.label("opening_delay");
    abs(p, M::Dec, 0x481);
    branch(p, M::Bne, "opening_done");
    write(p, 0x20a, 1);
    write(p, 0x480, 1);
    implied(p, M::Rts);
    p.label("opening_first");
    abs(p, M::Lda, 0x20d);
    byte(p, M::Cmp, 1);
    branch(p, M::Bne, "opening_done");
    write(p, 0x332, 1);
    for (at, value) in [
        (0x5000, 0u16),
        (0x5002, (-256i16) as u16),
        (0x5004, 0),
        (0x5006, 144),
        (0x4004, 320),
    ] {
        for (i, b) in value.to_le_bytes().into_iter().enumerate() {
            crate::sfc::emit(
                p,
                M::Lda,
                w65c816::AddressingMode::Immediate,
                w65c816::Operand::Byte(b),
            );
            crate::sfc::emit(
                p,
                M::Sta,
                w65c816::AddressingMode::AbsoluteLong,
                w65c816::Operand::Long(0x7e0000 + at + i as u32),
            );
        }
    }
    for (at, value) in [(0x5008, 1), (0x4006, 0), (0x4008, 1)] {
        crate::sfc::emit(
            p,
            M::Lda,
            w65c816::AddressingMode::Immediate,
            w65c816::Operand::Byte(value),
        );
        crate::sfc::emit(
            p,
            M::Sta,
            w65c816::AddressingMode::AbsoluteLong,
            w65c816::Operand::Long(0x7e0000 + at),
        );
    }
    write(p, 0x481, 33);
    write(p, 0x480, 2);
    implied(p, M::Rts);
    p.label("opening_move");
    abs(p, M::Dec, 0x481);
    branch(p, M::Beq, "opening_request_second");
    implied(p, M::Rts);
    p.label("opening_request_second");
    write(p, 0x20a, 2);
    write(p, 0x480, 3);
    implied(p, M::Rts);
    p.label("opening_second");
    abs(p, M::Lda, 0x20d);
    byte(p, M::Cmp, 2);
    branch(p, M::Bne, "opening_return");
    write(p, 0x480, 4);
    p.label("opening_return");
    implied(p, M::Rts);
}
