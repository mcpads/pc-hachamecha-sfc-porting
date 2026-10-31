use super::*;
use crate::sfc::{emit, implied};
use sfc_builder::cartridge::{CartridgeLayout, Section};
use w65c816::{AddressingMode as A, CpuMode, Mnemonic as M, Operand as O};
const INDEX: &str = "b1_text_cache_index";
const TILE_BYTES: usize = 32;
pub(super) struct Page {
    id: u16,
    runs: Vec<(u16, Vec<u8>)>,
    extent: [u8; ROWS],
}
impl Assets {
    pub fn cache_pages(
        &mut self,
        pages: &[String],
        selected: std::ops::Range<usize>,
    ) -> Result<()> {
        if selected.end > pages.len() || pages.len() != self.offsets.len() {
            return Err("Text cache selection outside prepared pages".into());
        }
        self.cached.clear();
        for id in selected {
            if pages[id].lines().count() > LINES {
                return Err("Cached text page exceeds the visible line count".into());
            }
            let page = &self.reference[id * self.blank.len()..(id + 1) * self.blank.len()];
            let mut runs: Vec<(u16, Vec<u8>)> = Vec::new();
            for (i, (tile, blank)) in page
                .as_chunks::<TILE_BYTES>()
                .0
                .iter()
                .zip(self.blank.as_chunks::<TILE_BYTES>().0)
                .enumerate()
            {
                if tile == blank {
                    continue;
                }
                let offset = i * TILE_BYTES;
                if let Some((start, bytes)) = runs.last_mut()
                    && usize::from(*start) + bytes.len() == offset
                {
                    bytes.extend(tile);
                } else {
                    runs.push((u16::try_from(offset)?, tile.to_vec()));
                }
            }
            let mut extent = [0; ROWS];
            let [mut x, mut y] = PEN;
            for step in steps(&pages[id], &self.index)? {
                match step {
                    Step::Glyph(glyph, _) => {
                        x += usize::from(self.glyphs[glyph * GLYPH_BYTES]);
                    }
                    Step::Break => {
                        mark_extent(&mut extent, x, y);
                        [x, y] = [PEN[0], y + DIALOGUE_LINE];
                    }
                }
            }
            mark_extent(&mut extent, x, y);
            self.cached.push(Page {
                id: u16::try_from(id)?,
                runs,
                extent,
            });
        }
        self.report["cached_pages"] = serde_json::json!(
            self.cached
                .iter()
                .map(|page| {
                    serde_json::json!({ "page" : page.id,
            "runs" : page.runs.len(), "tile_bytes" : page.runs.iter().map(| (_, b) | b
            .len()).sum::< usize > (), "extent" : page.extent })
                })
                .collect::<Vec<_>>()
        );
        Ok(())
    }
    pub fn append_cache(
        &self,
        sections: &mut Vec<Section>,
        space: &mut crate::sfc_rom::Space,
    ) -> Result<()> {
        let mut index = Vec::new();
        for page in &self.cached {
            let draft = assemble(page, 0xc0_0000, None)?;
            let offset = space.reserve_fast(draft.bytes().len())?;
            let origin = crate::sfc_rom::cpu(offset)?;
            let draft = assemble(page, origin, None)?;
            let sources = (0..page.runs.len())
                .map(|i| {
                    draft
                        .label_location(&format!("span_{i}"))
                        .map(|a| a.to_u32() as u16)
                        .ok_or_else(|| "Missing cached tile span".into())
                })
                .collect::<Result<Vec<_>>>()?;
            let code = assemble(page, origin, Some(&sources))?;
            if code.bytes().len() != draft.bytes().len() {
                return Err("Text cache changed size after placement".into());
            }
            index.extend(page.id.to_le_bytes());
            index.extend(&origin.to_le_bytes()[..3]);
            sections.push(Section {
                name: format!("b1_text_cache_{}", page.id),
                offset,
                bytes: code.bytes().to_vec(),
            });
        }
        crate::sfc_b1::add(sections, space, INDEX.into(), index)?;
        Ok(())
    }
}
fn mark_extent(extent: &mut [u8; ROWS], x: usize, y: usize) {
    let columns = (((x + 15) >> 3) + 1).min(COLUMNS) as u8;
    let last = ((y + DIALOGUE_LINE) >> 3).min(ROWS - 1);
    for row in extent.iter_mut().take(last + 1).skip(y >> 3) {
        *row = (*row).max(columns);
    }
}
fn assemble(
    page: &Page,
    origin: u32,
    sources: Option<&[u16]>,
) -> Result<w65c816::AssembledProgram> {
    let mut p = w65c816::Assembler::new();
    p.assume_mode(CpuMode::NATIVE_M16_X16);
    for (i, (destination, bytes)) in page.runs.iter().enumerate() {
        emit(
            &mut p,
            M::Ldx,
            A::Immediate,
            O::Word(sources.map_or(0, |s| s[i])),
        );
        emit(&mut p, M::Ldy, A::Immediate, O::Word(*destination));
        emit(
            &mut p,
            M::Lda,
            A::Immediate,
            O::Word(u16::try_from(bytes.len() - 1)?),
        );
        emit(
            &mut p,
            M::Mvn,
            A::BlockMove,
            O::BlockMove {
                destination_bank: (BUFFER >> 16) as u8,
                source_bank: (origin >> 16) as u8,
            },
        );
    }
    emit(&mut p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    for (row, &columns) in page.extent.iter().enumerate() {
        emit(&mut p, M::Lda, A::Immediate, O::Byte(columns));
        emit(
            &mut p,
            M::Sta,
            A::AbsoluteLong,
            O::Long(u32::from(EXTENT) + row as u32),
        );
    }
    emit(&mut p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    implied(&mut p, M::Sec);
    implied(&mut p, M::Rtl);
    for (i, (_, bytes)) in page.runs.iter().enumerate() {
        p.label(format!("span_{i}")).data(bytes.clone());
    }
    crate::sfc_width_check::assemble(
        &p,
        w65c816::CodeLocation::new((origin >> 16) as u8, origin as u16),
        CpuMode::NATIVE_M16_X16,
    )
}
pub(super) fn emit_dispatch(p: &mut w65c816::Assembler, layout: &CartridgeLayout) -> Result<()> {
    p.label("b1_text_cached_page")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    for entry in layout.section(INDEX)?.bytes.as_chunks::<5>().0 {
        let page = u16::from_le_bytes([entry[0], entry[1]]);
        let address = u32::from_le_bytes([entry[2], entry[3], entry[4], 0]);
        let next = format!("b1_text_cache_after_{page}");
        emit(p, M::Cmp, A::Immediate, O::Word(page));
        p.emit_label_ref(M::Bne, A::Relative8, &next);
        emit(p, M::Jsl, A::AbsoluteLong, O::Long(address));
        implied(p, M::Rts);
        p.label(next);
    }
    implied(p, M::Clc);
    implied(p, M::Rts);
    Ok(())
}
