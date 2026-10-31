use super::*;
impl Assets {
    pub fn append(
        &self,
        sections: &mut Vec<Section>,
        space: &mut crate::sfc_rom::Space,
    ) -> Result<()> {
        use crate::sfc_b1::{add, add_shared, transfer};
        let job = |sections: &mut Vec<Section>,
                   space: &mut crate::sfc_rom::Space,
                   name: String,
                   list: Vec<u8>|
         -> Result<(u32, u16)> {
            let count = (list.len() / 8) as u16;
            if count == 0 {
                return Ok((0, 0));
            }
            Ok((add(sections, space, name, list)?, count))
        };
        let zero = add_shared(sections, space, "ending_zero".into(), vec![0; CHUNK])?;
        let mut text_jobs = Vec::new();
        for (l, map) in self.text_maps.iter().enumerate() {
            let at = add(sections, space, format!("ending_text_map_{l}"), map.clone())?;
            let mut list = Vec::new();
            for i in 0..TEXT_BYTES.div_ceil(CHUNK) {
                let bytes = CHUNK.min(TEXT_BYTES - i * CHUNK);
                list.extend(transfer(zero, TEXT_TILES + (i * CHUNK) as u16, bytes, 0));
            }
            list.extend(transfer(
                at,
                if l == 0 { A_TEXT_MAP } else { B_TEXT_MAP },
                2048,
                0,
            ));
            text_jobs.push(list);
        }
        let mut records = Vec::new();
        for (i, load) in self.loads.iter().enumerate() {
            let mut list = Vec::new();
            if load.layout != 0 {
                list.extend(&text_jobs[usize::from(load.layout) - 1]);
            }
            for (k, (bytes, address)) in load.uploads.iter().enumerate() {
                let at = add_shared(
                    sections,
                    space,
                    format!("ending_load_{i}_{k}"),
                    bytes.clone(),
                )?;
                list.extend(transfer(at, *address, bytes.len(), 0));
            }
            let (jobs, count) = job(sections, space, format!("ending_load_{i}"), list)?;
            let colours = if load.colours.is_empty() {
                0
            } else {
                add_shared(
                    sections,
                    space,
                    format!("ending_colours_{i}"),
                    load.colours.clone(),
                )?
            };
            let mut r = load.va.to_le_bytes()[..3].to_vec();
            r.extend([load.slot, load.sheet, load.layout]);
            r.extend(&jobs.to_le_bytes()[..3]);
            r.extend(count.to_le_bytes());
            r.extend(&colours.to_le_bytes()[..3]);
            r.extend([load.base, (load.colours.len() / 3) as u8]);
            r.extend([load.blocks.0, load.blocks.1, load.blocks.2, load.alias]);
            r.resize(24, 0);
            records.extend(r);
        }
        records.extend([0xff; 24]);
        add(sections, space, "ending_loads".into(), records)?;
        let mut sheet_table = Vec::new();
        for (s, (kind, frames)) in self.sheets.iter().enumerate() {
            let mut table = Vec::new();
            let count = match frames {
                Frames::None => 0,
                Frames::Sprite(list) => {
                    for (f, pieces) in list.iter().enumerate() {
                        let mut bytes = vec![pieces.len() as u8];
                        for &(dx, dy, tile, attr, large) in pieces {
                            bytes.extend(dx.to_le_bytes());
                            bytes.extend(dy.to_le_bytes());
                            bytes.extend([tile, attr, if large { 0x80 } else { 0 }]);
                        }
                        let at =
                            add_shared(sections, space, format!("ending_pieces_{s}_{f}"), bytes)?;
                        table.extend(&at.to_le_bytes()[..3]);
                        table.push(0);
                    }
                    list.len()
                }
                Frames::Eyes(patches) => {
                    for (f, rows) in patches.iter().enumerate() {
                        let mut list = Vec::new();
                        for (address, entries) in rows {
                            let at = add_shared(
                                sections,
                                space,
                                format!("ending_eyes_{f}_{address:x}"),
                                entries.clone(),
                            )?;
                            list.extend(transfer(at, *address, entries.len(), 0));
                        }
                        let count = (list.len() / 8) as u16;
                        let list = add(sections, space, format!("ending_eyes_{f}"), list)?;
                        let mut record = list.to_le_bytes()[..3].to_vec();
                        record.extend(count.to_le_bytes());
                        let record = add(sections, space, format!("ending_eyes_job_{f}"), record)?;
                        table.extend(&record.to_le_bytes()[..3]);
                        table.push(0);
                    }
                    patches.len()
                }
            };
            let at = if table.is_empty() {
                0
            } else {
                add(sections, space, format!("ending_frames_{s}"), table)?
            };
            sheet_table.extend([*kind as u8, count as u8]);
            sheet_table.extend(&at.to_le_bytes()[..3]);
            sheet_table.push(0);
        }
        add(sections, space, "ending_sheets".into(), sheet_table)?;
        let cgram = add(
            sections,
            space,
            "ending_cgram_list".into(),
            transfer(CGRAM, 0, 512, 1),
        )?;
        let mut record = cgram.to_le_bytes()[..3].to_vec();
        record.extend(1u16.to_le_bytes());
        add(sections, space, "ending_cgram_job".into(), record)?;
        for (l, size) in [(0, 32usize), (1, 16)] {
            let mut list = Vec::new();
            for r in 0..6 {
                list.extend(transfer(
                    TEXT_BUFFER + (r * 32 * size) as u32,
                    TEXT_TILES + (r * 32 * size) as u16,
                    32 * size,
                    0,
                ));
            }
            let at = add(sections, space, format!("ending_text_list_{l}"), list)?;
            let mut record = at.to_le_bytes()[..3].to_vec();
            record.extend(6u16.to_le_bytes());
            add(sections, space, format!("ending_text_job_{l}"), record)?;
        }
        let list = add(
            sections,
            space,
            "ending_entry_list".into(),
            text_jobs[0].clone(),
        )?;
        let mut record = list.to_le_bytes()[..3].to_vec();
        record.extend(((text_jobs[0].len() / 8) as u16).to_le_bytes());
        add(sections, space, "ending_entry_job".into(), record)?;
        let mut colours = Vec::new();
        for [r, g, b] in self.text_colours {
            let v = u16::from(r >> 3) | u16::from(g >> 3) << 5 | u16::from(b >> 3) << 10;
            colours.extend(v.to_le_bytes());
        }
        colours.extend([0, 0]);
        add(sections, space, "ending_text_colours".into(), colours)?;
        Ok(())
    }
}
