use w65c816::{AddressingMode as A, Assembler, Instruction, Mnemonic as M, Operand as O};
pub(crate) fn op(p: &mut Assembler, mnemonic: M) {
    p.emit(Instruction::new(mnemonic, A::Implied, O::None));
}
pub(crate) fn abs(p: &mut Assembler, mnemonic: M, address: u16) {
    p.emit(Instruction::new(mnemonic, A::Absolute, O::Word(address)));
}
pub(crate) fn byte(p: &mut Assembler, mnemonic: M, value: u8) {
    p.emit(Instruction::new(mnemonic, A::Immediate, O::Byte(value)));
}
pub(crate) fn word(p: &mut Assembler, mnemonic: M, value: u16) {
    p.emit(Instruction::new(mnemonic, A::Immediate, O::Word(value)));
}
pub(crate) fn mask(p: &mut Assembler, mnemonic: M, value: u8) {
    p.emit(Instruction::new(mnemonic, A::ImmediateByte, O::Byte(value)));
}
pub(crate) fn write(p: &mut Assembler, address: u16, value: u8) {
    byte(p, M::Lda, value);
    abs(p, M::Sta, address);
}
pub(crate) fn branch(p: &mut Assembler, mnemonic: M, label: &str) {
    p.emit_label_ref(mnemonic, A::Relative8, label);
}
