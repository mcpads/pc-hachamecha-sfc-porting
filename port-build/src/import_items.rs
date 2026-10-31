use crate::{
    Result, SOURCE,
    import::Files,
    message_script::{continuations, decode_message},
    original::Originals,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
pub fn import(originals: &Originals) -> Result<Files> {
    let exe = originals.exe()?;
    let byte = |va: u32| -> Result<u8> { Ok(exe.read(va, 1)?[0]) };
    let word = |va: u32| -> Result<u16> { Ok(u16::from_le_bytes(exe.read(va, 2)?.try_into()?)) };
    let mut items = Vec::new();
    for id in 40..98u32 {
        let description = decode_message(exe, exe.u32(0x43f72c + id * 4)?, true)?;
        let name = description["tokens"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|t| t["kind"] == "text")
            .map(|t| t["text"].clone())
            .unwrap_or(Value::Null);
        items.push(json!({ "id" : id, "name" : name, "description" : description }));
    }
    let equipment: Vec<Value> = (88..98u32)
        .map(|id| {
            let va = if id < 93 { 0x461780 } else { 0x461783 } + id;
            Ok(json!(
                { "item_id" : id, "bonus_va" : va, "bonus" : byte(va) ?, "bonus_kind"
                : if id < 93 { "attack" } else { "defense" } }
            ))
        })
        .collect::<Result<_>>()?;
    let mut shops = Vec::new();
    for group in 0..5 {
        let (start, count) = (byte(0x4b5040 + group * 2)?, byte(0x4b5041 + group * 2)?);
        let mut entries = Vec::new();
        for index in u32::from(start)..u32::from(start) + u32::from(count) {
            let (item, stock) = (byte(0x4b4fd8 + index * 2)?, byte(0x4b4fd9 + index * 2)?);
            let price_index = if item <= 53 {
                u32::from(item) - 40
            } else {
                u32::from(item) - 74
            };
            entries.push(json!(
                { "item_id" : item, "stock_index" : index, "initial_stock" :
                stock, "price" : word(0x4b5050 + price_index * 2) ? }
            ));
        }
        shops.push(json!({ "group" : group, "entries" : entries }));
    }
    let mut commands: BTreeMap<u32, BTreeSet<u8>> = BTreeMap::new();
    for map in crate::import_world::maps(exe)? {
        let scopes = crate::import_world::scopes(exe, &map)?;
        for graph in [&scopes.entry, &scopes.exit, &scopes.events] {
            for node in graph.nodes.values().filter(|n| n.opcode() == 0x69) {
                commands.entry(node.va).or_default().insert(map.id);
            }
        }
    }
    let mut bindings = Vec::new();
    let mut pending = Vec::new();
    for (&va, maps) in &commands {
        let descriptor = exe.u32(va + 4)?;
        let messages: Vec<u32> = (0..4)
            .map(|i| exe.u32(descriptor + i * 4))
            .collect::<Result<_>>()?;
        pending.extend(&messages);
        bindings.push(json!(
            { "command_va" : va, "descriptor_va" : descriptor, "group" :
            byte(descriptor + 16) ?, "map_ids" : maps, "message_vas" : messages }
        ));
    }
    let (mut messages, mut unresolved, mut seen) = (BTreeMap::new(), Vec::new(), BTreeSet::new());
    while let Some(va) = pending.pop() {
        if !seen.insert(va) {
            continue;
        }
        let message = match decode_message(exe, va, true) {
            Ok(message) => message,
            Err(e) if e.to_string().starts_with("Unsupported message control") => {
                unresolved.push(json!({ "source_va" : va, "reason" : e.to_string() }));
                continue;
            }
            Err(e) => return Err(e),
        };
        pending.extend(continuations(&message));
        messages.insert(va, message);
    }
    unresolved.sort_by_key(|u| u["source_va"].as_u64());
    let ids: Vec<u32> = (40..54).chain(88..98).collect();
    let prices: Vec<u16> = (0..24)
        .map(|i| word(0x4b5050 + 2 * i))
        .collect::<Result<_>>()?;
    let document = json!(
        { "source_sha256" : SOURCE, "items" : items, "equipment_bonuses" : equipment,
        "shop_groups" : shops, "shop_bindings" : bindings, "shop_dialogue" : { "messages"
        : messages.into_values().collect::< Vec < _ >> (), "unresolved" : unresolved },
        "price_table" : { "va" : 0x4b5050, "item_ids" : ids, "prices" : prices } }
    );
    let mut bytes = serde_json::to_vec_pretty(&document)?;
    bytes.push(b'\n');
    Ok(vec![("items.json".into(), bytes)])
}
