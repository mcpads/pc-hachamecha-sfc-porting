use crate::{Result, content::Content, sha};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
pub const DIRECTION_MASKS: [u8; 4] = [1, 2, 8, 4];
#[cfg_attr(not(test), allow(dead_code))]
const DELTAS: [(i8, i8); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];
#[cfg_attr(not(test), allow(dead_code))]
pub const GLOBALS: usize = 192;
#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub id: u8,
    pub marker: u8,
    pub x: u8,
    pub y: u8,
    pub direction_mask: u8,
    pub kind: u8,
    pub script: Option<u16>,
    pub script_va: u32,
}
#[derive(Default)]
pub struct Catalogue {
    pub messages: Vec<u32>,
    pub objects: Vec<u32>,
    pub streams: Vec<u32>,
    pub palettes: Vec<u32>,
    pub follow_up: u16,
}
impl Catalogue {
    fn index(list: &mut Vec<u32>, va: u32) -> Result<u16> {
        let at = list.iter().position(|v| *v == va).unwrap_or_else(|| {
            list.push(va);
            list.len() - 1
        });
        Ok(u16::try_from(at)?)
    }
}
pub struct Floor {
    pub id: u8,
    pub events: Vec<Event>,
    pub entry: u16,
    pub exit: u16,
    pub program: Vec<u8>,
    pub sources: BTreeMap<u16, u32>,
    pub unconnected: Vec<(u32, String)>,
    pub ordered_walls: Vec<Vec<u32>>,
    pub pictures: Vec<u32>,
}
impl Floor {
    pub fn messages(&self) -> (BTreeSet<usize>, BTreeSet<usize>) {
        let (mut all, mut signs) = (BTreeSet::new(), BTreeSet::new());
        for &at in self.sources.keys() {
            let raw = &self.program[usize::from(at)..];
            let index = |o: usize| usize::from(u16::from_le_bytes([raw[o], raw[o + 1]]));
            match (raw[0], raw[1]) {
                (0x2f, _) => {
                    all.insert(index(4));
                }
                (0x75, 1) => {
                    all.insert(index(12));
                    signs.insert(index(12));
                }
                _ => {}
            }
        }
        (all, signs)
    }
    pub fn relocated(&self, base: u16) -> Result<(Vec<u8>, Vec<u8>)> {
        let shift = |v: u16| -> Result<u16> {
            v.checked_add(base)
                .ok_or_else(|| "Floor programs exceed one bank".into())
        };
        let mut program = self.program.clone();
        for (at, raw) in self.commands() {
            let o = match raw[0] {
                0x88 => Some(2),
                _ => local_pointer(raw),
            };
            if let Some(o) = o {
                let at = usize::from(at) + o;
                let v = u16::from_le_bytes([program[at], program[at + 1]]);
                if raw[0] == 0x88 && v == 0xffff {
                    continue;
                }
                program[at..at + 2].copy_from_slice(&shift(v)?.to_le_bytes());
            }
        }
        let mut records = Vec::new();
        for e in &self.events {
            let script = match e.script {
                Some(s) => shift(s)?,
                None => 0xffff,
            };
            let [lo, hi] = script.to_le_bytes();
            records.extend([e.x, e.y, e.direction_mask, e.kind, e.id, lo, hi, 0]);
        }
        Ok((program, records))
    }
    fn reached(&self, start: u16, avoid: Option<u16>) -> BTreeSet<u16> {
        let mut seen = BTreeSet::new();
        let mut pending = vec![start];
        while let Some(at) = pending.pop() {
            let Some(raw) = self.command(at) else {
                continue;
            };
            if Some(at) == avoid || !seen.insert(at) {
                continue;
            }
            let next = at + raw.len() as u16;
            match (raw[0], local_pointer(raw)) {
                (0x03, Some(o)) => pending.push(u16::from_le_bytes([raw[o], raw[o + 1]])),
                (0x00 | 0x05, _) => {}
                (_, Some(o)) => pending.extend([next, u16::from_le_bytes([raw[o], raw[o + 1]])]),
                _ => pending.push(next),
            }
        }
        seen
    }
    pub fn creations(&self, start: u16) -> Vec<(u16, u16, Option<u8>)> {
        self.reached(start, None)
            .iter()
            .filter_map(|&at| {
                let raw = self.command(at)?;
                (raw[0] == 0x77).then(|| {
                    let slot = self
                        .command(at + 8)
                        .filter(|next| next[0] == 0x28)
                        .map(|next| next[2]);
                    (at, u16::from_le_bytes([raw[4], raw[5]]), slot)
                })
            })
            .collect()
    }
    pub fn arrivals(&self, start: u16) -> Vec<(u16, u8, [u8; 3])> {
        let reached = self.reached(start, None);
        let mut taken = BTreeSet::new();
        let mut moves: Vec<(u16, u8, [u8; 3])> = reached
            .iter()
            .filter(|&&at| self.command(at).is_some_and(|raw| raw[0] == 0x70))
            .filter_map(|&at| {
                let floor = self.program[usize::from(at) + 4];
                let place = |raw: &[u8]| {
                    let facing = DIRECTION_MASKS.iter().position(|&m| m == raw[3])?;
                    Some([raw[1], raw[2], facing as u8])
                };
                let mut next = at + 8;
                while let Some(raw) = self.command(next) {
                    if raw[0] == 0x78 {
                        taken.insert(next);
                        return Some((at, floor, place(raw)?));
                    }
                    if matches!(raw[0], 0x00 | 0x03 | 0x05) {
                        break;
                    }
                    next += raw.len() as u16;
                }
                let before = at.checked_sub(4)?;
                if self.sources.get(&before)? + 4 != self.sources[&at] {
                    return None;
                }
                let raw = self.command(before).filter(|raw| raw[0] == 0x78)?;
                taken.insert(before);
                Some((at, floor, place(raw)?))
            })
            .collect();
        for &at in &reached {
            let Some(raw) = self.command(at).filter(|raw| raw[0] == 0x78) else {
                continue;
            };
            if taken.contains(&at) {
                continue;
            }
            let facing = match raw[3] {
                0 | 15 => KEEP_FACING,
                mask => match DIRECTION_MASKS.iter().position(|&m| m == mask) {
                    Some(d) => d as u8,
                    None => continue,
                },
            };
            moves.push((at, self.id, [raw[1], raw[2], facing]));
        }
        moves
    }
    pub fn slid_slots(&self, start: u16) -> Vec<(u8, usize)> {
        let reached = self.reached(start, None);
        let axis = usize::from(reached.iter().any(|&at| {
            self.command(at)
                .is_some_and(|raw| raw[..3] == [0x12, 0xc1, 0x20])
        }));
        reached
            .iter()
            .filter_map(|&at| self.command(at).filter(|raw| raw[..2] == [0x28, 1]))
            .map(|raw| (raw[2], axis))
            .collect()
    }
    pub fn dominates(&self, start: u16, a: u16, b: u16) -> bool {
        a == start || !self.reached(start, Some(a)).contains(&b)
    }
    pub fn leads_to(&self, a: u16, b: u16) -> bool {
        let mut seen = BTreeSet::new();
        let mut pending = vec![a];
        while let Some(at) = pending.pop() {
            let Some(raw) = self.command(at) else {
                continue;
            };
            if at == b && at != a {
                return true;
            }
            if !seen.insert(at) || (at != a && matches!(raw[0], 0x01 | 0x29 | 0x75)) {
                continue;
            }
            let next = at + raw.len() as u16;
            match (raw[0], local_pointer(raw)) {
                (0x03, Some(o)) => pending.push(u16::from_le_bytes([raw[o], raw[o + 1]])),
                (0x00 | 0x05, _) => {}
                (_, Some(o)) => pending.extend([next, u16::from_le_bytes([raw[o], raw[o + 1]])]),
                _ => pending.push(next),
            }
        }
        false
    }
    fn command(&self, at: u16) -> Option<&[u8]> {
        self.commands().find(|(o, _)| *o == at).map(|(_, raw)| raw)
    }
    pub fn references(&self) -> BTreeMap<u16, u8> {
        let lengths: BTreeMap<u16, usize> =
            self.commands().map(|(at, raw)| (at, raw.len())).collect();
        let mut seen: BTreeMap<u16, u8> = BTreeMap::new();
        let roots = self.events.iter().filter_map(|e| e.script);
        let mut pending: Vec<(u16, u8)> = roots
            .chain([self.entry, self.exit])
            .map(|at| (at, 1))
            .collect();
        while let Some((at, state)) = pending.pop() {
            let Some(raw) = self.command(at) else {
                continue;
            };
            let known = seen.entry(at).or_insert(0);
            if *known & state == state {
                continue;
            }
            *known |= state;
            let after = match (raw[0], raw[1]) {
                (0x6f, _) => 2,
                (0x4b, _) => 1,
                (0x4c, _) => 8,
                (0x28, 1) => 4,
                _ => state,
            };
            let next = at + lengths[&at] as u16;
            match raw[0] {
                0x00 | 0x05 => {}
                0x03 => pending.push((self.word16(at, 4), after)),
                _ => {
                    pending.push((next, after));
                    if let Some(o) = local_pointer(raw) {
                        pending.push((self.word16(at, o), after));
                    }
                }
            }
        }
        seen
    }
    fn word16(&self, at: u16, offset: usize) -> u16 {
        let at = usize::from(at) + offset;
        u16::from_le_bytes([self.program[at], self.program[at + 1]])
    }
    pub fn commands(&self) -> impl Iterator<Item = (u16, &[u8])> {
        let offsets: Vec<u16> = self.sources.keys().copied().collect();
        let end = self.program.len();
        (0..offsets.len()).map(move |i| {
            let start = usize::from(offsets[i]);
            let stop = offsets.get(i + 1).map_or(end, |o| usize::from(*o));
            (offsets[i], &self.program[start..stop])
        })
    }
}
struct Source {
    va: u32,
    raw: Vec<u8>,
}
fn pointer(raw: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(raw[at..at + 4].try_into().unwrap())
}
fn local_pointer(raw: &[u8]) -> Option<usize> {
    match (raw[0], raw[1]) {
        (0x03 | 0x04 | 0x06 | 0x13 | 0x32 | 0x74, _) | (0x7a | 0x96, 1) => Some(4),
        (0x14 | 0x15, _) => Some(raw.len() - 4),
        (0x75, 0) => Some(12),
        _ => None,
    }
}
fn successors(c: &Source) -> (Vec<u32>, Vec<u32>) {
    let (op, sub, next) = (c.raw[0], c.raw[1], c.va + c.raw.len() as u32);
    match op {
        0x03 => return (vec![pointer(&c.raw, 4)], vec![]),
        0x00 | 0x05 => return (vec![], vec![]),
        _ => {}
    }
    let mut targets = vec![next];
    match op {
        0x04 | 0x06 | 0x13 | 0x32 | 0x74 => targets.push(pointer(&c.raw, 4)),
        0x14 | 0x15 => targets.push(pointer(&c.raw, c.raw.len() - 4)),
        0x75 if sub == 0 => targets.push(pointer(&c.raw, 12)),
        0x7a | 0x96 if sub == 1 => targets.push(pointer(&c.raw, 4)),
        0x07 | 0x77 => return (targets, vec![pointer(&c.raw, 4)]),
        _ => {}
    }
    (targets, vec![])
}
fn unsupported(raw: &[u8]) -> Option<String> {
    let (op, sub) = (raw[0], raw[1]);
    let ok = match op {
        0x01 | 0x03 | 0x05 | 0x06 | 0x29 | 0x32 | 0x34 | 0x3b | 0x3f | 0x4b | 0x6f | 0x77 => true,
        0x3a | 0x3c => true,
        0x4c => sub == 0,
        0x86 | 0x44 | 0x9b => sub == 0,
        0x35 => sub < 2,
        0x8b => sub == 1 && raw[2] == 4,
        0x02 | 0x04 | 0x40 | 0x8c => true,
        0x69 => raw[4..8] != [0, 0, 0, 0],
        0x83 => true,
        0x8f => true,
        0x88 => {
            sub == 0 && [PHANTOM_SCENE, crate::sfc_ending::DESCRIPTOR].contains(&pointer(raw, 4))
        }
        0x07 => true,
        0x10..=0x12 => matches!(sub & 0xf0, 0x40 | 0xc0) && sub & 0x0f < 3,
        0x13 => matches!(sub & 0xc0, 0x40 | 0xc0) && sub & 0x30 == 0 && sub & 0x0f < 6,
        0x14 => matches!(sub & 0xc0, 0x40 | 0xc0) && sub & 0x30 == 0 && sub & 0x0f < 2,
        0x15 => sub & 0xf0 == 0x40 && sub & 0x0f < 2,
        0x24 => sub == 1,
        0x25 => sub < 2 || sub == 0x81 || sub == 0xf1,
        0x27 => sub == 0x22 || sub < 2,
        0x96 => sub < 2,
        0x26 => sub == 0x30,
        0x28 => sub < 2,
        0x2f => sub == 1,
        0x31 => sub < 2,
        0x38 => matches!(sub, 1 | 2),
        0x39 => sub < 2 || sub == 3,
        0x70 => sub == 0,
        0x75 => sub < 2,
        0x78 => matches!(raw[3], 0 | 1 | 2 | 4 | 8 | 15 | 16),
        0x7a | 0x7b | 0x81 => sub < 2,
        0x7e => sub < 3,
        0x93 => sub < 2,
        0x84 => sub < 64 && matches!(raw[2] & 0x7f, 1 | 2 | 4 | 8),
        0x85 => sub == 1 && raw[2] < 8 && raw[3] < 8,
        0x97 => true,
        _ => false,
    };
    (!ok).then(|| format!("{op:02x}/{sub:02x}"))
}
const PHANTOM_SCENE: u32 = 0x47d710;
const SCENE_TAIL: [(u32, u32); 2] = [(0x44473c, 0x4c0418), (0x444744, 0x4c0420)];
const SCENE_TAIL_END: u32 = 0x444748;
pub fn translate(
    world: &Value,
    events: &Value,
    map: u8,
    catalogue: &mut Catalogue,
    dead: &BTreeSet<(u32, u32)>,
) -> Result<Floor> {
    let commands: BTreeMap<u32, Source> = events["commands"]
        .as_array()
        .ok_or("Missing world commands")?
        .iter()
        .map(|c| -> Result<(u32, Source)> {
            let va = u32::try_from(c["va"].as_u64().ok_or("Missing command address")?)?;
            let hex = c["raw_hex"].as_str().ok_or("Missing command bytes")?;
            let raw = (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16))
                .collect::<std::result::Result<Vec<u8>, _>>()?;
            Ok((va, Source { va, raw }))
        })
        .collect::<Result<_>>()?;
    let maps = world["maps"].as_array().ok_or("Missing world maps")?;
    let source = maps
        .iter()
        .find(|m| m["id"] == map)
        .ok_or("Missing world map")?;
    let descriptors: BTreeMap<u64, u8> = maps
        .iter()
        .map(|m| {
            (
                m["descriptor_va"].as_u64().unwrap_or(0),
                m["id"].as_u64().unwrap_or(0) as u8,
            )
        })
        .collect();
    let va =
        |v: &Value| -> Result<u32> { Ok(u32::try_from(v.as_u64().ok_or("Missing address")?)?) };
    let entry_va = va(&source["entry_script_va"])?;
    let exit_va = va(&source["exit_script_va"])?;
    let records = source["events"].as_array().ok_or("Missing map events")?;
    let reach = |root: u32| -> Result<(BTreeSet<u32>, Vec<u32>)> {
        let once = root == entry_va || root == exit_va;
        let (mut seen, mut objects, mut pending) = (BTreeSet::new(), Vec::new(), vec![root]);
        while let Some(at) = pending.pop() {
            if !seen.insert(at) {
                continue;
            }
            let c = commands
                .get(&at)
                .ok_or_else(|| format!("Event path leaves decoded commands at {at:#x}"))?;
            let (next, spawned) = successors(c);
            if !(once && c.raw[0] == 0x01) {
                pending.extend(next.into_iter().filter(|t| !dead.contains(&(at, *t))));
            }
            objects.extend(spawned);
        }
        Ok((seen, objects))
    };
    let mut roots = vec![entry_va, exit_va];
    roots.extend(
        records
            .iter()
            .map(|e| va(&e["script_va"]))
            .collect::<Result<Vec<_>>>()?,
    );
    let mut scope = BTreeSet::new();
    let mut blocked = BTreeMap::new();
    for root in &roots {
        let (seen, objects) = reach(*root)?;
        let failures: Vec<String> = seen
            .iter()
            .filter_map(|at| unsupported(&commands[at].raw).map(|why| format!("{at:#x} {why}")))
            .collect();
        if failures.is_empty() {
            scope.extend(seen);
            for object in objects {
                Catalogue::index(&mut catalogue.objects, object)?;
            }
        } else {
            blocked.insert(*root, failures.join(", "));
        }
    }
    if blocked.contains_key(&entry_va) || blocked.contains_key(&exit_va) {
        return Err(format!("Map {map} entry or exit is not connected: {blocked:?}").into());
    }
    let mut offsets = BTreeMap::new();
    let mut program = Vec::new();
    for at in &scope {
        offsets.insert(*at, u16::try_from(program.len())?);
        program.extend(&commands[at].raw);
    }
    let offset = |target: u32| -> Result<[u8; 4]> {
        let o = offsets
            .get(&target)
            .ok_or_else(|| format!("Branch target {target:#x} outside the event program"))?;
        Ok([*o as u8, (*o >> 8) as u8, 0, 0])
    };
    let mut sources = BTreeMap::new();
    let mut pictures = Vec::new();
    for at in &scope {
        let start = usize::from(offsets[at]);
        let raw = &commands[at].raw;
        let bytes = &mut program[start..start + raw.len()];
        let mut put = |at: usize, value: [u8; 4]| bytes[at..at + 4].copy_from_slice(&value);
        match (local_pointer(raw), raw[0]) {
            (Some(o), _) => {
                let target = pointer(raw, o);
                let target = if dead.contains(&(*at, target)) {
                    *at + raw.len() as u32
                } else {
                    target
                };
                put(o, offset(target)?)
            }
            (None, 0x75) => {
                let i = Catalogue::index(&mut catalogue.messages, pointer(raw, 12))?;
                put(12, [i as u8, (i >> 8) as u8, 0, 0]);
            }
            (None, 0x2f) => {
                let i = Catalogue::index(&mut catalogue.messages, pointer(raw, 4))?;
                put(4, [i as u8, (i >> 8) as u8, 0, 0]);
            }
            (None, 0x07) => {
                let i = Catalogue::index(&mut catalogue.objects, pointer(raw, 4))?;
                put(4, [i as u8, (i >> 8) as u8, 0, 0]);
            }
            (None, 0x77) => {
                let next = *at + raw.len() as u32;
                let key = match commands.get(&next) {
                    Some(c) if c.raw[..2] == [0x39, 3] => next,
                    _ => pointer(raw, 4),
                };
                let i = Catalogue::index(&mut pictures, key)?;
                put(4, [i as u8, (i >> 8) as u8, 0, 0]);
            }
            (None, 0x39) if raw[1] == 3 => {
                let made = commands
                    .range(..*at)
                    .next_back()
                    .filter(|(va, c)| **va + c.raw.len() as u32 == *at && c.raw[0] == 0x77);
                if made.is_none() {
                    return Err(format!("Target colours at {at:#x} follow no picture").into());
                }
            }
            (None, 0x3f) => {
                let i = Catalogue::index(&mut catalogue.streams, pointer(raw, 4))?;
                put(4, [i as u8, (i >> 8) as u8, 0, 0]);
            }
            (None, 0x39) if raw[1] == 0 => {
                let i = Catalogue::index(&mut catalogue.palettes, pointer(raw, 4))?;
                put(4, [i as u8, (i >> 8) as u8, 0, 0]);
            }
            (None, 0x70) => {
                let target = descriptors
                    .get(&u64::from(pointer(raw, 4)))
                    .ok_or("Map change to an unknown descriptor")?;
                put(4, [*target, 0, 0, 0]);
            }
            (None, 0x26) => put(4, [0; 4]),
            _ => {}
        }
        sources.insert(offsets[at], *at);
    }
    for at in &scope {
        if commands[at].raw[0] != 0x88 {
            continue;
        }
        if pointer(&commands[at].raw, 4) == crate::sfc_ending::DESCRIPTOR {
            let o = usize::from(offsets[at]);
            program[o + 1] = 1;
            program[o + 2..o + 4].copy_from_slice(&0xffffu16.to_le_bytes());
            continue;
        }
        let tail = u16::try_from(program.len())?;
        for (caller, event) in SCENE_TAIL {
            let start = usize::from(
                *offsets
                    .get(&event)
                    .ok_or("Scene caller's command outside the event program")?,
            );
            let raw = &commands[&event].raw;
            sources.insert(u16::try_from(program.len())?, caller);
            let bytes = program[start..start + raw.len()].to_vec();
            program.extend(bytes);
        }
        if program[usize::from(tail)] != 0x70 || program[usize::from(tail) + 8] != 0x78 {
            return Err("Scene caller's commands are not a map load and a move".into());
        }
        sources.insert(u16::try_from(program.len())?, SCENE_TAIL_END);
        program.extend([0x05, 0, 0, 0]);
        let o = usize::from(offsets[at]);
        program[o + 2..o + 4].copy_from_slice(&tail.to_le_bytes());
    }
    let events = records
        .iter()
        .map(|e| -> Result<Event> {
            let script_va = va(&e["script_va"])?;
            let byte = |k: &str| -> Result<u8> {
                Ok(u8::try_from(e[k].as_u64().ok_or("Missing event field")?)?)
            };
            Ok(Event {
                id: byte("id")?,
                marker: byte("unknown_byte")?,
                x: byte("x")?,
                y: byte("y")?,
                direction_mask: byte("direction_mask")?,
                kind: byte("kind")?,
                script: (!blocked.contains_key(&script_va)).then(|| offsets[&script_va]),
                script_va,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Floor {
        id: map,
        events,
        entry: offsets[&entry_va],
        exit: offsets[&exit_va],
        program,
        sources,
        unconnected: blocked.into_iter().collect(),
        ordered_walls: Vec::new(),
        pictures,
    })
}
pub const FOLLOW_UP_MESSAGE: u32 = 0x4b02a0;
pub const KEEP_FACING: u8 = 0xff;
pub const FLOORS: [u8; 16] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
pub fn load(content: &Content) -> Result<(Vec<Floor>, Catalogue)> {
    let read = |name: &str| -> Result<Value> { content.json(name) };
    let (world, events) = (read("world.json")?, read("world-events.json")?);
    let guard = &read("phantom-return.json")?["boundary_guard"];
    let va =
        |k: &str| -> Result<u32> { Ok(u32::try_from(guard[k].as_u64().ok_or("Boundary guard")?)?) };
    let dead = BTreeSet::from([(va("second_predicate_va")?, va("contradictory_target_va")?)]);
    let mut catalogue = Catalogue::default();
    let mut floors: Vec<Floor> = FLOORS
        .iter()
        .map(|map| translate(&world, &events, *map, &mut catalogue, &dead))
        .collect::<Result<_>>()?;
    let prisms = read("prisms.json")?;
    let vas = |walls: &Value| -> Result<Vec<u32>> {
        walls
            .as_array()
            .ok_or("Light walls")?
            .iter()
            .map(|w| Ok(u32::try_from(w["va"].as_u64().ok_or("Light wall")?)?))
            .collect()
    };
    let mut ordered = vec![vas(&prisms["base_walls"])?];
    for segment in prisms["segments"].as_array().ok_or("Light segments")? {
        ordered.push(vas(&segment["walls"])?);
    }
    let map = u8::try_from(prisms["map_id"].as_u64().ok_or("Light map")?)?;
    if let Some(floor) = floors.iter_mut().find(|f| f.id == map) {
        floor.ordered_walls = ordered;
    }
    catalogue.follow_up = Catalogue::index(&mut catalogue.messages, FOLLOW_UP_MESSAGE)?;
    Ok((floors, catalogue))
}
fn census(content: &Content) -> Result<Value> {
    let read = |name: &str| -> Result<Value> { content.json(name) };
    let (world, events) = (read("world.json")?, read("world-events.json")?);
    let mut commands = BTreeMap::new();
    for c in events["commands"]
        .as_array()
        .ok_or("Missing world commands")?
    {
        let va = u32::try_from(c["va"].as_u64().ok_or("Command address")?)?;
        let hex = c["raw_hex"].as_str().ok_or("Command bytes")?;
        let raw = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16))
            .collect::<std::result::Result<Vec<u8>, _>>()?;
        commands.insert(va, Source { va, raw });
    }
    let walk = |roots: &[u32]| -> (BTreeSet<u32>, BTreeSet<u32>) {
        let (mut seen, mut spawned, mut pending) =
            (BTreeSet::new(), BTreeSet::new(), roots.to_vec());
        while let Some(at) = pending.pop() {
            let Some(c) = commands.get(&at) else { continue };
            if !seen.insert(at) {
                continue;
            }
            let (next, objects) = successors(c);
            pending.extend(next);
            spawned.extend(objects);
        }
        (seen, spawned)
    };
    let mut maps = Vec::new();
    let mut objects_all = BTreeSet::new();
    for map in world["maps"].as_array().ok_or("Missing world maps")? {
        let id = map["id"].as_u64().ok_or("Map id")? as u8;
        if !FLOORS.contains(&id) {
            continue;
        }
        let va = |v: &Value| v.as_u64().unwrap_or(0) as u32;
        let mut roots = vec![va(&map["entry_script_va"]), va(&map["exit_script_va"])];
        roots.extend(
            map["events"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|e| va(&e["script_va"])),
        );
        let (seen, spawned) = walk(&roots);
        let rejected: Vec<Value> = seen
            .iter()
            .filter_map(|at| {
                unsupported(&commands[at].raw)
                    .map(|why| serde_json::json!({ "va" : at, "command" : why }))
            })
            .collect();
        objects_all.extend(spawned.iter().copied());
        maps.push(serde_json::json!(
            { "map" : id, "reached" : seen.len(), "rejected" : rejected, "objects" :
            spawned }
        ));
    }
    let (object_commands, _) = walk(&objects_all.iter().copied().collect::<Vec<_>>());
    let mut ops: BTreeMap<String, usize> = BTreeMap::new();
    for at in &object_commands {
        let raw = &commands[at].raw;
        *ops.entry(format!("{:02x}/{:02x}", raw[0], raw[1]))
            .or_default() += 1;
    }
    Ok(serde_json::json!(
        { "maps" : maps, "object_roots" : objects_all.len(), "object_commands" :
        object_commands.len(), "object_ops" : ops }
    ))
}
pub fn compile(content: &Content) -> Result<Vec<(String, Vec<u8>)>> {
    let (floors, catalogue) = load(content)?;
    let report = serde_json::json!(
        { "floors" : floors.iter().map(| f | serde_json::json!({ "map" : f.id,
        "program_bytes" : f.program.len(), "program_sha256" : sha(& f.program), "entry" :
        f.entry, "exit" : f.exit, "commands" : f.sources.len(), "events" : f.events
        .len(), "connected_events" : f.events.iter().filter(| e | e.script.is_some())
        .count(), "unconnected" : f.unconnected.iter().map(| (va, why) |
        serde_json::json!({ "root_va" : va, "commands" : why })).collect::< Vec < _ >>
        (), "event_records" : f.events.iter().map(| e | serde_json::json!({ "id" : e.id,
        "x" : e.x, "y" : e.y, "direction_mask" : e.direction_mask, "kind" : e.kind,
        "script_va" : e.script_va, "connected" : e.script.is_some(), })).collect::< Vec <
        _ >> (), "translated_vas" : f.sources.values().collect::< BTreeSet < _ >> (), }))
        .collect::< Vec < _ >> (), "census" : census(content) ?, "messages" : catalogue
        .messages.len(), "objects" : catalogue.objects.len(), "streams" : catalogue
        .streams.len(), "limits" :
        ["The ROM runs the events of maps 1 to 16 (sfc_field_events); build lists what stays unconnected.",
        "Object scripts are catalogued, not executed; their pictures come from separate composition.",],
        }
    );
    Ok(vec![(
        "floor-events.json".into(),
        serde_json::to_vec_pretty(&report)?,
    )])
}
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub enum Effect {
    Message(u16),
    Flag(u16, bool),
    Spawn(u16),
    Register(u8),
    Delete(u8),
    Music(u8),
    MusicStop,
    SoundStop,
    Sound(u8, u8),
    Palette(Vec<u8>),
    Stream(u16),
    Place { x: u8, y: u8, direction: u8 },
    MapChange(u8),
    Block(u8),
    Item(u8, u8),
    Wall(u8, u8, u8),
    Visit(u8, u8),
    Marker(u8, u8),
    Recover,
    Battle(u8),
    HealingWait(u8),
}
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub enum Reference {
    Input,
    Game,
    Picture,
    Party,
}
#[cfg_attr(not(test), allow(dead_code))]
pub struct Machine<'a> {
    pub floor: &'a Floor,
    pub globals: [u8; GLOBALS],
    pub game: [u8; 0x100],
    pub input: [u8; 0x90],
    pub picture: [u8; 0x30],
    pub reference: Reference,
    pub context: bool,
    pub selected: Option<u16>,
    pub event_id: u8,
    pub origin: (u8, u8, u8),
    pub kind: u8,
    pub created: Option<u16>,
    pub slots: [Option<u16>; 256],
    pc: Option<u16>,
    stack: Vec<Option<u16>>,
    loop_target: Option<u16>,
    loop_count: u16,
    wait: u16,
    clicked: bool,
    pub effects: Vec<Effect>,
}
#[cfg_attr(not(test), allow(dead_code))]
pub const GAME_X: usize = 0x21;
#[cfg_attr(not(test), allow(dead_code))]
pub const GAME_Y: usize = 0x22;
#[cfg_attr(not(test), allow(dead_code))]
pub const GAME_DIRECTION: usize = 0x23;
#[cfg_attr(not(test), allow(dead_code))]
impl<'a> Machine<'a> {
    pub fn new(floor: &'a Floor) -> Self {
        Self {
            floor,
            globals: [0; GLOBALS],
            game: [0; 0x100],
            input: [0; 0x90],
            picture: [0; 0x30],
            reference: Reference::Input,
            context: false,
            selected: None,
            event_id: 255,
            origin: (0, 0, 0),
            kind: 0,
            created: None,
            slots: [None; 256],
            pc: None,
            stack: Vec::new(),
            loop_target: None,
            loop_count: 0,
            wait: 0,
            clicked: false,
            effects: Vec::new(),
        }
    }
    pub fn flag(&self, index: u16) -> bool {
        self.globals[usize::from(index >> 3)] & (1 << (index & 7)) != 0
    }
    fn set_flag(&mut self, index: u16, value: bool) {
        let byte = &mut self.globals[usize::from(index >> 3)];
        *byte = (*byte & !(1 << (index & 7))) | (u8::from(value) << (index & 7));
    }
    pub fn place(&mut self, x: u8, y: u8, direction: u8) {
        self.game[GAME_X] = x;
        self.game[GAME_Y] = y;
        self.game[GAME_DIRECTION] = direction;
    }
    pub fn position(&self) -> (u8, u8, u8) {
        (
            self.game[GAME_X],
            self.game[GAME_Y],
            self.game[GAME_DIRECTION],
        )
    }
    pub fn select(&mut self) {
        let (x, y, direction) = self.position();
        let mask = DIRECTION_MASKS[usize::from(direction)];
        let found = self
            .floor
            .events
            .iter()
            .find(|e| e.x == x && e.y == y && e.direction_mask & mask != 0);
        self.kind = 0;
        let Some(e) = found else {
            self.event_id = 255;
            self.selected = None;
            self.context = false;
            return;
        };
        self.kind = e.kind;
        if e.id != self.event_id {
            self.event_id = e.id;
            self.selected = e.script;
            self.origin = (x, y, direction);
            self.context = false;
        } else if e.direction_mask == 15 && self.origin.2 != direction {
            self.kind = 0;
            self.selected = None;
            self.context = false;
        } else if self.kind > 1 {
            self.kind = 0;
        }
    }
    pub fn frame(
        &mut self,
        input: &mut dyn FnMut(&mut Self, Prompt) -> Option<u8>,
    ) -> Result<bool> {
        self.clicked = input(self, Prompt::Click).is_some();
        if self.pc.is_none() {
            self.select();
            let Some(script) = self.selected else {
                return Ok(false);
            };
            match self.kind {
                1 => self.call(script),
                2 => {
                    self.context = true;
                    self.call(script);
                }
                _ => return Ok(false),
            }
        }
        self.run(input)
    }
    fn call(&mut self, script: u16) {
        self.stack.push(None);
        self.pc = Some(script);
    }
    fn word(&self, at: u16, width: usize) -> u32 {
        let at = usize::from(at);
        let mut bytes = [0; 4];
        for (i, byte) in bytes[..width].iter_mut().enumerate() {
            *byte = self.floor.program.get(at + i).copied().unwrap_or(0);
        }
        u32::from_le_bytes(bytes)
    }
    fn referenced(&mut self) -> &mut [u8] {
        match self.reference {
            Reference::Input => &mut self.input,
            Reference::Game => &mut self.game[0x20..],
            Reference::Picture => &mut self.picture,
            Reference::Party => &mut self.game[0xa8..],
        }
    }
    fn referenced_word(&mut self, at: u8) -> u32 {
        let at = usize::from(at);
        let r = self.referenced();
        u32::from(u16::from_le_bytes([r[at], r[at + 1]]))
    }
    fn global(&self, index: u8) -> u32 {
        let at = usize::from(index) * 4;
        u32::from_le_bytes(self.globals[at..at + 4].try_into().unwrap())
    }
    fn run(&mut self, input: &mut dyn FnMut(&mut Self, Prompt) -> Option<u8>) -> Result<bool> {
        for _ in 0..10_000 {
            let Some(pc) = self.pc else {
                return Ok(false);
            };
            let at = usize::from(pc);
            let raw = self
                .floor
                .program
                .get(at..at + 4)
                .ok_or("PC outside floor program")?;
            let (op, sub, b2, b3) = (raw[0], raw[1], raw[2], raw[3]);
            let target_value = self.word(pc + 4, 2) as u16;
            let target = || target_value;
            let mut next = pc + 4;
            match op {
                0x01 => {
                    self.pc = Some(pc + 4);
                    return Ok(true);
                }
                0x03 => next = target(),
                0x02 => {
                    let arg = self.word(pc + 2, 2) as u16;
                    if self.wait == 0 {
                        self.wait = arg;
                        return Ok(true);
                    }
                    self.wait -= 1;
                    if self.wait != 0 {
                        return Ok(true);
                    }
                }
                0x04 => {
                    self.stack.push(Some(pc + 8));
                    next = target();
                }
                0x40 => {}
                0x8c => self.effects.push(Effect::Recover),
                0x83 => self.effects.push(Effect::Battle(sub)),
                0x05 => match self.stack.pop().ok_or("Return without a caller")? {
                    Some(back) => next = back,
                    None => {
                        self.pc = None;
                        return Ok(false);
                    }
                },
                0x06 if self.loop_target != Some(target()) => {
                    self.loop_target = Some(target());
                    self.loop_count = self.word(pc + 2, 2) as u16;
                    next = pc;
                }
                0x06 => {
                    self.loop_count = self.loop_count.wrapping_sub(1);
                    if self.loop_count == 0 {
                        self.loop_target = None;
                        next = pc + 8;
                    } else {
                        next = target();
                    }
                }
                0x07 | 0x77 => {
                    let object = target();
                    self.created = Some(object);
                    self.effects.push(Effect::Spawn(object));
                    next = pc + 8;
                }
                0x10..=0x12 => {
                    let width = [1, 2, 4][usize::from(op - 0x10)];
                    let value = if op == 0x10 {
                        u32::from(b3)
                    } else {
                        self.word(pc + 4, width)
                    };
                    let at = usize::from(b2);
                    let old = if sub & 0xc0 == 0x40 {
                        self.global(b2)
                    } else {
                        let mut bytes = [0; 4];
                        bytes[..width].copy_from_slice(&self.referenced()[at..at + width]);
                        u32::from_le_bytes(bytes)
                    };
                    let new = match sub & 0x0f {
                        0 => value,
                        1 => old.wrapping_add(value),
                        _ => old.wrapping_sub(value),
                    }
                    .to_le_bytes();
                    if sub & 0xc0 == 0x40 {
                        self.globals[at * 4..at * 4 + width].copy_from_slice(&new[..width]);
                    } else {
                        self.referenced()[at..at + width].copy_from_slice(&new[..width]);
                    }
                    if op != 0x10 {
                        next = pc + 8;
                    }
                }
                0x13..=0x15 => {
                    let left = match sub & 0xc0 {
                        0x40 => self.global(b2),
                        _ if op == 0x13 => u32::from(self.referenced()[usize::from(b2)]),
                        _ => self.referenced_word(b2),
                    };
                    let (right, branch_at, length) = match op {
                        0x13 => (u32::from(b3), pc + 4, 8),
                        0x14 => (self.word(pc + 4, 2), pc + 8, 12),
                        _ => (self.word(pc + 4, 4), pc + 8, 12),
                    };
                    let taken = match sub & 0x0f {
                        0 => left != right,
                        1 => left == right,
                        2 => left > right,
                        3 => left < right,
                        4 => left >= right,
                        _ => left <= right,
                    };
                    next = if taken {
                        self.word(branch_at, 2) as u16
                    } else {
                        pc + length
                    };
                }
                0x24 => {
                    self.effects.push(Effect::Sound(b2, b3));
                }
                0x25 if sub == 0 => self.effects.push(Effect::MusicStop),
                0x25 => self.effects.push(Effect::SoundStop),
                0x26 => {
                    self.effects.push(Effect::Music(b3));
                    next = pc + 8;
                }
                0x28 if sub == 1 => self.reference = Reference::Picture,
                0x28 => {
                    self.slots[usize::from(b2)] = self.created;
                    self.effects.push(Effect::Register(b2));
                }
                0x29 => {
                    if self.slots[usize::from(sub)].take().is_some() {
                        self.effects.push(Effect::Delete(sub));
                    }
                }
                0x2f => {
                    let id = target();
                    self.effects.push(Effect::Message(id));
                    if let Some(answer) = input(self, Prompt::Message(id)) {
                        self.input[0x1d] = answer;
                    }
                    self.pc = Some(pc + 8);
                    return Ok(true);
                }
                0x31 => {
                    let index = self.word(pc + 2, 2) as u16;
                    self.set_flag(index, sub == 1);
                    self.effects.push(Effect::Flag(index, sub == 1));
                }
                0x32 => {
                    let index = self.word(pc + 2, 2) as u16;
                    next = if self.flag(index) == (sub == 1) {
                        target()
                    } else {
                        pc + 8
                    };
                }
                0x34 => {}
                0x38 | 0x39 => {
                    self.effects
                        .push(Effect::Palette(self.floor.program[at..at + 8].to_vec()));
                    next = pc + 8;
                }
                0x3a..=0x3c => self.effects.push(Effect::Palette(raw.to_vec())),
                0x3f => {
                    self.effects.push(Effect::Stream(target()));
                    next = pc + 8;
                }
                0x4b => self.reference = Reference::Input,
                0x4c => self.reference = Reference::Party,
                0x6f => self.reference = Reference::Game,
                0x70 => {
                    self.effects
                        .push(Effect::MapChange(self.floor.program[at + 4]));
                    next = pc + 8;
                }
                0x74 => next = pc + 8,
                0x75 => {
                    next = pc + 16;
                    if self.clicked {
                        let to = self.word(pc + 12, 2) as u16;
                        if sub == 0 {
                            next = to;
                        } else {
                            self.effects.push(Effect::Message(to));
                            if let Some(answer) = input(self, Prompt::Message(to)) {
                                self.input[0x1d] = answer;
                            }
                            self.pc = Some(next);
                            return Ok(true);
                        }
                    }
                }
                0x78 => {
                    let mut direction = self.game[GAME_DIRECTION];
                    direction = match b3 {
                        1 => 0,
                        2 => 1,
                        4 => 3,
                        8 => 2,
                        16 => (direction + 2) & 3,
                        _ => direction,
                    };
                    self.game[GAME_DIRECTION] = direction;
                    if b3 == 16 {
                        let (dx, dy) = DELTAS[usize::from(direction)];
                        self.game[GAME_X] = self.game[GAME_X].wrapping_add_signed(dx);
                        self.game[GAME_Y] = self.game[GAME_Y].wrapping_add_signed(dy);
                    } else {
                        self.game[GAME_X] = sub;
                        self.game[GAME_Y] = b2;
                    }
                    let (x, y, direction) = self.position();
                    self.effects.push(Effect::Place { x, y, direction });
                    self.effects.push(Effect::Block(0));
                }
                0x7a => {
                    if sub == 0 {
                        self.selected = Some(pc + 4);
                    } else {
                        self.selected = Some(target());
                        next = pc + 8;
                    }
                }
                0x7b => self.context = sub == 0,
                0x7e => self.effects.push(Effect::Block(sub)),
                0x93 => self.effects.push(Effect::HealingWait(sub)),
                0x27 => {}
                0x86 | 0x44 | 0x9b | 0x35 => {}
                0x8b => {}
                0x84 => self.effects.push(Effect::Wall(sub, b2, b3)),
                0x85 => self.effects.push(Effect::Visit(b2, b3)),
                0x97 => self.effects.push(Effect::Marker(sub, b2)),
                0x81 => {
                    let item = &mut self.game[usize::from(b2)];
                    *item = if sub == 0 {
                        item.saturating_add(1)
                    } else {
                        item.saturating_sub(1)
                    };
                    self.effects.push(Effect::Item(b2, *item));
                }
                _ => {
                    return Err(format!("Floor program opcode {op:#x} at {pc:#x}").into());
                }
            }
            self.pc = Some(next);
        }
        Err("Floor program did not yield".into())
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub enum Prompt {
    Message(u16),
    Click,
}
#[derive(Clone, Debug)]
pub struct WallChange {
    pub cell: u8,
    pub facing: u8,
    pub both: bool,
    pub values: Vec<u8>,
}
pub type WallSide = (u8, u8, bool);
#[derive(Clone, Debug)]
pub struct WallChoice {
    pub alternatives: Vec<Vec<(WallSide, u8)>>,
}
fn wall_side(raw: &[u8]) -> WallSide {
    let facing = match raw[2] & 0x7f {
        1 => 0,
        2 => 1,
        8 => 2,
        _ => 3,
    };
    (raw[1], facing, raw[2] & 0x80 != 0)
}
impl Floor {
    pub fn wall_choices(&self) -> Vec<WallChoice> {
        let ordered: BTreeSet<u32> = self.ordered_walls.iter().flatten().copied().collect();
        let mut prefixes = Vec::new();
        let mut writes: Vec<(WallSide, u8)> = Vec::new();
        for stage in &self.ordered_walls {
            for (at, raw) in self.commands() {
                if self.sources.get(&at).is_some_and(|va| stage.contains(va)) {
                    writes.push((wall_side(raw), raw[3]));
                }
            }
            prefixes.push(writes.clone());
        }
        let mut runs: Vec<Vec<(WallSide, u8)>> = Vec::new();
        let mut last: Option<u32> = None;
        for (at, raw) in self.commands() {
            if raw[0] != 0x84 || self.sources.get(&at).is_some_and(|va| ordered.contains(va)) {
                last = None;
                continue;
            }
            let source = self.sources.get(&at).copied();
            let write = (wall_side(raw), raw[3]);
            match (last, source, runs.last_mut()) {
                (Some(l), Some(s), Some(run)) if s == l + 4 => run.push(write),
                _ => runs.push(vec![write]),
            }
            last = source;
        }
        let mut groups: Vec<Vec<WallSide>> = Vec::new();
        for run in &runs {
            let sides: Vec<WallSide> = run.iter().map(|(side, _)| *side).collect();
            let mut merged: Vec<WallSide> = Vec::new();
            groups.retain(|g| {
                if g.iter().any(|s| sides.contains(s)) {
                    merged.extend(g);
                    false
                } else {
                    true
                }
            });
            for side in sides {
                if !merged.contains(&side) {
                    merged.push(side);
                }
            }
            groups.push(merged);
        }
        let mut choices = Vec::new();
        for group in groups {
            let touching: Vec<&Vec<(WallSide, u8)>> = runs
                .iter()
                .filter(|r| r.iter().any(|(side, _)| group.contains(side)))
                .collect();
            let whole = touching.iter().all(|r| {
                r.len() == group.len() && group.iter().all(|g| r.iter().any(|(s, _)| s == g))
            });
            if whole && group.len() > 1 {
                let mut alternatives: Vec<Vec<(WallSide, u8)>> = Vec::new();
                for run in touching {
                    let mut writes = run.clone();
                    writes.sort();
                    if !alternatives.contains(&writes) {
                        alternatives.push(writes);
                    }
                }
                choices.push(WallChoice { alternatives });
                continue;
            }
            for side in group {
                let mut alternatives: Vec<Vec<(WallSide, u8)>> = Vec::new();
                for &(s, v) in runs.iter().flatten() {
                    if s == side && !alternatives.contains(&vec![(s, v)]) {
                        alternatives.push(vec![(s, v)]);
                    }
                }
                choices.push(WallChoice { alternatives });
            }
        }
        if !prefixes.is_empty() {
            choices.insert(
                0,
                WallChoice {
                    alternatives: prefixes,
                },
            );
        }
        choices
    }
    pub fn number_pictures(&mut self, ids: &BTreeMap<u32, u8>) -> Result<()> {
        let at: Vec<u16> = self
            .commands()
            .filter(|(_, raw)| raw[0] == 0x77)
            .map(|(at, _)| at)
            .collect();
        let mut numbered = vec![0u32; ids.values().copied().max().map_or(0, usize::from)];
        for at in at {
            let o = usize::from(at) + 4;
            let index = usize::from(u16::from_le_bytes([self.program[o], self.program[o + 1]]));
            let entry = *self
                .pictures
                .get(index)
                .ok_or("Picture outside its floor's list")?;
            let id = *ids.get(&entry).ok_or("Picture without an id")?;
            if numbered[usize::from(id) - 1] == 0 {
                numbered[usize::from(id) - 1] = entry;
            }
            self.program[o..o + 2].copy_from_slice(&u16::from(id - 1).to_le_bytes());
        }
        self.pictures = numbered;
        Ok(())
    }
    pub fn marker_changes(&self) -> Vec<(u8, u8)> {
        let mut changes: Vec<(u8, u8)> = Vec::new();
        for (_, raw) in self.commands().filter(|(_, raw)| raw[0] == 0x97) {
            if !changes.contains(&(raw[1], raw[2])) {
                changes.push((raw[1], raw[2]));
            }
        }
        changes
    }
    pub fn wall_changes(&self) -> Vec<WallChange> {
        let mut changes: Vec<WallChange> = Vec::new();
        for (_, raw) in self.commands().filter(|(_, raw)| raw[0] == 0x84) {
            let facing = match raw[2] & 0x7f {
                1 => 0,
                2 => 1,
                8 => 2,
                _ => 3,
            };
            let (cell, both) = (raw[1], raw[2] & 0x80 != 0);
            match changes
                .iter_mut()
                .find(|c| (c.cell, c.facing, c.both) == (cell, facing, both))
            {
                Some(c) if !c.values.contains(&raw[3]) => c.values.push(raw[3]),
                Some(_) => {}
                None => changes.push(WallChange {
                    cell,
                    facing,
                    both,
                    values: vec![raw[3]],
                }),
            }
        }
        changes
    }
}
