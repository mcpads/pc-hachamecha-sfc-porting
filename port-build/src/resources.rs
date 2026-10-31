use crate::{Result, palette::Palette};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Clone, Deserialize)]
pub struct Reference {
    commands: Vec<Command>,
    assets: Vec<Asset>,
    #[serde(default)]
    #[cfg_attr(not(test), allow(dead_code))]
    pub surface_cases: Vec<crate::scene::Case>,
}
#[derive(Clone, Deserialize)]
struct Asset {
    file: String,
    colors: Vec<u8>,
}
#[derive(Clone, Deserialize)]
struct Command {
    va: u32,
    opcode: u8,
    bytes: u32,
    #[serde(default)]
    flags: u8,
    #[serde(default)]
    slot: i16,
    file: Option<String>,
    #[serde(default)]
    arguments: Vec<u32>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Event {
    pub opcode: u8,
    pub slot: u16,
    pub flags: u8,
    pub file: Option<String>,
    pub argument: u32,
}
impl Reference {
    pub fn color_count(&self, file: &str) -> Result<usize> {
        Ok(self
            .assets
            .iter()
            .find(|a| a.file == file)
            .ok_or("Unknown resource colors")?
            .colors
            .len()
            / 3)
    }
    pub fn run(&self, start: u32, palette: &mut Palette) -> Result<(u32, Vec<Event>)> {
        let commands: BTreeMap<_, _> = self.commands.iter().map(|c| (c.va, c)).collect();
        let mut pc = start;
        let mut events = Vec::new();
        for _ in 0..256 {
            let c = commands
                .get(&pc)
                .ok_or("Unknown resource command boundary")?;
            match c.opcode {
                0 => return Ok((pc, events)),
                1 | 2 | 6 => {
                    let asset = self
                        .assets
                        .iter()
                        .find(|a| Some(&a.file) == c.file.as_ref())
                        .ok_or("Missing resource palette")?;
                    palette.load(c.flags, &asset.colors)?;
                    events.push(Event {
                        opcode: c.opcode,
                        slot: c.slot as u16,
                        flags: c.flags,
                        file: c.file.clone(),
                        argument: *c.arguments.first().ok_or("Missing resource argument")?,
                    });
                }
                11 | 12 => events.push(Event {
                    opcode: c.opcode,
                    slot: c.slot as u16,
                    flags: if c.opcode == 11 { 0 } else { c.flags },
                    file: None,
                    argument: 0,
                }),
                13 => palette.mark_dirty(),
                _ => return Err("Unsupported resource runtime opcode".into()),
            }
            pc = pc.checked_add(c.bytes).ok_or("Resource PC overflow")?;
        }
        Err("Resource stream command budget exhausted".into())
    }
}
pub fn compile(
    reference: Option<&Reference>,
    image_files: &[(String, Vec<u8>)],
) -> Result<Vec<u8>> {
    let Some(r) = reference else {
        return Ok(Vec::new());
    };
    if r.assets.is_empty()
        || r.assets.len() > 65535
        || r.commands.is_empty()
        || r.commands.len() > 65535
    {
        return Err("Invalid resource program size".into());
    }
    let mut bytes = b"HMR1".to_vec();
    bytes.extend((r.assets.len() as u16).to_le_bytes());
    let mut ids = BTreeMap::new();
    for (i, a) in r.assets.iter().enumerate() {
        if a.file.is_empty()
            || a.file.len() > 255
            || ids.insert(&a.file, i as u16).is_some()
            || a.colors.is_empty()
            || a.colors.len() > 768
            || !a.colors.len().is_multiple_of(3)
        {
            return Err("Invalid resource asset".into());
        }
        let palette = image_files
            .iter()
            .find(|(n, _)| *n == format!("{}.rgb", a.file))
            .ok_or("Unbound resource asset")?;
        if !palette.1.starts_with(&a.colors) {
            return Err("Resource palette differs from verified image".into());
        }
        bytes.push(a.file.len() as u8);
        bytes.extend(a.file.as_bytes());
        bytes.extend(((a.colors.len() / 3) as u16).to_le_bytes());
        bytes.extend(&a.colors);
    }
    bytes.extend((r.commands.len() as u16).to_le_bytes());
    let mut seen = BTreeMap::new();
    let mut frame_tables = BTreeMap::new();
    for c in &r.commands {
        let load = matches!(c.opcode, 1 | 2 | 6);
        if seen.insert(c.va, ()).is_some()
            || c.slot < 0
            || c.slot > 255
            || c.bytes != if load { 12 } else { 4 }
            || !matches!(c.opcode, 0 | 1 | 2 | 6 | 11 | 12 | 13)
            || c.arguments.len() != usize::from(load)
            || c.file.is_some() != load
        {
            return Err("Invalid resource command".into());
        }
        let id = match &c.file {
            Some(name) => *ids.get(name).ok_or("Unknown resource image")?,
            None => u16::MAX,
        };
        if c.opcode == 1
            && let Some(previous) = frame_tables.insert(id, c.arguments[0])
            && previous != c.arguments[0]
        {
            return Err("Image has multiple unresolved frame tables".into());
        }
        bytes.extend(c.va.to_le_bytes());
        bytes.extend([c.opcode, c.flags]);
        bytes.extend(c.slot.to_le_bytes());
        bytes.extend(id.to_le_bytes());
        bytes.extend(c.arguments.first().copied().unwrap_or(0).to_le_bytes());
    }
    Ok(bytes)
}
