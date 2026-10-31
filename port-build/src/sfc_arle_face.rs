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
        write(p, 0x344 + i as u16, b);
    }
}
fn parent_word(p: &mut Assembler, offset: u32, value: u16) {
    imm(p, M::Lda, value);
    emit(p, M::Sta, A::AbsoluteLongX, O::Long(0x7e5000 + offset));
}
fn visual(p: &mut Assembler, visible: u8, frame: u16) {
    abs(p, M::Ldx, 0x34c);
    m16(p);
    parent_word(p, 10, frame);
    m8(p);
    byte(p, M::Lda, visible);
    emit(p, M::Sta, A::AbsoluteLongX, O::Long(0x7e5008));
    write(p, 0x224, frame as u8);
    write(p, 0x220, 1);
    write(p, 0x316, 1);
}
impl Parent {
    pub fn compile(commands: &[crate::vm::Command]) -> Result<Self> {
        let expected: &[(u32, &[u8])] = &[
            (
                0x46f280,
                &[
                    0x08, 0x00, 0x00, 0x00, 0x03, 0x1c, 0x00, 0x00, 0x00, 0x00, 0x90, 0x01, 0x03,
                    0x20, 0x00, 0x00, 0x00, 0x00, 0xc0, 0x00, 0x03, 0x2c, 0x00, 0x00, 0x05, 0x00,
                    0x00, 0x00, 0x03, 0x28, 0x00, 0x00, 0x02, 0x00, 0x29, 0x00, 0xff, 0x00, 0x00,
                    0x00,
                ],
            ),
            (0x46f2a8, &[0x4b, 0x00, 0x00, 0x00]),
            (0x46f2ac, &[0x01, 0x00, 0x00, 0x00]),
            (0x46f2b0, &[0x13, 0xc1, 0x19, 0x00, 0xac, 0xf2, 0x46, 0x00]),
            (0x46f2b8, &[0x12, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40]),
            (0x46f2c0, &[0x11, 0x80, 0x92, 0x00, 0x50, 0x00, 0x00, 0x00]),
            (0x46f2c8, &[0x07, 0x01, 0x00, 0x00, 0xe0, 0xfb, 0x46, 0x00]),
            (0x46f2d0, &[0x1f, 0x00, 0x00, 0x00]),
            (0x46f2d4, &[0x01, 0x00, 0x00, 0x00]),
            (0x46f2d8, &[0x13, 0xc1, 0x19, 0x01, 0xd4, 0xf2, 0x46, 0x00]),
            (0x46f2e0, &[0x12, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40]),
            (0x46f2e8, &[0x12, 0x80, 0x28, 0x00, 0x01, 0x00, 0x29, 0x00]),
            (0x46f2f0, &[0x11, 0x80, 0x92, 0x00, 0x50, 0x01, 0x00, 0x00]),
            (0x46f2f8, &[0x07, 0x01, 0x00, 0x00, 0xb4, 0xfb, 0x46, 0x00]),
            (0x46f300, &[0x1f, 0x00, 0x00, 0x00]),
            (0x46f304, &[0x01, 0x00, 0x00, 0x00]),
            (0x46f308, &[0x13, 0xc1, 0x19, 0x02, 0x04, 0xf3, 0x46, 0x00]),
            (0x46f310, &[0x12, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]),
            (0x46f318, &[0x11, 0x80, 0x92, 0x00, 0x50, 0x00, 0x00, 0x00]),
            (0x46f320, &[0x07, 0x01, 0x00, 0x00, 0xe0, 0xfb, 0x46, 0x00]),
            (0x46f328, &[0x1f, 0x00, 0x00, 0x00]),
            (0x46f32c, &[0x01, 0x00, 0x00, 0x00]),
            (0x46f330, &[0x13, 0xc1, 0x19, 0x03, 0x2c, 0xf3, 0x46, 0x00]),
            (0x46f338, &[0x12, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40]),
            (0x46f340, &[0x12, 0x80, 0x28, 0x00, 0x02, 0x00, 0x29, 0x00]),
            (0x46f348, &[0x02, 0x00, 0x10, 0x00]),
            (0x46f34c, &[0x12, 0x80, 0x28, 0x00, 0x01, 0x00, 0x29, 0x00]),
            (0x46f354, &[0x01, 0x00, 0x00, 0x00]),
            (0x46f358, &[0x03, 0x00, 0x00, 0x00, 0x54, 0xf3, 0x46, 0x00]),
        ];
        let mut source = Vec::new();
        for (va, bytes) in expected {
            let command = commands
                .iter()
                .find(|c| c.va == *va)
                .ok_or("Missing Arle parent command")?;
            if command.bytes != *bytes {
                return Err("Arle parent differs from admitted source".into());
            }
            source.push(serde_json::json!({ "va" : va, "bytes" : bytes }));
        }
        Ok(Self { source })
    }
    pub fn reset(&self, p: &mut Assembler) {
        for at in 0x340..=0x350 {
            write(p, at, 0);
        }
    }
    pub fn routine(&self, p: &mut Assembler) {
        p.label("arle_face_tick")
            .assume_mode(CpuMode::NATIVE_M8_X16);
        abs(p, M::Lda, 0x340);
        branch(p, M::Bne, "arle_face_start");
        jump(p, "arle_face_continue");
        p.label("arle_face_start");
        write(p, 0x340, 0);
        abs(p, M::Lda, 0x310);
        byte(p, M::Cmp, 2);
        branch(p, M::Bne, "arle_face_invalid");
        abs(p, M::Lda, 0x341);
        byte(p, M::Cmp, crate::sfc_logical_objects::COUNT as u8);
        branch(p, M::Bcc, "arle_face_slot_valid");
        p.label("arle_face_invalid");
        write(p, 0x34b, 1);
        implied(p, M::Rts);
        p.label("arle_face_slot_valid");
        abs(p, M::Sta, 0x34a);
        m16(p);
        imm(p, M::And, 255);
        for _ in 0..4 {
            emit(p, M::Asl, A::Accumulator, O::None);
        }
        abs(p, M::Sta, 0x34c);
        emit(p, M::Asl, A::Accumulator, O::None);
        abs(p, M::Sta, 0x34e);
        emit(p, M::Asl, A::Accumulator, O::None);
        implied(p, M::Clc);
        abs(p, M::Adc, 0x34e);
        abs(p, M::Sta, 0x34e);
        implied(p, M::Tax);
        m8(p);
        emit(p, M::Lda, A::AbsoluteLongX, O::Long(0x7e4009));
        byte(p, M::Cmp, 1);
        branch(p, M::Bne, "arle_face_initialize");
        jump(p, "arle_face_busy");
        p.label("arle_face_initialize");
        m16(p);
        abs(p, M::Ldx, 0x34c);
        for (offset, value) in [(0, 0), (2, 400), (4, 0), (6, 192), (10, 2), (12, 0x29)] {
            parent_word(p, offset, value);
        }
        imm(p, M::Lda, 0);
        abs(p, M::Sta, 0x348);
        m8(p);
        byte(p, M::Lda, 0);
        emit(p, M::Sta, A::AbsoluteLongX, O::Long(0x7e5008));
        write(p, 0x224, 2);
        write(p, 0x220, 1);
        write(p, 0x350, 0);
        write(p, 0x343, 1);
        write(p, 0x34b, 0);
        write(p, 0x316, 1);
        pc(p, 0x46f2b0);
        jump(p, "arle_face_count");
        p.label("arle_face_continue");
        abs(p, M::Lda, 0x343);
        branch(p, M::Bne, "arle_face_active");
        implied(p, M::Rts);
        p.label("arle_face_active");
        byte(p, M::Cmp, 255);
        branch(p, M::Bne, "arle_face_valid_phase");
        implied(p, M::Rts);
        p.label("arle_face_valid_phase");
        for (phase, signal, amplitude, direction, next_pc) in [
            (1u8, 0u8, 80u16, 0u8, 0x46f2d8u32),
            (2, 1, 336, 1, 0x46f308),
            (3, 2, 80, 0, 0x46f330),
        ] {
            let next = format!("arle_face_phase_{phase}_next");
            abs(p, M::Lda, 0x343);
            byte(p, M::Cmp, phase);
            branch(p, M::Beq, &format!("arle_face_phase_{phase}_selected"));
            jump(p, &next);
            p.label(format!("arle_face_phase_{phase}_selected"));
            abs(p, M::Lda, 0x332);
            byte(p, M::Cmp, signal);
            branch(p, M::Bne, &format!("arle_face_phase_{phase}_spawn"));
            jump(p, "arle_face_count");
            p.label(format!("arle_face_phase_{phase}_spawn"));
            abs(p, M::Ldx, 0x34e);
            emit(p, M::Lda, A::AbsoluteLongX, O::Long(0x7e4009));
            byte(p, M::Cmp, 1);
            branch(p, M::Bne, &format!("arle_face_phase_{phase}_available"));
            jump(p, "arle_face_busy");
            p.label(format!("arle_face_phase_{phase}_available"));
            m16(p);
            imm(p, M::Lda, amplitude);
            emit(p, M::Sta, A::AbsoluteLongX, O::Long(0x7e4004));
            m8(p);
            byte(p, M::Lda, direction);
            emit(p, M::Sta, A::AbsoluteLongX, O::Long(0x7e4006));
            byte(p, M::Lda, 1);
            emit(p, M::Sta, A::AbsoluteLongX, O::Long(0x7e4008));
            visual(
                p,
                if phase == 3 { 0 } else { 1 },
                if phase == 1 { 2 } else { 1 },
            );
            write(p, 0x343, phase + 1);
            pc(p, next_pc);
            jump(p, "arle_face_count");
            p.label(next);
        }
        abs(p, M::Lda, 0x343);
        byte(p, M::Cmp, 4);
        branch(p, M::Bne, "arle_face_wait");
        abs(p, M::Lda, 0x332);
        byte(p, M::Cmp, 3);
        branch(p, M::Bne, "arle_face_reveal");
        jump(p, "arle_face_count");
        p.label("arle_face_reveal");
        visual(p, 1, 2);
        write(p, 0x350, 16);
        write(p, 0x343, 5);
        pc(p, 0x46f348);
        jump(p, "arle_face_count");
        p.label("arle_face_wait");
        byte(p, M::Cmp, 5);
        branch(p, M::Bne, "arle_face_count");
        abs(p, M::Dec, 0x350);
        branch(p, M::Bne, "arle_face_count");
        visual(p, 1, 1);
        write(p, 0x343, 6);
        pc(p, 0x46f358);
        p.label("arle_face_count");
        m16(p);
        abs(p, M::Inc, 0x348);
        m8(p);
        implied(p, M::Rts);
        p.label("arle_face_busy");
        write(p, 0x34b, 2);
        write(p, 0x343, 255);
        implied(p, M::Rts);
    }
}
