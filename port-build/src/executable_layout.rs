use crate::{Result, sha};
use serde::Deserialize;
use std::collections::BTreeMap;

pub const ORIGINAL_SHA256: &str =
    "f5985723f2912287d05af933ef5d7b96ea158b4a87fc5d722713f9eb98f7121a";

#[derive(Deserialize)]
struct Layout {
    copies: Vec<[usize; 3]>,
    pointers: Vec<[u32; 2]>,
}

pub struct MappedData {
    pub image: Vec<u8>,
    pub ranges: Vec<[usize; 2]>,
}

pub fn normalize(image: &[u8]) -> Result<MappedData> {
    let layout: Layout = serde_json::from_slice(include_bytes!("../inputs/mado-layout.json"))?;
    let word = |at: usize| -> Result<u32> {
        Ok(u32::from_le_bytes(
            image
                .get(at..at + 4)
                .ok_or("Truncated executable layout")?
                .try_into()?,
        ))
    };
    let pe = word(0x3c)? as usize;
    let directory = pe + 24 + 96 + 5 * 8;
    let mut cursor = word(directory)? as usize;
    let end = cursor
        .checked_add(word(directory + 4)? as usize)
        .ok_or("Relocation size overflow")?;
    let mut relocations = Vec::new();
    while cursor < end {
        let page = word(cursor)? as usize;
        let size = word(cursor + 4)? as usize;
        if size < 8 || size % 2 != 0 || cursor + size > end {
            return Err("Invalid relocation block".into());
        }
        for at in (cursor + 8..cursor + size).step_by(2) {
            let entry = u16::from_le_bytes(
                image
                    .get(at..at + 2)
                    .ok_or("Truncated relocation")?
                    .try_into()?,
            );
            match entry >> 12 {
                0 => (),
                3 => relocations.push(page + usize::from(entry & 0xfff)),
                _ => return Err("Unsupported relocation type".into()),
            }
        }
        cursor += size;
    }
    let pointers: BTreeMap<_, _> = layout
        .pointers
        .into_iter()
        .map(|[source, target]| (source, target))
        .collect();
    let translate = |value: u32| -> Result<u32> {
        if let Some(&target) = pointers.get(&value) {
            return Ok(target);
        }
        let rva = value
            .checked_sub(0x400000)
            .ok_or("Pointer below executable image")? as usize;
        let mut target = None;
        for &[lo, hi, source] in &layout.copies {
            if source <= rva && rva < source + hi - lo {
                let candidate = (lo + rva - source) as u32 + 0x400000;
                if target.is_some_and(|previous| previous != candidate) {
                    return Err("Ambiguous executable pointer".into());
                }
                target = Some(candidate);
            }
        }
        target.ok_or_else(|| format!("Unmapped executable pointer {value:#x}").into())
    };
    let mut output = vec![0; image.len()];
    let mut previous_end = 0;
    for &[lo, hi, source] in &layout.copies {
        if lo < previous_end || hi <= lo {
            return Err("Overlapping executable copy ranges".into());
        }
        previous_end = hi;
        let data = image
            .get(source..source + hi - lo)
            .ok_or("Executable copy outside source")?;
        let destination = output
            .get_mut(lo..hi)
            .ok_or("Executable copy outside destination")?;
        destination.copy_from_slice(data);
        for &at in &relocations {
            if source <= at && at + 4 <= source + hi - lo {
                let target = translate(word(at)?)?;
                destination[at - source..at - source + 4].copy_from_slice(&target.to_le_bytes());
            }
        }
    }
    if sha(&output) != "1ff496b361d8e030c9d609106b0c25041662778f99be0cff9e64dcf5c690f71d" {
        return Err("Executable data layout verification failed".into());
    }
    Ok(MappedData {
        image: output,
        ranges: layout.copies.iter().map(|&[lo, hi, _]| [lo, hi]).collect(),
    })
}
