use super::*;
pub struct Tables {
    controller: Controller,
    thresholds: Vec<u8>,
    growth: Vec<u8>,
    songs: Vec<u8>,
    ending: u32,
}
pub fn load(content: &crate::content::Content) -> Result<Tables> {
    let controller = crate::sfc_battle_controller::load(content)?;
    let growth: serde_json::Value = content.json("level-growth.json")?;
    let mut thresholds = Vec::new();
    for t in growth["thresholds"]
        .as_array()
        .ok_or("Missing level thresholds")?
    {
        thresholds.extend((t.as_u64().ok_or("Threshold")? as u32).to_le_bytes());
    }
    if thresholds.len() != 400 {
        return Err("Level thresholds are not 100".into());
    }
    let mut slots = Vec::new();
    for row in growth["learned_slots"]
        .as_array()
        .ok_or("Missing spell growth")?
    {
        slots.push(row["level"].as_u64().ok_or("Growth level")? as u8);
        slots.push(row["slot"].as_u64().ok_or("Growth slot")? as u8);
    }
    if slots.len() != 32 {
        return Err("Spell growth is not 16 rows".into());
    }
    let actors: serde_json::Value = content.json("combat-actors.json")?;
    let mut songs = vec![0u8; crate::sfc_battle_scripts::DESCRIPTORS];
    for actor in actors["entry_resources"]["actors"]
        .as_array()
        .ok_or("Missing battle actor resources")?
    {
        let id = actor["id"].as_u64().ok_or("Battle actor id")? as usize;
        for node in actor["load_graph"]["nodes"]
            .as_array()
            .ok_or("Load graph")?
        {
            let raw = node["raw_hex"].as_str().ok_or("Load node bytes")?;
            if let Some(song) = raw.strip_prefix("10c022") {
                *songs.get_mut(id).ok_or("Battle actor id out of range")? =
                    u8::from_str_radix(&song[..2], 16)?;
            }
        }
    }
    let ending_json: serde_json::Value = content.json("ending.json")?;
    let descriptor = ending_json["descriptor_va"]
        .as_u64()
        .ok_or("Ending descriptor")?;
    let reserves = ending_json["transition_commands"]
        .as_array()
        .ok_or("Ending transition")?
        .iter()
        .any(|c| {
            c["va"] == 0x444e20
                && c["raw_hex"].as_str()
                    == Some(&format!("88000000{:08x}", (descriptor as u32).swap_bytes()))
        });
    if !reserves {
        return Err("The final completion path does not reserve the ending".into());
    }
    let ending = ending_json["script_va"].as_u64().ok_or("Ending script")? as u32;
    Ok(Tables {
        controller,
        ending,
        thresholds,
        growth: slots,
        songs,
    })
}
pub fn sections(
    sections: &mut Vec<Section>,
    space: &mut crate::sfc_rom::Space,
    tables: &Tables,
) -> Result<()> {
    let add = |s: &mut Vec<Section>, c: &mut crate::sfc_rom::Space, name: &str, bytes: Vec<u8>| {
        crate::sfc_b1::add(s, c, name.into(), bytes).map(|_| ())
    };
    add(
        sections,
        space,
        "battle_program",
        tables.controller.program.clone(),
    )?;
    add(
        sections,
        space,
        "battle_thresholds",
        tables.thresholds.clone(),
    )?;
    add(sections, space, "battle_growth", tables.growth.clone())?;
    add(sections, space, "battle_songs", tables.songs.clone())?;
    add(
        sections,
        space,
        "battle_ending_scene",
        tables.ending.to_le_bytes().to_vec(),
    )?;
    let mut entry = tables.controller.entry.to_le_bytes().to_vec();
    entry.extend(tables.controller.growth.to_le_bytes());
    add(sections, space, "battle_entry", entry)
}
