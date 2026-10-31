use crate::Result;
use std::collections::{BTreeMap, VecDeque};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Note {
    pub start: i64,
    pub end: i64,
    pub channel: u8,
    pub key: u8,
    pub velocity: u8,
}

pub struct Midi {
    pub notes: Vec<Note>,
    pub division: i64,
    pub tempo: i64,
    controls: BTreeMap<(u8, u8), Vec<(i64, u8)>>,
}

fn read<'a>(data: &'a [u8], at: &mut usize, length: usize) -> Result<&'a [u8]> {
    let end = at.checked_add(length).ok_or("MIDI extent overflow")?;
    let bytes = data.get(*at..end).ok_or("Truncated MIDI data")?;
    *at = end;
    Ok(bytes)
}

fn vlq(data: &[u8], at: &mut usize) -> Result<i64> {
    let mut value = 0_i64;
    for _ in 0..4 {
        let byte = read(data, at, 1)?[0];
        value = (value << 7) | i64::from(byte & 127);
        if byte < 128 {
            return Ok(value);
        }
    }
    Err("MIDI variable-length value exceeds four bytes".into())
}

pub fn song(bank: &[u8], index: usize) -> Result<&[u8]> {
    let count = *bank.first().ok_or("Empty MIDI bank")? as usize;
    if index >= count {
        return Err("MIDI bank index out of range".into());
    }
    let entry = bank
        .get(1 + 9 * index..10 + 9 * index)
        .ok_or("Truncated MIDI directory")?;
    let offset = u32::from_le_bytes(entry[1..5].try_into()?) as usize;
    let size = u32::from_le_bytes(entry[5..9].try_into()?) as usize;
    if offset < 1 + 9 * count {
        return Err("MIDI song overlaps directory".into());
    }
    bank.get(
        offset
            ..offset
                .checked_add(size)
                .ok_or("MIDI bank extent overflow")?,
    )
    .ok_or_else(|| "MIDI song outside bank".into())
}

impl Midi {
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.get(..4) != Some(b"MThd") {
            return Err("Not a MIDI file".into());
        }
        let division = i64::from(u16::from_be_bytes(
            data.get(12..14)
                .ok_or("Truncated MIDI header")?
                .try_into()?,
        ));
        let mut result = Self {
            notes: Vec::new(),
            division,
            tempo: 0,
            controls: BTreeMap::new(),
        };
        let mut held: BTreeMap<(u8, u8), VecDeque<(i64, u8)>> = BTreeMap::new();
        let mut at = 14;
        while at < data.len() {
            if read(data, &mut at, 4)? != b"MTrk" {
                return Err("Invalid MIDI track header".into());
            }
            let size = u32::from_be_bytes(read(data, &mut at, 4)?.try_into()?) as usize;
            let track = read(data, &mut at, size)?;
            let (mut pos, mut tick, mut status) = (0, 0_i64, 0_u8);
            while pos < track.len() {
                tick += vlq(track, &mut pos)?;
                let first = *track.get(pos).ok_or("Missing MIDI event")?;
                if first == 255 {
                    let kind = read(track, &mut pos, 2)?[1];
                    let length = vlq(track, &mut pos)? as usize;
                    let bytes = read(track, &mut pos, length)?;
                    if kind == 81 && result.tempo == 0 {
                        if bytes.len() != 3 {
                            return Err("Invalid MIDI tempo")?;
                        }
                        result.tempo = bytes.iter().fold(0, |v, b| v * 256 + i64::from(*b));
                    }
                    continue;
                }
                if first == 240 || first == 247 {
                    pos += 1;
                    let length = vlq(track, &mut pos)? as usize;
                    read(track, &mut pos, length)?;
                    continue;
                }
                if first & 128 != 0 {
                    status = first;
                    pos += 1;
                }
                let (kind, channel) = (status >> 4, status & 15);
                if !(8..=14).contains(&kind) {
                    return Err("Invalid MIDI running status".into());
                }
                let args = read(
                    track,
                    &mut pos,
                    if kind == 12 || kind == 13 { 1 } else { 2 },
                )?;
                if kind == 9 && args[1] != 0 {
                    held.entry((channel, args[0]))
                        .or_default()
                        .push_back((tick, args[1]));
                } else if kind == 8 || kind == 9 {
                    if let Some((start, velocity)) = held
                        .get_mut(&(channel, args[0]))
                        .and_then(VecDeque::pop_front)
                    {
                        result.notes.push(Note {
                            start,
                            end: tick,
                            channel,
                            key: args[0],
                            velocity,
                        });
                    }
                }
                if kind == 11 {
                    result
                        .controls
                        .entry((channel, args[0]))
                        .or_default()
                        .push((tick, args[1]));
                }
                if kind == 12 {
                    result
                        .controls
                        .entry((channel, 128))
                        .or_default()
                        .push((tick, args[0]));
                }
            }
        }
        if result.division != 48 || result.tempo == 0 || result.notes.is_empty() {
            return Err("Unsupported MIDI timing or empty score".into());
        }
        result.notes.sort();
        for values in result.controls.values_mut() {
            values.sort();
        }
        Ok(result)
    }

    pub fn control(&self, channel: u8, kind: u8, tick: i64, default: u8) -> u8 {
        self.controls
            .get(&(channel, kind))
            .and_then(|v| v.iter().take_while(|(at, _)| *at <= tick).last())
            .map_or(default, |(_, v)| *v)
    }

    pub fn select(&self, spec: &serde_json::Value) -> Result<Vec<Note>> {
        let channels = spec["channels"].as_array().ok_or("Missing MIDI channels")?;
        let drums = spec["drums"].as_object();
        let mut chosen: Vec<_> = self
            .notes
            .iter()
            .copied()
            .filter(|n| {
                channels
                    .iter()
                    .any(|c| c.as_u64() == Some(n.channel.into()))
                    && drums.is_none_or(|d| d.is_empty() || d.contains_key(&n.key.to_string()))
            })
            .collect();
        if let Some(voice) = spec["voice"].as_u64() {
            let mut groups: BTreeMap<i64, Vec<Note>> = BTreeMap::new();
            for n in chosen {
                groups.entry(n.start).or_default().push(n);
            }
            chosen = groups
                .into_values()
                .filter_map(|mut g| {
                    g.sort_by_key(|n| std::cmp::Reverse(n.key));
                    g.get(voice as usize).copied()
                })
                .collect();
            chosen.sort();
        }
        Ok(chosen)
    }
}
