use crate::Result;
use std::collections::BTreeMap;
pub struct GlyphMap {
    tiles: BTreeMap<char, Vec<u16>>,
    width: usize,
    height: usize,
}
impl GlyphMap {
    pub fn new(entries: impl IntoIterator<Item = (char, u16)>) -> Result<Self> {
        Self::blocks(
            1,
            1,
            entries
                .into_iter()
                .map(|(scalar, tile)| (scalar, vec![tile])),
        )
    }
    pub fn blocks(
        width: usize,
        height: usize,
        entries: impl IntoIterator<Item = (char, Vec<u16>)>,
    ) -> Result<Self> {
        if !(1..=64).contains(&width) || !(1..=64).contains(&height) {
            return Err("text.glyph_shape".into());
        }
        let mut tiles = BTreeMap::new();
        for (scalar, block) in entries {
            if scalar.is_control()
                || block.len() != width * height
                || block.iter().any(|&t| t > 1023)
            {
                return Err(format!("text.glyph_domain: U+{:04X}", scalar as u32).into());
            }
            if tiles.insert(scalar, block).is_some() {
                return Err(format!("text.duplicate_glyph: U+{:04X}", scalar as u32).into());
            }
        }
        Ok(Self {
            tiles,
            width,
            height,
        })
    }
    pub fn dimensions(&self) -> (usize, usize) {
        (self.width, self.height)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextWindow {
    width: usize,
    height: usize,
    cells: Vec<u16>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CellChange {
    pub x: usize,
    pub y: usize,
    pub previous: u16,
    pub tile: u16,
}
impl TextWindow {
    pub fn new(width: usize, height: usize, blank_tile: u16) -> Result<Self> {
        if !(1..=64).contains(&width) || !(1..=64).contains(&height) || blank_tile > 1023 {
            return Err("text.window_domain".into());
        }
        Ok(Self {
            width,
            height,
            cells: vec![blank_tile; width * height],
        })
    }
    pub fn cells(&self) -> &[u16] {
        &self.cells
    }
    pub fn compose_cursor(
        &self,
        glyphs: &GlyphMap,
        x: usize,
        y: usize,
        scalar: char,
    ) -> Result<Vec<u16>> {
        if scalar.is_control() {
            return Err("text.cursor_scalar".into());
        }
        let mut composed = self.clone();
        composed.write(glyphs, x, y, &scalar.to_string())?;
        Ok(composed.cells)
    }
    pub fn write(
        &mut self,
        glyphs: &GlyphMap,
        x: usize,
        y: usize,
        text: &str,
    ) -> Result<Vec<CellChange>> {
        if x >= self.width || y >= self.height {
            return Err("text.origin_extent".into());
        }
        let mut column = x;
        let mut row = y;
        let mut changes = vec![];
        for (byte, scalar) in text.char_indices() {
            if scalar == '\n' {
                column = x;
                row += glyphs.height;
                if row + glyphs.height > self.height {
                    return Err(format!("text.line_extent: byte {byte} row {row}").into());
                }
                continue;
            }
            if column + glyphs.width > self.width || row + glyphs.height > self.height {
                return Err(format!("text.glyph_extent: byte {byte} cell {column},{row}").into());
            }
            let block = glyphs.tiles.get(&scalar).ok_or_else(|| {
                format!("text.missing_glyph: byte {byte} U+{:04X}", scalar as u32)
            })?;
            for dy in 0..glyphs.height {
                for dx in 0..glyphs.width {
                    let x = column + dx;
                    let y = row + dy;
                    let tile = block[dy * glyphs.width + dx];
                    let previous = self.cells[y * self.width + x];
                    if previous != tile {
                        changes.push(CellChange {
                            x,
                            y,
                            previous,
                            tile,
                        });
                    }
                }
            }
            column += glyphs.width;
        }
        for change in &changes {
            self.cells[change.y * self.width + change.x] = change.tile;
        }
        Ok(changes)
    }
}
