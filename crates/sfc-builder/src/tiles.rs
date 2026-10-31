use crate::Result;
use asset_types::{CanonicalFourBitCharacters, CanonicalTwoBitCharacters};
use sfc_codecs::asset_codec::{encode_sfc_planar_2bpp, encode_sfc_planar_4bpp};
use std::collections::BTreeMap;
#[derive(Clone, Copy, Debug)]
pub enum Depth {
    Two,
    Four,
    Eight,
}
impl Depth {
    pub fn colors(self) -> u16 {
        match self {
            Self::Two => 4,
            Self::Four => 16,
            Self::Eight => 256,
        }
    }
}
pub(crate) fn encode(pixels: &[Vec<u8>], depth: Depth) -> Result<Vec<u8>> {
    match depth {
        Depth::Two => Ok(encode_sfc_planar_2bpp(&CanonicalTwoBitCharacters::new(
            pixels.to_vec(),
        )?)),
        Depth::Four => Ok(encode_sfc_planar_4bpp(&CanonicalFourBitCharacters::new(
            pixels.to_vec(),
        )?)),
        Depth::Eight => {
            if pixels.is_empty() || pixels.iter().any(|p| p.len() != 64) {
                return Err("tiles.character_extent".into());
            }
            let low = pixels
                .iter()
                .map(|p| p.iter().map(|v| v & 15).collect())
                .collect();
            let high = pixels
                .iter()
                .map(|p| p.iter().map(|v| v >> 4).collect())
                .collect();
            let low = encode_sfc_planar_4bpp(&CanonicalFourBitCharacters::new(low)?);
            let high = encode_sfc_planar_4bpp(&CanonicalFourBitCharacters::new(high)?);
            Ok(low
                .chunks_exact(32)
                .zip(high.chunks_exact(32))
                .flat_map(|(l, h)| l.iter().chain(h).copied())
                .collect())
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Attributes {
    pub palette: u8,
    pub priority: bool,
}
pub struct IndexedImage<'a> {
    pub width: usize,
    pub height: usize,
    pub pixels: &'a [u8],
    pub attributes: &'a [Attributes],
}
pub struct TileImage {
    width: usize,
    height: usize,
    depth: Depth,
    first_tile: u16,
    tiles: Vec<Vec<u8>>,
    native: Vec<u8>,
    cells: Vec<u16>,
    source_sha256: String,
    source_attributes: Vec<serde_json::Value>,
}
fn flipped(pixels: &[u8], h: bool, v: bool) -> Vec<u8> {
    (0..64)
        .map(|i| {
            let x = if h { 7 - i % 8 } else { i % 8 };
            let y = if v { 7 - i / 8 } else { i / 8 };
            pixels[y * 8 + x]
        })
        .collect()
}
impl TileImage {
    pub fn new(image: IndexedImage<'_>, depth: Depth, first_tile: u16) -> Result<Self> {
        if image.width == 0
            || image.height == 0
            || image.width > 512
            || image.height > 512
            || image.width % 8 != 0
            || image.height % 8 != 0
            || image.pixels.len() != image.width * image.height
            || image.attributes.len() != image.width / 8 * (image.height / 8)
            || first_tile > 1023
        {
            return Err(
                "tiles.image_extent: require 8-aligned dimensions 8..512 and exact pixels/attributes"
                    .into(),
            );
        }
        if let Some((i, value)) = image
            .pixels
            .iter()
            .enumerate()
            .find(|(_, v)| u16::from(**v) >= depth.colors())
        {
            return Err(format!(
                "tiles.pixel: x {} y {} index {value}",
                i % image.width,
                i / image.width
            )
            .into());
        }
        let mut tiles = vec![];
        let mut unique = BTreeMap::new();
        let mut cells = vec![];
        for ty in 0..image.height / 8 {
            for tx in 0..image.width / 8 {
                let attr = image.attributes[ty * (image.width / 8) + tx];
                if attr.palette > 7 || (matches!(depth, Depth::Eight) && attr.palette != 0) {
                    return Err(
                        format!("tiles.palette: cell {tx},{ty} palette {}", attr.palette).into(),
                    );
                }
                let pixels: Vec<_> = (0..64)
                    .map(|i| image.pixels[(ty * 8 + i / 8) * image.width + tx * 8 + i % 8])
                    .collect();
                let (canonical, h, v) =
                    [(false, false), (true, false), (false, true), (true, true)]
                        .into_iter()
                        .map(|(h, v)| (flipped(&pixels, h, v), h, v))
                        .min()
                        .unwrap();
                let tile = if let Some(tile) = unique.get(&canonical) {
                    *tile
                } else {
                    let index = usize::from(first_tile) + tiles.len();
                    if index > 1023 {
                        return Err(format!("tiles.name_extent: cell {tx},{ty}").into());
                    }
                    unique.insert(canonical.clone(), index as u16);
                    tiles.push(canonical);
                    index as u16
                };
                cells.push(
                    tile | (u16::from(attr.palette) << 10)
                        | (u16::from(attr.priority) << 13)
                        | (u16::from(h) << 14)
                        | (u16::from(v) << 15),
                );
            }
        }
        let native = encode(&tiles, depth)?;
        Ok(Self {
            width: image.width,
            height: image.height,
            depth,
            first_tile,
            tiles,
            native,
            cells,
            source_sha256: crate::sha256(image.pixels),
            source_attributes: image
                .attributes
                .iter()
                .map(|a| serde_json::json!({ "palette" : a.palette, "priority" : a.priority }))
                .collect(),
        })
    }
    pub fn native_tiles(&self) -> &[u8] {
        &self.native
    }
    pub fn cells(&self) -> &[u16] {
        &self.cells
    }
    pub fn bg_map(&self, width: usize, height: usize, blank_tile: u16) -> Result<Vec<u8>> {
        encode_map(
            self.width,
            self.height,
            &self.cells,
            width,
            height,
            blank_tile,
        )
    }
    pub fn report(&self) -> serde_json::Value {
        serde_json::json!(
            { "width" : self.width, "height" : self.height, "depth" : format!("{:?}",
            self.depth), "transparent_index" : 0, "first_tile" : self.first_tile,
            "unique_tiles" : self.tiles.len(), "source_pixels_sha256" : self
            .source_sha256, "source_attributes" : self.source_attributes, "native_sha256"
            : crate ::sha256(& self.native), "native_bytes" : self.native.len(), "cells"
            : self.cells, "origin" : { "x" : 0, "y" : 0 }, "scope" :
            "indexed BG image, one resident tile group; no quantization, Mode 7 or transfer deadline"
            }
        )
    }
}
fn encode_map(
    image_width: usize,
    image_height: usize,
    cells: &[u16],
    width: usize,
    height: usize,
    blank_tile: u16,
) -> Result<Vec<u8>> {
    if ![32, 64].contains(&width)
        || ![32, 64].contains(&height)
        || image_width / 8 > width
        || image_height / 8 > height
        || blank_tile > 1023
    {
        return Err("tiles.bg_map_extent".into());
    }
    let mut out = vec![0u8; width * height * 2];
    for y in 0..height {
        for x in 0..width {
            let word = if x < image_width / 8 && y < image_height / 8 {
                cells[y * (image_width / 8) + x]
            } else {
                blank_tile
            };
            let index = ((y / 32) * (width / 32) + x / 32) * 1024 + (y % 32) * 32 + x % 32;
            out[index * 2..index * 2 + 2].copy_from_slice(&word.to_le_bytes());
        }
    }
    Ok(out)
}
pub struct FrameInput<'a> {
    pub name: &'a str,
    pub image: IndexedImage<'a>,
    pub origin: [i32; 2],
}
pub struct ResidentFrame {
    width: usize,
    height: usize,
    cells: Vec<u16>,
    origin: [i32; 2],
    source_sha256: String,
    attributes: Vec<serde_json::Value>,
}
impl ResidentFrame {
    pub fn cells(&self) -> &[u16] {
        &self.cells
    }
    pub fn origin(&self) -> [i32; 2] {
        self.origin
    }
    pub fn bg_map(&self, width: usize, height: usize, blank_tile: u16) -> Result<Vec<u8>> {
        encode_map(
            self.width,
            self.height,
            &self.cells,
            width,
            height,
            blank_tile,
        )
    }
    fn report(&self) -> serde_json::Value {
        serde_json::json!(
            { "width" : self.width, "height" : self.height, "cells" : self.cells,
            "origin" : self.origin, "source_pixels_sha256" : self.source_sha256,
            "source_attributes" : self.attributes }
        )
    }
}
pub struct ResidentImages {
    frames: BTreeMap<String, ResidentFrame>,
    native: Vec<u8>,
    first_tile: u16,
    tile_count: usize,
    depth: Depth,
}
impl ResidentImages {
    pub fn new(inputs: Vec<FrameInput<'_>>, depth: Depth, first_tile: u16) -> Result<Self> {
        if inputs.is_empty() || first_tile > 1023 {
            return Err("tiles.resident_extent".into());
        }
        let mut ordered = BTreeMap::new();
        for input in inputs {
            if input.name.is_empty() || input.name.chars().any(char::is_control) {
                return Err("tiles.frame_name".into());
            }
            if ordered.insert(input.name, input).is_some() {
                return Err("tiles.duplicate_frame".into());
            }
        }
        let mut frames = BTreeMap::new();
        let mut tiles = vec![];
        let mut unique = BTreeMap::new();
        for (name, input) in ordered {
            let image = TileImage::new(input.image, depth, 0)
                .map_err(|e| format!("tiles.frame {name}: {e}"))?;
            let mut remap = vec![];
            for pixels in image.tiles {
                let tile = if let Some(tile) = unique.get(&pixels) {
                    *tile
                } else {
                    let index = usize::from(first_tile) + tiles.len();
                    if index > 1023 {
                        return Err(format!("tiles.resident_name_extent: frame {name}").into());
                    }
                    unique.insert(pixels.clone(), index as u16);
                    tiles.push(pixels);
                    index as u16
                };
                remap.push(tile);
            }
            let cells = image
                .cells
                .iter()
                .map(|c| (c & !1023) | remap[usize::from(c & 1023)])
                .collect();
            frames.insert(
                name.into(),
                ResidentFrame {
                    width: image.width,
                    height: image.height,
                    cells,
                    origin: input.origin,
                    source_sha256: image.source_sha256,
                    attributes: image.source_attributes,
                },
            );
        }
        Ok(Self {
            native: encode(&tiles, depth)?,
            tile_count: tiles.len(),
            frames,
            first_tile,
            depth,
        })
    }
    pub fn frame(&self, name: &str) -> Result<&ResidentFrame> {
        self.frames
            .get(name)
            .ok_or_else(|| format!("tiles.unknown_frame: {name}").into())
    }
    pub fn native_tiles(&self) -> &[u8] {
        &self.native
    }
    pub fn report(&self) -> serde_json::Value {
        serde_json::json!(
            { "depth" : format!("{:?}", self.depth), "first_tile" : self.first_tile,
            "unique_tiles" : self.tile_count, "native_bytes" : self.native.len(),
            "native_sha256" : crate ::sha256(& self.native), "frames" : self.frames
            .iter().map(| (name, f) | (name, f.report())).collect::< BTreeMap < _, _ >>
            (), "scope" :
            "one immutable resident tile range; consumer owns palette residency, placement and scheduling"
            }
        )
    }
}
