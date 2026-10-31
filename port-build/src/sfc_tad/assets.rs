use crate::sfc_b1::{add, add_shared};
use crate::{Result, content::Content, import::Files, original::Originals, sha};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sfc_builder::cartridge::Section;
use std::path::Path;
const CONTRACT: &[u8] = include_bytes!("../../../assets/audio.json");
pub(crate) use super::stream::RING_BYTES;
#[derive(Deserialize, Serialize)]
struct Clip {
    code: u8,
    source: usize,
    voice: bool,
    rate_profile: usize,
    sample_rate: f64,
    flags: u8,
    frames: u16,
    file: String,
    sha256: String,
    prefix: Option<Prefix>,
}
#[derive(Deserialize, Serialize)]
struct Prefix {
    file: String,
    sha256: String,
    frames: u8,
}
pub fn import(originals: &Originals, directory: &Path) -> Result<Files> {
    let contract: Value = serde_json::from_slice(CONTRACT)?;
    if contract["tad_io_version"] != super::stream::IO_VERSION
        || contract["live_transfer_bytes"] != super::stream::TRANSFER_BYTES
    {
        return Err("Audio transport contract mismatch".into());
    }
    let mut files = vec![("sound/sound.json".into(), CONTRACT.to_vec())];
    let mut take = |name: &str, expected: &str| -> Result<()> {
        let data = std::fs::read(directory.join(name))?;
        if sha(&data) != expected {
            return Err(format!("TAD audio input changed: {name}").into());
        }
        files.push((Path::new("sound").join(name), data));
        Ok(())
    };
    for (name, digest) in contract["modules"].as_object().ok_or("TAD modules")? {
        take(name, digest.as_str().ok_or("TAD module hash")?)?;
    }
    for song in contract["songs"].as_array().ok_or("TAD songs")? {
        for kind in ["common", "song"] {
            take(
                song[kind]["file"].as_str().ok_or("TAD file")?,
                song[kind]["sha256"].as_str().ok_or("TAD hash")?,
            )?;
        }
    }
    let pcm = originals.file("PCMDATA.WLK")?;
    let voice = originals.file("VOICE.WLK")?;
    let recipes: Value = serde_json::from_slice(crate::recipes::EFFECTS)?;
    let mut clips = Vec::new();
    let (mut resident, mut streamed) = (1_u8, 16_u8);
    for effect in recipes["effects"].as_array().ok_or("Effect recipes")? {
        let is_voice = effect["bank"] == "voice";
        let source = effect["source"].as_u64().ok_or("Effect source")? as usize;
        let (data, rate) = crate::import_sound::pcm(if is_voice { &voice } else { &pcm }, source)?;
        let mut level_recipe = effect.clone();
        if is_voice {
            for (key, value) in recipes["voice"].as_object().ok_or("Voice level recipe")? {
                level_recipe
                    .as_object_mut()
                    .unwrap()
                    .entry(key.clone())
                    .or_insert_with(|| value.clone());
            }
        }
        let rate_profile = super::sample::profile(&data, rate, is_voice);
        let (mut brr, frames) = super::sample::encode(&data, rate, &level_recipe, rate_profile)?;
        let short = effect["place"] == "resident" || effect["place"] == "bank";
        let code = if short {
            let n = resident;
            resident += 1;
            if resident > 16 {
                return Err("Too many resident codes".into());
            }
            n
        } else {
            let n = streamed;
            streamed += 1;
            if streamed >= 0xef {
                return Err("Too many streamed codes".into());
            }
            n
        };
        let slot_two = short || effect["place"] == "stream2";
        let flags = u8::from(effect["ambient"] == true)
            | if slot_two { 2 } else { 0 }
            | if is_voice { 4 } else { 0 }
            | (rate_profile as u8) << 3;
        let file = format!("clip-{code:03}.brr");
        let prefix = if let Some(ms) = effect["repeat_prefix_ms"].as_f64() {
            let (prefix, prefix_frames) =
                super::sample::encode_segment(&data, rate, &level_recipe, rate_profile, Some(ms))?;
            for header in brr.iter_mut().step_by(9) {
                if *header & 3 == 3 {
                    *header &= !3;
                }
            }
            let name = format!("prefix-{code:03}.brr");
            let descriptor = Prefix {
                file: name.clone(),
                sha256: sha(&prefix),
                frames: u8::try_from(prefix_frames)?,
            };
            files.push((Path::new("sound").join(name), prefix));
            Some(descriptor)
        } else {
            None
        };
        clips.push(Clip {
            code,
            source,
            voice: is_voice,
            rate_profile,
            sample_rate: 32000.0 * f64::from(super::stream::PITCHES[rate_profile]) / 4096.0,
            flags,
            frames,
            file: file.clone(),
            sha256: sha(&brr),
            prefix,
        });
        files.push((Path::new("sound").join(file), brr));
    }
    files.push(("sound/clips.json".into(), serde_json::to_vec(&clips)?));
    Ok(files)
}
pub struct Assets {
    root: Content,
    contract: Value,
    clips: Vec<Clip>,
    effect_map: Vec<u8>,
    voice_map: Vec<u8>,
    pub report: Value,
}
impl Assets {
    pub fn load(content: &Content) -> Result<Self> {
        let root = content.sub("sound");
        if root.read("sound.json")? != CONTRACT {
            return Err("Unidentified TAD audio contract".into());
        }
        let contract: Value = serde_json::from_slice(CONTRACT)?;
        let clips: Vec<Clip> = root.json("clips.json")?;
        let (mut effect_map, mut voice_map) = (vec![0; 64], vec![0; 160]);
        for c in &clips {
            let map = if c.voice {
                &mut voice_map
            } else {
                &mut effect_map
            };
            let slot = map.get_mut(c.source).ok_or("TAD sample source index")?;
            if *slot != 0 {
                return Err("Duplicate TAD source mapping".into());
            }
            *slot = c.code;
            if sha(&root.read(&c.file)?) != c.sha256 {
                return Err("TAD sample hash".into());
            }
        }
        let report = json!(
            { "backend" : "tad-live-stream", "contract_sha256" : sha(CONTRACT),
            "external_assets" : false, "effects" : effect_map.iter().filter(|&& c | c !=
            0).count(), "voices" : voice_map.iter().filter(|&& c | c != 0).count(),
            "sample_rate" : super::stream::PLAYBACK_RATE, "stream_ring_bytes" :
            RING_BYTES, "distribution_ready" : false, "effect_map" : effect_map,
            "voice_map" : voice_map, "clips" : clips, "songs" : crate ::sfc_sound::SONGS
            .iter().map(| (name, _, index) | json!({ "name" : name, "source_index" :
            index, "encoded" : contract["songs"].as_array().unwrap().iter().find(| s |
            s["source_index"] ==* index).unwrap() ["name"] })).collect::< Vec < _ >> () }
        );
        Ok(Self {
            root,
            contract,
            clips,
            effect_map,
            voice_map,
            report,
        })
    }
    pub fn effect_code(&self, source: u64) -> Option<u8> {
        self.effect_map
            .get(usize::try_from(source).ok()?)
            .copied()
            .filter(|&c| c != 0)
    }
    pub fn voice_code(&self, source: u64) -> Option<u8> {
        self.voice_map
            .get(usize::try_from(source).ok()?)
            .copied()
            .filter(|&c| c != 0)
    }
    pub fn append(
        &self,
        sections: &mut Vec<Section>,
        space: &mut crate::sfc_rom::Space,
    ) -> Result<()> {
        for (section, file) in [
            ("tad_loader", "loader.bin"),
            ("tad_driver", "audio-driver.bin"),
        ] {
            let mut data = self.root.read(file)?;
            if data.len() % 2 != 0 {
                data.push(0);
            }
            add(sections, space, section.into(), data)?;
        }
        let mut songs = Vec::new();
        let mut frames = Vec::new();
        for (_, _, index) in crate::sfc_sound::SONGS {
            let song = self.contract["songs"]
                .as_array()
                .unwrap()
                .iter()
                .find(|s| s["source_index"] == index)
                .ok_or("TAD song mapping")?;
            let mut row = vec![];
            for kind in ["common", "song"] {
                let mut data = self.root.read(song[kind]["file"].as_str().unwrap())?;
                if data.len() % 2 != 0 {
                    data.push(0);
                }
                let length = u16::try_from(data.len())?;
                let address = add_shared(sections, space, format!("tad_{index}_{kind}"), data)?;
                row.extend(&address.to_le_bytes()[..3]);
                row.push(0);
                row.extend(length.to_le_bytes());
            }
            let common = self
                .root
                .read(song["common"]["file"].as_str().ok_or("Audio common file")?)?;
            for buffer in super::layout::stream_buffers(&common)? {
                row.extend(buffer.to_le_bytes());
            }
            songs.extend(row);
            let duration = if song["looping"] == true {
                0
            } else {
                ((song["seconds"].as_f64().unwrap() + 0.5) * 60.0988).ceil() as u16
            };
            frames.extend(duration.to_le_bytes());
        }
        add(sections, space, "tad_songs".into(), songs)?;
        add(sections, space, "sound_song_frames".into(), frames)?;
        add(
            sections,
            space,
            "sound_effect_map".into(),
            self.effect_map.clone(),
        )?;
        add(
            sections,
            space,
            "sound_voice_map".into(),
            self.voice_map.clone(),
        )?;
        let mut table = vec![0; 256 * 8];
        let mut chant_map = vec![0; 256];
        let mut chants = vec![0; 8];
        for clip in &self.clips {
            let data = self.root.read(&clip.file)?;
            let length = u16::try_from(data.len())?;
            let offset = space.reserve_sample(data.len())?;
            let address = crate::sfc_rom::cpu(offset)?;
            sections.push(Section {
                name: format!("tad_clip_{}", clip.code),
                offset,
                bytes: data,
            });
            let at = usize::from(clip.code) * 8;
            table[at..at + 3].copy_from_slice(&address.to_le_bytes()[..3]);
            table[at + 3] = clip.flags;
            table[at + 4..at + 6].copy_from_slice(&length.to_le_bytes());
            table[at + 6..at + 8].copy_from_slice(&clip.frames.to_le_bytes());
            if let Some(prefix) = &clip.prefix {
                let data = self.root.read(&prefix.file)?;
                if sha(&data) != prefix.sha256 || data.len() % 288 != 0 || prefix.frames == 0 {
                    return Err("Invalid chant prefix".into());
                }
                let length = u16::try_from(data.len())?;
                let address = add(sections, space, format!("tad_prefix_{}", clip.code), data)?;
                chant_map[usize::from(clip.code)] = u8::try_from(chants.len() / 8)?;
                chants.extend(&address.to_le_bytes()[..3]);
                chants.push(prefix.frames);
                chants.extend(length.to_le_bytes());
                chants.push(u8::try_from(
                    (super::stream::MAX_CLIP_FRAMES - clip.frames) / u16::from(prefix.frames),
                )?);
                chants.push(0);
            }
        }
        add(sections, space, "tad_clips".into(), table)?;
        add(sections, space, "tad_chant_map".into(), chant_map)?;
        add(sections, space, "tad_chants".into(), chants)?;
        let rates = (0..super::stream::RATES.len())
            .flat_map(|i| super::stream::frame_bytes(i).to_le_bytes())
            .collect();
        add(sections, space, "tad_stream_rates".into(), rates)?;
        Ok(())
    }
}
