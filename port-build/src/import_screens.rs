use crate::{
    Result, SOURCE,
    import::Files,
    original::{Exe, Originals},
    script::{Options, decode_graph, decode_resource_script},
    sha,
};
use serde_json::{Value, json};
fn json(value: Value) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(&value)?;
    bytes.push(b'\n');
    Ok(bytes)
}
fn u16s(exe: &Exe, at: u32, count: usize) -> Result<Vec<u16>> {
    Ok(exe
        .read(at, count * 2)?
        .chunks(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect())
}
fn i32s(exe: &Exe, at: u32, count: usize) -> Result<Vec<i32>> {
    Ok(exe
        .read(at, count * 4)?
        .chunks(4)
        .map(|c| i32::from_le_bytes(c.try_into().unwrap()))
        .collect())
}
fn rectangles(exe: &Exe, at: u32, count: usize) -> Result<Vec<Vec<i32>>> {
    Ok(i32s(exe, at, count * 4)?
        .chunks(4)
        .map(<[i32]>::to_vec)
        .collect())
}
fn sheet(files: &mut Files, folder: &str, originals: &Originals, name: &str) -> Result<Value> {
    let sheet = originals.cns(name)?;
    files.push((
        format!("{folder}/{name}.indices").into(),
        sheet.pixels.clone(),
    ));
    files.push((format!("{folder}/{name}.rgb").into(), sheet.rgb.clone()));
    Ok(json!(
        { "file" : name, "width" : sheet.width, "height" : sheet.height, "colors" :
        sheet.colors, "pixels_sha256" : sha(& sheet.pixels), "palette_sha256" : sha(&
        sheet.rgb) }
    ))
}
fn hud(originals: &Originals, files: &mut Files) -> Result<()> {
    let exe = originals.exe()?;
    let commands = decode_resource_script(exe, 0x44444c)?;
    let mut resources = Vec::new();
    for name in [
        "FRAME.CNS",
        "WAKU.CNS",
        "JISHAKU.CNS",
        "TAMA.CNS",
        "MOUSE.CNS",
    ] {
        let mut resource = sheet(files, "hud", originals, name)?;
        let load = commands
            .iter()
            .find(|c| c.file.as_deref() == Some(name))
            .ok_or("Missing HUD load")?;
        resource["slot"] = load.slot.into();
        resource["load_flags"] = load.flags.into();
        resource["key"] = commands
            .iter()
            .find(|c| c.opcode == 12 && c.slot == load.slot)
            .map(|c| c.flags)
            .into();
        resources.push(resource);
    }
    let mut sequences = Vec::new();
    for at in i32s(exe, 0x446100, 12)? {
        let (mut cursor, mut frames) = (at as u32, Vec::new());
        while exe.read(cursor, 1)?[0] == 0x21 {
            let [_, mode] = exe.read(cursor, 2)?.try_into()?;
            let [hold, frame, slot] = u16s(exe, cursor + 2, 3)?.try_into().unwrap();
            if mode != 0 || slot != 20 || frame >= 72 {
                return Err("Unexpected stone frame command".into());
            }
            frames.push(json!({ "frame" : frame, "hold" : hold }));
            cursor += 8;
        }
        if exe.read(cursor, 4)? != [3, 0, 0, 0] {
            return Err("Missing stone sequence jump".into());
        }
        sequences.push(json!(
            { "source_va" : at, "frames" : frames, "next_va" : exe.u32(cursor +
            4) ? }
        ));
    }
    let mut palettes = Vec::new();
    for at in [0x446684, 0x4466c5, 0x446706, 0x446747] {
        let values = exe.read(at, 65)?;
        if values[64] != 255 || (0..16).any(|i| usize::from(values[i * 4]) != 144 + i) {
            return Err("Unexpected stone palette command".into());
        }
        let rgb: Vec<&[u8]> = (0..16).map(|i| &values[i * 4 + 1..i * 4 + 4]).collect();
        palettes.push(json!({ "source_va" : at, "rgb" : rgb, "component_bits" : 4 }));
    }
    for (at, expected) in [
        (0x4183e2, 0x49c000u32.to_le_bytes().to_vec()),
        (0x418421, 0x130u32.to_le_bytes().to_vec()),
        (0x418434, 0x4b4862u32.to_le_bytes().to_vec()),
        (0x418465, 0x49c068u32.to_le_bytes().to_vec()),
        (0x41846a, vec![0x6a, 0x12]),
        (0x418478, 0x49c004u32.to_le_bytes().to_vec()),
        (0x4184b7, 0x140u32.to_le_bytes().to_vec()),
        (0x4184ca, 0x4b4863u32.to_le_bytes().to_vec()),
    ] {
        if exe.read(at, expected.len())? != expected {
            return Err("Unexpected equipment draw".into());
        }
    }
    let boxes: Vec<Value> = (0..2)
        .map(|i| {
            Ok(json!(
                { "position" : u16s(exe, 0x49c000 + i * 4, 2) ?, "rectangle" :
                i32s(exe, 0x49c108 + 0x130 + i * 16, 4) ?, "item_va" : 0x4b4862 + i }
            ))
        })
        .collect::<Result<_>>()?;
    let turns: Vec<u8> = (0..4)
        .flat_map(|old| {
            (0..4).map(move |new| {
                if old == new {
                    new * 3
                } else {
                    new * 3 + old / 2 + 1
                }
            })
        })
        .collect();
    let positions: Vec<Vec<u16>> = (0..21)
        .map(|i| u16s(exe, 0x49bfa8 + i * 4, 2))
        .collect::<Result<_>>()?;
    let hud = json!(
        { "schema" : 1, "source_sha256" : SOURCE, "resources" : resources, "stone" : {
        "position" : [300, 228], "size" : [40, 40], "sequences" : sequences,
        "turn_sequences" : turns, "palettes" : palettes, "initial_mp" : u16s(exe,
        0x4b4850, 2) ? }, "experience" : { "positions" : positions, "gem_rectangles" :
        rectangles(exe, 0x49c008, 6) ? }, "equipment" : { "size" : [32, 32],
        "box_resource" : "FRAME.CNS", "icon_resource" : "MOUSE.CNS", "icon_first_item" :
        88, "icon_rectangles" : rectangles(exe, 0x49c068, 10) ?, "boxes" : boxes } }
    );
    files.push(("hud/hud.json".into(), json(hud)?));
    Ok(())
}
fn title(originals: &Originals, files: &mut Files) -> Result<()> {
    let exe = originals.exe()?;
    let mut resources = Vec::new();
    for at in [0x45f460, 0x45f478, 0x45f494] {
        let stream = decode_resource_script(exe, at)?;
        let load = &stream[0];
        let file = load.file.as_deref().ok_or("Title stream without a load")?;
        let mut resource = sheet(files, "title", originals, file)?;
        resource["palette_base"] = load.flags.into();
        resource["key"] = stream
            .iter()
            .find(|c| c.opcode == 12)
            .map(|c| c.flags)
            .into();
        resources.push(resource);
    }
    let rects = rectangles(exe, 0x45efa7, 26)?;
    let graph = decode_graph(exe, &[0x45f224], &Options::default())?;
    let mut draws = vec![
        json!({ "file" : "TIT_BG.CNS", "rectangle" : [0, 0, 640, 480], "position" : [0,
        0] }),
        json!({ "file" : "LOGO.CNS", "rectangle" : [0, 0, 640, 240], "position" :
        [0, 64] }),
    ];
    for at in [
        0x460248, 0x460344, 0x460440, 0x46053c, 0x460830, 0x46092c, 0x460a28, 0x460b24, 0x45f578,
        0x45f6c0, 0x45f808, 0x45f950, 0x45fd28, 0x45fe70, 0x45ffb8, 0x460100,
    ] {
        let raw = &graph.nodes.get(&at).ok_or("Missing subtitle command")?.raw;
        let field = |offset: u8| -> Result<i32> {
            let i = raw
                .windows(4)
                .position(|w| w == [3, offset, 0, 0])
                .ok_or("Missing subtitle field")?;
            Ok(i32::from_le_bytes(raw[i + 4..i + 8].try_into()?))
        };
        let packed = field(0x28)?;
        if packed >> 16 != 39 {
            return Err("Unexpected subtitle slot".into());
        }
        draws.push(json!(
            { "file" : "SUBTIT.CNS", "rectangle" : rects[(packed & 0xffff) as
            usize], "position" : [field(0x80) ?.div_euclid(65536), field(0x84)
            ?.div_euclid(65536)] }
        ));
    }
    draws.push(json!(
        { "file" : "SUBTIT.CNS", "rectangle" : rects[16], "position" : [248, 456]
        }
    ));
    let title = json!(
        { "schema" : 1, "source_sha256" : SOURCE, "resources" : resources, "draws" :
        draws, "source_rectangles" : rects }
    );
    files.push(("title/title.json".into(), json(title)?));
    Ok(())
}
pub fn import(originals: &Originals) -> Result<Files> {
    let mut files = Files::new();
    hud(originals, &mut files)?;
    title(originals, &mut files)?;
    Ok(files)
}
