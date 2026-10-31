use super::*;
use crate::sfc::{emit, implied};
use w65c816::{AddressingMode as A, CpuMode, Mnemonic as M, Operand as O};

pub(crate) fn emit_compact(p: &mut w65c816::Assembler) {
    let abs = |p: &mut w65c816::Assembler, m, at: u16| emit(p, m, A::Absolute, O::Word(at));
    let branch = |p: &mut w65c816::Assembler, m, label: &str| {
        p.emit_label_ref(m, A::Relative8, label);
    };
    let m16 = |p: &mut w65c816::Assembler| emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    let m8 = |p: &mut w65c816::Assembler| emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    p.label("b1_menu_compact")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    emit(p, M::Ldx, A::Immediate, O::Word(0));
    emit(p, M::Ldy, A::Immediate, O::Word(0));
    p.label("b1_menu_pack");
    emit(p, M::Lda, A::AbsoluteX, O::Word(OAM_BUFFER + 1));
    emit(p, M::And, A::Immediate, O::Word(255));
    emit(p, M::Cmp, A::Immediate, O::Word(224));
    branch(p, M::Bcs, "b1_menu_pack_next");
    for offset in [0, 2] {
        emit(p, M::Lda, A::AbsoluteX, O::Word(OAM_BUFFER + offset));
        emit(p, M::Sta, A::AbsoluteY, O::Word(OAM_BUFFER + offset));
    }
    for _ in 0..4 {
        implied(p, M::Iny);
    }
    p.label("b1_menu_pack_next");
    for _ in 0..4 {
        implied(p, M::Inx);
    }
    emit(p, M::Cpx, A::Immediate, O::Word((MENU_SPRITES * 4) as u16));
    branch(p, M::Bcc, "b1_menu_pack");
    implied(p, M::Tya);
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Word(15));
    emit(p, M::And, A::Immediate, O::Word(0xfff0));
    abs(p, M::Sta, OAM_USED);
    m8(p);
    p.label("b1_menu_pack_hide");
    emit(p, M::Cpy, A::Immediate, O::Word((MENU_SPRITES * 4) as u16));
    branch(p, M::Bcs, "b1_menu_pack_hidden");
    emit(p, M::Lda, A::Immediate, O::Byte(224));
    emit(p, M::Sta, A::AbsoluteY, O::Word(OAM_BUFFER + 1));
    for _ in 0..4 {
        implied(p, M::Iny);
    }
    branch(p, M::Bra, "b1_menu_pack_hide");
    p.label("b1_menu_pack_hidden");
    implied(p, M::Rts);
}
