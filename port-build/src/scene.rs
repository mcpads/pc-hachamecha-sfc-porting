use crate::{
    Result,
    images::ImageReport,
    objects::Object,
    resources::{Event, Reference},
    sha,
};
use serde::Deserialize;
use std::collections::BTreeMap;
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Description {
    slot: u16,
    file: String,
    width: usize,
    height: usize,
    kind: u8,
    surface_flags: u16,
    palette_base: u8,
    palette_end_field: u8,
    key: u8,
    grid_width: u16,
    grid_height: u16,
    columns: u16,
    frame_table: u32,
    pixels_sha256: String,
}
#[derive(Clone, Deserialize)]
#[cfg_attr(not(test), allow(dead_code))]
pub struct Case {
    stream: u32,
    slots: Vec<Description>,
    palette_sha256: String,
}
struct Slot {
    description: Description,
    pixels: Vec<u8>,
}
#[derive(Default)]
pub struct Scene {
    slots: BTreeMap<u16, Slot>,
}
impl Scene {
    fn apply(
        &mut self,
        event: &Event,
        reference: &Reference,
        images: &[ImageReport],
        files: &[(String, Vec<u8>)],
    ) -> Result<()> {
        match event.opcode {
            1 | 2 | 6 => {
                let name = event
                    .file
                    .as_ref()
                    .ok_or("Image registration lacks resource")?;
                let image = images
                    .iter()
                    .find(|i| &i.resource == name)
                    .ok_or("Unbound scene image")?;
                let source = &files
                    .iter()
                    .find(|(n, _)| *n == format!("{name}.indices"))
                    .ok_or("Missing scene pixels")?
                    .1;
                let pixels: Vec<_> = source.iter().map(|p| p.wrapping_add(event.flags)).collect();
                let kind = if event.opcode == 1 { 3 } else { 1 };
                let (grid_width, grid_height, columns, frame_table) = if kind == 1 {
                    let w = event.argument as u16;
                    let h = (event.argument >> 16) as u16;
                    if w == 0
                        || h == 0
                        || usize::from(w) > image.width
                        || usize::from(h) > image.height
                    {
                        return Err("Invalid grid extent".into());
                    }
                    (w, h, (image.width / usize::from(w)) as u16, 0)
                } else {
                    (0, 0, 0, event.argument)
                };
                let colors = reference.color_count(name)?;
                let description = Description {
                    slot: event.slot,
                    file: name.clone(),
                    width: image.width,
                    height: image.height,
                    kind,
                    surface_flags: if event.opcode == 6 { 3 } else { 1 },
                    palette_base: event.flags,
                    palette_end_field: event.flags.wrapping_add(colors as u8).wrapping_sub(2),
                    key: 0,
                    grid_width,
                    grid_height,
                    columns,
                    frame_table,
                    pixels_sha256: sha(&pixels),
                };
                self.slots.insert(
                    event.slot,
                    Slot {
                        description,
                        pixels,
                    },
                );
            }
            11 => {
                self.slots.remove(&event.slot);
            }
            12 => {
                self.slots
                    .get_mut(&event.slot)
                    .ok_or("Color key targets an empty slot")?
                    .description
                    .key = event.flags;
            }
            _ => return Err("Unknown scene resource request".into()),
        }
        Ok(())
    }
    fn rectangle(
        &self,
        slot: u16,
        clip: u16,
        catalog: &BTreeMap<(String, u16), [u16; 4]>,
    ) -> Result<[u16; 4]> {
        let s = &self
            .slots
            .get(&slot)
            .ok_or("Frame targets an empty slot")?
            .description;
        let rect = match s.kind {
            1 => {
                let x = u32::from(clip % s.columns) * u32::from(s.grid_width);
                let y = u32::from(clip / s.columns) * u32::from(s.grid_height);
                if x + u32::from(s.grid_width) > s.width as u32
                    || y + u32::from(s.grid_height) > s.height as u32
                {
                    return Err("Grid frame outside image".into());
                }
                [
                    x as u16,
                    y as u16,
                    x as u16 + s.grid_width,
                    y as u16 + s.grid_height,
                ]
            }
            3 => *catalog
                .get(&(s.file.clone(), clip))
                .ok_or("Unresolved source frame rectangle")?,
            _ => return Err("Unsupported slot frame layout".into()),
        };
        let [l, t, r, b] = rect;
        if l >= r || t >= b || usize::from(r) > s.width || usize::from(b) > s.height {
            return Err("Frame outside image".into());
        }
        Ok(rect)
    }
}
#[derive(Debug, serde::Serialize)]
pub struct Preview {
    pub file: String,
    pub policy: String,
    pub tick: usize,
    pub objects: usize,
    pub omitted_text_objects: usize,
    pub rendered_text_objects: usize,
    pub pixels_sha256: String,
    pub ppm_sha256: String,
}
pub struct PreviewOutput {
    pub reports: Vec<Preview>,
    pub files: Vec<(String, Vec<u8>)>,
    pub target_frames: Vec<crate::sfc::Frame>,
}
#[derive(Deserialize)]
pub struct Frame {
    file: String,
    clip: u16,
    rectangle: [u16; 4],
    table: u32,
}
pub struct TextFonts<'a> {
    pub port: &'a crate::font::PortFont,
    pub dialogue: &'a crate::font::DialogueFont,
    pub joins: &'a [crate::b1_dialogue::Join],
}
pub fn previews(
    vm: &crate::vm::Reference,
    images: &[ImageReport],
    files: &[(String, Vec<u8>)],
    objects: &[Object],
    text_fonts: Option<TextFonts>,
    content: &BTreeMap<u32, String>,
    b1: Option<&crate::sfc_b1::Assets>,
) -> Result<PreviewOutput> {
    let font = text_fonts.as_ref().map(|t| t.port);
    let dialogue = text_fonts.as_ref().map(|t| t.dialogue);
    let lines = text_fonts
        .as_ref()
        .map(|t| crate::sfc::prologue_layout(t.dialogue, t.joins));
    let mut reports = Vec::new();
    let mut output = Vec::new();
    let mut target_frames = Vec::new();
    let mut sprites = crate::sfc_sprites::Collector::default();
    let mut prologue = crate::sfc_prologue::Collector::default();
    let initial_catalog: BTreeMap<_, _> = objects
        .iter()
        .map(|o| ((o.frame.resource.clone(), o.frame.clip), o.frame.rectangle))
        .collect();
    for case in &vm.pool_cases {
        if case.draw_objects.is_empty() {
            continue;
        }
        if !matches!(case.policy.as_str(), "paced" | "skip") || vm.scene_windows.len() != 8 {
            return Err("Invalid artwork preview inputs".into());
        }
        let reference = vm.resources.as_ref().ok_or("Missing scene resources")?;
        let mut catalog = initial_catalog.clone();
        for frame in &case.scene_frames {
            if let Some(previous) =
                catalog.insert((frame.file.clone(), frame.clip), frame.rectangle)
                && previous != frame.rectangle
            {
                return Err("Conflicting runtime frame rectangle".into());
            }
        }
        let actions = case
            .action_order
            .as_ref()
            .ok_or("Missing scene action order")?;
        let mut scene = Scene::default();
        let mut text = crate::text_surface::TextSurface::new();
        let mut background = vec![0; 640 * 480];
        let mut cursor = 0;
        let mut panorama = None;
        let mut sequence = crate::sfc::Sequence::default();
        let stride = (case.draw_objects.len() / 16).max(1);
        if font.is_some() && case.policy == "paced" {
            prologue.waits =
                crate::world::skippable_waits(case, vm.motion.as_ref(), vm.resources.as_ref())?;
        }
        for (tick, draws) in case.draw_objects.iter().enumerate() {
            let end = *case
                .action_ends
                .get(tick)
                .ok_or("Missing scene action boundary")?;
            if end < cursor || end > actions.len() {
                return Err("Invalid scene action boundary".into());
            }
            for action in &actions[cursor..end] {
                match action.kind.as_str() {
                    "resource" => scene.apply(
                        case.resource_events
                            .get(action.index)
                            .ok_or("Missing scene resource event")?,
                        reference,
                        images,
                        files,
                    )?,
                    "render" => {
                        let event = case
                            .render_events
                            .get(action.index)
                            .ok_or("Missing scene render event")?;
                        if case.policy == "paced"
                            && matches!(
                                event,
                                crate::world::RenderEvent::Music { .. }
                                    | crate::world::RenderEvent::StopMusic
                                    | crate::world::RenderEvent::Audio { .. }
                            )
                        {
                            prologue
                                .audio
                                .push(serde_json::json!({ "tick" : tick, "event" : event }));
                        }
                        if let Some(font) = font {
                            let canonical = if let crate::world::RenderEvent::Text { pc, .. } =
                                event
                            {
                                if case.policy == "paced" {
                                    prologue.text_addresses.insert(*pc);
                                }
                                Some(
                                    content
                                        .get(pc)
                                        .ok_or("Source text missing from canonical dialogue JSON")?
                                        .as_str(),
                                )
                            } else {
                                None
                            };
                            text.apply(
                                event,
                                case.renderer.as_ref().ok_or("Missing text renderer")?,
                                font,
                                canonical,
                            )?;
                        }
                        if let crate::world::RenderEvent::Fill {
                            slot,
                            color,
                            rectangle,
                        } = event
                        {
                            if *slot != 1 {
                                return Err("Unsupported artwork fill slot".into());
                            }
                            let [l, t, r, b] = *rectangle;
                            if l > r || t > b || r > 640 || b > 480 {
                                return Err("Invalid artwork fill".into());
                            }
                            for y in t..b {
                                background
                                    [y as usize * 640 + l as usize..y as usize * 640 + r as usize]
                                    .fill(*color);
                            }
                        }
                    }
                    _ => return Err("Unknown scene action kind".into()),
                }
            }
            cursor = end;
            let sampled = tick % stride == 0 || tick + 1 == case.draw_objects.len();
            if !sampled && font.is_none() {
                continue;
            }
            let mut pixels = background.clone();
            let capture_layers = font.is_some() && case.policy == "paced";
            let mut backdrop = capture_layers.then(|| background.clone());
            let mut omitted = 0;
            let mut rendered = 0;
            let mut actors = Vec::new();
            for draw in draws {
                let slot_id = ((draw[2] >> 16) & 255) as u16;
                if slot_id == 3 {
                    if font.is_some() {
                        let rect = *case
                            .renderer
                            .as_ref()
                            .ok_or("Missing text renderer")?
                            .windows
                            .get((draw[2] & 65535) as usize)
                            .ok_or("Missing text frame")?;
                        if let Some(copy) = crate::blit::plan(
                            [draw[0] as i32, draw[1] as i32],
                            rect,
                            vm.scene_windows[(draw[3] & 7) as usize],
                            draw[3],
                            0,
                            0,
                        )? {
                            crate::blit::paint(
                                &copy,
                                crate::blit::Surface {
                                    pixels: &text.pixels,
                                    width: 640,
                                    height: 144,
                                },
                                crate::blit::Canvas {
                                    pixels: &mut pixels,
                                    width: 640,
                                    height: 480,
                                },
                                Some(0),
                            )?;
                        }
                        rendered += 1;
                    } else {
                        omitted += 1;
                    }
                    continue;
                }
                let clip = draw[2] as u16;
                let rectangle = scene
                    .rectangle(slot_id, clip, &catalog)
                    .map_err(|e| format!("Artwork tick {tick} slot {slot_id} clip {clip}: {e}"))?;
                let slot = scene.slots.get(&slot_id).ok_or("Empty artwork slot")?;
                if font.is_some() && slot.description.palette_base >= 128 {
                    actors.push(sprites.actor(
                        &slot.description.file,
                        clip,
                        rectangle,
                        &slot.pixels,
                        slot.description.width,
                        slot.description.palette_base,
                        slot.description.key,
                        draw,
                    )?);
                }
                if slot.description.kind == 3
                    && !case.scene_frames.iter().any(|f| {
                        f.file == slot.description.file
                            && f.clip == clip
                            && f.table == slot.description.frame_table
                    })
                {
                    return Err("Runtime frame table is not bound to scene slot".into());
                }
                if let Some(copy) = crate::blit::plan(
                    [draw[0] as i32, draw[1] as i32],
                    rectangle.map(i32::from),
                    vm.scene_windows[(draw[3] & 7) as usize],
                    draw[3],
                    0,
                    0,
                )? {
                    if slot.description.palette_base < 128
                        && let Some(backdrop) = backdrop.as_mut()
                    {
                        crate::blit::paint(
                            &copy,
                            crate::blit::Surface {
                                pixels: &slot.pixels,
                                width: slot.description.width,
                                height: slot.description.height,
                            },
                            crate::blit::Canvas {
                                pixels: backdrop,
                                width: 640,
                                height: 480,
                            },
                            if draw[3] & 0x80 == 0 {
                                Some(slot.description.key)
                            } else {
                                None
                            },
                        )?;
                    }
                    crate::blit::paint(
                        &copy,
                        crate::blit::Surface {
                            pixels: &slot.pixels,
                            width: slot.description.width,
                            height: slot.description.height,
                        },
                        crate::blit::Canvas {
                            pixels: &mut pixels,
                            width: 640,
                            height: 480,
                        },
                        if draw[3] & 0x80 == 0 {
                            Some(slot.description.key)
                        } else {
                            None
                        },
                    )?;
                }
            }
            let palette_id = case
                .scene_palette_ids
                .get(tick)
                .ok_or("Missing preview palette ID")?;
            if font.is_some() {
                sprites.tick(&case.policy, tick, actors)?;
            }
            let mut palette = case
                .scene_palettes
                .get(palette_id)
                .ok_or("Missing preview palette")?
                .clone();
            if palette.len() != 1024 {
                return Err("Invalid preview palette extent".into());
            }
            if font.is_some() {
                let colors = &case
                    .renderer
                    .as_ref()
                    .ok_or("Missing text renderer")?
                    .palette;
                if colors.len() != 6 {
                    return Err("Invalid startup text palette".into());
                }
                for (i, color) in colors.iter().enumerate() {
                    palette[(i + 1) * 4..(i + 1) * 4 + 3].copy_from_slice(color);
                }
            }
            let mut target_backdrop = None;
            if let Some(backdrop) = backdrop {
                let mut target = vec![0; 256 * 224];
                for y in 0..144 {
                    for x in 0..241 {
                        target[y * 256 + x + 7] = backdrop[(y * 382 / 144) * 640 + x * 640 / 241];
                    }
                }
                if tick == 502 {
                    output.push(("scene-backdrop.indices".into(), target.clone()));
                    output.push(("scene-palette.rgba".into(), palette.clone()));
                }
                target_backdrop = Some(target);
            }
            let mode = if font.is_some() { "text" } else { "artwork" };
            if font.is_some() {
                let (Some(dialogue), Some(lines)) = (dialogue, lines.as_ref()) else {
                    return Err("Prologue text needs the dialogue font".into());
                };
                let visible = draws.iter().any(|draw| ((draw[2] >> 16) & 255) == 3);
                let frame = crate::sfc::layout(
                    tick,
                    &pixels,
                    &palette,
                    visible.then_some(&text),
                    dialogue,
                    lines,
                )
                .map_err(|e| format!("SFC {} tick {tick}: {e}", case.policy))?;
                if case.policy == "paced" && (tick == stride || tick == stride * 7) {
                    target_frames.push(crate::sfc::layout(
                        tick,
                        &pixels,
                        &palette,
                        visible.then_some(&text),
                        dialogue,
                        lines,
                    )?);
                }
                if let Some(backdrop) = target_backdrop {
                    let scroll = draws
                        .iter()
                        .find(|d| (d[2] >> 16) & 255 == 38)
                        .map(|d| -(d[0] as i32 / 65536));
                    if scroll.is_some() && panorama.is_none() {
                        let first = scene.slots.get(&38).ok_or("Missing first panorama half")?;
                        let second = scene.slots.get(&39).ok_or("Missing second panorama half")?;
                        if first.description.width != 400 || second.description.width != 400 {
                            return Err("Unexpected panorama source dimensions".into());
                        }
                        let mut native = vec![0; 304 * 144];
                        for y in 0..144 {
                            for x in 0..304 {
                                let sx = x * 800 / 304;
                                let source = if sx < 400 { first } else { second };
                                native[y * 304 + x] =
                                    source.pixels[(y * 382 / 144) * 400 + sx % 400];
                            }
                        }
                        panorama = Some(native);
                    }
                    prologue.tick(
                        tick,
                        &backdrop,
                        &frame,
                        scroll.map(|x| (panorama.as_ref().unwrap().as_slice(), x)),
                    )?;
                }
                sequence.observe(frame)?;
            }
            if !sampled {
                continue;
            }
            let file = format!("prologue-{mode}-{}-{tick:05}.ppm", case.policy);
            let mut ppm = b"P6\n640 480\n255\n".to_vec();
            for pixel in &pixels {
                ppm.extend_from_slice(&palette[*pixel as usize * 4..*pixel as usize * 4 + 3]);
            }
            reports.push(Preview {
                file: file.clone(),
                policy: case.policy.clone(),
                tick,
                objects: draws.len(),
                omitted_text_objects: omitted,
                rendered_text_objects: rendered,
                pixels_sha256: sha(&pixels),
                ppm_sha256: sha(&ppm),
            });
            output.push((file, ppm));
        }
        if cursor != actions.len() {
            return Err("Unconsumed scene actions".into());
        }
        if font.is_some() {
            output.push((
                format!("sfc-sequence-{}.json", case.policy),
                serde_json::to_vec_pretty(&sequence.report(&case.policy))?,
            ));
        }
    }
    if font.is_some() {
        output.extend(sprites.finish()?);
        if prologue.text_addresses != content.keys().copied().collect() {
            return Err(
                "Complete prologue timeline does not visit every canonical dialogue text".into(),
            );
        }
        if let Some(unused) = lines.as_ref().map(|l| l.unused("prologue"))
            && !unused.is_empty()
        {
            return Err(format!("Unused prologue line joins {unused:?}").into());
        }
        output.extend(prologue.finish(&output, b1)?);
    }
    Ok(PreviewOutput {
        reports,
        files: output,
        target_frames,
    })
}
