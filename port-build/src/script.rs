#![allow(dead_code)]
use crate::{Result, original::Exe};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
#[rustfmt::skip]
const FIXED: [(u8, usize); 117] = [
    (0x00, 4),
    (0x01, 4),
    (0x02, 4),
    (0x03, 8),
    (0x04, 8),
    (0x05, 4),
    (0x06, 8),
    (0x07, 8),
    (0x10, 4),
    (0x13, 8),
    (0x25, 4),
    (0x1f, 4),
    (0x29, 4),
    (0x2d, 4),
    (0x28, 4),
    (0x2f, 8),
    (0x30, 8),
    (0x31, 4),
    (0x32, 8),
    (0x34, 4),
    (0x35, 4),
    (0x38, 8),
    (0x39, 8),
    (0x3b, 4),
    (0x3f, 8),
    (0x4b, 4),
    (0x6f, 4),
    (0x77, 8),
    (0x7b, 4),
    (0x88, 8),
    (0x8a, 4),
    (0x8d, 12),
    (0x90, 4),
    (0x91, 4),
    (0x9d, 8),
    (0x24, 4),
    (0x74, 8),
    (0x75, 16),
    (0x78, 4),
    (0x79, 4),
    (0x7e, 4),
    (0x81, 4),
    (0x41, 4),
    (0x45, 4),
    (0xa9, 8),
    (0x9b, 4),
    (0x40, 4),
    (0x44, 4),
    (0x7f, 4),
    (0x20, 8),
    (0x2b, 4),
    (0x3a, 4),
    (0x3c, 4),
    (0x3d, 4),
    (0x4c, 4),
    (0x5a, 8),
    (0x5c, 8),
    (0x69, 8),
    (0x83, 4),
    (0x84, 4),
    (0x85, 4),
    (0x86, 4),
    (0x8b, 4),
    (0x8c, 4),
    (0x8f, 4),
    (0x93, 4),
    (0x94, 12),
    (0x95, 4),
    (0x97, 4),
    (0x18, 8),
    (0x21, 8),
    (0x4d, 4),
    (0x5b, 4),
    (0x67, 4),
    (0x1e, 4),
    (0x68, 4),
    (0x5d, 4),
    (0x80, 12),
    (0x3e, 4),
    (0x43, 4),
    (0x9e, 8),
    (0x49, 4),
    (0x4e, 4),
    (0x4f, 4),
    (0x50, 8),
    (0x51, 4),
    (0x52, 4),
    (0x53, 4),
    (0x54, 4),
    (0x55, 4),
    (0x56, 4),
    (0x57, 4),
    (0x58, 4),
    (0x59, 8),
    (0x5e, 4),
    (0x5f, 4),
    (0x60, 8),
    (0x61, 8),
    (0x62, 8),
    (0x63, 4),
    (0x64, 4),
    (0x65, 8),
    (0x72, 4),
    (0x66, 8),
    (0x71, 4),
    (0x73, 4),
    (0x7c, 4),
    (0x89, 8),
    (0xa0, 8),
    (0xa1, 4),
    (0x92, 4),
    (0xa6, 4),
    (0x82, 8),
    (0xa2, 8),
    (0xa3, 4),
    (0xa5, 8),
    (0xaa, 4),
];
const AUDIO_FOUR: [u8; 17] = [1, 2, 3, 4, 5, 6, 7, 8, 12, 13, 14, 15, 16, 21, 32, 64, 65];
const AUDIO_EIGHT: [u8; 3] = [20, 22, 48];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Link {
    Message(u32),
    Resource(u32),
    Descriptor(u32),
}
#[derive(Clone, Debug)]
pub struct Node {
    pub va: u32,
    pub raw: Vec<u8>,
    pub successors: Vec<u32>,
    pub links: Vec<Link>,
}
impl Node {
    pub fn opcode(&self) -> u8 {
        self.raw[0]
    }
    pub fn raw_hex(&self) -> String {
        self.raw.iter().map(|b| format!("{b:02x}")).collect()
    }
    pub fn pointer(&self, offset: usize) -> u32 {
        u32::from_le_bytes(self.raw[offset..offset + 4].try_into().expect("in command"))
    }
}
#[derive(Default)]
pub struct Options {
    pub stop_at_yield: bool,
    pub foreign: BTreeSet<u32>,
    pub external: BTreeSet<u32>,
}
#[derive(Default)]
pub struct Graph {
    pub nodes: BTreeMap<u32, Node>,
    pub unknowns: BTreeMap<u32, [u8; 4]>,
    pub external: BTreeSet<u32>,
}
fn width(exe: &Exe, va: u32, header: [u8; 4]) -> Result<Option<usize>> {
    let [op, sub, count, _] = header;
    let mode = sub & 127;
    Ok(match op {
        0x08 | 0x0c => {
            let mut width = 4;
            for _ in 0..256 {
                let kind = exe.read(va + width as u32, 1)?[0];
                width += if kind == 3 { 8 } else { 4 };
                if kind == 255 {
                    return Ok(Some(width));
                }
            }
            return Err("Embedded assignment list lacks terminator".into());
        }
        0x11 | 0x12 => Some(if sub & 0x30 == 0 { 8 } else { 4 }),
        0x14 | 0x15 => Some(if sub & 0x30 == 0 { 12 } else { 8 }),
        0x26 if AUDIO_FOUR.contains(&mode) => Some(4),
        0x26 if AUDIO_EIGHT.contains(&mode) => Some(8),
        0x26 => None,
        0x27 => match mode {
            0x21 => Some(16),
            2 | 4 | 6 | 8 | 10 | 0x30 | 0x31 => Some(8),
            0 | 1 | 3 | 5 | 7 | 9 | 11..=16 | 0x20 | 0x22 | 0x40 | 0x41 => Some(4),
            _ => None,
        },
        0x96 | 0x7a => match sub {
            0 => Some(4),
            1 => Some(8),
            _ => None,
        },
        0x70 => match sub {
            0 => Some(8),
            1 => Some(4),
            _ => None,
        },
        0x0a | 0x0b => (count != 0).then(|| 4 + usize::from(count) * 4),
        _ => FIXED.iter().find(|(o, _)| *o == op).map(|(_, w)| *w),
    })
}
pub fn decode_graph(exe: &Exe, roots: &[u32], options: &Options) -> Result<Graph> {
    const BUDGET: usize = 8192;
    let mut graph = Graph::default();
    let mut pending: VecDeque<u32> = roots.iter().copied().collect();
    while let Some(va) = pending.pop_front() {
        if graph.nodes.contains_key(&va) || graph.unknowns.contains_key(&va) {
            continue;
        }
        if options.external.contains(&va) {
            graph.external.insert(va);
            continue;
        }
        if graph.nodes.len() + graph.unknowns.len() >= BUDGET {
            return Err("Game script graph budget exhausted".into());
        }
        let header: [u8; 4] = exe.read(va, 4)?.try_into()?;
        if options.foreign.contains(&va) {
            graph.unknowns.insert(va, header);
            continue;
        }
        let Some(node) = decode_node(exe, va, options.stop_at_yield)? else {
            graph.unknowns.insert(va, header);
            continue;
        };
        pending.extend(&node.successors);
        graph.nodes.insert(va, node);
    }
    let mut end = 0;
    for (&va, width) in graph
        .nodes
        .iter()
        .map(|(va, n)| (va, n.raw.len()))
        .chain(graph.unknowns.keys().map(|va| (va, 0)))
        .collect::<BTreeMap<_, _>>()
    {
        if va < end {
            return Err("Overlapping decoded instructions".into());
        }
        end = va + width as u32;
    }
    Ok(graph)
}
pub fn decode_node(exe: &Exe, va: u32, stop_at_yield: bool) -> Result<Option<Node>> {
    let header: [u8; 4] = exe.read(va, 4)?.try_into()?;
    let Some(width) = width(exe, va, header)? else {
        return Ok(None);
    };
    let [op, sub, count, _] = header;
    let raw = exe.read(va, width)?.to_vec();
    let pointer = |offset: usize| u32::from_le_bytes(raw[offset..offset + 4].try_into().unwrap());
    let next = va + width as u32;
    let mut successors = Vec::new();
    let mut links = Vec::new();
    match op {
        0x03 => successors.push(pointer(4)),
        0x00 | 0x05 => {}
        0x01 if stop_at_yield => {}
        0x0a | 0x0b => {
            successors.extend((0..usize::from(count)).map(|i| pointer(4 + i * 4)));
            if op == 0x0b {
                successors.push(next);
            }
        }
        _ => {
            successors.push(next);
            if [
                0x04, 0x06, 0x07, 0x77, 0x32, 0x13, 0x74, 0x5a, 0x5c, 0x9e, 0xa9, 0x50, 0x59, 0x60,
                0x61, 0x62, 0x65, 0x66, 0x82, 0x89, 0xa0, 0xa2, 0xa5,
            ]
            .contains(&op)
            {
                successors.push(pointer(4));
            }
        }
    }
    if [0x14, 0x15, 0x94].contains(&op) {
        successors.push(pointer(width - 4));
    }
    if op == 0x96 && sub == 1 {
        successors.push(pointer(4));
    }
    if op == 0x75 {
        match sub {
            0 | 2 => successors.push(pointer(12)),
            1 => links.push(Link::Message(pointer(12))),
            _ => {
                return Err(format!("Unsupported B1 click mode {sub:#x} at {va:#x}").into());
            }
        }
    }
    match op {
        0x3f => links.push(Link::Resource(pointer(4))),
        0x2f => links.push(Link::Message(pointer(4))),
        0x88 => links.push(Link::Descriptor(pointer(4))),
        _ => {}
    }
    Ok(Some(Node {
        va,
        raw,
        successors,
        links,
    }))
}
#[derive(Clone, Debug)]
pub struct ResourceCommand {
    pub va: u32,
    pub opcode: u8,
    pub flags: u8,
    pub slot: i16,
    pub raw: Vec<u8>,
    pub file: Option<String>,
    pub arguments: Vec<u32>,
}
pub fn decode_resource_script(exe: &Exe, start: u32) -> Result<Vec<ResourceCommand>> {
    let mut commands = Vec::new();
    let mut cursor = start;
    for _ in 0..256 {
        let header = exe.read(cursor, 4)?;
        let (opcode, flags) = (header[0], header[1]);
        let slot = i16::from_le_bytes([header[2], header[3]]);
        let size = match opcode {
            0 => 4,
            1 | 2 | 4 | 5 | 6 | 8 | 9 => 12,
            10 if [0, 1, 3, 4, 5, 7].contains(&flags) => 12,
            10 if [2, 6].contains(&flags) => 16,
            3 | 7 => 16,
            11 | 12 | 13 | 16 => 4,
            _ => {
                return Err(
                    format!("Unsupported resource opcode {opcode:#x} at {cursor:#x}").into(),
                );
            }
        };
        let raw = exe.read(cursor, size)?.to_vec();
        let (file, arguments) = if (1..=9).contains(&opcode) {
            let name_va = u32::from_le_bytes(raw[4..8].try_into()?);
            let name = exe.cstring(name_va, 128)?;
            if name.is_empty()
                || !name
                    .iter()
                    .all(|c| c.is_ascii_alphanumeric() || *c == b'_' || *c == b'.')
                || name.windows(2).any(|w| w == b"..")
            {
                return Err("Invalid resource filename".into());
            }
            let arguments = raw[8..]
                .chunks(4)
                .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
                .collect();
            (
                Some(String::from_utf8(name.to_ascii_uppercase())?),
                arguments,
            )
        } else {
            (None, Vec::new())
        };
        commands.push(ResourceCommand {
            va: cursor,
            opcode,
            flags,
            slot,
            raw,
            file,
            arguments,
        });
        if opcode == 0 {
            return Ok(commands);
        }
        cursor += size as u32;
    }
    Err("Resource command limit reached without terminator".into())
}
