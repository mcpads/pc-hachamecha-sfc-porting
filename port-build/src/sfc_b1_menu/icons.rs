use super::*;
pub(super) fn sheet(
    root: &crate::content::Content,
    sheets: &[Sheet],
    name: &str,
) -> Result<(Vec<u8>, Vec<[u8; 3]>, usize)> {
    let s = sheets
        .iter()
        .find(|s| s.file == name)
        .ok_or("Missing menu sheet")?;
    let pixels = root.read(format!("{name}.indices"))?;
    let rgb = root.read(format!("{name}.rgb"))?;
    if sha(&pixels) != s.pixels_sha256
        || sha(&rgb) != s.palette_sha256
        || pixels.len() != s.width * s.height
    {
        return Err("Invalid menu sheet".into());
    }
    Ok((pixels, rgb.as_chunks::<3>().0.to_vec(), s.width))
}
pub(super) fn rgb15(c: [u8; 3]) -> u16 {
    u16::from(c[0] >> 3) | u16::from(c[1] >> 3) << 5 | u16::from(c[2] >> 3) << 10
}
pub(super) fn tiles(pixels: &[u8], width: usize) -> Vec<u8> {
    let height = pixels.len() / width;
    let mut bytes = Vec::new();
    for ty in 0..height / 8 {
        for tx in 0..width / 8 {
            let mut tile = [0u8; 32];
            for y in 0..8 {
                for x in 0..8 {
                    let c = pixels[(ty * 8 + y) * width + tx * 8 + x];
                    for plane in 0..4 {
                        tile[plane / 2 * 16 + y * 2 + plane % 2] |= ((c >> plane) & 1) << (7 - x);
                    }
                }
            }
            bytes.extend(tile);
        }
    }
    bytes
}
pub(super) fn pack_merging(icons: &mut [Vec<u16>]) -> Result<Packed> {
    let mut merges = Vec::new();
    loop {
        if let Ok((slots, cgram)) = pack(icons) {
            return Ok((slots, cgram, merges));
        }
        let mut counts: BTreeMap<u16, usize> = BTreeMap::new();
        for c in icons.iter().flatten() {
            *counts.entry(*c).or_default() += 1;
        }
        let channel = |c: u16, s: u16| i32::from((c >> s) & 31);
        let distance = |a: u16, b: u16| {
            [0, 5, 10]
                .map(|s| (channel(a, s) - channel(b, s)).pow(2))
                .iter()
                .sum::<i32>()
        };
        let colors: Vec<u16> = counts.keys().copied().collect();
        let (from, to) = colors
            .iter()
            .enumerate()
            .flat_map(|(i, &a)| colors[i + 1..].iter().map(move |&b| (a, b)))
            .min_by_key(|&(a, b)| (distance(a, b), counts[&a].min(counts[&b])))
            .map(|(a, b)| {
                if counts[&a] < counts[&b] {
                    (a, b)
                } else {
                    (b, a)
                }
            })
            .ok_or("Menu icons exceed two sprite palettes")?;
        for c in icons.iter_mut().flatten() {
            if *c == from {
                *c = to;
            }
        }
        merges.push([from, to]);
    }
}
fn pack(icons: &[Vec<u16>]) -> Result<(Vec<u8>, Vec<u8>)> {
    let mut sets: [Vec<u16>; 2] = Default::default();
    let mut slots = vec![0u8; icons.len()];
    let mut order: Vec<usize> = (0..icons.len()).collect();
    let distinct = |i: usize| -> Vec<u16> {
        let mut c = icons[i].clone();
        c.sort();
        c.dedup();
        c
    };
    order.sort_by_key(|&i| std::cmp::Reverse(distinct(i).len()));
    for i in order {
        let colors = distinct(i);
        let fit = (0..2)
            .filter(|&p| {
                colors.iter().filter(|c| !sets[p].contains(c)).count() + sets[p].len() <= 15
            })
            .max_by_key(|&p| {
                (
                    colors.iter().filter(|c| sets[p].contains(c)).count(),
                    std::cmp::Reverse(p),
                )
            })
            .ok_or("Menu icons exceed two sprite palettes")?;
        for c in colors {
            if !sets[fit].contains(&c) {
                sets[fit].push(c);
            }
        }
        slots[i] = ICON_PALETTES[fit];
    }
    let mut cgram = Vec::new();
    for set in &sets {
        let mut palette = [0u16; 16];
        palette[1..=set.len()].copy_from_slice(set);
        cgram.extend(palette.iter().flat_map(|c| c.to_le_bytes()));
    }
    Ok((slots, cgram))
}
