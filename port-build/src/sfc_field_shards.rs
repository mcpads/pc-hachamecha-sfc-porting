use crate::{Result, SOURCE, sha};
use serde::Deserialize;
use sfc_builder::cartridge::{CartridgeLayout, Section};
use std::collections::BTreeMap;
pub const SHARD_TIME: u16 = 0x7e2;
const SHARD_COLOUR: u16 = 0x7e3;
pub const SHARD_DRAWS: u16 = 0x1a00;
const SHARD_OFFSETS: u16 = 0x1a40;
pub const SHARDS: usize = 32;
const FIRST_ENTRY: usize = 92;
const PALETTE: u8 = 3;
const FIRST_COLOUR: u16 = 128 + 16 * PALETTE as u16 + 4;
const COLOURS: usize = 5;
const FRAMES_PER_COLOUR: usize = 3;
const SURFACE: [i64; 2] = [224, 240];
#[derive(Deserialize)]
struct Sheet {
    schema: u8,
    source_sha256: String,
    width: usize,
    height: usize,
    clips: Vec<[usize; 4]>,
    pixels_sha256: String,
    palette_sha256: String,
}
struct Kind {
    start: [u16; 2],
    step: [i32; 2],
    sign: i64,
}
pub struct Assets {
    pub root: u32,
    shards: Vec<(u8, u16, u16)>,
    xs: Vec<u8>,
    ys: Vec<u8>,
    frames: Vec<u8>,
    tiles: Vec<u8>,
    colours: Vec<Vec<u8>>,
    pub report: serde_json::Value,
}
fn hex(text: &str) -> Result<Vec<u8>> {
    (0..text.len())
        .step_by(2)
        .map(|i| Ok(u8::from_str_radix(&text[i..i + 2], 16)?))
        .collect()
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
        let raw = nodes.get(&at).ok_or("Shard effect command missing")?;
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
        Err(format!("Unexpected shard effect command {raw:02x?}").into())
    }
}
impl Assets {
    pub fn load(content: &crate::content::Content, panel: [usize; 4]) -> Result<Self> {
        let trial: serde_json::Value = content.json("puyo-trial.json")?;
        if trial["source_sha256"] != SOURCE {
            return Err("Invalid puyo trial".into());
        }
        let playback = &trial["door_playback"];
        let mut nodes = BTreeMap::new();
        for list in [
            &playback["graph"]["nodes"],
            &playback["animations"]["nodes"],
        ] {
            for node in list.as_array().ok_or("Missing door playback nodes")? {
                let va = node["va"].as_u64().ok_or("Missing node address")? as u32;
                nodes.insert(
                    va,
                    hex(node["raw_hex"].as_str().ok_or("Missing node bytes")?)?,
                );
            }
        }
        let mut spawned: BTreeMap<u32, usize> = BTreeMap::new();
        for raw in nodes.values().filter(|r| r[0] == 0x07) {
            *spawned.entry(long(raw, 4)).or_default() += 1;
        }
        let root = *spawned
            .iter()
            .find(|(_, n)| **n == 5)
            .ok_or("No effect shared by the five doors")?
            .0;
        let run = walk(&nodes, root, 6)?;
        expect(run[0].1, &[0x3f])?;
        expect(run[1].1, &[0x24])?;
        let emitters: Vec<u32> = run[2..6]
            .iter()
            .map(|(_, raw)| expect(raw, &[0x07]).map(|_| long(raw, 4)))
            .collect::<Result<_>>()?;
        let mut places = Vec::new();
        let mut spawner = None;
        for &e in &emitters {
            let run = walk(&nodes, e, 3)?;
            expect(run[0].1, &[0x12, 0x80, 0x80])?;
            expect(run[1].1, &[0x12, 0x80, 0x84])?;
            places.push([long(run[0].1, 4) >> 16, long(run[1].1, 4) >> 16]);
            if *spawner.get_or_insert(run[2].0) != run[2].0 {
                return Err("Emitters loop apart".into());
            }
        }
        let spawner = spawner.ok_or("No emitters")?;
        let run = walk(&nodes, spawner, 11)?;
        let mut kinds_at = Vec::new();
        for (i, (_, raw)) in run.iter().enumerate() {
            let want: &[u8] = match i {
                0 | 5 => &[0x07],
                1 | 6 => &[0x1f],
                2 | 7 => &[0x12, 0xa0, 0xa8, 0xa4],
                3 | 8 => &[0x12, 0xe0, 0x80, 0x80],
                4 | 9 => &[0x12, 0xe0, 0x84, 0x84],
                _ => &[0x01],
            };
            expect(raw, want)?;
            if want == [0x07] {
                kinds_at.push(long(raw, 4));
            }
        }
        let (_, repeat) = walk(&nodes, run[10].0 + 4, 1)?[0];
        expect(repeat, &[0x06, 0x00])?;
        if long(repeat, 4) != spawner {
            return Err("Emitter loop does not repeat its spawns".into());
        }
        let ticks = usize::from(word(repeat, 2));
        let mut kinds = Vec::new();
        let mut amplitude = None;
        let mut animations: BTreeMap<u8, u32> = BTreeMap::new();
        let mut draw_range = None;
        let mut life = None;
        for &k in &kinds_at {
            let init = nodes.get(&k).ok_or("Missing shard")?;
            expect(init, &[0x08])?;
            let mut fields = BTreeMap::new();
            let mut at = 4;
            while init[at] != 0xff {
                match init[at] {
                    2 => {
                        fields.insert(init[at + 1], u32::from(word(init, at + 2)));
                        at += 4;
                    }
                    3 => {
                        fields.insert(init[at + 1], long(init, at + 4));
                        at += 8;
                    }
                    _ => return Err("Unknown shard field kind".into()),
                }
            }
            if fields.get(&0).is_none_or(|f| f & 0x4000_0000 == 0) {
                return Err("Shards no longer draw in screen space".into());
            }
            let a = [fields[&0x92], fields[&0x94]];
            if *amplitude.get_or_insert(a) != a {
                return Err("Shard kinds differ in amplitude".into());
            }
            let mut at = k + init.len() as u32;
            expect(&nodes[&at], &[0x4b])?;
            at += 4;
            let mut common = None;
            for _ in 0..COLOURS {
                let test = &nodes[&at];
                expect(test, &[0x13, 0xc1, 0x19])?;
                let pick = walk(&nodes, long(test, 4), 1)?[0].1;
                expect(pick, &[0x20])?;
                let animation = long(pick, 4);
                if *animations.entry(test[3]).or_insert(animation) != animation {
                    return Err("Shard kinds pick different animations".into());
                }
                let next = long(test, 4) + 8;
                let after = long(&nodes[&next], 4);
                if *common.get_or_insert(after) != after {
                    return Err("Shard colours continue apart".into());
                }
                at += 8;
            }
            let run = walk(&nodes, common.ok_or("No shard colours")?, 15)?;
            let pattern: [&[u8]; 15] = [
                &[0x2b, 0x00],
                &[0x11, 0xa1, 0x92, 0x58],
                &[0x2b, 0x00],
                &[0x11, 0xa1, 0x94, 0x58],
                &[0x11, 0x80, 0x8c],
                &[0x11, 0x80, 0x8e],
                &[0x11, 0xa0, 0x58, 0x92],
                &[0x12, 0x83, 0x58, 0x00, 0x00, 0x00, 0x01, 0x00],
                &[0x12],
                &[0x11],
                &[0x11],
                &[0x2d, 0x0b],
                &[0x01],
                &[0x06, 0x00],
                &[0x00],
            ];
            for ((_, raw), want) in run.iter().zip(pattern) {
                expect(raw, want)?;
            }
            for raw in [run[0].1, run[2].1] {
                if *draw_range.get_or_insert(raw[2]) != raw[2] {
                    return Err("Shard draws differ in range".into());
                }
            }
            let sign = match run[8].1[..4] {
                [0x12, 0xa1, 0x80, 0x58] => 1,
                [0x12, 0xa2, 0x80, 0x58] => -1,
                _ => return Err("Unexpected shard centre".into()),
            };
            let step = |raw: &[u8], field: u8| -> Result<i32> {
                match (raw[1], raw[2]) {
                    (0x81, f) if f == field => Ok(i32::from(word(raw, 4))),
                    (0x82, f) if f == field => Ok(-i32::from(word(raw, 4))),
                    _ => Err("Unexpected shard angle step".into()),
                }
            };
            if long(run[13].1, 4) != run[9].0 {
                return Err("Shard loop does not repeat its motion".into());
            }
            let l = usize::from(word(run[13].1, 2));
            if *life.get_or_insert(l) != l {
                return Err("Shard kinds differ in life".into());
            }
            kinds.push(Kind {
                start: [word(run[4].1, 4), word(run[5].1, 4)],
                step: [step(run[9].1, 0x8c)?, step(run[10].1, 0x8e)?],
                sign,
            });
        }
        let amplitude = amplitude.ok_or("No shard kinds")?;
        let draw_range = draw_range.ok_or("No shard draws")?;
        if draw_range != crate::sfc_field_events::SHARD_DRAW_RANGE {
            return Err("Shard draw range differs from the field program's".into());
        }
        let life = life.ok_or("No shard life")?;
        if kinds.len() != 2 || animations.len() != COLOURS || ticks * 8 != SHARDS {
            return Err("Unexpected shard effect shape".into());
        }
        let mut sequences = Vec::new();
        for (&selection, &animation) in &animations {
            let run = walk(&nodes, animation, 4)?;
            let mut shows = Vec::new();
            for (_, raw) in &run[..3] {
                expect(raw, &[0x21, 0x00])?;
                shows.push((usize::from(word(raw, 4)), usize::from(word(raw, 2))));
            }
            expect(run[3].1, &[0x01])?;
            if usize::from(selection) != sequences.len() + 1 {
                return Err("Shard colours are not 1..5".into());
            }
            sequences.push(shows);
        }
        let image = content.read("battle-scripts.bin")?;
        let manifest: serde_json::Value = content.json("battle-scripts.json")?;
        if manifest["image_sha256"] != sha(&image) {
            return Err("Battle script image differs from its manifest".into());
        }
        let base = (0x45b3c8 - crate::sfc_battle_objects::SOURCE_BASE) as usize;
        let table: Vec<u16> = (0..1024).map(|i| word(&image, base + 2 * i)).collect();
        let objects: serde_json::Value = content.json("floor-objects/floor-objects.json")?;
        let origin: [i64; 2] = serde_json::from_value(objects["screen_origin"].clone())?;
        let [px, py, pw, ph] = panel.map(|v| v as i64);
        let screen = |v: i64, axis: usize| -> Result<u8> {
            let (p, size) = [(px, pw), (py, ph)][axis];
            let v = v - origin[axis];
            Ok(u8::try_from(p + (v * size).div_euclid(SURFACE[axis]))?)
        };
        let mut xs = Vec::new();
        let mut ys = Vec::new();
        let xs_of: Vec<u32> = {
            let mut v: Vec<u32> = places.iter().map(|p| p[0]).collect();
            v.sort();
            v.dedup();
            v
        };
        let ys_of: Vec<u32> = {
            let mut v: Vec<u32> = places.iter().map(|p| p[1]).collect();
            v.sort();
            v.dedup();
            v
        };
        for axis in 0..2 {
            let of = [&xs_of, &ys_of][axis];
            for &origin in of {
                for kind in &kinds {
                    for r in 0..u32::from(draw_range) {
                        let amp = (amplitude[axis] + r) as u16 as i16 as i32 as u32;
                        let centre = if axis == 0 {
                            ((i64::from(origin) + kind.sign * i64::from(amp)) << 16) as u32
                        } else {
                            origin << 16
                        };
                        let mut angle = kind.start[axis];
                        for _ in 0..life {
                            angle = angle.wrapping_add(kind.step[axis] as u16);
                            let index = angle.wrapping_add(8) >> 4;
                            let value = crate::motion::sine(
                                &table,
                                if axis == 0 {
                                    index
                                } else {
                                    (index + 1024) & 4095
                                },
                            );
                            let delta = value.wrapping_mul(amp);
                            let at = if axis == 0 {
                                centre.wrapping_add(delta)
                            } else {
                                centre.wrapping_sub(delta)
                            };
                            let v = i64::from(at as i32 >> 16);
                            [&mut xs, &mut ys][axis].push(screen(v, axis)?);
                        }
                    }
                }
            }
        }
        let shard_index = |e: usize, kind: usize, axis: usize| -> Result<u16> {
            let of = [&xs_of, &ys_of][axis];
            let o = of
                .iter()
                .position(|&v| v == places[e][axis])
                .ok_or("Emitter")?;
            Ok(u16::try_from(
                (o * kinds.len() + kind) * usize::from(draw_range) * life,
            )?)
        };
        let mut shards = Vec::new();
        for tick in 0..ticks {
            for e in 0..emitters.len() {
                for kind in 0..2 {
                    shards.push((
                        tick as u8,
                        shard_index(e, kind, 0)?,
                        shard_index(e, kind, 1)?,
                    ));
                }
            }
        }
        let root_dir = content.sub("b4-shards");
        let sheet: Sheet = root_dir.json("shards.json")?;
        let pixels = root_dir.read("P_KAKERA.CNS.indices")?;
        let rgb = root_dir.read("P_KAKERA.CNS.rgb")?;
        if sheet.schema != 1
            || sheet.source_sha256 != SOURCE
            || pixels.len() != sheet.width * sheet.height
            || sha(&pixels) != sheet.pixels_sha256
            || sha(&rgb) != sheet.palette_sha256
            || sheet.clips.len() != COLOURS * FRAMES_PER_COLOUR
        {
            return Err("Invalid shard sheet".into());
        }
        let (pw, ph) = (panel[2], panel[3]);
        let surface = [SURFACE[0] as usize, SURFACE[1] as usize];
        let mut tiles = Vec::new();
        let mut colours = Vec::new();
        let mut placed = Vec::new();
        let mut cut_pixels = 0;
        for c in 0..COLOURS {
            let mut used: Vec<u8> = Vec::new();
            for f in 0..FRAMES_PER_COLOUR {
                let [l, t, r, b] = sheet.clips[c * FRAMES_PER_COLOUR + f];
                let at = |u: usize, v: usize| {
                    let mut counts = [0usize; 256];
                    for y in v * surface[1] / ph..((v + 1) * surface[1] / ph).min(b - t) {
                        for x in u * surface[0] / pw..((u + 1) * surface[0] / pw).min(r - l) {
                            counts[usize::from(pixels[(t + y) * sheet.width + l + x])] += 1;
                        }
                    }
                    (1..256)
                        .max_by_key(|&c| (counts[c], std::cmp::Reverse(c)))
                        .filter(|&c| counts[c] > 0)
                        .unwrap_or(0) as u8
                };
                let (w, h) = ((r - l) * pw / surface[0] + 1, (b - t) * ph / surface[1] + 1);
                let drawn: Vec<(usize, usize)> = (0..h)
                    .flat_map(|v| (0..w).map(move |u| (u, v)))
                    .filter(|&(u, v)| at(u, v) != 0)
                    .collect();
                let x0 = drawn.iter().map(|d| d.0).min().ok_or("Empty shard frame")?;
                let y0 = drawn.iter().map(|d| d.1).min().ok_or("Empty shard frame")?;
                let (drawn, cut): (Vec<_>, Vec<_>) = drawn
                    .into_iter()
                    .partition(|d| d.0 < x0 + 8 && d.1 < y0 + 8);
                cut_pixels += cut.len();
                let mut bytes = [0u8; 32];
                for &(u, v) in &drawn {
                    let value = at(u, v);
                    let i = used.iter().position(|&k| k == value).unwrap_or_else(|| {
                        used.push(value);
                        used.len() - 1
                    });
                    let colour = 4 + i as u8;
                    let (x, y) = (u - x0, v - y0);
                    for plane in 0..4 {
                        bytes[plane / 2 * 16 + y * 2 + plane % 2] |=
                            ((colour >> plane) & 1) << (7 - x);
                    }
                }
                tiles.extend(bytes);
                placed.push((x0 as u8, y0 as u8));
            }
            if 4 + used.len() > 16 {
                return Err("Shard colour exceeds its borrowed colours".into());
            }
            colours.push(
                used.iter()
                    .flat_map(|&i| {
                        let c = &rgb[usize::from(i) * 3..usize::from(i) * 3 + 3];
                        (u16::from(c[0] >> 3)
                            | u16::from(c[1] >> 3) << 5
                            | u16::from(c[2] >> 3) << 10)
                            .to_le_bytes()
                    })
                    .collect(),
            );
        }
        let mut frames = Vec::new();
        for shows in &sequences {
            for age in 0..life {
                let mut left = age;
                let mut frame = shows[shows.len() - 1].0;
                for &(f, ticks) in shows {
                    if left < ticks {
                        frame = f;
                        break;
                    }
                    left -= ticks;
                }
                let name = crate::sfc_b1_menu::ICON_NAME + frame as u16;
                let (dx, dy) = placed[frame];
                frames.extend([
                    dx,
                    dy,
                    name as u8,
                    (name >> 8) as u8 | PALETTE << 1 | 3 << 4,
                ]);
            }
        }
        Ok(Self {
            report: serde_json::json!(
                { "root" : root, "shards" : SHARDS, "life" : life, "draw_range" :
                draw_range, "emitters" : places, "amplitude" : amplitude, "cut_pixels" :
                cut_pixels, "limits" :
                ["Shard frames are reduced by majority colour per block into one 8x8 sprite; the 16x16 frames lose their ninth row.",
                "RNG draws are taken at the start in spawn order (tick, emitter, kind; x then y)."]
                }
            ),
            root,
            shards,
            xs,
            ys,
            frames,
            tiles,
            colours,
        })
    }
    pub fn append(
        &self,
        sections: &mut Vec<Section>,
        space: &mut crate::sfc_rom::Space,
    ) -> Result<()> {
        use super::sfc_b1::{add, transfer};
        let tiles = add(sections, space, "shard_tiles".into(), self.tiles.clone())?;
        let mut records = Vec::new();
        for (c, colours) in self.colours.iter().enumerate() {
            let source = add(
                sections,
                space,
                format!("shard_colours_{c}"),
                colours.clone(),
            )?;
            let mut list = transfer(tiles, crate::sfc_b1_menu::ICON_VRAM, self.tiles.len(), 0);
            list.extend(transfer(source, FIRST_COLOUR, colours.len(), 1));
            let list = add(sections, space, format!("shard_list_{c}"), list)?;
            records.extend(&list.to_le_bytes()[..3]);
            records.extend(2u16.to_le_bytes());
        }
        add(sections, space, "shard_jobs".into(), records)?;
        let mut shards = Vec::new();
        for &(born, x, y) in &self.shards {
            shards.push(born);
            shards.push(0);
            shards.extend(x.to_le_bytes());
            shards.extend(y.to_le_bytes());
        }
        add(sections, space, "shard_spawns".into(), shards)?;
        add(sections, space, "shard_xs".into(), self.xs.clone())?;
        add(sections, space, "shard_ys".into(), self.ys.clone())?;
        add(sections, space, "shard_frames".into(), self.frames.clone())?;
        Ok(())
    }
}
pub fn emit(p: &mut w65c816::Assembler, layout: &CartridgeLayout) -> Result<()> {
    use crate::sfc::{emit, implied, write};
    use crate::sfc_b1_menu::OAM_BUFFER;
    use w65c816::{AddressingMode as A, CpuMode, Mnemonic as M, Operand as O};
    let abs = |p: &mut w65c816::Assembler, m, at: u16| emit(p, m, A::Absolute, O::Word(at));
    let long_x = |p: &mut w65c816::Assembler, m, at: u32| emit(p, m, A::AbsoluteLongX, O::Long(at));
    let address = |name: &str| -> Result<u32> { layout.cpu_address(layout.section(name)?.offset) };
    let (jobs, spawns, xs, ys, frames, oam_job) = (
        address("shard_jobs")?,
        address("shard_spawns")?,
        address("shard_xs")?,
        address("shard_ys")?,
        address("shard_frames")?,
        address("menu_oam_job")?,
    );
    let m16 = |p: &mut w65c816::Assembler| emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    let m8 = |p: &mut w65c816::Assembler| emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    let life = u16::try_from(layout.section("shard_frames")?.bytes.len() / (COLOURS * 4))?;
    p.label("field_shards_start")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    implied(p, M::Phx);
    implied(p, M::Phy);
    emit(p, M::Dec, A::Accumulator, O::None);
    abs(p, M::Sta, SHARD_COLOUR);
    m16(p);
    emit(p, M::And, A::Immediate, O::Word(7));
    abs(p, M::Sta, 0xe0);
    emit(p, M::Asl, A::Accumulator, O::None);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Clc);
    abs(p, M::Adc, 0xe0);
    emit(p, M::Adc, A::Immediate, O::Word(jobs as u16));
    abs(p, M::Sta, 0xe0);
    m8(p);
    write(p, 0xe2, (jobs >> 16) as u8);
    emit(p, M::Ldx, A::Immediate, O::Word(crate::sfc_b1::FADE_SLOT));
    p.emit_label_ref(M::Jsr, A::Absolute, "b1_enqueue");
    emit(p, M::Ldx, A::Immediate, O::Word(0));
    emit(p, M::Ldy, A::Immediate, O::Word(0));
    p.label("field_shards_offset");
    m16(p);
    for axis in 0..2u16 {
        emit(p, M::Lda, A::AbsoluteY, O::Word(SHARD_DRAWS + axis));
        emit(p, M::And, A::Immediate, O::Word(255));
        abs(p, M::Sta, 0xe4);
        emit(p, M::Lda, A::Immediate, O::Word(0));
        for bit in (0..16 - life.leading_zeros()).rev() {
            emit(p, M::Asl, A::Accumulator, O::None);
            if life & (1 << bit) != 0 {
                implied(p, M::Clc);
                abs(p, M::Adc, 0xe4);
            }
        }
        implied(p, M::Clc);
        long_x(p, M::Adc, spawns + 2 + 2 * u32::from(axis));
        implied(p, M::Phx);
        implied(p, M::Pha);
        implied(p, M::Tya);
        emit(p, M::Asl, A::Accumulator, O::None);
        implied(p, M::Tax);
        implied(p, M::Pla);
        emit(p, M::Sta, A::AbsoluteX, O::Word(SHARD_OFFSETS + 2 * axis));
        implied(p, M::Plx);
    }
    m8(p);
    for _ in 0..6 {
        implied(p, M::Inx);
    }
    implied(p, M::Iny);
    implied(p, M::Iny);
    emit(p, M::Cpy, A::Immediate, O::Word(2 * SHARDS as u16));
    p.emit_label_ref(M::Beq, A::Relative8, "field_shards_offsets_done");
    p.emit_label_ref(M::Jmp, A::Absolute, "field_shards_offset");
    p.label("field_shards_offsets_done");
    write(p, SHARD_TIME, 1);
    implied(p, M::Ply);
    implied(p, M::Plx);
    implied(p, M::Rts);
    let last = (SHARDS / 8 - 1) as u16 + life;
    p.label("field_shards_tick")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, SHARD_TIME);
    p.emit_label_ref(M::Bne, A::Relative8, "field_shards_on");
    implied(p, M::Rts);
    p.label("field_shards_on");
    {
        let slot = crate::sfc_b1::QUEUE + crate::sfc_b1::FADE_SLOT;
        abs(p, M::Lda, slot + 3);
        abs(p, M::Ora, slot + 4);
        p.emit_label_ref(M::Beq, A::Relative8, "field_shards_ready");
        implied(p, M::Rts);
        p.label("field_shards_ready");
    }
    emit(p, M::Ldx, A::Immediate, O::Word(0));
    emit(p, M::Ldy, A::Immediate, O::Word(0));
    p.label("field_shards_each");
    abs(p, M::Lda, SHARD_TIME);
    implied(p, M::Clc);
    long_x(p, M::Sbc, spawns);
    p.emit_label_ref(M::Bcc, A::Relative8, "field_shards_hide_far");
    emit(p, M::Cmp, A::Immediate, O::Byte(life as u8));
    p.emit_label_ref(M::Bcc, A::Relative8, "field_shards_live");
    p.label("field_shards_hide_far");
    p.emit_label_ref(M::Jmp, A::Absolute, "field_shards_hide");
    p.label("field_shards_live");
    implied(p, M::Phx);
    abs(p, M::Sta, 0xe4);
    abs(p, M::Stz, 0xe5);
    abs(p, M::Lda, SHARD_COLOUR);
    m16(p);
    emit(p, M::And, A::Immediate, O::Word(7));
    abs(p, M::Sta, 0xe6);
    emit(p, M::Lda, A::Immediate, O::Word(0));
    for bit in (0..16 - life.leading_zeros()).rev() {
        emit(p, M::Asl, A::Accumulator, O::None);
        if life & (1 << bit) != 0 {
            implied(p, M::Clc);
            abs(p, M::Adc, 0xe6);
        }
    }
    implied(p, M::Clc);
    abs(p, M::Adc, 0xe4);
    emit(p, M::Asl, A::Accumulator, O::None);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Tax);
    m8(p);
    for k in 2..4u16 {
        long_x(p, M::Lda, frames + u32::from(k));
        emit(
            p,
            M::Sta,
            A::AbsoluteY,
            O::Word(OAM_BUFFER + FIRST_ENTRY as u16 * 4 + k),
        );
    }
    long_x(p, M::Lda, frames);
    abs(p, M::Sta, 0xe6);
    long_x(p, M::Lda, frames + 1);
    abs(p, M::Sta, 0xe7);
    for (axis, table, delta) in [(0u16, xs, 0xe6u16), (1, ys, 0xe7)] {
        m16(p);
        emit(p, M::Lda, A::AbsoluteY, O::Word(SHARD_OFFSETS + 2 * axis));
        implied(p, M::Clc);
        abs(p, M::Adc, 0xe4);
        implied(p, M::Tax);
        m8(p);
        long_x(p, M::Lda, table);
        implied(p, M::Clc);
        abs(p, M::Adc, delta);
        emit(
            p,
            M::Sta,
            A::AbsoluteY,
            O::Word(OAM_BUFFER + FIRST_ENTRY as u16 * 4 + axis),
        );
    }
    implied(p, M::Plx);
    p.emit_label_ref(M::Bra, A::Relative8, "field_shards_next");
    p.label("field_shards_hide");
    emit(p, M::Lda, A::Immediate, O::Byte(224));
    emit(
        p,
        M::Sta,
        A::AbsoluteY,
        O::Word(OAM_BUFFER + FIRST_ENTRY as u16 * 4 + 1),
    );
    p.label("field_shards_next");
    for _ in 0..6 {
        implied(p, M::Inx);
    }
    for _ in 0..4 {
        implied(p, M::Iny);
    }
    emit(p, M::Cpy, A::Immediate, O::Word(4 * SHARDS as u16));
    p.emit_label_ref(M::Beq, A::Relative8, "field_shards_placed");
    p.emit_label_ref(M::Jmp, A::Absolute, "field_shards_each");
    p.label("field_shards_placed");
    for e in (FIRST_ENTRY..FIRST_ENTRY + SHARDS).step_by(4) {
        write(p, OAM_BUFFER + 512 + (e / 4) as u16, 0);
    }
    m16(p);
    emit(p, M::Lda, A::Immediate, O::Word(oam_job as u16));
    abs(p, M::Sta, 0xe0);
    m8(p);
    write(p, 0xe2, (oam_job >> 16) as u8);
    emit(
        p,
        M::Ldx,
        A::Immediate,
        O::Word(crate::sfc_b1::MENU_OAM_SLOT),
    );
    p.emit_label_ref(M::Jsr, A::Absolute, "b1_enqueue");
    abs(p, M::Inc, SHARD_TIME);
    abs(p, M::Lda, SHARD_TIME);
    emit(p, M::Cmp, A::Immediate, O::Byte(last as u8 + 2));
    p.emit_label_ref(M::Bcc, A::Relative8, "field_shards_running");
    abs(p, M::Stz, SHARD_TIME);
    p.label("field_shards_running");
    implied(p, M::Rts);
    Ok(())
}
