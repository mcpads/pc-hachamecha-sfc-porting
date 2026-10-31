use super::*;
pub fn thin(image: &mut [u8], commands: &mut Vec<u32>) -> Result<Value> {
    let base = crate::sfc_battle_objects::SOURCE_BASE;
    let at = |va: u32| (va - base) as usize;
    let u32le = |v: u32| v.to_le_bytes();
    let mut report = Vec::new();
    let mut place = THIN_SPACE;
    if image[at(THIN_SPACE)..at(THIN_END)].iter().any(|&b| b != 0) {
        return Err("Ending thinning space is in use".into());
    }
    let mut patch = |image: &mut [u8], va: u32, from: &[u8], to: &[u8]| -> Result<()> {
        if &image[at(va)..at(va) + from.len()] != from {
            return Err(format!("Ending thinning: unexpected bytes at {va:#x}").into());
        }
        image[at(va)..at(va) + to.len()].copy_from_slice(to);
        report.push(serde_json::json!({ "va" : va, "from" : hex(from), "to" : hex(to) }));
        Ok(())
    };
    fn hex(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }
    let tick = |out: &mut Vec<u8>, cmds: &mut Vec<u32>, va: u32| {
        cmds.extend([va, va + 8, va + 12]);
        out.extend([0x11, 0x81, 0x8e, 0x00, 0x00, 0x00, 0x02, 0x00]);
        out.extend([0x2d, 0x0a, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00]);
    };
    let mut added = Vec::new();
    let loops: [(u32, u32, u32, Option<u32>, usize); 10] = [
        (0x4b98e0, 0x4b98ec, 0x4b9904, None, ORB_EVERY),
        (0x4b994c, 0x4b9958, 0x4b997c, None, ORB_EVERY),
        (0x4b953c, 0x4b9548, 0x4b9560, Some(0x4b9568), GEM_EVERY),
        (0x4b95f4, 0x4b9600, 0x4b9618, Some(0x4b9620), GEM_EVERY),
        (0x4b96ac, 0x4b96b8, 0x4b96d0, Some(0x4b96d8), GEM_EVERY),
        (0x4b9764, 0x4b9770, 0x4b9788, Some(0x4b9790), GEM_EVERY),
        (0x4b9574, 0x4b9580, 0x4b9598, None, GEM_EVERY),
        (0x4b962c, 0x4b9638, 0x4b9650, None, GEM_EVERY),
        (0x4b96e4, 0x4b96f0, 0x4b9708, None, GEM_EVERY),
        (0x4b979c, 0x4b97a8, 0x4b97c0, None, GEM_EVERY),
    ];
    for (start, count, end, exit, every) in loops {
        let mut from = vec![0x06, 0x00, 0x02, 0x00];
        from.extend(u32le(start));
        let mut to = vec![0x06, 0x00, 0x01, 0x00];
        to.extend(u32le(start));
        patch(image, count, &from, &to)?;
        let mut block = Vec::new();
        let tick_at = place;
        tick(&mut block, commands, tick_at);
        if let Some(exit) = exit {
            let va = place + block.len() as u32;
            commands.extend([va, va + 8]);
            block.extend([0x13, 0xc1, 0x19, 0x00]);
            block.extend(u32le(va + 16));
            block.extend([0x03, 0x00, 0x00, 0x00]);
            block.extend(u32le(exit));
        }
        commands.extend([place + block.len() as u32, place + block.len() as u32 + 8]);
        block.extend([0x06, 0x00, u8::try_from(every - 1)?, 0x00]);
        block.extend(u32le(tick_at));
        block.extend([0x03, 0x00, 0x00, 0x00]);
        block.extend(u32le(start));
        let (op, mut from) = if exit.is_some() {
            (0x13u8, vec![0x13, 0xc1, 0x19, 0x00])
        } else {
            (0x03, vec![0x03, 0x00, 0x00, 0x00])
        };
        from.extend(u32le(start));
        let mut to = from[..4].to_vec();
        to.extend(u32le(place));
        let _ = op;
        patch(image, end, &from, &to)?;
        if place + block.len() as u32 > THIN_END {
            return Err("Ending thinning space runs out".into());
        }
        image[at(place)..at(place) + block.len()].copy_from_slice(&block);
        added.push(serde_json::json!(
            { "loop" : start, "every" : every, "at" : place, "bytes" : block
            .len() }
        ));
        place += block.len() as u32;
    }
    commands.sort_unstable();
    commands.dedup();
    Ok(serde_json::json!(
        { "orb_every" : ORB_EVERY, "gem_every" : GEM_EVERY, "patches" : report,
        "added" : added }
    ))
}
