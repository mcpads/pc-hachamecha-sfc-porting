#![allow(dead_code)]
use crate::Result;
const MAXIMUM_OUTPUT: usize = 16 * 1024 * 1024;
pub fn decompress(data: &[u8]) -> Result<(Vec<u8>, usize)> {
    let mut output: Vec<u8> = Vec::new();
    let mut position = 0;
    let next = |count: usize, position: &mut usize| -> Result<std::ops::Range<usize>> {
        if *position + count > data.len() {
            return Err(format!("Truncated CNS stream at {:#x}", *position).into());
        }
        *position += count;
        Ok(*position - count..*position)
    };
    loop {
        let command = data[next(1, &mut position)?][0];
        if command == 0 {
            return Ok((output, position));
        }
        let family = command & 0xf0;
        let (literal, copy) = match family {
            0x40 | 0x50 => (usize::from(command & 0x1f), None),
            0x60 | 0x70 => {
                let low = data[next(1, &mut position)?][0];
                ((usize::from(command & 0x1f) << 8) | usize::from(low), None)
            }
            0x80.. => {
                let literal = usize::from((command & 0x70) >> 4);
                let range = next(literal, &mut position)?;
                if output.len() + literal > MAXIMUM_OUTPUT {
                    return Err("CNS output exceeds limit".into());
                }
                output.extend_from_slice(&data[range]);
                let distance = usize::from(data[next(1, &mut position)?][0]);
                (0, Some((usize::from(command & 0x0f) + 2, distance)))
            }
            _ => {
                let count = if family < 0x20 {
                    usize::from(command & 0x0f) + 2
                } else {
                    let low = data[next(1, &mut position)?][0];
                    (usize::from(command & 0x0f) << 8) | usize::from(low)
                };
                let wide = matches!(family, 0x10 | 0x30);
                let range = next(if wide { 2 } else { 1 }, &mut position)?;
                let distance = data[range]
                    .iter()
                    .rev()
                    .fold(0usize, |v, &b| (v << 8) | usize::from(b));
                (0, Some((count, distance)))
            }
        };
        if literal > 0 {
            let range = next(literal, &mut position)?;
            if output.len() + literal > MAXIMUM_OUTPUT {
                return Err("CNS output exceeds limit".into());
            }
            output.extend_from_slice(&data[range]);
        }
        if let Some((count, distance)) = copy {
            if !(1..=output.len()).contains(&distance) {
                return Err(
                    format!("Invalid CNS back reference at {position:#x}: {distance}").into(),
                );
            }
            if output.len() + count > MAXIMUM_OUTPUT {
                return Err("CNS output exceeds limit".into());
            }
            for _ in 0..count {
                output.push(output[output.len() - distance]);
            }
        }
    }
}
pub struct Sheet {
    pub width: usize,
    pub height: usize,
    pub colors: usize,
    pub pixels: Vec<u8>,
    pub rgb: Vec<u8>,
}
impl Sheet {
    pub fn decode(file: &[u8]) -> Result<Sheet> {
        let (decoded, _) = decompress(file)?;
        let word = |i: usize| -> Result<usize> {
            Ok(usize::from(u16::from_le_bytes(
                decoded
                    .get(i * 2..i * 2 + 2)
                    .ok_or("Truncated CNS header")?
                    .try_into()?,
            )))
        };
        let (flags, width, height, last) = (word(0)?, word(1)?, word(2)?, word(3)?);
        if flags != 0 || last > 255 {
            return Err("Unsupported indexed CNS".into());
        }
        let colors = last + 1;
        let stride = if last < 16 { width.div_ceil(2) } else { width };
        let start = 8 + 4 * colors;
        let packed = decoded
            .get(start..start + stride * height)
            .ok_or("Truncated CNS bitmap")?;
        let mut pixels = Vec::with_capacity(width * height);
        for y in (0..height).rev() {
            let row = &packed[y * stride..(y + 1) * stride];
            if last < 16 {
                pixels.extend(row.iter().flat_map(|b| [b >> 4, b & 15]).take(width));
            } else {
                pixels.extend_from_slice(row);
            }
        }
        let mut rgb = vec![0; 768];
        for i in 0..colors {
            let bgr = decoded
                .get(8 + i * 4..11 + i * 4)
                .ok_or("Truncated CNS palette")?;
            rgb[i * 3..i * 3 + 3].copy_from_slice(&[bgr[2], bgr[1], bgr[0]]);
        }
        Ok(Sheet {
            width,
            height,
            colors,
            pixels,
            rgb,
        })
    }
}
