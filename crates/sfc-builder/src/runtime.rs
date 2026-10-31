use crate::{Result, emit::*};
use w65c816::{AddressingMode as A, Assembler, CpuMode, Mnemonic as M};
pub const AUTO_PAD_GUARD_ITERATIONS: u16 = 192;
pub const AUTO_PAD_GUARD_CPU_CYCLES: u32 = 5 * AUTO_PAD_GUARD_ITERATIONS as u32 + 2;
#[derive(Clone, Copy)]
pub struct PadStorage(u16);
impl PadStorage {
    pub fn new(base: u16) -> Result<Self> {
        if base & 1 != 0 || base < 0x0200 || base.checked_add(10).is_none_or(|end| end > 0x1f00) {
            return Err(
                "runtime.pad_storage: require aligned ten-byte WRAM extent $0200..$1EFF".into(),
            );
        }
        Ok(Self(base))
    }
    pub fn base(self) -> u16 {
        self.0
    }
    pub fn held(self) -> u16 {
        self.0
    }
    pub fn previous(self) -> u16 {
        self.0 + 2
    }
    pub fn pressed(self) -> u16 {
        self.0 + 4
    }
    pub fn released(self) -> u16 {
        self.0 + 6
    }
    pub fn sample_status(self) -> u16 {
        self.0 + 8
    }
    pub fn faults(self) -> u16 {
        self.0 + 9
    }
}
pub fn emit_interrupt_entry(p: &mut Assembler) {
    emit_saved_native_context(p);
}
pub(crate) fn emit_saved_native_context(p: &mut Assembler) {
    p.assume_mode(CpuMode::NATIVE_M8_X8);
    mask(p, M::Rep, 0x30);
    for mnemonic in [M::Pha, M::Phx, M::Phy, M::Phd, M::Phb] {
        op(p, mnemonic);
    }
    op(p, M::Cld);
    word(p, M::Lda, 0);
    op(p, M::Tcd);
    mask(p, M::Sep, 0x20);
    byte(p, M::Lda, 0);
    op(p, M::Pha);
    op(p, M::Plb);
}
pub fn emit_interrupt_exit(p: &mut Assembler) {
    mask(p, M::Rep, 0x30);
    for mnemonic in [M::Plb, M::Pld, M::Ply, M::Plx, M::Pla, M::Rti] {
        op(p, mnemonic);
    }
}
pub fn emit_auto_pad(p: &mut Assembler, storage: PadStorage, label: &str) {
    p.label(label).assume_mode(CpuMode::NATIVE_M8_X16);
    word(p, M::Ldx, AUTO_PAD_GUARD_ITERATIONS);
    let wait = format!("{label}.guard");
    let ready = format!("{label}.ready");
    p.label(&wait);
    op(p, M::Dex);
    branch(p, M::Bne, &wait);
    abs(p, M::Lda, 0x4212);
    abs(p, M::Sta, storage.sample_status());
    byte(p, M::And, 0x81);
    byte(p, M::Cmp, 0x80);
    branch(p, M::Beq, &ready);
    abs(p, M::Inc, storage.faults());
    mask(p, M::Rep, 0x20);
    abs(p, M::Stz, storage.pressed());
    abs(p, M::Stz, storage.released());
    mask(p, M::Sep, 0x20);
    op(p, M::Sec);
    op(p, M::Rts);
    p.label(&ready);
    mask(p, M::Rep, 0x20);
    abs(p, M::Lda, storage.held());
    abs(p, M::Sta, storage.previous());
    abs(p, M::Lda, 0x4218);
    abs(p, M::Sta, storage.held());
    abs(p, M::Eor, storage.previous());
    abs(p, M::And, storage.held());
    abs(p, M::Sta, storage.pressed());
    abs(p, M::Lda, storage.held());
    word(p, M::Eor, 0xffff);
    abs(p, M::And, storage.previous());
    abs(p, M::Sta, storage.released());
    mask(p, M::Sep, 0x20);
    op(p, M::Clc);
    op(p, M::Rts);
}
pub fn emit_pad_call(p: &mut Assembler, label: &str, failure: &str) {
    p.emit_label_ref(M::Jsr, A::Absolute, label);
    branch(p, M::Bcs, failure);
}
