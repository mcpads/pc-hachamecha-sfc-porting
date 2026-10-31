use crate::{
    Result, SOURCE,
    import::Files,
    message_script::decode_message,
    original::{Exe, Originals},
    sha,
};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
const STRIPS: [(&str, u32); 9] = [
    ("main", 0x443f45),
    ("battle_main", 0x443fa5),
    ("shop_main", 0x444005),
    ("magic", 0x444065),
    ("items", 0x4440f5),
    ("settings", 0x4441b5),
    ("options", 0x444225),
    ("save_slots", 0x444275),
    ("load_slots", 0x4442c5),
];
const HOVER_TABLE: u32 = 0x43f72c;
const ITEM_ICONS: u32 = 0x4bdf9c;
const FRAME_RECTANGLES: u32 = 0x443a41;
const ITEMS_SLOT: u16 = 16;
const FRAME_SLOT: u16 = 2;
const SHEETS: [&str; 4] = ["ITEMS.CNS", "NUMBER.CNS", "MAPMINI.CNS", "MAPPARTS.CNS"];
const B1_STATE: [(u64, u64); 8] = [
    (79, 0),
    (80, 0),
    (85, 0),
    (86, 0),
    (87, 0),
    (93, 0),
    (94, 0),
    (95, 0),
];
const B1_MONEY: u32 = 100;
pub const HEAL_STEPS: u64 = 8;
const FIELD: u64 = 1;
type State = BTreeMap<u64, u64>;
struct Player<'a> {
    exe: &'a Exe,
    state: State,
    steps: Vec<Value>,
    window: Vec<Value>,
    audio: Vec<Value>,
}
impl Player<'_> {
    fn pick(&self, token: &Value) -> Result<u32> {
        let targets = token["targets"].as_array().ok_or("Dispatch targets")?;
        let selector = self.state[&token["selector_index"].as_u64().ok_or("Selector")?] as usize;
        Ok(targets[selector.min(targets.len() - 1)]
            .as_u64()
            .ok_or("Target")? as u32)
    }
    fn run(&mut self, address: u32, depth: usize) -> Result<&'static str> {
        if depth > 16 {
            return Err("Message nesting too deep".into());
        }
        let message = decode_message(self.exe, address, true)?;
        for token in message["tokens"].as_array().ok_or("Tokens")? {
            let kind = token["kind"].as_str().ok_or("Token kind")?;
            let target =
                |key: &str| -> Result<u32> { Ok(token[key].as_u64().ok_or("Target")? as u32) };
            match kind {
                "text" | "newline" | "palette_color" => {
                    let mut entry = Map::new();
                    for k in ["kind", "text", "value"] {
                        if let Some(v) = token.get(k) {
                            entry.insert(k.into(), v.clone());
                        }
                    }
                    self.window.push(Value::Object(entry));
                }
                "clear" => self.window.clear(),
                "wait_ticks" | "wait_input" => {
                    let ticks = token.get("value").and_then(Value::as_u64).unwrap_or(0x8000);
                    if self.window.iter().any(|t| t["kind"] == "text") {
                        let wait = if kind == "wait_input" || ticks >= 0x8000 {
                            json!("input")
                        } else {
                            json!(ticks)
                        };
                        self.steps.push(json!(
                            { "kind" : "page", "tokens" : self.window.clone(), "wait" :
                            wait }
                        ));
                    }
                }
                "animation_signal" => self
                    .steps
                    .push(json!({ "kind" : "signal", "mask" : token["mask"] })),
                "audio" | "sound_effect" | "repeated_audio" => {
                    let mut sound = token.as_object().unwrap().clone();
                    sound.remove("bytes");
                    sound.remove("opcode");
                    self.audio.push(Value::Object(sound.clone()));
                    sound.insert("kind".into(), "audio".into());
                    sound.insert("token".into(), kind.into());
                    self.steps.push(Value::Object(sound));
                }
                "external_word_number" => self
                    .window
                    .push(json!({ "kind" : "text", "text" : B1_MONEY.to_string() })),
                "message_call" | "message_dispatch_call" => {
                    let next = if kind == "message_call" {
                        target("target_va")?
                    } else {
                        self.pick(token)?
                    };
                    if self.run(next, depth + 1)? == "end" {
                        return Ok("end");
                    }
                }
                "message_dispatch" => return self.run(self.pick(token)?, depth + 1),
                "message_branch" => {
                    let hold = token["conditions"]
                        .as_array()
                        .ok_or("Conditions")?
                        .iter()
                        .all(|c| {
                            let value = self.state[&c["offset"].as_u64().unwrap()];
                            if c["kind"] == 1 {
                                value != c["value"].as_u64().unwrap()
                            } else {
                                value == c["value"].as_u64().unwrap()
                            }
                        });
                    let next = token["targets"][if hold { 0 } else { 1 }]
                        .as_u64()
                        .ok_or("Branch")? as u32;
                    return self.run(next, depth + 1);
                }
                "message_jump" => return self.run(target("target_va")?, depth + 1),
                "message_global_branch" => {
                    let index = if token["value"].as_u64() == Some(FIELD) {
                        0
                    } else {
                        1
                    };
                    return self.run(
                        token["targets"][index].as_u64().ok_or("Branch")? as u32,
                        depth + 1,
                    );
                }
                "return" => return Ok("return"),
                "end" | "yield" => return Ok("end"),
                "font" | "window" | "position" | "bind_external" | "delay" => {}
                _ => {
                    return Err(format!("Unsupported control {kind} at {address:#x}").into());
                }
            }
        }
        Ok("return")
    }
}
fn play(exe: &Exe, start: u32, state: State) -> Result<(Vec<Value>, Vec<Value>)> {
    let mut player = Player {
        exe,
        state,
        steps: Vec::new(),
        window: Vec::new(),
        audio: Vec::new(),
    };
    player.run(start, 0)?;
    let window = Value::Array(player.window.clone());
    if player.window.iter().any(|t| t["kind"] == "text")
        && player
            .steps
            .last()
            .is_none_or(|s| s.get("tokens") != Some(&window))
    {
        player
            .steps
            .push(json!({ "kind" : "page", "tokens" : window, "wait" : "input" }));
    }
    let mut merged: Vec<Value> = Vec::new();
    for step in player.steps {
        if let Some(last) = merged.last_mut()
            && step["kind"] == "page"
            && last["kind"] == "page"
            && last["tokens"] == step["tokens"]
        {
            last["wait"] = if last["wait"] == "input" || step["wait"] == "input" {
                json!("input")
            } else {
                json!(last["wait"].as_u64().unwrap() + step["wait"].as_u64().unwrap())
            };
            continue;
        }
        merged.push(step);
    }
    Ok((merged, player.audio))
}
fn b1_state(extra: &[(u64, u64)]) -> State {
    B1_STATE.iter().chain(extra).copied().collect()
}
fn actions(exe: &Exe) -> Result<Map<String, Value>> {
    let used = 0x4b0268;
    let follow: BTreeMap<u64, u32> = [
        (32, 0x4b0308),
        (33, 0x4b032a),
        (34, 0x4b034c),
        (35, 0x4b0370),
        (36, 0x4b03a4),
        (39, 0x4b0400),
    ]
    .into();
    let mut result = Map::new();
    for code in 32..88u64 {
        let state = b1_state(&[(64, code - 31)]);
        let (steps, audio) = play(exe, used, state.clone())?;
        let mut action = json!(
            { "use_va" : used, "follow_va" : follow.get(& code), "steps" : steps, "audio"
            : audio }
        );
        if code == 35 {
            let mut variants = Vec::new();
            for (shown, step) in (0..HEAL_STEPS).map(|s| (5, s)).chain([(9, 0)]) {
                let mut variant = state.clone();
                variant.insert(93, shown);
                variant.insert(79, step);
                variants.push(json!(play(exe, follow[&code], variant)?.0));
            }
            action["follow_variants"] = json!(variants);
        }
        result.insert(code.to_string(), action);
    }
    for code in 88..98u64 {
        let (display, slot, first) = if code < 93 {
            (0x56, 0x1a, 88)
        } else {
            (0x57, 0x1b, 93)
        };
        for (key, value) in [(code.to_string(), code), (format!("{code}_off"), 0)] {
            let state = b1_state(&[(64, code - 31), (display, code - first + 1), (slot, value)]);
            let (steps, audio) = play(exe, used, state)?;
            result.insert(
                key,
                json!(
                    { "use_va" : used, "follow_va" : null, "steps" : steps, "audio" :
                    audio }
                ),
            );
        }
    }
    for (name, address, state) in [
        ("growth", 0x4b091a, b1_state(&[(33, 1)])),
        ("status", 0x4b1376, b1_state(&[])),
    ] {
        let (steps, audio) = play(exe, address, state)?;
        result.insert(
            name.into(),
            json!({ "use_va" : address, "steps" : steps, "audio" : audio }),
        );
    }
    Ok(result)
}
fn texts(exe: &Exe, address: u32) -> Result<Vec<Value>> {
    let message = decode_message(exe, address, true)?;
    Ok(message["tokens"]
        .as_array()
        .ok_or("Tokens")?
        .iter()
        .filter(|t| t["kind"] == "text")
        .map(|t| t["text"].clone())
        .collect())
}
fn joined(exe: &Exe, address: u32) -> Result<String> {
    Ok(texts(exe, address)?
        .iter()
        .filter_map(Value::as_str)
        .collect())
}
fn save_files(exe: &Exe) -> Result<Value> {
    let names = decode_message(exe, 0x4b069a, true)?;
    let dispatch = names["tokens"]
        .as_array()
        .ok_or("Tokens")?
        .iter()
        .find(|t| t["kind"] == "message_dispatch")
        .ok_or("Map name dispatch")?
        .clone();
    let map_names: Vec<String> = dispatch["targets"]
        .as_array()
        .ok_or("Targets")?
        .iter()
        .map(|a| joined(exe, a.as_u64().unwrap() as u32))
        .collect::<Result<_>>()?;
    Ok(json!(
        { "hover_va" : 0x43fd4a, "hover" : texts(exe, 0x43fd4a) ?, "map_names" :
        map_names, "empty" : joined(exe, 0x4b059c) ?, "save_question" : texts(exe,
        0x4b04a8) ?, "quit_question" : texts(exe, 0x4b0462) ?, "load_question" :
        texts(exe, 0x4b0514) ? }
    ))
}
fn rectangles(exe: &Exe, at: u32, count: u32) -> Result<Vec<Vec<i32>>> {
    (0..count)
        .map(|i| {
            Ok(exe
                .read(at + 16 * i, 16)?
                .chunks(4)
                .map(|c| i32::from_le_bytes(c.try_into().unwrap()))
                .collect())
        })
        .collect()
}
const PIN_ANIMATION: u32 = 0x44510c;
fn pin_animation(exe: &Exe) -> Result<(Vec<u16>, u16)> {
    let (mut frames, mut hold, mut at) = (Vec::new(), None, PIN_ANIMATION);
    while exe.read(at, 1)?[0] == 0x21 {
        let raw = exe.read(at, 8)?;
        let word = |i: usize| u16::from_le_bytes([raw[i], raw[i + 1]]);
        if word(6) != 22 || hold.is_some_and(|h| h != word(2)) {
            return Err("Unexpected map pin animation".into());
        }
        hold = Some(word(2));
        frames.push(word(4));
        at += 8;
    }
    let back = u32::from_le_bytes(exe.read(at + 4, 4)?.try_into()?);
    if exe.read(at, 1)?[0] != 0x03 || back != PIN_ANIMATION || frames.is_empty() {
        return Err("Map pin animation does not loop".into());
    }
    Ok((frames, hold.ok_or("Map pin animation")?))
}
fn item_list_drawing(exe: &Exe) -> Result<Value> {
    let entry = |index: u32| -> Result<(u16, u16)> {
        let raw = exe.read(ITEM_ICONS + 4 * index, 4)?;
        Ok((
            u16::from_le_bytes([raw[0], raw[1]]),
            u16::from_le_bytes([raw[2], raw[3]]),
        ))
    };
    let (empty_frame, empty_slot) = entry(0)?;
    let mut icons = Vec::new();
    let mut framed = std::collections::BTreeSet::new();
    for id in 40..98 {
        let (frame, slot) = entry(id - 39)?;
        match slot {
            ITEMS_SLOT => {}
            FRAME_SLOT => {
                framed.insert(frame);
            }
            _ => return Err("Unexpected item icon slot".into()),
        }
        icons.push(json!({ "item_id" : id, "slot" : slot, "frame" : frame }));
    }
    let used: Vec<Value> = framed
        .into_iter()
        .map(|frame| {
            let rectangle = rectangles(exe, FRAME_RECTANGLES + 16 * u32::from(frame), 1)?;
            Ok(json!({ "frame" : frame, "rectangle" : rectangle[0] }))
        })
        .collect::<Result<_>>()?;
    Ok(json!(
        { "table_va" : ITEM_ICONS, "empty" : { "slot" : empty_slot, "frame" :
        empty_frame }, "sheets" : [{ "slot" : ITEMS_SLOT, "file" : "ITEMS.CNS",
        "cell" : [32, 32] }, { "slot" : FRAME_SLOT, "file" : "FRAME.CNS",
        "rectangles_va" : FRAME_RECTANGLES, "used_rectangles" : used }, { "slot" :
        23, "file" : "NUMBER.CNS", "cell" : [8, 8] }], "icons" : icons, "quantity" :
        { "minimum" : 2, "maximum" : 99 } }
    ))
}
fn map_screen(exe: &Exe) -> Result<Value> {
    let recolour = |mut at: u32| -> Result<Vec<Value>> {
        let mut out = Vec::new();
        while exe.read(at, 1)?[0] != 0xff {
            let [index, r, g, b] = exe.read(at, 4)?.try_into()?;
            out.push(json!(
                { "index" : index, "rgb" : [r.wrapping_mul(16), g.wrapping_mul(16), b
                .wrapping_mul(16)] }
            ));
            at += 4;
        }
        Ok(out)
    };
    let (pin_frames, pin_hold) = pin_animation(exe)?;
    let groups: Vec<Vec<Value>> = (0..4)
        .map(|g| recolour(0x4a1108 + 20 * g))
        .collect::<Result<_>>()?;
    Ok(json!(
        { "rectangles" : rectangles(exe, 0x4a0eb8, 12) ?, "markers" : rectangles(exe,
        0x4a0f78, 16) ?, "recolor_groups" : groups, "mapmini_base" : 0xa0,
        "mapmini_key" : 0, "mapparts_base" : 0x50, "mapparts_key" : 14, "surface" :
        [224, 240], "frame_at" : [16, 32], "tabs_at" : [16, 16], "tab_pitch" : 24,
        "digit_at" : [20, 16], "cell_origin" : [48, 64], "cell_size" : 16,
        "pin_origin" : [48, 56], "pin_animation_va" : PIN_ANIMATION, "pin_frames" :
        pin_frames, "pin_hold" : pin_hold, "wall_bits" : [{ "shift" : 0, "set" :
        0x40, "clear" : 0x80 }, { "shift" : 4, "set" : 0x04, "clear" : 0x08 }, {
        "shift" : 8, "set" : 0x10, "clear" : 0x20 }, { "shift" : 12, "set" : 0x01,
        "clear" : 0x02 }] }
    ))
}
pub fn import(originals: &Originals) -> Result<Files> {
    let exe = originals.exe()?;
    let mut files = Files::new();
    let mut sheets = Vec::new();
    for name in SHEETS {
        let sheet = originals.cns(name)?;
        sheets.push(json!(
            { "file" : name, "width" : sheet.width, "height" : sheet.height,
            "pixels_sha256" : sha(& sheet.pixels), "palette_sha256" : sha(& sheet
            .rgb) }
        ));
        files.push((format!("menu/{name}.indices").into(), sheet.pixels));
        files.push((format!("menu/{name}.rgb").into(), sheet.rgb));
    }
    let mut strips = Map::new();
    for (name, table) in STRIPS {
        let count = u16::from_le_bytes(exe.read(table, 2)?.try_into()?) - 1;
        let rows: Vec<Value> = (0..u32::from(count))
            .map(|i| {
                let raw = exe.read(table + 16 * (i + 1), 16)?;
                let w = |k: usize| u16::from_le_bytes([raw[k * 2], raw[k * 2 + 1]]);
                Ok(json!(
                    { "code" : w(0), "left" : w(1), "right" : w(2), "top" : w(3),
                    "bottom" : w(4), "icon" : w(5), "target" :
                    u32::from_le_bytes(raw[12..16].try_into() ?) }
                ))
            })
            .collect::<Result<_>>()?;
        strips.insert(name.into(), json!({ "table_va" : table, "rows" : rows }));
    }
    let mut hover = Map::new();
    let mut codes: Vec<u32> = [1, 2, 3, 4, 5, 6, 7, 8, 31]
        .into_iter()
        .chain(12..21)
        .chain(32..40)
        .collect();
    codes.sort_unstable();
    for code in codes {
        let address = exe.u32(HOVER_TABLE + 4 * code)?;
        let message = decode_message(exe, address, true)?;
        hover.insert(
            code.to_string(),
            json!({ "source_va" : address, "tokens" : message["tokens"] }),
        );
    }
    let menu = json!(
        { "schema" : 1, "source_sha256" : SOURCE, "sheets" : sheets, "strips" : strips,
        "map" : map_screen(exe) ?, "item_list_drawing" : item_list_drawing(exe) ?,
        "save_files" : save_files(exe) ?, "hover_table_va" : HOVER_TABLE, "hover" :
        hover, "actions" : actions(exe) ? }
    );
    let mut bytes = serde_json::to_vec_pretty(&menu)?;
    bytes.push(b'\n');
    files.push(("menu/menu.json".into(), bytes));
    Ok(files)
}
