use crate::{Result, cartridge::CartridgeLayout, emit::*};
use w65c816::{Assembler, Mnemonic as M};
#[derive(Clone, Copy)]
pub struct Channels {
    dma: u8,
    hdma: u8,
}
impl Channels {
    pub fn new(dma: u8, hdma: u8) -> Result<Self> {
        if dma & hdma != 0 {
            return Err("dma.channel_conflict".into());
        }
        Ok(Self { dma, hdma })
    }
    pub fn require_dma(self, channel: u8) -> Result<()> {
        Self::require(self.dma, channel)
    }
    pub fn require_hdma(self, channel: u8) -> Result<()> {
        Self::require(self.hdma, channel)
    }
    fn require(mask: u8, channel: u8) -> Result<()> {
        if channel > 7 || mask & (1 << channel) == 0 {
            return Err("dma.channel_not_owned".into());
        }
        Ok(())
    }
    pub fn hdma_mask(self) -> u8 {
        self.hdma
    }
}
#[derive(Clone, Copy, Debug)]
pub enum Destination {
    Vram(u16),
    Cgram(u16),
    Oam(u16),
}
impl Destination {
    fn parts(self) -> (usize, usize) {
        match self {
            Self::Vram(offset) => (usize::from(offset), 65536),
            Self::Cgram(offset) => (usize::from(offset), 512),
            Self::Oam(offset) => (usize::from(offset), 544),
        }
    }
    fn advance(self, bytes: usize) -> Self {
        let offset = (self.parts().0 + bytes) as u16;
        match self {
            Self::Vram(_) => Self::Vram(offset),
            Self::Cgram(_) => Self::Cgram(offset),
            Self::Oam(_) => Self::Oam(offset),
        }
    }
}
pub struct Chunk {
    source: u32,
    offset: Option<usize>,
    bytes: u16,
    destination: Destination,
}
pub struct TransferPlan<'a> {
    layout: &'a CartridgeLayout,
    section: String,
    chunks: Vec<Chunk>,
}
impl<'a> TransferPlan<'a> {
    pub fn new(
        layout: &'a CartridgeLayout,
        section: &str,
        offset: usize,
        bytes: usize,
        destination: Destination,
        byte_quota: usize,
    ) -> Result<Self> {
        let data = layout.section(section)?;
        let end = offset.checked_add(bytes).ok_or("dma.source_overflow")?;
        let (dest, capacity) = destination.parts();
        if bytes == 0 || end > data.bytes.len() {
            return Err("dma.source_extent".into());
        }
        if dest.checked_add(bytes).is_none_or(|end| end > capacity) {
            return Err("dma.destination_extent".into());
        }
        if (dest | bytes | offset | data.offset | byte_quota) & 1 != 0
            || !(2..=65534).contains(&byte_quota)
        {
            return Err("dma.alignment_or_quota: require even ranges and quota 2..65534".into());
        }
        let mut chunks = vec![];
        let mut consumed = 0;
        while consumed < bytes {
            let rom_offset = data.offset + offset + consumed;
            let count = (bytes - consumed)
                .min(byte_quota)
                .min(layout.source_window_remaining(rom_offset)?);
            chunks.push(Chunk {
                source: layout.cpu_address(rom_offset)?,
                offset: Some(rom_offset),
                bytes: count as u16,
                destination: destination.advance(consumed),
            });
            consumed += count;
        }
        Ok(Self {
            layout,
            section: section.into(),
            chunks,
        })
    }
    pub fn chunks(&self) -> &[Chunk] {
        &self.chunks
    }
    pub fn validate_memory(&self, memory: &crate::memory_map::MemoryMap) -> Result<()> {
        for chunk in &self.chunks {
            chunk.validate_memory(memory)?;
        }
        Ok(())
    }
    pub fn report(&self) -> serde_json::Value {
        serde_json::json!(
            { "section" : self.section, "rom_bytes" : self.layout.size(), "source_sha256"
            : crate ::sha256(& self.layout.section(& self.section).unwrap().bytes),
            "chunks" : self.chunks.iter().map(Chunk::report).collect::< Vec < _ >> () }
        )
    }
}
pub struct WramTransferPlan {
    chunks: Vec<Chunk>,
    source_offset: usize,
    bytes: usize,
}
impl WramTransferPlan {
    pub fn new(
        source_offset: usize,
        bytes: usize,
        destination: Destination,
        byte_quota: usize,
    ) -> Result<Self> {
        let (dest, capacity) = destination.parts();
        if source_offset < 0x2000
            || bytes == 0
            || source_offset
                .checked_add(bytes)
                .is_none_or(|end| end > 0x20000)
        {
            return Err("dma.wram_source_extent".into());
        }
        if dest.checked_add(bytes).is_none_or(|end| end > capacity) {
            return Err("dma.destination_extent".into());
        }
        if (source_offset | bytes | dest | byte_quota) & 1 != 0
            || !(2..=65534).contains(&byte_quota)
        {
            return Err("dma.alignment_or_quota".into());
        }
        let mut chunks = vec![];
        let mut consumed = 0;
        while consumed < bytes {
            let offset = source_offset + consumed;
            let count = (bytes - consumed)
                .min(byte_quota)
                .min(0x10000 - offset % 0x10000);
            chunks.push(Chunk {
                source: 0x7e0000 + offset as u32,
                offset: None,
                bytes: count as u16,
                destination: destination.advance(consumed),
            });
            consumed += count;
        }
        Ok(Self {
            chunks,
            source_offset,
            bytes,
        })
    }
    pub fn chunks(&self) -> &[Chunk] {
        &self.chunks
    }
    pub fn validate_memory(&self, memory: &crate::memory_map::MemoryMap) -> Result<()> {
        for chunk in &self.chunks {
            chunk.validate_memory(memory)?;
        }
        Ok(())
    }
    pub fn report(&self) -> serde_json::Value {
        serde_json::json!(
            { "wram_offset" : self.source_offset, "bytes" : self.bytes, "chunks" : self
            .chunks.iter().map(Chunk::report).collect::< Vec < _ >> (), "ownership" :
            "caller must exclude writes until all chunks complete" }
        )
    }
}
impl Chunk {
    fn validate_memory(&self, memory: &crate::memory_map::MemoryMap) -> Result<()> {
        use crate::memory_map::Space;
        let (space, offset) = match self.offset {
            Some(offset) => (Space::Rom, offset),
            None => (Space::Wram, (self.source - 0x7e0000) as usize),
        };
        memory.require_reserved(space, offset, self.bytes as usize)?;
        let destination = match self.destination {
            Destination::Vram(_) => Space::Vram,
            Destination::Cgram(_) => Space::Cgram,
            Destination::Oam(_) => Space::Oam,
        };
        memory.require_reserved(destination, self.destination.parts().0, self.bytes as usize)
    }
    pub fn report(&self) -> serde_json::Value {
        serde_json::json!(
            { "source" : self.source, "rom_offset" : self.offset, "bytes" : self.bytes,
            "destination" : format!("{:?}", self.destination), "dma_data_master_clocks" :
            u32::from(self.bytes) * 8, "setup_and_alignment_master_clocks" : null }
        )
    }
    pub fn emit_owned(&self, p: &mut Assembler, channel: u8, channels: Channels) -> Result<()> {
        channels.require_dma(channel)?;
        self.emit(p, channel)
    }
    pub fn emit(&self, p: &mut Assembler, channel: u8) -> Result<()> {
        if channel > 7 {
            return Err("dma.channel".into());
        }
        let (mode, port) = match self.destination {
            Destination::Vram(offset) => {
                write(p, 0x2115, 0x80);
                write(p, 0x2116, (offset / 2) as u8);
                write(p, 0x2117, (offset / 2 >> 8) as u8);
                (1, 0x18)
            }
            Destination::Cgram(offset) => {
                write(p, 0x2121, (offset / 2) as u8);
                (0, 0x22)
            }
            Destination::Oam(offset) => {
                write(p, 0x2102, (offset / 2) as u8);
                write(p, 0x2103, (offset / 2 >> 8) as u8);
                (0, 0x04)
            }
        };
        let base = 0x4300 + u16::from(channel) * 16;
        for (i, value) in [
            mode,
            port,
            self.source as u8,
            (self.source >> 8) as u8,
            (self.source >> 16) as u8,
            self.bytes as u8,
            (self.bytes >> 8) as u8,
        ]
        .into_iter()
        .enumerate()
        {
            write(p, base + i as u16, value);
        }
        byte(p, M::Lda, 1 << channel);
        abs(p, M::Sta, 0x420b);
        Ok(())
    }
}
