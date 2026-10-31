use crate::{
    Result, SOURCE,
    import::Files,
    message_script::{continuations, decode_message},
    original::Originals,
    script::{Options, decode_graph},
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
#[rustfmt::skip]
const ARLE_ANCHORS: [(u8, u16, i32, i32); 33] = [
    (27, 0, 408, 80),
    (27, 1, 392, 80),
    (27, 2, 392, 80),
    (27, 3, 408, 80),
    (27, 4, 408, 80),
    (27, 5, 392, 80),
    (27, 6, 392, 80),
    (27, 7, 408, 80),
    (27, 8, 408, 80),
    (27, 9, 392, 80),
    (27, 10, 392, 80),
    (27, 11, 408, 80),
    (28, 0, 392, 80),
    (28, 1, 392, 80),
    (28, 2, 392, 80),
    (28, 3, 376, 80),
    (28, 4, 376, 80),
    (28, 5, 376, 80),
    (28, 6, 376, 80),
    (28, 7, 392, 80),
    (29, 0, 412, 80),
    (29, 1, 412, 80),
    (29, 2, 416, 80),
    (29, 3, 404, 80),
    (29, 4, 400, 80),
    (29, 5, 404, 80),
    (29, 6, 412, 80),
    (29, 7, 384, 80),
    (29, 8, 400, 80),
    (29, 9, 404, 80),
    (65, 0, 392, 128),
    (65, 1, 494, 80),
    (65, 2, 408, 80),
];
fn json(value: Value) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(&value)?;
    bytes.push(b'\n');
    Ok(bytes)
}
pub fn import(originals: &Originals) -> Result<Files> {
    let exe = originals.exe()?;
    let mut files = Files::new();
    let (bonus, end) = (0x4617d4u32, 0x461783u32 + 97 + 1);
    let run = exe.read(bonus, (end - bonus) as usize)?;
    files.push((
        "battle-tables.json".into(),
        json(json!(
            { "schema" : 1, "source_sha256" : SOURCE, "bonus_va" : bonus,
            "bonus_hex" : run.iter().map(| b | format!("{b:02x}")).collect::<
            String > (), "spell_levels" : & run[..5], "staff_index_offset" :
            bonus - 0x461780, "ring_index_offset" : bonus - 0x461783 }
        ))?,
    ));
    let thresholds: Vec<u32> = (0..100)
        .map(|i| exe.u32(0x4b4e18 + 4 * i))
        .collect::<Result<_>>()?;
    if thresholds[0] != 0 || thresholds.windows(2).any(|w| w[0] >= w[1]) {
        return Err("Unexpected level thresholds".into());
    }
    let pairs = exe.read(0x4b4fa8, 32)?;
    let (mut messages, mut pending) = (BTreeMap::new(), vec![0x4b091au32]);
    while let Some(va) = pending.pop() {
        if messages.contains_key(&va) {
            continue;
        }
        let message = decode_message(exe, va, true)?;
        pending.extend(continuations(&message));
        messages.insert(va, message);
    }
    let dispatch = messages
        .values()
        .flat_map(|m| m["tokens"].as_array().into_iter().flatten())
        .find(|t| t["kind"] == "message_dispatch")
        .ok_or("Level-up dispatch")?
        .clone();
    let mut learned = Vec::new();
    for i in 0..16 {
        let target = dispatch["targets"][i + 1]
            .as_u64()
            .ok_or("Learned notice")? as u32;
        let text: String = messages[&target]["tokens"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|t| t["text"].as_str())
            .collect();
        let name = text
            .split_once('「')
            .and_then(|(_, rest)| rest.split_once('」'))
            .map(|(name, _)| name)
            .ok_or("Learned spell name")?;
        learned.push(json!(
            { "level" : pairs[2 * i], "slot" : pairs[2 * i + 1], "notice" : i +
            1, "message_va" : target, "spell_name" : name }
        ));
    }
    files.push((
        "level-growth.json".into(),
        json(json!(
            { "source_sha256" : SOURCE, "thresholds_va" : 0x4b4e18, "thresholds"
            : thresholds, "learned_slots_va" : 0x4b4fa8, "learned_slots" :
            learned, "level_cap" : 99, "display_steps" : 57 }
        ))?,
    ));
    let root = exe.u32(0x4b88b8)?;
    if root != 0x4b8b30 {
        return Err("Ending descriptor does not name its body".into());
    }
    let transition = decode_graph(
        exe,
        &[0x444db4],
        &Options {
            external: [root, 0x444e2c, 0x444c9c].into(),
            ..Default::default()
        },
    )?;
    if !transition.unknowns.is_empty() {
        return Err("Ending caller not decoded".into());
    }
    let commands: Vec<Value> = transition
        .nodes
        .values()
        .map(|n| {
            json!(
                { "va" : n.va, "opcode" : n.opcode(), "bytes" : n.raw.len(), "raw_hex" :
                n.raw_hex() }
            )
        })
        .collect();
    files.push((
        "ending.json".into(),
        json(json!(
            { "source_sha256" : SOURCE, "descriptor_va" : 0x4b88b8, "script_va" :
            root, "transition_commands" : commands }
        ))?,
    ));
    let anchors: Vec<Value> = ARLE_ANCHORS
        .iter()
        .map(|&(slot, frame, x, y)| json!({ "slot" : slot, "frame" : frame, "x" : x, "y" : y }))
        .collect();
    files.push((
        "battle-arle-anchors.json".into(),
        json(json!(
            { "schema" : 1, "source_sha256" : SOURCE, "descriptor" : 1, "anchors"
            : anchors }
        ))?,
    ));
    Ok(files)
}
const IMAGE: (u32, u32) = (0x428000, 0x4be000);
const SINE: (u32, usize) = (0x45b3c8, 0x802);
const ATAN: (u32, usize) = (0x45bbd0 - 2048, 2048 + 4096);
#[rustfmt::skip]
const BODY_WIDTHS: [(u8, u32); 47] = [
    (0x00, 4),
    (0x01, 4),
    (0x02, 4),
    (0x03, 8),
    (0x06, 8),
    (0x07, 8),
    (0x10, 4),
    (0x13, 8),
    (0x18, 8),
    (0x1e, 4),
    (0x1f, 4),
    (0x20, 8),
    (0x21, 8),
    (0x24, 4),
    (0x25, 4),
    (0x27, 4),
    (0x28, 4),
    (0x2b, 4),
    (0x2d, 4),
    (0x32, 8),
    (0x38, 8),
    (0x39, 8),
    (0x3a, 4),
    (0x3b, 4),
    (0x3c, 4),
    (0x3d, 4),
    (0x4b, 4),
    (0x4d, 4),
    (0x5b, 4),
    (0x67, 4),
    (0x68, 4),
    (0x6f, 4),
    (0x80, 12),
    (0x81, 4),
    (0xa2, 8),
    (0x31, 4),
    (0x88, 8),
    (0x4c, 4),
    (0x2f, 8),
    (0x3f, 8),
    (0x29, 4),
    (0x35, 4),
    (0x05, 4),
    (0x26, 8),
    (0x8b, 4),
    (0x8d, 12),
    (0x0a, 0),
];
#[rustfmt::skip]
const STATE_TABLES: [(u32, u32); 58] = [
    (0x49027c, 19),
    (0x47091c, 8),
    (0x456d50, 14),
    (0x4a4f58, 16),
    (0x432ed4, 13),
    (0x496d04, 14),
    (0x44bbd8, 14),
    (0x43deac, 15),
    (0x43cc58, 14),
    (0x4a1604, 15),
    (0x4a1604, 15),
    (0x49dd20, 13),
    (0x473b10, 16),
    (0x42d7b8, 14),
    (0x4b1b78, 15),
    (0x4a0038, 13),
    (0x49ea5c, 15),
    (0x49ef7c, 15),
    (0x49f3f0, 15),
    (0x49f574, 15),
    (0x48c460, 13),
    (0x47669c, 14),
    (0x46cd7c, 12),
    (0x42e678, 15),
    (0x42ebfc, 15),
    (0x42f068, 15),
    (0x42f468, 15),
    (0x474a04, 15),
    (0x44d4a0, 13),
    (0x475940, 15),
    (0x4364b4, 15),
    (0x436aa0, 15),
    (0x436ffc, 15),
    (0x437704, 15),
    (0x434008, 15),
    (0x48d2fc, 16),
    (0x454df0, 14),
    (0x446c18, 14),
    (0x446f7c, 14),
    (0x4473e4, 14),
    (0x447674, 14),
    (0x447908, 14),
    (0x478604, 14),
    (0x4306e8, 14),
    (0x43b54c, 14),
    (0x42a51c, 14),
    (0x469fb8, 15),
    (0x46a268, 15),
    (0x46a640, 15),
    (0x46a8f8, 15),
    (0x46aa30, 15),
    (0x46ad14, 15),
    (0x4bbbcc, 15),
    (0x4bbf18, 15),
    (0x4bc030, 15),
    (0x4bc290, 15),
    (0x4bc3a8, 15),
    (0x4bc634, 15),
];
const LOOKUPS: [(u32, u32); 4] = [(0x490e48, 4), (0x490e80, 4), (0x4801ac, 4), (0x4801e4, 4)];
pub fn battle_scripts(originals: &Originals) -> Result<Files> {
    let exe = originals.exe()?;
    let size = (IMAGE.1 - IMAGE.0) as usize;
    let mut roots = serde_json::Map::new();
    let mut pending: Vec<(u32, Option<u32>)> = Vec::new();
    for actor in 1..40u32 {
        let root = exe.u32(exe.u32(0x4bdeb0 + 4 * actor)? + 8)?;
        roots.insert(actor.to_string(), root.into());
        pending.push((root, Some(actor)));
    }
    let phantom = decode_graph(exe, &[0x47eb58], &Options::default())?;
    let mut owned = BTreeSet::new();
    let mut walk = vec![0x47eb58u32];
    while let Some(at) = walk.pop() {
        if !owned.insert(at) {
            continue;
        }
        let node = phantom.nodes.get(&at).ok_or("Phantom scene command")?;
        if node.opcode() == 7 {
            walk.push(at + node.raw.len() as u32);
        } else {
            walk.extend(&node.successors);
        }
    }
    let mut scene_roots: BTreeSet<u32> = owned
        .iter()
        .filter(|a| phantom.nodes[a].opcode() == 7)
        .map(|a| phantom.nodes[a].pointer(4))
        .collect();
    scene_roots.insert(0x47eb58);
    let ending = decode_graph(exe, &[0x4b8b30], &Options::default())?;
    scene_roots.insert(0x4b8b30);
    scene_roots.extend(
        ending
            .nodes
            .values()
            .filter(|n| [7, 0x20].contains(&n.opcode()))
            .map(|n| n.pointer(4)),
    );
    pending.extend(scene_roots.iter().map(|&va| (va, None)));
    let read = |va: u32, n: usize| -> Result<Vec<u8>> { Ok(exe.read(va, n)?.to_vec()) };
    let ScriptWalk {
        keep,
        seen,
        census,
        tables,
    } = walk_scripts(&read, pending)?;
    let mut image = vec![0; size];
    let mut cursor = 0;
    while cursor < size {
        if !keep[cursor] {
            cursor += 1;
            continue;
        }
        let start = cursor;
        while cursor < size && keep[cursor] {
            cursor += 1;
        }
        image[start..cursor].copy_from_slice(exe.read(IMAGE.0 + start as u32, cursor - start)?);
    }
    let manifest = json!(
        { "schema" : 1, "source_sha256" : SOURCE, "base" : IMAGE.0, "end" : IMAGE.1,
        "bytes_kept" : keep.iter().filter(|&& k | k).count(), "commands" : seen.len(),
        "roots" : roots, "scene_roots" : scene_roots, "command_addresses" : seen,
        "state_tables" : tables.iter().map(| (k, v) | (format!("{k:#x}"), json!(v)))
        .collect::< serde_json::Map < _, _ >> (), "census" : census.iter().map(| (k, v) |
        (format!("{k:02x}"), json!(v))).collect::< serde_json::Map < _, _ >> (),
        "image_sha256" : crate ::sha(& image) }
    );
    let mut kept: Vec<[usize; 2]> = Vec::new();
    for (at, &k) in keep.iter().enumerate() {
        match kept.last_mut() {
            Some([_, end]) if k && *end == at => *end = at + 1,
            _ if k => kept.push([at, at + 1]),
            _ => {}
        }
    }
    Ok(vec![
        ("battle-scripts.bin".into(), image),
        ("battle-scripts.json".into(), json(manifest)?),
        (
            "battle-scripts-kept.json".into(),
            json(json!({ "schema"
            : 1, "base" : IMAGE.0, "kept" : kept }))?,
        ),
    ])
}
pub(crate) struct ScriptWalk {
    pub keep: Vec<bool>,
    pub seen: BTreeSet<u32>,
    pub census: BTreeMap<u8, u32>,
    pub tables: BTreeMap<u32, u32>,
}
pub(crate) fn walk_scripts(
    read: &dyn Fn(u32, usize) -> Result<Vec<u8>>,
    mut pending: Vec<(u32, Option<u32>)>,
) -> Result<ScriptWalk> {
    let word_at = |at: u32| -> Result<u32> {
        Ok(u32::from_le_bytes(
            read(at, 4)?.try_into().map_err(|_| "Script word")?,
        ))
    };
    let width_of = |va: u32| -> Result<u32> {
        let [op, mode] = <[u8; 2]>::try_from(read(va, 2)?.as_slice())?;
        Ok(match op {
            0x08 => {
                let mut n = 4;
                while read(va + n, 1)?[0] != 0xff {
                    n += if read(va + n, 1)?[0] == 3 { 8 } else { 4 };
                }
                n + 4
            }
            0x0a => 4 + 4 * u32::from(read(va + 2, 1)?[0]),
            0x11 | 0x12 => {
                if mode & 0x30 != 0 {
                    4
                } else {
                    8
                }
            }
            0x14 | 0x15 => {
                if mode & 0x30 != 0 {
                    8
                } else {
                    12
                }
            }
            _ => BODY_WIDTHS
                .iter()
                .find(|(o, _)| *o == op)
                .map(|(_, w)| *w)
                .ok_or_else(|| format!("Unknown game-VM opcode {op:02x} at {va:#x}"))?,
        })
    };
    let mut selectors: BTreeMap<u32, u32> = BTreeMap::new();
    for (table, count) in STATE_TABLES {
        if *selectors.entry(table).or_insert(count) != count {
            return Err(format!("State table {table:#x} lengths differ").into());
        }
    }
    let mut lookups: BTreeMap<u32, u32> = LOOKUPS.into();
    let early_records: Vec<Vec<u8>> = [0x490e48, 0x490e80]
        .iter()
        .map(|&t| Ok(read(t, 16)?.to_vec()))
        .collect::<Result<_>>()?;
    let size = (IMAGE.1 - IMAGE.0) as usize;
    let mut keep = vec![false; size];
    let mark = |va: u32, n: usize, keep: &mut Vec<bool>| -> Result<()> {
        if va < IMAGE.0 || va as usize + n > IMAGE.1 as usize {
            return Err(format!("Script data outside the image at {va:#x}").into());
        }
        let at = (va - IMAGE.0) as usize;
        keep[at..at + n].fill(true);
        Ok(())
    };
    let (mut seen, mut census, mut owner_tables, mut tables) = (
        BTreeSet::new(),
        BTreeMap::<u8, u32>::new(),
        BTreeMap::new(),
        BTreeMap::new(),
    );
    while let Some((start, actor)) = pending.pop() {
        let mut va = start;
        while seen.insert(va) {
            let n = width_of(va)?;
            mark(va, n as usize, &mut keep)?;
            let [op, mode] = <[u8; 2]>::try_from(read(va, 2)?.as_slice())?;
            *census.entry(op).or_default() += 1;
            let word = |at: u32| word_at(at);
            match op {
                0x00 | 0x05 => break,
                0x03 => {
                    va = word(va + 4)?;
                    continue;
                }
                _ => {}
            }
            if [0x06, 0x13, 0x32, 0xa2, 0x07, 0x20].contains(&op) {
                pending.push((word(va + 4)?, actor));
            }
            if op == 0x14 || op == 0x15 {
                pending.push((word(va + if mode & 0x30 != 0 { 4 } else { 8 })?, actor));
            }
            if op == 0x0a {
                for i in 0..u32::from(read(va + 2, 1)?[0]) {
                    pending.push((word(va + 4 + 4 * i)?, actor));
                }
                break;
            }
            if (op == 0x38 || op == 0x39) && mode == 0 {
                let mut at = word(va + 4)?;
                while read(at, 1)?[0] != 0xff {
                    mark(at, 4, &mut keep)?;
                    at += 4;
                }
                mark(at, 1, &mut keep)?;
            }
            if op == 0x88 {
                let descriptor = word(va + 4)?;
                mark(descriptor, 4, &mut keep)?;
                pending.push((word(descriptor)?, None));
            }
            if op == 0x2d {
                mark(SINE.0, SINE.1, &mut keep)?;
            }
            if op == 0x80 {
                mark(ATAN.0, ATAN.1, &mut keep)?;
            }
            if op == 0x18 {
                let (table, c) = (word(va + 4)?, read(va, 4)?);
                if c[1..4] == [0xb0, 0x64, 0x42] {
                    let count = selectors
                        .get(&table)
                        .copied()
                        .or_else(|| owner_tables.get(&actor).copied())
                        .ok_or_else(|| format!("State table {table:#x} without a length"))?;
                    if let Some(&known) = selectors.get(&table) {
                        owner_tables.insert(actor, known);
                    }
                    tables.insert(table, count);
                    mark(table, 4 * count as usize, &mut keep)?;
                    for i in 0..count {
                        pending.push((word(table + 4 * i)?, actor));
                    }
                } else if let Some(&count) = lookups.get(&table) {
                    mark(table, 4 * count as usize, &mut keep)?;
                } else if c[1..4] == [0xa0, 0x28, 0x58]
                    && early_records
                        .iter()
                        .any(|r| *r == read(table, 16).unwrap_or_default())
                {
                    lookups.insert(table, 4);
                    mark(table, 16, &mut keep)?;
                } else {
                    return Err(format!("Unbounded table {table:#x} at {va:#x}").into());
                }
            }
            va += n;
        }
    }
    Ok(ScriptWalk {
        keep,
        seen,
        census,
        tables,
    })
}
