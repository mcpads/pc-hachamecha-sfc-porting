use crate::{
    Result, SOURCE,
    import::Files,
    import_world::{events, maps},
    original::{Exe, Originals},
    script::{Options, ResourceCommand, decode_graph, decode_resource_script},
    sha,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
const SCREEN_ORIGIN: [i32; 2] = [64, 32];
const SCREEN_SPACE: u32 = 0x40000000;
const AFTER_LIST: [u8; 12] = [
    0x79, 0x20, 0x4b, 0x11, 0x14, 0x77, 0x1f, 0x2d, 0x3b, 0x3a, 0x03, 0x01,
];
fn json(value: Value) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(&value)?;
    bytes.push(b'\n');
    Ok(bytes)
}
fn u32_at(raw: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(raw[at..at + 4].try_into().unwrap())
}
struct World<'a> {
    exe: &'a Exe,
    commands: BTreeMap<u32, Vec<u8>>,
    dead: BTreeSet<(u32, u32)>,
}
impl World<'_> {
    fn command(&self, at: u32) -> Result<&[u8]> {
        self.commands
            .get(&at)
            .map(Vec::as_slice)
            .ok_or_else(|| format!("No decoded command at {at:#x}").into())
    }
    fn reachable(&self, root: u32) -> Result<Vec<u32>> {
        let (mut seen, mut pending) = (BTreeSet::new(), vec![root]);
        while let Some(at) = pending.pop() {
            if !seen.insert(at) {
                continue;
            }
            let c = self.command(at)?;
            let (op, sub) = (c[0], c[1]);
            if op == 0x03 {
                pending.push(u32_at(c, 4));
                continue;
            }
            if op == 0x00 || op == 0x05 {
                continue;
            }
            pending.push(at + c.len() as u32);
            if [0x04, 0x06, 0x13, 0x32, 0x74].contains(&op)
                || ([0x7a, 0x96].contains(&op) && sub == 1)
            {
                let target = u32_at(c, 4);
                if !self.dead.contains(&(at, target)) {
                    pending.push(target);
                }
            } else if op == 0x14 || op == 0x15 {
                pending.push(u32_at(c, c.len() - 4));
            } else if op == 0x75 && sub == 0 {
                pending.push(u32_at(c, 12));
            }
        }
        Ok(seen.into_iter().collect())
    }
    fn step(&self, at: u32) -> Result<u32> {
        let c = self.command(at)?;
        Ok(if c[0] == 0x03 {
            u32_at(c, 4)
        } else {
            at + c.len() as u32
        })
    }
    fn draws_surface(&self, entry: u32) -> Result<bool> {
        let mut at = entry;
        for _ in 0..64 {
            match self.command(at)?[0] {
                0x79 => return Ok(true),
                0x01 => return Ok(false),
                _ => at = self.step(at)?,
            }
        }
        Err(format!("Object {entry:#x} did not yield").into())
    }
    fn leaving(&self, init: u32) -> Result<Option<u8>> {
        let mut at = init;
        let mut found = false;
        for _ in 0..32 {
            let c = self.command(at)?;
            if c[0] == 0x74 {
                at += c.len() as u32;
                found = true;
                break;
            }
            at = self.step(at)?;
        }
        if !found {
            return Err(format!("Object {init:#x} has no position loop").into());
        }
        let mut song = None;
        for _ in 0..8 {
            let c = self.command(at)?;
            if c[0] == 0x26 {
                song = Some(c[3]);
            }
            if c[0] == 0x00 || c[0] == 0x05 {
                return Ok(song);
            }
            at = self.step(at)?;
        }
        Err(format!("Object {init:#x} does not end after leaving").into())
    }
    fn palette_writes(&self, mut data: u32) -> Result<Vec<[u32; 4]>> {
        let mut writes = Vec::new();
        while self.exe.read(data, 1)?[0] != 0xff {
            let [i, r, g, b] = self.exe.read(data, 4)?.try_into()?;
            writes.push([
                u32::from(i),
                u32::from(r) << 4,
                u32::from(g) << 4,
                u32::from(b) << 4,
            ]);
            data += 4;
        }
        Ok(writes)
    }
    #[allow(clippy::type_complexity)]
    fn prefix(
        &self,
        entry: u32,
    ) -> Result<(
        BTreeMap<u8, (usize, u32)>,
        Option<u32>,
        Option<u32>,
        Vec<u32>,
    )> {
        let (mut presets, mut stream, mut at, mut writes) =
            (BTreeMap::new(), None, entry, Vec::new());
        for _ in 0..12 {
            let c = self.command(at)?;
            match (c[0], c[1]) {
                (0x11 | 0x12, 0x80) => {
                    let width = if c[0] == 0x11 { 2 } else { 4 };
                    let value = c[4..4 + width]
                        .iter()
                        .rev()
                        .fold(0u32, |v, &b| (v << 8) | u32::from(b));
                    presets.insert(c[2], (width, value));
                }
                (0x03, _) => {
                    at = u32_at(c, 4);
                    continue;
                }
                (0x3f, _) => stream = Some(u32_at(c, 4)),
                (0x39, 0) => {
                    if !writes.is_empty() {
                        return Err(format!("Object {entry:#x} sets two colour lists").into());
                    }
                    writes = vec![u32_at(c, 4)];
                }
                (0x3b | 0x3c, _) | (0x39, 1) => {}
                (0x08, _) => return Ok((presets, stream, Some(at), writes)),
                (0x77, _) if presets.contains_key(&0x28) => {
                    return Ok((presets, stream, None, writes));
                }
                _ => return Err(format!("Unexpected object prefix at {at:#x}").into()),
            }
            at += c.len() as u32;
        }
        Err(format!("Object {entry:#x} has no initializer").into())
    }
    fn initialize(
        &self,
        init: Option<u32>,
        presets: &BTreeMap<u8, (usize, u32)>,
    ) -> Result<[u8; 256]> {
        let mut state = [0u8; 256];
        for (&field, &(width, value)) in presets {
            let at = usize::from(field);
            state[at..at + width].copy_from_slice(&value.to_le_bytes()[..width]);
        }
        let Some(init) = init else { return Ok(state) };
        let list = self.command(init)?;
        let mut k = 4;
        while list[k] != 0xff {
            let (kind, field) = (list[k], usize::from(list[k + 1]));
            match kind {
                1 => {
                    state[field] = list[k + 2];
                    k += 4;
                }
                2 => {
                    state[field..field + 2].copy_from_slice(&list[k + 2..k + 4]);
                    k += 4;
                }
                3 => {
                    state[field..field + 4].copy_from_slice(&list[k + 4..k + 8]);
                    k += 8;
                }
                _ => return Err(format!("Unsupported initializer at {init:#x}").into()),
            }
        }
        let mut at = init + list.len() as u32;
        for _ in 0..16 {
            let c = self.command(at)?;
            if !AFTER_LIST.contains(&c[0]) {
                return Err(format!(
                    "Initializer {init:#x} runs {:#x}, which may move its picture",
                    c[0]
                )
                .into());
            }
            if c[0] == 0x01 {
                return Ok(state);
            }
            at = self.step(at)?;
        }
        Err(format!("Initializer {init:#x} did not yield").into())
    }
}
pub fn import(originals: &Originals) -> Result<Files> {
    let exe = originals.exe()?;
    let all_maps = maps(exe)?;
    let mut commands = BTreeMap::new();
    let mut roots = Vec::new();
    for map in &all_maps {
        let scopes = crate::import_world::scopes(exe, map)?;
        for graph in [&scopes.entry, &scopes.exit, &scopes.events] {
            for node in graph.nodes.values() {
                commands.insert(node.va, node.raw.clone());
            }
        }
        roots.push(
            events(exe, map)?
                .iter()
                .map(|e| e.script_va)
                .collect::<Vec<_>>(),
        );
    }
    let world = World {
        exe,
        commands,
        dead: [(0x4b814c, 0x4b6784)].into(),
    };
    let mut sheets: Vec<String> = Vec::new();
    let mut sheet_rows: BTreeMap<String, Value> = BTreeMap::new();
    let mut files = Files::new();
    let mut floors = Vec::new();
    let mut firewall = None;
    for (map, roots) in all_maps.iter().zip(&roots) {
        let mut objects: BTreeMap<u32, Value> = BTreeMap::new();
        for &root in roots {
            let mut loaded: BTreeMap<i16, (u32, Vec<ResourceCommand>, ResourceCommand)> =
                BTreeMap::new();
            let path = world.reachable(root)?;
            let mut targets = BTreeMap::new();
            for &va in &path {
                let c = world.command(va)?;
                let next = va + c.len() as u32;
                let after = world.commands.get(&next).map(Vec::as_slice).unwrap_or(&[]);
                if c[0] != 0x77 || after.len() < 2 || after[..2] != [0x39, 3] {
                    continue;
                }
                let mut at = next + after.len() as u32;
                let mut shape = Vec::new();
                for _ in 0..3 {
                    let c = world.command(at)?;
                    shape.push(c[..4].to_vec());
                    at += c.len() as u32;
                }
                if shape
                    != [
                        [0x3b, 0x30, 0x20, 0],
                        [0x39, 0x01, 0x30, 0x20],
                        [0x38, 0x02, 0x0a, 0xe6],
                    ]
                {
                    return Err(format!("Target colours at {next:#x} are not faded to").into());
                }
                targets.insert(va, (next, u32_at(after, 4)));
            }
            for &va in &path {
                let c = world.command(va)?;
                if c[0] == 0x3f {
                    let stream = u32_at(c, 4);
                    let resource = decode_resource_script(exe, stream)?;
                    for r in resource.iter().filter(|r| [1, 2, 4].contains(&r.opcode)) {
                        loaded.insert(r.slot, (stream, resource.clone(), r.clone()));
                    }
                }
                if c[0] != 0x77 {
                    continue;
                }
                let entry = u32_at(c, 4);
                let (presets, stream, init, writes) = world.prefix(entry)?;
                let (created, target) = targets
                    .get(&va)
                    .map_or((entry, None), |&(a, t)| (a, Some(t)));
                if let Some(stream) = stream {
                    let resource = decode_resource_script(exe, stream)?;
                    for r in resource.iter().filter(|r| [1, 2, 4].contains(&r.opcode)) {
                        loaded.insert(r.slot, (stream, resource.clone(), r.clone()));
                    }
                }
                let state = world.initialize(init, &presets)?;
                let field = |at: usize| u32::from_le_bytes(state[at..at + 4].try_into().unwrap());
                let packed = field(0x28);
                let (slot, frame) = ((packed >> 16) as i16, packed & 0xffff);
                let (stream_va, stream_commands, load) = loaded.get(&slot).ok_or_else(|| {
                    format!("Object {entry:#x} draws from unloaded slot {slot:#x}")
                })?;
                let screen = field(0) & SCREEN_SPACE != 0;
                let surface = world.draws_surface(entry)?;
                if surface && screen {
                    return Err(format!(
                        "Object {entry:#x} is drawn both into the view and over it"
                    )
                    .into());
                }
                let file = load.file.clone().ok_or("Object load without a file")?;
                let picture = originals.cns(&file)?;
                let (mut x, mut y) = ((field(0x1c) as i32) >> 16, (field(0x20) as i32) >> 16);
                let rectangle: Vec<i32> = match load.opcode {
                    2 => {
                        let (w, h) = (load.arguments[0] & 0xffff, load.arguments[0] >> 16);
                        let columns = picture.width as u32 / w;
                        [
                            frame % columns * w,
                            frame / columns * h,
                            (frame % columns + 1) * w,
                            (frame / columns + 1) * h,
                        ]
                        .map(|v| v as i32)
                        .to_vec()
                    }
                    1 => exe
                        .read(load.arguments[0] + frame * 16, 16)?
                        .chunks(4)
                        .map(|c| i32::from_le_bytes(c.try_into().unwrap()))
                        .collect(),
                    _ => {
                        let v: Vec<i32> = exe
                            .read(load.arguments[0] + frame * 24, 24)?
                            .chunks(4)
                            .map(|c| i32::from_le_bytes(c.try_into().unwrap()))
                            .collect();
                        x -= v[4];
                        y -= v[5];
                        v[..4].to_vec()
                    }
                };
                let key = stream_commands
                    .iter()
                    .find(|r| r.opcode == 12 && r.slot == slot)
                    .map_or(0, |r| r.flags);
                let presets_json: serde_json::Map<String, Value> = presets
                    .iter()
                    .map(|(k, (_, v))| (format!("{k:#x}"), json!(v)))
                    .collect();
                let position = if screen {
                    [x - SCREEN_ORIGIN[0], y - SCREEN_ORIGIN[1]]
                } else {
                    [x, y]
                };
                let mut record = json!(
                    { "entry_va" : created, "init_va" : init, "resource_va" : stream_va,
                    "presets" : presets_json, "source_position" : [x, y], "position" :
                    position, "space" : if screen { "screen" } else { "viewport" },
                    "resource" : file, "slot" : slot, "frame" : frame, "z" : field(0x24)
                    as i32, "rectangle" : rectangle, "palette_base" : load.flags, "key" :
                    key, "surface" : surface, "leaving_song" : world.leaving(init
                    .unwrap_or(entry)) ? }
                );
                let mut colours: BTreeMap<u32, [u32; 4]> = BTreeMap::new();
                if let Some(&list) = writes.first() {
                    record["palette_va"] = list.into();
                    for w in world.palette_writes(list)? {
                        colours.insert(w[0], w);
                    }
                    record["palette_writes"] = json!(world.palette_writes(list)?);
                }
                if let Some(target) = target {
                    record["object_va"] = entry.into();
                    record["target_palette_va"] = target.into();
                    let listed = world.palette_writes(target)?;
                    let base = u32::from(load.flags);
                    let restates = listed.iter().all(|w| {
                        w[0] >= base
                            && w[0] - base < 256
                            && (0..3).all(|k| {
                                w[k + 1]
                                    == u32::from(
                                        picture.rgb[((w[0] - base) * 3) as usize + k] & 0xf0,
                                    )
                            })
                    });
                    for w in listed {
                        if restates {
                            colours.remove(&w[0]);
                        } else {
                            colours.insert(w[0], w);
                        }
                    }
                    let obj = record.as_object_mut().unwrap();
                    if colours.is_empty() {
                        obj.remove("palette_writes");
                    } else {
                        obj.insert(
                            "palette_writes".into(),
                            json!(colours.values().collect::<Vec<_>>()),
                        );
                    }
                }
                if objects.get(&created).is_some_and(|r| *r != record) {
                    return Err(format!("Object {created:#x} differs between events").into());
                }
                if file == "FIREWALL.CNS" {
                    firewall = Some((created, *stream_va));
                }
                objects.insert(created, record);
                if !sheet_rows.contains_key(&file) {
                    files.push((
                        format!("floor-objects/{file}.indices").into(),
                        picture.pixels.clone(),
                    ));
                    files.push((
                        format!("floor-objects/{file}.rgb").into(),
                        picture.rgb.clone(),
                    ));
                    sheet_rows.insert(
                        file.clone(),
                        json!(
                            { "file" : file, "width" : picture.width, "height" : picture
                            .height, "pixels_sha256" : sha(& picture.pixels),
                            "palette_sha256" : sha(& picture.rgb) }
                        ),
                    );
                    sheets.push(file);
                }
            }
        }
        floors.push(json!(
            { "floor" : map.id, "objects" : objects.into_values().collect::< Vec
            < _ >> () }
        ));
    }
    let resources: Vec<&Value> = sheets.iter().map(|f| &sheet_rows[f]).collect();
    files.push((
        "floor-objects/floor-objects.json".into(),
        json(json!(
            { "schema" : 1, "source_sha256" : SOURCE, "screen_origin" :
            SCREEN_ORIGIN, "resources" : resources, "floors" : floors }
        ))?,
    ));
    let (body, stream) = firewall.ok_or("No FIREWALL object")?;
    flames(originals, &mut files, body, stream)?;
    Ok(files)
}
fn flames(originals: &Originals, files: &mut Files, body: u32, stream: u32) -> Result<()> {
    let exe = originals.exe()?;
    let loads: Vec<_> = decode_resource_script(exe, stream)?
        .into_iter()
        .filter(|r| r.opcode == 1 && r.file.as_deref() == Some("FIREWALL.CNS"))
        .collect();
    let [load] = &loads[..] else {
        return Err("FIREWALL is not loaded once".into());
    };
    let graph = decode_graph(exe, &[body], &Options::default())?;
    let roots: Vec<u32> = graph
        .nodes
        .values()
        .filter(|n| n.opcode() == 0x20)
        .map(|n| n.pointer(4))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let animations = decode_graph(exe, &roots, &Options::default())?;
    if !graph.unknowns.is_empty() || !animations.unknowns.is_empty() {
        return Err("FIREWALL commands not decoded".into());
    }
    let nodes: Vec<Value> = graph
        .nodes
        .values()
        .chain(animations.nodes.values())
        .map(|n| json!({ "va" : n.va, "raw_hex" : n.raw_hex() }))
        .collect();
    let sheet = originals.cns("FIREWALL.CNS")?;
    let clips_va = load.arguments[0];
    let clips: Vec<Vec<i32>> = (0..16)
        .map(|i| {
            Ok(exe
                .read(clips_va + 16 * i, 16)?
                .chunks(4)
                .map(|c| i32::from_le_bytes(c.try_into().unwrap()))
                .collect())
        })
        .collect::<Result<_>>()?;
    files.push((
        "b5-flames/FIREWALL.CNS.indices".into(),
        sheet.pixels.clone(),
    ));
    files.push(("b5-flames/FIREWALL.CNS.rgb".into(), sheet.rgb.clone()));
    files.push((
        "b5-flames/flames.json".into(),
        json(json!(
            { "schema" : 1, "source_sha256" : SOURCE, "body_va" : body, "nodes" :
            nodes, "file" : "FIREWALL.CNS", "width" : sheet.width, "height" :
            sheet.height, "clips_va" : clips_va, "clips" : clips, "pixels_sha256"
            : sha(& sheet.pixels), "palette_sha256" : sha(& sheet.rgb) }
        ))?,
    ));
    Ok(())
}
