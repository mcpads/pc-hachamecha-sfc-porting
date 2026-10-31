use super::*;
fn guarded_spawns(image: &[u8], commands: &[u32]) -> Vec<(u32, u16)> {
    let at = |va: u32| (va - SOURCE_BASE) as usize;
    let word = |va: u32| u32::from_le_bytes(image[at(va)..at(va) + 4].try_into().unwrap());
    let mut found = Vec::new();
    for &va in commands {
        let c = &image[at(va)..at(va) + 8];
        if c[0] != 0x07 || c[2] != 0 || c[3] != 0 || image[at(va + 8)] == 0x1f {
            continue;
        }
        let mut pc = word(va + 4);
        if !(SOURCE_BASE..SOURCE_END).contains(&pc) {
            continue;
        }
        let mut bound = false;
        let mask = loop {
            let (op, mode) = (image[at(pc)], image[at(pc) + 1]);
            match op {
                0x08 => {
                    let mut n = 4;
                    while image[at(pc) + n] != 0xff {
                        n += if image[at(pc) + n] == 3 { 8 } else { 4 };
                    }
                    pc += n as u32 + 4;
                }
                0x20 => pc += 8,
                0x10 if mode == 0x80 => pc += 4,
                0x11 | 0x12 if mode == 0x80 || mode == 0x81 => pc += 8,
                0x4d if image[at(pc) + 1..at(pc) + 4] == [0, 0, 0] => {
                    bound = true;
                    pc += 4;
                }
                0x13 if bound && image[at(pc)..at(pc) + 4] == [0x13, 0xc0, 0x61, 0] => {
                    let dead = word(pc + 4);
                    let test = &image[at(pc + 8)..at(pc + 20)];
                    let after = &image[at(pc + 20)..at(pc + 28)];
                    if image[at(dead)] == 0
                        && test[..4] == [0x14, 0xc6, 0x2e, 0]
                        && test[6..8] == [0, 0]
                        && after[..4] == [3, 0, 0, 0]
                        && word(pc + 24) == dead
                    {
                        break Some(u16::from_le_bytes([test[4], test[5]]));
                    }
                    break None;
                }
                _ => break None,
            }
        };
        if let Some(mask) = mask {
            found.push((va, mask));
        }
    }
    found
}
pub fn thinning_report() -> serde_json::Value {
    THINNING
        .iter()
        .chain(&SCENE_THINNING)
        .chain(&LATE_THINNING)
        .chain(&FINAL_BURST_THINNING)
        .chain(&INWARD_BURST_THINNING)
        .chain(&vertical_motion::DENSITY)
        .map(|(va, from, to)| serde_json::json!({ "va" : va, "from" : from, "to" : to }))
        .collect()
}
pub fn thinned(image: &[u8]) -> Vec<u8> {
    assert_eq!(
        crate::sha(&image[(0x4bd4c8 - SOURCE_BASE) as usize..(0x4bd7d8 - SOURCE_BASE) as usize]),
        "0ad033223f81da522964c2d40513ae273ccf3f761321863198b5374cc9580150",
        "Final-boss burst parent or leaf changed"
    );
    vertical_motion::check_source(image).expect("Vertical particle density source boundary");
    for (start, end, expected) in [
        (
            0x46bb44,
            0x46bb90,
            "a3f3e9fcb08529f26ad7355d96a69eeff1def850885b42d4082a0fb2b6e495d3",
        ),
        (
            0x46be24,
            0x46bee0,
            "f8c4db551c4823ec1b7bf375b1b2a52bcf20bdddf70231c8322a455a0e0eb8c6",
        ),
    ] {
        assert_eq!(
            crate::sha(&image[(start - SOURCE_BASE) as usize..(end - SOURCE_BASE) as usize]),
            expected,
            "Inward burst emitter or leaf changed"
        );
    }
    let mut out = image.to_vec();
    for (va, from, to) in THINNING
        .into_iter()
        .chain(SCENE_THINNING)
        .chain(LATE_THINNING)
        .chain(FINAL_BURST_THINNING)
        .chain(INWARD_BURST_THINNING)
        .chain(vertical_motion::DENSITY)
    {
        let at = (va - SOURCE_BASE) as usize;
        assert_eq!(image[at..at + 4], from, "thinned command at {va:#x}");
        out[at..at + 4].copy_from_slice(&to);
    }
    radial_velocity::patch(&mut out, image, false)
        .expect("Constant-velocity burst source boundary");
    linked_velocity::patch(&mut out, image, false).expect("Linked velocity source boundary");
    jugem_density::check_source(image).expect("Jugem density source boundary");
    lightning_density::patch(&mut out, image).expect("Lightning density source boundary");
    out
}
pub fn reached(thinned: &[u8], manifest: &serde_json::Value) -> Result<Vec<bool>> {
    let read = |va: u32, n: usize| -> Result<Vec<u8>> {
        let at = va
            .checked_sub(SOURCE_BASE)
            .map(|a| a as usize)
            .filter(|a| a + n <= thinned.len())
            .ok_or_else(|| format!("Script read outside the image at {va:#x}"))?;
        Ok(thinned[at..at + n].to_vec())
    };
    let mut roots = Vec::new();
    for actor in 1..40u32 {
        let root = manifest["roots"][actor.to_string()]
            .as_u64()
            .ok_or("Battle body root")?;
        roots.push((root as u32, Some(actor)));
    }
    for scene in manifest["scene_roots"].as_array().ok_or("Scene roots")? {
        roots.push((scene.as_u64().ok_or("Scene root")? as u32, None));
    }
    let walk = crate::import_battle::walk_scripts(&read, roots)?;
    if walk.keep.len() != thinned.len() {
        return Err("Script walk covers another image".into());
    }
    Ok(walk.keep)
}
pub fn specialized(image: &[u8], commands: &[u32]) -> Result<Vec<u8>> {
    if commands
        .iter()
        .any(|&va| image[(va - SOURCE_BASE) as usize] == 0xaf)
    {
        return Err("Own-immediate multiply opcode already used by a source command".into());
    }
    let outside_velocity: Vec<_> = commands
        .iter()
        .copied()
        .filter(|&pc| !radial_velocity::contains(pc) && !linked_velocity::contains(pc))
        .collect();
    let commands = outside_velocity.as_slice();
    let mut out = thinned(image);
    elide_overwritten_motion(&mut out, commands)?;
    for &va in commands {
        let at = (va - SOURCE_BASE) as usize;
        if image[at] == 0x88 {
            let descriptor = u32::from_le_bytes(image[at + 4..at + 8].try_into().unwrap());
            let d = (descriptor - SOURCE_BASE) as usize;
            out[at + 4..at + 8].copy_from_slice(&image[d..d + 4]);
        }
    }
    for (va, axes) in trig_loops(image, commands) {
        let at = (va - SOURCE_BASE) as usize;
        out[at] = TRIG_LOOP;
        out[at + 1] = axes;
    }
    for (va, mask) in guarded_spawns(image, commands) {
        let at = (va - SOURCE_BASE) as usize;
        out[at] = GUARDED_SPAWN;
        out[at + 2..at + 4].copy_from_slice(&mask.to_le_bytes());
    }
    for &va in commands {
        let at = (va - SOURCE_BASE) as usize;
        let (op, mode) = (image[at], image[at + 1]);
        if (0x13..=0x15).contains(&op)
            && matches!(mode & 0xf0, 0x80 | 0xc0)
            && mode & 15 < COMPARE_RELATIONS
        {
            out[at] = compare_variant(op, mode & 0xf0 == 0x80, mode & 15);
        } else if let Some(v) = VARIANTS
            .iter()
            .find(|v| v.0 == op && v.1 == mode & 0xf0 && v.2.is_none_or(|o| o == mode & 15))
        {
            out[at] = v.3;
        }
    }
    blocks::patch(&mut out, commands)?;
    inward_turn::patch(&mut out, image, commands)?;
    radius_loop::patch(&mut out, commands)?;
    radial_branch::validate(&out, commands)?;
    radial_motion::patch(&mut out, image, commands)?;
    phase_table::patch(&mut out, commands)?;
    vertical_motion::patch(&mut out, image, commands)?;
    if commands
        .iter()
        .any(|&pc| image[(pc - SOURCE_BASE) as usize] == 0xaa)
    {
        return Err("Constant-velocity opcode already used by source".into());
    }
    radial_velocity::patch(&mut out, image, true)?;
    linked_velocity::patch(&mut out, image, true)?;
    orbit_turn::patch(&mut out, image, commands)?;
    falling_turn::patch(&mut out, image, commands)?;
    initializers::patch(&mut out, image, commands)?;
    jugem_density::patch(&mut out, image, commands)?;
    falling_density::patch(&mut out, image, commands)?;
    falling_birth::patch(&mut out, image, commands)?;
    falling_setup::patch(&mut out, image, commands)?;
    bound_child_copy::patch(&mut out, image, commands)?;
    common_radial::patch(&mut out, image, commands)?;
    common_blink::patch(&mut out, image, commands)?;
    common_density::patch(&mut out, image, commands)?;
    common_blink_density::patch(&mut out, image, commands)?;
    lightning_radius::patch(&mut out, image, commands)?;
    common_cohort::patch(&mut out, image, commands)?;
    lightning_turn::patch(&mut out, image, commands)?;
    hidden_child::patch(&mut out, image, commands)?;
    Ok(out)
}
pub(super) fn elide_overwritten_motion(image: &mut [u8], commands: &[u32]) -> Result<()> {
    const START: u32 = 0x4a81ac;
    const SEQUENCE: [u8; 40] = [
        0x11, 0xa1, 0x92, 0x58, 0x11, 0xa1, 0x94, 0x58, 0x2d, 0x0b, 0, 0, 0x11, 0xa1, 0x92, 0x58,
        0x11, 0xa1, 0x94, 0x58, 0x2d, 0x0b, 0, 0, 0x11, 0x81, 0x58, 0, 1, 0, 0, 0, 0x06, 0, 16, 0,
        0xac, 0x81, 0x4a, 0,
    ];
    let at = (START - SOURCE_BASE) as usize;
    if image.get(at..at + SEQUENCE.len()) != Some(SEQUENCE.as_slice())
        || [0, 4, 8, 12, 16, 20, 24, 32, 40]
            .iter()
            .any(|offset| !commands.contains(&(START + offset)))
    {
        return Err("Overwritten body motion sequence or boundaries changed".into());
    }
    if commands.iter().any(|pc| {
        pc.checked_sub(SOURCE_BASE)
            .and_then(|offset| image.get(offset as usize))
            == Some(&INIT_LOOP_MOTION)
    }) {
        return Err("Initialization motion opcode already used by a source command".into());
    }
    image[at + 9] = 0;
    image[at + 20] = INIT_LOOP_MOTION;
    Ok(())
}
fn trig_loops(image: &[u8], commands: &[u32]) -> Vec<(u32, u8)> {
    let byte = |va: u32| image[(va - SOURCE_BASE) as usize];
    let set: BTreeSet<u32> = commands.iter().copied().collect();
    commands
        .iter()
        .filter_map(|&va| {
            let at = (va - SOURCE_BASE) as usize;
            if image[at] != 0x06 || image[at + 1] != 0 {
                return None;
            }
            let target = u32::from_le_bytes(image[at + 4..at + 8].try_into().unwrap());
            (set.contains(&target)
                && set.contains(&(target + 4))
                && byte(target) == 0x2d
                && byte(target + 2) == 0
                && byte(target + 3) == 0
                && image[(target + 4 - SOURCE_BASE) as usize..(target + 8 - SOURCE_BASE) as usize]
                    == [0x01, 0, 0, 0])
            .then(|| (va, byte(target + 1)))
        })
        .collect()
}
pub fn unread(
    image: &[u8],
    kept: &serde_json::Value,
    reached: &[bool],
) -> Result<Vec<(usize, usize)>> {
    if kept["base"].as_u64() != Some(u64::from(SOURCE_BASE)) {
        return Err("Kept script runs are for another image".into());
    }
    let mut read = vec![false; image.len()];
    for run in kept["kept"].as_array().ok_or("Kept script runs")? {
        let (start, end) = (
            run[0].as_u64().ok_or("Kept run")? as usize,
            run[1].as_u64().ok_or("Kept run")? as usize,
        );
        if start >= end || end > image.len() {
            return Err("Kept run outside the image".into());
        }
        read[start.saturating_sub(UNREAD_MARGIN)..(end + UNREAD_MARGIN).min(image.len())]
            .fill(true);
    }
    if reached.len() != image.len() {
        return Err("Reached script bytes cover another image".into());
    }
    for (at, _) in reached.iter().enumerate().filter(|(_, r)| **r) {
        read[at.saturating_sub(UNREAD_MARGIN)..(at + 1 + UNREAD_MARGIN).min(image.len())]
            .fill(true);
    }
    let mut runs = Vec::new();
    let mut at = 0;
    while at < image.len() {
        if read[at] || image[at] != 0 {
            at += 1;
            continue;
        }
        let start = at;
        while at < image.len() && !read[at] && image[at] == 0 {
            at += 1;
        }
        if at - start >= UNREAD_LEAST {
            runs.push((IMAGE_OFFSET + start, IMAGE_OFFSET + at));
        }
    }
    Ok(runs)
}
pub fn sections(
    sections: &mut Vec<Section>,
    space: &mut crate::sfc_rom::Space,
    image: &[u8],
    unread: &[(usize, usize)],
    roots: &[u32; crate::sfc_battle_scripts::DESCRIPTORS],
    sound: &crate::sfc_sound::Assets,
) -> Result<()> {
    crate::sfc_b1::add(
        sections,
        space,
        "bvm_effects".into(),
        (0..EFFECTS)
            .map(|n| sound.effect_code(n).unwrap_or(0))
            .collect(),
    )?;
    crate::sfc_b1::add(sections, space, "bvm_zero".into(), vec![0])?;
    crate::sfc_b1::add(
        sections,
        space,
        "battle_body_roots".into(),
        roots.iter().flat_map(|r| r.to_le_bytes()).collect(),
    )?;
    let quarter = |i: u32| -> i64 {
        let at = (0x45b3c8 + 2 * i - SOURCE_BASE) as usize;
        i64::from(u16::from_le_bytes([image[at], image[at + 1]]))
    };
    let sine = |a: u32| -> i64 {
        match a & 0xc00 {
            0 => quarter(a),
            0x400 => match 0x800 - a {
                b if b >= 0x400 => 0x10000,
                b => quarter(b),
            },
            0x800 => -quarter(a - 0x800),
            _ => match 0x1000 - a {
                b if b >= 0x400 => -0x10000,
                b => -quarter(b),
            },
        }
    };
    let cosine = |a: u32| -> i64 {
        match a & 0xc00 {
            0 if a == 0 => 0x10000,
            0 => quarter(0x400 - a),
            0x400 => -quarter(a - 0x400),
            0x800 => match 0xc00 - a {
                b if b >= 0x400 => -0x10000,
                b => -quarter(b),
            },
            _ => quarter(a - 0xc00),
        }
    };
    if (0..4096u32).any(|a| cosine(a) != sine((a + 0x400) & 0xfff)) {
        return Err("Cosine is not the sine a quarter turn on".into());
    }
    {
        let mut table = Vec::with_capacity(4 * 4096);
        for a in 0..4096u32 {
            table.extend(sine_digits(sine(a))?);
        }
        phase_table::fill_padding(&mut table)?;
        crate::sfc_b1::add(sections, space, "bvm_sine".into(), table)?;
    }
    if image.len() != (SOURCE_END - SOURCE_BASE) as usize {
        return Err("Battle script image size".into());
    }
    let mut read = Vec::new();
    let mut from = IMAGE_OFFSET;
    for &(start, end) in unread.iter().chain([&(IMAGE_OFFSET + image.len(), 0)]) {
        if start > from {
            read.push((from, start));
        }
        from = end;
    }
    let mut i = 0;
    for (start, end) in read {
        if sections
            .iter()
            .any(|s| s.offset < end && s.offset + s.bytes.len() > start)
        {
            return Err("ROM data reaches the battle script image".into());
        }
        let mut offset = start;
        while offset < end {
            let piece = ((offset | 0x7fff) + 1).min(end);
            sections.push(Section {
                name: format!("battle_scripts_{i}"),
                offset,
                bytes: image[offset - IMAGE_OFFSET..piece - IMAGE_OFFSET].to_vec(),
            });
            offset = piece;
            i += 1;
        }
    }
    Ok(())
}
