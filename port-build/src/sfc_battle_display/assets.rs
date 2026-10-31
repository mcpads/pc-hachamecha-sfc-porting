use super::*;
struct SheetUse {
    file: &'static str,
    slot: u8,
    colors: u16,
    tiles: bool,
}
const SHEETS: [SheetUse; 13] = [
    SheetUse {
        file: "ANM.CNS",
        slot: 27,
        colors: ARLE_COLORS,
        tiles: true,
    },
    SheetUse {
        file: "ANM_ATK.CNS",
        slot: 28,
        colors: ARLE_COLORS,
        tiles: true,
    },
    SheetUse {
        file: "ANM_HEAL.CNS",
        slot: 29,
        colors: ARLE_COLORS,
        tiles: true,
    },
    SheetUse {
        file: "A_DEAD.CNS",
        slot: 65,
        colors: ARLE_COLORS,
        tiles: true,
    },
    SheetUse {
        file: "EFECT.CNS",
        slot: 30,
        colors: 192,
        tiles: false,
    },
    SheetUse {
        file: "AT_EFECT.CNS",
        slot: 31,
        colors: 192,
        tiles: false,
    },
    SheetUse {
        file: "DOKU.CNS",
        slot: 32,
        colors: 144,
        tiles: false,
    },
    SheetUse {
        file: "KAR.CNS",
        slot: 37,
        colors: 240,
        tiles: false,
    },
    SheetUse {
        file: "PUYO.CNS",
        slot: 33,
        colors: 208,
        tiles: false,
    },
    SheetUse {
        file: "SKELTON.CNS",
        slot: 33,
        colors: 208,
        tiles: false,
    },
    SheetUse {
        file: "SCU.CNS",
        slot: 33,
        colors: 208,
        tiles: false,
    },
    SheetUse {
        file: "RULUE.CNS",
        slot: 33,
        colors: 208,
        tiles: false,
    },
    SheetUse {
        file: "RULUE_A.CNS",
        slot: 34,
        colors: 192,
        tiles: false,
    },
];
const SCENE_SHEETS: [SheetUse; 6] = [
    SheetUse {
        file: "RULUE.CNS",
        slot: 45,
        colors: 224,
        tiles: false,
    },
    SheetUse {
        file: "SCHEZO1.CNS",
        slot: 47,
        colors: 208,
        tiles: false,
    },
    SheetUse {
        file: "SCHEZO2.CNS",
        slot: 48,
        colors: 208,
        tiles: false,
    },
    SheetUse {
        file: "SHEZ_EF.CNS",
        slot: 49,
        colors: 192,
        tiles: false,
    },
    SheetUse {
        file: "LB_EF.CNS",
        slot: 51,
        colors: 192,
        tiles: false,
    },
    SheetUse {
        file: "JSTORM.CNS",
        slot: 52,
        colors: 192,
        tiles: false,
    },
];
const SCENE_FIRST_FRAME_ONLY: u8 = 41;
const SCENE_ALIASES: [(u8, &str); 8] = [
    (38, "ANM.CNS"),
    (39, "ANM_ATK.CNS"),
    (40, "ANM_HEAL.CNS"),
    (41, "A_DEAD.CNS"),
    (42, "EFECT.CNS"),
    (43, "AT_EFECT.CNS"),
    (44, "KAR.CNS"),
    (46, "RULUE_A.CNS"),
];
const BACKDROP_SLOT: u8 = 50;
pub(super) struct Sheet {
    width: usize,
    pixels: Vec<u8>,
    rgb: Vec<u8>,
}
pub(super) struct Scaled {
    width: usize,
    height: usize,
    pub(super) pixels: Vec<u8>,
}
pub(super) fn scaled(sheet: &Sheet, rect: [usize; 4], key: u8) -> Scaled {
    let [l, t, r, b] = rect;
    let (w, h) = (r - l, b - t);
    let (width, height) = (scale(w, 104, 224), scale(h, 128, 240));
    let swap = |v: u8| match v {
        v if v == key => 0,
        0 => key,
        v => v,
    };
    let pixels = (0..width * height)
        .map(|i| {
            let (x, y) = (i % width * 224 / 104, i / width * 240 / 128);
            swap(sheet.pixels[(t + y.min(h - 1)) * sheet.width + l + x.min(w - 1)])
        })
        .collect();
    Scaled {
        width,
        height,
        pixels,
    }
}
fn scaled_at(sheet: &Sheet, rect: [usize; 4], key: u8, panel: Panel, at: (i32, i32)) -> Scaled {
    let [l, t, r, b] = rect;
    let (w, h) = ((r - l) as i32, (b - t) as i32);
    let (left, top) = (screen_x(panel, at.0), screen_y(panel, at.1));
    let width = (screen_x(panel, at.0 + w - 1) - left + 1) as usize;
    let height = (screen_y(panel, at.1 + h - 1) - top + 1) as usize;
    let first_x = |s: i32| panel[0] + ((s - panel[2]) * 224 + 103).div_euclid(104);
    let first_y = |s: i32| panel[1] + ((s - panel[3]) * 240 + 127).div_euclid(128);
    let swap = |v: u8| match v {
        v if v == key => 0,
        0 => key,
        v => v,
    };
    let pixels = (0..width * height)
        .map(|i| {
            let x = (first_x(left + (i % width) as i32) - at.0).clamp(0, w - 1) as usize;
            let y = (first_y(top + (i / width) as i32) - at.1).clamp(0, h - 1) as usize;
            swap(sheet.pixels[(t + y) * sheet.width + l + x])
        })
        .collect();
    Scaled {
        width,
        height,
        pixels,
    }
}
fn anchor_align(at: (i32, i32)) -> (usize, usize) {
    (
        screen_x(RIGHT, at.0).rem_euclid(8) as usize,
        screen_y(RIGHT, at.1).rem_euclid(8) as usize,
    )
}
fn planar4(pixel: impl Fn(usize, usize) -> u8) -> [u8; 32] {
    let mut tile = [0; 32];
    for y in 0..8 {
        for x in 0..8 {
            let v = pixel(x, y);
            for plane in 0..4 {
                if v >> plane & 1 != 0 {
                    tile[(plane / 2) * 16 + y * 2 + plane % 2] |= 0x80 >> x;
                }
            }
        }
    }
    tile
}
pub(super) fn palette(rgb: &[u8], key: u8) -> Vec<u8> {
    let mut colors: Vec<u16> = rgb
        .chunks(3)
        .map(|c| u16::from(c[0] >> 3) | u16::from(c[1] >> 3) << 5 | u16::from(c[2] >> 3) << 10)
        .collect();
    colors.swap(0, key.into());
    colors.iter().flat_map(|c| c.to_le_bytes()).collect()
}
pub(super) fn sprite_frame(frame: &Scaled, anchor: (i32, i32)) -> Result<Option<Vec<Chunk>>> {
    let (ax, ay) = (
        (anchor.0 * 104 + 112).div_euclid(224),
        (anchor.1 * 128 + 120).div_euclid(240),
    );
    let at = |x: usize, y: usize| {
        if x < frame.width && y < frame.height {
            frame.pixels[y * frame.width + x]
        } else {
            0
        }
    };
    let mut blocks = Vec::new();
    for by in (0..frame.height).step_by(16) {
        for bx in (0..frame.width).step_by(16) {
            if (0..256).any(|i| at(bx + i % 16, by + i / 16) != 0) {
                let tiles = [(0, 0), (8, 0), (0, 8), (8, 8)]
                    .map(|(dx, dy)| planar4(|x, y| at(bx + dx + x, by + dy + y)));
                blocks.push(((bx as i32 - ax, by as i32 - ay), tiles));
            }
        }
    }
    if blocks.len() > usize::from(ROWS * 8) {
        return Ok(None);
    }
    let blocks = blocks
        .into_iter()
        .map(
            |((dx, dy), tiles)| match (i8::try_from(dx), i8::try_from(dy)) {
                (Ok(x), Ok(y)) => Ok(([(x as u8) ^ 0x80, (y as u8) ^ 0x80], tiles)),
                _ => Err(format!("Sprite block offset ({dx},{dy}) past a signed byte").into()),
            },
        )
        .collect::<Result<Vec<_>>>()?;
    Ok(Some(
        blocks
            .chunks(8)
            .map(|c| Chunk {
                offsets: c.iter().map(|b| b.0).collect(),
                tops: c.iter().flat_map(|b| [b.1[0], b.1[1]].concat()).collect(),
                bottoms: c.iter().flat_map(|b| [b.1[2], b.1[3]].concat()).collect(),
                upper: false,
            })
            .collect(),
    ))
}
fn layered_frame(frame: &Scaled, anchor: (i32, i32)) -> Result<Option<Vec<Chunk>>> {
    if frame.pixels.iter().all(|&v| v < 16) {
        return sprite_frame(frame, anchor);
    }
    if frame.pixels.contains(&16) || frame.pixels.iter().any(|&v| v >= 32) {
        return Err("A 32-colour frame uses colour 16 or past 31".into());
    }
    let layer = |upper: bool| Scaled {
        width: frame.width,
        height: frame.height,
        pixels: frame
            .pixels
            .iter()
            .map(|&v| match (upper, v >= 16) {
                (false, false) => v,
                (true, true) => v - 16,
                _ => 0,
            })
            .collect(),
    };
    let (Some(lower), Some(mut upper)) = (
        sprite_frame(&layer(false), anchor)?,
        sprite_frame(&layer(true), anchor)?,
    ) else {
        return Ok(None);
    };
    upper.iter_mut().for_each(|c| c.upper = true);
    let mut chunks = lower;
    chunks.extend(upper);
    if chunks.iter().map(|c| c.offsets.len()).sum::<usize>() > usize::from(ROWS * 8) {
        return Ok(None);
    }
    Ok(Some(chunks))
}
fn sprite_frames(
    sheet: &Sheet,
    rects: &[[usize; 4]],
    key: u8,
    anchors: &[(i32, i32)],
    file: &str,
    oversized: &mut Vec<(String, usize)>,
) -> Result<Vec<Vec<Chunk>>> {
    let mut frames = Vec::new();
    for (f, (&r, &anchor)) in rects.iter().zip(anchors).enumerate() {
        frames.push(match layered_frame(&scaled(sheet, r, key), anchor)? {
            Some(chunks) => chunks,
            None => {
                oversized.push((file.to_string(), f));
                Vec::new()
            }
        });
    }
    Ok(frames)
}
fn anchors_of(value: &serde_json::Value) -> Result<Vec<(i32, i32)>> {
    let Some(list) = value.as_array() else {
        return Ok(Vec::new());
    };
    list.iter()
        .map(|a| {
            let v = |i: usize| a[i].as_i64().ok_or("Frame anchor").map(|v| v as i32);
            Ok((v(0)?, v(1)?))
        })
        .collect()
}
fn tile_frame(frame: &Scaled, align: (usize, usize)) -> Result<TileFrame> {
    let (ax, ay) = align;
    let (columns, rows) = (
        (ax + frame.width).div_ceil(8),
        (ay + frame.height).div_ceil(8),
    );
    let at = |x: usize, y: usize| {
        if x >= ax && y >= ay && x - ax < frame.width && y - ay < frame.height {
            frame.pixels[(y - ay) * frame.width + x - ax]
        } else {
            0
        }
    };
    let mut ids: BTreeMap<[u8; 32], u16> = BTreeMap::new();
    let mut tiles = Vec::new();
    let mut cells = Vec::new();
    for r in 0..rows {
        for c in 0..columns {
            let tile = planar4(|x, y| at(c * 8 + x, r * 8 + y));
            if tile == [0; 32] {
                cells.push(0);
                continue;
            }
            let next = ids.len();
            let index = *ids.entry(tile).or_insert_with(|| {
                tiles.extend(tile);
                next as u16
            });
            cells.push(index + 1);
        }
    }
    if ids.len() > ARLE_TILES {
        return Err(format!(
            "Arle's battle frame exceeds its BG2 tiles ({} of {ARLE_TILES}, alignment {:?})",
            ids.len(),
            align
        )
        .into());
    }
    Ok(TileFrame {
        columns,
        rows,
        align,
        cells,
        tiles,
    })
}
fn boss_rows(frame: &Scaled, base: u8) -> Result<Vec<u8>> {
    if frame.width != 104 {
        return Err("The boss frame does not span the left panel".into());
    }
    let mut rows = Vec::with_capacity(frame.height * BOSS_ROW);
    for y in 0..frame.height {
        for tile in 0..13 {
            let mut mask = 0u8;
            let mut planes = [0u8; 8];
            for x in 0..8 {
                let v = frame.pixels[y * 104 + tile * 8 + x];
                if v == 0 {
                    continue;
                }
                let colour = base.checked_add(v).ok_or("Boss colour past 255")?;
                mask |= 0x80 >> x;
                for (plane, byte) in planes.iter_mut().enumerate() {
                    if colour >> plane & 1 != 0 {
                        *byte |= 0x80 >> x;
                    }
                }
            }
            rows.push(mask);
            rows.extend(planes);
        }
    }
    Ok(rows)
}
pub(super) fn overlay_rows(rows: &[u8], height: usize) -> Result<(Vec<u8>, Vec<u8>)> {
    let mut data = Vec::new();
    let mut offsets = Vec::new();
    for y in 0..height {
        offsets.extend(u16::try_from(data.len())?.to_le_bytes());
        let row = &rows[y * BOSS_ROW..(y + 1) * BOSS_ROW];
        for opaque in [true, false] {
            let tiles: Vec<(usize, &[u8])> = (0..13)
                .map(|t| (t, &row[t * 9..t * 9 + 9]))
                .filter(|(_, r)| {
                    if opaque {
                        r[0] == 0xff
                    } else {
                        r[0] != 0 && r[0] != 0xff
                    }
                })
                .collect();
            data.extend(u16::try_from(tiles.len())?.to_le_bytes());
            for (t, r) in tiles {
                data.extend(u16::try_from(t * 64)?.to_le_bytes());
                if !opaque {
                    data.extend([!r[0], !r[0]]);
                }
                data.extend(&r[1..9]);
            }
        }
    }
    if data.len() > 0xffff {
        return Err("Boss overlay rows past 64 KiB".into());
    }
    Ok((data, offsets))
}
pub fn load(content: &crate::content::Content) -> Result<Assets> {
    let actors: serde_json::Value = content.json("combat-actors.json")?;
    let entry = &actors["entry_resources"];
    let manifest: serde_json::Value = content.json("battle-bodies.json")?;
    let anchor_file: serde_json::Value = content.json("battle-arle-anchors.json")?;
    let mut anchors = BTreeMap::new();
    for a in anchor_file["anchors"].as_array().ok_or("Arle anchors")? {
        let field = |k: &str| a[k].as_i64().ok_or("Arle anchor field");
        anchors.insert(
            (field("slot")? as u8, field("frame")? as usize),
            (field("x")? as i32, field("y")? as i32),
        );
    }
    let mut sheets = BTreeMap::new();
    for s in manifest["sheets"].as_array().ok_or("Battle sheets")? {
        let file = s["file"].as_str().ok_or("Battle sheet file")?;
        let folder = content.sub("battle-bodies");
        let pixels = folder.read(format!("{file}.indices"))?;
        let rgb = folder.read(format!("{file}.rgb"))?;
        if crate::sha(&pixels) != s["pixels_sha256"] || crate::sha(&rgb) != s["palette_sha256"] {
            return Err(format!("Battle sheet {file} differs from its manifest").into());
        }
        let width = s["width"].as_u64().ok_or("Battle sheet width")? as usize;
        sheets.insert(file.to_string(), Sheet { width, pixels, rgb });
    }
    let commands: Vec<&serde_json::Value> = entry["resource_streams"]
        .as_array()
        .ok_or("Battle resource streams")?
        .iter()
        .flat_map(|s| s["commands"].as_array().into_iter().flatten())
        .collect();
    let rectangles = entry["frame_rectangles"]
        .as_array()
        .ok_or("Battle frame rectangles")?;
    let scene: serde_json::Value = content.json("scene-sheets.json")?;
    let scene_loads: Vec<&serde_json::Value> = scene["streams"]
        .as_array()
        .ok_or("Scene streams")?
        .iter()
        .flat_map(|s| s["actions"].as_array().into_iter().flatten())
        .filter(|a| a["op"] == "load")
        .collect();
    let rects_of = |frames: &serde_json::Value| -> Result<Vec<[usize; 4]>> {
        frames
            .as_array()
            .ok_or("Sheet frames")?
            .iter()
            .map(|f| {
                let r = f
                    .get("rect")
                    .unwrap_or(f)
                    .as_array()
                    .ok_or("Frame rectangle")?;
                let mut out = [0; 4];
                for (i, v) in r.iter().take(4).enumerate() {
                    out[i] = v.as_u64().ok_or("Frame rectangle value")? as usize;
                }
                Ok(out)
            })
            .collect()
    };
    let scene_load = |file: &str, slot: u8| {
        scene_loads
            .iter()
            .find(|a| a["file"] == file && a["slot"] == slot)
            .copied()
            .ok_or_else(|| format!("Scene load of {file}"))
    };
    let mut resources = Vec::new();
    let mut oversized = Vec::new();
    let mut most_tiles = 0;
    let mut tile_counts = BTreeMap::<usize, usize>::new();
    let mut shifted = Vec::new();
    for (use_, in_scene) in SHEETS
        .iter()
        .map(|u| (u, false))
        .chain(SCENE_SHEETS.iter().map(|u| (u, true)))
    {
        let mut frame_anchors = Vec::new();
        let (base, key, rects) = if in_scene {
            let load = scene_load(use_.file, use_.slot)?;
            let base = load["base"].as_u64().ok_or("Scene sheet base")?;
            let key = load["key"].as_u64().ok_or("Scene sheet key")? as u8;
            frame_anchors = anchors_of(&load["centers"])?;
            (base, key, rects_of(&load["frames"])?)
        } else {
            let load = commands
                .iter()
                .find(|c| c["file"] == use_.file && c["slot"] == use_.slot)
                .ok_or(format!("Load of {}", use_.file))?;
            let base = load["flags"].as_u64().ok_or("Sheet palette base")?;
            let key = commands
                .iter()
                .find(|c| {
                    c["opcode"] == 12 && c["slot"] == use_.slot && c["flags"].as_u64() >= Some(base)
                })
                .and_then(|c| c["flags"].as_u64())
                .map_or(0, |k| k - base) as u8;
            let rects = rects_of(
                &rectangles
                    .iter()
                    .find(|r| r["file"] == use_.file)
                    .ok_or(format!("Frames of {}", use_.file))?["frames"],
            )?;
            (base, key, rects)
        };
        frame_anchors.resize(rects.len(), (0, 0));
        let sheet = &sheets[use_.file];
        let frames = if use_.tiles {
            let frames = rects
                .iter()
                .enumerate()
                .map(|(f, &r)| {
                    let at = anchors.get(&(use_.slot, f)).copied().unwrap_or(ARLE_REST);
                    let image = scaled_at(sheet, r, key, RIGHT, at);
                    let (ax, ay) = anchor_align(at);
                    for d in [0i32, -1, 1, -2, 2, -3, 3, -4] {
                        if let Ok(frame) =
                            tile_frame(&image, ((ax as i32 + d).rem_euclid(8) as usize, ay))
                        {
                            if d != 0 {
                                shifted.push((use_.file, f, d));
                            }
                            return Ok(frame);
                        }
                    }
                    tile_frame(&image, (ax, ay))
                })
                .collect::<Result<Vec<_>>>()?;
            most_tiles =
                most_tiles.max(frames.iter().map(|f| f.tiles.len() / 32).max().unwrap_or(0));
            for f in &frames {
                *tile_counts.entry(f.tiles.len() / 32).or_default() += 1;
            }
            Frames::Tiles(frames)
        } else {
            Frames::Sprites(sprite_frames(
                sheet,
                &rects,
                key,
                &frame_anchors,
                use_.file,
                &mut oversized,
            )?)
        };
        resources.push(Resource {
            file: use_.file.to_string(),
            slot: use_.slot,
            rects: rects.clone(),
            used: 0,
            anchors: frame_anchors,
            colors: use_.colors,
            palette: palette(&sheet.rgb, key),
            source_base: u8::try_from(base).map_err(|_| "Sheet palette base")?,
            key,
            rgb: sheet.rgb[..48].to_vec(),
            rgb_upper: sheet.rgb.get(48..96).map(<[u8]>::to_vec),
            frames,
        });
    }
    let resource_of = |file: &str, slot: Option<u8>| {
        resources
            .iter()
            .position(|r: &Resource| r.file == file && slot.is_none_or(|s| r.slot == s))
            .ok_or_else(|| format!("Scene resource {file}"))
    };
    let mut scene_slots = BTreeMap::new();
    for use_ in &SCENE_SHEETS {
        scene_slots.insert(use_.slot, resource_of(use_.file, Some(use_.slot))?);
    }
    for (slot, file) in SCENE_ALIASES {
        let r = resource_of(file, None)?;
        let load = scene_load(file, slot)?;
        let rects = rects_of(&load["frames"])?;
        let battle = rects_of(
            &rectangles
                .iter()
                .find(|x| x["file"] == file)
                .ok_or(format!("Frames of {file}"))?["frames"],
        )?;
        let resource = &resources[r];
        let compared = if slot == SCENE_FIRST_FRAME_ONLY {
            1
        } else {
            rects.len()
        };
        if load["base"].as_u64() != Some(resource.source_base.into())
            || load["key"].as_u64() != Some(resource.key.into())
            || rects.len() > battle.len()
            || rects
                .iter()
                .zip(&battle)
                .take(compared)
                .any(|(a, b)| a != b)
        {
            return Err(format!("Scene slot {slot} does not draw {file} as the battle").into());
        }
        scene_slots.insert(slot, r);
    }
    let mut streams = Vec::new();
    for stream in scene["streams"].as_array().ok_or("Scene streams")? {
        let mut actions = Vec::new();
        for a in stream["actions"].as_array().ok_or("Scene actions")? {
            let slot = a["slot"].as_u64().ok_or("Scene slot")? as u8;
            if a["op"] == "release" {
                actions.push(StreamAction::Release { slot });
            } else if slot != BACKDROP_SLOT {
                let r = *scene_slots
                    .get(&slot)
                    .ok_or_else(|| format!("Scene slot {slot} has no sheet"))?;
                actions.push(StreamAction::Load {
                    slot,
                    resource: u8::try_from(r)?,
                    base: resources[r].source_base,
                });
            }
        }
        streams.push((stream["va"].as_u64().ok_or("Scene stream")? as u32, actions));
    }
    let backdrop_load = scene_loads
        .iter()
        .find(|a| a["slot"] == BACKDROP_SLOT)
        .ok_or("Scene backdrop")?;
    let backdrop_file = backdrop_load["file"].as_str().ok_or("Backdrop file")?;
    if backdrop_load["base"].as_u64() != Some(160)
        || rects_of(&backdrop_load["frames"])? != [[0, 0, 224, 320]]
    {
        return Err("Scene backdrop is not one 224x320 frame at 160".into());
    }
    let backdrop_rgb = sheets[backdrop_file].rgb[..48].to_vec();
    let load_streams: BTreeMap<u64, &Vec<serde_json::Value>> = entry["resource_streams"]
        .as_array()
        .ok_or("Battle resource streams")?
        .iter()
        .filter_map(|s| Some((s["root_va"].as_u64()?, s["commands"].as_array()?)))
        .collect();
    let tables: BTreeMap<u64, &serde_json::Value> = rectangles
        .iter()
        .filter_map(|r| Some((r["resource_va"].as_u64()?, r)))
        .collect();
    let mut enemy_loads = vec![Vec::new(); crate::sfc_battle_scripts::DESCRIPTORS];
    let mut skipped = Vec::new();
    for actor in entry["actors"].as_array().ok_or("Battle actors")? {
        let id = actor["id"].as_u64().ok_or("Battle actor id")? as usize;
        if !(3..crate::sfc_battle_scripts::DESCRIPTORS).contains(&id) {
            continue;
        }
        let roots: Vec<u64> = actor["load_graph"]["nodes"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|n| n["links"].as_array().into_iter().flatten())
            .filter(|l| l["vm"] == "resource")
            .filter_map(|l| l["va"].as_u64())
            .collect();
        if roots.is_empty() {
            return Err(format!("Enemy {id} without a resource stream").into());
        }
        for root in roots {
            let commands = load_streams.get(&root).ok_or("Enemy stream")?;
            let field = |c: &serde_json::Value, k: &str| c[k].as_u64().unwrap_or(0);
            let mut loaded: BTreeMap<u64, (u64, String)> = BTreeMap::new();
            for c in commands.iter() {
                let (op, flags, slot) = (field(c, "opcode"), field(c, "flags"), field(c, "slot"));
                if !(matches!(op, 1 | 2 | 4 | 5 | 6) || op == 10 && flags == 7) {
                    continue;
                }
                let table = tables
                    .get(&field(c, "va"))
                    .ok_or_else(|| format!("Frames of enemy {id} load at {:#x}", field(c, "va")))?;
                let file = table["file"]
                    .as_str()
                    .ok_or("Frame table file")?
                    .to_string();
                let base = if op == 10 {
                    let from = table["derived_from_slot"].as_u64().ok_or("Derived slot")?;
                    loaded.get(&from).ok_or("Derived from an empty slot")?.0
                } else {
                    flags
                };
                loaded.insert(slot, (base, file.clone()));
                let key_slot = if op == 10 {
                    table["derived_from_slot"].as_u64().unwrap_or(slot)
                } else {
                    slot
                };
                let key = commands
                    .iter()
                    .find(|k| field(k, "opcode") == 12 && field(k, "slot") == key_slot)
                    .map_or(0, |k| field(k, "flags").wrapping_sub(base));
                let rects = rects_of(&table["frames"])?;
                let mut anchors = anchors_of(&table["centres"])?;
                anchors.resize(rects.len(), (0, 0));
                if !sheets.contains_key(&file) {
                    skipped.push(serde_json::json!(
                        { "enemy" : id, "file" : file, "slot" : slot }
                    ));
                    continue;
                }
                let colors = match base {
                    192 | 208 | 224 => base as u16,
                    _ => return Err(format!("{file} colours at {base}").into()),
                };
                let (slot, key, base) = (slot as u8, u8::try_from(key)?, base as u8);
                if key >= 16 {
                    return Err(format!("{file} transparent index {key}").into());
                }
                let r = match resources.iter().position(|r| {
                    r.file == file
                        && r.slot == slot
                        && r.source_base == base
                        && r.key == key
                        && r.rects == rects
                        && r.anchors == anchors
                }) {
                    Some(r) => r,
                    None if id <= 10 => {
                        return Err(format!("Enemy {id} sheet {file} is not a battle sheet").into());
                    }
                    None => {
                        let sheet = &sheets[&file];
                        resources.push(Resource {
                            file: file.clone(),
                            slot,
                            rects: rects.clone(),
                            used: 0,
                            anchors: anchors.clone(),
                            colors,
                            palette: palette(&sheet.rgb, key),
                            source_base: base,
                            key,
                            rgb: sheet.rgb[..48].to_vec(),
                            rgb_upper: sheet.rgb.get(48..96).map(<[u8]>::to_vec),
                            frames: Frames::Sprites(sprite_frames(
                                sheet,
                                &rects,
                                key,
                                &anchors,
                                &file,
                                &mut oversized,
                            )?),
                        });
                        resources.len() - 1
                    }
                };
                enemy_loads[id].push((slot, r));
            }
        }
    }
    for loads in enemy_loads.iter_mut() {
        for (_, r) in loads.iter_mut() {
            let short = &resources[*r];
            if let Some(longer) = (0..resources.len())
                .filter(|&o| {
                    let l = &resources[o];
                    l.file == short.file
                        && l.slot == short.slot
                        && l.source_base == short.source_base
                        && l.key == short.key
                        && l.rects.len() > short.rects.len()
                        && l.rects[..short.rects.len()] == short.rects[..]
                        && l.anchors[..short.anchors.len()] == short.anchors[..]
                })
                .max_by_key(|&o| resources[o].rects.len())
            {
                *r = longer;
            }
        }
    }
    for enemy in [38, 39] {
        let resource = enemy_loads[enemy]
            .iter()
            .rev()
            .find(|&&(slot, _)| slot == 36)
            .map(|&(_, resource)| &resources[resource])
            .ok_or("Vertical particle resource is missing")?;
        if resource.file != "LB_EF.CNS" {
            return Err("Vertical particle resource changed".into());
        }
        for &(frame, speed, update) in crate::sfc_battle_objects::vertical_particle_retirements() {
            let &[_, top, _, bottom] = resource
                .rects
                .get(frame)
                .ok_or("Vertical particle frame is missing")?;
            let anchor = resource.anchors.get(frame).map_or(0, |a| a.1);
            let reach = (bottom - top) as i32 - anchor + 2;
            let y = 272 - (speed * update) as i32;
            if y + reach >= LEFT[1] {
                return Err("Vertical particle would retire before leaving the panel".into());
            }
        }
    }
    let mut reserved_streams = Vec::new();
    let mut reserved_roots = Vec::new();
    for scene_ in scene["reserved"]
        .as_object()
        .ok_or("Reserved scenes")?
        .values()
    {
        reserved_roots.push(scene_["root_va"].as_u64().ok_or("Reserved scene root")? as u32);
        for stream in scene_["streams"].as_array().ok_or("Reserved streams")? {
            let mut actions = Vec::new();
            for a in stream["actions"].as_array().ok_or("Reserved actions")? {
                let slot = a["slot"].as_u64().ok_or("Reserved slot")? as u8;
                if a["op"] == "release" {
                    actions.push(StreamAction::Release { slot });
                    continue;
                }
                let file = a["file"].as_str().ok_or("Reserved sheet")?;
                let base = u8::try_from(a["base"].as_u64().ok_or("Reserved base")?)?;
                let key = u8::try_from(a["key"].as_u64().ok_or("Reserved key")?)?;
                let rects = rects_of(&a["frames"])?;
                let r = match resources.iter().position(|r| {
                    r.file == file && r.source_base == base && r.key == key && r.rects == rects
                }) {
                    Some(r) => r,
                    None => {
                        if !matches!(base, 192 | 208 | 224) || key >= 16 {
                            return Err(format!("Reserved {file} at {base}").into());
                        }
                        let sheet = &sheets[file];
                        let anchors = vec![(0, 0); rects.len()];
                        resources.push(Resource {
                            file: file.to_string(),
                            slot,
                            rects: rects.clone(),
                            used: 0,
                            anchors: anchors.clone(),
                            colors: base.into(),
                            palette: palette(&sheet.rgb, key),
                            source_base: base,
                            key,
                            rgb: sheet.rgb[..48].to_vec(),
                            rgb_upper: sheet.rgb.get(48..96).map(<[u8]>::to_vec),
                            frames: Frames::Sprites(sprite_frames(
                                sheet,
                                &rects,
                                key,
                                &anchors,
                                file,
                                &mut oversized,
                            )?),
                        });
                        resources.len() - 1
                    }
                };
                actions.push(StreamAction::Load {
                    slot,
                    resource: u8::try_from(r)?,
                    base,
                });
            }
            reserved_streams.push((
                stream["va"].as_u64().ok_or("Reserved stream")? as u32,
                actions,
            ));
        }
    }
    if reserved_roots != crate::sfc_ending::BATTLE_SCENES {
        return Err("Reserved scenes differ from the ones battle exit plays".into());
    }
    let arle_effect_resource = resources.len();
    resources.push(arle_effects::resource(&resources[0], &sheets["ANM.CNS"])?);
    if resources.len() >= 255 {
        return Err("Battle resources exceed a byte".into());
    }
    for r in &mut resources {
        let sheet = &sheets[&r.file];
        for &[l, t, right, b] in &r.rects {
            for y in t..b {
                for &v in &sheet.pixels[y * sheet.width + l..y * sheet.width + right] {
                    r.used |= 1 << v;
                }
            }
        }
    }
    let report = serde_json::json!(
        { "resources" : resources.iter().map(| r | serde_json::json!({ "file" : r.file,
        "slot" : r.slot, "colors" : r.colors, "source_base" : r.source_base, "key" : r
        .key, "frames" : match & r.frames { Frames::Sprites(f) => f.len(),
        Frames::Tiles(f) => f.len() }, "most_blocks" : match & r.frames {
        Frames::Sprites(f) => f.iter().map(| c | c.iter().map(| c | c.offsets.len())
        .sum::< usize > ()).max().unwrap_or(0), Frames::Tiles(_) => 0 } })).collect::<
        Vec < _ >> (), "arle_effect_resource" : arle_effect_resource,
        "arle_effect_frames" : [12, 22], "arle_effect_colors" : [180, 181, 182, 183],
        "arle_most_tiles" : most_tiles, "arle_tile_counts" : tile_counts,
        "arle_shifted_frames" : shifted.iter().map(| (file, f, d) | serde_json::json!({
        "file" : file, "frame" : f, "pixels" : d })).collect::< Vec < _ >> (),
        "enemy_loads" : enemy_loads.iter().map(| l | l.iter().map(|& (slot, r) |
        serde_json::json!([slot, resources[r].file])).collect::< Vec < _ >> ())
        .collect::< Vec < _ >> (), "sheets_not_carried" : skipped,
        "frames_past_the_cache" : oversized, "action_scripts_not_carried" : crate
        ::sfc_battle_scripts::load(content) ?.uncarried.iter().map(| (ids, raw) |
        serde_json::json!({ "actors" : ids, "first_command" : raw })).collect::< Vec < _
        >> (), "limits" :
        ["Each of Arle's frames is aligned for the position the source shows it at (battle-arle-anchors.json); other positions round to the nearest 8-pixel cell.",
        "The right panel's background (source surface 8) is not drawn: the panel is black.",
        "Scaled draws are drawn unscaled; the bodies' wall shifts (3a) are skipped.",
        "Sprite blocks past the bodies' sprite entries are not drawn that update; a frame the cache cannot place yet shows the object's last drawn frame."],
        }
    );
    let boss = match oversized.as_slice() {
        [] => None,
        [(file, f)] => {
            let (r, resource) = resources
                .iter()
                .enumerate()
                .find(|(_, r)| &r.file == file)
                .ok_or("Missing boss resource")?;
            let sheet = &sheets[file];
            let frame = scaled_at(sheet, resource.rects[*f], resource.key, LEFT, (64, 32));
            let enemies = enemy_loads
                .iter()
                .enumerate()
                .filter(|(_, l)| {
                    l.iter()
                        .any(|&(s, o)| s == resource.slot && resources[o].file == *file)
                })
                .map(|(id, _)| id as u8)
                .collect();
            let _ = r;
            Some(BossView {
                slot: resource.slot,
                frame: *f as u16,
                enemies,
                rows: boss_rows(&frame, resource.source_base)?,
                height: frame.height,
            })
        }
        _ => return Err("More than one frame past the sprite cache".into()),
    };
    Ok(Assets {
        resources,
        arle_effect_resource,
        streams,
        reserved_streams,
        backdrop_rgb,
        enemy_loads,
        boss,
        report,
    })
}
