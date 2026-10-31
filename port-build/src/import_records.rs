use crate::{
    Result, SOURCE,
    import::Files,
    import_dialogue::message,
    original::{Exe, Originals},
    script::{Graph, Link},
    sha,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
const CALLER: u32 = 0x2003000;
const FONTS: [[i16; 2]; 5] = [[16, 16], [12, 12], [24, 20], [32, 32], [64, 64]];
fn json(value: Value) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(&value)?;
    bytes.push(b'\n');
    Ok(bytes)
}
fn patches() -> Vec<Value> {
    let mut patches = vec![
        (0x4c3144u32, 0x2006000u32),
        (0x4c3138, 0x4b01f8),
        (0x4c3014, 0x4b0248),
        (0x4c4748, 0x4b01e0),
    ];
    for index in 0..5 {
        patches.push((0x4c3018 + index * 4, 0x2005000 + index * 8));
        patches.push((0x4c4750 + index * 4, 0x500 + index));
    }
    patches
        .into_iter()
        .map(|(address, value)| json!({ "address" : address, "width" : 4, "value" : value }))
        .collect()
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
fn messages(exe: &Exe, graph: &Graph) -> Result<Vec<Value>> {
    let handoff = exe.read(0x4514c8, 8)?;
    let mut b1 = message(exe, u32::from_le_bytes(handoff[4..8].try_into()?), false)?;
    b1["id"] = 0.into();
    b1["event_id"] = 17.into();
    b1["scene"] = "b1".into();
    b1["handoff_va"] = 0x4514c8.into();
    let mut out = vec![b1];
    let links: std::collections::BTreeSet<u32> = graph
        .nodes
        .values()
        .flat_map(|n| &n.links)
        .filter_map(|l| match l {
            Link::Message(va) => Some(*va),
            _ => None,
        })
        .collect();
    for (i, va) in links.into_iter().enumerate() {
        let mut item = message(exe, va, false)?;
        item["id"] = (i + 1).into();
        item["event_id"] = Value::Null;
        item["scene"] = "prologue".into();
        out.push(item);
    }
    Ok(out)
}
fn command_json(c: &crate::script::ResourceCommand) -> Value {
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
pub fn import(originals: &Originals) -> Result<Files> {
    let exe = originals.exe()?;
    let mut files = Files::new();
    let b1 = &crate::import_world::maps(exe)?[0];
    if b1.descriptor_va != 0x4511d0 || b1.cells_va != 0x4512b4 {
        return Err("Unexpected B1 descriptor".into());
    }
    let mut records = Vec::new();
    for (role, name, address, size) in [
        ("cells", "b1-cells.bin", b1.cells_va, 128),
        (
            "encounter_masks",
            "b1-encounter-masks.bin",
            b1.encounter_masks_va,
            64,
        ),
        ("direction_deltas", "direction-deltas.bin", 0x4a0b58, 16),
    ] {
        let bytes = exe.read(address, size)?.to_vec();
        records.push(json!(
            { "role" : role, "file" : name, "source_va" : address, "bytes" :
            size, "sha256" : sha(& bytes) }
        ));
        files.push((format!("records/{name}").into(), bytes));
    }
    let graph = crate::import_prologue::prologue(originals)?;
    let messages = messages(exe, &graph)?;
    let mut roots: Vec<u32> = resource_links(&graph);
    roots.push(0x4513b8);
    roots.sort_unstable();
    roots.dedup();
    let mut commands: Vec<crate::script::ResourceCommand> = Vec::new();
    for root in roots {
        for c in crate::script::decode_resource_script(exe, root)? {
            if let Some(at) = commands.iter().position(|d| d.va == c.va) {
                commands[at] = c;
            } else {
                commands.push(c);
            }
        }
    }
    let names: std::collections::BTreeSet<&str> =
        commands.iter().filter_map(|c| c.file.as_deref()).collect();
    let assets: Vec<Value> = names
        .iter()
        .map(|&name| {
            let sheet = originals.cns(name)?;
            Ok(json!({ "file" : name, "colors" : & sheet.rgb[..sheet.colors * 3] }))
        })
        .collect::<Result<_>>()?;
    let resources = json!(
        { "commands" : commands.iter().map(command_json).collect::< Vec < _ >> (),
        "assets" : assets }
    );
    let table: Vec<u16> = exe
        .read(0x45b3c8, 2048)?
        .chunks(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    let motion = json!(
        { "table_source_va" : 0x45b3c8, "table" : table, "table_sha256" : sha(exe
        .read(0x45b3c8, 2048) ?) }
    );
    let windows: Vec<Vec<i32>> = (0..8)
        .map(|i| {
            Ok(exe
                .read(0x443de9 + i * 16, 16)?
                .chunks(4)
                .map(|c| i32::from_le_bytes(c.try_into().unwrap()) / 65536)
                .collect())
        })
        .collect::<Result<_>>()?;
    let mut characters: Vec<char> = messages
        .iter()
        .flat_map(|m| m["tokens"].as_array().into_iter().flatten())
        .filter_map(|t| t["text"].as_str())
        .flat_map(str::chars)
        .collect();
    characters.sort_unstable();
    characters.dedup();
    let encoding: Vec<[u32; 2]> = characters
        .iter()
        .map(|&ch| {
            let mut buffer = [0; 4];
            let (encoded, _, failed) = encoding_rs::EUC_KR.encode(ch.encode_utf8(&mut buffer));
            if failed || encoded.len() != 2 {
                return Err(format!("Expected a two-byte source glyph for {ch}").into());
            }
            Ok([
                u32::from(u16::from_be_bytes([encoded[0], encoded[1]])),
                u32::from(ch),
            ])
        })
        .collect::<Result<_>>()?;
    let mut program: Vec<Value> = graph
        .nodes
        .values()
        .map(|n| json!({ "va" : n.va, "bytes" : n.raw }))
        .collect();
    let mut caller = vec![4, 0, 0, 0];
    caller.extend(crate::import_prologue::PROLOGUE_ROOT.to_le_bytes());
    program.push(json!({ "va" : CALLER, "bytes" : caller }));
    program.push(json!({ "va" : CALLER + 8, "bytes" : [1, 0, 0, 0] }));
    program.push(json!({ "va" : CALLER + 12, "bytes" : [0, 0, 0, 0] }));
    let mut controls: Vec<Value> = Vec::new();
    let mut seen: BTreeMap<u64, usize> = BTreeMap::new();
    let mut banks = serde_json::Map::new();
    for token in messages
        .iter()
        .flat_map(|m| m["tokens"].as_array().into_iter().flatten())
    {
        let va = token["va"].as_u64().ok_or("Token address")? as u32;
        let size = token["bytes"].as_u64().ok_or("Token size")? as u32;
        let pieces: Vec<(u32, u32)> = if token["kind"] == "text" {
            (0..size).step_by(2).map(|i| (va + i, 2)).collect()
        } else {
            vec![(va, size)]
        };
        for (at, n) in pieces {
            let control = json!({ "va" : at, "bytes" : exe.read(at, n as usize) ? });
            match seen.get(&u64::from(at)) {
                Some(&i) => controls[i] = control,
                None => {
                    seen.insert(u64::from(at), controls.len());
                    controls.push(control);
                }
            }
        }
        if token["kind"] == "audio" {
            let bank = exe.u32(va + 4)?;
            banks.insert(
                bank.to_string(),
                String::from_utf8(exe.cstring(bank, 128)?.to_vec())?.into(),
            );
        }
    }
    for node in graph.nodes.values().filter(|n| n.opcode() == 0x26) {
        let bank = node.pointer(4);
        banks.insert(
            bank.to_string(),
            String::from_utf8(exe.cstring(bank, 128)?.to_vec())?.into(),
        );
    }
    let mut game_state = exe.read(0x4b47c0, 256)?.to_vec();
    game_state[0x4b4808 - 0x4b47c0] = 1;
    let renderer = json!(
        { "fonts" : FONTS, "windows" : (0..5).map(| i | Ok(exe.read(0x4b01f8 + i * 16,
        16) ?.chunks(4).map(| c | i32::from_le_bytes(c.try_into().unwrap())).collect::<
        Vec < _ >> ())).collect::< Result < Vec < _ >>> () ?, "palette" : (0..6).map(| i
        | Ok(exe.read(0x4b01e0 + i * 3, 3) ?.to_vec())).collect::< Result < Vec < _ >>>
        () ?, "banks" : banks }
    );
    let motion_reference: crate::motion::Reference = serde_json::from_value(motion.clone())?;
    let resource_reference: crate::resources::Reference =
        serde_json::from_value(resources.clone())?;
    let mut cases = Vec::new();
    for policy in ["paced", "skip"] {
        let mut case = json!(
            { "kind" : "full_prologue_control", "policy" : policy, "root_group" : 0,
            "root" : CALLER, "commands" : program, "message_commands" : controls,
            "game_state" : game_state, "initial_patches" : patches(), "renderer" :
            renderer, "palette_initial" : { "current" : vec![0; 1024], "target" : vec![0;
            1024], "dirty" : 0 }, "initial_sha256" : "", "after_sha256" : [] }
        );
        let input: crate::world::Case = serde_json::from_value(case.clone())?;
        let recorded = crate::world::record(
            &input,
            Some(&motion_reference),
            Some(&resource_reference),
            exe,
        )?;
        for (k, v) in recorded.as_object().unwrap() {
            case[k] = v.clone();
        }
        cases.push(case);
    }
    let vm = json(json!(
        { "text_encoding" : encoding, "scene_windows" : windows, "resources" :
        resources, "commands" : graph.nodes.values().map(| n | json!({ "va" : n.va,
        "bytes" : n.raw })).collect::< Vec < _ >> (), "motion" : motion, "pool_cases"
        : cases }
    ))?;
    let events: Vec<Value> = crate::import_world::events(exe, b1)?
        .iter()
        .map(|e| {
            json!(
                { "id" : e.id, "x" : e.x, "y" : e.y, "direction_mask" : e.direction_mask,
                "kind" : e.kind }
            )
        })
        .collect();
    let masks: Vec<u16> = exe
        .read(0x4a1158, 8)?
        .chunks(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    let manifest = json!(
        { "schema" : 8, "source_sha256" : SOURCE, "floor" : 1, "width" : 8, "height" : 8,
        "coverage" : "partial_b1_and_prologue", "records" : records, "initial_state" :
        [1, 3, 7, 0], "direction_masks" : masks, "events" : events, "messages" :
        messages, "vm_reference_sha256" : sha(& vm) }
    );
    files.push(("records/prologue-vm.json".into(), vm));
    files.push(("records/manifest.json".into(), json(manifest)?));
    Ok(files)
}
