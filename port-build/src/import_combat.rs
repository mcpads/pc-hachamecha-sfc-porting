use crate::{
    Result, SOURCE,
    import::Files,
    original::{Exe, Originals},
    script::{Graph, Link, Node, Options, ResourceCommand, decode_graph, decode_resource_script},
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
const DESCRIPTORS: (u32, u32) = (0x4bdeb0, 0x4bdf58);
const CONTROLLER_EXITS: [u32; 4] = [0x444330, 0x44463c, 0x4448fc, 0x444e20];
const FRAME_COUNTS: [(&str, u32); 13] = [
    ("SCU.CNS", 24),
    ("PUYO.CNS", 41),
    ("KAR.CNS", 53),
    ("ANM.CNS", 22),
    ("ANM_ATK.CNS", 8),
    ("ANM_HEAL.CNS", 10),
    ("A_DEAD.CNS", 3),
    ("EFECT.CNS", 24),
    ("AT_EFECT.CNS", 60),
    ("DOKU.CNS", 22),
    ("SKELTON.CNS", 21),
    ("RULUE.CNS", 20),
    ("RULUE_A.CNS", 20),
];
const IDLE: [u32; 10] = [
    0x490258, 0x4708f8, 0x456d30, 0x4a4f38, 0x432eb4, 0x496ce4, 0x44bbb8, 0x43de88, 0x43cc34,
    0x4a15e0,
];
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn node_json(node: &Node) -> Value {
    let links: Vec<Value> = node
        .links
        .iter()
        .map(|l| match l {
            Link::Message(va) => json!({ "vm" : "message", "va" : va }),
            Link::Resource(va) => json!({ "vm" : "resource", "va" : va }),
            Link::Descriptor(va) => json!({ "vm" : "game_descriptor", "va" : va }),
        })
        .collect();
    json!(
        { "va" : node.va, "opcode" : node.opcode(), "bytes" : node.raw.len(), "raw_hex" :
        node.raw_hex(), "successors" : node.successors, "links" : links }
    )
}
fn graph_json(roots: &[u32], graph: &Graph) -> Value {
    json!(
        { "roots" : roots, "nodes" : graph.nodes.values().map(node_json).collect::< Vec <
        _ >> () }
    )
}
fn command_json(c: &ResourceCommand) -> Value {
    let mut value = json!({ "va" : c.va, "opcode" : c.opcode, "bytes" : c.raw.len() });
    if c.opcode != 0 {
        value["flags"] = c.flags.into();
        value["slot"] = c.slot.into();
    }
    if let Some(file) = &c.file {
        value["file"] = file.clone().into();
        value["filename_va"] = u32::from_le_bytes(c.raw[4..8].try_into().unwrap()).into();
        value["arguments"] = json!(c.arguments);
    }
    value
}
fn i32s(exe: &Exe, at: u32, count: u32) -> Result<Vec<i32>> {
    Ok(exe
        .read(at, count as usize * 4)?
        .chunks(4)
        .map(|c| i32::from_le_bytes(c.try_into().unwrap()))
        .collect())
}
struct Descriptor {
    va: u32,
    load_va: u32,
    object_va: u32,
    action_va: u32,
    reaction_va: u32,
}
#[derive(Default)]
struct Streams {
    order: Vec<u32>,
    commands: BTreeMap<u32, Vec<ResourceCommand>>,
}
impl Streams {
    fn add(&mut self, exe: &Exe, va: u32) -> Result<()> {
        if let std::collections::btree_map::Entry::Vacant(slot) = self.commands.entry(va) {
            slot.insert(decode_resource_script(exe, va)?);
            self.order.push(va);
        }
        Ok(())
    }
    fn in_order(&self) -> impl Iterator<Item = &Vec<ResourceCommand>> {
        self.order.iter().map(|va| &self.commands[va])
    }
}
fn resource_links(graph: &Graph) -> Vec<u32> {
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
pub fn import(originals: &Originals) -> Result<Files> {
    let exe = originals.exe()?;
    let party: Vec<Value> = [0x4b4848u32, 0x4b4878]
        .iter()
        .enumerate()
        .map(|(slot, &va)| {
            Ok(json!(
                { "slot" : slot, "stats_va" : va, "raw_hex" : hex(exe.read(va, 32) ?)
                }
            ))
        })
        .collect::<Result<_>>()?;
    let spells = json!(
        { "field_va" : 0x4b486e, "initial_values" : exe.read(0x4b486e, 8) ?,
        "mp_cost_table_va" : 0x4b4fc6, "actions" : (1..9u32).map(| i | Ok(json!({
        "action_id" : i, "command_id" : i + 31, "mp_cost" : u16::from_le_bytes(exe
        .read(0x4b4fc6 + i * 2, 2) ?.try_into() ?) }))).collect::< Result < Vec < _ >>>
        () ? }
    );
    let mut entries = Vec::new();
    let mut entry_descriptors = Vec::new();
    let mut descriptors: Vec<Descriptor> = Vec::new();
    for i in 0..(DESCRIPTORS.1 - DESCRIPTORS.0) / 4 {
        let address = exe.u32(DESCRIPTORS.0 + 4 * i)?;
        let stats_va = 0x4b48d8 + i * 32;
        let stats = exe.read(stats_va, 32)?;
        if stats[..2] != [2, i as u8] {
            return Err(format!("Unexpected enemy record {i}").into());
        }
        entries.push(json!(
            { "id" : i, "descriptor_va" : address, "stats_va" : stats_va,
            "stats_hex" : hex(stats) }
        ));
        entry_descriptors.push(address);
        if address == 0 || descriptors.iter().any(|d| d.va == address) {
            continue;
        }
        let words: Vec<u32> = (0..5)
            .map(|k| exe.u32(address + 4 * k))
            .collect::<Result<_>>()?;
        descriptors.push(Descriptor {
            va: address,
            load_va: words[0],
            object_va: words[2],
            action_va: words[3],
            reaction_va: words[4],
        });
    }
    let descriptor_of = |id: usize| -> Result<&Descriptor> {
        descriptors
            .iter()
            .find(|d| d.va == entry_descriptors[id])
            .ok_or_else(|| format!("Actor {id} descriptor").into())
    };
    let controller = decode_graph(
        exe,
        &[0x444aa4],
        &Options {
            external: CONTROLLER_EXITS.into(),
            ..Default::default()
        },
    )?;
    if !controller.unknowns.is_empty() {
        return Err("Incomplete common battle graph".into());
    }
    let scripts: Vec<Value> = descriptors
        .iter()
        .map(|d| {
            let ids: Vec<usize> = entry_descriptors
                .iter()
                .enumerate()
                .filter(|(_, v)| **v == d.va)
                .map(|(i, _)| i)
                .collect();
            let graph = decode_graph(exe, &[d.action_va], &Options::default())?;
            let reaction = decode_graph(exe, &[d.reaction_va], &Options::default())?;
            Ok(json!(
                { "descriptor_va" : d.va, "root_va" : d.action_va, "actor_ids" : ids,
                "graph" : graph_json(& [d.action_va], & graph), "reaction_graph" :
                graph_json(& [d.reaction_va], & reaction) }
            ))
        })
        .collect::<Result<_>>()?;
    let loaders = Options {
        stop_at_yield: true,
        ..Default::default()
    };
    let mut early = Streams::default();
    early.add(exe, 0x444574)?;
    let mut actors = Vec::new();
    for (k, id) in (1..11usize).enumerate() {
        let d = descriptor_of(id)?;
        let graph = decode_graph(exe, &[d.load_va], &loaders)?;
        if !graph.unknowns.is_empty() {
            return Err("Unresolved early battle resource loader".into());
        }
        for va in resource_links(&graph) {
            early.add(exe, va)?;
        }
        if exe.read(IDLE[k], 4)? != [0x13, 0xc0, 0x41, 0x01]
            || exe.read(IDLE[k] - 4, 4)? != [1, 0, 0, 0]
        {
            return Err(format!("Unexpected idle predicate of actor {id}").into());
        }
        actors.push(json!(
            { "id" : id, "descriptor_va" : d.va, "object_va" : d.object_va,
            "load_graph" : graph_json(& [d.load_va], & graph) }
        ));
    }
    let counts: BTreeMap<&str, u32> = FRAME_COUNTS.into();
    let mut boundaries: BTreeSet<u32> = early
        .in_order()
        .flatten()
        .filter(|c| [1, 5].contains(&c.opcode))
        .map(|c| c.arguments[0])
        .collect();
    boundaries.extend(descriptors.iter().map(|d| d.load_va));
    let mut rectangles = Vec::new();
    for c in early
        .in_order()
        .flatten()
        .filter(|c| [1, 5].contains(&c.opcode))
    {
        let file = c.file.as_deref().ok_or("Load without a file")?;
        let table = c.arguments[0];
        let count = *counts
            .get(file)
            .ok_or_else(|| format!("No frame count for {file}"))?;
        let end = table + count * 16;
        let following = *boundaries
            .range(table + 1..)
            .next()
            .ok_or("Frame table end")?;
        if following < end
            || following - end > 3
            || exe
                .read(end, (following - end) as usize)?
                .iter()
                .any(|&b| b != 0)
        {
            return Err(format!("Frame table {table:#x} does not end at {following:#x}").into());
        }
        let frames: Vec<Value> = (0..count)
            .map(|i| Ok(json!({ "index" : i, "rect" : i32s(exe, table + i * 16, 4) ? })))
            .collect::<Result<_>>()?;
        rectangles.push(json!(
            { "resource_va" : c.va, "slot" : c.slot, "file" : file, "table_va" :
            table, "end_va" : end, "following_va" : following, "frames" : frames
            }
        ));
    }
    let known: BTreeSet<u32> = rectangles
        .iter()
        .map(|r| r["table_va"].as_u64().unwrap() as u32)
        .collect();
    let mut later = Streams::default();
    let mut graphs = Vec::new();
    for id in 11..40usize {
        let d = descriptor_of(id)?;
        let graph = decode_graph(exe, &[d.load_va], &loaders)?;
        if !graph.unknowns.is_empty() {
            return Err(format!("Unresolved battle resource loader of actor {id}").into());
        }
        for va in resource_links(&graph) {
            later.add(exe, va)?;
        }
        actors.push(json!(
            { "id" : id, "descriptor_va" : d.va, "object_va" : d.object_va,
            "load_graph" : graph_json(& [d.load_va], & graph) }
        ));
        graphs.push(graph);
    }
    let derived = |c: &ResourceCommand| c.opcode == 10 && c.flags == 7;
    let mut boundaries: BTreeSet<u32> = later
        .in_order()
        .flatten()
        .filter(|c| [1, 4, 5].contains(&c.opcode))
        .map(|c| c.arguments[0])
        .collect();
    for c in later.in_order().flatten().filter(|c| derived(c)) {
        boundaries.insert(exe.u32(c.va + 8)?);
    }
    boundaries.extend(later.order.iter().copied());
    boundaries.extend(known);
    boundaries.extend(descriptors.iter().flat_map(|d| [d.load_va, d.va]));
    boundaries.extend(graphs.iter().flat_map(|g| g.nodes.keys().copied()));
    let mut sizes: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for stream in later.in_order() {
        let mut files: BTreeMap<i16, String> = BTreeMap::new();
        for c in stream {
            if let Some(file) = &c.file {
                files.insert(c.slot, file.clone());
            }
            if ![1, 2, 4, 5, 6].contains(&c.opcode) && !derived(c) {
                continue;
            }
            let (file, table, kind, source) = if c.opcode == 10 {
                let source = i16::from_le_bytes(exe.read(c.va + 4, 2)?.try_into()?);
                (
                    files
                        .get(&source)
                        .cloned()
                        .ok_or("Derived surface source")?,
                    exe.u32(c.va + 8)?,
                    4,
                    Some(source),
                )
            } else {
                let kind = match c.opcode {
                    1 | 5 => 3,
                    2 | 6 => 1,
                    _ => 4,
                };
                (
                    c.file.clone().ok_or("Load without a file")?,
                    c.arguments[0],
                    kind,
                    None,
                )
            };
            if !sizes.contains_key(&file) {
                let sheet = originals.cns(&file)?;
                sizes.insert(file.clone(), (sheet.width, sheet.height));
            }
            let (width, height) = sizes[&file];
            let mut entry = json!(
                { "resource_va" : c.va, "slot" : c.slot, "file" : file, "kind" : kind }
            );
            let rects: Vec<Vec<i32>> = if kind == 1 {
                let (w, h) = ((table & 0xffff) as usize, (table >> 16) as usize);
                let mut rects = Vec::new();
                if w > 0 && h > 0 && h <= height && w <= width {
                    for y in (0..=height - h).step_by(h) {
                        for x in (0..=width - w).step_by(w) {
                            rects.push([x, y, x + w, y + h].map(|v| v as i32).to_vec());
                        }
                    }
                }
                entry["table_va"] = Value::Null;
                entry["end_va"] = Value::Null;
                entry["following_va"] = Value::Null;
                entry["grid"] = json!([table & 0xffff, table >> 16]);
                rects
            } else {
                let size = if kind == 3 { 16 } else { 24 };
                let following = *boundaries
                    .range(table + 1..)
                    .next()
                    .ok_or("Frame table end")?;
                let count = (following - table) / size;
                let end = table + count * size;
                if following - end > 3
                    || exe
                        .read(end, (following - end) as usize)?
                        .iter()
                        .any(|&b| b != 0)
                {
                    return Err(
                        format!("Frame table {table:#x} does not end at {following:#x}").into(),
                    );
                }
                entry["table_va"] = table.into();
                entry["end_va"] = end.into();
                entry["following_va"] = following.into();
                if kind == 4 {
                    let centres: Vec<Vec<i32>> = (0..count)
                        .map(|i| i32s(exe, table + size * i + 16, 2))
                        .collect::<Result<_>>()?;
                    entry["centres"] = json!(centres);
                }
                (0..count)
                    .map(|i| i32s(exe, table + size * i, 4))
                    .collect::<Result<_>>()?
            };
            if rects.is_empty()
                || !rects.iter().all(|r| {
                    0 <= r[0]
                        && r[0] < r[2]
                        && r[2] as usize <= width
                        && 0 <= r[1]
                        && r[1] < r[3]
                        && r[3] as usize <= height
                })
            {
                return Err(format!("Frame outside {} ({table:#x})", entry["file"]).into());
            }
            entry["frames"] = json!(
                rects
                    .iter()
                    .enumerate()
                    .map(|(i, r)| json!({ "index" : i, "rect" : r
                    }))
                    .collect::<Vec<_>>()
            );
            if let Some(source) = source {
                entry["derived_from_slot"] = source.into();
            }
            rectangles.push(entry);
        }
    }
    let mut streams: Vec<Value> = early
        .commands
        .iter()
        .map(|(va, c)| {
            json!(
                { "root_va" : va, "commands" : c.iter().map(command_json).collect::< Vec
                < _ >> () }
            )
        })
        .collect();
    streams.extend(
        later
            .commands
            .iter()
            .filter(|(va, _)| !early.commands.contains_key(va))
            .map(|(va, c)| {
                json!(
                    { "root_va" : va, "commands" : c.iter().map(command_json)
                    .collect::< Vec < _ >> () }
                )
            }),
    );
    let actors_json = json!(
        { "source_sha256" : SOURCE, "table_va" : DESCRIPTORS.0, "party_initial" : party,
        "player_spells" : spells, "entries" : entries, "common_controller" : { "graph" :
        graph_json(& [0x444aa4], & controller) }, "action_scripts" : { "scripts" :
        scripts }, "entry_resources" : { "actors" : actors, "resource_streams" : streams,
        "frame_rectangles" : rectangles } }
    );
    let mut bytes = serde_json::to_vec_pretty(&actors_json)?;
    bytes.push(b'\n');
    Ok(vec![("combat-actors.json".into(), bytes)])
}
