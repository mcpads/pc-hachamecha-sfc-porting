use crate::{Result, SOURCE, sha};
use serde::Deserialize;
use sfc_builder::cartridge::{CartridgeLayout, Section};
pub const SMOKE_TIME: u16 = 0x7d0;
pub const SMOKE_DRAWS: u16 = 0x7d1;
const SMOKE_SHOWN: u16 = 0x7e1;
pub const PUFFS: usize = 8;
pub const PUFF_EVERY: usize = 16;
const SPAWN_TICKS: usize = 4;
const FRAME_TICKS: usize = 4;
const FRAMES: usize = 6;
const FRAME: usize = 48;
const SURFACE: [usize; 2] = [224, 240];
const ORIGIN: [usize; 2] = [96, 64];
const DRAWS: [usize; 2] = [0x70, 0x80];
const FIRST_ENTRY: usize = 92;
const ENTRIES_PER_PUFF: usize = 4;
const PALETTE: u8 = 3;
const FIRST_COLOUR: u16 = 128 + 16 * PALETTE as u16 + 4;
fn quadrant_name(j: usize) -> u16 {
    crate::sfc_b1_menu::ICON_NAME + (2 * (j % 8) + 32 * (j / 8)) as u16
}
#[derive(Deserialize)]
struct Sheet {
    schema: u8,
    source_sha256: String,
    width: usize,
    height: usize,
    pixels_sha256: String,
    palette_sha256: String,
}
pub struct Assets {
    frames: Vec<Vec<(u8, u8, usize)>>,
    tiles: Vec<u8>,
    colours: Vec<u8>,
    xs: Vec<u8>,
    ys: Vec<u8>,
    pub report: serde_json::Value,
}
impl Assets {
    pub fn load(content: &crate::content::Content, panel: [usize; 4]) -> Result<Self> {
        let root = content.sub("b3-smoke");
        let sheet: Sheet = root.json("smoke.json")?;
        let pixels = root.read("KEMURI.CNS.indices")?;
        let rgb = root.read("KEMURI.CNS.rgb")?;
        if sheet.schema != 1
            || sheet.source_sha256 != SOURCE
            || sheet.width != FRAME * FRAMES
            || sheet.height != FRAME
            || sha(&pixels) != sheet.pixels_sha256
            || sha(&rgb) != sheet.palette_sha256
        {
            return Err("Invalid smoke sheet".into());
        }
        let objects: serde_json::Value = content.json("floor-objects/floor-objects.json")?;
        let origin: [usize; 2] = serde_json::from_value(objects["screen_origin"].clone())?;
        let [px, py, pw, ph] = panel;
        let scale = |v: usize, axis: usize| (v - origin[axis]) * [pw, ph][axis] / SURFACE[axis];
        let (w, h) = (
            (FRAME * pw).div_ceil(SURFACE[0]),
            (FRAME * ph).div_ceil(SURFACE[1]),
        );
        let mut used: Vec<u8> = Vec::new();
        let mut frames = Vec::new();
        let mut tiles = Vec::new();
        let mut quadrant = 0;
        for f in 0..FRAMES {
            let at = |u: usize, v: usize| {
                let mut counts = [0usize; 256];
                for y in v * SURFACE[1] / ph..((v + 1) * SURFACE[1] / ph).min(FRAME) {
                    for x in u * SURFACE[0] / pw..((u + 1) * SURFACE[0] / pw).min(FRAME) {
                        counts[usize::from(pixels[y * sheet.width + f * FRAME + x])] += 1;
                    }
                }
                (1..256)
                    .max_by_key(|&c| (counts[c], std::cmp::Reverse(c)))
                    .filter(|&c| counts[c] > 0)
                    .unwrap_or(0) as u8
            };
            let drawn: Vec<(usize, usize)> = (0..h)
                .flat_map(|v| (0..w).map(move |u| (u, v)))
                .filter(|&(u, v)| at(u, v) != 0)
                .collect();
            let mut sprites = Vec::new();
            if let (Some(x0), Some(y0)) = (
                drawn.iter().map(|d| d.0).min(),
                drawn.iter().map(|d| d.1).min(),
            ) {
                let (x1, y1) = (
                    drawn.iter().map(|d| d.0).max().unwrap_or(x0),
                    drawn.iter().map(|d| d.1).max().unwrap_or(y0),
                );
                for qy in (y0..=y1).step_by(16) {
                    for qx in (x0..=x1).step_by(16) {
                        let mut cell = [[0u8; 16]; 16];
                        for (y, row) in cell.iter_mut().enumerate() {
                            for (x, c) in row.iter_mut().enumerate() {
                                let (u, v) = (qx + x, qy + y);
                                if u < w && v < h && at(u, v) != 0 {
                                    let value = at(u, v);
                                    let i = used.iter().position(|&k| k == value).unwrap_or_else(
                                        || {
                                            used.push(value);
                                            used.len() - 1
                                        },
                                    );
                                    *c = 4 + i as u8;
                                }
                            }
                        }
                        for half in 0..2 {
                            for t in 0..2 {
                                let mut bytes = [0u8; 32];
                                for y in 0..8 {
                                    for x in 0..8 {
                                        let c = cell[half * 8 + y][t * 8 + x];
                                        for plane in 0..4 {
                                            bytes[plane / 2 * 16 + y * 2 + plane % 2] |=
                                                ((c >> plane) & 1) << (7 - x);
                                        }
                                    }
                                }
                                tiles.extend(bytes);
                            }
                        }
                        sprites.push((qx as u8, qy as u8, quadrant));
                        quadrant += 1;
                    }
                }
            }
            if sprites.len() > ENTRIES_PER_PUFF {
                return Err("Smoke frame needs more sprites".into());
            }
            frames.push(sprites);
        }
        if quadrant > 16 || 4 + used.len() > 16 {
            return Err("Smoke exceeds its borrowed names or colours".into());
        }
        let colours = used
            .iter()
            .flat_map(|&i| {
                let c = &rgb[usize::from(i) * 3..usize::from(i) * 3 + 3];
                (u16::from(c[0] >> 3) | u16::from(c[1] >> 3) << 5 | u16::from(c[2] >> 3) << 10)
                    .to_le_bytes()
            })
            .collect();
        let xs = (0..DRAWS[0])
            .map(|r| u8::try_from(px + scale(ORIGIN[0] + r, 0)))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let ys = (0..DRAWS[1])
            .map(|r| u8::try_from(py + scale(ORIGIN[1] + r, 1)))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(Self {
            report: serde_json::json!(
                { "puffs" : PUFFS, "of" : PUFFS * PUFF_EVERY, "frames" : frames.iter()
                .map(Vec::len).collect::< Vec < _ >> (), "quadrants" : quadrant,
                "colours" : used.len(), "limits" :
                ["One puff in sixteen is drawn (eight, one every four ticks); every RNG draw is kept.",
                "Frames are reduced by majority colour per block, not the view's nearest sampling."]
                }
            ),
            frames,
            tiles,
            colours,
            xs,
            ys,
        })
    }
    pub fn append(
        &self,
        sections: &mut Vec<Section>,
        space: &mut crate::sfc_rom::Space,
    ) -> Result<()> {
        use super::sfc_b1::{add, transfer};
        let tiles = add(sections, space, "smoke_tiles".into(), self.tiles.clone())?;
        let mut list = Vec::new();
        for j in 0..self.tiles.len() / 128 {
            for half in 0..2u16 {
                let name = quadrant_name(j) + 16 * half;
                list.extend(transfer(
                    tiles + (j * 128) as u32 + u32::from(half) * 64,
                    0xe000 + (name - 256) * 32,
                    64,
                    0,
                ));
            }
        }
        let colours = add(
            sections,
            space,
            "smoke_colours".into(),
            self.colours.clone(),
        )?;
        list.extend(transfer(colours, FIRST_COLOUR, self.colours.len(), 1));
        let count = list.len() / 8;
        let list = add(sections, space, "smoke_list".into(), list)?;
        let mut record = list.to_le_bytes()[..3].to_vec();
        record.extend((count as u16).to_le_bytes());
        add(sections, space, "smoke_job".into(), record)?;
        let mut table = Vec::new();
        for sprites in &self.frames {
            for k in 0..ENTRIES_PER_PUFF {
                match sprites.get(k) {
                    Some(&(dx, dy, j)) => {
                        let name = quadrant_name(j);
                        let attribute = (name >> 8) as u8 | PALETTE << 1 | 3 << 4;
                        table.extend([dx, dy, name as u8, attribute]);
                    }
                    None => table.extend([0, 0xff, 0, 0]),
                }
            }
        }
        add(sections, space, "smoke_frames".into(), table)?;
        add(sections, space, "smoke_xs".into(), self.xs.clone())?;
        add(sections, space, "smoke_ys".into(), self.ys.clone())?;
        Ok(())
    }
}
pub fn emit(p: &mut w65c816::Assembler, layout: &CartridgeLayout) -> Result<()> {
    use crate::sfc::{emit, implied, write};
    use crate::sfc_b1_menu::OAM_BUFFER;
    use w65c816::{AddressingMode as A, CpuMode, Mnemonic as M, Operand as O};
    let abs = |p: &mut w65c816::Assembler, m, at: u16| emit(p, m, A::Absolute, O::Word(at));
    let long_x = |p: &mut w65c816::Assembler, m, at: u32| emit(p, m, A::AbsoluteLongX, O::Long(at));
    let address = |name: &str| -> Result<u32> { layout.cpu_address(layout.section(name)?.offset) };
    let (job, frames, xs, ys, oam_job) = (
        address("smoke_job")?,
        address("smoke_frames")?,
        address("smoke_xs")?,
        address("smoke_ys")?,
        address("menu_oam_job")?,
    );
    let enqueue = |p: &mut w65c816::Assembler, record: u32, slot: u16| {
        emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
        emit(p, M::Lda, A::Immediate, O::Word(record as u16));
        abs(p, M::Sta, 0xe0);
        emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
        write(p, 0xe2, (record >> 16) as u8);
        emit(p, M::Ldx, A::Immediate, O::Word(slot));
        p.emit_label_ref(M::Jsr, A::Absolute, "b1_enqueue");
    };
    p.label("field_smoke_start")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    implied(p, M::Phx);
    enqueue(p, job, crate::sfc_b1::FADE_SLOT);
    implied(p, M::Plx);
    write(p, SMOKE_TIME, 1);
    implied(p, M::Rts);
    let last = (PUFFS - 1) * SPAWN_TICKS + FRAMES * FRAME_TICKS;
    p.label("field_smoke_tick")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, SMOKE_TIME);
    p.emit_label_ref(M::Bne, A::Relative8, "field_smoke_on");
    implied(p, M::Rts);
    p.label("field_smoke_on");
    {
        let slot = crate::sfc_b1::QUEUE + crate::sfc_b1::FADE_SLOT;
        abs(p, M::Lda, slot + 3);
        abs(p, M::Ora, slot + 4);
        p.emit_label_ref(M::Beq, A::Relative8, "field_smoke_ready");
        implied(p, M::Rts);
        p.label("field_smoke_ready");
    }
    let sr = |p: &mut w65c816::Assembler, m, at: u8| emit(p, m, A::StackRelative, O::Byte(at));
    let m16 = |p: &mut w65c816::Assembler| emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    let m8 = |p: &mut w65c816::Assembler| emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    let entries = OAM_BUFFER + (FIRST_ENTRY * 4) as u16;
    const _: () = assert!(ENTRIES_PER_PUFF * 4 == 16);
    emit(p, M::Ldy, A::Immediate, O::Word(0));
    m16(p);
    abs(p, M::Lda, SMOKE_TIME);
    emit(p, M::And, A::Immediate, O::Word(0xff));
    emit(p, M::Dec, A::Accumulator, O::None);
    implied(p, M::Pha);
    m8(p);
    emit(p, M::Lda, A::Immediate, O::Byte(0));
    implied(p, M::Pha);
    p.label("field_smoke_puff")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    sr(p, M::Lda, 2);
    p.emit_label_ref(M::Bmi, A::Relative8, "field_smoke_hide");
    emit(
        p,
        M::Cmp,
        A::Immediate,
        O::Word((FRAMES * FRAME_TICKS) as u16),
    );
    p.emit_label_ref(M::Bcs, A::Relative8, "field_smoke_hide");
    for (at, table) in [(1u16, ys), (0, xs)] {
        sr(p, M::Lda, if at == 1 { 1 } else { 2 });
        emit(p, M::And, A::Immediate, O::Word(0xff));
        implied(p, M::Tax);
        emit(p, M::Lda, A::AbsoluteX, O::Word(SMOKE_DRAWS + at));
        emit(p, M::And, A::Immediate, O::Word(0xff));
        implied(p, M::Tax);
        m8(p);
        long_x(p, M::Lda, table);
        implied(p, M::Pha);
        m16(p);
    }
    sr(p, M::Lda, 4);
    emit(p, M::And, A::Immediate, O::Word(!(FRAME_TICKS as u16 - 1)));
    emit(p, M::Asl, A::Accumulator, O::None);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Tax);
    m8(p);
    p.label("field_smoke_entry")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    long_x(p, M::Lda, frames + 1);
    emit(p, M::Cmp, A::Immediate, O::Byte(0xff));
    p.emit_label_ref(M::Beq, A::Relative8, "field_smoke_off");
    implied(p, M::Clc);
    sr(p, M::Adc, 2);
    emit(p, M::Sta, A::AbsoluteY, O::Word(entries + 1));
    long_x(p, M::Lda, frames);
    implied(p, M::Clc);
    sr(p, M::Adc, 1);
    emit(p, M::Sta, A::AbsoluteY, O::Word(entries));
    long_x(p, M::Lda, frames + 2);
    emit(p, M::Sta, A::AbsoluteY, O::Word(entries + 2));
    long_x(p, M::Lda, frames + 3);
    emit(p, M::Sta, A::AbsoluteY, O::Word(entries + 3));
    p.emit_label_ref(M::Bra, A::Relative8, "field_smoke_next");
    p.label("field_smoke_off");
    emit(p, M::Lda, A::Immediate, O::Byte(224));
    emit(p, M::Sta, A::AbsoluteY, O::Word(entries + 1));
    p.label("field_smoke_next");
    for _ in 0..4 {
        implied(p, M::Inx);
    }
    for _ in 0..4 {
        implied(p, M::Iny);
    }
    implied(p, M::Tya);
    emit(p, M::And, A::Immediate, O::Byte(0x0f));
    p.emit_label_ref(M::Bne, A::Relative8, "field_smoke_entry");
    implied(p, M::Pla);
    implied(p, M::Pla);
    p.emit_label_ref(M::Bra, A::Relative8, "field_smoke_advance");
    p.label("field_smoke_hide")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    m8(p);
    emit(p, M::Lda, A::Immediate, O::Byte(224));
    for k in 0..ENTRIES_PER_PUFF as u16 {
        emit(p, M::Sta, A::AbsoluteY, O::Word(entries + 4 * k + 1));
    }
    m16(p);
    implied(p, M::Tya);
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Word(16));
    implied(p, M::Tay);
    m8(p);
    p.label("field_smoke_advance")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    sr(p, M::Lda, 1);
    emit(p, M::Inc, A::Accumulator, O::None);
    emit(p, M::Inc, A::Accumulator, O::None);
    sr(p, M::Sta, 1);
    m16(p);
    sr(p, M::Lda, 2);
    implied(p, M::Sec);
    emit(p, M::Sbc, A::Immediate, O::Word(SPAWN_TICKS as u16));
    sr(p, M::Sta, 2);
    m8(p);
    sr(p, M::Lda, 1);
    emit(p, M::Cmp, A::Immediate, O::Byte(2 * PUFFS as u8));
    p.emit_label_ref(M::Beq, A::Relative8, "field_smoke_puffs_done");
    p.emit_label_ref(M::Brl, A::Relative16, "field_smoke_puff");
    p.label("field_smoke_puffs_done");
    for _ in 0..3 {
        implied(p, M::Pla);
    }
    for e in (FIRST_ENTRY..FIRST_ENTRY + PUFFS * ENTRIES_PER_PUFF).step_by(4) {
        write(p, OAM_BUFFER + 512 + (e / 4) as u16, 0xaa);
    }
    write(p, SMOKE_SHOWN, 1);
    enqueue(p, oam_job, crate::sfc_b1::MENU_OAM_SLOT);
    abs(p, M::Inc, SMOKE_TIME);
    abs(p, M::Lda, SMOKE_TIME);
    emit(p, M::Cmp, A::Immediate, O::Byte(last as u8 + 2));
    p.emit_label_ref(M::Bcc, A::Relative8, "field_smoke_running");
    abs(p, M::Stz, SMOKE_TIME);
    p.label("field_smoke_running");
    implied(p, M::Rts);
    Ok(())
}
