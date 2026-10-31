use crate::{
    Result, SOURCE,
    b1::{Action, World},
    sha,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
pub const GATES: [(u8, u8); 2] = [(7, 1), (11, 8)];
#[derive(Default, Debug, Serialize)]
pub struct Progress {
    pub flags: u16,
    pub golden_apples: u8,
}
impl Progress {
    pub fn locked(&self, event: Option<u8>) -> bool {
        GATES
            .iter()
            .any(|(id, mask)| event == Some(*id) && self.flags & u16::from(*mask) == 0)
    }
    pub fn step(&self, world: &World, before: [u8; 4], action: Action) -> Result<([u8; 4], u8)> {
        if matches!(action, Action::Forward) && self.locked(world.event(before).map(|e| e.id)) {
            Ok((before, 3))
        } else {
            world.step(before, action)
        }
    }
    pub fn stairs_choice(&self, before: [u8; 4], yes: bool) -> Result<[u8; 4]> {
        if before[0..3] != [1, 7, 7] || before[3] >= 4 {
            return Err("B1 stairs choice outside exit tile".into());
        }
        Ok(if yes {
            [2, 7, 7, before[3]]
        } else {
            [1, 7, 6, 0]
        })
    }
    pub fn switch_choice(&mut self, yes: bool) -> u32 {
        if self.flags & 1 != 0 {
            return 0x49ca1c;
        }
        if !yes {
            return 0x49c9ca;
        }
        self.flags |= 1;
        0x49c94a
    }
    pub fn chest_choice(&mut self, yes: bool) -> Vec<u32> {
        if !yes {
            return vec![0x479c1c];
        }
        if self.flags & 8 != 0 {
            return vec![0x479c58];
        }
        self.flags |= 8;
        vec![0x49d6de, 0x49d770]
    }
    pub fn broken_wall(&mut self) -> u32 {
        if self.flags & 4 != 0 {
            return 0x49c7f0;
        }
        self.golden_apples = self.golden_apples.saturating_add(1);
        self.flags |= 4;
        0x49c74a
    }
}
#[derive(Deserialize)]
struct Case {
    flags: u16,
    event: u8,
    before: [u8; 4],
    action: Action,
    lock: u8,
    after: [u8; 4],
    effect: u8,
}
#[derive(Deserialize)]
struct Reference {
    schema: u8,
    source_sha256: String,
    cases: Vec<Case>,
}
pub fn compile(
    root: &crate::content::Content,
    natives: Option<&std::path::Path>,
) -> Result<Vec<(String, Vec<u8>)>> {
    let world_bytes = root.read("b1-world.json")?;
    let world: World = serde_json::from_slice(&world_bytes)?;
    world.validate()?;
    let source: Value = serde_json::from_slice(&world_bytes)?;
    let bindings = [
        (0x4514dc, "31000500"),
        (0x4514e4, "3201050060154500"),
        (0x4514ec, "31010500"),
        (0x4514fc, "2f010100c2994700"),
        (0x451504, "13c01d0158154500"),
        (0x45152c, "7000000044184300"),
        (0x45155c, "78070601"),
        (0x451bd4, "320104005c1c4500"),
        (0x451bfc, "2f010100b2d34900"),
        (0x451c1c, "2f01010048d44900"),
        (0x451c24, "29120000"),
        (0x451c28, "2f01010086d54900"),
        (0x451c30, "07020000381e4500"),
        (0x451c38, "2f010100ded54900"),
        (0x451c40, "29130000"),
        (0x451c44, "31010400"),
        (0x451c5c, "2f01010004d44900"),
        (0x451740, "320101005c174500"),
        (0x451748, "2f010100c0984700"),
        (0x451750, "31010100"),
        (0x45175c, "2f01010058984700"),
        (0x451784, "32010100a0174500"),
        (0x4517c8, "32010100e4174500"),
        (0x45180c, "3201010028184500"),
        (0x451644, "2f010100a6c84900"),
        (0x4518a4, "2f01010018994700"),
        (0x4518c8, "2f01010058984700"),
        (0x4518f4, "2f01010018994700"),
        (0x45190c, "2f01010058984700"),
        (0x451934, "2f01010058984700"),
        (0x45195c, "2f01010090cd4900"),
        (0x451988, "32010600a8194500"),
        (0x451998, "2f010100e6ce4900"),
        (0x4519c0, "2f0101009ccf4900"),
        (0x4519cc, "31010600"),
        (0x451a50, "32010700701a4500"),
        (0x451a60, "2f01010040d04900"),
        (0x451a88, "2f0101008ad14900"),
        (0x451a94, "31010700"),
        (0x451b18, "32010800381b4500"),
        (0x451b28, "2f0101002ad24900"),
        (0x451b50, "2f010100f4d24900"),
        (0x451b5c, "31010800"),
        (0x451884, "32010000b0184500"),
        (0x45188c, "7e010000"),
        (0x4518bc, "32010300c8184500"),
        (0x4518c4, "7e010000"),
        (0x451664, "32010000a8164500"),
        (0x451674, "13c11d0188164500"),
        (0x45167c, "2f010100cac94900"),
        (0x451698, "2f0101004ac94900"),
        (0x4516a0, "31010000"),
        (0x4516a8, "2f0101001cca4900"),
        (0x451d70, "13c11d01881d4500"),
        (0x451d78, "2f0101001c9c4700"),
        (0x451d94, "32000300ac1d4500"),
        (0x451d9c, "2f010100589c4700"),
        (0x451dac, "2f010100ded64900"),
        (0x451db4, "31010300"),
        (0x451db8, "2f01010070d74900"),
        (0x451844, "13c11e0150184500"),
        (0x451854, "3201020074184500"),
        (0x45185c, "2f0101004ac74900"),
        (0x451864, "81003500"),
        (0x451868, "31010200"),
        (0x451874, "2f010100f0c74900"),
    ];
    let commands = source["commands"].as_array().ok_or("Missing B1 commands")?;
    for (address, bytes) in bindings {
        if !commands
            .iter()
            .any(|n| n["va"] == address && n["raw_hex"] == bytes)
        {
            return Err(format!("B1 progress rule changed at {address:#x}").into());
        }
    }
    let native = natives.map(|dir| verify_native(&world, dir)).transpose()?;
    let mut progress = Progress::default();
    let mut sequence = Vec::new();
    for (action, messages) in [
        ("switch_no", vec![progress.switch_choice(false)]),
        ("switch_yes", vec![progress.switch_choice(true)]),
        ("switch_again", vec![progress.switch_choice(true)]),
        ("chest_no", progress.chest_choice(false)),
        ("chest_yes", progress.chest_choice(true)),
        ("chest_again", progress.chest_choice(true)),
        ("wall_first", vec![progress.broken_wall()]),
        ("wall_again", vec![progress.broken_wall()]),
    ] {
        sequence.push(serde_json::json!({ "action" : action, "messages" : messages }));
    }
    let report = serde_json::json!(
        { "schema" : 1, "scope" :
        "B1 host progression rules; not a ROM execution report", "source_sha256" :
        SOURCE, "world_sha256" : sha(& world_bytes), "native" : native, "source_bindings"
        : bindings.as_slice(), "sequence" : sequence, "final_state" : progress, "flags" :
        { "switch_on" : 0, "golden_apple_taken" : 2, "direction_stone_taken" : 3 },
        "inventory" : { "golden_apple_index" : 53, "direction_stone" :
        "flag 3; not a generic inventory increment" }, "limits" :
        ["This report does not verify choice UI, event drawing/audio, wall trigger or SFC execution.",
        "Flags gate forward movement at event 7 and 11; no map-cell rewrite is claimed."]
        }
    );
    Ok(vec![(
        "b1-progress-build.json".into(),
        serde_json::to_vec_pretty(&report)?,
    )])
}
fn verify_native(world: &World, natives: &std::path::Path) -> Result<Value> {
    let native_bytes = fs::read(natives.join("../b1-progress-native.json"))?;
    let native: Reference = serde_json::from_slice(&native_bytes)?;
    if native.schema != 1 || native.source_sha256 != SOURCE || native.cases.len() != 16 {
        return Err("Invalid B1 progression reference".into());
    }
    for case in &native.cases {
        let state = Progress {
            flags: case.flags,
            golden_apples: 0,
        };
        if world.event(case.before).map(|e| e.id) != Some(case.event)
            || u8::from(state.locked(Some(case.event))) != case.lock
            || state.step(world, case.before, case.action)? != (case.after, case.effect)
        {
            return Err("B1 progression gate differs from native event and movement".into());
        }
    }
    let exit_raw = fs::read(natives.join("b1-exit.json"))?;
    let exit: Value = serde_json::from_slice(&exit_raw)?;
    let exit_cases = exit["cases"]
        .as_array()
        .ok_or("Missing B1 exit observations")?;
    if exit["source_sha256"] != SOURCE || exit_cases.len() != 8 {
        return Err("Invalid B1 exit reference".into());
    }
    for case in exit_cases {
        let before: [u8; 4] = serde_json::from_value(case["before"].clone())?;
        let expected: [u8; 4] = serde_json::from_value(case["after"].clone())?;
        let yes = match case["action"].as_str() {
            Some("descend") => true,
            Some("cancel") => false,
            _ => return Err("Unknown stairs action".into()),
        };
        if Progress::default().stairs_choice(before, yes)? != expected {
            return Err("B1 stairs state differs from original handler".into());
        }
    }
    for case in exit["one_way"]["native_moves"]
        .as_array()
        .ok_or("Missing one-way passage")?
    {
        let before = serde_json::from_value(case["before"].clone())?;
        let after = serde_json::from_value(case["after"].clone())?;
        let effect = serde_json::from_value(case["effect"].clone())?;
        if world.step(before, Action::Forward)? != (after, effect) {
            return Err("B1 one-way passage differs from original map".into());
        }
    }
    Ok(serde_json::json!(
        { "native_sha256" : sha(& native_bytes), "native_gate_cases" : native.cases
        .len(), "exit_reference_sha256" : sha(& exit_raw), "native_exit_cases" :
        exit_cases.len(), "one_way_passage" : exit["one_way"] }
    ))
}
