use crate::{
    Result,
    font::PortFont,
    messages::{TimedPage, TimedProgram},
    sfc::{emit, implied, startup, write},
    sfc_message::{emit_timed_tick, set_wait},
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
const TEXT_MAP_OFFSET: usize = 18 * 32 * 2;
pub fn build(
    programs: &[TimedProgram],
    colors: &[[u8; 3]],
    font: &PortFont,
    artwork: Option<&crate::sfc_layers::Artwork>,
    root: Option<&crate::sfc_root_message::Bridge>,
    objects: Option<&crate::sfc_object_vm::Program>,
    opening: bool,
) -> Result<Vec<(String, Vec<u8>)>> {
    if opening && (artwork.is_none() || root.is_some() || objects.is_some() || programs.len() != 2)
    {
        return Err("Opening requires two dialogue entries and resident artwork".into());
    }
    if objects.is_some() && (artwork.is_none() || root.is_some()) {
        return Err("Object scene requires artwork and exclusive scheduler ownership".into());
    }
    if programs.is_empty() || colors.len() != 6 {
        return Err("Missing message catalog or source text palette".into());
    }
    let mut ids = std::collections::BTreeSet::new();
    let mut states = Vec::new();
    let mut entries = Vec::new();
    for program in programs {
        if !(1..=255).contains(&program.id) || !ids.insert(program.id) || program.pages.is_empty() {
            return Err("Invalid message service ID".into());
        }
        let first = states.len();
        for page in &program.pages {
            if page.wait & 0x7fff == 0
                || page.cells.len() > 80
                || page.cells.iter().any(|(_, c)| *c >= 6)
            {
                return Err("Invalid message service page".into());
            }
            states.push(page.clone());
        }
        let end = states.len();
        states.push(TimedPage {
            cells: Vec::new(),
            wait: 0,
        });
        entries.push((program.id as u8, first, end));
    }
    if states.len() > 255 || !ids.contains(&1) {
        return Err("Message service requires entry 1 and at most 255 states".into());
    }
    let text_map_bytes = states
        .iter()
        .map(|page| page.cells.len())
        .max()
        .unwrap()
        .div_ceil(16)
        .max(1)
        * 2
        * 32
        * 2;
    let pairs: std::collections::BTreeSet<_> = states
        .iter()
        .flat_map(|p| p.cells.iter().copied())
        .collect();
    let keys: std::collections::BTreeMap<_, _> = pairs
        .iter()
        .enumerate()
        .map(|(i, pair)| {
            char::from_u32(0xf0000 + i as u32)
                .map(|key| (*pair, key))
                .ok_or("Colored glyph key space exhausted")
        })
        .collect::<std::result::Result<_, _>>()?;
    let glyphs = keys
        .iter()
        .map(|((ch, color), key)| {
            Ok((
                *key,
                font.mask16(*ch)?
                    .iter()
                    .map(|ink| if *ink == 0 { 0 } else { color + 1 })
                    .collect(),
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    let atlas = GlyphAtlas::sized(&glyphs, 16, 16, 1, Depth::Four)?;
    let mut tiles = vec![0; 32];
    tiles.extend_from_slice(atlas.bytes());
    let tile_count = tiles.len() / 32;
    let prefix = if opening {
        "prologue-opening"
    } else if objects.is_some() {
        "object-scene"
    } else if root.is_some() {
        "root-message"
    } else if artwork.is_some() {
        "scene-message"
    } else {
        "message-catalog"
    };
    let glyph_address = if artwork.is_some() { 0xa000 } else { 0 };
    let mut sections = Vec::new();
    let mut cursor = 0x8000;
    let mut add = |name: String, bytes: Vec<u8>| {
        let len = bytes.len();
        sections.push(Section {
            name,
            offset: cursor,
            bytes,
        });
        cursor += len;
    };
    add(
        "glyphs".into(),
        if artwork.is_some() {
            vec![0; 32]
        } else {
            tiles
        },
    );
    let mut palette = artwork.map_or_else(|| vec![0; 512], |a| a.palette.clone());
    let mut target_colors = Vec::new();
    for (index, color) in colors.iter().enumerate() {
        let word = u16::from(color[0] >> 3)
            | u16::from(color[1] >> 3) << 5
            | u16::from(color[2] >> 3) << 10;
        palette[(index + 1) * 2..(index + 1) * 2 + 2].copy_from_slice(&word.to_le_bytes());
        target_colors.push(color.map(|c| {
            let v = c >> 3;
            (v << 3) | (v >> 2)
        }));
    }
    add("palette".into(), palette);
    add("idle_map".into(), vec![0; 2048]);
    let mut files = Vec::new();
    let mut state_reports = Vec::new();
    let mut page_tile_bytes = Vec::new();
    for (index, page) in states.iter().enumerate() {
        let local_entries: Vec<_> = glyphs
            .iter()
            .filter(|(key, _)| page.cells.iter().any(|pair| keys[pair] == *key))
            .cloned()
            .collect();
        let local_atlas = if artwork.is_some() && !local_entries.is_empty() {
            Some(GlyphAtlas::sized(&local_entries, 16, 16, 1, Depth::Four)?)
        } else {
            None
        };
        if artwork.is_some() {
            let mut data = vec![0; 32];
            if let Some(local) = &local_atlas {
                data.extend_from_slice(local.bytes());
            }
            if data.len() > 8192 {
                return Err("Page glyphs exceed 8 KiB residency".into());
            }
            page_tile_bytes.push(data.len());
            add(format!("page_glyphs_{index}"), data);
        }
        let page_atlas = local_atlas.as_ref().unwrap_or(&atlas);
        let mut window = TextWindow::new(32, 32, 0)?;
        let mut rgb = artwork.map_or_else(|| vec![0u8; 256 * 224 * 3], |a| a.rgb.clone());
        for (i, (ch, color)) in page.cells.iter().enumerate() {
            let x = i % 16 * 2;
            let y = 18 + i / 16 * 2;
            window.write(
                page_atlas.mapping(),
                x,
                y,
                &keys[&(*ch, *color)].to_string(),
            )?;
            for (p, ink) in font.mask16(*ch)?.iter().enumerate() {
                if *ink != 0 {
                    let offset = ((y * 8 + p / 16) * 256 + x * 8 + p % 16) * 3;
                    rgb[offset..offset + 3].copy_from_slice(&target_colors[*color as usize]);
                }
            }
        }
        let map: Vec<_> = window
            .cells()
            .iter()
            .flat_map(|tile| tile.to_le_bytes())
            .collect();
        if map[..TEXT_MAP_OFFSET]
            .iter()
            .chain(&map[TEXT_MAP_OFFSET + text_map_bytes..])
            .any(|byte| *byte != 0)
        {
            return Err("Dialogue tile outside the page DMA window".into());
        }
        state_reports.push(serde_json::json!(
            { "state" : index, "wait" : page.wait, "cells" : page.cells,
            "map_sha256" : sha(& map), "rgb_sha256" : sha(& rgb) }
        ));
        add(format!("page_{index}"), map);
        files.push((format!("{prefix}-{index}.rgb"), rgb));
    }
    if let Some(art) = artwork {
        for (name, bytes, _) in &art.uploads {
            add(name.clone(), bytes.clone());
        }
        add("art_window".into(), vec![127, 0x13, 17, 0x13, 80, 0x02, 0]);
    }
    if let Some(art) = artwork {
        cursor = cursor.next_multiple_of(256);
        sections.push(Section {
            name: "motion_sine".into(),
            offset: cursor,
            bytes: art.motion.table.clone(),
        });
        cursor += art.motion.table.len();
    }
    let root_offsets = root.map(|bridge| {
        cursor = cursor.next_multiple_of(0x8000);
        let source = cursor;
        sections.push(Section {
            name: "root_source".into(),
            offset: cursor,
            bytes: bridge.root.bytes.clone(),
        });
        cursor += bridge.root.bytes.len();
        let lengths = cursor;
        sections.push(Section {
            name: "root_lengths".into(),
            offset: cursor,
            bytes: bridge.root.lengths.clone(),
        });
        cursor += bridge.root.lengths.len();
        (source, lengths)
    });
    if let Some(program) = objects {
        for (name, bytes) in [
            ("object_source", &program.bytes),
            ("object_lengths", &program.lengths),
            ("object_sine", &program.sine),
        ] {
            cursor = cursor.next_multiple_of(0x8000);
            sections.push(Section {
                name: name.into(),
                offset: cursor,
                bytes: bytes.clone(),
            });
            cursor += bytes.len();
        }
    }
    let layout = CartridgeLayout::new(cursor.next_power_of_two(), 0x7fb0, sections)?;
    let bg = Bg1::new(Mode::One, 0, tile_count, Map::new(0xf000, 32, 32)?)?;
    let text_map: u16 = if artwork.is_some() {
        crate::sfc_prologue::TEXT_MAP_WORD * 2
    } else {
        0xf000
    };
    let pad = runtime::PadStorage::new(0x300)?;
    let mut p = Assembler::new();
    startup(&mut p);
    bg.emit_configure(&mut p);
    if artwork.is_some() {
        for (address, value) in [
            (0x2105, 3),
            (0x2107, 0x7c),
            (0x2108, (crate::sfc_prologue::TEXT_MAP_WORD >> 8) as u8),
            (0x210b, 0x50),
            (0x2101, 3),
            (0x210f, 0),
            (0x210f, 0),
            (0x2110, 0xff),
            (0x2110, 0xff),
            (0x2125, 3),
            (0x2126, 7),
            (0x2127, 247),
            (0x212b, 0),
            (0x212e, 0x10),
        ] {
            write(&mut p, address, value);
        }
    }
    for (name, bytes, destination) in [
        (
            "glyphs",
            if artwork.is_some() {
                32
            } else {
                tile_count * 32
            },
            Destination::Vram(glyph_address),
        ),
        ("palette", 512, Destination::Cgram(0)),
        ("idle_map", 2048, Destination::Vram(text_map)),
    ] {
        for chunk in TransferPlan::new(&layout, name, 0, bytes, destination, 8192)?.chunks() {
            chunk.emit(&mut p, 0)?;
        }
    }
    if let Some(art) = artwork {
        for (name, bytes, destination) in &art.uploads {
            for chunk in
                TransferPlan::new(&layout, name, 0, bytes.len(), *destination, 8192)?.chunks()
            {
                chunk.emit(&mut p, 0)?;
            }
        }
        let table = layout.section("art_window")?;
        let source = layout.cpu_address(table.offset)?;
        if source >> 16 != layout.cpu_address(table.offset + table.bytes.len() - 1)? >> 16 {
            return Err("Scene window table crosses ROM bank".into());
        }
        for (address, value) in [
            (0x4310, 0),
            (0x4311, 0x2c),
            (0x4312, source as u8),
            (0x4313, (source >> 8) as u8),
            (0x4314, (source >> 16) as u8),
            (0x420c, 2),
        ] {
            write(&mut p, address, value);
        }
    }
    if let Some(art) = artwork {
        art.actors.reset(&mut p)?;
        if objects.is_none() {
            art.expression.reset(&mut p);
            art.motion.reset(&mut p);
            art.arle_parent.reset(&mut p);
            art.arle_face.reset(&mut p);
        }
    }
    for address in 0x204..=if artwork.is_some() { 0x20e } else { 0x20d } {
        write(&mut p, address, 0);
    }
    write(&mut p, 0x207, 1);
    write(&mut p, 0x20a, if root.is_some() || opening { 0 } else { 1 });
    if root.is_some() {
        for at in 0x360..=0x397 {
            write(&mut p, at, 0);
        }
        write(&mut p, 0x390, 0x42);
        write(&mut p, 0x392, 1);
        write(&mut p, 0x360, 1);
    }
    if objects.is_some() {
        crate::sfc_object_vm::emit_reset(&mut p);
        crate::sfc_object_display::reset(&mut p, artwork.unwrap().actors.positions.len());
    }
    if opening {
        crate::sfc_opening::reset(&mut p);
    }
    write(&mut p, 0x201, 0x42);
    write(&mut p, 0x212c, if artwork.is_some() { 0x13 } else { 1 });
    write(&mut p, 0x2100, 15);
    emit(&mut p, M::Lda, A::Absolute, O::Word(0x4210));
    write(&mut p, 0x4200, 0x81);
    p.label("idle");
    implied(&mut p, M::Wai);
    if opening {
        p.emit_label_ref(M::Jsr, A::Absolute, "opening_tick");
    }
    if root.is_some() {
        p.emit_label_ref(M::Jsr, A::Absolute, "root_message_tick");
    }
    if artwork.is_some() {
        if objects.is_some() {
            p.emit_label_ref(M::Jsr, A::Absolute, "pool_tick");
            p.emit_label_ref(M::Jsr, A::Absolute, "object_tick");
            p.emit_label_ref(M::Jsr, A::Absolute, "object_display_tick");
        } else {
            p.emit_label_ref(M::Jsr, A::Absolute, "arle_parent_tick");
            p.emit_label_ref(M::Jsr, A::Absolute, "arle_face_tick");
            p.emit_label_ref(M::Jsr, A::Absolute, "motion_dispatch");
            p.emit_label_ref(M::Jsr, A::Absolute, "expression_tick");
        }
        p.emit_label_ref(M::Jsr, A::Absolute, "actors_update");
    }
    p.emit_label_ref(M::Bra, A::Relative8, "idle");
    p.label("nmi");
    runtime::emit_interrupt_entry(&mut p);
    emit(&mut p, M::Lda, A::Absolute, O::Word(0x4210));
    emit(&mut p, M::Inc, A::Absolute, O::Word(0x202));
    runtime::emit_pad_call(&mut p, "sample", "done");
    p.emit_label_ref(M::Jsr, A::Absolute, "request");
    p.emit_label_ref(M::Bcs, A::Relative8, "done");
    p.emit_label_ref(M::Jsr, A::Absolute, "timed_tick");
    p.label("done");
    if artwork.is_some() {
        p.emit_label_ref(M::Jsr, A::Absolute, "actors_consume");
    }
    p.label("nmi_uploads_done");
    runtime::emit_interrupt_exit(&mut p);
    runtime::emit_auto_pad(&mut p, pad, "sample");
    emit_timed_tick(&mut p, pad, states.len() - 1, true);
    if let Some(art) = artwork {
        art.actors.routines(&mut p)?;
        if objects.is_none() {
            art.expression.routine(&mut p);
            art.motion.routine(&mut p, &layout)?;
            art.arle_parent.routine(&mut p);
            art.arle_face.routine(&mut p);
        } else {
            art.motion.project(&mut p, "motion_project", 241, 640, 7);
            art.motion.project(&mut p, "motion_project_y", 144, 382, 0);
            crate::sfc_object_display::emit_routine(&mut p, art.actors.positions.len());
        }
    }
    if let Some(program) = objects {
        program.emit(&mut p, &layout)?;
    }
    if let Some(bridge) = root {
        let (source, lengths) = root_offsets.unwrap();
        crate::sfc_root::emit_routine(
            &mut p,
            layout.cpu_address(source)?,
            layout.cpu_address(lengths)?,
        );
        bridge.emit(&mut p);
    }
    if opening {
        crate::sfc_opening::emit_routine(&mut p, pad.pressed());
    }
    p.label("request").assume_mode(CpuMode::NATIVE_M8_X16);
    emit(&mut p, M::Lda, A::Absolute, O::Word(0x20a));
    p.emit_label_ref(M::Bne, A::Relative8, "has_request");
    implied(&mut p, M::Clc);
    implied(&mut p, M::Rts);
    p.label("has_request");
    emit(&mut p, M::Lda, A::Absolute, O::Word(0x207));
    emit(&mut p, M::Cmp, A::Immediate, O::Byte(1));
    p.emit_label_ref(M::Beq, A::Relative8, "dispatch_request");
    write(&mut p, 0x20c, 2);
    write(&mut p, 0x20a, 0);
    implied(&mut p, M::Clc);
    implied(&mut p, M::Rts);
    p.label("dispatch_request");
    for (id, first, end) in &entries {
        emit(&mut p, M::Lda, A::Absolute, O::Word(0x20a));
        emit(&mut p, M::Cmp, A::Immediate, O::Byte(*id));
        p.emit_label_ref(M::Bne, A::Relative8, format!("not_request_{id}"));
        for (address, value) in [
            (0x20a, 0),
            (0x20b, *id),
            (0x20c, 0),
            (0x20d, 0),
            (0x207, 0),
            (0x206, 0),
            (0x209, *end as u8),
            (0x200, *first as u8),
        ] {
            write(&mut p, address, value);
        }
        p.emit_label_ref(M::Jsr, A::Absolute, format!("load_{first}"));
        p.emit_label_ref(M::Jsr, A::Absolute, "entry_skip");
        implied(&mut p, M::Sec);
        implied(&mut p, M::Rts);
        p.label(format!("not_request_{id}"));
    }
    write(&mut p, 0x20a, 0);
    write(&mut p, 0x20c, 1);
    implied(&mut p, M::Clc);
    implied(&mut p, M::Rts);
    p.label("entry_skip");
    emit(&mut p, M::Lda, A::Absolute, O::Word(0x208));
    p.emit_label_ref(M::Beq, A::Relative8, "entry_skip_done");
    emit(&mut p, M::Lda, A::Absolute, O::Word(pad.pressed()));
    emit(&mut p, M::And, A::Immediate, O::Byte(0x80));
    p.emit_label_ref(M::Beq, A::Relative8, "entry_skip_done");
    write(&mut p, 0x204, 0);
    write(&mut p, 0x205, 0);
    write(&mut p, 0x206, 1);
    p.label("entry_skip_done");
    implied(&mut p, M::Rts);
    for (index, page) in states.iter().enumerate() {
        p.label(format!("load_{index}"))
            .assume_mode(CpuMode::NATIVE_M8_X16);
        if artwork.is_some() {
            for chunk in TransferPlan::new(
                &layout,
                &format!("page_glyphs_{index}"),
                0,
                page_tile_bytes[index],
                Destination::Vram(glyph_address),
                8192,
            )?
            .chunks()
            {
                chunk.emit(&mut p, 0)?;
            }
        }
        for chunk in TransferPlan::new(
            &layout,
            &format!("page_{index}"),
            TEXT_MAP_OFFSET,
            text_map_bytes,
            Destination::Vram(text_map + TEXT_MAP_OFFSET as u16),
            8192,
        )?
        .chunks()
        {
            chunk.emit(&mut p, 0)?;
        }
        p.label(format!("page_upload_done_{index}"));
        set_wait(&mut p, page.wait);
        implied(&mut p, M::Rts);
    }
    p.label("irq").assume_mode(CpuMode::NATIVE_M8_X16);
    implied(&mut p, M::Rti);
    p.label("page_loaders").data(vec![0; states.len() * 2]);
    let assembled =
        crate::sfc_width_check::assemble(&p, CodeLocation::new(0, 0x8000), CpuMode::Emulation)?;
    let addr = |name: &str| assembled.label_location(name).unwrap().program_counter();
    let mut code = assembled.bytes().to_vec();
    let loader_table = usize::from(addr("page_loaders") - 0x8000);
    for index in 0..states.len() {
        code[loader_table + index * 2..loader_table + index * 2 + 2]
            .copy_from_slice(&addr(&format!("load_{index}")).to_le_bytes());
    }
    let rom = layout.build(&code, addr("reset"), addr("nmi"), addr("irq"))?;
    let mut report = serde_json::json!(
        { "scope" : if artwork.is_some() {
        "Callable dialogue over resident BG1/OBJ scene at source tick 502; no scene scheduler or audio"
        } else {
        "Callable timed message service; root scheduler, artwork and audio are not connected"
        }, "artwork_tick" : artwork.map(| _ | 502), "motion_program" : artwork.map(| art
        | serde_json::json!({ "source" : art.motion.source, "origin" : 656, "amplitude" :
        660, "direction" : 662, "owner" : 663, "request" : 664, "phase" : 665, "index" :
        666, "callbacks" : 667, "parent_x" : 672, "angle" : 676, "loop_remaining" : 678,
        "pc" : 680, "error" : 684, "scratch_start" : 688, "scratch_end" : 752, "scope" :
        "Four source movement children; mode 0/1 horizontal, 2/3 vertical; caller-selected parent, no root scheduler"
        })), "expression_program" : artwork.map(| art | serde_json::json!({ "source" :
        art.expression.source, "request" : 608, "phase" : 609, "counter" : 610,
        "callbacks" : 615, "flags" : 616, "pc" : 620, "frame" : 548, "scope" :
        "Lowered source 0x46f338; caller-started, no root scheduler" })),
        "motion_contexts" : artwork.map(| art | serde_json::json!({ "mode" : 784, "error"
        : 785, "passes" : 786, "busy" : 787, "base" : 16384, "stride" : 96, "count" : art
        .motion.parent_count, "scope" :
        "One movement child per displayed parent; shared publication after all children; no root scheduler"
        })), "arle_parent" : artwork.map(| art | serde_json::json!({ "source" : art
        .arle_parent.source, "request" : 816, "slot" : 817, "scene_phase" : 818, "phase"
        : 819, "pc" : 820, "callbacks" : 824, "captured_slot" : 826, "error" : 827,
        "scratch" : 828, "scope" :
        "Source body parent 0x46f1a0; caller supplies scene signal; overlapping child creation rejected"
        })), "arle_face" : artwork.map(| art | serde_json::json!({ "source" : art
        .arle_face.source, "request" : 832, "slot" : 833, "scene_phase" : 818, "phase" :
        835, "pc" : 836, "callbacks" : 840, "captured_slot" : 842, "error" : 843,
        "scratch" : 844, "wait" : 848, "scope" :
        "Source expression parent 0x46f280; owns resident expression frame while active; caller supplies shared signal"
        })), "logical_objects" : artwork.map(| _ | serde_json::json!({ "mode" : 2, "base"
        : 20480, "stride" : 16, "count" : crate ::sfc_logical_objects::COUNT, "x" : 0,
        "y" : 4, "visible" : 8, "display_bindings" : 800, "publish_request" : 790,
        "binding_error" : 789, "projection_scratch" : 792, "scope" :
        "Logical parent coordinates with one movement child each; no source object allocator or group scheduler"
        })), "actor_service" : artwork.map(| art | serde_json::json!({ "request" : 544,
        "error" : 545, "expression_frame" : 548, "publication" : 560, "positions" : 576,
        "count" : art.actors.positions.len(), "initial_positions" : art.actors.positions,
        "shadow_oam" : 8192, "scratch" : 640, "coordinate_domain" : [- 1024, 1023],
        "scope" : "Caller-updated positions and Arle expression 1/2; no root scheduler"
        })), "vram" : if artwork.is_some() { serde_json::json!({ "background" : 0,
        "actors" : 49152, "glyphs" : 40960, "text_map" : 61440, "background_map" : 63488
        }) } else { serde_json::Value::Null }, "rom_sha256" : sha(& rom), "rom_bytes" :
        rom.len(), "code_bytes" : assembled.bytes().len(), "toolchain_revision" :
        "4871594b3731aa495381de2bcef00f75328a0368", "entries" : entries, "states" :
        state_reports, "glyphs" : atlas.report(), "sections" : layout.section_report(),
        "source_colors" : colors, "page_glyph_bytes" : page_tile_bytes,
        "page_map_transfer" : { "source_offset" : TEXT_MAP_OFFSET, "bytes" :
        text_map_bytes, "vram" : usize::from(text_map) + TEXT_MAP_OFFSET },
        "actor_transfer" : artwork.map(| art | serde_json::json!({ "low_bytes" : art
        .actors.pieces.len() * 4, "high_bytes" : art.actors.pieces.len().div_ceil(4)
        .next_multiple_of(2), "pieces" : art.actors.pieces.len(), "startup_bytes" : 544
        })), "page_forced_blank" : false, "page_dispatch" : { "table" :
        addr("page_loaders"), "entries" : states.len(), "targets" : (0..states.len())
        .map(| index | addr(& format!("load_{index}"))).collect::< Vec < _ >> () },
        "upload_boundaries" : { "nmi_done" : addr("nmi_uploads_done"), "pages" : (0
        ..states.len()).map(| index | addr(& format!("page_upload_done_{index}")))
        .collect::< Vec < _ >> () }, "palette_contract" :
        "Text uses CGRAM entries 1..6; zero is transparent; all maps use palette 0",
        "glyph_keys" : keys.iter().map(| ((ch, color), key) | serde_json::json!({
        "source" : ch, "color" : color, "key" : key })).collect::< Vec < _ >> (), "wram"
        : { "page" : 512, "ready" : 513, "nmi_counter" : 514, "remaining" : 516,
        "skip_pending" : 518, "finished" : 519, "skip_allowed" : 520, "end_page" : 521,
        "request" : 522, "accepted" : 523, "error" : 524, "completed" : 525, "pad" : 768
        }, "request_contract" :
        "Write an ID only when finished=1. NMI clears request; error 1 unsupported, 2 busy. Completion publishes accepted ID.",
        "harness_boot_request" : 1, "limits" :
        ["A rising edge maps to source skip; one callback per NMI is a port timing policy.",
        "Root scheduler is not connected; test requests do not prove original narrative order.",
        "Voice-dependent message 20 is intentionally unsupported.",
        "No frame deadline or physical-console proof."] }
    );
    if let Some(bridge) = root {
        report["scope"] = "Source root connected to real timed message service over resident scene; other consumers unbound"
            .into();
        report["harness_boot_request"] = 0.into();
        report["root_message"] = serde_json::json!(
            { "bindings" : bridge.bindings, "enable" : 914, "error" : 915,
            "owned_message" : 916, "sequence" : 917, "phase" : 919 }
        );
        report["limits"] = serde_json::json!([
            "Non-message actions require actual consumers; test acknowledgements are not consumption proof.",
            "Message 20 requires audio support and stays pending.",
            "Fixed resident scene, no root object allocator or resource/scene changes.",
            "Logical source callbacks and physical NMI timing are distinct; no wall-clock fidelity claim."
        ]);
    }
    if objects.is_some() {
        for key in [
            "motion_program",
            "expression_program",
            "motion_contexts",
            "arle_parent",
            "arle_face",
            "logical_objects",
        ] {
            report.as_object_mut().unwrap().remove(key);
        }
        report["scope"] = "Source object VM with resident actor projection and independent callable dialogue; resource, narrative and audio services unbound"
            .into();
        report["object_display"] = serde_json::json!(
            { "enable" : 0x450, "error" : 0x451, "bindings" : 0x460, "count" : artwork
            .unwrap().actors.positions.len(), "unused_binding" : 255, "source_pool" :
            0x6000, "source_stride" : 230, "source_count" : 16, "publication_boundary" :
            addr("object_display_done") }
        );
        report["limits"] = serde_json::json!([
            "Caller binds resident artwork to source slots; no automatic source resource selection or Z sorting.",
            "Persistent visibility is projected; one-shot draw flags remain unsupported.",
            "VM requests and dialogue requests are independent; no full prologue or B1 playback."
        ]);
    }
    if opening {
        report["scope"] = "Autonomous opening: Arle introduction, source horizontal movement and Schezo introduction; incomplete prologue"
            .into();
        report["harness_boot_request"] = 0.into();
        report["opening"] = serde_json::json!(
            { "phase" : 0x480, "wait" : 0x481, "messages" : [1, 2], "controls" : { "a" :
            "skip current timed dialogue wait", "start" : "replay after second dialogue"
            }, "source_root" : [0x46e948, 0x46e950, 0x46e968, 0x46e96c], "limits" :
            ["No audio, title, later scenes or B1 transition yet.",
            "Resident artwork and port frame timing; not full source playback."] }
        );
    }
    files.push((format!("{prefix}.sfc"), rom));
    files.push((
        format!("{prefix}-build.json"),
        serde_json::to_vec_pretty(&report)?,
    ));
    Ok(files)
}
