use crate::Result;
use std::collections::BTreeMap;
pub const ENTRY: u32 = 0x444aa4;
const GROWTH: u32 = 0x4448f0;
pub const EXIT_FIELD: u8 = 1;
pub const EXIT_DEFEAT: u8 = 2;
pub const EXIT_QUIT: u8 = 3;
pub const EXIT_FINAL: u8 = 4;
pub const EXIT_SCENE: u8 = 5;
const EXITS: [(u32, u8); 4] = [
    (0x44463c, EXIT_FIELD),
    (0x444fa8, EXIT_DEFEAT),
    (0x4448fc, EXIT_QUIT),
    (0x444de8, EXIT_FINAL),
];
pub struct Controller {
    pub program: Vec<u8>,
    pub entry: u16,
    pub growth: u16,
}
fn hex(text: &str) -> Result<Vec<u8>> {
    (0..text.len())
        .step_by(2)
        .map(|i| Ok(u8::from_str_radix(&text[i..i + 2], 16)?))
        .collect()
}
fn pointer(raw: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([raw[at], raw[at + 1], raw[at + 2], raw[at + 3]])
}
fn targets(raw: &[u8]) -> Option<Vec<usize>> {
    Some(match raw[0] {
        0x03 | 0x04 | 0x59 | 0x5a | 0x5c | 0x60 | 0x61 | 0x62 | 0x65 | 0x66 | 0x89 | 0xa2
        | 0xa5 => vec![4],
        0x32 if raw[1] == 1 => vec![4],
        0x83 => vec![],
        0x13 if raw[..4] == [0x13, 0xc0, 0x12, 0x01] => vec![4],
        0x0a if raw[1] == 0x58 => (0..usize::from(raw[2])).map(|i| 4 + 4 * i).collect(),
        0x05
        | 0x4b
        | 0x4f
        | 0x51..=0x58
        | 0x5f
        | 0x63
        | 0x64
        | 0x72
        | 0x8a
        | 0x92
        | 0xa3
        | 0xaa => vec![],
        0x01 | 0x24 | 0x27 | 0x29 | 0x34 | 0x3a | 0x3b | 0x3c | 0x40 | 0x49 | 0x4e | 0x5e
        | 0x71 | 0x7c | 0x8b | 0x9b | 0x28 => vec![],
        0x26 if matches!(raw[1], 0x20 | 0x30) => vec![],
        0x38 if raw[1] == 1 => vec![],
        0x06 => vec![4],
        0x7b | 0x02 => vec![],
        0x2f if raw[1] == 1 => vec![],
        0x07 | 0x3f => vec![],
        _ => return None,
    })
}
pub fn load(content: &crate::content::Content) -> Result<Controller> {
    let actors: serde_json::Value = content.json("combat-actors.json")?;
    let mut source = BTreeMap::new();
    for node in actors["common_controller"]["graph"]["nodes"]
        .as_array()
        .ok_or("Missing battle controller")?
    {
        let va = node["va"].as_u64().ok_or("Controller node address")? as u32;
        source.insert(
            va,
            hex(node["raw_hex"].as_str().ok_or("Controller node bytes")?)?,
        );
    }
    let exit = |va: u32| {
        EXITS
            .iter()
            .find(|(at, _)| *at == va)
            .map(|(_, code)| *code)
    };
    let mut nodes: BTreeMap<u32, Vec<u8>> = BTreeMap::new();
    let mut pending = vec![ENTRY];
    let mut fall_exits: BTreeMap<u32, Vec<u8>> = BTreeMap::new();
    while let Some(va) = pending.pop() {
        if nodes.contains_key(&va) || exit(va).is_some() {
            continue;
        }
        let raw = source
            .get(&va)
            .ok_or_else(|| format!("Controller leaves its graph at {va:#x}"))?
            .clone();
        let branch = targets(&raw)
            .ok_or_else(|| format!("Controller command {} at {va:#x}", hex_of(&raw)))?;
        for at in &branch {
            pending.push(pointer(&raw, *at));
        }
        if !matches!(raw[0], 0x03 | 0x05 | 0x0a) {
            let after = va + raw.len() as u32;
            if exit(after).is_some() {
                let mut jump = vec![0x03, 0, 0, 0];
                jump.extend(after.to_le_bytes());
                fall_exits.insert(after, jump);
            }
            pending.push(after);
        }
        nodes.insert(va, raw);
    }
    nodes.extend(fall_exits);
    let mut at = BTreeMap::new();
    let mut size = 0usize;
    for (&va, raw) in &nodes {
        at.insert(va, u16::try_from(size)?);
        size += raw.len();
    }
    if size >= 0xff00 {
        return Err("Battle controller program too large".into());
    }
    let mut program = Vec::new();
    for (&va, raw) in &nodes {
        let mut raw = raw.clone();
        for o in targets(&raw).unwrap_or_default() {
            let target = pointer(&raw, o);
            let to = match exit(target) {
                Some(code) => 0xff00 | u16::from(code),
                None => *at
                    .get(&target)
                    .ok_or_else(|| format!("Controller target {target:#x}"))?,
            };
            raw[o..o + 4].copy_from_slice(&[to as u8, (to >> 8) as u8, 0, 0]);
        }
        if !matches!(raw[0], 0x03 | 0x05 | 0x0a) && !nodes.contains_key(&(va + raw.len() as u32)) {
            return Err(format!("Controller falls out of its program at {va:#x}").into());
        }
        program.extend(raw);
    }
    Ok(Controller {
        program,
        entry: at[&ENTRY],
        growth: *at
            .get(&GROWTH)
            .ok_or("Growth loop outside the controller")?,
    })
}
fn hex_of(raw: &[u8]) -> String {
    raw.iter().map(|b| format!("{b:02x}")).collect()
}
