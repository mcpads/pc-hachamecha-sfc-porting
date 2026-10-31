use crate::{
    Result, SOURCE,
    import::Files,
    message_script::{continuations, decode_message},
    original::{Exe, Originals},
    script::{Graph, Link, Options, decode_graph},
    sha,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
const B1_HANDOFF: u32 = 0x4514c8;
const B1_EVENTS: u32 = 0x451420;
const COMMON: [(&str, u32); 2] = [("field_result", 0x4b02a0), ("field_use", 0x4b0268)];
fn json(value: Value) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(&value)?;
    bytes.push(b'\n');
    Ok(bytes)
}
pub fn message(exe: &Exe, va: u32, game_controls: bool) -> Result<Value> {
    let mut message = decode_message(exe, va, game_controls)?;
    let bytes = message["source_bytes"].as_u64().ok_or("Message extent")? as usize;
    message["source_sha256"] = sha(exe.read(va, bytes)?).into();
    Ok(message)
}
fn message_links<'a>(graphs: impl IntoIterator<Item = &'a Graph>) -> BTreeSet<u32> {
    graphs
        .into_iter()
        .flat_map(|g| g.nodes.values())
        .flat_map(|n| &n.links)
        .filter_map(|l| match l {
            Link::Message(va) => Some(*va),
            _ => None,
        })
        .collect()
}
fn opening(originals: &Originals, files: &mut Files) -> Result<()> {
    let exe = originals.exe()?;
    let handoff = exe.read(B1_HANDOFF, 8)?;
    if handoff[..4] != [0x2f, 1, 1, 0] {
        return Err("Unexpected B1 message handoff".into());
    }
    let mut b1 = message(exe, u32::from_le_bytes(handoff[4..8].try_into()?), false)?;
    b1["id"] = 0.into();
    b1["event_id"] = 17.into();
    b1["scene"] = "b1".into();
    b1["handoff_va"] = B1_HANDOFF.into();
    let graph = crate::import_prologue::prologue(originals)?;
    let mut prologue = Vec::new();
    for (i, va) in message_links([&graph]).into_iter().enumerate() {
        let mut item = message(exe, va, false)?;
        item["id"] = (i + 1).into();
        item["event_id"] = Value::Null;
        item["scene"] = "prologue".into();
        prologue.push(item);
    }
    for (name, messages) in [
        ("b1-dialogue.json", vec![b1]),
        ("prologue-dialogue.json", prologue),
    ] {
        files.push((
            name.into(),
            json(json!(
                { "schema" : 1, "source_sha256" : SOURCE, "messages" : messages }
            ))?,
        ));
    }
    Ok(())
}
fn b1_events(originals: &Originals, files: &mut Files) -> Result<()> {
    let exe = originals.exe()?;
    let mut owners: BTreeMap<u32, BTreeSet<u8>> = BTreeMap::new();
    let mut count = 0;
    let mut cursor = B1_EVENTS;
    while exe.read(cursor, 1)?[0] != 255 {
        let raw = exe.read(cursor, 8)?;
        let graph = decode_graph(
            exe,
            &[u32::from_le_bytes(raw[4..8].try_into()?)],
            &Options::default(),
        )?;
        if !graph.unknowns.is_empty() {
            return Err(format!("B1 event {} unresolved", raw[0]).into());
        }
        for va in message_links([&graph]) {
            owners.entry(va).or_default().insert(raw[0]);
        }
        cursor += 8;
        count += 1;
    }
    if count != 18 {
        return Err("Unexpected B1 event inventory".into());
    }
    let messages: Vec<Value> = owners
        .iter()
        .map(|(&va, events)| {
            let mut item = message(exe, va, true)?;
            item["event_ids"] = json!(events);
            Ok(item)
        })
        .collect::<Result<_>>()?;
    files.push((
        "b1-event-dialogue.json".into(),
        json(json!({ "schema" : 1, "source_sha256" : SOURCE, "messages" : messages }))?,
    ));
    Ok(())
}
fn world(originals: &Originals, files: &mut Files) -> Result<()> {
    let exe = originals.exe()?;
    let mut messages: BTreeMap<u32, Value> = BTreeMap::new();
    let add = |va: u32, message: Value, messages: &mut BTreeMap<u32, Value>| -> Result<()> {
        if messages.get(&va).is_some_and(|m| *m != message) {
            return Err(format!("Conflicting message extraction at {va:#x}").into());
        }
        messages.insert(va, message);
        Ok(())
    };
    for map in crate::import_world::maps(exe)? {
        let scopes = crate::import_world::scopes(exe, &map)?;
        let mut addresses: Vec<u32> = message_links([&scopes.entry, &scopes.exit, &scopes.events])
            .into_iter()
            .collect();
        let mut i = 0;
        while i < addresses.len() {
            let item = message(exe, addresses[i], true)?;
            for target in continuations(&item) {
                if !addresses.contains(&target) {
                    addresses.push(target);
                }
            }
            add(addresses[i], item, &mut messages)?;
            i += 1;
        }
    }
    let mut document = json!({ "source_sha256" : SOURCE });
    for (name, root) in COMMON {
        let (mut pending, mut seen) = (vec![root], BTreeSet::new());
        let (mut found, mut unresolved) = (Vec::new(), Vec::new());
        while let Some(va) = pending.pop() {
            if !seen.insert(va) {
                continue;
            }
            let item = match message(exe, va, true) {
                Ok(item) => item,
                Err(e) if e.to_string().starts_with("Unsupported message control") => {
                    unresolved.push(json!({ "source_va" : va, "reason" : e.to_string() }));
                    continue;
                }
                Err(e) => return Err(e),
            };
            pending.extend(continuations(&item));
            add(va, item, &mut messages)?;
            found.push(va);
        }
        found.sort_unstable();
        unresolved.sort_by_key(|u| u["source_va"].as_u64());
        document[name] = json!(
            { "root_va" : root, "messages" : found, "unresolved" : unresolved }
        );
    }
    document["messages"] = json!(messages.into_values().collect::<Vec<_>>());
    files.push(("world-dialogue.json".into(), json(document)?));
    Ok(())
}
pub fn import(originals: &Originals) -> Result<Files> {
    let mut files = Files::new();
    opening(originals, &mut files)?;
    b1_events(originals, &mut files)?;
    world(originals, &mut files)?;
    battle(originals, &mut files)?;
    Ok(files)
}
const DESCRIPTOR_TABLE: u32 = 0x4bdeb0;
const CONTROLLER: [(&str, u32); 5] = [
    ("reward", 0x4a3650),
    ("level_up", 0x4b091a),
    ("status", 0x4b1376),
    ("item_refusal", 0x4a3590),
    ("field_use", 0x4b0268),
];
fn scene_messages(exe: &Exe, root: u32) -> Result<Vec<u32>> {
    let graph = decode_graph(exe, &[root], &Options::default())?;
    Ok(graph
        .nodes
        .values()
        .filter(|n| n.opcode() == 0x2f)
        .flat_map(|n| &n.links)
        .filter_map(|l| match l {
            Link::Message(va) => Some(*va),
            _ => None,
        })
        .collect())
}
fn battle(originals: &Originals, files: &mut Files) -> Result<()> {
    let exe = originals.exe()?;
    let mut roots = serde_json::Map::new();
    for d in 1..40 {
        let descriptor = exe.u32(DESCRIPTOR_TABLE + 4 * d)?;
        let slots: &[u32] = if d == 1 {
            &[0, 1, 2, 3, 5, 6, 7]
        } else {
            &[0, 1, 2, 3]
        };
        for slot in slots {
            roots.insert(
                format!("{d}:{slot}"),
                exe.u32(descriptor + 0x14 + 4 * slot)?.into(),
            );
        }
    }
    for (name, va) in CONTROLLER {
        roots.insert(name.into(), va.into());
    }
    let mut phantom = BTreeMap::new();
    let mut pending: Vec<u32> = scene_messages(exe, 0x47eb58)?
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    while let Some(va) = pending.pop() {
        if phantom.contains_key(&va) {
            continue;
        }
        let item = decode_message(exe, va, true)?;
        pending.extend(continuations(&item));
        phantom.insert(va, ());
    }
    for (i, va) in phantom.keys().enumerate() {
        roots.insert(format!("phantom_{i}"), (*va).into());
    }
    for (i, va) in scene_messages(exe, 0x4b8b30)?.into_iter().enumerate() {
        roots.insert(format!("ending_{i}"), va.into());
    }
    for (i, va) in scene_messages(exe, 0x4ab5a8)?.into_iter().enumerate() {
        roots.insert(format!("rulue_b7_{i}"), va.into());
    }
    let mut bodies: BTreeMap<u32, Value> = BTreeMap::new();
    let mut pending: Vec<u32> = roots.values().map(|v| v.as_u64().unwrap() as u32).collect();
    while let Some(va) = pending.pop() {
        if bodies.contains_key(&va) {
            continue;
        }
        let body = decode_message(exe, va, true)?;
        for t in body["tokens"].as_array().into_iter().flatten() {
            for key in ["target_va", "return_va"] {
                if let Some(target) = t.get(key)
                    && t["kind"] != "game_call"
                {
                    pending.push(target.as_u64().ok_or("Message target")? as u32);
                }
            }
            for target in t["targets"].as_array().into_iter().flatten() {
                pending.push(target.as_u64().ok_or("Message target")? as u32);
            }
        }
        bodies.insert(va, body);
    }
    files.push((
        "battle-messages.json".into(),
        json(json!(
            { "schema" : 1, "source_sha256" : SOURCE, "roots" : roots, "bodies" :
            bodies.into_values().collect::< Vec < _ >> () }
        ))?,
    ));
    Ok(())
}
