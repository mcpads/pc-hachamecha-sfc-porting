use crate::{Result, SOURCE, font::DialogueFont, sha};
use serde::Deserialize;
use sfc_builder::{
    cartridge::{CartridgeLayout, Section},
    tiles::{Attributes, Depth, IndexedImage, TileImage},
};
use std::collections::BTreeMap;
pub const TEXT: &str = "처음부터계속©COMPILE 1997세이브데이타1234역시 처음부터";
const MODE: u16 = 0x22d;
const BRIGHTNESS: u16 = 0x22f;
const FRAME: u16 = 0x6e0;
const SCROLL: u16 = 0x6e2;
const SCRATCH: u16 = 0x6e4;
const CHOICE: u16 = 0x6e6;
const CONTINUING: u16 = 0x6e7;
const FADE_STEP: u8 = 4;
const BG2_TILES: u16 = 0x0000;
const BG1_BASE: u16 = 0x6000;
const LOGO_NAME: u16 = 64;
const OBJ_BASE: u16 = 0xc000;
const GLYPH_NAME: u16 = 64;
const TEXT_SHEET: u16 = 0xe000;
const BG2_MAP: u16 = 0xe800;
const BG1_MAP: u16 = 0xf000;
const BG2_PALETTES: usize = 6;
const LOGO_COLORS: usize = 96;
const OBJ_PALETTE: usize = 240;
const DIM_PALETTE: usize = 224;
const GLYPH: u8 = 1;
const GLYPH_SHADOW: u8 = 2;
const WHITE: u8 = 3;
const GREY: u8 = 4;
const SHADOW: u8 = 5;
const COPYRIGHT: u8 = 6;
const LOGO_TICKS: usize = 152;
const SUBTITLE_TICK: usize = 152;
const COPYRIGHT_TICK: usize = 302;
const MENU_TICK: usize = 430;
const GLYPH_DELAYS: [usize; 8] = [8, 16, 24, 32, 56, 64, 72, 80];
const GLYPH_ANGLES: [u16; 8] = [
    0xb400, 0xa800, 0x9e00, 0x9200, 0x6e00, 0x6200, 0x5600, 0x4a00,
];
const GLYPH_TICKS: usize = 31;
const STREAM_BUDGET: usize = super::sfc_b1::TITLE_STREAM_BUDGET as usize;
const DESCRIPTOR_COST: usize = super::sfc_b1::DESCRIPTOR_COST as usize;
const HIDDEN_Y: u8 = 224;
#[derive(Deserialize)]
struct Resource {
    file: String,
    width: usize,
    height: usize,
    colors: usize,
    palette_base: usize,
    key: Option<usize>,
    pixels_sha256: String,
    palette_sha256: String,
}
#[derive(Deserialize)]
struct Draw {
    file: String,
    rectangle: [usize; 4],
    position: [i64; 2],
}
#[derive(Deserialize)]
struct Definition {
    resources: Vec<Resource>,
    draws: Vec<Draw>,
    source_rectangles: Vec<[usize; 4]>,
}
#[derive(Clone, PartialEq)]
struct Sprite {
    x: i32,
    y: i32,
    name: u16,
    large: bool,
    dim: bool,
}
type Upload = (u16, u8, Vec<u8>);
pub struct Assets {
    entry: Vec<Upload>,
    frames: Vec<Vec<Upload>>,
    last: Vec<Upload>,
    hidden_oam: Vec<u8>,
    menus: Vec<Vec<Upload>>,
    slot_text: Vec<Upload>,
    scroll: Vec<u16>,
    pub report: serde_json::Value,
}
fn rgb15(c: [u8; 3]) -> u16 {
    u16::from(c[0] >> 3) | u16::from(c[1] >> 3) << 5 | u16::from(c[2] >> 3) << 10
}
fn channels(c: u16) -> [u16; 3] {
    [c & 31, c >> 5 & 31, c >> 10 & 31]
}
fn distance(a: u16, b: u16) -> u32 {
    channels(a)
        .iter()
        .zip(channels(b))
        .zip([3, 4, 2])
        .map(|((x, y), w)| w * u32::from(*x).abs_diff(u32::from(y)).pow(2))
        .sum()
}
fn sheet(pixels: &[u8], width: usize) -> Vec<u8> {
    let height = pixels.len() / width;
    let mut bytes = Vec::new();
    for ty in 0..height / 8 {
        for tx in 0..width / 8 {
            let mut tile = [0u8; 32];
            for y in 0..8 {
                for x in 0..8 {
                    let color = pixels[(ty * 8 + y) * width + tx * 8 + x];
                    for plane in 0..4 {
                        tile[plane / 2 * 16 + y * 2 + plane % 2] |=
                            ((color >> plane) & 1) << (7 - x);
                    }
                }
            }
            bytes.extend(tile);
        }
    }
    bytes
}
fn text(
    pixels: &mut [u8],
    width: usize,
    font: &DialogueFont,
    value: &str,
    [left, top]: [usize; 2],
    color: u8,
) -> Result<(usize, usize)> {
    let mut lift = usize::MAX;
    for ch in value.chars() {
        lift = lift.min(
            font.glyph(ch)?
                .ink
                .iter()
                .map(|i| i[1])
                .min()
                .unwrap_or(lift),
        );
    }
    let mut x = left;
    for ch in value.chars() {
        let glyph = font.glyph(ch)?;
        for (sx, sy, c) in [(1, 1, SHADOW), (0, 0, color)] {
            for [gx, gy] in &glyph.ink {
                pixels[(top + gy + sy - lift) * width + x + gx + sx] = c;
            }
        }
        x += glyph.advance;
    }
    Ok((x + 1 - left, lift))
}
fn quantize(cells: &[BTreeMap<u16, usize>]) -> (u16, Vec<Vec<u16>>, Vec<usize>) {
    let mut total: BTreeMap<u16, usize> = BTreeMap::new();
    for cell in cells {
        for (&c, &n) in cell {
            *total.entry(c).or_default() += n;
        }
    }
    let backdrop = total
        .iter()
        .max_by_key(|(c, n)| (**n, std::cmp::Reverse(**c)))
        .map_or(0, |(c, _)| *c);
    let cost = |weights: &BTreeMap<u16, usize>, palette: &[u16]| -> u64 {
        weights
            .iter()
            .map(|(&c, &n)| {
                let nearest = palette
                    .iter()
                    .chain([&backdrop])
                    .map(|&p| distance(c, p))
                    .min()
                    .unwrap_or(0);
                n as u64 * u64::from(nearest)
            })
            .sum()
    };
    let choose = |weights: &BTreeMap<u16, usize>| -> Vec<u16> {
        let mut chosen: Vec<u16> = Vec::new();
        let mut candidates: Vec<u16> = weights.keys().copied().filter(|&c| c != backdrop).collect();
        while chosen.len() < 15 && !candidates.is_empty() {
            let at = (0..candidates.len())
                .min_by_key(|&i| {
                    let mut trial = chosen.clone();
                    trial.push(candidates[i]);
                    (cost(weights, &trial), i)
                })
                .unwrap_or(0);
            chosen.push(candidates.remove(at));
        }
        chosen
    };
    let mut order: Vec<usize> = (0..cells.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(cells[i].len()));
    let mut palettes: Vec<Vec<u16>> = order[..BG2_PALETTES]
        .iter()
        .map(|&i| choose(&cells[i]))
        .collect();
    let assign = |palettes: &[Vec<u16>]| -> Vec<usize> {
        cells
            .iter()
            .map(|cell| {
                (0..palettes.len())
                    .min_by_key(|&p| (cost(cell, &palettes[p]), p))
                    .unwrap_or(0)
            })
            .collect()
    };
    for _ in 0..12 {
        let assigned = assign(&palettes);
        for (p, palette) in palettes.iter_mut().enumerate() {
            let mut weights = BTreeMap::new();
            for (cell, _) in cells.iter().zip(&assigned).filter(|(_, a)| **a == p) {
                for (&c, &n) in cell {
                    *weights.entry(c).or_default() += n;
                }
            }
            if !weights.is_empty() {
                *palette = choose(&weights);
            }
        }
    }
    let assigned = assign(&palettes);
    (backdrop, palettes, assigned)
}
fn glyph_motion(m: usize) -> (f64, f64) {
    let m = m as i64;
    let (amp, scale) = match m {
        1..=25 => (100 - 5 * m, 0x18000 - (m - 1) * 0x666),
        26..=30 => (-25 + 5 * (m - 25), 0xe00a + (m - 26) * 0x666),
        _ => (0, 0x10000),
    };
    (amp as f64, scale as f64 / 65536.0)
}
struct GlyphSource {
    pixels: Vec<bool>,
    width: usize,
    height: usize,
    rest: [f64; 2],
}
impl GlyphSource {
    fn covers(&self, at: [f64; 2], scale: f64, x: i32, y: i32) -> bool {
        let q = [f64::from(x) * 640.0 / 256.0, f64::from(y) * 480.0 / 224.0];
        let [w, h] = [self.width as f64, self.height as f64];
        let u = ((q[0] - at[0] - w / 2.0) / scale + w / 2.0).floor();
        let v = ((q[1] - at[1] - h / 2.0) / scale + h / 2.0).floor();
        u >= 0.0 && v >= 0.0 && u < w && v < h && self.pixels[v as usize * self.width + u as usize]
    }
}
struct GlyphFrame {
    image: Vec<u8>,
    origin: [i32; 2],
    lower: bool,
}
fn glyph_frame(fg: &GlyphSource, shadow: &GlyphSource, angle: u16, m: usize) -> GlyphFrame {
    let (amp, scale) = glyph_motion(m);
    let turn = f64::from(angle.wrapping_add(8) >> 4) * std::f64::consts::TAU / 4096.0;
    let offset = [turn.sin() * amp, -turn.cos() * amp];
    let place = |rest: [f64; 2]| [(rest[0] + offset[0]).trunc(), (rest[1] + offset[1]).trunc()];
    let (fg_at, shadow_at) = (place(fg.rest), place(shadow.rest));
    let mut drawn = Vec::new();
    for y in -64..288 {
        for x in -64..320 {
            if fg.covers(fg_at, scale, x, y) {
                drawn.push((x, y, GLYPH));
            } else if shadow.covers(shadow_at, 1.0, x, y) {
                drawn.push((x, y, GLYPH_SHADOW));
            }
        }
    }
    let span = |values: Vec<i32>| {
        (
            values.iter().copied().min().unwrap_or(0),
            values.iter().copied().max().unwrap_or(0) + 1,
        )
    };
    let (x0, x1) = span(drawn.iter().map(|d| d.0).collect());
    let (y0, y1) = span(drawn.iter().map(|d| d.1).collect());
    let bx = x0 - (32 - (x1 - x0)).div_euclid(2);
    let by = if y1 - y0 <= 32 {
        y1 - 32
    } else {
        y0 - (48 - (y1 - y0)).div_euclid(2)
    };
    let mut image = vec![0u8; 32 * 48];
    for (x, y, c) in drawn {
        let (u, v) = (x - bx, y - by);
        if (0..32).contains(&u) && (0..48).contains(&v) {
            image[v as usize * 32 + u as usize] = c;
        }
    }
    let lower = image[32 * 32..].iter().any(|&c| c != 0);
    GlyphFrame {
        image,
        origin: [bx, by],
        lower,
    }
}
fn oam(sprites: &[Sprite]) -> Vec<u8> {
    let mut low = Vec::new();
    let mut high = vec![0u8; sprites.len().div_ceil(4)];
    for (i, s) in sprites.iter().enumerate() {
        let x = s.x.rem_euclid(512) as u16;
        let y = if s.y < -32 || s.y >= 224 {
            HIDDEN_Y
        } else {
            s.y.rem_euclid(256) as u8
        };
        let palette = if s.dim { 0x0c } else { 0x0e };
        low.extend([
            x as u8,
            y,
            s.name as u8,
            0x30 | palette | (s.name >> 8) as u8,
        ]);
        high[i / 4] |= (((x >> 8) as u8) | (u8::from(s.large) << 1)) << (i % 4 * 2);
    }
    low.extend(high);
    low
}
fn check_lines(sprites: &[Sprite]) -> Result<()> {
    for line in 0..224 {
        let (mut count, mut slivers) = (0, 0);
        for s in sprites {
            let size = if s.large { 32 } else { 16 };
            if s.y < 224 && (s.y..s.y + size).contains(&line) && s.x > -size && s.x < 256 {
                count += 1;
                slivers += (0..size / 8)
                    .filter(|t| (-7..256).contains(&(s.x + t * 8)))
                    .count();
            }
        }
        if count > 32 || slivers > 34 {
            return Err(format!("Title sprites overflow line {line}").into());
        }
    }
    Ok(())
}
impl Assets {
    pub fn prepare(content: &crate::content::Content, font: &DialogueFont) -> Result<Self> {
        let root = content.sub("title");
        let raw = root.read("title.json")?;
        let def: Definition = serde_json::from_slice(&raw)?;
        let value: serde_json::Value = serde_json::from_slice(&raw)?;
        if value["source_sha256"] != SOURCE || value["schema"] != 1 {
            return Err("Invalid title source".into());
        }
        let mut source = [0u16; 256];
        let mut images = Vec::new();
        for r in &def.resources {
            let pixels = root.read(format!("{}.indices", r.file))?;
            let rgb = root.read(format!("{}.rgb", r.file))?;
            if sha(&pixels) != r.pixels_sha256
                || sha(&rgb) != r.palette_sha256
                || pixels.len() != r.width * r.height
                || r.palette_base < 16
                || r.palette_base + r.colors > 256
            {
                return Err("Invalid title resource".into());
            }
            for (i, c) in rgb.as_chunks::<3>().0.iter().take(r.colors).enumerate() {
                source[r.palette_base + i] = rgb15(*c);
            }
            images.push((r, pixels));
        }
        let resource = |file: &str| -> Result<(&Resource, &Vec<u8>)> {
            images
                .iter()
                .find(|(r, _)| r.file == file)
                .map(|(r, p)| (*r, p))
                .ok_or_else(|| "Missing title resource".into())
        };
        let layer = |file: &str, top: i64| -> Result<Vec<Option<u8>>> {
            let (r, pixels) = resource(file)?;
            let mut canvas = vec![None; 640 * 480];
            for y in 0..r.height {
                for x in 0..r.width {
                    let local = usize::from(pixels[y * r.width + x]);
                    let py = top + y as i64;
                    if r.key == Some(local + r.palette_base) || !(0..480).contains(&py) {
                        continue;
                    }
                    canvas[py as usize * 640 + x] = Some(((local + r.palette_base) % 256) as u8);
                }
            }
            Ok((0..256 * 224)
                .map(|i| canvas[(i / 256 * 480 / 224) * 640 + i % 256 * 640 / 256])
                .collect())
        };
        let logo_rest = def
            .draws
            .iter()
            .find(|d| d.file == "LOGO.CNS")
            .ok_or("Missing logo draw")?
            .position;
        if logo_rest != [0, 64] {
            return Err("Unexpected logo rest".into());
        }
        let background: Vec<u16> = layer("TIT_BG.CNS", 0)?
            .into_iter()
            .map(|p| source[usize::from(p.unwrap_or(0))])
            .collect();
        let cells: Vec<BTreeMap<u16, usize>> = (0..32 * 28)
            .map(|cell| {
                let mut counts = BTreeMap::new();
                for i in 0..64 {
                    let at = (cell / 32 * 8 + i / 8) * 256 + cell % 32 * 8 + i % 8;
                    *counts.entry(background[at]).or_default() += 1;
                }
                counts
            })
            .collect();
        let (backdrop, palettes, assigned) = quantize(&cells);
        let mut changed = 0;
        let bg_pixels: Vec<u8> = background
            .iter()
            .enumerate()
            .map(|(i, &c)| {
                let palette = &palettes[assigned[(i / 256 / 8) * 32 + i % 256 / 8]];
                let (index, best) = std::iter::once(backdrop)
                    .chain(palette.iter().copied())
                    .enumerate()
                    .min_by_key(|(k, p)| (distance(c, *p), *k))
                    .unwrap_or((0, backdrop));
                changed += usize::from(best != c);
                index as u8
            })
            .collect();
        let bg_attributes: Vec<Attributes> = assigned
            .iter()
            .map(|&p| Attributes {
                palette: p as u8,
                priority: false,
            })
            .collect();
        let bg_image = TileImage::new(
            IndexedImage {
                width: 256,
                height: 224,
                pixels: &bg_pixels,
                attributes: &bg_attributes,
            },
            Depth::Four,
            0,
        )?;
        if bg_image.native_tiles().len() > usize::from(BG1_BASE + LOGO_NAME * 64 - BG2_TILES) {
            return Err("Title background exceeds BG2 residency".into());
        }
        let mut logo_colors: Vec<u16> = Vec::new();
        let logo_pixels: Vec<u8> = layer("LOGO.CNS", logo_rest[1])?
            .into_iter()
            .map(|p| {
                p.map_or(0, |p| {
                    let c = source[usize::from(p)];
                    let at = logo_colors.iter().position(|&l| l == c).unwrap_or_else(|| {
                        logo_colors.push(c);
                        logo_colors.len() - 1
                    });
                    (LOGO_COLORS + at) as u8
                })
            })
            .collect();
        if LOGO_COLORS + logo_colors.len() > OBJ_PALETTE {
            return Err("Title logo colours overlap the sprite palette".into());
        }
        let logo_image = TileImage::new(
            IndexedImage {
                width: 256,
                height: 224,
                pixels: &logo_pixels,
                attributes: &vec![
                    Attributes {
                        palette: 0,
                        priority: false,
                    };
                    32 * 28
                ],
            },
            Depth::Eight,
            LOGO_NAME,
        )?;
        if logo_image.native_tiles()[..64].iter().any(|&b| b != 0)
            || usize::from(BG1_BASE + LOGO_NAME * 64) + logo_image.native_tiles().len()
                > usize::from(OBJ_BASE + GLYPH_NAME * 32)
        {
            return Err("Title logo exceeds BG1 residency".into());
        }
        let scroll: Vec<u16> = (0..=MENU_TICK)
            .map(|tick| {
                let lift = 2 * LOGO_TICKS.saturating_sub(tick + 1);
                (crate::sfc::source_y_to_screen(lift as i64) as u16).wrapping_sub(1)
            })
            .collect();
        let (subtitle, subtitle_pixels) = resource("SUBTIT.CNS")?;
        let rects = &def.source_rectangles;
        let opaque = |x: usize, y: usize| {
            let local = usize::from(subtitle_pixels[y * subtitle.width + x]);
            subtitle.key != Some(local + subtitle.palette_base)
        };
        let ink = |rect: [usize; 4]| -> Result<u16> {
            let [l, t, r, b] = rect;
            let mut found = None;
            for y in t..b {
                for x in (l..r).filter(|&x| opaque(x, y)) {
                    let local = subtitle_pixels[y * subtitle.width + x];
                    if found.is_some_and(|f| f != local) {
                        return Err("Subtitle image uses several colours".into());
                    }
                    found = Some(local);
                }
            }
            let local = found.ok_or("Empty subtitle image")?;
            Ok(source[subtitle.palette_base + usize::from(local)])
        };
        let pink = [255u8, 153, 238];
        if ink(rects[16])? != rgb15(pink) {
            return Err("Unexpected copyright colour".into());
        }
        let mut target = [0u16; 256];
        target[0] = backdrop;
        for (p, palette) in palettes.iter().enumerate() {
            for (k, &c) in palette.iter().enumerate() {
                target[p * 16 + 1 + k] = c;
            }
        }
        for (k, &c) in logo_colors.iter().enumerate() {
            target[LOGO_COLORS + k] = c;
        }
        for (index, color) in [
            (GLYPH, ink(rects[0])?),
            (GLYPH_SHADOW, ink(rects[8])?),
            (WHITE, rgb15([255, 255, 255])),
            (GREY, rgb15([120, 120, 136])),
            (SHADOW, rgb15([16, 8, 24])),
            (COPYRIGHT, rgb15(pink)),
        ] {
            target[OBJ_PALETTE + usize::from(index)] = color;
        }
        const _: () = assert!(DIM_PALETTE + 16 <= OBJ_PALETTE);
        if LOGO_COLORS + logo_colors.len() > DIM_PALETTE {
            return Err("Logo colours reach the dim sprite palette".into());
        }
        target[DIM_PALETTE + usize::from(WHITE)] = target[OBJ_PALETTE + usize::from(GREY)];
        target[DIM_PALETTE + usize::from(SHADOW)] = target[OBJ_PALETTE + usize::from(SHADOW)];
        let cgram = |tick: usize| -> Vec<u8> {
            let level = ((tick + 1) / 4).min(31) as u16;
            let mut bytes = Vec::new();
            for (i, &c) in target.iter().enumerate() {
                let value = if i == OBJ_PALETTE + usize::from(COPYRIGHT) {
                    let steps = tick.saturating_sub(COPYRIGHT_TICK + 31) as i32;
                    let channel = |to: u8| (255 - 4 * steps).max(i32::from(to)) as u8;
                    if tick >= COPYRIGHT_TICK + 96 {
                        c
                    } else {
                        rgb15(pink.map(channel))
                    }
                } else {
                    let [r, g, b] = channels(c).map(|v| v.min(level));
                    r | g << 5 | b << 10
                };
                bytes.extend(value.to_le_bytes());
            }
            bytes
        };
        let glyph_source = |rect: [usize; 4], rest: [i64; 2]| GlyphSource {
            pixels: (rect[1]..rect[3])
                .flat_map(|y| (rect[0]..rect[2]).map(move |x| (x, y)))
                .map(|(x, y)| opaque(x, y))
                .collect(),
            width: rect[2] - rect[0],
            height: rect[3] - rect[1],
            rest: [rest[0] as f64, rest[1] as f64],
        };
        let draw_at = |rect: [usize; 4]| -> Result<[i64; 2]> {
            Ok(def
                .draws
                .iter()
                .find(|d| d.file == "SUBTIT.CNS" && d.rectangle == rect)
                .ok_or("Missing subtitle draw")?
                .position)
        };
        let mut glyph_frames: Vec<Vec<GlyphFrame>> = Vec::new();
        for (g, angle) in GLYPH_ANGLES.iter().enumerate() {
            let (fg_at, shadow_at) = (draw_at(rects[g])?, draw_at(rects[8 + g])?);
            if [shadow_at[0] - fg_at[0], shadow_at[1] - fg_at[1]] != [4, 4] {
                return Err("Unexpected subtitle shadow offset".into());
            }
            let (fg, shadow) = (
                glyph_source(rects[g], fg_at),
                glyph_source(rects[8 + g], shadow_at),
            );
            glyph_frames.push(
                (1..=GLYPH_TICKS)
                    .map(|m| glyph_frame(&fg, &shadow, *angle, m))
                    .collect(),
            );
        }
        let glyph_name = |g: usize| GLYPH_NAME + (g / 4 * 6 * 16 + g % 4 * 4) as u16;
        let band_uploads =
            |vram: &mut [[Option<Vec<u8>>; 6]], frames: &[Option<&GlyphFrame>]| -> Vec<Upload> {
                let tiles: Vec<Option<Vec<u8>>> = frames
                    .iter()
                    .map(|f| f.map(|f| sheet(&f.image, 32)))
                    .collect();
                let row_of = |g: usize, row: usize| {
                    tiles[g]
                        .as_ref()
                        .map(|t| t[row * 128..(row + 1) * 128].to_vec())
                };
                let mut uploads = Vec::new();
                for band in 0..2 {
                    for row in 0..6 {
                        let stale: Vec<usize> = (band * 4..band * 4 + 4)
                            .filter(|&g| {
                                let slot: &[Option<Vec<u8>>; 6] = &vram[g];
                                frames[g].is_some_and(|f| row < 4 || f.lower)
                                    && slot[row] != row_of(g, row)
                            })
                            .collect();
                        let (Some(&first), Some(&last)) = (stale.first(), stale.last()) else {
                            continue;
                        };
                        let mut data = Vec::new();
                        for (g, slot) in vram.iter_mut().enumerate().take(last + 1).skip(first) {
                            let content = row_of(g, row).unwrap_or_else(|| vec![0; 128]);
                            data.extend(&content);
                            slot[row] = Some(content);
                        }
                        let name = glyph_name(first) + row as u16 * 16;
                        uploads.push((OBJ_BASE + name * 32, 0, data));
                    }
                }
                uploads
            };
        let mut text_sheet = vec![0u8; 128 * 32];
        let drawn = [
            text(
                &mut text_sheet,
                128,
                font,
                "©COMPILE 1997",
                [0, 0],
                COPYRIGHT,
            )?,
            text(&mut text_sheet, 128, font, "처음부터", [0, 16], WHITE)?,
            text(&mut text_sheet, 128, font, "계속", [64, 16], WHITE)?,
        ];
        if drawn[0].0 > 128 || drawn[1].0 > 64 || drawn[2].0 > 64 {
            return Err("Title text exceeds its sprite sheet".into());
        }
        let words = [
            (drawn[0], 256u16, 456 * 224 / 480 - 2, COPYRIGHT_TICK, false),
            (drawn[1], 256 + 32, 356 * 224 / 480, MENU_TICK, false),
            (drawn[2], 256 + 40, 388 * 224 / 480, MENU_TICK, true),
        ];
        let glyph_tick = |g: usize, tick: usize| -> Option<usize> {
            let first = SUBTITLE_TICK + GLYPH_DELAYS[g];
            (tick >= first).then(|| (tick - first).min(GLYPH_TICKS - 1))
        };
        let sprites_at = |tick: usize| -> Vec<Sprite> {
            let mut list = Vec::new();
            for (g, frames) in glyph_frames.iter().enumerate() {
                let name = glyph_name(g);
                let (origin, lower) = glyph_tick(g, tick)
                    .map_or(([0, 224], false), |f| (frames[f].origin, frames[f].lower));
                let lower_y = if lower { origin[1] + 32 } else { 224 };
                list.extend([
                    Sprite {
                        x: origin[0],
                        y: origin[1],
                        name,
                        large: true,
                        dim: false,
                    },
                    Sprite {
                        x: origin[0],
                        y: lower_y,
                        name: name + 64,
                        large: false,
                        dim: false,
                    },
                    Sprite {
                        x: origin[0] + 16,
                        y: lower_y,
                        name: name + 66,
                        large: false,
                        dim: false,
                    },
                ]);
            }
            for &((width, lift), name, top, from, dim) in &words {
                for k in 0..width.div_ceil(16) {
                    list.push(Sprite {
                        dim,
                        x: (128 - width / 2 + 16 * k) as i32,
                        y: if tick >= from {
                            (top + lift) as i32
                        } else {
                            224
                        },
                        name: name + 2 * k as u16,
                        large: false,
                    });
                }
            }
            list
        };
        let oam_uploads = |sprites: &[Sprite]| -> Vec<Upload> {
            let table = oam(sprites);
            let split = sprites.len() * 4;
            vec![
                (0, 2, table[..split].to_vec()),
                (0x100, 2, table[split..].to_vec()),
            ]
        };
        let mut frames = Vec::new();
        let mut vram: Vec<[Option<Vec<u8>>; 6]> = vec![Default::default(); 8];
        let mut previous_cgram = vec![0u8; 512];
        let mut previous_oam = Vec::new();
        let mut largest = 0;
        for tick in 0..=MENU_TICK {
            let mut uploads = Vec::new();
            let colors = cgram(tick);
            if colors != previous_cgram {
                uploads.push((0, 1, colors.clone()));
                previous_cgram = colors;
            }
            let shown: Vec<Option<&GlyphFrame>> = glyph_frames
                .iter()
                .enumerate()
                .map(|(g, glyph)| glyph_tick(g, tick).map(|f| &glyph[f]))
                .collect();
            uploads.extend(band_uploads(&mut vram, &shown));
            let sprites = sprites_at(tick);
            check_lines(&sprites)?;
            let table = oam(&sprites);
            if table != previous_oam {
                uploads.extend(oam_uploads(&sprites));
                previous_oam = table;
            }
            let bytes: usize = uploads.iter().map(|u| u.2.len() + DESCRIPTOR_COST).sum();
            if bytes > STREAM_BUDGET {
                return Err(format!("Title tick {tick} exceeds a vertical blank").into());
            }
            largest = largest.max(bytes);
            frames.push(uploads);
        }
        let mut last = vec![(0, 1, cgram(MENU_TICK))];
        let rest: Vec<Option<&GlyphFrame>> = glyph_frames
            .iter()
            .map(|glyph| Some(&glyph[GLYPH_TICKS - 1]))
            .collect();
        last.extend(band_uploads(&mut vec![Default::default(); 8], &rest));
        last.extend(oam_uploads(&sprites_at(MENU_TICK)));
        if glyph_frames
            .iter()
            .any(|frames| frames[GLYPH_TICKS - 1].lower)
        {
            return Err("A resting subtitle glyph uses its cell's lower band".into());
        }
        let row = |source_y: usize| source_y * 224 / 480;
        let word = |(width, lift): (usize, usize),
                    x: usize,
                    top: usize,
                    names: &dyn Fn(usize) -> u16,
                    dim| {
            (0..width.div_ceil(16))
                .map(|k| Sprite {
                    x: (x + 16 * k) as i32,
                    y: (top + lift) as i32,
                    name: names(k),
                    large: false,
                    dim,
                })
                .collect::<Vec<_>>()
        };
        let resting = sprites_at(MENU_TICK);
        let base = resting[..3 * glyph_frames.len() + drawn[0].0.div_ceil(16)].to_vec();
        let main_menu = |selected: usize| {
            let mut list = base.clone();
            for (i, (drawn, name, top)) in [
                (drawn[1], 256 + 32, row(356)),
                (drawn[2], 256 + 40, row(388)),
            ]
            .into_iter()
            .enumerate()
            {
                list.extend(word(
                    drawn,
                    128 - drawn.0 / 2,
                    top,
                    &|k| name + 2 * k as u16,
                    selected != i,
                ));
            }
            list
        };
        if main_menu(0) != resting {
            return Err("The title's last tick differs from the menu's first screen".into());
        }
        let mut slot_sheet = vec![0u8; 256 * 16];
        let cell = |k: usize| -> u16 { GLYPH_NAME + (k / 8 * 96 + 64 + k % 8 * 2) as u16 };
        const LABEL: usize = 0;
        const DIGITS: usize = 5;
        const START_OVER: usize = 9;
        let label = text(
            &mut slot_sheet,
            256,
            font,
            "세이브데이타",
            [16 * LABEL, 0],
            WHITE,
        )?;
        let digits = (0..4)
            .map(|i| {
                text(
                    &mut slot_sheet,
                    256,
                    font,
                    &(i + 1).to_string(),
                    [16 * (DIGITS + i), 0],
                    WHITE,
                )
            })
            .collect::<Result<Vec<_>>>()?;
        let start_over = text(
            &mut slot_sheet,
            256,
            font,
            "역시 처음부터",
            [16 * START_OVER, 0],
            WHITE,
        )?;
        if label.0 > 16 * (DIGITS - LABEL)
            || digits.iter().any(|d| d.0 > 16)
            || start_over.0 > 16 * (16 - START_OVER)
        {
            return Err("Title slot text exceeds its cells".into());
        }
        const DIGIT_STEP: usize = 13;
        let slot_menu = |selected: usize| {
            let mut list = base.clone();
            let total = label.0 + 6 + 3 * DIGIT_STEP + digits[3].0;
            let left = 128 - total / 2;
            list.extend(word(label, left, row(388), &|k| cell(LABEL + k), false));
            for (i, &digit) in digits.iter().enumerate() {
                let x = left + label.0 + 6 + DIGIT_STEP * i;
                list.extend(word(
                    digit,
                    x,
                    row(388),
                    &|_| cell(DIGITS + i),
                    selected != i,
                ));
            }
            list.extend(word(
                start_over,
                128 - start_over.0 / 2,
                row(420),
                &|k| cell(START_OVER + k),
                selected != 4,
            ));
            list
        };
        let whole = |mut list: Vec<Sprite>| -> Result<Vec<Upload>> {
            check_lines(&list)?;
            while list.len() < 128 {
                list.push(Sprite {
                    x: 0,
                    y: 224,
                    name: 0,
                    large: false,
                    dim: false,
                });
            }
            Ok(vec![(0, 2, oam(&list))])
        };
        let menus = [main_menu(0), main_menu(1)]
            .into_iter()
            .chain((0..5).map(slot_menu))
            .map(whole)
            .collect::<Result<Vec<_>>>()?;
        let slot_text = (0..2)
            .map(|band| {
                let strip: Vec<u8> = (0..16)
                    .flat_map(|y| {
                        slot_sheet[y * 256 + 128 * band..y * 256 + 128 * band + 128].to_vec()
                    })
                    .collect();
                (
                    OBJ_BASE + (GLYPH_NAME + band as u16 * 96 + 64) * 32,
                    0,
                    sheet(&strip, 128),
                )
            })
            .collect::<Vec<Upload>>();
        let mut hidden_oam = Vec::new();
        for _ in 0..128 {
            hidden_oam.extend([0, HIDDEN_Y, 0, 0]);
        }
        hidden_oam.extend([0; 32]);
        let entry = vec![
            (BG2_TILES, 0, bg_image.native_tiles().to_vec()),
            (
                BG1_BASE + LOGO_NAME * 64,
                0,
                logo_image.native_tiles().to_vec(),
            ),
            (TEXT_SHEET, 0, sheet(&text_sheet, 128)),
            (BG2_MAP, 0, bg_image.bg_map(32, 32, 0)?),
            (BG1_MAP, 0, logo_image.bg_map(32, 64, LOGO_NAME)?),
            (0, 2, hidden_oam.clone()),
            (0, 1, vec![0; 512]),
        ];
        let report = serde_json::json!(
            { "definition_sha256" : sha(& raw), "background" : { "tiles" : bg_image
            .native_tiles().len() / 32, "palettes" : BG2_PALETTES, "changed_pixels" :
            changed }, "logo" : { "tiles" : logo_image.native_tiles().len() / 64,
            "colors" : logo_colors.len() }, "ticks" : { "logo" : LOGO_TICKS, "subtitle" :
            SUBTITLE_TICK, "copyright" : COPYRIGHT_TICK, "menu" : MENU_TICK },
            "largest_tick_charge" : largest, "stream_budget" : STREAM_BUDGET, "limits" :
            ["One source tick per SFC frame (the source runs about 62.5 ticks per second).",
            "Background colours are quantized to six 4bpp palettes.",
            "Glyph scaling is anchored at the glyph centre (inferred); the sine table is computed, not read from 0x45b3c8.",
            "Music (song 1) and the glyph sound effects play through the game's sound driver; the menu offers New Game and Continue.",
            "Index 255 in TIT_BG and LOGO is drawn with byte wrap-around of the palette base (inferred, not runtime verified)."]
            }
        );
        Ok(Self {
            entry,
            frames,
            last,
            hidden_oam,
            menus,
            slot_text,
            scroll,
            report,
        })
    }
    pub fn append(
        &self,
        sections: &mut Vec<Section>,
        space: &mut crate::sfc_rom::Space,
    ) -> Result<()> {
        use super::sfc_b1::{add, transfer};
        let mut stored: BTreeMap<Vec<u8>, u32> = BTreeMap::new();
        let mut job = |sections: &mut Vec<Section>,
                       space: &mut crate::sfc_rom::Space,
                       name: &str,
                       uploads: &[Upload]|
         -> Result<Vec<u8>> {
            let mut list = Vec::new();
            for (i, (address, kind, data)) in uploads.iter().enumerate() {
                for (c, chunk) in data.chunks(0x4000).enumerate() {
                    let source = match stored.get(chunk) {
                        Some(&source) => source,
                        None => {
                            let source =
                                add(sections, space, format!("{name}_{i}_{c}"), chunk.to_vec())?;
                            stored.insert(chunk.to_vec(), source);
                            source
                        }
                    };
                    list.extend(transfer(
                        source,
                        address + (c * 0x4000) as u16,
                        chunk.len(),
                        *kind,
                    ));
                }
            }
            let count = list.len() / 8;
            let mut record = if list.is_empty() {
                vec![0; 3]
            } else {
                add(sections, space, format!("{name}_list"), list)?.to_le_bytes()[..3].to_vec()
            };
            record.extend((count as u16).to_le_bytes());
            Ok(record)
        };
        let mut stream = Vec::new();
        let mut ends = Vec::new();
        let mut jobs = Vec::new();
        for (tick, uploads) in self.frames.iter().enumerate() {
            let mut list = Vec::new();
            for (address, kind, data) in uploads {
                for (c, chunk) in data.chunks(0x4000).enumerate() {
                    let source = 0x7f0000 | (stream.len() as u32 & 0xffff);
                    stream.extend(chunk);
                    list.extend(transfer(
                        source,
                        address + (c * 0x4000) as u16,
                        chunk.len(),
                        *kind,
                    ));
                }
            }
            ends.extend(&(stream.len() as u32).to_le_bytes()[..3]);
            let count = list.len() / 8;
            let mut record = if list.is_empty() {
                vec![0; 3]
            } else {
                add(sections, space, format!("title_{tick}_list"), list)?.to_le_bytes()[..3]
                    .to_vec()
            };
            record.extend((count as u16).to_le_bytes());
            jobs.extend(record);
        }
        let mut table = Vec::new();
        for (i, item) in crate::sfc_unpack::items(&stream)?.into_iter().enumerate() {
            let length = u16::try_from(item.len())?;
            let at = add(sections, space, format!("title_stream_{i}"), item)?;
            table.extend(&at.to_le_bytes()[..3]);
            table.extend(length.to_le_bytes());
        }
        let length = u16::try_from(table.len())?;
        let at = add(sections, space, "title_stream_items".into(), table)?;
        let streams = sections
            .iter_mut()
            .find(|s| s.name == crate::sfc_unpack::STREAMS_SECTION)
            .ok_or("Missing unpack streams")?;
        let entry = usize::from(crate::sfc_unpack::TITLE_STREAM);
        streams.bytes[entry..entry + 3].copy_from_slice(&at.to_le_bytes()[..3]);
        streams.bytes[entry + 3..entry + 5].copy_from_slice(&length.to_le_bytes());
        add(sections, space, "title_ends".into(), ends)?;
        let exit = vec![(0, 2, self.hidden_oam.clone())];
        for (name, uploads) in [
            ("title_entry", &self.entry),
            ("title_last", &self.last),
            ("title_exit", &exit),
        ] {
            let record = job(sections, space, name, uploads)?;
            add(sections, space, name.into(), record)?;
        }
        add(sections, space, "title_jobs".into(), jobs)?;
        let mut menus = Vec::new();
        for (i, uploads) in self.menus.iter().enumerate() {
            menus.extend(job(sections, space, &format!("title_menu_{i}"), uploads)?);
        }
        add(sections, space, "title_menus".into(), menus)?;
        let record = job(sections, space, "title_slot_text", &self.slot_text)?;
        add(sections, space, "title_slot_text".into(), record)?;
        add(
            sections,
            space,
            "title_scroll".into(),
            self.scroll.iter().flat_map(|s| s.to_le_bytes()).collect(),
        )?;
        Ok(())
    }
}
pub fn emit(p: &mut w65c816::Assembler, layout: &CartridgeLayout, pressed: u16) -> Result<()> {
    use crate::sfc::{emit, implied, write};
    use w65c816::{AddressingMode as A, CpuMode, Mnemonic as M, Operand as O};
    let abs = |p: &mut w65c816::Assembler, m, at| emit(p, m, A::Absolute, O::Word(at));
    let branch = |p: &mut w65c816::Assembler, m, label: &str| {
        p.emit_label_ref(m, A::Relative8, label);
    };
    let call = |p: &mut w65c816::Assembler, label: &str| {
        p.emit_label_ref(M::Jsr, A::Absolute, label);
    };
    let address = |name: &str| -> Result<u32> { layout.cpu_address(layout.section(name)?.offset) };
    let enqueue = |p: &mut w65c816::Assembler, job: u32| {
        emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
        emit(p, M::Lda, A::Immediate, O::Word(job as u16));
        abs(p, M::Sta, 0xe0);
        emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
        write(p, 0xe2, (job >> 16) as u8);
        emit(p, M::Ldx, A::Immediate, O::Word(0));
        call(p, "b1_enqueue");
    };
    let jobs = address("title_jobs")?;
    let ends = address("title_ends")?;
    let unpack = {
        let bytes = &layout.section("prologue_unpack_entries")?.bytes;
        let at = |k: usize| u32::from_le_bytes([bytes[k], bytes[k + 1], bytes[k + 2], 0]);
        [at(0), at(3), at(6)]
    };
    let unpack_active = u32::from(crate::sfc_unpack::PAGE) + u32::from(crate::sfc_unpack::ACTIVE);
    let scroll = address("title_scroll")?;
    let hdma = [
        (0x251u16, 0x13u8, 0x13u8),
        (0x253, 0x13, 0x13),
        (0x255, 0x02, 0x13),
    ];
    let registers = [
        (0x2101u16, 0x03u8, 0x60 | (OBJ_BASE >> 14) as u8),
        (0x2107, 0x7c, (BG1_MAP >> 11 << 2) as u8 | 2),
        (
            0x2108,
            (crate::sfc_prologue::TEXT_MAP_WORD >> 8) as u8,
            (BG2_MAP >> 11 << 2) as u8,
        ),
        (
            0x210b,
            0x50,
            (BG2_TILES >> 13 << 4) as u8 | (BG1_BASE >> 13) as u8,
        ),
        (0x212e, 0x11, 0),
    ];
    p.label("title_enter").assume_mode(CpuMode::NATIVE_M8_X16);
    write(p, 0x2100, 0x80);
    for at in 0x220..=0x22c {
        write(p, at, 0);
    }
    write(p, CHOICE, 0);
    write(p, CONTINUING, 0);
    for at in 0..super::sfc_b1::QUEUE_BYTES {
        write(p, super::sfc_b1::QUEUE + at, 0);
    }
    for (at, _, title) in registers {
        write(p, at, title);
    }
    for (at, value) in [
        (0x4310, 0),
        (0x4311, 0x2c),
        (0x4312, 0x50),
        (0x4313, 0x02),
        (0x4314, 0),
        (0x2125, 3),
        (0x2126, 7),
        (0x2127, 247),
        (0x212b, 0),
    ] {
        write(p, at, value);
    }
    for (at, _, title) in hdma {
        write(p, at, title);
    }
    write(p, crate::sfc_b1_menu::SETTINGS, 0);
    write(p, crate::sfc_sound::WANTED, 0xff);
    emit(
        p,
        M::Lda,
        A::Immediate,
        O::Byte(crate::sfc_sound::song("title")),
    );
    call(p, "sound_music");
    enqueue(p, address("title_entry")?);
    call(p, "b1_stream_all");
    emit(
        p,
        M::Ldx,
        A::Immediate,
        O::Word(crate::sfc_unpack::TITLE_STREAM),
    );
    emit(p, M::Jsl, A::AbsoluteLong, O::Long(unpack[0]));
    for i in 0..2 {
        emit(p, M::Lda, A::AbsoluteLong, O::Long(scroll + i));
        abs(p, M::Sta, SCROLL + i as u16);
    }
    call(p, "title_scroll_apply");
    for at in [FRAME, FRAME + 1] {
        write(p, at, 0);
    }
    write(p, MODE, 1);
    write(p, BRIGHTNESS, 15);
    implied(p, M::Rts);
    p.label("title_scroll_apply")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, SCROLL);
    abs(p, M::Sta, 0x210e);
    abs(p, M::Lda, SCROLL + 1);
    abs(p, M::Sta, 0x210e);
    implied(p, M::Rts);
    p.label("title_ppu_restore")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    for (at, prologue, _) in registers {
        write(p, at, prologue);
    }
    write(p, 0x210e, 0xff);
    write(p, 0x210e, 0xff);
    implied(p, M::Rts);
    p.label("title_tick").assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, BRIGHTNESS);
    abs(p, M::Sta, 0x2100);
    abs(p, M::Lda, MODE);
    emit(p, M::Cmp, A::Immediate, O::Byte(1));
    branch(p, M::Beq, "title_playing");
    p.emit_label_ref(M::Jmp, A::Absolute, "title_not_playing");
    p.label("title_playing");
    abs(p, M::Lda, pressed);
    emit(p, M::And, A::Immediate, O::Byte(0x80));
    branch(p, M::Bne, "title_skip_near");
    abs(p, M::Lda, pressed + 1);
    emit(p, M::And, A::Immediate, O::Byte(0x10));
    branch(p, M::Beq, "title_no_skip");
    p.label("title_skip_near");
    p.emit_label_ref(M::Jmp, A::Absolute, "title_skip");
    p.label("title_no_skip");
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    abs(p, M::Lda, FRAME);
    emit(p, M::Cmp, A::Immediate, O::Word((MENU_TICK + 1) as u16));
    branch(p, M::Bcc, "title_timeline_going");
    p.emit_label_ref(M::Jmp, A::Absolute, "title_timeline_done");
    p.label("title_timeline_going")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    abs(p, M::Sta, SCRATCH);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Clc);
    abs(p, M::Adc, SCRATCH);
    implied(p, M::Clc);
    emit(
        p,
        M::Adc,
        A::Immediate,
        O::Word((ends as u16).wrapping_sub(12)),
    );
    abs(p, M::Sta, 0xe0);
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    write(p, 0xe2, (ends >> 16) as u8);
    emit(p, M::Jsl, A::AbsoluteLong, O::Long(unpack[2]));
    branch(p, M::Bcs, "title_tick_ready");
    implied(p, M::Rts);
    p.label("title_tick_ready");
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    abs(p, M::Lda, FRAME);
    abs(p, M::Sta, SCRATCH);
    emit(p, M::Asl, A::Accumulator, O::None);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Clc);
    abs(p, M::Adc, SCRATCH);
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Word(jobs as u16));
    abs(p, M::Sta, 0xe0);
    abs(p, M::Lda, SCRATCH);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Tax);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(scroll));
    abs(p, M::Sta, SCROLL);
    if let Some(effect) = crate::sfc_sound::effect(layout, 28)? {
        abs(p, M::Lda, SCRATCH);
        for delay in GLYPH_DELAYS {
            emit(
                p,
                M::Cmp,
                A::Immediate,
                O::Word((SUBTITLE_TICK + delay) as u16),
            );
            branch(p, M::Beq, "title_letter_effect");
        }
        branch(p, M::Bra, "title_no_letter_effect");
        p.label("title_letter_effect");
        emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
        emit(p, M::Lda, A::Immediate, O::Byte(effect));
        call(p, "sound_effect");
        emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
        p.label("title_no_letter_effect");
    }
    abs(p, M::Inc, FRAME);
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    write(p, 0xe2, (jobs >> 16) as u8);
    emit(p, M::Ldx, A::Immediate, O::Word(0));
    call(p, "b1_enqueue");
    implied(p, M::Rts);
    p.label("title_timeline_done");
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    write(p, MODE, 3);
    emit(p, M::Lda, A::Immediate, O::Byte(0));
    emit(p, M::Sta, A::AbsoluteLong, O::Long(unpack_active));
    implied(p, M::Rts);
    p.label("title_skip");
    emit(p, M::Lda, A::Immediate, O::Byte(0));
    emit(p, M::Sta, A::AbsoluteLong, O::Long(unpack_active));
    enqueue(p, address("title_last")?);
    write(p, SCROLL, 0xff);
    write(p, SCROLL + 1, 0xff);
    write(p, MODE, 3);
    p.label("title_return");
    implied(p, M::Rts);
    p.label("title_not_playing");
    emit(p, M::Cmp, A::Immediate, O::Byte(4));
    branch(p, M::Bne, "title_menu");
    p.emit_label_ref(M::Jmp, A::Absolute, "title_fading_out");
    let queue = super::sfc_b1::QUEUE;
    let (oam_slot, text_slot) = (super::sfc_b1::MENU_OAM_SLOT, super::sfc_b1::MENU_STRIP_SLOT);
    p.label("title_menu");
    for slot in [0, text_slot, oam_slot] {
        abs(p, M::Lda, queue + slot + 3);
        abs(p, M::Ora, queue + slot + 4);
        branch(p, M::Bne, "title_menu_rts");
    }
    abs(p, M::Lda, pressed);
    emit(p, M::And, A::Immediate, O::Byte(0x80));
    branch(p, M::Bne, "title_confirm_near");
    abs(p, M::Lda, pressed + 1);
    emit(p, M::And, A::Immediate, O::Byte(0x10));
    branch(p, M::Beq, "title_navigate");
    p.label("title_confirm_near");
    p.emit_label_ref(M::Jmp, A::Absolute, "title_confirm");
    p.label("title_navigate");
    abs(p, M::Lda, MODE);
    emit(p, M::Cmp, A::Immediate, O::Byte(5));
    branch(p, M::Beq, "title_slots");
    abs(p, M::Lda, pressed + 1);
    emit(p, M::And, A::Immediate, O::Byte(0x0c));
    branch(p, M::Beq, "title_menu_rts");
    abs(p, M::Lda, CHOICE);
    emit(p, M::Eor, A::Immediate, O::Byte(1));
    abs(p, M::Sta, CHOICE);
    p.emit_label_ref(M::Jmp, A::Absolute, "title_show");
    p.label("title_slots");
    abs(p, M::Lda, pressed + 1);
    branch(p, M::Bmi, "title_back");
    abs(p, M::Lda, CHOICE);
    abs(p, M::Sta, SCRATCH);
    abs(p, M::Lda, pressed + 1);
    for label in ["title_right", "title_left", "title_down", "title_up"] {
        emit(p, M::Lsr, A::Accumulator, O::None);
        branch(p, M::Bcs, label);
    }
    p.label("title_menu_rts");
    implied(p, M::Rts);
    p.label("title_right");
    abs(p, M::Lda, SCRATCH);
    emit(p, M::Cmp, A::Immediate, O::Byte(3));
    branch(p, M::Bcs, "title_menu_rts");
    abs(p, M::Inc, CHOICE);
    branch(p, M::Bra, "title_show");
    p.label("title_left");
    abs(p, M::Lda, SCRATCH);
    branch(p, M::Beq, "title_menu_rts");
    emit(p, M::Cmp, A::Immediate, O::Byte(4));
    branch(p, M::Beq, "title_menu_rts");
    abs(p, M::Dec, CHOICE);
    branch(p, M::Bra, "title_show");
    p.label("title_down");
    write(p, CHOICE, 4);
    branch(p, M::Bra, "title_show");
    p.label("title_up");
    abs(p, M::Lda, SCRATCH);
    emit(p, M::Cmp, A::Immediate, O::Byte(4));
    branch(p, M::Bne, "title_menu_rts");
    abs(p, M::Stz, CHOICE);
    branch(p, M::Bra, "title_show");
    p.label("title_back");
    if let Some(effect) = crate::sfc_sound::effect(layout, 1)? {
        emit(p, M::Lda, A::Immediate, O::Byte(effect));
        call(p, "sound_effect");
    }
    write(p, MODE, 3);
    write(p, CHOICE, 1);
    branch(p, M::Bra, "title_show");
    let menus = address("title_menus")?;
    p.label("title_show");
    abs(p, M::Lda, CHOICE);
    abs(p, M::Sta, SCRATCH);
    abs(p, M::Stz, SCRATCH + 1);
    abs(p, M::Lda, MODE);
    emit(p, M::Cmp, A::Immediate, O::Byte(5));
    branch(p, M::Bne, "title_show_main");
    abs(p, M::Inc, SCRATCH);
    abs(p, M::Inc, SCRATCH);
    p.label("title_show_main");
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    abs(p, M::Lda, SCRATCH);
    emit(p, M::Asl, A::Accumulator, O::None);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Clc);
    abs(p, M::Adc, SCRATCH);
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Word(menus as u16));
    abs(p, M::Sta, 0xe0);
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    write(p, 0xe2, (menus >> 16) as u8);
    emit(p, M::Ldx, A::Immediate, O::Word(oam_slot));
    call(p, "b1_enqueue");
    implied(p, M::Rts);
    p.label("title_confirm");
    abs(p, M::Lda, MODE);
    emit(p, M::Cmp, A::Immediate, O::Byte(5));
    branch(p, M::Beq, "title_confirm_slot");
    abs(p, M::Lda, CHOICE);
    branch(p, M::Beq, "title_start");
    if let Some(effect) = crate::sfc_sound::effect(layout, 0)? {
        emit(p, M::Lda, A::Immediate, O::Byte(effect));
        call(p, "sound_effect");
    }
    let text = address("title_slot_text")?;
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    emit(p, M::Lda, A::Immediate, O::Word(text as u16));
    abs(p, M::Sta, 0xe0);
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    write(p, 0xe2, (text >> 16) as u8);
    emit(p, M::Ldx, A::Immediate, O::Word(text_slot));
    call(p, "b1_enqueue");
    write(p, MODE, 5);
    abs(p, M::Stz, CHOICE);
    p.emit_label_ref(M::Jmp, A::Absolute, "title_show");
    p.label("title_confirm_slot");
    abs(p, M::Lda, CHOICE);
    emit(p, M::Cmp, A::Immediate, O::Byte(4));
    branch(p, M::Beq, "title_start");
    call(p, "save_read");
    branch(p, M::Bcc, "title_load");
    if let Some(effect) = crate::sfc_sound::effect(layout, 24)? {
        emit(p, M::Lda, A::Immediate, O::Byte(effect));
        call(p, "sound_effect");
    }
    implied(p, M::Rts);
    p.label("title_load");
    if let Some(voice) = crate::sfc_sound::voice(layout, 17)? {
        emit(p, M::Lda, A::Immediate, O::Byte(voice));
        call(p, "sound_effect");
    }
    abs(p, M::Lda, CHOICE);
    emit(p, M::Inc, A::Accumulator, O::None);
    abs(p, M::Sta, crate::sfc_save::LOADING);
    write(p, CONTINUING, 1);
    p.label("title_start");
    if let Some(effect) = crate::sfc_sound::effect(layout, 0)? {
        emit(p, M::Lda, A::Immediate, O::Byte(effect));
        call(p, "sound_effect");
    }
    write(p, MODE, 4);
    write(p, FRAME, 0);
    implied(p, M::Rts);
    p.label("title_fading_out");
    abs(p, M::Inc, FRAME);
    abs(p, M::Lda, FRAME);
    emit(p, M::Cmp, A::Immediate, O::Byte(FADE_STEP));
    branch(p, M::Bcc, "title_fade_wait");
    write(p, FRAME, 0);
    abs(p, M::Lda, BRIGHTNESS);
    branch(p, M::Beq, "title_exit");
    abs(p, M::Dec, BRIGHTNESS);
    p.label("title_fade_wait");
    implied(p, M::Rts);
    p.label("title_exit");
    write(p, 0x2100, 0x80);
    write(p, 0x4200, 1);
    call(p, "sound_stop");
    for at in 0..super::sfc_b1::QUEUE_BYTES {
        write(p, super::sfc_b1::QUEUE + at, 0);
    }
    enqueue(p, address("title_exit")?);
    call(p, "b1_stream_all");
    call(p, "text_plane_reset");
    write(p, 0x4200, 0x81);
    call(p, "title_ppu_restore");
    for (at, prologue, _) in hdma {
        write(p, at, prologue);
    }
    write(p, MODE, 0);
    abs(p, M::Lda, CONTINUING);
    branch(p, M::Beq, "title_new_game");
    abs(p, M::Stz, CONTINUING);
    write(p, 0x22a, 1);
    implied(p, M::Rts);
    p.label("title_new_game");
    call(p, "restart");
    implied(p, M::Rts);
    Ok(())
}
