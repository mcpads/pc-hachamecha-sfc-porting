use crate::{Result, sha};
use serde::Serialize;
use sfc_builder::sprite::{Oam, Object, Size};
use sfc_builder::sprite_frame::{FrameImage, FrameInstance, SpriteFrames};
use std::collections::{BTreeMap, BTreeSet};
const ACTOR_SCALE: (usize, usize) = (1, 2);
pub const OBJ_BYTES: usize = 0x3800;
const ART_LINES: usize = 144;
const ART_COLUMNS: std::ops::RangeInclusive<i32> = 7..=247;
#[derive(Serialize)]
struct Frame {
    name: String,
    resource: String,
    clip: u16,
    source_rectangle: [u16; 4],
    width: u16,
    height: u16,
    origin_x: i16,
    origin_y: i16,
    pixels: Vec<u8>,
}
#[derive(Clone, Serialize)]
pub struct Instance {
    name: String,
    resource: String,
    x: i16,
    y: i16,
    palette: u8,
    #[serde(skip)]
    source: (i32, i32, i32, i32, u16),
}
#[derive(Default)]
pub struct Collector {
    frames: BTreeMap<String, Frame>,
    ticks: Vec<(String, usize, Vec<Instance>)>,
}
impl Collector {
    #[allow(clippy::too_many_arguments)]
    pub fn actor(
        &mut self,
        resource: &str,
        clip: u16,
        rectangle: [u16; 4],
        source: &[u8],
        width: usize,
        base: u8,
        key: u8,
        draw: &[u32; 6],
    ) -> Result<Instance> {
        if base < 128 || !base.is_multiple_of(16) || key != base || draw[3] & !7 != 0 {
            return Err("Unsupported actor palette, transparency or draw flags".into());
        }
        let name = format!("{resource}-{clip}");
        let [l, t, r, b] = rectangle.map(usize::from);
        if !self.frames.contains_key(&name) {
            let (n, d) = ACTOR_SCALE;
            let w = ((r - l) * n).div_ceil(d);
            let h = ((b - t) * n).div_ceil(d);
            let padded_w = w.div_ceil(8) * 8;
            let padded_h = h.div_ceil(8) * 8;
            let mut pixels = vec![0; padded_w * padded_h];
            for y in 0..h {
                for x in 0..w {
                    let pixel = source[(t + y * d / n) * width + l + x * d / n].wrapping_sub(base);
                    if pixel > 15 {
                        return Err("Actor pixel exceeds OBJ palette".into());
                    }
                    pixels[y * padded_w + x] = pixel;
                }
            }
            let opaque: Vec<_> = pixels
                .iter()
                .enumerate()
                .filter(|(_, p)| **p != 0)
                .map(|(i, _)| (i % padded_w, i / padded_w))
                .collect();
            let left = opaque.iter().map(|(x, _)| *x).min().unwrap_or(0);
            let top = opaque.iter().map(|(_, y)| *y).min().unwrap_or(0);
            let right = opaque.iter().map(|(x, _)| *x + 1).max().unwrap_or(1);
            let bottom = opaque.iter().map(|(_, y)| *y + 1).max().unwrap_or(1);
            let cropped_w = (right - left).div_ceil(8) * 8;
            let cropped_h = (bottom - top).div_ceil(8) * 8;
            let mut cropped = vec![0; cropped_w * cropped_h];
            for y in top..bottom {
                for x in left..right {
                    cropped[(y - top) * cropped_w + x - left] = pixels[y * padded_w + x];
                }
            }
            self.frames.insert(
                name.clone(),
                Frame {
                    name: name.clone(),
                    resource: resource.into(),
                    clip,
                    source_rectangle: rectangle,
                    width: cropped_w as u16,
                    height: cropped_h as u16,
                    origin_x: -(left as i16),
                    origin_y: -(top as i16),
                    pixels: cropped,
                },
            );
        } else if self.frames[&name].source_rectangle != rectangle {
            return Err("Actor frame identity changed rectangle".into());
        }
        let source_x = draw[0] as i32 / 65536;
        let source_y = draw[1] as i32 / 65536;
        Ok(Instance {
            name,
            resource: resource.into(),
            x: 0,
            y: 0,
            palette: (base - 128) / 16,
            source: (source_x, source_y, (r - l) as i32, (b - t) as i32, clip),
        })
    }
    pub fn tick(&mut self, policy: &str, tick: usize, mut instances: Vec<Instance>) -> Result<()> {
        let mut feet: BTreeMap<String, (i32, i32)> = BTreeMap::new();
        for i in instances
            .iter()
            .filter(|i| i.source.4 == 0)
            .chain(&instances)
        {
            feet.entry(i.resource.clone())
                .or_insert((i.source.0 + i.source.2 / 2, i.source.1 + i.source.3));
        }
        let (n, d) = (ACTOR_SCALE.0 as i32, ACTOR_SCALE.1 as i32);
        for i in &mut instances {
            let (fx, fy) = feet[&i.resource];
            let (tx, ty) = (7 + (fx * 241).div_euclid(640), (fy * 144).div_euclid(382));
            i.x = i16::try_from(tx + ((i.source.0 - fx) * n).div_euclid(d))?;
            i.y = i16::try_from(ty + ((i.source.1 - fy) * n).div_euclid(d))?;
        }
        self.ticks.push((policy.into(), tick, instances));
        Ok(())
    }
    pub fn finish(self) -> Result<Vec<(String, Vec<u8>)>> {
        let atlas_of = |resources: &BTreeSet<String>| {
            let images: Vec<_> = self
                .frames
                .values()
                .filter(|f| resources.contains(&f.resource))
                .map(|f| FrameImage {
                    name: &f.name,
                    width: f.width,
                    height: f.height,
                    origin_x: f.origin_x,
                    origin_y: f.origin_y,
                    pixels: &f.pixels,
                })
                .collect();
            SpriteFrames::new(&images).map_err(|e| format!("Actor set {resources:?}: {e}"))
        };
        let mut sets: Vec<BTreeSet<String>> = Vec::new();
        let mut assigned: Vec<Option<usize>> = vec![None; self.ticks.len()];
        let mut policy_start = 0;
        while policy_start < self.ticks.len() {
            let policy = &self.ticks[policy_start].0;
            let policy_end = (policy_start..self.ticks.len())
                .find(|&k| &self.ticks[k].0 != policy)
                .unwrap_or(self.ticks.len());
            let mut current: Option<(BTreeSet<String>, Vec<usize>)> = None;
            let mut runs = Vec::new();
            let mut last: BTreeSet<String> = BTreeSet::new();
            for k in policy_start..policy_end {
                let wanted: BTreeSet<String> =
                    self.ticks[k].2.iter().map(|i| i.resource.clone()).collect();
                if wanted.is_empty() {
                    continue;
                }
                let cast_goes_on = !wanted.is_disjoint(&last);
                last = wanted.clone();
                if let Some((set, ticks)) = current.as_mut().filter(|_| cast_goes_on) {
                    if wanted.is_subset(set) {
                        ticks.push(k);
                        continue;
                    }
                    let union: BTreeSet<String> = set.union(&wanted).cloned().collect();
                    if atlas_of(&union).is_ok_and(|a| a.bytes().len() <= OBJ_BYTES) {
                        *set = union;
                        ticks.push(k);
                        continue;
                    }
                }
                runs.extend(current.take());
                current = Some((wanted, vec![k]));
            }
            runs.extend(current.take());
            for (set, ticks) in runs {
                let index = match sets.iter().position(|s| s == &set) {
                    Some(index) => index,
                    None => {
                        sets.push(set);
                        sets.len() - 1
                    }
                };
                for k in ticks {
                    assigned[k] = Some(index);
                }
            }
            policy_start = policy_end;
        }
        let mut groups = Vec::new();
        let mut files = Vec::new();
        let mut group_reports = Vec::new();
        for (index, resources) in sets.into_iter().enumerate() {
            let atlas = atlas_of(&resources)?;
            if atlas.bytes().len() > OBJ_BYTES {
                return Err(format!("Actor set {resources:?} exceeds the OBJ tiles").into());
            }
            files.push((format!("actors-{index}.4bpp"), atlas.bytes().to_vec()));
            group_reports.push(serde_json::json!(
                { "index" : index, "resources" : resources, "frames" : atlas
                .report() }
            ));
            groups.push(atlas);
        }
        let mut oams = Vec::new();
        let mut cases = Vec::new();
        for (k, (policy, tick, instances)) in self.ticks.iter().enumerate() {
            let group = assigned[k];
            let front: Vec<_> = instances
                .iter()
                .rev()
                .map(|i| FrameInstance {
                    name: &i.name,
                    x: i.x,
                    y: i.y,
                    palette: i.palette,
                    priority: 3,
                    flip_x: false,
                    flip_y: false,
                })
                .collect();
            let mut pixels = vec![0; 256 * 224];
            for i in instances {
                let frame = &self.frames[&i.name];
                for y in 0..i32::from(frame.height) {
                    for x in 0..i32::from(frame.width) {
                        let (dx, dy) = (
                            i32::from(i.x) + x - i32::from(frame.origin_x),
                            i32::from(i.y) + y - i32::from(frame.origin_y),
                        );
                        let pixel =
                            frame.pixels[y as usize * usize::from(frame.width) + x as usize];
                        if pixel != 0 && (0..256).contains(&dx) && (0..224).contains(&dy) {
                            pixels[dy as usize * 256 + dx as usize] = 128 + i.palette * 16 + pixel;
                        }
                    }
                }
            }
            let (oam, load) = match group {
                Some(group) => shown_objects(&groups[group], &front, &pixels)
                    .map_err(|e| format!("Actor {policy} tick {tick}: {e}"))?,
                None => (sfc_builder::sprite::Oam::new(&[])?, (0, 0)),
            };
            cases.push(serde_json::json!(
                { "policy" : policy, "tick" : tick, "group" : group, "instances"
                : instances, "oam_offset" : oams.len(), "oam_sha256" : sha(oam
                .bytes()), "pixels_sha256" : sha(& pixels), "maximum_objects" :
                load.0, "maximum_tiles" : load.1 }
            ));
            oams.extend_from_slice(oam.bytes());
        }
        let mut frames = Vec::new();
        for frame in self.frames.values() {
            let file = format!("actor-{}.indices", frame.name);
            frames.push(serde_json::json!(
                { "name" : frame.name, "resource" : frame.resource, "clip" :
                frame.clip, "source_rectangle" : frame.source_rectangle, "width"
                : frame.width, "height" : frame.height, "origin_x" : frame
                .origin_x, "origin_y" : frame.origin_y, "pixels_file" : file,
                "pixels_sha256" : sha(& frame.pixels) }
            ));
            files.push((file, frame.pixels.clone()));
        }
        let report = serde_json::json!(
            { "scope" :
            "Actor resource compilation and host OAM admission; not native animation",
            "frames" : frames, "groups" : group_reports, "cases" : cases, "limits" :
            ["Object-local nearest sampling differs from resampling a completed 640x480 frame.",
            "Line admission counts the artwork lines 0..144, where OBJ shows."] }
        );
        files.push(("actors.oam".into(), oams));
        files.push((
            "actors-build.json".into(),
            serde_json::to_vec_pretty(&report)?,
        ));
        Ok(files)
    }
}
fn piece_pixels(atlas: &[u8], object: &Object) -> Vec<(i32, i32, u8)> {
    let edge = if object.size == Size::Large { 16 } else { 8 };
    let t = usize::from(object.tile);
    let mut out = Vec::new();
    for py in 0..edge {
        for px in 0..edge {
            let tile = (t & 0x100) | (((t & 0xf0) + py / 8 * 16) & 0xf0) | ((t + px / 8) & 0x0f);
            let mut value = 0;
            for plane in 0..4 {
                let byte = atlas.get(tile * 32 + plane / 2 * 16 + py % 8 * 2 + plane % 2);
                value |= ((byte.copied().unwrap_or(0) >> (7 - px % 8)) & 1) << plane;
            }
            if value != 0 {
                out.push((
                    i32::from(object.x) + px as i32,
                    object.y_screen() + py as i32,
                    value,
                ));
            }
        }
    }
    out
}
trait ScreenY {
    fn y_screen(&self) -> i32;
}
impl ScreenY for Object {
    fn y_screen(&self) -> i32 {
        let y = i32::from(self.y);
        if y >= 224 { y - 256 } else { y }
    }
}
fn shown_objects(
    atlas: &SpriteFrames,
    front: &[FrameInstance<'_>],
    pixels: &[u8],
) -> Result<(Oam, (usize, usize))> {
    let shown = |x: i32, y: i32| (0..ART_LINES as i32).contains(&y) && ART_COLUMNS.contains(&x);
    let mut covered = vec![false; 256 * ART_LINES];
    let mut drawn = vec![0u8; 256 * ART_LINES];
    let mut kept = Vec::new();
    for instance in front {
        let one = atlas.compose(std::slice::from_ref(instance))?;
        let b = one.bytes();
        for slot in 0..128 {
            let high = (b[512 + slot / 4] >> ((slot % 4) * 2)) & 3;
            let object = Object {
                x: i16::from(b[slot * 4]) - if high & 1 != 0 { 256 } else { 0 },
                y: b[slot * 4 + 1],
                tile: u16::from(b[slot * 4 + 2]) | u16::from(b[slot * 4 + 3] & 1) << 8,
                size: if high & 2 != 0 {
                    Size::Large
                } else {
                    Size::Small
                },
                palette: (b[slot * 4 + 3] >> 1) & 7,
                priority: (b[slot * 4 + 3] >> 4) & 3,
                flip_x: b[slot * 4 + 3] & 0x40 != 0,
                flip_y: b[slot * 4 + 3] & 0x80 != 0,
            };
            if object.x == -16 && object.y == 240 && object.size == Size::Small {
                continue;
            }
            if object.flip_x || object.flip_y {
                return Err("Flipped actor piece".into());
            }
            let opaque: Vec<_> = piece_pixels(atlas.bytes(), &object)
                .into_iter()
                .filter(|&(x, y, _)| shown(x, y))
                .collect();
            if opaque
                .iter()
                .all(|&(x, y, _)| covered[y as usize * 256 + x as usize])
            {
                continue;
            }
            for &(x, y, value) in &opaque {
                let at = y as usize * 256 + x as usize;
                if !covered[at] {
                    covered[at] = true;
                    drawn[at] = 128 + object.palette * 16 + value;
                }
            }
            kept.push(object);
        }
    }
    for y in 0..ART_LINES {
        for x in ART_COLUMNS {
            let at = y * 256 + x as usize;
            if drawn[at] != pixels[at] {
                return Err(format!("kept pieces differ from the actors at ({x}, {y})").into());
            }
        }
    }
    let oam = Oam::new(&kept)?;
    let lines = &oam.scanlines()[..ART_LINES];
    if let Some(l) = lines.iter().find(|l| l.objects > 32 || l.tiles > 34) {
        return Err(format!(
            "sprite.scanline_limit: line {} objects {} tiles {}",
            l.line, l.objects, l.tiles
        )
        .into());
    }
    let load = (
        lines.iter().map(|l| l.objects).max().unwrap_or(0),
        lines.iter().map(|l| l.tiles).max().unwrap_or(0),
    );
    Ok((oam, load))
}
