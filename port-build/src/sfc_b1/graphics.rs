use super::*;
pub(super) fn planar(pixels: &[u8], width: usize, tile: usize, bits: usize) -> Vec<u8> {
    let (tx, ty) = (tile % (width / 8), tile / (width / 8));
    let mut bytes = vec![0; bits * 8];
    for y in 0..8 {
        for x in 0..8 {
            let color = pixels[(ty * 8 + y) * width + tx * 8 + x];
            for plane in 0..bits {
                bytes[plane / 2 * 16 + y * 2 + plane % 2] |= ((color >> plane) & 1) << (7 - x);
            }
        }
    }
    bytes
}
pub(super) fn dialogue_page(
    page: &str,
    font: &DialogueFont,
    frame: &crate::sfc_b1_frame::Assets,
    interior: &BTreeMap<u8, u8>,
) -> Result<Vec<Vec<u8>>> {
    let [column, row, columns, rows] = TEXT_CELLS;
    let (width, height) = (columns * 8, rows * 8);
    let mut pixels = vec![0; width * height];
    for y in 0..height {
        for x in 0..width {
            let (sx, sy) = (column * 8 + x, row * 8 + y);
            if crate::sfc_b1_frame::window_interior(sx, sy) {
                pixels[y * width + x] = interior[&frame.background(sx, sy)];
            }
        }
    }
    let mut ink = Vec::new();
    let mut color = DEFAULT_TEXT;
    for (line, text) in page.lines().enumerate() {
        if line >= TEXT_LINES {
            return Err("B1 dialogue page exceeds window lines".into());
        }
        let mut x = TEXT_ORIGIN[0];
        for ch in text.chars() {
            if let Some(next) = crate::b1_dialogue::mark_color(ch) {
                if usize::from(next) > TEXT_COLORS {
                    return Err("B1 text colour outside the dialogue palette".into());
                }
                color = next;
                continue;
            }
            let glyph = font.glyph(ch)?;
            for [dx, dy] in &glyph.ink {
                ink.push([
                    x + dx,
                    TEXT_ORIGIN[1] + line * DIALOGUE_LINE + dy,
                    usize::from(color),
                ]);
            }
            x += glyph.advance;
        }
    }
    for shadow in [true, false] {
        for &[x, y, color] in &ink {
            let (sx, sy, color) = if shadow {
                (x + 1, y + 1, TEXT_SHADOW)
            } else {
                (x, y, crate::sfc_b1_text::ink_index(color as u8))
            };
            if !crate::sfc_b1_frame::window_interior(sx, sy) {
                return Err("B1 dialogue ink outside the window interior".into());
            }
            pixels[(sy - row * 8) * width + sx - column * 8] = color;
        }
    }
    Ok((0..rows)
        .map(|r| {
            (0..columns)
                .flat_map(|c| planar(&pixels, width, r * columns + c, 4))
                .collect()
        })
        .collect())
}
pub(super) fn scale_portrait(
    source: &[u8],
    carbuncle: &[bool],
    palette: &[[f64; 3]],
    order: &[usize],
) -> Vec<u8> {
    let [_, _, pw, ph] = PORTRAIT;
    crate::sfc_portrait_scale::reduce(source, carbuncle, palette, (pw, ph))
        .into_iter()
        .map(|colour| (80 + order[usize::from(colour)]) as u8)
        .collect()
}
pub(super) fn static_screen(
    frame: &crate::sfc_b1_frame::Assets,
    portraits: &[Vec<u8>],
    scale: crate::sfc_portrait_scale::Reduce<'_>,
    idle: &crate::sfc_b1_portrait::Assets,
) -> Result<StaticScreen> {
    let carbuncle = idle.carbuncle_first_pose();
    let mut pixels = frame.pixels.clone();
    let [px, py, pw, ph] = PORTRAIT;
    for (i, color) in scale(&portraits[0], &carbuncle, 0).into_iter().enumerate() {
        pixels[(py + i / pw) * 256 + px + i % pw] = color;
    }
    let [sc, sr, scs, srs] = crate::sfc_b1_stone::CELLS;
    let [dx, dy, _, _] = DUNGEON;
    let mut dynamic = BTreeMap::new();
    for r in 0..DUNGEON_ROWS {
        for c in 0..DUNGEON_COLUMNS {
            dynamic.insert(
                (dy / 8 + r) * 32 + dx / 8 + c,
                DUNGEON_TILE + r * DUNGEON_COLUMNS + c,
            );
        }
    }
    for (i, cell) in frame.cells.iter().enumerate() {
        dynamic.insert(*cell, usize::from(crate::sfc_b1_frame::XP_BASE) / 64 + i);
    }
    for (cell, tile) in idle.cells() {
        let (r, c) = (cell / (pw / 8), cell % (pw / 8));
        dynamic.insert((py / 8 + r) * 32 + px / 8 + c, usize::from(tile));
    }
    for r in 0..srs {
        for c in 0..scs {
            dynamic.insert(
                (sr + r) * 32 + sc + c,
                usize::from(crate::sfc_b1_frame::STONE_BASE) / 64
                    + crate::sfc_b1_stone::slot(r * scs + c),
            );
        }
    }
    for (cell, _) in crate::sfc_b1_equipment::Assets::cells(0) {
        if dynamic.insert(cell, 0).is_some() {
            return Err("Equipment cells overlap other dynamic cells".into());
        }
    }
    for cell in dynamic.keys() {
        for y in 0..8 {
            let at = (cell / 32 * 8 + y) * 256 + cell % 32 * 8;
            pixels[at..at + 8].fill(0);
        }
    }
    let mut portrait_pixels = vec![0; pw * ph];
    for y in 0..ph {
        let at = (py + y) * 256 + px;
        portrait_pixels[y * pw..(y + 1) * pw].copy_from_slice(&pixels[at..at + pw]);
        pixels[at..at + pw].fill(0);
    }
    let panel_cells: Vec<usize> = dynamic
        .keys()
        .filter(|&&cell| {
            let (r, c) = (cell / 32, cell % 32);
            (py / 8..(py + ph) / 8).contains(&r) && (px / 8..(px + pw) / 8).contains(&c)
        })
        .map(|&cell| (cell / 32 - py / 8) * (pw / 8) + cell % 32 - px / 8)
        .collect();
    let variants: Vec<Vec<u8>> = portraits[1..]
        .iter()
        .enumerate()
        .map(|(k, portrait)| {
            let mut panel = scale(portrait, &carbuncle, k + 1);
            for &cell in &panel_cells {
                for y in 0..8 {
                    let at = (cell / (pw / 8) * 8 + y) * pw + cell % (pw / 8) * 8;
                    panel[at..at + 8].fill(0);
                }
            }
            panel
        })
        .collect();
    let image = |width: usize, height: usize, pixels: &[u8], first: usize| {
        let attrs = vec![
            Attributes {
                palette: 0,
                priority: false
            };
            width / 8 * height / 8
        ];
        TileImage::new(
            IndexedImage {
                width,
                height,
                pixels,
                attributes: &attrs,
            },
            Depth::Eight,
            first as u16,
        )
    };
    let screen = image(256, 224, &pixels, STATIC_TILE)?;
    let counted = image(pw, ph, &portrait_pixels, 0)?.native_tiles().len() / 64;
    let portrait_first = (usize::from(idle.first_tile()) - counted).min(0x8000 / 64);
    let block = image(pw, ph, &portrait_pixels, portrait_first)?;
    let mut portrait_tiles = vec![block.native_tiles().to_vec()];
    for variant in &variants {
        let mut tiles: Vec<Option<Vec<u8>>> = vec![None; block.native_tiles().len() / 64];
        for (cell, &word) in block.cells().iter().enumerate() {
            let tile = usize::from(word & 0x3ff) - portrait_first;
            let (h, v) = (word & 0x4000 != 0, word & 0x8000 != 0);
            let (cx, cy) = (cell % (pw / 8), cell / (pw / 8));
            let pixels: Vec<u8> = (0..64)
                .map(|i| {
                    let x = if h { 7 - i % 8 } else { i % 8 };
                    let y = if v { 7 - i / 8 } else { i / 8 };
                    variant[(cy * 8 + y) * pw + cx * 8 + x]
                })
                .collect();
            match &tiles[tile] {
                Some(existing) if *existing != pixels => {
                    return Err("Portrait backgrounds disagree on a shared tile".into());
                }
                _ => tiles[tile] = Some(pixels),
            }
        }
        let mut native = Vec::new();
        for tile in tiles {
            let tile = tile.ok_or("Portrait tile without a cell")?;
            let mut bytes = vec![0; 64];
            for (i, &color) in tile.iter().enumerate() {
                for plane in 0..8 {
                    bytes[plane / 2 * 16 + i / 8 * 2 + plane % 2] |=
                        ((color >> plane) & 1) << (7 - i % 8);
                }
            }
            native.extend(bytes);
        }
        portrait_tiles.push(native);
    }
    let count = portrait_tiles[0].len() / 64;
    let same = |t: usize| {
        portrait_tiles
            .iter()
            .all(|b| b[t * 64..][..64] == portrait_tiles[0][t * 64..][..64])
    };
    let sequence: Vec<usize> = (0..count)
        .filter(|&t| same(t))
        .chain((0..count).filter(|&t| !same(t)))
        .collect();
    let mut slot = vec![0; count];
    for (new, &old) in sequence.iter().enumerate() {
        slot[old] = new;
    }
    for tiles in &mut portrait_tiles {
        *tiles = sequence
            .iter()
            .flat_map(|&t| tiles[t * 64..][..64].to_vec())
            .collect();
    }
    let equipment_first = STATIC_TILE + screen.native_tiles().len() / 64;
    for (cell, tile) in crate::sfc_b1_equipment::Assets::cells(equipment_first) {
        dynamic.insert(cell, tile);
    }
    if equipment_first + crate::sfc_b1_equipment::Assets::tile_count() > portrait_first {
        return Err("B1 frame and portrait exceed BG1 residency".into());
    }
    if (equipment_first + crate::sfc_b1_equipment::Assets::tile_count()) * 64
        > usize::from(crate::sfc_b1_text::ROW8_HOME)
        || portrait_first * 64 < usize::from(crate::sfc_b1_text::BELOW_PORTRAIT)
    {
        return Err("B1 dialogue tiles below the portrait block do not fit".into());
    }
    let mut cells = screen.cells().to_vec();
    for (i, &tile) in block.cells().iter().enumerate() {
        let moved = portrait_first + slot[usize::from(tile & 0x3ff) - portrait_first];
        cells[(py / 8 + i / (pw / 8)) * 32 + px / 8 + i % (pw / 8)] = tile & !0x3ff | moved as u16;
    }
    for (cell, tile) in dynamic {
        cells[cell] = tile as u16;
    }
    let mut map = vec![0; 2048];
    for (i, cell) in cells.iter().enumerate() {
        map[i * 2..i * 2 + 2].copy_from_slice(&cell.to_le_bytes());
    }
    let mut battle_map = map.clone();
    for r in py / 8..(py + ph) / 8 {
        let at = (r * 32 + px / 8) * 2;
        battle_map[at..at + pw / 4].fill(0);
    }
    Ok(StaticScreen {
        frame_tiles: screen.native_tiles().to_vec(),
        portrait_tiles,
        portrait_first,
        map,
        battle_map,
        equipment_first,
    })
}
