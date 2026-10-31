use crate::{
    Result,
    font::PortFont,
    sfc::{emit, implied, startup, write},
    sha,
};
use sfc_builder::{
    background::{Bg1, Map, Mode},
    cartridge::{CartridgeLayout, Section},
    glyph::{Depth, GlyphAtlas},
    runtime,
    text::TextWindow,
    transfer::{Destination, TransferPlan},
};
use w65c816::{AddressingMode as A, Assembler, CodeLocation, CpuMode, Mnemonic as M, Operand as O};
pub(crate) fn set_wait(p: &mut Assembler, word: u16) {
    write(p, 0x204, word as u8);
    write(p, 0x205, ((word & 0x7fff) >> 8) as u8);
    write(p, 0x208, u8::from(word & 0x8000 != 0));
}
pub(crate) fn emit_timed_tick(
    p: &mut Assembler,
    pad: runtime::PadStorage,
    count: usize,
    catalog: bool,
) {
    p.label("timed_tick").assume_mode(CpuMode::NATIVE_M8_X16);
    emit(p, M::Lda, A::Absolute, O::Word(0x207));
    p.emit_label_ref(M::Beq, A::Relative8, "not_finished");
    implied(p, M::Rts);
    p.label("not_finished");
    emit(p, M::Lda, A::Absolute, O::Word(0x200));
    if catalog {
        emit(p, M::Cmp, A::Absolute, O::Word(0x209));
    } else {
        emit(p, M::Cmp, A::Immediate, O::Byte(count as u8));
    }
    p.emit_label_ref(M::Bne, A::Relative8, "waiting");
    write(p, 0x207, 1);
    if catalog {
        emit(p, M::Lda, A::Absolute, O::Word(0x20b));
        emit(p, M::Sta, A::Absolute, O::Word(0x20d));
    }
    implied(p, M::Rts);
    p.label("waiting");
    emit(p, M::Lda, A::Absolute, O::Word(0x206));
    p.emit_label_ref(M::Bne, A::Relative8, "advance");
    emit(p, M::Lda, A::Absolute, O::Word(0x208));
    p.emit_label_ref(M::Beq, A::Relative8, "countdown");
    emit(p, M::Lda, A::Absolute, O::Word(pad.pressed()));
    emit(p, M::And, A::Immediate, O::Byte(0x80));
    p.emit_label_ref(M::Beq, A::Relative8, "countdown");
    write(p, 0x204, 0);
    write(p, 0x205, 0);
    write(p, 0x206, 1);
    implied(p, M::Rts);
    p.label("countdown");
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    emit(p, M::Dec, A::Absolute, O::Word(0x204));
    p.emit_label_ref(M::Beq, A::Relative8, "elapsed");
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    implied(p, M::Rts);
    p.label("elapsed").assume_mode(CpuMode::NATIVE_M16_X16);
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    p.label("advance");
    write(p, 0x206, 0);
    emit(p, M::Inc, A::Absolute, O::Word(0x200));
    if catalog {
        emit(p, M::Lda, A::Absolute, O::Word(0x200));
        emit(p, M::Cmp, A::Immediate, O::Byte(1));
        p.emit_label_ref(M::Bcc, A::Relative8, "invalid_page");
        emit(p, M::Cmp, A::Immediate, O::Byte((count + 1) as u8));
        p.emit_label_ref(M::Bcs, A::Relative8, "invalid_page");
        emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
        emit(p, M::And, A::Immediate, O::Word(0xff));
        emit(p, M::Asl, A::Accumulator, O::None);
        implied(p, M::Tax);
        emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
        p.emit_label_ref(M::Jmp, A::AbsoluteIndexedIndirectX, "page_loaders");
        p.label("invalid_page");
    } else {
        for index in 1..=count {
            emit(p, M::Lda, A::Absolute, O::Word(0x200));
            emit(p, M::Cmp, A::Immediate, O::Byte(index as u8));
            p.emit_label_ref(M::Bne, A::Relative8, format!("not_page_{index}"));
            p.emit_label_ref(M::Jsr, A::Absolute, format!("load_{index}"));
            implied(p, M::Rts);
            p.label(format!("not_page_{index}"));
        }
    }
    write(p, 0x207, 0xff);
    implied(p, M::Rts);
}
pub fn build(pages: &[String], font: &PortFont) -> Result<Vec<(String, Vec<u8>)>> {
    build_profile(pages, font, None, "b1-message")
}
pub fn build_timed(
    pages: &[String],
    waits: &[u16],
    font: &PortFont,
) -> Result<Vec<(String, Vec<u8>)>> {
    if waits.len() != pages.len() || waits.iter().any(|w| w & 0x7fff == 0) {
        return Err("Invalid timed message waits".into());
    }
    build_profile(pages, font, Some(waits), "prologue-message")
}
fn build_profile(
    pages: &[String],
    font: &PortFont,
    waits: Option<&[u16]>,
    prefix: &str,
) -> Result<Vec<(String, Vec<u8>)>> {
    if (waits.is_none() && pages.len() != 2)
        || pages.is_empty()
        || pages.len() > 254
        || pages.iter().any(|p| p.chars().count() > 80)
    {
        return Err("Invalid entry message pages".into());
    }
    let characters: std::collections::BTreeSet<_> = pages.iter().flat_map(|p| p.chars()).collect();
    let entries = characters
        .iter()
        .map(|ch| Ok((*ch, font.mask16(*ch)?.to_vec())))
        .collect::<Result<Vec<_>>>()?;
    let atlas = GlyphAtlas::sized(&entries, 16, 16, 1, Depth::Four)?;
    let mut tiles = vec![0; 32];
    tiles.extend_from_slice(atlas.bytes());
    let tile_count = tiles.len() / 32;
    let mut sections = vec![Section {
        name: "glyphs".into(),
        offset: 0x8000,
        bytes: tiles,
    }];
    let mut cursor = 0x8000 + sections[0].bytes.len();
    let mut files = Vec::new();
    let mut expected = Vec::new();
    for (index, text) in pages
        .iter()
        .map(String::as_str)
        .chain(std::iter::once(""))
        .enumerate()
    {
        let mut window = TextWindow::new(32, 32, 0)?;
        let mut pixels = vec![0u8; 256 * 224];
        for (i, ch) in text.chars().enumerate() {
            let x = i % 16 * 2;
            let y = 18 + i / 16 * 2;
            window.write(atlas.mapping(), x, y, &ch.to_string())?;
            for (p, ink) in font.mask16(ch)?.iter().enumerate() {
                pixels[(y * 8 + p / 16) * 256 + x * 8 + p % 16] = *ink;
            }
        }
        let bytes: Vec<_> = window
            .cells()
            .iter()
            .flat_map(|w| w.to_le_bytes())
            .collect();
        let map_hash = sha(&bytes);
        sections.push(Section {
            name: format!("page_{index}"),
            offset: cursor,
            bytes,
        });
        cursor += 2048;
        let rgb: Vec<u8> = pixels
            .iter()
            .flat_map(|p| [if *p == 0 { 0 } else { 255 }; 3])
            .collect();
        expected.push(serde_json::json!(
            { "page" : index, "characters" : text.chars().count(), "text" : text,
            "map_sha256" : map_hash, "rgb_sha256" : sha(& rgb) }
        ));
        files.push((format!("{prefix}-{index}.rgb"), rgb));
    }
    let mut palette = vec![0; 512];
    palette[2..4].copy_from_slice(&0x7fffu16.to_le_bytes());
    sections.push(Section {
        name: "palette".into(),
        offset: cursor,
        bytes: palette,
    });
    cursor += 512;
    let layout = CartridgeLayout::new(cursor.next_power_of_two(), 0x7000, sections)?;
    let bg = Bg1::new(Mode::One, 0, tile_count, Map::new(0xf000, 32, 32)?)?;
    let pad = runtime::PadStorage::new(0x300)?;
    let mut p = Assembler::new();
    startup(&mut p);
    bg.emit_configure(&mut p);
    for (name, bytes, destination) in [
        ("glyphs", tile_count * 32, Destination::Vram(0)),
        ("palette", 512, Destination::Cgram(0)),
        ("page_0", 2048, Destination::Vram(0xf000)),
    ] {
        for chunk in TransferPlan::new(&layout, name, 0, bytes, destination, 8192)?.chunks() {
            chunk.emit(&mut p, 0)?;
        }
    }
    if let Some(waits) = waits {
        set_wait(&mut p, waits[0]);
        write(&mut p, 0x206, 0);
        write(&mut p, 0x207, 0);
    }
    write(&mut p, 0x212c, 1);
    write(&mut p, 0x2100, 15);
    write(&mut p, 0x201, 0x42);
    emit(&mut p, M::Lda, A::Absolute, O::Word(0x4210));
    write(&mut p, 0x4200, 0x81);
    p.label("idle");
    implied(&mut p, M::Wai);
    p.emit_label_ref(M::Bra, A::Relative8, "idle");
    p.label("nmi");
    runtime::emit_interrupt_entry(&mut p);
    emit(&mut p, M::Lda, A::Absolute, O::Word(0x4210));
    emit(&mut p, M::Inc, A::Absolute, O::Word(0x202));
    runtime::emit_pad_call(&mut p, "sample", "done");
    if waits.is_some() {
        p.emit_label_ref(M::Jsr, A::Absolute, "timed_tick");
    } else {
        emit(&mut p, M::Lda, A::Absolute, O::Word(pad.pressed()));
        emit(&mut p, M::And, A::Immediate, O::Byte(0x80));
        p.emit_label_ref(M::Beq, A::Relative8, "done");
        emit(&mut p, M::Lda, A::Absolute, O::Word(0x200));
        emit(&mut p, M::Cmp, A::Immediate, O::Byte(2));
        p.emit_label_ref(M::Bcs, A::Relative8, "done");
        emit(&mut p, M::Inc, A::Absolute, O::Word(0x200));
        emit(&mut p, M::Cmp, A::Immediate, O::Byte(0));
        p.emit_label_ref(M::Bne, A::Relative8, "clear");
        p.emit_label_ref(M::Jsr, A::Absolute, "load_1");
        p.emit_label_ref(M::Bra, A::Relative8, "done");
        p.label("clear");
        p.emit_label_ref(M::Jsr, A::Absolute, "load_2");
    }
    p.label("done");
    runtime::emit_interrupt_exit(&mut p);
    runtime::emit_auto_pad(&mut p, pad, "sample");
    if waits.is_some() {
        emit_timed_tick(&mut p, pad, pages.len(), false);
    }
    for index in 1..=pages.len() {
        p.label(format!("load_{index}"))
            .assume_mode(CpuMode::NATIVE_M8_X16);
        for chunk in TransferPlan::new(
            &layout,
            &format!("page_{index}"),
            0,
            2048,
            Destination::Vram(0xf000),
            8192,
        )?
        .chunks()
        {
            chunk.emit(&mut p, 0)?;
        }
        if let Some(waits) = waits {
            set_wait(&mut p, waits.get(index).copied().unwrap_or(0));
        }
        implied(&mut p, M::Rts);
    }
    p.label("irq").assume_mode(CpuMode::NATIVE_M8_X16);
    implied(&mut p, M::Rti);
    let assembled =
        crate::sfc_width_check::assemble(&p, CodeLocation::new(0, 0x8000), CpuMode::Emulation)?;
    let addr = |name| assembled.label_location(name).unwrap().program_counter();
    let rom = layout.build(assembled.bytes(), addr("reset"), addr("nmi"), addr("irq"))?;
    let report = serde_json::json!(
        { "scope" :
        "B1 entry message 0, event 17: two input waits then clear; no B1 movement",
        "rom_sha256" : sha(& rom), "rom_bytes" : rom.len(), "code_bytes" : assembled
        .bytes().len(), "toolchain_revision" :
        "4871594b3731aa495381de2bcef00f75328a0368", "glyphs" : atlas.report(), "sections"
        : layout.section_report(), "pages" : expected, "wram" : { "page" : 512, "ready" :
        513, "nmi_counter" : 514, "pad" : 768 }, "limits" :
        ["Standalone text consumer, not dungeon or complete prologue.",
        "Source rows reflowed to 16 columns; font is a port candidate.",
        "Map upload timing requires native observation; no hardware deadline claim."] }
    );
    let mut report = report;
    if let Some(waits) = waits {
        report["scope"] =
            serde_json::json!("Prologue message 3: timed text consumer; not complete narrative");
        report["wait_words"] = serde_json::json!(waits);
        report["wram"]["remaining"] = 0x204.into();
        report["wram"]["skip_pending"] = 0x206.into();
        report["wram"]["finished"] = 0x207.into();
        report["limits"] = serde_json::json!([
            "One source message callback per NMI is a port timing policy, not measured original wall-clock timing.",
            "A rising edge supplies the source skip signal; skip yields once before advancing.",
            "White immediate text profile only; colored text, voice and root scheduler remain pending.",
            "Standalone text consumer; no artwork, dungeon or game-event return."
        ]);
    }
    files.push((format!("{prefix}.sfc"), rom));
    files.push((
        format!("{prefix}-build.json"),
        serde_json::to_vec_pretty(&report)?,
    ));
    Ok(files)
}
