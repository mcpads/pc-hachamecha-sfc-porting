use crate::Result;
use sfc_builder::cartridge::Section;
use std::collections::HashMap;
const TILE: usize = 32;
const ITEM: usize = 0x8000;
const GATHER_SECTION: &str = "body_gather_code";
const GATHER_BYTES: usize = 0x80;
const ZERO_SECTION: &str = "body_zero";
pub const STAGES_SECTION: &str = "body_stages";
#[derive(Default)]
pub struct Store {
    items: Vec<Vec<u8>>,
    at: HashMap<Vec<u8>, (usize, usize)>,
    cpu: Vec<u32>,
}
impl Store {
    pub fn note(&mut self, half: &[u8]) -> Result<()> {
        if !half.len().is_multiple_of(TILE) {
            return Err("Body half is not whole tiles".into());
        }
        for tile in half.chunks(TILE) {
            if tile.iter().all(|&b| b == 0) || self.at.contains_key(tile) {
                continue;
            }
            if self.items.last().is_none_or(|i| i.len() + TILE > ITEM) {
                self.items.push(Vec::new());
            }
            let item = self.items.len() - 1;
            self.at
                .insert(tile.to_vec(), (item, self.items[item].len()));
            self.items[item].extend(tile);
        }
        Ok(())
    }
    pub fn place_tiles(
        &mut self,
        sections: &mut Vec<Section>,
        space: &mut crate::sfc_rom::Space,
        name: &str,
    ) -> Result<()> {
        for (i, item) in self.items.iter().enumerate() {
            self.cpu.push(crate::sfc_b1::add(
                sections,
                space,
                format!("{name}_{i}"),
                item.clone(),
            )?);
        }
        Ok(())
    }
    pub fn place(
        &mut self,
        sections: &mut Vec<Section>,
        space: &mut crate::sfc_rom::Space,
    ) -> Result<()> {
        self.place_tiles(sections, space, "body_tiles")?;
        crate::sfc_b1::add(sections, space, ZERO_SECTION.into(), vec![0])?;
        crate::sfc_b1::add(
            sections,
            space,
            STAGES_SECTION.into(),
            STAGES.iter().flat_map(|a| a.to_le_bytes()).collect(),
        )?;
        let offset = space.reserve_fast(GATHER_BYTES)?;
        sections.push(Section {
            name: GATHER_SECTION.into(),
            offset,
            bytes: vec![0; GATHER_BYTES],
        });
        Ok(())
    }
    pub fn list(&mut self, half: &[u8]) -> Result<Vec<u8>> {
        let mut runs: Vec<(Option<(usize, u32)>, usize)> = Vec::new();
        for tile in half.chunks(TILE) {
            let place = if tile.iter().all(|&b| b == 0) {
                None
            } else {
                let (item, at) = *self.at.get(tile).ok_or("Body tile not in the store")?;
                Some((item, self.cpu[item] + at as u32))
            };
            match (runs.last_mut(), place) {
                (Some((None, n)), None) if *n < 255 => *n += 1,
                (Some((Some((item, from)), n)), Some((i, address)))
                    if *n < 255 && *item == i && *from + (*n * TILE) as u32 == address =>
                {
                    *n += 1;
                }
                _ => runs.push((place, 1)),
            }
        }
        let mut bytes = Vec::new();
        for (place, n) in runs {
            let address = place.map_or(0, |(_, a)| a);
            bytes.extend(&address.to_le_bytes()[..3]);
            bytes.push(n as u8);
        }
        Ok(bytes)
    }
    pub fn rebuild(sections: &[Section], list: &[u8]) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        for run in list.chunks(4) {
            let address = u32::from_le_bytes([run[0], run[1], run[2], 0]);
            let bytes = usize::from(run[3]) * TILE;
            if address == 0 {
                out.extend(vec![0; bytes]);
                continue;
            }
            let offset = crate::sfc_rom::MAPPING.rom_offset(address)?;
            let section = sections
                .iter()
                .find(|s| (s.offset..s.offset + s.bytes.len()).contains(&offset))
                .ok_or("Body run outside the store")?;
            let at = offset - section.offset;
            out.extend(
                section
                    .bytes
                    .get(at..at + bytes)
                    .ok_or("Body run past its item")?,
            );
        }
        Ok(out)
    }
}
pub fn place_code(
    sections: &mut [Section],
    layout: &sfc_builder::cartridge::CartridgeLayout,
) -> Result<()> {
    let mut bytes = gather_code(layout)?.bytes().to_vec();
    bytes.resize(GATHER_BYTES, 0);
    sections
        .iter_mut()
        .find(|s| s.name == GATHER_SECTION)
        .ok_or("Missing body gather section")?
        .bytes = bytes;
    Ok(())
}
pub fn gather_address(layout: &sfc_builder::cartridge::CartridgeLayout) -> Result<u32> {
    layout.cpu_address(layout.section(GATHER_SECTION)?.offset)
}
fn gather_code(
    layout: &sfc_builder::cartridge::CartridgeLayout,
) -> Result<w65c816::AssembledProgram> {
    use crate::sfc::{emit, implied};
    use w65c816::{AddressingMode as A, CpuMode, Mnemonic as M, Operand as O};
    let at = gather_address(layout)?;
    let zero = layout.cpu_address(layout.section(ZERO_SECTION)?.offset)?;
    let mut program = w65c816::Assembler::new();
    let p = &mut program;
    let abs = |p: &mut w65c816::Assembler, m, at: u16| emit(p, m, A::Absolute, O::Word(at));
    let m8 = |p: &mut w65c816::Assembler| emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    let m16 = |p: &mut w65c816::Assembler| emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    let branch = |p: &mut w65c816::Assembler, m, to: &str| {
        p.emit_label_ref(m, A::Relative8, to);
    };
    p.label("body_gather").assume_mode(CpuMode::NATIVE_M16_X16);
    abs(p, M::Sta, 0x2181);
    m8(p);
    emit(p, M::Lda, A::Immediate, O::Byte(STAGE_BANK & 1));
    abs(p, M::Sta, 0x2183);
    emit(p, M::Lda, A::Immediate, O::Byte(0x80));
    abs(p, M::Sta, 0x4301);
    m16(p);
    implied(p, M::Txa);
    abs(p, M::Sta, GATHER_LEFT);
    emit(p, M::Ldy, A::Immediate, O::Word(0));
    p.label("body_gather_run");
    emit(p, M::Iny, A::Implied, O::None);
    emit(p, M::Iny, A::Implied, O::None);
    emit(p, M::Iny, A::Implied, O::None);
    emit(
        p,
        M::Lda,
        A::DirectPageIndirectLongIndexedY,
        O::Byte(GATHER_POINTER),
    );
    emit(p, M::And, A::Immediate, O::Word(0xff));
    for _ in 0..TILE.trailing_zeros() {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    abs(p, M::Sta, 0x4305);
    implied(p, M::Sec);
    implied(p, M::Pha);
    abs(p, M::Lda, GATHER_LEFT);
    emit(p, M::Sbc, A::StackRelative, O::Byte(1));
    abs(p, M::Sta, GATHER_LEFT);
    implied(p, M::Pla);
    implied(p, M::Dey);
    implied(p, M::Dey);
    implied(p, M::Dey);
    emit(
        p,
        M::Lda,
        A::DirectPageIndirectLongIndexedY,
        O::Byte(GATHER_POINTER),
    );
    abs(p, M::Sta, 0x4302);
    implied(p, M::Iny);
    implied(p, M::Iny);
    m8(p);
    emit(
        p,
        M::Lda,
        A::DirectPageIndirectLongIndexedY,
        O::Byte(GATHER_POINTER),
    );
    branch(p, M::Bne, "body_gather_tiles");
    emit(p, M::Lda, A::Immediate, O::Byte(8));
    abs(p, M::Sta, 0x4300);
    m16(p);
    emit(p, M::Lda, A::Immediate, O::Word(zero as u16));
    abs(p, M::Sta, 0x4302);
    m8(p);
    emit(p, M::Lda, A::Immediate, O::Byte((zero >> 16) as u8));
    branch(p, M::Bra, "body_gather_go");
    p.label("body_gather_tiles")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    implied(p, M::Pha);
    emit(p, M::Lda, A::Immediate, O::Byte(0));
    abs(p, M::Sta, 0x4300);
    implied(p, M::Pla);
    p.label("body_gather_go")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Sta, 0x4304);
    emit(p, M::Lda, A::Immediate, O::Byte(1));
    abs(p, M::Sta, 0x420b);
    m16(p);
    implied(p, M::Iny);
    implied(p, M::Iny);
    abs(p, M::Lda, GATHER_LEFT);
    branch(p, M::Bne, "body_gather_run");
    implied(p, M::Rtl);
    let program = crate::sfc_width_check::assemble(
        &program,
        w65c816::CodeLocation::new((at >> 16) as u8, at as u16),
        CpuMode::NATIVE_M16_X16,
    )?;
    if program.bytes().len() > GATHER_BYTES {
        return Err("Body gather code exceeds its section".into());
    }
    Ok(program)
}
pub const STAGE_BANK: u8 = 0x7f;
pub const STAGES: [u16; 12] = [
    0x2a00, 0x2c00, 0x2e00, 0x3000, 0x3200, 0x3400, 0x3600, 0x3800, 0x3a00, 0x3c00, 0x3e00, 0x6400,
];
pub const GATHER_POINTER: u8 = 0xf4;
const GATHER_LEFT: u16 = 0xf7;
