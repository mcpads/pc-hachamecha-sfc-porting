use crate::{
    Result,
    tiles::{self, Depth},
};
use std::collections::BTreeMap;
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Size {
    Small,
    Large,
}
impl Size {
    fn edge(self) -> usize {
        match self {
            Self::Small => 8,
            Self::Large => 16,
        }
    }
    fn cells(self, tile: usize) -> Vec<usize> {
        match self {
            Self::Small => vec![tile],
            Self::Large => vec![tile, tile + 1, tile + 16, tile + 17],
        }
    }
    fn fits(self, tile: usize) -> bool {
        tile < 512 && (self == Self::Small || (tile % 16 < 15 && tile % 256 / 16 < 15))
    }
}
pub struct SpriteImage<'a> {
    pub name: &'a str,
    pub size: Size,
    pub pixels: &'a [u8],
}
pub struct SpriteAtlas {
    bytes: Vec<u8>,
    entries: BTreeMap<String, (Size, u16)>,
    sources: Vec<serde_json::Value>,
}
impl SpriteAtlas {
    pub fn new(images: &[SpriteImage<'_>]) -> Result<Self> {
        if images.is_empty() {
            return Err("sprite.empty_atlas".into());
        }
        let mut ordered = BTreeMap::new();
        for image in images {
            if image.name.is_empty()
                || image.name.chars().any(char::is_control)
                || image.pixels.len() != image.size.edge().pow(2)
            {
                return Err(format!("sprite.image_domain: {}", image.name).into());
            }
            if let Some((offset, value)) = image.pixels.iter().enumerate().find(|(_, v)| **v > 15) {
                return Err(format!(
                    "sprite.pixel: {} x {} y {} index {}",
                    image.name,
                    offset % image.size.edge(),
                    offset / image.size.edge(),
                    value
                )
                .into());
            }
            if ordered.insert(image.name, image).is_some() {
                return Err(format!("sprite.duplicate_name: {}", image.name).into());
            }
        }
        let mut occupied = [false; 512];
        let mut tiles = vec![vec![0; 64]; 512];
        let mut unique = BTreeMap::new();
        let mut entries = BTreeMap::new();
        let mut sources = vec![];
        for (name, image) in ordered {
            let key = (image.size, image.pixels.to_vec());
            let first = if let Some(&tile) = unique.get(&key) {
                tile
            } else {
                let first = (0..512)
                    .find(|&t| {
                        image.size.fits(t) && image.size.cells(t).iter().all(|&i| !occupied[i])
                    })
                    .ok_or_else(|| format!("sprite.atlas_full: {name}"))?;
                let edge = image.size.edge();
                for (i, tile) in image.size.cells(first).into_iter().enumerate() {
                    occupied[tile] = true;
                    let tx = i % (edge / 8);
                    let ty = i / (edge / 8);
                    tiles[tile] = (0..64)
                        .map(|p| image.pixels[(ty * 8 + p / 8) * edge + tx * 8 + p % 8])
                        .collect();
                }
                unique.insert(key, first);
                first
            };
            entries.insert(name.to_string(), (image.size, first as u16));
            sources.push(serde_json::json!(
                { "name" : name, "edge" : image.size.edge(), "tile" : first,
                "source_pixels_sha256" : crate ::sha256(image.pixels) }
            ));
        }
        let count = occupied.iter().rposition(|v| *v).unwrap() + 1;
        Ok(Self {
            bytes: tiles::encode(&tiles[..count], Depth::Four)?,
            entries,
            sources,
        })
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn entry(&self, name: &str) -> Result<(Size, u16)> {
        self.entries
            .get(name)
            .copied()
            .ok_or_else(|| format!("sprite.missing_image: {name}").into())
    }
    pub fn report(&self) -> serde_json::Value {
        serde_json::json!(
            { "obsel" : 0, "character_base" : 0, "second_page" : 8192,
            "transparent_index" : 0, "native_bytes" : self.bytes.len(), "native_sha256" :
            crate ::sha256(& self.bytes), "sources" : self.sources }
        )
    }
}
#[derive(Clone, Copy)]
pub struct Object {
    pub x: i16,
    pub y: u8,
    pub tile: u16,
    pub size: Size,
    pub palette: u8,
    pub priority: u8,
    pub flip_x: bool,
    pub flip_y: bool,
}
pub struct Oam {
    bytes: Vec<u8>,
    scanlines: Vec<ScanlineLoad>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScanlineLoad {
    pub line: u16,
    pub objects: usize,
    pub tiles: usize,
}
impl Oam {
    pub fn with_scanline_limits(objects: &[Object]) -> Result<Self> {
        let oam = Self::new(objects)?;
        if let Some(load) = oam
            .scanlines
            .iter()
            .find(|l| l.objects > 32 || l.tiles > 34)
        {
            return Err(format!(
                "sprite.scanline_limit: line {} objects {} tiles {}",
                load.line, load.objects, load.tiles
            )
            .into());
        }
        Ok(oam)
    }
    pub fn new(objects: &[Object]) -> Result<Self> {
        if objects.len() > 128 {
            return Err("sprite.oam_capacity".into());
        }
        let mut bytes = vec![0; 544];
        for i in 0..128 {
            bytes[i * 4] = 0xf0;
            bytes[i * 4 + 1] = 240;
            bytes[512 + i / 4] |= 1 << ((i % 4) * 2);
        }
        for (i, o) in objects.iter().enumerate() {
            if !(-256..=255).contains(&o.x)
                || o.palette > 7
                || o.priority > 3
                || !o.size.fits(usize::from(o.tile))
            {
                return Err(format!("sprite.object_domain: slot {i}").into());
            }
            bytes[i * 4] = o.x as u8;
            bytes[i * 4 + 1] = o.y;
            bytes[i * 4 + 2] = o.tile as u8;
            bytes[i * 4 + 3] = ((o.tile >> 8) as u8)
                | o.palette << 1
                | o.priority << 4
                | u8::from(o.flip_x) << 6
                | u8::from(o.flip_y) << 7;
            let shift = (i % 4) * 2;
            bytes[512 + i / 4] = (bytes[512 + i / 4] & !(3 << shift))
                | ((u8::from(o.x < 0) | u8::from(o.size == Size::Large) << 1) << shift);
        }
        let scanlines = (0..224u16)
            .map(|line| {
                let mut load = ScanlineLoad {
                    line,
                    objects: 0,
                    tiles: 0,
                };
                for o in objects {
                    let edge = o.size.edge() as i16;
                    if (line as u8).wrapping_sub(o.y) as i16 >= edge
                        || (o.x != -256 && o.x + edge <= 0)
                    {
                        continue;
                    }
                    load.objects += 1;
                    load.tiles += (0..edge / 8)
                        .filter(|&column| {
                            let x = if o.x == -256 {
                                column * 8
                            } else {
                                o.x + column * 8
                            };
                            x + 8 > 0 && x < 256
                        })
                        .count();
                }
                load
            })
            .collect();
        Ok(Self { bytes, scanlines })
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn scanlines(&self) -> &[ScanlineLoad] {
        &self.scanlines
    }
    pub fn report(&self) -> serde_json::Value {
        serde_json::json!(
            { "visible_lines" : 224, "object_limit" : 32, "tile_limit" : 34,
            "maximum_objects" : self.scanlines.iter().map(| l | l.objects).max()
            .unwrap(), "maximum_tiles" : self.scanlines.iter().map(| l | l.tiles).max()
            .unwrap(), "admitted" : self.scanlines.iter().all(| l | l.objects <= 32 && l
            .tiles <= 34), "active_lines" : self.scanlines.iter().filter(| l | l.objects
            > 0).map(| l | serde_json::json!({ "line" : l.line, "objects" : l.objects,
            "tiles" : l.tiles })).collect::< Vec < _ >> () }
        )
    }
}
