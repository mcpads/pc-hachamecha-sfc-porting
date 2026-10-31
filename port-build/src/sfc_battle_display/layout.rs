use super::assets::{overlay_rows, palette};
use super::*;
fn validate_palette_owners(table: &[[u8; 4]; 16]) -> Result<()> {
    let mut owners = [None; 256];
    for (source, entry) in table.iter().enumerate() {
        let target = usize::from(entry[0]);
        if target == 0 {
            continue;
        }
        if target + 16 > owners.len() {
            return Err("Battle palette extends beyond CGRAM".into());
        }
        for owner in &mut owners[target..target + 16] {
            if let Some(previous) = *owner {
                return Err(format!(
                    "Source palette blocks {previous} and {} share CGRAM {target}",
                    source * 16
                )
                .into());
            }
            *owner = Some(source * 16);
        }
    }
    Ok(())
}
fn data(
    sections: &mut Vec<Section>,
    space: &mut crate::sfc_rom::Space,
    name: String,
    bytes: Vec<u8>,
) -> Result<u32> {
    if bytes.is_empty() {
        return Ok(0);
    }
    crate::sfc_b1::add_shared(sections, space, name, bytes)
}
pub fn sections(
    sections: &mut Vec<Section>,
    space: &mut crate::sfc_rom::Space,
    assets: &Assets,
    battle_map: u32,
) -> Result<()> {
    use crate::sfc_b1::add;
    let effect_resource = &assets.resources[assets.arle_effect_resource];
    let effect_palette = add(
        sections,
        space,
        "battle_arle_effect_palette".into(),
        effect_resource.palette[8..16].to_vec(),
    )?;
    add(
        sections,
        space,
        "battle_arle_effect_resource".into(),
        vec![u8::try_from(assets.arle_effect_resource)?],
    )?;
    let mut lists: Vec<u8> = Vec::new();
    let mut store = crate::sfc_body_tiles::Store::default();
    for resource in &assets.resources {
        if let Frames::Sprites(frames) = &resource.frames {
            for chunk in frames.iter().flatten() {
                store.note(&chunk.tops)?;
                store.note(&chunk.bottoms)?;
            }
        }
    }
    store.place(sections, space)?;
    let mut tile_jobs: Vec<Vec<[(usize, usize); 2]>> = Vec::new();
    let mut tile_sources: Vec<Vec<u32>> = Vec::new();
    let mut chunk_sources: Vec<Vec<Vec<(u32, u32)>>> = Vec::new();
    for (r, resource) in assets.resources.iter().enumerate() {
        let mut jobs = Vec::new();
        let mut sources = Vec::new();
        let mut chunks = Vec::new();
        match &resource.frames {
            Frames::Tiles(frames) => {
                for (f, frame) in frames.iter().enumerate() {
                    let source = data(
                        sections,
                        space,
                        format!("bd_tiles_{r}_{f}"),
                        frame.tiles.clone(),
                    )?;
                    sources.push(source);
                    let mut both = [(0, 0); 2];
                    for (b, runs) in ARLE_BUFFERS.iter().enumerate() {
                        let start = lists.len();
                        let split = (runs[0].1 * 32).min(frame.tiles.len());
                        let mut count = 0;
                        for (i, part) in [&frame.tiles[..split], &frame.tiles[split..]]
                            .iter()
                            .enumerate()
                        {
                            if !part.is_empty() {
                                let from = source + if i == 0 { 0 } else { split as u32 };
                                lists.extend(transfer(from, runs[i].0, part.len(), 0));
                                count += 1;
                            }
                        }
                        lists.extend(transfer(SHADOW, SHADOW_VRAM, 16 * 64, 0));
                        both[b] = (start, count + 1);
                    }
                    jobs.push(both);
                }
            }
            Frames::Sprites(frames) => {
                for (f, frame) in frames.iter().enumerate() {
                    if frame.len() > CHUNK_NAMES_MAX {
                        return Err(format!(
                            "{} frame {f} has more chunks than the draw keeps",
                            resource.file
                        )
                        .into());
                    }
                    let mut at = Vec::new();
                    for (c, chunk) in frame.iter().enumerate() {
                        let mut half = |name: String, bytes: &[u8]| -> Result<u32> {
                            let list = store.list(bytes)?;
                            let source = data(sections, space, name, list.clone())?;
                            if crate::sfc_body_tiles::Store::rebuild(sections, &list)? != bytes {
                                return Err("Body runs do not rebuild their half".into());
                            }
                            Ok(source)
                        };
                        let tops = half(format!("bd_tops_{r}_{f}_{c}"), &chunk.tops)?;
                        let bottoms = half(format!("bd_bottoms_{r}_{f}_{c}"), &chunk.bottoms)?;
                        at.push((tops, bottoms));
                    }
                    chunks.push(at);
                }
            }
        }
        tile_jobs.push(jobs);
        tile_sources.push(sources);
        chunk_sources.push(chunks);
    }
    let mut palettes = Vec::new();
    for (r, resource) in assets.resources.iter().enumerate() {
        palettes.push(add(
            sections,
            space,
            format!("bd_palette_{r}"),
            resource.palette.clone(),
        )?);
    }
    let mut shared = vec![BTreeMap::new(); crate::sfc_battle_scripts::DESCRIPTORS];
    for (id, loads) in assets.enemy_loads.iter().enumerate() {
        let mut blocks: BTreeMap<u8, Vec<usize>> = BTreeMap::new();
        for &(_, r) in loads {
            let base = assets.resources[r].source_base;
            let list = blocks.entry(base).or_insert_with(|| {
                BATTLE_SHEETS
                    .into_iter()
                    .filter(|&b| assets.resources[b].source_base == base)
                    .collect()
            });
            if !list.contains(&r) {
                list.push(r);
            }
        }
        for (base, list) in blocks {
            let res = |r: usize| &assets.resources[r];
            if list.iter().all(|&r| res(r).key == res(list[0]).key) {
                continue;
            }
            let last = *list.last().unwrap();
            let fits = |p: u8| {
                list.iter().all(|&r| {
                    let opaque = res(r).used & !(1 << res(r).key);
                    res(r).key == p || opaque & (1 << p | 1) == 0
                })
            };
            let p = std::iter::once(res(last).key)
                .chain(0..16)
                .find(|&p| fits(p))
                .ok_or_else(|| format!("Enemy {id} sheets at {base} cannot share colours"))?;
            let at = add(
                sections,
                space,
                format!("bd_palette_e{id}_{base}"),
                palette(&res(last).rgb, p),
            )?;
            shared[id].insert(base, (p, last, at));
        }
    }
    let mut rgb = Vec::new();
    for resource in &assets.resources {
        rgb.extend(&resource.rgb);
    }
    let backdrop_rgb = rgb.len() as u16;
    rgb.extend(&assets.backdrop_rgb);
    let mut upper_rgb = BTreeMap::new();
    for (r, resource) in assets.resources.iter().enumerate() {
        if let Some(upper) = &resource.rgb_upper {
            upper_rgb.insert(r, u16::try_from(rgb.len())?);
            rgb.extend(upper);
        }
    }
    add(sections, space, "battle_palette_rgb".into(), rgb)?;
    let scene_table = {
        let mut table = [[0u8; 4]; 16];
        let mut seen = std::collections::BTreeSet::new();
        for (_, actions) in &assets.streams {
            for action in actions {
                if let StreamAction::Load { resource, base, .. } = *action
                    && seen.insert(base)
                {
                    let r = &assets.resources[usize::from(resource)];
                    let offset = (48 * usize::from(resource)) as u16;
                    table[usize::from(base / 16)] =
                        [r.colors as u8, r.key, offset as u8, (offset >> 8) as u8];
                }
            }
        }
        table[160 / 16] = [
            BACKDROP_COLORS,
            0,
            backdrop_rgb as u8,
            (backdrop_rgb >> 8) as u8,
        ];
        validate_palette_owners(&table)?;
        table
    };
    let mut blocks = Vec::new();
    for (id, (loads, shared)) in assets.enemy_loads.iter().zip(&shared).enumerate() {
        if id == SCENE_ID {
            blocks.extend(scene_table.iter().flatten());
            continue;
        }
        let mut table = [[0u8; 4]; 16];
        for r in PALETTE_SHEETS
            .into_iter()
            .chain(loads.iter().map(|&(_, r)| r))
        {
            let resource = &assets.resources[r];
            if !resource.source_base.is_multiple_of(16) {
                return Err(format!("{} colours start inside a block", resource.file).into());
            }
            let key = shared
                .get(&resource.source_base)
                .map_or(resource.key, |&(p, _, _)| p);
            let offset = (48 * r) as u16;
            table[usize::from(resource.source_base / 16)] = [
                resource.colors as u8,
                key,
                offset as u8,
                (offset >> 8) as u8,
            ];
            if let Some(&upper) = upper_rgb.get(&r) {
                table[usize::from(resource.source_base / 16) + 1] = [
                    resource.colors as u8 + 16,
                    0,
                    upper as u8,
                    (upper >> 8) as u8,
                ];
            }
        }
        validate_palette_owners(&table)?;
        blocks.extend(table.iter().flatten());
    }
    add(sections, space, "battle_palette_blocks".into(), blocks)?;
    let mut setups = Vec::new();
    let mut setup_counts = Vec::new();
    for (id, (loads, shared)) in assets.enemy_loads.iter().zip(&shared).enumerate() {
        setups.push(lists.len());
        lists.extend(transfer(battle_map, 0xf800, 2048, 0));
        lists.extend(transfer(effect_palette, arle_effects::COLORS, 8, 1));
        if id == SCENE_ID {
            setup_counts.push(2);
            continue;
        }
        let mut count = 2;
        for r in PALETTE_SHEETS {
            let bytes = assets.resources[r].palette.len();
            lists.extend(transfer(palettes[r], assets.resources[r].colors, bytes, 1));
            count += 1;
        }
        let mut sent = Vec::new();
        for &(_, r) in loads {
            if !sent.contains(&r) {
                sent.push(r);
                let colours = match shared.get(&assets.resources[r].source_base) {
                    Some(&(_, last, at)) if last == r => at,
                    _ => palettes[r],
                };
                let bytes = if colours == palettes[r] {
                    assets.resources[r].palette.len()
                } else {
                    32
                };
                lists.extend(transfer(colours, assets.resources[r].colors, bytes, 1));
                count += 1;
            }
        }
        setup_counts.push(count);
    }
    if lists.len() > 0x8000 {
        return Err("Battle display lists exceed a bank".into());
    }
    let lists_at = add(sections, space, "battle_display_lists".into(), lists)?;
    let job = |offset: usize, count: usize| {
        let mut b = (lists_at + offset as u32).to_le_bytes()[..3].to_vec();
        b.extend((count as u16).to_le_bytes());
        b
    };
    let header = 4 * assets.resources.len();
    let mut records = vec![0u8; header];
    for (r, resource) in assets.resources.iter().enumerate() {
        let frames = match &resource.frames {
            Frames::Tiles(f) => f.len(),
            Frames::Sprites(f) => f.len(),
        };
        let table = records.len();
        records[4 * r..4 * r + 2].copy_from_slice(&(table as u16).to_le_bytes());
        let palette = ((resource.colors - 128) / 16) as u8;
        records[4 * r + 2] = matches!(resource.frames, Frames::Tiles(_)) as u8;
        records[4 * r + 3] = 0x20
            | if matches!(resource.frames, Frames::Tiles(_)) {
                0
            } else {
                palette << 1
            };
        records.extend(vec![0; 2 * frames]);
        for f in 0..frames {
            let at = records.len() as u16;
            records[table + 2 * f..table + 2 * f + 2].copy_from_slice(&at.to_le_bytes());
            match &resource.frames {
                Frames::Tiles(t) => {
                    for (start, count) in tile_jobs[r][f] {
                        records.extend(job(start, count));
                    }
                    records.extend([
                        t[f].columns as u8,
                        t[f].rows as u8,
                        t[f].align.0 as u8,
                        t[f].align.1 as u8,
                    ]);
                    records.extend(t[f].cells.iter().flat_map(|c| c.to_le_bytes()));
                }
                Frames::Sprites(s) => {
                    records.push(s[f].len() as u8);
                    for (c, chunk) in s[f].iter().enumerate() {
                        let (tops, bottoms) = chunk_sources[r][f][c];
                        records
                            .push(chunk.offsets.len() as u8 | if chunk.upper { 0x80 } else { 0 });
                        records.extend(&tops.to_le_bytes()[..3]);
                        records.extend(&bottoms.to_le_bytes()[..3]);
                        records.extend(chunk.offsets.iter().flatten());
                    }
                }
            }
        }
    }
    if records.len() > 0x10000 {
        return Err(format!(
            "Battle display records ({} bytes) exceed a bank",
            records.len()
        )
        .into());
    }
    let offset = space.reserve_table(records.len())?;
    sections.push(Section {
        name: "battle_display".into(),
        offset,
        bytes: records,
    });
    let mut jobs = Vec::new();
    for at in setups {
        jobs.extend(job(at, 0));
    }
    for (id, count) in setup_counts.into_iter().enumerate() {
        jobs[id * 5 + 3..id * 5 + 5].copy_from_slice(&(count as u16).to_le_bytes());
    }
    add(sections, space, "battle_display_setup".into(), jobs)?;
    let mut slots = Vec::new();
    for id in 0..crate::sfc_battle_scripts::DESCRIPTORS {
        let mut row = vec![0u8; SLOTS_PER_TABLE];
        if id != SCENE_ID {
            for &r in &BATTLE_SHEETS {
                row[usize::from(assets.resources[r].slot)] = r as u8 + 1;
            }
            for &(slot, r) in &assets.enemy_loads[id] {
                row[usize::from(slot)] = r as u8 + 1;
            }
        }
        slots.extend(row);
    }
    add(sections, space, "battle_display_slots".into(), slots)?;
    let mut names = Vec::new();
    for runs in ARLE_BUFFERS {
        for cell in 0..=ARLE_TILES {
            let index = cell.saturating_sub(1);
            let (vram, at) = if index < runs[0].1 {
                (runs[0].0, index)
            } else {
                (runs[1].0, index - runs[0].1)
            };
            let name = (vram - crate::sfc_b1::BG2_TILES) / 32 + at as u16;
            names.extend((name | ARLE_PALETTE << 10 | 0x2000).to_le_bytes());
        }
    }
    add(sections, space, "battle_display_arle_names".into(), names)?;
    let high_bits = (0..4u8)
        .flat_map(|k| [3u8 << (2 * k), 2 << (2 * k)])
        .collect();
    add(
        sections,
        space,
        "battle_display_high_bits".into(),
        high_bits,
    )?;
    let mut streams = Vec::new();
    for (va, actions) in assets.streams.iter().chain(&assets.reserved_streams) {
        streams.extend(&va.to_le_bytes()[..3]);
        streams.push(u8::try_from(actions.len())?);
        for action in actions {
            streams.extend(match *action {
                StreamAction::Load {
                    slot,
                    resource,
                    base,
                } => [slot, resource + 1, base],
                StreamAction::Release { slot } => [slot, 0, 0],
            });
        }
    }
    streams.extend([0, 0, 0]);
    add(sections, space, "scene_streams".into(), streams)?;
    let (rows, offsets) = match &assets.boss {
        Some(b) => overlay_rows(&b.rows, b.height)?,
        None => (vec![0, 0, 0, 0], vec![0, 0]),
    };
    let mut extents = vec![0x00, 0x40, 0x00, 0xc0];
    let mut firsts: Vec<u8> = vec![0, 0];
    for r in &assets.resources {
        firsts.extend(u16::try_from(extents.len())?.to_le_bytes());
        for (f, &[_, t, _, b]) in r.rects.iter().enumerate() {
            let ay = r.anchors.get(f).map_or(0, |a| a.1);
            extents.extend(
                ((b - t) as i32 - ay + 2)
                    .clamp(-0x4000, 0x4000)
                    .to_le_bytes()[..2]
                    .to_vec(),
            );
            extents.extend((-ay - 2).clamp(-0x4000, 0x4000).to_le_bytes()[..2].to_vec());
        }
    }
    add(sections, space, "battle_display_extents".into(), extents)?;
    add(
        sections,
        space,
        "battle_display_extent_firsts".into(),
        firsts,
    )?;
    add(sections, space, "battle_boss_view".into(), rows)?;
    add(sections, space, "battle_boss_rows".into(), offsets)?;
    let mut info = Vec::new();
    match &assets.boss {
        Some(b) => {
            info.extend((b.height as u16).to_le_bytes());
            info.push(b.slot);
            info.extend(b.frame.to_le_bytes());
            info.push(u8::try_from(b.enemies.len())?);
            info.extend(&b.enemies);
        }
        None => info.extend([0; 6]),
    }
    add(sections, space, "battle_boss_info".into(), info)?;
    add(
        sections,
        space,
        "battle_boss_lines".into(),
        (0..32u16)
            .flat_map(|r| ((r >> 3) * 13 * 64 + (r & 7) * 2).to_le_bytes())
            .collect(),
    )?;
    for (name, f) in [
        (
            "battle_display_left_x",
            &(|v: i32| screen_x(LEFT, v)) as &dyn Fn(i32) -> i32,
        ),
        ("battle_display_right_x", &|v: i32| screen_x(RIGHT, v)),
        ("battle_display_y", &|v: i32| screen_y(LEFT, v)),
    ] {
        add(
            sections,
            space,
            name.into(),
            (0..COORDS as i32)
                .flat_map(|i| (f(COORD_FIRST + i) as i16).to_le_bytes())
                .collect(),
        )?;
    }
    Ok(())
}
