mod banks;
mod compact;
mod instruments;
mod midi;
mod score;
use crate::{Result, sha};
use serde_json::{Value, json};
use std::{fs, path::Path};
const MUSIC: &[u8] = include_bytes!("../../../assets/music.json");
const AUDIO: &[u8] = include_bytes!("../../../assets/audio.json");
const RECIPES: &[(&str, &[u8])] = &[
    (
        "00-opening-2",
        include_bytes!("../../../assets/sfc-music/00-opening-2.json"),
    ),
    (
        "01-title",
        include_bytes!("../../../assets/sfc-music/01-title.json"),
    ),
    (
        "02-prologue-arle",
        include_bytes!("../../../assets/sfc-music/02-prologue-arle.json"),
    ),
    (
        "03-ending",
        include_bytes!("../../../assets/sfc-music/03-ending.json"),
    ),
    (
        "04-labyrinth-intro",
        include_bytes!("../../../assets/sfc-music/04-labyrinth-intro.json"),
    ),
    (
        "05-try-out",
        include_bytes!("../../../assets/sfc-music/05-try-out.json"),
    ),
    (
        "06-b5-b7",
        include_bytes!("../../../assets/sfc-music/06-b5-b7.json"),
    ),
    (
        "07-phantom-zone",
        include_bytes!("../../../assets/sfc-music/07-phantom-zone.json"),
    ),
    (
        "08-unused-8",
        include_bytes!("../../../assets/sfc-music/08-unused-8.json"),
    ),
    (
        "09-shop",
        include_bytes!("../../../assets/sfc-music/09-shop.json"),
    ),
    (
        "10-event",
        include_bytes!("../../../assets/sfc-music/10-event.json"),
    ),
    (
        "11-ending-2",
        include_bytes!("../../../assets/sfc-music/11-ending-2.json"),
    ),
    (
        "12-battle",
        include_bytes!("../../../assets/sfc-music/12-battle.json"),
    ),
    (
        "13-battle",
        include_bytes!("../../../assets/sfc-music/13-battle.json"),
    ),
    (
        "14-ancient",
        include_bytes!("../../../assets/sfc-music/14-ancient.json"),
    ),
    (
        "15-guardian",
        include_bytes!("../../../assets/sfc-music/15-guardian.json"),
    ),
    (
        "16-rulue",
        include_bytes!("../../../assets/sfc-music/16-rulue.json"),
    ),
    (
        "17-battle",
        include_bytes!("../../../assets/sfc-music/17-battle.json"),
    ),
    (
        "18-final-boss",
        include_bytes!("../../../assets/sfc-music/18-final-boss.json"),
    ),
    (
        "19-event-item",
        include_bytes!("../../../assets/sfc-music/19-event-item.json"),
    ),
    (
        "20-item",
        include_bytes!("../../../assets/sfc-music/20-item.json"),
    ),
    (
        "21-monster",
        include_bytes!("../../../assets/sfc-music/21-monster.json"),
    ),
    (
        "22-level-up",
        include_bytes!("../../../assets/sfc-music/22-level-up.json"),
    ),
    (
        "23-victory",
        include_bytes!("../../../assets/sfc-music/23-victory.json"),
    ),
    (
        "24-defeat",
        include_bytes!("../../../assets/sfc-music/24-defeat.json"),
    ),
    (
        "25-crystal",
        include_bytes!("../../../assets/sfc-music/25-crystal.json"),
    ),
    (
        "26-panotti",
        include_bytes!("../../../assets/sfc-music/26-panotti.json"),
    ),
    (
        "27-unknown-27",
        include_bytes!("../../../assets/sfc-music/27-unknown-27.json"),
    ),
];
pub fn prepare(bank: &[u8], output: &Path) -> Result<()> {
    let music: Value = serde_json::from_slice(MUSIC)?;
    if Some(bank.len() as u64) != music["source"]["bytes"].as_u64()
        || sha(bank) != music["source"]["sha256"]
    {
        return Err("Unsupported MIDI bank".into());
    }
    let mut projects = Vec::new();
    for accepted in music["songs"]
        .as_array()
        .ok_or("Missing music configuration")?
    {
        let name = accepted["name"].as_str().ok_or("Missing song name")?;
        let recipe = RECIPES
            .iter()
            .find(|(n, _)| *n == name)
            .ok_or("Unknown music arrangement")?
            .1;
        if sha(recipe) != accepted["recipe_sha256"] {
            return Err(format!("Music arrangement changed: {name}").into());
        }
        let recipe: Value = serde_json::from_slice(recipe)?;
        let index = recipe["source"]["index"]
            .as_u64()
            .ok_or("Missing song index")? as usize;
        let midi = midi::Midi::parse(midi::song(bank, index)?)?;
        let (score, used) = score::make(
            &recipe,
            &midi,
            accepted["gain_db"].as_f64().ok_or("Missing music gain")?,
        )?;
        if sha(score.as_bytes()) != accepted["score_sha256"] {
            return Err(format!("Music score mismatch: {name}: {}", sha(score.as_bytes())).into());
        }
        projects.push((name.to_owned(), compact::score(&score), used));
    }
    let audio: Value = serde_json::from_slice(AUDIO)?;
    banks::expand(&audio["instrument_banks"], &mut projects)?;
    fs::create_dir(output)?;
    let samples = instruments::write(output)?;
    let mut streams = Vec::new();
    for slot in 0..2 {
        let mut ring = vec![0_u8; crate::sfc_tad::stream::RING_BYTES + 2];
        ring[2] = (slot + 1) << 4;
        ring[crate::sfc_tad::stream::RING_BYTES + 2 - 9] = 3;
        fs::write(output.join(format!("stream{slot}.brr")), ring)?;
        for profile in 0..crate::sfc_tad::stream::RATES.len() {
            streams.push(crate::sfc_tad::stream::effect_program(slot, profile));
        }
    }
    fs::write(output.join("streams.txt"), streams.join("\n"))?;
    for (name, score, used) in projects {
        fs::write(output.join(format!("{name}.mml")), score)?;
        let mut instruments: Vec<_> = samples
            .iter()
            .filter(|s| used.contains(s["name"].as_str().unwrap()))
            .cloned()
            .collect();
        for slot in 0..2 {
            instruments.push(json!({"name":format!("stream{slot}"),"source":{"type":"brr","source":format!("stream{slot}.brr")},
                "pitches":{"type":"samples","sample_rates":[crate::sfc_tad::stream::INPUT_RATE as u32]},"envelope":"gain 127"}));
        }
        let effects: Vec<_> = (0..crate::sfc_tad::stream::RATES.len())
            .flat_map(|profile| (0..2).map(move |slot| format!("stream{slot}_{profile}")))
            .collect();
        let project = json!({"brr_samples":instruments,"sound_effect_file":"streams.txt","sound_effects":effects,
            "songs":[{"name":format!("song_{}",name.replace('-',"_")),"source":format!("{name}.mml")}]});
        fs::write(
            output.join(format!("{name}.terrificaudio")),
            serde_json::to_vec(&project)?,
        )?;
    }
    fs::write(output.join("music.json"), AUDIO)?;
    Ok(())
}
