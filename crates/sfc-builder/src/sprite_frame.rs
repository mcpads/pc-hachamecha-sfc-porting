use crate::{
    Result,
    sprite::{Oam, Object, Size, SpriteAtlas, SpriteImage},
};
use std::collections::BTreeMap;
pub struct FrameImage<'a> {
    pub name: &'a str,
    pub width: u16,
    pub height: u16,
    pub origin_x: i16,
    pub origin_y: i16,
    pub pixels: &'a [u8],
}
pub struct FrameInstance<'a> {
    pub name: &'a str,
    pub x: i16,
    pub y: i16,
    pub palette: u8,
    pub priority: u8,
    pub flip_x: bool,
    pub flip_y: bool,
}
struct Piece {
    x: u16,
    y: u16,
    edge: u16,
    size: Size,
    tile: u16,
}
struct Frame {
    width: u16,
    height: u16,
    origin_x: i16,
    origin_y: i16,
    source_sha256: String,
    pieces: Vec<Piece>,
}
pub struct SpriteFrames {
    atlas: SpriteAtlas,
    frames: BTreeMap<String, Frame>,
}
impl SpriteFrames {
    pub fn new(images: &[FrameImage<'_>]) -> Result<Self> {
        let mut ordered = BTreeMap::new();
        for image in images {
            if image.name.is_empty()
                || image.name.chars().any(char::is_control)
                || !(8..=256).contains(&image.width)
                || !(8..=256).contains(&image.height)
                || image.width % 8 != 0
                || image.height % 8 != 0
                || image.pixels.len() != usize::from(image.width) * usize::from(image.height)
            {
                return Err(format!("sprite.frame_domain: {}", image.name).into());
            }
            if let Some((offset, value)) = image.pixels.iter().enumerate().find(|(_, v)| **v > 15) {
                return Err(format!(
                    "sprite.frame_pixel: {} x {} y {} index {}",
                    image.name,
                    offset % usize::from(image.width),
                    offset / usize::from(image.width),
                    value
                )
                .into());
            }
            if ordered.insert(image.name, image).is_some() {
                return Err(format!("sprite.duplicate_frame: {}", image.name).into());
            }
        }
        if ordered.is_empty() {
            return Err("sprite.empty_frames".into());
        }
        let mut sources = vec![];
        let mut frames = BTreeMap::new();
        for (frame_index, (name, image)) in ordered.into_iter().enumerate() {
            let mut pieces = vec![];
            let columns = image.width / 8;
            let rows = image.height / 8;
            let mut used = vec![false; usize::from(columns * rows)];
            for cy in 0..rows {
                for cx in 0..columns {
                    if used[usize::from(cy * columns + cx)] {
                        continue;
                    }
                    let cells = if cx + 1 < columns
                        && cy + 1 < rows
                        && !used[usize::from(cy * columns + cx + 1)]
                        && !used[usize::from((cy + 1) * columns + cx)]
                        && !used[usize::from((cy + 1) * columns + cx + 1)]
                    {
                        2
                    } else {
                        1
                    };
                    let edge = cells * 8;
                    let (x, y) = (cx * 8, cy * 8);
                    for dy in 0..cells {
                        for dx in 0..cells {
                            used[usize::from((cy + dy) * columns + cx + dx)] = true;
                        }
                    }
                    let pixels: Vec<_> = (0..edge)
                        .flat_map(|dy| {
                            (0..edge).map(move |dx| {
                                image.pixels[usize::from(y + dy) * usize::from(image.width)
                                    + usize::from(x + dx)]
                            })
                        })
                        .collect();
                    let size = if cells == 2 { Size::Large } else { Size::Small };
                    let key = format!("{frame_index:05}-{:05}", pieces.len());
                    sources.push((key, size, pixels));
                    pieces.push(Piece {
                        x,
                        y,
                        edge,
                        size,
                        tile: 0,
                    });
                }
            }
            frames.insert(
                name.to_owned(),
                Frame {
                    width: image.width,
                    height: image.height,
                    origin_x: image.origin_x,
                    origin_y: image.origin_y,
                    source_sha256: crate::sha256(image.pixels),
                    pieces,
                },
            );
        }
        let inputs: Vec<_> = sources
            .iter()
            .map(|(name, size, pixels)| SpriteImage {
                name,
                size: *size,
                pixels,
            })
            .collect();
        let atlas = SpriteAtlas::new(&inputs)?;
        for (frame_index, frame) in frames.values_mut().enumerate() {
            for (piece_index, piece) in frame.pieces.iter_mut().enumerate() {
                piece.tile = atlas
                    .entry(&format!("{frame_index:05}-{piece_index:05}"))?
                    .1;
            }
        }
        Ok(Self { atlas, frames })
    }
    pub fn bytes(&self) -> &[u8] {
        self.atlas.bytes()
    }
    pub fn compose(&self, instances: &[FrameInstance<'_>]) -> Result<Oam> {
        let mut objects = vec![];
        for instance in instances {
            let frame = self
                .frames
                .get(instance.name)
                .ok_or_else(|| format!("sprite.missing_frame: {}", instance.name))?;
            if instance.palette > 7 || instance.priority > 3 {
                return Err(format!("sprite.frame_attributes: {}", instance.name).into());
            }
            for piece in &frame.pieces {
                let position = |anchor: i16, origin: i16, offset: u16, flipped: bool| {
                    i32::from(anchor)
                        + if flipped {
                            i32::from(origin) - i32::from(offset) - i32::from(piece.edge)
                        } else {
                            i32::from(offset) - i32::from(origin)
                        }
                };
                let x = position(instance.x, frame.origin_x, piece.x, instance.flip_x);
                let y = position(instance.y, frame.origin_y, piece.y, instance.flip_y);
                if x >= 256
                    || y >= 224
                    || x + i32::from(piece.edge) <= 0
                    || y + i32::from(piece.edge) <= 0
                {
                    continue;
                }
                if objects.len() == 128 {
                    return Err("sprite.oam_capacity".into());
                }
                objects.push(Object {
                    x: x as i16,
                    y: y as u8,
                    tile: piece.tile,
                    size: piece.size,
                    palette: instance.palette,
                    priority: instance.priority,
                    flip_x: instance.flip_x,
                    flip_y: instance.flip_y,
                });
            }
        }
        Oam::with_scanline_limits(&objects)
    }
    pub fn report(&self) -> serde_json::Value {
        serde_json::json!(
            { "atlas" : self.atlas.report(), "frames" : self.frames.iter().map(| (name,
            f) | serde_json::json!({ "name" : name, "width" : f.width, "height" : f
            .height, "origin_x" : f.origin_x, "origin_y" : f.origin_y,
            "source_pixels_sha256" : f.source_sha256, "pieces" : f.pieces.iter().map(| p
            | serde_json::json!({ "x" : p.x, "y" : p.y, "edge" : p.edge, "tile" : p.tile
            })).collect::< Vec < _ >> () })).collect::< Vec < _ >> () }
        )
    }
}
