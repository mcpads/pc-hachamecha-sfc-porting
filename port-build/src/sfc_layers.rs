use crate::Result;
use sfc_builder::{
    tiles::{Attributes, Depth, IndexedImage, TileImage},
    transfer::Destination,
};
pub struct Artwork {
    pub arle_face: crate::sfc_arle_face::Parent,
    pub arle_parent: crate::sfc_arle_parent::Parent,
    pub motion: crate::sfc_motion::Motion,
    pub expression: crate::sfc_expression::Expression,
    pub actors: crate::sfc_actor_service::Actors,
    pub uploads: Vec<(String, Vec<u8>, Destination)>,
    pub palette: Vec<u8>,
    pub rgb: Vec<u8>,
}
pub fn prepare(
    files: &[(String, Vec<u8>)],
    commands: &[crate::vm::Command],
    motion_table: &[u16],
) -> Result<Artwork> {
    let file = |name: &str| -> Result<&[u8]> {
        files
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, b)| b.as_slice())
            .ok_or_else(|| format!("Missing scene asset {name}").into())
    };
    let mut pixels = file("scene-backdrop.indices")?.to_vec();
    if pixels.len() != 256 * 224 || pixels.iter().any(|p| (1..7).contains(p)) {
        return Err(format!(
            "Backdrop occupies actor or text colors: {:?}",
            pixels
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>()
        )
        .into());
    }
    let attributes = vec![
        Attributes {
            palette: 0,
            priority: false
        };
        32 * 28
    ];
    let bg = TileImage::new(
        IndexedImage {
            width: 256,
            height: 224,
            pixels: &pixels,
            attributes: &attributes,
        },
        Depth::Eight,
        0,
    )?;
    if bg.native_tiles().len() > 0x9800 {
        return Err(format!(
            "Backdrop needs {} bytes; the text map begins at 0x9800",
            bg.native_tiles().len()
        )
        .into());
    }
    let report: serde_json::Value = serde_json::from_slice(file("actors-build.json")?)?;
    let case = report["cases"]
        .as_array()
        .ok_or("Missing actor cases")?
        .iter()
        .find(|c| c["policy"] == "paced" && c["tick"] == 502)
        .ok_or("Missing scene actor state")?;
    let group = case["group"].as_u64().ok_or("Missing scene group")?;
    let actors = file(&format!("actors-{group}.4bpp"))?;
    if actors.len() > crate::sfc_sprites::OBJ_BYTES {
        return Err("Actor atlas crosses the BG1 map".into());
    }
    for instance in case["instances"].as_array().ok_or("Missing instances")? {
        let frame = report["frames"]
            .as_array()
            .ok_or("Missing frames")?
            .iter()
            .find(|f| f["name"] == instance["name"])
            .ok_or("Missing actor frame")?;
        let data = file(frame["pixels_file"].as_str().ok_or("Missing pixels")?)?;
        let width = frame["width"].as_u64().ok_or("Missing width")? as usize;
        let x = instance["x"].as_i64().ok_or("Missing x")?
            - frame["origin_x"].as_i64().ok_or("Missing origin")?;
        let y = instance["y"].as_i64().ok_or("Missing y")?
            - frame["origin_y"].as_i64().ok_or("Missing origin")?;
        let base = 128 + instance["palette"].as_u64().ok_or("Missing palette")? as u8 * 16;
        for (i, p) in data.iter().enumerate() {
            let (dx, dy) = (x + (i % width) as i64, y + (i / width) as i64);
            if *p != 0 && (7..248).contains(&dx) && (0..144).contains(&dy) {
                pixels[dy as usize * 256 + dx as usize] = base + p;
            }
        }
    }
    let mut positions = Vec::new();
    let mut expression_tiles = Vec::new();
    let mut pieces = Vec::new();
    let mut objects = Vec::new();
    let group_frames = report["groups"][group as usize]["frames"]["frames"]
        .as_array()
        .ok_or("Missing resident frames")?;
    for (owner, instance) in case["instances"]
        .as_array()
        .ok_or("Missing actor instances")?
        .iter()
        .rev()
        .enumerate()
    {
        let pos = [
            instance["x"].as_i64().ok_or("Missing actor x")? as i16,
            instance["y"].as_i64().ok_or("Missing actor y")? as i16,
        ];
        positions.push(pos);
        let frame = group_frames
            .iter()
            .find(|f| f["name"] == instance["name"])
            .ok_or("Missing actor pieces")?;
        for piece in frame["pieces"].as_array().ok_or("Missing pieces")? {
            let dx = (piece["x"].as_i64().ok_or("Missing piece x")?
                - frame["origin_x"].as_i64().ok_or("Missing origin")?) as i16;
            let dy = (piece["y"].as_i64().ok_or("Missing piece y")?
                - frame["origin_y"].as_i64().ok_or("Missing origin")?) as i16;
            let edge = piece["edge"].as_u64().ok_or("Missing piece size")? as u16;
            let slot = pieces.len();
            if slot >= 128 || ![8, 16].contains(&edge) {
                return Err("Actor service pieces exceed the OAM".into());
            }
            let y = pos[1] + dy;
            objects.push(sfc_builder::sprite::Object {
                x: pos[0] + dx,
                y: if (-16..224).contains(&y) {
                    y as u8
                } else {
                    224
                },
                tile: u16::try_from(piece["tile"].as_u64().ok_or("Missing piece tile")?)?,
                size: if edge == 16 {
                    sfc_builder::sprite::Size::Large
                } else {
                    sfc_builder::sprite::Size::Small
                },
                palette: instance["palette"].as_u64().ok_or("Missing palette")? as u8,
                priority: 3,
                flip_x: false,
                flip_y: false,
            });
            if instance["name"] == "DEM_ARLE.CNS-2" {
                let alternate = group_frames
                    .iter()
                    .find(|f| f["name"] == "DEM_ARLE.CNS-1")
                    .ok_or("Missing expression alternate")?;
                for field in ["width", "height", "origin_x", "origin_y"] {
                    if alternate[field] != frame[field] {
                        return Err("Expression geometry changed".into());
                    }
                }
                let index = pieces
                    .iter()
                    .filter(|p: &&crate::sfc_actor_service::Piece| p.owner == owner)
                    .count();
                let other = &alternate["pieces"][index];
                for field in ["x", "y", "edge"] {
                    if other[field] != piece[field] {
                        return Err("Expression piece geometry changed".into());
                    }
                }
                expression_tiles.push((
                    slot,
                    other["tile"].as_u64().ok_or("Missing alternate tile")? as u16,
                    piece["tile"].as_u64().ok_or("Missing original tile")? as u16,
                ));
            }
            pieces.push(crate::sfc_actor_service::Piece {
                owner,
                dx,
                dy,
                edge,
            });
        }
    }
    let oam = sfc_builder::sprite::Oam::new(&objects)?.bytes().to_vec();
    let rgba = file("scene-palette.rgba")?;
    if rgba.len() != 1024 {
        return Err("Invalid scene palette".into());
    }
    let mut palette: Vec<u8> = rgba
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|c| {
            (u16::from(c[0] >> 3) | u16::from(c[1] >> 3) << 5 | u16::from(c[2] >> 3) << 10)
                .to_le_bytes()
        })
        .collect();
    palette[..2].fill(0);
    let rgb = pixels
        .iter()
        .flat_map(|p| {
            let word = u16::from_le_bytes([palette[*p as usize * 2], palette[*p as usize * 2 + 1]]);
            [0, 5, 10].map(|s| {
                let v = ((word >> s) & 31) as u8;
                (v << 3) | (v >> 2)
            })
        })
        .collect();
    Ok(Artwork {
        arle_parent: crate::sfc_arle_parent::Parent::compile(commands)?,
        arle_face: crate::sfc_arle_face::Parent::compile(commands)?,
        motion: crate::sfc_motion::Motion::compile(commands, motion_table, positions.len())?,
        expression: crate::sfc_expression::Expression::compile(commands)?,
        actors: crate::sfc_actor_service::Actors {
            positions,
            pieces,
            initial_oam: oam.to_vec(),
            expression_tiles,
        },
        palette,
        rgb,
        uploads: vec![
            (
                "art_tiles".into(),
                bg.native_tiles().to_vec(),
                Destination::Vram(0),
            ),
            (
                "art_map".into(),
                bg.bg_map(32, 32, 0)?,
                Destination::Vram(0xf800),
            ),
            (
                "actor_tiles".into(),
                actors.to_vec(),
                Destination::Vram(0xc000),
            ),
            ("actor_oam".into(), oam.to_vec(), Destination::Oam(0)),
        ],
    })
}
