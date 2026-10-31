use crate::{Result, SOURCE, sha};
use serde::Deserialize;
use sfc_builder::cartridge::{CartridgeLayout, Section};
use std::collections::BTreeSet;
pub const XP_BASE: u16 = 0xa800;
const XP_END: u16 = 0xb3c0;
pub const STONE_BASE: u16 = 0xbc00;
#[derive(Deserialize)]
struct Resource {
    file: String,
    pixels_sha256: String,
    palette_sha256: String,
}
#[derive(Deserialize)]
struct Experience {
    positions: Vec<[usize; 2]>,
    gem_rectangles: Vec<[usize; 4]>,
}
#[derive(Deserialize)]
struct Definition {
    schema: u8,
    source_sha256: String,
    resources: Vec<Resource>,
    experience: Experience,
}
pub struct Assets {
    pub pixels: Vec<u8>,
    pub palette: Vec<u8>,
    source_image: Vec<u8>,
    variants: crate::sfc_b1_view::Table,
    drawn: [bool; 256],
    pub cells: Vec<usize>,
}
const COLUMNS: [[usize; 4]; 5] = [
    [0, 8, 0, 64],
    [8, 112, 64, 288],
    [112, 144, 288, 352],
    [144, 248, 352, 576],
    [248, 256, 576, 640],
];
const ROWS: [[usize; 4]; 5] = [
    [0, 8, 0, 32],
    [8, 136, 32, 272],
    [136, 144, 272, 336],
    [144, 216, 336, 448],
    [216, 224, 448, 480],
];
const WINDOW_INTERIOR: [usize; 4] = [83, 339, 557, 445];
fn axis(value: usize, spans: &[[usize; 4]; 5]) -> usize {
    let [start, end, from, to] = *spans
        .iter()
        .find(|s| (s[0]..s[1]).contains(&value))
        .expect("SFC coordinate inside screen");
    from + (value - start) * (to - from) / (end - start)
}
pub fn source(x: usize, y: usize) -> [usize; 2] {
    [axis(x, &COLUMNS), axis(y, &ROWS)]
}
pub fn panel(x: usize, y: usize) -> bool {
    (8..136).contains(&y) && ((8..112).contains(&x) || (144..248).contains(&x))
}
pub fn window_interior(x: usize, y: usize) -> bool {
    let [sx, sy] = source(x, y);
    let [l, t, r, b] = WINDOW_INTERIOR;
    (l..r).contains(&sx) && (t..b).contains(&sy)
}
fn mapped(image: &[u8]) -> Vec<u8> {
    (0..256 * 224)
        .map(|i| {
            let (x, y) = (i % 256, i / 256);
            if panel(x, y) || window_interior(x, y) {
                0
            } else {
                let [sx, sy] = source(x, y);
                image[sy * 640 + sx]
            }
        })
        .collect()
}
fn forward(x2: usize, y2: usize) -> [usize; 2] {
    let axis2 = |v2: usize, spans: &[[usize; 4]; 5]| {
        let [start, end, from, to] = *spans
            .iter()
            .find(|s| (2 * s[2]..2 * s[3]).contains(&v2))
            .unwrap_or(&spans[4]);
        (2 * start * (to - from) + (v2 - 2 * from) * (end - start)) / (2 * (to - from))
    };
    [axis2(x2, &COLUMNS), axis2(y2, &ROWS)]
}
type GemPiece = (Vec<Option<u8>>, [usize; 4]);
fn gem_piece(gems: &[u8], [l, t, r, b]: [usize; 4]) -> Result<GemPiece> {
    let at = |x: usize, y: usize| gems[(t + y) * 96 + l + x];
    let (w, h) = (r - l, b - t);
    let drawn: Vec<(usize, usize)> = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .filter(|&(x, y)| at(x, y) != 6)
        .collect();
    let x0 = drawn
        .iter()
        .map(|p| p.0)
        .min()
        .ok_or("Empty experience gem")?;
    let y0 = drawn
        .iter()
        .map(|p| p.1)
        .min()
        .ok_or("Empty experience gem")?;
    let x1 = drawn
        .iter()
        .map(|p| p.0)
        .max()
        .ok_or("Empty experience gem")?
        + 1;
    let y1 = drawn
        .iter()
        .map(|p| p.1)
        .max()
        .ok_or("Empty experience gem")?
        + 1;
    let piece = (y0..y1)
        .flat_map(|y| (x0..x1).map(move |x| (x, y)))
        .map(|(x, y)| (at(x, y) != 6).then(|| 10 + at(x, y)))
        .collect();
    Ok((piece, [x0, y0, x1 - x0, y1 - y0]))
}
fn reduce_gem(
    piece: &[Option<u8>],
    w: usize,
    h: usize,
    ow: usize,
    oh: usize,
    rgb: &dyn Fn(u8) -> [i32; 3],
) -> Vec<Option<u8>> {
    let mut colours: Vec<u8> = piece.iter().flatten().copied().collect();
    colours.sort_unstable();
    colours.dedup();
    let mut out = vec![None; ow * oh];
    for oy in 0..oh {
        for ox in 0..ow {
            let (mut total, mut covered) = (0i64, 0i64);
            let mut sum = [0i64; 3];
            for sy in oy * h / oh..((oy + 1) * h).div_ceil(oh) {
                let wy = ((oy + 1) * h).min((sy + 1) * oh) as i64 - (oy * h).max(sy * oh) as i64;
                for sx in ox * w / ow..((ox + 1) * w).div_ceil(ow) {
                    let wx =
                        ((ox + 1) * w).min((sx + 1) * ow) as i64 - (ox * w).max(sx * ow) as i64;
                    if wx <= 0 || wy <= 0 {
                        continue;
                    }
                    total += wx * wy;
                    if let Some(c) = piece[sy * w + sx] {
                        covered += wx * wy;
                        for (s, v) in sum.iter_mut().zip(rgb(c)) {
                            *s += wx * wy * i64::from(v);
                        }
                    }
                }
            }
            if covered == 0 || covered * 2 < total {
                continue;
            }
            out[oy * ow + ox] = colours.iter().copied().min_by_key(|&c| {
                rgb(c)
                    .iter()
                    .zip(sum)
                    .map(|(&v, s)| {
                        let d = i64::from(v) * covered - s;
                        d * d
                    })
                    .sum::<i64>()
            });
        }
    }
    out
}
impl Assets {
    pub fn units_report(&self) -> serde_json::Value {
        self.variants.report()
    }
    pub fn overlay_tiles(&self) -> usize {
        self.cells.len()
    }
    pub fn source_pixel(&self, x: usize, y: usize) -> u8 {
        self.source_image[y * 640 + x]
    }
    pub fn draws(&self, colour: usize) -> bool {
        self.drawn.get(colour).copied().unwrap_or(false)
    }
    pub fn background(&self, x: usize, y: usize) -> u8 {
        let [sx, sy] = source(x, y);
        self.source_image[sy * 640 + sx]
    }
    pub fn load(content: &crate::content::Content) -> Result<Self> {
        let root = content.sub("hud");
        let def: Definition = root.json("hud.json")?;
        if def.schema != 1
            || def.source_sha256 != SOURCE
            || def.experience.positions.len() != 21
            || def.experience.gem_rectangles.len() != 6
        {
            return Err("Invalid frame source".into());
        }
        let read = |name: &str| -> Result<(Vec<u8>, Vec<u8>)> {
            let r = def
                .resources
                .iter()
                .find(|r| r.file == name)
                .ok_or("Missing frame resource")?;
            let pixels = root.read(format!("{name}.indices"))?;
            let rgb = root.read(format!("{name}.rgb"))?;
            if sha(&pixels) != r.pixels_sha256 || sha(&rgb) != r.palette_sha256 || rgb.len() != 768
            {
                return Err("Invalid frame resource hash".into());
            }
            Ok((pixels, rgb))
        };
        let (frame, frame_rgb) = read("FRAME.CNS")?;
        let (gems, gem_rgb) = read("TAMA.CNS")?;
        let (_, window_rgb) = read("WAKU.CNS")?;
        if frame.len() != 640 * 480
            || gems.len() != 96 * 48
            || frame.iter().any(|&p| p >= 32)
            || gems.iter().any(|&p| !matches!(p, 0..=6 | 21))
        {
            return Err("Invalid frame pixel extent".into());
        }
        let mut palette = vec![0; 512];
        for (base, rgb, count) in [
            (10, &gem_rgb, 22),
            (16, &frame_rgb, 32),
            (16, &window_rgb, 16),
        ] {
            for (i, c) in rgb.as_chunks::<3>().0.iter().take(count).enumerate() {
                let value = u16::from(c[0] >> 3)
                    | (u16::from(c[1] >> 3) << 5)
                    | (u16::from(c[2] >> 3) << 10);
                palette[(base + i) * 2..(base + i + 1) * 2].copy_from_slice(&value.to_le_bytes());
            }
        }
        let base: Vec<u8> = frame.iter().map(|v| 16 + v).collect();
        let mut drawn = [false; 256];
        for c in base.iter().copied().chain(gems.iter().map(|g| 10 + g)) {
            drawn[usize::from(c)] = true;
        }
        let pixels = mapped(&base);
        let rgb = |c: u8| -> [i32; 3] {
            let v =
                u16::from_le_bytes([palette[usize::from(c) * 2], palette[usize::from(c) * 2 + 1]]);
            [
                i32::from(v & 31),
                i32::from((v >> 5) & 31),
                i32::from(v >> 10),
            ]
        };
        let mut images = vec![pixels.clone()];
        let mut image = pixels.clone();
        for step in 1..=57 {
            let position = if step >= 55 {
                18 + step % 3
            } else {
                (step - 1) / 3
            };
            let index = if step >= 55 { 3 + step % 3 } else { step % 3 };
            let [x, y] = def.experience.positions[position];
            let [l, t, r, b] = def.experience.gem_rectangles[index];
            if r > 96 || b > 48 || r <= l || b <= t || x + r - l > 640 || y + b - t > 480 {
                return Err("Invalid experience gem rectangle".into());
            }
            let (piece, [bx, by, bw, bh]) = gem_piece(&gems, [l, t, r, b])?;
            let centre = position >= 18;
            let (ow, oh) = (bw.div_ceil(2), bh.div_ceil(2));
            let reduced = reduce_gem(&piece, bw, bh, ow, oh, &rgb);
            let (x0, y0) = if centre {
                (128 - ow / 2, 12 - oh / 2)
            } else {
                let [cx, cy] = forward(2 * (x + bx) + bw, 2 * (y + by) + bh);
                let y0 = cy.saturating_sub(oh / 2);
                if x < 64 {
                    ((8 - ow) / 2, y0.min(224 - oh))
                } else if x >= 576 {
                    (248 + (8 - ow) / 2, y0.min(224 - oh))
                } else if y < 32 {
                    (cx.saturating_sub(4) + (8 - ow) / 2, (8 - oh) / 2)
                } else {
                    return Err("Experience gem off the frame borders".into());
                }
            };
            for dy in 0..oh {
                for dx in 0..ow {
                    if let Some(c) = reduced[dy * ow + dx] {
                        let (px, py) = (x0 + dx, y0 + dy);
                        if panel(px, py) || window_interior(px, py) {
                            return Err("Experience gem over a panel".into());
                        }
                        image[py * 256 + px] = c;
                    }
                }
            }
            images.push(image.clone());
        }
        let cells: Vec<usize> = images
            .iter()
            .flat_map(|image| {
                image
                    .iter()
                    .zip(&pixels)
                    .enumerate()
                    .filter_map(|(i, (a, b))| (a != b).then_some((i / 256 / 8) * 32 + i % 256 / 8))
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let slots = usize::from(XP_END - XP_BASE) / 64;
        let mut cells = cells;
        for cell in (3..17).flat_map(|row| (14..18).map(move |column| row * 32 + column)) {
            if cells.len() >= slots {
                break;
            }
            if !cells.contains(&cell) {
                cells.push(cell);
            }
        }
        cells.sort_unstable();
        if cells.is_empty() || cells.len() * 64 > usize::from(XP_END - XP_BASE) {
            return Err("Experience overlay exceeds reserved VRAM".into());
        }
        let variants: Vec<Vec<u8>> = images
            .iter()
            .map(|image| {
                let mut tiles = vec![0; cells.len() * 64];
                for (n, cell) in cells.iter().enumerate() {
                    for y in 0..8 {
                        for x in 0..8 {
                            let color = image[(cell / 32 * 8 + y) * 256 + cell % 32 * 8 + x];
                            for p in 0..8 {
                                tiles[n * 64 + p / 2 * 16 + y * 2 + p % 2] |=
                                    ((color >> p) & 1) << (7 - x);
                            }
                        }
                    }
                }
                tiles
            })
            .collect();
        let variants = crate::sfc_b1_view::Table::new(
            "b1_experience",
            &variants
                .iter()
                .map(|v| vec![v.as_slice()])
                .collect::<Vec<_>>(),
        )?;
        Ok(Self {
            pixels,
            palette,
            source_image: base,
            variants,
            drawn,
            cells,
        })
    }
    pub fn append(
        &self,
        sections: &mut Vec<Section>,
        space: &mut crate::sfc_rom::Space,
    ) -> Result<()> {
        let mut jobs = Vec::new();
        for record in self.variants.append(sections, space)? {
            jobs.extend(crate::sfc_b1_view::descriptor(
                crate::sfc_b1_view::EXPERIENCE_KIND,
                usize::from(XP_BASE),
                self.variants.block_bytes(),
                record,
            )?);
        }
        super::sfc_b1::add(sections, space, "b1_experience_jobs".into(), jobs)?;
        Ok(())
    }
}
pub fn emit(p: &mut w65c816::Assembler, layout: &CartridgeLayout) -> Result<()> {
    use crate::sfc::{emit, implied};
    use w65c816::{AddressingMode as A, CpuMode, Mnemonic as M, Operand as O};
    let abs = |p: &mut w65c816::Assembler, m, a| emit(p, m, A::Absolute, O::Word(a));
    let jobs_section = layout.section("b1_experience_jobs")?;
    let jobs = layout.cpu_address(jobs_section.offset)?;
    let bytes = u16::from_le_bytes([jobs_section.bytes[3], jobs_section.bytes[4]]);
    crate::sfc_b1_view::emit_block(
        p,
        "b1_descriptor_experience",
        crate::sfc_b1_view::segment_units(usize::from(bytes)),
    )?;
    const SHOWN_GEMS: u16 = crate::sfc_battle::STATS + 0x24;
    p.label("b1_experience_tick")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, SHOWN_GEMS + 1);
    p.emit_label_ref(M::Bne, A::Relative8, "experience_full");
    abs(p, M::Lda, SHOWN_GEMS);
    emit(p, M::Cmp, A::Immediate, O::Byte(58));
    p.emit_label_ref(M::Bcc, A::Relative8, "experience_in_range");
    p.label("experience_full");
    emit(p, M::Lda, A::Immediate, O::Byte(57));
    p.label("experience_in_range");
    abs(p, M::Cmp, 0x651);
    p.emit_label_ref(M::Bne, A::Relative8, "experience_upload");
    implied(p, M::Rts);
    p.label("experience_upload");
    abs(p, M::Ldx, super::sfc_b1::QUEUE + 13);
    p.emit_label_ref(M::Beq, A::Relative8, "experience_free");
    implied(p, M::Rts);
    p.label("experience_free");
    abs(p, M::Sta, 0x651);
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    emit(p, M::And, A::Immediate, O::Word(255));
    for _ in 0..3 {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Word(jobs as u16));
    abs(p, M::Sta, super::sfc_b1::QUEUE + 10);
    emit(p, M::Lda, A::Immediate, O::Word(1));
    abs(p, M::Sta, super::sfc_b1::QUEUE + 13);
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    emit(p, M::Lda, A::Immediate, O::Byte((jobs >> 16) as u8));
    abs(p, M::Sta, super::sfc_b1::QUEUE + 12);
    implied(p, M::Rts);
    Ok(())
}
