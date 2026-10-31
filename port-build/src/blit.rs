use crate::Result;
use serde::Deserialize;
#[derive(Debug, Deserialize, PartialEq)]
pub struct Copy {
    pub destination: [i32; 4],
    pub source: [i32; 4],
    flags: u32,
}
#[derive(Deserialize)]
#[cfg_attr(not(test), allow(dead_code))]
pub struct Case {
    xy_fixed: [i32; 2],
    rectangle: [i32; 4],
    window: [i32; 4],
    object_flags: u32,
    calls: Vec<Copy>,
    pixels_sha256: String,
}
pub fn plan(
    xy_fixed: [i32; 2],
    mut source: [i32; 4],
    window: [i32; 4],
    object_flags: u32,
    flags: u32,
    key_mask: u32,
) -> Result<Option<Copy>> {
    if object_flags & 0x208 != 0 {
        return Err("Scaled blit is not implemented".into());
    }
    if source[0] >= source[2]
        || source[1] >= source[3]
        || window[0] >= window[2]
        || window[1] >= window[3]
    {
        return Err("Invalid blit rectangle".into());
    }
    let x = xy_fixed[0] / 65536;
    let y = xy_fixed[1] / 65536;
    let mut destination = [
        x,
        y,
        x.wrapping_add(source[2].wrapping_sub(source[0])),
        y.wrapping_add(source[3].wrapping_sub(source[1])),
    ];
    for axis in 0..2 {
        if destination[axis] < window[axis] {
            if destination[axis + 2] <= window[axis] {
                return Ok(None);
            }
            source[axis] = source[axis].wrapping_add(window[axis].wrapping_sub(destination[axis]));
            destination[axis] = window[axis];
        }
        if window[axis + 2] <= destination[axis + 2] {
            if window[axis + 2] <= destination[axis] {
                return Ok(None);
            }
            source[axis + 2] =
                source[axis + 2].wrapping_add(window[axis + 2].wrapping_sub(destination[axis + 2]));
            destination[axis + 2] = window[axis + 2];
        }
    }
    Ok(Some(Copy {
        destination,
        source,
        flags: if object_flags & 0x80 != 0 {
            flags & !key_mask
        } else {
            flags
        },
    }))
}
pub struct Surface<'a> {
    pub pixels: &'a [u8],
    pub width: usize,
    pub height: usize,
}
pub struct Canvas<'a> {
    pub pixels: &'a mut [u8],
    pub width: usize,
    pub height: usize,
}
pub fn paint(copy: &Copy, source: Surface<'_>, target: Canvas<'_>, key: Option<u8>) -> Result<()> {
    let s = copy.source;
    let d = copy.destination;
    let inside = |r: [i32; 4], w: usize, h: usize| {
        r[0] >= 0
            && r[1] >= 0
            && r[0] < r[2]
            && r[1] < r[3]
            && r[2] as usize <= w
            && r[3] as usize <= h
    };
    if !inside(s, source.width, source.height)
        || !inside(d, target.width, target.height)
        || s[2] - s[0] != d[2] - d[0]
        || s[3] - s[1] != d[3] - d[1]
        || source.width.checked_mul(source.height) != Some(source.pixels.len())
        || target.width.checked_mul(target.height) != Some(target.pixels.len())
    {
        return Err("Invalid indexed copy extent".into());
    }
    for y in 0..(s[3] - s[1]) as usize {
        for x in 0..(s[2] - s[0]) as usize {
            let pixel = source.pixels[(s[1] as usize + y) * source.width + s[0] as usize + x];
            if Some(pixel) != key {
                target.pixels[(d[1] as usize + y) * target.width + d[0] as usize + x] = pixel;
            }
        }
    }
    Ok(())
}
