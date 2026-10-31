use super::*;
pub fn prepare(
    content: &crate::content::Content,
    files: &[(String, Vec<u8>)],
    font: &DialogueFont,
    pages: &[String],
    dialogue: &crate::b1_dialogue::Catalogue,
    joins: &[crate::b1_dialogue::Join],
    source_colors: &[[u8; 3]],
) -> Result<Assets> {
    let text_layout = PageLayout {
        width: TEXT_WIDTH,
        lines: TEXT_LINES,
        advance: Box::new(|ch| font.advance(ch)),
        joins,
        used: Default::default(),
    };
    let world: World = content.json("b1-world.json")?;
    world.validate()?;
    let definition_bytes = content.read("b1-render.json")?;
    let def: Definition = serde_json::from_slice(&definition_bytes)?;
    if def.schema != 1
        || def.source_sha256 != SOURCE
        || def.source_viewport != [224, 240]
        || def.backdrop_rectangles.len() != 2
        || def.resource_slots.len() != 16
        || def.placements.len() != 19
        || pages.len() != 2
    {
        return Err("Invalid B1 render source".into());
    }
    let file = |name: &str| {
        files
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, b)| b.as_slice())
            .ok_or_else(|| format!("Missing B1 source {name}"))
    };
    let mut palette = vec![0; 512];
    palette[2..4].copy_from_slice(&0x7fffu16.to_le_bytes());
    let colors = file("DOOR2.CNS.rgb")?;
    for (i, c) in colors.as_chunks::<3>().0.iter().take(32).enumerate() {
        let value = u16::from(c[0] >> 3) | u16::from(c[1] >> 3) << 5 | u16::from(c[2] >> 3) << 10;
        palette[(48 + i) * 2..(49 + i) * 2].copy_from_slice(&value.to_le_bytes());
    }
    let (mut floors, catalogue) = crate::floor_events::load(content)?;
    {
        let objects: serde_json::Value = content.json("floor-objects/floor-objects.json")?;
        let mut alike: BTreeMap<String, u32> = BTreeMap::new();
        let mut drawing: BTreeMap<u32, u32> = BTreeMap::new();
        for object in objects["floors"]
            .as_array()
            .ok_or("Floor objects")?
            .iter()
            .flat_map(|f| f["objects"].as_array().into_iter().flatten())
        {
            let entry = u32::try_from(object["entry_va"].as_u64().ok_or("Object entry")?)?;
            let mut look = object.as_object().ok_or("Object")?.clone();
            for k in [
                "entry_va",
                "init_va",
                "resource_va",
                "presets",
                "initializer_state_sha256",
                "palette_va",
                "object_va",
                "target_palette_va",
            ] {
                look.remove(k);
            }
            let first = *alike
                .entry(serde_json::Value::Object(look).to_string())
                .or_insert(entry);
            drawing.insert(entry, first);
        }
        let draws = |entry: u32| drawing.get(&entry).copied().unwrap_or(entry);
        let mut shown: BTreeMap<u8, BTreeSet<u32>> = BTreeMap::new();
        for floor in &floors {
            shown
                .entry(floor.id)
                .or_default()
                .extend(floor.pictures.iter().map(|&e| draws(e)));
            for job in picture_jobs_of(floor)? {
                shown
                    .entry(job.state[0])
                    .or_default()
                    .extend(job.entries.iter().map(|&e| draws(e)));
            }
        }
        let mut ids: BTreeMap<u32, u8> = BTreeMap::new();
        for &entry in shown.values().flatten().collect::<BTreeSet<_>>() {
            let taken: BTreeSet<u8> = shown
                .values()
                .filter(|s| s.contains(&entry))
                .flatten()
                .filter_map(|e| ids.get(e).copied())
                .collect();
            let id = (1..crate::sfc_field_events::SLIDE_KEY)
                .find(|i| !taken.contains(i))
                .ok_or("Picture ids run out on one floor")?;
            ids.insert(entry, id);
        }
        for (&entry, &first) in &drawing {
            if let Some(&id) = ids.get(&first) {
                ids.insert(entry, id);
            }
        }
        for floor in &mut floors {
            floor.number_pictures(&ids)?;
        }
    }
    let walls_raw = content.read("floor-walls/floor-walls.json")?;
    let walls_source: WallSource = serde_json::from_slice(&walls_raw)?;
    let world_maps: serde_json::Value = content.json("world.json")?;
    let encounters = crate::sfc_encounter::load(&world_maps, BOUNDARY_FLOOR - 1)?;
    let mut floor_views = Vec::new();
    for floor in &floors {
        let id = floor.id;
        let map = if id == 1 {
            world.clone()
        } else {
            let cells = world_maps["maps"]
                .as_array()
                .and_then(|m| m.iter().find(|m| m["id"] == id))
                .ok_or("Missing floor cells")?["cells"]
                .clone();
            world.floor_map(id, serde_json::from_value(cells)?)?
        };
        let source = walls_source
            .floors
            .iter()
            .find(|f| f.floor == id)
            .ok_or("Missing floor wall sheets")?;
        if id == 1
            && (source.loads.len() != def.resources.len()
                || def.resources.iter().any(|r| {
                    !source.loads.iter().any(|l| {
                        (l.slot, &l.file, l.width, l.height, l.palette_base, l.key)
                            == (
                                r.slot,
                                &r.resource,
                                r.width,
                                r.height,
                                r.palette_base,
                                r.key,
                            )
                    })
                }))
        {
            return Err("B1 wall sheets differ from the render definition".into());
        }
        let mut sheets = BTreeMap::new();
        for load in &source.loads {
            let pixels = content.read(format!("floor-walls/{}.indices", load.file))?;
            if pixels.len() != load.width * load.height {
                return Err("Invalid floor wall sheet".into());
            }
            sheets.insert(load.slot, (load.clone(), pixels));
        }
        let floor_palette = content.read(format!("floor-walls/floor-{id}.palette"))?;
        let mut wall_palette = palette.clone();
        for (i, c) in floor_palette
            .get(48 * 3..80 * 3)
            .ok_or("Short floor palette")?
            .as_chunks::<3>()
            .0
            .iter()
            .enumerate()
        {
            let value =
                u16::from(c[0] >> 3) | u16::from(c[1] >> 3) << 5 | u16::from(c[2] >> 3) << 10;
            wall_palette[(48 + i) * 2..(49 + i) * 2].copy_from_slice(&value.to_le_bytes());
        }
        floor_views.push(FloorView {
            id,
            map,
            sheets,
            changes: floor.wall_changes(),
            choices: floor.wall_choices(),
            markers: floor.marker_changes(),
            palette: wall_palette,
        });
    }
    let portrait_root = content.sub("b1-portrait");
    let portrait_raw = portrait_root.read("portrait.json")?;
    let portrait: serde_json::Value = serde_json::from_slice(&portrait_raw)?;
    let portrait_pixels = portrait_root.read("healthy.indices")?;
    let portrait_rgb = portrait_root.read("healthy.rgb")?;
    if portrait["source_sha256"] != SOURCE
        || portrait["schema"] != 1
        || portrait["healthy"]["width"] != 224
        || portrait["healthy"]["height"] != 240
        || portrait_pixels.len() != 224 * 240
        || portrait_rgb.len() % 3 != 0
        || portrait_rgb.len() > 112 * 3
        || portrait_pixels
            .iter()
            .any(|&v| usize::from(v) >= portrait_rgb.len() / 3)
        || portrait["healthy"]["pixels_sha256"] != sha(&portrait_pixels)
        || portrait["healthy"]["palette_sha256"] != sha(&portrait_rgb)
    {
        return Err("Invalid healthy HUD portrait".into());
    }
    let mut portraits = Vec::new();
    for k in 0..crate::sfc_b1_portrait::BACKGROUNDS {
        let pixels = portrait_root.read(format!("healthy-{k}.indices"))?;
        let rgb = portrait_root.read(format!("healthy-{k}.rgb"))?;
        let expected = &portrait["healthy"]["backgrounds"][k];
        if expected["pixels_sha256"] != sha(&pixels)
            || expected["palette_sha256"] != sha(&rgb)
            || rgb.len() != portrait_rgb.len()
            || (k == 0 && (pixels != portrait_pixels || rgb != portrait_rgb))
        {
            return Err("Invalid HP portrait background".into());
        }
        portraits.push((pixels, rgb));
    }
    let background_slots: Vec<usize> = portrait["idle"]["background_slots"]
        .as_array()
        .ok_or("Missing portrait background slots")?
        .iter()
        .filter_map(|v| v.as_u64().map(|v| v as usize))
        .collect();
    let frame = crate::sfc_b1_frame::Assets::load(content)?;
    palette[20..96].copy_from_slice(&frame.palette[20..96]);
    let equipment = crate::sfc_b1_equipment::Assets::load(
        content,
        &frame,
        &palette,
        &portrait_rgb,
        &background_slots,
    )?;
    let order = &equipment.portrait_order;
    let mut portrait_palettes = Vec::new();
    for (k, (_, rgb)) in portraits.iter().enumerate() {
        let mut colours = palette.clone();
        for (i, c) in rgb.as_chunks::<3>().0.iter().enumerate() {
            let value =
                u16::from(c[0] >> 3) | u16::from(c[1] >> 3) << 5 | u16::from(c[2] >> 3) << 10;
            let at = 80 + order[i];
            colours[at * 2..at * 2 + 2].copy_from_slice(&value.to_le_bytes());
        }
        for &(at, value) in &equipment.colors {
            colours[at * 2..at * 2 + 2].copy_from_slice(&value.to_le_bytes());
        }
        if k == 0 {
            palette = colours.clone();
        }
        portrait_palettes.push(colours[2 * 80..2 * 112].to_vec());
    }
    let stone = crate::sfc_b1_stone::Assets::load(content, &frame)?;
    let [_, _, pw, ph] = PORTRAIT;
    let source_palettes: Vec<Vec<[f64; 3]>> = portraits
        .iter()
        .map(|(_, rgb)| crate::sfc_portrait_scale::palette(rgb))
        .collect();
    let scale = |source: &[u8], carbuncle: &[bool], background: usize| {
        scale_portrait(source, carbuncle, &source_palettes[background], order)
    };
    let portrait = crate::sfc_b1_portrait::Assets::load(
        content,
        pw,
        ph,
        &scale,
        crate::sfc_b1_frame::XP_BASE / 64,
    )?;
    let pictures: Vec<Vec<u8>> = portraits.into_iter().map(|(pixels, _)| pixels).collect();
    let screen = static_screen(&frame, &pictures, &scale, &portrait)?;
    let title = crate::sfc_title::Assets::prepare(content, font)?;
    let mut rows: Vec<Vec<u8>> = Vec::new();
    let mut row_ids = BTreeMap::new();
    let mut views: Vec<View> = Vec::new();
    let mut lookup = BTreeMap::new();
    let mut states = Vec::new();
    let mut receipts = Vec::new();
    let (rays, masks, slot_zero) = world.rays();
    let walls = crate::sfc_floor_walls::Walls {
        rays: &rays,
        masks,
        slot_zero,
    };
    for fv in &floor_views {
        for s in 0..256usize {
            let (x, y, d) = (((s >> 2) & 7) as u8, (s >> 5) as u8, (s & 3) as u8);
            if walls.slots(fv.map.cells(), x, y, d)? != fv.map.view([fv.id, x, y, d])? {
                return Err("Cast wall slots differ from the view model".into());
            }
        }
    }
    type Key = [u8; crate::sfc_floor_walls::KEY_BYTES];
    type ViewJob = (u8, [u8; 20], u8, Vec<u32>, [i32; 2]);
    let mut jobs: Vec<ViewJob> = Vec::new();
    let mut keyed: Vec<BTreeMap<Key, u16>> = Vec::new();
    let mut state_keys: BTreeMap<[u8; 4], Vec<Key>> = BTreeMap::new();
    for fv in &floor_views {
        let mut floor_keys = BTreeMap::new();
        for s in 0..256usize {
            let (x, y, d) = (((s >> 2) & 7) as u8, (s >> 5) as u8, (s & 3) as u8);
            let mut keys = Vec::new();
            for cells in wall_variants(fv.map.cells(), &fv.choices, x, y)? {
                let slots = walls.slots(&cells, x, y, d)?;
                let key = crate::sfc_floor_walls::key(&slots, (x ^ y ^ d) & 1);
                if !keys.contains(&key) {
                    keys.push(key);
                }
                floor_keys.entry(key).or_insert_with(|| {
                    jobs.push((fv.id, slots, (x ^ y ^ d) & 1, Vec::new(), [0, 0]));
                    u16::try_from(jobs.len() - 1).unwrap_or(u16::MAX)
                });
            }
            state_keys.insert([fv.id, x, y, d], keys);
        }
        keyed.push(floor_keys);
    }
    let mut picture_entries: BTreeMap<usize, Vec<[u8; 6]>> = BTreeMap::new();
    let mut all_jobs = Vec::new();
    let mut shown_from: BTreeMap<u8, Vec<u8>> = BTreeMap::new();
    for fv in &floor_views {
        for job in picture_jobs_of(&floors[usize::from(fv.id) - 1])? {
            if !state_keys.contains_key(&job.state) {
                continue;
            }
            let from = shown_from
                .entry(job.state[0])
                .or_insert_with(|| vec![job.state[0]]);
            if !from.contains(&fv.id) {
                from.push(fv.id);
            }
            all_jobs.push(job);
        }
    }
    for fv in &floor_views {
        for job in all_jobs.iter().filter(|j| j.state[0] == fv.id) {
            let [_, x, y, d] = job.state;
            for key in &state_keys[&job.state] {
                let base = keyed[usize::from(fv.id) - 1][key];
                let slots = jobs[usize::from(base)].1;
                jobs.push((
                    fv.id,
                    slots,
                    (x ^ y ^ d) & 1,
                    job.entries.clone(),
                    job.shift,
                ));
                let [b0, b1] = base.to_le_bytes();
                let [r0, r1] = u16::try_from(jobs.len() - 1)?.to_le_bytes();
                let slot = (usize::from(fv.id) - 1) * 256 + usize::from((y * 8 + x) * 4 + d);
                picture_entries
                    .entry(slot)
                    .or_default()
                    .push([job.key[0], job.key[1], b0, b1, r0, r1]);
            }
        }
    }
    let mut guardian = crate::sfc_field_guardian::Assets::load(content, DUNGEON)?;
    let sprite_pictures = guardian.sprite_pictures();
    let mut floor_pictures = BTreeMap::new();
    for fv in &floor_views {
        let from = shown_from
            .get(&fv.id)
            .cloned()
            .unwrap_or_else(|| vec![fv.id]);
        floor_pictures.insert(
            fv.id,
            crate::b1_art::FloorPictures::load(content, &from)?.with_sprites(&sprite_pictures),
        );
    }
    let mut job_targets = Vec::new();
    for (floor, slots, parity, pictures, shift) in &jobs {
        let fv = &floor_views[usize::from(*floor) - 1];
        let draws = draws_of(&def, slots, *parity)?;
        let mut source_pixels = vec![0; 224 * 240];
        for draw in &draws {
            let (sheet, pixels) = fv.sheets.get(&draw.slot).ok_or("Unloaded wall sheet")?;
            let [l, t, r, b] = draw.source;
            let [dx, dy, right, bottom] = draw.destination;
            if l >= r
                || t >= b
                || r > sheet.width
                || b > sheet.height
                || right > 224
                || bottom > 240
            {
                return Err("Invalid wall rectangle".into());
            }
            for sy in t..b {
                for sx in l..r {
                    let color = pixels[sy * sheet.width + sx]
                        .checked_add(sheet.palette_base)
                        .ok_or("Wall palette overflow")?;
                    if !draw.transparent || Some(color) != sheet.key {
                        source_pixels[(dy + sy - t) * 224 + dx + sx - l] = color;
                    }
                }
            }
        }
        let mut frame_palette = fv.palette.clone();
        floor_pictures[floor].paint(
            pictures,
            *shift,
            &mut source_pixels,
            &mut frame_palette,
            &VIEW_COLORS,
        )?;
        for (n, chunk) in fv.palette.as_chunks::<2>().0.iter().enumerate() {
            let changed = frame_palette[n * 2..n * 2 + 2] != *chunk;
            if changed && !VIEW_COLORS.iter().any(|[a, b]| (*a..*b).contains(&n)) {
                return Err(format!(
                    "Picture palette entry {n} outside view ranges: {pictures:x?}"
                )
                .into());
            }
        }
        let [_, _, width, height] = DUNGEON;
        let mut pixels = vec![0; width * height];
        for dy in 0..height {
            for dx in 0..width {
                pixels[dy * width + dx] =
                    source_pixels[(dy * 240 / height) * 224 + dx * 224 / width];
            }
        }
        let mut view_rows = [0; DUNGEON_ROWS];
        for (r, slot) in view_rows.iter_mut().enumerate() {
            let bytes: Vec<u8> = (0..DUNGEON_COLUMNS)
                .flat_map(|c| planar(&pixels, width, r * DUNGEON_COLUMNS + c, 8))
                .collect();
            *slot = *row_ids.entry(bytes.clone()).or_insert_with(|| {
                rows.push(bytes);
                rows.len() - 1
            });
        }
        let view_palette: Vec<u8> = VIEW_COLORS
            .iter()
            .flat_map(|[a, b]| frame_palette[a * 2..b * 2].to_vec())
            .collect();
        let key = (view_rows, view_palette.clone());
        let id = *lookup.entry(key).or_insert_with(|| {
            views.push(View {
                rows: view_rows,
                palette: view_palette,
            });
            views.len() - 1
        });
        states.push(id);
        job_targets.push((sha(&source_pixels), sha(&pixels)));
    }
    for (state, keys) in &state_keys {
        let floor = usize::from(state[0]) - 1;
        let base = usize::from(keyed[floor][&keys[0]]);
        receipts.push(serde_json::json!(
            { "state" : state, "objects" : Vec::< String >::new(), "view" :
            states[base], "source_pixels_sha256" : job_targets[base].0,
            "target_pixels_sha256" : job_targets[base].1 }
        ));
    }
    for (slot, entries) in &picture_entries {
        let state = [
            (slot / 256 + 1) as u8,
            (((slot % 256) >> 2) & 7) as u8,
            ((slot % 256) >> 5) as u8,
            (slot & 3) as u8,
        ];
        let initial = keyed[slot / 256][&state_keys[&state][0]];
        for entry in entries
            .iter()
            .filter(|e| u16::from_le_bytes([e[2], e[3]]) == initial)
        {
            let job = usize::from(u16::from_le_bytes([entry[4], entry[5]]));
            let objects: Vec<String> = jobs[job].3.iter().map(|e| format!("{e:#x}")).collect();
            receipts.push(serde_json::json!(
                { "state" : state, "objects" : objects, "shift" : jobs[job].4,
                "view" : states[job], "source_pixels_sha256" : job_targets[job]
                .0, "target_pixels_sha256" : job_targets[job].1 }
            ));
        }
    }
    let view_keys: Vec<serde_json::Value> = keyed
        .iter()
        .map(|floor_keys| {
            floor_keys
                .iter()
                .map(|(key, &job)| {
                    let hex: String = key.iter().map(|b| format!("{b:02x}")).collect();
                    (
                        hex,
                        serde_json::Value::from(job_targets[usize::from(job)].1.clone()),
                    )
                })
                .collect::<serde_json::Map<_, _>>()
                .into()
        })
        .collect();
    let mut picture_keys: Vec<serde_json::Map<String, serde_json::Value>> =
        vec![serde_json::Map::new(); floor_views.len()];
    for (slot, entries) in &picture_entries {
        let floor = slot / 256;
        for e in entries {
            let base = u16::from_le_bytes([e[2], e[3]]);
            let job = usize::from(u16::from_le_bytes([e[4], e[5]]));
            if let Some((key, _)) = keyed[floor].iter().find(|(_, r)| **r == base) {
                let hex: String = key.iter().map(|b| format!("{b:02x}")).collect();
                picture_keys[floor].insert(
                    format!("{hex}:{}:{}", e[0], e[1]),
                    job_targets[job].1.clone().into(),
                );
            }
        }
    }
    let records = states.clone();
    let mut view_tables = Vec::new();
    for floor_keys in &keyed {
        view_tables.push(crate::sfc_floor_walls::table(floor_keys)?);
    }
    let picture_jobs: Vec<PictureJob> = floors
        .iter()
        .map(picture_jobs_of)
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect();
    let mut picture_table = vec![0xff; floor_views.len() * 512];
    for (slot, entries) in &picture_entries {
        let at = u16::try_from(picture_table.len())?.to_le_bytes();
        picture_table[slot * 2..slot * 2 + 2].copy_from_slice(&at);
        for entry in entries {
            picture_table.extend(entry);
        }
        picture_table.extend([0xff, 0, 0, 0, 0, 0]);
    }
    let floor_cells: Vec<Vec<u16>> = floor_views.iter().map(|f| f.map.cells().to_vec()).collect();
    let mut picture_songs = vec![0; floors.len() * 256];
    for (&floor, pictures) in &floor_pictures {
        let created = &floors[usize::from(floor) - 1].pictures;
        for picture in pictures.pictures.values() {
            let Some(id) = created.iter().position(|&v| v == picture.entry_va) else {
                continue;
            };
            let at = (usize::from(floor) - 1) * 256 + id + 1;
            if !picture.surface {
                picture_songs[at] |= 0x80;
            }
            let Some(song) = picture.leaving_song else {
                continue;
            };
            let song =
                crate::sfc_sound::source_song(song.into()).ok_or("Unported leaving song")? + 1;
            if song >= 0x80 {
                return Err("Leaving song number overlaps the picture's deletion bit".into());
            }
            picture_songs[at] |= song;
        }
    }
    let walls: serde_json::Value = content.json("floor-walls/floor-walls.json")?;
    let wall_lists: BTreeSet<u64> = walls["floors"]
        .as_array()
        .ok_or("Floor walls")?
        .iter()
        .flat_map(|f| f["colour_lists"].as_array().into_iter().flatten())
        .filter_map(|v| v.as_u64())
        .collect();
    let mut recolours = Vec::new();
    for &va in &catalogue.palettes {
        if wall_lists.contains(&u64::from(va)) {
            recolours.extend([0, 0]);
            continue;
        }
        let pair = floor_pictures
            .values()
            .map(|f| f.recoloured(va))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .next()
            .ok_or_else(|| format!("Colour list {va:#x} recolours no picture"))?;
        let created = &floors
            .iter()
            .find(|f| f.pictures.contains(&pair.0) && f.pictures.contains(&pair.1))
            .ok_or("Recoloured pictures are not created on one floor")?
            .pictures;
        let id = |entry: u32| -> Result<u8> {
            let at = created
                .iter()
                .position(|&v| v == entry)
                .ok_or("Recoloured picture is not created")?;
            Ok(u8::try_from(at + 1)?)
        };
        recolours.extend([id(pair.0)?, id(pair.1)?]);
    }
    let backdrop_view = {
        let folder = content.sub("battle-bodies");
        let source = folder.read("LB_2.CNS.indices")?;
        if source.len() != 224 * 320 {
            return Err("Scene backdrop is not 224x320".into());
        }
        let [_, _, width, height] = DUNGEON;
        let mut pixels = vec![0; width * height];
        for dy in 0..height {
            for dx in 0..width {
                pixels[dy * width + dx] = source[(dy * 240 / height) * 224 + dx * 224 / width]
                    + crate::sfc_battle_display::BACKDROP_COLORS;
            }
        }
        let mut view_rows = [0; DUNGEON_ROWS];
        for (r, slot) in view_rows.iter_mut().enumerate() {
            let bytes: Vec<u8> = (0..DUNGEON_COLUMNS)
                .flat_map(|c| planar(&pixels, width, r * DUNGEON_COLUMNS + c, 8))
                .collect();
            *slot = *row_ids.entry(bytes.clone()).or_insert_with(|| {
                rows.push(bytes);
                rows.len() - 1
            });
        }
        views.push(View {
            rows: view_rows,
            palette: vec![0; VIEW_COLORS.iter().map(|[a, b]| (b - a) * 2).sum()],
        });
        views.len() - 1
    };
    let wall_changes: BTreeMap<u8, Vec<crate::floor_events::WallChange>> = floor_views
        .iter()
        .map(|f| (f.id, f.changes.clone()))
        .collect();
    let marker_changes: BTreeMap<u8, Vec<(u8, u8)>> = floor_views
        .iter()
        .map(|f| (f.id, f.markers.clone()))
        .collect();
    let map = crate::sfc_b1_map::Assets::load(content, &wall_changes, &marker_changes)?;
    let map_first_view = views.len();
    let mut panel_views: Vec<[usize; 16]> = views.iter().map(|v| v.rows).collect();
    for panel in map.panels() {
        let mut ids = [0; 16];
        for (slot, row) in ids.iter_mut().zip(panel) {
            *slot = *row_ids.entry(map.rows()[*row].clone()).or_insert_with(|| {
                rows.push(map.rows()[*row].clone());
                rows.len() - 1
            });
        }
        panel_views.push(ids);
    }
    let view_tiles =
        crate::sfc_b1_view::Assets::pack(&rows, &panel_views, DUNGEON_COLUMNS, DUNGEON_TILE * 64)?;
    let mut all_pages = Vec::new();
    for page in pages {
        let laid = text_layout.pages(page)?;
        if laid.len() != 1 {
            return Err("B1 introductory page exceeds the dialogue window".into());
        }
        all_pages.extend(laid);
    }
    all_pages.push(String::new());
    let sound = crate::sfc_sound::Assets::load(content)?;
    let hover_layout = PageLayout {
        width: TEXT_WIDTH,
        lines: TEXT_LINES - 1,
        advance: Box::new(|ch| font.advance(ch)),
        joins,
        used: Default::default(),
    };
    let menu = crate::sfc_b1_menu::Assets::load(content, &text_layout, &hover_layout, &sound)?;
    let b3: serde_json::Value = content.json("b3-puzzles.json")?;
    let effects = b3["interactions"]
        .as_array()
        .and_then(|list| list.iter().find_map(|i| i["effects"].as_object()))
        .ok_or("Missing B3 cracked-wall effect")?;
    let root = effects["graph"]["roots"][0]
        .as_u64()
        .ok_or("Missing effect root")? as u32;
    let quake_root = catalogue
        .objects
        .iter()
        .position(|&va| va == root)
        .ok_or("Cracked-wall effect is not a floor object")? as u16;
    let offsets: Vec<i64> = effects["explosion"]["shake_offsets"]
        .as_array()
        .ok_or("Missing explosion shake")?
        .iter()
        .filter_map(|v| v.as_i64())
        .collect();
    if b3["source_sha256"] != SOURCE || offsets.len() != 32 {
        return Err("Invalid B3 cracked-wall effect".into());
    }
    let smoke = crate::sfc_field_smoke::Assets::load(content, DUNGEON)?;
    let shards = crate::sfc_field_shards::Assets::load(content, DUNGEON)?;
    let mut flames = crate::sfc_field_flames::Assets::load(content, DUNGEON)?;
    flames.body_id = u8::try_from(
        floors
            .iter()
            .find_map(|f| f.pictures.iter().position(|&va| va == flames.body))
            .ok_or("Fire barrier body is not a floor object")?
            + 1,
    )?;
    for (i, (map, entry)) in guardian.maps().into_iter().enumerate() {
        guardian.ids[i] = u8::try_from(
            floors
                .iter()
                .find(|f| f.id == map)
                .and_then(|f| f.pictures.iter().position(|&va| va == entry))
                .ok_or("Guardian display is not a picture of its map")?
                + 1,
        )?;
    }
    let shard_root = catalogue
        .objects
        .iter()
        .position(|&va| va == shards.root)
        .ok_or("Puyo door effect is not a floor object")? as u16;
    let scroll = |offset: i64| {
        let lines = crate::sfc::source_y_to_screen(offset);
        0xffffu16.wrapping_sub(lines as u16)
    };
    let mut quake = vec![0xffffu16];
    for &offset in offsets.iter().rev() {
        quake.push(scroll(offset));
    }
    let warps: serde_json::Value = content.json("circle-warps.json")?;
    if warps["source_sha256"] != SOURCE
        || warps["hum"]["done_flag"] != crate::sfc_b1_portrait::HUM_FLAG
    {
        return Err("Invalid circle warps".into());
    }
    let numbers = |list: &serde_json::Value| -> Result<Vec<i64>> {
        list.as_array()
            .ok_or("Circle warp list")?
            .iter()
            .map(|v| v.as_i64().ok_or_else(|| "Circle warp value".into()))
            .collect()
    };
    let hum: Vec<u16> = numbers(&warps["hum"]["offsets"])?
        .into_iter()
        .map(scroll)
        .collect();
    if hum.len() != crate::sfc_b1_portrait::HUM_TICKS {
        return Err("Circle hum is not one cycle".into());
    }
    let number = |va: i64| {
        catalogue
            .objects
            .iter()
            .position(|&v| i64::from(v) == va)
            .map(|i| i as u16)
    };
    let warp_hums: Vec<u16> = numbers(&warps["hum"]["objects"])?
        .into_iter()
        .filter_map(number)
        .collect();
    let mut warp_quakes: Vec<(u16, u16, u8)> = Vec::new();
    for site in warps["sites"].as_array().ok_or("Circle warp sites")? {
        let va = site["quake_va"].as_i64().ok_or("Circle quake")?;
        let Some(n) = number(va) else { continue };
        if warp_quakes.iter().any(|&(m, ..)| m == n) {
            continue;
        }
        let run = numbers(&warps["quakes"][format!("{va:#x}")])?;
        let base = if run == offsets {
            0
        } else {
            let at = quake.len();
            quake.extend(run.iter().rev().map(|&o| scroll(o)));
            at - 1
        };
        warp_quakes.push((n, u16::try_from(base * 2)?, u8::try_from(run.len())?));
    }
    let menu_first_page = u8::try_from(all_pages.len())?;
    all_pages.extend(menu.pages.iter().cloned());
    u8::try_from(all_pages.len() - 1).map_err(|_| "Menu pages exceed one byte")?;
    let field_tables = crate::sfc_field_events::tables(&floors, catalogue.follow_up)?;
    if crate::sfc_field_events::boundary(&floors) != BOUNDARY_FLOOR {
        return Err("Segment boundary is not the floor after the last program".into());
    }
    let (mut shown, mut signs) = (BTreeSet::new(), BTreeSet::new());
    for floor in &floors {
        let (all, clicked) = floor.messages();
        shown.extend(all);
        signs.extend(clicked);
    }
    shown.insert(usize::from(catalogue.follow_up));
    let mut field_messages = vec![[0xff; 8]; catalogue.messages.len()];
    let mut alternatives = Vec::new();
    let mut message_report = Vec::new();
    let mut audio_pending = Vec::new();
    let mut page_audio: Vec<[u8; 2]> = vec![[0, 0]; all_pages.len()];
    let mut page_pairs: Vec<[u8; 2]> = Vec::new();
    let mut page_delays = vec![0; all_pages.len()];
    let mut impact_page = None;
    for (index, &address) in catalogue.messages.iter().enumerate() {
        if !shown.contains(&index) {
            continue;
        }
        use crate::sfc_field_events::{MESSAGE_NOTICE, MESSAGE_QUESTION, MESSAGE_WAIT};
        if address == ENTRY_MESSAGE {
            field_messages[index] = [0, 0, 2, 0, MESSAGE_WAIT, 0, 0, 0];
            message_report.push(serde_json::json!(
                { "index" : index, "message_va" : address, "start" : 0, "end" :
                2, "mode" : MESSAGE_WAIT }
            ));
            continue;
        }
        let dispatch = dialogue.dispatch(address)?;
        let variants: Vec<usize> = match dispatch {
            Some((_, count)) => (0..count).collect(),
            None => vec![0],
        };
        let mut entries = Vec::new();
        for &variant in &variants {
            let silent = dispatch.is_some() && dialogue.is_silent(address, variant)?;
            let heal = address == crate::floor_events::FOLLOW_UP_MESSAGE && variant == 35 - 31;
            let crate::b1_dialogue::EventPages {
                pages,
                choice,
                audio,
                delays,
            } = if signs.contains(&index) {
                let (pages, audio) = dialogue.sign_variant(address, variant, &text_layout)?;
                crate::b1_dialogue::EventPages {
                    delays: vec![0; pages.len()],
                    pages,
                    choice: false,
                    audio,
                }
            } else if heal {
                let [b0, b1] =
                    u16::try_from(catalogue.messages.len() + alternatives.len())?.to_le_bytes();
                for (pages, delays) in &menu.heal_follow {
                    let first = all_pages.len();
                    page_audio.resize(first + pages.len(), [0, 0]);
                    page_delays.extend(delays);
                    all_pages.extend(pages.iter().cloned());
                    let ([f0, f1], [e0, e1]) = (
                        u16::try_from(first)?.to_le_bytes(),
                        u16::try_from(all_pages.len())?.to_le_bytes(),
                    );
                    alternatives.push([f0, f1, e0, e1, MESSAGE_WAIT, 0, 0, 0]);
                    message_report.push(serde_json::json!(
                        { "index" : index, "message_va" : address, "variant" :
                        variant, "heal" : alternatives.len(), "start" : first, "end"
                        : all_pages.len(), "mode" : MESSAGE_WAIT }
                    ));
                }
                entries.push([
                    b0,
                    b1,
                    u8::try_from(menu.heal_follow.len())?,
                    0,
                    crate::sfc_field_events::MESSAGE_DISPATCH,
                    crate::sfc_field_events::FOLLOW_PICK_SLOT,
                    0,
                    0,
                ]);
                continue;
            } else if silent {
                crate::b1_dialogue::EventPages {
                    pages: vec![String::new()],
                    choice: false,
                    audio: Vec::new(),
                    delays: vec![0],
                }
            } else if dialogue.is_notice(address) {
                crate::b1_dialogue::EventPages {
                    pages: vec![dialogue.notice(address, &text_layout)?],
                    choice: false,
                    audio: dialogue
                        .notice_audio(address)
                        .into_iter()
                        .map(|token| (0, token))
                        .collect(),
                    delays: vec![0],
                }
            } else {
                dialogue.event_variant(address, variant, &text_layout)?
            };
            let mode = if choice {
                MESSAGE_QUESTION
            } else if silent || dialogue.is_notice(address) {
                MESSAGE_NOTICE
            } else {
                MESSAGE_WAIT
            };
            let first = all_pages.len();
            if address == 0x49d5de {
                impact_page = Some(u16::try_from(first)?);
            }
            page_audio.resize(first + pages.len(), [0, 0]);
            let mut waited = false;
            for (page, token) in &audio {
                let at = first + (*page).min(pages.len() - 1);
                match token["kind"].as_str() {
                    Some("wait_audio") => {
                        waited = true;
                        page_audio[at][0] |= JINGLE_WAIT;
                    }
                    Some("audio_control")
                        if waited
                            && token["index"]
                                .as_u64()
                                .is_some_and(|i| FIELD_SONGS.contains(&i)) => {}
                    Some("audio_control") => {
                        let index = token["index"].as_u64().ok_or("B1 music index")?;
                        page_audio[at][0] =
                            crate::sfc_sound::source_song(index).ok_or("Unported B1 song")? + 1;
                    }
                    Some("sound_effect") => {
                        let index = token["index"].as_u64().ok_or("B1 effect index")?;
                        if let Some(effect) = sound.effect_code(index) {
                            let first = page_audio[at][1];
                            if first != 0
                                && first != effect + 1
                                && first < PAGE_PAIRS
                                && (first - 1 < crate::sfc_sound::STREAMED
                                    || effect < crate::sfc_sound::STREAMED)
                            {
                                page_pairs.push([first, effect + 1]);
                                page_audio[at][1] = PAGE_PAIRS - 1
                                    + u8::try_from(page_pairs.len())
                                        .ok()
                                        .filter(|&n| n <= 0xff - PAGE_PAIRS + 1)
                                        .ok_or("Too many two-effect pages")?;
                            } else {
                                page_audio[at][1] = effect + 1;
                            }
                        }
                    }
                    Some("audio") => {
                        let index = token["index"].as_u64().ok_or("Voice index")?;
                        if let Some(effect) = sound.voice_code(index) {
                            page_audio[at][1] = effect + 1;
                        }
                    }
                    _ => return Err("Unsupported B1 audio token".into()),
                }
            }
            audio_pending.extend(audio.into_iter().map(|(_, token)| token));
            page_delays.extend(delays);
            all_pages.extend(pages);
            let end = all_pages.len();
            let ([f0, f1], [e0, e1]) = (
                u16::try_from(first)?.to_le_bytes(),
                u16::try_from(end)?.to_le_bytes(),
            );
            entries.push([f0, f1, e0, e1, mode, 0, 0, 0]);
            message_report.push(serde_json::json!(
                { "index" : index, "message_va" : address, "variant" : variant,
                "start" : first, "end" : end, "mode" : mode }
            ));
        }
        field_messages[index] = match dispatch {
            None => entries[0],
            Some((slot, _)) => {
                let [b0, b1] =
                    u16::try_from(catalogue.messages.len() + alternatives.len())?.to_le_bytes();
                let count = u8::try_from(entries.len())?;
                alternatives.extend(entries);
                [
                    b0,
                    b1,
                    count,
                    0,
                    crate::sfc_field_events::MESSAGE_DISPATCH,
                    slot,
                    0,
                    0,
                ]
            }
        };
    }
    field_messages.extend(alternatives);
    let unused = text_layout.unused("b1");
    if !unused.is_empty() {
        return Err(format!("Unused B1 line joins {unused:?}").into());
    }
    let slot_first = u16::try_from(all_pages.len())?;
    all_pages.extend(menu.slot_pages.iter().cloned());
    u16::try_from(all_pages.len()).map_err(|_| "Dialogue pages exceed a word")?;
    page_delays.resize(all_pages.len(), 0);
    page_audio.resize(all_pages.len(), [0, 0]);
    let mut interior = BTreeMap::new();
    for y in 0..224 {
        for x in 0..256 {
            if crate::sfc_b1_frame::window_interior(x, y) {
                let color = frame.background(x, y);
                let next = FIRST_BRICK + interior.len() as u8;
                interior.entry(color).or_insert(next);
            }
        }
    }
    let fifth = usize::from(crate::sfc_b1_text::ink_index(5));
    let sixth = usize::from(crate::sfc_b1_text::ink_index(6));
    if interior.len() > fifth - usize::from(FIRST_BRICK) || source_colors.len() < TEXT_COLORS {
        return Err("B1 dialogue interior exceeds one BG2 palette".into());
    }
    let mut text_colors = [[0u8; 3]; 16];
    text_colors[1..5].copy_from_slice(&source_colors[..4]);
    text_colors[fifth] = source_colors[4];
    text_colors[sixth] = source_colors[5];
    text_colors[usize::from(TEXT_SHADOW)] = [24, 8, 0];
    for (color, index) in &interior {
        let at = usize::from(*color) * 2;
        let value = u16::from_le_bytes([palette[at], palette[at + 1]]);
        let dim = |shift: u16| ((value >> shift) & 31) * 2 / 5;
        text_colors[usize::from(*index)] = [
            (dim(0) << 3) as u8,
            (dim(5) << 3) as u8,
            (dim(10) << 3) as u8,
        ];
    }
    for (i, [r, g, b]) in text_colors.iter().enumerate().skip(1) {
        let value = u16::from(r >> 3) | u16::from(g >> 3) << 5 | u16::from(b >> 3) << 10;
        palette[(TEXT_PALETTE * 16 + i) * 2..(TEXT_PALETTE * 16 + i + 1) * 2]
            .copy_from_slice(&value.to_le_bytes());
    }
    let interior: BTreeMap<u8, u8> = interior
        .into_iter()
        .map(|(color, index)| (color, if color == 16 { 0 } else { index }))
        .collect();
    let mut battle_messages = crate::sfc_battle_message::load(content)?;
    let ending_breaks = crate::sfc_ending::wrap(&mut battle_messages, font)?;
    let mut text =
        crate::sfc_b1_text::Assets::prepare(&all_pages, &battle_messages.text(), font, |page| {
            dialogue_page(page, font, &frame, &interior)
        })?;
    let magic_first = usize::from(menu_first_page) + 5;
    text.cache_pages(&all_pages, magic_first..magic_first + 8)?;
    let message_fit = battle_messages.fit(&text)?;
    let battle_messages = battle_messages.compile(&text, &sound)?;
    let battle_display = crate::sfc_battle_display::load(content)?;
    let ending = crate::sfc_ending::prepare(content, source_colors, ending_breaks)?;
    let body_scripts = content.read("battle-scripts.bin")?;
    let scripts: serde_json::Value = content.json("battle-scripts.json")?;
    if scripts["image_sha256"] != sha(&body_scripts) {
        return Err("Battle script image differs from its manifest".into());
    }
    let mut body_scripts = body_scripts;
    let mut commands: Vec<u32> = scripts["command_addresses"]
        .as_array()
        .ok_or("Battle script commands")?
        .iter()
        .map(|v| v.as_u64().unwrap_or(0) as u32)
        .collect();
    let ending_thinning = crate::sfc_ending::thin(&mut body_scripts, &mut commands)?;
    let reached = crate::sfc_battle_objects::reached(
        &crate::sfc_battle_objects::thinned(&body_scripts),
        &scripts,
    )?;
    let body_scripts = crate::sfc_battle_objects::specialized(&body_scripts, &commands)?;
    let image_unread = crate::sfc_battle_objects::unread(
        &body_scripts,
        &content.json("battle-scripts-kept.json")?,
        &reached,
    )?;
    let mut beyond_kept: Vec<[u32; 2]> = Vec::new();
    {
        let kept: serde_json::Value = content.json("battle-scripts-kept.json")?;
        let mut source_kept = vec![false; reached.len()];
        for run in kept["kept"].as_array().ok_or("Kept script runs")? {
            let (a, b) = (
                run[0].as_u64().unwrap_or(0) as usize,
                run[1].as_u64().unwrap_or(0) as usize,
            );
            source_kept[a..b.min(reached.len())].fill(true);
        }
        for at in (0..reached.len()).filter(|&a| reached[a] && !source_kept[a]) {
            let va = crate::sfc_battle_objects::SOURCE_BASE + at as u32;
            match beyond_kept.last_mut() {
                Some([_, end]) if *end == va => *end = va + 1,
                _ => beyond_kept.push([va, va + 1]),
            }
        }
    }
    let mut image_reached: Vec<(usize, usize)> = Vec::new();
    for (at, _) in reached.iter().enumerate().filter(|(_, r)| **r) {
        let file = crate::sfc_battle_objects::IMAGE_OFFSET + at;
        match image_reached.last_mut() {
            Some((_, end)) if *end == file => *end = file + 1,
            _ => image_reached.push((file, file + 1)),
        }
    }
    let mut body_roots = [0u32; crate::sfc_battle_scripts::DESCRIPTORS];
    for (id, root) in body_roots.iter_mut().enumerate().skip(1) {
        *root = scripts["roots"][id.to_string()]
            .as_u64()
            .ok_or("Battle body root")? as u32;
    }
    let [column, row, columns, rows_count] = TEXT_CELLS;
    let mut text_map = TEXT_NAME.to_le_bytes().repeat(1024);
    for r in 0..rows_count {
        for c in 0..columns {
            let name = if r == 8 {
                (crate::sfc_b1_text::ROW8_HOME - BG2_TILES) / 32 + c as u16
            } else {
                (1 + r * columns + c) as u16 + TEXT_NAME
            };
            let entry = name | (TEXT_PALETTE as u16) << 10;
            let at = ((row + r) * 32 + column + c) * 2;
            text_map[at..at + 2].copy_from_slice(&entry.to_le_bytes());
        }
    }
    let report = serde_json::json!(
        { "scope" :
        "B1 dungeon, events, portrait, direction stone and source experience frame",
        "messages" : message_report, "field_program" : floors.iter().map(| f |
        serde_json::json!({ "floor" : f.id, "bytes" : f.program.len(), "events" : f
        .events.len(), "sha256" : sha(& f.program) })).collect::< Vec < _ >> (),
        "dialogue" : dialogue.report, "page_delays" : page_delays, "page_audio" :
        page_audio, "source_audio" : audio_pending, "location_ui" : { "always_visible" :
        false, "map_menu_connected" : true }, "battle_display" : battle_display.report(),
        "script_reach" : { "reached_bytes" : reached.iter().filter(|&& r | r).count(),
        "beyond_kept" : beyond_kept }, "battle_thinning" : crate
        ::sfc_battle_objects::thinning_report(), "frame" : { "experience_steps" : 57,
        "overlay_tiles" : frame.overlay_tiles(), "overlay_units" : frame.units_report(),
        "vram_start" : crate ::sfc_b1_frame::XP_BASE, "display_step_wram" : crate
        ::sfc_battle::STATS + 0x24, "total_experience_connected" : true, "static_tiles" :
        screen.frame_tiles.len() / 64, "first_static_tile" : STATIC_TILE,
        "portrait_tiles" : screen.portrait_tiles.len() / 64, "first_portrait_tile" :
        screen.portrait_first }, "dialogue" : { "font" : font.report, "origin" :
        TEXT_ORIGIN, "width" : TEXT_WIDTH, "lines" : TEXT_LINES, "bg2_cells" :
        TEXT_CELLS, "palette" : TEXT_PALETTE, "interior_colors" : interior.len(), "pages"
        : all_pages.len(), "page_text" : all_pages, "drawn" : text.report,
        "battle_message_fit" : message_fit }, "stone" : { "cells" : crate
        ::sfc_b1_stone::CELLS, "vram_start" : crate ::sfc_b1_frame::STONE_BASE,
        "color_indices" : crate ::sfc_b1_stone::COLORS, "resting_frames" : 32,
        "palette_count" : 4, "mp_state_connected" : true, "turn_transition_connected" :
        true, "composited_over_frame" : true }, "title" : title.report, "menu" : menu
        .report, "map" : map.report, "smoke" : smoke.report, "shards" : shards.report,
        "sound" : sound.report, "menu_first_page" : menu_first_page, "portrait" : {
        "definition_sha256" : sha(& portrait_raw), "rectangle" : PORTRAIT, "palette_base"
        : 80, "idle" : portrait.report }, "object_definition_sha256" : floor_pictures
        .values().map(| f | f.definition_sha256.clone()).next(), "b2_draw_lists_verified"
        : 256, "view_keys" : view_keys, "picture_keys" : picture_keys, "pictures" : {
        "views" : picture_jobs.iter().map(| j | serde_json::json!({ "state" : j.state,
        "pictures" : j.entries, "key" : j.key })).collect::< Vec < _ >> () },
        "b2_moves_verified" : 1024, "b2_view_record_start" : 256, "definition_sha256" :
        sha(& definition_bytes), "native_draw_lists_verified" : 256, "unique_views" :
        views.len(), "unique_rows" : rows.len(), "view_tiles" : view_tiles.report,
        "stream_budget" : STREAM_BUDGET, "descriptor_cost" : DESCRIPTOR_COST,
        "source_viewport" : [224, 240], "target_viewport" : DUNGEON, "states" : receipts,
        "limits" :
        ["The events of maps 1 to 16 run from their floor programs (build lists what stays unconnected); the collision face and screen shake are drawn; the collision's view zoom is not.",
        "Movement streams the dungeon rows over several vertical blanks without blanking the screen; intermediate frames mix the old and new views."]
        }
    );
    let mut report = report;
    report["flames"] = flames.report.clone();
    report["ending"] = ending.report.clone();
    report["ending"]["thinning"] = ending_thinning;
    report["guardian_display"] = guardian.report.clone();
    Ok(Assets {
        frame,
        stone,
        equipment,
        portrait,
        title,
        screen,
        text_map,
        palette,
        portrait_palettes,
        view_tiles,
        views,
        backdrop_view,
        records,
        floor_cells,
        ray_bytes: crate::sfc_floor_walls::rays(&rays)?,
        view_tables,
        slot_zero,
        text,
        field_messages: field_messages.concat(),
        field_tables,
        picture_table,
        picture_songs,
        recolours,
        page_delays,
        page_audio,
        page_pairs,
        impact_page: impact_page.ok_or("Missing Rulue collision page")?,
        quake_root,
        quake,
        hum,
        warp_hums,
        warp_quakes,
        smoke,
        shards,
        flames,
        guardian,
        shard_root,
        encounters,
        party_stats: crate::sfc_battle::initial_stats(content)?,
        enemy_records: crate::sfc_battle::enemy_records(content)?,
        battle_scripts: crate::sfc_battle_scripts::load(content)?,
        battle_tables: crate::sfc_battle_act::load(content)?,
        battle_vm: crate::sfc_battle_vm::load(content)?,
        battle_messages,
        battle_display,
        ending,
        shops: crate::sfc_shop::load(content)?,
        body_scripts,
        image_unread,
        image_reached,
        body_roots,
        menu,
        menu_first_page,
        slot_first,
        map,
        map_first_view,
        sound,
        report,
    })
}
