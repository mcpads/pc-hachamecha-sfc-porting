use crate::{
    Result, SOURCE,
    import::Files,
    import_world::{Event, events, maps, scopes},
    original::{Exe, Originals},
    script::{Graph, Node, Options, decode_graph},
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
fn json(value: Value) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(&value)?;
    bytes.push(b'\n');
    Ok(bytes)
}
fn u16s(exe: &Exe, at: u32, count: usize) -> Result<Vec<u16>> {
    Ok(exe
        .read(at, count * 2)?
        .chunks(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect())
}
fn i16s(exe: &Exe, at: u32, count: usize) -> Result<Vec<i16>> {
    Ok(exe
        .read(at, count * 2)?
        .chunks(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect())
}
fn i32s(exe: &Exe, at: u32, count: usize) -> Result<Vec<i32>> {
    Ok(exe
        .read(at, count * 4)?
        .chunks(4)
        .map(|c| i32::from_le_bytes(c.try_into().unwrap()))
        .collect())
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn event_json(e: &Event) -> Value {
    json!(
        { "source_va" : e.source_va, "id" : e.id, "unknown_byte" : e.unknown_byte, "x" :
        e.x, "y" : e.y, "direction_mask" : e.direction_mask, "kind" : e.kind, "script_va"
        : e.script_va }
    )
}
fn command(node: &Node) -> Value {
    json!(
        { "va" : node.va, "opcode" : node.opcode(), "bytes" : node.raw.len(), "raw_hex" :
        node.raw_hex() }
    )
}
fn world(exe: &Exe, files: &mut Files) -> Result<()> {
    let mut rows = Vec::new();
    for map in maps(exe)? {
        let count = exe.read(map.encounter_candidates_va, 1)?[0];
        rows.push(json!(
            { "id" : map.id, "descriptor_va" : map.descriptor_va, "entry_script_va" :
            map.entry_script_va, "exit_script_va" : map.exit_script_va,
            "event_table_va" : map.event_table_va, "cells_va" : map.cells_va,
            "encounter_masks_va" : map.encounter_masks_va, "encounter_candidates_va"
            : map.encounter_candidates_va, "encounter_candidates" : exe.read(map
            .encounter_candidates_va + 1, usize::from(count)) ?, "map_metadata_hex" :
            hex(exe.read(map.cells_va + 128, 4) ?), "map_group" : u16s(exe, map
            .cells_va + 130, 1) ? [0], "width" : 8, "height" : 8, "cells" : u16s(exe,
            map.cells_va, 64) ?, "encounter_masks" : exe.read(map.encounter_masks_va,
            64) ?, "events" : events(exe, & map) ?.iter().map(event_json).collect::<
            Vec < _ >> () }
        ));
    }
    let tabs: Vec<Value> = (0..4)
        .map(|group| {
            let at = 0x4a10e0 + group * 10;
            let raw = exe.read(at, 10)?;
            let entries: Vec<Value> = raw[1..9]
                .iter()
                .enumerate()
                .filter(|(_, v)| **v != 0)
                .map(|(slot, &v)| {
                    json!(
                        { "slot" : slot, "raw" : v, "map_id" : v & 31, "arrow" : v & 32
                        != 0 }
                    )
                })
                .collect();
            Ok(json!(
                { "group" : group, "source_va" : at, "raw_hex" : hex(raw), "entries"
                : entries }
            ))
        })
        .collect::<Result<_>>()?;
    files.push((
        "world.json".into(),
        json(json!({ "source_sha256" : SOURCE, "maps" : rows, "map_tabs" : tabs }))?,
    ));
    Ok(())
}
fn world_events(exe: &Exe, files: &mut Files) -> Result<()> {
    let mut commands: BTreeMap<u32, Value> = BTreeMap::new();
    for map in maps(exe)? {
        let scopes = scopes(exe, &map)?;
        for graph in [&scopes.entry, &scopes.exit, &scopes.events] {
            for node in graph.nodes.values() {
                let value = command(node);
                if commands.get(&node.va).is_some_and(|v| *v != value) {
                    return Err(format!("Conflicting source instruction at {:#x}", node.va).into());
                }
                commands.insert(node.va, value);
            }
        }
    }
    files.push((
        "world-events.json".into(),
        json(json!(
            { "source_sha256" : SOURCE, "commands" : commands.into_values()
            .collect::< Vec < _ >> () }
        ))?,
    ));
    Ok(())
}
fn b1_world(exe: &Exe, files: &mut Files) -> Result<()> {
    let b1 = &maps(exe)?[0];
    let events = events(exe, b1)?;
    if b1.event_table_va != 0x451420 || events.len() != 18 {
        return Err("Unexpected B1 event inventory".into());
    }
    let mut commands: BTreeMap<u32, Value> = BTreeMap::new();
    for event in &events {
        let graph: Graph = decode_graph(exe, &[event.script_va], &Options::default())?;
        if !graph.unknowns.is_empty() {
            return Err(format!("B1 event {} unresolved", event.id).into());
        }
        for node in graph.nodes.values() {
            commands.insert(node.va, command(node));
        }
    }
    let rays: Vec<Value> = (0..11)
        .map(|i| {
            let count = usize::from(u16s(exe, 0x4a0be0 + i * 2, 1)?[0]);
            Ok(json!(
                { "turns" : & i16s(exe, 0x4a0bf8 + i * 10, 5) ? [..count], "slots" :
                & i16s(exe, 0x4a0c68 + i * 10, 5) ? [..count] }
            ))
        })
        .collect::<Result<_>>()?;
    let world = json!(
        { "schema" : 1, "source_sha256" : SOURCE, "floor" : 1, "width" : 8, "height" : 8,
        "initial_state" : [1, 3, 7, 0], "view_rays" : rays, "view_slot_zero" : exe
        .u32(0x4a0b90) ?, "view_masks" : u16s(exe, 0x4a0b50, 4) ?, "cells" : u16s(exe,
        0x4512b4, 64) ?, "encounter_masks" : exe.read(0x451338, 64) ?, "direction_masks"
        : [1, 2, 8, 4], "events" : events.iter().map(event_json).collect::< Vec < _ >>
        (), "commands" : commands.into_values().collect::< Vec < _ >> () }
    );
    files.push(("b1-world.json".into(), json(world)?));
    let mut placements = Vec::new();
    for slot in 1..20u32 {
        let raw = exe.read(0x4a0cd8 + slot * 8, 8)?;
        let frame = u32::from_le_bytes(raw[..4].try_into()?);
        placements.push(json!(
            { "slot" : slot, "frame" : frame, "destination" :
            [u16::from_le_bytes([raw[4], raw[5]]), u16::from_le_bytes([raw[6],
            raw[7]])], "source" : i32s(exe, 0x4a0d78 + frame * 16, 4) ? }
        ));
    }
    let resources: Vec<Value> = [
        (15, "BG1_BG.CNS", 448, None),
        (9, "BG1.CNS", 400, Some(48)),
        (12, "DOOR1.CNS", 400, Some(48)),
        (13, "DOOR2.CNS", 400, Some(64)),
    ]
    .into_iter()
    .map(|(slot, name, width, key)| {
        json!(
            { "slot" : slot, "resource" : name, "width" : width, "height" : 240,
            "palette_base" : 48, "key" : key }
        )
    })
    .collect();
    let render = json!(
        { "schema" : 1, "source_sha256" : SOURCE, "source_viewport" : [224, 240],
        "backdrop_rectangles" : [i32s(exe, 0x4a0e98, 4) ?, i32s(exe, 0x4a0ea8, 4) ?],
        "resource_slots" : u16s(exe, 0x4a0b70, 16) ?, "placements" : placements,
        "resources" : resources }
    );
    files.push(("b1-render.json".into(), json(render)?));
    Ok(())
}
fn phantom_return(exe: &Exe, files: &mut Files) -> Result<()> {
    if hex(exe.read(0x4b80d0, 8)?) != "3201fc004c814b00"
        || hex(exe.read(0x4b814c, 8)?) != "3200fc0084674b00"
    {
        return Err("Unexpected map 12 talisman tests".into());
    }
    let guard = json!(
        { "map_id" : 12, "event_root_va" : 0x4b80cc, "flag" : 0xfc, "first_predicate_va"
        : 0x4b80d0, "second_predicate_va" : 0x4b814c, "contradictory_target_va" :
        0x4b6784, "resource_boundary_va" : 0x4b67a4 }
    );
    files.push((
        "phantom-return.json".into(),
        json(json!({ "source_sha256" : SOURCE, "boundary_guard" : guard }))?,
    ));
    Ok(())
}
fn prisms(exe: &Exe, files: &mut Files) -> Result<()> {
    let walls = |start: u32, end: u32| -> Result<Vec<Value>> {
        (start..end)
            .step_by(4)
            .map(|a| {
                let b = exe.read(a, 4)?;
                if b[0] != 0x84 {
                    return Err(format!("Unexpected light wall at {a:#x}").into());
                }
                Ok(json!(
                    { "va" : a, "cell" : b[1], "direction" : b[2] & 15, "mirror" :
                    b[2] & 128 != 0, "value" : b[3], "raw_hex" : hex(b) }
                ))
            })
            .collect()
    };
    let mut segments = Vec::new();
    for (flag, start, end) in [
        (0x10e, 0x4b7b3c, 0x4b7b60),
        (0x10a, 0x4b7b68, 0x4b7b88),
        (0x10b, 0x4b7b90, 0x4b7ba8),
        (0x10c, 0x4b7bb0, 0x4b7bc0),
        (0x10d, 0x4b7bc8, 0x4b7bd4),
    ] {
        if exe.read(start - 8, 4)? != [0x32, 0, (flag & 255) as u8, (flag >> 8) as u8] {
            return Err("Unexpected light segment test".into());
        }
        segments.push(json!({ "required_flag" : flag, "walls" : walls(start, end) ? }));
    }
    let light = json!(
        { "source_sha256" : SOURCE, "map_id" : 12, "rebuild_va" : 0x4b7abc, "base_walls"
        : walls(0x4b7abc, 0x4b7b34) ?, "segments" : segments }
    );
    files.push(("prisms.json".into(), json(light)?));
    Ok(())
}
pub fn import(originals: &Originals) -> Result<Files> {
    let exe = originals.exe()?;
    let mut files = Files::new();
    world(exe, &mut files)?;
    world_events(exe, &mut files)?;
    b1_world(exe, &mut files)?;
    phantom_return(exe, &mut files)?;
    prisms(exe, &mut files)?;
    b3_puzzles(exe, &mut files)?;
    puyo_trial(exe, &mut files)?;
    Ok(files)
}
fn graph_json(graph: &Graph) -> Value {
    json!(
        { "roots" : [], "nodes" : graph.nodes.values().map(command).collect::< Vec < _ >>
        () }
    )
}
fn whole(exe: &Exe, roots: &[u32]) -> Result<Graph> {
    let graph = decode_graph(exe, roots, &Options::default())?;
    if !graph.unknowns.is_empty() {
        return Err(format!("Unresolved commands from {roots:x?}").into());
    }
    Ok(graph)
}
fn b3_puzzles(exe: &Exe, files: &mut Files) -> Result<()> {
    let effect = whole(exe, &[0x4a9bfc])?;
    let expect = [
        (0x4a9bfc, "3f000000489d4a00"),
        (0x4a9c04, "07010000e49c4a00"),
        (0x4a9c2c, "02000100"),
        (0x4a9c30, "060020000c9c4a00"),
        (0x4a9c70, "2b007000"),
        (0x4a9c80, "2b008000"),
        (0x4a9c98, "01000000"),
        (0x4a9c9c, "06001800989c4a00"),
        (0x4a9d04, "11818c0000840000"),
        (0x4a9d0c, "2d090000"),
        (0x4a9d10, "12841c0000000100"),
        (0x4a9d34, "12e00c1c"),
        (0x4a9d3c, "06002000049d4a00"),
        (0x4a9c0c, "07010000489c4a00"),
        (0x4a9c14, "07010000489c4a00"),
        (0x4a9c1c, "07010000489c4a00"),
        (0x4a9c24, "07010000489c4a00"),
        (
            0x4a9ce8,
            "080000000300000000000040038000000000000002920e00ff000000",
        ),
    ];
    for (va, raw) in expect {
        if effect.nodes.get(&va).map(Node::raw_hex).as_deref() != Some(raw) {
            return Err(format!("Unexpected explosion command at {va:#x}").into());
        }
    }
    let table = u16s(exe, 0x45b3c8, 1024)?;
    let sine = |angle: u32| -> i64 {
        let a = ((angle + 8) & 0xffff) >> 4;
        let i = (a & 1023) as usize;
        let value = if a & 1024 != 0 {
            if i == 0 {
                65536
            } else {
                i64::from(table[1024 - i])
            }
        } else {
            i64::from(table[i])
        };
        if a & 2048 != 0 { -value } else { value }
    };
    let mut angle = 0;
    let offsets: Vec<i16> = (0..32)
        .map(|_| {
            angle = (angle + 0x8400) & 0xffff;
            (((sine(angle) * 14) & 0xffff_ffff) >> 16) as u16 as i16
        })
        .collect();
    let mut interactions = Vec::new();
    for (event, check) in [(6, 0x4a9bac), (5, 0x4a9d84), (17, 0x4a9e2c)] {
        let predicate = exe.read(check, 8)?;
        if predicate[..3] != [0x13, 0xc1, 0x1f] {
            return Err("Unexpected B3 item predicate".into());
        }
        let effects = if event == 6 {
            let mut graph = graph_json(&effect);
            graph["roots"] = json!([0x4a9bfc]);
            json!(
                { "graph" : graph, "explosion" : { "ticks" : 32, "shake_offsets" :
                offsets } }
            )
        } else {
            Value::Null
        };
        interactions.push(json!(
            { "event_id" : event, "item_id" : predicate[3], "predicate_va" :
            check, "effects" : effects }
        ));
    }
    files.push((
        "b3-puzzles.json".into(),
        json(json!(
            { "source_sha256" : SOURCE, "map_id" : 3, "interactions" :
            interactions }
        ))?,
    ));
    Ok(())
}
fn puyo_trial(exe: &Exe, files: &mut Files) -> Result<()> {
    let bases = [0x47bf04, 0x47c158, 0x47c3ac, 0x47c600, 0x47c854];
    for base in bases {
        let (count, item) = (exe.read(base, 8)?, exe.read(base + 0x84, 8)?);
        if count[..2] != [0x13, 0x41]
            || count[3] != 4
            || item[..3] != [0x13, 0xc1, 0x1f]
            || exe.read(base + 0x14c, 4)? != [0x81, 1, item[3], 0]
            || exe.read(base + 0x234, 1)?[0] != 0x84
        {
            return Err(format!("Unexpected colour door at {base:#x}").into());
        }
    }
    let doors = whole(exe, &bases)?;
    let roots: Vec<u32> = doors
        .nodes
        .values()
        .filter(|n| n.opcode() == 0x20)
        .map(|n| n.pointer(4))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let animations = whole(exe, &roots)?;
    let playback = json!(
        { "graph" : { "nodes" : graph_json(& doors) ["nodes"] }, "animations" : { "nodes"
        : graph_json(& animations) ["nodes"] } }
    );
    files.push((
        "puyo-trial.json".into(),
        json(json!({ "source_sha256" : SOURCE, "door_playback" : playback }))?,
    ));
    Ok(())
}
