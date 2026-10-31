type Rgb = [u8; 3];
pub(super) fn halve(pixels: &[Option<Rgb>], width: usize, height: usize) -> Vec<Option<Rgb>> {
    let (w, h) = (width / 2, height / 2);
    (0..w * h)
        .map(|i| {
            let (x, y) = (i % w * 2, i / w * 2);
            [(0, 0), (1, 0), (0, 1), (1, 1)]
                .iter()
                .filter_map(|(dx, dy)| pixels[(y + dy) * width + x + dx])
                .min_by_key(|c| c.iter().map(|&v| u16::from(v)).sum::<u16>())
        })
        .collect()
}

pub(super) fn halve_cell(pixels: &[Option<Rgb>], width: usize, at: [usize; 2]) -> Vec<Option<Rgb>> {
    (0..64)
        .map(|i| {
            let (x, y) = (i % 8 * 2, i / 8 * 2);
            [(0, 0), (1, 0), (0, 1), (1, 1)]
                .into_iter()
                .filter(|&(dx, dy)| {
                    (x != 0 || dx == 0)
                        && (x != 14 || dx == 1)
                        && (y != 0 || dy == 0)
                        && (y != 14 || dy == 1)
                })
                .filter_map(|(dx, dy)| pixels[(at[1] + y + dy) * width + at[0] + x + dx])
                .min_by_key(|c| c.iter().map(|&v| u16::from(v)).sum::<u16>())
        })
        .collect()
}
