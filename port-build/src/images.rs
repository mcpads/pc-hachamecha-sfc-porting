use crate::{Resource, Result, sha};
use asset_types::CanonicalFourBitCharacters;
use serde::{Deserialize, Serialize};
use sfc_codecs::asset_codec::encode_sfc_planar_4bpp;
use std::collections::BTreeSet;
#[derive(Deserialize)]
pub struct Image {
    resource: String,
    width: usize,
    height: usize,
    pixels_file: String,
    palette_file: String,
    pixels_sha256: String,
    palette_sha256: String,
}
#[derive(Debug, Serialize)]
pub struct ImageReport {
    pub resource: String,
    pub width: usize,
    pub height: usize,
    pub source_indices_sha256: String,
    pub source_rgb_sha256: String,
    pub status: &'static str,
    pub tiles_file: Option<String>,
    pub tiles_sha256: Option<String>,
    pub tile_count: usize,
}
#[derive(Debug)]
pub struct Compiled {
    pub files: Vec<(String, Vec<u8>)>,
    pub reports: Vec<ImageReport>,
}
pub fn compile(
    input: &crate::content::Content,
    images: &[Image],
    resources: &[Resource],
) -> Result<Compiled> {
    let expected: BTreeSet<_> = resources.iter().map(|r| &r.file).collect();
    let mut seen = BTreeSet::new();
    let mut output = Compiled {
        files: vec![],
        reports: vec![],
    };
    for image in images {
        if !expected.contains(&image.resource)
            || !seen.insert(&image.resource)
            || image.pixels_file != format!("{}.indices", image.resource)
            || image.palette_file != format!("{}.rgb", image.resource)
            || image.width == 0
            || image.height == 0
            || image.width > 4096
            || image.height > 4096
        {
            return Err("Invalid analyzed image extent, binding or filename".into());
        }
        let pixels = input.read(&image.pixels_file)?;
        let palette = input.read(&image.palette_file)?;
        if pixels.len() != image.width * image.height
            || palette.len() != 768
            || sha(&pixels) != image.pixels_sha256
            || sha(&palette) != image.palette_sha256
        {
            return Err(format!("Analyzed pixels or palette changed: {}", image.resource).into());
        }
        output
            .files
            .push((image.pixels_file.clone(), pixels.clone()));
        output.files.push((image.palette_file.clone(), palette));
        let mut report = ImageReport {
            resource: image.resource.clone(),
            width: image.width,
            height: image.height,
            source_indices_sha256: image.pixels_sha256.clone(),
            source_rgb_sha256: image.palette_sha256.clone(),
            status: "requires_palette_adaptation",
            tiles_file: None,
            tiles_sha256: None,
            tile_count: 0,
        };
        if pixels.iter().all(|&p| p < 16) {
            let tiles = split_tiles(&pixels, image.width, image.height)?;
            report.tile_count = tiles.len();
            let bytes = encode_sfc_planar_4bpp(&CanonicalFourBitCharacters::new(tiles)?);
            let name = format!("{}.4bpp", image.resource);
            report.status = "source_size_4bpp_tiles";
            report.tiles_sha256 = Some(sha(&bytes));
            report.tiles_file = Some(name.clone());
            output.files.push((name, bytes));
        }
        output.reports.push(report);
    }
    if seen != expected {
        return Err("Missing analyzed source image".into());
    }
    Ok(output)
}
fn split_tiles(pixels: &[u8], width: usize, height: usize) -> Result<Vec<Vec<u8>>> {
    if !width.is_multiple_of(8) || !height.is_multiple_of(8) {
        return Err("Source sheet needs an explicit padding or cropping decision".into());
    }
    let mut tiles = Vec::new();
    for y in (0..height).step_by(8) {
        for x in (0..width).step_by(8) {
            let mut tile = Vec::with_capacity(64);
            for row in y..y + 8 {
                tile.extend_from_slice(&pixels[row * width + x..row * width + x + 8]);
            }
            tiles.push(tile);
        }
    }
    Ok(tiles)
}
