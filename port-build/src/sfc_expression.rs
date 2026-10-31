use crate::{
    Result,
    sfc::{emit, implied, write},
};
use w65c816::{AddressingMode as A, Assembler, CpuMode, Mnemonic as M, Operand as O};
pub struct Expression {
    pub source: Vec<serde_json::Value>,
}
impl Expression {
    pub fn compile(commands: &[crate::vm::Command]) -> Result<Self> {
        let expected: &[(u32, &[u8])] = &[
            (0x46f338, &[0x12, 0x80, 0, 0, 0, 0, 0, 0x40]),
            (0x46f340, &[0x12, 0x80, 0x28, 0, 2, 0, 0x29, 0]),
            (0x46f348, &[2, 0, 16, 0]),
            (0x46f34c, &[0x12, 0x80, 0x28, 0, 1, 0, 0x29, 0]),
            (0x46f354, &[1, 0, 0, 0]),
            (0x46f358, &[3, 0, 0, 0, 0x54, 0xf3, 0x46, 0]),
        ];
        let mut source = Vec::new();
        for (va, bytes) in expected {
            let command = commands
                .iter()
                .find(|c| c.va == *va)
                .ok_or("Missing expression command")?;
            if command.bytes != *bytes {
                return Err("Expression source differs from admitted lowering".into());
            }
            source.push(serde_json::json!({ "va" : va, "bytes" : command.bytes }));
        }
        Ok(Self { source })
    }
    pub fn reset(&self, p: &mut Assembler) {
        for address in 0x260..=0x26f {
            write(p, address, 0);
        }
    }
    pub fn routine(&self, p: &mut Assembler) {
        p.label("expression_tick")
            .assume_mode(CpuMode::NATIVE_M8_X16);
        emit(p, M::Lda, A::Absolute, O::Word(0x343));
        p.emit_label_ref(M::Beq, A::Relative8, "expression_standalone");
        write(p, 0x260, 0);
        implied(p, M::Rts);
        p.label("expression_standalone");
        emit(p, M::Lda, A::Absolute, O::Word(0x260));
        p.emit_label_ref(M::Beq, A::Relative8, "expression_continue");
        write(p, 0x260, 0);
        write(p, 0x261, 1);
        write(p, 0x262, 16);
        write(p, 0x263, 0);
        write(p, 0x267, 1);
        for (i, b) in 0x40000000u32.to_le_bytes().iter().enumerate() {
            write(p, 0x268 + i as u16, *b);
        }
        for (i, b) in 0x46f348u32.to_le_bytes().iter().enumerate() {
            write(p, 0x26c + i as u16, *b);
        }
        write(p, 0x224, 2);
        write(p, 0x220, 1);
        implied(p, M::Rts);
        p.label("expression_continue");
        emit(p, M::Lda, A::Absolute, O::Word(0x261));
        p.emit_label_ref(M::Bne, A::Relative8, "expression_active");
        implied(p, M::Rts);
        p.label("expression_active");
        emit(p, M::Inc, A::Absolute, O::Word(0x267));
        emit(p, M::Cmp, A::Immediate, O::Byte(1));
        p.emit_label_ref(M::Beq, A::Relative8, "expression_wait");
        implied(p, M::Rts);
        p.label("expression_wait");
        emit(p, M::Dec, A::Absolute, O::Word(0x262));
        p.emit_label_ref(M::Beq, A::Relative8, "expression_change");
        implied(p, M::Rts);
        p.label("expression_change");
        write(p, 0x261, 2);
        write(p, 0x224, 1);
        write(p, 0x220, 1);
        for (i, b) in 0x46f358u32.to_le_bytes().iter().enumerate() {
            write(p, 0x26c + i as u16, *b);
        }
        implied(p, M::Rts);
    }
}
