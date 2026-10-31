use super::{instruments, midi::Midi};
use crate::Result;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

fn number(v: &Value, key: &str, default: i64) -> i64 {
    v[key].as_i64().unwrap_or(default)
}
fn label<'a>(v: &'a Value, key: &str, default: &'a str) -> &'a str {
    v[key].as_str().unwrap_or(default)
}
fn rounded(v: f64) -> i64 {
    v.round_ties_even() as i64
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Hit {
    pitch: i64,
    volume: i64,
    tone: &'static str,
    program: u8,
}

fn monophonic(mut events: Vec<(i64, i64, Hit)>, total: i64, fill: i64) -> Vec<(i64, Option<Hit>)> {
    events.sort();
    let starts: BTreeMap<_, _> = events
        .into_iter()
        .filter(|(s, _, _)| *s >= 0 && *s < total)
        .map(|(s, l, k)| (s, (l, k)))
        .collect();
    let mut result = Vec::new();
    let entries: Vec<_> = starts.into_iter().collect();
    let mut now = 0;
    for (i, (start, (length, hit))) in entries.iter().enumerate() {
        let next = entries.get(i + 1).map_or(total, |(s, _)| *s);
        let mut length = (*length).min(next - start);
        let gap = next - start - length;
        if gap > 0 && gap <= fill {
            length = next - start;
        }
        if *start > now {
            result.push((start - now, None));
        }
        if length > 0 {
            result.push((length, Some(hit.clone())));
        }
        now = start + length.max(0);
    }
    if now < total {
        result.push((total - now, None));
    }
    result
}

pub fn make(recipe: &Value, midi: &Midi, gain_db: f64) -> Result<(String, BTreeSet<&'static str>)> {
    let tracks = recipe["tracks"].as_array().ok_or("Missing music tracks")?;
    if tracks.len() > 8 {
        return Err("Music exceeds eight channels".into());
    }
    let looping = recipe["loop"].as_bool().unwrap_or(true);
    let mut offset = number(recipe, "offset_ticks", 0);
    let lead = if looping {
        midi.notes.iter().map(|n| n.start).min().unwrap() - offset
    } else {
        0
    };
    offset += lead;
    let mut total = midi.notes.iter().map(|n| n.end).max().unwrap() - offset;
    let ppqn = number(recipe, "ppqn", 12);
    if ppqn <= 0 || ppqn > midi.division {
        return Err("Invalid score resolution".into());
    }
    let unit = if recipe["whole_bars"].as_bool().unwrap_or(true) {
        192
    } else {
        midi.division / ppqn
    };
    total = ((total as f64 / unit as f64).ceil() as i64) * unit;
    let scale = if (midi.tempo as f64 / (midi.division * 125) as f64) < 64.0 {
        2
    } else {
        1
    };
    total = (total as f64 / scale as f64).ceil() as i64;
    let lead = rounded(lead as f64 / scale as f64);
    let timer = rounded((midi.tempo * scale) as f64 / (midi.division * 125) as f64);
    if !(64..=256).contains(&timer) {
        return Err("Unsupported music timer".into());
    }
    let factor = 10_f64.powf(gain_db / 20.0);
    let main = rounded(100.0 * factor).clamp(100, 127);
    let residual = factor / (main as f64 / 100.0);
    let mut lines = vec![
        format!("#Title KR MIDI {}", recipe["source"]["index"]),
        format!("#Timer {timer}"),
        "#ZenLen 192".into(),
        format!("#MainVolume {main}"),
        "#SpcFadeout 0".into(),
        "#EchoLength 96".into(),
        "#EchoFeedback 24".into(),
        format!(
            "#EchoVolume {} {}",
            rounded((18 * main) as f64 / 100.0),
            rounded((15 * main) as f64 / 100.0)
        ),
        "#FirFilter 96 24 8 0 0 0 0 0".into(),
    ];
    let header = lines.len();
    let mut used = BTreeSet::new();
    for (index, spec) in tracks.iter().enumerate() {
        let original = if let Some(copy) = spec["copy_of"].as_str() {
            tracks
                .iter()
                .find(|t| t["name"] == copy)
                .ok_or("Unknown music copy source")?
        } else {
            spec
        };
        let notes = midi.select(original)?;
        let role = label(spec, "role", label(original, "role", "lead"));
        let mut base: f64 = match role {
            "lead" => 165.,
            "countermelody" => 108.,
            "bass" => 115.,
            "accompaniment" => 80.,
            "pad" => 68.,
            "echo" => 58.,
            "layer" => 76.,
            "kick snare" => 105.,
            "cymbals" => 75.,
            _ => 100.,
        };
        if role == "pad" && !tracks.iter().any(|t| t["role"] == "lead") {
            base = 110.0;
        }
        let peak = tracks
            .iter()
            .filter(|t| label(t, "role", "lead") == role)
            .map(|t| number(t, "volume", 24))
            .max()
            .unwrap_or(24);
        base *= 10_f64.powf((number(spec, "volume", 24) - peak) as f64 * 0.6 / 20.0);
        let mut velocities: Vec<_> = notes.iter().map(|n| n.velocity).collect();
        velocities.sort();
        let reference = if velocities.is_empty() {
            100.0
        } else if velocities.len() % 2 == 1 {
            velocities[velocities.len() / 2] as f64
        } else {
            (velocities[velocities.len() / 2 - 1] as f64 + velocities[velocities.len() / 2] as f64)
                / 2.0
        };
        let copied = spec["copy_of"].is_string();
        let delay = if copied {
            rounded(number(spec, "delay", 0) as f64 / scale as f64)
        } else {
            0
        };
        let drums = original["drums"].as_object().is_some_and(|d| !d.is_empty());
        let mut events = Vec::new();
        for n in notes {
            let program = midi.control(n.channel, 128, n.start, 0);
            let tone = if drums {
                instruments::drum(n.key)
            } else {
                instruments::family(program, label(original, "role", role))
            };
            let cc = (u32::from(midi.control(n.channel, 7, n.start, 127))
                * u32::from(midi.control(n.channel, 11, n.start, 127))) as f64
                / 16129.0;
            let volume =
                rounded(base * (n.velocity as f64 / reference).sqrt().min(1.15) * cc).clamp(0, 240);
            let length = if drums {
                total
            } else {
                rounded((n.end - n.start) as f64 / scale as f64).max(2)
            };
            let start = rounded((n.start - offset) as f64 / scale as f64);
            if start < total {
                events.push((
                    start,
                    length,
                    Hit {
                        pitch: n.key as i64
                            + if copied {
                                number(spec, "transpose", 0)
                            } else {
                                0
                            },
                        volume,
                        tone,
                        program,
                    },
                ));
            }
        }
        let tokens = monophonic(
            events,
            total,
            number(original, "fill_gaps", 0) * (midi.division / ppqn) / scale,
        );
        let pan = (64 - rounded(number(spec, "pan", 0) as f64 / 2.0)).clamp(0, 128);
        let mut parts = vec![format!("p{pan}")];
        if lead + delay != 0 {
            parts.push(format!("r%{}", lead + delay));
        }
        if looping {
            parts.push("L".into());
        }
        for (length, hit) in tokens {
            let Some(hit) = hit else {
                parts.push(format!("r%{length}"));
                continue;
            };
            used.insert(hit.tone);
            let volume = rounded(hit.volume as f64 * residual);
            if volume > 255 {
                return Err("Music gain exceeds channel range".into());
            }
            parts.push(format!("@{} V{volume}", instruments::id(hit.tone)));
            if drums {
                parts.push(format!("E0 MP0 PR32000,%{length}"));
                continue;
            }
            parts.push(if role == "bass" { "E0" } else { "E1" }.into());
            if ["strings", "brass", "reed", "flute", "saw", "pad"].contains(&hit.tone) {
                let depth = if hit.tone == "strings" || hit.tone == "pad" {
                    7
                } else {
                    12
                };
                parts.push(format!(
                    "MP{depth},{},{}",
                    rounded(8000.0 / timer as f64 / 20.0).max(1),
                    rounded(0.15 * 8000.0 / timer as f64).max(1)
                ));
            } else {
                parts.push("MP0".into());
            }
            if hit.pitch > 107 {
                let hz = 440.0 * 2_f64.powf((hit.pitch - 69) as f64 / 12.0);
                parts.push(format!("PF{},%{length}", rounded(hz)));
            } else {
                parts.push(format!("l%{} n{}", length.min(192), hit.pitch));
                if length > 192 {
                    parts.push(format!("^%{}", length - 192));
                }
            }
        }
        lines.push(format!(
            "{} {}",
            (b'A' + index as u8) as char,
            parts.join(" ")
        ));
    }
    lines.splice(
        header..header,
        used.iter()
            .map(|name| format!("@{} {name}", instruments::id(name))),
    );
    Ok((lines.join("\n") + "\n", used))
}
