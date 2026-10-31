use crate::{Result, SOURCE, sha};
use serde::Deserialize;
use std::collections::BTreeMap;
#[derive(Deserialize)]
struct Sheet {
    file: String,
    width: usize,
    height: usize,
    pixels_sha256: String,
    palette_sha256: String,
}
#[derive(Clone, Deserialize)]
pub struct Picture {
    pub entry_va: u32,
    resource: String,
    rectangle: [usize; 4],
    position: [i32; 2],
    palette_base: u8,
    key: u8,
    z: i32,
    pub leaving_song: Option<u8>,
    pub surface: bool,
    #[serde(default)]
    palette_va: Option<u32>,
    #[serde(default)]
    pub palette_writes: Vec<[u8; 4]>,
}
#[derive(Deserialize)]
struct FloorObjects {
    floor: u8,
    objects: Vec<Picture>,
}
#[derive(Deserialize)]
struct Pictures {
    schema: u8,
    source_sha256: String,
    resources: Vec<Sheet>,
    floors: Vec<FloorObjects>,
}
pub struct FloorPictures {
    pub pictures: BTreeMap<u32, Picture>,
    sheets: BTreeMap<String, (Sheet, Vec<u8>, Vec<u8>)>,
    pub definition_sha256: String,
    sprites: BTreeMap<u32, (Vec<u8>, Vec<u8>)>,
}
impl FloorPictures {
    pub fn load(content: &crate::content::Content, floors: &[u8]) -> Result<Self> {
        let root = content.sub("floor-objects");
        let raw = root.read("floor-objects.json")?;
        let d: Pictures = serde_json::from_slice(&raw)?;
        if d.schema != 1 || d.source_sha256 != SOURCE {
            return Err("Invalid floor object pictures".into());
        }
        let mut objects = Vec::new();
        for floor in floors {
            let from = d
                .floors
                .iter()
                .find(|f| f.floor == *floor)
                .ok_or("Missing floor object pictures")?;
            objects.extend(from.objects.iter().cloned());
        }
        let mut sheets = BTreeMap::new();
        for sheet in d.resources {
            if !objects.iter().any(|o| o.resource == sheet.file) {
                continue;
            }
            if !sheet
                .file
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"_.".contains(&c))
                || sheet.file.contains("..")
            {
                return Err("Invalid object sheet filename".into());
            }
            let pixels = root.read(format!("{}.indices", sheet.file))?;
            let palette = root.read(format!("{}.rgb", sheet.file))?;
            if pixels.len() != sheet.width * sheet.height
                || palette.len() != 768
                || sha(&pixels) != sheet.pixels_sha256
                || sha(&palette) != sheet.palette_sha256
            {
                return Err("Object sheet hash or dimensions differ".into());
            }
            sheets.insert(sheet.file.clone(), (sheet, pixels, palette));
        }
        Ok(Self {
            pictures: objects.into_iter().map(|o| (o.entry_va, o)).collect(),
            sheets,
            definition_sha256: sha(&raw),
            sprites: BTreeMap::new(),
        })
    }
    pub fn with_sprites(mut self, sprites: &[(u32, u8, Vec<u8>, Vec<u8>)]) -> Self {
        for (entry, _, body, gem) in sprites {
            if self.pictures.contains_key(entry) {
                self.sprites.insert(*entry, (body.clone(), gem.clone()));
            }
        }
        self
    }
    pub fn recoloured(&self, va: u32) -> Result<Option<(u32, u32)>> {
        let Some(to) = self.pictures.values().find(|p| p.palette_va == Some(va)) else {
            return Ok(None);
        };
        let same = |p: &&Picture| {
            p.palette_va.is_none()
                && p.resource == to.resource
                && p.rectangle == to.rectangle
                && p.position == to.position
                && p.palette_base == to.palette_base
                && p.key == to.key
                && p.z == to.z
        };
        let mut from = self.pictures.values().filter(same);
        match (from.next(), from.next()) {
            (Some(f), None) => Ok(Some((f.entry_va, to.entry_va))),
            _ => Err(format!("Colour list {va:#x} has no single uncoloured picture").into()),
        }
    }
    pub fn paint(
        &self,
        entries: &[u32],
        shift: [i32; 2],
        pixels: &mut [u8],
        palette: &mut [u8],
        ranges: &[[usize; 2]],
    ) -> Result<()> {
        let mut order: Vec<&Picture> = entries
            .iter()
            .map(|e| self.pictures.get(e).ok_or("Unknown object picture"))
            .collect::<std::result::Result<_, _>>()?;
        order.sort_by_key(|p| std::cmp::Reverse(p.z));
        let mut written = BTreeMap::new();
        let inside = |base: usize| ranges.iter().any(|[a, b]| (*a..*b).contains(&base));
        let sprites = order.iter().any(|p| self.sprites.contains_key(&p.entry_va));
        let taken: Vec<usize> = order
            .iter()
            .filter(|p| inside(usize::from(p.palette_base)))
            .map(|p| usize::from(p.palette_base) & !15)
            .chain(
                sprites
                    .then_some([
                        crate::sfc_field_guardian::GEM_BLOCK,
                        crate::sfc_field_guardian::BODY_BLOCK,
                    ])
                    .into_iter()
                    .flatten()
                    .map(usize::from),
            )
            .collect();
        let mut free = ranges
            .iter()
            .flat_map(|&[a, b]| (a.next_multiple_of(16)..b).step_by(16))
            .filter(|block| *block >= 128 && !taken.contains(block));
        for picture in order {
            if let Some((body, gem)) = self.sprites.get(&picture.entry_va) {
                if shift != [0, 0] || !picture.palette_writes.is_empty() {
                    return Err("Sprite picture moved or recoloured".into());
                }
                for (block, rgb) in [
                    (crate::sfc_field_guardian::BODY_BLOCK, body),
                    (crate::sfc_field_guardian::GEM_BLOCK, gem),
                ] {
                    for (i, c) in rgb.as_chunks::<3>().0.iter().enumerate().skip(1) {
                        let value = u16::from(c[0] >> 3)
                            | u16::from(c[1] >> 3) << 5
                            | u16::from(c[2] >> 3) << 10;
                        let index = block + i as u8;
                        if *written.entry(index).or_insert(value) != value {
                            return Err("Object pictures share a palette entry".into());
                        }
                        let at = usize::from(index) * 2;
                        palette[at..at + 2].copy_from_slice(&value.to_le_bytes());
                    }
                }
                continue;
            }
            let (sheet, source, colors) = &self.sheets[&picture.resource];
            let base = if inside(usize::from(picture.palette_base)) {
                picture.palette_base
            } else {
                u8::try_from(free.next().ok_or("No free picture palette block")?)?
            };
            let [l, t, r, b] = picture.rectangle;
            if l >= r || t >= b || r > sheet.width || b > sheet.height {
                return Err("Invalid object picture rectangle".into());
            }
            let mut used = BTreeMap::new();
            for sy in t..b {
                for sx in l..r {
                    let x = picture.position[0] + shift[0] + (sx - l) as i32;
                    let y = picture.position[1] + shift[1] + (sy - t) as i32;
                    let index = source[sy * sheet.width + sx]
                        .checked_add(picture.palette_base)
                        .ok_or("Object picture colour overflow")?;
                    if index == picture.key || !(0..224).contains(&x) || !(0..240).contains(&y) {
                        continue;
                    }
                    let index = source[sy * sheet.width + sx] + base;
                    pixels[y as usize * 224 + x as usize] = index;
                    used.insert(index, source[sy * sheet.width + sx]);
                }
            }
            for (index, color) in used {
                let source = color.checked_add(picture.palette_base);
                let c = match picture.palette_writes.iter().find(|w| Some(w[0]) == source) {
                    Some(w) => &w[1..4],
                    None => &colors[usize::from(color) * 3..usize::from(color) * 3 + 3],
                };
                let value =
                    u16::from(c[0] >> 3) | u16::from(c[1] >> 3) << 5 | u16::from(c[2] >> 3) << 10;
                if *written.entry(index).or_insert(value) != value {
                    return Err("Object pictures share a palette entry".into());
                }
                let at = usize::from(index) * 2;
                palette[at..at + 2].copy_from_slice(&value.to_le_bytes());
            }
        }
        Ok(())
    }
}
