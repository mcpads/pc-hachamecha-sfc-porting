use crate::{Result, SOURCE, sha};
use serde::Deserialize;
use sfc_builder::cartridge::{CartridgeLayout, Section};
pub const CELLS: [usize; 4] = [15, 6, 2, 3];
const BOXES: usize = 2;
const BYTES: usize = CELLS[2] * CELLS[3] * 64;
const VARIANTS: usize = 11;
const STATS_VA: u32 = 0x4b4848;
const FREE_COLORS: [usize; 10] = [2, 3, 4, 5, 6, 7, 8, 9, 111, 47];
const PORTRAIT_BASE: usize = 80;
const ARLE_COLORS: usize = 16;
pub const SHOWN: u16 = 0x652;
#[derive(Deserialize)]
struct Resource {
    file: String,
    width: usize,
    height: usize,
    key: Option<u16>,
    load_flags: u16,
    pixels_sha256: String,
    palette_sha256: String,
}
#[derive(Deserialize)]
struct EquipmentBox {
    position: [usize; 2],
    rectangle: [usize; 4],
    item_va: u32,
}
#[derive(Deserialize)]
struct Equipment {
    size: [usize; 2],
    box_resource: String,
    icon_resource: String,
    icon_first_item: u8,
    icon_rectangles: Vec<[usize; 4]>,
    boxes: Vec<EquipmentBox>,
}
#[derive(Deserialize)]
struct Definition {
    schema: u8,
    source_sha256: String,
    resources: Vec<Resource>,
    equipment: Equipment,
}
pub struct Assets {
    tiles: Vec<u8>,
    items: [u16; BOXES],
    first_item: u8,
    pub portrait_order: Vec<usize>,
    pub colors: Vec<(usize, u16)>,
}
fn bgr555(c: &[u8]) -> u16 {
    u16::from(c[0] >> 3) | (u16::from(c[1] >> 3) << 5) | (u16::from(c[2] >> 3) << 10)
}
impl Assets {
    pub fn load(
        content: &crate::content::Content,
        frame: &crate::sfc_b1_frame::Assets,
        palette: &[u8],
        portrait_rgb: &[u8],
        background_slots: &[usize],
    ) -> Result<Self> {
        let root = content.sub("hud");
        let def: Definition = root.json("hud.json")?;
        let e = &def.equipment;
        let icon = def
            .resources
            .iter()
            .find(|r| r.file == e.icon_resource)
            .ok_or("Missing equipment icon resource")?;
        let pixels = root.read(format!("{}.indices", icon.file))?;
        let rgb = root.read(format!("{}.rgb", icon.file))?;
        let key = icon
            .key
            .and_then(|k| k.checked_sub(icon.load_flags))
            .ok_or("Equipment icons without a key")?;
        if def.schema != 1
            || def.source_sha256 != SOURCE
            || e.box_resource != "FRAME.CNS"
            || e.size != [32, 32]
            || e.boxes.len() != BOXES
            || e.icon_rectangles.len() != VARIANTS - 1
            || pixels.len() != icon.width * icon.height
            || sha(&pixels) != icon.pixels_sha256
            || sha(&rgb) != icon.palette_sha256
            || rgb.len() != 768
            || key > 255
            || e.icon_rectangles.iter().any(|&[l, t, r, b]| {
                r - l != 32 || b - t != 32 || r > icon.width || b > icon.height
            })
            || e.boxes.iter().any(|b| {
                let [l, t, r, bottom] = b.rectangle;
                r - l != 32 || bottom - t != 32 || r > 640 || bottom > 480
            })
        {
            return Err("Invalid equipment source".into());
        }
        let mut items = [0; BOXES];
        for (item, b) in items.iter_mut().zip(&e.boxes) {
            let offset = b
                .item_va
                .checked_sub(STATS_VA)
                .filter(|&o| (o as usize) < crate::sfc_battle::STATS_BYTES)
                .ok_or("Equipment byte outside the stat block")?;
            *item = crate::sfc_battle::STATS + offset as u16;
        }
        let mut used = [false; 256];
        for &[l, t, r, b] in &e.icon_rectangles {
            for y in t..b {
                for x in l..r {
                    used[usize::from(pixels[y * icon.width + x])] = true;
                }
            }
        }
        used[key as usize] = false;
        let portrait: Vec<u16> = portrait_rgb
            .as_chunks::<3>()
            .0
            .iter()
            .map(|c| bgr555(c))
            .collect();
        let portrait_colors = PORTRAIT_BASE..PORTRAIT_BASE + portrait.len();
        let text_colors = crate::sfc_b1::TEXT_PALETTE * 16..crate::sfc_b1::TEXT_PALETTE * 16 + 16;
        for &i in &FREE_COLORS {
            if portrait_colors.contains(&i) {
                return Err(format!("Equipment spare colour {i} is a portrait colour").into());
            }
            if i < 2
                || text_colors.contains(&i)
                || frame.draws(i)
                || i >= 128
                || crate::sfc_b1_stone::COLORS.contains(&(i as u8))
            {
                return Err(format!("Equipment spare colour {i} is in use").into());
            }
        }
        let entry = |i: usize| u16::from_le_bytes([palette[i * 2], palette[i * 2 + 1]]);
        let mut colour_of = [None; 256];
        let mut kept = Vec::new();
        let mut colors = Vec::new();
        for c in (0..256).filter(|&c| used[c]) {
            let value = bgr555(&rgb[c * 3..c * 3 + 3]);
            if let Some(i) = (10..48).find(|&i| frame.draws(i) && entry(i) == value) {
                colour_of[c] = Some(i);
            } else if let Some(p) = (0..portrait.len())
                .position(|i| portrait[i] == value && !background_slots.contains(&i))
            {
                if !kept.contains(&p) {
                    kept.push(p);
                }
            } else {
                let i = *FREE_COLORS
                    .get(colors.len())
                    .ok_or("Equipment icon colours exceed the free BG1 colours")?;
                colors.push((i, value));
                colour_of[c] = Some(i);
            }
        }
        if kept.len() + ARLE_COLORS > portrait.len() {
            return Err("Equipment icon colours exceed the portrait's kept colours".into());
        }
        let mut portrait_order = vec![usize::MAX; portrait.len()];
        for (n, &p) in kept.iter().enumerate() {
            portrait_order[p] = ARLE_COLORS + n;
        }
        let mut free =
            (0..portrait.len()).filter(|s| !(ARLE_COLORS..ARLE_COLORS + kept.len()).contains(s));
        for slot in portrait_order.iter_mut().filter(|s| **s == usize::MAX) {
            *slot = free.next().ok_or("Portrait order")?;
        }
        let pending: Vec<usize> = (0..256)
            .filter(|&c| used[c] && colour_of[c].is_none())
            .collect();
        for c in pending {
            let value = bgr555(&rgb[c * 3..c * 3 + 3]);
            let p = portrait
                .iter()
                .position(|&v| v == value)
                .ok_or("Icon colour")?;
            colour_of[c] = Some(PORTRAIT_BASE + portrait_order[p]);
        }
        let [cx, cy, cw, ch] = CELLS;
        let mut tiles = Vec::with_capacity(BOXES * VARIANTS * BYTES);
        for (n, b) in e.boxes.iter().enumerate() {
            let [px, py] = b.position;
            let [bl, bt, _, _] = b.rectangle;
            let top = cy + n * ch;
            for y in 0..224 {
                for x in 0..256 {
                    let [ox, oy] = crate::sfc_b1_frame::source(x, y);
                    let inside = (px..px + 32).contains(&ox) && (py..py + 32).contains(&oy);
                    let cell = (cx * 8..(cx + cw) * 8).contains(&x)
                        && (top * 8..(top + ch) * 8).contains(&y);
                    if inside && !cell {
                        return Err("Equipment box outside its SFC cells".into());
                    }
                }
            }
            for variant in 0..VARIANTS {
                let mut box_tiles = vec![0; BYTES];
                for y in 0..ch * 8 {
                    for x in 0..cw * 8 {
                        let (sx, sy) = (cx * 8 + x, top * 8 + y);
                        let [ox, oy] = crate::sfc_b1_frame::source(sx, sy);
                        let mut color = frame.background(sx, sy);
                        if (px..px + 32).contains(&ox) && (py..py + 32).contains(&oy) {
                            let (dx, dy) = (ox - px, oy - py);
                            color = frame.source_pixel(bl + dx, bt + dy);
                            if variant > 0 {
                                let [il, it, _, _] = e.icon_rectangles[variant - 1];
                                let local = usize::from(pixels[(it + dy) * icon.width + il + dx]);
                                if local != key as usize {
                                    color = colour_of[local].ok_or("Icon colour")? as u8;
                                }
                            }
                        }
                        for plane in 0..8 {
                            box_tiles[(y / 8 * cw + x / 8) * 64
                                + plane / 2 * 16
                                + y % 8 * 2
                                + plane % 2] |= ((color >> plane) & 1) << (7 - x % 8);
                        }
                    }
                }
                tiles.extend(box_tiles);
            }
        }
        Ok(Self {
            tiles,
            items,
            first_item: e.icon_first_item,
            portrait_order,
            colors,
        })
    }
    pub fn cells(first: usize) -> impl Iterator<Item = (usize, usize)> {
        let [cx, cy, cw, ch] = CELLS;
        (0..BOXES * ch * cw).map(move |i| {
            let (n, r, c) = (i / (ch * cw), i / cw % ch, i % cw);
            ((cy + n * ch + r) * 32 + cx + c, first + i)
        })
    }
    pub fn tile_count() -> usize {
        BOXES * BYTES / 64
    }
    pub fn append(
        &self,
        sections: &mut Vec<Section>,
        space: &mut crate::sfc_rom::Space,
        vram: u16,
    ) -> Result<()> {
        let source = super::sfc_b1::add(
            sections,
            space,
            "b1_equipment_tiles".into(),
            self.tiles.clone(),
        )?;
        let mut jobs = Vec::new();
        for n in 0..BOXES {
            for variant in 0..VARIANTS {
                jobs.extend(super::sfc_b1::transfer(
                    source + ((n * VARIANTS + variant) * BYTES) as u32,
                    vram + (n * BYTES) as u16,
                    BYTES,
                    0,
                ));
            }
        }
        super::sfc_b1::add(sections, space, "b1_equipment_jobs".into(), jobs)?;
        let mut items: Vec<u8> = self.items.iter().flat_map(|a| a.to_le_bytes()).collect();
        items.push(self.first_item);
        super::sfc_b1::add(sections, space, "b1_equipment_items".into(), items)?;
        Ok(())
    }
}
pub fn emit(p: &mut w65c816::Assembler, layout: &CartridgeLayout) -> Result<()> {
    use crate::sfc::{emit, implied};
    use w65c816::{AddressingMode as A, CpuMode, Mnemonic as M, Operand as O};
    let abs = |p: &mut w65c816::Assembler, m, a| emit(p, m, A::Absolute, O::Word(a));
    let jobs = layout.cpu_address(layout.section("b1_equipment_jobs")?.offset)?;
    let items = &layout.section("b1_equipment_items")?.bytes;
    let first_item = items[2 * BOXES];
    p.label("b1_equipment_tick")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    abs(p, M::Lda, super::sfc_b1::QUEUE + 13);
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    p.emit_label_ref(M::Beq, A::Relative8, "equipment_free");
    implied(p, M::Rts);
    p.label("equipment_free");
    for n in 0..BOXES {
        let item = u16::from_le_bytes([items[2 * n], items[2 * n + 1]]);
        let next = format!("equipment_box_{}", n + 1);
        let none = format!("equipment_none_{n}");
        let state = format!("equipment_state_{n}");
        abs(p, M::Lda, item);
        implied(p, M::Sec);
        emit(p, M::Sbc, A::Immediate, O::Byte(first_item));
        emit(p, M::Cmp, A::Immediate, O::Byte((VARIANTS - 1) as u8));
        p.emit_label_ref(M::Bcs, A::Relative8, &none);
        emit(p, M::Inc, A::Accumulator, O::None);
        p.emit_label_ref(M::Bra, A::Relative8, &state);
        p.label(&none);
        emit(p, M::Lda, A::Immediate, O::Byte(0));
        p.label(&state);
        abs(p, M::Cmp, SHOWN + n as u16);
        p.emit_label_ref(M::Beq, A::Relative8, &next);
        abs(p, M::Sta, SHOWN + n as u16);
        emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
        emit(p, M::And, A::Immediate, O::Word(255));
        for _ in 0..3 {
            emit(p, M::Asl, A::Accumulator, O::None);
        }
        implied(p, M::Clc);
        emit(
            p,
            M::Adc,
            A::Immediate,
            O::Word((jobs + (n * VARIANTS * 8) as u32) as u16),
        );
        abs(p, M::Sta, super::sfc_b1::QUEUE + 10);
        emit(p, M::Lda, A::Immediate, O::Word(1));
        abs(p, M::Sta, super::sfc_b1::QUEUE + 13);
        emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
        emit(p, M::Lda, A::Immediate, O::Byte((jobs >> 16) as u8));
        abs(p, M::Sta, super::sfc_b1::QUEUE + 12);
        implied(p, M::Rts);
        p.label(&next);
    }
    implied(p, M::Rts);
    Ok(())
}
