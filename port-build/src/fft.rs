use std::f64::consts::PI;
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Complex {
    pub re: f64,
    pub im: f64,
}
impl Complex {
    fn new(re: f64, im: f64) -> Self {
        Complex { re, im }
    }
    fn mul(self, o: Complex) -> Complex {
        Complex::new(
            self.re * o.re - self.im * o.im,
            self.re * o.im + self.im * o.re,
        )
    }
    fn add(self, o: Complex) -> Complex {
        Complex::new(self.re + o.re, self.im + o.im)
    }
    fn sub(self, o: Complex) -> Complex {
        Complex::new(self.re - o.re, self.im - o.im)
    }
    fn conj(self) -> Complex {
        Complex::new(self.re, -self.im)
    }
}
fn radix2(data: &mut [Complex], inverse: bool) {
    let n = data.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            data.swap(i, j);
        }
    }
    let sign = if inverse { 1.0 } else { -1.0 };
    let mut length = 2;
    while length <= n {
        let half = length / 2;
        for k in 0..half {
            let angle = sign * 2.0 * PI * k as f64 / length as f64;
            let w = Complex::new(angle.cos(), angle.sin());
            for start in (0..n).step_by(length) {
                let (a, b) = (data[start + k], data[start + k + half].mul(w));
                data[start + k] = a.add(b);
                data[start + k + half] = a.sub(b);
            }
        }
        length <<= 1;
    }
}
pub fn dft(input: &[Complex], inverse: bool) -> Vec<Complex> {
    let n = input.len();
    if n == 0 {
        return Vec::new();
    }
    if n.is_power_of_two() {
        let mut data = input.to_vec();
        radix2(&mut data, inverse);
        return data;
    }
    let sign = if inverse { 1.0 } else { -1.0 };
    let chirp: Vec<Complex> = (0..n)
        .map(|k| {
            let q = ((k as u128 * k as u128) % (2 * n as u128)) as f64;
            let angle = sign * PI * q / n as f64;
            Complex::new(angle.cos(), angle.sin())
        })
        .collect();
    let m = (2 * n - 1).next_power_of_two();
    let mut a = vec![Complex::default(); m];
    for k in 0..n {
        a[k] = input[k].mul(chirp[k]);
    }
    let mut b = vec![Complex::default(); m];
    b[0] = chirp[0].conj();
    for k in 1..n {
        b[k] = chirp[k].conj();
        b[m - k] = chirp[k].conj();
    }
    radix2(&mut a, false);
    radix2(&mut b, false);
    for k in 0..m {
        a[k] = a[k].mul(b[k]);
    }
    radix2(&mut a, true);
    let scale = 1.0 / m as f64;
    (0..n)
        .map(|k| Complex::new(a[k].re * scale, a[k].im * scale).mul(chirp[k]))
        .collect()
}
pub fn rfft(input: &[f64]) -> Vec<Complex> {
    let data: Vec<Complex> = input.iter().map(|&x| Complex::new(x, 0.0)).collect();
    let mut out = dft(&data, false);
    out.truncate(input.len() / 2 + 1);
    out
}
pub fn irfft(spectrum: &[Complex], n: usize) -> Vec<f64> {
    let mut full = vec![Complex::default(); n];
    let half = n / 2 + 1;
    let kept = half.min(spectrum.len());
    full[..kept].copy_from_slice(&spectrum[..kept]);
    full[0].im = 0.0;
    if n.is_multiple_of(2) && n / 2 < spectrum.len() {
        full[n / 2].im = 0.0;
    }
    for k in 1..n.div_ceil(2) {
        full[n - k] = full[k].conj();
    }
    dft(&full, true).iter().map(|c| c.re / n as f64).collect()
}
