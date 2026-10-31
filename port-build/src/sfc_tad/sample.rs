use super::stream;
use crate::{Result, import_sound};
use serde_json::Value;
pub(super) fn encode(
    data: &[f64],
    rate: f64,
    settings: &Value,
    profile: usize,
) -> Result<(Vec<u8>, u16)> {
    encode_segment(data, rate, settings, profile, None)
}
pub(super) fn encode_segment(
    data: &[f64],
    rate: f64,
    settings: &Value,
    profile: usize,
    prefix_ms: Option<f64>,
) -> Result<(Vec<u8>, u16)> {
    if data.is_empty() || !rate.is_finite() || rate <= 0.0 || data.iter().any(|v| !v.is_finite()) {
        return Err("Invalid PCM sample input".into());
    }
    let target = *stream::RATES
        .get(profile)
        .ok_or("Invalid audio rate profile")?;
    let playback = 32000.0 * f64::from(stream::PITCHES[profile]) / 4096.0;
    let mut samples = import_sound::resample(data, rate, target);
    if samples.is_empty() {
        return Err("Empty converted sample".into());
    }
    let gain = if settings["bank"] == "voice" {
        import_sound::loudness_level(data, rate, settings)
            * settings["gain"].as_f64().unwrap_or(1.0)
    } else {
        let (power, _, total) = powers(data, rate);
        let compensation = if power[profile] > 0.0 {
            (total / power[profile]).sqrt().min(8.0)
        } else {
            1.0
        };
        let volume = settings["volume"].as_u64().unwrap_or(28);
        if volume > 31 {
            return Err("Invalid effect volume".into());
        }
        compensation
            * if volume == 0 {
                0.0
            } else {
                10_f64.powf((volume as f64 - 31.0) * 0.6 / 20.0)
            }
    };
    let knee = settings["knee"].as_f64().unwrap_or(0.7) * 30000.0;
    if !gain.is_finite() || gain < 0.0 || !(0.0..30000.0).contains(&knee) {
        return Err("Invalid sample level settings".into());
    }
    for value in &mut samples {
        *value *= gain;
        if value.abs() > knee {
            *value = value.signum()
                * (knee + (30000.0 - knee) * ((value.abs() - knee) / (30000.0 - knee)).tanh());
        }
    }
    if let Some(ms) = prefix_ms {
        let length = ((ms * target / 1000.0 / 512.0).round() as usize).max(1) * 512;
        if length >= samples.len() {
            return Err("Chant prefix exceeds voice length".into());
        }
        samples.truncate(length);
        let fade = (target * 0.004).ceil() as usize;
        for i in 0..fade {
            samples[i] *= i as f64 / fade as f64;
            samples[length - 1 - i] *= i as f64 / fade as f64;
        }
    }
    let mut brr = crate::brr::encode(&samples);
    if prefix_ms.is_some() {
        for header in brr.iter_mut().step_by(9) {
            *header &= !3;
        }
        let frames = (samples.len() as f64 / playback * stream::FRAMES_PER_SECOND).ceil() as u16;
        return Ok((brr, frames));
    }
    for at in (stream::RING_BYTES - 9..brr.len().saturating_sub(9)).step_by(stream::RING_BYTES) {
        brr[at] |= 3;
    }
    let transfer = usize::from(stream::TRANSFER_BYTES);
    brr.resize(brr.len().div_ceil(transfer) * transfer, 0);
    if brr.len() > u16::MAX as usize {
        return Err("Sample exceeds stream byte counter".into());
    }
    let frames = (samples.len() as f64 / playback * stream::FRAMES_PER_SECOND).ceil() as u16 + 4;
    if frames > stream::MAX_CLIP_FRAMES {
        return Err("Sample exceeds stream playback duration".into());
    }
    Ok((brr, frames))
}
fn powers(data: &[f64], rate: f64) -> ([f64; 4], f64, f64) {
    let spectrum = crate::fft::rfft(data);
    let mut powers = [0.0; 4];
    let mut total = 0.0;
    let mut retained = 0.0;
    for (i, v) in spectrum.iter().enumerate() {
        let p = v.re * v.re + v.im * v.im;
        let frequency = i as f64 * rate / data.len() as f64;
        total += p;
        if frequency <= stream::RATES[2] / 2.0 {
            retained += p;
        }
        for (profile, &target) in stream::RATES.iter().enumerate() {
            if frequency <= target / 2.0 {
                powers[profile] += p * (-10.8 * (frequency / target).powi(2)).exp();
            }
        }
    }
    (
        powers,
        if total > 0.0 { retained / total } else { 1.0 },
        total,
    )
}
pub(super) fn profile(data: &[f64], rate: f64, voice: bool) -> usize {
    if voice {
        return 0;
    }
    let (power, retained, _) = powers(data, rate);
    if retained < 0.6 {
        return 3;
    }
    power.iter().position(|&p| p >= power[2] * 0.7).unwrap_or(2)
}
