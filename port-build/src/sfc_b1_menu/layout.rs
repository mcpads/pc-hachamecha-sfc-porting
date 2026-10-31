use super::*;
impl Assets {
    pub fn append(
        &self,
        sections: &mut Vec<Section>,
        space: &mut crate::sfc_rom::Space,
        first_page: u8,
        slot_first: u16,
    ) -> Result<()> {
        use crate::sfc_b1::{add, transfer};
        let page = |offset: usize| -> Result<u8> {
            Ok(first_page
                .checked_add(u8::try_from(offset)?)
                .ok_or("Menu pages exceed one byte")?)
        };
        let static_source = add(
            sections,
            space,
            "menu_static_tiles".into(),
            self.static_tiles.clone(),
        )?;
        if self.mark_palette[8..].iter().any(|&c| c != 0) {
            return Err("Menu palette overlaps reserved battle effect colours".into());
        }
        let mark_source = add(
            sections,
            space,
            "menu_mark_palette".into(),
            self.mark_palette.clone(),
        )?;
        let mark_tile = add(
            sections,
            space,
            "menu_mark_tile".into(),
            self.mark_tile.clone(),
        )?;
        let mut fixed = transfer(mark_source, 176, 8, 1);
        fixed.extend(transfer(
            static_source,
            STATIC_VRAM,
            self.static_tiles.len(),
            0,
        ));
        fixed.extend(transfer(mark_tile, MARK_VRAM, self.mark_tile.len(), 0));
        let fixed_list = add(sections, space, "menu_fixed_list".into(), fixed)?;
        let mut fixed_job = fixed_list.to_le_bytes()[..3].to_vec();
        fixed_job.extend(3u16.to_le_bytes());
        add(sections, space, "menu_fixed_job".into(), fixed_job)?;
        let mut common = Vec::new();
        let mut cells = Vec::new();
        for (s, strip) in self.strips.iter().enumerate() {
            let palettes = add(
                sections,
                space,
                format!("menu_palettes_{s}"),
                strip.palettes.clone(),
            )?;
            common.extend(transfer(palettes + 2, 129, 30, 1));
            common.extend(transfer(palettes + 34, 161, 30, 1));
            let mut table = vec![0u8; 128 * 4];
            for (&cell, (data, palette)) in &strip.cells {
                let source = add(
                    sections,
                    space,
                    format!("menu_icon_{s}_{cell}"),
                    data.clone(),
                )?;
                let at = usize::from(cell) * 4;
                table[at..at + 3].copy_from_slice(&source.to_le_bytes()[..3]);
                table[at + 3] = *palette;
            }
            cells.extend(table);
        }
        add(sections, space, "menu_common".into(), common)?;
        add(sections, space, "menu_cells".into(), cells)?;
        let x0 = |n: usize| 128 - (PITCH * n - 2) / 2;
        let entry = |x: usize, y: usize, name: u16, attributes: u8| -> [u8; 4] {
            [
                x as u8,
                y as u8,
                name as u8,
                0x30 | attributes | (name >> 8) as u8,
            ]
        };
        let (mut icons, mut corners, mut digits) = (Vec::new(), Vec::new(), Vec::new());
        let mut bases = Vec::new();
        for n in 1..=SLOTS {
            bases.extend([
                icons.len() as u16,
                corners.len() as u16,
                digits.len() as u16,
            ]);
            for i in 0..n {
                let x = x0(n) + PITCH * i;
                for t in 0..ICON_TILES {
                    let tile = ICON_NAME + (i * ICON_TILES + t) as u16;
                    icons.extend(entry(x + t % 3 * 8, BAR_Y + t / 3 * 8, tile, 0));
                }
                for (dx, dy, flip) in [
                    (0, 0, 0),
                    (ICON - 6, 0, 0x40),
                    (0, ICON - 6, 0x80),
                    (ICON - 6, ICON - 6, 0xc0),
                ] {
                    corners.extend(entry(
                        x - 2 + dx,
                        BAR_Y - 2 + dy,
                        CORNER_NAME,
                        flip | MARK_PALETTE << 1,
                    ));
                }
                for dx in [11, 17] {
                    digits.extend(entry(x + dx, BAR_Y + 16, DIGIT_NAME, MARK_PALETTE << 1));
                }
            }
        }
        let mut words = Vec::new();
        for value in bases {
            words.extend(value.to_le_bytes());
        }
        add(sections, space, "menu_template_bases".into(), words)?;
        add(sections, space, "menu_icon_templates".into(), icons)?;
        add(sections, space, "menu_corner_templates".into(), corners)?;
        add(sections, space, "menu_digit_templates".into(), digits)?;
        let mut hidden = Vec::new();
        for _ in 0..128 {
            hidden.extend([0, 224, 0, 0]);
        }
        hidden.extend([0; 32]);
        add(sections, space, "menu_hidden_oam".into(), hidden)?;
        let list = add(
            sections,
            space,
            "menu_oam_job_list".into(),
            transfer(u32::from(OAM_BUFFER), 0, OAM_BYTES, 2),
        )?;
        let mut record = list.to_le_bytes()[..3].to_vec();
        record.extend(1u16.to_le_bytes());
        add(sections, space, "menu_oam_job".into(), record)?;
        add(
            sections,
            space,
            "menu_shop_codes".into(),
            SHOP_CODES.to_vec(),
        )?;
        add(
            sections,
            space,
            "menu_spell_costs".into(),
            SPELL_COSTS.to_vec(),
        )?;
        add(
            sections,
            space,
            "menu_settings_icons".into(),
            SETTINGS_ICONS
                .iter()
                .flat_map(|&(a, b, m)| [a as u8, b as u8, m])
                .collect(),
        )?;
        let mut hover = vec![EMPTY_PAGE; 7 * SLOTS];
        for (i, page_at) in [0, 1, 22 + HELD_ITEMS.len(), 3, 4].into_iter().enumerate() {
            hover[usize::from(BATTLE_STRIP) * SLOTS + i] = page(page_at)?;
        }
        let shop_pages = [1, 2, 3].map(|k| 22 + HELD_ITEMS.len() + k);
        for (i, page_at) in [shop_pages[0], shop_pages[1], shop_pages[2], 3, 4]
            .into_iter()
            .enumerate()
        {
            hover[usize::from(SHOP_STRIP) * SLOTS + i] = page(page_at)?;
        }
        for (slot, offset) in (0..5)
            .map(|i| (i, i))
            .chain((0..8).map(|i| (SLOTS + i, 5 + i)))
            .chain(
                [12, 13, 14, 16, 18, 20]
                    .into_iter()
                    .enumerate()
                    .map(|(i, code)| (3 * SLOTS + i, 13 + code - 12)),
            )
        {
            hover[slot] = page(offset)?;
        }
        add(sections, space, "menu_hover".into(), hover)?;
        let slot_bases: Vec<u8> = (0..4)
            .flat_map(|kind| {
                (slot_first + (kind * crate::sfc_save::SLOTS * SAVE_MAPS) as u16).to_le_bytes()
            })
            .collect();
        add(sections, space, "menu_slot_pages".into(), slot_bases)?;
        let mut item_hover = vec![EMPTY_PAGE; ITEM_KINDS];
        for (i, id) in HELD_ITEMS.iter().enumerate() {
            item_hover[usize::from(id - FIRST_ITEM)] = page(22 + i)?;
        }
        add(sections, space, "menu_item_hover".into(), item_hover)?;
        let mut code = Vec::new();
        let mut index = vec![0xffu8; 2 * (1 + 8 + ITEM_KINDS)];
        let keys: Vec<(usize, String)> = std::iter::once((0, "status".to_string()))
            .chain((0..8).map(|s| (1 + s, (32 + s).to_string())))
            .chain(
                HELD_ITEMS
                    .iter()
                    .map(|&id| (9 + usize::from(id - FIRST_ITEM), id.to_string())),
            )
            .collect();
        let mut placed: BTreeMap<String, u16> = BTreeMap::new();
        let targets: Vec<String> = self
            .results
            .values()
            .flatten()
            .filter_map(|op| match op {
                Op::Unless(_, key) => Some(key.clone()),
                _ => None,
            })
            .collect();
        let emit_ops = |code: &mut Vec<u8>,
                        ops: &[Op],
                        placed: &BTreeMap<String, u16>|
         -> Result<()> {
            for op in ops {
                match op {
                    Op::Page(p, wait) => code.extend([PAGE, page(*p)?, *wait]),
                    Op::Set(at, value) => code.extend([SET, *at as u8, (at >> 8) as u8, *value]),
                    Op::Sound(effect) => code.extend([SOUND, *effect]),
                    Op::Close => code.push(CLOSE),
                    Op::Use(id) => code.extend([USE, *id]),
                    Op::Grow => code.push(GROW),
                    Op::StatusData => code.push(STATUS_DATA),
                    Op::Say(va) => code.extend([SAY, *va as u8, (va >> 8) as u8, (va >> 16) as u8]),
                    Op::Unless(at, key) => {
                        let to = placed.get(key).ok_or("Unplaced menu result branch")?;
                        code.extend([
                            BRANCH,
                            *at as u8,
                            (at >> 8) as u8,
                            *to as u8,
                            (to >> 8) as u8,
                        ]);
                    }
                }
            }
            code.push(END);
            Ok(())
        };
        for key in &targets {
            placed.insert(key.clone(), u16::try_from(code.len())?);
            let ops = self.results.get(key).ok_or("Missing menu result")?;
            emit_ops(&mut code, ops, &placed)?;
        }
        for (slot, key) in keys {
            let ops = self.results.get(&key).ok_or("Missing menu result")?;
            index[slot * 2..slot * 2 + 2]
                .copy_from_slice(&u16::try_from(code.len())?.to_le_bytes());
            emit_ops(&mut code, ops, &placed)?;
        }
        add(sections, space, "menu_results".into(), code)?;
        add(sections, space, "menu_result_index".into(), index)?;
        let mut start = vec![0u8; ITEM_KINDS + STATS];
        for (id, n) in START_INVENTORY {
            start[usize::from(id - FIRST_ITEM)] = n;
        }
        add(sections, space, "menu_start_state".into(), start)?;
        Ok(())
    }
}
