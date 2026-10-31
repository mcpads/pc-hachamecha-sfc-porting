use crate::Result;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Space {
    Rom,
    Wram,
    Vram,
    Cgram,
    Oam,
    SoundRam,
    Sram,
}
struct Reservation {
    space: Space,
    owner: String,
    offset: usize,
    bytes: usize,
}
pub struct MemoryMap {
    rom_bytes: usize,
    sram_bytes: usize,
    mapping: crate::cartridge::Mapping,
    reservations: Vec<Reservation>,
}
impl MemoryMap {
    pub fn from_report(
        report: &serde_json::Value,
        rom_bytes: usize,
        sram_bytes: usize,
    ) -> Result<Self> {
        Self::from_report_for_mapping(
            report,
            crate::cartridge::Mapping::for_size(rom_bytes)?,
            rom_bytes,
            sram_bytes,
        )
    }
    pub fn from_report_for_mapping(
        report: &serde_json::Value,
        mapping: crate::cartridge::Mapping,
        rom_bytes: usize,
        sram_bytes: usize,
    ) -> Result<Self> {
        let mut map = Self::new_for_mapping(mapping, rom_bytes, sram_bytes)?;
        let spaces = [
            Space::Rom,
            Space::Wram,
            Space::Vram,
            Space::Cgram,
            Space::Oam,
            Space::SoundRam,
            Space::Sram,
        ];
        for section in report["spaces"].as_array().ok_or("memory_map.report")? {
            let space = spaces
                .iter()
                .find(|s| section["space"] == format!("{s:?}"))
                .copied()
                .ok_or("memory_map.report_space")?;
            for entry in section["reservations"]
                .as_array()
                .ok_or("memory_map.report")?
            {
                let number = |name: &str| {
                    entry[name]
                        .as_u64()
                        .and_then(|n| usize::try_from(n).ok())
                        .ok_or("memory_map.report_extent")
                };
                map.reserve(
                    space,
                    entry["owner"].as_str().ok_or("memory_map.report_owner")?,
                    number("offset")?,
                    number("bytes")?,
                )?;
            }
        }
        if map.report() != *report {
            return Err("memory_map.report_mismatch".into());
        }
        Ok(map)
    }
    pub fn new(rom_bytes: usize, sram_bytes: usize) -> Result<Self> {
        Self::new_for_mapping(
            crate::cartridge::Mapping::for_size(rom_bytes)?,
            rom_bytes,
            sram_bytes,
        )
    }
    pub fn new_for_mapping(
        mapping: crate::cartridge::Mapping,
        rom_bytes: usize,
        sram_bytes: usize,
    ) -> Result<Self> {
        if mapping.validate_size(rom_bytes).is_err()
            || ![0, 2048, 8192, 32768].contains(&sram_bytes)
        {
            return Err("memory_map.cartridge_capacity".into());
        }
        Ok(Self {
            rom_bytes,
            sram_bytes,
            mapping,
            reservations: vec![],
        })
    }
    fn capacity(&self, space: Space) -> usize {
        match space {
            Space::Rom => self.rom_bytes,
            Space::Wram => 0x20000,
            Space::Vram => 0x10000,
            Space::Cgram => 512,
            Space::Oam => 544,
            Space::SoundRam => 0x10000,
            Space::Sram => self.sram_bytes,
        }
    }
    pub fn reserve(
        &mut self,
        space: Space,
        owner: &str,
        offset: usize,
        bytes: usize,
    ) -> Result<()> {
        let end = offset.checked_add(bytes).ok_or("memory_map.extent")?;
        if owner.is_empty() || bytes == 0 || end > self.capacity(space) {
            return Err("memory_map.extent".into());
        }
        if self
            .reservations
            .iter()
            .any(|r| r.space == space && r.owner == owner)
        {
            return Err("memory_map.duplicate_owner".into());
        }
        if self
            .reservations
            .iter()
            .any(|r| r.space == space && offset < r.offset + r.bytes && r.offset < end)
        {
            return Err("memory_map.overlap".into());
        }
        self.reservations.push(Reservation {
            space,
            owner: owner.into(),
            offset,
            bytes,
        });
        Ok(())
    }
    pub fn require_reserved(&self, space: Space, offset: usize, bytes: usize) -> Result<()> {
        let end = offset.checked_add(bytes).ok_or("memory_map.extent")?;
        if bytes == 0 || end > self.capacity(space) {
            return Err("memory_map.extent".into());
        }
        let mut entries = self
            .reservations
            .iter()
            .filter(|r| r.space == space)
            .collect::<Vec<_>>();
        entries.sort_by_key(|r| r.offset);
        let mut cursor = offset;
        for r in entries {
            if r.offset > cursor {
                break;
            }
            if cursor < r.offset + r.bytes {
                cursor = r.offset + r.bytes;
                if cursor >= end {
                    return Ok(());
                }
            }
        }
        Err("memory_map.unreserved_access".into())
    }
    pub fn report(&self) -> serde_json::Value {
        let spaces = [
            Space::Rom,
            Space::Wram,
            Space::Vram,
            Space::Cgram,
            Space::Oam,
            Space::SoundRam,
            Space::Sram,
        ]
        .map(|space| {
            let mut entries = self
                .reservations
                .iter()
                .filter(|r| r.space == space)
                .collect::<Vec<_>>();
            entries.sort_by_key(|r| r.offset);
            let mut cursor = 0;
            let mut unclaimed = vec![];
            for r in &entries {
                if cursor < r.offset {
                    unclaimed.push(serde_json::json!(
                        { "offset" : cursor, "bytes" : r.offset - cursor }
                    ));
                }
                cursor = r.offset + r.bytes;
            }
            if cursor < self.capacity(space) {
                unclaimed.push(serde_json::json!(
                    { "offset" : cursor, "bytes" : self.capacity(space) - cursor
                    }
                ));
            }
            serde_json::json!(
                { "space" : format!("{space:?}"), "capacity_bytes" : self
                .capacity(space), "reserved_bytes" : entries.iter().map(| r | r
                .bytes).sum::< usize > (), "reservations" : entries.iter().map(| r |
                serde_json::json!({ "owner" : r.owner, "offset" : r.offset, "bytes" :
                r.bytes })).collect::< Vec < _ >> (), "unclaimed" : unclaimed,
                "coverage" :
                "declared reservations only; unclaimed is not proven unused" }
            )
        });
        serde_json::json!(
            { "schema" : "sfc-memory-map-v1", "offset_unit" : "physical_bytes", "spaces"
            : spaces, "complete_access_coverage" : false, "rom_banks" : (0..self
            .rom_bytes).step_by(0x8000).map(| offset | { serde_json::json!({ "offset" :
            offset, "bytes" : (self.rom_bytes - offset).min(0x8000),
            "canonical_cpu_start" : self.mapping.cpu_address(offset).ok() }) })
            .collect::< Vec < _ >> () }
        )
    }
}
