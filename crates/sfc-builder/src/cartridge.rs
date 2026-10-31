use crate::Result;
pub const ROM_SIZE: usize = 0x8000;
pub const HEADER: usize = 0x7fc0;
pub fn frame(code: &[u8], reset: u16, nmi: u16, irq: u16) -> Result<Vec<u8>> {
    CartridgeLayout::new(ROM_SIZE, 0x7fb0, vec![])?.build(code, reset, nmi, irq)
}
#[derive(Clone)]
pub struct Section {
    pub name: String,
    pub offset: usize,
    pub bytes: Vec<u8>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mapping {
    LoRom,
    HiRom,
    ExHiRom,
}
impl Mapping {
    pub(crate) fn for_size(size: usize) -> Result<Self> {
        if (0x8000..=0x400000).contains(&size) && size.is_power_of_two() {
            Ok(Self::LoRom)
        } else if matches!(size, 0x600000 | 0x800000) {
            Ok(Self::ExHiRom)
        } else {
            Err("cartridge.capacity".into())
        }
    }
    pub fn validate_size(self, size: usize) -> Result<()> {
        let valid = match self {
            Self::LoRom => (0x8000..=0x400000).contains(&size) && size.is_power_of_two(),
            Self::HiRom => size == 0x400000,
            Self::ExHiRom => matches!(size, 0x600000 | 0x800000),
        };
        if valid {
            Ok(())
        } else {
            Err("cartridge.capacity".into())
        }
    }
    fn mode(self) -> u8 {
        match self {
            Self::LoRom => 0x20,
            Self::HiRom => 0x21,
            Self::ExHiRom => 0x25,
        }
    }
    pub fn from_rom(rom: &[u8]) -> Result<Self> {
        let mut found = None;
        for mapping in [Self::LoRom, Self::HiRom, Self::ExHiRom] {
            if verify_mapping(rom, mapping).is_ok() {
                if found.is_some() {
                    return Err("cartridge.ambiguous_mapping".into());
                }
                found = Some(mapping);
            }
        }
        match found {
            Some(mapping) => Ok(mapping),
            None => Err(verify_mapping(rom, Self::for_size(rom.len())?).unwrap_err()),
        }
    }
    pub fn header_offset(self) -> usize {
        match self {
            Self::LoRom => HEADER,
            Self::HiRom => 0xffc0,
            Self::ExHiRom => 0x40ffc0,
        }
    }
    pub fn code_offset(self) -> usize {
        match self {
            Self::LoRom => 0,
            Self::HiRom => 0x8000,
            Self::ExHiRom => 0x408000,
        }
    }
    pub(crate) fn reservations(self) -> Vec<(usize, usize)> {
        match self {
            Self::LoRom => vec![(0x7fb0, 0x8000)],
            Self::HiRom => vec![(0xffb0, 0x10000)],
            Self::ExHiRom => {
                vec![
                    (0xffb0, 0x10000),
                    (0x40ffb0, 0x410000),
                    (0x7e0000, 0x7e8000),
                    (0x7f0000, 0x7f8000),
                ]
            }
        }
    }
    pub fn cpu_address(self, offset: usize) -> Result<u32> {
        match self {
            Self::LoRom if offset < 0x400000 => {
                Ok((((offset / 0x8000) as u32 + 0x80) << 16) | 0x8000 | (offset as u32 & 0x7fff))
            }
            Self::HiRom | Self::ExHiRom if offset < 0x400000 => Ok(0xc00000 | offset as u32),
            Self::ExHiRom if offset < 0x7e0000 => Ok(offset as u32),
            Self::ExHiRom if offset < 0x800000 && offset & 0x8000 != 0 => {
                Ok((offset as u32) & 0x3fffff)
            }
            _ => Err("cartridge.unmapped_rom_offset".into()),
        }
    }
    pub fn rom_offset(self, address: u32) -> Result<usize> {
        let bank = address >> 16;
        let pc = address & 0xffff;
        if address > 0xffffff || matches!(bank, 0x7e | 0x7f) {
            return Err("cartridge.not_rom_address".into());
        }
        let offset = match self {
            Self::LoRom if pc >= 0x8000 => ((bank & 0x7f) * 0x8000) + (pc & 0x7fff),
            Self::HiRom if bank >= 0xc0 || (0x40..=0x7d).contains(&bank) || pc >= 0x8000 => {
                address & 0x3fffff
            }
            Self::ExHiRom if bank >= 0xc0 => address & 0x3fffff,
            Self::ExHiRom if bank >= 0x80 && pc >= 0x8000 => address & 0x3fffff,
            Self::ExHiRom if (0x40..=0x7d).contains(&bank) => address,
            Self::ExHiRom if bank <= 0x3f && pc >= 0x8000 => address | 0x400000,
            _ => return Err("cartridge.not_rom_address".into()),
        };
        Ok(offset as usize)
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RomSpeed {
    #[default]
    Slow,
    Fast,
}
impl RomSpeed {
    fn header_bit(self) -> u8 {
        match self {
            Self::Slow => 0,
            Self::Fast => 0x10,
        }
    }
}
#[derive(Clone)]
pub struct CartridgeLayout {
    size: usize,
    mapping: Mapping,
    code_capacity: usize,
    sections: Vec<Section>,
    battery: Option<BatteryRam>,
    title: [u8; 21],
    rom_speed: RomSpeed,
}
#[derive(Clone, Copy)]
pub struct BatteryRam {
    bytes: usize,
    mapping: Mapping,
}
impl BatteryRam {
    pub fn new(bytes: usize) -> Result<Self> {
        if ![2048, 8192, 32768].contains(&bytes) {
            return Err("cartridge.battery_capacity: require 2, 8 or 32 KiB".into());
        }
        Ok(Self {
            bytes,
            mapping: Mapping::LoRom,
        })
    }
    pub fn new_hirom(bytes: usize) -> Result<Self> {
        let mut ram = Self::new(bytes)?;
        ram.mapping = Mapping::HiRom;
        Ok(ram)
    }
    pub fn new_exhirom(bytes: usize) -> Result<Self> {
        let mut ram = Self::new(bytes)?;
        ram.mapping = Mapping::ExHiRom;
        Ok(ram)
    }
    pub fn contiguous_bytes(self, offset: usize) -> Result<usize> {
        if offset >= self.bytes {
            return Err("cartridge.battery_extent".into());
        }
        let window = if self.mapping == Mapping::LoRom {
            0x8000
        } else {
            0x2000
        };
        Ok((window - offset % window).min(self.bytes - offset))
    }
    pub fn bytes(self) -> usize {
        self.bytes
    }
    pub fn cpu_address(self, offset: usize, bytes: usize) -> Result<u32> {
        if bytes == 0 || offset.checked_add(bytes).is_none_or(|end| end > self.bytes) {
            return Err("cartridge.battery_extent".into());
        }
        if bytes > self.contiguous_bytes(offset)? {
            return Err("cartridge.battery_crosses_window".into());
        }
        Ok(match self.mapping {
            Mapping::LoRom => 0x700000 + offset as u32,
            Mapping::HiRom => {
                0x206000 + ((offset / 0x2000) as u32 * 0x10000) + (offset % 0x2000) as u32
            }
            Mapping::ExHiRom => {
                0xa06000 + ((offset / 0x2000) as u32 * 0x10000) + (offset % 0x2000) as u32
            }
        })
    }
}
impl CartridgeLayout {
    pub fn new(size: usize, code_capacity: usize, sections: Vec<Section>) -> Result<Self> {
        if !(0x8000..=0x400000).contains(&size) || !size.is_power_of_two() {
            return Err("cartridge.capacity: require power-of-two 32 KiB..4 MiB".into());
        }
        Self::with_mapping(size, code_capacity, sections, Mapping::LoRom)
    }
    pub fn new_hirom(size: usize, code_capacity: usize, sections: Vec<Section>) -> Result<Self> {
        Mapping::HiRom.validate_size(size)?;
        Self::with_mapping(size, code_capacity, sections, Mapping::HiRom)
    }
    pub fn new_exhirom(size: usize, code_capacity: usize, sections: Vec<Section>) -> Result<Self> {
        if !matches!(size, 0x600000 | 0x800000) {
            return Err("cartridge.exhirom_capacity: require 6 or 8 MiB".into());
        }
        Self::with_mapping(size, code_capacity, sections, Mapping::ExHiRom)
    }
    fn with_mapping(
        size: usize,
        code_capacity: usize,
        sections: Vec<Section>,
        mapping: Mapping,
    ) -> Result<Self> {
        if code_capacity == 0 || code_capacity > 0x7fb0 {
            return Err("cartridge.code_reservation".into());
        }
        Self::validate_sections(size, code_capacity, mapping, sections.iter())?;
        Ok(Self {
            size,
            mapping,
            code_capacity,
            sections,
            battery: None,
            title: *b"MADOU SFC CARTRIDGE  ",
            rom_speed: RomSpeed::Slow,
        })
    }
    fn validate_sections<'a>(
        size: usize,
        code_capacity: usize,
        mapping: Mapping,
        sections: impl IntoIterator<Item = &'a Section>,
    ) -> Result<()> {
        let mut occupied = mapping.reservations();
        occupied.push((mapping.code_offset(), mapping.code_offset() + code_capacity));
        let mut names = std::collections::BTreeSet::new();
        for section in sections {
            let end = section
                .offset
                .checked_add(section.bytes.len())
                .ok_or("cartridge.section_overflow")?;
            if section.name.is_empty()
                || !names.insert(section.name.clone())
                || section.bytes.is_empty()
                || end > size
            {
                return Err("cartridge.section_identity_or_extent".into());
            }
            if occupied
                .iter()
                .any(|&(start, stop)| section.offset < stop && start < end)
            {
                return Err("cartridge.section_overlap".into());
            }
            occupied.push((section.offset, end));
        }
        Ok(())
    }
    pub fn add_sections(&mut self, sections: Vec<Section>) -> Result<()> {
        Self::validate_sections(
            self.size,
            self.code_capacity,
            self.mapping,
            self.sections.iter().chain(&sections),
        )?;
        self.sections.extend(sections);
        Ok(())
    }
    pub fn plan_code_addresses(&self, sizes: &[usize], alignment: usize) -> Result<Vec<u32>> {
        if sizes.is_empty() || !alignment.is_power_of_two() || alignment > 0x10000 {
            return Err("cartridge.code_requests_or_alignment".into());
        }
        let max_bytes = match self.mapping {
            Mapping::LoRom => 0x8000,
            Mapping::HiRom | Mapping::ExHiRom => 0x10000,
        };
        if sizes.iter().any(|&bytes| bytes == 0 || bytes > max_bytes) {
            return Err("cartridge.code_extent".into());
        }
        let mut occupied = self.mapping.reservations();
        occupied.push((
            self.mapping.code_offset(),
            self.mapping.code_offset() + self.code_capacity,
        ));
        occupied.extend(
            self.sections
                .iter()
                .map(|s| (s.offset, s.offset + s.bytes.len())),
        );
        let mut addresses = Vec::new();
        for &bytes in sizes {
            let mut offset = 0usize;
            loop {
                if offset >= self.size {
                    return Err("cartridge.code_space_exhausted".into());
                }
                if let Some(&(_, end)) = occupied
                    .iter()
                    .find(|&&(start, end)| start <= offset && offset < end)
                {
                    offset = end;
                    continue;
                }
                let address = self.cpu_address(offset)?;
                let window = self.source_window_remaining(offset)?;
                let padding = (alignment - (address as usize & (alignment - 1))) & (alignment - 1);
                if padding + bytes > window {
                    offset += window;
                    continue;
                }
                offset += padding;
                let end = offset + bytes;
                if let Some(&(_, stop)) = occupied
                    .iter()
                    .find(|&&(start, stop)| offset < stop && start < end)
                {
                    offset = stop;
                    continue;
                }
                addresses.push(self.cpu_address(offset)?);
                occupied.push((offset, end));
                break;
            }
        }
        Ok(addresses)
    }
    pub fn set_title(&mut self, title: &str) -> Result<()> {
        if title.is_empty() || title.len() > 21 {
            return Err("cartridge.title_length: require 1..=21 bytes".into());
        }
        if !title.bytes().all(|byte| (0x20..=0x7e).contains(&byte)) {
            return Err("cartridge.title_encoding: require printable ASCII".into());
        }
        let mut encoded = [b' '; 21];
        encoded[..title.len()].copy_from_slice(title.as_bytes());
        self.title = encoded;
        Ok(())
    }
    pub fn title(&self) -> &[u8; 21] {
        &self.title
    }
    pub fn battery_ram(&self) -> Result<BatteryRam> {
        self.check_battery()?;
        self.battery
            .ok_or_else(|| "cartridge.battery_absent".into())
    }
    pub fn with_battery(mut self, battery: BatteryRam) -> Self {
        self.battery = Some(battery);
        self
    }
    pub fn set_rom_speed(&mut self, speed: RomSpeed) {
        self.rom_speed = speed;
    }
    pub fn rom_speed(&self) -> RomSpeed {
        self.rom_speed
    }
    pub fn mapping(&self) -> Mapping {
        self.mapping
    }
    pub fn size(&self) -> usize {
        self.size
    }
    pub fn section(&self, name: &str) -> Result<&Section> {
        self.sections
            .iter()
            .find(|s| s.name == name)
            .ok_or_else(|| "cartridge.unknown_section".into())
    }
    pub fn section_report(&self) -> serde_json::Value {
        self.sections
            .iter()
            .map(|s| {
                serde_json::json!(
                    { "name" : s.name, "offset" : s.offset, "bytes" : s.bytes.len(),
                    "sha256" : crate ::sha256(& s.bytes) }
                )
            })
            .collect::<Vec<_>>()
            .into()
    }
    pub fn memory_map(&self) -> Result<crate::memory_map::MemoryMap> {
        use crate::memory_map::{MemoryMap, Space};
        self.check_battery()?;
        let mut map = MemoryMap::new_for_mapping(
            self.mapping,
            self.size,
            self.battery.map_or(0, |ram| ram.bytes),
        )?;
        map.reserve(
            Space::Rom,
            "first_bank_code_capacity",
            self.mapping.code_offset(),
            self.code_capacity,
        )?;
        for (i, (start, end)) in self.mapping.reservations().into_iter().enumerate() {
            if start < self.size {
                let owner = if i == 0 && self.mapping == Mapping::LoRom {
                    "header_and_vectors".into()
                } else {
                    format!("mapping_reserved:{i}")
                };
                map.reserve(Space::Rom, &owner, start, end - start)?;
            }
        }
        for section in &self.sections {
            map.reserve(
                Space::Rom,
                &format!("section:{}", section.name),
                section.offset,
                section.bytes.len(),
            )?;
        }
        Ok(map)
    }
    pub fn cpu_address(&self, offset: usize) -> Result<u32> {
        if offset >= self.size {
            return Err("cartridge.address_outside_image".into());
        }
        self.mapping.cpu_address(offset)
    }
    pub fn source_window_remaining(&self, offset: usize) -> Result<usize> {
        let address = self.cpu_address(offset)?;
        Ok((0x10000 - (address as usize & 0xffff)).min(self.size - offset))
    }
    fn check_battery(&self) -> Result<()> {
        if self.battery.is_some_and(|ram| ram.mapping != self.mapping) {
            return Err("cartridge.battery_mapping_mismatch".into());
        }
        Ok(())
    }
    pub fn build(&self, code: &[u8], reset: u16, nmi: u16, irq: u16) -> Result<Vec<u8>> {
        self.check_battery()?;
        if code.is_empty() || code.len() > self.code_capacity {
            return Err("cartridge.code_extent: empty code or reservation exceeded".into());
        }
        for address in [reset, nmi, irq] {
            if address < 0x8000 || usize::from(address - 0x8000) >= code.len() {
                return Err("cartridge.vector_outside_code".into());
            }
        }
        let mut rom = vec![0xff; self.size];
        let base = self.mapping.code_offset();
        let header = self.mapping.header_offset();
        rom[base..base + code.len()].copy_from_slice(code);
        rom[header - 16..header + 64].fill(0);
        for section in &self.sections {
            rom[section.offset..section.offset + section.bytes.len()]
                .copy_from_slice(&section.bytes);
        }
        rom[header..header + 21].copy_from_slice(&self.title);
        rom[header + 0x15] = self.rom_speed.header_bit() | self.mapping.mode();
        rom[header + 0x16] = if self.battery.is_some() { 2 } else { 0 };
        rom[header + 0x17] = self.size.next_power_of_two().trailing_zeros() as u8 - 10;
        rom[header + 0x18] = self
            .battery
            .map_or(0, |ram| ram.bytes.trailing_zeros() as u8 - 10);
        rom[header + 0x19] = 1;
        for (offset, address) in [
            (0x7fe4, irq),
            (0x7fe6, irq),
            (0x7fe8, irq),
            (0x7fea, nmi),
            (0x7fee, irq),
            (0x7ff4, irq),
            (0x7ff8, irq),
            (0x7ffa, nmi),
            (0x7ffc, reset),
            (0x7ffe, irq),
        ] {
            let offset = header + (offset - HEADER);
            rom[offset..offset + 2].copy_from_slice(&address.to_le_bytes());
        }
        rom[header + 0x1c..header + 0x20].copy_from_slice(&[0xff, 0xff, 0, 0]);
        if self.mapping == Mapping::ExHiRom {
            rom.copy_within(0x40ffb0..0x410000, 0xffb0);
        }
        let checksum = checksum(&rom);
        rom[header + 0x1c..header + 0x1e].copy_from_slice(&(!checksum).to_le_bytes());
        rom[header + 0x1e..header + 0x20].copy_from_slice(&checksum.to_le_bytes());
        if self.mapping == Mapping::ExHiRom {
            rom.copy_within(0x40ffb0..0x410000, 0xffb0);
        }
        verify(&rom)?;
        Ok(rom)
    }
}
fn checksum(rom: &[u8]) -> u16 {
    let sum = |bytes: &[u8]| {
        bytes
            .iter()
            .fold(0u16, |sum, byte| sum.wrapping_add(u16::from(*byte)))
    };
    let total = sum(rom);
    if rom.len() == 0x600000 {
        total.wrapping_add(sum(&rom[0x400000..]))
    } else {
        total
    }
}
pub fn verify(rom: &[u8]) -> Result<()> {
    Mapping::from_rom(rom).map(|_| ())
}
fn verify_mapping(rom: &[u8], mapping: Mapping) -> Result<()> {
    mapping.validate_size(rom.len())?;
    let h = mapping.header_offset();
    let mode = mapping.mode();
    if rom[h + 0x15] & !0x10 != mode
        || rom[h + 0x17] != rom.len().next_power_of_two().trailing_zeros() as u8 - 10
    {
        return Err("cartridge.profile_mismatch".into());
    }
    if !matches!((rom[h + 0x16], rom[h + 0x18]), (0, 0) | (2, 1 | 3 | 5)) {
        return Err("cartridge.battery_profile_mismatch".into());
    }
    if mapping == Mapping::ExHiRom && rom[0xffb0..0x10000] != rom[0x40ffb0..0x410000] {
        return Err("cartridge.header_mirror_mismatch".into());
    }
    let stored = u16::from_le_bytes([rom[h + 0x1e], rom[h + 0x1f]]);
    let inverse = u16::from_le_bytes([rom[h + 0x1c], rom[h + 0x1d]]);
    if stored != checksum(rom) || stored ^ inverse != 0xffff {
        return Err("cartridge.checksum_mismatch".into());
    }
    Ok(())
}
