use crate::{
    Result,
    sfc::{emit, implied, write},
};
use w65c816::{AddressingMode as A, Assembler, CpuMode, Mnemonic as M, Operand as O};
pub struct Bridge {
    pub root: crate::sfc_root::Program,
    pub bindings: Vec<(u32, u8)>,
}
fn abs(p: &mut Assembler, m: M, a: u16) {
    emit(p, m, A::Absolute, O::Word(a));
}
fn byte(p: &mut Assembler, m: M, a: u8) {
    emit(p, m, A::Immediate, O::Byte(a));
}
fn branch(p: &mut Assembler, m: M, s: &str) {
    p.emit_label_ref(m, A::Relative8, s);
}
fn jump(p: &mut Assembler, s: &str) {
    p.emit_label_ref(M::Brl, A::Relative16, s);
}
fn call(p: &mut Assembler, s: &str) {
    p.emit_label_ref(M::Jsr, A::Absolute, s);
}
fn m16(p: &mut Assembler) {
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
}
fn m8(p: &mut Assembler) {
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
}
impl Bridge {
    pub fn compile(
        commands: &[crate::vm::Command],
        messages: &[crate::messages::Message],
    ) -> Result<Self> {
        let root = crate::sfc_root::compile(commands)?;
        let bindings = crate::messages::root_bindings(messages)?;
        for c in commands
            .iter()
            .filter(|c| (0x46e89c..0x46ee98).contains(&c.va) && c.bytes[0] == 0x2f)
        {
            let target = u32::from_le_bytes(c.bytes[4..8].try_into()?);
            if !bindings.iter().any(|(va, _)| *va == target) {
                return Err("Unbound root message".into());
            }
        }
        Ok(Self { root, bindings })
    }
    pub fn emit(&self, p: &mut Assembler) {
        p.label("root_message_tick")
            .assume_mode(CpuMode::NATIVE_M8_X16);
        abs(p, M::Lda, 0x392);
        branch(p, M::Bne, "rm_enabled");
        implied(p, M::Rts);
        p.label("rm_enabled");
        abs(p, M::Lda, 0x360);
        branch(p, M::Beq, "rm_auto");
        abs(p, M::Lda, 0x394);
        branch(p, M::Beq, "rm_manual_valid");
        write(p, 0x360, 0);
        jump(p, "rm_foreign");
        p.label("rm_manual_valid");
        call(p, "root_tick");
        jump(p, "rm_inspect");
        p.label("rm_auto");
        abs(p, M::Lda, 0x361);
        byte(p, M::Cmp, 1);
        branch(p, M::Bne, "rm_not_ready");
        write(p, 0x360, 2);
        call(p, "root_tick");
        jump(p, "rm_inspect");
        p.label("rm_not_ready");
        byte(p, M::Cmp, 3);
        branch(p, M::Beq, "rm_message");
        jump(p, "rm_inspect");
        p.label("rm_message");
        abs(p, M::Lda, 0x394);
        branch(p, M::Bne, "rm_owned");
        jump(p, "rm_foreign");
        p.label("rm_owned");
        abs(p, M::Lda, 0x20a);
        branch(p, M::Beq, "rm_accepted");
        implied(p, M::Rts);
        p.label("rm_accepted");
        abs(p, M::Lda, 0x20c);
        branch(p, M::Beq, "rm_no_error");
        jump(p, "rm_foreign");
        p.label("rm_no_error");
        abs(p, M::Lda, 0x20b);
        abs(p, M::Cmp, 0x394);
        branch(p, M::Beq, "rm_same_id");
        jump(p, "rm_foreign");
        p.label("rm_same_id");
        write(p, 0x397, 2);
        abs(p, M::Lda, 0x207);
        byte(p, M::Cmp, 1);
        branch(p, M::Beq, "rm_finished");
        byte(p, M::Cmp, 0);
        branch(p, M::Beq, "rm_active");
        jump(p, "rm_foreign");
        p.label("rm_active");
        write(p, 0x360, 2);
        call(p, "root_tick");
        implied(p, M::Rts);
        p.label("rm_finished");
        abs(p, M::Lda, 0x20d);
        abs(p, M::Cmp, 0x394);
        branch(p, M::Beq, "rm_finish_valid");
        jump(p, "rm_foreign");
        p.label("rm_finish_valid");
        m16(p);
        abs(p, M::Lda, 0x395);
        abs(p, M::Sta, 0x370);
        m8(p);
        write(p, 0x360, 4);
        call(p, "root_tick");
        abs(p, M::Lda, 0x362);
        branch(p, M::Beq, "rm_released");
        jump(p, "rm_foreign");
        p.label("rm_released");
        write(p, 0x394, 0);
        write(p, 0x397, 0);
        write(p, 0x393, 0);
        implied(p, M::Rts);
        p.label("rm_inspect");
        abs(p, M::Lda, 0x361);
        byte(p, M::Cmp, 2);
        branch(p, M::Beq, "rm_pending");
        implied(p, M::Rts);
        p.label("rm_pending");
        abs(p, M::Lda, 0x378);
        byte(p, M::Cmp, 0x2f);
        branch(p, M::Beq, "rm_handoff");
        implied(p, M::Rts);
        p.label("rm_handoff");
        abs(p, M::Lda, 0x394);
        branch(p, M::Beq, "rm_free");
        jump(p, "rm_foreign");
        p.label("rm_free");
        abs(p, M::Lda, 0x207);
        byte(p, M::Cmp, 1);
        branch(p, M::Bne, "rm_busy");
        abs(p, M::Lda, 0x20a);
        branch(p, M::Beq, "rm_lookup");
        p.label("rm_busy");
        write(p, 0x393, 1);
        implied(p, M::Rts);
        p.label("rm_lookup");
        for (va, id) in &self.bindings {
            let next = format!("rm_not_{id}");
            m16(p);
            abs(p, M::Lda, 0x37c);
            emit(p, M::Cmp, A::Immediate, O::Word(*va as u16));
            branch(p, M::Bne, &next);
            abs(p, M::Lda, 0x37e);
            emit(p, M::Cmp, A::Immediate, O::Word((*va >> 16) as u16));
            branch(p, M::Bne, &next);
            m8(p);
            if *id == 20 {
                jump(p, "rm_unsupported");
            } else {
                write(p, 0x394, *id);
                write(p, 0x20a, *id);
                jump(p, "rm_queue");
            }
            p.label(next).assume_mode(CpuMode::NATIVE_M16_X16);
            m8(p);
        }
        p.label("rm_unsupported");
        write(p, 0x393, 2);
        implied(p, M::Rts);
        p.label("rm_queue");
        write(p, 0x20c, 0);
        write(p, 0x20d, 0);
        write(p, 0x393, 0);
        write(p, 0x397, 1);
        m16(p);
        abs(p, M::Lda, 0x36e);
        abs(p, M::Sta, 0x395);
        abs(p, M::Sta, 0x370);
        m8(p);
        write(p, 0x360, 3);
        call(p, "root_tick");
        implied(p, M::Rts);
        p.label("rm_foreign");
        write(p, 0x393, 3);
        implied(p, M::Rts);
    }
}
