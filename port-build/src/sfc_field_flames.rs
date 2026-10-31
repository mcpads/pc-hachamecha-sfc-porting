use crate::{Result, SOURCE, sha};
use serde::Deserialize;
use sfc_builder::cartridge::{CartridgeLayout, Section};
use std::collections::BTreeMap;
pub const FLAME_TIME: u16 = 0x7e4;
const FLAME_RECORDS: u16 = 0x1b00;
const SLOTS: usize = 12;
const REC: usize = 5;
const DRAWN_COLUMNS: usize = 6;
const FLAME_HEIGHTS: u16 = FLAME_RECORDS + (DRAWN_COLUMNS * SLOTS * REC) as u16;
const FLAME_SIZES: u16 = FLAME_HEIGHTS + DRAWN_COLUMNS as u16;
const EVERY: usize = 3;
const PER_COLUMN: usize = SLOTS / EVERY;
const SPRITES: usize = DRAWN_COLUMNS * PER_COLUMN;
const FIRST_ENTRY: usize = 92;
const PALETTE: u8 = 3;
const FIRST_COLOUR: u16 = 128 + 16 * PALETTE as u16 + 4;
const COLOURS: usize = 12;
const CELLS: [u16; 6] = [355, 357, 359, 361, 363, 365];
const SINGLES: [u16; 5] = [367, 368, 369, 370, 383];
const SINE_STEPS: usize = 128;
const SURFACE: [i64; 2] = [224, 240];
const ALL_HEIGHTS: u16 = FLAME_SIZES + SPRITES as u16;
const X_INDEX: u16 = ALL_HEIGHTS + 16;
const Y_INDEX: u16 = X_INDEX + 2;
const MOVED: u16 = Y_INDEX + 2;
const OWED: u16 = MOVED + 2;
const PHASE: u16 = OWED + 1;
const _: () = assert!((PHASE as usize) < 0x1d00);
#[derive(Deserialize)]
struct Node {
    va: u32,
    raw_hex: String,
}
#[derive(Deserialize)]
struct Sheet {
    schema: u8,
    body_va: u32,
    nodes: Vec<Node>,
    source_sha256: String,
    width: usize,
    height: usize,
    clips: Vec<[i32; 4]>,
    pixels_sha256: String,
    palette_sha256: String,
}
pub struct Assets {
    pub body: u32,
    pub body_id: u8,
    column_draw: u8,
    columns: usize,
    spark_draws: Vec<u8>,
    flame_draws: [u8; 3],
    period: usize,
    life: usize,
    wait: usize,
    xs: Vec<u8>,
    ys: Vec<u8>,
    steps: usize,
    ages: Vec<u8>,
    tiles: Vec<u8>,
    colours: Vec<u8>,
    pub report: serde_json::Value,
}
fn word(raw: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([raw[at], raw[at + 1]])
}
fn long(raw: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(raw[at..at + 4].try_into().unwrap())
}
fn walk(nodes: &BTreeMap<u32, Vec<u8>>, mut at: u32, count: usize) -> Result<Vec<(u32, &[u8])>> {
    let mut run = Vec::new();
    while run.len() < count {
        let raw = nodes
            .get(&at)
            .ok_or_else(|| format!("Fire barrier command {at:#x} missing"))?;
        if raw[0] == 0x03 {
            at = long(raw, 4);
            continue;
        }
        run.push((at, raw.as_slice()));
        at += raw.len() as u32;
    }
    Ok(run)
}
fn expect(raw: &[u8], prefix: &[u8]) -> Result<()> {
    if raw.starts_with(prefix) {
        Ok(())
    } else {
        Err(format!("Unexpected fire barrier command {raw:02x?}").into())
    }
}
fn fields(raw: &[u8]) -> Result<BTreeMap<u8, u32>> {
    expect(raw, &[0x08])?;
    let mut out = BTreeMap::new();
    let mut at = 4;
    while raw[at] != 0xff {
        match raw[at] {
            2 => {
                out.insert(raw[at + 1], u32::from(word(raw, at + 2)));
                at += 4;
            }
            3 => {
                out.insert(raw[at + 1], long(raw, at + 4));
                at += 8;
            }
            _ => return Err("Unknown fire barrier field kind".into()),
        }
    }
    Ok(out)
}
impl Assets {
    pub fn load(content: &crate::content::Content, panel: [usize; 4]) -> Result<Self> {
        let dir = content.sub("b5-flames");
        let raw = dir.read("flames.json")?;
        let sheet: Sheet = serde_json::from_slice(&raw)?;
        let mut nodes = BTreeMap::new();
        for node in &sheet.nodes {
            let raw = (0..node.raw_hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&node.raw_hex[i..i + 2], 16))
                .collect::<std::result::Result<Vec<u8>, _>>()?;
            nodes.insert(node.va, raw);
        }
        let objects: serde_json::Value = content.json("floor-objects/floor-objects.json")?;
        let origin: [i64; 2] = serde_json::from_value(objects["screen_origin"].clone())?;
        let bodies: Vec<u32> = objects["floors"]
            .as_array()
            .ok_or("Missing floor objects")?
            .iter()
            .flat_map(|f| f["objects"].as_array().into_iter().flatten())
            .filter(|o| o["resource"] == "FIREWALL.CNS")
            .filter_map(|o| o["entry_va"].as_u64().map(|v| v as u32))
            .collect();
        let [body] = bodies[..] else {
            return Err("Expected one fire barrier body".into());
        };
        if body != sheet.body_va {
            return Err("Flame sheet names another body".into());
        }
        let run = walk(&nodes, body, 27)?;
        let pattern: [&[u8]; 27] = [
            &[0x3f],
            &[0x12, 0x80, 0x1c],
            &[0x12, 0x80, 0x20],
            &[0x12, 0x80, 0x24],
            &[0x12, 0x80, 0x28],
            &[0x11, 0x80, 0x8c, 0x00, 0x01, 0x00],
            &[0x77, 0x01],
            &[0x1f],
            &[0x2b, 0x00],
            &[0x12, 0x83, 0x58, 0x00, 0x00, 0x00, 0x01, 0x00],
            &[0x12, 0x80, 0x24],
            &[0x12, 0xa2, 0x24, 0x58],
            &[0x12, 0xa0, 0x84, 0x20],
            &[0x12, 0xa1, 0x84, 0x58],
            &[0x12, 0xa0, 0xa8, 0xa4],
            &[0x12, 0xe0, 0x1c, 0x1c],
            &[0x12, 0xe0, 0x20, 0x84],
            &[0x12, 0xe0, 0x24, 0x24],
            &[0x12, 0xe0, 0x28, 0x28],
            &[0x11, 0x82, 0x8c, 0x00, 0x01, 0x00],
            &[0x14, 0x80, 0x8c],
            &[0x12, 0x81, 0x1c],
            &[0x06, 0x00],
            &[0x77, 0x01],
            &[0x01],
            &[0x74],
            &[0x3f],
        ];
        for ((_, raw), want) in run.iter().zip(pattern) {
            expect(raw, want)?;
        }
        if long(run[20].1, 8) != run[6].0 || long(run[22].1, 4) != run[5].0 {
            return Err("Fire barrier body loops apart".into());
        }
        let x0 = i64::from(long(run[1].1, 4) >> 16);
        let y0 = i64::from(long(run[2].1, 4) >> 16);
        let column_draw = run[8].1[2];
        let dx = i64::from(long(run[21].1, 4) >> 16);
        let columns = usize::from(word(run[22].1, 2));
        let (emitter, spark) = (long(run[6].1, 4), long(run[23].1, 4));
        let run = walk(&nodes, emitter, 13)?;
        let pattern: [&[u8]; 13] = [
            &[0x12, 0x80, 0x14],
            &[0x11, 0x80, 0x8c],
            &[0x11, 0x82, 0x8c, 0x00, 0x01, 0x00],
            &[0x14, 0x80, 0x8c],
            &[0x77, 0x01],
            &[0x1f],
            &[0x12, 0xa0, 0xa8, 0xa4],
            &[0x12, 0xe0, 0x80, 0x1c],
            &[0x12, 0xe0, 0x20, 0x20],
            &[0x12, 0xe0, 0x24, 0x24],
            &[0x11, 0x80, 0x8c],
            &[0x01],
            &[0x74],
        ];
        for ((_, raw), want) in run.iter().zip(pattern) {
            expect(raw, want)?;
        }
        let period = usize::from(word(run[1].1, 4));
        if word(run[10].1, 4) as usize != period || long(run[3].1, 8) != run[11].0 {
            return Err("Fire barrier emitter differs".into());
        }
        let flame = long(run[4].1, 4);
        let init = fields(&nodes[&flame])?;
        if init.get(&0).is_none_or(|f| f & 0x4000_0000 == 0) {
            return Err("Flames no longer draw in screen space".into());
        }
        let (amplitude0, life) = (init[&0x92], init[&0x96] as usize);
        let run = walk(&nodes, flame + nodes[&flame].len() as u32, 9)?;
        let pattern: [&[u8]; 9] = [
            &[0x2b, 0x00],
            &[0x11, 0xa1, 0x92, 0x58],
            &[0x2b, 0x00, 0xff, 0xff],
            &[0x11, 0xa0, 0x8c, 0x58],
            &[0x2b, 0x00],
            &[0x12, 0x81, 0x58],
            &[0x12, 0x83, 0x58, 0x00, 0x00, 0x00, 0x01, 0x00],
            &[0x2d, 0x09],
            &[0x01],
        ];
        for ((_, raw), want) in run.iter().zip(pattern) {
            expect(raw, want)?;
        }
        let flame_draws = [run[0].1[2], 0, run[4].1[2]];
        let speed0 = long(run[5].1, 4);
        let mut at = run[8].0 + 4;
        let mut wait = 1;
        let animation = loop {
            let raw = &nodes[&at];
            expect(raw, &[0x74])?;
            let (_, next) = walk(&nodes, long(raw, 4), 1)?[0];
            if next[0] == 0x01 {
                wait += 1;
                at = long(raw, 4) + 4;
                continue;
            }
            expect(next, &[0x20])?;
            break (long(raw, 4), long(next, 4));
        };
        let run = walk(&nodes, animation.0 + 8, 8)?;
        let pattern: [&[u8]; 8] = [
            &[0x12, 0xa2, 0x20, 0x58],
            &[0x11, 0x81, 0x8c],
            &[0x2d, 0x09],
            &[0x01],
            &[0x11, 0x82, 0x96, 0x00, 0x01, 0x00],
            &[0x14, 0x81, 0x96],
            &[0x74],
            &[0x00],
        ];
        for ((_, raw), want) in run.iter().zip(pattern) {
            expect(raw, want)?;
        }
        if long(run[6].1, 4) != run[0].0 {
            return Err("Flame loop differs".into());
        }
        let step = word(run[1].1, 4);
        let mut shows = Vec::new();
        for (_, raw) in walk(&nodes, animation.1, 9)? {
            if raw[0] == 0x01 {
                break;
            }
            expect(raw, &[0x21, 0x00])?;
            shows.push((usize::from(word(raw, 4)), usize::from(word(raw, 2))));
        }
        let mut spark_draws = Vec::new();
        let mut at = spark;
        loop {
            let raw = &nodes[&at];
            match raw[0] {
                0x2b => spark_draws.push(raw[2]),
                0x0a => break,
                _ => {}
            }
            at += raw.len() as u32;
        }
        if run_len(&nodes, spark)? > 40 || spark_draws.is_empty() {
            return Err("Spark differs".into());
        }
        let image = content.read("battle-scripts.bin")?;
        let manifest: serde_json::Value = content.json("battle-scripts.json")?;
        if manifest["image_sha256"] != sha(&image) {
            return Err("Battle script image differs from its manifest".into());
        }
        let base = (0x45b3c8 - crate::sfc_battle_objects::SOURCE_BASE) as usize;
        let table: Vec<u16> = (0..1024).map(|i| word(&image, base + 2 * i)).collect();
        let [px, py, pw, ph] = panel.map(|v| v as i64);
        let screen = |v: i64, axis: usize| -> i64 {
            let (p, size) = [(px, pw), (py, ph)][axis];
            p + ((v - origin[axis]) * size).div_euclid(SURFACE[axis])
        };
        let byte = |v: i64| -> Result<u8> { Ok(u8::try_from(v)?) };
        let steps = life + 1;
        let amplitudes = usize::from(flame_draws[0]);
        if usize::from(step) * SINE_STEPS != 65536 {
            return Err("Flame angle step is not 1/128 turn".into());
        }
        let mut xs = Vec::new();
        for c in 0..DRAWN_COLUMNS {
            let column = x0 + dx * (2 * c) as i64;
            for a in 0..amplitudes {
                let amp = amplitude0 + a as u32;
                for q in 0..SINE_STEPS {
                    let index = ((q * 512 + 8) >> 4) as u16;
                    let delta = crate::motion::sine(&table, index).wrapping_mul(amp);
                    let at = ((column << 16) as u32).wrapping_add(delta);
                    xs.push(byte(screen(i64::from(at as i32 >> 16), 0))?);
                }
            }
        }
        let speeds = usize::from(flame_draws[2]);
        let mut ys = Vec::new();
        for r in 0..i64::from(column_draw) {
            for s in 0..speeds as u32 {
                let speed = i64::from((speed0 + s) as i32);
                for m in 0..steps as i64 {
                    ys.push(byte(screen(y0 + r - speed * m, 1))?);
                }
            }
        }
        let pixels = dir.read("FIREWALL.CNS.indices")?;
        let rgb = dir.read("FIREWALL.CNS.rgb")?;
        if sheet.schema != 1
            || sheet.source_sha256 != SOURCE
            || pixels.len() != sheet.width * sheet.height
            || sha(&pixels) != sheet.pixels_sha256
            || sha(&rgb) != sheet.palette_sha256
        {
            return Err("Invalid flame sheet".into());
        }
        let (pw, ph) = (panel[2], panel[3]);
        let surface = [SURFACE[0] as usize, SURFACE[1] as usize];
        let mut reduced = Vec::new();
        let mut counts: BTreeMap<u8, usize> = BTreeMap::new();
        for &(frame, _) in &shows {
            let [l, t, r, b] = sheet
                .clips
                .get(frame)
                .ok_or("Missing flame clip")?
                .map(|v| v as usize);
            let at = |u: usize, v: usize| {
                let mut n = [0usize; 256];
                for y in v * surface[1] / ph..((v + 1) * surface[1] / ph).min(b - t) {
                    for x in u * surface[0] / pw..((u + 1) * surface[0] / pw).min(r - l) {
                        n[usize::from(pixels[(t + y) * sheet.width + l + x])] += 1;
                    }
                }
                (1..256)
                    .max_by_key(|&c| (n[c], std::cmp::Reverse(c)))
                    .filter(|&c| n[c] > 0)
                    .unwrap_or(0) as u8
            };
            let (w, h) = ((r - l) * pw / surface[0] + 1, (b - t) * ph / surface[1] + 1);
            let drawn: Vec<(usize, usize, u8)> = (0..h)
                .flat_map(|v| (0..w).map(move |u| (u, v)))
                .map(|(u, v)| (u, v, at(u, v)))
                .filter(|d| d.2 != 0)
                .collect();
            for d in &drawn {
                *counts.entry(d.2).or_default() += 1;
            }
            reduced.push(drawn);
        }
        let colour = |i: u8| -> [i32; 3] {
            let c = &rgb[usize::from(i) * 3..usize::from(i) * 3 + 3];
            [c[0].into(), c[1].into(), c[2].into()]
        };
        let mut merged: BTreeMap<u8, u8> = counts.keys().map(|&c| (c, c)).collect();
        let mut merges = Vec::new();
        while counts.len() > COLOURS {
            let (&least, _) = counts.iter().min_by_key(|(c, n)| (**n, **c)).unwrap();
            let n = counts.remove(&least).unwrap();
            let [r0, g0, b0] = colour(least);
            let (&into, _) = counts
                .iter()
                .min_by_key(|(c, _)| {
                    let [r, g, b] = colour(**c);
                    ((r - r0).pow(2) + (g - g0).pow(2) + (b - b0).pow(2), **c)
                })
                .unwrap();
            *counts.get_mut(&into).unwrap() += n;
            for v in merged.values_mut().filter(|v| **v == least) {
                *v = into;
            }
            merges.push([least, into]);
        }
        let used: Vec<u8> = counts.keys().copied().collect();
        let index = |c: u8| 4 + used.iter().position(|&u| u == merged[&c]).unwrap() as u8;
        let mut tiles = BTreeMap::new();
        let mut placed = Vec::new();
        let mut cut_pixels = 0;
        let (mut cells, mut singles) = (CELLS.iter(), SINGLES.iter());
        for drawn in &reduced {
            let x0 = drawn.iter().map(|d| d.0).min().ok_or("Empty flame frame")?;
            let y0 = drawn.iter().map(|d| d.1).min().ok_or("Empty flame frame")?;
            let x1 = drawn.iter().map(|d| d.0).max().unwrap();
            let y1 = drawn.iter().map(|d| d.1).max().unwrap();
            let small = x1 - x0 < 8 && y1 - y0 < 8;
            let name = *if small {
                singles.next().ok_or("No free flame tile")?
            } else {
                cells.next().ok_or("No free flame cell")?
            };
            for &(u, v, c) in drawn {
                let (x, y) = (u - x0, v - y0);
                if x >= 16 || y >= 16 {
                    cut_pixels += 1;
                    continue;
                }
                let n = name + (x / 8) as u16 + 16 * (y / 8) as u16;
                let tile = tiles.entry(n).or_insert([0u8; 32]);
                let value = index(c);
                for plane in 0..4 {
                    tile[plane / 2 * 16 + (y % 8) * 2 + plane % 2] |=
                        ((value >> plane) & 1) << (7 - x % 8);
                }
            }
            placed.push((x0 as u8, y0 as u8, name, small));
        }
        let mut sheet_tiles = Vec::new();
        for n in CELLS[0]..=SINGLES[SINGLES.len() - 1] {
            sheet_tiles.extend(tiles.get(&n).copied().unwrap_or([0; 32]));
        }
        let colours = used
            .iter()
            .flat_map(|&i| {
                let [r, g, b] = colour(i).map(|v| (v >> 3) as u16);
                (r | g << 5 | b << 10).to_le_bytes()
            })
            .collect();
        let mut ages = Vec::new();
        for age in 0..life + wait {
            let mut frame = 0;
            if age >= wait {
                let mut left = age - wait;
                frame = shows.len() - 1;
                for (i, &(_, ticks)) in shows.iter().enumerate() {
                    if left < ticks {
                        frame = i;
                        break;
                    }
                    left -= ticks;
                }
            }
            let (dx, dy, name, small) = placed[frame];
            ages.extend([
                dx,
                dy,
                name as u8,
                (name >> 8) as u8 | PALETTE << 1 | 3 << 4,
                u8::from(!small),
            ]);
        }
        if shows.first().map(|s| s.0) != Some(init[&0x28] as usize & 0xffff) {
            return Err("Flames wait on another frame".into());
        }
        Ok(Self {
            report: serde_json::json!(
                { "body" : body, "columns" : columns, "period" : period, "wait" : wait,
                "life" : life, "column_draw" : column_draw, "flame_draws" :
                [flame_draws[0], "seed high word", flame_draws[2]], "spark_draws" :
                spark_draws, "colour_merges" : merges, "cut_pixels" : cut_pixels,
                "limits" :
                ["Every RNG draw of the body, its sparks and its flames is taken in the source's object order (new objects run in their creation tick); sparks are not drawn.",
                "Every third flame of every other column shows (24 sprites), each frame reduced by majority colour; a flame's start angle keeps 1/128 of a turn."]
                }
            ),
            body,
            body_id: 0,
            column_draw,
            columns,
            spark_draws,
            flame_draws,
            period,
            life: life + wait,
            wait,
            xs,
            ys,
            steps,
            ages,
            tiles: sheet_tiles,
            colours,
        })
    }
    pub fn append(
        &self,
        sections: &mut Vec<Section>,
        space: &mut crate::sfc_rom::Space,
    ) -> Result<()> {
        use super::sfc_b1::{add, transfer};
        let tiles = add(sections, space, "flame_tiles".into(), self.tiles.clone())?;
        let colours = add(
            sections,
            space,
            "flame_colours".into(),
            self.colours.clone(),
        )?;
        let vram = 0xe000 + (CELLS[0] - 256) * 32;
        let mut list = transfer(tiles, vram, self.tiles.len(), 0);
        list.extend(transfer(colours, FIRST_COLOUR, self.colours.len(), 1));
        let list = add(sections, space, "flame_list".into(), list)?;
        let mut job = list.to_le_bytes()[..3].to_vec();
        job.extend(2u16.to_le_bytes());
        add(sections, space, "flame_job".into(), job)?;
        add(sections, space, "flame_xs".into(), self.xs.clone())?;
        add(sections, space, "flame_ys".into(), self.ys.clone())?;
        add(sections, space, "flame_ages".into(), self.ages.clone())?;
        let mut slots = Vec::new();
        for k in 0..SPRITES {
            let (c, j) = (k / PER_COLUMN, k % PER_COLUMN);
            slots.extend((((c * SLOTS + c % EVERY + EVERY * j) * REC) as u16).to_le_bytes());
        }
        add(sections, space, "flame_slots".into(), slots)?;
        if self.body_id == 0 {
            return Err("Fire barrier body id not set".into());
        }
        let mut shape = vec![
            self.body_id,
            u8::try_from(self.columns)?,
            self.column_draw,
            u8::try_from(self.period)?,
            u8::try_from(self.life)?,
            u8::try_from(self.wait)?,
            u8::try_from(self.steps)?,
            self.flame_draws[0],
            self.flame_draws[2],
        ];
        shape.extend(&self.spark_draws);
        add(sections, space, "flame_shape".into(), shape)?;
        Ok(())
    }
}
struct Shape {
    body_id: u8,
    columns: usize,
    column_draw: u8,
    period: usize,
    life: usize,
    wait: usize,
    steps: usize,
    flame_draws: [u8; 3],
    spark_draws: Vec<u8>,
}
pub fn emit(p: &mut w65c816::Assembler, layout: &CartridgeLayout) -> Result<()> {
    use crate::sfc::{emit, implied, write};
    use crate::sfc_b1_menu::OAM_BUFFER;
    use crate::sfc_field_events::{LIVE, LIVE_STATE};
    use w65c816::{AddressingMode as A, CpuMode, Mnemonic as M, Operand as O};
    let raw = &layout.section("flame_shape")?.bytes;
    let shape = Shape {
        body_id: raw[0],
        columns: raw[1].into(),
        column_draw: raw[2],
        period: raw[3].into(),
        life: raw[4].into(),
        wait: raw[5].into(),
        steps: raw[6].into(),
        flame_draws: [raw[7], 0, raw[8]],
        spark_draws: raw[9..].to_vec(),
    };
    let abs = |p: &mut w65c816::Assembler, m, at: u16| emit(p, m, A::Absolute, O::Word(at));
    let long_x = |p: &mut w65c816::Assembler, m, at: u32| emit(p, m, A::AbsoluteLongX, O::Long(at));
    let abs_x = |p: &mut w65c816::Assembler, m, at: u16| emit(p, m, A::AbsoluteX, O::Word(at));
    let abs_y = |p: &mut w65c816::Assembler, m, at: u16| emit(p, m, A::AbsoluteY, O::Word(at));
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
    let address = |name: &str| -> Result<u32> { layout.cpu_address(layout.section(name)?.offset) };
    let (job, xs, ys, ages, slots, oam_job) = (
        address("flame_job")?,
        address("flame_xs")?,
        address("flame_ys")?,
        address("flame_ages")?,
        address("flame_slots")?,
        address("menu_oam_job")?,
    );
    let enqueue = |p: &mut w65c816::Assembler, record: u32, slot: u16| {
        m16(p);
        imm16(p, M::Lda, record as u16);
        abs(p, M::Sta, 0xe0);
        m8(p);
        write(p, 0xe2, (record >> 16) as u8);
        imm16(p, M::Ldx, slot);
        call(p, "b1_enqueue");
    };
    let rng = |p: &mut w65c816::Assembler, divisor: u8| {
        imm(p, M::Lda, divisor);
        call(p, "encounter_rng");
    };
    let oam = OAM_BUFFER + FIRST_ENTRY as u16 * 4;
    let amplitudes = u16::from(shape.flame_draws[0]);
    let speeds = u16::from(shape.flame_draws[2]);
    p.label("field_flames_tick")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, LIVE_STATE);
    abs(p, M::Cmp, 0x500);
    branch(p, M::Bne, "field_flames_gone");
    for i in 0..2 {
        abs(p, M::Lda, LIVE + i);
        imm(p, M::Cmp, shape.body_id);
        branch(p, M::Beq, "field_flames_live");
    }
    p.label("field_flames_gone");
    m16(p);
    abs(p, M::Lda, FLAME_TIME);
    m8(p);
    branch(p, M::Bne, "field_flames_stop");
    implied(p, M::Rts);
    p.label("field_flames_stop");
    abs(p, M::Stz, FLAME_TIME);
    abs(p, M::Stz, FLAME_TIME + 1);
    imm16(p, M::Ldy, 0);
    p.label("field_flames_hide_all");
    imm(p, M::Lda, 224);
    abs_y(p, M::Sta, oam + 1);
    for _ in 0..4 {
        implied(p, M::Iny);
    }
    imm16(p, M::Cpy, 4 * SPRITES as u16);
    branch(p, M::Bne, "field_flames_hide_all");
    enqueue(p, oam_job, crate::sfc_b1::MENU_OAM_SLOT);
    implied(p, M::Rts);
    p.label("field_flames_live");
    m16(p);
    abs(p, M::Inc, FLAME_TIME);
    abs(p, M::Lda, FLAME_TIME);
    imm16(p, M::Cmp, 1);
    m8(p);
    branch(p, M::Beq, "field_flames_first");
    jump(p, "field_flames_sparks");
    p.label("field_flames_first");
    imm16(p, M::Ldx, 0);
    p.label("field_flames_clear");
    abs_x(p, M::Stz, FLAME_RECORDS);
    implied(p, M::Inx);
    imm16(p, M::Cpx, (DRAWN_COLUMNS * SLOTS * REC) as u16);
    branch(p, M::Bne, "field_flames_clear");
    enqueue(p, job, crate::sfc_b1::FADE_SLOT);
    abs(p, M::Stz, 0xe4);
    abs(p, M::Stz, 0xe5);
    p.label("field_flames_heights");
    rng(p, shape.column_draw);
    abs(p, M::Ldx, 0xe4);
    abs_x(p, M::Sta, ALL_HEIGHTS);
    abs(p, M::Inc, 0xe4);
    abs(p, M::Lda, 0xe4);
    imm(p, M::Cmp, shape.columns as u8);
    branch(p, M::Bne, "field_flames_heights");
    for c in 0..DRAWN_COLUMNS as u16 {
        abs(p, M::Lda, ALL_HEIGHTS + 2 * c);
        abs(p, M::Sta, FLAME_HEIGHTS + c);
    }
    p.label("field_flames_sparks");
    let sparks = shape.spark_draws.len();
    if sparks + 3 * shape.columns > crate::sfc_encounter::SKIP_MOST {
        return Err("Flame draws owed exceed the RNG skip table".into());
    }
    imm(p, M::Lda, sparks as u8);
    abs(p, M::Sta, OWED);
    m16(p);
    abs(p, M::Lda, FLAME_TIME);
    abs(p, M::Sta, 0x4204);
    m8(p);
    imm(p, M::Lda, shape.period as u8);
    abs(p, M::Sta, 0x4206);
    for _ in 0..8 {
        implied(p, M::Nop);
    }
    m16(p);
    abs(p, M::Lda, 0x4216);
    branch(p, M::Beq, "field_flames_born");
    m8(p);
    call(p, "field_flames_pay");
    jump(p, "field_flames_place");
    p.label("field_flames_pay")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, OWED);
    branch(p, M::Beq, "field_flames_paid");
    call(p, "encounter_rng_skip");
    abs(p, M::Stz, OWED);
    p.label("field_flames_paid");
    implied(p, M::Rts);
    p.label("field_flames_born")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    abs(p, M::Lda, 0x4214);
    emit(p, M::Dec, A::Accumulator, O::None);
    abs(p, M::Sta, 0x4204);
    m8(p);
    imm(p, M::Lda, SLOTS as u8);
    abs(p, M::Sta, 0x4206);
    for _ in 0..8 {
        implied(p, M::Nop);
    }
    m16(p);
    abs(p, M::Lda, 0x4216);
    abs(p, M::Sta, 0xe6);
    const _: () = assert!(REC == 5);
    emit(p, M::Asl, A::Accumulator, O::None);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Clc);
    abs(p, M::Adc, 0xe6);
    implied(p, M::Pha);
    m8(p);
    abs(p, M::Lda, 0xe6);
    p.label("field_flames_third");
    imm(p, M::Cmp, EVERY as u8);
    branch(p, M::Bcc, "field_flames_third_done");
    imm(p, M::Sbc, EVERY as u8);
    branch(p, M::Bra, "field_flames_third");
    p.label("field_flames_third_done");
    abs(p, M::Sta, PHASE);
    m16(p);
    implied(p, M::Pla);
    abs(p, M::Sta, 0xe6);
    m8(p);
    abs(p, M::Stz, 0xe4);
    p.label("field_flames_column");
    abs(p, M::Lda, 0xe4);
    emit(p, M::Lsr, A::Accumulator, O::None);
    branch(p, M::Bcs, "field_flames_owed");
    imm(p, M::Cmp, DRAWN_COLUMNS as u8);
    branch(p, M::Bcs, "field_flames_owed");
    p.label("field_flames_column_third");
    imm(p, M::Cmp, EVERY as u8);
    branch(p, M::Bcc, "field_flames_column_third_done");
    imm(p, M::Sbc, EVERY as u8);
    branch(p, M::Bra, "field_flames_column_third");
    p.label("field_flames_column_third_done");
    abs(p, M::Cmp, PHASE);
    branch(p, M::Beq, "field_flames_shown_flame");
    p.label("field_flames_owed");
    abs(p, M::Lda, OWED);
    implied(p, M::Clc);
    imm(p, M::Adc, 3);
    abs(p, M::Sta, OWED);
    jump(p, "field_flames_kept");
    p.label("field_flames_shown_flame");
    call(p, "field_flames_pay");
    rng(p, shape.flame_draws[0]);
    emit(p, M::Inc, A::Accumulator, O::None);
    abs(p, M::Sta, 0xe9);
    rng(p, 0);
    m16(p);
    abs(p, M::Lda, crate::sfc_encounter::SEED + 2);
    imm16(p, M::Cmp, 0xffff);
    branch(p, M::Bne, "field_flames_angle");
    imm16(p, M::Lda, 0);
    p.label("field_flames_angle");
    implied(p, M::Clc);
    imm16(p, M::Adc, 8);
    for _ in 0..9 {
        emit(p, M::Lsr, A::Accumulator, O::None);
    }
    m8(p);
    abs(p, M::Sta, 0xea);
    rng(p, shape.flame_draws[2]);
    abs(p, M::Sta, 0xeb);
    m16(p);
    abs(p, M::Lda, 0xe4);
    imm16(p, M::And, 0xff);
    emit(p, M::Lsr, A::Accumulator, O::None);
    abs(p, M::Sta, X_INDEX);
    mul(p, (SLOTS * REC) as u16);
    implied(p, M::Clc);
    abs(p, M::Adc, 0xe6);
    implied(p, M::Tax);
    abs(p, M::Lda, X_INDEX);
    mul(p, amplitudes);
    abs(p, M::Sta, Y_INDEX);
    abs(p, M::Lda, 0xe9);
    imm16(p, M::And, 0xff);
    implied(p, M::Clc);
    abs(p, M::Adc, Y_INDEX);
    emit(p, M::Dec, A::Accumulator, O::None);
    for _ in 0..SINE_STEPS.trailing_zeros() {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    abs(p, M::Sta, Y_INDEX);
    abs(p, M::Lda, 0xea);
    imm16(p, M::And, 0xff);
    abs(p, M::Ora, Y_INDEX);
    abs_x(p, M::Sta, FLAME_RECORDS);
    abs(p, M::Ldy, X_INDEX);
    emit(p, M::Lda, A::AbsoluteY, O::Word(FLAME_HEIGHTS));
    imm16(p, M::And, 0xff);
    mul(p, speeds);
    abs(p, M::Sta, Y_INDEX);
    abs(p, M::Lda, 0xeb);
    imm16(p, M::And, 0xff);
    implied(p, M::Clc);
    abs(p, M::Adc, Y_INDEX);
    mul(p, shape.steps as u16);
    imm16(p, M::Ora, 0x8000);
    abs_x(p, M::Sta, FLAME_RECORDS + 2);
    m8(p);
    abs(p, M::Lda, FLAME_TIME);
    abs_x(p, M::Sta, FLAME_RECORDS + 4);
    p.label("field_flames_kept");
    abs(p, M::Inc, 0xe4);
    abs(p, M::Lda, 0xe4);
    imm(p, M::Cmp, shape.columns as u8);
    branch(p, M::Beq, "field_flames_columns_done");
    jump(p, "field_flames_column");
    p.label("field_flames_columns_done");
    call(p, "field_flames_pay");
    p.label("field_flames_place");
    {
        let slot = crate::sfc_b1::QUEUE + crate::sfc_b1::FADE_SLOT;
        abs(p, M::Lda, slot + 3);
        abs(p, M::Ora, slot + 4);
        branch(p, M::Beq, "field_flames_ready");
        implied(p, M::Rts);
        p.label("field_flames_ready");
    }
    abs(p, M::Stz, 0xe4);
    abs(p, M::Stz, 0xe5);
    p.label("field_flames_sprite");
    m16(p);
    abs(p, M::Lda, 0xe4);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Tax);
    long_x(p, M::Lda, slots);
    abs(p, M::Sta, 0xe6);
    implied(p, M::Txa);
    emit(p, M::Asl, A::Accumulator, O::None);
    abs(p, M::Sta, 0xe8);
    m8(p);
    abs(p, M::Ldx, 0xe6);
    abs(p, M::Ldy, 0xe8);
    abs_x(p, M::Lda, FLAME_RECORDS + 3);
    branch(p, M::Bpl, "field_flames_hide");
    abs(p, M::Lda, FLAME_TIME);
    implied(p, M::Sec);
    abs_x(p, M::Sbc, FLAME_RECORDS + 4);
    imm(p, M::Cmp, shape.life as u8);
    branch(p, M::Bcc, "field_flames_shown");
    p.label("field_flames_hide");
    imm(p, M::Lda, 224);
    abs_y(p, M::Sta, oam + 1);
    abs(p, M::Ldx, 0xe4);
    abs_x(p, M::Stz, FLAME_SIZES);
    jump(p, "field_flames_next");
    p.label("field_flames_shown");
    abs(p, M::Sta, 0xea);
    implied(p, M::Sec);
    imm(p, M::Sbc, shape.wait as u8 - 1);
    branch(p, M::Bcs, "field_flames_moved");
    imm(p, M::Lda, 0);
    p.label("field_flames_moved");
    abs(p, M::Sta, MOVED);
    abs(p, M::Stz, MOVED + 1);
    m16(p);
    abs_x(p, M::Lda, FLAME_RECORDS);
    implied(p, M::Clc);
    abs(p, M::Adc, MOVED);
    imm16(p, M::And, (SINE_STEPS - 1) as u16);
    abs(p, M::Sta, X_INDEX);
    abs_x(p, M::Lda, FLAME_RECORDS);
    imm16(p, M::And, !((SINE_STEPS - 1) as u16));
    abs(p, M::Ora, X_INDEX);
    abs(p, M::Sta, X_INDEX);
    abs_x(p, M::Lda, FLAME_RECORDS + 2);
    imm16(p, M::And, 0x7fff);
    implied(p, M::Clc);
    abs(p, M::Adc, MOVED);
    abs(p, M::Sta, Y_INDEX);
    abs(p, M::Lda, 0xea);
    imm16(p, M::And, 0xff);
    abs(p, M::Sta, 0xe6);
    emit(p, M::Asl, A::Accumulator, O::None);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Clc);
    abs(p, M::Adc, 0xe6);
    implied(p, M::Tax);
    m8(p);
    abs(p, M::Ldy, 0xe8);
    long_x(p, M::Lda, ages + 2);
    abs_y(p, M::Sta, oam + 2);
    long_x(p, M::Lda, ages + 3);
    abs_y(p, M::Sta, oam + 3);
    long_x(p, M::Lda, ages);
    abs(p, M::Sta, 0xe6);
    long_x(p, M::Lda, ages + 1);
    abs(p, M::Sta, 0xe7);
    long_x(p, M::Lda, ages + 4);
    abs(p, M::Ldx, 0xe4);
    abs_x(p, M::Sta, FLAME_SIZES);
    abs(p, M::Ldx, X_INDEX);
    long_x(p, M::Lda, xs);
    implied(p, M::Clc);
    abs(p, M::Adc, 0xe6);
    abs_y(p, M::Sta, oam);
    abs(p, M::Ldx, Y_INDEX);
    long_x(p, M::Lda, ys);
    implied(p, M::Clc);
    abs(p, M::Adc, 0xe7);
    abs_y(p, M::Sta, oam + 1);
    p.label("field_flames_next");
    abs(p, M::Inc, 0xe4);
    abs(p, M::Lda, 0xe4);
    imm(p, M::Cmp, SPRITES as u8);
    branch(p, M::Beq, "field_flames_high");
    jump(p, "field_flames_sprite");
    p.label("field_flames_high");
    imm16(p, M::Ldx, 0);
    imm16(p, M::Ldy, 0);
    p.label("field_flames_high_byte");
    abs(p, M::Stz, 0xe6);
    for i in 0..4u16 {
        let skip = format!("field_flames_small_{i}");
        abs_x(p, M::Lda, FLAME_SIZES + i);
        branch(p, M::Beq, &skip);
        abs(p, M::Lda, 0xe6);
        imm(p, M::Ora, 2 << (2 * i));
        abs(p, M::Sta, 0xe6);
        p.label(&skip);
    }
    abs(p, M::Lda, 0xe6);
    abs_y(p, M::Sta, OAM_BUFFER + 512 + FIRST_ENTRY as u16 / 4);
    for _ in 0..4 {
        implied(p, M::Inx);
    }
    implied(p, M::Iny);
    imm16(p, M::Cpy, (SPRITES / 4) as u16);
    branch(p, M::Bne, "field_flames_high_byte");
    enqueue(p, oam_job, crate::sfc_b1::MENU_OAM_SLOT);
    implied(p, M::Rts);
    Ok(())
}
fn mul(p: &mut w65c816::Assembler, factor: u16) {
    use crate::sfc::{emit, implied};
    use w65c816::{AddressingMode as A, Mnemonic as M, Operand as O};
    emit(p, M::Sta, A::Absolute, O::Word(0xec));
    emit(p, M::Lda, A::Immediate, O::Word(0));
    for bit in (0..16 - factor.leading_zeros()).rev() {
        emit(p, M::Asl, A::Accumulator, O::None);
        if factor & (1 << bit) != 0 {
            implied(p, M::Clc);
            emit(p, M::Adc, A::Absolute, O::Word(0xec));
        }
    }
}
fn run_len(nodes: &BTreeMap<u32, Vec<u8>>, mut at: u32) -> Result<usize> {
    let mut n = 0;
    loop {
        let raw = nodes.get(&at).ok_or("Spark command missing")?;
        n += 1;
        if raw[0] == 0x00 {
            return Ok(n);
        }
        at += raw.len() as u32;
    }
}
