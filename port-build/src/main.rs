mod b1;
mod b1_art;
mod b1_dialogue;
mod b1_progress;
mod blit;
mod brr;
mod cns;
mod executable_layout;
mod fft;
mod floor_events;
mod font;
mod images;
mod import;
mod import_battle;
mod import_combat;
mod import_dialogue;
mod import_field;
mod import_floor_objects;
mod import_items;
mod import_maps;
mod import_menu;
mod import_portrait;
mod import_prologue;
mod import_records;
mod import_scenes;
mod import_screens;
mod import_sheets;
mod import_sound;
mod import_world;
mod message_script;
mod messages;
mod motion;
mod music;
mod objects;
mod original;
mod package_inputs;
mod palette;
mod recipes;
mod resources;
mod scene;
mod script;
mod sfc;
mod sfc_actor_service;
mod sfc_arle_face;
mod sfc_arle_parent;
mod sfc_b1;
mod sfc_b1_equipment;
mod sfc_b1_frame;
mod sfc_b1_map;
mod sfc_b1_menu;
mod sfc_b1_portrait;
mod sfc_b1_stone;
mod sfc_b1_text;
mod sfc_b1_text_draw;
mod sfc_b1_view;
mod sfc_battle;
mod sfc_battle_act;
mod sfc_battle_controller;
mod sfc_battle_display;
mod sfc_battle_field;
mod sfc_battle_message;
mod sfc_battle_objects;
mod sfc_battle_scripts;
mod sfc_battle_vm;
mod sfc_body_tiles;
mod sfc_encounter;
mod sfc_ending;
mod sfc_expression;
mod sfc_far_code;
mod sfc_field_events;
mod sfc_field_flames;
mod sfc_field_guardian;
mod sfc_field_shards;
mod sfc_field_smoke;
mod sfc_floor_walls;
mod sfc_item_use;
mod sfc_layers;
mod sfc_logical_objects;
mod sfc_message;
mod sfc_message_catalog;
mod sfc_motion;
mod sfc_object_arithmetic;
mod sfc_object_display;
mod sfc_object_motion;
mod sfc_object_pool;
mod sfc_object_vm;
mod sfc_opening;
mod sfc_portrait_scale;
mod sfc_prologue;
mod sfc_rom;
mod sfc_root;
mod sfc_root_message;
mod sfc_save;
mod sfc_shop;
mod sfc_sound;
mod sfc_sprites;
mod sfc_tad;
mod sfc_title;
mod sfc_unpack;
mod sfc_width_check;
mod sound_commands;
mod text_surface;
mod vm;
mod world;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{error::Error, fs, path::Path};
type Result<T> = std::result::Result<T, Box<dyn Error>>;
mod content;
const SOURCE: &str = "0805df715b161aa015b9fa71551559fc1652a520487e0d9a02b9e2618a16d308";
#[derive(Deserialize)]
struct Manifest {
    schema: u32,
    source_sha256: String,
    floor: u8,
    width: u8,
    height: u8,
    coverage: String,
    records: Vec<Record>,
    #[serde(default)]
    resources: Vec<Resource>,
    initial_state: [u8; 4],
    direction_masks: [u8; 4],
    events: Vec<Event>,
    #[serde(default)]
    #[cfg_attr(not(test), allow(dead_code))]
    event_reference_sha256: String,
    messages: Vec<messages::Message>,
    #[serde(default)]
    images: Vec<images::Image>,
    #[serde(default)]
    objects: Vec<objects::Object>,
    vm_reference_sha256: String,
}
#[derive(Deserialize)]
struct Event {
    id: u8,
    x: u8,
    y: u8,
    direction_mask: u8,
    kind: u8,
}
#[derive(Debug, Deserialize, Serialize)]
struct Resource {
    file: String,
    bytes: usize,
    sha256: String,
}
#[derive(Deserialize)]
struct Record {
    role: String,
    file: String,
    bytes: usize,
    sha256: String,
}
#[derive(Debug, Serialize)]
struct BuildReport {
    status: &'static str,
    source_sha256: &'static str,
    records_manifest_sha256: String,
    content_sha256: std::collections::BTreeMap<String, String>,
    layout_sha256: String,
    layout_bytes: usize,
    limits: [&'static str; 3],
    validation: &'static str,
    source_resources: Vec<Resource>,
    event_table_sha256: String,
    message_count: usize,
    glyph_count: usize,
    messages_sha256: String,
    glyphs_sha256: String,
    initial_state: [u8; 4],
    images: Vec<images::ImageReport>,
    object_count: usize,
    objects_sha256: String,
    artwork_previews: Vec<scene::Preview>,
    port_font: Option<font::Report>,
    sfc_display: Option<serde_json::Value>,
    script_sha256: String,
    motion_sha256: String,
    resources_sha256: String,
}
fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
#[derive(Debug)]
struct BuildOutput {
    layout: Vec<u8>,
    event_table: Vec<u8>,
    messages: messages::Compiled,
    image_files: Vec<(String, Vec<u8>)>,
    objects: Vec<u8>,
    script: Vec<u8>,
    motion: Vec<u8>,
    resources: Vec<u8>,
    report: BuildReport,
}
fn compile(
    input: &content::Content,
    font_path: Option<&Path>,
    dialogue_font_path: Option<&Path>,
    content_root: Option<&content::Content>,
) -> Result<BuildOutput> {
    let raw = input.read("manifest.json")?;
    let mut manifest: Manifest = serde_json::from_slice(&raw)?;
    if manifest.schema != 8
        || manifest.source_sha256 != SOURCE
        || (manifest.floor, manifest.width, manifest.height) != (1, 8, 8)
        || manifest.coverage != "partial_b1_and_prologue"
        || manifest.records.len() != 3
        || manifest.initial_state != [1, 3, 7, 0]
        || manifest.direction_masks != [1, 2, 8, 4]
    {
        return Err("Unsupported records manifest".into());
    }
    let content_sha256 = match content_root {
        Some(root) => content::load(root, &mut manifest)?,
        None => Default::default(),
    };
    let mut payload = Vec::new();
    for (role, filename, size) in [
        ("cells", "b1-cells.bin", 128),
        ("encounter_masks", "b1-encounter-masks.bin", 64),
        ("direction_deltas", "direction-deltas.bin", 16),
    ] {
        let matching: Vec<_> = manifest.records.iter().filter(|r| r.role == role).collect();
        if matching.len() != 1 {
            return Err(format!("Missing or duplicate role: {role}").into());
        }
        let record = matching[0];
        if record.file != filename || record.bytes != size {
            return Err(format!("Invalid extent or filename: {role}").into());
        }
        let bytes = input.read(filename)?;
        if bytes.len() != size || sha(&bytes) != record.sha256 {
            return Err(format!("Records payload changed: {role}").into());
        }
        payload.extend(bytes);
    }
    let mut names = std::collections::BTreeSet::new();
    if manifest.resources.is_empty() {
        return Err("Missing B1 resource dependencies".into());
    }
    for resource in &manifest.resources {
        if !resource.file.ends_with(".CNS")
            || resource.file.contains("..")
            || !resource
                .file
                .bytes()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_' || c == b'.')
            || !names.insert(&resource.file)
        {
            return Err("Invalid or duplicate resource filename".into());
        }
    }
    let mut ids = std::collections::BTreeSet::new();
    if manifest.events.is_empty() || manifest.events.len() > 254 {
        return Err("Invalid event count".into());
    }
    let mut event_table = b"HME1".to_vec();
    event_table.push(manifest.events.len() as u8);
    event_table.extend(manifest.initial_state);
    event_table.extend(manifest.direction_masks);
    for event in &manifest.events {
        if event.id == 255
            || !ids.insert(event.id)
            || event.x >= 8
            || event.y >= 8
            || event.direction_mask > 15
            || event.kind > 15
        {
            return Err("Invalid event record".into());
        }
        event_table.extend([event.id, event.x, event.y, event.direction_mask, event.kind]);
    }
    let messages = messages::compile(&manifest.messages, &ids)?;
    let image_root = content_root.map_or_else(|| input.clone(), |root| root.sub("images"));
    let images = images::compile(&image_root, &manifest.images, &manifest.resources)?;
    let objects = objects::compile(&manifest.objects, &images.reports)?;
    let vm_raw = input.read("prologue-vm.json")?;
    if sha(&vm_raw) != manifest.vm_reference_sha256 {
        return Err("Native VM reference changed".into());
    }
    let reference: vm::Reference = serde_json::from_slice(&vm_raw)?;
    let resources = resources::compile(reference.resources.as_ref(), &images.files)?;
    let motion = motion::compile(reference.motion.as_ref())?;
    let script = vm::compile(&reference)?;
    let b1_dialogue = content_root.map(b1_dialogue::Catalogue::load).transpose()?;
    let port_font = font_path
        .map(|path| {
            font::PortFont::load(
                path,
                &messages.glyphs,
                &reference.text_encoding,
                b1_dialogue.as_ref().map_or("", |d| d.text.as_str()),
            )
        })
        .transpose()?;
    let joins = match content_root {
        Some(_) => b1_dialogue::load_joins()?,
        None => Vec::new(),
    };
    let prologue_text = messages::prologue_text(&manifest.messages)?;
    let dialogue = match (dialogue_font_path, b1_dialogue.as_ref()) {
        (Some(path), Some(catalogue)) => {
            let entry_pages = messages::entry_pages(&manifest.messages)?;
            let text = format!(
                " {}{}{}{}{}{}{}",
                catalogue.text,
                content_root
                    .map(sfc_b1_menu::text)
                    .transpose()?
                    .unwrap_or_default(),
                content_root
                    .map(|c| sfc_battle_message::load(c).map(|m| m.text()))
                    .transpose()?
                    .unwrap_or_default(),
                entry_pages.concat(),
                b1_dialogue::CHOICE,
                prologue_text.values().cloned().collect::<String>(),
                sfc_title::TEXT
            );
            Some((
                font::DialogueFont::load(path, &text)?,
                entry_pages,
                catalogue,
            ))
        }
        _ => None,
    };
    let b1_artwork = match (content_root, dialogue.as_ref()) {
        (Some(root), Some((font, entry_pages, catalogue))) => Some(sfc_b1::prepare(
            root,
            &images.files,
            font,
            entry_pages,
            catalogue,
            &joins,
            &reference
                .pool_cases
                .iter()
                .find_map(|case| case.renderer.as_ref())
                .ok_or("Missing source text palette")?
                .palette,
        )?),
        _ => None,
    };
    let text_fonts = match (port_font.as_ref(), dialogue.as_ref()) {
        (Some(port), Some((dialogue, _, _))) => Some(scene::TextFonts {
            port,
            dialogue,
            joins: &joins,
        }),
        (Some(_), None) if content_root.is_some() => {
            return Err("Prologue and B1 text need the pinned dialogue font".into());
        }
        _ => None,
    };
    let previews = scene::previews(
        &reference,
        &images.reports,
        &images.files,
        &manifest.objects,
        text_fonts,
        &prologue_text,
        b1_artwork.as_ref(),
    )?;
    let sfc_display = sfc::build(&previews.target_frames)?;
    let mut extra_files = previews.files;
    if let Some(root) = content_root {
        extra_files.extend(b1_progress::compile(root, None)?);
        extra_files.extend(floor_events::compile(root)?);
    }
    if let Some(root) = content_root {
        extra_files.extend(b1::compile(root)?);
    }
    if let Some(display) = &sfc_display {
        extra_files.extend(display.files.clone());
    }
    if let Some(font) = &port_font {
        extra_files.extend(sfc_root::build(&reference.commands)?);
        extra_files.extend(sfc_object_pool::build()?);
        extra_files.extend(sfc_object_vm::build(
            &reference.commands,
            &reference
                .motion
                .as_ref()
                .ok_or("Missing object motion table")?
                .table,
        )?);
        extra_files.push(("font-16x16.bin".into(), font.packed.clone()));
        let entry_pages: Vec<String> = messages::entry_pages(&manifest.messages)?
            .iter()
            .map(|page| page.replace(b1_dialogue::SOURCE_BREAK, ""))
            .collect();
        extra_files.extend(sfc_message::build(&entry_pages, font)?);
        let (pages, waits) = messages::timed_pages(&manifest.messages, 3)?;
        extra_files.extend(sfc_message::build_timed(&pages, &waits, font)?);
        let programs = (1..20)
            .map(|id| messages::timed_program(&manifest.messages, id))
            .collect::<Result<Vec<_>>>()?;
        let colors = &reference
            .pool_cases
            .iter()
            .find_map(|case| case.renderer.as_ref())
            .ok_or("Missing source text palette")?
            .palette;
        extra_files.extend(sfc_message_catalog::build(
            &programs, colors, font, None, None, None, false,
        )?);
        let artwork = sfc_layers::prepare(
            &extra_files,
            &reference.commands,
            &reference
                .motion
                .as_ref()
                .ok_or("Missing source motion")?
                .table,
        )?;
        extra_files.extend(sfc_message_catalog::build(
            &programs,
            colors,
            font,
            Some(&artwork),
            None,
            None,
            false,
        )?);
        let bridge = sfc_root_message::Bridge::compile(&reference.commands, &manifest.messages)?;
        extra_files.extend(sfc_message_catalog::build(
            &programs,
            colors,
            font,
            Some(&artwork),
            Some(&bridge),
            None,
            false,
        )?);
        let object_program = sfc_object_vm::compile(
            &reference.commands,
            &reference.motion.as_ref().unwrap().table,
        )?;
        extra_files.extend(sfc_message_catalog::build(
            &programs,
            colors,
            font,
            Some(&artwork),
            None,
            Some(&object_program),
            false,
        )?);
        extra_files.extend(sfc_message_catalog::build(
            &programs[..2],
            colors,
            font,
            Some(&artwork),
            None,
            None,
            true,
        )?);
    }
    let mut layout = b"HMB1\x01\x08\x08\x01".to_vec();
    layout.extend(payload);
    let report = BuildReport {
        status: "compiled_game_data",
        source_sha256: SOURCE,
        records_manifest_sha256: sha(&raw),
        content_sha256,
        layout_sha256: sha(&layout),
        layout_bytes: layout.len(),
        source_resources: manifest.resources,
        event_table_sha256: sha(&event_table),
        message_count: messages.message_count,
        glyph_count: messages.glyph_count,
        messages_sha256: sha(&messages.messages),
        glyphs_sha256: sha(&messages.glyphs),
        initial_state: manifest.initial_state,
        images: images.reports,
        object_count: manifest.objects.len(),
        objects_sha256: sha(&objects),
        artwork_previews: previews.reports,
        port_font: port_font.map(|font| font.report),
        sfc_display: sfc_display.map(|display| display.report),
        script_sha256: sha(&script),
        motion_sha256: sha(&motion),
        resources_sha256: sha(&resources),
        validation: "Validated source conversion.",
        limits: [
            "Static cells may be changed by original initialization and events.",
            "No complete B1 event scripts or composed prologue playback.",
            "SFC display ROM contains two sampled scenes, not game VM or B1 execution.",
        ],
    };
    Ok(BuildOutput {
        layout,
        event_table,
        messages,
        image_files: images.files.into_iter().chain(extra_files).collect(),
        objects,
        script,
        motion,
        resources,
        report,
    })
}
const OUTPUT_MARKER: &str = ".port-build-output";
fn validate_output(output: &Path) -> Result<()> {
    if output.exists()
        && (output.is_symlink()
            || fs::read_to_string(output.join(OUTPUT_MARKER))
                .ok()
                .as_deref()
                != Some("hachamecha-port-build\n"))
    {
        return Err("Existing output is not a managed build directory".into());
    }
    Ok(())
}
fn prepare_output(output: &Path) -> Result<()> {
    validate_output(output)?;
    if output.exists() {
        fs::remove_dir_all(output)?;
    }
    fs::create_dir_all(output)?;
    fs::write(output.join(OUTPUT_MARKER), "hachamecha-port-build\n")?;
    Ok(())
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.first().is_some_and(|a| a == "prepare-audio") && args.len() == 3 {
        let inputs = package_inputs::snapshot_kr(Path::new(&args[1]))?;
        let bank = inputs
            .files()
            .get("MIDDATA.MLK")
            .ok_or("Missing MIDI bank")?;
        music::prepare(bank, Path::new(&args[2]))?;
        return Ok(());
    }
    if args.first().is_some_and(|a| a == "check-kr") && args.len() == 2 {
        let files = package_inputs::read_kr(Path::new(&args[1]))?;
        println!(
            "{}",
            serde_json::json!({ "validated" : true, "files" : files.len(),
            "contract" : package_inputs::manifest() ?, "executable_sha256": sha(files.get("MADO1.EXE").ok_or("Missing executable")?) })
        );
        return Ok(());
    }
    if !args.first().is_some_and(|a| a == "build-kr-tad") {
        return Err(
            "Use check-kr <ZIP> or build-kr-tad --game ZIP --audio DIR --output DIR --font FILE --dialogue-font FILE"
                .into(),
        );
    }
    let options = package_inputs::BuildOptions::parse(&args[1..])?;
    validate_output(&options.output)?;
    let originals = original::Originals::kr_only(&options.game)?;
    let mut files = import::run_except(&originals, &["sound"])?;
    files.extend(sfc_tad::import(&originals, &options.audio)?);
    let (records, assets) = import::split(files);
    let output = compile(
        &content::Content::new(None, records)?,
        Some(&options.font),
        Some(&options.dialogue_font),
        Some(&content::Content::new(None, assets)?),
    )?;
    prepare_output(&options.output)?;
    for (name, bytes) in output.image_files {
        fs::write(options.output.join(name), bytes)?;
    }
    Ok(())
}
