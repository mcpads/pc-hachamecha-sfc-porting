use super::*;
pub(crate) fn add(
    sections: &mut Vec<Section>,
    space: &mut crate::sfc_rom::Space,
    name: String,
    bytes: Vec<u8>,
) -> Result<u32> {
    let offset = space
        .reserve(bytes.len())
        .map_err(|e| format!("{e}: {name} ({} bytes)", bytes.len()))?;
    let source = crate::sfc_rom::cpu(offset)?;
    sections.push(Section {
        name,
        offset,
        bytes,
    });
    Ok(source)
}
pub(crate) fn add_shared(
    sections: &mut Vec<Section>,
    space: &mut crate::sfc_rom::Space,
    name: String,
    bytes: Vec<u8>,
) -> Result<u32> {
    if let Some(source) = space.shared(&bytes) {
        return Ok(source);
    }
    let source = add(sections, space, name, bytes.clone())?;
    space.share(bytes, source);
    Ok(source)
}
pub(crate) fn transfer(source: u32, address: u16, bytes: usize, kind: u8) -> Vec<u8> {
    let mut result = vec![kind];
    result.extend((if kind == 0 { address / 2 } else { address }).to_le_bytes());
    result.extend((bytes as u16).to_le_bytes());
    result.extend(&source.to_le_bytes()[..3]);
    result
}
fn job(source: u32, count: usize) -> Vec<u8> {
    let mut bytes = source.to_le_bytes()[..3].to_vec();
    bytes.extend((count as u16).to_le_bytes());
    bytes.resize(16, 0);
    bytes
}
fn view_list_code(at: u32, template: u32, bytes: usize) -> Result<Vec<u8>> {
    use crate::sfc::{emit, implied};
    use w65c816::{AddressingMode as A, CpuMode, Mnemonic as M, Operand as O};
    const SOURCES: u8 = 0xf8;
    let mut program = w65c816::Assembler::new();
    let p = &mut program;
    let m16 = |p: &mut w65c816::Assembler| emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    let m8 = |p: &mut w65c816::Assembler| emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    let dp = |p: &mut w65c816::Assembler, m, at: u8| emit(p, m, A::DirectPage, O::Byte(at));
    let ind_y = |p: &mut w65c816::Assembler, m, at: u8| {
        emit(p, m, A::DirectPageIndirectLongIndexedY, O::Byte(at));
    };
    p.label("b1_view_list").assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    emit(p, M::Ldy, A::Immediate, O::Word(0));
    ind_y(p, M::Lda, 0xe0);
    dp(p, M::Sta, SOURCES);
    emit(p, M::Ldy, A::Immediate, O::Word(2));
    m8(p);
    ind_y(p, M::Lda, 0xe0);
    dp(p, M::Sta, SOURCES + 2);
    m16(p);
    emit(p, M::Ldx, A::Immediate, O::Word((bytes - 2) as u16));
    p.label("b1_view_list_copy");
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(template));
    emit(p, M::Sta, A::AbsoluteLongX, O::Long(VIEW_LIST));
    implied(p, M::Dex);
    implied(p, M::Dex);
    p.emit_label_ref(M::Bpl, A::Relative8, "b1_view_list_copy");
    for k in 0..(bytes / 8) as u16 {
        emit(p, M::Ldy, A::Immediate, O::Word(3 * k));
        ind_y(p, M::Lda, SOURCES);
        emit(
            p,
            M::Sta,
            A::AbsoluteLong,
            O::Long(VIEW_LIST + u32::from(8 * k) + 5),
        );
        emit(p, M::Ldy, A::Immediate, O::Word(3 * k + 1));
        ind_y(p, M::Lda, SOURCES);
        emit(
            p,
            M::Sta,
            A::AbsoluteLong,
            O::Long(VIEW_LIST + u32::from(8 * k) + 6),
        );
    }
    emit(p, M::Lda, A::Immediate, O::Word(VIEW_LIST as u16));
    emit(p, M::Sta, A::AbsoluteLong, O::Long(VIEW_JOB));
    emit(p, M::Lda, A::Immediate, O::Word((bytes / 8) as u16));
    emit(p, M::Sta, A::AbsoluteLong, O::Long(VIEW_JOB + 3));
    emit(p, M::Lda, A::Immediate, O::Word(VIEW_JOB as u16));
    dp(p, M::Sta, 0xe0);
    m8(p);
    emit(p, M::Lda, A::Immediate, O::Byte((VIEW_LIST >> 16) as u8));
    emit(p, M::Sta, A::AbsoluteLong, O::Long(VIEW_JOB + 2));
    dp(p, M::Sta, 0xe2);
    implied(p, M::Rtl);
    let program = crate::sfc_width_check::assemble(
        &program,
        w65c816::CodeLocation::new((at >> 16) as u8, at as u16),
        CpuMode::NATIVE_M8_X16,
    )?;
    let mut bytes = program.bytes().to_vec();
    if bytes.len() > VIEW_LIST_CODE {
        return Err("View list code exceeds its section".into());
    }
    bytes.resize(VIEW_LIST_CODE, 0);
    Ok(bytes)
}
fn view_record(source: u32, count: usize) -> Vec<u8> {
    let mut bytes = source.to_le_bytes()[..3].to_vec();
    bytes.push(count as u8);
    bytes.resize(usize::from(crate::sfc_floor_walls::VIEW_RECORD), 0);
    bytes
}
fn resident(
    sections: &mut Vec<Section>,
    space: &mut crate::sfc_rom::Space,
    name: &str,
    data: &[u8],
    address: u16,
) -> Result<Vec<u8>> {
    let mut list = Vec::new();
    for (i, chunk) in data.chunks(0x4000).enumerate() {
        let source = add(sections, space, format!("{name}_{i}"), chunk.to_vec())?;
        list.extend(transfer(
            source,
            address + (i * 0x4000) as u16,
            chunk.len(),
            0,
        ));
    }
    Ok(list)
}
impl Assets {
    pub fn append(
        &self,
        sections: &mut Vec<Section>,
        space: &mut crate::sfc_rom::Space,
    ) -> Result<()> {
        self.frame.append(sections, space)?;
        self.stone.append(sections, space)?;
        self.equipment
            .append(sections, space, (self.screen.equipment_first * 64) as u16)?;
        let unit_starts = self.portrait.append(sections, space)?;
        self.smoke.append(sections, space)?;
        self.shards.append(sections, space)?;
        self.flames.append(sections, space)?;
        self.guardian.append(sections, space)?;
        self.title.append(sections, space)?;
        crate::sfc_far_code::append(sections, space)?;
        crate::sfc_b1_text_draw::append(sections, space)?;
        self.menu
            .append(sections, space, self.menu_first_page, self.slot_first)?;
        self.sound.append(sections, space)?;
        let mut entry = vec![];
        let zero = add(sections, space, "b1_zero_tile".into(), vec![0; 64])?;
        entry.extend(transfer(zero, 0, 64, 0));
        entry.extend(transfer(zero, TEXT_TILES, 32, 0));
        entry.extend(resident(
            sections,
            space,
            "b1_static",
            &self.screen.frame_tiles,
            (STATIC_TILE * 64) as u16,
        )?);
        let mut pieces: BTreeMap<Vec<u8>, u32> = BTreeMap::new();
        let mut blocks = Vec::new();
        for (k, tiles) in self.screen.portrait_tiles.iter().enumerate() {
            let mut block = Vec::new();
            for (i, chunk) in tiles.chunks(3072).enumerate() {
                let source = match pieces.get(chunk) {
                    Some(&source) => source,
                    None => {
                        let source = add(
                            sections,
                            space,
                            format!("b1_portrait_static_{k}_{i}"),
                            chunk.to_vec(),
                        )?;
                        pieces.insert(chunk.to_vec(), source);
                        source
                    }
                };
                block.extend(transfer(
                    source,
                    (self.screen.portrait_first * 64 + i * 3072) as u16,
                    chunk.len(),
                    0,
                ));
            }
            blocks.push(block);
        }
        entry.extend(&blocks[0]);
        let map = add(
            sections,
            space,
            "b1_static_map".into(),
            self.screen.map.clone(),
        )?;
        let battle_map = add(
            sections,
            space,
            "b1_battle_map".into(),
            self.screen.battle_map.clone(),
        )?;
        entry.extend(transfer(map, 0xf800, 2048, 0));
        let text_map = add(sections, space, "b1_text_map".into(), self.text_map.clone())?;
        entry.extend(transfer(text_map, TEXT_MAP_VRAM, 2048, 0));
        let palette = add(sections, space, "b1_palette".into(), self.palette.clone())?;
        entry.extend(transfer(palette, 0, 512, 1));
        let mut backgrounds = Vec::new();
        let mut restores = Vec::new();
        for (k, block) in blocks.iter().enumerate() {
            let colours = add(
                sections,
                space,
                format!("b1_portrait_palette_{k}"),
                self.portrait_palettes[k].clone(),
            )?;
            let mut list = transfer(colours, 80, 2 * 32, 1);
            list.extend(block);
            let mut restore = transfer(text_map + 64, TEXT_MAP_VRAM + 64, 16 * 64, 0);
            restore.extend(&list);
            restore.extend(&unit_starts[k]);
            restore.extend(transfer(map, 0xf800, 2048, 0));
            for (records, list, name) in [
                (&mut backgrounds, list, "background"),
                (&mut restores, restore, "restore"),
            ] {
                let count = list.len() / 8;
                let at = add(
                    sections,
                    space,
                    format!("b1_portrait_{name}_list_{k}"),
                    list,
                )?;
                records.extend(&at.to_le_bytes()[..3]);
                records.extend((count as u16).to_le_bytes());
            }
        }
        add(
            sections,
            space,
            "b1_portrait_backgrounds".into(),
            backgrounds,
        )?;
        add(sections, space, "b1_portrait_restore".into(), restores)?;
        crate::sfc_battle_display::sections(sections, space, &self.battle_display, battle_map)?;
        self.ending.append(sections, space)?;
        let entry_count = entry.len() / 8;
        let entry = add(sections, space, "b1_entry_transfers".into(), entry)?;
        add(sections, space, "b1_entry".into(), job(entry, entry_count))?;
        let blocks = self.view_tiles.append(sections, space)?;
        self.map
            .append(sections, space, &blocks[self.map_first_view..])?;
        let mut lists = Vec::new();
        let mut palettes: BTreeMap<&[u8], u32> = BTreeMap::new();
        let mut view_template: Option<Vec<u8>> = None;
        for (id, view) in self.views.iter().enumerate() {
            let mut list = blocks[id].clone();
            let mut offset = 0;
            for [a, b] in VIEW_COLORS {
                let range = &view.palette[offset..offset + (b - a) * 2];
                let colors = match palettes.get(range) {
                    Some(&at) => at,
                    None => {
                        let name = format!("b1_view_colors_{}", palettes.len());
                        let at = add(sections, space, name, range.to_vec())?;
                        palettes.insert(range, at);
                        at
                    }
                };
                list.extend(transfer(colors, a as u16, (b - a) * 2, 1));
                offset += (b - a) * 2;
            }
            let mut template = list.clone();
            let mut sources = Vec::new();
            for d in template.chunks_mut(8) {
                sources.extend(&d[5..8]);
                d[5..8].fill(0);
            }
            match &view_template {
                Some(t) if *t != template => {
                    return Err("View lists differ in shape".into());
                }
                Some(_) => {}
                None => view_template = Some(template),
            }
            lists.push((
                add(sections, space, format!("b1_view_{id}"), sources)?,
                list.len() / 8,
            ));
        }
        let view_template = view_template.ok_or("No views")?;
        let template = add(
            sections,
            space,
            "b1_view_template".into(),
            view_template.clone(),
        )?;
        let code = space.reserve_fast(VIEW_LIST_CODE)?;
        let bytes = view_list_code(crate::sfc_rom::cpu(code)?, template, view_template.len())?;
        sections.push(Section {
            name: "b1_view_list_code".into(),
            offset: code,
            bytes,
        });
        let backdrop = add(
            sections,
            space,
            "scene_backdrop_list".into(),
            blocks[self.backdrop_view].clone(),
        )?;
        add(
            sections,
            space,
            "scene_backdrop".into(),
            job(backdrop, blocks[self.backdrop_view].len() / 8),
        )?;
        let records: Vec<u8> = self
            .records
            .iter()
            .flat_map(|id| view_record(lists[*id].0, lists[*id].1))
            .collect();
        let offset = space.reserve_table(records.len())?;
        sections.push(Section {
            name: "b1_views".into(),
            offset,
            bytes: records,
        });
        add(
            sections,
            space,
            "field_messages".into(),
            self.field_messages.clone(),
        )?;
        let offset = space.reserve_table(self.field_tables[0].len())?;
        sections.push(Section {
            name: "field_program".into(),
            offset,
            bytes: self.field_tables[0].clone(),
        });
        for (name, bytes) in ["field_events", "field_floors"]
            .into_iter()
            .zip(&self.field_tables[1..])
        {
            add(sections, space, name.into(), bytes.clone())?;
        }
        add(
            sections,
            space,
            "field_event_lookup".into(),
            crate::sfc_field_events::event_lookup(&self.field_tables[1], &self.field_tables[2])?,
        )?;
        add(
            sections,
            space,
            "b2_pictures".into(),
            self.picture_table.clone(),
        )?;
        add(
            sections,
            space,
            "field_picture_songs".into(),
            self.picture_songs.clone(),
        )?;
        let mut recolours = self.recolours.clone();
        recolours.extend([0, 0]);
        add(sections, space, "field_recolours".into(), recolours)?;
        add(
            sections,
            space,
            "field_songs".into(),
            crate::sfc_field_events::songs(),
        )?;
        add(
            sections,
            space,
            "field_direction_masks".into(),
            crate::floor_events::DIRECTION_MASKS.to_vec(),
        )?;
        add(
            sections,
            space,
            "field_bits".into(),
            (0..8).map(|i| 1 << i).collect(),
        )?;
        add(
            sections,
            space,
            "b1_page_delays".into(),
            self.page_delays.clone(),
        )?;
        add(
            sections,
            space,
            "b1_page_audio".into(),
            self.page_audio.concat(),
        )?;
        add(
            sections,
            space,
            "b1_page_pairs".into(),
            [self.page_pairs.concat(), vec![0, 0]].concat(),
        )?;
        add(
            sections,
            space,
            "b1_impact_page".into(),
            self.impact_page.to_le_bytes().to_vec(),
        )?;
        add(
            sections,
            space,
            "field_quake".into(),
            self.quake.iter().flat_map(|v| v.to_le_bytes()).collect(),
        )?;
        add(
            sections,
            space,
            "field_quake_root".into(),
            self.quake_root.to_le_bytes().to_vec(),
        )?;
        add(
            sections,
            space,
            "field_hum".into(),
            self.hum.iter().flat_map(|v| v.to_le_bytes()).collect(),
        )?;
        let mut hums: Vec<u8> = self
            .warp_hums
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        hums.extend([0xff, 0xff]);
        add(sections, space, "field_warp_hums".into(), hums)?;
        let mut quakes = Vec::new();
        for &(n, base, ticks) in &self.warp_quakes {
            quakes.extend(n.to_le_bytes());
            quakes.extend(base.to_le_bytes());
            quakes.push(ticks);
        }
        quakes.extend([0xff, 0xff]);
        add(sections, space, "field_warp_quakes".into(), quakes)?;
        add(
            sections,
            space,
            "field_shard_root".into(),
            self.shard_root.to_le_bytes().to_vec(),
        )?;
        self.text.append(sections, space)?;
        crate::sfc_floor_walls::sections(
            sections,
            space,
            &self.floor_cells,
            self.ray_bytes.clone(),
            &self.view_tables,
            self.slot_zero,
        )?;
        crate::sfc_encounter::sections(sections, space, &self.encounters)?;
        crate::sfc_battle::sections(
            sections,
            space,
            &self.party_stats,
            &self.enemy_records,
            &self.battle_scripts,
        )?;
        crate::sfc_battle_act::sections(sections, space, &self.battle_tables)?;
        crate::sfc_battle_vm::sections(sections, space, &self.battle_vm)?;
        crate::sfc_battle_message::sections(sections, space, &self.battle_messages)?;
        self.shops.sections(sections, space)?;
        crate::sfc_battle_objects::sections(
            sections,
            space,
            &self.body_scripts,
            &self.image_unread,
            &self.body_roots,
            &self.sound,
        )?;
        self.text.append_cache(sections, space)?;
        Ok(())
    }
}
