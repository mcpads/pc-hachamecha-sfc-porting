use crate::{Result, SOURCE, sha};
use serde::Deserialize;
use sfc_builder::cartridge::{CartridgeLayout, Section};
use std::collections::BTreeMap;
pub const GUARD_TIME: u16 = 0x1d00;
const GUARD: u16 = 0x1d02;
const WAIT: u16 = 0x1d03;
const SECOND_WAIT: u16 = 0x1d04;
const STATIC_SENT: u16 = 0x1d05;
const STEP_SENT: u16 = 0x1d06;
const READY: u16 = 0x1d07;
const HEADS: u16 = 0x1d08;
const OFFSETS: u16 = 0x1d0a;
const RECORDS: u16 = 0x1d20;
const RING: usize = 5;
const RECORD: usize = 8;
const S_SLOT: u16 = RECORDS + (2 * RING * RECORD) as u16;
const S_K: u16 = S_SLOT + 2;
const S_AGE: u16 = S_K + 2;
const S_R16: u16 = S_AGE + 2;
const S_XO: u16 = S_R16 + 2;
const S_YO: u16 = S_XO + 2;
const S_T: u16 = S_YO + 2;
const S_AX: u16 = S_T + 2;
const S_AY: u16 = S_AX + 2;
const _: () = assert!(S_AY as usize + 2 <= 0x1da0);
const NAME_RUNS: [(u16, u16); 2] = [(272, 344), (355, 384)];
pub const GEM_BLOCK: u8 = 192;
pub const BODY_BLOCK: u8 = 208;
const FRONT: usize = 0;
const SURFACE: [i64; 2] = [224, 240];
const STEP_TICKS: usize = 4;
const STEPS: usize = 8;
const AGES: usize = 32;
const SPREAD_DIRECTIONS: usize = 128;
const CODE_SECTION: &str = "guardian_code";
const CODE_BYTES: usize = 0x1000;
#[derive(Deserialize)]
struct Frame {
    rectangle: [i64; 4],
    anchor: [i64; 2],
}
#[derive(Deserialize)]
struct Body {
    file: String,
    palette_base: u8,
    frames: BTreeMap<String, Frame>,
    base: [i64; 2],
    amplitude: i64,
    phase_step: u16,
    show: Vec<[usize; 2]>,
}
#[derive(Deserialize)]
struct Gem {
    file: String,
    palette_base: u8,
    key: u8,
    grid: [usize; 2],
    place: [i64; 2],
    depth: i64,
    show: Vec<[usize; 2]>,
    width: usize,
    height: usize,
    pixels_sha256: String,
    palette_sha256: String,
}
#[derive(Deserialize)]
struct Emitter {
    wait: u8,
    draw: u8,
    depths: [i64; 2],
}
#[derive(Deserialize)]
struct Side {
    phases: [u16; 2],
    steps: [i32; 2],
    base_moves: i64,
    repeats: usize,
}
#[derive(Deserialize)]
struct Arc {
    amplitudes: [i64; 2],
    draws: [u16; 2],
    side_draw: u16,
    sides: Vec<Side>,
}
#[derive(Deserialize)]
struct Spread {
    phase: u16,
    phase_draw: u16,
    amplitude: i64,
    step_base: i64,
    step_draw: u16,
    repeats: usize,
}
#[derive(Deserialize)]
struct Particle {
    base: [i64; 2],
    x_draw: u16,
    path_draw: u16,
    show: Vec<[usize; 2]>,
    arc: Arc,
    spread: Spread,
}
#[derive(Deserialize)]
struct Guardian {
    map_id: u8,
    entry_va: u32,
    body: Body,
    gem: Gem,
    emitter: Emitter,
    particle: Particle,
}
#[derive(Deserialize)]
struct Display {
    schema: u8,
    source_sha256: String,
    guardians: Vec<Guardian>,
}
#[derive(Deserialize)]
struct SheetRecord {
    file: String,
    width: usize,
    height: usize,
    pixels_sha256: String,
    palette_sha256: String,
}
struct Sprites {
    map_id: u8,
    entry_va: u32,
    statics: Vec<(u16, [u8; 32])>,
    varying: Vec<u16>,
    gem: Vec<u16>,
    step_tiles: Vec<(Vec<Tile>, Vec<Tile>)>,
    body_oam: Vec<Vec<[u8; 4]>>,
    gem_oam: Vec<[u8; 4]>,
    particle_names: Vec<u16>,
    body_rgb: Vec<u8>,
    gem_rgb: Vec<u8>,
}
pub struct Assets {
    guardians: Vec<Sprites>,
    pub ids: [u8; 4],
    bob: Vec<u8>,
    variants: usize,
    body_entries: usize,
    gem_entries: usize,
    xs: Vec<u8>,
    ys: Vec<u8>,
    screen_x: Vec<u8>,
    screen_y: Vec<u8>,
    arc_draws: [u8; 3],
    spread_step_draw: u8,
    x_draw: u8,
    path_draw: u8,
    emitter_wait: u8,
    emitter_draw: u8,
    particle_start: [i64; 2],
    pub report: serde_json::Value,
}
type Tile = [u8; 32];
type Pixels = BTreeMap<(i64, i64), u8>;
type Cells = (i64, i64, Vec<(i64, i64)>);
fn attr(name: u16, block: u8) -> u8 {
    ((name >> 8) as u8 & 1) | (((block - 128) / 16) << 1) | (3 << 4)
}
fn planar(pixels: &Pixels, x0: i64, y0: i64) -> Tile {
    let mut tile = [0u8; 32];
    for y in 0..8 {
        for x in 0..8 {
            let value = pixels.get(&(x0 + x, y0 + y)).copied().unwrap_or(0);
            for plane in 0..4 {
                tile[plane / 2 * 16 + y as usize * 2 + plane % 2] |=
                    ((value >> plane) & 1) << (7 - x);
            }
        }
    }
    tile
}
fn sample(
    sheet: &[u8],
    width: usize,
    rect: [i64; 4],
    place: [i64; 2],
    panel: [usize; 4],
) -> BTreeMap<(i64, i64), u8> {
    let [_, _, pw, ph] = panel.map(|v| v as i64);
    let [l, t, r, b] = rect;
    let mut out = BTreeMap::new();
    for dy in 0..ph {
        for dx in 0..pw {
            let (sx, sy) = (dx * SURFACE[0] / pw, dy * SURFACE[1] / ph);
            let (u, v) = (sx - place[0] + l, sy - place[1] + t);
            if (l..r).contains(&u) && (t..b).contains(&v) {
                let c = sheet[v as usize * width + u as usize];
                if c != 0 {
                    out.insert((dx, dy), c);
                }
            }
        }
    }
    out
}
impl Assets {
    pub fn load(content: &crate::content::Content, panel: [usize; 4]) -> Result<Self> {
        let dir = content.sub("guardian-display");
        let display: Display = dir.json("guardian-display.json")?;
        if display.schema != 1 || display.source_sha256 != SOURCE || display.guardians.len() != 4 {
            return Err("Invalid guardian display".into());
        }
        let objects: serde_json::Value = content.json("floor-objects/floor-objects.json")?;
        let origin: [i64; 2] = serde_json::from_value(objects["screen_origin"].clone())?;
        let records: Vec<SheetRecord> = serde_json::from_value(objects["resources"].clone())?;
        let image = content.read("battle-scripts.bin")?;
        let manifest: serde_json::Value = content.json("battle-scripts.json")?;
        if manifest["image_sha256"] != sha(&image) {
            return Err("Battle script image differs from its manifest".into());
        }
        let base = (0x45b3c8 - crate::sfc_battle_objects::SOURCE_BASE) as usize;
        let table: Vec<u16> = (0..1024)
            .map(|i| u16::from_le_bytes([image[base + 2 * i], image[base + 2 * i + 1]]))
            .collect();
        let sine = |phase: u16| crate::motion::sine(&table, phase.wrapping_add(8) >> 4) as i32;
        let cosine = |phase: u16| {
            crate::motion::sine(&table, ((phase.wrapping_add(8) >> 4) + 1024) & 4095) as i32
        };
        let [px, py, pw, ph] = panel.map(|v| v as i64);
        let screen = |v: i64, axis: usize| -> i64 {
            let (p, size) = [(px, pw), (py, ph)][axis];
            p + ((v - origin[axis]) * size).div_euclid(SURFACE[axis])
        };
        let first = &display.guardians[0];
        let (body0, particle, emitter) = (&first.body, &first.particle, &first.emitter);
        let body_show: Vec<[usize; 2]> = (0..4).map(|f| [f, 2 * STEP_TICKS]).collect();
        let gem_show: Vec<[usize; 2]> = (0..STEPS).map(|f| [f, STEP_TICKS]).collect();
        let particle_frames = [14usize, 15, 16, 17];
        for g in &display.guardians {
            if g.body.show != body_show
                || g.gem.show != gem_show
                || g.body.base != [176, 208]
                || g.body.amplitude != 2
                || g.body.phase_step != 0x800
                || g.gem.depth != 1 << 16
                || g.gem.palette_base != g.gem.key
                || g.emitter.depths != [1 << 16, 0]
                || g.particle.show.iter().map(|s| s[0]).collect::<Vec<_>>() != particle_frames
                || g.particle.show.iter().map(|s| s[1]).sum::<usize>() != AGES
                || g.particle.arc.sides.len() != 2
                || g.particle.arc.sides.iter().any(|s| s.repeats != AGES)
                || g.particle.spread.repeats != AGES
                || g.particle.spread.phase_draw != 0x7fff
                || g.particle.base != particle.base
                || g.particle.arc.draws != particle.arc.draws
            {
                return Err(format!("Guardian display {} differs", g.map_id).into());
            }
        }
        let mut bob_rows = Vec::new();
        for t in 0..AGES as u32 {
            let phase = (body0.phase_step as u32 * (t + 1)) as u16;
            let y = ((body0.base[1] << 16) as i32)
                .wrapping_sub(cosine(phase).wrapping_mul(body0.amplitude as i32))
                >> 16;
            bob_rows.push(screen(i64::from(y), 1) - screen(body0.base[1], 1));
        }
        let mut variants: Vec<i64> = bob_rows.clone();
        variants.sort();
        variants.dedup();
        let bob: Vec<u8> = bob_rows
            .iter()
            .map(|r| variants.iter().position(|v| v == r).unwrap() as u8)
            .collect();
        let start = particle.base;
        let arc = &particle.arc;
        let draws = arc.draws.map(usize::from);
        if draws[0] != draws[1] {
            return Err("Guardian arc draws differ by axis".into());
        }
        let mut xs: Vec<i64> = Vec::new();
        let mut ys: Vec<i64> = Vec::new();
        for side in &arc.sides {
            for (axis, out) in [(0, &mut xs), (1, &mut ys)] {
                for d in 0..draws[axis] {
                    let amp = (arc.amplitudes[axis] + d as i64) as i32;
                    for age in 0..AGES as i32 {
                        let phase =
                            (i32::from(side.phases[axis]) + side.steps[axis] * (age + 1)) as u16;
                        let at = if axis == 0 {
                            ((start[0] << 16) as i32)
                                .wrapping_add((side.base_moves as i32 * amp) << 16)
                                .wrapping_add(sine(phase).wrapping_mul(amp))
                        } else {
                            ((start[1] << 16) as i32).wrapping_sub(cosine(phase).wrapping_mul(amp))
                        };
                        out.push(i64::from(at >> 16) - start[axis]);
                    }
                }
            }
        }
        let spread = &particle.spread;
        let spread_at = xs.len();
        if spread_at != ys.len() || spread_at != 2 * draws[0] * AGES {
            return Err("Guardian arc tables differ in size".into());
        }
        let bucket = 0x8000 / SPREAD_DIRECTIONS;
        for q in 0..SPREAD_DIRECTIONS {
            let phase = spread.phase.wrapping_add((q * bucket + bucket / 2) as u16);
            for s in 0..i64::from(spread.step_draw) {
                for age in 0..AGES as i64 {
                    let amp = (spread.amplitude + age * (spread.step_base + s)) as i32;
                    xs.push(
                        i64::from(
                            ((start[0] << 16) as i32).wrapping_add(sine(phase).wrapping_mul(amp))
                                >> 16,
                        ) - start[0],
                    );
                    ys.push(
                        i64::from(
                            ((start[1] << 16) as i32).wrapping_sub(cosine(phase).wrapping_mul(amp))
                                >> 16,
                        ) - start[1],
                    );
                }
            }
        }
        let to_i8 = |v: &Vec<i64>| -> Result<Vec<u8>> {
            v.iter().map(|&o| Ok(i8::try_from(o)? as u8)).collect()
        };
        let top = [start[0] - 8, start[1] - 8];
        if ys.iter().any(|&o| !(0..256).contains(&(top[1] + o)))
            || xs.iter().any(|&o| {
                !(0..320).contains(&(top[0] + o)) || !(0..320).contains(&(top[0] + o + 15))
            })
        {
            return Err("Guardian particles leave the position tables".into());
        }
        let screen_x: Vec<u8> = (0..320)
            .map(|x| {
                let v = screen(x, 0);
                if (0..255).contains(&v) { v as u8 } else { 0xff }
            })
            .collect();
        let screen_y: Vec<u8> = (0..256)
            .map(|y| {
                let v = screen(y, 1);
                if (0..224).contains(&v) { v as u8 } else { 0xff }
            })
            .collect();
        let mut free: Vec<u16> = NAME_RUNS.iter().flat_map(|&(a, b)| a..b).collect();
        free.reverse();
        let mut guardians = Vec::new();
        let mut body_entries = 0;
        let mut gem_entries = 0;
        let mut report = Vec::new();
        for g in &display.guardians {
            let record = records
                .iter()
                .find(|r| r.file == g.body.file)
                .ok_or("Guardian body sheet missing")?;
            let body_pixels = content.read(format!("floor-objects/{}.indices", g.body.file))?;
            let body_rgb = content.read(format!("floor-objects/{}.rgb", g.body.file))?;
            if body_pixels.len() != record.width * record.height
                || sha(&body_pixels) != record.pixels_sha256
                || sha(&body_rgb) != record.palette_sha256
            {
                return Err("Guardian body sheet differs".into());
            }
            let gem_pixels = dir.read(format!("{}.indices", g.gem.file))?;
            let gem_rgb = dir.read(format!("{}.rgb", g.gem.file))?;
            if gem_pixels.len() != g.gem.width * g.gem.height
                || sha(&gem_pixels) != g.gem.pixels_sha256
                || sha(&gem_rgb) != g.gem.palette_sha256
                || g.gem.width != 4 * g.gem.grid[0]
                || g.gem.height != 2 * g.gem.grid[1]
                || gem_pixels.iter().chain(&body_pixels).any(|&c| c > 15)
            {
                return Err("Guardian gem sheet differs".into());
            }
            let frame = |f: usize| {
                g.body
                    .frames
                    .get(&f.to_string())
                    .ok_or("Guardian frame missing")
            };
            let mut body_frames = Vec::new();
            for f in 0..4 {
                let fr = frame(f)?;
                let place = [
                    g.body.base[0] - fr.anchor[0] - origin[0],
                    g.body.base[1] - fr.anchor[1] - origin[1],
                ];
                body_frames.push(sample(
                    &body_pixels,
                    record.width,
                    fr.rectangle,
                    place,
                    panel,
                ));
            }
            let [gw, gh] = g.gem.grid.map(|v| v as i64);
            let gem_place = [g.gem.place[0] - origin[0], g.gem.place[1] - origin[1]];
            let gem_frames: Vec<_> = (0..STEPS as i64)
                .map(|i| {
                    let rect = [i % 4 * gw, i / 4 * gh, (i % 4 + 1) * gw, (i / 4 + 1) * gh];
                    sample(&gem_pixels, g.gem.width, rect, gem_place, panel)
                })
                .collect();
            let cells = |frames: &[Pixels]| -> Result<Cells> {
                let x0 = frames
                    .iter()
                    .flat_map(|f| f.keys().map(|k| k.0))
                    .min()
                    .ok_or("Empty frames")?;
                let y0 = frames
                    .iter()
                    .flat_map(|f| f.keys().map(|k| k.1))
                    .min()
                    .ok_or("Empty frames")?;
                let mut c: Vec<(i64, i64)> = frames
                    .iter()
                    .flat_map(|f| f.keys().map(|&(x, y)| ((x - x0) / 8, (y - y0) / 8)))
                    .collect();
                c.sort_by_key(|&(x, y)| (y, x));
                c.dedup();
                Ok((x0, y0, c))
            };
            let (bx, by, body_cells) = cells(&body_frames)?;
            let tiles = |f: &BTreeMap<(i64, i64), u8>, x0: i64, y0: i64, c: (i64, i64)| {
                planar(f, x0 + 8 * c.0, y0 + 8 * c.1)
            };
            let (stat, vary): (Vec<_>, Vec<_>) = body_cells.iter().partition(|&&c| {
                body_frames
                    .iter()
                    .all(|f| tiles(f, bx, by, c) == tiles(&body_frames[0], bx, by, c))
            });
            let (gx, gy, gem_cells) = cells(&gem_frames)?;
            let mut particle_tiles = Vec::new();
            for &f in &particle_frames {
                let fr = frame(f)?;
                let [l, t, r, b] = fr.rectangle;
                if fr.anchor != [8, 8] || r - l != 16 || b - t != 16 {
                    return Err("Guardian particle frame differs".into());
                }
                let mut reduced = BTreeMap::new();
                for v in 0..8i64 {
                    for u in 0..8i64 {
                        let mut n = [0usize; 16];
                        for y in v * SURFACE[1] / ph..((v + 1) * SURFACE[1] / ph).min(16) {
                            for x in u * SURFACE[0] / pw..((u + 1) * SURFACE[0] / pw).min(16) {
                                n[usize::from(
                                    body_pixels[(t + y) as usize * record.width + (l + x) as usize],
                                )] += 1;
                            }
                        }
                        if let Some(c) = (1..16)
                            .filter(|&c| n[c] > 0)
                            .max_by_key(|&c| (n[c], std::cmp::Reverse(c)))
                        {
                            reduced.insert((u, v), c as u8);
                        }
                    }
                }
                particle_tiles.push(planar(&reduced, 0, 0));
            }
            let mut take = |n: usize, run: bool| -> Result<Vec<u16>> {
                let mut names = Vec::new();
                for _ in 0..n {
                    names.push(free.pop().ok_or("No free guardian sprite name")?);
                }
                if run && names.windows(2).any(|w| w[1] != w[0] + 1) {
                    return Err("Guardian sprite names split".into());
                }
                Ok(names)
            };
            let gem_names = take(gem_cells.len(), true)?;
            let vary_names = take(vary.len(), true)?;
            let stat_names = take(stat.len(), false)?;
            let particle_names = take(particle_tiles.len(), false)?;
            free.extend(
                gem_names
                    .iter()
                    .chain(&vary_names)
                    .chain(&stat_names)
                    .chain(&particle_names)
                    .rev(),
            );
            free.sort_by(|a, b| b.cmp(a));
            let statics: Vec<(u16, [u8; 32])> = stat
                .iter()
                .zip(&stat_names)
                .map(|(&&c, &n)| (n, tiles(&body_frames[0], bx, by, c)))
                .chain(
                    particle_names
                        .iter()
                        .copied()
                        .zip(particle_tiles.iter().copied()),
                )
                .collect();
            let step_tiles = (0..STEPS)
                .map(|k| {
                    (
                        vary.iter()
                            .map(|&&c| tiles(&body_frames[k / 2], bx, by, c))
                            .collect(),
                        gem_cells
                            .iter()
                            .map(|&c| tiles(&gem_frames[k], gx, gy, c))
                            .collect(),
                    )
                })
                .collect();
            let named: BTreeMap<(i64, i64), u16> = stat
                .iter()
                .map(|&&c| c)
                .zip(stat_names.iter().copied())
                .chain(vary.iter().map(|&&c| c).zip(vary_names.iter().copied()))
                .collect();
            let byte = |v: i64| -> Result<u8> { Ok(u8::try_from(v)?) };
            let mut body_oam = Vec::new();
            for &row in &variants {
                let mut block = Vec::new();
                for (&(cx, cy), &name) in &named {
                    block.push([
                        byte(px + bx + 8 * cx)?,
                        byte(py + by + 8 * cy + row)?,
                        name as u8,
                        attr(name, BODY_BLOCK),
                    ]);
                }
                body_oam.push(block);
            }
            let gem_oam = gem_cells
                .iter()
                .zip(&gem_names)
                .map(|(&(cx, cy), &name)| {
                    Ok([
                        byte(px + gx + 8 * cx)?,
                        byte(py + gy + 8 * cy)?,
                        name as u8,
                        attr(name, GEM_BLOCK),
                    ])
                })
                .collect::<Result<Vec<_>>>()?;
            let mut ages = Vec::new();
            for (i, s) in g.particle.show.iter().enumerate() {
                ages.extend(std::iter::repeat_n(particle_names[i], s[1]));
            }
            body_entries = body_entries.max(named.len());
            gem_entries = gem_entries.max(gem_oam.len());
            report.push(serde_json::json!(
                { "map" : g.map_id, "entry" : g.entry_va, "body" : g.body.file,
                "gem" : g.gem.file, "body_tiles" : named.len(),
                "changing_body_tiles" : vary.len(), "gem_tiles" : gem_cells
                .len(), "gem_top_left" : [px + gx, py + gy], "body_top_left" :
                [px + bx, py + by], "names" : [gem_names.first(), stat_names
                .iter().chain(& particle_names).max()] }
            ));
            guardians.push(Sprites {
                map_id: g.map_id,
                entry_va: g.entry_va,
                statics,
                varying: vary_names,
                gem: gem_names,
                step_tiles,
                body_oam,
                gem_oam,
                particle_names: ages,
                body_rgb: body_rgb[..48].to_vec(),
                gem_rgb: gem_rgb[..48].to_vec(),
            });
            if g.body.palette_base != 176 || g.gem.palette_base != GEM_BLOCK {
                return Err("Guardian palettes moved".into());
            }
        }
        if FRONT + 2 * RING + body_entries + gem_entries > 128 {
            return Err("Guardian sprites exceed the sprite table".into());
        }
        Ok(Self {
            report: serde_json::json!(
                { "guardians" : report, "bob_rows" : bob_rows, "body_entries" :
                body_entries, "gem_entries" : gem_entries, "particle_records" : 2 * RING,
                "spread_directions" : SPREAD_DIRECTIONS, "limits" :
                ["Body, gem and flames are sprites; the picture view keeps the walls and the colours.",
                "The body's rest frames are sampled as the view composer samples pictures; the bob moves them by whole rows.",
                "A particle frame is one 8x8 tile (a block's most drawn colour); the spread's start phase keeps one of 128 directions over its half turn.",
                "Every RNG value of the emitter and the particles is drawn in the source order; one tick is one NMI."]
                }
            ),
            guardians,
            ids: [0; 4],
            bob,
            variants: variants.len(),
            body_entries,
            gem_entries,
            xs: to_i8(&xs)?,
            ys: to_i8(&ys)?,
            screen_x,
            screen_y,
            arc_draws: [
                u8::try_from(draws[0])?,
                u8::try_from(draws[1])?,
                u8::try_from(arc.side_draw)?,
            ],
            spread_step_draw: u8::try_from(spread.step_draw)?,
            x_draw: u8::try_from(particle.x_draw)?,
            path_draw: u8::try_from(particle.path_draw)?,
            emitter_wait: emitter.wait,
            emitter_draw: emitter.draw,
            particle_start: top,
        })
    }
    pub fn sprite_pictures(&self) -> Vec<(u32, u8, Vec<u8>, Vec<u8>)> {
        self.guardians
            .iter()
            .map(|g| (g.entry_va, g.map_id, g.body_rgb.clone(), g.gem_rgb.clone()))
            .collect()
    }
    pub fn maps(&self) -> Vec<(u8, u32)> {
        self.guardians
            .iter()
            .map(|g| (g.map_id, g.entry_va))
            .collect()
    }
    pub fn append(
        &self,
        sections: &mut Vec<Section>,
        space: &mut crate::sfc_rom::Space,
    ) -> Result<()> {
        use super::sfc_b1::{add, transfer};
        if self.ids.contains(&0) {
            return Err("Guardian picture ids not set".into());
        }
        let vram = |name: u16| 0xe000 + (name - 256) * 32;
        let total = self.total();
        let mut jobs = Vec::new();
        let mut bob = Vec::new();
        let mut body_oam = Vec::new();
        let mut gem_oam = Vec::new();
        let mut particle_frames = Vec::new();
        let mut offsets = Vec::new();
        let hidden = [0u8, 224, 0, 0];
        for (gi, g) in self.guardians.iter().enumerate() {
            offsets.extend((u16::try_from(gi * AGES * 2)?).to_le_bytes());
            offsets.extend((u16::try_from(gem_oam.len())?).to_le_bytes());
            offsets.extend((u16::try_from(gi * AGES * 2)?).to_le_bytes());
            offsets.extend((u16::try_from(gi * (1 + STEPS) * 5)?).to_le_bytes());
            let mut list = Vec::new();
            let mut count = 0;
            let mut run: Vec<(u16, [u8; 32])> = Vec::new();
            let mut flush = |run: &mut Vec<(u16, [u8; 32])>,
                             list: &mut Vec<u8>,
                             count: &mut usize|
             -> Result<()> {
                if let Some(&(first, _)) = run.first() {
                    let bytes: Vec<u8> = run.iter().flat_map(|(_, t)| *t).collect();
                    let source = add(
                        sections,
                        space,
                        format!("guardian_static_{}_{first}", g.map_id),
                        bytes.clone(),
                    )?;
                    list.extend(transfer(source, vram(first), bytes.len(), 0));
                    *count += 1;
                    run.clear();
                }
                Ok(())
            };
            for &(name, tile) in &g.statics {
                if run.last().is_some_and(|&(n, _)| n + 1 != name) {
                    flush(&mut run, &mut list, &mut count)?;
                }
                run.push((name, tile));
            }
            flush(&mut run, &mut list, &mut count)?;
            let source = add(
                sections,
                space,
                format!("guardian_static_list_{}", g.map_id),
                list,
            )?;
            jobs.extend(&source.to_le_bytes()[..3]);
            jobs.extend((count as u16).to_le_bytes());
            for (k, (vary, gem)) in g.step_tiles.iter().enumerate() {
                let mut list = Vec::new();
                for (names, tiles, kind) in [(&g.varying, vary, "body"), (&g.gem, gem, "gem")] {
                    let bytes: Vec<u8> = tiles.iter().flatten().copied().collect();
                    let source = super::sfc_b1::add_shared(
                        sections,
                        space,
                        format!("guardian_{kind}_{}_{k}", g.map_id),
                        bytes.clone(),
                    )?;
                    list.extend(transfer(source, vram(names[0]), bytes.len(), 0));
                }
                let source = add(
                    sections,
                    space,
                    format!("guardian_step_list_{}_{k}", g.map_id),
                    list,
                )?;
                jobs.extend(&source.to_le_bytes()[..3]);
                jobs.extend(2u16.to_le_bytes());
            }
            for t in 0..AGES {
                let v = usize::from(self.bob[t]);
                bob.extend(
                    u16::try_from(((gi * self.variants + v) * self.body_entries) * 4)?
                        .to_le_bytes(),
                );
            }
            for block in &g.body_oam {
                for e in block
                    .iter()
                    .copied()
                    .chain(std::iter::repeat(hidden))
                    .take(self.body_entries)
                {
                    body_oam.extend(e);
                }
            }
            for e in g
                .gem_oam
                .iter()
                .copied()
                .chain(std::iter::repeat(hidden))
                .take(self.gem_entries)
            {
                gem_oam.extend(e);
            }
            for &name in &g.particle_names {
                particle_frames.extend([name as u8, attr(name, BODY_BLOCK)]);
            }
        }
        let mut shape = self.ids.to_vec();
        shape.extend([
            u8::try_from(self.body_entries)?,
            u8::try_from(self.gem_entries)?,
            u8::try_from(total)?,
            self.arc_draws[0],
            self.arc_draws[1],
            self.arc_draws[2],
            self.spread_step_draw,
            self.x_draw,
            self.path_draw,
            self.emitter_wait,
            self.emitter_draw,
            u8::try_from(self.particle_start[0])?,
            u8::try_from(self.particle_start[1])?,
        ]);
        add(sections, space, "guardian_shape".into(), shape)?;
        add(sections, space, "guardian_jobs".into(), jobs)?;
        add(sections, space, "guardian_offsets".into(), offsets)?;
        add(sections, space, "guardian_bob".into(), bob)?;
        add(sections, space, "guardian_body_oam".into(), body_oam)?;
        add(sections, space, "guardian_gem_oam".into(), gem_oam)?;
        add(
            sections,
            space,
            "guardian_particle_frames".into(),
            particle_frames,
        )?;
        add(sections, space, "guardian_xs".into(), self.xs.clone())?;
        add(sections, space, "guardian_ys".into(), self.ys.clone())?;
        add(
            sections,
            space,
            "guardian_screen_x".into(),
            self.screen_x.clone(),
        )?;
        add(
            sections,
            space,
            "guardian_screen_y".into(),
            self.screen_y.clone(),
        )?;
        let offset = space.reserve_table(CODE_BYTES)?;
        sections.push(Section {
            name: CODE_SECTION.into(),
            offset,
            bytes: vec![0; CODE_BYTES],
        });
        Ok(())
    }
    fn total(&self) -> usize {
        (FRONT + 2 * RING + self.body_entries + self.gem_entries).next_multiple_of(4)
    }
}
struct Shape {
    body_entries: usize,
    gem_entries: usize,
    total: usize,
    arc_draws: [u8; 3],
    spread_step_draw: u8,
    x_draw: u8,
    path_draw: u8,
    emitter_wait: u8,
    emitter_draw: u8,
    start: [u8; 2],
}
pub fn emit(p: &mut w65c816::Assembler, layout: &CartridgeLayout) -> Result<()> {
    use crate::sfc::{emit, implied};
    use w65c816::{AddressingMode as A, CpuMode, Mnemonic as M, Operand as O};
    let program = code(layout)?;
    let placed = &layout.section(CODE_SECTION)?.bytes;
    if placed[..program.bytes().len()] != *program.bytes() {
        return Err("Guardian display bank differs from its placed section".into());
    }
    let far = program
        .label_location("field_guardian_tick_far")
        .ok_or("Missing guardian entry")?;
    p.label("field_guardian_tick")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    emit(p, M::Jsl, A::AbsoluteLong, O::Long(far.to_u32()));
    implied(p, M::Rts);
    Ok(())
}
pub fn place_code(sections: &mut [Section], layout: &CartridgeLayout) -> Result<()> {
    let mut bytes = code(layout)?.bytes().to_vec();
    let helper = crate::sfc_battle_objects::falling_density::code(layout)?;
    let birth = crate::sfc_battle_objects::falling_birth::code(layout)?;
    bytes.resize(CODE_BYTES, 0);
    let start = crate::sfc_battle_objects::falling_birth::OFFSET;
    bytes[start..start + birth.bytes().len()].copy_from_slice(birth.bytes());
    let common_birth = crate::sfc_battle_objects::common_birth::code(layout)?;
    let start = crate::sfc_battle_objects::common_birth::OFFSET;
    if bytes[start..start + common_birth.bytes().len()]
        .iter()
        .any(|&b| b != 0)
    {
        return Err("Common birth overlaps occupied guardian bytes".into());
    }
    bytes[start..start + common_birth.bytes().len()].copy_from_slice(common_birth.bytes());
    let copies = crate::sfc_battle_objects::bound_child_copy::code(layout)?;
    let start = crate::sfc_battle_objects::bound_child_copy::OFFSET;
    if bytes[start..start + copies.bytes().len()]
        .iter()
        .any(|&b| b != 0)
    {
        return Err("Bound-child copy overlaps occupied guardian bytes".into());
    }
    bytes[start..start + copies.bytes().len()].copy_from_slice(copies.bytes());
    let setup = crate::sfc_battle_objects::falling_setup::code(layout)?;
    let start = crate::sfc_battle_objects::falling_setup::OFFSET;
    if bytes[start..start + setup.bytes().len()]
        .iter()
        .any(|&b| b != 0)
    {
        return Err("Falling setup overlaps occupied guardian bytes".into());
    }
    bytes[start..start + setup.bytes().len()].copy_from_slice(setup.bytes());
    let projection = crate::sfc_battle_objects::inward_projection::code(layout)?;
    let start = crate::sfc_battle_objects::inward_projection::OFFSET;
    if bytes[start..start + projection.bytes().len()]
        .iter()
        .any(|&b| b != 0)
    {
        return Err("Inward projection overlaps occupied guardian bytes".into());
    }
    bytes[start..start + projection.bytes().len()].copy_from_slice(projection.bytes());
    let start = crate::sfc_battle_objects::falling_density::OFFSET;
    bytes[start..start + helper.bytes().len()].copy_from_slice(helper.bytes());
    sections
        .iter_mut()
        .find(|s| s.name == CODE_SECTION)
        .ok_or("Missing guardian code section")?
        .bytes = bytes;
    Ok(())
}
pub fn code_bytes(layout: &CartridgeLayout) -> Result<usize> {
    let helper = crate::sfc_battle_objects::falling_density::code(layout)?;
    Ok(crate::sfc_battle_objects::falling_density::OFFSET + helper.bytes().len())
}
fn code(layout: &CartridgeLayout) -> Result<w65c816::AssembledProgram> {
    use crate::sfc::{emit, implied};
    use crate::sfc_b1::{FADE_SLOT, MENU_OAM_SLOT, MENU_STRIP_SLOT, QUEUE};
    use crate::sfc_b1_menu::OAM_BUFFER;
    use crate::sfc_field_events::{LIVE, LIVE_STATE};
    use w65c816::{AddressingMode as A, CpuMode, Mnemonic as M, Operand as O};
    let at = layout.cpu_address(layout.section(CODE_SECTION)?.offset)?;
    let raw = &layout.section("guardian_shape")?.bytes;
    let s = Shape {
        body_entries: raw[4].into(),
        gem_entries: raw[5].into(),
        total: raw[6].into(),
        arc_draws: [raw[7], raw[8], raw[9]],
        spread_step_draw: raw[10],
        x_draw: raw[11],
        path_draw: raw[12],
        emitter_wait: raw[13],
        emitter_draw: raw[14],
        start: [raw[15], raw[16]],
    };
    let address = |name: &str| -> Result<u32> { layout.cpu_address(layout.section(name)?.offset) };
    let rng = crate::sfc_battle::code(layout)?
        .label_location("encounter_rng_far")
        .ok_or("Missing RNG entry")?
        .to_u32();
    let (jobs, offsets, bob, body_oam, gem_oam, frames, xs, ys, screen_x, screen_y, oam_job) = (
        address("guardian_jobs")?,
        address("guardian_offsets")?,
        address("guardian_bob")?,
        address("guardian_body_oam")?,
        address("guardian_gem_oam")?,
        address("guardian_particle_frames")?,
        address("guardian_xs")?,
        address("guardian_ys")?,
        address("guardian_screen_x")?,
        address("guardian_screen_y")?,
        address("menu_oam_job")?,
    );
    let arcs = 2 * u16::from(s.arc_draws[0]) * AGES as u16;
    let mut p = w65c816::Assembler::new();
    let p = &mut p;
    let abs = |p: &mut w65c816::Assembler, m, at: u16| emit(p, m, A::Absolute, O::Word(at));
    let abs_x = |p: &mut w65c816::Assembler, m, at: u16| emit(p, m, A::AbsoluteX, O::Word(at));
    let abs_y = |p: &mut w65c816::Assembler, m, at: u16| emit(p, m, A::AbsoluteY, O::Word(at));
    let long_x = |p: &mut w65c816::Assembler, m, at: u32| emit(p, m, A::AbsoluteLongX, O::Long(at));
    let imm = |p: &mut w65c816::Assembler, m, v: u8| emit(p, m, A::Immediate, O::Byte(v));
    let imm16 = |p: &mut w65c816::Assembler, m, v: u16| emit(p, m, A::Immediate, O::Word(v));
    let m16 = |p: &mut w65c816::Assembler| emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    let m8 = |p: &mut w65c816::Assembler| emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    let call = |p: &mut w65c816::Assembler, label: &str| {
        p.emit_label_ref(M::Jsr, A::Absolute, label);
    };
    let branch = |p: &mut w65c816::Assembler, m, label: &str| {
        p.emit_label_ref(m, A::Relative8, label);
    };
    let jump = |p: &mut w65c816::Assembler, label: &str| {
        p.emit_label_ref(M::Jmp, A::Absolute, label);
    };
    let random = |p: &mut w65c816::Assembler, divisor: u8| {
        imm(p, M::Lda, divisor);
        emit(p, M::Jsl, A::AbsoluteLong, O::Long(rng));
    };
    let asl = |p: &mut w65c816::Assembler, n: usize| {
        for _ in 0..n {
            emit(p, M::Asl, A::Accumulator, O::None);
        }
    };
    let oam = OAM_BUFFER;
    let body_at = (FRONT + RING) as u16 * 4;
    let back_at = body_at + s.body_entries as u16 * 4;
    let gem_at = back_at + RING as u16 * 4;
    p.label("field_guardian_tick_far")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    call(p, "guardian_tick");
    implied(p, M::Rtl);
    p.label("guardian_tick").assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, 0x50c);
    implied(p, M::Sec);
    imm(p, M::Sbc, 13);
    imm(p, M::Cmp, 4);
    branch(p, M::Bcs, "guardian_gone");
    m16(p);
    imm16(p, M::And, 0xff);
    implied(p, M::Tax);
    m8(p);
    abs(p, M::Lda, LIVE_STATE);
    abs(p, M::Cmp, 0x500);
    branch(p, M::Bne, "guardian_gone");
    let ids = address("guardian_shape")?;
    long_x(p, M::Lda, ids);
    abs(p, M::Cmp, LIVE);
    branch(p, M::Beq, "guardian_live");
    abs(p, M::Cmp, LIVE + 1);
    branch(p, M::Beq, "guardian_live");
    p.label("guardian_gone");
    abs(p, M::Lda, GUARD_TIME);
    abs(p, M::Ora, GUARD_TIME + 1);
    branch(p, M::Bne, "guardian_stop");
    implied(p, M::Rts);
    p.label("guardian_stop");
    abs(p, M::Stz, GUARD_TIME);
    abs(p, M::Stz, GUARD_TIME + 1);
    call(p, "guardian_hide");
    jump(p, "guardian_oam_first");
    p.label("guardian_live");
    m16(p);
    abs(p, M::Lda, GUARD_TIME);
    branch(p, M::Beq, "guardian_new");
    jump(p, "guardian_running");
    p.label("guardian_new");
    m8(p);
    implied(p, M::Txa);
    abs(p, M::Sta, GUARD);
    abs(p, M::Stz, STATIC_SENT);
    abs(p, M::Stz, READY);
    imm(p, M::Lda, 0xff);
    abs(p, M::Sta, STEP_SENT);
    abs(p, M::Stz, HEADS);
    abs(p, M::Stz, HEADS + 1);
    for k in 0..2 * RING as u16 {
        abs(p, M::Stz, RECORDS + k * RECORD as u16);
    }
    m16(p);
    implied(p, M::Txa);
    asl(p, 3);
    implied(p, M::Tax);
    for k in 0..4u16 {
        long_x(p, M::Lda, offsets + 2 * u32::from(k));
        abs(p, M::Sta, OFFSETS + 2 * k);
    }
    m8(p);
    call(p, "guardian_hide");
    call(p, "guardian_oam_first");
    m16(p);
    p.label("guardian_running");
    abs(p, M::Inc, GUARD_TIME);
    branch(p, M::Bne, "guardian_counted");
    imm16(p, M::Lda, 256);
    abs(p, M::Sta, GUARD_TIME);
    p.label("guardian_counted");
    abs(p, M::Lda, GUARD_TIME);
    imm16(p, M::Cmp, 1);
    m8(p);
    branch(p, M::Bne, "guardian_emitter");
    call(p, "guardian_pair");
    jump(p, "guardian_uploads");
    p.label("guardian_emitter");
    abs(p, M::Lda, SECOND_WAIT);
    branch(p, M::Bne, "guardian_second_wait");
    abs(p, M::Dec, WAIT);
    branch(p, M::Bne, "guardian_uploads");
    random(p, s.emitter_draw);
    emit(p, M::Inc, A::Accumulator, O::None);
    abs(p, M::Sta, WAIT);
    imm(p, M::Lda, 1);
    abs(p, M::Sta, SECOND_WAIT);
    branch(p, M::Bra, "guardian_uploads");
    p.label("guardian_second_wait");
    abs(p, M::Dec, WAIT);
    branch(p, M::Bne, "guardian_uploads");
    call(p, "guardian_pair");
    p.label("guardian_uploads");
    let slot = QUEUE + FADE_SLOT;
    abs(p, M::Lda, slot + 3);
    abs(p, M::Ora, slot + 4);
    branch(p, M::Bne, "guardian_ready_check");
    abs(p, M::Lda, STATIC_SENT);
    branch(p, M::Bne, "guardian_step");
    imm(p, M::Lda, 1);
    abs(p, M::Sta, STATIC_SENT);
    m16(p);
    abs(p, M::Lda, OFFSETS + 6);
    implied(p, M::Tax);
    m8(p);
    call(p, "guardian_enqueue");
    implied(p, M::Rts);
    p.label("guardian_step");
    m16(p);
    abs(p, M::Lda, GUARD_TIME);
    emit(p, M::Dec, A::Accumulator, O::None);
    for _ in 0..STEP_TICKS.trailing_zeros() {
        emit(p, M::Lsr, A::Accumulator, O::None);
    }
    imm16(p, M::And, (STEPS - 1) as u16);
    m8(p);
    abs(p, M::Cmp, STEP_SENT);
    branch(p, M::Beq, "guardian_ready_check");
    abs(p, M::Sta, STEP_SENT);
    m16(p);
    imm16(p, M::And, 0xff);
    emit(p, M::Inc, A::Accumulator, O::None);
    abs(p, M::Sta, S_T);
    asl(p, 2);
    implied(p, M::Clc);
    abs(p, M::Adc, S_T);
    implied(p, M::Clc);
    abs(p, M::Adc, OFFSETS + 6);
    implied(p, M::Tax);
    m8(p);
    call(p, "guardian_enqueue");
    p.label("guardian_ready_check");
    abs(p, M::Lda, READY);
    branch(p, M::Bne, "guardian_place");
    abs(p, M::Lda, STEP_SENT);
    imm(p, M::Cmp, 0xff);
    branch(p, M::Beq, "guardian_not_ready");
    abs(p, M::Lda, slot + 3);
    abs(p, M::Ora, slot + 4);
    abs(p, M::Ora, QUEUE + 3);
    abs(p, M::Ora, QUEUE + 4);
    branch(p, M::Beq, "guardian_now_ready");
    p.label("guardian_not_ready");
    implied(p, M::Rts);
    p.label("guardian_now_ready");
    imm(p, M::Lda, 1);
    abs(p, M::Sta, READY);
    m16(p);
    abs(p, M::Lda, OFFSETS + 2);
    implied(p, M::Tax);
    imm16(p, M::Ldy, 0);
    p.label("guardian_gem_copy");
    long_x(p, M::Lda, gem_oam);
    abs_y(p, M::Sta, oam + gem_at);
    implied(p, M::Inx);
    implied(p, M::Inx);
    implied(p, M::Iny);
    implied(p, M::Iny);
    imm16(p, M::Cpy, s.gem_entries as u16 * 4);
    branch(p, M::Bne, "guardian_gem_copy");
    m8(p);
    p.label("guardian_place");
    m16(p);
    abs(p, M::Lda, GUARD_TIME);
    emit(p, M::Dec, A::Accumulator, O::None);
    imm16(p, M::And, (AGES - 1) as u16);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Clc);
    abs(p, M::Adc, OFFSETS);
    implied(p, M::Tax);
    long_x(p, M::Lda, bob);
    implied(p, M::Tax);
    imm16(p, M::Ldy, 0);
    p.label("guardian_body_copy");
    long_x(p, M::Lda, body_oam);
    abs_y(p, M::Sta, oam + body_at);
    implied(p, M::Inx);
    implied(p, M::Inx);
    implied(p, M::Iny);
    implied(p, M::Iny);
    imm16(p, M::Cpy, s.body_entries as u16 * 4);
    branch(p, M::Bne, "guardian_body_copy");
    m8(p);
    for (group, entries) in [(1u16, FRONT as u16 * 4), (0, back_at)] {
        let label = |name: &str| format!("guardian_{name}_{group}");
        abs(p, M::Lda, HEADS + group);
        abs(p, M::Sta, S_SLOT);
        imm(p, M::Lda, RING as u8);
        abs(p, M::Sta, S_K);
        imm16(p, M::Ldy, oam + entries);
        p.label(label("each"));
        abs(p, M::Lda, S_SLOT);
        branch(p, M::Bne, &label("wrapped"));
        imm(p, M::Lda, RING as u8);
        p.label(label("wrapped"));
        emit(p, M::Dec, A::Accumulator, O::None);
        abs(p, M::Sta, S_SLOT);
        m16(p);
        imm16(p, M::And, 0xff);
        asl(p, RECORD.trailing_zeros() as usize);
        implied(p, M::Clc);
        imm16(p, M::Adc, group * (RING * RECORD) as u16);
        implied(p, M::Tax);
        m8(p);
        abs_x(p, M::Lda, RECORDS);
        branch(p, M::Beq, &label("hide"));
        abs(p, M::Lda, GUARD_TIME);
        implied(p, M::Sec);
        abs_x(p, M::Sbc, RECORDS + 1);
        imm(p, M::Cmp, AGES as u8);
        branch(p, M::Bcc, &label("shown"));
        p.label(label("hide"));
        imm(p, M::Lda, 224);
        abs_y(p, M::Sta, 1);
        jump(p, &label("next"));
        p.label(label("shown"));
        abs(p, M::Sta, S_AGE);
        abs(p, M::Stz, S_AGE + 1);
        abs_x(p, M::Lda, RECORDS + 2);
        abs(p, M::Sta, S_R16);
        abs(p, M::Stz, S_R16 + 1);
        m16(p);
        abs_x(p, M::Lda, RECORDS + 4);
        abs(p, M::Sta, S_XO);
        abs_x(p, M::Lda, RECORDS + 6);
        abs(p, M::Sta, S_YO);
        abs(p, M::Lda, S_XO);
        implied(p, M::Clc);
        abs(p, M::Adc, S_AGE);
        implied(p, M::Tax);
        long_x(p, M::Lda, xs);
        imm16(p, M::And, 0xff);
        imm16(p, M::Cmp, 0x80);
        branch(p, M::Bcc, &label("x_up"));
        imm16(p, M::Ora, 0xff00);
        p.label(label("x_up"));
        implied(p, M::Clc);
        abs(p, M::Adc, S_R16);
        implied(p, M::Clc);
        imm16(p, M::Adc, u16::from(s.start[0]));
        implied(p, M::Tax);
        m8(p);
        long_x(p, M::Lda, screen_x);
        imm(p, M::Cmp, 0xff);
        branch(p, M::Beq, &label("hide"));
        abs_y(p, M::Sta, 0);
        m16(p);
        abs(p, M::Lda, S_YO);
        implied(p, M::Clc);
        abs(p, M::Adc, S_AGE);
        implied(p, M::Tax);
        m8(p);
        long_x(p, M::Lda, ys);
        implied(p, M::Clc);
        imm(p, M::Adc, s.start[1]);
        m16(p);
        imm16(p, M::And, 0xff);
        implied(p, M::Tax);
        m8(p);
        long_x(p, M::Lda, screen_y);
        abs_y(p, M::Sta, 1);
        m16(p);
        abs(p, M::Lda, S_AGE);
        emit(p, M::Asl, A::Accumulator, O::None);
        implied(p, M::Clc);
        abs(p, M::Adc, OFFSETS + 4);
        implied(p, M::Tax);
        long_x(p, M::Lda, frames);
        abs_y(p, M::Sta, 2);
        m8(p);
        p.label(label("next"));
        implied(p, M::Iny);
        implied(p, M::Iny);
        implied(p, M::Iny);
        implied(p, M::Iny);
        abs(p, M::Dec, S_K);
        branch(p, M::Beq, &label("done"));
        jump(p, &label("each"));
        p.label(label("done"));
    }
    for (label, slot) in [
        ("guardian_oam_job", MENU_OAM_SLOT),
        ("guardian_oam_first", MENU_STRIP_SLOT),
    ] {
        p.label(label).assume_mode(CpuMode::NATIVE_M8_X16);
        m16(p);
        for i in [0u32, 2] {
            emit(p, M::Lda, A::AbsoluteLong, O::Long(oam_job + i));
            abs(p, M::Sta, QUEUE + slot + i as u16);
        }
        m8(p);
        emit(p, M::Lda, A::AbsoluteLong, O::Long(oam_job + 4));
        abs(p, M::Sta, QUEUE + slot + 4);
        implied(p, M::Rts);
    }
    p.label("guardian_enqueue")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    for i in 0..5u16 {
        long_x(p, M::Lda, jobs + u32::from(i));
        abs(p, M::Sta, slot + i);
    }
    implied(p, M::Rts);
    p.label("guardian_hide").assume_mode(CpuMode::NATIVE_M8_X16);
    imm16(p, M::Ldx, 0);
    p.label("guardian_hide_entry");
    imm(p, M::Lda, 224);
    abs_x(p, M::Sta, oam + 1);
    implied(p, M::Inx);
    implied(p, M::Inx);
    implied(p, M::Inx);
    implied(p, M::Inx);
    imm16(p, M::Cpx, s.total as u16 * 4);
    branch(p, M::Bne, "guardian_hide_entry");
    for i in 0..(s.total / 4) as u16 {
        abs(p, M::Stz, oam + 512 + i);
    }
    implied(p, M::Rts);
    p.label("guardian_pair").assume_mode(CpuMode::NATIVE_M8_X16);
    imm(p, M::Lda, s.emitter_wait);
    abs(p, M::Sta, WAIT);
    abs(p, M::Stz, SECOND_WAIT);
    for group in 0..2u16 {
        let label = |name: &str| format!("guardian_spawn_{name}_{group}");
        abs(p, M::Lda, HEADS + group);
        m16(p);
        imm16(p, M::And, 0xff);
        asl(p, RECORD.trailing_zeros() as usize);
        implied(p, M::Clc);
        imm16(p, M::Adc, group * (RING * RECORD) as u16);
        implied(p, M::Tay);
        m8(p);
        abs(p, M::Inc, HEADS + group);
        abs(p, M::Lda, HEADS + group);
        imm(p, M::Cmp, RING as u8);
        branch(p, M::Bne, &label("head"));
        abs(p, M::Stz, HEADS + group);
        p.label(label("head"));
        random(p, s.x_draw);
        abs_y(p, M::Sta, RECORDS + 2);
        random(p, s.path_draw);
        imm(p, M::Cmp, 0);
        branch(p, M::Bne, &label("spread"));
        random(p, s.arc_draws[0]);
        abs(p, M::Sta, S_AX);
        abs(p, M::Stz, S_AX + 1);
        random(p, s.arc_draws[1]);
        abs(p, M::Sta, S_AY);
        abs(p, M::Stz, S_AY + 1);
        random(p, s.arc_draws[2]);
        m16(p);
        imm16(p, M::And, 0xff);
        branch(p, M::Beq, &label("side"));
        imm16(p, M::Lda, u16::from(s.arc_draws[0]));
        p.label(label("side"));
        abs(p, M::Sta, S_T);
        abs(p, M::Lda, S_AX);
        implied(p, M::Clc);
        abs(p, M::Adc, S_T);
        asl(p, 5);
        abs_y(p, M::Sta, RECORDS + 4);
        abs(p, M::Lda, S_AY);
        implied(p, M::Clc);
        abs(p, M::Adc, S_T);
        asl(p, 5);
        abs_y(p, M::Sta, RECORDS + 6);
        m8(p);
        jump(p, &label("stored"));
        p.label(label("spread"));
        random(p, 0);
        m16(p);
        abs(p, M::Lda, crate::sfc_encounter::SEED + 2);
        imm16(p, M::Cmp, 0xfffe);
        branch(p, M::Bcc, &label("below"));
        implied(p, M::Sec);
        imm16(p, M::Sbc, 0xfffe);
        branch(p, M::Bra, &label("reduced"));
        p.label(label("below"));
        imm16(p, M::Cmp, 0x7fff);
        branch(p, M::Bcc, &label("reduced"));
        implied(p, M::Sec);
        imm16(p, M::Sbc, 0x7fff);
        p.label(label("reduced"));
        for _ in 0..(0x8000 / SPREAD_DIRECTIONS).trailing_zeros() {
            emit(p, M::Lsr, A::Accumulator, O::None);
        }
        abs(p, M::Sta, S_T);
        for _ in 1..s.spread_step_draw {
            implied(p, M::Clc);
            abs(p, M::Adc, S_T);
        }
        abs(p, M::Sta, S_T);
        m8(p);
        random(p, s.spread_step_draw);
        m16(p);
        imm16(p, M::And, 0xff);
        implied(p, M::Clc);
        abs(p, M::Adc, S_T);
        asl(p, 5);
        implied(p, M::Clc);
        imm16(p, M::Adc, arcs);
        abs_y(p, M::Sta, RECORDS + 4);
        abs_y(p, M::Sta, RECORDS + 6);
        m8(p);
        p.label(label("stored"));
        abs(p, M::Lda, GUARD_TIME);
        abs_y(p, M::Sta, RECORDS + 1);
        imm(p, M::Lda, 1);
        abs_y(p, M::Sta, RECORDS);
    }
    implied(p, M::Rts);
    let program = crate::sfc_width_check::assemble(
        p,
        w65c816::CodeLocation::new((at >> 16) as u8, at as u16),
        CpuMode::NATIVE_M8_X16,
    )?;
    if program.bytes().len() > crate::sfc_battle_objects::falling_birth::OFFSET {
        return Err(format!(
            "Guardian display code takes {} bytes",
            program.bytes().len()
        )
        .into());
    }
    Ok(program)
}
