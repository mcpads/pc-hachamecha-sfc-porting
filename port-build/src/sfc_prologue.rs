use crate::{Result, sha};
use serde::Serialize;
use sfc_builder::cartridge::{CartridgeLayout, Section};
use std::collections::BTreeMap;
type Outputs = Vec<(String, Vec<u8>)>;
const NMI_BUSY: u16 = 0x22e;
const PRESS_GATHER: u16 = 0x30a;
const GATHER_HELD: u16 = 0x30c;
#[derive(Clone, Serialize)]
struct Upload {
    kind: u8,
    address: u16,
    blob: usize,
}
#[derive(Serialize)]
struct Tick {
    tick: usize,
    uploads: Vec<Upload>,
}
#[derive(Default)]
pub struct Collector {
    blobs: Vec<Vec<u8>>,
    ids: BTreeMap<Vec<u8>, usize>,
    ticks: Vec<Tick>,
    backdrop: Vec<u8>,
    text: Vec<u8>,
    palette: Vec<u8>,
    ring: [Option<i32>; 32],
    was_panorama: bool,
    scroll: i32,
    epoch: i32,
    pub text_addresses: std::collections::BTreeSet<u32>,
    pub audio: Vec<serde_json::Value>,
    pub waits: Vec<(usize, usize)>,
}
const RECORD_BUDGET: usize = 5000;
const DESCRIPTOR_COST: usize = 170;
const CARRIERS: usize = 8;
const UNPACK_SECTION: &str = "prologue_unpack_code";
const UNPACK_BYTES: usize = 0x200;
const OBJ_TILES: std::ops::Range<usize> = 0xc000..0xc000 + crate::sfc_sprites::OBJ_BYTES;
pub const TEXT_MAP_WORD: u16 = 0x4c00;
fn shown_tiles(oam: &[u8]) -> std::collections::BTreeSet<usize> {
    let mut tiles = std::collections::BTreeSet::new();
    for e in 0..128 {
        let y = oam[4 * e + 1];
        if (224..240).contains(&y) {
            continue;
        }
        let t = usize::from(oam[4 * e + 2]) | usize::from(oam[4 * e + 3] & 1) << 8;
        let large = oam[512 + e / 4] >> ((e % 4) * 2 + 1) & 1 != 0;
        let parts: &[(usize, usize)] = if large {
            &[(0, 0), (1, 0), (0, 1), (1, 1)]
        } else {
            &[(0, 0)]
        };
        for &(dx, dy) in parts {
            let tile = (t & 0x100) | ((t & 0xf0) + dy * 16) & 0xf0 | ((t + dx) & 0x0f);
            tiles.insert(0xc000 + (tile & 0xff) * 32 + (tile >> 8) * 0x2000);
        }
    }
    tiles
}
impl Collector {
    fn cost(&self, uploads: &[Upload]) -> usize {
        uploads
            .iter()
            .map(|u| self.blobs[u.blob].len() + DESCRIPTOR_COST)
            .sum()
    }
    fn balance(&mut self) -> serde_json::Value {
        let mut vram = vec![0u8; 0x10000];
        let mut cgram = vec![0u8; 512];
        let mut vram_known = vec![false; 0x10000];
        let mut cgram_known = vec![false; 512];
        let mut oam = vec![0u8; 544];
        let mut shown = Vec::new();
        let before: usize = self.ticks.iter().map(|t| self.cost(&t.uploads)).sum();
        for i in 0..self.ticks.len() {
            let uploads = std::mem::take(&mut self.ticks[i].uploads);
            let mut kept = Vec::new();
            for u in &uploads {
                let data = self.blobs[u.blob].clone();
                let (memory, known, start, unit) = match u.kind {
                    0 => (&mut vram, &mut vram_known, usize::from(u.address), 32),
                    1 => (&mut cgram, &mut cgram_known, usize::from(u.address) * 2, 2),
                    _ => {
                        if u.kind == 2 && u.address == 0 && data.len() == 544 {
                            oam.copy_from_slice(&data);
                        }
                        kept.push(u.clone());
                        continue;
                    }
                };
                let mut runs: Vec<(usize, usize)> = Vec::new();
                for off in (0..data.len()).step_by(unit) {
                    let end = (off + unit).min(data.len());
                    if memory[start + off..start + end] != data[off..end]
                        || !known[start + off..start + end].iter().all(|k| *k)
                    {
                        match runs.last_mut() {
                            Some(run) if off - run.1 <= 2 * unit => run.1 = end,
                            _ => runs.push((off, end)),
                        }
                    }
                }
                memory[start..start + data.len()].copy_from_slice(&data);
                known[start..start + data.len()].fill(true);
                for (a, b) in runs {
                    let address = if u.kind == 0 {
                        start + a
                    } else {
                        (start + a) / 2
                    };
                    kept.push(Upload {
                        kind: u.kind,
                        address: address as u16,
                        blob: self.blob(data[a..b].to_vec()),
                    });
                }
            }
            let (mut runs, others): (Vec<Upload>, Vec<Upload>) =
                kept.into_iter().partition(|u| u.kind == 0);
            runs.sort_by_key(|u| u.address);
            let mut joined: Vec<(usize, usize)> = Vec::new();
            for u in &runs {
                let start = usize::from(u.address);
                let end = start + self.blobs[u.blob].len();
                match joined.last_mut() {
                    Some(run)
                        if start <= run.1 + DESCRIPTOR_COST && end.max(run.1) - run.0 <= 0x8000 =>
                    {
                        run.1 = run.1.max(end)
                    }
                    _ => joined.push((start, end)),
                }
            }
            let mut kept = others;
            for (start, end) in joined {
                vram_known[start..end].fill(true);
                kept.push(Upload {
                    kind: 0,
                    address: start as u16,
                    blob: self.blob(vram[start..end].to_vec()),
                });
            }
            if kept.is_empty() {
                let smallest = uploads
                    .iter()
                    .min_by_key(|u| self.blobs[u.blob].len())
                    .cloned();
                kept.extend(smallest);
            }
            self.ticks[i].uploads = kept;
            shown.push(shown_tiles(&oam));
        }
        let mut moved = 0;
        let mut moved_bytes = 0;
        for i in 1..self.ticks.len() {
            if self.cost(&self.ticks[i].uploads) <= RECORD_BUDGET {
                continue;
            }
            let mut free_from: BTreeMap<usize, usize> = BTreeMap::new();
            for u in &self.ticks[i].uploads {
                let start = usize::from(u.address);
                if u.kind != 0 || !OBJ_TILES.contains(&start) || start % 32 != 0 {
                    continue;
                }
                for tile in (start..start + self.blobs[u.blob].len()).step_by(32) {
                    let mut from = i;
                    for j in (0..i).rev() {
                        let written = self.ticks[j].uploads.iter().any(|w| {
                            w.kind == 0 && {
                                let ws = usize::from(w.address);
                                ws < tile + 32 && tile < ws + self.blobs[w.blob].len()
                            }
                        });
                        if shown[j].contains(&tile) || written {
                            break;
                        }
                        from = j;
                    }
                    if from < i {
                        free_from.insert(tile, from);
                    }
                }
            }
            let mut runs: Vec<(usize, usize, usize)> = Vec::new();
            for (&tile, &from) in &free_from {
                match runs.last_mut() {
                    Some(run) if run.1 == tile && run.1 - run.0 < 1024 => {
                        run.1 += 32;
                        run.2 = run.2.max(from);
                    }
                    _ => runs.push((tile, tile + 32, from)),
                }
            }
            runs.sort_by_key(|r| std::cmp::Reverse(r.1 - r.0));
            for (start, end, from) in runs {
                if self.cost(&self.ticks[i].uploads) <= RECORD_BUDGET {
                    break;
                }
                let Some(j) = (from..i).rev().find(|&j| {
                    self.cost(&self.ticks[j].uploads) + (end - start) + DESCRIPTOR_COST
                        <= RECORD_BUDGET
                }) else {
                    continue;
                };
                let Some(k) = self.ticks[i].uploads.iter().position(|u| {
                    let us = usize::from(u.address);
                    u.kind == 0 && us <= start && end <= us + self.blobs[u.blob].len()
                }) else {
                    continue;
                };
                let u = self.ticks[i].uploads.remove(k);
                let data = self.blobs[u.blob].clone();
                let us = usize::from(u.address);
                for (a, b) in [(us, start), (end, us + data.len())] {
                    if a < b {
                        let blob = self.blob(data[a - us..b - us].to_vec());
                        self.ticks[i].uploads.push(Upload {
                            kind: 0,
                            address: a as u16,
                            blob,
                        });
                    }
                }
                let blob = self.blob(data[start - us..end - us].to_vec());
                self.ticks[j].uploads.push(Upload {
                    kind: 0,
                    address: start as u16,
                    blob,
                });
                moved += 1;
                moved_bytes += end - start;
            }
        }
        let after: usize = self.ticks.iter().map(|t| self.cost(&t.uploads)).sum();
        let over: Vec<usize> = self
            .ticks
            .iter()
            .filter(|t| self.cost(&t.uploads) > RECORD_BUDGET)
            .map(|t| t.tick)
            .collect();
        serde_json::json!(
            { "cost_before" : before, "cost_after" : after, "obj_runs_moved_earlier" :
            moved, "obj_bytes_moved_earlier" : moved_bytes, "over_budget_ticks" : over }
        )
    }
    fn blob(&mut self, bytes: Vec<u8>) -> usize {
        if let Some(id) = self.ids.get(&bytes) {
            return *id;
        }
        let id = self.blobs.len();
        self.ids.insert(bytes.clone(), id);
        self.blobs.push(bytes);
        id
    }
    fn upload(&mut self, uploads: &mut Vec<Upload>, kind: u8, address: u16, bytes: Vec<u8>) {
        for (index, chunk) in bytes.chunks(0x8000).enumerate() {
            uploads.push(Upload {
                kind,
                address: address + (index * 0x8000) as u16,
                blob: self.blob(chunk.to_vec()),
            });
        }
    }
    pub fn tick(
        &mut self,
        tick: usize,
        backdrop: &[u8],
        frame: &crate::sfc::Frame,
        panorama: Option<(&[u8], i32)>,
    ) -> Result<()> {
        if tick != self.ticks.len() || backdrop.len() != 256 * 224 {
            return Err("Prologue scene ticks are not contiguous".into());
        }
        let mut uploads = Vec::new();
        if let Some((pixels, source_scroll)) = panorama {
            if !self.was_panorama {
                self.ring = [None; 32];
                self.epoch = 0;
                self.scroll = 0;
                let map: Vec<u8> = (0..1024usize)
                    .flat_map(|i| {
                        let tile = if i / 32 < 18 {
                            (i % 32) * 18 + i / 32
                        } else {
                            576
                        };
                        (tile as u16).to_le_bytes()
                    })
                    .collect();
                self.upload(&mut uploads, 0, 0xf800, map);
                self.upload(&mut uploads, 0, 576 * 64, vec![0; 64]);
            }
            let phase = source_scroll.rem_euclid(800) * 304 / 800;
            if phase < self.scroll {
                self.epoch += 304;
            }
            self.scroll = phase;
            let position = self.epoch + phase;
            for column in position / 8..=(position + 240) / 8 {
                let slot = column.rem_euclid(32) as usize;
                if self.ring[slot] == Some(column) {
                    continue;
                }
                self.ring[slot] = Some(column);
                let mut bytes = vec![0; 18 * 64];
                for row in 0..18 {
                    for y in 0..8 {
                        for x in 0..8 {
                            for plane in 0..8 {
                                let color = pixels
                                    [(row * 8 + y) * 304 + column.rem_euclid(38) as usize * 8 + x];
                                bytes[row * 64 + (plane / 2) * 16 + y * 2 + plane % 2] |=
                                    ((color >> plane) & 1) << (7 - x);
                            }
                        }
                    }
                }
                self.upload(&mut uploads, 0, (slot * 18 * 64) as u16, bytes);
            }
            let scroll = ((position - 7).rem_euclid(256) as u16)
                .to_le_bytes()
                .to_vec();
            if !self.was_panorama || self.backdrop != scroll {
                self.upload(&mut uploads, 3, 0, scroll.clone());
                self.backdrop = scroll;
            }
            self.was_panorama = true;
        } else {
            if self.was_panorama {
                self.was_panorama = false;
                self.backdrop.clear();
                self.upload(&mut uploads, 3, 0, vec![0; 2]);
            }
            if self.backdrop != backdrop {
                let mut runs: Vec<(usize, Vec<u8>)> = Vec::new();
                for tile in 0..576 {
                    let start = (tile / 32) * 8 * 256 + (tile % 32) * 8;
                    if !self.backdrop.is_empty()
                        && (0..8).all(|y| {
                            self.backdrop[start + y * 256..start + y * 256 + 8]
                                == backdrop[start + y * 256..start + y * 256 + 8]
                        })
                    {
                        continue;
                    }
                    let mut bytes = vec![0; 64];
                    for y in 0..8 {
                        for x in 0..8 {
                            for plane in 0..8 {
                                bytes[(plane / 2) * 16 + y * 2 + plane % 2] |=
                                    ((backdrop[start + y * 256 + x] >> plane) & 1) << (7 - x);
                            }
                        }
                    }
                    runs.push((tile, bytes));
                }
                for (tile, bytes) in runs {
                    self.upload(&mut uploads, 0, (tile * 64) as u16, bytes);
                }
                if self.backdrop.is_empty() {
                    self.upload(&mut uploads, 0, 576 * 64, vec![0; 64]);
                    let map: Vec<u8> = (0..1024u16)
                        .flat_map(|i| {
                            if i < 576 {
                                i.to_le_bytes()
                            } else {
                                576u16.to_le_bytes()
                            }
                        })
                        .collect();
                    self.upload(&mut uploads, 0, 0xf800, map);
                }
                self.backdrop = backdrop.to_vec();
            }
        }
        let mut text = vec![0; 256 * 32];
        for ty in 0..8 {
            for tx in 0..32 {
                for y in 0..8 {
                    for x in 0..8 {
                        let color = frame.pixels[(144 + ty * 8 + y) * 256 + tx * 8 + x];
                        let color = if color == 16 { 0 } else { color };
                        if color > 6 {
                            return Err("Non-text color in prologue dialogue plane".into());
                        }
                        for plane in 0..4 {
                            text[(ty * 32 + tx) * 32 + (plane / 2) * 16 + y * 2 + plane % 2] |=
                                ((color >> plane) & 1) << (7 - x);
                        }
                    }
                }
            }
        }
        let mut tile = 0;
        while tile < 256 {
            if self.text.get(tile * 32..tile * 32 + 32) == Some(&text[tile * 32..tile * 32 + 32]) {
                tile += 1;
                continue;
            }
            let start = tile;
            tile += 1;
            while tile < 256
                && self.text.get(tile * 32..tile * 32 + 32)
                    != Some(&text[tile * 32..tile * 32 + 32])
            {
                tile += 1;
            }
            self.upload(
                &mut uploads,
                0,
                0xa000 + (start * 32) as u16,
                text[start * 32..tile * 32].to_vec(),
            );
        }
        if text[255 * 32..].iter().any(|b| *b != 0)
            || frame.pixels[208 * 256..].iter().any(|p| *p != 16)
        {
            return Err("Prologue dialogue exceeds four-line fixed tile residency".into());
        }
        self.text = text;
        let mut palette: Vec<_> = frame
            .palette
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|c| {
                (u16::from(c[0] >> 3) | u16::from(c[1] >> 3) << 5 | u16::from(c[2] >> 3) << 10)
                    .to_le_bytes()
            })
            .collect();
        palette[..2].fill(0);
        if palette != self.palette {
            self.upload(&mut uploads, 1, 0, palette.clone());
            self.palette = palette;
        }
        self.ticks.push(Tick { tick, uploads });
        Ok(())
    }
    pub fn finish(
        mut self,
        files: &[(String, Vec<u8>)],
        b1: Option<&crate::sfc_b1::Assets>,
    ) -> Result<Vec<(String, Vec<u8>)>> {
        if self.ticks.is_empty() {
            return Ok(Vec::new());
        }
        let file = |name: &str| {
            files
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, b)| b.as_slice())
                .ok_or_else(|| format!("Missing prologue {name}"))
        };
        let actors: serde_json::Value = serde_json::from_slice(file("actors-build.json")?)?;
        let cases = actors["cases"]
            .as_array()
            .ok_or("Missing prologue actors")?;
        let mut previous_group = None;
        let mut previous_oam = Vec::new();
        let mut atlas: &[u8] = &[];
        let mut sent = std::collections::BTreeSet::new();
        let mut seen = 0;
        for case in cases.iter().filter(|c| c["policy"] == "paced") {
            let tick = case["tick"].as_u64().ok_or("Missing actor tick")? as usize;
            if tick != seen || tick >= self.ticks.len() {
                return Err("Prologue actor tick mismatch".into());
            }
            seen += 1;
            let group = case["group"].as_u64();
            let mut uploads = Vec::new();
            if group != previous_group
                && let Some(group) = group
            {
                atlas = file(&format!("actors-{group}.4bpp"))?;
                if atlas.len() > crate::sfc_sprites::OBJ_BYTES {
                    return Err("Prologue actor atlas exceeds OBJ VRAM".into());
                }
                sent.clear();
            }
            previous_group = group;
            let at = case["oam_offset"].as_u64().ok_or("Missing actor offset")? as usize;
            let oam = file("actors.oam")?
                .get(at..at + 544)
                .ok_or("Truncated actor OAM")?;
            let mut runs: Vec<(usize, usize)> = Vec::new();
            for tile in shown_tiles(oam) {
                if !sent.insert(tile) {
                    continue;
                }
                if tile + 32 > 0xc000 + atlas.len() {
                    return Err("Prologue OAM shows a tile outside its atlas".into());
                }
                match runs.last_mut() {
                    Some(run) if run.1 == tile => run.1 += 32,
                    _ => runs.push((tile, tile + 32)),
                }
            }
            for (start, end) in runs {
                let bytes = atlas[start - 0xc000..end - 0xc000].to_vec();
                self.upload(&mut uploads, 0, start as u16, bytes);
            }
            if oam != previous_oam {
                self.upload(&mut uploads, 2, 0, oam.to_vec());
                previous_oam = oam.to_vec();
            }
            self.ticks[tick].uploads.extend(uploads);
        }
        if seen != self.ticks.len() {
            return Err("Missing prologue actor ticks".into());
        }
        let ticks = self.ticks.len();
        self.ticks.retain(|t| !t.uploads.is_empty());
        let mut carriers = Vec::new();
        for k in 1..self.ticks.len() {
            if self.cost(&self.ticks[k].uploads) > RECORD_BUDGET {
                let (previous, tick) = (self.ticks[k - 1].tick, self.ticks[k].tick);
                carriers.extend((previous + 1..tick).rev().take(CARRIERS).map(|tick| Tick {
                    tick,
                    uploads: Vec::new(),
                }));
            }
        }
        self.ticks.extend(carriers);
        self.ticks.sort_by_key(|t| t.tick);
        let balance = self.balance();
        self.ticks.retain(|t| !t.uploads.is_empty());
        use crate::sfc_rom::{Space, Target};
        let mut output = Vec::new();
        let targets: &[Target] = if b1.is_some() {
            &[Target::ExHiRom, Target::HiRom]
        } else {
            &[Target::ExHiRom]
        };
        for &target in targets {
            let (_, recorded) = self.assemble(b1, ticks, &balance, Space::recording(target))?;
            let plan = recorded.plan()?;
            let (files, planned) = self.assemble(b1, ticks, &balance, plan)?;
            planned.finished()?;
            output.extend(files);
        }
        Ok(output)
    }
    fn assemble(
        &self,
        b1: Option<&crate::sfc_b1::Assets>,
        ticks: usize,
        balance: &serde_json::Value,
        mut space: crate::sfc_rom::Space,
    ) -> Result<(Outputs, crate::sfc_rom::Space)> {
        let mut sections = Vec::new();
        if let Some(b1) = b1 {
            space.release(&b1.image_unread)?;
        }
        let mut stream = Vec::new();
        let mut placed: Vec<Vec<usize>> = Vec::new();
        let mut ends = Vec::new();
        for tick in &self.ticks {
            let mut at = Vec::new();
            for u in &tick.uploads {
                at.push(stream.len());
                stream.extend(&self.blobs[u.blob]);
            }
            placed.push(at);
            ends.push(stream.len());
        }
        if stream.len() >= 1 << 24 {
            return Err("Prologue stream exceeds 16 MiB".into());
        }
        let items = crate::sfc_unpack::items(&stream)?;
        let mut table = Vec::new();
        for (i, item) in items.iter().enumerate() {
            let offset = space.reserve(item.len())?;
            sections.push(Section {
                name: format!("prologue_stream_{i}"),
                offset,
                bytes: item.clone(),
            });
            table.extend(&crate::sfc_rom::cpu(offset)?.to_le_bytes()[..3]);
            table.extend(u16::try_from(item.len())?.to_le_bytes());
        }
        let table_offset = space.reserve(table.len())?;
        let table_bytes = table.len();
        sections.push(Section {
            name: "prologue_stream_items".into(),
            offset: table_offset,
            bytes: table,
        });
        let mut streams = crate::sfc_rom::cpu(table_offset)?.to_le_bytes()[..3].to_vec();
        streams.extend(u16::try_from(table_bytes)?.to_le_bytes());
        streams.extend([0; crate::sfc_unpack::STREAM_ENTRY]);
        let streams_offset = space.reserve(streams.len())?;
        sections.push(Section {
            name: crate::sfc_unpack::STREAMS_SECTION.into(),
            offset: streams_offset,
            bytes: streams,
        });
        let code = space.reserve_fast(UNPACK_BYTES)?;
        let at = crate::sfc_rom::cpu(code)?;
        let mut unpacker = w65c816::Assembler::new();
        crate::sfc_unpack::emit(&mut unpacker, crate::sfc_rom::cpu(streams_offset)?);
        let unpacker = crate::sfc_width_check::assemble(
            &unpacker,
            w65c816::CodeLocation::new((at >> 16) as u8, at as u16),
            w65c816::CpuMode::NATIVE_M8_X16,
        )?;
        if unpacker.bytes().len() > UNPACK_BYTES {
            return Err("Prologue unpacker exceeds its section".into());
        }
        let mut bytes = unpacker.bytes().to_vec();
        bytes.resize(UNPACK_BYTES, 0);
        sections.push(Section {
            name: UNPACK_SECTION.into(),
            offset: code,
            bytes,
        });
        let unpack_entries: Vec<u8> = ["unpack_reset", "unpack_step", "unpack_ready"]
            .iter()
            .flat_map(|l| {
                let at = unpacker.label_location(l).unwrap().to_u32();
                at.to_le_bytes()[..3].to_vec()
            })
            .collect();
        sections.push(Section {
            name: "prologue_unpack_entries".into(),
            offset: space.reserve(9)?,
            bytes: unpack_entries,
        });
        let stream_report = serde_json::json!(
            { "bytes" : stream.len(), "packed" : items.iter().map(Vec::len).sum::< usize
            > (), "items" : items.len() }
        );
        let cpu = crate::sfc_rom::cpu;
        let mut records = Vec::new();
        let mut reports = Vec::new();
        for (index, event) in self.ticks.iter().enumerate() {
            let duration = self.ticks.get(index + 1).map_or(ticks, |t| t.tick) - event.tick;
            let mut descriptors = Vec::new();
            let bytes: usize = event.uploads.iter().map(|u| self.blobs[u.blob].len()).sum();
            for (u, &from) in event.uploads.iter().zip(&placed[index]) {
                let source = 0x7f0000 | (from as u32 & 0xffff);
                descriptors.push(u.kind);
                descriptors.extend_from_slice(
                    &(if u.kind == 0 {
                        u.address / 2
                    } else {
                        u.address
                    })
                    .to_le_bytes(),
                );
                descriptors.extend_from_slice(&(self.blobs[u.blob].len() as u16).to_le_bytes());
                descriptors.extend_from_slice(&source.to_le_bytes()[..3]);
            }
            let offset = space.reserve(descriptors.len())?;
            let source = cpu(offset)?;
            sections.push(Section {
                name: format!("transfers_{index}"),
                offset,
                bytes: descriptors.clone(),
            });
            records.extend_from_slice(&u16::try_from(duration)?.to_le_bytes());
            records.extend_from_slice(&u16::try_from(event.tick)?.to_le_bytes());
            records.extend_from_slice(&source.to_le_bytes()[..3]);
            records.extend_from_slice(&u16::try_from(event.uploads.len())?.to_le_bytes());
            let blank = self.cost(&event.uploads) > RECORD_BUDGET;
            records.push(u8::from(blank));
            let previous = index.checked_sub(1).map(|i| self.ticks[i].tick);
            let mut music = 0u8;
            for audio in &self.audio {
                let tick = audio["tick"].as_u64().ok_or("Prologue audio tick")? as usize;
                if previous.is_some_and(|p| tick <= p) || tick > event.tick {
                    continue;
                }
                match audio["event"]["kind"].as_str() {
                    Some("music") => {
                        let index = audio["event"]["index"].as_u64().ok_or("Music index")?;
                        music = crate::sfc_sound::source_song(index).ok_or("Unported song")? + 1;
                    }
                    Some("stop_music") if music == 0 => music = 0x80,
                    _ => {}
                }
            }
            records.push(music);
            let mut voice = 0u8;
            for audio in &self.audio {
                let tick = audio["tick"].as_u64().ok_or("Prologue audio tick")? as usize;
                if previous.is_some_and(|p| tick <= p) || tick > event.tick {
                    continue;
                }
                if audio["event"]["kind"] == "audio" && audio["event"]["bank"] == "voice.wlk" {
                    voice =
                        u8::try_from(audio["event"]["index"].as_u64().ok_or("Voice index")? + 1)?;
                }
            }
            records.push(voice);
            records.extend_from_slice(&(ends[index] as u32).to_le_bytes()[..3]);
            records.push(0);
            reports.push(serde_json::json!(
                { "tick" : event.tick, "duration" : duration, "transfer_bytes" :
                bytes, "forced_blank" : blank, "uploads" : event.uploads }
            ));
        }
        let mut terminal = [0; 16];
        terminal[2..4].copy_from_slice(&u16::try_from(ticks)?.to_le_bytes());
        records.extend_from_slice(&terminal);
        let offset = space.reserve(records.len())?;
        let script = cpu(offset)?;
        sections.push(Section {
            name: "timeline".into(),
            offset,
            bytes: records.clone(),
        });
        let mut waits = self.waits.clone();
        waits.sort_unstable();
        if waits.is_empty() || waits.windows(2).any(|w| w[1].0 < w[0].1) {
            return Err("Prologue message waits missing or overlapping".into());
        }
        let mut table = u16::try_from(waits.len())?.to_le_bytes().to_vec();
        for &(start, end) in &waits {
            table.extend(u16::try_from(start)?.to_le_bytes());
            table.extend(u16::try_from(end)?.to_le_bytes());
        }
        let blank_inside = waits
            .iter()
            .filter(|&&(start, end)| {
                self.ticks.iter().zip(&reports).any(|(t, r)| {
                    t.tick > start && t.tick <= end && r["forced_blank"].as_bool() == Some(true)
                })
            })
            .count();
        let offset = space.reserve(table.len())?;
        sections.push(Section {
            name: "prologue_waits".into(),
            offset,
            bytes: table,
        });
        let mut combined_sections: Vec<Section> = sections
            .iter()
            .map(|s| Section {
                name: s.name.clone(),
                offset: s.offset,
                bytes: s.bytes.clone(),
            })
            .collect();
        let layout = CartridgeLayout::new_exhirom(crate::sfc_rom::SIZE, 0x7000, sections)?;
        let (rom, code_bytes, _) = runtime(&layout, script, false)?;
        let report = serde_json::json!(
            { "schema" : 1, "scope" :
            "complete source-paced prologue visual timeline; native BG and OBJ; audio pending; B1 pending",
            "source_ticks" : ticks, "canonical_text_addresses" : self.text_addresses,
            "records" : reports, "audio_events" : self.audio, "rom_bytes" : rom.len(),
            "rom_sha256" : sha(& rom), "code_bytes" : code_bytes, "stream" :
            stream_report, "controls" :
            "A (a new press) inside a message wait skips to its end; Start replays after completion",
            "skippable_waits" : waits.len(), "skippable_waits_with_blank_records" :
            blank_inside, "record_balance" : balance, "wram" : { "source_tick" : 0x222,
            "remaining_ticks" : 0x220, "complete" : 0x22a, "transitions" : 0x22b },
            "limitations" : ["Audio events retained but not played.",
            "Large resource transitions use forced blank.",
            "Source-paced timing is compiled; interactive game VM and B1 are separate."]
            }
        );
        let mut output = vec![
            ("prologue-full.sfc".into(), rom),
            (
                "prologue-full-build.json".into(),
                serde_json::to_vec_pretty(&report)?,
            ),
        ];
        if let Some(b1) = b1 {
            b1.append(&mut combined_sections, &mut space)?;
            let copy = |sections: &[Section]| -> Vec<Section> {
                sections
                    .iter()
                    .map(|s| Section {
                        name: s.name.clone(),
                        offset: s.offset,
                        bytes: s.bytes.clone(),
                    })
                    .collect()
            };
            let target = space.target();
            if target == crate::sfc_rom::Target::HiRom {
                output.clear();
                let rom_space = crate::sfc_rom::report(&combined_sections, &space);
                if combined_sections
                    .iter()
                    .any(|s| s.offset + s.bytes.len() > crate::sfc_rom::HIROM_SIZE)
                {
                    let report = serde_json::json!(
                        { "mapping" : "HiRom", "fits" : false, "rom_space" : rom_space }
                    );
                    output.push((
                        "madou-hirom-build.json".into(),
                        serde_json::to_vec_pretty(&report)?,
                    ));
                    return Ok((output, space));
                }
            }
            let draft = target
                .layout(copy(&combined_sections))?
                .with_battery(crate::sfc_save::battery(target)?);
            crate::sfc_battle::place_code(&mut combined_sections, &draft)?;
            crate::sfc_field_guardian::place_code(&mut combined_sections, &draft)?;
            crate::sfc_b1_view::place_code(&mut combined_sections, &draft)?;
            crate::sfc_body_tiles::place_code(&mut combined_sections, &draft)?;
            crate::sfc_battle_display::place_code(&mut combined_sections, &draft)?;
            crate::sfc_b1_portrait::place_code(&mut combined_sections, &draft)?;
            crate::sfc_far_code::place_code(&mut combined_sections, &draft)?;
            crate::sfc_b1_text_draw::place_code(&mut combined_sections, &draft)?;
            for section in &combined_sections {
                if section.name.starts_with("battle_scripts_") {
                    continue;
                }
                let (start, end) = (section.offset, section.offset + section.bytes.len());
                let at = b1.image_reached.partition_point(|&(_, e)| e <= start);
                if b1.image_reached.get(at).is_some_and(|&(s, _)| s < end) {
                    return Err(format!(
                        "Section {} lies on script bytes the body VM reads",
                        section.name
                    )
                    .into());
                }
            }
            let rom_space = crate::sfc_rom::report(&combined_sections, &space);
            let layout = target
                .layout(combined_sections)?
                .with_battery(crate::sfc_save::battery(target)?);
            let (rom, code_bytes, main_labels) = runtime(&layout, script, true)?;
            let report = serde_json::json!(
                { "scope" :
                "the whole game: title, prologue, the initial maze B1-B7, the ancient maze, the phantom zone, the guardian zones, the final battle and the ending, with the Compile SFC sound driver",
                "mapping" : format!("{target:?}"), "main_labels" : main_labels, "save" :
                crate ::sfc_save::report(), "battle_rules" : crate ::sfc_battle::report(&
                layout) ?, "guardian_code_bytes" : crate
                ::sfc_field_guardian::code_bytes(& layout) ?, "far_code_bytes" : crate
                ::sfc_far_code::code_bytes(& layout) ?, "text_draw_bytes" : crate
                ::sfc_b1_text_draw::code_bytes(& layout) ?, "rom_sha256" : sha(& rom),
                "rom_bytes" : rom.len(), "code_bytes" : code_bytes, "b1" : b1.report,
                "rom_space" : rom_space, "sections" : layout.section_report(), "wram" : {
                "b1_mode" : 0x22c, "state_index" : 0x500, "intro_page" : 0x501,
                "movement_effect" : 0x504, "event_id" : 0x505, "x" : 0x508, "y" : 0x509,
                "direction" : 0x50a, "source_globals" : crate
                ::sfc_field_events::GLOBALS, "source_globals_bytes" : 192,
                "question_pending" : 0x511, "compass_index" : 0x513, "jingle_hold" :
                crate ::sfc_b1::JINGLE_HOLD }, "controls" :
                "Title: A/Start skips and chooses. Prologue: a new A press inside a message wait skips to its end. B1: A investigates/advances/accepts, B rejects a choice or closes; Up forward, Left/Right turn, Down turn around; X/Start opens the menu.",
                "limits" : ["Not checked on real hardware.",
                "Source questions still open are listed in build"] }
            );
            if target == crate::sfc_rom::Target::HiRom {
                output.push(("madou-hirom.sfc".into(), rom));
                output.push((
                    "madou-hirom-build.json".into(),
                    serde_json::to_vec_pretty(&report)?,
                ));
                return Ok((output, space));
            }
            output.push(("madou.sfc".into(), rom));
            output.push(("b1-text-pages.bin".into(), b1.text_reference()));
            output.push((
                "madou-build.json".into(),
                serde_json::to_vec_pretty(&report)?,
            ));
        }
        Ok((output, space))
    }
}
fn runtime(
    layout: &CartridgeLayout,
    script: u32,
    b1: bool,
) -> Result<(Vec<u8>, usize, serde_json::Value)> {
    let unpack = {
        let bytes = &layout.section("prologue_unpack_entries")?.bytes;
        let at = |k: usize| u32::from_le_bytes([bytes[k], bytes[k + 1], bytes[k + 2], 0]);
        [at(0), at(3), at(6)]
    };
    use crate::sfc::{emit, implied, write};
    use sfc_builder::runtime as rt;
    use w65c816::{
        AddressingMode as A, Assembler, CodeLocation, CpuMode, Mnemonic as M, Operand as O,
    };
    let mut p = Assembler::new();
    let abs = |p: &mut Assembler, m, at| emit(p, m, A::Absolute, O::Word(at));
    let call = |p: &mut Assembler, label: &str| {
        p.emit_label_ref(M::Jsr, A::Absolute, label);
    };
    let branch = |p: &mut Assembler, m, label: &str| {
        p.emit_label_ref(m, A::Relative8, label);
    };
    let read = |p: &mut Assembler, ptr: u8, index: u16, to: u16| {
        emit(p, M::Ldy, A::Immediate, O::Word(index));
        emit(p, M::Lda, A::DirectPageIndirectLongIndexedY, O::Byte(ptr));
        abs(p, M::Sta, to);
    };
    let pad = rt::PadStorage::new(0x300)?;
    let fast = layout.mapping() == sfc_builder::cartridge::Mapping::HiRom;
    if b1 {
        p.label("far_trampolines");
        crate::sfc_far_code::emit_trampolines(&mut p);
    }
    crate::sfc::startup_entered(&mut p, fast);
    if b1 {
        abs(&mut p, M::Stz, crate::sfc_save::LOADING);
        abs(&mut p, M::Stz, crate::sfc_battle_field::ACTIVE);
        abs(&mut p, M::Stz, crate::sfc_ending::ACTIVE);
        abs(&mut p, M::Stz, crate::sfc_ending::STOP);
        crate::sfc::write_word(&mut p, crate::sfc_encounter::SEED, 1);
        crate::sfc::write_word(&mut p, crate::sfc_encounter::SEED + 2, 0);
        use crate::sfc_battle::{ACTOR_BYTES, ACTORS};
        emit(&mut p, M::Ldx, A::Immediate, O::Word(0));
        p.label("reset_actors");
        emit(&mut p, M::Stz, A::AbsoluteX, O::Word(ACTORS));
        implied(&mut p, M::Inx);
        emit(&mut p, M::Cpx, A::Immediate, O::Word(4 * ACTOR_BYTES));
        p.emit_label_ref(M::Bne, A::Relative8, "reset_actors");
    }
    for (a, v) in [
        (0x2105, 3),
        (0x2107, 0x7c),
        (0x2108, (TEXT_MAP_WORD >> 8) as u8),
        (0x210b, 0x50),
        (0x2101, 3),
        (0x210f, 0),
        (0x210f, 0),
        (0x2110, 0xff),
        (0x2110, 0xff),
        (0x2123, 3),
        (0x2125, 3),
        (0x2126, 7),
        (0x2127, 247),
        (0x212b, 0),
        (0x212e, 0x11),
    ] {
        write(&mut p, a, v);
    }
    call(&mut p, "text_plane_reset");
    for (i, v) in [127, 0x13, 17, 0x13, 80, 0x02, 0].into_iter().enumerate() {
        write(&mut p, 0x250 + i as u16, v);
    }
    for (a, v) in [
        (0x4310, 0),
        (0x4311, 0x2c),
        (0x4312, 0x50),
        (0x4313, 2),
        (0x4314, 0),
        (0x420c, 2),
        (0x212c, 0x13),
    ] {
        write(&mut p, a, v);
    }
    if b1 {
        call(&mut p, "sound_boot");
        for at in [
            crate::sfc_b1_text::SERIAL,
            crate::sfc_b1_text::DONE,
            crate::sfc_b1_text::QUEUED,
            crate::sfc_field_smoke::SMOKE_TIME,
            crate::sfc_field_shards::SHARD_TIME,
            crate::sfc_field_flames::FLAME_TIME,
            crate::sfc_field_flames::FLAME_TIME + 1,
            crate::sfc_field_guardian::GUARD_TIME,
            crate::sfc_field_guardian::GUARD_TIME + 1,
            crate::sfc_field_events::SCREEN_DARK,
            crate::sfc_battle_vm::FADE,
            crate::sfc_b1::JINGLE_HOLD,
            crate::sfc_battle_field::LAG,
            crate::sfc_battle_field::UPDATES,
            crate::sfc_sound::EXCHANGES,
            crate::sfc_unpack::PAGE + u16::from(crate::sfc_unpack::ACTIVE),
        ] {
            write(&mut p, at, 0);
        }
        call(&mut p, "b1_map_markers_reset");
        call(&mut p, "title_enter");
    } else {
        call(&mut p, "restart");
    }
    write(&mut p, 0x201, 0x42);
    if b1 {
        write(&mut p, NMI_BUSY, 0);
        write(&mut p, crate::sfc_battle_objects::IN_UPDATE, 0);
    }
    abs(&mut p, M::Lda, 0x4210);
    write(&mut p, 0x4200, 0x81);
    p.label("idle");
    implied(&mut p, M::Wai);
    if b1 {
        call(&mut p, "b1_text_work");
        call(&mut p, "ending_text_work");
    }
    branch(&mut p, M::Bra, "idle");
    p.label("text_plane_reset")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    write(&mut p, 0x2115, 0x80);
    write(&mut p, 0x2116, 0x00);
    write(&mut p, 0x2117, (TEXT_MAP_WORD >> 8) as u8);
    for (label, entries) in [("text_map_above", 18 * 32u16), ("text_map_below", 6 * 32)] {
        if label == "text_map_below" {
            emit(&mut p, M::Ldx, A::Immediate, O::Word(0));
            p.label("text_map_rows");
            implied(&mut p, M::Txa);
            abs(&mut p, M::Sta, 0x2118);
            abs(&mut p, M::Stz, 0x2119);
            implied(&mut p, M::Inx);
            emit(&mut p, M::Cpx, A::Immediate, O::Word(256));
            branch(&mut p, M::Bne, "text_map_rows");
        }
        emit(&mut p, M::Ldx, A::Immediate, O::Word(entries));
        emit(&mut p, M::Lda, A::Immediate, O::Byte(255));
        p.label(label);
        abs(&mut p, M::Sta, 0x2118);
        abs(&mut p, M::Stz, 0x2119);
        implied(&mut p, M::Dex);
        branch(&mut p, M::Bne, label);
    }
    write(&mut p, 0x2116, 0x00);
    write(&mut p, 0x2117, 0x50);
    emit(&mut p, M::Ldx, A::Immediate, O::Word(4096));
    p.label("clear_text");
    write(&mut p, 0x2118, 0);
    write(&mut p, 0x2119, 0);
    implied(&mut p, M::Dex);
    branch(&mut p, M::Bne, "clear_text");
    implied(&mut p, M::Rts);
    p.label("restart");
    for (i, b) in script.to_le_bytes()[..3].iter().enumerate() {
        write(&mut p, 0xe0 + i as u16, *b);
    }
    for a in 0x220..=0x22c {
        write(&mut p, a, 0);
    }
    emit(&mut p, M::Ldx, A::Immediate, O::Word(0));
    emit(&mut p, M::Jsl, A::AbsoluteLong, O::Long(unpack[0]));
    implied(&mut p, M::Rts);
    p.label("nmi");
    if fast {
        p.assume_mode(CpuMode::NATIVE_M8_X8);
        p.emit_label_ref(M::Jml, A::AbsoluteLong, "nmi_fast");
        p.label("nmi_fast");
    }
    rt::emit_interrupt_entry(&mut p);
    abs(&mut p, M::Lda, 0x4210);
    if b1 {
        abs(&mut p, M::Lda, NMI_BUSY);
        branch(&mut p, M::Beq, "nmi_free");
        call(&mut p, "sound_frame_now");
        call(&mut p, "pad_gather");
        abs(&mut p, M::Inc, crate::sfc_battle_field::LAG);
        p.emit_label_ref(M::Brl, A::Relative16, "nmi_skipped");
        p.label("nmi_free");
        abs(&mut p, M::Inc, NMI_BUSY);
    }
    abs(&mut p, M::Inc, 0x202);
    if b1 {
        abs(&mut p, M::Lda, crate::sfc_ending::ACTIVE);
        branch(&mut p, M::Beq, "not_ending");
        call(&mut p, "ending_nmi");
        p.emit_label_ref(M::Brl, A::Relative16, "nmi_done");
        p.label("not_ending").assume_mode(CpuMode::NATIVE_M8_X16);
        abs(&mut p, M::Lda, 0x22c);
        branch(&mut p, M::Bne, "b1_nmi");
        p.emit_label_ref(M::Jmp, A::Absolute, "not_b1");
        p.label("b1_nmi");
        abs(&mut p, M::Lda, 0x228);
        branch(&mut p, M::Beq, "b1_display_on");
        write(&mut p, 0x2100, 15);
        p.label("b1_display_on");
        abs(&mut p, M::Lda, crate::sfc_battle_vm::FADE);
        branch(&mut p, M::Beq, "b1_display_lit");
        for _ in 0..4 {
            emit(&mut p, M::Lsr, A::Accumulator, O::None);
        }
        emit(&mut p, M::Eor, A::Immediate, O::Byte(15));
        abs(&mut p, M::Sta, 0x2100);
        p.label("b1_display_lit");
        call(&mut p, "b1_shake_apply");
        abs(&mut p, M::Lda, crate::sfc_field_events::SCREEN_DARK);
        for _ in 0..3 {
            emit(&mut p, M::Lsr, A::Accumulator, O::None);
        }
        branch(&mut p, M::Beq, "b1_screen_lit");
        write(&mut p, 0x2130, 0);
        abs(&mut p, M::Lda, crate::sfc_ending::SCREEN_WHITE);
        branch(&mut p, M::Beq, "b1_screen_subtract");
        emit(&mut p, M::Lda, A::Immediate, O::Byte(0x21));
        branch(&mut p, M::Bra, "b1_screen_math");
        p.label("b1_screen_subtract");
        emit(&mut p, M::Lda, A::Immediate, O::Byte(0xa1));
        p.label("b1_screen_math");
        abs(&mut p, M::Sta, 0x2131);
        abs(&mut p, M::Lda, crate::sfc_field_events::SCREEN_DARK);
        for _ in 0..3 {
            emit(&mut p, M::Lsr, A::Accumulator, O::None);
        }
        emit(&mut p, M::Ora, A::Immediate, O::Byte(0xe0));
        abs(&mut p, M::Sta, 0x2132);
        branch(&mut p, M::Bra, "b1_screen_set");
        p.label("b1_screen_lit");
        abs(&mut p, M::Stz, 0x2131);
        p.label("b1_screen_set");
        call(&mut p, "b1_stream");
        call(&mut p, "b1_stone_tick");
        call(&mut p, "b1_map_event_tick");
        abs(&mut p, M::Lda, 0x2137);
        abs(&mut p, M::Lda, 0x213d);
        abs(&mut p, M::Sta, 0x67e);
        abs(&mut p, M::Lda, 0x213d);
        abs(&mut p, M::Sta, 0x67f);
        call(&mut p, "b1_text_post");
        call(&mut p, "sound_frame");
        call(&mut p, "b1_sample");
        call(&mut p, "b1_tick");
        call(&mut p, "b1_experience_tick");
        call(&mut p, "b1_equipment_tick");
        call(&mut p, "b1_portrait_tick");
        call(&mut p, "field_smoke_tick");
        call(&mut p, "field_shards_tick");
        call(&mut p, "field_flames_tick");
        call(&mut p, "field_guardian_tick");
        call(&mut p, "b1_view_gather");
        p.emit_label_ref(M::Brl, A::Relative16, "nmi_done");
        p.label("not_b1");
        abs(&mut p, M::Stz, 0x2131);
        abs(&mut p, M::Lda, 0x22a);
        branch(&mut p, M::Bne, "not_title");
        abs(&mut p, M::Lda, 0x22d);
        branch(&mut p, M::Beq, "not_title");
        call(&mut p, "b1_stream_title");
        call(&mut p, "title_scroll_apply");
        abs(&mut p, M::Lda, 0x2137);
        abs(&mut p, M::Lda, 0x213d);
        abs(&mut p, M::Sta, 0x67e);
        abs(&mut p, M::Lda, 0x213d);
        abs(&mut p, M::Sta, 0x67f);
        call(&mut p, "sound_frame");
        call(&mut p, "sample");
        emit(&mut p, M::Rep, A::ImmediateByte, O::Byte(0x20));
        abs(&mut p, M::Stz, PRESS_GATHER);
        abs(&mut p, M::Lda, pad.held());
        abs(&mut p, M::Sta, GATHER_HELD);
        emit(&mut p, M::Sep, A::ImmediateByte, O::Byte(0x20));
        call(&mut p, "title_tick");
        emit(&mut p, M::Jsl, A::AbsoluteLong, O::Long(unpack[1]));
        p.emit_label_ref(M::Brl, A::Relative16, "nmi_done");
        p.label("not_title");
    }
    abs(&mut p, M::Lda, 0x22a);
    branch(&mut p, M::Beq, "running");
    if b1 {
        if let Some(voice) = crate::sfc_sound::voice(layout, 17)? {
            abs(&mut p, M::Lda, crate::sfc_save::LOADING);
            branch(&mut p, M::Beq, "b1_entry_now");
            emit(&mut p, M::Lda, A::Immediate, O::Byte(voice));
            call(&mut p, "sound_effect_sounding");
            p.emit_label_ref(M::Bcc, A::Relative8, "b1_entry_now");
            p.emit_label_ref(M::Brl, A::Relative16, "prologue_tail");
            p.label("b1_entry_now");
        }
        call(&mut p, "b1_enter");
        p.emit_label_ref(M::Brl, A::Relative16, "prologue_tail");
    }
    abs(&mut p, M::Lda, pad.pressed() + 1);
    emit(&mut p, M::And, A::Immediate, O::Byte(0x10));
    branch(&mut p, M::Beq, "prologue_tail");
    call(&mut p, "restart");
    p.label("running");
    abs(&mut p, M::Lda, 0x228);
    branch(&mut p, M::Beq, "countdown");
    write(&mut p, 0x2100, 15);
    abs(&mut p, M::Stz, 0x228);
    branch(&mut p, M::Bra, "prologue_tail");
    p.label("countdown");
    emit(&mut p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    abs(&mut p, M::Lda, 0x220);
    branch(&mut p, M::Beq, "next_record");
    abs(&mut p, M::Dec, 0x220);
    branch(&mut p, M::Bne, "record_press");
    p.label("next_record");
    emit(&mut p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    emit(&mut p, M::Jsl, A::AbsoluteLong, O::Long(unpack[2]));
    branch(&mut p, M::Bcc, "prologue_tail");
    call(&mut p, "load_record");
    p.label("record_press");
    emit(&mut p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    abs(&mut p, M::Lda, 0x220);
    branch(&mut p, M::Beq, "prologue_tail");
    abs(&mut p, M::Lda, pad.pressed());
    call(&mut p, "prologue_skip");
    p.label("prologue_tail");
    emit(&mut p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    if b1 {
        call(&mut p, "sound_frame");
    }
    call(&mut p, "sample");
    emit(&mut p, M::Jsl, A::AbsoluteLong, O::Long(unpack[1]));
    if b1 {
        emit(&mut p, M::Rep, A::ImmediateByte, O::Byte(0x20));
        abs(&mut p, M::Stz, PRESS_GATHER);
        abs(&mut p, M::Lda, pad.held());
        abs(&mut p, M::Sta, GATHER_HELD);
        emit(&mut p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    }
    p.label("nmi_done");
    if b1 {
        emit(&mut p, M::Sep, A::ImmediateByte, O::Byte(0x20));
        call(&mut p, "sound_frame_owed");
        abs(&mut p, M::Lda, 0x2137);
        abs(&mut p, M::Lda, 0x213d);
        abs(&mut p, M::Sta, 0x67c);
        abs(&mut p, M::Lda, 0x213d);
        abs(&mut p, M::Sta, 0x67d);
        abs(&mut p, M::Stz, NMI_BUSY);
        p.label("nmi_skipped");
    }
    rt::emit_interrupt_exit(&mut p);
    rt::emit_auto_pad(&mut p, pad, "sample");
    if b1 {
        p.label("b1_sample").assume_mode(CpuMode::NATIVE_M8_X16);
        p.label("b1_sample_wait");
        abs(&mut p, M::Lda, 0x4212);
        emit(&mut p, M::And, A::Immediate, O::Byte(0x01));
        branch(&mut p, M::Bne, "b1_sample_wait");
        emit(&mut p, M::Rep, A::ImmediateByte, O::Byte(0x20));
        abs(&mut p, M::Lda, pad.held());
        abs(&mut p, M::Sta, pad.previous());
        abs(&mut p, M::Lda, 0x4218);
        abs(&mut p, M::Sta, pad.held());
        abs(&mut p, M::Eor, pad.previous());
        abs(&mut p, M::And, pad.held());
        abs(&mut p, M::Ora, PRESS_GATHER);
        abs(&mut p, M::Sta, pad.pressed());
        abs(&mut p, M::Lda, pad.held());
        emit(&mut p, M::Eor, A::Immediate, O::Word(0xffff));
        abs(&mut p, M::And, pad.previous());
        abs(&mut p, M::Sta, pad.released());
        abs(&mut p, M::Stz, PRESS_GATHER);
        abs(&mut p, M::Lda, pad.held());
        abs(&mut p, M::Sta, GATHER_HELD);
        emit(&mut p, M::Sep, A::ImmediateByte, O::Byte(0x20));
        implied(&mut p, M::Rts);
        p.label("pad_gather").assume_mode(CpuMode::NATIVE_M8_X16);
        p.label("pad_gather_wait");
        abs(&mut p, M::Lda, 0x4212);
        emit(&mut p, M::And, A::Immediate, O::Byte(0x01));
        branch(&mut p, M::Bne, "pad_gather_wait");
        emit(&mut p, M::Rep, A::ImmediateByte, O::Byte(0x20));
        abs(&mut p, M::Lda, GATHER_HELD);
        emit(&mut p, M::Eor, A::Immediate, O::Word(0xffff));
        abs(&mut p, M::And, 0x4218);
        abs(&mut p, M::Ora, PRESS_GATHER);
        abs(&mut p, M::Sta, PRESS_GATHER);
        abs(&mut p, M::Lda, 0x4218);
        abs(&mut p, M::Sta, GATHER_HELD);
        emit(&mut p, M::Sep, A::ImmediateByte, O::Byte(0x20));
        implied(&mut p, M::Rts);
    }
    p.label("load_record").assume_mode(CpuMode::NATIVE_M8_X16);
    for (i, to) in [
        0x220, 0x221, 0x222, 0x223, 0xe4, 0xe5, 0xe6, 0x224, 0x225, 0x228, 0x229, 0x226,
    ]
    .into_iter()
    .enumerate()
    {
        read(&mut p, 0xe0, i as u16, to);
    }
    call(&mut p, "prologue_record_loaded");
    if b1 {
        abs(&mut p, M::Lda, 0x229);
        branch(&mut p, M::Beq, "record_music_done");
        branch(&mut p, M::Bpl, "record_music_play");
        call(&mut p, "sound_stop");
        branch(&mut p, M::Bra, "record_music_done");
        p.label("record_music_play");
        emit(&mut p, M::Dec, A::Accumulator, O::None);
        call(&mut p, "sound_play");
        p.label("record_music_done");
        abs(&mut p, M::Lda, 0x226);
        branch(&mut p, M::Beq, "record_voice_done");
        emit(&mut p, M::Rep, A::ImmediateByte, O::Byte(0x20));
        emit(&mut p, M::And, A::Immediate, O::Word(0xff));
        implied(&mut p, M::Tax);
        emit(&mut p, M::Sep, A::ImmediateByte, O::Byte(0x20));
        let voices = layout.cpu_address(layout.section("sound_voice_map")?.offset)?;
        emit(&mut p, M::Lda, A::AbsoluteLongX, O::Long(voices - 1));
        branch(&mut p, M::Beq, "record_voice_done");
        call(&mut p, "sound_effect");
        p.label("record_voice_done");
    }
    abs(&mut p, M::Lda, 0x220);
    abs(&mut p, M::Ora, 0x221);
    branch(&mut p, M::Bne, "record_present");
    write(&mut p, 0x22a, 1);
    implied(&mut p, M::Rts);
    p.label("record_present");
    abs(&mut p, M::Lda, 0x228);
    branch(&mut p, M::Beq, "upload");
    write(&mut p, 0x4200, 1);
    write(&mut p, 0x2100, 0x80);
    abs(&mut p, M::Inc, 0x22b);
    p.label("upload");
    read(&mut p, 0xe4, 0, 0x227);
    abs(&mut p, M::Lda, 0x227);
    branch(&mut p, M::Bne, "not_vram");
    write(&mut p, 0x2115, 0x80);
    write(&mut p, 0x4300, 1);
    write(&mut p, 0x4301, 0x18);
    read(&mut p, 0xe4, 1, 0x2116);
    read(&mut p, 0xe4, 2, 0x2117);
    branch(&mut p, M::Bra, "dma");
    p.label("not_vram");
    emit(&mut p, M::Cmp, A::Immediate, O::Byte(1));
    branch(&mut p, M::Bne, "oam");
    write(&mut p, 0x4300, 0);
    write(&mut p, 0x4301, 0x22);
    read(&mut p, 0xe4, 1, 0x2121);
    branch(&mut p, M::Bra, "dma");
    p.label("oam");
    emit(&mut p, M::Cmp, A::Immediate, O::Byte(3));
    branch(&mut p, M::Bne, "actual_oam");
    write(&mut p, 0x4300, 0);
    write(&mut p, 0x4301, 0x0d);
    branch(&mut p, M::Bra, "dma");
    p.label("actual_oam");
    write(&mut p, 0x4300, 0);
    write(&mut p, 0x4301, 4);
    read(&mut p, 0xe4, 1, 0x2102);
    read(&mut p, 0xe4, 2, 0x2103);
    p.label("dma");
    for (index, to) in [
        (3, 0x4305),
        (4, 0x4306),
        (5, 0x4302),
        (6, 0x4303),
        (7, 0x4304),
    ] {
        read(&mut p, 0xe4, index, to);
    }
    write(&mut p, 0x420b, 1);
    emit(&mut p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    abs(&mut p, M::Lda, 0xe4);
    implied(&mut p, M::Clc);
    emit(&mut p, M::Adc, A::Immediate, O::Word(8));
    abs(&mut p, M::Sta, 0xe4);
    emit(&mut p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    emit(&mut p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    abs(&mut p, M::Dec, 0x224);
    emit(&mut p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    branch(&mut p, M::Beq, "advance_record");
    p.emit_label_ref(M::Brl, A::Relative16, "upload");
    p.label("advance_record");
    abs(&mut p, M::Lda, 0xe0);
    implied(&mut p, M::Clc);
    emit(&mut p, M::Adc, A::Immediate, O::Byte(16));
    abs(&mut p, M::Sta, 0xe0);
    branch(&mut p, M::Bcc, "record_done");
    abs(&mut p, M::Inc, 0xe1);
    branch(&mut p, M::Bne, "record_done");
    write(&mut p, 0xe1, 0x80);
    abs(&mut p, M::Inc, 0xe2);
    p.label("record_done");
    write(&mut p, 0x4200, 0x81);
    implied(&mut p, M::Rts);
    if b1 {
        crate::sfc_b1::emit(&mut p, layout, pad.pressed())?;
    } else {
        emit_skip(&mut p, layout)?;
    }
    p.label("irq").assume_mode(CpuMode::NATIVE_M8_X16);
    implied(&mut p, M::Rti);
    let bank = if fast { 0x80 } else { 0 };
    let assembled =
        crate::sfc_width_check::assemble(&p, CodeLocation::new(bank, 0x8000), CpuMode::Emulation)?;
    let mut main_labels = serde_json::Map::new();
    if b1 {
        for name in [
            "b1_portrait_tick",
            "b1_portrait_select",
            "b1_portrait_shown",
            "b1_portrait_animate",
            "stone_palette_send",
            "b1_menu_upload",
            "b1_menu_descriptors_done",
            "b1_menu_no_icons",
            "b1_text_work",
            "b1_text_restored",
            "b1_text_cached_page",
            "b1_text_uncached_page",
            "b1_text_publish",
            "b1_text_lines_counted",
            "b1_text_next",
            "b1_text_glyph_draw",
            "b1_text_finish",
            "b1_text_work_end",
            "b1_battle_tick",
            "b1_battle_no_bar",
            "b1_battle_drawn",
            "b1_battle_caught_up",
            "b1_tick",
            "b1_input",
            "b1_input_ready",
            "b1_field_busy",
            "b1_direction_free",
            "b1_navigation_return",
        ] {
            let address = assembled
                .label_location(name)
                .ok_or("Missing main profiling entry")?;
            main_labels.insert(name.to_string(), address.to_u32().into());
        }
    }
    let at = |label| assembled.label_location(label).unwrap().program_counter();
    if b1 && at("far_trampolines") != 0x8000 {
        return Err("Far code trampolines left the bank's start".into());
    }
    let mut main_bytes = assembled.bytes().to_vec();
    if b1 {
        crate::sfc_battle_objects::common_density::place_main(&mut main_bytes, layout)?;
        crate::sfc_battle_objects::common_blink_density::place_main(&mut main_bytes, layout)?;
        crate::sfc_battle_objects::common_radial::place_main(&mut main_bytes, layout)?;
        crate::sfc_battle_objects::lightning_radius::place_main(&mut main_bytes, layout)?;
        crate::sfc_battle_objects::common_blink::place_main(&mut main_bytes, layout)?;
        crate::sfc_battle_objects::common_batch::place_main(&mut main_bytes, layout)?;
    }
    Ok((
        layout.build(&main_bytes, at("reset"), at("nmi"), at("irq"))?,
        main_bytes.len(),
        main_labels.into(),
    ))
}
const NEXT_TICK: u16 = 0x210;
const SKIP_END: u16 = 0x212;
const SKIPPING: u16 = 0x214;
const PRESS_TICK: u16 = 0x216;
pub fn emit_skip(
    p: &mut w65c816::Assembler,
    layout: &sfc_builder::cartridge::CartridgeLayout,
) -> Result<()> {
    use crate::sfc::emit;
    use w65c816::{AddressingMode as A, CpuMode, Mnemonic as M, Operand as O};
    let section = layout.section("prologue_waits")?;
    let waits = layout.cpu_address(section.offset)?;
    let count = u16::from_le_bytes([section.bytes[0], section.bytes[1]]);
    let abs = |p: &mut w65c816::Assembler, m, at: u16| emit(p, m, A::Absolute, O::Word(at));
    let word = |p: &mut w65c816::Assembler, m, v: u16| emit(p, m, A::Immediate, O::Word(v));
    let long_x = |p: &mut w65c816::Assembler, m, at: u32| emit(p, m, A::AbsoluteLongX, O::Long(at));
    let branch = |p: &mut w65c816::Assembler, m, label: &str| {
        p.emit_label_ref(m, A::Relative8, label);
    };
    let implied = |p: &mut w65c816::Assembler, m| emit(p, m, A::Implied, O::None);
    p.label("prologue_skip")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    word(p, M::And, 0x80);
    branch(p, M::Beq, "prologue_skip_done");
    abs(p, M::Lda, NEXT_TICK);
    implied(p, M::Sec);
    abs(p, M::Sbc, 0x220);
    abs(p, M::Sta, PRESS_TICK);
    emit(p, M::Ldx, A::Immediate, O::Word(0));
    p.label("prologue_skip_find");
    long_x(p, M::Lda, waits + 2);
    abs(p, M::Cmp, PRESS_TICK);
    branch(p, M::Beq, "prologue_skip_started");
    branch(p, M::Bcs, "prologue_skip_next");
    p.label("prologue_skip_started");
    long_x(p, M::Lda, waits + 4);
    abs(p, M::Cmp, PRESS_TICK);
    branch(p, M::Beq, "prologue_skip_next");
    branch(p, M::Bcs, "prologue_skip_found");
    p.label("prologue_skip_next");
    implied(p, M::Inx);
    implied(p, M::Inx);
    implied(p, M::Inx);
    implied(p, M::Inx);
    emit(p, M::Cpx, A::Immediate, O::Word(count * 4));
    branch(p, M::Bne, "prologue_skip_find");
    p.label("prologue_skip_done");
    implied(p, M::Rts);
    p.label("prologue_skip_found");
    abs(p, M::Sta, SKIP_END);
    p.emit_label_ref(M::Jsr, A::Absolute, "prologue_skip_shorten");
    implied(p, M::Rts);
    p.label("prologue_record_loaded")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    abs(p, M::Lda, 0x222);
    branch(p, M::Bne, "prologue_record_later");
    abs(p, M::Stz, SKIPPING);
    p.label("prologue_record_later");
    abs(p, M::Lda, 0x220);
    branch(p, M::Beq, "prologue_record_done");
    implied(p, M::Clc);
    abs(p, M::Adc, 0x222);
    abs(p, M::Sta, NEXT_TICK);
    abs(p, M::Lda, SKIPPING);
    word(p, M::And, 0xff);
    branch(p, M::Beq, "prologue_record_done");
    p.emit_label_ref(M::Jsr, A::Absolute, "prologue_skip_shorten");
    p.label("prologue_record_done");
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    implied(p, M::Rts);
    p.label("prologue_skip_shorten")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    abs(p, M::Lda, NEXT_TICK);
    abs(p, M::Cmp, SKIP_END);
    branch(p, M::Beq, "prologue_skip_inside");
    branch(p, M::Bcc, "prologue_skip_inside");
    implied(p, M::Sec);
    abs(p, M::Sbc, SKIP_END);
    abs(p, M::Sta, 0x220);
    abs(p, M::Stz, SKIPPING);
    implied(p, M::Rts);
    p.label("prologue_skip_inside");
    word(p, M::Lda, 1);
    abs(p, M::Sta, 0x220);
    abs(p, M::Sta, SKIPPING);
    implied(p, M::Rts);
    Ok(())
}
