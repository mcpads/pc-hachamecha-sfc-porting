use crate::{Result, SOURCE, cns::Sheet, import::Files, original::Originals, sha};
use serde_json::{Value, json};
use std::collections::BTreeMap;
const PUPILS: u16 = 256;
const SIZE: (i32, i32) = (224, 240);
const ORIGIN: (i32, i32) = (352, 32);
const FACE_RECTANGLES: (u32, usize) = (0x443aa1, 27);
const CARBUNCLE_RECTANGLES: (u32, usize) = (0x443c51, 19);
const CARBUNCLE: [(&str, u32); 3] = [
    ("body", 0x445ea4),
    ("carb_eyes", 0x445f68),
    ("carb_mouth", 0x44601c),
];
const SHAKE: [i32; 16] = [-1, 1, -3, 2, -4, 3, -4, 4, -4, 3, -4, 2, -3, 1, -1, 0];
type Steps = Vec<(u16, u16)>;
type Rgb = [u8; 3];
type Layer = Vec<Option<Rgb>>;
#[derive(Clone, Copy, PartialEq)]
enum Key {
    Background(u8),
    Colour(Rgb),
}
struct Sheets {
    kao: Sheet,
    kar: Sheet,
    face: Sheet,
    backgrounds: Vec<Sheet>,
    shared: Vec<u8>,
    rects: Vec<[i32; 4]>,
    carb_rects: Vec<[i32; 4]>,
}
impl Sheets {
    fn paint(
        &self,
        layer: &mut Layer,
        sheet: &Sheet,
        rect: [i32; 4],
        position: (i32, i32),
        key: u8,
    ) {
        let [l, t, r, b] = rect;
        for y in 0..b - t {
            for x in 0..r - l {
                let (sx, sy) = (l + x, t + y);
                let index = if (0..sheet.width as i32).contains(&sx)
                    && (0..sheet.height as i32).contains(&sy)
                {
                    sheet.pixels[sy as usize * sheet.width + sx as usize]
                } else {
                    0
                };
                let (dx, dy) = (position.0 + x, position.1 + y);
                if index == key || !(0..SIZE.0).contains(&dx) || !(0..SIZE.1).contains(&dy) {
                    continue;
                }
                let i = usize::from(index) * 3;
                layer[(dy * SIZE.0 + dx) as usize] =
                    Some([self.shared[i], self.shared[i + 1], self.shared[i + 2]]);
            }
        }
    }
    fn blank() -> Layer {
        vec![None; (SIZE.0 * SIZE.1) as usize]
    }
}
fn rectangles(originals: &Originals, (at, count): (u32, usize)) -> Result<Vec<[i32; 4]>> {
    Ok(originals
        .exe()?
        .read(at, count * 16)?
        .chunks(16)
        .map(|c| {
            std::array::from_fn(|i| i32::from_le_bytes(c[i * 4..i * 4 + 4].try_into().unwrap()))
        })
        .collect())
}
fn script(originals: &Originals, va: u32, slot: u16, limit: u16) -> Result<(Steps, Vec<u32>)> {
    let exe = originals.exe()?;
    let (mut at, mut steps, mut helpers) = (va, Vec::new(), Vec::new());
    loop {
        let raw = exe.read(at, 8)?;
        match raw[0] {
            0x21 => {
                let word = |i: usize| u16::from_le_bytes([raw[i], raw[i + 1]]);
                let (hold, clip, s) = (word(2), word(4), word(6));
                if s != slot || clip >= limit {
                    return Err(format!("Unexpected portrait SHOW at {at:#x}").into());
                }
                steps.push((clip, hold));
                at += 8;
            }
            0x07 if slot == 26 => {
                helpers.push(u32::from_le_bytes(raw[4..8].try_into()?));
                at += 8;
            }
            0x1f if slot == 26 => at += 4,
            0x03 => break,
            op => {
                return Err(format!("Unexpected portrait command {op:#x} at {at:#x}").into());
            }
        }
    }
    if steps.is_empty() {
        return Err("Portrait script without a pose".into());
    }
    Ok((steps, helpers))
}
fn pupils(originals: &Originals, roots: &[u32]) -> Result<Vec<Vec<(i32, i32)>>> {
    let exe = originals.exe()?;
    let table: Vec<u16> = exe
        .read(0x45b3c8, 2048)?
        .chunks(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    let trig = |angle: u32, cosine: bool| -> i64 {
        let mut a = ((angle + 8) & 0xffff) >> 4;
        if cosine {
            a = (a + 1024) & 4095;
        }
        let i = (a & 1023) as usize;
        let value = if a & 1024 != 0 {
            if i == 0 {
                65536
            } else {
                i64::from(table[1024 - i])
            }
        } else {
            i64::from(table[i])
        };
        if a & 2048 != 0 { -value } else { value }
    };
    let mut tracks = Vec::new();
    for &root in roots {
        let raw = exe.read(root, 0x80)?;
        if raw[..4] != [0x12, 0xa0, 0xa8, 0xa0] || raw[4] != 0x08 {
            return Err("Unexpected pupil helper".into());
        }
        let (mut fields, mut cursor) = (BTreeMap::new(), 8);
        while raw[cursor] != 0xff {
            match raw[cursor] {
                3 => {
                    fields.insert(
                        raw[cursor + 1],
                        u32::from_le_bytes(raw[cursor + 4..cursor + 8].try_into()?),
                    );
                    cursor += 8;
                }
                2 => {
                    fields.insert(
                        raw[cursor + 1],
                        u32::from(u16::from_le_bytes([raw[cursor + 2], raw[cursor + 3]])),
                    );
                    cursor += 4;
                }
                _ => return Err("Unexpected pupil assignment".into()),
            }
        }
        cursor += 4;
        if raw[cursor..cursor + 4] != [0x2d, 3, 0, 0] {
            return Err("Unexpected pupil motion".into());
        }
        let mut turns = [0i64; 2];
        for (k, turn) in turns.iter_mut().enumerate() {
            let at = cursor + 4 + 8 * k;
            let value = i64::from(u32::from_le_bytes(raw[at + 4..at + 8].try_into()?));
            if raw[at] != 0x11
                || ![0x81, 0x82].contains(&raw[at + 1])
                || raw[at + 2] != 0x8c + 2 * k as u8
            {
                return Err("Unexpected pupil turn".into());
            }
            *turn = if raw[at + 1] == 0x81 { value } else { -value };
        }
        let field = |k: u8| fields.get(&k).copied().ok_or("Missing pupil field");
        if fields.get(&0x28) != Some(&0x001a001a) || field(0x92)? != field(0x94)? {
            return Err("Unexpected pupil picture or amplitude".into());
        }
        let amplitude = i64::from(field(0x92)? as u16 as i16);
        let (mut x, mut y) = (i64::from(field(0x1c)?), i64::from(field(0x20)?));
        let (mut ax, mut ay) = (i64::from(field(0x8c)?), i64::from(field(0x8e)?));
        let mut track = Vec::new();
        for _ in 0..32 {
            x = (x + trig(ax as u32, false) * amplitude) & 0xffff_ffff;
            y = (y - trig(ay as u32, true) * amplitude) & 0xffff_ffff;
            ax = (ax + turns[0]) & 0xffff;
            ay = (ay + turns[1]) & 0xffff;
            track.push(((x >> 16) as i32, (y >> 16) as i32));
        }
        if (x, y, ax, ay)
            != (
                i64::from(field(0x1c)?),
                i64::from(field(0x20)?),
                i64::from(field(0x8c)?),
                i64::from(field(0x8e)?),
            )
        {
            return Err("Pupil motion does not repeat every 32 ticks".into());
        }
        tracks.push(track);
    }
    Ok(tracks)
}
fn unique<T: PartialEq + Copy>(items: impl IntoIterator<Item = T>) -> Vec<T> {
    let mut out = Vec::new();
    for item in items {
        if !out.contains(&item) {
            out.push(item);
        }
    }
    out
}
fn steps(steps: &[(u16, u16)]) -> Value {
    json!(
        steps
            .iter()
            .map(|(clip, hold)| json!({ "clip" : clip, "hold" : hold }))
            .collect::<Vec<_>>()
    )
}
pub fn import(originals: &Originals) -> Result<Files> {
    let exe = originals.exe()?;
    let kar = originals.cns("KAR2.CNS")?;
    let sheets = Sheets {
        kao: originals.cns("KAO_2.CNS")?,
        face: originals.cns("FACE.CNS")?,
        backgrounds: (1..=4)
            .map(|k| originals.cns(&format!("KAO_BG{k}.CNS")))
            .collect::<Result<_>>()?,
        shared: kar.rgb.clone(),
        kar,
        rects: rectangles(originals, FACE_RECTANGLES)?,
        carb_rects: rectangles(originals, CARBUNCLE_RECTANGLES)?,
    };
    let mut states = Vec::new();
    for step in 0..8 {
        let eyes = script(originals, exe.u32(0x445924 + 4 * step)?, 26, u16::MAX)?;
        let mouth = script(originals, exe.u32(0x445bbc + 4 * step)?, 26, u16::MAX)?;
        states.push((eyes, mouth));
    }
    let carbuncle: Vec<(&str, Steps)> = CARBUNCLE
        .iter()
        .map(|&(name, va)| Ok((name, script(originals, va, 64, 19)?.0)))
        .collect::<Result<_>>()?;
    let masks: Vec<Vec<bool>> = sheets
        .backgrounds
        .iter()
        .map(|b| b.pixels.iter().map(|&v| v != 0).collect())
        .collect();
    if masks.iter().any(|m| *m != masks[0]) {
        return Err("Portrait backgrounds differ in shape".into());
    }
    let tracks = pupils(originals, &states[7].0.1)?;
    if states[7].0.0.iter().map(|s| s.0).collect::<Vec<_>>() != [8] {
        return Err("Unexpected step 7 eyes".into());
    }
    let face: Vec<Rgb> = sheets
        .face
        .pixels
        .iter()
        .map(|&v| {
            let i = usize::from(v) * 3;
            [sheets.shared[i], sheets.shared[i + 1], sheets.shared[i + 2]]
        })
        .collect();
    let base_keys = |k: usize| -> Vec<Option<Key>> {
        sheets.backgrounds[k]
            .pixels
            .iter()
            .zip(&face)
            .map(|(&v, &rgb)| {
                Some(if v != 0 {
                    Key::Background(v)
                } else {
                    Key::Colour(rgb)
                })
            })
            .collect()
    };
    let over = |mut keys: Vec<Option<Key>>, layers: &[&Layer]| {
        for layer in layers {
            for (key, pixel) in keys.iter_mut().zip(layer.iter()) {
                if let Some(rgb) = pixel {
                    *key = Some(Key::Colour(*rgb));
                }
            }
        }
        keys
    };
    let arle = |eyes: u16, mouth: u16| -> Layer {
        let mut layer = Sheets::blank();
        let phase = (eyes >= PUPILS).then(|| usize::from(eyes - PUPILS));
        let eye_clip = if phase.is_some() { 8 } else { eyes };
        sheets.paint(
            &mut layer,
            &sheets.kao,
            sheets.rects[usize::from(eye_clip)],
            (64, 104),
            0,
        );
        sheets.paint(
            &mut layer,
            &sheets.kao,
            sheets.rects[usize::from(mouth)],
            (80, 152),
            0,
        );
        if let Some(phase) = phase {
            for track in &tracks {
                let (x, y) = track[phase];
                sheets.paint(
                    &mut layer,
                    &sheets.kao,
                    sheets.rects[26],
                    (x - ORIGIN.0, y - ORIGIN.1),
                    0,
                );
            }
        }
        layer
    };
    let carbuncle_layer = |clips: [u16; 3]| -> Layer {
        let mut layer = Sheets::blank();
        for (clip, position) in clips.into_iter().zip([(160, 80), (160, 160), (160, 176)]) {
            sheets.paint(
                &mut layer,
                &sheets.kar,
                sheets.carb_rects[usize::from(clip)],
                position,
                61,
            );
        }
        layer
    };
    let mut loops: Vec<(Steps, Steps)> = states
        .iter()
        .map(|((e, _), (m, _))| (e.clone(), m.clone()))
        .collect();
    loops[7].0 = (0..32).map(|k| (PUPILS + k, 1)).collect();
    let eye_clips = unique(loops.iter().flat_map(|(e, _)| e.iter().map(|s| s.0)));
    let mouth_clips = unique(loops.iter().flat_map(|(_, m)| m.iter().map(|s| s.0)));
    let combos = unique(loops.iter().flat_map(|(e, m)| {
        e.iter()
            .flat_map(move |a| m.iter().map(move |b| (a.0, b.0)))
    }));
    let carb_clips: Vec<Vec<u16>> = carbuncle
        .iter()
        .map(|(_, seq)| unique(seq.iter().map(|s| s.0)))
        .collect();
    let mut layers: Vec<(String, Layer)> = combos
        .iter()
        .map(|&(e, m)| (format!("arle-{e}-{m}"), arle(e, m)))
        .collect();
    for a in 0..carb_clips[0].len() {
        for b in 0..carb_clips[1].len() {
            for c in 0..carb_clips[2].len() {
                layers.push((
                    format!("carbuncle-{a}-{b}-{c}"),
                    carbuncle_layer([carb_clips[0][a], carb_clips[1][b], carb_clips[2][c]]),
                ));
            }
        }
    }
    let default_arle = arle(states[0].0.0[0].0, states[0].1.0[0].0);
    let default_carbuncle = carbuncle_layer([carb_clips[0][0], carb_clips[1][0], carb_clips[2][0]]);
    let mut colors: Vec<Key> = Vec::new();
    let mut index = |keys: &[Option<Key>]| -> Vec<u8> {
        keys.iter()
            .map(|key| match key {
                None => 255,
                Some(key) => {
                    if !colors.contains(key) {
                        colors.push(*key);
                    }
                    colors.iter().position(|c| c == key).unwrap() as u8
                }
            })
            .collect()
    };
    let colour_keys =
        |layer: &Layer| -> Vec<Option<Key>> { layer.iter().map(|p| p.map(Key::Colour)).collect() };
    let healthy: Vec<Vec<u8>> = (0..4)
        .map(|k| index(&over(base_keys(k), &[&default_arle, &default_carbuncle])))
        .collect();
    let mut frames: Vec<(String, Vec<u8>)> = (0..4)
        .map(|k| (format!("base-{k}"), index(&base_keys(k))))
        .collect();
    for (name, layer) in &layers {
        frames.push((name.clone(), index(&colour_keys(layer))));
    }
    let mut impact = Sheets::blank();
    sheets.paint(
        &mut impact,
        &sheets.kao,
        sheets.rects[25],
        (416 - ORIGIN.0, 128 - ORIGIN.1),
        0,
    );
    frames.push(("impact".into(), index(&colour_keys(&impact))));
    let eating = |va: u32| -> Result<(Steps, Steps)> {
        let (mut at, mut before, mut after, mut waited) = (va, Vec::new(), Vec::new(), false);
        loop {
            let raw = exe.read(at, 8)?;
            match raw[0] {
                0x21 => {
                    let word = |i: usize| u16::from_le_bytes([raw[i], raw[i + 1]]);
                    if word(6) != 26 {
                        return Err("Unexpected eating face slot".into());
                    }
                    if waited { &mut after } else { &mut before }.push((word(4), word(2)));
                    at += 8;
                }
                0x67 => {
                    waited = true;
                    at += 4;
                }
                0x03 => return Ok((before, after)),
                _ => return Err("Unexpected eating face command".into()),
            }
        }
    };
    if exe.read(0x445868 + 12, 6)? != [0xa1, 0x03, 0x54, 0x01, 0x02, 0x00] {
        return Err("Unexpected eating face parent".into());
    }
    let length = u16::from_le_bytes(exe.read(0x445868 + 18, 2)?.try_into()?);
    let (eyes, mouth) = (eating(0x445a0c)?, eating(0x445cb4)?);
    let pose = |steps: &[(u16, u16)], mut tick: u16| {
        for &(clip, hold) in steps {
            if tick < hold {
                return clip;
            }
            tick -= hold;
        }
        steps.last().unwrap().0
    };
    let waiting = (
        eyes.0.last().ok_or("Eating eyes")?.0,
        mouth.0.last().ok_or("Eating mouth")?.0,
    );
    let timeline: Vec<(u16, u16)> = (0..length)
        .map(|t| (pose(&eyes.1, t), pose(&mouth.1, t)))
        .collect();
    let eat_poses = unique(std::iter::once(waiting).chain(timeline.iter().copied()));
    let mut inside = vec![false; (SIZE.0 * SIZE.1) as usize];
    let eye_area: Vec<u16> = unique(
        eye_clips
            .iter()
            .copied()
            .filter(|&c| c < PUPILS)
            .chain(eat_poses.iter().map(|p| p.0)),
    );
    let mouth_area: Vec<u16> = unique(
        mouth_clips
            .iter()
            .copied()
            .chain(eat_poses.iter().map(|p| p.1)),
    );
    for (clips, (px, py)) in [(eye_area, (64, 104)), (mouth_area, (80, 152))] {
        for clip in clips {
            let [l, t, r, b] = sheets.rects[usize::from(clip)];
            for y in py.max(0)..(py + b - t).min(SIZE.1) {
                for x in px.max(0)..(px + r - l).min(SIZE.0) {
                    inside[(y * SIZE.0 + x) as usize] = true;
                }
            }
        }
    }
    let base0 = base_keys(0);
    if inside
        .iter()
        .zip(&base0)
        .any(|(m, k)| *m && matches!(k, Some(Key::Background(_))))
    {
        return Err("Eye or mouth clip area reaches the background".into());
    }
    for (k, &(e, m)) in eat_poses.iter().enumerate() {
        let keys: Vec<Option<Key>> = inside
            .iter()
            .zip(&base0)
            .map(|(m, k)| if *m { *k } else { None })
            .collect();
        frames.push((format!("eat-{k}"), index(&over(keys, &[&arle(e, m)]))));
    }
    if colors.len() > 32 {
        return Err("Portrait exceeds reserved palette".into());
    }
    let rgb_of = |k: usize| -> Vec<u8> {
        colors
            .iter()
            .flat_map(|key| match key {
                Key::Background(v) => {
                    let i = usize::from(*v) * 3;
                    sheets.backgrounds[k].rgb[i..i + 3].try_into().unwrap()
                }
                Key::Colour(rgb) => *rgb,
            })
            .collect()
    };
    let slot = |v: u8| colors.iter().position(|c| *c == Key::Background(v));
    let rgb: Vec<Vec<u8>> = (0..4).map(rgb_of).collect();
    let mut files = Files::new();
    let digest = |name: &str| sha(&frames.iter().find(|(n, _)| n == name).unwrap().1);
    let idle = json!(
        { "channels" : carbuncle.iter().zip(& carb_clips).map(| ((name, seq), clips) |
        (name.to_string(), json!({ "clips" : clips, "sequence" : steps(seq) })))
        .collect::< serde_json::Map < _, _ >> (), "bases" : (0..4).map(| k | digest(&
        format!("base-{k}"))).collect::< Vec < _ >> (), "background_slots" : (1..8)
        .map(slot).collect::< Vec < _ >> (), "arle" : { "eye_clips" : eye_clips,
        "mouth_clips" : mouth_clips, "combos" : combos.iter().map(|& (e, m) | [e, m])
        .collect::< Vec < _ >> (), "states" : loops.iter().map(| (e, m) | json!({ "eyes"
        : steps(e), "mouth" : steps(m) })).collect::< Vec < _ >> (), "layers" : combos
        .iter().map(|& (e, m) | { let n = format!("arle-{e}-{m}"); let d = digest(& n);
        (n, json!(d)) }).collect::< serde_json::Map < _, _ >> () }, "groups" : [{ "name"
        : "carbuncle", "parts" : ["body", "carb_eyes", "carb_mouth"], "layers" : frames
        .iter().filter(| (n, _) | n.starts_with("carbuncle-")).map(| (n, d) | (n.clone(),
        json!(sha(d)))).collect::< serde_json::Map < _, _ >> () }], "reactions" : [{
        "name" : "impact", "clip" : 25, "position" : [64, 96], "ticks" : 16,
        "layer_sha256" : digest("impact"), "shake" : SHAKE }, { "name" : "eat", "poses" :
        eat_poses.iter().map(|& (e, m) | [e, m]).collect::< Vec < _ >> (), "waiting" :
        eat_poses.iter().position(| p | * p == waiting), "ticks" : length, "timeline" :
        timeline.iter().map(| p | eat_poses.iter().position(| q | q == p).unwrap())
        .collect::< Vec < _ >> (), "layers" : (0..eat_poses.len()).map(| k | { let n =
        format!("eat-{k}"); let d = digest(& n); (n, json!(d)) }).collect::<
        serde_json::Map < _, _ >> () }] }
    );
    let definition = json!(
        { "schema" : 1, "source_sha256" : SOURCE, "healthy" : { "width" : SIZE.0,
        "height" : SIZE.1, "colors" : colors.len(), "pixels_sha256" : sha(& healthy[0]),
        "palette_sha256" : sha(& rgb[0]), "backgrounds" : (0..4).map(| k | json!({ "file"
        : format!("KAO_BG{}.CNS", k + 1), "pixels_sha256" : sha(& healthy[k]),
        "palette_sha256" : sha(& rgb[k]) })).collect::< Vec < _ >> () }, "idle" : idle }
    );
    files.push(("b1-portrait/healthy.indices".into(), healthy[0].clone()));
    files.push(("b1-portrait/healthy.rgb".into(), rgb[0].clone()));
    for k in 0..4 {
        files.push((
            format!("b1-portrait/healthy-{k}.indices").into(),
            healthy[k].clone(),
        ));
        files.push((
            format!("b1-portrait/healthy-{k}.rgb").into(),
            rgb[k].clone(),
        ));
    }
    for (name, data) in frames {
        files.push((format!("b1-portrait/idle/{name}.indices").into(), data));
    }
    let mut bytes = serde_json::to_vec_pretty(&definition)?;
    bytes.push(b'\n');
    files.push(("b1-portrait/portrait.json".into(), bytes));
    Ok(files)
}
