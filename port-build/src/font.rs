use crate::{Result, sha};
use serde::Serialize;
use std::{collections::BTreeMap, path::Path};
const HASH: &str = "d61b60eccb731f8ca9c7da582e4a05a94db66b570471809950aa9a7261b941d6";
#[derive(Debug, Serialize)]
pub struct Report {
    source_sha256: String,
    family: &'static str,
    license: &'static str,
    rasterizer: &'static str,
    glyphs: usize,
    packed_sha256: String,
    glyph_aliases: Vec<[u32; 2]>,
    preview_cell: [u8; 2],
}
pub struct PortFont {
    pub report: Report,
    pub packed: Vec<u8>,
    encoding: BTreeMap<u16, char>,
    masks: BTreeMap<char, Vec<u8>>,
    masks16: BTreeMap<char, Vec<u8>>,
}
impl PortFont {
    pub fn load(
        path: &Path,
        glyphs: &[u8],
        encoding: &[[u32; 2]],
        extra_text: &str,
    ) -> Result<Self> {
        let raw = std::fs::read(path)?;
        if raw.len() != 651044 || sha(&raw) != HASH {
            return Err("Port font must match the pinned NeoDunggeunmo input".into());
        }
        let font = fontdue::Font::from_bytes(raw, fontdue::FontSettings::default())?;
        if glyphs.len() < 6 || &glyphs[..4] != b"HMG1" || !(glyphs.len() - 6).is_multiple_of(4) {
            return Err("Invalid font scalar input".into());
        }
        let mut characters = Vec::new();
        for bytes in glyphs[6..].as_chunks::<4>().0 {
            characters
                .push(char::from_u32(u32::from_le_bytes(*bytes)).ok_or("Invalid font scalar")?);
        }
        if usize::from(u16::from_le_bytes(glyphs[4..6].try_into()?)) != characters.len() {
            return Err("Invalid font scalar count".into());
        }
        let mut mapping = BTreeMap::new();
        for [code, scalar] in encoding {
            let character = char::from_u32(*scalar).ok_or("Invalid encoded scalar")?;
            if !characters.contains(&character)
                || mapping.insert(u16::try_from(*code)?, character).is_some()
            {
                return Err("Invalid source glyph encoding".into());
            }
        }
        if mapping
            .values()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            != characters.iter().copied().collect()
        {
            return Err("Incomplete source glyph encoding".into());
        }
        for character in crate::sfc_b1::UI_TEXT.chars().chain(extra_text.chars()) {
            if !characters.contains(&character) {
                characters.push(character);
            }
        }
        let mut packed = b"HMF1".to_vec();
        packed.extend((characters.len() as u16).to_le_bytes());
        packed.extend([16, 16]);
        let mut masks = BTreeMap::new();
        let mut masks16 = BTreeMap::new();
        let missing: Vec<_> = characters
            .iter()
            .filter(|ch| font.lookup_glyph_index(port_glyph_source(**ch)) == 0)
            .collect();
        if !missing.is_empty() {
            return Err(format!("Missing font glyphs {missing:?}").into());
        }
        for character in &characters {
            if font.lookup_glyph_index(port_glyph_source(*character)) == 0 {
                return Err(format!("Missing font glyph {character:?}").into());
            }
            packed.extend((*character as u32).to_le_bytes());
            let small = raster(&font, *character, 16)?;
            for row in small.as_chunks::<16>().0 {
                let bits = row
                    .iter()
                    .fold(0u16, |bits, p| (bits << 1) | u16::from(*p != 0));
                packed.extend(bits.to_be_bytes());
            }
            masks.insert(*character, raster(&font, *character, 20)?);
            masks16.insert(*character, small);
        }
        Ok(Self {
            report: Report {
                source_sha256: HASH.into(),
                family: "NeoDunggeunmo",
                license: "SIL Open Font License 1.1",
                rasterizer: "fontdue 0.9.3; coverage >127",
                glyphs: characters.len(),
                packed_sha256: sha(&packed),
                glyph_aliases: characters
                    .iter()
                    .filter_map(|ch| {
                        let source = port_glyph_source(*ch);
                        (source != *ch).then_some([*ch as u32, source as u32])
                    })
                    .collect(),
                preview_cell: [20, 24],
            },
            packed,
            encoding: mapping,
            masks,
            masks16,
        })
    }
    pub fn characters(&self, bytes: &[u8]) -> Result<Vec<char>> {
        if !bytes.len().is_multiple_of(2) {
            return Err("Odd source text byte count".into());
        }
        bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| {
                self.encoding
                    .get(&u16::from_be_bytes([b[0], b[1]]))
                    .copied()
                    .ok_or_else(|| "Unmapped source text character".into())
            })
            .collect()
    }
    pub fn mask(&self, ch: char) -> Result<&[u8]> {
        self.masks
            .get(&ch)
            .map(Vec::as_slice)
            .ok_or_else(|| "Missing rendered text character".into())
    }
    pub fn mask16(&self, ch: char) -> Result<&[u8]> {
        self.masks16
            .get(&ch)
            .map(Vec::as_slice)
            .ok_or_else(|| "Missing target glyph".into())
    }
}
fn raster(font: &fontdue::Font, ch: char, size: usize) -> Result<Vec<u8>> {
    let (metrics, coverage) = font.rasterize(port_glyph_source(ch), size as f32);
    let ascent = font
        .horizontal_line_metrics(size as f32)
        .ok_or("Missing font line metrics")?
        .ascent
        .ceil() as i32;
    let top = ascent - metrics.ymin - metrics.height as i32;
    let mut pixels = vec![0; size * size];
    for y in 0..metrics.height {
        for x in 0..metrics.width {
            if coverage[y * metrics.width + x] <= 127 {
                continue;
            }
            let dx = metrics.xmin + x as i32;
            let dy = top + y as i32;
            if dx < 0 || dy < 0 || dx >= size as i32 || dy >= size as i32 {
                return Err(
                    format!("Glyph {ch:?} exceeds {size} pixel cell at ({dx},{dy})").into(),
                );
            }
            pixels[dy as usize * size + dx as usize] = 1;
        }
    }
    Ok(pixels)
}
const DIALOGUE_HASH: &str = "2c709890595668f7bdb6df408420fda957dde0288e95b31a1cc17a2ab98b4b4f";
const DIALOGUE_SIZE: usize = 12;
pub const DIALOGUE_LINE: usize = 13;
pub struct DialogueGlyph {
    pub advance: usize,
    pub ink: Vec<[usize; 2]>,
}
pub struct DialogueFont {
    glyphs: BTreeMap<char, DialogueGlyph>,
    pub report: serde_json::Value,
}
impl DialogueFont {
    pub fn load(path: &Path, text: &str) -> Result<Self> {
        let raw = std::fs::read(path)?;
        if sha(&raw) != DIALOGUE_HASH {
            return Err("Dialogue font must match the pinned Galmuri11 input".into());
        }
        let font = fontdue::Font::from_bytes(raw, fontdue::FontSettings::default())?;
        let size = DIALOGUE_SIZE as f32;
        let ascent = font
            .horizontal_line_metrics(size)
            .ok_or("Missing dialogue font line metrics")?
            .ascent
            .ceil() as i32;
        let mut glyphs = BTreeMap::new();
        for mark in std::iter::once(crate::b1_dialogue::SOURCE_BREAK)
            .chain((1..=6).map(crate::b1_dialogue::color_mark))
        {
            glyphs.insert(
                mark,
                DialogueGlyph {
                    advance: 0,
                    ink: Vec::new(),
                },
            );
        }
        for ch in text.chars().filter(|ch| *ch != '\n') {
            if glyphs.contains_key(&ch) {
                continue;
            }
            let shape = narrow_form(glyph_source(ch));
            if font.lookup_glyph_index(shape) == 0 {
                return Err(format!("Missing dialogue glyph {ch:?}").into());
            }
            let (metrics, coverage) = font.rasterize(shape, size);
            let top = (ascent - metrics.ymin - metrics.height as i32)
                .min(DIALOGUE_LINE as i32 - metrics.height as i32);
            let mut ink = Vec::new();
            for y in 0..metrics.height {
                for x in 0..metrics.width {
                    if coverage[y * metrics.width + x] <= 127 {
                        continue;
                    }
                    let dx = metrics.xmin + x as i32;
                    let dy = top + y as i32;
                    if dx < 0 || dy < 0 || dy as usize >= DIALOGUE_LINE {
                        return Err(format!("Dialogue glyph {ch:?} exceeds its line").into());
                    }
                    ink.push([dx as usize, dy as usize]);
                }
            }
            let advance = metrics.advance_width.round() as usize;
            glyphs.insert(ch, DialogueGlyph { advance, ink });
        }
        let report = serde_json::json!(
            { "source_sha256" : DIALOGUE_HASH, "family" : "Galmuri11", "license" :
            "SIL Open Font License 1.1", "rasterizer" : "fontdue 0.9.3; coverage >127",
            "size" : DIALOGUE_SIZE, "line_pitch" : DIALOGUE_LINE, "glyphs" : glyphs.len()
            }
        );
        Ok(Self { glyphs, report })
    }
    pub fn glyph(&self, ch: char) -> Result<&DialogueGlyph> {
        self.glyphs
            .get(&ch)
            .ok_or_else(|| format!("Missing dialogue glyph {ch:?}").into())
    }
    pub fn advance(&self, ch: char) -> Result<usize> {
        Ok(self.glyph(ch)?.advance)
    }
}
fn narrow_form(ch: char) -> char {
    match ch {
        '\u{3000}' => ' ',
        '\u{ff0d}' | '\u{ff5e}' => ch,
        '\u{ff01}'..='\u{ff5d}' => char::from_u32(u32::from(ch) - 0xfee0).unwrap_or(ch),
        _ => ch,
    }
}
fn glyph_source(ch: char) -> char {
    if ch == '\u{223c}' { '\u{ff5e}' } else { ch }
}
fn port_glyph_source(ch: char) -> char {
    match ch {
        '\u{2190}' => '\u{25c0}',
        '\u{2192}' => '\u{25b6}',
        _ => glyph_source(ch),
    }
}
