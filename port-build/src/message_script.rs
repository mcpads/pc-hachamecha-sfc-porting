use crate::{Result, original::Exe};
use serde_json::{Value, json};
const LIMIT: u32 = 65536;
pub fn cp949(bytes: &[u8]) -> Result<String> {
    encoding_rs::EUC_KR
        .decode_without_bom_handling_and_without_replacement(bytes)
        .map(|text| text.into_owned())
        .ok_or_else(|| format!("Text is not CP949: {bytes:02x?}").into())
}
fn bank(exe: &Exe, va: u32) -> Result<String> {
    let name = exe
        .cstring(va, 64)
        .map_err(|_| format!("Unterminated audio bank name at {va:#x}"))?;
    Ok(String::from_utf8(name.to_ascii_uppercase())?)
}
pub fn decode_message(exe: &Exe, start: u32, game_controls: bool) -> Result<Value> {
    let mut tokens = Vec::new();
    let mut cursor = start;
    let u16_at = |at: u32| -> Result<u16> { Ok(u16::from_le_bytes(exe.read(at, 2)?.try_into()?)) };
    while cursor - start < LIMIT {
        if exe.read(cursor, 1)?[0] != b'@' {
            let beginning = cursor;
            let mut text = Vec::new();
            while exe.read(cursor, 1)?[0] != b'@' {
                if cursor - start + 2 > LIMIT {
                    return Err("Message extent exhausted".into());
                }
                text.extend_from_slice(exe.read(cursor, 2)?);
                cursor += 2;
            }
            tokens.push(json!(
                { "kind" : "text", "va" : beginning, "bytes" : text.len(), "text"
                : cp949(& text) ? }
            ));
            continue;
        }
        let header: [u8; 4] = exe.read(cursor, 4)?.try_into()?;
        let opcode = header[1];
        let pointer = |offset: u32| exe.u32(cursor + offset);
        let mut length = 4;
        let mut token = json!({ "va" : cursor, "opcode" : opcode });
        let mut set = |fields: Value| {
            for (k, v) in fields.as_object().unwrap() {
                token[k] = v.clone();
            }
        };
        let g = game_controls;
        match opcode {
            0 => set(json!({ "kind" : "end" })),
            1 if g => {
                length = 8;
                set(json!({ "kind" : "message_jump", "target_va" : pointer(4) ? }));
            }
            2 => set(json!({ "kind" : "newline" })),
            3 => set(json!({ "kind" : "font", "value" : header[2] })),
            4 => set(json!(
                { "kind" : "delay", "value" : u16::from_le_bytes([header[2],
                header[3]]) }
            )),
            5 => set(json!(
                { "kind" : "wait_ticks", "value" : u16::from_le_bytes([header[2],
                header[3]]) }
            )),
            6 => set(json!({ "kind" : "wait_input" })),
            7 => {
                length = 8;
                set(json!(
                    { "kind" : "position", "x" : u16_at(cursor + 2) ?, "y" :
                    u16_at(cursor + 4) ? }
                ));
            }
            8 if header[2] == 0 => set(json!({ "kind" : "palette_color", "value" : header[3] })),
            9 if g => {
                length = 8;
                set(json!({ "kind" : "message_call", "target_va" : pointer(4) ? }));
            }
            10 if g => set(json!({ "kind" : "return" })),
            0x11 if g && header[2] == 3 => {
                let mut assignments = Vec::new();
                let mut done = false;
                for _ in 0..256 {
                    let operation = exe.read(cursor + length, 4)?;
                    if operation[0] == 255 {
                        length += 4;
                        done = true;
                        break;
                    }
                    let width: u32 = match operation[0] {
                        1 => 1,
                        2 => 2,
                        3 => 4,
                        op => {
                            return Err(format!(
                                "Unsupported message assignment {op:#x} at {:#x}",
                                cursor + length
                            )
                            .into());
                        }
                    };
                    let size = if width == 4 { 8 } else { 4 };
                    let raw = exe.read(cursor + length, size as usize)?;
                    let bytes = if width == 4 {
                        &raw[4..8]
                    } else {
                        &raw[2..2 + width as usize]
                    };
                    let value = bytes
                        .iter()
                        .rev()
                        .fold(0u32, |v, &b| (v << 8) | u32::from(b));
                    assignments.push(json!(
                        { "offset" : operation[1], "width" : width, "value" : value
                        }
                    ));
                    length += size;
                }
                if !done {
                    return Err("Message assignment list lacks terminator".into());
                }
                set(json!(
                    { "kind" : "external_assignments", "mode" : 3, "assignments" :
                    assignments }
                ));
            }
            0x12 if g && header[2] == 3 => {
                length = 8;
                let mut conditions = Vec::new();
                let mut done = false;
                for _ in 0..256 {
                    let condition = exe.read(cursor + length, 4)?;
                    length += 4;
                    if condition[0] == 255 {
                        done = true;
                        break;
                    }
                    if ![1, 4, 5].contains(&condition[0]) {
                        return Err(format!(
                            "Unsupported message condition {:#x} at {:#x}",
                            condition[0],
                            cursor + length - 4
                        )
                        .into());
                    }
                    let width = if condition[0] == 5 { 2 } else { 1 };
                    let value = if width == 2 {
                        u32::from(u16::from_le_bytes([condition[2], condition[3]]))
                    } else {
                        u32::from(condition[2])
                    };
                    conditions.push(json!(
                        { "kind" : condition[0], "offset" : condition[1], "value" :
                        value, "width" : width }
                    ));
                }
                if !done {
                    return Err("Message condition list lacks terminator".into());
                }
                set(json!(
                    { "kind" : "message_branch", "mode" : 3, "conditions" :
                    conditions, "targets" : [pointer(4) ?, cursor + length] }
                ));
            }
            0x23 if g => {
                length = 8;
                set(json!(
                    { "kind" : "message_external_bit_branch", "offset" : header[2],
                    "width" : 2, "bit" : header[3], "shift_mask" : 31, "targets" :
                    [pointer(4) ?, cursor + 8] }
                ));
            }
            0x22 if g => {
                length = 8;
                set(json!(
                    { "kind" : "message_global_branch", "global_va" : 0x4c4326,
                    "value" : header[2], "targets" : [pointer(4) ?, cursor + 8] }
                ));
            }
            0x13 | 0x14 if g => {
                let [count, mode] = exe.read(cursor + 4, 2)?.try_into()?;
                if count == 0 || mode > 1 {
                    return Err("Unsupported message dispatch table".into());
                }
                length = if mode == 0 {
                    8 + u32::from(count) * 4
                } else {
                    12
                };
                let table = if mode == 0 { cursor + 8 } else { pointer(8)? };
                let targets: Vec<u32> = (0..u32::from(count))
                    .map(|i| exe.u32(table + i * 4))
                    .collect::<Result<_>>()?;
                set(json!(
                    { "kind" : if opcode == 0x13 { "message_dispatch" } else {
                    "message_dispatch_call" }, "selector_mode" : header[2],
                    "selector_index" : header[3], "table_mode" : mode, "table_va" :
                    table, "targets" : targets }
                ));
                if opcode == 0x14 {
                    set(json!({ "return_va" : cursor + length }));
                }
            }
            0x2f if g && header[2] == 4 => {
                length = 8;
                set(json!(
                    { "kind" : "external_word_number", "offset" : exe.read(cursor +
                    4, 1) ? [0], "width" : 2, "format_argument" : header[3] }
                ));
            }
            11 => set(json!({ "kind" : "clear" })),
            13 => {
                length = 8;
                set(json!({ "kind" : "window", "value" : pointer(4) ? }));
            }
            15 => set(json!({ "kind" : "yield" })),
            0x1a if g && header[2] <= 1 => set(json!(
                { "kind" : "shop_transaction", "mode" : header[2], "handler_va" :
                0x416a89 }
            )),
            0x1b if g => set(json!(
                { "kind" : "bind_external", "mode" : header[2], "index" :
                header[3] }
            )),
            0x20 if g => {
                length = 8;
                set(json!(
                    { "kind" : "external_bit_branch", "offset" : header[2], "mask" :
                    header[3], "targets" : [pointer(4) ?, cursor + 8] }
                ));
            }
            0x21 if g && header[2] <= 1 => {
                set(json!({ "kind" : "save_slot_info", "mode" : header[2] }))
            }
            0x2f if g && header[2] == 3 => {
                length = 8;
                set(json!(
                    { "kind" : "decimal_external_byte", "digits" : header[3],
                    "offset" : exe.read(cursor + 4, 1) ? [0] }
                ));
            }
            0x1e if g => set(json!({ "kind" : "animation_signal", "mask" : header[2] })),
            0x16 if g => {
                length = 8;
                set(json!(
                    { "kind" : "game_call", "group" : header[2], "target_va" :
                    pointer(4) ? }
                ));
            }
            0x2c if g => match header[2] {
                2 => {
                    length = 12;
                    let raw = exe.read(cursor, 12)?;
                    set(json!(
                        { "kind" : "audio_control", "mode" : 2, "channel" : raw[4],
                        "index" : raw[5], "bank" : bank(exe, pointer(8) ?) ? }
                    ));
                }
                0x80 => {
                    length = 8;
                    let raw = exe.read(cursor, 8)?;
                    set(json!(
                        { "kind" : "sound_effect", "channel" : raw[4], "index" :
                        raw[5] }
                    ));
                }
                mode => {
                    return Err(
                        format!("Unsupported message audio mode {mode:#x} at {cursor:#x}").into(),
                    );
                }
            },
            0x2d if g && header[2] == 2 => set(json!({ "kind" : "wait_audio" })),
            0x2d if g && header[2] == 0x82 => {
                length = 8;
                set(json!(
                    { "kind" : "wait_sound_effect", "handle" : i16::from_le_bytes(exe
                    .read(cursor + 4, 2) ?.try_into() ?) }
                ));
            }
            0x2b if g => {
                length = 12;
                let raw = exe.read(cursor, 12)?;
                if raw[8] > 3 {
                    return Err("Unsupported repeated audio count mode".into());
                }
                set(json!(
                    { "kind" : "repeated_audio", "bank" : bank(exe, pointer(4) ?) ?,
                    "channel" : header[2], "index" : header[3], "count_mode" :
                    raw[8], "count_operand" : raw[9], "handler_va" : 0x416f66 }
                ));
            }
            42 => {
                length = 8;
                set(json!(
                    { "kind" : "audio", "bank" : bank(exe, pointer(4) ?) ?, "channel"
                    : header[2], "index" : header[3] }
                ));
            }
            _ => {
                return Err(
                    format!("Unsupported message control {opcode:#x} at {cursor:#x}").into(),
                );
            }
        }
        if cursor - start + length > LIMIT {
            return Err("Message extent exhausted".into());
        }
        exe.read(cursor, length as usize)?;
        token["bytes"] = length.into();
        tokens.push(token);
        cursor += length;
        if opcode == 0 || (g && [1, 10, 0x12, 0x13, 0x20, 0x22, 0x23].contains(&opcode)) {
            return Ok(json!(
                { "source_va" : start, "source_bytes" : cursor - start, "tokens" :
                tokens }
            ));
        }
    }
    Err("Message extent exhausted".into())
}
pub fn continuations(message: &Value) -> Vec<u32> {
    let mut out = Vec::new();
    for token in message["tokens"].as_array().into_iter().flatten() {
        match token["kind"].as_str() {
            Some("message_call" | "message_jump") => {
                out.extend(token["target_va"].as_u64().map(|v| v as u32))
            }
            Some(
                "message_dispatch"
                | "message_dispatch_call"
                | "message_branch"
                | "message_global_branch"
                | "message_external_bit_branch",
            ) => out.extend(
                token["targets"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|t| t.as_u64().map(|v| v as u32)),
            ),
            _ => {}
        }
    }
    out
}
