use crate::{Result, SOURCE, sha};
use serde::Deserialize;
use sfc_builder::cartridge::{CartridgeLayout, Section};
use std::collections::BTreeMap;
const COUNTERS: u16 = 0x6a0;
const GATHERED: [usize; 2] = [0, 2];
const BUFFERS: [[u16; 2]; 2] = [[0x6600, 0x6d00], [0x5600, 0xa900]];
const JOBS: [[u16; 2]; 2] = [[0xb800, 0xb880], [0xb700, 0xb780]];
const JOB_BYTES: usize = 0x80;
const CAPACITY: [u16; 2] = [0x700, 0x700];
const HALVES: [u16; 2] = [0xb900 - 2, 0xb800 - 2];
pub fn emit_halves_reset(p: &mut w65c816::Assembler) {
    use crate::sfc::emit;
    use w65c816::{AddressingMode as A, Mnemonic as M, Operand as O};
    let bank = u32::from(crate::sfc_body_tiles::STAGE_BANK) << 16;
    emit(p, M::Lda, A::Immediate, O::Byte(0));
    for half in HALVES {
        emit(p, M::Sta, A::AbsoluteLong, O::Long(bank | u32::from(half)));
    }
}
const POSE_SECTION: &str = "b1_portrait_pose_code";
const POSE_BYTES: usize = 0x180;
const GROUP_POSES: u16 = 0x6b0;
const UNIT_POSES: u16 = 0x6c0;
const SCRATCH: u16 = 0x6be;
pub const FACE_STEP: u16 = 0x6ac;
pub const FACE_BACKGROUND: u16 = 0x6ad;
const OFFSET: u16 = 0x6ae;
const RAW: u16 = 0x6a6;
pub const QUAKE: u16 = 0x6a8;
pub const QUAKE_BASE: u16 = 0x6aa;
pub const HUM: u16 = 0x6a9;
pub const HUM_ON: u8 = 0x80;
const HUM_STARTED: u8 = 0x40;
pub const HUM_FLAG: u16 = 0x140;
pub const HUM_TICKS: usize = 32;
pub const IMPACT: u16 = 0x6bb;
pub const SHAKE: u16 = 0x6b8;
pub const IMPACT_TICKS: u8 = 16;
pub const EAT: u16 = 0x6ba;
pub const EAT_WAITING: u8 = 0xff;
pub const EAT_TICKS: u8 = 74;
pub const STEPS: usize = 8;
pub const BACKGROUNDS: usize = 4;
const REST_SCROLL: u16 = 0xffff;
pub const MAX_UNITS: usize = 3;
const CHUNK_TILES: usize = 16;
const TRANSPARENT: u8 = 255;
#[derive(Deserialize)]
struct Step {
    clip: u16,
    hold: u16,
}
#[derive(Deserialize)]
struct Channel {
    clips: Vec<u16>,
    sequence: Vec<Step>,
}
#[derive(Deserialize)]
struct GroupSource {
    name: String,
    parts: Vec<String>,
    layers: BTreeMap<String, String>,
}
#[derive(Deserialize)]
struct StepSource {
    eyes: Vec<Step>,
    mouth: Vec<Step>,
}
#[derive(Deserialize)]
struct ArleSource {
    eye_clips: Vec<u16>,
    mouth_clips: Vec<u16>,
    combos: Vec<[u16; 2]>,
    states: Vec<StepSource>,
    layers: BTreeMap<String, String>,
}
#[derive(Deserialize)]
struct ReactionSource {
    name: String,
    ticks: u8,
    #[serde(default)]
    layer_sha256: String,
    #[serde(default)]
    shake: Vec<i16>,
    #[serde(default)]
    layers: BTreeMap<String, String>,
    #[serde(default)]
    waiting: u8,
    #[serde(default)]
    timeline: Vec<u8>,
}
#[derive(Deserialize)]
struct Idle {
    channels: BTreeMap<String, Channel>,
    bases: Vec<String>,
    background_slots: Vec<Option<usize>>,
    arle: ArleSource,
    groups: Vec<GroupSource>,
    reactions: Vec<ReactionSource>,
}
#[derive(Deserialize)]
struct Definition {
    schema: u8,
    source_sha256: String,
    idle: Idle,
}
struct Part {
    tables: Vec<Vec<u8>>,
    stride: u8,
}
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Idle = 0,
    Impact = 1,
    IdleOrEat = 2,
    Background = 3,
}
struct Group {
    kind: Kind,
    parts: Vec<Part>,
    map: Option<Vec<u8>>,
    layers: Vec<Vec<u8>>,
    carbuncle: bool,
}
struct Unit {
    members: Vec<(usize, u16)>,
    cells: Vec<(usize, u16)>,
    chunks: Vec<std::ops::Range<usize>>,
    poses: Vec<Vec<u8>>,
}
pub struct Assets {
    groups: Vec<Group>,
    units: Vec<Unit>,
    shake: Vec<u16>,
    eat: Vec<u8>,
    pub report: serde_json::Value,
}
fn planar(pixels: &[u8], width: usize, cell: usize) -> Vec<u8> {
    let (cx, cy) = (cell % (width / 8), cell / (width / 8));
    let mut bytes = vec![0; 64];
    for y in 0..8 {
        for x in 0..8 {
            let color = pixels[(cy * 8 + y) * width + cx * 8 + x];
            for plane in 0..8 {
                bytes[plane / 2 * 16 + y * 2 + plane % 2] |= ((color >> plane) & 1) << (7 - x);
            }
        }
    }
    bytes
}
fn loop_ticks(clips: &[u16], sequence: &[Step]) -> Result<Vec<u8>> {
    let mut ticks = Vec::new();
    for step in sequence {
        let index = clips
            .iter()
            .position(|c| *c == step.clip)
            .ok_or("Loop step outside its clips")?;
        ticks.extend(std::iter::repeat_n(index as u8, usize::from(step.hold)));
    }
    if ticks.is_empty() || ticks.len() > 255 {
        return Err("Loop outside one-byte tick range".into());
    }
    Ok(ticks)
}
impl Assets {
    pub fn carbuncle_first_pose(&self) -> Vec<bool> {
        let mut drawn = vec![false; 224 * 240];
        for group in self.groups.iter().filter(|g| g.carbuncle) {
            for (d, p) in drawn.iter_mut().zip(&group.layers[0]) {
                *d |= *p != TRANSPARENT;
            }
        }
        drawn
    }
    pub fn load(
        content: &crate::content::Content,
        width: usize,
        height: usize,
        scale: crate::sfc_portrait_scale::Reduce<'_>,
        last_tile: u16,
    ) -> Result<Self> {
        let root = content.sub("b1-portrait");
        let def: Definition = root.json("portrait.json")?;
        if def.schema != 1 || def.source_sha256 != SOURCE {
            return Err("Invalid idle portrait source".into());
        }
        let read = |name: &str, expected: &str| -> Result<Vec<u8>> {
            let pixels = root.sub("idle").read(format!("{name}.indices"))?;
            if sha(&pixels) != expected || pixels.len() != 224 * 240 {
                return Err(format!("Invalid idle image {name}").into());
            }
            Ok(pixels)
        };
        let idle = &def.idle;
        if idle.bases.len() != BACKGROUNDS || idle.arle.states.len() != STEPS {
            return Err("Expected four backgrounds and eight HP steps".into());
        }
        let bases = idle
            .bases
            .iter()
            .enumerate()
            .map(|(k, expected)| read(&format!("base-{k}"), expected))
            .collect::<Result<Vec<_>>>()?;
        let background_slots: Vec<usize> =
            idle.background_slots.iter().flatten().copied().collect();
        let mut groups = vec![Group {
            kind: Kind::Background,
            parts: Vec::new(),
            map: None,
            layers: bases.clone(),
            carbuncle: false,
        }];
        let arle = &idle.arle;
        let (eyes, mouths) = (&arle.eye_clips, &arle.mouth_clips);
        let mut eye_tables = Vec::new();
        let mut mouth_tables = Vec::new();
        for step in &arle.states {
            eye_tables.push(loop_ticks(eyes, &step.eyes)?);
            mouth_tables.push(loop_ticks(mouths, &step.mouth)?);
        }
        let mut map = vec![TRANSPARENT; eyes.len() * mouths.len()];
        let mut layers = Vec::new();
        for (pose, &[e, m]) in arle.combos.iter().enumerate() {
            let at = eyes
                .iter()
                .position(|&c| c == e)
                .ok_or("Arle pair outside her clips")?
                * mouths.len()
                + mouths
                    .iter()
                    .position(|&c| c == m)
                    .ok_or("Arle pair outside her clips")?;
            map[at] = u8::try_from(pose)?;
            let name = format!("arle-{e}-{m}");
            layers.push(read(
                &name,
                arle.layers.get(&name).ok_or("Missing Arle layer")?,
            )?);
        }
        for (s, step) in arle.states.iter().enumerate() {
            for e in &step.eyes {
                for m in &step.mouth {
                    let at = eyes.iter().position(|&c| c == e.clip).unwrap_or(0) * mouths.len()
                        + mouths.iter().position(|&c| c == m.clip).unwrap_or(0);
                    if map[at] == TRANSPARENT {
                        return Err(format!("HP step {s} shows a pair without a layer").into());
                    }
                }
            }
        }
        groups.push(Group {
            kind: Kind::IdleOrEat,
            parts: vec![
                Part {
                    tables: eye_tables,
                    stride: u8::try_from(mouths.len())?,
                },
                Part {
                    tables: mouth_tables,
                    stride: 1,
                },
            ],
            map: Some(map),
            layers,
            carbuncle: false,
        });
        for source in &idle.groups {
            let mut radix = Vec::new();
            let mut parts = Vec::new();
            for name in &source.parts {
                let channel = idle.channels.get(name).ok_or("Missing idle channel")?;
                radix.push(channel.clips.len());
                parts.push(Part {
                    tables: vec![loop_ticks(&channel.clips, &channel.sequence)?],
                    stride: 0,
                });
            }
            let mut stride = 1;
            for (part, size) in parts.iter_mut().zip(&radix).rev() {
                part.stride = u8::try_from(stride)?;
                stride *= size;
            }
            let mut layers = Vec::new();
            for pose in 0..stride {
                let digits: Vec<String> = parts
                    .iter()
                    .zip(&radix)
                    .map(|(part, size)| (pose / usize::from(part.stride) % size).to_string())
                    .collect();
                let name = format!("{}-{}", source.name, digits.join("-"));
                let expected = source.layers.get(&name).ok_or("Missing idle layer")?;
                layers.push(read(&name, expected)?);
            }
            groups.push(Group {
                kind: Kind::Idle,
                parts,
                map: None,
                layers,
                carbuncle: source.name == "carbuncle",
            });
        }
        let [impact, eat] = idle.reactions.as_slice() else {
            return Err("Expected the collision and eating reactions".into());
        };
        if eat.name != "eat"
            || eat.ticks != EAT_TICKS
            || eat.timeline.len() != usize::from(eat.ticks)
            || eat
                .timeline
                .iter()
                .chain([&eat.waiting])
                .any(|&p| usize::from(p) >= eat.layers.len())
        {
            return Err("Invalid eating reaction".into());
        }
        let arle = &mut groups[1];
        let first_eat = u8::try_from(arle.layers.len())?;
        for k in 0..eat.layers.len() {
            let name = format!("eat-{k}");
            arle.layers.push(read(
                &name,
                eat.layers.get(&name).ok_or("Missing eating layer")?,
            )?);
        }
        let eat_poses: Vec<u8> = std::iter::once(eat.waiting)
            .chain(eat.timeline.iter().copied())
            .map(|p| p + first_eat)
            .collect();
        if impact.name != "impact"
            || impact.ticks != IMPACT_TICKS
            || impact.shake.len() != usize::from(impact.ticks)
        {
            return Err("Invalid collision reaction".into());
        }
        groups.push(Group {
            kind: Kind::Impact,
            parts: Vec::new(),
            map: None,
            layers: vec![
                vec![TRANSPARENT; 224 * 240],
                read(&impact.name, &impact.layer_sha256)?,
            ],
            carbuncle: false,
        });
        let mut shake = vec![REST_SCROLL];
        for &offset in impact.shake.iter().rev() {
            let lines = crate::sfc::source_y_to_screen(i64::from(offset));
            shake.push(REST_SCROLL.wrapping_sub(lines as u16));
        }
        if groups.iter().any(|g| g.layers.len() > 255) {
            return Err("Idle group poses exceed one byte".into());
        }
        let compose = |poses: &[usize]| -> Vec<u8> {
            let mut image = bases[0].clone();
            let mut carbuncle = vec![false; image.len()];
            for (group, pose) in groups.iter().zip(poses) {
                for ((pixel, drawn), carb) in image
                    .iter_mut()
                    .zip(&group.layers[*pose])
                    .zip(carbuncle.iter_mut())
                {
                    if *drawn != TRANSPARENT {
                        *pixel = *drawn;
                        *carb = group.carbuncle;
                    }
                }
            }
            scale(&image, &carbuncle, poses[0])
        };
        let cells = (width / 8) * (height / 8);
        let default = compose(&vec![0; groups.len()]);
        let mut members: Vec<Vec<usize>> = vec![Vec::new(); cells];
        for (g, group) in groups.iter().enumerate() {
            if group.kind == Kind::Background {
                continue;
            }
            for pose in 1..group.layers.len() {
                let mut poses = vec![0; groups.len()];
                poses[g] = pose;
                let image = compose(&poses);
                for (cell, list) in members.iter_mut().enumerate() {
                    if !list.contains(&g)
                        && planar(&image, width, cell) != planar(&default, width, cell)
                    {
                        list.push(g);
                    }
                }
            }
        }
        let none = vec![false; 224 * 240];
        let scaled_bases: Vec<Vec<u8>> = bases
            .iter()
            .enumerate()
            .map(|(k, b)| scale(b, &none, k))
            .collect();
        for (cell, list) in members.iter_mut().enumerate() {
            if !list.is_empty()
                && scaled_bases[1..]
                    .iter()
                    .any(|b| planar(b, width, cell) != planar(&scaled_bases[0], width, cell))
            {
                list.push(0);
            }
        }
        let own = members.clone();
        let mut by_members: BTreeMap<Vec<usize>, Vec<usize>> = BTreeMap::new();
        for (cell, list) in members.iter_mut().enumerate() {
            if !list.is_empty() {
                list.sort();
                by_members.entry(list.clone()).or_default().push(cell);
            }
        }
        let poses =
            |list: &[usize]| -> usize { list.iter().map(|&g| groups[g].layers.len()).product() };
        while by_members.len() > MAX_UNITS {
            let (from, into) = by_members
                .iter()
                .flat_map(|(a, cells)| {
                    by_members
                        .keys()
                        .filter(move |b| *b != a && a.iter().all(|g| b.contains(g)))
                        .map(move |b| (cells.len() * poses(b), a.clone(), b.clone()))
                })
                .min()
                .map(|(_, a, b)| (a, b))
                .ok_or("Idle portrait needs more upload slots")?;
            let cells = by_members.remove(&from).unwrap_or_default();
            let target = by_members.entry(into).or_default();
            target.extend(cells);
        }
        let total: usize = by_members.values().map(Vec::len).sum();
        let mut tile = last_tile
            .checked_sub(u16::try_from(total)?)
            .ok_or("Idle portrait cells exceed VRAM")?;
        let mut units = Vec::new();
        let mut report = Vec::new();
        for (list, mut unit_cells) in by_members {
            let mut stride = 1usize;
            let mut members: Vec<(usize, u16)> = Vec::new();
            for &g in list.iter().rev() {
                members.push((g, u16::try_from(stride)?));
                stride *= groups[g].layers.len();
            }
            members.reverse();
            unit_cells.sort_by_key(|&c| {
                let mut groups = own[c].clone();
                groups.sort();
                (groups.len(), groups, c)
            });
            let mut chunks: Vec<std::ops::Range<usize>> = Vec::new();
            for (i, &c) in unit_cells.iter().enumerate() {
                match chunks.last_mut() {
                    Some(last)
                        if last.len() < CHUNK_TILES && {
                            let (mut a, mut b) =
                                (own[unit_cells[last.start]].clone(), own[c].clone());
                            a.sort();
                            b.sort();
                            a == b
                        } =>
                    {
                        last.end = i + 1
                    }
                    _ => chunks.push(i..i + 1),
                }
            }
            let poses = (0..stride)
                .map(|pose| {
                    let mut group_poses = vec![0; groups.len()];
                    for &(g, s) in &members {
                        group_poses[g] = pose / usize::from(s) % groups[g].layers.len();
                    }
                    let image = compose(&group_poses);
                    unit_cells
                        .iter()
                        .flat_map(|c| planar(&image, width, *c))
                        .collect()
                })
                .collect();
            report.push(serde_json::json!(
                { "groups" : list, "cells" : unit_cells.len(), "poses" : stride,
                "chunks" : chunks.len() }
            ));
            let cells = unit_cells
                .into_iter()
                .map(|c| {
                    tile += 1;
                    (c, tile - 1)
                })
                .collect();
            units.push(Unit {
                members,
                cells,
                chunks,
                poses,
            });
        }
        Ok(Self {
            report: serde_json::json!(
                { "units" : report, "first_tile" : last_tile as usize - total, "loops" :
                groups.iter().map(| g | g.parts.iter().map(| p | p.tables.iter()
                .map(Vec::len).collect::< Vec < _ >> ()).collect::< Vec < _ >> ())
                .collect::< Vec < _ >> (), "tick" :
                "one NMI per source scheduler tick (unverified test speed)", "impact" : {
                "ticks" : impact.ticks, "scroll" : shake, "zoom" : false, "audio" : false
                }, "eat" : { "ticks" : EAT_TICKS, "poses" : eat_poses }, "hp_steps" :
                STEPS, "backgrounds" : BACKGROUNDS, "background_slots" :
                background_slots, "limits" :
                ["HP-step changes refresh the field portrait after pending uploads drain; eating and battle presentation keep ownership. Step 8 (HP 0) shows step 7.",
                "Step 7's two flying helpers (KAO_2 clip 26) and the fountain's reaction (message 9) are not reproduced.",
                "The collision's dungeon zoom (1.24x to 1.0x) and sound 12 are not reproduced."]
                }
            ),
            groups,
            units,
            shake,
            eat: eat_poses,
        })
    }
    pub fn first_tile(&self) -> u16 {
        self.cells().map(|(_, t)| t).min().unwrap_or(u16::MAX)
    }
    pub fn cells(&self) -> impl Iterator<Item = (usize, u16)> + '_ {
        self.units.iter().flat_map(|u| u.cells.iter().copied())
    }
    pub fn append(
        &self,
        sections: &mut Vec<Section>,
        space: &mut crate::sfc_rom::Space,
    ) -> Result<Vec<Vec<u8>>> {
        let mut shape = vec![self.groups.len() as u8];
        for group in &self.groups {
            shape.push(group.kind as u8);
            shape.push(group.parts.len() as u8);
            for part in &group.parts {
                shape.push(part.stride);
                shape.push(part.tables.len() as u8);
            }
            shape.push(u8::from(group.map.is_some()));
        }
        shape.push(self.units.len() as u8);
        for unit in &self.units {
            shape.push(unit.members.len() as u8);
            for &(g, stride) in &unit.members {
                shape.push(g as u8);
                shape.extend(stride.to_le_bytes());
            }
        }
        super::sfc_b1::add(sections, space, "b1_portrait_shape".into(), shape)?;
        for (g, group) in self.groups.iter().enumerate() {
            for (p, part) in group.parts.iter().enumerate() {
                let mut offsets = Vec::new();
                let mut lengths = Vec::new();
                let mut ticks = Vec::new();
                for table in &part.tables {
                    offsets.extend(u16::try_from(ticks.len())?.to_le_bytes());
                    lengths.push(table.len() as u8);
                    ticks.extend(table);
                }
                super::sfc_b1::add(sections, space, format!("b1_portrait_ticks_{g}_{p}"), ticks)?;
                if part.tables.len() > 1 {
                    super::sfc_b1::add(
                        sections,
                        space,
                        format!("b1_portrait_offsets_{g}_{p}"),
                        offsets,
                    )?;
                    super::sfc_b1::add(
                        sections,
                        space,
                        format!("b1_portrait_lengths_{g}_{p}"),
                        lengths,
                    )?;
                }
            }
            if let Some(map) = &group.map {
                super::sfc_b1::add(sections, space, format!("b1_portrait_map_{g}"), map.clone())?;
            }
        }
        super::sfc_b1::add(sections, space, "b1_portrait_eat".into(), self.eat.clone())?;
        let mut shake = Vec::new();
        for value in &self.shake {
            shake.extend(value.to_le_bytes());
        }
        super::sfc_b1::add(sections, space, "b1_portrait_shake".into(), shake)?;
        let mut stored: BTreeMap<Vec<u8>, u32> = BTreeMap::new();
        let mut lists: BTreeMap<Vec<u8>, u32> = BTreeMap::new();
        let mut count = 0;
        let mut first: Vec<Vec<u8>> = vec![Vec::new(); BACKGROUNDS];
        let mut store = crate::sfc_body_tiles::Store::default();
        for (u, unit) in self.units.iter().enumerate() {
            if GATHERED.contains(&u) {
                for pose in &unit.poses {
                    store.note(pose)?;
                }
            }
        }
        store.place_tiles(sections, space, "b1_portrait_tiles")?;
        for (u, unit) in self.units.iter().enumerate() {
            let mut table = Vec::new();
            let mut by_pose = Vec::new();
            let firsts: Vec<usize> = (0..BACKGROUNDS)
                .map(|k| {
                    unit.members
                        .iter()
                        .map(|&(g, stride)| self.first_pose(g, k) * usize::from(stride))
                        .sum()
                })
                .collect();
            for (index, pose) in unit.poses.iter().enumerate() {
                let gathered = GATHERED.contains(&u);
                let mut list = Vec::new();
                let mut template = Vec::new();
                let mut offset = 0usize;
                for chunk in &unit.chunks {
                    let data = pose[chunk.start * 64..chunk.end * 64].to_vec();
                    let vram = unit.cells[chunk.start].1 * 64;
                    template.extend(super::sfc_b1::transfer(
                        offset as u32,
                        vram,
                        chunk.len() * 64,
                        0,
                    ));
                    offset += data.len();
                    if gathered && !firsts.contains(&index) {
                        continue;
                    }
                    let source = match stored.get(&data) {
                        Some(&source) => source,
                        None => {
                            count += 1;
                            let source = super::sfc_b1::add(
                                sections,
                                space,
                                format!("b1_portrait_chunk_{count}"),
                                data.clone(),
                            )?;
                            stored.insert(data, source);
                            source
                        }
                    };
                    list.extend(super::sfc_b1::transfer(source, vram, chunk.len() * 64, 0));
                }
                let descriptors = template.len() / 8;
                if gathered {
                    let g = GATHERED.iter().position(|&x| x == u).unwrap();
                    if offset > usize::from(CAPACITY[g]) || template.len() > JOB_BYTES - 2 {
                        return Err(
                            format!(
                                "Portrait pose exceeds its WRAM half: unit {u}, {offset} bytes, {} job bytes",
                                template.len()
                            )
                                .into(),
                        );
                    }
                    let pose_bytes: Vec<u8> = unit
                        .chunks
                        .iter()
                        .flat_map(|c| pose[c.start * 64..c.end * 64].to_vec())
                        .collect();
                    let mut record = vec![u8::try_from(descriptors)?];
                    record.extend(u16::try_from(pose_bytes.len())?.to_le_bytes());
                    record.extend(&template);
                    let runs = store.list(&pose_bytes)?;
                    record.extend(&runs);
                    if crate::sfc_body_tiles::Store::rebuild(sections, &runs)? != pose_bytes {
                        return Err("Portrait runs do not rebuild their pose".into());
                    }
                    let at = match lists.get(&record) {
                        Some(&at) => at,
                        None => {
                            count += 1;
                            let at = super::sfc_b1::add(
                                sections,
                                space,
                                format!("b1_portrait_pose_{count}"),
                                record.clone(),
                            )?;
                            lists.insert(record, at);
                            at
                        }
                    };
                    table.extend(&at.to_le_bytes()[..3]);
                    table.push(u8::try_from(descriptors)?);
                } else {
                    let at = match lists.get(&list) {
                        Some(&at) => at,
                        None => {
                            count += 1;
                            let at = super::sfc_b1::add(
                                sections,
                                space,
                                format!("b1_portrait_jobs_{count}"),
                                list.clone(),
                            )?;
                            lists.insert(list.clone(), at);
                            at
                        }
                    };
                    table.extend(&at.to_le_bytes()[..3]);
                    table.push(u8::try_from(descriptors)?);
                }
                by_pose.push(list);
            }
            super::sfc_b1::add(sections, space, format!("b1_portrait_poses_{u}"), table)?;
            for (k, list) in first.iter_mut().enumerate() {
                list.extend(&by_pose[firsts[k]]);
            }
        }
        let offset = space.reserve_fast(POSE_BYTES)?;
        sections.push(Section {
            name: POSE_SECTION.into(),
            offset,
            bytes: vec![0; POSE_BYTES],
        });
        Ok(first)
    }
    fn first_pose(&self, g: usize, k: usize) -> usize {
        let group = &self.groups[g];
        if group.kind == Kind::Background {
            return k;
        }
        let step = (k * 2).min(STEPS - 1);
        let raw: usize = group
            .parts
            .iter()
            .map(|part| {
                let table = part.tables.get(step).unwrap_or(&part.tables[0]);
                usize::from(table[0]) * usize::from(part.stride)
            })
            .sum();
        group.map.as_ref().map_or(raw, |map| usize::from(map[raw]))
    }
}
pub fn emit(p: &mut w65c816::Assembler, layout: &CartridgeLayout) -> Result<()> {
    use crate::sfc::{emit, implied, write};
    use w65c816::{AddressingMode as A, CpuMode, Mnemonic as M, Operand as O};
    let abs = |p: &mut w65c816::Assembler, m, a| emit(p, m, A::Absolute, O::Word(a));
    let address = |name: &str| -> Result<u32> { layout.cpu_address(layout.section(name)?.offset) };
    let shape = &layout.section("b1_portrait_shape")?.bytes;
    type Parts = Vec<(u8, usize, usize)>;
    let mut groups: Vec<(u8, Parts, bool)> = Vec::new();
    let mut at = 1;
    for g in 0..usize::from(shape[0]) {
        let kind = shape[at];
        let count = usize::from(shape[at + 1]);
        let mut parts = Vec::new();
        for i in 0..count {
            let ticks = layout
                .section(&format!("b1_portrait_ticks_{g}_{i}"))?
                .bytes
                .len();
            parts.push((
                shape[at + 2 + i * 2],
                usize::from(shape[at + 3 + i * 2]),
                ticks,
            ));
        }
        at += 2 + count * 2;
        groups.push((kind, parts, shape[at] != 0));
        at += 1;
    }
    let mut units: Vec<Vec<(u16, u16)>> = Vec::new();
    let unit_count = usize::from(shape[at]);
    at += 1;
    for _ in 0..unit_count {
        let count = usize::from(shape[at]);
        units.push(
            (0..count)
                .map(|m| {
                    let field = at + 1 + m * 3;
                    (
                        u16::from(shape[field]),
                        u16::from_le_bytes([shape[field + 1], shape[field + 2]]),
                    )
                })
                .collect(),
        );
        at += 1 + count * 3;
    }
    if units.len() > MAX_UNITS {
        return Err("Idle portrait units exceed queue slots".into());
    }
    let mut stepped = Vec::new();
    let mut counter = COUNTERS;
    for (_, parts, _) in &groups {
        for &(_, loops, _) in parts {
            if loops > 1 {
                if loops != STEPS {
                    return Err("A part loops per HP step or not at all".into());
                }
                stepped.push(counter);
            }
            counter += 1;
        }
    }
    p.label("b1_portrait_reset")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    write(p, crate::sfc_battle_display::DISPLAY, 0);
    let mut counter = COUNTERS;
    for (_, parts, _) in &groups {
        for _ in parts {
            write(p, counter, 255);
            counter += 1;
        }
    }
    for u in 0..units.len() as u16 {
        write(p, UNIT_POSES + u * 2, 255);
        write(p, UNIT_POSES + u * 2 + 1, 255);
    }
    write(p, IMPACT, 0);
    write(p, QUAKE, 0);
    write(p, HUM, 0);
    write(p, EAT, 0);
    for (at, value) in [
        (SHAKE, REST_SCROLL as u8),
        (SHAKE + 1, (REST_SCROLL >> 8) as u8),
    ] {
        write(p, at, value);
    }
    p.label("b1_portrait_select")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    p.emit_label_ref(M::Jsr, A::Absolute, "field_shortage");
    let step = crate::sfc_battle::ACTORS + 0x4f;
    abs(p, M::Lda, step);
    emit(p, M::Cmp, A::Immediate, O::Byte(STEPS as u8));
    p.emit_label_ref(M::Bcc, A::Relative8, "b1_portrait_step");
    emit(p, M::Lda, A::Immediate, O::Byte(STEPS as u8 - 1));
    p.label("b1_portrait_step");
    abs(p, M::Sta, FACE_STEP);
    for &c in &stepped {
        write(p, c, 255);
    }
    abs(p, M::Lda, step);
    emit(p, M::Lsr, A::Accumulator, O::None);
    emit(p, M::Cmp, A::Immediate, O::Byte(BACKGROUNDS as u8));
    p.emit_label_ref(M::Bcc, A::Relative8, "b1_portrait_background");
    emit(p, M::Lda, A::Immediate, O::Byte(BACKGROUNDS as u8 - 1));
    p.label("b1_portrait_background");
    abs(p, M::Cmp, FACE_BACKGROUND);
    p.emit_label_ref(M::Beq, A::Relative8, "b1_portrait_selected");
    abs(p, M::Sta, FACE_BACKGROUND);
    let records = address("b1_portrait_backgrounds")?;
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    emit(p, M::And, A::Immediate, O::Word(255));
    abs(p, M::Sta, OFFSET);
    emit(p, M::Asl, A::Accumulator, O::None);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Clc);
    abs(p, M::Adc, OFFSET);
    implied(p, M::Tax);
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    let slot = super::sfc_b1::QUEUE + 15;
    for i in 0..5u16 {
        emit(p, M::Lda, A::AbsoluteLongX, O::Long(records + u32::from(i)));
        abs(p, M::Sta, slot + i);
    }
    emit(
        p,
        M::Lda,
        A::Immediate,
        O::Byte(crate::sfc_battle_display::RESTORING),
    );
    abs(p, M::Sta, crate::sfc_battle_display::DISPLAY);
    p.label("b1_portrait_selected");
    implied(p, M::Rts);
    p.label("b1_shake_apply")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    for register in [0x210e, 0x2110] {
        abs(p, M::Lda, SHAKE);
        abs(p, M::Sta, register);
        abs(p, M::Lda, SHAKE + 1);
        abs(p, M::Sta, register);
    }
    implied(p, M::Rts);
    p.label("b1_portrait_tick")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    {
        use crate::sfc_battle_display::DISPLAY;
        let restore = crate::sfc_b1::QUEUE + 3 * 5;
        abs(p, M::Lda, DISPLAY);
        p.emit_label_ref(M::Beq, A::Relative8, "b1_portrait_shown");
        emit(
            p,
            M::Cmp,
            A::Immediate,
            O::Byte(crate::sfc_battle_display::RESTORING),
        );
        p.emit_label_ref(M::Bne, A::Relative8, "b1_portrait_lent");
        abs(p, M::Lda, restore + 3);
        abs(p, M::Ora, restore + 4);
        p.emit_label_ref(M::Bne, A::Relative8, "b1_portrait_lent");
        p.emit_label_ref(M::Jmp, A::Absolute, "b1_portrait_reset");
        p.label("b1_portrait_lent");
        implied(p, M::Rts);
        p.label("b1_portrait_shown");
    }
    abs(p, M::Lda, EAT);
    p.emit_label_ref(M::Bne, A::Relative8, "b1_portrait_animate");
    abs(p, M::Lda, crate::sfc_battle::ACTORS + 0x4f);
    emit(p, M::Cmp, A::Immediate, O::Byte(STEPS as u8));
    p.emit_label_ref(M::Bcc, A::Relative8, "b1_portrait_current_step");
    emit(p, M::Lda, A::Immediate, O::Byte(STEPS as u8 - 1));
    p.label("b1_portrait_current_step");
    abs(p, M::Cmp, FACE_STEP);
    p.emit_label_ref(M::Beq, A::Relative8, "b1_portrait_animate");
    for u in 0..units.len() as u16 {
        let slot = super::sfc_b1::QUEUE + 15 + 5 * u;
        abs(p, M::Lda, slot + 3);
        abs(p, M::Ora, slot + 4);
        p.emit_label_ref(M::Bne, A::Relative8, "b1_portrait_refresh_wait");
    }
    p.emit_label_ref(M::Jmp, A::Absolute, "b1_portrait_select");
    p.label("b1_portrait_refresh_wait");
    implied(p, M::Rts);
    p.label("b1_portrait_animate");
    let shake = address("b1_portrait_shake")?;
    let eat = address("b1_portrait_eat")?;
    let mut counter = COUNTERS;
    for (g, (kind, parts, mapped)) in groups.iter().enumerate() {
        write(p, GROUP_POSES + g as u16, 0);
        if *kind == Kind::Background as u8 {
            abs(p, M::Lda, FACE_BACKGROUND);
            abs(p, M::Sta, GROUP_POSES + g as u16);
            continue;
        }
        if *kind == Kind::IdleOrEat as u8 {
            write(p, SCRATCH + 1, 0);
            abs(p, M::Lda, EAT);
            p.emit_label_ref(M::Beq, A::Relative8, "b1_portrait_no_eat");
            emit(p, M::Cmp, A::Immediate, O::Byte(EAT_WAITING));
            emit(p, M::Lda, A::Immediate, O::Byte(0));
            p.emit_label_ref(M::Bcs, A::Relative8, "b1_portrait_eat_pose");
            abs(p, M::Lda, EAT);
            abs(p, M::Sta, SCRATCH);
            abs(p, M::Dec, EAT);
            p.emit_label_ref(M::Bne, A::Relative8, "b1_portrait_eating");
            p.emit_label_ref(M::Jsr, A::Absolute, "b1_portrait_select");
            p.label("b1_portrait_eating");
            emit(p, M::Lda, A::Immediate, O::Byte(EAT_TICKS + 1));
            implied(p, M::Sec);
            abs(p, M::Sbc, SCRATCH);
            p.label("b1_portrait_eat_pose");
            emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
            emit(p, M::And, A::Immediate, O::Word(255));
            implied(p, M::Tax);
            emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
            emit(p, M::Lda, A::AbsoluteLongX, O::Long(eat));
            abs(p, M::Sta, SCRATCH + 1);
            p.label("b1_portrait_no_eat");
        }
        if *kind == Kind::Impact as u8 {
            emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
            abs(p, M::Lda, IMPACT);
            emit(p, M::And, A::Immediate, O::Word(255));
            emit(p, M::Asl, A::Accumulator, O::None);
            implied(p, M::Tax);
            emit(p, M::Lda, A::AbsoluteLongX, O::Long(shake));
            abs(p, M::Sta, SHAKE);
            emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
            abs(p, M::Lda, IMPACT);
            p.emit_label_ref(M::Beq, A::Relative8, "b1_portrait_no_impact");
            abs(p, M::Dec, IMPACT);
            write(p, GROUP_POSES + g as u16, 1);
            p.label("b1_portrait_no_impact");
            let hum = address("field_hum")?;
            abs(p, M::Lda, HUM);
            p.emit_label_ref(M::Beq, A::Relative8, "b1_portrait_no_hum");
            emit(p, M::And, A::Immediate, O::Byte(HUM_STARTED));
            p.emit_label_ref(M::Beq, A::Relative8, "b1_portrait_hum_show");
            {
                use crate::sfc_sound::{JOB, JOB_STREAM, STREAM_PENDING, STREAM_TIMER};
                abs(p, M::Lda, STREAM_PENDING);
                emit(p, M::Cmp, A::Immediate, O::Byte(0xff));
                p.emit_label_ref(M::Bne, A::Relative8, "b1_portrait_hum_show");
                abs(p, M::Lda, JOB);
                emit(p, M::Cmp, A::Immediate, O::Byte(JOB_STREAM));
                p.emit_label_ref(M::Beq, A::Relative8, "b1_portrait_hum_show");
                abs(p, M::Lda, STREAM_TIMER);
                abs(p, M::Ora, STREAM_TIMER + 1);
                p.emit_label_ref(M::Bne, A::Relative8, "b1_portrait_hum_show");
            }
            abs(p, M::Stz, HUM);
            let flag = crate::sfc_field_events::GLOBALS + HUM_FLAG / 8;
            abs(p, M::Lda, flag);
            emit(p, M::Ora, A::Immediate, O::Byte(1 << (HUM_FLAG % 8)));
            abs(p, M::Sta, flag);
            p.emit_label_ref(M::Bra, A::Relative8, "b1_portrait_no_hum");
            p.label("b1_portrait_hum_show");
            emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
            abs(p, M::Lda, HUM);
            emit(p, M::And, A::Immediate, O::Word(HUM_TICKS as u16 - 1));
            emit(p, M::Asl, A::Accumulator, O::None);
            implied(p, M::Tax);
            emit(p, M::Lda, A::AbsoluteLongX, O::Long(hum));
            abs(p, M::Sta, SHAKE);
            emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
            abs(p, M::Lda, HUM);
            emit(p, M::Inc, A::Accumulator, O::None);
            emit(p, M::And, A::Immediate, O::Byte(HUM_TICKS as u8 - 1));
            emit(p, M::Ora, A::Immediate, O::Byte(HUM_ON | HUM_STARTED));
            abs(p, M::Sta, HUM);
            p.label("b1_portrait_no_hum");
            let quake = address("field_quake")?;
            abs(p, M::Lda, QUAKE);
            p.emit_label_ref(M::Beq, A::Relative8, "b1_portrait_no_quake");
            emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
            emit(p, M::And, A::Immediate, O::Word(255));
            emit(p, M::Asl, A::Accumulator, O::None);
            implied(p, M::Clc);
            abs(p, M::Adc, QUAKE_BASE);
            implied(p, M::Tax);
            emit(p, M::Lda, A::AbsoluteLongX, O::Long(quake));
            abs(p, M::Sta, SHAKE);
            emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
            abs(p, M::Dec, QUAKE);
            p.label("b1_portrait_no_quake");
            continue;
        }
        for (i, &(stride, loops, length)) in parts.iter().enumerate() {
            let ticks = address(&format!("b1_portrait_ticks_{g}_{i}"))?;
            let next = format!("b1_portrait_tick_{g}_{i}");
            abs(p, M::Inc, counter);
            if loops > 1 {
                let lengths = address(&format!("b1_portrait_lengths_{g}_{i}"))?;
                let offsets = address(&format!("b1_portrait_offsets_{g}_{i}"))?;
                emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
                abs(p, M::Lda, FACE_STEP);
                emit(p, M::And, A::Immediate, O::Word(255));
                implied(p, M::Tax);
                emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
                emit(p, M::Lda, A::AbsoluteLongX, O::Long(lengths));
                abs(p, M::Sta, OFFSET);
                abs(p, M::Lda, counter);
                abs(p, M::Cmp, OFFSET);
                p.emit_label_ref(M::Bcc, A::Relative8, &next);
                write(p, counter, 0);
                p.label(next);
                emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
                abs(p, M::Lda, FACE_STEP);
                emit(p, M::And, A::Immediate, O::Word(255));
                emit(p, M::Asl, A::Accumulator, O::None);
                implied(p, M::Tax);
                emit(p, M::Lda, A::AbsoluteLongX, O::Long(offsets));
                abs(p, M::Sta, OFFSET);
                abs(p, M::Lda, counter);
                emit(p, M::And, A::Immediate, O::Word(255));
                implied(p, M::Clc);
                abs(p, M::Adc, OFFSET);
                implied(p, M::Tax);
                emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
            } else {
                abs(p, M::Lda, counter);
                emit(p, M::Cmp, A::Immediate, O::Byte(length as u8));
                p.emit_label_ref(M::Bcc, A::Relative8, &next);
                write(p, counter, 0);
                p.label(next);
                emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
                abs(p, M::Lda, counter);
                emit(p, M::And, A::Immediate, O::Word(255));
                implied(p, M::Tax);
                emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
            }
            emit(p, M::Lda, A::AbsoluteLongX, O::Long(ticks));
            abs(p, M::Sta, SCRATCH);
            if *mapped {
                emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
                if i == 0 {
                    abs(p, M::Stz, RAW);
                }
                for _ in 0..stride {
                    abs(p, M::Lda, SCRATCH);
                    emit(p, M::And, A::Immediate, O::Word(255));
                    implied(p, M::Clc);
                    abs(p, M::Adc, RAW);
                    abs(p, M::Sta, RAW);
                }
                emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
            } else {
                for _ in 0..stride {
                    implied(p, M::Clc);
                    abs(p, M::Lda, GROUP_POSES + g as u16);
                    abs(p, M::Adc, SCRATCH);
                    abs(p, M::Sta, GROUP_POSES + g as u16);
                }
            }
            counter += 1;
        }
        if *mapped {
            let map = address(&format!("b1_portrait_map_{g}"))?;
            emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
            abs(p, M::Lda, RAW);
            implied(p, M::Tax);
            emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
            emit(p, M::Lda, A::AbsoluteLongX, O::Long(map));
            abs(p, M::Sta, GROUP_POSES + g as u16);
        }
        if *kind == Kind::IdleOrEat as u8 {
            let idle = format!("b1_portrait_idle_{g}");
            abs(p, M::Lda, SCRATCH + 1);
            p.emit_label_ref(M::Beq, A::Relative8, &idle);
            abs(p, M::Sta, GROUP_POSES + g as u16);
            p.label(idle);
        }
    }
    abs(p, M::Lda, crate::sfc_battle_display::DISPLAY);
    p.emit_label_ref(M::Beq, A::Relative8, "b1_portrait_units");
    implied(p, M::Rts);
    p.label("b1_portrait_units");
    for (u, members) in units.iter().enumerate() {
        let pose = UNIT_POSES + 2 * u as u16;
        let same = format!("b1_portrait_same_{u}");
        emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
        abs(p, M::Stz, SCRATCH);
        for &(g, stride) in members {
            abs(p, M::Lda, GROUP_POSES + g);
            emit(p, M::And, A::Immediate, O::Word(255));
            implied(p, M::Tax);
            let done = format!("b1_portrait_scaled_{u}_{g}");
            let again = format!("b1_portrait_scale_{u}_{g}");
            p.label(&again);
            emit(p, M::Cpx, A::Immediate, O::Word(0));
            p.emit_label_ref(M::Beq, A::Relative8, &done);
            implied(p, M::Clc);
            abs(p, M::Lda, SCRATCH);
            emit(p, M::Adc, A::Immediate, O::Word(stride));
            abs(p, M::Sta, SCRATCH);
            implied(p, M::Dex);
            p.emit_label_ref(M::Bra, A::Relative8, &again);
            p.label(done);
        }
        abs(p, M::Lda, SCRATCH);
        abs(p, M::Cmp, pose);
        p.emit_label_ref(M::Beq, A::Relative8, &same);
        abs(p, M::Sta, pose);
        let table = address(&format!("b1_portrait_poses_{u}"))?;
        let slot = super::sfc_b1::QUEUE + 15 + 5 * u as u16;
        if let Some(g) = GATHERED.iter().position(|&x| x == u) {
            emit(
                p,
                M::Jsl,
                A::AbsoluteLong,
                O::Long(pose_entries(layout)?[g]),
            );
            p.label(same);
            emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
            continue;
        }
        for _ in 0..2 {
            emit(p, M::Asl, A::Accumulator, O::None);
        }
        implied(p, M::Tax);
        emit(p, M::Lda, A::AbsoluteLongX, O::Long(table));
        abs(p, M::Sta, slot);
        emit(p, M::Lda, A::AbsoluteLongX, O::Long(table + 2));
        emit(p, M::And, A::Immediate, O::Word(0xff00));
        implied(p, M::Xba);
        abs(p, M::Sta, slot + 3);
        emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
        emit(p, M::Lda, A::AbsoluteLongX, O::Long(table + 2));
        abs(p, M::Sta, slot + 2);
        p.label(same);
        emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    }
    implied(p, M::Rts);
    Ok(())
}
pub fn place_code(sections: &mut [Section], layout: &CartridgeLayout) -> Result<()> {
    let mut bytes = pose_code(layout)?.0.bytes().to_vec();
    bytes.resize(POSE_BYTES, 0);
    sections
        .iter_mut()
        .find(|s| s.name == POSE_SECTION)
        .ok_or("Missing portrait pose section")?
        .bytes = bytes;
    Ok(())
}
fn pose_entries(layout: &CartridgeLayout) -> Result<Vec<u32>> {
    Ok(pose_code(layout)?.1)
}
fn pose_code(layout: &CartridgeLayout) -> Result<(w65c816::AssembledProgram, Vec<u32>)> {
    use crate::sfc::{emit, implied};
    use w65c816::{AddressingMode as A, CpuMode, Mnemonic as M, Operand as O};
    let at = layout.cpu_address(layout.section(POSE_SECTION)?.offset)?;
    let gather = crate::sfc_body_tiles::gather_address(layout)?;
    let bank = u32::from(crate::sfc_body_tiles::STAGE_BANK) << 16;
    let pointer = crate::sfc_body_tiles::GATHER_POINTER;
    const RECORD: u8 = 0xf8;
    const DESTINATION: u8 = 0xfb;
    const COUNT: u8 = 0xfd;
    let mut program = w65c816::Assembler::new();
    let p = &mut program;
    let dp = |p: &mut w65c816::Assembler, m, at: u8| emit(p, m, A::DirectPage, O::Byte(at));
    let m8 = |p: &mut w65c816::Assembler| emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    let m16 = |p: &mut w65c816::Assembler| emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    let imm = |p: &mut w65c816::Assembler, m, v: u16| emit(p, m, A::Immediate, O::Word(v));
    let long = |p: &mut w65c816::Assembler, m, at: u32| emit(p, m, A::AbsoluteLong, O::Long(at));
    let ind_y = |p: &mut w65c816::Assembler, m| {
        emit(p, m, A::DirectPageIndirectLongIndexedY, O::Byte(RECORD));
    };
    for (g, &u) in GATHERED.iter().enumerate() {
        let table =
            layout.cpu_address(layout.section(&format!("b1_portrait_poses_{u}"))?.offset)?;
        let slot = u32::from(super::sfc_b1::QUEUE + 15 + 5 * u as u16);
        let label = |s: &str| format!("portrait_pose_{u}_{s}");
        p.label(label("entry")).assume_mode(CpuMode::NATIVE_M16_X16);
        emit(p, M::Asl, A::Accumulator, O::None);
        emit(p, M::Asl, A::Accumulator, O::None);
        implied(p, M::Tax);
        emit(p, M::Lda, A::AbsoluteLongX, O::Long(table));
        dp(p, M::Sta, RECORD);
        m8(p);
        emit(p, M::Lda, A::AbsoluteLongX, O::Long(table + 2));
        dp(p, M::Sta, RECORD + 2);
        long(p, M::Lda, bank | u32::from(HALVES[g]));
        emit(p, M::Eor, A::Immediate, O::Byte(1));
        long(p, M::Sta, bank | u32::from(HALVES[g]));
        m16(p);
        imm(p, M::And, 1);
        p.emit_label_ref(M::Bne, A::Relative8, label("second"));
        imm(p, M::Lda, BUFFERS[g][0]);
        dp(p, M::Sta, DESTINATION);
        imm(p, M::Ldx, JOBS[g][0]);
        p.emit_label_ref(M::Bra, A::Relative8, label("half"));
        p.label(label("second"))
            .assume_mode(CpuMode::NATIVE_M16_X16);
        imm(p, M::Lda, BUFFERS[g][1]);
        dp(p, M::Sta, DESTINATION);
        imm(p, M::Ldx, JOBS[g][1]);
        p.label(label("half")).assume_mode(CpuMode::NATIVE_M16_X16);
        implied(p, M::Txa);
        long(p, M::Sta, slot);
        m8(p);
        imm8(p, M::Lda, (bank >> 16) as u8);
        long(p, M::Sta, slot + 2);
        m16(p);
        emit(p, M::Lda, A::DirectPageIndirectLong, O::Byte(RECORD));
        imm(p, M::And, 0xff);
        dp(p, M::Sta, COUNT);
        emit(p, M::Ldy, A::Immediate, O::Word(3));
        p.label(label("descriptor"))
            .assume_mode(CpuMode::NATIVE_M16_X16);
        for word in 0..2u16 {
            ind_y(p, M::Lda);
            emit(
                p,
                M::Sta,
                A::AbsoluteLongX,
                O::Long(bank + u32::from(2 * word)),
            );
            implied(p, M::Iny);
            implied(p, M::Iny);
        }
        ind_y(p, M::Lda);
        emit(p, M::Sta, A::AbsoluteLongX, O::Long(bank + 4));
        implied(p, M::Iny);
        ind_y(p, M::Lda);
        implied(p, M::Clc);
        dp(p, M::Adc, DESTINATION);
        emit(p, M::Sta, A::AbsoluteLongX, O::Long(bank + 5));
        implied(p, M::Iny);
        implied(p, M::Iny);
        m8(p);
        imm8(p, M::Lda, (bank >> 16) as u8);
        emit(p, M::Sta, A::AbsoluteLongX, O::Long(bank + 7));
        m16(p);
        implied(p, M::Iny);
        implied(p, M::Txa);
        implied(p, M::Clc);
        imm(p, M::Adc, 8);
        implied(p, M::Tax);
        emit(p, M::Dec, A::DirectPage, O::Byte(COUNT));
        p.emit_label_ref(M::Bne, A::Relative8, label("descriptor"));
        implied(p, M::Tya);
        implied(p, M::Clc);
        dp(p, M::Adc, RECORD);
        dp(p, M::Sta, pointer);
        m8(p);
        dp(p, M::Lda, RECORD + 2);
        dp(p, M::Sta, pointer + 2);
        m16(p);
        emit(p, M::Lda, A::DirectPageIndirectLong, O::Byte(RECORD));
        imm(p, M::And, 0xff);
        implied(p, M::Pha);
        emit(p, M::Ldy, A::Immediate, O::Word(1));
        ind_y(p, M::Lda);
        implied(p, M::Tax);
        dp(p, M::Lda, DESTINATION);
        emit(p, M::Jsl, A::AbsoluteLong, O::Long(gather));
        implied(p, M::Pla);
        long(p, M::Sta, slot + 3);
        implied(p, M::Rtl);
    }
    let program = crate::sfc_width_check::assemble(
        &program,
        w65c816::CodeLocation::new((at >> 16) as u8, at as u16),
        CpuMode::NATIVE_M16_X16,
    )?;
    if program.bytes().len() > POSE_BYTES {
        return Err("Portrait pose code exceeds its section".into());
    }
    let entries = GATHERED
        .iter()
        .map(|u| {
            program
                .label_location(&format!("portrait_pose_{u}_entry"))
                .map(|l| l.to_u32())
                .ok_or_else(|| "Missing portrait pose entry".into())
        })
        .collect::<Result<Vec<_>>>()?;
    Ok((program, entries))
}
fn imm8(p: &mut w65c816::Assembler, m: w65c816::Mnemonic, v: u8) {
    crate::sfc::emit(
        p,
        m,
        w65c816::AddressingMode::Immediate,
        w65c816::Operand::Byte(v),
    );
}
