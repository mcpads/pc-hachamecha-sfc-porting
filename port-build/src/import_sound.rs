use crate::Result;
use serde_json::Value;
pub fn round_digits(x: f64, digits: usize) -> f64 {
    let exact = format!("{:.60}", x.abs());
    let point = exact.find('.').unwrap();
    let (head, tail) = exact.split_at(point + 1 + digits);
    let mut kept: Vec<u8> = head.bytes().filter(|b| *b != b'.').collect();
    let rest = tail.as_bytes();
    let up = match rest.first() {
        Some(b'5') if rest[1..].iter().all(|&b| b == b'0') => {
            (kept.last().unwrap() - b'0') % 2 == 1
        }
        Some(&d) => d >= b'5',
        None => false,
    };
    if up {
        let mut i = kept.len();
        loop {
            if i == 0 {
                kept.insert(0, b'1');
                break;
            }
            i -= 1;
            if kept[i] == b'9' {
                kept[i] = b'0';
            } else {
                kept[i] += 1;
                break;
            }
        }
    }
    let text = String::from_utf8(kept).unwrap();
    let (int, frac) = text.split_at(text.len() - digits);
    let value: f64 = format!("{int}.{frac}").parse().unwrap();
    if x < 0.0 { -value } else { value }
}
pub(crate) fn loudness_level(a: &[f64], rate: f64, effect: &Value) -> f64 {
    let Some(loudness) = effect["loudness"].as_f64() else {
        return 1.0;
    };
    let w = (rate * 0.1) as usize;
    let n = (a.len() / w).max(1);
    let mut rms: Vec<f64> = (0..n)
        .map(|i| {
            let window = &a[(i * w).min(a.len())..((i + 1) * w).min(a.len())];
            let squares = window.iter().fold(0.0, |s, v| s + v * v);
            (squares / window.len() as f64).sqrt()
        })
        .collect();
    rms.sort_by(|x, y| y.total_cmp(x));
    rms.truncate(3);
    let mean = rms.iter().fold(0.0, |s, r| s + r) / rms.len() as f64;
    (loudness / mean).min(effect["max_level"].as_f64().unwrap_or(f64::INFINITY))
}
pub(crate) fn pcm(raw: &[u8], index: usize) -> Result<(Vec<f64>, f64)> {
    let at = 2 + 14 * index;
    let flags = u16::from_le_bytes(raw[at..at + 2].try_into()?);
    let word = |k: usize| {
        u32::from_le_bytes(raw[at + 2 + 4 * k..at + 6 + 4 * k].try_into().unwrap()) as usize
    };
    let (start, size, rate) = (word(0), word(1), word(2));
    let data = raw
        .get(start..start + size)
        .ok_or("WLK entry past the file")?;
    let mut a: Vec<f64> = if flags & 0x8000 != 0 {
        data.as_chunks::<2>()
            .0
            .iter()
            .map(|c| f64::from(i16::from_le_bytes(*c)))
            .collect()
    } else {
        data.iter()
            .map(|&b| (f64::from(b) - 128.0) * 256.0)
            .collect()
    };
    if flags & 0x4000 != 0 {
        a = a
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| (c[0] + c[1]) / 2.0)
            .collect();
    }
    Ok((a, rate as f64))
}
pub(crate) fn resample(a: &[f64], rate: f64, target: f64) -> Vec<f64> {
    let n = (a.len() as f64 * target / rate).round_ties_even() as usize;
    let spectrum = crate::fft::rfft(a);
    let keep = n / 2 + 1;
    let out = crate::fft::irfft(&spectrum[..keep.min(spectrum.len())], n);
    out.iter().map(|v| v * n as f64 / a.len() as f64).collect()
}
