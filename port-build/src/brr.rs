fn sample(n: i32, shift: u32, filter: u8, b1: i32, b2: i32) -> i32 {
    let mut s = if shift <= 12 {
        (n << shift) >> 1
    } else if n < 0 {
        -2048
    } else {
        0
    };
    let (p1, p2) = (b1, b2 >> 1);
    match filter {
        1 => s += (p1 >> 1) + ((-p1) >> 5),
        2 => s += p1 - p2 + (p2 >> 4) + ((p1 * -3) >> 6),
        3 => s += p1 - p2 + ((p1 * -13) >> 7) + ((p2 * 3) >> 4),
        _ => {}
    }
    let s = (s.clamp(-32768, 32767) << 1) & 0xffff;
    if s & 0x8000 != 0 { s - 0x10000 } else { s }
}
fn block(
    target: &[i32],
    shift: u32,
    filter: u8,
    mut b1: i32,
    mut b2: i32,
    limit: Option<i64>,
) -> Option<(i64, Vec<i32>, i32, i32)> {
    let (mut err, mut nibbles) = (0i64, Vec::with_capacity(16));
    for &x in target {
        let base = sample(0, 0, filter, b1, b2);
        let mut n = (f64::from(x - base) / f64::from(1u32 << shift))
            .round_ties_even()
            .clamp(-8.0, 7.0) as i32;
        let mut out = sample(n, shift, filter, b1, b2);
        for m in [n - 1, n + 1] {
            if (-8..=7).contains(&m) {
                let o = sample(m, shift, filter, b1, b2);
                if (o - x).abs() < (out - x).abs() {
                    n = m;
                    out = o;
                }
            }
        }
        err += i64::from(out - x).pow(2);
        if limit.is_some_and(|l| err >= l) {
            return None;
        }
        nibbles.push(n);
        b2 = b1;
        b1 = out;
    }
    Some((err, nibbles, b1, b2))
}
pub fn encode(samples: &[f64]) -> Vec<u8> {
    encode_with(samples, false)
}
pub fn encode_with(samples: &[f64], looped: bool) -> Vec<u8> {
    let mut x: Vec<i32> = samples
        .iter()
        .map(|v| v.round_ties_even().clamp(-32768.0, 32767.0) as i32)
        .collect();
    x.resize(x.len().div_ceil(16) * 16, 0);
    let blocks = x.len() / 16;
    let (mut out, mut b1, mut b2) = (Vec::with_capacity(blocks * 9), 0, 0);
    for b in 0..blocks {
        let target = &x[b * 16..(b + 1) * 16];
        let mut best: Option<(i64, u32, u8, Vec<i32>, i32, i32)> = None;
        for filter in if b == 0 { 0..1 } else { 0..4 } {
            for shift in 0..13 {
                if let Some(r) = block(target, shift, filter, b1, b2, best.as_ref().map(|b| b.0))
                    && best.as_ref().is_none_or(|b| r.0 < b.0)
                {
                    best = Some((r.0, shift, filter, r.1, r.2, r.3));
                }
            }
        }
        let (_, shift, filter, nibbles, n1, n2) = best.expect("a block always encodes");
        b1 = n1;
        b2 = n2;
        let last = b == blocks - 1;
        out.push((shift << 4) as u8 | filter << 2 | u8::from(last && looped) << 1 | u8::from(last));
        for pair in nibbles.chunks(2) {
            out.push(((pair[0] & 15) << 4 | (pair[1] & 15)) as u8);
        }
    }
    out
}
