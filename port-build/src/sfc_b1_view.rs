mod records;
use crate::Result;
use std::collections::HashMap;
const BANK: usize = 0x8000;
const TILE: usize = 64;
const UNIT: usize = 3;
const ENTRY: usize = 4;
const BLOCK: usize = 4;
pub(crate) const VIEW_KIND: u8 = 3;
pub(crate) const EXPERIENCE_KIND: u8 = 4;
const CHANNELS: [u8; 6] = [2, 3, 4, 5, 6, 7];
pub struct Table {
    name: &'static str,
    expected: Vec<Vec<u8>>,
    blocks: Vec<Vec<usize>>,
    units: Vec<Vec<u8>>,
}
impl Table {
    pub fn new(name: &'static str, blocks: &[Vec<&[u8]>]) -> Result<Table> {
        let shape = |b: &Vec<&[u8]>| b.iter().map(|s| s.len()).collect::<Vec<_>>();
        if blocks.is_empty()
            || blocks.iter().any(|b| shape(b) != shape(&blocks[0]))
            || blocks[0]
                .iter()
                .any(|s| s.is_empty() || s.len() % TILE != 0)
        {
            return Err("Unit table blocks differ in shape".into());
        }
        let mut units: Vec<Vec<u8>> = Vec::new();
        let mut ids: HashMap<&[u8], usize> = HashMap::new();
        let unit_blocks = blocks
            .iter()
            .map(|block| {
                block
                    .iter()
                    .flat_map(|segment| segment.chunks(UNIT * TILE))
                    .map(|unit| {
                        *ids.entry(unit).or_insert_with(|| {
                            units.push(unit.to_vec());
                            units.len() - 1
                        })
                    })
                    .collect()
            })
            .collect();
        Ok(Table {
            name,
            expected: blocks.iter().map(|b| b.concat()).collect(),
            blocks: unit_blocks,
            units,
        })
    }
    pub fn block_bytes(&self) -> usize {
        self.expected[0].len()
    }
    pub fn report(&self) -> serde_json::Value {
        serde_json::json!(
            { "tiles_per_unit" : UNIT, "units" : self.units.len(), "unit_bytes" : self
            .units.iter().map(Vec::len).sum::< usize > (), "blocks" : self.blocks.len(),
            "record_bytes" : self.blocks.iter().map(| b | b.len() * ENTRY).sum::< usize >
            () }
        )
    }
    pub fn append(
        &self,
        sections: &mut Vec<sfc_builder::cartridge::Section>,
        space: &mut crate::sfc_rom::Space,
    ) -> Result<Vec<u32>> {
        let units = place(
            sections,
            space,
            &format!("{}_units", self.name),
            &self.units,
        )?;
        let records: Vec<Vec<u8>> = self
            .blocks
            .iter()
            .map(|block| {
                block
                    .iter()
                    .flat_map(|u| {
                        let mut entry = units[*u].to_le_bytes()[..3].to_vec();
                        entry.push(self.units[*u].len() as u8);
                        entry
                    })
                    .collect()
            })
            .collect();
        let addresses = place(sections, space, &format!("{}_records", self.name), &records)?;
        let read = |address: u32, length: usize| -> Result<Vec<u8>> {
            let offset = crate::sfc_rom::MAPPING.rom_offset(address)?;
            let section = sections
                .iter()
                .find(|s| (s.offset..s.offset + s.bytes.len()).contains(&offset))
                .ok_or("Unit address outside sections")?;
            let at = offset - section.offset;
            Ok(section
                .bytes
                .get(at..at + length)
                .ok_or("Unit read")?
                .to_vec())
        };
        for ((block, units), address) in self.expected.iter().zip(&self.blocks).zip(&addresses) {
            let mut sent = Vec::new();
            for entry in read(*address, units.len() * ENTRY)?.chunks(ENTRY) {
                let source = u32::from_le_bytes([entry[0], entry[1], entry[2], 0]);
                sent.extend(read(source, usize::from(entry[3]))?);
            }
            if &sent != block {
                return Err("Unit record does not rebuild its block".into());
            }
        }
        Ok(addresses)
    }
}
pub fn descriptor(kind: u8, address: usize, bytes: usize, record: u32) -> Result<Vec<u8>> {
    if bytes + usize::from(crate::sfc_b1::DESCRIPTOR_COST)
        > usize::from(crate::sfc_b1::STREAM_BUDGET)
    {
        return Err("View or experience descriptor exceeds the stream budget".into());
    }
    let mut list = vec![kind];
    list.extend(u16::try_from(address / 2)?.to_le_bytes());
    list.extend(u16::try_from(bytes)?.to_le_bytes());
    list.extend(&record.to_le_bytes()[..3]);
    Ok(list)
}
pub struct TileTable {
    expected: Vec<Vec<u8>>,
    records: Vec<Vec<u8>>,
    bases: Vec<[u16; BLOCK_TILES]>,
    chunks: Vec<Vec<Vec<u8>>>,
    high_ids: Vec<Option<u16>>,
    highs: Vec<Vec<u8>>,
}
const HALF: usize = TILE / 2;
const CHUNK: usize = BANK / HALF;
const HIGH_CHUNK: usize = CHUNK - 1;
pub(crate) const BLOCK_TILES: usize = 52;
const DELTA: u8 = 0x80;
const DELTA_HEAD: usize = 6;
const BASES: usize = 128;
pub(crate) const STAGING: u32 = 0x7f4000;
pub(crate) const STAGED: u16 = 0x6d4;
impl TileTable {
    pub fn new(blocks: &[(usize, Vec<&[u8]>)]) -> Result<Self> {
        let mut places: HashMap<(usize, usize), Vec<&[u8]>> = HashMap::new();
        let mut indices = Vec::new();
        for (slot, block) in blocks {
            let tiles: Vec<&[u8]> = block.iter().flat_map(|row| row.chunks(TILE)).collect();
            if tiles.len() != BLOCK_TILES {
                return Err("View block tile count".into());
            }
            let list = tiles
                .iter()
                .enumerate()
                .map(|(at, tile)| {
                    let place = places.entry((*slot, at)).or_default();
                    match place.iter().position(|t| t == tile) {
                        Some(i) => i,
                        None => {
                            place.push(tile);
                            place.len() - 1
                        }
                    }
                })
                .collect::<Vec<_>>();
            indices.push(list);
        }
        let mut keys: Vec<(usize, usize)> = places.keys().copied().collect();
        keys.sort_by_key(|k| (std::cmp::Reverse(places[k].len()), *k));
        let mut chunks: Vec<Vec<Vec<u8>>> = Vec::new();
        let mut first = HashMap::new();
        for key in keys {
            let place = &places[&key];
            if place.len() > CHUNK {
                return Err("View tile place exceeds a chunk".into());
            }
            let chunk = match chunks.iter().position(|c| c.len() + place.len() <= CHUNK) {
                Some(c) => c,
                None => {
                    chunks.push(Vec::new());
                    chunks.len() - 1
                }
            };
            first.insert(key, chunk * CHUNK + chunks[chunk].len());
            chunks[chunk].extend(place.iter().map(|t| t[..HALF].to_vec()));
        }
        let mut high_ids = vec![None; chunks.len() * CHUNK];
        let mut highs: Vec<Vec<u8>> = Vec::new();
        let mut high_of: HashMap<&[u8], u16> = HashMap::new();
        let mut keys: Vec<&(usize, usize)> = places.keys().collect();
        keys.sort();
        for key in keys {
            for (i, tile) in places[key].iter().enumerate() {
                let next = u16::try_from(highs.len())?;
                let high = *high_of.entry(&tile[HALF..]).or_insert_with(|| {
                    highs.push(tile[HALF..].to_vec());
                    next
                });
                high_ids[first[key] + i] = Some(high);
            }
        }
        if high_ids.len() > 0x8000 {
            return Err("View tiles exceed the high-address table's bank".into());
        }
        let mut bases: Vec<(usize, [u16; BLOCK_TILES])> = Vec::new();
        let mut records = Vec::new();
        for ((slot, _), list) in blocks.iter().zip(&indices) {
            let ids: Vec<usize> = list
                .iter()
                .enumerate()
                .map(|(at, i)| first[&(*slot, at)] + i)
                .collect();
            let fits = |base: &[u16; BLOCK_TILES]| {
                ids.iter()
                    .zip(base)
                    .all(|(&id, &b)| id >= usize::from(b) && id - usize::from(b) < 256)
            };
            let selector = match bases.iter().position(|(s, b)| s == slot && fits(b)) {
                Some(i) => i,
                None => {
                    let mut base = [0; BLOCK_TILES];
                    for (at, b) in base.iter_mut().enumerate() {
                        let start = first[&(*slot, at)];
                        *b = u16::try_from(start.max(ids[at].saturating_sub(255)))?;
                    }
                    bases.push((*slot, base));
                    bases.len() - 1
                }
            };
            let base = &bases[selector].1;
            let mut record = vec![u8::try_from(selector)?];
            record.extend(
                ids.iter()
                    .zip(base)
                    .map(|(&id, &b)| (id - usize::from(b)) as u8),
            );
            records.push(record);
        }
        if bases.len() >= usize::from(DELTA) || bases.len() * BASES > 0x10000 {
            return Err("View base tables exceed a bank".into());
        }
        Ok(Self {
            expected: blocks.iter().map(|(_, b)| b.concat()).collect(),
            records,
            bases: bases.into_iter().map(|(_, b)| b).collect(),
            chunks,
            high_ids,
            highs,
        })
    }
    pub fn report(&self) -> serde_json::Value {
        let tiles: usize = self.chunks.iter().map(Vec::len).sum();
        serde_json::json!(
            { "tiles" : tiles, "low_bytes" : tiles * HALF, "low_chunks" : self.chunks
            .len(), "high_halves" : self.highs.len(), "high_bytes" : self.highs.len() *
            HALF, "high_table_bytes" : self.high_ids.len() * 3, "base_tables" : self
            .bases.len(), "blocks" : self.records.len(), "record_bytes" : self.records
            .iter().map(Vec::len).sum::< usize > () }
        )
    }
    pub fn append(
        &self,
        sections: &mut Vec<sfc_builder::cartridge::Section>,
        space: &mut crate::sfc_rom::Space,
    ) -> Result<Vec<u32>> {
        let place_chunk = |sections: &mut Vec<sfc_builder::cartridge::Section>,
                           space: &mut crate::sfc_rom::Space,
                           name: String,
                           bytes: Vec<u8>|
         -> Result<u8> {
            let offset = space.reserve_upper_window(bytes.len())?;
            let at = crate::sfc_rom::cpu(offset)?;
            if at & 0xffff != 0x8000 {
                return Err("View tile chunk is not bank aligned".into());
            }
            sections.push(sfc_builder::cartridge::Section {
                name,
                offset,
                bytes,
            });
            Ok((at >> 16) as u8)
        };
        let mut low_banks = Vec::new();
        for (i, chunk) in self.chunks.iter().enumerate() {
            low_banks.push(place_chunk(
                sections,
                space,
                format!("b1_view_lows_{i}"),
                chunk.concat(),
            )?);
        }
        let mut high_banks = Vec::new();
        for (i, chunk) in self.highs.chunks(HIGH_CHUNK).enumerate() {
            if i >= HALF {
                return Err("View high halves exceed the address's chunk bits".into());
            }
            let mut bytes = vec![0; i];
            bytes.extend(chunk.concat());
            high_banks.push(place_chunk(
                sections,
                space,
                format!("b1_view_highs_{i}"),
                bytes,
            )?);
        }
        let address = |id: usize| ((0x8000 + (id % CHUNK) * HALF) as u16).to_le_bytes();
        let high_address =
            |h: usize| ((0x8000 + h / HIGH_CHUNK + (h % HIGH_CHUNK) * HALF) as u16).to_le_bytes();
        let mut tables: Vec<(&str, Vec<u8>)> = vec![
            ("b1_view_high_offsets", Vec::new()),
            ("b1_view_high_banks", Vec::new()),
            ("b1_view_low_offsets", Vec::new()),
            ("b1_view_low_banks", Vec::new()),
            ("b1_view_high_index", Vec::new()),
        ];
        for high in &self.high_ids {
            tables[0]
                .1
                .extend(high_address(usize::from(high.unwrap_or(0))));
        }
        tables[1].1 = high_banks;
        for base in &self.bases {
            for &id in base {
                let id = usize::from(id);
                for (t, bytes) in [
                    (2, address(id)),
                    (3, [low_banks[id / CHUNK], 0]),
                    (4, u16::try_from(id * 2)?.to_le_bytes()),
                ] {
                    tables[t].1.extend(bytes);
                }
            }
            for (_, table) in &mut tables[2..5] {
                let len = table.len().next_multiple_of(BASES);
                table.resize(len, 0);
            }
        }
        for (name, bytes) in &tables {
            if *name == "b1_view_high_banks" {
                crate::sfc_b1::add(sections, space, (*name).into(), bytes.clone())?;
                continue;
            }
            let offset = space.reserve_table(bytes.len())?;
            sections.push(sfc_builder::cartridge::Section {
                name: (*name).into(),
                offset,
                bytes: bytes.clone(),
            });
        }
        let offset = space.reserve_fast(GATHER_BYTES)?;
        sections.push(sfc_builder::cartridge::Section {
            name: GATHER_SECTION.into(),
            offset,
            bytes: vec![0; GATHER_BYTES],
        });
        let parent = records::parents(&self.records, 11);
        let fulls: Vec<usize> = parent
            .iter()
            .enumerate()
            .filter_map(|(i, p)| p.is_none().then_some(i))
            .collect();
        let full_records: Vec<Vec<u8>> = fulls.iter().map(|&i| self.records[i].clone()).collect();
        let full_addresses = place(sections, space, "b1_view_records", &full_records)?;
        let mut addresses = vec![0; self.records.len()];
        for (&i, &at) in fulls.iter().zip(&full_addresses) {
            addresses[i] = at;
        }
        let mut masks: Vec<[u8; 8]> = Vec::new();
        let mut deltas = Vec::new();
        let mut delta_blocks = Vec::new();
        for (i, record) in self.records.iter().enumerate() {
            let Some(j) = parent[i] else { continue };
            let mut delta = vec![DELTA];
            delta.extend(&addresses[j].to_le_bytes()[..3]);
            let mut mask = [0u8; 8];
            let mut changed = Vec::new();
            for (at, (a, b)) in record[1..].iter().zip(&self.records[j][1..]).enumerate() {
                if a != b {
                    mask[at / 8] |= 1 << (at % 8);
                    changed.push(*a);
                }
            }
            let index = masks.iter().position(|m| m == &mask).unwrap_or_else(|| {
                masks.push(mask);
                masks.len() - 1
            });
            delta.extend(u16::try_from(index * 8)?.to_le_bytes());
            delta.extend(changed);
            deltas.push(delta);
            delta_blocks.push(i);
        }
        let mask_bytes = masks.concat();
        let offset = space.reserve_table(mask_bytes.len())?;
        sections.push(sfc_builder::cartridge::Section {
            name: "b1_view_masks".into(),
            offset,
            bytes: mask_bytes,
        });
        let mask_address = crate::sfc_rom::cpu(offset)?;
        for (&i, at) in delta_blocks
            .iter()
            .zip(place(sections, space, "b1_view_deltas", &deltas)?)
        {
            addresses[i] = at;
        }
        let word = |t: usize, at: usize| u16::from_le_bytes([tables[t].1[at], tables[t].1[at + 1]]);
        let read = |bank: u8, address: u16| -> Result<Vec<u8>> {
            let offset =
                crate::sfc_rom::MAPPING.rom_offset(u32::from(bank) << 16 | u32::from(address))?;
            let section = sections
                .iter()
                .find(|s| (s.offset..s.offset + s.bytes.len()).contains(&offset))
                .ok_or("Tile half outside sections")?;
            let at = offset - section.offset;
            Ok(section
                .bytes
                .get(at..at + HALF)
                .ok_or("Tile half read")?
                .to_vec())
        };
        let byte_at = |address: u32| -> Result<u8> {
            let offset = crate::sfc_rom::MAPPING.rom_offset(address)?;
            let section = sections
                .iter()
                .find(|s| (s.offset..s.offset + s.bytes.len()).contains(&offset))
                .ok_or("Record outside sections")?;
            Ok(section.bytes[offset - section.offset])
        };
        let effective = |address: u32| -> Result<Vec<u8>> {
            let head = byte_at(address)?;
            if head != DELTA {
                return (0..=BLOCK_TILES as u32)
                    .map(|i| byte_at(address + i))
                    .collect();
            }
            let parent = u32::from_le_bytes([
                byte_at(address + 1)?,
                byte_at(address + 2)?,
                byte_at(address + 3)?,
                0,
            ]);
            let mut record: Vec<u8> = (0..=BLOCK_TILES as u32)
                .map(|i| byte_at(parent + i))
                .collect::<Result<_>>()?;
            let mask_offset = u16::from_le_bytes([byte_at(address + 4)?, byte_at(address + 5)?]);
            let mut next = address + DELTA_HEAD as u32;
            for at in 0..BLOCK_TILES {
                if byte_at(mask_address + u32::from(mask_offset) + (at / 8) as u32)? >> (at % 8) & 1
                    != 0
                {
                    record[1 + at] = byte_at(next)?;
                    next += 1;
                }
            }
            Ok(record)
        };
        for ((block, record), &address) in self.expected.iter().zip(&self.records).zip(&addresses) {
            if &effective(address)? != record {
                return Err("View delta record does not rebuild its record".into());
            }
            let mut sent: Vec<u8> = Vec::new();
            for (at, &byte) in record[1..].iter().enumerate() {
                let x = usize::from(record[0]) * BASES + 2 * at;
                let low = word(2, x).wrapping_add(u16::from(byte) << 5);
                sent.extend(read(tables[3].1[x], low)?);
                let index = usize::from(word(4, x)) + 2 * usize::from(byte);
                let high = word(0, index);
                sent.extend(read(tables[1].1[usize::from(high) % HALF], high)?);
            }
            if &sent != block {
                return Err("Tile record does not rebuild its block".into());
            }
        }
        Ok(addresses)
    }
}
pub struct Assets {
    table: TileTable,
    views: Vec<Vec<usize>>,
    address: usize,
    pub report: serde_json::Value,
}
impl Assets {
    pub fn pack(
        rows: &[Vec<u8>],
        views: &[[usize; 16]],
        columns: usize,
        address: usize,
    ) -> Result<Assets> {
        if rows.iter().any(|r| r.len() != columns * TILE) {
            return Err("B1 view row width".into());
        }
        let mut blocks: Vec<(usize, Vec<&[u8]>)> = Vec::new();
        let mut block_ids: HashMap<&[usize], usize> = HashMap::new();
        let views = views
            .iter()
            .map(|view| {
                view.chunks(BLOCK)
                    .enumerate()
                    .map(|(slot, block)| {
                        *block_ids.entry(block).or_insert_with(|| {
                            blocks
                                .push((slot, block.iter().map(|r| rows[*r].as_slice()).collect()));
                            blocks.len() - 1
                        })
                    })
                    .collect()
            })
            .collect();
        let table = TileTable::new(&blocks)?;
        let mut report = table.report();
        report["rows_per_block"] = BLOCK.into();
        Ok(Assets {
            table,
            views,
            address,
            report,
        })
    }
    pub fn append(
        &self,
        sections: &mut Vec<sfc_builder::cartridge::Section>,
        space: &mut crate::sfc_rom::Space,
    ) -> Result<Vec<Vec<u8>>> {
        let addresses = self.table.append(sections, space)?;
        let bytes = self.table.expected[0].len();
        self.views
            .iter()
            .map(|view| {
                let mut list = Vec::new();
                for (i, block) in view.iter().enumerate() {
                    list.extend(descriptor(
                        VIEW_KIND,
                        self.address + i * bytes,
                        bytes,
                        addresses[*block],
                    )?);
                }
                Ok(list)
            })
            .collect()
    }
}
pub fn segment_units(bytes: usize) -> usize {
    (bytes / TILE).div_ceil(UNIT)
}
fn place(
    sections: &mut Vec<sfc_builder::cartridge::Section>,
    space: &mut crate::sfc_rom::Space,
    name: &str,
    items: &[Vec<u8>],
) -> Result<Vec<u32>> {
    let mut addresses = Vec::new();
    let mut rest = items;
    while !rest.is_empty() {
        let mut take = 0;
        let mut bytes = Vec::new();
        while take < rest.len() && bytes.len() + rest[take].len() <= BANK {
            bytes.extend(&rest[take]);
            take += 1;
        }
        let start =
            crate::sfc_b1::add(sections, space, format!("{name}_{}", sections.len()), bytes)?;
        let mut offset = 0;
        for item in &rest[..take] {
            addresses.push(start + offset);
            offset += item.len() as u32;
        }
        rest = &rest[take..];
    }
    Ok(addresses)
}
pub fn emit_block(p: &mut w65c816::Assembler, label: &str, units: usize) -> Result<()> {
    use crate::sfc::{emit, implied};
    use w65c816::{AddressingMode as A, CpuMode, Mnemonic as M, Operand as O};
    let dp = |p: &mut w65c816::Assembler, m, at: u8| emit(p, m, A::DirectPage, O::Byte(at));
    let m16 = |p: &mut w65c816::Assembler| emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    let m8 = |p: &mut w65c816::Assembler| emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    let field = |p: &mut w65c816::Assembler, index: u16| {
        emit(p, M::Ldy, A::Immediate, O::Word(index));
        emit(p, M::Lda, A::DirectPageIndirectLongIndexedY, O::Byte(0xe4));
    };
    p.label(label).assume_mode(CpuMode::NATIVE_M8_X16);
    implied(p, M::Phx);
    implied(p, M::Phb);
    implied(p, M::Phd);
    emit(p, M::Lda, A::Immediate, O::Byte(0x80));
    emit(p, M::Sta, A::Absolute, O::Word(0x2115));
    m16(p);
    field(p, 1);
    emit(p, M::Sta, A::Absolute, O::Word(0x2116));
    field(p, 5);
    implied(p, M::Tax);
    m8(p);
    field(p, 7);
    implied(p, M::Pha);
    implied(p, M::Plb);
    m16(p);
    emit(p, M::Lda, A::Immediate, O::Word(0x4300));
    implied(p, M::Tcd);
    let units: Vec<usize> = (0..units).collect();
    for group in units.chunks(CHANNELS.len()) {
        m16(p);
        let mut mask = 0u8;
        for (unit, channel) in group.iter().zip(CHANNELS) {
            let entry = u16::try_from(unit * ENTRY)?;
            emit(p, M::Lda, A::AbsoluteX, O::Word(entry));
            dp(p, M::Sta, channel * 16 + 2);
            emit(p, M::Lda, A::AbsoluteX, O::Word(entry + 2));
            dp(p, M::Sta, channel * 16 + 4);
            mask |= 1 << channel;
        }
        m8(p);
        emit(p, M::Lda, A::Immediate, O::Byte(mask));
        emit(p, M::Sta, A::AbsoluteLong, O::Long(0x420b));
    }
    implied(p, M::Pld);
    implied(p, M::Plb);
    implied(p, M::Plx);
    implied(p, M::Rts);
    Ok(())
}
pub fn emit_channels(p: &mut w65c816::Assembler) {
    use crate::sfc::{emit, implied};
    use w65c816::{AddressingMode as A, CpuMode, Mnemonic as M, Operand as O};
    p.label("b1_view_channels")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    for c in CHANNELS {
        let base = 0x4300 + u16::from(c) * 16;
        for (at, value) in [(0, 0x01), (1, 0x18), (6, 0)] {
            emit(p, M::Lda, A::Immediate, O::Byte(value));
            emit(p, M::Sta, A::AbsoluteLong, O::Long(u32::from(base + at)));
        }
    }
    implied(p, M::Rts);
}
pub fn emit_view(
    p: &mut w65c816::Assembler,
    layout: &sfc_builder::cartridge::CartridgeLayout,
    tiles: usize,
) -> Result<()> {
    use crate::sfc::{emit, implied, write};
    use w65c816::{AddressingMode as A, CpuMode, Mnemonic as M, Operand as O};
    let abs = |p: &mut w65c816::Assembler, m, at: u16| emit(p, m, A::Absolute, O::Word(at));
    let m16 = |p: &mut w65c816::Assembler| emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    let m8 = |p: &mut w65c816::Assembler| emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    let field = |p: &mut w65c816::Assembler, index: u16| {
        emit(p, M::Ldy, A::Immediate, O::Word(index));
        emit(p, M::Lda, A::DirectPageIndirectLongIndexedY, O::Byte(0xe4));
    };
    let address = |name: &str| -> Result<u32> { layout.cpu_address(layout.section(name)?.offset) };
    let queue = crate::sfc_b1::QUEUE;
    p.label("b1_descriptor_view")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    write(p, 0x2115, 0x80);
    m16(p);
    field(p, 1);
    abs(p, M::Sta, 0x2116);
    emit(p, M::Lda, A::Immediate, O::Word(0x1801));
    abs(p, M::Sta, 0x4300);
    emit(p, M::Lda, A::Immediate, O::Word(STAGING as u16));
    abs(p, M::Sta, 0x4302);
    field(p, 3);
    abs(p, M::Sta, 0x4305);
    m8(p);
    write(p, 0x4304, (STAGING >> 16) as u8);
    write(p, 0x420b, 1);
    implied(p, M::Rts);
    p.label("b1_view_unstaged")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    emit(p, M::Lda, A::DirectPageIndirectLong, O::Byte(0xe4));
    emit(p, M::Cmp, A::Immediate, O::Byte(VIEW_KIND));
    branch(p, M::Bne, "b1_view_staged");
    for i in 0..3u16 {
        field(p, 5 + i);
        abs(p, M::Cmp, STAGED + i);
        branch(p, M::Bne, "b1_view_not_staged");
    }
    p.label("b1_view_staged");
    implied(p, M::Clc);
    implied(p, M::Rts);
    p.label("b1_view_not_staged");
    implied(p, M::Sec);
    implied(p, M::Rts);
    p.label("b1_view_gather")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    abs(p, M::Lda, queue + 3);
    branch(p, M::Bne, "b1_view_gather_queued");
    m8(p);
    implied(p, M::Rts);
    p.label("b1_view_gather_queued")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    abs(p, M::Lda, queue);
    abs(p, M::Sta, 0xe4);
    abs(p, M::Lda, queue + 2);
    abs(p, M::Sta, 0xe6);
    m8(p);
    call(p, "b1_view_unstaged");
    branch(p, M::Bcs, "b1_view_gather_block");
    implied(p, M::Rts);
    p.label("b1_view_gather_block");
    write(p, STAGED + 2, 0xff);
    for i in 0..3u16 {
        field(p, 5 + i);
        abs(p, M::Sta, 0xe8 + i);
    }
    write(p, 0x2181, STAGING as u8);
    write(p, 0x2182, (STAGING >> 8) as u8);
    write(p, 0x2183, ((STAGING >> 16) & 1) as u8);
    m16(p);
    emit(p, M::Lda, A::Immediate, O::Word(0x8000));
    abs(p, M::Sta, 0x4300);
    if tiles != BLOCK_TILES {
        return Err("View block tile count".into());
    }
    emit(
        p,
        M::Jsl,
        A::AbsoluteLong,
        O::Long(address(GATHER_SECTION)?),
    );
    m8(p);
    call(p, "battle_view_overlay");
    for i in 0..3u16 {
        abs(p, M::Lda, 0xe8 + i);
        abs(p, M::Sta, STAGED + i);
    }
    implied(p, M::Rts);
    Ok(())
}
const GATHER_SECTION: &str = "b1_view_gather_code";
const MASK: u8 = 0xf2;
const CHANGED: u8 = 0xfa;
const BITS: u8 = 0xfd;
const BYTE: u8 = 0xfe;
const GATHER_BYTES: usize = 0x100;
pub fn place_code(
    sections: &mut [sfc_builder::cartridge::Section],
    layout: &sfc_builder::cartridge::CartridgeLayout,
) -> Result<()> {
    let mut bytes = gather_code(layout)?.bytes().to_vec();
    bytes.resize(GATHER_BYTES, 0);
    sections
        .iter_mut()
        .find(|s| s.name == GATHER_SECTION)
        .ok_or("Missing view gather section")?
        .bytes = bytes;
    Ok(())
}
fn gather_code(
    layout: &sfc_builder::cartridge::CartridgeLayout,
) -> Result<w65c816::AssembledProgram> {
    use crate::sfc::{emit, implied, write};
    use w65c816::{AddressingMode as A, CpuMode, Mnemonic as M, Operand as O};
    let abs = |p: &mut w65c816::Assembler, m, at: u16| emit(p, m, A::Absolute, O::Word(at));
    let m16 = |p: &mut w65c816::Assembler| emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    let m8 = |p: &mut w65c816::Assembler| emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    let address = |name: &str| -> Result<u32> { layout.cpu_address(layout.section(name)?.offset) };
    let at = address(GATHER_SECTION)?;
    let low_offsets = address("b1_view_low_offsets")?;
    let low_banks = address("b1_view_low_banks")?;
    let high_index = address("b1_view_high_index")?;
    let high_offsets = address("b1_view_high_offsets")?;
    let high_banks = address("b1_view_high_banks")?;
    let tiles = BLOCK_TILES;
    let mut program = w65c816::Assembler::new();
    let p = &mut program;
    let dp = |p: &mut w65c816::Assembler, m, at: u8| emit(p, m, A::DirectPage, O::Byte(at));
    p.label("gather").assume_mode(CpuMode::NATIVE_M16_X16);
    dp(p, M::Lda, 0xe8);
    implied(p, M::Pha);
    dp(p, M::Lda, 0xe9);
    implied(p, M::Pha);
    for at in (MASK..MASK + 8).step_by(2) {
        emit(p, M::Stz, A::DirectPage, O::Byte(at));
    }
    emit(p, M::Lda, A::DirectPageIndirectLong, O::Byte(0xe8));
    emit(p, M::And, A::Immediate, O::Word(0xff));
    emit(p, M::Cmp, A::Immediate, O::Word(u16::from(DELTA)));
    branch(p, M::Bcc, "gather_full");
    emit(p, M::Ldy, A::Immediate, O::Word(4));
    emit(p, M::Lda, A::DirectPageIndirectLongIndexedY, O::Byte(0xe8));
    implied(p, M::Tax);
    for i in (0..8).step_by(2) {
        emit(
            p,
            M::Lda,
            A::AbsoluteLongX,
            O::Long(address("b1_view_masks")? + i),
        );
        dp(p, M::Sta, MASK + i as u8);
    }
    m8(p);
    dp(p, M::Lda, 0xea);
    dp(p, M::Sta, CHANGED + 2);
    m16(p);
    dp(p, M::Lda, 0xe8);
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Word(DELTA_HEAD as u16));
    dp(p, M::Sta, CHANGED);
    emit(p, M::Ldy, A::Immediate, O::Word(1));
    emit(p, M::Lda, A::DirectPageIndirectLongIndexedY, O::Byte(0xe8));
    implied(p, M::Pha);
    emit(p, M::Ldy, A::Immediate, O::Word(3));
    m8(p);
    emit(p, M::Lda, A::DirectPageIndirectLongIndexedY, O::Byte(0xe8));
    dp(p, M::Sta, 0xea);
    m16(p);
    implied(p, M::Pla);
    dp(p, M::Sta, 0xe8);
    p.label("gather_full");
    emit(p, M::Lda, A::DirectPageIndirectLong, O::Byte(0xe8));
    emit(p, M::And, A::Immediate, O::Word(0xff));
    for _ in 0..BASES.trailing_zeros() {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    implied(p, M::Tax);
    emit(p, M::Ldy, A::Immediate, O::Word(1));
    p.label("b1_view_gather_tile");
    implied(p, M::Tya);
    emit(p, M::Dec, A::Accumulator, O::None);
    emit(p, M::And, A::Immediate, O::Word(7));
    branch(p, M::Bne, "gather_bit");
    implied(p, M::Tya);
    emit(p, M::Dec, A::Accumulator, O::None);
    for _ in 0..3 {
        emit(p, M::Lsr, A::Accumulator, O::None);
    }
    implied(p, M::Phx);
    implied(p, M::Tax);
    emit(p, M::Lda, A::DirectPageX, O::Byte(MASK));
    implied(p, M::Plx);
    dp(p, M::Sta, BITS);
    p.label("gather_bit");
    emit(p, M::Lsr, A::DirectPage, O::Byte(BITS));
    branch(p, M::Bcs, "gather_changed");
    emit(p, M::Lda, A::DirectPageIndirectLongIndexedY, O::Byte(0xe8));
    branch(p, M::Bra, "gather_byte");
    p.label("gather_changed");
    emit(p, M::Lda, A::DirectPageIndirectLong, O::Byte(CHANGED));
    emit(p, M::Inc, A::DirectPage, O::Byte(CHANGED));
    p.label("gather_byte");
    emit(p, M::And, A::Immediate, O::Word(0xff));
    dp(p, M::Sta, BYTE);
    for _ in 0..HALF.trailing_zeros() {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    emit(p, M::Adc, A::AbsoluteLongX, O::Long(low_offsets));
    abs(p, M::Sta, 0x4302);
    m8(p);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(low_banks));
    abs(p, M::Sta, 0x4304);
    write(p, 0x4305, HALF as u8);
    write(p, 0x420b, 1);
    m16(p);
    dp(p, M::Lda, BYTE);
    emit(p, M::Asl, A::Accumulator, O::None);
    emit(p, M::Adc, A::AbsoluteLongX, O::Long(high_index));
    implied(p, M::Phx);
    implied(p, M::Tax);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(high_offsets));
    abs(p, M::Sta, 0x4302);
    emit(p, M::And, A::Immediate, O::Word(HALF as u16 - 1));
    implied(p, M::Tax);
    m8(p);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(high_banks));
    abs(p, M::Sta, 0x4304);
    write(p, 0x4305, HALF as u8);
    write(p, 0x420b, 1);
    m16(p);
    implied(p, M::Plx);
    implied(p, M::Inx);
    implied(p, M::Inx);
    implied(p, M::Iny);
    emit(p, M::Cpy, A::Immediate, O::Word(u16::try_from(tiles + 1)?));
    branch(p, M::Beq, "gather_done");
    p.emit_label_ref(M::Brl, A::Relative16, "b1_view_gather_tile");
    p.label("gather_done");
    implied(p, M::Pla);
    dp(p, M::Sta, 0xe9);
    implied(p, M::Pla);
    dp(p, M::Sta, 0xe8);
    implied(p, M::Rtl);
    let program = crate::sfc_width_check::assemble(
        &program,
        w65c816::CodeLocation::new((at >> 16) as u8, at as u16),
        CpuMode::NATIVE_M16_X16,
    )?;
    if program.bytes().len() > GATHER_BYTES {
        return Err("View gather code exceeds its section".into());
    }
    Ok(program)
}
fn branch(p: &mut w65c816::Assembler, m: w65c816::Mnemonic, label: &str) {
    p.emit_label_ref(m, w65c816::AddressingMode::Relative8, label);
}
fn call(p: &mut w65c816::Assembler, label: &str) {
    p.emit_label_ref(
        w65c816::Mnemonic::Jsr,
        w65c816::AddressingMode::Absolute,
        label,
    );
}
