use super::*;
fn load_sheet(dir: &crate::content::Content, file: &str) -> Result<Sheet> {
    let pixels = dir.read(format!("{file}.indices"))?;
    let rgb = dir.read(format!("{file}.rgb"))?;
    let json: Value = dir.json("ending-scene.json")?;
    let info = json["sheets"]
        .as_array()
        .ok_or("Ending sheets")?
        .iter()
        .find(|s| s["file"] == file)
        .ok_or_else(|| format!("Ending sheet {file}"))?;
    let width = info["width"].as_u64().ok_or("Sheet width")? as usize;
    let height = info["height"].as_u64().ok_or("Sheet height")? as usize;
    if pixels.len() != width * height {
        return Err(format!("Ending sheet {file} size").into());
    }
    let _ = height;
    Ok(Sheet { width, pixels, rgb })
}
fn tile(pixels: &[u8], width: usize, x: usize, y: usize, bits: usize) -> Vec<u8> {
    let mut bytes = vec![0; bits * 8];
    for r in 0..8 {
        for c in 0..8 {
            let v = pixels[(y + r) * width + x + c];
            for plane in 0..bits {
                bytes[plane / 2 * 16 + r * 2 + plane % 2] |= ((v >> plane) & 1) << (7 - c);
            }
        }
    }
    bytes
}
fn half(sheet: &Sheet, rect: [usize; 4]) -> (usize, usize, Vec<u8>) {
    let [l, t, r, b] = rect;
    let (w, h) = ((r - l).div_ceil(2), (b - t).div_ceil(2));
    let pixels = (0..w * h)
        .map(|i| sheet.pixels[(t + 2 * (i / w)) * sheet.width + l + 2 * (i % w)])
        .collect();
    (w, h, pixels)
}
struct Names {
    rows: usize,
    used: Vec<bool>,
}
impl Names {
    fn new(names: usize) -> Names {
        Names {
            rows: names / 16,
            used: vec![false; names],
        }
    }
    fn large(&mut self, k: usize) -> Option<(u16, Vec<u16>)> {
        for r in (0..self.rows.saturating_sub(k - 1)).step_by(k) {
            for c in (0..16).step_by(k) {
                let names: Vec<usize> = (0..k)
                    .flat_map(|y| (0..k).map(move |x| (r + y) * 16 + c + x))
                    .collect();
                if names.iter().all(|&n| !self.used[n]) {
                    for &n in &names {
                        self.used[n] = true;
                    }
                    return Some((names[0] as u16, names.iter().map(|&n| n as u16).collect()));
                }
            }
        }
        None
    }
    fn small(&mut self) -> Option<u16> {
        let n = self.used.iter().position(|u| !u)?;
        self.used[n] = true;
        Some(n as u16)
    }
    fn free(&mut self, names: &[u16]) {
        for &n in names {
            self.used[usize::from(n)] = false;
        }
    }
}
fn obj_uploads(
    base: u16,
    name: u16,
    pixels: &[u8],
    width: usize,
    x: usize,
    y: usize,
    k: usize,
) -> Vec<(Vec<u8>, u16)> {
    (0..k)
        .map(|row| {
            let bytes = (0..k)
                .flat_map(|c| tile(pixels, width, x + 8 * c, y + 8 * row, 4))
                .collect();
            (bytes, base + (name + 16 * row as u16) * 32)
        })
        .collect()
}
pub fn wrap(
    messages: &mut crate::sfc_battle_message::Messages,
    font: &crate::font::DialogueFont,
) -> Result<usize> {
    let roots = (0..9)
        .map(|i| {
            messages
                .root(&format!("ending_{i}"))
                .ok_or_else(|| format!("Ending message {i}").into())
        })
        .collect::<Result<Vec<u32>>>()?;
    messages.wrap(&roots, TEXT_WIDTH, 3, &|ch| font.advance(ch))
}
pub fn prepare(
    content: &crate::content::Content,
    source_colours: &[[u8; 3]],
    breaks: usize,
) -> Result<Assets> {
    let dir = content.sub("ending");
    let json: Value = dir.json("ending-scene.json")?;
    if json["root_va"].as_u64() != Some(u64::from(ROOT))
        || json["descriptor_va"].as_u64() != Some(u64::from(DESCRIPTOR))
    {
        return Err("Ending root differs".into());
    }
    let va = |v: &Value| -> Result<u32> { Ok(v.as_u64().ok_or("Ending address")? as u32) };
    let streams: BTreeMap<u32, &Value> = json["streams"]
        .as_array()
        .ok_or("Ending streams")?
        .iter()
        .map(|s| Ok((va(&s["va"])?, s)))
        .collect::<Result<_>>()?;
    let mut starts: BTreeMap<u8, Vec<(i32, i32)>> = BTreeMap::new();
    for (_, fields) in json["starts"].as_object().ok_or("Ending starts")? {
        let get = |k: &str| fields[k].as_i64().unwrap_or(0) as i32;
        let slot = (get("0x28") >> 16) as u8;
        let y = if fields.get("0x84").is_some() {
            get("0x84")
        } else {
            get("0x20")
        };
        starts
            .entry(slot)
            .or_default()
            .push((get("0x1c") >> 16, y >> 16));
    }
    let kind_of = |file: &str| match file {
        "ED1.CNS" | "ED2.CNS" => Some(Kind::PictureA),
        "ED3_BG.CNS" => Some(Kind::PictureB),
        "ED1_ME.CNS" => Some(Kind::Eyes),
        "ED3_LB.CNS" => Some(Kind::Layer),
        "MOUSE.CNS" => None,
        _ => Some(Kind::Sprite),
    };
    let mut sheets: Vec<(Kind, Frames)> = Vec::new();
    let mut sheet_ids: BTreeMap<(String, u32), u8> = BTreeMap::new();
    let mut loads = Vec::new();
    let mut layout = 1u8;
    let mut names = [Names::new(A_OBJ_NAMES), Names::new(B_OBJ_NAMES)];
    let mut held: BTreeMap<u8, (usize, u8, Vec<u16>)> = BTreeMap::new();
    let mut picture_tiles = 0usize;
    let mut picture: Option<(Sheet, Vec<[u8; 64]>, Vec<u16>)> = None;
    let mut report_loads = Vec::new();
    let mut cropped = 0usize;
    for order in json["stream_order"]
        .as_array()
        .ok_or("Ending stream order")?
    {
        let at = va(order)?;
        let stream = streams.get(&at).ok_or("Ending stream")?;
        let actions = stream["actions"].as_array().ok_or("Stream actions")?;
        if actions.len() != 1 {
            return Err(format!("Ending stream {at:#x} has {} actions", actions.len()).into());
        }
        let a = &actions[0];
        let slot = a["slot"].as_u64().ok_or("Stream slot")? as u8;
        let mut load = Load {
            va: at,
            slot,
            sheet: 0,
            layout: 0,
            uploads: Vec::new(),
            colours: Vec::new(),
            base: 0,
            blocks: (0, 0, 0xff),
            alias: 0,
        };
        if a["op"] == "release" {
            if let Some((sheet, held_layout, held_names)) = held.remove(&slot) {
                let (kind, _) = &sheets[sheet];
                let base = loads
                    .iter()
                    .rev()
                    .find(|l: &&Load| l.slot == slot && l.sheet != 0)
                    .map(|l| l.blocks)
                    .ok_or("Release without its load")?;
                load.blocks = (base.0, base.1, 0xff);
                if *kind == Kind::Sprite {
                    names[usize::from(held_layout) - 1].free(&held_names);
                }
                if *kind == Kind::PictureB {
                    layout = 1;
                    load.layout = 1;
                }
            }
            report_loads.push(serde_json::json!({ "va" : at, "slot" : slot, "release" : true }));
            loads.push(load);
            continue;
        }
        let file = a["file"].as_str().ok_or("Stream file")?;
        let sheet = load_sheet(&dir, file)?;
        let base = a["base"].as_u64().ok_or("Stream base")? as u8;
        let Some(kind) = kind_of(file) else {
            load.base = base;
            load.colours = sheet.rgb.clone();
            loads.push(load);
            continue;
        };
        let key = a["key"].as_u64().map(|k| k as u8);
        let frames: Vec<[usize; 4]> = a["frames"]
            .as_array()
            .ok_or("Stream frames")?
            .iter()
            .map(|f| {
                let v: Vec<usize> = f
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|n| n.as_u64().unwrap() as usize)
                    .collect();
                [v[0], v[1], v[2], v[3]]
            })
            .collect();
        let colours = sheet.rgb.len() / 3;
        let blocks = colours.div_ceil(16) as u8;
        load.base = base;
        load.colours = sheet.rgb.clone();
        load.blocks = (base / 16, blocks, base / 16);
        let id = sheets.len() as u8;
        load.sheet = id + 1;
        let mut names_held = Vec::new();
        let frames_out;
        match kind {
            Kind::PictureA => {
                if frames.len() != 1 || frames[0] != [0, 0, 640, 400] {
                    return Err(format!("{file} is not one 640x400 picture").into());
                }
                if starts.get(&slot) != Some(&vec![(0, 0)]) {
                    return Err(format!("{file} is not drawn at the origin").into());
                }
                let (w, h, px) = half(&sheet, [64, 0, 576, 400]);
                let px: Vec<u8> = px.iter().map(|v| v + base).collect();
                let mut tiles: Vec<[u8; 64]> = vec![[0; 64]];
                let mut index: BTreeMap<[u8; 64], u16> = BTreeMap::new();
                index.insert([0; 64], 0);
                let mut map = vec![0u8; 2048];
                for ty in 0..h / 8 {
                    for tx in 0..w / 8 {
                        let t: [u8; 64] = tile(&px, w, tx * 8, ty * 8, 8).try_into().unwrap();
                        let n = *index.entry(t).or_insert_with(|| {
                            tiles.push(t);
                            (tiles.len() - 1) as u16
                        });
                        map[(ty * 32 + tx) * 2..][..2].copy_from_slice(&n.to_le_bytes());
                    }
                }
                let bytes: Vec<u8> = tiles.iter().flatten().copied().collect();
                if bytes.len() > usize::from(A_TEXT_MAP) {
                    return Err(format!("{file} tiles reach the text map").into());
                }
                for (i, chunk) in bytes.chunks(CHUNK).enumerate() {
                    load.uploads.push((chunk.to_vec(), (i * CHUNK) as u16));
                }
                load.uploads.push((map.clone(), A_BG1_MAP));
                picture_tiles = tiles.len();
                let entries = (0..1024)
                    .map(|i| u16::from_le_bytes([map[i * 2], map[i * 2 + 1]]))
                    .collect();
                picture = Some((sheet, tiles, entries));
                frames_out = Frames::None;
                if layout != 1 {
                    return Err("Picture A loaded in layout B".into());
                }
            }
            Kind::Eyes => {
                let Some((ref picture_sheet, ref mut tiles, ref entries)) = picture else {
                    return Err("Eyes before their picture".into());
                };
                let places = starts.get(&slot).cloned().unwrap_or_default();
                let objects: Vec<(i32, i32)> = json["starts"]
                    .as_object()
                    .unwrap()
                    .iter()
                    .filter(|(k, _)| k.as_str() == "0x4b9ac4" || k.as_str() == "0x4b9b40")
                    .map(|(_, f)| {
                        (
                            (f["0x1c"].as_i64().unwrap() >> 16) as i32,
                            (f["0x20"].as_i64().unwrap() >> 16) as i32,
                        )
                    })
                    .collect();
                let _ = places;
                if objects != [(400, 112), (400, 96)] || frames.len() != 5 {
                    return Err("ED1's eyes are not at their two places".into());
                }
                let place = |f: usize| if f < 4 { objects[0] } else { objects[1] };
                let rect_of = |f: usize| {
                    let [l, t, r, b] = frames[f];
                    let (x, y) = place(f);
                    let x0 = (x - 64) as usize / 2;
                    let y0 = y as usize / 2;
                    [x0, y0, x0 + (r - l) / 2, y0 + (b - t) / 2]
                };
                let mut union = rect_of(0);
                for f in 1..5 {
                    let r = rect_of(f);
                    union = [
                        union[0].min(r[0]),
                        union[1].min(r[1]),
                        union[2].max(r[2]),
                        union[3].max(r[3]),
                    ];
                }
                let [ux, uy, ur, ub] = [
                    union[0] / 8,
                    union[1] / 8,
                    union[2].div_ceil(8),
                    union[3].div_ceil(8),
                ];
                let (pw, _ph, pic) = half(picture_sheet, [64, 0, 576, 400]);
                let pic_base = loads
                    .iter()
                    .rev()
                    .find(|l: &&Load| {
                        l.sheet != 0 && sheets[usize::from(l.sheet - 1)].0 == Kind::PictureA
                    })
                    .map(|l| l.base)
                    .ok_or("Eyes' picture base")?;
                let mut index: BTreeMap<[u8; 64], u16> = tiles
                    .iter()
                    .enumerate()
                    .map(|(i, t)| (*t, i as u16))
                    .collect();
                let first_new = tiles.len();
                let mut patches = Vec::new();
                for (f, &[l, t, r, b]) in frames.iter().enumerate() {
                    let (fw, fh, fpx) = half(&sheet, [l, t, r, b]);
                    let [fx, fy, ..] = rect_of(f);
                    let mut rows = Vec::new();
                    for ty in uy..ub {
                        let mut row = Vec::new();
                        for tx in ux..ur {
                            let mut px = [0u8; 64];
                            for yy in 0..8 {
                                for xx in 0..8 {
                                    let (sx, sy) = (tx * 8 + xx, ty * 8 + yy);
                                    px[yy * 8 + xx] =
                                        if sx >= fx && sx < fx + fw && sy >= fy && sy < fy + fh {
                                            fpx[(sy - fy) * fw + sx - fx] + base
                                        } else {
                                            pic[sy * pw + sx] + pic_base
                                        };
                                }
                            }
                            let t: [u8; 64] = tile(&px, 8, 0, 0, 8).try_into().unwrap();
                            let n = *index.entry(t).or_insert_with(|| {
                                tiles.push(t);
                                (tiles.len() - 1) as u16
                            });
                            row.extend(n.to_le_bytes());
                        }
                        rows.push((A_BG1_MAP + ((ty * 32 + ux) * 2) as u16, row));
                    }
                    patches.push(rows);
                }
                let _ = entries;
                let bytes: Vec<u8> = tiles[first_new..].iter().flatten().copied().collect();
                let start = first_new * 64;
                if start + bytes.len() > usize::from(A_OBJ) {
                    return Err("ED1's eyes reach the sprite tiles".into());
                }
                for (i, chunk) in bytes.chunks(CHUNK).enumerate() {
                    load.uploads
                        .push((chunk.to_vec(), (start + i * CHUNK) as u16));
                }
                picture_tiles = tiles.len();
                frames_out = Frames::Eyes(patches);
            }
            Kind::PictureB => {
                if base != 32 || colours > 16 || frames.len() != 1 || frames[0] != [0, 0, 640, 672]
                {
                    return Err("ED3_BG is not one 640x672 picture at 32".into());
                }
                let used: Vec<bool> = (0..16).map(|c| sheet.pixels.contains(&c)).collect();
                let alias = (1..16)
                    .find(|&c| !used[c])
                    .ok_or("ED3_BG uses every index")? as u8;
                let (w, h, px) = half(&sheet, [64, 0, 576, 672]);
                let px: Vec<u8> = px.iter().map(|&v| if v == 0 { alias } else { v }).collect();
                let mut tiles: Vec<Vec<u8>> = vec![vec![0; 32]];
                let mut index: BTreeMap<Vec<u8>, u16> = BTreeMap::new();
                index.insert(vec![0; 32], 0);
                let mut map = vec![0u8; 4096];
                for ty in 0..h / 8 {
                    for tx in 0..w / 8 {
                        let t = tile(&px, w, tx * 8, ty * 8, 4);
                        let n = *index.entry(t.clone()).or_insert_with(|| {
                            tiles.push(t);
                            (tiles.len() - 1) as u16
                        });
                        let entry = n | 2 << 10;
                        map[(ty * 32 + tx) * 2..][..2].copy_from_slice(&entry.to_le_bytes());
                    }
                }
                let bytes: Vec<u8> = tiles.concat();
                if bytes.len() > usize::from(B_BG1_MAP) {
                    return Err("ED3_BG tiles reach its map".into());
                }
                load.uploads.push((bytes, 0));
                for (i, chunk) in map.chunks(CHUNK).enumerate() {
                    load.uploads
                        .push((chunk.to_vec(), B_BG1_MAP + (i * CHUNK) as u16));
                }
                load.alias = alias;
                load.layout = 2;
                layout = 2;
                frames_out = Frames::None;
            }
            Kind::Layer => {
                if base != 48 || key != Some(0) || frames.len() != 1 || layout != 2 {
                    return Err("ED3_LB is not one layer at 48".into());
                }
                let [l, t, r, b] = frames[0];
                let (w, h, px) = half(&sheet, [l, t, r, b]);
                let (wt, ht) = (w.div_ceil(8), h.div_ceil(8));
                let padded: Vec<u8> = (0..wt * 8 * ht * 8)
                    .map(|i| {
                        let (x, y) = (i % (wt * 8), i / (wt * 8));
                        if x < w && y < h { px[y * w + x] } else { 0 }
                    })
                    .collect();
                let mut tiles: Vec<Vec<u8>> = vec![vec![0; 32]];
                let mut index: BTreeMap<Vec<u8>, u16> = BTreeMap::new();
                index.insert(vec![0; 32], 0);
                let mut map = vec![0u8; 4096];
                for ty in 0..ht {
                    for tx in 0..wt {
                        let t = tile(&padded, wt * 8, tx * 8, ty * 8, 4);
                        let n = *index.entry(t.clone()).or_insert_with(|| {
                            tiles.push(t);
                            (tiles.len() - 1) as u16
                        });
                        let entry = n | 3 << 10 | 1 << 13;
                        map[(ty * 32 + tx) * 2..][..2].copy_from_slice(&entry.to_le_bytes());
                    }
                }
                let bytes: Vec<u8> = tiles.concat();
                if usize::from(B_BG2_TILES) + bytes.len() > usize::from(B_BG2_MAP) {
                    return Err("ED3_LB tiles reach its map".into());
                }
                for (i, chunk) in bytes.chunks(CHUNK).enumerate() {
                    load.uploads
                        .push((chunk.to_vec(), B_BG2_TILES + (i * CHUNK) as u16));
                }
                for (i, chunk) in map.chunks(CHUNK).enumerate() {
                    load.uploads
                        .push((chunk.to_vec(), B_BG2_MAP + (i * CHUNK) as u16));
                }
                frames_out = Frames::None;
            }
            Kind::Sprite => {
                if key != Some(0) {
                    return Err(format!("{file} has a transparent index other than 0").into());
                }
                let dest = match (layout, base) {
                    (1, 80) => 10,
                    (1, 96) => 11,
                    (2, 64..=112) => base / 16 + 8,
                    (_, b) if b >= 128 => b / 16,
                    _ => {
                        return Err(format!("{file}: no sprite palette for base {base}").into());
                    }
                };
                load.blocks = (base / 16, 1, dest);
                let priority: u8 = if layout == 1 { 1 } else { 2 };
                let attribute = (dest - 8) << 1 | priority << 4;
                let obj_base = if layout == 1 { A_OBJ } else { B_OBJ };
                let space = &mut names[usize::from(layout) - 1];
                let spots = starts.get(&slot).cloned().unwrap_or_default();
                let mut shared: BTreeMap<(Vec<u8>, bool), u16> = BTreeMap::new();
                let mut out = Vec::new();
                let big = if layout == 1 { 32 } else { 16 };
                let mut frequency: BTreeMap<Vec<u8>, usize> = BTreeMap::new();
                if layout == 2 {
                    for &rect in &frames {
                        let (w, h, px) = half(&sheet, rect);
                        for y in (0..h).step_by(16) {
                            for x in (0..w).step_by(16) {
                                let block: Vec<u8> = (0..256)
                                    .map(|i| {
                                        let (xx, yy) = (x + i % 16, y + i / 16);
                                        if xx < w && yy < h { px[yy * w + xx] } else { 0 }
                                    })
                                    .collect();
                                *frequency.entry(block).or_default() += 1;
                            }
                        }
                    }
                }
                for (f, &[l, t, r, b]) in frames.iter().enumerate() {
                    let (w, h, px) = half(&sheet, [l, t, r, b]);
                    let small = layout == 1 && w <= 8 && h <= 8;
                    let step = if small { 8 } else { big };
                    let (wt, ht) = (w.div_ceil(step), h.div_ceil(step));
                    let pw = wt * step;
                    let padded: Vec<u8> = (0..pw * ht * step)
                        .map(|i| {
                            let (x, y) = (i % pw, i / pw);
                            if x < w && y < h { px[y * w + x] } else { 0 }
                        })
                        .collect();
                    let mut pieces = Pieces::new();
                    let mut covered = vec![false; wt * ht];
                    let retained = |x: usize| {
                        f != 0
                            || spots.is_empty()
                            || !(41..=48).contains(&slot)
                            || spots.iter().any(|&(sx, _)| {
                                let left = (sx - 64).div_euclid(2) + x as i32;
                                left + 16 > 0 && left < 256
                            })
                    };
                    for by in 0..ht {
                        for bx in 0..wt {
                            if covered[by * wt + bx] {
                                continue;
                            }
                            let (x, y) = (bx * step, by * step);
                            let mut draw_step = step;
                            if layout == 2
                                && bx + 1 < wt
                                && by + 1 < ht
                                && retained(x)
                                && retained(x + 16)
                                && !covered[by * wt + bx + 1]
                                && !covered[(by + 1) * wt + bx]
                                && !covered[(by + 1) * wt + bx + 1]
                            {
                                let unique = (0..2).all(|dy| {
                                    (0..2).all(|dx| {
                                        let block: Vec<u8> = (0..256)
                                            .map(|i| {
                                                padded[(y + dy * 16 + i / 16) * pw
                                                    + x
                                                    + dx * 16
                                                    + i % 16]
                                            })
                                            .collect();
                                        block.iter().any(|&v| v != 0)
                                            && frequency.get(&block) == Some(&1)
                                    })
                                });
                                if unique {
                                    draw_step = 32;
                                    covered[by * wt + bx + 1] = true;
                                    covered[(by + 1) * wt + bx] = true;
                                    covered[(by + 1) * wt + bx + 1] = true;
                                }
                            }
                            let step = draw_step;
                            let block: Vec<u8> = (0..step * step)
                                .map(|i| padded[(y + i / step) * pw + x + i % step])
                                .collect();
                            if block.iter().all(|&v| v == 0) {
                                continue;
                            }
                            if f == 0 && !spots.is_empty() && (41..=48).contains(&slot) {
                                let on = spots.iter().any(|&(sx, _)| {
                                    let left = (sx - 64).div_euclid(2) + x as i32;
                                    left + step as i32 > 0 && left < 256
                                });
                                if !on {
                                    cropped += 1;
                                    continue;
                                }
                            }
                            let name = match shared.get(&(block.clone(), small)) {
                                Some(&n) => n,
                                None => {
                                    let (n, all) = if step == 8 {
                                        space.small().map(|n| (n, vec![n]))
                                    } else {
                                        space.large(step / 8)
                                    }
                                    .ok_or_else(|| format!("Sprite names run out at {file}"))?;
                                    load.uploads.extend(obj_uploads(
                                        obj_base,
                                        n,
                                        &padded,
                                        pw,
                                        x,
                                        y,
                                        step / 8,
                                    ));
                                    names_held.extend(all);
                                    shared.insert((block, small), n);
                                    n
                                }
                            };
                            let tile = (name & 0xff) as u8;
                            let attr = attribute | (name >> 8) as u8;
                            pieces.push((x as i16, y as i16, tile, attr, step == 32));
                        }
                    }
                    out.push(pieces);
                }
                frames_out = Frames::Sprite(out);
            }
        }
        report_loads.push(serde_json::json!(
            { "va" : at, "slot" : slot, "file" : file, "kind" :
            format!("{kind:?}"), "bytes" : load.uploads.iter().map(| u | u.0
            .len()).sum::< usize > (), "layout" : layout }
        ));
        held.insert(slot, (sheets.len(), layout, names_held));
        sheets.push((kind, frames_out));
        sheet_ids.insert((file.to_string(), at), id);
        loads.push(load);
    }
    let _ = picture_tiles;
    let text_map = |palette: u16| -> Vec<u8> {
        let mut map = vec![0u8; 2048];
        for r in 0..6 {
            for c in 0..32 {
                let entry = (r * 32 + c) as u16 | palette << 10 | 1 << 13;
                map[((TEXT_ROW + r) * 32 + c) * 2..][..2].copy_from_slice(&entry.to_le_bytes());
            }
        }
        map
    };
    if source_colours.len() < 2 {
        return Err("Ending text colours".into());
    }
    let report = serde_json::json!(
        { "loads" : report_loads, "sheets" : sheets.len(), "cropped_sprite_blocks" :
        cropped, "text_breaks_added" : breaks, "text_width" : TEXT_WIDTH, "names_left" :
        [names[0].used.iter().filter(| u | !** u).count(), names[1].used.iter().filter(|
        u | !** u).count()] }
    );
    Ok(Assets {
        loads,
        sheets,
        text_maps: [text_map(0), text_map(0)],
        text_colours: [source_colours[0], source_colours[1]],
        report,
    })
}
