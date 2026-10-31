use crate::{Result, sha};
use sfc_builder::{
    background::{Bg1, Map, Mode},
    cartridge::{CartridgeLayout, Section},
    runtime,
    tiles::{Attributes, Depth, IndexedImage, TileImage},
    transfer::{Destination, TransferPlan},
};
use w65c816::{
    AddressingMode as A, Assembler, CodeLocation, CpuMode, Instruction, Mnemonic as M, Operand as O,
};
pub struct Frame {
    pub tick: usize,
    pub pixels: Vec<u8>,
    pub palette: Vec<u8>,
    pub text: String,
}
pub struct Output {
    pub files: Vec<(String, Vec<u8>)>,
    pub report: serde_json::Value,
}
pub(crate) fn source_y_to_screen(offset: i64) -> i16 {
    let offset = i128::from(offset);
    let lines = (offset * 7 + offset.signum() * 7) / 15;
    lines.clamp(i128::from(i16::MIN), i128::from(i16::MAX)) as i16
}
const PROLOGUE_TEXT: [usize; 2] = [8, 146];
const PROLOGUE_PITCH: usize = 12;
pub const PROLOGUE_WIDTH: usize = 240;
pub const PROLOGUE_LINES: usize = 5;
use crate::b1_dialogue::{color_mark, mark_color};
pub fn prologue_layout<'a>(
    font: &'a crate::font::DialogueFont,
    joins: &'a [crate::b1_dialogue::Join],
) -> crate::b1_dialogue::PageLayout<'a> {
    crate::b1_dialogue::PageLayout {
        width: PROLOGUE_WIDTH,
        lines: PROLOGUE_LINES,
        advance: Box::new(move |ch| {
            if mark_color(ch).is_some() {
                Ok(0)
            } else {
                font.advance(ch)
            }
        }),
        joins,
        used: Default::default(),
    }
}
pub fn layout(
    tick: usize,
    source: &[u8],
    palette: &[u8],
    text: Option<&crate::text_surface::TextSurface>,
    font: &crate::font::DialogueFont,
    lines: &crate::b1_dialogue::PageLayout,
) -> Result<Frame> {
    if source.len() != 640 * 480 || palette.len() != 1024 {
        return Err("Invalid SFC layout source".into());
    }
    let mut pixels = vec![16; 256 * 224];
    for y in 0..144 {
        for x in 0..241 {
            pixels[y * 256 + x + 7] = source[(y * 382 / 144) * 640 + x * 640 / 241];
        }
    }
    let mut rows = Vec::new();
    let mut current = 0;
    for row in 0..4 {
        let Some(text) = text else { break };
        let Some(last) = text
            .cells
            .keys()
            .filter(|(r, _)| *r == row)
            .map(|(_, c)| *c)
            .max()
        else {
            continue;
        };
        let mut line = String::new();
        for col in 0..=last {
            let (ch, color) = *text.cells.get(&(row, col)).unwrap_or(&('　', current));
            if color != current && ch != '　' {
                line.push(color_mark(color));
                current = color;
            }
            line.push(ch);
        }
        rows.push(line);
    }
    let pages = lines.pages(&rows.join(&crate::b1_dialogue::SOURCE_BREAK.to_string()))?;
    if pages.len() > 1 {
        return Err("Prologue dialogue exceeds its window".into());
    }
    let mut color = 1;
    let mut visible = String::new();
    for (row, line) in pages.first().map_or("", String::as_str).lines().enumerate() {
        let mut x = PROLOGUE_TEXT[0];
        for ch in line.chars() {
            if let Some(next) = mark_color(ch) {
                color = next;
                continue;
            }
            visible.push(ch);
            let glyph = font.glyph(ch)?;
            for [dx, dy] in &glyph.ink {
                let (px, py) = (x + dx, PROLOGUE_TEXT[1] + row * PROLOGUE_PITCH + dy);
                if px >= PROLOGUE_TEXT[0] + PROLOGUE_WIDTH || py >= 208 {
                    return Err("Prologue ink outside its window".into());
                }
                pixels[py * 256 + px] = color;
            }
            x += glyph.advance;
        }
    }
    Ok(Frame {
        tick,
        pixels,
        palette: palette.to_vec(),
        text: visible,
    })
}
#[derive(Default)]
pub struct Sequence {
    previous: Option<Frame>,
    ticks: Vec<serde_json::Value>,
    peak_bytes: usize,
    peak_text: usize,
    total_bytes: usize,
}
impl Sequence {
    pub fn observe(&mut self, frame: Frame) -> Result<()> {
        if frame.pixels.len() != 256 * 224 || frame.palette.len() != 1024 {
            return Err("Invalid sequence frame extent".into());
        }
        let mut runs: Vec<(usize, usize)> = Vec::new();
        let mut changed = 0;
        for tile in 0..32 * 28 {
            let start = tile / 32 * 8 * 256 + tile % 32 * 8;
            let differs = self.previous.as_ref().is_none_or(|old| {
                (0..8).any(|row| {
                    let offset = start + row * 256;
                    frame.pixels[offset..offset + 8] != old.pixels[offset..offset + 8]
                })
            });
            if differs {
                changed += 1;
                if let Some((first, count)) = runs.last_mut()
                    && *first + *count == tile
                {
                    *count += 1;
                } else {
                    runs.push((tile, 1));
                }
            }
        }
        let palette_changed = self.previous.as_ref().is_none_or(|old| {
            (0..256).any(|i| {
                (0..3).any(|c| frame.palette[i * 4 + c] >> 3 != old.palette[i * 4 + c] >> 3)
            })
        });
        let bytes = changed * 64 + usize::from(palette_changed) * 512;
        self.peak_bytes = self.peak_bytes.max(bytes);
        self.peak_text = self.peak_text.max(frame.text.chars().count());
        self.total_bytes += bytes;
        self.ticks.push(serde_json::json!(
            { "tick" : frame.tick, "changed_tiles" : changed, "tile_runs" : runs,
            "palette_bytes" : if palette_changed { 512 } else { 0 },
            "transfer_bytes" : bytes, "visible_characters" : frame.text.chars()
            .count(), "pixels_sha256" : sha(& frame.pixels) }
        ));
        self.previous = Some(frame);
        Ok(())
    }
    pub fn report(self, policy: &str) -> serde_json::Value {
        serde_json::json!(
            { "policy" : policy, "ticks" : self.ticks, "peak_transfer_bytes" : self
            .peak_bytes, "total_transfer_bytes" : self.total_bytes,
            "peak_visible_characters" : self.peak_text, "tile_storage_bytes" : 256 * 224,
            "map_bytes" : 2048, "map_vram_byte_address" : 0xf000, "scope" :
            "host full-trace layout and fixed-cell change measurement; not guest playback",
            "limits" :
            ["Transfer bytes exclude setup, CPU composition, and map initialization.",
            "One logical source tick is not yet bound to one SFC frame.",
            "Recomposition starts from last fill; original framebuffer retention remains unverified."]
            }
        )
    }
}
pub(crate) fn emit(p: &mut Assembler, m: M, a: A, o: O) {
    p.emit(Instruction::new(m, a, o));
}
pub(crate) fn implied(p: &mut Assembler, m: M) {
    emit(p, m, A::Implied, O::None);
}
pub(crate) fn write(p: &mut Assembler, address: u16, value: u8) {
    emit(p, M::Lda, A::Immediate, O::Byte(value));
    emit(p, M::Sta, A::Absolute, O::Word(address));
}
pub(crate) fn write_word(p: &mut Assembler, address: u16, value: u16) {
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    emit(p, M::Lda, A::Immediate, O::Word(value));
    emit(p, M::Sta, A::Absolute, O::Word(address));
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
}
pub(crate) fn startup(p: &mut Assembler) {
    startup_entered(p, false);
}
pub(crate) fn startup_entered(p: &mut Assembler, fast: bool) {
    p.label("reset");
    for m in [M::Sei, M::Cld, M::Clc, M::Xce] {
        implied(p, m);
    }
    if fast {
        p.assume_mode(CpuMode::NATIVE_M8_X8);
        p.emit_label_ref(M::Jml, A::AbsoluteLong, "reset_fast");
        p.label("reset_fast");
    }
    p.assume_mode(CpuMode::NATIVE_M8_X8);
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x30));
    emit(p, M::Ldx, A::Immediate, O::Word(0x1fff));
    implied(p, M::Txs);
    emit(p, M::Lda, A::Immediate, O::Word(0));
    implied(p, M::Tcd);
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    emit(p, M::Lda, A::Immediate, O::Byte(0));
    implied(p, M::Pha);
    implied(p, M::Plb);
    for (a, v) in [
        (0x4200, 0),
        (0x420b, 0),
        (0x420c, 0),
        (0x2100, 0x80),
        (0x212c, 0),
        (0x212d, 0),
        (0x212e, 0),
        (0x212f, 0),
        (0x2130, 0),
        (0x2131, 0),
        (0x2133, 0),
        (0x420d, 1),
        (0x210d, 0),
        (0x210d, 0),
        (0x210e, 0xff),
        (0x210e, 0xff),
        (0x200, 0),
        (0x201, 0),
        (0x202, 0),
    ] {
        write(p, a, v);
    }
    for a in 0x300..0x30a {
        write(p, a, 0);
    }
}
pub fn build(frames: &[Frame]) -> Result<Option<Output>> {
    if frames.is_empty() {
        return Ok(None);
    }
    if frames.len() != 2 {
        return Err("Display consumer requires two identified scenes".into());
    }
    let mut sections = Vec::new();
    let mut cursor = 0x8000;
    let mut counts = Vec::new();
    let mut receipts = Vec::new();
    let mut files = Vec::new();
    for (index, frame) in frames.iter().enumerate() {
        let attributes = vec![
            Attributes {
                palette: 0,
                priority: false
            };
            32 * 28
        ];
        let image = TileImage::new(
            IndexedImage {
                width: 256,
                height: 224,
                pixels: &frame.pixels,
                attributes: &attributes,
            },
            Depth::Eight,
            0,
        )?;
        let tiles = image.native_tiles().to_vec();
        counts.push(tiles.len());
        let words: Vec<u16> = frame
            .palette
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| {
                u16::from(c[0] >> 3) | (u16::from(c[1] >> 3) << 5) | (u16::from(c[2] >> 3) << 10)
            })
            .collect();
        let palette = sfc_codecs::asset_codec::encode_sfc_cgram_u15(
            &asset_types::CanonicalU15PaletteWords::new(words.clone())?,
        );
        for (name, bytes) in [
            ("tiles", tiles),
            ("map", image.bg_map(32, 32, 0)?),
            ("palette", palette),
        ] {
            let size = bytes.len();
            sections.push(Section {
                name: format!("{name}_{index}"),
                offset: cursor,
                bytes,
            });
            cursor += size;
        }
        let rgb: Vec<u8> = frame
            .pixels
            .iter()
            .flat_map(|i| {
                let w = words[*i as usize];
                [0, 5, 10].map(|s| {
                    let v = ((w >> s) & 31) as u8;
                    (v << 3) | (v >> 2)
                })
            })
            .collect();
        let mut ppm = b"P6\n256 224\n255\n".to_vec();
        ppm.extend(&rgb);
        receipts.push(serde_json::json!(
            { "index" : index, "source_tick" : frame.tick, "text" : frame.text,
            "pixels_sha256" : sha(& frame.pixels), "expected_rgb_sha256" : sha(&
            rgb), "image" : image.report() }
        ));
        files.push((format!("sfc-scene-{index}.ppm"), ppm));
        files.push((format!("sfc-scene-{index}.rgb"), rgb));
    }
    let layout = CartridgeLayout::new(cursor.next_power_of_two(), 0x7000, sections)?;
    let background = Bg1::new(
        Mode::Three,
        0,
        *counts.iter().max().unwrap() / 64,
        Map::new(0xf000, 32, 32)?,
    )?;
    let pad = runtime::PadStorage::new(0x300)?;
    let mut p = Assembler::new();
    startup(&mut p);
    background.emit_configure(&mut p);
    p.emit_label_ref(M::Jsr, A::Absolute, "load_0");
    emit(&mut p, M::Lda, A::Absolute, O::Word(0x4210));
    write(&mut p, 0x4200, 0x81);
    p.label("main");
    implied(&mut p, M::Wai);
    emit(&mut p, M::Lda, A::Absolute, O::Word(pad.pressed()));
    emit(&mut p, M::And, A::Immediate, O::Byte(0x80));
    p.emit_label_ref(M::Beq, A::Relative8, "main");
    emit(&mut p, M::Lda, A::Absolute, O::Word(0x200));
    emit(&mut p, M::Eor, A::Immediate, O::Byte(1));
    emit(&mut p, M::Sta, A::Absolute, O::Word(0x200));
    p.emit_label_ref(M::Bne, A::Relative8, "select_1");
    p.emit_label_ref(M::Jsr, A::Absolute, "load_0");
    p.emit_label_ref(M::Bra, A::Relative8, "main");
    p.label("select_1");
    p.emit_label_ref(M::Jsr, A::Absolute, "load_1");
    p.emit_label_ref(M::Bra, A::Relative8, "main");
    for (index, bytes) in counts.iter().enumerate() {
        p.label(format!("load_{index}"));
        write(&mut p, 0x2100, 0x80);
        write(&mut p, 0x201, 0);
        for (name, size, destination) in [
            ("tiles", *bytes, Destination::Vram(0)),
            ("map", 2048, Destination::Vram(0xf000)),
            ("palette", 512, Destination::Cgram(0)),
        ] {
            let plan = TransferPlan::new(
                &layout,
                &format!("{name}_{index}"),
                0,
                size,
                destination,
                8192,
            )?;
            for chunk in plan.chunks() {
                chunk.emit(&mut p, 0)?;
            }
        }
        write(&mut p, 0x212c, 1);
        write(&mut p, 0x2100, 15);
        write(&mut p, 0x201, 0x42);
        implied(&mut p, M::Rts);
    }
    p.label("nmi");
    runtime::emit_interrupt_entry(&mut p);
    emit(&mut p, M::Lda, A::Absolute, O::Word(0x4210));
    emit(&mut p, M::Inc, A::Absolute, O::Word(0x202));
    runtime::emit_pad_call(&mut p, "sample", "nmi_done");
    p.label("nmi_done");
    runtime::emit_interrupt_exit(&mut p);
    runtime::emit_auto_pad(&mut p, pad, "sample");
    p.label("irq").assume_mode(CpuMode::NATIVE_M8_X16);
    implied(&mut p, M::Rti);
    let assembled =
        crate::sfc_width_check::assemble(&p, CodeLocation::new(0, 0x8000), CpuMode::Emulation)?;
    let addr = |label| assembled.label_location(label).unwrap().program_counter();
    let rom = layout.build(assembled.bytes(), addr("reset"), addr("nmi"), addr("irq"))?;
    let report = serde_json::json!(
        { "scope" : "two sampled prologue scenes; A toggles; not the game VM or B1",
        "toolchain_revision" : "4871594b3731aa495381de2bcef00f75328a0368", "rom_sha256" :
        sha(& rom), "rom_bytes" : rom.len(), "code_bytes" : assembled.bytes().len(),
        "sections" : layout.section_report(), "frames" : receipts, "wram" : { "selected"
        : 512, "ready" : 513, "nmi_counter" : 514, "pad" : 768 }, "transfer_policy" :
        "forced blank; immutable ROM DMA; no seamless transition or worst-case deadline claim",
        "instructions" : assembled.instruction_spans().iter().zip(assembled
        .instruction_width_states()).map(| (s, w) | serde_json::json!({ "address" : s
        .location.to_u32(), "bytes" : s.bytes.as_slice(), "widths" : format!("{:?}", w
        .widths) })).collect::< Vec < _ >> () }
    );
    files.push(("prologue-display.sfc".into(), rom));
    files.push(("sfc-build.json".into(), serde_json::to_vec_pretty(&report)?));
    Ok(Some(Output { files, report }))
}
