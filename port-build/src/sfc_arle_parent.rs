use crate::{
    Result,
    sfc::{emit, implied, write},
};
use w65c816::{AddressingMode as A, Assembler, CpuMode, Mnemonic as M, Operand as O};
pub struct Parent {
    pub source: Vec<serde_json::Value>,
}
fn abs(p: &mut Assembler, m: M, a: u16) {
    emit(p, m, A::Absolute, O::Word(a));
}
fn imm(p: &mut Assembler, m: M, n: u16) {
    emit(p, m, A::Immediate, O::Word(n));
}
fn byte(p: &mut Assembler, m: M, n: u8) {
    emit(p, m, A::Immediate, O::Byte(n));
}
fn branch(p: &mut Assembler, m: M, label: &str) {
    p.emit_label_ref(m, A::Relative8, label);
}
fn jump(p: &mut Assembler, label: &str) {
    p.emit_label_ref(M::Brl, A::Relative16, label);
}
fn m16(p: &mut Assembler) {
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
}
fn m8(p: &mut Assembler) {
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
}
fn pc(p: &mut Assembler, value: u32) {
    for (i, b) in value.to_le_bytes().into_iter().enumerate() {
        write(p, 0x334 + i as u16, b);
    }
}
fn parent_word(p: &mut Assembler, offset: u32, value: u16) {
    imm(p, M::Lda, value);
    emit(p, M::Sta, A::AbsoluteLongX, O::Long(0x7e5000 + offset));
}
impl Parent {
    pub fn compile(commands: &[crate::vm::Command]) -> Result<Self> {
        let mut expected: Vec<(u32, Vec<u8>)> = vec![
            (
                0x46f1a0,
                vec![
                    8, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0x40, 3, 0x1c, 0, 0, 0, 0, 0x40, 1, 3, 0x20,
                    0, 0, 0, 0, 0xa0, 0, 3, 0x2c, 0, 0, 5, 0, 0, 0, 3, 0x28, 0, 0, 0, 0, 0x29, 0,
                    255, 0, 0, 0,
                ],
            ),
            (0x46f1d0, vec![0x4b, 0, 0, 0]),
        ];
        for (start, signal, amplitude, child) in [
            (0x46f1d4u32, 0u8, 80u16, 0x46fbe0u32),
            (0x46f1f4, 1, 336, 0x46fbb4),
            (0x46f214, 2, 80, 0x46fbe0),
        ] {
            expected.push((start, vec![1, 0, 0, 0]));
            let mut compare = vec![0x13, 0xc1, 0x19, signal];
            compare.extend(start.to_le_bytes());
            expected.push((start + 4, compare));
            let mut assign = vec![0x11, 0x80, 0x92, 0];
            assign.extend(u32::from(amplitude).to_le_bytes());
            expected.push((start + 12, assign));
            let mut spawn = vec![7, 1, 0, 0];
            spawn.extend(child.to_le_bytes());
            expected.push((start + 20, spawn));
            expected.push((start + 28, vec![0x1f, 0, 0, 0]));
        }
        expected.push((0x46f234, vec![1, 0, 0, 0]));
        expected.push((0x46f238, vec![3, 0, 0, 0, 0x34, 0xf2, 0x46, 0]));
        let mut source = Vec::new();
        for (va, bytes) in expected {
            let command = commands
                .iter()
                .find(|c| c.va == va)
                .ok_or("Missing Arle parent command")?;
            if command.bytes != bytes {
                return Err("Arle parent differs from admitted source".into());
            }
            source.push(serde_json::json!({ "va" : va, "bytes" : bytes }));
        }
        Ok(Self { source })
    }
    pub fn reset(&self, p: &mut Assembler) {
        for at in 0x330..=0x33f {
            write(p, at, 0);
        }
    }
    pub fn routine(&self, p: &mut Assembler) {
        p.label("arle_parent_tick")
            .assume_mode(CpuMode::NATIVE_M8_X16);
        abs(p, M::Lda, 0x330);
        branch(p, M::Bne, "arle_parent_start");
        jump(p, "arle_parent_continue");
        p.label("arle_parent_start");
        write(p, 0x330, 0);
        abs(p, M::Lda, 0x310);
        byte(p, M::Cmp, 2);
        branch(p, M::Bne, "arle_parent_invalid");
        abs(p, M::Lda, 0x331);
        byte(p, M::Cmp, crate::sfc_logical_objects::COUNT as u8);
        branch(p, M::Bcc, "arle_parent_slot_valid");
        p.label("arle_parent_invalid");
        write(p, 0x33b, 1);
        implied(p, M::Rts);
        p.label("arle_parent_slot_valid");
        abs(p, M::Sta, 0x33a);
        m16(p);
        imm(p, M::And, 255);
        for _ in 0..4 {
            emit(p, M::Asl, A::Accumulator, O::None);
        }
        abs(p, M::Sta, 0x33c);
        emit(p, M::Asl, A::Accumulator, O::None);
        abs(p, M::Sta, 0x33e);
        emit(p, M::Asl, A::Accumulator, O::None);
        implied(p, M::Clc);
        abs(p, M::Adc, 0x33e);
        abs(p, M::Sta, 0x33e);
        implied(p, M::Tax);
        m8(p);
        emit(p, M::Lda, A::AbsoluteLongX, O::Long(0x7e4009));
        byte(p, M::Cmp, 1);
        branch(p, M::Bne, "arle_parent_initialize");
        jump(p, "arle_parent_busy");
        p.label("arle_parent_initialize");
        m16(p);
        abs(p, M::Ldx, 0x33c);
        for (offset, value) in [(0, 0), (2, 320), (4, 0), (6, 160), (10, 0), (12, 0x29)] {
            parent_word(p, offset, value);
        }
        imm(p, M::Lda, 0);
        abs(p, M::Sta, 0x338);
        m8(p);
        byte(p, M::Lda, 1);
        emit(p, M::Sta, A::AbsoluteLongX, O::Long(0x7e5008));
        write(p, 0x333, 1);
        write(p, 0x33b, 0);
        write(p, 0x316, 1);
        pc(p, 0x46f1d8);
        jump(p, "arle_parent_count");
        p.label("arle_parent_continue");
        abs(p, M::Lda, 0x333);
        branch(p, M::Bne, "arle_parent_active");
        implied(p, M::Rts);
        p.label("arle_parent_active");
        byte(p, M::Cmp, 255);
        branch(p, M::Bne, "arle_parent_valid_phase");
        implied(p, M::Rts);
        p.label("arle_parent_valid_phase");
        for (phase, signal, amplitude, direction, next_pc) in [
            (1u8, 0u8, 80u16, 0u8, 0x46f1f8u32),
            (2, 1, 336, 1, 0x46f218),
            (3, 2, 80, 0, 0x46f238),
        ] {
            let next = format!("arle_parent_phase_{phase}_next");
            abs(p, M::Lda, 0x333);
            byte(p, M::Cmp, phase);
            branch(p, M::Bne, &next);
            abs(p, M::Lda, 0x332);
            byte(p, M::Cmp, signal);
            branch(p, M::Bne, &format!("arle_parent_phase_{phase}_spawn"));
            jump(p, "arle_parent_count");
            p.label(format!("arle_parent_phase_{phase}_spawn"));
            abs(p, M::Ldx, 0x33e);
            emit(p, M::Lda, A::AbsoluteLongX, O::Long(0x7e4009));
            byte(p, M::Cmp, 1);
            branch(p, M::Bne, &format!("arle_parent_phase_{phase}_available"));
            jump(p, "arle_parent_busy");
            p.label(format!("arle_parent_phase_{phase}_available"));
            m16(p);
            imm(p, M::Lda, amplitude);
            emit(p, M::Sta, A::AbsoluteLongX, O::Long(0x7e4004));
            m8(p);
            byte(p, M::Lda, direction);
            emit(p, M::Sta, A::AbsoluteLongX, O::Long(0x7e4006));
            byte(p, M::Lda, 1);
            emit(p, M::Sta, A::AbsoluteLongX, O::Long(0x7e4008));
            write(p, 0x333, phase + 1);
            pc(p, next_pc);
            jump(p, "arle_parent_count");
            p.label(next);
        }
        p.label("arle_parent_count");
        m16(p);
        abs(p, M::Inc, 0x338);
        m8(p);
        implied(p, M::Rts);
        p.label("arle_parent_busy");
        write(p, 0x33b, 2);
        write(p, 0x333, 255);
        implied(p, M::Rts);
    }
}
