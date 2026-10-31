use crate::{
    Result, SOURCE,
    import::Files,
    original::{Exe, Originals},
    script::{Graph, Link, Options, ResourceCommand, decode_graph, decode_resource_script},
    sha,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
const ENDING_DESCRIPTOR: u32 = 0x4b88b8;
const ENDING_ROOT: u32 = 0x4b8b30;
const PHANTOM_ROOT: u32 = 0x47eb58;
const RESERVED: [(&str, u32); 1] = [("rulue_b7", 0x4ab5a8)];
fn json(value: Value) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(&value)?;
    bytes.push(b'\n');
    Ok(bytes)
}
fn whole(exe: &Exe, root: u32) -> Result<Graph> {
    let graph = decode_graph(exe, &[root], &Options::default())?;
    if !graph.unknowns.is_empty() {
        return Err(format!("Scene {root:#x} commands not decoded").into());
    }
    Ok(graph)
}
fn stream_roots(graph: &Graph) -> BTreeSet<u32> {
    graph
        .nodes
        .values()
        .flat_map(|n| &n.links)
        .filter_map(|l| match l {
            Link::Resource(va) => Some(*va),
            _ => None,
        })
        .collect()
}
fn rectangle(exe: &Exe, at: u32) -> Result<[i32; 4]> {
    let raw = exe.read(at, 16)?;
    Ok(std::array::from_fn(|i| {
        i32::from_le_bytes(raw[i * 4..i * 4 + 4].try_into().unwrap())
    }))
}
fn grid(table: u32, width: usize, height: usize) -> Vec<[i32; 4]> {
    let (w, h) = ((table & 0xffff) as usize, (table >> 16) as usize);
    let mut frames = Vec::new();
    for y in (0..=height.saturating_sub(h)).step_by(h.max(1)) {
        for x in (0..=width.saturating_sub(w)).step_by(w.max(1)) {
            frames.push([x, y, x + w, y + h].map(|v| v as i32));
        }
    }
    if h > height || w > width {
        frames.clear();
    }
    frames
}
fn inside(frames: &[[i32; 4]], width: usize, height: usize) -> bool {
    frames.iter().all(|&[l, t, r, b]| {
        0 <= l && l < r && r as usize <= width && 0 <= t && t < b && b as usize <= height
    })
}
fn ending(originals: &Originals, files: &mut Files) -> Result<()> {
    let exe = originals.exe()?;
    if exe.u32(ENDING_DESCRIPTOR)? != ENDING_ROOT {
        return Err("Ending descriptor does not name its body".into());
    }
    let graph = whole(exe, ENDING_ROOT)?;
    let roots: BTreeSet<u32> = graph
        .nodes
        .values()
        .filter(|n| [0x07, 0x20].contains(&n.opcode()))
        .map(|n| n.pointer(4))
        .chain([ENDING_ROOT])
        .collect();
    let messages: Vec<u32> = graph
        .nodes
        .values()
        .filter(|n| n.opcode() == 0x2f)
        .flat_map(|n| &n.links)
        .filter_map(|l| match l {
            Link::Message(va) => Some(*va),
            _ => None,
        })
        .collect();
    let stream_vas = stream_roots(&graph);
    let decoded: BTreeMap<u32, Vec<ResourceCommand>> = stream_vas
        .iter()
        .map(|&va| Ok((va, decode_resource_script(exe, va)?)))
        .collect::<Result<_>>()?;
    let tables: BTreeSet<u32> = decoded
        .values()
        .flatten()
        .filter(|c| c.opcode == 1)
        .map(|c| c.arguments[0])
        .collect();
    let boundaries: Vec<u32> = tables.iter().copied().chain([ENDING_ROOT]).collect();
    let mut sheets: Vec<Value> = Vec::new();
    let mut sizes: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut streams = Vec::new();
    for (&va, commands) in &decoded {
        let mut actions = Vec::new();
        for c in commands {
            match c.opcode {
                1 | 2 | 6 => {
                    let file = c.file.as_deref().ok_or("Ending load without a file")?;
                    if !sizes.contains_key(file) {
                        let sheet = originals.cns(file)?;
                        let rgb = &sheet.rgb[..sheet.colors * 3];
                        sheets.push(json!(
                            { "file" : file, "width" : sheet.width, "height" : sheet
                            .height, "colours" : sheet.colors, "pixels_sha256" : sha(&
                            sheet.pixels), "rgb_sha256" : sha(rgb) }
                        ));
                        files.push((
                            format!("ending/{file}.indices").into(),
                            sheet.pixels.clone(),
                        ));
                        files.push((format!("ending/{file}.rgb").into(), rgb.to_vec()));
                        sizes.insert(file.to_string(), (sheet.width, sheet.height));
                    }
                    let (width, height) = sizes[file];
                    let keys: Vec<i32> = commands
                        .iter()
                        .filter(|k| k.opcode == 12 && k.slot == c.slot)
                        .map(|k| i32::from(k.flags) - i32::from(c.flags))
                        .collect();
                    if keys.len() > 1 || !keys.iter().all(|k| (0..16).contains(k)) {
                        return Err(format!("Stream {va:#x} transparent index").into());
                    }
                    let table = c.arguments[0];
                    let frames: Vec<[i32; 4]> = match c.opcode {
                        1 => {
                            let end = *boundaries
                                .iter()
                                .find(|&&b| b > table)
                                .ok_or("Frame table end")?;
                            if (end - table) % 16 != 0 {
                                return Err(format!(
                                    "Frame table {table:#x} does not end at {end:#x}"
                                )
                                .into());
                            }
                            (0..(end - table) / 16)
                                .map(|i| rectangle(exe, table + 16 * i))
                                .collect::<Result<_>>()?
                        }
                        2 => grid(table, width, height),
                        _ => Vec::new(),
                    };
                    if !inside(&frames, width, height) {
                        return Err(format!("Frame outside {file}").into());
                    }
                    actions.push(json!(
                        { "op" : "load", "kind" : c.opcode, "slot" : c.slot, "file"
                        : file, "base" : c.flags, "key" : keys.first(), "frames" :
                        frames }
                    ));
                }
                11 => actions.push(json!({ "op" : "release", "slot" : c.slot })),
                0 | 12 => {}
                op => return Err(format!("Stream {va:#x} command {op}").into()),
            }
        }
        streams.push(json!({ "va" : va, "actions" : actions }));
    }
    let order: Vec<u32> = graph
        .nodes
        .values()
        .filter(|n| n.opcode() == 0x3f)
        .flat_map(|n| &n.links)
        .filter_map(|l| match l {
            Link::Resource(va) => Some(*va),
            _ => None,
        })
        .collect();
    let mut starts = serde_json::Map::new();
    for &va in &roots {
        let mut at = va;
        for _ in 0..2 {
            let Some(node) = graph.nodes.get(&at) else {
                break;
            };
            let raw = &node.raw;
            if raw[0] == 0x08 {
                let mut fields = BTreeMap::new();
                let mut k = 4;
                while raw[k] != 0xff {
                    if raw[k] == 3 {
                        fields.insert(
                            raw[k + 1],
                            i32::from_le_bytes(raw[k + 4..k + 8].try_into()?),
                        );
                        k += 8;
                    } else {
                        k += 4;
                    }
                }
                let fields: serde_json::Map<String, Value> = fields
                    .iter()
                    .map(|(f, v)| (format!("{f:#04x}"), json!(v)))
                    .collect();
                starts.insert(format!("{va:#x}"), Value::Object(fields));
                break;
            }
            at += node.raw.len() as u32;
        }
    }
    let scene = json!(
        { "schema" : 1, "source_sha256" : SOURCE, "descriptor_va" : ENDING_DESCRIPTOR,
        "root_va" : ENDING_ROOT, "roots" : roots, "messages" : messages, "streams" :
        streams, "stream_order" : order, "starts" : starts, "sheets" : sheets }
    );
    files.push(("ending/ending-scene.json".into(), json(scene)?));
    Ok(())
}
fn streams_of(originals: &Originals, roots: &BTreeSet<u32>, root: u32) -> Result<Vec<Value>> {
    let exe = originals.exe()?;
    let decoded: Vec<(u32, Vec<ResourceCommand>)> = roots
        .iter()
        .map(|&va| Ok((va, decode_resource_script(exe, va)?)))
        .collect::<Result<_>>()?;
    let tables: BTreeSet<u32> = decoded
        .iter()
        .flat_map(|(_, s)| s)
        .filter(|c| [1, 4].contains(&c.opcode))
        .map(|c| c.arguments[0])
        .collect();
    let boundaries: Vec<u32> = tables.iter().copied().chain([root]).collect();
    let mut streams = Vec::new();
    for (va, commands) in &decoded {
        let mut actions = Vec::new();
        for c in commands {
            match c.opcode {
                1 | 2 | 4 => {
                    let keys: Vec<u8> = commands
                        .iter()
                        .filter(|k| k.opcode == 12 && k.slot == c.slot)
                        .map(|k| k.flags)
                        .collect();
                    let [key] = keys[..] else {
                        return Err(format!("Stream {va:#x} without one transparent index").into());
                    };
                    let key = i32::from(key) - i32::from(c.flags);
                    if !(0..16).contains(&key) {
                        return Err(format!("Stream {va:#x} without one transparent index").into());
                    }
                    let file = c.file.as_deref().ok_or("Scene load without a file")?;
                    let sheet = originals.cns(file)?;
                    if ![16, 32].contains(&sheet.colors) {
                        return Err(format!("Unsupported battle sheet: {file}").into());
                    }
                    let table = c.arguments[0];
                    let mut centers = None;
                    let frames: Vec<[i32; 4]> = if c.opcode == 2 {
                        grid(table, sheet.width, sheet.height)
                    } else {
                        let end = *boundaries
                            .iter()
                            .find(|&&b| b > table)
                            .ok_or("Frame table end")?;
                        let size = if c.opcode == 1 { 16 } else { 24 };
                        let count = (end - table) / size;
                        let rest = end - table - count * size;
                        if rest > 3
                            || exe
                                .read(table + count * size, rest as usize)?
                                .iter()
                                .any(|&b| b != 0)
                        {
                            return Err(
                                format!("Frame table {table:#x} does not end at {end:#x}").into()
                            );
                        }
                        if c.opcode == 4 {
                            centers = Some(
                                (0..count)
                                    .map(|i| {
                                        let raw = exe.read(table + size * i + 16, 8)?;
                                        Ok([0, 4].map(|o| {
                                            i32::from_le_bytes(raw[o..o + 4].try_into().unwrap())
                                        }))
                                    })
                                    .collect::<Result<Vec<_>>>()?,
                            );
                        }
                        (0..count)
                            .map(|i| rectangle(exe, table + size * i))
                            .collect::<Result<_>>()?
                    };
                    if !inside(&frames, sheet.width, sheet.height) {
                        return Err(format!("Frame outside {file}").into());
                    }
                    let mut action = json!(
                        { "op" : "load", "kind" : c.opcode, "slot" : c.slot, "file" :
                        file, "base" : c.flags, "key" : key, "table_va" : table, "frames"
                        : frames }
                    );
                    if let Some(centers) = centers {
                        action["centers"] = json!(centers);
                    }
                    actions.push(action);
                }
                11 => actions.push(json!({ "op" : "release", "slot" : c.slot })),
                0 | 12 | 13 => {}
                op => return Err(format!("Stream {va:#x} command {op}").into()),
            }
        }
        streams.push(json!({ "va" : va, "actions" : actions }));
    }
    Ok(streams)
}
fn scene_sheets(originals: &Originals, files: &mut Files) -> Result<()> {
    let exe = originals.exe()?;
    let phantom = stream_roots(&whole(exe, PHANTOM_ROOT)?);
    let mut reserved = serde_json::Map::new();
    for (name, root) in RESERVED {
        let roots = stream_roots(&whole(exe, root)?);
        reserved.insert(
            name.into(),
            json!(
                { "root_va" : root, "streams" : streams_of(originals, & roots, root)
                ? }
            ),
        );
    }
    let sheets = json!(
        { "schema" : 1, "source_sha256" : SOURCE, "streams" : streams_of(originals, &
        phantom, PHANTOM_ROOT) ?, "reserved" : reserved }
    );
    files.push(("scene-sheets.json".into(), json(sheets)?));
    Ok(())
}
pub fn import(originals: &Originals) -> Result<Files> {
    let mut files = Files::new();
    ending(originals, &mut files)?;
    scene_sheets(originals, &mut files)?;
    Ok(files)
}
