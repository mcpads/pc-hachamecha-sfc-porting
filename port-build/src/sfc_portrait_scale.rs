pub const SOURCE: (usize, usize) = (224, 240);
pub type Reduce<'a> = &'a dyn Fn(&[u8], &[bool], usize) -> Vec<u8>;
const LUMA: [f64; 3] = [0.299, 0.587, 0.114];
const NEAR: f64 = 50.0;
const BAYER: [[f64; 2]; 2] = [[0.125, 0.625], [0.875, 0.375]];
fn luma(c: &[f64; 3]) -> f64 {
    c[0] * LUMA[0] + c[1] * LUMA[1] + c[2] * LUMA[2]
}
fn spans(n_out: usize, n_in: usize) -> Vec<Vec<(usize, f64)>> {
    (0..n_out)
        .map(|o| {
            let a = o as f64 * n_in as f64 / n_out as f64;
            let b = (o + 1) as f64 * n_in as f64 / n_out as f64;
            (a as usize..b.ceil() as usize)
                .filter_map(|i| {
                    let w = b.min(i as f64 + 1.0) - a.max(i as f64);
                    (w > 1e-9).then_some((i, w))
                })
                .collect()
        })
        .collect()
}
fn hsv(c: &[f64; 3]) -> (f64, f64, f64) {
    let [r, g, b] = c.map(|v| v / 255.0);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    if max == min {
        return (0.0, 0.0, max);
    }
    let s = (max - min) / max;
    let (rc, gc, bc) = (
        (max - r) / (max - min),
        (max - g) / (max - min),
        (max - b) / (max - min),
    );
    let h = if r == max {
        bc - gc
    } else if g == max {
        2.0 + rc - bc
    } else {
        4.0 + gc - rc
    };
    ((h / 6.0).rem_euclid(1.0) * 360.0, s, max)
}
fn family(p: &[f64; 3], q: &[f64; 3]) -> bool {
    let (hp, hq) = (hsv(p), hsv(q));
    let white = |h: (f64, f64, f64)| h.1 < 0.1 && h.2 > 0.9;
    let tint = |h: (f64, f64, f64)| h.1 < 0.5 && h.2 > 0.9;
    if white(hp) || white(hq) {
        return (white(hp) || tint(hp)) && (white(hq) || tint(hq));
    }
    if hp.1 < 0.15 || hq.1 < 0.15 {
        return false;
    }
    let d = (hp.0 - hq.0).abs();
    d.min(360.0 - d) <= 30.0
}
pub fn reduce(
    source: &[u8],
    carbuncle: &[bool],
    palette: &[[f64; 3]],
    (width, height): (usize, usize),
) -> Vec<u8> {
    let (xs, ys) = (spans(width, SOURCE.0), spans(height, SOURCE.1));
    let lum: Vec<f64> = palette.iter().map(luma).collect();
    let n = palette.len();
    let mut families: Vec<Option<bool>> = vec![None; n * n];
    let mut out = Vec::with_capacity(width * height);
    for (y, wy) in ys.iter().enumerate() {
        for (x, wx) in xs.iter().enumerate() {
            let mut mean = [0.0; 3];
            let mut total = 0.0;
            let mut carb = 0.0;
            let mut shares = [(0u8, 0.0f64); 16];
            let mut count = 0;
            for &(sy, a) in wy {
                for &(sx, b) in wx {
                    let at = sy * SOURCE.0 + sx;
                    let c = source[at];
                    let rgb = &palette[usize::from(c)];
                    for k in 0..3 {
                        mean[k] += rgb[k] * a * b;
                    }
                    total += a * b;
                    if carbuncle[at] {
                        carb += a * b;
                    }
                    match shares[..count].iter_mut().find(|(k, _)| *k == c) {
                        Some(s) => s.1 += a * b,
                        None => {
                            shares[count] = (c, a * b);
                            count += 1;
                        }
                    }
                }
            }
            let shares = &shares[..count];
            let mean = mean.map(|v| v / total);
            let target = luma(&mean);
            let cost = |c: u8| {
                let rgb = &palette[usize::from(c)];
                let distance: f64 = (0..3).map(|k| (rgb[k] - mean[k]).powi(2)).sum();
                (lum[usize::from(c)] - target).abs() + 0.25 * distance.sqrt()
            };
            let mut nearest: Option<(u8, f64)> = None;
            for &(c, _) in shares {
                let v = cost(c);
                if nearest.is_none_or(|(b, bv)| v < bv || (v == bv && c < b)) {
                    nearest = Some((c, v));
                }
            }
            let nearest = nearest.expect("a block has a colour").0;
            if carb / total >= 0.5 {
                out.push(nearest);
                continue;
            }
            let key = |c: u8| (lum[usize::from(c)], c);
            let below = shares
                .iter()
                .map(|s| s.0)
                .filter(|&c| lum[usize::from(c)] <= target)
                .max_by(|&p, &q| key(p).partial_cmp(&key(q)).unwrap());
            let above = shares
                .iter()
                .map(|s| s.0)
                .filter(|&c| lum[usize::from(c)] >= target)
                .min_by(|&p, &q| key(p).partial_cmp(&key(q)).unwrap());
            if let (Some(a), Some(b)) = (below, above) {
                let (la, lb) = (lum[usize::from(a)], lum[usize::from(b)]);
                let pair = usize::from(a) * n + usize::from(b);
                if a != b
                    && lb > la
                    && lb - la < NEAR
                    && *families[pair].get_or_insert_with(|| {
                        family(&palette[usize::from(a)], &palette[usize::from(b)])
                    })
                {
                    let f = (target - la) / (lb - la);
                    out.push(if f > BAYER[y % 2][x % 2] { b } else { a });
                    continue;
                }
            }
            let top = shares
                .iter()
                .fold(None, |best: Option<(u8, f64)>, &(c, w)| match best {
                    Some((_, bw)) if bw >= w => best,
                    _ => Some((c, w)),
                })
                .expect("a block has a colour");
            out.push(if top.1 / total >= 0.5 { top.0 } else { nearest });
        }
    }
    out
}
pub fn palette(rgb: &[u8]) -> Vec<[f64; 3]> {
    rgb.as_chunks::<3>()
        .0
        .iter()
        .map(|c| c.map(|v| f64::from(v >> 3 << 3)))
        .collect()
}
