pub use crate::tiles::Depth;
use crate::{Result, text::GlyphMap};
use std::collections::BTreeMap;
pub struct GlyphAtlas {
    first_tile: u16,
    depth: Depth,
    width: usize,
    height: usize,
    pixels: Vec<Vec<u8>>,
    bytes: Vec<u8>,
    mapping: GlyphMap,
    sources: Vec<serde_json::Value>,
}
impl GlyphAtlas {
    pub fn new(entries: &[(char, Vec<u8>)], first_tile: u16, depth: Depth) -> Result<Self> {
        Self::sized(entries, 8, 8, first_tile, depth)
    }
    pub fn sized(
        entries: &[(char, Vec<u8>)],
        width: usize,
        height: usize,
        first_tile: u16,
        depth: Depth,
    ) -> Result<Self> {
        if width == 0
            || height == 0
            || width > 64
            || height > 64
            || width % 8 != 0
            || height % 8 != 0
        {
            return Err("glyph.dimensions".into());
        }
        if entries.is_empty() || first_tile > 1023 {
            return Err("glyph.empty_or_tile_extent".into());
        }
        let mut ordered = BTreeMap::new();
        let limit = depth.colors();
        for (scalar, pixels) in entries {
            if scalar.is_control() || pixels.len() != width * height {
                return Err(format!(
                    "glyph.domain: U+{:04X} pixels {}",
                    *scalar as u32,
                    pixels.len()
                )
                .into());
            }
            if let Some((offset, value)) = pixels
                .iter()
                .enumerate()
                .find(|(_, value)| u16::from(**value) >= limit)
            {
                return Err(format!(
                    "glyph.pixel: U+{:04X} x {} y {} index {value}",
                    *scalar as u32,
                    offset % width,
                    offset / width
                )
                .into());
            }
            if ordered.insert(*scalar, pixels).is_some() {
                return Err(format!("glyph.duplicate_scalar: U+{:04X}", *scalar as u32).into());
            }
        }
        let mut pixels = vec![];
        let mut unique = BTreeMap::new();
        let mut mapping = vec![];
        let mut sources = vec![];
        for (scalar, input) in ordered {
            let mut block = vec![];
            for ty in 0..height / 8 {
                for tx in 0..width / 8 {
                    let part: Vec<_> = (0..64)
                        .map(|i| input[(ty * 8 + i / 8) * width + tx * 8 + i % 8])
                        .collect();
                    let tile = if let Some(tile) = unique.get(&part) {
                        *tile
                    } else {
                        let index = usize::from(first_tile) + pixels.len();
                        if index > 1023 {
                            return Err(format!(
                                "glyph.tile_extent: U+{:04X} cell {tx},{ty}",
                                scalar as u32
                            )
                            .into());
                        }
                        let tile = index as u16;
                        unique.insert(part.clone(), tile);
                        pixels.push(part);
                        tile
                    };
                    block.push(tile);
                }
            }
            sources.push(serde_json::json!(
                { "scalar" : scalar.to_string(), "codepoint" : scalar as u32,
                "source_pixels_sha256" : crate ::sha256(input), "tiles" : block }
            ));
            mapping.push((scalar, block));
        }
        let bytes = crate::tiles::encode(&pixels, depth)?;
        Ok(Self {
            first_tile,
            depth,
            width,
            height,
            pixels,
            bytes,
            mapping: GlyphMap::blocks(width / 8, height / 8, mapping)?,
            sources,
        })
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn pixels(&self) -> &[Vec<u8>] {
        &self.pixels
    }
    pub fn mapping(&self) -> &GlyphMap {
        &self.mapping
    }
    pub fn report(&self) -> serde_json::Value {
        serde_json::json!(
            { "first_tile" : self.first_tile, "depth" : format!("{:?}", self.depth),
            "glyph_width" : self.width, "glyph_height" : self.height, "transparent_index"
            : 0, "unique_tiles" : self.pixels.len(), "native_sha256" : crate ::sha256(&
            self.bytes), "native_bytes" : self.bytes.len(), "sources" : self.sources, }
        )
    }
}
