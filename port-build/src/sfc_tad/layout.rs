use super::assets::RING_BYTES;
use crate::Result;

pub(super) fn stream_buffers(common: &[u8]) -> Result<[u16; 2]> {
    const HEADER: usize = 24;
    let word = |at: usize| -> Result<usize> {
        let bytes = common.get(at..at + 2).ok_or("Truncated audio directory")?;
        Ok(usize::from(u16::from_le_bytes(bytes.try_into()?)))
    };
    let instruments = word(0)?;
    let count = word(2)?
        .checked_sub(instruments)
        .ok_or("Invalid audio table")?;
    if !(2..=256).contains(&count)
        || word(4)? != instruments + count * 2
        || word(6)? != instruments + count * 3
        || common.len() < HEADER + count * 4
    {
        return Err("Invalid audio instrument tables".into());
    }
    let mut buffers = [0; 2];
    let mut base = None;
    for slot in 0..2 {
        let entry = HEADER + (count - 2 + slot) * 4;
        let address = word(entry)?;
        if word(entry + 2)? != address {
            return Err("Audio stream must loop to its start".into());
        }
        let mut template = vec![0; RING_BYTES];
        template[0] = ((slot + 1) << 4) as u8;
        template[RING_BYTES - 9] = 3;
        let mut matches = common
            .windows(RING_BYTES)
            .enumerate()
            .filter_map(|(i, data)| (data == template).then_some(i));
        let offset = matches.next().ok_or("Missing audio stream buffer")?;
        if matches.next().is_some() {
            return Err("Ambiguous audio stream buffer".into());
        }
        let actual_base = address
            .checked_sub(offset)
            .ok_or("Invalid audio stream address")?;
        if base.is_some_and(|b| b != actual_base)
            || (actual_base + HEADER) % 256 != 0
            || address + RING_BYTES > 65536
            || (slot == 1 && address != usize::from(buffers[0]) + RING_BYTES)
        {
            return Err("Audio stream directory and payload disagree".into());
        }
        base = Some(actual_base);
        buffers[slot] = u16::try_from(address)?;
    }
    let base = base.ok_or("Missing audio load address")?;
    let ring_start = usize::from(buffers[0]);
    if base + common.len() != usize::from(buffers[1]) + RING_BYTES {
        return Err("Audio streams must own the end of common data".into());
    }
    for i in 0..count - 2 {
        for at in [HEADER + i * 4, HEADER + i * 4 + 2] {
            let address = word(at)?;
            if address < instruments + 4 * count || address >= ring_start {
                return Err("Instrument overlaps audio stream buffers".into());
            }
        }
    }
    Ok(buffers)
}
