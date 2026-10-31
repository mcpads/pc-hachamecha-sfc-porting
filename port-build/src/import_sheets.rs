use crate::{Result, SOURCE, import::Files, original::Originals, sha};
use serde_json::json;
const BATTLE_BODIES: &[&str] = &[
    "ANM.CNS",
    "ANM_ATK.CNS",
    "ANM_HEAL.CNS",
    "A_DEAD.CNS",
    "EFECT.CNS",
    "AT_EFECT.CNS",
    "DOKU.CNS",
    "KAR.CNS",
    "PUYO.CNS",
    "SKELTON.CNS",
    "SCU.CNS",
    "RULUE.CNS",
    "RULUE_A.CNS",
    "SCHEZO1.CNS",
    "SCHEZO2.CNS",
    "SHEZ_EF.CNS",
    "LB_2.CNS",
    "LB_EF.CNS",
    "JSTORM.CNS",
    "HARPY.CNS",
    "HARPY2.CNS",
    "HARPY3.CNS",
    "UROK.CNS",
    "UROK_ATK.CNS",
    "WITCH_N.CNS",
    "WITCH.CNS",
    "DRAC.CNS",
    "DRAC_ATK.CNS",
    "AT_EFEC2.CNS",
    "OWLBEAR.CNS",
    "SATAN.CNS",
    "STN_EF.CNS",
    "ARCH_A.CNS",
    "ARCH_B.CNS",
    "ARCH_C.CNS",
    "ARCH_EF.CNS",
    "L_POOR.CNS",
    "LP_HI.CNS",
    "SPECTER.CNS",
    "LDOOL.CNS",
    "DOPPER1.CNS",
    "DOPPER2.CNS",
    "DOPPER3.CNS",
    "J_THUN.CNS",
    "MAMONO.CNS",
    "MA_PLA.CNS",
    "JE_RED.CNS",
    "RED_R.CNS",
    "GW.CNS",
    "JE_YELLO.CNS",
    "YELLOW_D.CNS",
    "JE_BLUE.CNS",
    "BLUE_S.CNS",
    "JE_GREEN.CNS",
    "GREEN_E.CNS",
    "LASTBOSS.CNS",
    "LB_BARIA.CNS",
    "LASTBOS2.CNS",
];
const SHARD_CLIPS: (u32, usize) = (0x47b813, 15);
fn json(value: serde_json::Value) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(&value)?;
    bytes.push(b'\n');
    Ok(bytes)
}
fn sheet(
    files: &mut Files,
    folder: &str,
    name: &str,
    sheet: &crate::cns::Sheet,
    rgb: &[u8],
) -> serde_json::Value {
    files.push((
        format!("{folder}/{name}.indices").into(),
        sheet.pixels.clone(),
    ));
    files.push((format!("{folder}/{name}.rgb").into(), rgb.to_vec()));
    json!(
        { "file" : name, "width" : sheet.width, "height" : sheet.height, "pixels_sha256"
        : sha(& sheet.pixels), "palette_sha256" : sha(rgb) }
    )
}
pub fn import(originals: &Originals) -> Result<Files> {
    let mut files = Files::new();
    let exe = originals.exe()?;
    let smoke = originals.cns("KEMURI.CNS")?;
    let mut description = sheet(&mut files, "b3-smoke", "KEMURI.CNS", &smoke, &smoke.rgb);
    description["schema"] = 1.into();
    description["source_sha256"] = SOURCE.into();
    files.push(("b3-smoke/smoke.json".into(), json(description)?));
    let shards = originals.cns("P_KAKERA.CNS")?;
    let mut description = sheet(
        &mut files,
        "b4-shards",
        "P_KAKERA.CNS",
        &shards,
        &shards.rgb,
    );
    let (at, count) = SHARD_CLIPS;
    let clips: Vec<[i32; 4]> = exe
        .read(at, count * 16)?
        .chunks(16)
        .map(|c| {
            std::array::from_fn(|i| i32::from_le_bytes(c[i * 4..i * 4 + 4].try_into().unwrap()))
        })
        .collect();
    description["schema"] = 1.into();
    description["source_sha256"] = SOURCE.into();
    description["clips"] = json!(clips);
    files.push(("b4-shards/shards.json".into(), json(description)?));
    let mut sheets = Vec::new();
    for name in BATTLE_BODIES {
        let body = originals.cns(name)?;
        if ![16, 32].contains(&body.colors) {
            return Err(format!("Unsupported battle sheet: {name}").into());
        }
        let rgb = body.rgb[..body.colors * 3].to_vec();
        sheets.push(sheet(&mut files, "battle-bodies", name, &body, &rgb));
    }
    files.push((
        "battle-bodies.json".into(),
        json(json!({ "schema" : 1, "sheets" : sheets }))?,
    ));
    Ok(files)
}
