use crate::Result;
use serde::Deserialize;
use std::collections::BTreeSet;
#[derive(Deserialize)]
pub struct Assignment {
    offset: u8,
    width: u8,
    value: u32,
}
#[derive(Deserialize)]
#[cfg_attr(not(test), allow(dead_code))]
struct Observation {
    seed: u8,
    sha256: String,
}
#[derive(Deserialize)]
pub struct Frame {
    pub slot: u16,
    pub clip: u16,
    pub resource: String,
    pub rectangle: [u16; 4],
}
#[derive(Deserialize)]
pub struct Object {
    source_va: u32,
    source_bytes: u32,
    assignments: Vec<Assignment>,
    #[serde(default)]
    #[cfg_attr(not(test), allow(dead_code))]
    native_observations: Vec<Observation>,
    pub frame: Frame,
}
pub fn apply(state: &mut [u8; 256], assignments: &[Assignment]) -> Result<()> {
    for a in assignments {
        let start = usize::from(a.offset);
        let end = start + usize::from(a.width);
        if ![1, 2, 4].contains(&a.width)
            || end > state.len()
            || (start < 0x44 && end > 0x40)
            || (a.width < 4 && a.value >= (1u32 << (a.width * 8)))
        {
            return Err("Unsupported object assignment extent or value".into());
        }
        state[start..end].copy_from_slice(&a.value.to_le_bytes()[..usize::from(a.width)]);
    }
    Ok(())
}
pub fn compile(objects: &[Object], images: &[crate::images::ImageReport]) -> Result<Vec<u8>> {
    if objects.is_empty() || objects.len() > u16::MAX as usize {
        return Err("Missing or excessive object initializers".into());
    }
    let mut ids = BTreeSet::new();
    let mut output = b"HMO1".to_vec();
    output.extend((objects.len() as u16).to_le_bytes());
    for object in objects {
        if !ids.insert(object.source_va) || object.assignments.len() > 255 {
            return Err("Duplicate object ID or excessive assignments".into());
        }
        object
            .source_va
            .checked_add(object.source_bytes)
            .ok_or("Object extent overflow")?;
        for seed in [0, 0xa5] {
            let mut state = [seed; 256];
            apply(&mut state, &object.assignments)?;
            if u32::from_le_bytes(state[0x28..0x2c].try_into()?)
                != (u32::from(object.frame.slot) << 16 | u32::from(object.frame.clip))
            {
                return Err("Object frame does not match assignments".into());
            }
        }
        let image_id = images
            .iter()
            .position(|i| i.resource == object.frame.resource)
            .ok_or("Missing object source image")?;
        let [left, top, right, bottom] = object.frame.rectangle;
        if left >= right
            || top >= bottom
            || usize::from(right) > images[image_id].width
            || usize::from(bottom) > images[image_id].height
            || image_id > u16::MAX as usize
        {
            return Err("Invalid source frame rectangle".into());
        }
        output.extend(object.source_va.to_le_bytes());
        output.push(object.assignments.len() as u8);
        for a in &object.assignments {
            output.extend([a.offset, a.width]);
            output.extend(a.value.to_le_bytes());
        }
        output.extend((image_id as u16).to_le_bytes());
        output.extend(object.frame.slot.to_le_bytes());
        output.extend(object.frame.clip.to_le_bytes());
        for coordinate in object.frame.rectangle {
            output.extend(coordinate.to_le_bytes());
        }
    }
    Ok(output)
}
