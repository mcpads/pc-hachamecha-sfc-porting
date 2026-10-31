use crate::{Result, cartridge::BatteryRam};
use w65c816::{AddressingMode, Assembler, Instruction, Mnemonic, Operand};
pub fn emit_read(p: &mut Assembler, ram: BatteryRam, offset: usize) -> Result<()> {
    p.emit(Instruction::new(
        Mnemonic::Lda,
        AddressingMode::AbsoluteLong,
        Operand::Long(ram.cpu_address(offset, 1)?),
    ));
    Ok(())
}
pub fn emit_write(p: &mut Assembler, ram: BatteryRam, offset: usize) -> Result<()> {
    p.emit(Instruction::new(
        Mnemonic::Sta,
        AddressingMode::AbsoluteLong,
        Operand::Long(ram.cpu_address(offset, 1)?),
    ));
    Ok(())
}
pub struct CopyPlan<'a> {
    _layout: &'a crate::cartridge::CartridgeLayout,
    chunks: Vec<(u32, u32, u16)>,
}
impl<'a> CopyPlan<'a> {
    pub fn new(
        layout: &'a crate::cartridge::CartridgeLayout,
        memory: &crate::memory_map::MemoryMap,
        sram_offset: usize,
        wram_offset: usize,
        bytes: usize,
    ) -> Result<Self> {
        use crate::memory_map::Space;
        let ram = layout.battery_ram()?;
        memory.require_reserved(Space::Sram, sram_offset, bytes)?;
        memory.require_reserved(Space::Wram, wram_offset, bytes)?;
        if sram_offset
            .checked_add(bytes)
            .is_none_or(|end| end > ram.bytes())
        {
            return Err("battery.copy_extent".into());
        }
        let mut chunks = vec![];
        let mut done = 0;
        while done < bytes {
            let wram = wram_offset + done;
            let count = (bytes - done)
                .min(ram.contiguous_bytes(sram_offset + done)?)
                .min(0x10000 - wram % 0x10000);
            chunks.push((
                ram.cpu_address(sram_offset + done, count)?,
                0x7e0000 + wram as u32,
                count as u16,
            ));
            done += count;
        }
        Ok(Self {
            _layout: layout,
            chunks,
        })
    }
    pub fn emit_save(&self, p: &mut Assembler, prefix: &str) -> Result<()> {
        self.emit(p, prefix, true)
    }
    pub fn emit_load(&self, p: &mut Assembler, prefix: &str) -> Result<()> {
        self.emit(p, prefix, false)
    }
    fn emit(&self, p: &mut Assembler, prefix: &str, save: bool) -> Result<()> {
        use crate::emit::*;
        use Mnemonic as M;
        if prefix.is_empty() {
            return Err("battery.copy_label".into());
        }
        for (i, &(sram, wram, bytes)) in self.chunks.iter().enumerate() {
            let label = format!("{prefix}_{i}");
            word(p, M::Ldx, 0);
            p.label(&label);
            for (op, address) in [
                (M::Lda, if save { wram } else { sram }),
                (M::Sta, if save { sram } else { wram }),
            ] {
                p.emit(Instruction::new(
                    op,
                    AddressingMode::AbsoluteLongX,
                    Operand::Long(address),
                ));
            }
            op(p, M::Inx);
            word(p, M::Cpx, bytes);
            branch(p, M::Bne, &label);
        }
        Ok(())
    }
    pub fn report(&self) -> serde_json::Value {
        serde_json::json!(
            { "chunks" : self.chunks.iter().map(|& (sram, wram, bytes) |
            serde_json::json!({ "sram_cpu" : sram, "wram_cpu" : wram, "bytes" : bytes }))
            .collect::< Vec < _ >> (), "mode" : "native M8/X16", "clobbers" : ["A", "X",
            "N", "Z", "C"], "atomic" : false, "runtime_timing_verified" : false }
        )
    }
}
