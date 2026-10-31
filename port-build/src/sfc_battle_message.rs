use crate::Result;
use crate::sfc::{emit, implied};
use crate::sfc_battle::{ACTOR_BYTES, ACTORS, STATS};
use crate::sfc_battle_act::{abs, branch, byte, call, jump, m8, m16, word};
use serde_json::Value;
use sfc_builder::cartridge::{CartridgeLayout, Section};
use std::collections::BTreeMap;
use w65c816::{AddressingMode as A, Assembler, CpuMode, Mnemonic as M, Operand as O};
const END: u8 = 0x00;
const TEXT: u8 = 0x01;
const BREAK: u8 = 0x02;
const CLEAR: u8 = 0x03;
const WAIT_INPUT: u8 = 0x04;
const WAIT: u8 = 0x05;
const YIELD: u8 = 0x06;
const CALL: u8 = 0x07;
const RETURN: u8 = 0x08;
const JUMP: u8 = 0x09;
const BIND: u8 = 0x0a;
const CONDITIONS: u8 = 0x0b;
const BITS: u8 = 0x0c;
const WORD_BIT: u8 = 0x0d;
const GLOBAL: u8 = 0x0e;
const DISPATCH: u8 = 0x0f;
const DISPATCH_CALL: u8 = 0x10;
const NUMBER: u8 = 0x11;
const QUESTION: u8 = 0x12;
const SIGNAL: u8 = 0x13;
const SOUND: u8 = 0x14;
const TRADE: u8 = 0x15;
const MUSIC: u8 = 0x16;
const ASSIGN: u8 = 0x17;
const CHANT: u8 = 0x18;
const QUESTION_SCRIPT: u32 = 0x4be0b8;
const DESCRIPTORS: usize = crate::sfc_battle_scripts::DESCRIPTORS;
const SLOTS: usize = 8;
pub struct Messages {
    bodies: BTreeMap<u32, Vec<Value>>,
    roots: BTreeMap<String, u32>,
}
pub struct Compiled {
    program: Vec<u8>,
    roots: Vec<u8>,
    sources: Vec<u8>,
    numbers: Vec<u8>,
}
fn hex_va(v: &Value) -> Result<u32> {
    Ok(v.as_u64().ok_or("Message address")? as u32)
}
pub fn load(content: &crate::content::Content) -> Result<Messages> {
    let json: Value = content.json("battle-messages.json")?;
    let mut bodies = BTreeMap::new();
    for body in json["bodies"]
        .as_array()
        .ok_or("Missing battle message bodies")?
    {
        let tokens = body["tokens"].as_array().ok_or("Message tokens")?.clone();
        bodies.insert(hex_va(&body["source_va"])?, tokens);
    }
    let mut roots = BTreeMap::new();
    for (name, va) in json["roots"].as_object().ok_or("Missing message roots")? {
        roots.insert(name.clone(), hex_va(va)?);
    }
    let items: Value = content.json("items.json")?;
    for body in items["shop_dialogue"]["messages"]
        .as_array()
        .ok_or("Missing shop messages")?
    {
        let tokens = body["tokens"].as_array().ok_or("Message tokens")?.clone();
        bodies.insert(hex_va(&body["source_va"])?, tokens);
    }
    for va in crate::sfc_shop::load(content)?.message_roots() {
        roots.insert(format!("shop_{va:x}"), va);
    }
    Ok(Messages { bodies, roots })
}
impl Messages {
    pub fn wrap(
        &mut self,
        roots: &[u32],
        width: usize,
        lines: usize,
        advance: &dyn Fn(char) -> Result<usize>,
    ) -> Result<usize> {
        let mut added = 0;
        for root in roots {
            let tokens = self
                .bodies
                .get_mut(root)
                .ok_or_else(|| format!("Message {root:#x} to wrap"))?;
            let mut out = Vec::new();
            let (mut used, mut line) = (0usize, 0usize);
            let text_token = |t: &Value, text: &str| {
                let mut piece = t.clone();
                piece["text"] = Value::from(text);
                piece
            };
            let newline = serde_json::json!({ "kind" : "newline" });
            for t in tokens.drain(..) {
                match t["kind"].as_str() {
                    Some("text") => {
                        let mut piece = String::new();
                        for ch in t["text"].as_str().ok_or("Text")?.chars() {
                            let a = advance(ch)?;
                            if used + a > width && used > 0 {
                                out.push(text_token(&t, &piece));
                                out.push(newline.clone());
                                piece.clear();
                                used = 0;
                                line += 1;
                                added += 1;
                            }
                            piece.push(ch);
                            used += a;
                        }
                        if line >= lines {
                            return Err(
                                format!("Message {root:#x} needs more than {lines} lines").into()
                            );
                        }
                        out.push(text_token(&t, &piece));
                    }
                    Some("newline") => {
                        used = 0;
                        line += 1;
                        out.push(t);
                    }
                    Some("clear") => {
                        used = 0;
                        line = 0;
                        out.push(t);
                    }
                    _ => out.push(t),
                }
            }
            *tokens = out;
        }
        Ok(added)
    }
    pub fn root(&self, name: &str) -> Option<u32> {
        self.roots.get(name).copied()
    }
    pub fn text(&self) -> String {
        let mut text: String = self
            .bodies
            .values()
            .flatten()
            .filter_map(|t| t["text"].as_str())
            .collect();
        text.push_str("０１２３４５６７８９　");
        text
    }
    pub fn fit(&self, glyphs: &crate::sfc_b1_text::Assets) -> Result<Value> {
        use crate::sfc_b1_text::{COLUMNS, LINES, PEN};
        type At = (u32, usize, usize, usize, Vec<(u32, usize)>);
        let digits = "０１２３４５６７８９　";
        let cell = digits
            .chars()
            .filter_map(|ch| glyphs.metrics(ch))
            .fold((0, 0), |a, (adv, reach)| (a.0.max(adv), a.1.max(reach)));
        let mut over_lines = std::collections::BTreeSet::new();
        let mut wrapping = std::collections::BTreeSet::new();
        let mut most_lines = 0;
        for (name, &root) in self.roots.iter().filter(|(n, _)| !n.starts_with("ending_")) {
            let mut seen = std::collections::HashSet::new();
            let mut todo: Vec<At> = vec![(root, 0, 0, PEN[0], Vec::new())];
            let (mut lines, mut wraps) = (0usize, false);
            while let Some((va, i, mut line, mut x, mut stack)) = todo.pop() {
                if !seen.insert((va, i, line, x, stack.clone())) {
                    continue;
                }
                if seen.len() > 200_000 {
                    return Err(format!("Battle message {name} has too many paths").into());
                }
                let tokens = self
                    .bodies
                    .get(&va)
                    .ok_or_else(|| format!("Battle message body {va:#x}"))?;
                let Some(t) = tokens.get(i) else {
                    let last = tokens.last().ok_or("Empty message body")?;
                    let next = hex_va(&last["va"])? + last["bytes"].as_u64().ok_or("Size")? as u32;
                    todo.push((next, 0, line, x, stack));
                    continue;
                };
                let mut draw = |line: &mut usize, x: &mut usize, adv: usize, reach: usize| {
                    if *x + reach > COLUMNS * 8 {
                        [*line, *x] = [*line + 1, PEN[0]];
                        wraps = true;
                    }
                    lines = lines.max(*line + 1);
                    *x += adv;
                };
                let next = (va, i + 1);
                let targets = |key: &str| -> Result<Vec<u32>> {
                    t[key]
                        .as_array()
                        .ok_or("Targets")?
                        .iter()
                        .map(hex_va)
                        .collect()
                };
                match t["kind"].as_str().ok_or("Message token kind")? {
                    "text" => {
                        for ch in t["text"].as_str().ok_or("Text")?.chars() {
                            if let Some((adv, reach)) = glyphs.metrics(ch) {
                                draw(&mut line, &mut x, adv, reach);
                            }
                        }
                    }
                    "newline" => [line, x] = [line + 1, PEN[0]],
                    "clear" => [line, x] = [0, PEN[0]],
                    "external_word_number" => {
                        for _ in 0..t["format_argument"].as_u64().ok_or("Number width")? {
                            draw(&mut line, &mut x, cell.0, cell.1);
                        }
                    }
                    "game_call" => {
                        [line, x] = [line + 1, PEN[0]];
                        for ch in crate::b1_dialogue::CHOICE.chars() {
                            if let Some((adv, reach)) = glyphs.metrics(ch) {
                                draw(&mut line, &mut x, adv, reach);
                            }
                        }
                    }
                    "end" => continue,
                    "return" => {
                        if let Some((to, at)) = stack.pop() {
                            todo.push((to, at, line, x, stack));
                        }
                        continue;
                    }
                    "message_call" => {
                        stack.push(next);
                        todo.push((hex_va(&t["target_va"])?, 0, line, x, stack));
                        continue;
                    }
                    "message_jump" => {
                        todo.push((hex_va(&t["target_va"])?, 0, line, x, stack));
                        continue;
                    }
                    "message_branch"
                    | "external_bit_branch"
                    | "message_external_bit_branch"
                    | "message_global_branch"
                    | "message_dispatch" => {
                        for to in targets("targets")? {
                            todo.push((to, 0, line, x, stack.clone()));
                        }
                        continue;
                    }
                    "message_dispatch_call" => {
                        let mut back = stack.clone();
                        back.push((hex_va(&t["return_va"])?, 0));
                        for to in targets("targets")? {
                            todo.push((to, 0, line, x, back.clone()));
                        }
                        continue;
                    }
                    _ => {}
                }
                todo.push((next.0, next.1, line, x, stack));
            }
            if lines > LINES {
                over_lines.insert(name.clone());
            }
            if wraps {
                wrapping.insert(name.clone());
            }
            most_lines = most_lines.max(lines);
        }
        Ok(serde_json::json!(
            { "most_lines" : most_lines, "lines_shown" : LINES, "roots_over_lines" :
            over_lines, "roots_wrapping" : wrapping, "window_px" : COLUMNS * 8 }
        ))
    }
    pub fn compile(
        &self,
        glyphs: &crate::sfc_b1_text::Assets,
        sound: &crate::sfc_sound::Assets,
    ) -> Result<Compiled> {
        let mut program = Vec::new();
        let mut at: BTreeMap<u32, u16> = BTreeMap::new();
        let mut fixups: Vec<(usize, u32)> = Vec::new();
        let target = |p: &mut Vec<u8>, f: &mut Vec<(usize, u32)>, va: u32| {
            f.push((p.len(), va));
            p.extend([0, 0]);
        };
        for (&va, tokens) in &self.bodies {
            at.insert(va, u16::try_from(program.len())?);
            let mut ended = false;
            for t in tokens {
                let kind = t["kind"].as_str().ok_or("Message token kind")?;
                let num = |key: &str| -> Result<u64> {
                    t[key]
                        .as_u64()
                        .ok_or_else(|| format!("Message {kind} {key}").into())
                };
                match kind {
                    "text" => {
                        let codes = glyphs.encode(t["text"].as_str().ok_or("Text")?)?;
                        for chunk in codes.chunks(255) {
                            program.push(TEXT);
                            program.push(chunk.len() as u8);
                            program.extend(chunk);
                        }
                    }
                    "palette_color" => {
                        program.extend([TEXT, 1, crate::sfc_b1_text::COLOR + num("value")? as u8]);
                    }
                    "newline" => program.push(BREAK),
                    "clear" => program.push(CLEAR),
                    "wait_input" => program.push(WAIT_INPUT),
                    "wait_ticks" => {
                        program.push(WAIT);
                        program.extend((num("value")? as u16).to_le_bytes());
                    }
                    "yield" => program.push(YIELD),
                    "end" => {
                        program.push(END);
                        ended = true;
                    }
                    "return" => {
                        program.push(RETURN);
                        ended = true;
                    }
                    "message_call" => {
                        program.push(CALL);
                        target(&mut program, &mut fixups, hex_va(&t["target_va"])?);
                    }
                    "message_jump" => {
                        program.push(JUMP);
                        target(&mut program, &mut fixups, hex_va(&t["target_va"])?);
                        ended = true;
                    }
                    "external_assignments" => {
                        for a in t["assignments"].as_array().ok_or("Assignments")? {
                            let field = |k: &str| a[k].as_u64().ok_or("Assignment field");
                            if field("width")? != 1 {
                                return Err("Battle message assignment wider than a byte".into());
                            }
                            program.extend([ASSIGN, field("offset")? as u8, field("value")? as u8]);
                        }
                    }
                    "bind_external" => {
                        program.extend([BIND, num("mode")? as u8, num("index")? as u8]);
                    }
                    "message_branch" => {
                        let conditions = t["conditions"].as_array().ok_or("Conditions")?;
                        program.extend([CONDITIONS, conditions.len() as u8]);
                        for c in conditions {
                            let value = c["value"].as_u64().ok_or("Condition value")? as u16;
                            let kind = c["kind"].as_u64().ok_or("Condition kind")? as u8;
                            if !matches!(kind, 1 | 4 | 5) {
                                return Err(format!("Message condition kind {kind}").into());
                            }
                            program.extend([kind, c["offset"].as_u64().ok_or("Offset")? as u8]);
                            program.extend(value.to_le_bytes());
                        }
                        branch_pair(&mut program, &mut fixups, t, &target)?;
                        ended = true;
                    }
                    "external_bit_branch" => {
                        program.extend([BITS, num("offset")? as u8, num("mask")? as u8]);
                        branch_pair(&mut program, &mut fixups, t, &target)?;
                        ended = true;
                    }
                    "message_external_bit_branch" => {
                        program.extend([WORD_BIT, num("offset")? as u8, num("bit")? as u8 & 31]);
                        branch_pair(&mut program, &mut fixups, t, &target)?;
                        ended = true;
                    }
                    "message_global_branch" => {
                        if num("global_va")? != 0x4c4326 {
                            return Err("Unknown message global".into());
                        }
                        program.extend([GLOBAL, num("value")? as u8]);
                        branch_pair(&mut program, &mut fixups, t, &target)?;
                        ended = true;
                    }
                    "message_dispatch" | "message_dispatch_call" => {
                        if num("selector_mode")? != 3 {
                            return Err("Message dispatch other than an external byte".into());
                        }
                        let targets = t["targets"].as_array().ok_or("Dispatch targets")?;
                        let call = kind == "message_dispatch_call";
                        program.extend([
                            if call { DISPATCH_CALL } else { DISPATCH },
                            num("selector_index")? as u8,
                            targets.len() as u8,
                        ]);
                        for v in targets {
                            target(&mut program, &mut fixups, hex_va(v)?);
                        }
                        if call {
                            target(&mut program, &mut fixups, hex_va(&t["return_va"])?);
                        }
                        ended = true;
                    }
                    "external_word_number" => {
                        program.extend([
                            NUMBER,
                            num("offset")? as u8,
                            num("format_argument")? as u8,
                        ]);
                    }
                    "game_call" => {
                        if hex_va(&t["target_va"])? != QUESTION_SCRIPT {
                            return Err("Message game call other than the question".into());
                        }
                        program.push(BREAK);
                        let choice = glyphs.encode(crate::b1_dialogue::CHOICE)?;
                        program.extend([TEXT, choice.len() as u8]);
                        program.extend(choice);
                        program.push(QUESTION);
                    }
                    "animation_signal" => program.extend([SIGNAL, num("mask")? as u8]),
                    "shop_transaction" => program.extend([TRADE, num("mode")? as u8]),
                    "audio" | "repeated_audio" | "sound_effect" => {
                        let index = num("index")?;
                        let code = match t["bank"].as_str() {
                            Some("VOICE.WLK") => sound.voice_code(index),
                            Some("PCMDATA.WLK") | None => sound.effect_code(index),
                            Some(other) => {
                                return Err(format!("Message audio bank {other}").into());
                            }
                        };
                        if let Some(code) = code {
                            if kind == "repeated_audio" {
                                if num("count_mode")? != 3 {
                                    return Err("Unsupported chant count binding".into());
                                }
                                program.extend([CHANT, code, u8::try_from(num("count_operand")?)?]);
                            } else {
                                program.extend([SOUND, code]);
                            }
                        }
                    }
                    "audio_control" if t["bank"] == "MIDDATA.MLK" => {
                        let index = num("index")?;
                        if crate::sfc_sound::source_song(index).is_none() {
                            return Err(format!("Unported message song {index}").into());
                        }
                        program.extend([MUSIC, u8::try_from(index)?]);
                    }
                    "font" | "window" | "position" | "delay" | "audio_control" | "wait_audio"
                    | "wait_sound_effect" => {}
                    other => return Err(format!("Battle message command {other}").into()),
                }
            }
            if !ended {
                let last = tokens.last().ok_or("Empty message body")?;
                let next =
                    hex_va(&last["va"])? + last["bytes"].as_u64().ok_or("Token size")? as u32;
                program.push(JUMP);
                target(&mut program, &mut fixups, next);
            }
        }
        for (offset, va) in fixups {
            let to = *at
                .get(&va)
                .ok_or_else(|| format!("Battle message target {va:#x} not extracted"))?;
            program[offset..offset + 2].copy_from_slice(&to.to_le_bytes());
        }
        let mut roots = vec![0xffu8; DESCRIPTORS * SLOTS * 2];
        let mut sources = Vec::new();
        for (name, va) in &self.roots {
            let to = at[va];
            if let Some((id, slot)) = name.split_once(':') {
                let (id, slot): (usize, usize) = (id.parse()?, slot.parse()?);
                let i = (id * SLOTS + slot) * 2;
                roots[i..i + 2].copy_from_slice(&to.to_le_bytes());
            } else {
                sources.extend(va.to_le_bytes());
                sources.extend(to.to_le_bytes());
            }
        }
        let mut numbers = Vec::new();
        for ch in "０１２３４５６７８９　".chars() {
            let codes = glyphs.encode(&ch.to_string())?;
            if codes.is_empty() || codes.len() > 2 {
                return Err(format!("Number glyph {ch} has no code").into());
            }
            numbers.push(codes.len() as u8);
            numbers.extend(&codes);
            numbers.resize(numbers.len().next_multiple_of(3), 0);
        }
        Ok(Compiled {
            program,
            roots,
            sources,
            numbers,
        })
    }
}
fn branch_pair(
    program: &mut Vec<u8>,
    fixups: &mut Vec<(usize, u32)>,
    t: &Value,
    target: &impl Fn(&mut Vec<u8>, &mut Vec<(usize, u32)>, u32),
) -> Result<()> {
    let targets = t["targets"].as_array().ok_or("Branch targets")?;
    if targets.len() != 2 {
        return Err("Branch without two targets".into());
    }
    for v in targets {
        target(program, fixups, hex_va(v)?);
    }
    Ok(())
}
pub fn sections(
    sections: &mut Vec<Section>,
    space: &mut crate::sfc_rom::Space,
    compiled: &Compiled,
) -> Result<()> {
    crate::sfc_b1::add(
        sections,
        space,
        "battle_messages".into(),
        compiled.program.clone(),
    )?;
    crate::sfc_b1::add(
        sections,
        space,
        "battle_message_roots".into(),
        compiled.roots.clone(),
    )?;
    let mut sources = compiled.sources.clone();
    sources.extend([0; 6]);
    crate::sfc_b1::add(sections, space, "battle_message_sources".into(), sources)?;
    crate::sfc_b1::add(
        sections,
        space,
        "battle_number_glyphs".into(),
        compiled.numbers.clone(),
    )?;
    Ok(())
}
const MPC: u16 = 0xd20;
const MSTACK: u16 = 0xd22;
const MDEPTH: u16 = 0xd32;
const BOUND: u16 = 0xd34;
const WAITING: u16 = 0xd36;
const TICKS: u16 = 0xd37;
const LENGTH: u16 = 0xd39;
const DIRTY: u16 = 0xd3b;
pub(crate) const STARTED: u16 = 0xd3c;
const MWORK: u16 = 0xd3d;
pub const INPUT: u16 = 0xd3f;
pub const DRAW: u16 = 0xd40;
pub const SOUND_REQUEST: u16 = 0xd4a;
pub const SOUND_STOP: u16 = 0xd4b;
pub const BODY_REQUEST: u16 = 0xd4c;
pub const SOUND_REPEATS: u16 = 0xd4d;
pub const SOUND_WAIT: u16 = 0xd4e;
const FIELD_SHARED: u16 = 0x8c0;
const GAME_BLOCK: u16 = 0x970;
const WAIT_TICKS: u8 = 1;
const WAIT_KEY: u8 = 2;
const WAIT_ANSWER: u8 = 3;
const WAIT_FRAME: u8 = 4;
const WAIT_SOUND: u8 = 5;
const PAGE: u32 = crate::sfc_b1_text::RUNTIME_CODES;
fn program(p: &mut Assembler, m: M, base: u32, offset: u32) {
    emit(p, m, A::AbsoluteLongX, O::Long(base + offset));
}
fn advance(p: &mut Assembler, n: u16) {
    m16(p);
    abs(p, M::Lda, MPC);
    implied(p, M::Clc);
    word(p, M::Adc, n);
    abs(p, M::Sta, MPC);
    m8(p);
    jump(p, "battle_message_step");
}
fn append(p: &mut Assembler) {
    call(p, "battle_message_append");
}
pub fn emit_rules(p: &mut Assembler, layout: &CartridgeLayout) -> Result<()> {
    let base = layout.cpu_address(layout.section("battle_messages")?.offset)?;
    let roots = layout.cpu_address(layout.section("battle_message_roots")?.offset)?;
    let sources = layout.cpu_address(layout.section("battle_message_sources")?.offset)?;
    let numbers = layout.cpu_address(layout.section("battle_number_glyphs")?.offset)?;
    use crate::sfc_battle_vm::{
        KIND_DESCRIPTOR, MESSAGE, MESSAGE_KIND, MESSAGE_SLOT, MESSAGE_SOURCE, MESSAGE_WHO,
    };
    p.label("battle_message_tick")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, MESSAGE);
    branch(p, M::Bne, "battle_message_requested");
    abs(p, M::Stz, STARTED);
    implied(p, M::Rts);
    p.label("battle_message_requested");
    abs(p, M::Lda, STARTED);
    branch(p, M::Beq, "battle_message_start");
    jump(p, "battle_message_waits");
    p.label("battle_message_start");
    byte(p, M::Lda, 1);
    abs(p, M::Sta, STARTED);
    m16(p);
    abs(p, M::Stz, MDEPTH);
    abs(p, M::Stz, LENGTH);
    word(p, M::Lda, FIELD_SHARED);
    abs(p, M::Sta, BOUND);
    m8(p);
    abs(p, M::Stz, WAITING);
    abs(p, M::Lda, MESSAGE_KIND);
    emit(p, M::Cmp, A::Immediate, O::Byte(KIND_DESCRIPTOR));
    branch(p, M::Bne, "battle_message_source");
    abs(p, M::Lda, MESSAGE_WHO);
    emit(p, M::Inc, A::Accumulator, O::None);
    emit(p, M::Cmp, A::Immediate, O::Byte(3));
    branch(p, M::Bcc, "battle_message_descriptor");
    abs(p, M::Lda, crate::sfc_encounter::ENEMY);
    p.label("battle_message_descriptor");
    m16(p);
    word(p, M::And, 0xff);
    for _ in 0..3 {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    abs(p, M::Sta, MWORK);
    abs(p, M::Lda, MESSAGE_SLOT);
    word(p, M::And, 7);
    implied(p, M::Clc);
    abs(p, M::Adc, MWORK);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Tax);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(roots));
    jump(p, "battle_message_root");
    p.label("battle_message_source")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    emit(p, M::Ldx, A::Immediate, O::Word(0));
    p.label("battle_message_source_row");
    m16(p);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(sources));
    branch(p, M::Beq, "battle_message_unknown");
    abs(p, M::Cmp, MESSAGE_SOURCE);
    branch(p, M::Bne, "battle_message_source_next");
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(sources + 2));
    abs(p, M::Cmp, MESSAGE_SOURCE + 2);
    branch(p, M::Bne, "battle_message_source_next");
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(sources + 4));
    jump(p, "battle_message_root");
    p.label("battle_message_source_next")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    implied(p, M::Txa);
    implied(p, M::Clc);
    word(p, M::Adc, 6);
    implied(p, M::Tax);
    branch(p, M::Bra, "battle_message_source_row");
    p.label("battle_message_unknown")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    word(p, M::Lda, 0xffff);
    p.label("battle_message_root")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    abs(p, M::Sta, MPC);
    word(p, M::Cmp, 0xffff);
    m8(p);
    branch(p, M::Beq, "battle_message_none");
    jump(p, "battle_message_waits");
    p.label("battle_message_none");
    jump(p, "battle_message_done");
    p.label("battle_message_waits")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, WAITING);
    branch(p, M::Bne, "battle_message_waiting");
    jump(p, "battle_message_step");
    p.label("battle_message_waiting");
    emit(p, M::Cmp, A::Immediate, O::Byte(WAIT_KEY));
    branch(p, M::Bne, "battle_message_not_key");
    abs(p, M::Lda, INPUT);
    byte(p, M::And, 1);
    branch(p, M::Bne, "battle_message_resume");
    implied(p, M::Rts);
    p.label("battle_message_not_key");
    emit(p, M::Cmp, A::Immediate, O::Byte(WAIT_ANSWER));
    branch(p, M::Bne, "battle_message_not_answer");
    abs(p, M::Lda, INPUT);
    byte(p, M::And, 3);
    branch(p, M::Bne, "battle_message_answered");
    implied(p, M::Rts);
    p.label("battle_message_answered");
    byte(p, M::And, 1);
    abs(p, M::Sta, FIELD_SHARED + 0x1d);
    let answer = |n| -> Result<u8> { Ok(crate::sfc_sound::effect(layout, n)?.unwrap_or(0)) };
    branch(p, M::Beq, "battle_message_answered_no");
    byte(p, M::Lda, answer(0)?);
    branch(p, M::Bra, "battle_message_answered_sound");
    p.label("battle_message_answered_no");
    byte(p, M::Lda, answer(1)?);
    p.label("battle_message_answered_sound");
    abs(p, M::Sta, SOUND_REQUEST);
    branch(p, M::Bra, "battle_message_resume");
    p.label("battle_message_not_answer");
    byte(p, M::Cmp, WAIT_SOUND);
    branch(p, M::Bne, "battle_message_not_sound");
    abs(p, M::Lda, SOUND_WAIT);
    branch(p, M::Beq, "battle_message_resume");
    implied(p, M::Rts);
    p.label("battle_message_not_sound");
    emit(p, M::Cmp, A::Immediate, O::Byte(WAIT_FRAME));
    branch(p, M::Beq, "battle_message_resume");
    abs(p, M::Lda, TICKS + 1);
    branch(p, M::Bpl, "battle_message_count");
    abs(p, M::Lda, INPUT);
    byte(p, M::And, 1);
    branch(p, M::Bne, "battle_message_resume");
    p.label("battle_message_count");
    m16(p);
    abs(p, M::Lda, TICKS);
    word(p, M::And, 0x7fff);
    branch(p, M::Beq, "battle_message_counted");
    abs(p, M::Dec, TICKS);
    abs(p, M::Lda, TICKS);
    word(p, M::And, 0x7fff);
    branch(p, M::Beq, "battle_message_counted");
    m8(p);
    implied(p, M::Rts);
    p.label("battle_message_counted")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    m8(p);
    p.label("battle_message_resume");
    abs(p, M::Stz, WAITING);
    p.label("battle_message_step")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    emit(p, M::Ldx, A::Absolute, O::Word(MPC));
    program(p, M::Lda, base, 0);
    let handlers: &[(u8, &str)] = &[
        (END, "bm_end"),
        (TEXT, "bm_text"),
        (BREAK, "bm_break"),
        (CLEAR, "bm_clear"),
        (WAIT_INPUT, "bm_wait_input"),
        (WAIT, "bm_wait"),
        (YIELD, "bm_yield"),
        (CALL, "bm_call"),
        (RETURN, "bm_return"),
        (JUMP, "bm_jump"),
        (BIND, "bm_bind"),
        (CONDITIONS, "bm_conditions"),
        (BITS, "bm_bits"),
        (WORD_BIT, "bm_word_bit"),
        (GLOBAL, "bm_global"),
        (DISPATCH, "bm_dispatch"),
        (DISPATCH_CALL, "bm_dispatch_call"),
        (NUMBER, "bm_number"),
        (QUESTION, "bm_question"),
        (SIGNAL, "bm_signal"),
        (SOUND, "bm_sound"),
        (TRADE, "bm_trade"),
        (MUSIC, "bm_music"),
        (ASSIGN, "bm_assign"),
        (CHANT, "bm_chant"),
    ];
    for (i, (op, handler)) in handlers.iter().enumerate() {
        let skip = format!("bm_not_{i}");
        emit(p, M::Cmp, A::Immediate, O::Byte(*op));
        branch(p, M::Bne, &skip);
        jump(p, handler);
        p.label(skip);
    }
    jump(p, "bm_end");
    p.label("bm_end").assume_mode(CpuMode::NATIVE_M8_X16);
    p.label("battle_message_done");
    call(p, "battle_message_show");
    abs(p, M::Stz, MESSAGE);
    abs(p, M::Stz, STARTED);
    implied(p, M::Rts);
    p.label("bm_text").assume_mode(CpuMode::NATIVE_M8_X16);
    program(p, M::Lda, base, 1);
    abs(p, M::Sta, MWORK);
    m16(p);
    abs(p, M::Lda, MPC);
    implied(p, M::Clc);
    word(p, M::Adc, 2);
    abs(p, M::Sta, MPC);
    m8(p);
    p.label("bm_text_byte");
    abs(p, M::Lda, MWORK);
    branch(p, M::Beq, "bm_text_done");
    emit(p, M::Ldx, A::Absolute, O::Word(MPC));
    program(p, M::Lda, base, 0);
    append(p);
    m16(p);
    abs(p, M::Inc, MPC);
    m8(p);
    abs(p, M::Dec, MWORK);
    branch(p, M::Bra, "bm_text_byte");
    p.label("bm_text_done");
    jump(p, "battle_message_step");
    p.label("bm_break").assume_mode(CpuMode::NATIVE_M8_X16);
    byte(p, M::Lda, crate::sfc_b1_text::BREAK);
    append(p);
    advance(p, 1);
    p.label("bm_clear").assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    abs(p, M::Stz, LENGTH);
    m8(p);
    byte(p, M::Lda, 1);
    abs(p, M::Sta, DIRTY);
    advance(p, 1);
    p.label("bm_wait_input").assume_mode(CpuMode::NATIVE_M8_X16);
    byte(p, M::Lda, WAIT_KEY);
    jump(p, "bm_hold1");
    p.label("bm_yield").assume_mode(CpuMode::NATIVE_M8_X16);
    byte(p, M::Lda, WAIT_FRAME);
    p.label("bm_hold1");
    abs(p, M::Sta, WAITING);
    m16(p);
    abs(p, M::Inc, MPC);
    m8(p);
    jump(p, "battle_message_show");
    p.label("bm_wait").assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    program(p, M::Lda, base, 1);
    abs(p, M::Sta, TICKS);
    abs(p, M::Lda, MPC);
    implied(p, M::Clc);
    word(p, M::Adc, 3);
    abs(p, M::Sta, MPC);
    m8(p);
    byte(p, M::Lda, WAIT_TICKS);
    abs(p, M::Sta, WAITING);
    jump(p, "battle_message_show");
    p.label("bm_question").assume_mode(CpuMode::NATIVE_M8_X16);
    byte(p, M::Lda, WAIT_ANSWER);
    jump(p, "bm_hold1");
    p.label("bm_call").assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    abs(p, M::Lda, MPC);
    implied(p, M::Clc);
    word(p, M::Adc, 3);
    call(p, "bm_push");
    emit(p, M::Ldx, A::Absolute, O::Word(MPC));
    jump(p, "bm_jump_m16");
    p.label("bm_jump").assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    p.label("bm_jump_m16").assume_mode(CpuMode::NATIVE_M16_X16);
    program(p, M::Lda, base, 1);
    abs(p, M::Sta, MPC);
    m8(p);
    jump(p, "battle_message_step");
    p.label("bm_return").assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    abs(p, M::Lda, MDEPTH);
    branch(p, M::Bne, "bm_return_pop");
    m8(p);
    jump(p, "bm_end");
    p.label("bm_return_pop")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    emit(p, M::Dec, A::Accumulator, O::None);
    emit(p, M::Dec, A::Accumulator, O::None);
    abs(p, M::Sta, MDEPTH);
    implied(p, M::Tax);
    emit(p, M::Lda, A::AbsoluteX, O::Word(MSTACK));
    abs(p, M::Sta, MPC);
    m8(p);
    jump(p, "battle_message_step");
    p.label("bm_push").assume_mode(CpuMode::NATIVE_M16_X16);
    emit(p, M::Ldx, A::Absolute, O::Word(MDEPTH));
    emit(p, M::Sta, A::AbsoluteX, O::Word(MSTACK));
    implied(p, M::Inx);
    implied(p, M::Inx);
    implied(p, M::Txa);
    word(p, M::And, 0x0f);
    abs(p, M::Sta, MDEPTH);
    implied(p, M::Rts);
    p.label("bm_bind").assume_mode(CpuMode::NATIVE_M8_X16);
    program(p, M::Lda, base, 2);
    abs(p, M::Sta, MWORK);
    abs(p, M::Stz, MWORK + 1);
    program(p, M::Lda, base, 1);
    m16(p);
    word(p, M::And, 0xff);
    word(p, M::Cmp, 4);
    branch(p, M::Bcs, "bm_bind_done");
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Tay);
    emit(p, M::Lda, A::Immediate, O::Word(0));
    for (mode, base_at, size) in [
        (0u16, FIELD_SHARED, 0u16),
        (2, GAME_BLOCK, 0),
        (4, STATS, 48),
        (6, ACTORS, ACTOR_BYTES),
    ] {
        let next = format!("bm_bind_not_{mode}");
        emit(p, M::Cpy, A::Immediate, O::Word(mode));
        branch(p, M::Bne, &next);
        word(p, M::Lda, base_at);
        p.label(format!("bm_bind_add_{mode}"));
        emit(p, M::Ldx, A::Absolute, O::Word(MWORK));
        p.label(format!("bm_bind_index_{mode}"));
        emit(p, M::Cpx, A::Immediate, O::Word(0));
        branch(p, M::Beq, "bm_bind_store");
        implied(p, M::Clc);
        word(p, M::Adc, size);
        implied(p, M::Dex);
        branch(p, M::Bra, &format!("bm_bind_index_{mode}"));
        p.label(next);
    }
    p.label("bm_bind_store");
    abs(p, M::Sta, BOUND);
    p.label("bm_bind_done");
    m8(p);
    advance(p, 3);
    p.label("bm_assign").assume_mode(CpuMode::NATIVE_M8_X16);
    program(p, M::Lda, base, 1);
    abs(p, M::Sta, MWORK);
    abs(p, M::Stz, MWORK + 1);
    m16(p);
    abs(p, M::Lda, BOUND);
    implied(p, M::Clc);
    abs(p, M::Adc, MWORK);
    implied(p, M::Tay);
    m8(p);
    program(p, M::Lda, base, 2);
    emit(p, M::Sta, A::AbsoluteY, O::Word(0));
    advance(p, 3);
    p.label("bm_conditions").assume_mode(CpuMode::NATIVE_M8_X16);
    program(p, M::Lda, base, 1);
    abs(p, M::Sta, MWORK);
    m16(p);
    abs(p, M::Inc, MPC);
    abs(p, M::Inc, MPC);
    m8(p);
    abs(p, M::Stz, MWORK + 1);
    p.label("bm_condition");
    abs(p, M::Lda, MWORK);
    branch(p, M::Bne, "bm_condition_check");
    jump(p, "bm_conditions_end");
    p.label("bm_condition_check");
    emit(p, M::Ldx, A::Absolute, O::Word(MPC));
    program(p, M::Lda, base, 1);
    call(p, "bm_bound_at");
    program(p, M::Lda, base, 0);
    emit(p, M::Cmp, A::Immediate, O::Byte(5));
    branch(p, M::Beq, "bm_condition_word");
    emit(p, M::Lda, A::AbsoluteY, O::Word(0));
    program(p, M::Cmp, base, 2);
    branch(p, M::Beq, "bm_condition_equal");
    program(p, M::Lda, base, 0);
    emit(p, M::Cmp, A::Immediate, O::Byte(1));
    branch(p, M::Beq, "bm_condition_next");
    branch(p, M::Bra, "bm_condition_fail");
    p.label("bm_condition_equal");
    program(p, M::Lda, base, 0);
    emit(p, M::Cmp, A::Immediate, O::Byte(4));
    branch(p, M::Beq, "bm_condition_next");
    branch(p, M::Bra, "bm_condition_fail");
    p.label("bm_condition_word");
    m16(p);
    emit(p, M::Lda, A::AbsoluteY, O::Word(0));
    program(p, M::Cmp, base, 2);
    m8(p);
    branch(p, M::Beq, "bm_condition_next");
    p.label("bm_condition_fail");
    byte(p, M::Lda, 1);
    abs(p, M::Sta, MWORK + 1);
    p.label("bm_condition_next");
    m16(p);
    abs(p, M::Lda, MPC);
    implied(p, M::Clc);
    word(p, M::Adc, 4);
    abs(p, M::Sta, MPC);
    m8(p);
    abs(p, M::Dec, MWORK);
    jump(p, "bm_condition");
    p.label("bm_conditions_end");
    emit(p, M::Ldx, A::Absolute, O::Word(MPC));
    abs(p, M::Lda, MWORK + 1);
    jump(p, "bm_take_if_zero");
    p.label("bm_take_if_zero")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    branch(p, M::Beq, "bm_take_first");
    m16(p);
    program(p, M::Lda, base, 2);
    abs(p, M::Sta, MPC);
    m8(p);
    jump(p, "battle_message_step");
    p.label("bm_take_first");
    m16(p);
    program(p, M::Lda, base, 0);
    abs(p, M::Sta, MPC);
    m8(p);
    jump(p, "battle_message_step");
    p.label("bm_bound_at").assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    word(p, M::And, 0xff);
    implied(p, M::Clc);
    abs(p, M::Adc, BOUND);
    implied(p, M::Tay);
    m8(p);
    implied(p, M::Rts);
    for (label, word_bit) in [("bm_bits", false), ("bm_word_bit", true)] {
        p.label(label).assume_mode(CpuMode::NATIVE_M8_X16);
        program(p, M::Lda, base, 1);
        call(p, "bm_bound_at");
        abs(p, M::Stz, MWORK + 1);
        if word_bit {
            program(p, M::Lda, base, 2);
            m16(p);
            word(p, M::And, 0x1f);
            implied(p, M::Tax);
            word(p, M::Lda, 1);
            p.label("bm_word_bit_shift");
            emit(p, M::Cpx, A::Immediate, O::Word(0));
            branch(p, M::Beq, "bm_word_bit_mask");
            emit(p, M::Asl, A::Accumulator, O::None);
            implied(p, M::Dex);
            branch(p, M::Bra, "bm_word_bit_shift");
            p.label("bm_word_bit_mask");
            emit(p, M::And, A::AbsoluteY, O::Word(0));
            m8(p);
        } else {
            emit(p, M::Lda, A::AbsoluteY, O::Word(0));
            program(p, M::And, base, 2);
        }
        branch(p, M::Bne, &format!("{label}_set"));
        abs(p, M::Inc, MWORK + 1);
        p.label(format!("{label}_set"));
        m16(p);
        abs(p, M::Lda, MPC);
        implied(p, M::Clc);
        word(p, M::Adc, 3);
        abs(p, M::Sta, MPC);
        implied(p, M::Tax);
        m8(p);
        abs(p, M::Lda, MWORK + 1);
        jump(p, "bm_take_if_zero");
    }
    p.label("bm_global").assume_mode(CpuMode::NATIVE_M8_X16);
    program(p, M::Lda, base, 1);
    abs(p, M::Sta, MWORK);
    m16(p);
    abs(p, M::Lda, MPC);
    implied(p, M::Clc);
    word(p, M::Adc, 2);
    abs(p, M::Sta, MPC);
    implied(p, M::Tax);
    m8(p);
    abs(p, M::Lda, crate::sfc_battle_field::ACTIVE);
    branch(p, M::Beq, "bm_global_field");
    abs(p, M::Lda, MWORK);
    implied(p, M::Sec);
    byte(p, M::Sbc, 2);
    jump(p, "bm_take_if_zero");
    p.label("bm_global_field");
    abs(p, M::Lda, MWORK);
    implied(p, M::Sec);
    byte(p, M::Sbc, 1);
    jump(p, "bm_take_if_zero");
    for (label, call_form) in [("bm_dispatch", false), ("bm_dispatch_call", true)] {
        p.label(label).assume_mode(CpuMode::NATIVE_M8_X16);
        if call_form {
            program(p, M::Lda, base, 2);
            m16(p);
            word(p, M::And, 0xff);
            emit(p, M::Asl, A::Accumulator, O::None);
            implied(p, M::Clc);
            abs(p, M::Adc, MPC);
            implied(p, M::Tax);
            program(p, M::Lda, base, 3);
            call(p, "bm_push");
            emit(p, M::Ldx, A::Absolute, O::Word(MPC));
            m8(p);
        }
        program(p, M::Lda, base, 1);
        call(p, "bm_bound_at");
        emit(p, M::Lda, A::AbsoluteY, O::Word(0));
        program(p, M::Cmp, base, 2);
        branch(p, M::Bcc, &format!("{label}_in"));
        program(p, M::Lda, base, 2);
        emit(p, M::Dec, A::Accumulator, O::None);
        p.label(format!("{label}_in"));
        m16(p);
        word(p, M::And, 0xff);
        emit(p, M::Asl, A::Accumulator, O::None);
        implied(p, M::Clc);
        abs(p, M::Adc, MPC);
        implied(p, M::Tax);
        program(p, M::Lda, base, 3);
        abs(p, M::Sta, MPC);
        m8(p);
        jump(p, "battle_message_step");
    }
    p.label("bm_number").assume_mode(CpuMode::NATIVE_M8_X16);
    program(p, M::Lda, base, 2);
    abs(p, M::Sta, MWORK + 1);
    program(p, M::Lda, base, 1);
    call(p, "bm_bound_at");
    m16(p);
    emit(p, M::Lda, A::AbsoluteY, O::Word(0));
    abs(p, M::Sta, crate::sfc_battle::DIVIDEND);
    m8(p);
    emit(p, M::Ldx, A::Immediate, O::Word(0));
    p.label("bm_number_blank");
    byte(p, M::Lda, 0xff);
    emit(p, M::Sta, A::AbsoluteX, O::Word(NUMBER_CELLS));
    implied(p, M::Inx);
    emit(p, M::Cpx, A::Immediate, O::Word(8));
    branch(p, M::Bne, "bm_number_blank");
    emit(p, M::Ldx, A::Immediate, O::Word(7));
    p.label("bm_number_digit");
    m16(p);
    word(p, M::Lda, 10);
    abs(p, M::Sta, crate::sfc_battle::DIVISOR);
    m8(p);
    call(p, "battle_divide");
    abs(p, M::Lda, crate::sfc_battle::DIVIDEND);
    emit(p, M::Sta, A::AbsoluteX, O::Word(NUMBER_CELLS));
    m16(p);
    abs(p, M::Lda, crate::sfc_battle::QUOTIENT);
    abs(p, M::Sta, crate::sfc_battle::DIVIDEND);
    m8(p);
    implied(p, M::Dex);
    branch(p, M::Beq, "bm_number_cells");
    m16(p);
    abs(p, M::Lda, crate::sfc_battle::DIVIDEND);
    m8(p);
    branch(p, M::Bne, "bm_number_digit");
    p.label("bm_number_cells");
    byte(p, M::Lda, 8);
    implied(p, M::Sec);
    abs(p, M::Sbc, MWORK + 1);
    m16(p);
    word(p, M::And, 0xff);
    implied(p, M::Tax);
    m8(p);
    p.label("bm_number_out");
    emit(p, M::Cpx, A::Immediate, O::Word(8));
    branch(p, M::Bcs, "bm_number_done");
    emit(p, M::Lda, A::AbsoluteX, O::Word(NUMBER_CELLS));
    implied(p, M::Phx);
    call(p, "bm_number_glyph");
    implied(p, M::Plx);
    implied(p, M::Inx);
    branch(p, M::Bra, "bm_number_out");
    p.label("bm_number_done");
    advance(p, 3);
    p.label("bm_signal").assume_mode(CpuMode::NATIVE_M8_X16);
    byte(p, M::Lda, 0x54);
    call(p, "bm_bound_at");
    program(p, M::Lda, base, 1);
    branch(p, M::Beq, "bm_signal_store");
    emit(p, M::Ora, A::AbsoluteY, O::Word(0));
    p.label("bm_signal_store");
    emit(p, M::Sta, A::AbsoluteY, O::Word(0));
    advance(p, 2);
    p.label("bm_sound").assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Stz, SOUND_REPEATS);
    program(p, M::Lda, base, 1);
    abs(p, M::Sta, SOUND_REQUEST);
    advance(p, 2);
    p.label("bm_chant").assume_mode(CpuMode::NATIVE_M8_X16);
    program(p, M::Lda, base, 2);
    call(p, "bm_bound_at");
    emit(p, M::Lda, A::AbsoluteY, O::Word(0));
    abs(p, M::Sta, SOUND_REPEATS);
    program(p, M::Lda, base, 1);
    abs(p, M::Sta, SOUND_REQUEST);
    abs(p, M::Sta, SOUND_WAIT);
    byte(p, M::Lda, WAIT_SOUND);
    abs(p, M::Sta, WAITING);
    m16(p);
    abs(p, M::Lda, MPC);
    implied(p, M::Clc);
    word(p, M::Adc, 3);
    abs(p, M::Sta, MPC);
    m8(p);
    implied(p, M::Rts);
    p.label("bm_music").assume_mode(CpuMode::NATIVE_M8_X16);
    program(p, M::Lda, base, 1);
    abs(p, M::Sta, crate::sfc_battle_vm::MUSIC + 1);
    abs(p, M::Stz, crate::sfc_battle_vm::MUSIC);
    byte(p, M::Lda, 1);
    abs(p, M::Sta, crate::sfc_battle_vm::MUSIC_PENDING);
    advance(p, 2);
    p.label("bm_trade").assume_mode(CpuMode::NATIVE_M8_X16);
    program(p, M::Lda, base, 1);
    call(p, "shop_trade");
    advance(p, 2);
    p.label("battle_message_append")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    emit(p, M::Ldx, A::Absolute, O::Word(LENGTH));
    emit(
        p,
        M::Cpx,
        A::Immediate,
        O::Word(crate::sfc_b1_text::RUNTIME_BYTES - 1),
    );
    branch(p, M::Bcs, "battle_message_full");
    emit(p, M::Sta, A::AbsoluteLongX, O::Long(PAGE));
    implied(p, M::Inx);
    emit(p, M::Stx, A::Absolute, O::Word(LENGTH));
    byte(p, M::Lda, 1);
    abs(p, M::Sta, DIRTY);
    p.label("battle_message_full");
    implied(p, M::Rts);
    p.label("battle_message_show")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, DIRTY);
    branch(p, M::Beq, "battle_message_shown");
    abs(p, M::Stz, DIRTY);
    emit(p, M::Ldx, A::Absolute, O::Word(LENGTH));
    byte(p, M::Lda, crate::sfc_b1_text::END);
    emit(p, M::Sta, A::AbsoluteLongX, O::Long(PAGE));
    byte(p, M::Lda, 1);
    abs(p, M::Sta, DRAW);
    p.label("battle_message_shown");
    implied(p, M::Rts);
    p.label("bm_number_glyph")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    emit(p, M::Cmp, A::Immediate, O::Byte(10));
    branch(p, M::Bcc, "bm_number_glyph_digit");
    byte(p, M::Lda, 10);
    p.label("bm_number_glyph_digit");
    m16(p);
    word(p, M::And, 0xff);
    abs(p, M::Sta, MWORK);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Clc);
    abs(p, M::Adc, MWORK);
    implied(p, M::Tax);
    m8(p);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(numbers + 1));
    implied(p, M::Phx);
    append(p);
    implied(p, M::Plx);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(numbers));
    emit(p, M::Cmp, A::Immediate, O::Byte(2));
    branch(p, M::Bcc, "bm_number_glyph_done");
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(numbers + 2));
    append(p);
    p.label("bm_number_glyph_done");
    implied(p, M::Rts);
    Ok(())
}
const NUMBER_CELLS: u16 = 0xd42;
