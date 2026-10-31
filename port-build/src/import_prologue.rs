use crate::{
    Result, SOURCE,
    import::Files,
    original::Originals,
    script::{Graph, Link, Options, ResourceCommand, decode_graph, decode_resource_script},
    sha,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
pub const PROLOGUE_ROOT: u32 = 0x46e89c;
fn json(value: Value) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(&value)?;
    bytes.push(b'\n');
    Ok(bytes)
}
pub fn prologue(originals: &Originals) -> Result<Graph> {
    let graph = decode_graph(originals.exe()?, &[PROLOGUE_ROOT], &Options::default())?;
    if !graph.unknowns.is_empty() {
        return Err("Unresolved prologue command boundaries".into());
    }
    Ok(graph)
}
pub fn prologue_resources(originals: &Originals, graph: &Graph) -> Result<Vec<ResourceCommand>> {
    let roots: BTreeSet<u32> = graph
        .nodes
        .values()
        .flat_map(|n| &n.links)
        .filter_map(|l| match l {
            Link::Resource(va) => Some(*va),
            _ => None,
        })
        .collect();
    let mut commands = Vec::new();
    for root in roots {
        commands.extend(decode_resource_script(originals.exe()?, root)?);
    }
    Ok(commands)
}
fn objects(
    originals: &Originals,
    graph: &Graph,
    commands: &[ResourceCommand],
    sizes: &BTreeMap<String, (usize, usize)>,
) -> Result<Vec<Value>> {
    let exe = originals.exe()?;
    let mut registrations: BTreeMap<i16, (u8, &str, &[u32])> = BTreeMap::new();
    for command in commands.iter().filter(|c| [1, 2].contains(&c.opcode)) {
        let key = (
            command.opcode,
            command
                .file
                .as_deref()
                .ok_or("Registration without a file")?,
            command.arguments.as_slice(),
        );
        if *registrations.entry(command.slot).or_insert(key) != key {
            return Err("Resource slot has changing frame geometry".into());
        }
    }
    let mut objects = Vec::new();
    for node in graph.nodes.values().filter(|n| n.opcode() == 8) {
        let raw = &node.raw;
        let (mut cursor, mut assignments) = (4, Vec::new());
        while raw[cursor] != 255 {
            let (kind, offset) = (raw[cursor], usize::from(raw[cursor + 1]));
            let width = match kind {
                1 => 1,
                2 => 2,
                3 => 4,
                _ => return Err("Unsupported object assignment".into()),
            };
            if offset + width > 256 || (offset < 0x44 && offset + width > 0x40) {
                return Err("Unsupported object assignment".into());
            }
            let start = cursor + if kind == 3 { 4 } else { 2 };
            let value = raw[start..start + width]
                .iter()
                .rev()
                .fold(0u32, |v, &b| (v << 8) | u32::from(b));
            assignments.push(json!({ "offset" : offset, "width" : width, "value" : value }));
            cursor += if kind == 3 { 8 } else { 4 };
        }
        if cursor + 4 != raw.len() {
            return Err("Unexpected initialization extent".into());
        }
        let frames: Vec<u32> = assignments
            .iter()
            .filter(|a| a["offset"] == 0x28 && a["width"] == 4)
            .map(|a| a["value"].as_u64().unwrap() as u32)
            .collect();
        let [packed] = frames[..] else {
            return Err("Missing or ambiguous object frame".into());
        };
        let (slot, clip) = ((packed >> 16) as i16, packed & 0xffff);
        let (op, resource, args) = *registrations.get(&slot).ok_or("Unregistered object slot")?;
        let (width, height) = sizes[resource];
        let rectangle: Vec<i32> = if op == 1 {
            exe.read(args[0] + clip * 16, 16)?
                .chunks(4)
                .map(|c| i32::from_le_bytes(c.try_into().unwrap()))
                .collect()
        } else if clip == 0 {
            vec![0, 0, (args[0] & 0xffff) as i32, (args[0] >> 16) as i32]
        } else {
            return Err("Nonzero grid clip requires draw-path records".into());
        };
        let [l, t, r, b] = rectangle[..] else {
            unreachable!()
        };
        if !(0 <= l && l < r && r as usize <= width && 0 <= t && t < b && b as usize <= height) {
            return Err("Frame rectangle lies outside source sheet".into());
        }
        objects.push(json!(
            { "source_va" : node.va, "source_bytes" : raw.len(), "assignments" :
            assignments, "frame" : { "slot" : slot, "clip" : clip, "resource" :
            resource, "rectangle" : rectangle } }
        ));
    }
    Ok(objects)
}
pub fn import(originals: &Originals) -> Result<Files> {
    let mut files = Files::new();
    let exe = originals.exe()?;
    let b1 = &crate::import_world::maps(exe)?[0];
    if exe.read(b1.entry_script_va, 4)? != [0x3f, 0, 0, 0] {
        return Err("Unexpected B1 entry command".into());
    }
    let mut commands = decode_resource_script(exe, exe.u32(b1.entry_script_va + 4)?)?;
    let graph = prologue(originals)?;
    let prologue_commands = prologue_resources(originals, &graph)?;
    commands.extend(prologue_commands.iter().cloned());
    let names: BTreeSet<&str> = commands.iter().filter_map(|c| c.file.as_deref()).collect();
    let (mut resources, mut images, mut sizes) = (Vec::new(), Vec::new(), BTreeMap::new());
    for name in names {
        let file = originals.file(name)?;
        resources.push(json!({ "file" : name, "bytes" : file.len(), "sha256" : sha(& file) }));
        let sheet = crate::cns::Sheet::decode(&file)?;
        images.push(json!(
            { "resource" : name, "width" : sheet.width, "height" : sheet.height,
            "pixels_file" : format!("{name}.indices"), "palette_file" :
            format!("{name}.rgb"), "pixels_sha256" : sha(& sheet.pixels),
            "palette_sha256" : sha(& sheet.rgb) }
        ));
        sizes.insert(name.to_string(), (sheet.width, sheet.height));
        files.push((format!("images/{name}.indices").into(), sheet.pixels));
        files.push((format!("images/{name}.rgb").into(), sheet.rgb));
    }
    files.push((
        "images.json".into(),
        json(json!(
            { "schema" : 1, "source_sha256" : SOURCE, "resources" : resources,
            "images" : images }
        ))?,
    ));
    let objects = objects(originals, &graph, &prologue_commands, &sizes)?;
    files.push((
        "objects.json".into(),
        json(json!({ "schema" : 1, "source_sha256" : SOURCE, "objects" : objects }))?,
    ));
    Ok(files)
}
