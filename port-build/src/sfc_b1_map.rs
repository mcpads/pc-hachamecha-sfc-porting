use crate::{Result, SOURCE, sha};
use serde::Deserialize;
use sfc_builder::cartridge::{CartridgeLayout, Section};
use std::collections::BTreeMap;
const VISITED: u16 = 0x1680 - 8;
pub(crate) const VISITS: (u16, u16) = (VISITED + 8, 8 * MAPS as u16);
pub const PATH: u16 = 0x998;
const ROUTE_FLAG: u16 = 0x120;
pub const ROUTE: [u8; 8] = [0x00, 0xfe, 0xc2, 0xc2, 0x82, 0x82, 0xfe, 0xc0];
const HIGHEST: u16 = 0x7c0;
pub(crate) const VIEWED: u16 = 0x7c1;
const CELL_BASE: u16 = 0x7c2;
const MAPS: u8 = 16;
const _: () = assert!(MAPS.is_power_of_two());
const PIN_FRAME: u16 = 0x7c8;
const PIN_TIMER: u16 = 0x7c9;
pub(crate) const SCRATCH: u16 = 0x7ca;
const VARIANT_LIST: u16 = 0x7c4;
const VARIANT_WORD: u16 = 0x7c6;
const VARIANT_INDEX: u16 = 0x7ce;
const VARIANT_COUNT: u16 = 0x7cf;
const SURFACE: [usize; 2] = [224, 240];
const PANEL: [usize; 2] = [104, 128];
const COLUMNS: usize = PANEL[0] / 8;
const ROWS: usize = PANEL[1] / 8;
const PIN_PALETTE: u16 = 128;
const HEART: u8 = 13;
const HEART_SHOWN: u16 = 0x95;
const HEART_GONE: u16 = 0x93;
pub const MAP_EVENT: u16 = 0x7f0;
const MARKERS: u16 = 0x7f2;
const MARKER_SLOTS: usize = 14;
const _: () = assert!(MARKERS + MARKER_SLOTS as u16 <= crate::sfc_field_events::GLOBALS);
#[derive(Deserialize)]
struct Sheet {
    file: String,
    width: usize,
    height: usize,
    pixels_sha256: String,
    palette_sha256: String,
}
#[derive(Deserialize)]
struct Recolor {
    index: usize,
    rgb: [u8; 3],
}
#[derive(Deserialize)]
struct WallBit {
    shift: u16,
    set: u8,
    clear: u8,
}
#[derive(Deserialize)]
struct MapSource {
    rectangles: Vec<[usize; 4]>,
    markers: Vec<[usize; 4]>,
    recolor_groups: Vec<Vec<Recolor>>,
    mapmini_base: usize,
    mapmini_key: u8,
    mapparts_key: u8,
    surface: [usize; 2],
    frame_at: [usize; 2],
    tabs_at: [usize; 2],
    tab_pitch: usize,
    digit_at: [usize; 2],
    cell_origin: [usize; 2],
    cell_size: usize,
    pin_origin: [usize; 2],
    pin_frames: Vec<usize>,
    pin_hold: u8,
    wall_bits: Vec<WallBit>,
}
#[derive(Deserialize)]
struct Definition {
    schema: u8,
    source_sha256: String,
    sheets: Vec<Sheet>,
    map: MapSource,
}
#[derive(Deserialize)]
struct Event {
    id: u8,
    unknown_byte: u8,
    x: usize,
    y: usize,
}
#[derive(Deserialize)]
struct WorldMap {
    id: u8,
    map_group: usize,
    width: usize,
    height: usize,
    cells: Vec<u16>,
    events: Vec<Event>,
}
#[derive(Deserialize)]
struct World {
    maps: Vec<WorldMap>,
    map_tabs: Vec<TabGroup>,
}
#[derive(Deserialize)]
struct TabGroup {
    group: usize,
    entries: Vec<TabEntry>,
}
#[derive(Deserialize)]
struct TabEntry {
    slot: usize,
    map_id: usize,
    arrow: bool,
}
type WallVariants = (usize, usize, Vec<(u8, Vec<u8>)>);
type WallRegion = (u8, Vec<Option<Rgb>>);
pub struct Assets {
    rows: Vec<Vec<u8>>,
    panels: Vec<Vec<usize>>,
    palettes: Vec<(u16, Vec<u8>)>,
    cells: Vec<u8>,
    variants: Vec<WallVariants>,
    wall_bits: Vec<(u16, u8, u8)>,
    pin_tiles: Vec<u8>,
    pin_palette: Vec<u8>,
    pin_hold: u8,
    pin_frames: u8,
    pin_origin: [u8; 2],
    heart_name: u8,
    hearts: Vec<u8>,
    live: Vec<[u8; 4]>,
    live_initial: Vec<u8>,
    cell_origin: [u8; 2],
    pub report: serde_json::Value,
}
mod scale;
use scale::{halve, halve_cell};

type Rgb = [u8; 3];
fn rgb15(c: Rgb) -> u16 {
    u16::from(c[0] >> 3) | u16::from(c[1] >> 3) << 5 | u16::from(c[2] >> 3) << 10
}

fn planar(pixels: &[u8], width: usize, tile: usize, bits: usize) -> Vec<u8> {
    let (tx, ty) = (tile % (width / 8), tile / (width / 8));
    let mut bytes = vec![0; bits * 8];
    for y in 0..8 {
        for x in 0..8 {
            let color = pixels[(ty * 8 + y) * width + tx * 8 + x];
            for plane in 0..bits {
                bytes[plane / 2 * 16 + y * 2 + plane % 2] |= ((color >> plane) & 1) << (7 - x);
            }
        }
    }
    bytes
}
const EXIT_SOURCE_ROWS: usize = 30;
impl Assets {
    pub fn load(
        content: &crate::content::Content,
        changes: &BTreeMap<u8, Vec<crate::floor_events::WallChange>>,
        marker_changes: &BTreeMap<u8, Vec<(u8, u8)>>,
    ) -> Result<Self> {
        let root = content.sub("menu");
        let def: Definition = root.json("menu.json")?;
        if def.schema != 1 || def.source_sha256 != SOURCE {
            return Err("Invalid map source".into());
        }
        let map = &def.map;
        let sheet = |name: &str| -> Result<(Vec<u8>, Vec<Rgb>, usize)> {
            let s = def
                .sheets
                .iter()
                .find(|s| s.file == name)
                .ok_or("Missing map sheet")?;
            let pixels = root.read(format!("{name}.indices"))?;
            let rgb = root.read(format!("{name}.rgb"))?;
            if sha(&pixels) != s.pixels_sha256
                || sha(&rgb) != s.palette_sha256
                || pixels.len() != s.width * s.height
            {
                return Err("Invalid map sheet".into());
            }
            Ok((pixels, rgb.as_chunks::<3>().0.to_vec(), s.width))
        };
        let (mini, mini_rgb, mini_width) = sheet("MAPMINI.CNS")?;
        let (parts, parts_rgb, parts_width) = sheet("MAPPARTS.CNS")?;
        let mut minis = Vec::new();
        for recolor in &map.recolor_groups {
            let mut rgb = mini_rgb.clone();
            for r in recolor {
                *rgb.get_mut(
                    r.index
                        .checked_sub(map.mapmini_base)
                        .ok_or("Recolour outside MAPMINI")?,
                )
                .ok_or("Recolour outside MAPMINI")? = r.rgb;
            }
            minis.push(rgb);
        }
        let world: World = content.json("world.json")?;
        let worlds = (1..=MAPS)
            .map(|id| {
                world
                    .maps
                    .iter()
                    .find(|m| m.id == id)
                    .filter(|m| m.width == 8 && m.height == 8 && m.cells.len() == 64)
                    .ok_or("Unexpected map grid")
            })
            .collect::<std::result::Result<Vec<_>, _>>()?;
        if map.surface != SURFACE {
            return Err("Unexpected map surface".into());
        }
        let mut live_records: Vec<(usize, usize, Vec<u8>)> = Vec::new();
        for (m, world) in worlds.iter().enumerate() {
            for &(id, value) in marker_changes.get(&world.id).into_iter().flatten() {
                let record = world
                    .events
                    .iter()
                    .position(|e| e.id == id)
                    .ok_or("A map marker change names no event")?;
                match live_records
                    .iter_mut()
                    .find(|(lm, r, _)| (*lm, *r) == (m, record))
                {
                    Some((_, _, values)) => values.push(value),
                    None => live_records.push((m, record, vec![value])),
                }
            }
        }
        let live = |m: usize, record: usize| {
            live_records
                .iter()
                .any(|&(lm, r, _)| (lm, r) == (m, record))
        };
        let [sw, sh] = SURFACE;
        let blit = |surface: &mut Vec<Option<Rgb>>,
                    (src, rgb, width): (&[u8], &[Rgb], usize),
                    rect: [usize; 4],
                    at: [usize; 2],
                    key: Option<u8>| {
            let [l, t, r, b] = rect;
            for y in t..b {
                for x in l..r {
                    let v = src[y * width + x];
                    let (px, py) = (at[0] + x - l, at[1] + y - t);
                    if key != Some(v) && px < sw && py < sh {
                        surface[py * sw + px] = Some(rgb[usize::from(v)]);
                    }
                }
            }
        };
        let mini_sheet = |group: usize| (mini.as_slice(), minis[group].as_slice(), mini_width);
        let parts_sheet = (parts.as_slice(), parts_rgb.as_slice(), parts_width);
        let mut bases = Vec::new();
        for (group, mini_rgb) in minis.iter().enumerate() {
            let mut base = vec![None; sw * sh];
            blit(
                &mut base,
                mini_sheet(group),
                map.rectangles[0],
                map.frame_at,
                Some(map.mapmini_key),
            );
            let [fl, ft, ..] = map.rectangles[0];
            for (x0, x1, y0, y1) in [(84, 188, 162, 192), (162, 188, 160, 162)] {
                for y in y0..y1 {
                    for x in x0..x1 {
                        let v = mini[(ft + (191 - y).min(EXIT_SOURCE_ROWS)) * mini_width + fl + x];
                        let (px, py) = (map.frame_at[0] + x, map.frame_at[1] + y);
                        if px < sw && py < sh {
                            base[py * sw + px] =
                                (v != map.mapmini_key).then(|| mini_rgb[usize::from(v)]);
                        }
                    }
                }
            }
            bases.push(base);
        }
        let tabs = |shown: usize, highest: usize| -> Result<Vec<Option<Rgb>>> {
            let group = worlds[shown - 1].map_group;
            let row = world
                .map_tabs
                .iter()
                .find(|t| t.group == group)
                .ok_or("Missing map tab group")?;
            let mut surface = bases[group].clone();
            for slot in 0..8 {
                let entry = row.entries.iter().find(|e| e.slot == slot);
                let at = [map.tabs_at[0] + map.tab_pitch * slot, map.tabs_at[1]];
                let background = if entry.is_some_and(|e| e.map_id == shown) {
                    11
                } else {
                    10
                };
                blit(
                    &mut surface,
                    mini_sheet(group),
                    map.rectangles[background],
                    at,
                    Some(map.mapmini_key),
                );
                let Some(entry) = entry.filter(|e| e.map_id <= highest) else {
                    continue;
                };
                let rect = match (entry.arrow, slot, group) {
                    (true, 0, _) => 1,
                    (true, ..) => 9,
                    (false, _, 0) => slot + 2,
                    _ => slot + 1,
                };
                let at = [map.digit_at[0] + map.tab_pitch * slot, map.digit_at[1]];
                blit(
                    &mut surface,
                    mini_sheet(group),
                    map.rectangles[rect],
                    at,
                    Some(map.mapmini_key),
                );
            }
            Ok(surface)
        };
        let mut panels = Vec::new();
        for highest in 1..=usize::from(MAPS) {
            for shown in 1..=usize::from(MAPS) {
                panels.push(tabs(shown, highest)?);
            }
        }
        let mut maps = Vec::new();
        let mut markers = 0;
        let wall_index = |value: u16| -> u8 {
            let mut index = 0u8;
            for bit in &map.wall_bits {
                let nibble = value >> bit.shift & 15;
                if nibble & 7 != 0 {
                    index |= if nibble & 8 != 0 { bit.set } else { bit.clear };
                }
            }
            index
        };
        let mut regions: Vec<(usize, usize, Vec<WallRegion>)> = Vec::new();
        for (m, world) in worlds.iter().enumerate() {
            let mut sides: BTreeMap<usize, [Vec<u16>; 4]> = BTreeMap::new();
            for change in changes.get(&world.id).into_iter().flatten() {
                let (cell, facing) = (usize::from(change.cell), usize::from(change.facing));
                let mut targets = vec![(cell, facing)];
                if change.both {
                    let (dx, dy) = [(0i32, -1i32), (1, 0), (0, 1), (-1, 0)][facing];
                    let neighbour = (cell as i32 + dy * 8 + dx).rem_euclid(64) as usize;
                    targets.push((neighbour, (facing + 2) % 4));
                }
                for (cell, facing) in targets {
                    let entry = sides.entry(cell).or_insert_with(|| {
                        std::array::from_fn(|f| vec![world.cells[cell] >> (4 * f) & 15])
                    });
                    for &v in &change.values {
                        let v = u16::from(v & 15);
                        if !entry[facing].contains(&v) {
                            entry[facing].push(v);
                        }
                    }
                }
            }
            for (&cell, options) in &sides {
                let mut words = vec![0u16];
                for (f, values) in options.iter().enumerate() {
                    words = words
                        .iter()
                        .flat_map(|w| values.iter().map(move |v| w | v << (4 * f)))
                        .collect();
                }
                let mut drawn: Vec<(u8, Vec<Option<Rgb>>)> = Vec::new();
                for word in words {
                    let index = wall_index(word);
                    if drawn.iter().any(|(i, _)| *i == index) {
                        continue;
                    }
                    let at = [
                        map.cell_origin[0] + map.cell_size * (cell % 8),
                        map.cell_origin[1] + map.cell_size * (cell / 8),
                    ];
                    let mut surface = bases[world.map_group].clone();
                    let (x, y) = (usize::from(index & 15) * 16, usize::from(index & 0xf0));
                    blit(
                        &mut surface,
                        mini_sheet(world.map_group),
                        [x, y, x + 16, y + 16],
                        at,
                        None,
                    );
                    for (_, event) in world.events.iter().enumerate().filter(|&(i, e)| {
                        e.unknown_byte != 0
                            && e.unknown_byte != HEART
                            && e.y * 8 + e.x == cell
                            && !live(m, i)
                    }) {
                        let rect = *map
                            .markers
                            .get(usize::from(event.unknown_byte) + 1)
                            .ok_or("Unknown map marker")?;
                        blit(&mut surface, parts_sheet, rect, at, Some(map.mapparts_key));
                    }
                    let region = (0..256)
                        .map(|i| surface[(at[1] + i / 16) * sw + at[0] + i % 16])
                        .collect();
                    drawn.push((index, region));
                }
                regions.push((m, cell, drawn));
            }
        }
        for (m, world) in worlds.iter().enumerate() {
            let mut visited = bases[world.map_group].clone();
            for (c, &value) in world.cells.iter().enumerate() {
                let at = [
                    map.cell_origin[0] + map.cell_size * (c % 8),
                    map.cell_origin[1] + map.cell_size * (c / 8),
                ];
                let index = wall_index(value);
                let (x, y) = (usize::from(index & 15) * 16, usize::from(index & 0xf0));
                blit(
                    &mut visited,
                    mini_sheet(world.map_group),
                    [x, y, x + 16, y + 16],
                    at,
                    None,
                );
            }
            for (_, event) in world
                .events
                .iter()
                .enumerate()
                .filter(|&(i, e)| e.unknown_byte != 0 && e.unknown_byte != HEART && !live(m, i))
            {
                let rect = *map
                    .markers
                    .get(usize::from(event.unknown_byte) + 1)
                    .ok_or("Unknown map marker")?;
                let at = [
                    map.cell_origin[0] + map.cell_size * event.x,
                    map.cell_origin[1] + map.cell_size * event.y,
                ];
                blit(&mut visited, parts_sheet, rect, at, Some(map.mapparts_key));
                markers += 1;
            }
            maps.push(visited);
        }
        let pad = |surface: Vec<Option<Rgb>>, visited: bool| -> Vec<Option<Rgb>> {
            let mut half = halve(&surface, sw, sh);
            if visited {
                for c in 0..64 {
                    let at = [
                        map.cell_origin[0] + (c % 8) * 16,
                        map.cell_origin[1] + (c / 8) * 16,
                    ];
                    for (i, pixel) in halve_cell(&surface, sw, at).into_iter().enumerate() {
                        half[(at[1] / 2 + i / 8) * (sw / 2) + at[0] / 2 + i % 8] = pixel;
                    }
                }
            }
            (0..PANEL[0] * PANEL[1])
                .map(|i| {
                    let (x, y) = (i % PANEL[0], i / PANEL[0]);
                    if y < sh / 2 && x < sw / 2 {
                        half[y * sw / 2 + x]
                    } else {
                        None
                    }
                })
                .collect()
        };
        let panels: Vec<_> = panels.into_iter().map(|s| pad(s, false)).collect();
        let maps: Vec<_> = maps.into_iter().map(|s| pad(s, true)).collect();
        let mut colors: BTreeMap<u16, u8> = BTreeMap::new();
        let slots: Vec<u8> = crate::sfc_b1::VIEW_COLORS
            .iter()
            .flat_map(|&[a, b]| (a..b).map(|i| i as u8))
            .collect();
        let mut index = |c: Option<Rgb>| -> Result<u8> {
            let Some(c) = c else { return Ok(0) };
            let next = colors.len();
            let at = *colors
                .entry(rgb15(c))
                .or_insert_with(|| slots.get(next).copied().unwrap_or(0));
            if at == 0 {
                return Err("Map colours exceed the view palette".into());
            }
            Ok(at)
        };
        let mut indexed = |surface: &Vec<Option<Rgb>>| -> Result<Vec<u8>> {
            surface.iter().map(|&c| index(c)).collect()
        };
        let panel_pixels = panels
            .iter()
            .map(&mut indexed)
            .collect::<Result<Vec<_>>>()?;
        let map_pixels = maps.iter().map(&mut indexed).collect::<Result<Vec<_>>>()?;
        let mut variants = Vec::new();
        for (m, cell, drawn) in regions {
            let mut tiles = Vec::new();
            for (index, region) in drawn {
                let pixels = indexed(&halve_cell(&region, 16, [0, 0]))?;
                tiles.push((index, planar(&pixels, 8, 0, 8)));
            }
            variants.push((m, cell, tiles));
        }
        let tile_of = |pixels: &[u8], col: usize, row: usize| {
            planar(pixels, PANEL[0], row * COLUMNS + col, 8)
        };
        let mut rows: Vec<Vec<u8>> = Vec::new();
        let panels = panel_pixels
            .iter()
            .map(|pixels| {
                (0..ROWS)
                    .map(|r| {
                        let row: Vec<u8> =
                            (0..COLUMNS).flat_map(|c| tile_of(pixels, c, r)).collect();
                        rows.iter().position(|x| *x == row).unwrap_or_else(|| {
                            rows.push(row);
                            rows.len() - 1
                        })
                    })
                    .collect()
            })
            .collect();
        let (cell_col, cell_row) = (map.cell_origin[0] / 2 / 8, map.cell_origin[1] / 2 / 8);
        if !map.cell_origin[0].is_multiple_of(16)
            || !map.cell_origin[1].is_multiple_of(16)
            || map.cell_size != 16
        {
            return Err("Map cells are not tile aligned at half size".into());
        }
        let cells = map_pixels
            .iter()
            .flat_map(|pixels| {
                (0..64).flat_map(move |c| tile_of(pixels, cell_col + c % 8, cell_row + c / 8))
            })
            .collect();
        let mut cgram = [0u16; 256];
        for (&c, &at) in &colors {
            cgram[usize::from(at)] = c;
        }
        let palettes = crate::sfc_b1::VIEW_COLORS
            .iter()
            .map(|&[a, b]| {
                (
                    a as u16,
                    cgram[a..b].iter().flat_map(|c| c.to_le_bytes()).collect(),
                )
            })
            .collect();
        let mut pin_colors: Vec<u16> = Vec::new();
        let mut pin_tiles = Vec::new();
        for &frame in &map.pin_frames {
            let mut pin = vec![None; 16 * 16];
            let (fx, fy) = (frame % 11 * 16, frame / 11 * 16);
            for y in 0..16 {
                for x in 0..16 {
                    let v = parts[(fy + y) * parts_width + fx + x];
                    if v != map.mapparts_key {
                        pin[y * 16 + x] = Some(parts_rgb[usize::from(v)]);
                    }
                }
            }
            let half = halve(&pin, 16, 16);
            let pixels = half
                .iter()
                .map(|c| match c {
                    None => Ok(0),
                    Some(c) => {
                        let c = rgb15(*c);
                        let at = pin_colors.iter().position(|&p| p == c).unwrap_or_else(|| {
                            pin_colors.push(c);
                            pin_colors.len() - 1
                        });
                        u8::try_from(at + 1).map_err(|_| "Pin colours".into())
                    }
                })
                .collect::<Result<Vec<u8>>>()?;
            pin_tiles.extend(planar(&pixels, 8, 0, 4));
        }
        let mut marker_sprite = |value: u8, pin_colors: &mut Vec<u16>| -> Result<u8> {
            let name = u8::try_from(pin_tiles.len() / 32)?;
            let [l, t, r, b] = *map
                .markers
                .get(usize::from(value) + 1)
                .ok_or("Missing map marker")?;
            let mut heart = vec![None; 16 * 16];
            for y in 0..(b - t).min(16) {
                for x in 0..(r - l).min(16) {
                    let v = parts[(t + y) * parts_width + l + x];
                    if v != map.mapparts_key {
                        heart[y * 16 + x] = Some(parts_rgb[usize::from(v)]);
                    }
                }
            }
            let pixels = halve(&heart, 16, 16)
                .iter()
                .map(|c| match c {
                    None => Ok(0),
                    Some(c) => {
                        let c = rgb15(*c);
                        let at = pin_colors.iter().position(|&p| p == c).unwrap_or_else(|| {
                            pin_colors.push(c);
                            pin_colors.len() - 1
                        });
                        u8::try_from(at + 1).map_err(|_| "Marker colours".into())
                    }
                })
                .collect::<Result<Vec<u8>>>()?;
            pin_tiles.extend(planar(&pixels, 8, 0, 4));
            Ok(name)
        };
        let heart_name = marker_sprite(HEART, &mut pin_colors)?;
        let mut live = Vec::new();
        let mut live_initial = Vec::new();
        let mut names: BTreeMap<u8, u8> = BTreeMap::new();
        for (m, record, values) in &live_records {
            let event = &worlds[*m].events[*record];
            let mut shown: Vec<u8> = values
                .iter()
                .chain([&event.unknown_byte])
                .copied()
                .filter(|&v| v != 0)
                .collect();
            shown.dedup();
            let name = match shown[..] {
                [] => 0,
                [v] if v != HEART => match names.get(&v) {
                    Some(&n) => n,
                    None => {
                        let n = marker_sprite(v, &mut pin_colors)?;
                        names.insert(v, n);
                        n
                    }
                },
                _ => return Err("A changing map marker takes two pictures".into()),
            };
            live.push([
                worlds[*m].id,
                event.id,
                u8::try_from(event.y * 8 + event.x)?,
                name,
            ]);
            live_initial.push(event.unknown_byte);
        }
        if live.len() > MARKER_SLOTS {
            return Err("Too many changing map markers".into());
        }
        let mut hearts = vec![0xffu8; usize::from(MAPS)];
        for (m, world) in worlds.iter().enumerate() {
            let cells: Vec<usize> = world
                .events
                .iter()
                .filter(|e| e.unknown_byte == HEART)
                .map(|e| e.y * 8 + e.x)
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect();
            match cells[..] {
                [] => {}
                [cell] => hearts[m] = u8::try_from(cell)?,
                _ => return Err("Two hearts on one map".into()),
            }
        }
        if pin_colors.len() > 15 {
            return Err("Map pin exceeds one sprite palette".into());
        }
        let mut pin_palette = vec![0u8; 32];
        for (i, c) in pin_colors.iter().enumerate() {
            pin_palette[(i + 1) * 2..(i + 2) * 2].copy_from_slice(&c.to_le_bytes());
        }
        let report = serde_json::json!(
            { "colors" : colors.len(), "markers" : markers, "pin_colors" : pin_colors
            .len(), "maps" : MAPS, "panel_rows" : rows.len(), "scale" :
            "half, darkest of 2x2", "limits" :
            ["Maps 1 to 16 with walls programs can change: the shown map's group gives the tab row and wall colours, its tab is selected, and digits and arrows show up to the highest map reached; left/right stand for clicking a tab.",
            "The source surface key and the field view behind it show as black."] }
        );
        let changeable: usize = variants.iter().map(|(_, _, t)| t.len()).sum();
        Ok(Self {
            rows,
            panels,
            palettes,
            cells,
            wall_bits: map
                .wall_bits
                .iter()
                .map(|b| (b.shift, b.set, b.clear))
                .collect(),
            variants,
            pin_tiles,
            pin_palette,
            pin_hold: map.pin_hold,
            pin_frames: u8::try_from(map.pin_frames.len())?,
            heart_name,
            hearts,
            live,
            live_initial,
            cell_origin: [
                u8::try_from(map.cell_origin[0] / 2)?,
                u8::try_from(map.cell_origin[1] / 2)?,
            ],
            pin_origin: [
                u8::try_from(map.pin_origin[0] / 2)?,
                u8::try_from(map.pin_origin[1] / 2)?,
            ],
            report: {
                let mut report = report;
                report["changeable_wall_tiles"] = changeable.into();
                report
            },
        })
    }
    pub fn rows(&self) -> &[Vec<u8>] {
        &self.rows
    }
    pub fn panels(&self) -> &[Vec<usize>] {
        &self.panels
    }
    pub fn append(
        &self,
        sections: &mut Vec<Section>,
        space: &mut crate::sfc_rom::Space,
        panel_blocks: &[Vec<u8>],
    ) -> Result<()> {
        use super::sfc_b1::{add, transfer};
        if panel_blocks.len() != self.panels.len() {
            return Err("Map panel views".into());
        }
        let mut common = Vec::new();
        for (i, (at, data)) in self.palettes.iter().enumerate() {
            let source = add(sections, space, format!("map_palette_{i}"), data.clone())?;
            common.extend(transfer(source, *at, data.len(), 1));
        }
        let pin = add(
            sections,
            space,
            "map_pin_tiles".into(),
            self.pin_tiles.clone(),
        )?;
        common.extend(transfer(
            pin,
            crate::sfc_b1_menu::ICON_VRAM,
            self.pin_tiles.len(),
            0,
        ));
        let pin_palette = add(
            sections,
            space,
            "map_pin_palette".into(),
            self.pin_palette.clone(),
        )?;
        common.extend(transfer(pin_palette + 2, PIN_PALETTE + 1, 30, 1));
        let mut records = Vec::new();
        for (i, blocks) in panel_blocks.iter().enumerate() {
            let mut list = blocks.clone();
            list.extend(&common);
            let count = list.len() / 8;
            let list =
                crate::sfc_b1::add_shared(sections, space, format!("map_panel_list_{i}"), list)?;
            records.extend(&list.to_le_bytes()[..3]);
            records.extend((count as u16).to_le_bytes());
        }
        add(sections, space, "map_base_jobs".into(), records)?;
        let mut tiles: Vec<&[u8]> = Vec::new();
        let mut at = Vec::new();
        for tile in self.cells.chunks(64) {
            let i = match tiles.iter().position(|t| *t == tile) {
                Some(i) => i,
                None => {
                    tiles.push(tile);
                    tiles.len() - 1
                }
            };
            at.push(i * 64);
        }
        let offset = space.reserve_table(tiles.len() * 64)?;
        sections.push(Section {
            name: "map_cells".into(),
            offset,
            bytes: tiles.concat(),
        });
        let base = crate::sfc_rom::cpu(offset)?;
        let index: Vec<u8> = at
            .iter()
            .flat_map(|&a| u16::try_from((base & 0xffff) as usize + a).map(u16::to_le_bytes))
            .flatten()
            .collect();
        if index.len() != self.cells.len() / 32 {
            return Err("Map cell tile address past its bank".into());
        }
        add(sections, space, "map_cell_tiles".into(), index)?;
        let mut slots = Vec::new();
        for c in 0..64usize {
            let tile = crate::sfc_b1::DUNGEON_TILE + (4 + c / 8) * COLUMNS + 3 + c % 8;
            slots.extend(((tile * 32) as u16).to_le_bytes());
        }
        add(sections, space, "map_cell_vram".into(), slots)?;
        let mut tiles = Vec::new();
        let mut placed = Vec::new();
        for (_, _, variants) in &self.variants {
            for (_, tile) in variants {
                placed.push(tiles.len());
                tiles.extend(tile);
            }
        }
        let tile_base = if tiles.is_empty() {
            0
        } else {
            add(sections, space, "map_wall_tiles".into(), tiles)?
        };
        let mut index = vec![0xffu8; usize::from(MAPS) * 64 * 2];
        let mut lists = vec![0u8];
        let mut next = 0;
        for (m, cell, variants) in &self.variants {
            let at = (m * 64 + cell) * 2;
            index[at..at + 2].copy_from_slice(&u16::try_from(lists.len())?.to_le_bytes());
            lists.push(u8::try_from(variants.len())?);
            for (wall, _) in variants {
                lists.push(*wall);
                lists.extend(&(tile_base + placed[next] as u32).to_le_bytes()[..3]);
                next += 1;
            }
        }
        add(sections, space, "map_wall_index".into(), index)?;
        add(sections, space, "map_wall_lists".into(), lists)?;
        let bits = self
            .wall_bits
            .iter()
            .flat_map(|&(shift, set, clear)| [shift as u8, set, clear])
            .collect();
        add(sections, space, "map_wall_bits".into(), bits)?;
        add(
            sections,
            space,
            "map_pin".into(),
            vec![
                self.pin_hold,
                self.pin_origin[0],
                self.pin_origin[1],
                self.heart_name,
                self.cell_origin[0],
                self.cell_origin[1],
                self.pin_frames,
            ],
        )?;
        add(sections, space, "map_hearts".into(), self.hearts.clone())?;
        add(
            sections,
            space,
            "map_live_markers".into(),
            self.live.iter().flatten().copied().collect(),
        )?;
        add(
            sections,
            space,
            "map_live_initial".into(),
            self.live_initial.clone(),
        )?;
        Ok(())
    }
}
pub fn emit(p: &mut w65c816::Assembler, layout: &CartridgeLayout, pressed: u16) -> Result<()> {
    use crate::sfc::{emit, implied, write};
    use crate::sfc_b1_menu::{EMPTY_PAGE, ICON_NAME, OAM_BUFFER, STRIP_JOB, STRIP_LIST};
    use w65c816::{AddressingMode as A, CpuMode, Mnemonic as M, Operand as O};
    let abs = |p: &mut w65c816::Assembler, m, at: u16| emit(p, m, A::Absolute, O::Word(at));
    let long_x = |p: &mut w65c816::Assembler, m, at: u32| emit(p, m, A::AbsoluteLongX, O::Long(at));
    let branch = |p: &mut w65c816::Assembler, m, label: &str| {
        p.emit_label_ref(m, A::Relative8, label);
    };
    let call = |p: &mut w65c816::Assembler, label: &str| {
        p.emit_label_ref(M::Jsr, A::Absolute, label);
    };
    let m16 = |p: &mut w65c816::Assembler| emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    let m8 = |p: &mut w65c816::Assembler| emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    let section = |name: &str| -> Result<u32> { layout.cpu_address(layout.section(name)?.offset) };
    let enqueue = |p: &mut w65c816::Assembler, record: u32, slot: u16| {
        emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
        emit(p, M::Lda, A::Immediate, O::Word(record as u16));
        emit(p, M::Sta, A::Absolute, O::Word(0xe0));
        emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
        write(p, 0xe2, (record >> 16) as u8);
        emit(p, M::Ldx, A::Immediate, O::Word(slot));
        p.emit_label_ref(M::Jsr, A::Absolute, "b1_enqueue");
    };
    let (base_jobs, cells, cell_vram, cell_tiles) = (
        section("map_base_jobs")?,
        section("map_cells")?,
        section("map_cell_vram")?,
        section("map_cell_tiles")?,
    );
    let (hidden, oam_job) = (section("menu_hidden_oam")?, section("menu_oam_job")?);
    let pin = &layout.section("map_pin")?.bytes;
    let (pin_hold, pin_x, pin_y) = (pin[0], usize::from(pin[1]), usize::from(pin[2]));
    let (heart_name, cell_x, cell_y) = (pin[3], usize::from(pin[4]), usize::from(pin[5]));
    let pin_frames = pin[6];
    let hearts = section("map_hearts")?;
    let live = layout.section("map_live_markers")?.bytes.clone();
    let (live_at, live_initial) = (
        section("map_live_markers")?,
        layout.section("map_live_initial")?.bytes.clone(),
    );
    let [dx, dy] = [crate::sfc_b1::DUNGEON[0], crate::sfc_b1::DUNGEON[1]];
    p.label("b1_map_markers_reset")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    for (i, &v) in live_initial.iter().enumerate() {
        write(p, MARKERS + i as u16, v);
    }
    implied(p, M::Rts);
    p.label("b1_map_marker_set")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    emit(p, M::Ldx, A::Immediate, O::Word(0));
    p.label("b1_map_marker_find");
    emit(p, M::Cpx, A::Immediate, O::Word(live.len() as u16));
    branch(p, M::Bne, "b1_map_marker_entry");
    implied(p, M::Rts);
    p.label("b1_map_marker_entry");
    long_x(p, M::Lda, live_at);
    abs(p, M::Cmp, 0x50c);
    branch(p, M::Bne, "b1_map_marker_next");
    long_x(p, M::Lda, live_at + 1);
    abs(p, M::Cmp, SCRATCH);
    branch(p, M::Bne, "b1_map_marker_next");
    m16(p);
    implied(p, M::Txa);
    emit(p, M::Lsr, A::Accumulator, O::None);
    emit(p, M::Lsr, A::Accumulator, O::None);
    implied(p, M::Tax);
    m8(p);
    abs(p, M::Lda, SCRATCH + 2);
    emit(p, M::Sta, A::AbsoluteX, O::Word(MARKERS));
    implied(p, M::Rts);
    p.label("b1_map_marker_next");
    for _ in 0..4 {
        implied(p, M::Inx);
    }
    branch(p, M::Bra, "b1_map_marker_find");
    p.label("b1_map_mark_cell")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, 0x50c);
    emit(p, M::Cmp, A::Immediate, O::Byte(MAPS + 1));
    branch(p, M::Bcc, "b1_map_mark_kept");
    implied(p, M::Rts);
    p.label("b1_map_mark_kept");
    m16(p);
    abs(p, M::Lda, SCRATCH + 1);
    emit(p, M::And, A::Immediate, O::Word(7));
    abs(p, M::Sta, SCRATCH + 2);
    abs(p, M::Lda, 0x50c);
    emit(p, M::And, A::Immediate, O::Word(255));
    for _ in 0..3 {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    implied(p, M::Clc);
    abs(p, M::Adc, SCRATCH + 2);
    implied(p, M::Tax);
    m8(p);
    abs(p, M::Lda, SCRATCH);
    emit(p, M::And, A::Immediate, O::Byte(7));
    abs(p, M::Sta, SCRATCH + 2);
    emit(p, M::Lda, A::Immediate, O::Byte(1));
    p.label("b1_map_mark_bit");
    abs(p, M::Dec, SCRATCH + 2);
    branch(p, M::Bmi, "b1_map_mark_bit_done");
    emit(p, M::Asl, A::Accumulator, O::None);
    branch(p, M::Bra, "b1_map_mark_bit");
    p.label("b1_map_mark_bit_done");
    emit(p, M::Ora, A::AbsoluteX, O::Word(VISITED));
    emit(p, M::Sta, A::AbsoluteX, O::Word(VISITED));
    implied(p, M::Rts);
    p.label("b1_map_cell_sprite")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    abs(p, M::Lda, SCRATCH);
    emit(p, M::And, A::Immediate, O::Word(0x38));
    emit(p, M::Lsr, A::Accumulator, O::None);
    emit(p, M::Lsr, A::Accumulator, O::None);
    emit(p, M::Lsr, A::Accumulator, O::None);
    abs(p, M::Sta, SCRATCH + 2);
    abs(p, M::Lda, VIEWED);
    emit(p, M::And, A::Immediate, O::Word(255));
    for _ in 0..3 {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    implied(p, M::Clc);
    abs(p, M::Adc, SCRATCH + 2);
    implied(p, M::Tax);
    m8(p);
    emit(p, M::Lda, A::AbsoluteX, O::Word(VISITED));
    abs(p, M::Sta, SCRATCH + 2);
    m16(p);
    abs(p, M::Lda, SCRATCH);
    emit(p, M::And, A::Immediate, O::Word(7));
    implied(p, M::Tax);
    m8(p);
    p.label("b1_map_cell_bit");
    branch(p, M::Beq, "b1_map_cell_bit_done");
    abs(p, M::Lsr, SCRATCH + 2);
    implied(p, M::Dex);
    branch(p, M::Bra, "b1_map_cell_bit");
    p.label("b1_map_cell_bit_done");
    abs(p, M::Lda, SCRATCH + 2);
    emit(p, M::And, A::Immediate, O::Byte(1));
    branch(p, M::Beq, "b1_map_cell_hidden");
    abs(p, M::Lda, SCRATCH);
    emit(p, M::And, A::Immediate, O::Byte(7));
    for _ in 0..3 {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Byte((dx + cell_x) as u8));
    emit(p, M::Sta, A::AbsoluteY, O::Word(OAM_BUFFER));
    abs(p, M::Lda, SCRATCH);
    emit(p, M::And, A::Immediate, O::Byte(0x38));
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Byte((dy + cell_y) as u8));
    emit(p, M::Sta, A::AbsoluteY, O::Word(OAM_BUFFER + 1));
    abs(p, M::Lda, SCRATCH + 1);
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Byte(ICON_NAME as u8));
    emit(p, M::Sta, A::AbsoluteY, O::Word(OAM_BUFFER + 2));
    emit(
        p,
        M::Lda,
        A::Immediate,
        O::Byte(0x30 | (ICON_NAME >> 8) as u8),
    );
    emit(p, M::Sta, A::AbsoluteY, O::Word(OAM_BUFFER + 3));
    p.label("b1_map_cell_hidden");
    implied(p, M::Rts);
    p.label("b1_map_visit").assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, 0x50c);
    emit(p, M::Cmp, A::Immediate, O::Byte(MAPS + 1));
    branch(p, M::Bcc, "b1_map_visit_floor");
    implied(p, M::Rts);
    p.label("b1_map_visit_floor");
    abs(p, M::Cmp, HIGHEST);
    branch(p, M::Bcc, "b1_map_visit_highest");
    abs(p, M::Sta, HIGHEST);
    p.label("b1_map_visit_highest");
    m16(p);
    abs(p, M::Lda, 0x500);
    emit(p, M::And, A::Immediate, O::Word(255));
    emit(p, M::Lsr, A::Accumulator, O::None);
    emit(p, M::Lsr, A::Accumulator, O::None);
    abs(p, M::Sta, SCRATCH);
    emit(p, M::And, A::Immediate, O::Word(7));
    implied(p, M::Tax);
    m8(p);
    emit(p, M::Lda, A::Immediate, O::Byte(1));
    p.label("b1_map_visit_bit");
    emit(p, M::Cpx, A::Immediate, O::Word(0));
    branch(p, M::Beq, "b1_map_visit_mask");
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Dex);
    branch(p, M::Bra, "b1_map_visit_bit");
    p.label("b1_map_visit_mask");
    abs(p, M::Sta, SCRATCH + 2);
    m16(p);
    abs(p, M::Lda, 0x50c);
    emit(p, M::And, A::Immediate, O::Word(255));
    for _ in 0..3 {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    abs(p, M::Sta, CELL_BASE);
    abs(p, M::Lda, SCRATCH);
    emit(p, M::Lsr, A::Accumulator, O::None);
    emit(p, M::Lsr, A::Accumulator, O::None);
    emit(p, M::Lsr, A::Accumulator, O::None);
    implied(p, M::Clc);
    abs(p, M::Adc, CELL_BASE);
    implied(p, M::Tax);
    m8(p);
    emit(p, M::Lda, A::AbsoluteX, O::Word(VISITED));
    abs(p, M::Ora, SCRATCH + 2);
    emit(p, M::Sta, A::AbsoluteX, O::Word(VISITED));
    implied(p, M::Rts);
    p.label("b1_map_path_step")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    abs(p, M::Lda, 0x500);
    emit(p, M::And, A::Immediate, O::Word(255));
    emit(p, M::Lsr, A::Accumulator, O::None);
    emit(p, M::Lsr, A::Accumulator, O::None);
    abs(p, M::Sta, SCRATCH);
    emit(p, M::And, A::Immediate, O::Word(7));
    implied(p, M::Tax);
    m8(p);
    emit(p, M::Lda, A::Immediate, O::Byte(1));
    p.label("b1_map_path_bit");
    emit(p, M::Cpx, A::Immediate, O::Word(0));
    branch(p, M::Beq, "b1_map_path_mask");
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Dex);
    branch(p, M::Bra, "b1_map_path_bit");
    p.label("b1_map_path_mask");
    abs(p, M::Sta, SCRATCH + 2);
    m16(p);
    abs(p, M::Lda, SCRATCH);
    for _ in 0..3 {
        emit(p, M::Lsr, A::Accumulator, O::None);
    }
    implied(p, M::Tax);
    m8(p);
    emit(p, M::Lda, A::AbsoluteX, O::Word(PATH));
    abs(p, M::And, SCRATCH + 2);
    branch(p, M::Bne, "b1_map_path_again");
    emit(p, M::Lda, A::AbsoluteX, O::Word(PATH));
    abs(p, M::Ora, SCRATCH + 2);
    emit(p, M::Sta, A::AbsoluteX, O::Word(PATH));
    implied(p, M::Rts);
    p.label("b1_map_path_again");
    let flag = crate::sfc_field_events::GLOBALS + ROUTE_FLAG / 8;
    abs(p, M::Lda, flag);
    emit(p, M::And, A::Immediate, O::Byte(!(1u8 << (ROUTE_FLAG % 8))));
    abs(p, M::Sta, flag);
    implied(p, M::Rts);
    p.label("b1_map_reset").assume_mode(CpuMode::NATIVE_M8_X16);
    for i in 8..8 * (u16::from(MAPS) + 1) {
        abs(p, M::Stz, VISITED + i);
    }
    for i in 0..8 {
        abs(p, M::Stz, PATH + i);
    }
    abs(p, M::Stz, HIGHEST);
    implied(p, M::Rts);
    p.label("b1_map_open").assume_mode(CpuMode::NATIVE_M8_X16);
    write(p, crate::sfc_b1_menu::MODE, 3);
    crate::sfc::write_word(p, 0x501, EMPTY_PAGE.into());
    call(p, "b1_page");
    abs(p, M::Lda, 0x50c);
    abs(p, M::Sta, VIEWED);
    write(p, PIN_FRAME, 0);
    write(p, PIN_TIMER, 0);
    p.label("b1_map_draw").assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    abs(p, M::Lda, HIGHEST);
    emit(p, M::And, A::Immediate, O::Word(255));
    emit(p, M::Dec, A::Accumulator, O::None);
    for _ in 0..MAPS.trailing_zeros() {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    abs(p, M::Sta, SCRATCH);
    abs(p, M::Lda, VIEWED);
    emit(p, M::And, A::Immediate, O::Word(255));
    emit(p, M::Dec, A::Accumulator, O::None);
    implied(p, M::Clc);
    abs(p, M::Adc, SCRATCH);
    abs(p, M::Sta, SCRATCH);
    emit(p, M::Asl, A::Accumulator, O::None);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Clc);
    abs(p, M::Adc, SCRATCH);
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Word(base_jobs as u16));
    abs(p, M::Sta, 0xe0);
    abs(p, M::Lda, VIEWED);
    emit(p, M::And, A::Immediate, O::Word(255));
    emit(p, M::Dec, A::Accumulator, O::None);
    implied(p, M::Xba);
    emit(p, M::Lsr, A::Accumulator, O::None);
    abs(p, M::Sta, CELL_BASE);
    m8(p);
    write(p, 0xe2, (base_jobs >> 16) as u8);
    emit(p, M::Ldx, A::Immediate, O::Word(0));
    call(p, "b1_enqueue");
    emit(p, M::Ldx, A::Immediate, O::Word(0));
    emit(p, M::Ldy, A::Immediate, O::Word(0));
    p.label("b1_map_cell");
    implied(p, M::Txa);
    emit(p, M::And, A::Immediate, O::Byte(7));
    branch(p, M::Bne, "b1_map_same_row");
    implied(p, M::Phx);
    m16(p);
    implied(p, M::Txa);
    emit(p, M::Lsr, A::Accumulator, O::None);
    emit(p, M::Lsr, A::Accumulator, O::None);
    emit(p, M::Lsr, A::Accumulator, O::None);
    abs(p, M::Sta, SCRATCH);
    abs(p, M::Lda, VIEWED);
    emit(p, M::And, A::Immediate, O::Word(255));
    for _ in 0..3 {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    implied(p, M::Clc);
    abs(p, M::Adc, SCRATCH);
    implied(p, M::Tax);
    m8(p);
    emit(p, M::Lda, A::AbsoluteX, O::Word(VISITED));
    abs(p, M::Sta, SCRATCH);
    implied(p, M::Plx);
    p.label("b1_map_same_row");
    abs(p, M::Lsr, SCRATCH);
    branch(p, M::Bcc, "b1_map_next_cell");
    implied(p, M::Phx);
    m16(p);
    implied(p, M::Txa);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Tax);
    long_x(p, M::Lda, cell_vram);
    emit(p, M::Sta, A::AbsoluteY, O::Word(STRIP_LIST + 1));
    emit(p, M::Lda, A::Immediate, O::Word(64));
    emit(p, M::Sta, A::AbsoluteY, O::Word(STRIP_LIST + 3));
    implied(p, M::Plx);
    implied(p, M::Phx);
    implied(p, M::Txa);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Clc);
    abs(p, M::Adc, CELL_BASE);
    implied(p, M::Tax);
    long_x(p, M::Lda, cell_tiles);
    emit(p, M::Sta, A::AbsoluteY, O::Word(STRIP_LIST + 5));
    m8(p);
    emit(p, M::Lda, A::Immediate, O::Byte((cells >> 16) as u8));
    emit(p, M::Sta, A::AbsoluteY, O::Word(STRIP_LIST + 7));
    implied(p, M::Plx);
    implied(p, M::Phx);
    call(p, "b1_map_wall");
    emit(p, M::Lda, A::Immediate, O::Byte(0));
    emit(p, M::Sta, A::AbsoluteY, O::Word(STRIP_LIST));
    implied(p, M::Plx);
    m16(p);
    implied(p, M::Tya);
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Word(8));
    implied(p, M::Tay);
    m8(p);
    p.label("b1_map_next_cell");
    implied(p, M::Inx);
    emit(p, M::Cpx, A::Immediate, O::Word(64));
    branch(p, M::Bne, "b1_map_cell");
    m16(p);
    emit(p, M::Lda, A::Immediate, O::Word(STRIP_LIST));
    abs(p, M::Sta, STRIP_JOB);
    implied(p, M::Tya);
    emit(p, M::Lsr, A::Accumulator, O::None);
    emit(p, M::Lsr, A::Accumulator, O::None);
    emit(p, M::Lsr, A::Accumulator, O::None);
    abs(p, M::Sta, STRIP_JOB + 3);
    m8(p);
    write(p, STRIP_JOB + 2, 0);
    enqueue(p, u32::from(STRIP_JOB), crate::sfc_b1::FADE_SLOT);
    p.label("b1_map_pin").assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    emit(p, M::Ldx, A::Immediate, O::Word(0));
    p.label("b1_map_clear_oam");
    long_x(p, M::Lda, hidden);
    emit(p, M::Sta, A::AbsoluteX, O::Word(OAM_BUFFER));
    implied(p, M::Inx);
    implied(p, M::Inx);
    emit(p, M::Cpx, A::Immediate, O::Word(544));
    branch(p, M::Bne, "b1_map_clear_oam");
    m8(p);
    {
        let flag = |f: u16| (crate::sfc_field_events::GLOBALS + f / 8, 1u8 << (f % 8));
        let (shown, shown_bit) = flag(HEART_SHOWN);
        let (gone, gone_bit) = flag(HEART_GONE);
        abs(p, M::Lda, shown);
        emit(p, M::And, A::Immediate, O::Byte(shown_bit));
        branch(p, M::Beq, "b1_map_no_heart");
        abs(p, M::Lda, gone);
        emit(p, M::And, A::Immediate, O::Byte(gone_bit));
        branch(p, M::Bne, "b1_map_no_heart");
        m16(p);
        abs(p, M::Lda, VIEWED);
        emit(p, M::And, A::Immediate, O::Word(255));
        emit(p, M::Dec, A::Accumulator, O::None);
        implied(p, M::Tax);
        m8(p);
        long_x(p, M::Lda, hearts);
        emit(p, M::Cmp, A::Immediate, O::Byte(0xff));
        branch(p, M::Beq, "b1_map_no_heart");
        abs(p, M::Sta, SCRATCH);
        write(p, SCRATCH + 1, heart_name);
        emit(p, M::Ldy, A::Immediate, O::Word(8));
        call(p, "b1_map_cell_sprite");
        p.label("b1_map_no_heart");
    }
    for (i, entry) in live.chunks(4).enumerate() {
        let skip = format!("b1_map_no_marker_{i}");
        abs(p, M::Lda, VIEWED);
        emit(p, M::Cmp, A::Immediate, O::Byte(entry[0]));
        branch(p, M::Bne, &skip);
        abs(p, M::Lda, MARKERS + i as u16);
        branch(p, M::Beq, &skip);
        write(p, SCRATCH, entry[2]);
        write(p, SCRATCH + 1, entry[3]);
        emit(p, M::Ldy, A::Immediate, O::Word(12 + 4 * i as u16));
        call(p, "b1_map_cell_sprite");
        p.label(skip);
    }
    abs(p, M::Lda, VIEWED);
    abs(p, M::Cmp, 0x50c);
    branch(p, M::Beq, "b1_map_pin_shown");
    p.label("b1_map_pin_hidden");
    enqueue(p, oam_job, crate::sfc_b1::MENU_OAM_SLOT);
    implied(p, M::Rts);
    p.label("b1_map_pin_shown");
    abs(p, M::Lda, 0x500);
    emit(p, M::Lsr, A::Accumulator, O::None);
    emit(p, M::Lsr, A::Accumulator, O::None);
    emit(p, M::And, A::Immediate, O::Byte(7));
    for _ in 0..3 {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Byte((dx + pin_x) as u8));
    abs(p, M::Sta, OAM_BUFFER);
    abs(p, M::Lda, 0x500);
    for _ in 0..2 {
        emit(p, M::Lsr, A::Accumulator, O::None);
    }
    emit(p, M::And, A::Immediate, O::Byte(0x38));
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Byte((dy + pin_y) as u8));
    abs(p, M::Sta, OAM_BUFFER + 1);
    abs(p, M::Lda, PIN_FRAME);
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Byte(ICON_NAME as u8));
    abs(p, M::Sta, OAM_BUFFER + 2);
    write(p, OAM_BUFFER + 3, 0x30 | (ICON_NAME >> 8) as u8);
    enqueue(p, oam_job, crate::sfc_b1::MENU_OAM_SLOT);
    implied(p, M::Rts);
    p.label("b1_map_tick").assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, pressed);
    abs(p, M::Ora, pressed + 1);
    emit(p, M::And, A::Immediate, O::Byte(0x80));
    branch(p, M::Beq, "b1_map_open_on");
    p.emit_label_ref(M::Jmp, A::Absolute, "b1_map_close");
    p.label("b1_map_open_on");
    abs(p, M::Lda, pressed + 1);
    emit(p, M::And, A::Immediate, O::Byte(15));
    emit(p, M::Cmp, A::Immediate, O::Byte(2));
    branch(p, M::Bne, "b1_map_not_left");
    abs(p, M::Lda, VIEWED);
    emit(p, M::Cmp, A::Immediate, O::Byte(2));
    branch(p, M::Bcc, "b1_map_turn");
    abs(p, M::Dec, VIEWED);
    p.emit_label_ref(M::Jmp, A::Absolute, "b1_map_draw");
    p.label("b1_map_not_left");
    emit(p, M::Cmp, A::Immediate, O::Byte(1));
    branch(p, M::Bne, "b1_map_turn");
    abs(p, M::Lda, VIEWED);
    abs(p, M::Cmp, HIGHEST);
    branch(p, M::Bcs, "b1_map_turn");
    abs(p, M::Inc, VIEWED);
    p.emit_label_ref(M::Jmp, A::Absolute, "b1_map_draw");
    p.label("b1_map_turn");
    abs(p, M::Lda, VIEWED);
    abs(p, M::Cmp, 0x50c);
    branch(p, M::Bne, "b1_map_still");
    abs(p, M::Inc, PIN_TIMER);
    abs(p, M::Lda, PIN_TIMER);
    emit(p, M::Cmp, A::Immediate, O::Byte(pin_hold));
    branch(p, M::Bcc, "b1_map_still");
    write(p, PIN_TIMER, 0);
    abs(p, M::Lda, PIN_FRAME);
    emit(p, M::Inc, A::Accumulator, O::None);
    emit(p, M::Cmp, A::Immediate, O::Byte(pin_frames));
    branch(p, M::Bcc, "b1_map_frame_next");
    emit(p, M::Lda, A::Immediate, O::Byte(0));
    p.label("b1_map_frame_next");
    abs(p, M::Sta, PIN_FRAME);
    p.emit_label_ref(M::Jmp, A::Absolute, "b1_map_pin");
    p.label("b1_map_still");
    implied(p, M::Rts);
    p.label("b1_map_event_open")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    write(p, MAP_EVENT, 1);
    write(p, PIN_FRAME, 0);
    write(p, PIN_TIMER, 0);
    p.emit_label_ref(M::Jmp, A::Absolute, "b1_map_draw");
    p.label("b1_map_event_tick")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, MAP_EVENT);
    branch(p, M::Beq, "b1_map_event_still");
    p.emit_label_ref(M::Jmp, A::Absolute, "b1_map_turn");
    p.label("b1_map_event_still");
    implied(p, M::Rts);
    p.label("b1_map_event_close")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Stz, MAP_EVENT);
    for i in 3..5 {
        write(p, crate::sfc_b1::QUEUE + crate::sfc_b1::FADE_SLOT + i, 0);
    }
    let map_sprites = 3 + live.chunks(4).count() as u16;
    emit(p, M::Ldx, A::Immediate, O::Word(0));
    emit(p, M::Lda, A::Immediate, O::Byte(224));
    p.label("b1_map_event_hide");
    emit(p, M::Sta, A::AbsoluteX, O::Word(OAM_BUFFER + 1));
    for _ in 0..4 {
        implied(p, M::Inx);
    }
    emit(p, M::Cpx, A::Immediate, O::Word(4 * map_sprites));
    branch(p, M::Bne, "b1_map_event_hide");
    enqueue(p, oam_job, crate::sfc_b1::MENU_OAM_SLOT);
    implied(p, M::Rts);
    p.label("b1_map_close").assume_mode(CpuMode::NATIVE_M8_X16);
    if let Some(effect) = crate::sfc_sound::effect(layout, 1)? {
        emit(p, M::Lda, A::Immediate, O::Byte(effect));
        call(p, "sound_effect");
    }
    for i in 3..5 {
        write(p, crate::sfc_b1::QUEUE + crate::sfc_b1::FADE_SLOT + i, 0);
    }
    write(p, crate::sfc_b1_menu::MODE, 1);
    call(p, "b1_render");
    p.emit_label_ref(M::Jmp, A::Absolute, "b1_menu_return");
    let (walls, lists) = (section("map_wall_index")?, section("map_wall_lists")?);
    p.label("b1_map_wall").assume_mode(CpuMode::NATIVE_M8_X16);
    implied(p, M::Phx);
    implied(p, M::Phy);
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    abs(p, M::Stx, VARIANT_WORD);
    abs(p, M::Lda, VIEWED);
    emit(p, M::And, A::Immediate, O::Word(255));
    emit(p, M::Dec, A::Accumulator, O::None);
    for _ in 0..6 {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    implied(p, M::Clc);
    abs(p, M::Adc, VARIANT_WORD);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Tax);
    long_x(p, M::Lda, walls);
    emit(p, M::Cmp, A::Immediate, O::Word(0xffff));
    p.emit_label_ref(M::Bne, A::Relative8, "b1_map_wall_some");
    p.emit_label_ref(M::Jmp, A::Absolute, "b1_map_wall_none");
    p.label("b1_map_wall_some");
    abs(p, M::Sta, VARIANT_LIST);
    abs(p, M::Lda, VIEWED);
    emit(p, M::And, A::Immediate, O::Word(255));
    emit(p, M::Dec, A::Accumulator, O::None);
    for _ in 0..6 {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    implied(p, M::Clc);
    abs(p, M::Adc, VARIANT_WORD);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Tax);
    long_x(p, M::Lda, crate::sfc_floor_walls::CELLS);
    abs(p, M::Sta, VARIANT_WORD);
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    abs(p, M::Stz, VARIANT_INDEX);
    for (i, &(shift, set, clear)) in wall_bits(layout)?.iter().enumerate() {
        let (next, clear_label) = (
            format!("b1_map_wall_side_{i}"),
            format!("b1_map_wall_clear_{i}"),
        );
        emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
        abs(p, M::Lda, VARIANT_WORD);
        for _ in 0..shift {
            emit(p, M::Lsr, A::Accumulator, O::None);
        }
        emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
        emit(p, M::And, A::Immediate, O::Byte(15));
        abs(p, M::Sta, VARIANT_COUNT);
        emit(p, M::And, A::Immediate, O::Byte(7));
        p.emit_label_ref(M::Beq, A::Relative8, &next);
        abs(p, M::Lda, VARIANT_COUNT);
        emit(p, M::And, A::Immediate, O::Byte(8));
        p.emit_label_ref(M::Beq, A::Relative8, &clear_label);
        abs(p, M::Lda, VARIANT_INDEX);
        emit(p, M::Ora, A::Immediate, O::Byte(set));
        p.emit_label_ref(M::Bra, A::Relative8, format!("{next}_store"));
        p.label(&clear_label);
        abs(p, M::Lda, VARIANT_INDEX);
        emit(p, M::Ora, A::Immediate, O::Byte(clear));
        p.label(format!("{next}_store"));
        abs(p, M::Sta, VARIANT_INDEX);
        p.label(&next);
    }
    abs(p, M::Ldx, VARIANT_LIST);
    long_x(p, M::Lda, lists);
    abs(p, M::Sta, VARIANT_COUNT);
    implied(p, M::Inx);
    p.label("b1_map_wall_find");
    long_x(p, M::Lda, lists);
    abs(p, M::Cmp, VARIANT_INDEX);
    p.emit_label_ref(M::Beq, A::Relative8, "b1_map_wall_found");
    for _ in 0..4 {
        implied(p, M::Inx);
    }
    abs(p, M::Dec, VARIANT_COUNT);
    p.emit_label_ref(M::Bne, A::Relative8, "b1_map_wall_find");
    p.emit_label_ref(M::Bra, A::Relative8, "b1_map_wall_done");
    p.label("b1_map_wall_found");
    implied(p, M::Ply);
    implied(p, M::Phy);
    for i in 0..3u16 {
        long_x(p, M::Lda, lists + 1 + u32::from(i));
        emit(p, M::Sta, A::AbsoluteY, O::Word(STRIP_LIST + 5 + i));
    }
    p.emit_label_ref(M::Bra, A::Relative8, "b1_map_wall_done");
    p.label("b1_map_wall_none");
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    p.label("b1_map_wall_done");
    implied(p, M::Ply);
    implied(p, M::Plx);
    implied(p, M::Rts);
    Ok(())
}
fn wall_bits(layout: &CartridgeLayout) -> Result<Vec<(u16, u8, u8)>> {
    let bytes = &layout.section("map_wall_bits")?.bytes;
    Ok(bytes
        .chunks(3)
        .map(|c| (u16::from(c[0]), c[1], c[2]))
        .collect())
}
