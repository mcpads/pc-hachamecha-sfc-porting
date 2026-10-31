use crate::Result;
use serde_json::{Value, json};
use std::{f64::consts::PI, fs, path::Path};

pub const NAMES: [&str; 21] = [
    "piano",
    "bell",
    "organ",
    "pluck",
    "bass",
    "slap",
    "strings",
    "brass",
    "reed",
    "flute",
    "saw",
    "square",
    "pad",
    "crystal",
    "kick",
    "snare",
    "hat",
    "open_hat",
    "crash",
    "tom",
    "tambourine",
];
const TONES: [(&[f64], f64, &str); 14] = [
    (&[1., 0.6, 0.3, 0.15, 0.1], 0.8, "adsr 15 5 3 12"),
    (
        &[1., 0.08, 0.65, 0.05, 0.3, 0., 0.12],
        0.7,
        "adsr 15 4 2 13",
    ),
    (&[1., 0.6, 0.4, 0.15, 0.2, 0.08], 0.05, "adsr 15 0 7 0"),
    (&[1., 0.6, 0.32, 0.2, 0.15, 0.08], 1., "adsr 15 5 3 12"),
    (&[1., 0.5, 0.22, 0.1], 0.5, "adsr 15 3 6 6"),
    (&[1., 0.7, 0.4, 0.3, 0.18, 0.1], 1.4, "adsr 15 6 3 12"),
    (
        &[1., 0.55, 0.32, 0.24, 0.14, 0.1, 0.06],
        0.2,
        "adsr 12 1 6 3",
    ),
    (&[1., 0.7, 0.45, 0.22, 0.12, 0.08], 0.8, "adsr 14 2 6 4"),
    (&[1., 0., 0.5, 0., 0.22, 0., 0.1], 0.3, "adsr 14 1 6 4"),
    (&[1., 0.07, 0.13, 0.03], 0.25, "adsr 14 1 6 3"),
    (
        &[1., 0.5, 0.33, 0.25, 0.2, 0.16, 0.14, 0.12],
        0.5,
        "adsr 15 2 6 5",
    ),
    (&[1., 0., 0.33, 0., 0.2, 0., 0.14], 0.3, "adsr 15 2 6 5"),
    (&[1., 0.28, 0.14, 0.08], 0.15, "adsr 11 1 6 2"),
    (&[1., 0.1, 0.8, 0.1, 0.3, 0.05, 0.15], 0.8, "adsr 15 4 3 10"),
];

pub fn id(name: &str) -> usize {
    NAMES.iter().position(|n| *n == name).unwrap() + 1
}

pub fn family(program: u8, role: &str) -> &'static str {
    if role == "bass" {
        return if program == 36 || program == 37 {
            "slap"
        } else {
            "bass"
        };
    }
    match program {
        0..8 => "piano",
        8..16 => "bell",
        16..24 => "organ",
        24..32 => "pluck",
        36 | 37 => "slap",
        32..40 => "bass",
        40..52 => "strings",
        52..56 => "pad",
        56..64 => "brass",
        64..72 => "reed",
        72..80 => "flute",
        80 => "square",
        81..88 => "saw",
        88..96 => "pad",
        96 | 98 => "crystal",
        97..104 => "pad",
        _ => "pluck",
    }
}

pub fn drum(key: u8) -> &'static str {
    match key {
        35 | 36 => "kick",
        38 | 40 => "snare",
        41 | 43 | 45 | 47 | 48 | 50 => "tom",
        46 | 51 | 53 | 59 => "open_hat",
        49 | 52 | 55 | 57 => "crash",
        54 => "tambourine",
        _ => "hat",
    }
}

fn sum(values: impl Iterator<Item = f64>) -> f64 {
    let (mut total, mut correction) = (0.0_f64, 0.0_f64);
    for x in values {
        let t = total + x;
        correction += if total.abs() >= x.abs() {
            (total - t) + x
        } else {
            (x - t) + total
        };
        total = t;
    }
    total + correction
}

fn wav(samples: &[i16]) -> Vec<u8> {
    let bytes = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + bytes as usize);
    out.extend(b"RIFF");
    out.extend((36 + bytes).to_le_bytes());
    out.extend(b"WAVEfmt ");
    out.extend(16_u32.to_le_bytes());
    out.extend(1_u16.to_le_bytes());
    out.extend(1_u16.to_le_bytes());
    out.extend(32000_u32.to_le_bytes());
    out.extend(64000_u32.to_le_bytes());
    out.extend(2_u16.to_le_bytes());
    out.extend(16_u16.to_le_bytes());
    out.extend(b"data");
    out.extend(bytes.to_le_bytes());
    for sample in samples {
        out.extend(sample.to_le_bytes());
    }
    out
}

pub fn write(output: &Path) -> Result<Vec<Value>> {
    let mut descriptions = Vec::new();
    for (index, name) in NAMES.iter().enumerate() {
        let (values, source, envelope) = if index < TONES.len() {
            let (harmonics, bright, envelope) = TONES[index];
            let mut values = Vec::with_capacity(320);
            for i in 0..320 {
                let attack = bright * (1.0 - i as f64 / 256.0).max(0.0).powi(2);
                values.push(sum(harmonics.iter().enumerate().map(|(h, a)| {
                    let h = (h + 1) as f64;
                    a * (1.0 + attack * (h - 1.0)) * (2.0 * PI * h * i as f64 / 16.0).sin()
                })));
            }
            let rms = (sum(values[values.len() - 64..].iter().map(|v| v * v)) / 64.0).sqrt();
            let peak = values.iter().map(|v| v.abs()).fold(0.0_f64, f64::max);
            let gain = (8500.0 / rms).min(26000.0 / peak);
            (
                values
                    .into_iter()
                    .map(|v| (v * gain).round_ties_even() as i16)
                    .collect::<Vec<_>>(),
                json!({"type":"wav","source":format!("{name}.wav"),"loop_point":256,"loop_filter":"reset_filter"}),
                envelope,
            )
        } else {
            let length = match *name {
                "kick" | "snare" => 6400,
                "hat" => 1600,
                "open_hat" | "tom" => 8000,
                "crash" => 16000,
                _ => 4000,
            };
            let mut values = Vec::with_capacity(length);
            let (mut seed, mut phase, mut previous) = (1831565813_u32, 0.0, 0.0);
            for i in 0..length {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                let noise = seed as f64 / 4294967295.0 * 2.0 - 1.0;
                let t = i as f64 / 32000.0;
                let hz = if *name == "kick" {
                    46.0 + 130.0 * (-t * 45.0).exp()
                } else {
                    110.0 + 100.0 * (-t * 22.0).exp()
                };
                phase += 2.0 * PI * hz / 32000.0;
                let (value, decay) = match *name {
                    "kick" | "tom" => (phase.sin(), 23.0),
                    "snare" => (0.7 * noise + 0.3 * (2.0 * PI * 180.0 * t).sin(), 26.0),
                    _ => (
                        0.65 * (noise - previous) + 0.15 * (2.0 * PI * 5760.0 * t).sin(),
                        match *name {
                            "hat" => 80.0,
                            "open_hat" => 16.0,
                            "crash" => 9.0,
                            _ => 32.0,
                        },
                    ),
                };
                previous = noise;
                values.push(
                    (16000.0 * value * (-t * decay).exp() * (i as f64 / 16.0).min(1.0))
                        .round_ties_even() as i16,
                );
            }
            for (i, v) in values[length - 64..].iter_mut().enumerate() {
                *v = (f64::from(*v) * (63 - i) as f64 / 63.0).round_ties_even() as i16;
            }
            (
                values,
                json!({"type":"wav","source":format!("{name}.wav")}),
                "gain 110",
            )
        };
        fs::write(output.join(format!("{name}.wav")), wav(&values))?;
        descriptions.push(json!({"name":name,"source":source,"ignore_gaussian_overflow":false,
            "pitches":{"type":"octave","frequency":2000.0,"first_octave":1,"last_octave":7},"envelope":envelope}));
    }
    Ok(descriptions)
}
