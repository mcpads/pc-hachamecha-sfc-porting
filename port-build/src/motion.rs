use crate::{Result, sha, vm::Command};
use serde::Deserialize;
#[derive(Deserialize)]
pub struct Reference {
    table_source_va: u32,
    pub table: Vec<u16>,
    table_sha256: String,
    #[serde(default)]
    #[cfg_attr(not(test), allow(dead_code))]
    native_trig_sha256: String,
    #[serde(default)]
    #[cfg_attr(not(test), allow(dead_code))]
    angle_cases: usize,
    #[serde(default)]
    #[cfg_attr(not(test), allow(dead_code))]
    cases: Vec<Case>,
}
#[derive(Deserialize)]
#[cfg_attr(not(test), allow(dead_code))]
struct Case {
    command: Command,
    synthetic: bool,
    before: Vec<u8>,
    after_sha256: String,
}
pub(crate) fn sine(table: &[u16], index: u16) -> u32 {
    let phase = index & 1023;
    let magnitude = match index >> 10 {
        0 | 2 => u32::from(table[usize::from(phase)]),
        _ if phase == 0 => 65536,
        _ => u32::from(table[usize::from(1024 - phase)]),
    };
    if index & 2048 != 0 {
        magnitude.wrapping_neg()
    } else {
        magnitude
    }
}
pub fn apply(object: &mut [u8; 256], command: &Command, table: &[u16]) -> Result<()> {
    if table.len() != 1024 || command.bytes.len() != 4 || command.bytes[0] != 0x2d {
        return Err("Invalid motion table or command".into());
    }
    let read16 = |at| u16::from_le_bytes(object[at..at + 2].try_into().unwrap());
    let read32 = |at| u32::from_le_bytes(object[at..at + 4].try_into().unwrap());
    if read32(0x40) != command.va {
        return Err("Invalid motion command boundary".into());
    }
    let mode = command.bytes[1];
    if mode & 0xf8 > 8 {
        return Err("Unsupported motion mode".into());
    }
    let mut positions = [read32(0x1c), read32(0x20), read32(0x24)];
    for (axis, position) in positions.iter_mut().enumerate() {
        if mode & (1 << axis) == 0 {
            continue;
        }
        let angle = read16(0x8c + axis * 2).wrapping_add(8) >> 4;
        let value = sine(
            table,
            if axis == 0 {
                angle
            } else {
                (angle + 1024) & 4095
            },
        );
        let amplitude = read16(0x92 + axis * 2) as i16 as i32 as u32;
        let delta = value.wrapping_mul(amplitude);
        let origin = if mode & 8 == 0 {
            *position
        } else {
            read32(0x80 + axis * 4)
        };
        *position = if axis == 1 {
            origin.wrapping_sub(delta)
        } else {
            origin.wrapping_add(delta)
        };
    }
    for (axis, position) in positions.into_iter().enumerate() {
        object[0x1c + axis * 4..0x20 + axis * 4].copy_from_slice(&position.to_le_bytes());
    }
    object[0x40..0x44].copy_from_slice(&command.va.wrapping_add(4).to_le_bytes());
    Ok(())
}
pub fn compile(reference: Option<&Reference>) -> Result<Vec<u8>> {
    let Some(r) = reference else {
        return Ok(Vec::new());
    };
    if r.table.len() != 1024 || r.table_source_va != 0x45b3c8 {
        return Err("Incomplete motion table".into());
    }
    let table: Vec<u8> = r.table.iter().flat_map(|n| n.to_le_bytes()).collect();
    if sha(&table) != r.table_sha256 {
        return Err("Motion table hash mismatch".into());
    }
    let mut bytes = b"HMT1".to_vec();
    bytes.extend(1024u16.to_le_bytes());
    bytes.extend(table);
    Ok(bytes)
}
