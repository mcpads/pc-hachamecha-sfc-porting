use crate::{Result, emit::*, transfer::WramTransferPlan};
use w65c816::{AddressingMode as A, Assembler, CpuMode, Mnemonic as M};
pub struct Publication {
    control: u16,
    plan: WramTransferPlan,
}
impl Publication {
    pub fn new(control: u16, plan: WramTransferPlan) -> Result<Self> {
        if !(0x0200..=0x1efd).contains(&control) || plan.chunks().len() > 255 {
            return Err("publication.control_or_job_extent".into());
        }
        Ok(Self { control, plan })
    }
    pub fn report(&self) -> serde_json::Value {
        serde_json::json!(
            { "control" : self.control, "control_bytes" : 3, "plan" : self.plan.report(),
            "states" : { "idle" : 0, "preparing" : 1, "pending" : 2 }, "generation" :
            "u8 wraps after 255; completion observation, not unique identity",
            "publication" :
            "one chunk per successful consume call; destination may be partially updated"
            }
        )
    }
    pub fn validate_memory(&self, memory: &crate::memory_map::MemoryMap) -> Result<()> {
        memory.require_reserved(crate::memory_map::Space::Wram, self.control as usize, 3)?;
        self.plan.validate_memory(memory)
    }
    pub fn emit_reset(&self, p: &mut Assembler) {
        for offset in 0..3 {
            write(p, self.control + offset, 0);
        }
    }
    pub fn emit_begin(&self, p: &mut Assembler, label: &str) {
        p.label(label).assume_mode(CpuMode::NATIVE_M8_X16);
        abs(p, M::Lda, self.control);
        branch(p, M::Bne, &format!("{label}.busy"));
        write(p, self.control, 1);
        op(p, M::Clc);
        op(p, M::Rts);
        p.label(format!("{label}.busy"));
        op(p, M::Sec);
        op(p, M::Rts);
    }
    pub fn emit_submit(&self, p: &mut Assembler, label: &str) {
        p.label(label).assume_mode(CpuMode::NATIVE_M8_X16);
        abs(p, M::Lda, self.control);
        byte(p, M::Cmp, 1);
        branch(p, M::Bne, &format!("{label}.refused"));
        write(p, self.control + 1, 0);
        write(p, self.control, 2);
        op(p, M::Clc);
        op(p, M::Rts);
        p.label(format!("{label}.refused"));
        op(p, M::Sec);
        op(p, M::Rts);
    }
    pub fn emit_consume(&self, p: &mut Assembler, channel: u8, label: &str) -> Result<()> {
        self.emit_consumer(p, channel, label, None)
    }
    pub fn emit_consume_blanked(
        &self,
        p: &mut Assembler,
        channel: u8,
        label: &str,
        brightness: u8,
    ) -> Result<()> {
        if brightness > 15 {
            return Err("publication.brightness".into());
        }
        self.emit_consumer(p, channel, label, Some(brightness))
    }
    fn emit_consumer(
        &self,
        p: &mut Assembler,
        channel: u8,
        label: &str,
        brightness: Option<u8>,
    ) -> Result<()> {
        if channel > 7 {
            return Err("dma.channel".into());
        }
        p.label(label).assume_mode(CpuMode::NATIVE_M8_X16);
        abs(p, M::Lda, self.control);
        byte(p, M::Cmp, 2);
        branch(p, M::Beq, &format!("{label}.dispatch"));
        op(p, M::Sec);
        op(p, M::Rts);
        p.label(format!("{label}.dispatch"));
        if brightness.is_some() {
            write(p, 0x2100, 0x80);
        }
        for (index, chunk) in self.plan.chunks().iter().enumerate() {
            let next = format!("{label}.next{index}");
            abs(p, M::Lda, self.control + 1);
            byte(p, M::Cmp, index as u8);
            branch(p, M::Bne, &next);
            chunk.emit(p, channel)?;
            abs(p, M::Inc, self.control + 1);
            if index + 1 == self.plan.chunks().len() {
                if let Some(brightness) = brightness {
                    write(p, 0x2100, brightness);
                }
                abs(p, M::Inc, self.control + 2);
                write(p, self.control, 0);
            }
            op(p, M::Clc);
            op(p, M::Rts);
            p.label(next);
        }
        op(p, M::Sec);
        op(p, M::Rts);
        Ok(())
    }
    pub fn emit_call(p: &mut Assembler, label: &str) {
        p.emit_label_ref(M::Jsr, A::Absolute, label);
    }
}
pub(crate) fn emit_immutable_steps<'a>(
    p: &mut Assembler,
    steps: impl Iterator<Item = &'a crate::transfer::Chunk>,
    control: u16,
    channel: u8,
    channels: crate::transfer::Channels,
    label: &str,
    admission: &str,
) -> Result<()> {
    p.label(label).assume_mode(CpuMode::NATIVE_M8_X16);
    for (index, chunk) in steps.enumerate() {
        let next = format!("{label}.next{index}");
        let admitted = format!("{label}.admitted{index}");
        abs(p, M::Lda, control);
        byte(p, M::Cmp, index as u8);
        branch(p, M::Bne, &next);
        p.emit_label_ref(M::Jsr, A::Absolute, admission);
        branch(p, M::Bcc, &admitted);
        op(p, M::Sec);
        op(p, M::Rts);
        p.label(admitted);
        chunk.emit_owned(p, channel, channels)?;
        abs(p, M::Inc, control);
        op(p, M::Clc);
        op(p, M::Rts);
        p.label(next);
    }
    op(p, M::Sec);
    op(p, M::Rts);
    Ok(())
}
