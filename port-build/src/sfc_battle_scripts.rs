use crate::Result;
use std::collections::BTreeMap;
pub const DESCRIPTORS: usize = 40;
pub struct Scripts {
    pub code: Vec<u8>,
    pub roots: Vec<u8>,
    pub uncarried: Vec<(Vec<usize>, String)>,
}
fn hex(text: &str) -> Result<Vec<u8>> {
    (0..text.len())
        .step_by(2)
        .map(|i| Ok(u8::from_str_radix(&text[i..i + 2], 16)?))
        .collect()
}
fn targets(raw: &[u8]) -> Result<Vec<usize>> {
    Ok(match raw[0] {
        0x03 | 0x13 => vec![4],
        0x14 => vec![8],
        0x0a => (0..usize::from(raw[2])).map(|i| 4 + 4 * i).collect(),
        0x05 | 0x10 | 0x2b | 0x4b | 0x4d | 0x5d => vec![],
        op => {
            return Err(format!("Battle script command {op:02x} is not carried").into());
        }
    })
}
fn admitted(raw: &[u8]) -> bool {
    match raw[0] {
        0x05 | 0x4b => raw.len() == 4,
        0x4d => raw[1] < 3,
        0x10 => matches!(raw[1], 0xc0 | 0xc2 | 0xc7),
        0x13 => matches!(raw[1], 0xc0 | 0xc1 | 0xc4) && raw.len() == 8,
        0x14 => {
            raw.len() == 12
                && raw[6..8] == [0, 0]
                && (raw[1] == 0xc6
                    || raw[1] == 0x81 && raw[2] == 0x58 && raw[3] == 0 && raw[5] == 0)
        }
        0x03 => raw.len() == 8 && raw[1..4] == [0, 0, 0],
        0x2b => raw[1] == 0 && raw[3] == 0 && raw[2] > 0,
        0x0a => raw[1] == 0x58 && raw[2] > 0,
        0x5d => raw[1] < 16,
        _ => false,
    }
}
pub fn load(content: &crate::content::Content) -> Result<Scripts> {
    let actors: serde_json::Value = content.json("combat-actors.json")?;
    let scripts = actors["action_scripts"]["scripts"]
        .as_array()
        .ok_or("Missing action scripts")?;
    let mut nodes: BTreeMap<u32, Vec<u8>> = BTreeMap::new();
    let mut roots = vec![[0xffffu32; 2]; DESCRIPTORS];
    let mut uncarried = Vec::new();
    'scripts: for script in scripts {
        let ids: Vec<usize> = serde_json::from_value(script["actor_ids"].clone())?;
        let carried: Vec<usize> = ids.into_iter().filter(|&i| i < DESCRIPTORS).collect();
        if carried.is_empty() {
            continue;
        }
        for graph in ["graph", "reaction_graph"] {
            for node in script[graph]["nodes"]
                .as_array()
                .ok_or("Missing graph nodes")?
            {
                let raw = hex(node["raw_hex"].as_str().ok_or("Missing node bytes")?)?;
                if !admitted(&raw) && carried.iter().all(|&i| i > 10) {
                    uncarried.push((carried, node["raw_hex"].as_str().unwrap_or("").into()));
                    continue 'scripts;
                }
            }
        }
        for (slot, graph) in ["graph", "reaction_graph"].into_iter().enumerate() {
            let graph = &script[graph];
            let root = graph["roots"][0].as_u64().ok_or("Missing graph root")? as u32;
            for id in &carried {
                roots[*id][slot] = root;
            }
            for node in graph["nodes"].as_array().ok_or("Missing graph nodes")? {
                let va = node["va"].as_u64().ok_or("Missing node address")? as u32;
                let raw = hex(node["raw_hex"].as_str().ok_or("Missing node bytes")?)?;
                if !admitted(&raw) {
                    return Err(
                        format!("Battle script command {} at {va:#x}", node["raw_hex"]).into(),
                    );
                }
                nodes.insert(va, raw);
            }
        }
    }
    for (&va, raw) in &nodes {
        if raw[0] == 0x14 && raw[1] == 0x81 && nodes.get(&(va - 4)).is_none_or(|p| p[0] != 0x2b) {
            return Err(format!("Result word test at {va:#x} not after a draw").into());
        }
    }
    let mut at = BTreeMap::new();
    let mut size = 0usize;
    for (&va, raw) in &nodes {
        at.insert(va, u16::try_from(size)?);
        size += raw.len();
    }
    let offset = |va: u32| -> Result<u16> {
        at.get(&va)
            .copied()
            .ok_or_else(|| format!("Battle script target {va:#x} outside the graphs").into())
    };
    let mut code = Vec::new();
    for (&va, raw) in &nodes {
        let mut raw = raw.clone();
        for o in targets(&raw)? {
            let target = u32::from_le_bytes(raw[o..o + 4].try_into()?);
            let to = offset(target)?;
            raw[o..o + 4].copy_from_slice(&[to as u8, (to >> 8) as u8, 0, 0]);
        }
        if !matches!(raw[0], 0x03 | 0x05 | 0x0a) && !nodes.contains_key(&(va + raw.len() as u32)) {
            return Err(format!("Battle script falls out of its graph at {va:#x}").into());
        }
        code.extend(raw);
    }
    let mut table = Vec::new();
    for [action, reaction] in roots {
        for root in [action, reaction] {
            let v = if root == 0xffff {
                0xffff
            } else {
                offset(root)?
            };
            table.extend(v.to_le_bytes());
        }
    }
    Ok(Scripts {
        code,
        roots: table,
        uncarried,
    })
}
