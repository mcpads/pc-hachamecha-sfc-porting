use crate::{
    Result, SOURCE,
    import::Files,
    original::{Exe, Originals},
    script::{Graph, Options, decode_graph, decode_resource_script},
    sha,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
pub const FLOORS: std::ops::RangeInclusive<u8> = 1..=16;
pub struct Map {
    pub id: u8,
    pub descriptor_va: u32,
    pub entry_script_va: u32,
    pub exit_script_va: u32,
    pub event_table_va: u32,
    pub cells_va: u32,
    pub encounter_masks_va: u32,
    pub encounter_candidates_va: u32,
}
pub fn maps(exe: &Exe) -> Result<Vec<Map>> {
    if exe.u32(0x4bdf58)? != 0 {
        return Err("Map table slot 0 is not null".into());
    }
    FLOORS
        .map(|id| {
            let descriptor = exe.u32(0x4bdf58 + u32::from(id) * 4)?;
            let field = |i: u32| exe.u32(descriptor + i * 4);
            let map = Map {
                id,
                descriptor_va: descriptor,
                entry_script_va: field(0)?,
                exit_script_va: field(1)?,
                event_table_va: field(2)?,
                cells_va: field(3)?,
                encounter_masks_va: field(4)?,
                encounter_candidates_va: field(5)?,
            };
            if exe.read(map.cells_va + 128, 1)?[0] != id {
                return Err(format!("Map identity mismatch at {id}").into());
            }
            Ok(map)
        })
        .collect()
}
pub fn entry_scope(exe: &Exe, map: &Map) -> Result<Graph> {
    let options = Options {
        stop_at_yield: true,
        ..Default::default()
    };
    decode_graph(exe, &[map.entry_script_va], &options)
}
pub struct Event {
    pub source_va: u32,
    pub id: u8,
    pub unknown_byte: u8,
    pub x: u8,
    pub y: u8,
    pub direction_mask: u8,
    pub kind: u8,
    pub script_va: u32,
}
pub fn events(exe: &Exe, map: &Map) -> Result<Vec<Event>> {
    let mut events = Vec::new();
    for index in 0..256 {
        let va = map.event_table_va + index * 8;
        let raw = exe.read(va, 8)?;
        if raw[0] == 255 {
            return Ok(events);
        }
        let script_va = u32::from_le_bytes(raw[4..8].try_into()?);
        exe.read(script_va, 4)?;
        events.push(Event {
            source_va: va,
            id: raw[0],
            unknown_byte: raw[1],
            x: raw[2] & 15,
            y: raw[2] >> 4,
            direction_mask: raw[3] & 15,
            kind: raw[3] >> 4,
            script_va,
        });
    }
    Err("Missing event terminator".into())
}
pub struct Scopes {
    pub entry: Graph,
    pub exit: Graph,
    pub events: Graph,
}
pub fn scopes(exe: &Exe, map: &Map) -> Result<Scopes> {
    let lifecycle = Options {
        stop_at_yield: true,
        ..Default::default()
    };
    let entry = decode_graph(exe, &[map.entry_script_va], &lifecycle)?;
    let exit = decode_graph(exe, &[map.exit_script_va], &lifecycle)?;
    let foreign = [&entry, &exit]
        .iter()
        .flat_map(|g| g.nodes.values())
        .flat_map(|n| &n.links)
        .filter_map(|l| match l {
            crate::script::Link::Resource(va) => Some(*va),
            _ => None,
        })
        .collect();
    let roots: Vec<u32> = events(exe, map)?.iter().map(|e| e.script_va).collect();
    let events = decode_graph(
        exe,
        &roots,
        &Options {
            foreign,
            ..Default::default()
        },
    )?;
    Ok(Scopes {
        entry,
        exit,
        events,
    })
}
fn json(value: Value) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(&value)?;
    bytes.push(b'\n');
    Ok(bytes)
}
fn floor_walls(originals: &Originals, files: &mut Files) -> Result<()> {
    let exe = originals.exe()?;
    let mut sheets: BTreeMap<String, Value> = BTreeMap::new();
    let mut order = Vec::new();
    let mut floors = Vec::new();
    for map in maps(exe)? {
        let scope = entry_scope(exe, &map)?;
        let streams: Vec<_> = scope
            .nodes
            .values()
            .filter(|n| n.opcode() == 0x3f)
            .collect();
        let [stream_command] = streams[..] else {
            return Err(format!(
                "Floor {} entry has {} resource streams",
                map.id,
                streams.len()
            )
            .into());
        };
        let stream_va = stream_command.pointer(4);
        let stream = decode_resource_script(exe, stream_va)?;
        let keys: BTreeMap<i16, u8> = stream
            .iter()
            .filter(|c| c.opcode == 12)
            .map(|c| (c.slot, c.flags))
            .collect();
        let mut palette = vec![0u8; 768];
        let mut loads = Vec::new();
        for command in stream.iter().filter(|c| [1, 2].contains(&c.opcode)) {
            let name = command.file.as_deref().ok_or("Wall load without a file")?;
            let sheet = originals.cns(name)?;
            let base = usize::from(command.flags);
            let count = sheet.colors.min(256 - base);
            palette[base * 3..(base + count) * 3].copy_from_slice(&sheet.rgb[..count * 3]);
            loads.push(json!(
                { "slot" : command.slot, "file" : name, "palette_base" : base,
                "key" : keys.get(& command.slot), "width" : sheet.width, "height"
                : sheet.height, "colors" : sheet.colors }
            ));
            if !sheets.contains_key(name) {
                files.push((
                    format!("floor-walls/{name}.indices").into(),
                    sheet.pixels.clone(),
                ));
                files.push((format!("floor-walls/{name}.rgb").into(), sheet.rgb.clone()));
                order.push(name.to_string());
                sheets.insert(
                    name.to_string(),
                    json!(
                        { "file" : name, "width" : sheet.width, "height" : sheet
                        .height, "pixels_sha256" : sha(& sheet.pixels),
                        "palette_sha256" : sha(& sheet.rgb) }
                    ),
                );
            }
        }
        let mut lists = Vec::new();
        for node in scope.nodes.values().filter(|n| n.opcode() == 0x39) {
            if node.raw[1] != 0 || node.va < stream_command.va {
                return Err(format!("Floor {} entry colours at {:#x}", map.id, node.va).into());
            }
            let mut data = node.pointer(4);
            while exe.read(data, 1)?[0] != 0xff {
                let [index, r, g, b] = exe.read(data, 4)?.try_into()?;
                let at = usize::from(index) * 3;
                palette[at..at + 3].copy_from_slice(&[r << 4, g << 4, b << 4]);
                data += 4;
            }
            lists.push(node.pointer(4));
        }
        floors.push(json!(
            { "floor" : map.id, "stream_va" : stream_va, "loads" : loads,
            "colour_lists" : lists, "palette_sha256" : sha(& palette) }
        ));
        files.push((
            format!("floor-walls/floor-{}.palette", map.id).into(),
            palette,
        ));
    }
    let sheets: Vec<&Value> = order.iter().map(|n| &sheets[n]).collect();
    files.push((
        "floor-walls/floor-walls.json".into(),
        json(json!(
            { "schema" : 1, "source_sha256" : SOURCE, "sheets" : sheets, "floors"
            : floors }
        ))?,
    ));
    Ok(())
}
pub fn import(originals: &Originals) -> Result<Files> {
    let mut files = Files::new();
    floor_walls(originals, &mut files)?;
    Ok(files)
}
