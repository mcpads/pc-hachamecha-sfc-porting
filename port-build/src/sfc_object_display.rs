use crate::{
    sfc::{emit, implied, write},
    sfc_object_pool::*,
};
use w65c816::{AddressingMode as A, Assembler, CpuMode, Mnemonic as M, Operand as O};
pub fn reset(p: &mut Assembler, displays: usize) {
    for at in 0x450..0x454 {
        write(p, at, 0);
    }
    for i in 0..displays {
        write(p, 0x460 + i as u16, 255);
    }
}
fn long(p: &mut Assembler, m: M, at: u32) {
    emit(p, m, A::AbsoluteLongX, O::Long(at));
}
fn index(p: &mut Assembler, binding: u16, label: &str) {
    m16(p);
    abs(p, M::Lda, binding);
    word(p, M::And, 255);
    implied(p, M::Tay);
    word(p, M::Lda, 0);
    p.label(format!("{label}_multiply"));
    word(p, M::Cpy, 0);
    branch(p, M::Beq, &format!("{label}_indexed"));
    implied(p, M::Clc);
    word(p, M::Adc, 230);
    implied(p, M::Dey);
    branch(p, M::Bra, &format!("{label}_multiply"));
    p.label(format!("{label}_indexed"));
    abs(p, M::Sta, 0x452);
    implied(p, M::Tax);
    m8(p);
}
pub fn emit_routine(p: &mut Assembler, displays: usize) {
    p.label("object_display_tick")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, 0x450);
    branch(p, M::Bne, "object_display_enabled");
    implied(p, M::Rts);
    p.label("object_display_enabled");
    abs(p, M::Lda, 0x230);
    abs(p, M::Ora, 0x404);
    branch(p, M::Beq, "object_display_validate");
    implied(p, M::Rts);
    p.label("object_display_validate");
    for i in 0..displays {
        let next = format!("object_display_valid_{i}");
        abs(p, M::Lda, 0x460 + i as u16);
        byte(p, M::Cmp, 255);
        branch(p, M::Beq, &next);
        byte(p, M::Cmp, 16);
        branch(p, M::Bcc, &format!("{next}_slot"));
        write(p, 0x451, 1);
        implied(p, M::Rts);
        p.label(format!("{next}_slot"));
        index(p, 0x460 + i as u16, &next);
        long(p, M::Lda, 0x7e6003);
        byte(p, M::And, 0x60);
        byte(p, M::Cmp, 0x20);
        branch(p, M::Bne, &next);
        write(p, 0x451, 2);
        implied(p, M::Rts);
        p.label(next);
    }
    for i in 0..displays {
        let base = format!("object_display_{i}");
        abs(p, M::Lda, 0x460 + i as u16);
        byte(p, M::Cmp, 255);
        branch(p, M::Beq, &format!("{base}_hidden"));
        m16(p);
        word(p, M::And, 255);
        implied(p, M::Tax);
        m8(p);
        long(p, M::Lda, 0x7e03d0);
        branch(p, M::Beq, &format!("{base}_hidden"));
        index(p, 0x460 + i as u16, &base);
        long(p, M::Lda, 0x7e6003);
        byte(p, M::And, 0x40);
        branch(p, M::Bne, &format!("{base}_visible"));
        p.label(format!("{base}_hidden"));
        m16(p);
        word(p, M::Lda, (-1024i16) as u16);
        abs(p, M::Sta, 0x240 + i as u16 * 4);
        abs(p, M::Sta, 0x242 + i as u16 * 4);
        m8(p);
        jump(p, &format!("{base}_done"));
        p.label(format!("{base}_visible"));
        for (axis, routine) in [(0, "motion_project"), (1, "motion_project_y")] {
            m16(p);
            abs(p, M::Ldx, 0x452);
            long(p, M::Lda, 0x7e601c + axis * 4);
            abs(p, M::Sta, 0x2a0);
            long(p, M::Lda, 0x7e601e + axis * 4);
            abs(p, M::Sta, 0x2a2);
            call(p, routine);
            abs(p, M::Lda, 0x2e0);
            abs(p, M::Sta, 0x240 + i as u16 * 4 + axis as u16 * 2);
            m8(p);
        }
        p.label(format!("{base}_done"));
    }
    write(p, 0x451, 0);
    write(p, 0x220, 1);
    p.label("object_display_done");
    implied(p, M::Rts);
}
