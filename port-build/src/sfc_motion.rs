use crate::{
    Result,
    sfc::{emit, implied, write},
};
use sfc_builder::cartridge::CartridgeLayout;
use w65c816::{AddressingMode as A, Assembler, CpuMode, Mnemonic as M, Operand as O};
pub struct Motion {
    pub table: Vec<u8>,
    pub source: Vec<serde_json::Value>,
    pub parent_count: usize,
}
fn abs(p: &mut Assembler, m: M, a: u16) {
    emit(p, m, A::Absolute, O::Word(a));
}
fn imm(p: &mut Assembler, m: M, v: u16) {
    emit(p, m, A::Immediate, O::Word(v));
}
fn word(p: &mut Assembler, a: u16, v: u16) {
    imm(p, M::Lda, v);
    abs(p, M::Sta, a);
}
fn m16(p: &mut Assembler) {
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
}
fn m8(p: &mut Assembler) {
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
}
fn branch(p: &mut Assembler, m: M, label: &str) {
    p.emit_label_ref(m, A::Relative8, label);
}
fn jump(p: &mut Assembler, label: &str) {
    p.emit_label_ref(M::Brl, A::Relative16, label);
}
fn pc(p: &mut Assembler, addresses: [u32; 4], label: &str) {
    for (mode, address) in addresses.iter().enumerate() {
        if mode != 3 {
            abs(p, M::Lda, 0x2b6);
            emit(p, M::Cmp, A::Immediate, O::Byte(mode as u8));
            branch(p, M::Bne, &format!("{label}_next_{mode}"));
        }
        for (i, b) in address.to_le_bytes().iter().enumerate() {
            write(p, 0x2a8 + i as u16, *b);
        }
        jump(p, &format!("{label}_pc_done"));
        p.label(format!("{label}_next_{mode}"));
    }
    p.label(format!("{label}_pc_done"));
}
impl Motion {
    pub fn compile(
        commands: &[crate::vm::Command],
        table: &[u16],
        parent_count: usize,
    ) -> Result<Self> {
        if !(1..=8).contains(&parent_count) {
            return Err("Motion requires 1..8 displayed parents".into());
        }
        if table.len() != 1024 {
            return Err("Missing original motion table".into());
        }
        let mut source = Vec::new();
        for (entry, sign) in [(0x46fbb4u32, 0x82), (0x46fbe0, 0x81)] {
            let expected = [
                (0, vec![0x12, 0xa0, 0xa8, 0xa0]),
                (4, vec![0x12, 0xb0, 0x80, 0x1c]),
                (8, vec![0x11, 0xb0, 0x92, 0x92]),
                (12, vec![0x11, sign, 0x8c, 0, 0, 2, 0, 0]),
                (20, vec![0x2d, 9, 0, 0]),
                (24, vec![0x12, 0xe0, 0x1c, 0x1c]),
                (28, vec![1, 0, 0, 0]),
                (32, {
                    let mut b = vec![6, 0, 32, 0];
                    b.extend((entry + 12).to_le_bytes());
                    b
                }),
                (40, vec![0, 0, 0, 0]),
            ];
            for (offset, bytes) in expected {
                let va = entry + offset;
                let c = commands
                    .iter()
                    .find(|c| c.va == va)
                    .ok_or("Missing horizontal motion command")?;
                if c.bytes != bytes {
                    return Err("Horizontal motion source differs from admitted lowering".into());
                }
                source.push(serde_json::json!({ "va" : va, "bytes" : c.bytes }));
            }
        }
        for (entry, expected) in [
            (
                0x46fb44,
                vec![
                    (0, vec![0x12, 0xa0, 0xa8, 0xa0]),
                    (4, vec![0x12, 0xb0, 0x84, 0x20]),
                    (8, vec![0x11, 0xb0, 0x94, 0x94]),
                    (12, vec![0x11, 0x80, 0x8e, 0, 0, 0x40, 0, 0]),
                    (20, vec![0x11, 0x82, 0x8e, 0, 0, 2, 0, 0]),
                    (28, vec![0x2d, 0x0a, 0, 0]),
                    (32, vec![0x12, 0xe0, 0x20, 0x20]),
                    (36, vec![1, 0, 0, 0]),
                    (40, vec![6, 0, 32, 0, 0x58, 0xfb, 0x46, 0]),
                    (48, vec![0, 0, 0, 0]),
                ],
            ),
            (
                0x46fb78,
                vec![
                    (0, vec![0x12, 0xa0, 0xa8, 0xa0]),
                    (4, vec![0x12, 0xb0, 0x84, 0x20]),
                    (8, vec![0x11, 0xb0, 0x94, 0x94]),
                    (12, vec![0x11, 0xa0, 0x58, 0x94]),
                    (16, vec![0x12, 0x83, 0x58, 0, 0, 0, 1, 0]),
                    (24, vec![0x12, 0xa1, 0x84, 0x58]),
                    (28, vec![0x11, 0x81, 0x8e, 0, 0, 2, 0, 0]),
                    (36, vec![0x2d, 0x0a, 0, 0]),
                    (40, vec![0x12, 0xe0, 0x20, 0x20]),
                    (44, vec![1, 0, 0, 0]),
                    (48, vec![6, 0, 32, 0, 0x94, 0xfb, 0x46, 0]),
                    (56, vec![0, 0, 0, 0]),
                ],
            ),
        ] {
            for (offset, bytes) in expected {
                let va = entry + offset;
                let command = commands
                    .iter()
                    .find(|c| c.va == va)
                    .ok_or("Missing vertical motion command")?;
                if command.bytes != bytes {
                    return Err("Vertical motion source differs from admitted lowering".into());
                }
                source.push(serde_json::json!({ "va" : va, "bytes" : command.bytes }));
            }
        }
        let mut packed: Vec<u8> = (1..=32)
            .flat_map(|n| {
                let v = if n == 32 {
                    65536
                } else {
                    u32::from(table[n * 32])
                };
                v.to_le_bytes()
            })
            .collect();
        packed
            .extend((1..=32).flat_map(|n| (65536 - u32::from(table[1024 - n * 32])).to_le_bytes()));
        Ok(Self {
            table: packed,
            source,
            parent_count,
        })
    }
    pub fn reset(&self, p: &mut Assembler) {
        for a in 0x290..=0x2ef {
            write(p, a, 0);
        }
        for a in 0x310..=0x313 {
            write(p, a, 0);
        }
        m16(p);
        imm(p, M::Ldx, 0);
        imm(p, M::Lda, 0);
        p.label("motion_clear_contexts");
        emit(p, M::Sta, A::AbsoluteLongX, O::Long(0x7e4000));
        implied(p, M::Inx);
        implied(p, M::Inx);
        imm(p, M::Cpx, (crate::sfc_logical_objects::COUNT * 96) as u16);
        branch(p, M::Bne, "motion_clear_contexts");
        m8(p);
        crate::sfc_logical_objects::reset(p, self.parent_count);
    }
    fn dispatch(&self, p: &mut Assembler) {
        p.label("motion_dispatch")
            .assume_mode(CpuMode::NATIVE_M8_X16);
        abs(p, M::Lda, 0x310);
        branch(p, M::Bne, "motion_dispatch_mode");
        p.emit_label_ref(M::Jsr, A::Absolute, "motion_tick");
        implied(p, M::Rts);
        p.label("motion_dispatch_mode");
        emit(p, M::Cmp, A::Immediate, O::Byte(2));
        branch(p, M::Bne, "motion_dispatch_legacy_pool");
        p.emit_label_ref(M::Jsr, A::Absolute, "logical_tick");
        implied(p, M::Rts);
        p.label("motion_dispatch_legacy_pool");
        emit(p, M::Cmp, A::Immediate, O::Byte(1));
        branch(p, M::Beq, "motion_dispatch_pool");
        write(p, 0x311, 1);
        implied(p, M::Rts);
        p.label("motion_dispatch_pool");
        write(p, 0x311, 0);
        write(p, 0x313, 1);
        for owner in 0..self.parent_count {
            let address = 0x7e4000 + (owner * 96) as u32;
            m16(p);
            imm(p, M::Ldx, 0);
            let load = format!("motion_load_{owner}");
            p.label(&load);
            emit(p, M::Lda, A::AbsoluteLongX, O::Long(address));
            emit(p, M::Sta, A::AbsoluteX, O::Word(0x290));
            implied(p, M::Inx);
            implied(p, M::Inx);
            imm(p, M::Cpx, 96);
            branch(p, M::Bne, &load);
            m8(p);
            write(p, 0x297, owner as u8);
            p.emit_label_ref(M::Jsr, A::Absolute, "motion_tick");
            m16(p);
            imm(p, M::Ldx, 0);
            let save = format!("motion_save_{owner}");
            p.label(&save);
            emit(p, M::Lda, A::AbsoluteX, O::Word(0x290));
            emit(p, M::Sta, A::AbsoluteLongX, O::Long(address));
            implied(p, M::Inx);
            implied(p, M::Inx);
            imm(p, M::Cpx, 96);
            branch(p, M::Bne, &save);
            m8(p);
        }
        abs(p, M::Inc, 0x312);
        write(p, 0x313, 0);
        implied(p, M::Rts);
    }
    pub fn routine(&self, p: &mut Assembler, layout: &CartridgeLayout) -> Result<()> {
        self.dispatch(p);
        let section = layout.section("motion_sine")?;
        let source = layout.cpu_address(section.offset)?;
        if source >> 16 != layout.cpu_address(section.offset + self.table.len() - 1)? >> 16 {
            return Err("Motion table crosses LoROM bank".into());
        }
        p.label("motion_tick").assume_mode(CpuMode::NATIVE_M8_X16);
        abs(p, M::Lda, 0x298);
        branch(p, M::Beq, "motion_continue");
        write(p, 0x298, 0);
        abs(p, M::Lda, 0x296);
        emit(p, M::Cmp, A::Immediate, O::Byte(4));
        branch(p, M::Bcc, "motion_direction_valid");
        jump(p, "motion_invalid");
        p.label("motion_direction_valid");
        abs(p, M::Lda, 0x297);
        emit(p, M::Cmp, A::Immediate, O::Byte(self.parent_count as u8));
        branch(p, M::Bcc, "motion_owner_valid");
        jump(p, "motion_invalid");
        p.label("motion_owner_valid");
        m16(p);
        for offset in [0, 2, 4, 6] {
            abs(p, M::Lda, 0x290 + offset);
            abs(p, M::Sta, 0x2b0 + offset);
        }
        m8(p);
        abs(p, M::Lda, 0x2b6);
        emit(p, M::Inc, A::Accumulator, O::None);
        emit(p, M::Lsr, A::Accumulator, O::None);
        emit(p, M::And, A::Immediate, O::Byte(1));
        abs(p, M::Sta, 0x2c3);
        for (a, v) in [(0x299, 1), (0x29a, 0), (0x29b, 0), (0x2ac, 0)] {
            write(p, a, v);
        }
        p.label("motion_continue");
        abs(p, M::Lda, 0x299);
        emit(p, M::Cmp, A::Immediate, O::Byte(1));
        branch(p, M::Beq, "motion_running");
        implied(p, M::Rts);
        p.label("motion_running");
        abs(p, M::Inc, 0x29b);
        abs(p, M::Lda, 0x29a);
        emit(p, M::Cmp, A::Immediate, O::Byte(32));
        branch(p, M::Bcs, "motion_complete");
        jump(p, "motion_step");
        p.label("motion_complete");
        write(p, 0x299, 2);
        write(p, 0x2a6, 0);
        write(p, 0x2a7, 0);
        pc(p, [0x46fc08, 0x46fbdc, 0x46fb74, 0x46fbb0], "motion_end");
        implied(p, M::Rts);
        p.label("motion_step");
        m16(p);
        abs(p, M::Lda, 0x29a);
        imm(p, M::And, 255);
        branch(p, M::Beq, "motion_first_loop");
        implied(p, M::Sec);
        imm(p, M::Eor, 0xffff);
        imm(p, M::Adc, 32);
        p.label("motion_first_loop");
        abs(p, M::Sta, 0x2a6);
        abs(p, M::Lda, 0x29a);
        imm(p, M::And, 255);
        emit(p, M::Asl, A::Accumulator, O::None);
        emit(p, M::Asl, A::Accumulator, O::None);
        implied(p, M::Tax);
        abs(p, M::Lda, 0x2b6);
        imm(p, M::And, 255);
        imm(p, M::Cmp, 3);
        branch(p, M::Bne, "motion_table_selected");
        implied(p, M::Txa);
        implied(p, M::Clc);
        imm(p, M::Adc, 128);
        implied(p, M::Tax);
        p.label("motion_table_selected");
        emit(p, M::Lda, A::AbsoluteLongX, O::Long(source));
        abs(p, M::Sta, 0x2ba);
        emit(p, M::Lda, A::AbsoluteLongX, O::Long(source + 2));
        abs(p, M::Sta, 0x2bc);
        abs(p, M::Lda, 0x2b4);
        branch(p, M::Bpl, "motion_amplitude_positive");
        imm(p, M::Eor, 0xffff);
        emit(p, M::Inc, A::Accumulator, O::None);
        abs(p, M::Sta, 0x2b8);
        m8(p);
        abs(p, M::Lda, 0x2c3);
        emit(p, M::Eor, A::Immediate, O::Byte(1));
        abs(p, M::Sta, 0x2c2);
        branch(p, M::Bra, "motion_sign_ready");
        p.label("motion_amplitude_positive")
            .assume_mode(CpuMode::NATIVE_M16_X16);
        abs(p, M::Sta, 0x2b8);
        m8(p);
        abs(p, M::Lda, 0x2c3);
        abs(p, M::Sta, 0x2c2);
        p.label("motion_sign_ready");
        m16(p);
        word(p, 0x2be, 0);
        word(p, 0x2c0, 0);
        emit(p, M::Ldx, A::Immediate, O::Word(16));
        p.label("motion_multiply");
        abs(p, M::Lsr, 0x2b8);
        branch(p, M::Bcc, "motion_no_add");
        implied(p, M::Clc);
        abs(p, M::Lda, 0x2be);
        abs(p, M::Adc, 0x2ba);
        abs(p, M::Sta, 0x2be);
        abs(p, M::Lda, 0x2c0);
        abs(p, M::Adc, 0x2bc);
        abs(p, M::Sta, 0x2c0);
        p.label("motion_no_add");
        abs(p, M::Asl, 0x2ba);
        abs(p, M::Rol, 0x2bc);
        implied(p, M::Dex);
        branch(p, M::Bne, "motion_multiply");
        m8(p);
        abs(p, M::Lda, 0x2c2);
        branch(p, M::Beq, "motion_product_positive");
        m16(p);
        abs(p, M::Lda, 0x2be);
        imm(p, M::Eor, 0xffff);
        implied(p, M::Clc);
        imm(p, M::Adc, 1);
        abs(p, M::Sta, 0x2be);
        abs(p, M::Lda, 0x2c0);
        imm(p, M::Eor, 0xffff);
        imm(p, M::Adc, 0);
        abs(p, M::Sta, 0x2c0);
        m8(p);
        p.label("motion_product_positive");
        m16(p);
        implied(p, M::Clc);
        abs(p, M::Lda, 0x2b0);
        abs(p, M::Adc, 0x2be);
        abs(p, M::Sta, 0x2a0);
        abs(p, M::Lda, 0x2b2);
        abs(p, M::Adc, 0x2c0);
        abs(p, M::Sta, 0x2a2);
        abs(p, M::Lda, 0x29a);
        imm(p, M::And, 255);
        emit(p, M::Inc, A::Accumulator, O::None);
        for _ in 0..9 {
            emit(p, M::Asl, A::Accumulator, O::None);
        }
        abs(p, M::Sta, 0x2a4);
        m8(p);
        abs(p, M::Lda, 0x2b6);
        emit(p, M::And, A::Immediate, O::Byte(1));
        abs(p, M::Cmp, 0x2c3);
        branch(p, M::Bne, "motion_vertical_angle");
        abs(p, M::Lda, 0x2c3);
        branch(p, M::Beq, "motion_angle_ready");
        m16(p);
        abs(p, M::Lda, 0x2a4);
        imm(p, M::Eor, 0xffff);
        emit(p, M::Inc, A::Accumulator, O::None);
        abs(p, M::Sta, 0x2a4);
        m8(p);
        branch(p, M::Bra, "motion_angle_ready");
        p.label("motion_vertical_angle");
        abs(p, M::Lda, 0x2b6);
        emit(p, M::Cmp, A::Immediate, O::Byte(2));
        branch(p, M::Bne, "motion_angle_ready");
        m16(p);
        imm(p, M::Lda, 0x4000);
        implied(p, M::Sec);
        abs(p, M::Sbc, 0x2a4);
        abs(p, M::Sta, 0x2a4);
        m8(p);
        p.label("motion_angle_ready");
        abs(p, M::Inc, 0x29a);
        pc(p, [0x46fc00, 0x46fbd4, 0x46fb6c, 0x46fba8], "motion_yield");
        abs(p, M::Lda, 0x310);
        emit(p, M::Cmp, A::Immediate, O::Byte(2));
        branch(p, M::Bne, "motion_display_projection");
        implied(p, M::Rts);
        p.label("motion_display_projection");
        abs(p, M::Lda, 0x2b6);
        emit(p, M::Cmp, A::Immediate, O::Byte(2));
        branch(p, M::Bcs, "motion_project_vertical");
        m16(p);
        p.emit_label_ref(M::Jsr, A::Absolute, "motion_project");
        branch(p, M::Bra, "motion_project_done");
        p.label("motion_project_vertical")
            .assume_mode(CpuMode::NATIVE_M8_X16);
        m16(p);
        p.emit_label_ref(M::Jsr, A::Absolute, "motion_project_y");
        p.label("motion_project_done")
            .assume_mode(CpuMode::NATIVE_M16_X16);
        abs(p, M::Lda, 0x2b7);
        imm(p, M::And, 255);
        emit(p, M::Asl, A::Accumulator, O::None);
        emit(p, M::Asl, A::Accumulator, O::None);
        implied(p, M::Tax);
        abs(p, M::Lda, 0x2b6);
        imm(p, M::And, 255);
        imm(p, M::Cmp, 2);
        branch(p, M::Bcc, "motion_axis_selected");
        implied(p, M::Inx);
        implied(p, M::Inx);
        p.label("motion_axis_selected");
        abs(p, M::Lda, 0x2e0);
        emit(p, M::Sta, A::AbsoluteX, O::Word(0x240));
        m8(p);
        write(p, 0x220, 1);
        implied(p, M::Rts);
        p.label("motion_invalid")
            .assume_mode(CpuMode::NATIVE_M8_X16);
        write(p, 0x299, 3);
        write(p, 0x2ac, 1);
        implied(p, M::Rts);
        self.project(p, "motion_project", 241, 640, 7);
        self.project(p, "motion_project_y", 144, 382, 0);
        crate::sfc_logical_objects::routines(p, self.parent_count);
        Ok(())
    }
    pub(crate) fn project(
        &self,
        p: &mut Assembler,
        name: &str,
        scale: u16,
        divisor: u16,
        offset: u16,
    ) {
        p.label(name).assume_mode(CpuMode::NATIVE_M16_X16);
        word(p, 0x2d2, 0);
        abs(p, M::Lda, 0x2a2);
        abs(p, M::Sta, 0x2d0);
        branch(p, M::Bpl, &format!("{name}_projection_integer"));
        word(p, 0x2d2, 1);
        abs(p, M::Lda, 0x2a0);
        branch(p, M::Beq, &format!("{name}_projection_truncated"));
        abs(p, M::Inc, 0x2d0);
        p.label(format!("{name}_projection_truncated"));
        abs(p, M::Lda, 0x2d0);
        imm(p, M::Eor, 0xffff);
        emit(p, M::Inc, A::Accumulator, O::None);
        abs(p, M::Sta, 0x2d0);
        p.label(format!("{name}_projection_integer"));
        abs(p, M::Lda, 0x2d0);
        abs(p, M::Sta, 0x2d8);
        word(p, 0x2da, 0);
        word(p, 0x2d4, 0);
        word(p, 0x2d6, 0);
        for bit in 0..8 {
            if scale & (1 << bit) != 0 {
                implied(p, M::Clc);
                abs(p, M::Lda, 0x2d4);
                abs(p, M::Adc, 0x2d8);
                abs(p, M::Sta, 0x2d4);
                abs(p, M::Lda, 0x2d6);
                abs(p, M::Adc, 0x2da);
                abs(p, M::Sta, 0x2d6);
            }
            abs(p, M::Asl, 0x2d8);
            abs(p, M::Rol, 0x2da);
        }
        word(p, 0x2dc, 0);
        emit(p, M::Ldx, A::Immediate, O::Word(32));
        p.label(format!("{name}_projection_divide"));
        abs(p, M::Asl, 0x2d4);
        abs(p, M::Rol, 0x2d6);
        abs(p, M::Lda, 0x2dc);
        emit(p, M::Rol, A::Accumulator, O::None);
        imm(p, M::Cmp, divisor);
        branch(p, M::Bcc, &format!("{name}_projection_remainder"));
        imm(p, M::Sbc, divisor);
        abs(p, M::Inc, 0x2d4);
        p.label(format!("{name}_projection_remainder"));
        abs(p, M::Sta, 0x2dc);
        implied(p, M::Dex);
        branch(p, M::Bne, &format!("{name}_projection_divide"));
        abs(p, M::Lda, 0x2d2);
        branch(p, M::Beq, &format!("{name}_projection_positive"));
        abs(p, M::Lda, 0x2d4);
        imm(p, M::Eor, 0xffff);
        emit(p, M::Inc, A::Accumulator, O::None);
        branch(p, M::Bra, &format!("{name}_projection_signed"));
        p.label(format!("{name}_projection_positive"));
        abs(p, M::Lda, 0x2d4);
        p.label(format!("{name}_projection_signed"));
        implied(p, M::Clc);
        imm(p, M::Adc, offset);
        branch(p, M::Bmi, &format!("{name}_projection_negative"));
        imm(p, M::Cmp, 1024);
        branch(p, M::Bcc, &format!("{name}_projection_store"));
        imm(p, M::Lda, 1023);
        branch(p, M::Bra, &format!("{name}_projection_store"));
        p.label(format!("{name}_projection_negative"));
        imm(p, M::Cmp, (-1024i16) as u16);
        branch(p, M::Bcs, &format!("{name}_projection_store"));
        imm(p, M::Lda, (-1024i16) as u16);
        p.label(format!("{name}_projection_store"));
        abs(p, M::Sta, 0x2e0);
        implied(p, M::Rts);
    }
}
