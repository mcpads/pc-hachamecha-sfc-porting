use crate::{Result, emit::write};
use w65c816::Assembler;
#[derive(Clone, Copy, Debug)]
pub enum Mode {
    One,
    Three,
}
impl Mode {
    fn register(self) -> u8 {
        match self {
            Self::One => 1,
            Self::Three => 3,
        }
    }
    fn tile_bytes(self) -> usize {
        match self {
            Self::One => 32,
            Self::Three => 64,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Map {
    address: u16,
    width: usize,
    height: usize,
}
impl Map {
    pub fn new(address: u16, width: usize, height: usize) -> Result<Self> {
        if ![32, 64].contains(&width)
            || ![32, 64].contains(&height)
            || address % 2048 != 0
            || usize::from(address) + width * height * 2 > 65536
        {
            return Err("bg.map_extent_or_alignment".into());
        }
        Ok(Self {
            address,
            width,
            height,
        })
    }
    fn register(self) -> u8 {
        (self.address / 512) as u8 | u8::from(self.width == 64) | (u8::from(self.height == 64) << 1)
    }
}
pub struct Bg1 {
    mode: Mode,
    characters: u16,
    tile_count: usize,
    map: Map,
}
impl Bg1 {
    pub fn new(mode: Mode, characters: u16, tile_count: usize, map: Map) -> Result<Self> {
        if characters % 8192 != 0 || !(1..=1024).contains(&tile_count) {
            return Err("bg.character_extent_or_alignment".into());
        }
        let value = Self {
            mode,
            characters,
            tile_count,
            map,
        };
        value.validate_map(map)?;
        Ok(value)
    }
    fn validate_map(&self, map: Map) -> Result<()> {
        let start = usize::from(self.characters);
        let end = start + self.tile_count * self.mode.tile_bytes();
        let map_start = usize::from(map.address);
        let map_end = map_start + map.width * map.height * 2;
        if end > 65536 || (start < map_end && map_start < end) {
            return Err("bg.residency_overlap_or_extent".into());
        }
        Ok(())
    }
    pub fn emit_configure(&self, p: &mut Assembler) {
        write(p, 0x2105, self.mode.register());
        write(p, 0x2106, 0);
        write(p, 0x2107, self.map.register());
        write(p, 0x210b, (self.characters / 8192) as u8);
    }
    pub fn emit_select_map(&self, p: &mut Assembler, map: Map) -> Result<()> {
        self.validate_map(map)?;
        write(p, 0x2107, map.register());
        Ok(())
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Scroll {
    horizontal: u16,
    vertical: u16,
}
impl Scroll {
    pub fn new(horizontal: u16, vertical: u16) -> Result<Self> {
        if horizontal > 1023 || vertical > 1023 {
            return Err("bg.scroll_extent".into());
        }
        Ok(Self {
            horizontal,
            vertical,
        })
    }
    pub fn emit(&self, p: &mut Assembler) {
        self.emit_layer(p, Layer::One);
    }
    pub fn emit_layer(&self, p: &mut Assembler, layer: Layer) {
        let base = match layer {
            Layer::One => 0x210d,
            Layer::Two => 0x210f,
        };
        for (address, value) in [(base, self.horizontal), (base + 1, self.vertical)] {
            write(p, address, value as u8);
            write(p, address, (value >> 8) as u8);
        }
    }
}
#[derive(Clone, Copy)]
pub enum Layer {
    One,
    Two,
}
pub struct Layer4bpp {
    base: Bg1,
    maps: Vec<Map>,
}
impl Layer4bpp {
    pub fn new(characters: u16, tile_count: usize, maps: Vec<Map>) -> Result<Self> {
        let map = *maps.first().ok_or("bg.empty_maps")?;
        let base = Bg1::new(Mode::One, characters, tile_count, map)?;
        let result = Self { base, maps };
        reject_overlap(&result.ranges())?;
        Ok(result)
    }
    fn ranges(&self) -> Vec<(usize, usize)> {
        let start = usize::from(self.base.characters);
        let mut ranges = vec![(start, start + self.base.tile_count * 32)];
        ranges.extend(self.maps.iter().map(|m| {
            (
                usize::from(m.address),
                usize::from(m.address) + m.width * m.height * 2,
            )
        }));
        ranges
    }
}
fn reject_overlap(ranges: &[(usize, usize)]) -> Result<()> {
    for (i, &(start, end)) in ranges.iter().enumerate() {
        if end > 65536 || ranges[..i].iter().any(|&(a, b)| start < b && a < end) {
            return Err("bg.residency_overlap_or_extent".into());
        }
    }
    Ok(())
}
pub struct Mode1Pair {
    layers: [Layer4bpp; 2],
}
impl Mode1Pair {
    pub fn new(bg1: Layer4bpp, bg2: Layer4bpp) -> Result<Self> {
        let mut ranges = bg1.ranges();
        ranges.extend(bg2.ranges());
        reject_overlap(&ranges)?;
        Ok(Self { layers: [bg1, bg2] })
    }
    pub fn emit_configure(&self, p: &mut Assembler) {
        write(p, 0x2105, 1);
        write(p, 0x2106, 0);
        write(p, 0x2107, self.layers[0].maps[0].register());
        write(p, 0x2108, self.layers[1].maps[0].register());
        write(
            p,
            0x210b,
            (self.layers[0].base.characters / 8192) as u8
                | ((self.layers[1].base.characters / 8192) as u8) << 4,
        );
    }
    pub fn emit_select_map(&self, p: &mut Assembler, layer: Layer, index: usize) -> Result<()> {
        let i = match layer {
            Layer::One => 0,
            Layer::Two => 1,
        };
        let map = self.layers[i]
            .maps
            .get(index)
            .ok_or("bg.map_not_declared")?;
        write(p, 0x2107 + i as u16, map.register());
        Ok(())
    }
}
