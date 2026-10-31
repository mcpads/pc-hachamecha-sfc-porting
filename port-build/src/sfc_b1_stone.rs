use crate::{Result, SOURCE, sha};
use serde::Deserialize;
use sfc_builder::cartridge::{CartridgeLayout, Section};
#[derive(Deserialize)]
struct Resource {
    file: String,
    width: usize,
    height: usize,
    pixels_sha256: String,
}
#[derive(Deserialize)]
struct Step {
    frame: usize,
    hold: u16,
}
#[derive(Deserialize)]
struct Sequence {
    source_va: u32,
    next_va: u32,
    frames: Vec<Step>,
}
#[derive(Deserialize)]
struct Palette {
    rgb: Vec<[u8; 3]>,
    component_bits: u8,
}
pub const CELLS: [usize; 4] = [14, 14, 4, 3];
pub(crate) const SHOWN: u16 = 0x642;
pub(crate) const SHOWN_CONTEXT: u16 = 0x644;
const MP_SEEN: u16 = 0x9a0;
pub(crate) const MP_MAX_SEEN: u16 = 0x9a2;
const BYTES: usize = CELLS[2] * CELLS[3] * 64;
pub(crate) const COLORS: [u8; 16] = [
    0, 0, 32, 34, 35, 43, 184, 185, 186, 187, 188, 189, 190, 191, 128, 160,
];
const SLOTS: [usize; 12] = [1, 2, 5, 6, 9, 10, 4, 7, 0, 3, 8, 11];
const GROUPS: [usize; 4] = [4, 2, 2, 4];
pub fn slot(cell: usize) -> usize {
    SLOTS.iter().position(|&c| c == cell).unwrap()
}
#[derive(Deserialize)]
struct Stone {
    position: [usize; 2],
    size: [usize; 2],
    turn_sequences: Vec<u8>,
    initial_mp: [u16; 2],
    sequences: Vec<Sequence>,
    palettes: Vec<Palette>,
}
#[derive(Deserialize)]
struct Definition {
    schema: u8,
    source_sha256: String,
    resources: Vec<Resource>,
    stone: Stone,
}
pub struct Assets {
    sequences: Vec<u8>,
    lengths: Vec<u8>,
    next: Vec<u8>,
    turns: Vec<u8>,
    initial_mp: [u16; 2],
    frames: Vec<Vec<u8>>,
    palettes: Vec<u8>,
}
impl Assets {
    pub fn load(
        content: &crate::content::Content,
        frame: &crate::sfc_b1_frame::Assets,
    ) -> Result<Self> {
        let root = content.sub("hud");
        let def: Definition = root.json("hud.json")?;
        let pixels = root.read("JISHAKU.CNS.indices")?;
        let resource = def
            .resources
            .iter()
            .find(|r| r.file == "JISHAKU.CNS")
            .ok_or("Missing stone resource")?;
        if def.schema != 1
            || def.source_sha256 != SOURCE
            || resource.width != 480
            || resource.height != 240
            || pixels.len() != 480 * 240
            || sha(&pixels) != resource.pixels_sha256
            || pixels.iter().any(|&v| v >= 16)
            || def.stone.sequences.len() != 12
            || def.stone.palettes.len() != 4
        {
            return Err("Invalid source direction stone".into());
        }
        let mut frames = Vec::new();
        if COLORS[2..].iter().any(|&c| frame.draws(usize::from(c))) {
            return Err("Direction-stone colors overlap frame artwork".into());
        }
        let mut sequences = Vec::new();
        let mut lengths = Vec::new();
        let mut next = Vec::new();
        for (i, sequence) in def.stone.sequences.iter().enumerate() {
            let expected = if i % 3 == 0 { (8, 2) } else { (6, 1) };
            if sequence.frames.len() != expected.0
                || sequence
                    .frames
                    .iter()
                    .any(|s| s.frame >= 72 || s.hold != expected.1)
            {
                return Err("Invalid stone sequence".into());
            }
            let mut ticks: Vec<u8> = sequence
                .frames
                .iter()
                .flat_map(|s| std::iter::repeat_n(s.frame as u8, usize::from(s.hold)))
                .collect();
            lengths.push(ticks.len() as u8);
            ticks.resize(16, 0);
            sequences.extend(ticks);
            let target = def
                .stone
                .sequences
                .iter()
                .position(|s| s.source_va == sequence.next_va)
                .ok_or("Unknown stone sequence continuation")?;
            if target != i / 3 * 3 {
                return Err("Invalid stone resting continuation".into());
            }
            next.push(target as u8);
        }
        if def.stone.turn_sequences.len() != 16 || def.stone.turn_sequences.iter().any(|&v| v >= 12)
        {
            return Err("Invalid stone turn selection".into());
        }
        let [px, py] = def.stone.position;
        let [pw, ph] = def.stone.size;
        let [cx, cy, cw, chh] = CELLS;
        let mut covered = false;
        let mut compose = |index: Option<usize>| -> Result<Vec<u8>> {
            let mut tiles = vec![0; BYTES];
            for y in 0..chh * 8 {
                for x in 0..cw * 8 {
                    let (sx, sy) = (cx * 8 + x, cy * 8 + y);
                    let [ox, oy] = crate::sfc_b1_frame::source(sx, sy);
                    let mut color = frame.background(sx, sy);
                    if (px..px + pw).contains(&ox) && (py..py + ph).contains(&oy) {
                        covered = true;
                        if let Some(index) = index {
                            let local = pixels
                                [(index / 12 * 40 + oy - py) * 480 + index % 12 * 40 + ox - px];
                            if local != 0 {
                                color = COLORS[usize::from(local)];
                            }
                        }
                    }
                    for plane in 0..8 {
                        tiles
                            [(y / 8 * cw + x / 8) * 64 + plane / 2 * 16 + y % 8 * 2 + plane % 2] |=
                            ((color >> plane) & 1) << (7 - x % 8);
                    }
                }
            }
            Ok(tiles)
        };
        for index in 0..72 {
            frames.push(compose(Some(index))?);
        }
        frames.push(compose(None)?);
        if !covered || pw != 40 || ph != 40 {
            return Err("Direction stone outside its SFC cells".into());
        }
        let mut palettes = Vec::new();
        for palette in def.stone.palettes {
            if palette.component_bits != 4
                || palette.rgb.len() != 16
                || palette.rgb[1] != [0, 0, 0]
                || palette.rgb.iter().flatten().any(|&v| v > 15)
            {
                return Err("Invalid source MP palette".into());
            }
            for [r, g, b] in palette.rgb {
                let value =
                    (u16::from(r) * 2) | ((u16::from(g) * 2) << 5) | ((u16::from(b) * 2) << 10);
                palettes.extend(value.to_le_bytes());
            }
        }
        Ok(Self {
            sequences,
            lengths,
            next,
            turns: def.stone.turn_sequences,
            frames,
            palettes,
            initial_mp: def.stone.initial_mp,
        })
    }
    pub fn append(
        &self,
        sections: &mut Vec<Section>,
        space: &mut crate::sfc_rom::Space,
    ) -> Result<()> {
        super::sfc_b1::add(
            sections,
            space,
            "b1_initial_mp".into(),
            self.initial_mp
                .iter()
                .flat_map(|v| v.to_le_bytes())
                .collect(),
        )?;
        for (name, bytes) in [
            ("sequences", &self.sequences),
            ("lengths", &self.lengths),
            ("next", &self.next),
            ("turns", &self.turns),
        ] {
            super::sfc_b1::add(sections, space, format!("b1_stone_{name}"), bytes.clone())?;
        }
        let mut addresses = Vec::new();
        for (i, frame) in self.frames.iter().enumerate() {
            let slots: Vec<u8> = SLOTS
                .iter()
                .flat_map(|&c| frame[c * 64..][..64].to_vec())
                .collect();
            let mut at = 0;
            for (g, n) in GROUPS.iter().enumerate() {
                let bytes = slots[at..at + n * 64].to_vec();
                at += n * 64;
                let source =
                    super::sfc_b1::add_shared(sections, space, format!("b1_stone_{g}_{i}"), bytes)?;
                addresses.extend(source.to_le_bytes());
            }
        }
        super::sfc_b1::add(sections, space, "b1_stone_addresses".into(), addresses)?;
        super::sfc_b1::add(
            sections,
            space,
            "b1_stone_palettes".into(),
            self.palettes.clone(),
        )?;
        let mut writes = Vec::new();
        for palette in self.palettes.as_chunks::<32>().0 {
            let start = writes.len();
            for (source, &target) in COLORS.iter().enumerate().skip(2) {
                writes.extend([target, palette[source * 2], palette[source * 2 + 1]]);
            }
            writes.resize(start + 64, 0);
        }
        super::sfc_b1::add(sections, space, "b1_stone_palette_writes".into(), writes)?;
        Ok(())
    }
}
pub fn emit(p: &mut w65c816::Assembler, layout: &CartridgeLayout) -> Result<()> {
    use crate::sfc::{emit, implied, write};
    use w65c816::{AddressingMode as A, CpuMode, Mnemonic as M, Operand as O};
    let abs = |p: &mut w65c816::Assembler, m, at| emit(p, m, A::Absolute, O::Word(at));
    let addresses = layout.cpu_address(layout.section("b1_stone_addresses")?.offset)?;
    let palettes = layout.cpu_address(layout.section("b1_stone_palette_writes")?.offset)?;
    p.label("b1_stone_mp").assume_mode(CpuMode::NATIVE_M8_X16);
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    abs(p, M::Lda, crate::sfc_battle::MP);
    abs(p, M::Cmp, MP_SEEN);
    p.emit_label_ref(M::Bne, A::Relative8, "stone_mp_changed");
    abs(p, M::Lda, crate::sfc_battle::MP_MAX);
    abs(p, M::Cmp, MP_MAX_SEEN);
    p.emit_label_ref(M::Bne, A::Relative8, "stone_mp_changed");
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    implied(p, M::Rts);
    p.label("stone_mp_changed")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    abs(p, M::Lda, crate::sfc_battle::MP);
    abs(p, M::Sta, MP_SEEN);
    abs(p, M::Lda, crate::sfc_battle::MP_MAX);
    abs(p, M::Sta, MP_MAX_SEEN);
    p.emit_label_ref(M::Bne, A::Relative8, "stone_mp_nonzero_max");
    emit(p, M::Lda, A::Immediate, O::Word(3));
    p.emit_label_ref(M::Bra, A::Relative8, "stone_mp_done");
    p.label("stone_mp_nonzero_max");
    abs(p, M::Lda, crate::sfc_battle::MP);
    abs(p, M::Cmp, crate::sfc_battle::MP_MAX);
    p.emit_label_ref(M::Bcc, A::Relative8, "stone_mp_fraction");
    emit(p, M::Lda, A::Immediate, O::Word(0));
    p.emit_label_ref(M::Bra, A::Relative8, "stone_mp_done");
    p.label("stone_mp_fraction");
    abs(p, M::Stz, 0x64a);
    for _ in 0..3 {
        emit(p, M::Asl, A::Accumulator, O::None);
        abs(p, M::Rol, 0x64a);
    }
    abs(p, M::Sta, 0x648);
    emit(p, M::Ldx, A::Immediate, O::Word(8));
    p.label("stone_mp_divide");
    abs(p, M::Lda, 0x648);
    abs(p, M::Ora, 0x64a);
    p.emit_label_ref(M::Beq, A::Relative8, "stone_mp_quotient");
    implied(p, M::Dex);
    abs(p, M::Lda, 0x648);
    implied(p, M::Sec);
    abs(p, M::Sbc, crate::sfc_battle::MP_MAX);
    abs(p, M::Sta, 0x648);
    abs(p, M::Lda, 0x64a);
    emit(p, M::Sbc, A::Immediate, O::Word(0));
    abs(p, M::Sta, 0x64a);
    p.emit_label_ref(M::Bcs, A::Relative8, "stone_mp_divide");
    p.label("stone_mp_quotient");
    implied(p, M::Txa);
    emit(p, M::Cmp, A::Immediate, O::Word(4));
    p.emit_label_ref(M::Bcc, A::Relative8, "stone_mp_done");
    emit(p, M::Lda, A::Immediate, O::Word(3));
    p.label("stone_mp_done");
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    abs(p, M::Sta, 0x641);
    implied(p, M::Rts);
    p.label("b1_stone_tick").assume_mode(CpuMode::NATIVE_M8_X16);
    p.emit_label_ref(M::Jsr, A::Absolute, "b1_stone_mp");
    let sequences = layout.cpu_address(layout.section("b1_stone_sequences")?.offset)?;
    let lengths = layout.cpu_address(layout.section("b1_stone_lengths")?.offset)?;
    let next = layout.cpu_address(layout.section("b1_stone_next")?.offset)?;
    let turns = layout.cpu_address(layout.section("b1_stone_turns")?.offset)?;
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    abs(p, M::Lda, crate::sfc_field_events::GLOBALS);
    emit(p, M::And, A::Immediate, O::Word(8));
    p.emit_label_ref(M::Bne, A::Relative8, "stone_visible");
    emit(p, M::Lda, A::Immediate, O::Word(255));
    abs(p, M::Sta, 0x64c);
    emit(p, M::Lda, A::Immediate, O::Word(72));
    abs(p, M::Sta, 0x646);
    p.emit_label_ref(M::Brl, A::Relative16, "stone_upload");
    p.label("stone_visible");
    abs(p, M::Lda, 0x500);
    emit(p, M::And, A::Immediate, O::Word(3));
    abs(p, M::Sta, 0x64e);
    abs(p, M::Lda, 0x64c);
    emit(p, M::And, A::Immediate, O::Word(255));
    emit(p, M::Cmp, A::Immediate, O::Word(4));
    p.emit_label_ref(M::Bcc, A::Relative8, "stone_check_turn");
    abs(p, M::Lda, 0x64e);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Clc);
    abs(p, M::Adc, 0x64e);
    p.emit_label_ref(M::Bra, A::Relative8, "stone_set_sequence");
    p.label("stone_check_turn");
    abs(p, M::Cmp, 0x64e);
    p.emit_label_ref(M::Beq, A::Relative8, "stone_frame");
    for _ in 0..2 {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    abs(p, M::Ora, 0x64e);
    implied(p, M::Tax);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(turns));
    emit(p, M::And, A::Immediate, O::Word(255));
    p.label("stone_set_sequence");
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    abs(p, M::Sta, 0x64d);
    write(p, 0x640, 0);
    abs(p, M::Lda, 0x64e);
    abs(p, M::Sta, 0x64c);
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    p.label("stone_frame");
    abs(p, M::Lda, 0x64d);
    emit(p, M::And, A::Immediate, O::Word(255));
    for _ in 0..4 {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    abs(p, M::Sta, 0x64e);
    abs(p, M::Lda, 0x640);
    emit(p, M::And, A::Immediate, O::Word(255));
    abs(p, M::Ora, 0x64e);
    implied(p, M::Tax);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(sequences));
    emit(p, M::And, A::Immediate, O::Word(255));
    abs(p, M::Sta, 0x646);
    abs(p, M::Lda, 0x64d);
    emit(p, M::And, A::Immediate, O::Word(255));
    implied(p, M::Tax);
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    abs(p, M::Inc, 0x640);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(lengths));
    abs(p, M::Cmp, 0x640);
    p.emit_label_ref(M::Bne, A::Relative8, "stone_continue_sequence");
    write(p, 0x640, 0);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(next));
    abs(p, M::Sta, 0x64d);
    p.label("stone_continue_sequence");
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    p.label("stone_upload");
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    abs(p, M::Lda, crate::sfc_b1_text::STONE_SKIP);
    p.emit_label_ref(M::Beq, A::Relative8, "stone_upload_now");
    abs(p, M::Stz, crate::sfc_b1_text::STONE_SKIP);
    implied(p, M::Rts);
    p.label("stone_upload_now");
    abs(p, M::Lda, crate::sfc_b1_menu::MODE);
    abs(p, M::Sta, 0x645);
    for (at, bit) in [
        (crate::sfc_battle_field::ACTIVE, 0x80u8),
        (crate::sfc_b1_map::MAP_EVENT, 0x40),
        (crate::sfc_battle_vm::SCENE, 0x20),
    ] {
        let label = format!("stone_context_{bit:02x}");
        abs(p, M::Lda, at);
        p.emit_label_ref(M::Beq, A::Relative8, &label);
        abs(p, M::Lda, 0x645);
        emit(p, M::Ora, A::Immediate, O::Byte(bit));
        abs(p, M::Sta, 0x645);
        p.label(&label);
    }
    abs(p, M::Lda, 0x645);
    abs(p, M::Cmp, SHOWN_CONTEXT);
    p.emit_label_ref(M::Bne, A::Relative8, "stone_tiles_send");
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    abs(p, M::Lda, 0x646);
    abs(p, M::Cmp, SHOWN);
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    p.emit_label_ref(M::Bne, A::Relative8, "stone_tiles_send");
    p.emit_label_ref(M::Jmp, A::Absolute, "stone_palette_send");
    p.label("stone_tiles_send");
    abs(p, M::Lda, 0x645);
    abs(p, M::Sta, SHOWN_CONTEXT);
    write(p, SHOWN, 0xff);
    write(p, SHOWN + 1, 0xff);
    abs(p, M::Lda, 0x4212);
    abs(p, M::Sta, 0x645);
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    abs(p, M::Lda, 0x646);
    for _ in 0..4 {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    implied(p, M::Tax);
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    write(p, 0x2115, 0x80);
    write(p, 0x2116, 0x00);
    write(p, 0x2117, 0x5e);
    write(p, 0x4300, 1);
    write(p, 0x4301, 0x18);
    for (g, n) in GROUPS.iter().enumerate() {
        let entry = addresses + g as u32 * 4;
        emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
        emit(p, M::Lda, A::AbsoluteLongX, O::Long(entry));
        abs(p, M::Sta, 0x4302);
        emit(p, M::Lda, A::Immediate, O::Word((n * 64) as u16));
        abs(p, M::Sta, 0x4305);
        emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
        emit(p, M::Lda, A::AbsoluteLongX, O::Long(entry + 2));
        abs(p, M::Sta, 0x4304);
        write(p, 0x420b, 1);
    }
    abs(p, M::Lda, 0x4212);
    abs(p, M::And, 0x645);
    p.emit_label_ref(M::Bpl, A::Relative8, "stone_palette_send");
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    abs(p, M::Lda, 0x646);
    abs(p, M::Sta, SHOWN);
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    p.label("stone_palette_send");
    p.emit_label_ref(M::Jsr, A::Absolute, "b1_palette_window");
    p.emit_label_ref(M::Bcs, A::Relative8, "stone_palette_room");
    implied(p, M::Rts);
    p.label("stone_palette_room");
    abs(p, M::Lda, 0x641);
    emit(p, M::Cmp, A::Immediate, O::Byte(4));
    p.emit_label_ref(M::Bcc, A::Relative8, "b1_stone_palette_valid");
    emit(p, M::Lda, A::Immediate, O::Byte(3));
    p.label("b1_stone_palette_valid");
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    emit(p, M::And, A::Immediate, O::Word(255));
    for _ in 0..6 {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    implied(p, M::Tax);
    emit(p, M::Ldy, A::Immediate, O::Word(14));
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    p.label("stone_palette_color");
    for register in [0x2121, 0x2122, 0x2122] {
        emit(p, M::Lda, A::AbsoluteLongX, O::Long(palettes));
        abs(p, M::Sta, register);
        implied(p, M::Inx);
    }
    implied(p, M::Dey);
    p.emit_label_ref(M::Bne, A::Relative8, "stone_palette_color");
    implied(p, M::Rts);
    Ok(())
}

pub(crate) fn emit_palette_window(p: &mut w65c816::Assembler) {
    use crate::sfc::{emit, implied};
    use w65c816::{AddressingMode as A, CpuMode, Mnemonic as M, Operand as O};
    p.label("b1_palette_window")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    emit(p, M::Lda, A::Absolute, O::Word(0x4212));
    p.emit_label_ref(M::Bpl, A::Relative8, "b1_palette_late");
    for register in [0x213f, 0x2137, 0x213d, 0x213d] {
        emit(p, M::Lda, A::Absolute, O::Word(register));
    }
    emit(p, M::And, A::Immediate, O::Byte(1));
    p.emit_label_ref(M::Bne, A::Relative8, "b1_palette_late");
    implied(p, M::Sec);
    implied(p, M::Rts);
    p.label("b1_palette_late");
    implied(p, M::Clc);
    implied(p, M::Rts);
}
