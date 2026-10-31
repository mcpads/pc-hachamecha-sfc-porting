use super::*;
impl Assets {
    pub fn load(
        content: &crate::content::Content,
        layout: &PageLayout,
        hover_layout: &PageLayout,
        sound: &crate::sfc_sound::Assets,
    ) -> Result<Self> {
        let root = content.sub("menu");
        let (raw, def) = definition(content)?;
        let items_sheet = sheet(&root, &def.sheets, "ITEMS.CNS")?;
        let hud = content.sub("hud");
        let frame_sheet = sheet(
            &hud,
            &hud.json::<HudSheets>("hud.json")?.resources,
            "FRAME.CNS",
        )?;
        let cut =
            |(pixels, rgb, width): &(Vec<u8>, Vec<[u8; 3]>, usize), left: usize, top: usize| {
                (0..ICON * ICON)
                    .map(|i| {
                        let (x, y) = (i % ICON * 32 / ICON, i / ICON * 32 / ICON);
                        rgb15(rgb[usize::from(pixels[(top + y) * width + left + x])])
                    })
                    .collect::<Vec<u16>>()
            };
        let icon = |cell: u16| {
            cut(
                &items_sheet,
                usize::from(cell) % 20 * 32,
                usize::from(cell) / 20 * 32,
            )
        };
        let drawing = &def.item_list_drawing;
        if drawing.quantity
            != (Quantity {
                minimum: SHOWN_QUANTITY.0,
                maximum: SHOWN_QUANTITY.1,
            })
        {
            return Err("Unexpected item quantity rule".into());
        }
        let sheet_of = |slot: u16, file: &str| {
            drawing
                .sheets
                .iter()
                .find(|s| s.slot == slot && s.file == file)
                .ok_or("Missing item icon sheet")
        };
        sheet_of(16, "ITEMS.CNS")?;
        let framed = &sheet_of(2, "FRAME.CNS")?.used_rectangles;
        let item_icon = |id: u16| -> Result<Vec<u16>> {
            let entry = drawing
                .icons
                .iter()
                .find(|i| u16::from(i.item_id) == id)
                .ok_or("Missing item icon")?;
            match entry.slot {
                16 => Ok(icon(entry.frame)),
                2 => {
                    let [l, t, r, b] = framed
                        .iter()
                        .find(|u| u.frame == entry.frame)
                        .ok_or("Missing item icon rectangle")?
                        .rectangle;
                    if r != l + 32
                        || b != t + 32
                        || r > frame_sheet.2
                        || b * frame_sheet.2 > frame_sheet.0.len()
                    {
                        return Err("Invalid item icon rectangle".into());
                    }
                    Ok(cut(&frame_sheet, l, t))
                }
                _ => Err("Unexpected item icon slot".into()),
            }
        };
        let strip_rows = |name: &str| -> Result<Vec<(u16, u16)>> {
            Ok(def
                .strips
                .get(name)
                .ok_or("Missing menu strip")?
                .rows
                .iter()
                .map(|r| (r.code, r.icon))
                .collect())
        };
        let (main, magic, settings, battle) = (
            strip_rows("main")?,
            strip_rows("magic")?,
            strip_rows("settings")?,
            strip_rows("battle_main")?,
        );
        if strip_rows("shop_main")?
            != SHOP_CODES
                .into_iter()
                .map(u16::from)
                .zip(SHOP_CELLS)
                .collect::<Vec<_>>()
        {
            return Err("Unexpected shop bar rows".into());
        }
        if battle
            != [1u16, 2, ESCAPE.into(), 4, STATUS.into()]
                .into_iter()
                .zip(BATTLE_CELLS)
                .collect::<Vec<_>>()
        {
            return Err("Unexpected battle bar rows".into());
        }
        if main != (1..=5).map(|c| (c, c - 1)).collect::<Vec<_>>()
            || magic != (32..40).map(|c| (c, c)).collect::<Vec<_>>()
            || settings.iter().map(|r| (r.0, r.1)).collect::<Vec<_>>()
                != [(12, 9), (13, 8), (14, 10), (16, 11), (18, 26), (20, 12)]
        {
            return Err("Unexpected menu strip rows".into());
        }
        let magic_cells: Vec<u16> = (32..40)
            .chain([UNLEARNED])
            .chain((35..40).map(|c| c - 15))
            .collect();
        let strip_cells: [Vec<u16>; 7] = [
            (0..5).collect(),
            magic_cells,
            HELD_ITEMS.iter().map(|&i| u16::from(i)).collect(),
            SETTINGS_ICONS
                .iter()
                .flat_map(|&(a, b, _)| [a, b])
                .collect(),
            SLOT_ICONS.to_vec(),
            BATTLE_CELLS.to_vec(),
            SHOP_CELLS.to_vec(),
        ];
        let mut strips = Vec::new();
        let mut merged = Vec::new();
        for (strip, cells) in strip_cells.iter().enumerate() {
            let mut cells = cells.clone();
            cells.sort();
            cells.dedup();
            let mut images: Vec<Vec<u16>> = if strip == usize::from(ITEMS) {
                cells.iter().map(|&c| item_icon(c)).collect::<Result<_>>()?
            } else {
                cells.iter().map(|&c| icon(c)).collect()
            };
            let (slot_palettes, palettes, merges) = pack_merging(&mut images)?;
            merged.extend(merges);
            let mut table = BTreeMap::new();
            for ((&cell, image), &palette) in cells.iter().zip(&images).zip(&slot_palettes) {
                let at = ICON_PALETTES
                    .iter()
                    .position(|&p| p == palette)
                    .unwrap_or(0);
                let entries: Vec<u16> = palettes[at * 32..at * 32 + 32]
                    .chunks(2)
                    .map(|b| u16::from_le_bytes([b[0], b[1]]))
                    .collect();
                let pixels = image
                    .iter()
                    .map(|c| {
                        entries[1..]
                            .iter()
                            .position(|e| e == c)
                            .map(|i| i as u8 + 1)
                    })
                    .collect::<Option<Vec<u8>>>()
                    .ok_or("Icon colour missing from its palette")?;
                table.insert(cell, (tiles(&pixels, ICON), palette));
            }
            strips.push(Strip {
                palettes,
                cells: table,
            });
        }
        let (digits, digit_rgb, digit_width) = sheet(&root, &def.sheets, "NUMBER.CNS")?;
        let mut static_pixels = vec![0u8; 8 * 88];
        for d in 0..10 {
            for y in 0..8 {
                for x in 0..8 {
                    static_pixels[y * 88 + d * 8 + x] = match digits[y * digit_width + d * 8 + x] {
                        14 => 0,
                        0 => 2,
                        15 => 3,
                        _ => return Err("Unexpected digit colour".into()),
                    };
                }
            }
        }
        let mut mark_pixels = vec![0u8; 64];
        for y in 0..8 {
            for x in 0..8 {
                mark_pixels[y * 8 + x] = match digits[y * digit_width + 80 + x] {
                    14 => 0,
                    0 => 2,
                    15 => 3,
                    _ => return Err("Unexpected equipped mark colour".into()),
                };
            }
        }
        for y in 0..8 {
            for x in 0..8 {
                if x < 2 || y < 2 {
                    static_pixels[y * 88 + 80 + x] = 1;
                }
            }
        }
        let mut mark_palette = vec![0u8; 32];
        for (i, c) in [
            rgb15([232, 200, 72]),
            rgb15(digit_rgb[0]),
            rgb15(digit_rgb[15]),
        ]
        .into_iter()
        .enumerate()
        {
            mark_palette[(i + 1) * 2..(i + 2) * 2].copy_from_slice(&c.to_le_bytes());
        }
        let mut pages = Vec::new();
        let mut hover = |text: &str| -> Result<()> {
            let laid = hover_layout.pages(text)?;
            if laid.len() != 1 {
                return Err("Menu text exceeds the window below the bar".into());
            }
            pages.push(format!("\n{}", laid[0]));
            Ok(())
        };
        for code in (1..=5).chain(32..40).chain(12..=20) {
            hover(&message_text(
                &def.hover
                    .get(&code.to_string())
                    .ok_or("Missing menu hover text")?
                    .tokens,
            )?)?;
        }
        let items: Items = content.json("items.json")?;
        for id in HELD_ITEMS {
            let item = items
                .items
                .iter()
                .find(|i| i.id == id)
                .ok_or("Missing item")?;
            hover(&message_text(&item.description.tokens)?)?;
        }
        for code in [ESCAPE, 6, 7, 8] {
            hover(&message_text(
                &def.hover
                    .get(&code.to_string())
                    .ok_or("Missing bar hover text")?
                    .tokens,
            )?)?;
        }
        let mut results = BTreeMap::new();
        let played = |key: &str, pages: &mut Vec<String>| -> Result<Vec<Op>> {
            let action = def.actions.get(key).ok_or("Missing menu result")?;
            let mut ops = Vec::new();
            for step in &action.steps {
                match step.kind.as_str() {
                    "page" => {
                        let laid = layout.pages(&message_text(&step.tokens)?)?;
                        if laid.len() != 1 {
                            return Err(format!("Menu result {key} page overflows").into());
                        }
                        let wait = match &step.wait {
                            Value::String(s) if s == "input" => 0,
                            w => u8::try_from(w.as_u64().ok_or("Invalid result wait")?.max(1))?,
                        };
                        ops.push(Op::Page(pages.len(), wait));
                        pages.push(laid[0].clone());
                    }
                    "signal" => ops.push(Op::Set(EAT, EAT_TICKS)),
                    "audio" => {
                        let index = step.extra["index"].as_u64().ok_or("Menu audio index")?;
                        let code = match step.extra["token"].as_str() {
                            Some("sound_effect") => sound.effect_code(index),
                            Some("audio" | "repeated_audio") => sound.voice_code(index),
                            _ => return Err("Unknown menu audio".into()),
                        };
                        ops.extend(code.map(Op::Sound));
                    }
                    kind => return Err(format!("Unknown result step {kind}").into()),
                }
            }
            Ok(ops)
        };
        results.insert(
            "status".into(),
            vec![Op::StatusData, Op::Say(STATUS_MESSAGE)],
        );
        for id in [40u8, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50] {
            results.insert(
                id.to_string(),
                vec![Op::Use(id), Op::Set(EAT, EAT_WAITING), Op::Say(USE_MESSAGE)],
            );
        }
        results.insert("51".into(), played("51", &mut pages)?);
        for id in 88u8..98 {
            let slot = crate::sfc_battle::STATS + if id < 93 { 0x1a } else { 0x1b };
            let off = format!("{id}_off");
            let mut ops = vec![Op::Use(id), Op::Unless(slot, off.clone())];
            ops.extend(played(&id.to_string(), &mut pages)?);
            results.insert(id.to_string(), ops);
            results.insert(off.clone(), played(&off, &mut pages)?);
        }
        use crate::sfc_field_events::{CHOSEN, FOLLOW_UP};
        for id in (32..40).chain(FIELD_ITEMS) {
            let mut ops = if id == 75 || id == 35 {
                vec![Op::Use(id)]
            } else {
                Vec::new()
            };
            ops.extend(played(&id.to_string(), &mut pages)?);
            ops.extend([Op::Set(CHOSEN, id), Op::Set(FOLLOW_UP, id), Op::Close]);
            results.insert(id.to_string(), ops);
        }
        let growth: Growth = content.json("level-growth.json")?;
        if growth.display_steps != 57 {
            return Err("Unexpected level growth source".into());
        }
        let mut apple = vec![Op::Use(53), Op::Set(EAT, EAT_WAITING)];
        apple.extend(played("53", &mut pages)?);
        apple.push(Op::Grow);
        results.insert("53".into(), apple);
        let report = serde_json::json!(
            { "definition_sha256" : sha(& raw), "pages" : pages.len(), "icon_size" :
            ICON, "bar_y" : BAR_Y, "merged_icon_colors" : merged.iter().map(| [a, b] |
            format!("{a:04x}->{b:04x}")).collect::< Vec < _ >> (), "results" : results
            .keys().collect::< Vec < _ >> (), "limits" :
            ["Icons scale 32 to 24 pixels by nearest sampling.",
            "Items show eight a page; left/right past an end turn the page (the source's previous/next buttons)."]
            }
        );
        let files = &def.save_files;
        use crate::b1_dialogue::SOURCE_BREAK as BREAK;
        let mut slot_pages = Vec::new();
        for kind in 0..3 {
            for slot in 1..=crate::sfc_save::SLOTS as u32 {
                let digit = char::from_u32(0xff10 + slot).ok_or("Slot digit")?;
                for map in 0..SAVE_MAPS {
                    let file = if map == 0 {
                        files.empty.clone()
                    } else {
                        files
                            .map_names
                            .get(map)
                            .ok_or("Missing save map name")?
                            .trim_end_matches('　')
                            .to_string()
                    };
                    let page = match kind {
                        0 => {
                            let laid = hover_layout
                                .pages(&format!("{}{digit}{BREAK}{file}", files.hover.concat()))?;
                            if laid.len() != 1 {
                                return Err("Save slot hover overflows".into());
                            }
                            format!("\n{}", laid[0])
                        }
                        _ => {
                            let question = if kind == 1 {
                                &files.save_question
                            } else {
                                &files.load_question
                            };
                            let [head, tail] = question.as_slice() else {
                                return Err("Unexpected save question".into());
                            };
                            layout.choice(&format!("{head}{digit}{tail}{BREAK}{file}"))?
                        }
                    };
                    slot_pages.push(page);
                }
            }
        }
        slot_pages.push(layout.choice(&files.quit_question.concat())?);
        let mut heal_follow = Vec::new();
        for steps in &def
            .actions
            .get("35")
            .ok_or("Missing healing")?
            .follow_variants
        {
            let (mut pages, mut delays) = (Vec::new(), Vec::new());
            for step in steps {
                if step.kind != "page" {
                    return Err("Healing follow-up holds more than pages".into());
                }
                let laid = layout.pages(&message_text(&step.tokens)?)?;
                let delay = match &step.wait {
                    Value::String(s) if s == "input" => 0,
                    w => {
                        let ticks = u8::try_from(w.as_u64().ok_or("Invalid follow-up wait")?)?;
                        if ticks == 0 || ticks >= crate::b1_dialogue::TIMED {
                            return Err("Unsupported healing follow-up wait".into());
                        }
                        crate::b1_dialogue::TIMED | ticks
                    }
                };
                if laid.len() != 1 {
                    return Err("Healing follow-up page overflows".into());
                }
                pages.extend(laid);
                delays.push(delay);
            }
            if pages.is_empty() {
                return Err("Missing healing follow-up".into());
            }
            heal_follow.push((pages, delays));
        }
        if heal_follow.len() != crate::import_menu::HEAL_STEPS as usize + 1 {
            return Err("Healing follow-up variants differ".into());
        }
        Ok(Self {
            strips,
            mark_palette,
            static_tiles: tiles(&static_pixels, 88),
            mark_tile: tiles(&mark_pixels, 8),
            pages,
            slot_pages,
            results,
            heal_follow,
            report,
        })
    }
}
