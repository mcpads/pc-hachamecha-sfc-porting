use crate::sfc::{emit, implied, write};
use w65c816::{AddressingMode as A, Assembler, CpuMode, Mnemonic as M, Operand as O};
pub const COUNT: usize = 16;
fn abs(p: &mut Assembler, m: M, address: u16) {
    emit(p, m, A::Absolute, O::Word(address));
}
fn long_x(p: &mut Assembler, m: M, address: u32) {
    emit(p, m, A::AbsoluteLongX, O::Long(address));
}
fn imm(p: &mut Assembler, m: M, value: u16) {
    emit(p, m, A::Immediate, O::Word(value));
}
fn m16(p: &mut Assembler) {
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
}
fn m8(p: &mut Assembler) {
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
}
fn branch(p: &mut Assembler, m: M, name: &str) {
    p.emit_label_ref(m, A::Relative8, name);
}
fn jump(p: &mut Assembler, name: &str) {
    p.emit_label_ref(M::Brl, A::Relative16, name);
}
pub fn reset(p: &mut Assembler, displays: usize) {
    for a in 0x314..=0x31d {
        write(p, a, 0);
    }
    for i in 0..displays {
        write(p, 0x320 + i as u16, 255);
    }
    m16(p);
    imm(p, M::Ldx, 0);
    imm(p, M::Lda, 0);
    p.label("logical_clear");
    emit(p, M::Sta, A::AbsoluteLongX, O::Long(0x7e5000));
    implied(p, M::Inx);
    implied(p, M::Inx);
    imm(p, M::Cpx, (COUNT * 16) as u16);
    branch(p, M::Bne, "logical_clear");
    m8(p);
}
pub fn routines(p: &mut Assembler, displays: usize) {
    p.label("logical_tick").assume_mode(CpuMode::NATIVE_M8_X16);
    write(p, 0x311, 0);
    write(p, 0x313, 1);
    m16(p);
    imm(p, M::Lda, 0);
    abs(p, M::Sta, 0x31a);
    abs(p, M::Sta, 0x31c);
    m8(p);
    p.label("logical_slot");
    abs(p, M::Ldx, 0x31a);
    long_x(p, M::Lda, 0x7e4008);
    branch(p, M::Bne, "logical_active");
    long_x(p, M::Lda, 0x7e4009);
    emit(p, M::Cmp, A::Immediate, O::Byte(1));
    branch(p, M::Beq, "logical_active");
    jump(p, "logical_next");
    p.label("logical_active");
    m16(p);
    abs(p, M::Ldx, 0x31a);
    imm(p, M::Ldy, 0);
    p.label("logical_load");
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(0x7e4000));
    emit(p, M::Sta, A::AbsoluteY, O::Word(0x290));
    implied(p, M::Inx);
    implied(p, M::Inx);
    implied(p, M::Iny);
    implied(p, M::Iny);
    imm(p, M::Cpy, 96);
    branch(p, M::Bne, "logical_load");
    m8(p);
    abs(p, M::Ldx, 0x31c);
    abs(p, M::Lda, 0x298);
    branch(p, M::Beq, "logical_run");
    abs(p, M::Lda, 0x296);
    emit(p, M::Cmp, A::Immediate, O::Byte(2));
    branch(p, M::Bcs, "logical_origin_y");
    m16(p);
    long_x(p, M::Lda, 0x7e5000);
    abs(p, M::Sta, 0x290);
    long_x(p, M::Lda, 0x7e5002);
    abs(p, M::Sta, 0x292);
    m8(p);
    branch(p, M::Bra, "logical_run");
    p.label("logical_origin_y");
    m16(p);
    long_x(p, M::Lda, 0x7e5004);
    abs(p, M::Sta, 0x290);
    long_x(p, M::Lda, 0x7e5006);
    abs(p, M::Sta, 0x292);
    m8(p);
    p.label("logical_run");
    write(p, 0x297, 0);
    p.emit_label_ref(M::Jsr, A::Absolute, "motion_tick");
    abs(p, M::Ldx, 0x31c);
    abs(p, M::Lda, 0x299);
    emit(p, M::Cmp, A::Immediate, O::Byte(3));
    branch(p, M::Beq, "logical_save");
    abs(p, M::Lda, 0x2b6);
    emit(p, M::Cmp, A::Immediate, O::Byte(2));
    branch(p, M::Bcs, "logical_write_y");
    m16(p);
    abs(p, M::Lda, 0x2a0);
    long_x(p, M::Sta, 0x7e5000);
    abs(p, M::Lda, 0x2a2);
    long_x(p, M::Sta, 0x7e5002);
    m8(p);
    branch(p, M::Bra, "logical_changed");
    p.label("logical_write_y");
    m16(p);
    abs(p, M::Lda, 0x2a0);
    long_x(p, M::Sta, 0x7e5004);
    abs(p, M::Lda, 0x2a2);
    long_x(p, M::Sta, 0x7e5006);
    m8(p);
    p.label("logical_changed");
    write(p, 0x316, 1);
    p.label("logical_save");
    m16(p);
    abs(p, M::Ldx, 0x31a);
    imm(p, M::Ldy, 0);
    p.label("logical_copy");
    emit(p, M::Lda, A::AbsoluteY, O::Word(0x290));
    emit(p, M::Sta, A::AbsoluteLongX, O::Long(0x7e4000));
    implied(p, M::Inx);
    implied(p, M::Inx);
    implied(p, M::Iny);
    implied(p, M::Iny);
    imm(p, M::Cpy, 96);
    branch(p, M::Bne, "logical_copy");
    m8(p);
    p.label("logical_next");
    m16(p);
    abs(p, M::Lda, 0x31a);
    implied(p, M::Clc);
    imm(p, M::Adc, 96);
    abs(p, M::Sta, 0x31a);
    abs(p, M::Lda, 0x31c);
    implied(p, M::Clc);
    imm(p, M::Adc, 16);
    abs(p, M::Sta, 0x31c);
    imm(p, M::Cmp, (COUNT * 16) as u16);
    m8(p);
    branch(p, M::Bcs, "logical_pass_done");
    jump(p, "logical_slot");
    p.label("logical_pass_done");
    abs(p, M::Inc, 0x312);
    p.emit_label_ref(M::Jsr, A::Absolute, "logical_publish");
    write(p, 0x313, 0);
    implied(p, M::Rts);
    p.label("logical_publish")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, 0x316);
    branch(p, M::Bne, "logical_publish_requested");
    implied(p, M::Rts);
    p.label("logical_publish_requested");
    write(p, 0x316, 0);
    for display in 0..displays {
        let valid = format!("logical_binding_{display}_valid");
        abs(p, M::Lda, 0x320 + display as u16);
        emit(p, M::Cmp, A::Immediate, O::Byte(255));
        branch(p, M::Beq, &valid);
        emit(p, M::Cmp, A::Immediate, O::Byte(COUNT as u8));
        branch(p, M::Bcc, &valid);
        write(p, 0x315, 1);
        implied(p, M::Rts);
        p.label(valid);
    }
    write(p, 0x315, 0);
    for display in 0..displays {
        let shown = format!("logical_display_{display}_shown");
        let hidden = format!("logical_display_{display}_hidden");
        let done = format!("logical_display_{display}_done");
        abs(p, M::Lda, 0x320 + display as u16);
        emit(p, M::Cmp, A::Immediate, O::Byte(255));
        branch(p, M::Beq, &hidden);
        m16(p);
        imm(p, M::And, 255);
        for _ in 0..4 {
            emit(p, M::Asl, A::Accumulator, O::None);
        }
        abs(p, M::Sta, 0x318);
        implied(p, M::Tax);
        m8(p);
        emit(p, M::Lda, A::AbsoluteLongX, O::Long(0x7e5008));
        branch(p, M::Bne, &shown);
        p.label(hidden);
        m16(p);
        imm(p, M::Lda, (-1024i16) as u16);
        abs(p, M::Sta, 0x240 + display as u16 * 4);
        abs(p, M::Sta, 0x242 + display as u16 * 4);
        m8(p);
        jump(p, &done);
        p.label(shown).assume_mode(CpuMode::NATIVE_M8_X16);
        for (axis, routine) in [(0, "motion_project"), (4, "motion_project_y")] {
            m16(p);
            abs(p, M::Ldx, 0x318);
            emit(p, M::Lda, A::AbsoluteLongX, O::Long(0x7e5000 + axis));
            abs(p, M::Sta, 0x2a0);
            emit(p, M::Lda, A::AbsoluteLongX, O::Long(0x7e5002 + axis));
            abs(p, M::Sta, 0x2a2);
            p.emit_label_ref(M::Jsr, A::Absolute, routine);
            abs(p, M::Lda, 0x2e0);
            abs(p, M::Sta, 0x240 + display as u16 * 4 + axis as u16 / 2);
            m8(p);
        }
        p.label(done);
    }
    write(p, 0x315, 0);
    write(p, 0x220, 1);
    implied(p, M::Rts);
}
