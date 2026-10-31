use crate::{Result, SOURCE, sha};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;
pub type SignPages = (Vec<String>, Vec<(usize, Value)>);
#[derive(Deserialize)]
struct Document {
    schema: u8,
    source_sha256: String,
    messages: Vec<Message>,
}
#[derive(Deserialize)]
struct Message {
    source_va: u32,
    tokens: Vec<Value>,
}
#[derive(Deserialize)]
struct World {
    source_sha256: String,
    messages: Vec<Message>,
}
fn va_of(value: &Value) -> Result<u32> {
    Ok(u32::try_from(
        value.as_u64().ok_or("Invalid message address")?,
    )?)
}
pub const SOURCE_BREAK: char = '\u{200b}';
const COLOR_MARK: u32 = 0xe000;
pub fn color_mark(index: u8) -> char {
    char::from_u32(COLOR_MARK + u32::from(index)).unwrap_or(SOURCE_BREAK)
}
pub fn mark_color(ch: char) -> Option<u8> {
    let value = u32::from(ch).checked_sub(COLOR_MARK)?;
    (1..=6).contains(&value).then_some(value as u8)
}
const DEFAULT_COLOR: u8 = 2;
fn set_color(text: &mut String, color: &mut u8, value: &Value) -> Result<()> {
    *color = 1 + u8::try_from(value.as_u64().ok_or("Invalid text colour")?)?;
    if text
        .chars()
        .any(|c| c != SOURCE_BREAK && mark_color(c).is_none())
    {
        text.push(color_mark(*color));
    }
    Ok(())
}
fn push_text(text: &mut String, color: u8, value: &Value) -> Result<()> {
    if color != DEFAULT_COLOR && text.chars().all(|c| c == SOURCE_BREAK) {
        text.push(color_mark(color));
    }
    text.push_str(value.as_str().ok_or("Invalid B1 text")?);
    Ok(())
}
fn space(ch: char) -> bool {
    matches!(ch, ' ' | '\u{3000}')
}
#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Rows,
    Unindented,
}
const SIGNATURE_INDENT: usize = 3;
pub struct PageLayout<'a> {
    pub width: usize,
    pub lines: usize,
    pub advance: Box<dyn Fn(char) -> Result<usize> + 'a>,
    pub joins: &'a [Join],
    pub used: std::cell::RefCell<std::collections::BTreeSet<usize>>,
}
#[derive(Deserialize)]
pub struct Join {
    pub scope: String,
    pub before: String,
    pub after: String,
}
#[derive(Deserialize)]
struct Joins {
    joins: Vec<Join>,
}
pub fn load_joins() -> Result<Vec<Join>> {
    let joins: Joins = serde_json::from_slice(crate::recipes::JOINS)?;
    if joins
        .joins
        .iter()
        .any(|j| j.before.is_empty() || j.after.trim_start_matches(space).is_empty())
    {
        return Err("Invalid source line join".into());
    }
    Ok(joins.joins)
}
impl PageLayout<'_> {
    pub fn unused(&self, scope: &str) -> Vec<String> {
        let used = self.used.borrow();
        self.joins
            .iter()
            .enumerate()
            .filter(|(i, j)| j.scope == scope && !used.contains(i))
            .map(|(_, j)| format!("{}|{}", j.before, j.after))
            .collect()
    }
    fn join(&self, text: &str) -> String {
        let mut text = text.to_string();
        for (i, join) in self.joins.iter().enumerate() {
            let split = format!("{}{SOURCE_BREAK}{}", join.before, join.after);
            if text.contains(&split) {
                let joined = format!("{}{}", join.before, join.after.trim_start_matches(space));
                text = text.replace(&split, &joined);
                self.used.borrow_mut().insert(i);
            }
        }
        text
    }
    fn ink(&self, text: &str) -> Result<usize> {
        text.chars().map(&self.advance).sum()
    }
    pub fn pages(&self, text: &str) -> Result<Vec<String>> {
        let text = self.join(text);
        let rows: Vec<&str> = text
            .trim_matches(SOURCE_BREAK)
            .split(SOURCE_BREAK)
            .collect();
        for mode in [Mode::Rows, Mode::Unindented] {
            let lines = self.lines_for(&rows, mode)?;
            if lines.iter().flatten().count() <= self.lines {
                return Ok(vec![lines.concat().join("\n")]);
            }
        }
        let units: Vec<Vec<String>> = self
            .lines_for(&rows, Mode::Rows)?
            .into_iter()
            .flat_map(|group| {
                group
                    .chunks(self.lines)
                    .map(<[String]>::to_vec)
                    .collect::<Vec<_>>()
            })
            .collect();
        let pack = |capacity: usize| {
            let mut pages: Vec<Vec<String>> = Vec::new();
            for unit in &units {
                match pages.last_mut() {
                    Some(page) if page.len() + unit.len() <= capacity => page.extend(unit.clone()),
                    _ => pages.push(unit.clone()),
                }
            }
            pages
        };
        let fewest = pack(self.lines).len();
        let largest = units.iter().map(Vec::len).max().unwrap_or(1);
        let pages = (largest..=self.lines)
            .map(pack)
            .find(|pages| pages.len() == fewest)
            .unwrap_or_else(|| pack(self.lines));
        Ok(pages.into_iter().map(|p| p.join("\n")).collect())
    }
    fn lines_for(&self, rows: &[&str], mode: Mode) -> Result<Vec<Vec<String>>> {
        let mut groups = Vec::new();
        for row in rows {
            let lead: String = row
                .chars()
                .take_while(|c| space(*c) || mark_color(*c).is_some())
                .collect();
            let indent = lead.chars().filter(|c| space(*c)).count();
            let marks: String = lead.chars().filter(|c| mark_color(*c).is_some()).collect();
            let body = &format!(
                "{marks}{}",
                single_spaced(row[lead.len()..].trim_end_matches(space))
            );
            if indent >= SIGNATURE_INDENT {
                let pad = (self.advance)(' ')?;
                groups.push(
                    self.wrap(body, self.width)?
                        .into_iter()
                        .map(|line| Ok(" ".repeat((self.width - self.ink(&line)?) / pad) + &line))
                        .collect::<Result<Vec<_>>>()?,
                );
            } else if indent > 0 && mode == Mode::Rows {
                let width = self.width - (self.advance)('\u{3000}')?;
                groups.push(
                    self.wrap(body, width)?
                        .into_iter()
                        .map(|line| format!("\u{3000}{line}"))
                        .collect(),
                );
            } else {
                groups.push(self.wrap(body, self.width)?);
            }
        }
        Ok(groups)
    }
    fn wrap(&self, row: &str, width: usize) -> Result<Vec<String>> {
        let words = words(row);
        if words.is_empty() {
            return Ok(Vec::new());
        }
        let inks = words
            .iter()
            .map(|w| self.ink(w.trim_end_matches(space)))
            .collect::<Result<Vec<_>>>()?;
        let fulls = words
            .iter()
            .map(|w| self.ink(w))
            .collect::<Result<Vec<_>>>()?;
        if inks.iter().any(|&ink| ink > width) {
            return self.break_characters(row, width);
        }
        let span = |i: usize, j: usize| fulls[i..j - 1].iter().sum::<usize>() + inks[j - 1];
        let n = words.len();
        for count in 1..=n {
            let mut best = vec![vec![None; n + 1]; count + 1];
            let mut from = vec![vec![0; n + 1]; count + 1];
            best[0][0] = Some(0);
            for k in 1..=count {
                for j in 1..=n {
                    for i in k - 1..j {
                        let (Some(before), true) = (best[k - 1][i], span(i, j) <= width) else {
                            continue;
                        };
                        let widest = before.max(span(i, j));
                        if best[k][j].is_none_or(|b| widest < b) {
                            best[k][j] = Some(widest);
                            from[k][j] = i;
                        }
                    }
                }
            }
            if best[count][n].is_some() {
                let mut lines = Vec::new();
                let mut j = n;
                for k in (1..=count).rev() {
                    let i = from[k][j];
                    lines.push(words[i..j].concat().trim_end_matches(space).to_string());
                    j = i;
                }
                lines.reverse();
                return Ok(lines);
            }
        }
        Err("Unreachable dialogue wrap".into())
    }
    fn break_characters(&self, row: &str, width: usize) -> Result<Vec<String>> {
        let mut lines = Vec::new();
        let mut line = String::new();
        let mut used = 0;
        for ch in row.chars() {
            let advance = (self.advance)(ch)?;
            if !space(ch) && !line.is_empty() && used + advance > width {
                lines.push(line.trim_end_matches(space).to_string());
                line.clear();
                used = 0;
            }
            if line.is_empty() && space(ch) {
                continue;
            }
            line.push(ch);
            used += advance;
        }
        if !line.is_empty() {
            lines.push(line.trim_end_matches(space).to_string());
        }
        Ok(lines)
    }
    pub fn choice(&self, text: &str) -> Result<String> {
        let pages = self.pages(text)?;
        match pages.as_slice() {
            [page] if page.lines().count() < self.lines => Ok(format!("{page}\n{CHOICE}")),
            _ => Err("Choice prompt exceeds screen".into()),
        }
    }
}
pub const TIMED: u8 = 0x80;
pub const EFFECT_WAIT: u8 = TIMED;
pub const STANDING: u8 = 0x7f;
pub const CHOICE: &str = "Ａ 예　Ｂ 아니오";
fn single_spaced(row: &str) -> String {
    let mut text = String::new();
    for ch in row.chars() {
        if !(space(ch) && text.ends_with(space)) {
            text.push(ch);
        }
    }
    text
}
fn words(text: &str) -> Vec<&str> {
    let mut words = Vec::new();
    let mut start = 0;
    let mut previous = None;
    for (i, ch) in text.char_indices() {
        if previous.is_some_and(space) && !space(ch) {
            words.push(&text[start..i]);
            start = i;
        }
        previous = Some(ch);
    }
    if start < text.len() {
        words.push(&text[start..]);
    }
    words
}
pub struct EventPages {
    pub pages: Vec<String>,
    pub choice: bool,
    pub audio: Vec<(usize, Value)>,
    pub delays: Vec<u8>,
}
pub struct Catalogue {
    messages: BTreeMap<u32, Vec<Value>>,
    pub text: String,
    pub report: Value,
}
impl Catalogue {
    pub fn load(root: &crate::content::Content) -> Result<Self> {
        let raw = root.read("b1-event-dialogue.json")?;
        let source: Document = serde_json::from_slice(&raw)?;
        if source.schema != 1 || source.source_sha256 != SOURCE || source.messages.len() != 37 {
            return Err("Invalid B1 dialogue source".into());
        }
        let world_raw = root.read("world-dialogue.json")?;
        let world: World = serde_json::from_slice(&world_raw)?;
        if world.source_sha256 != SOURCE {
            return Err("Invalid world dialogue source".into());
        }
        let mut world: BTreeMap<u32, Vec<Value>> = world
            .messages
            .into_iter()
            .map(|m| (m.source_va, m.tokens))
            .collect();
        let mut messages = BTreeMap::new();
        for message in source.messages {
            if messages.insert(message.source_va, message.tokens).is_some() {
                return Err("Duplicate B1 message address".into());
            }
        }
        let (floors, ids) = crate::floor_events::load(root)?;
        let mut pending: Vec<u32> = floors
            .iter()
            .flat_map(|f| f.messages().0)
            .map(|i| ids.messages[i])
            .chain([crate::floor_events::FOLLOW_UP_MESSAGE])
            .collect();
        let mut seen = std::collections::BTreeSet::new();
        while let Some(at) = pending.pop() {
            if !seen.insert(at) {
                continue;
            }
            let tokens = match messages.entry(at) {
                std::collections::btree_map::Entry::Occupied(e) => e.into_mut(),
                std::collections::btree_map::Entry::Vacant(e) => e.insert(
                    world
                        .remove(&at)
                        .ok_or_else(|| format!("Missing floor message {at:#x}"))?,
                ),
            };
            for token in tokens.iter() {
                match token["kind"].as_str() {
                    Some("message_call") => pending.push(va_of(&token["target_va"])?),
                    Some("message_dispatch") => {
                        for target in token["targets"].as_array().ok_or("Missing targets")? {
                            pending.push(va_of(target)?);
                        }
                    }
                    _ => {}
                }
            }
        }
        let mut text = String::new();
        for token in messages.values().flatten() {
            if token["kind"] == "text" {
                text.push_str(token["text"].as_str().ok_or("Invalid message text")?);
            }
        }
        let report = serde_json::json!(
            { "source_sha256" : sha(& raw), "world_sha256" : sha(& world_raw), "messages"
            : messages, "limits" :
            ["Source controls retained; message calls are made inline and a state dispatch becomes one page set per alternative.",
            "Source colors and row layout remain in tokens; pages reflow by word to the monochrome 12-pixel dialogue window."]
            }
        );
        Ok(Self {
            messages,
            text,
            report,
        })
    }
    fn expanded(&self, address: u32, variant: usize) -> Result<Vec<Value>> {
        let tokens = self
            .messages
            .get(&address)
            .ok_or_else(|| format!("Missing message {address:#x}"))?;
        let mut out = Vec::new();
        for token in tokens {
            if token["kind"] != "message_call" {
                out.push(token.clone());
                continue;
            }
            let mut body = &self.messages[&va_of(&token["target_va"])?];
            if let Some(first) = body.first().filter(|t| t["kind"] == "message_dispatch") {
                let targets = first["targets"].as_array().ok_or("Missing targets")?;
                let pick = targets.get(variant.min(targets.len() - 1));
                body = &self.messages[&va_of(pick.ok_or("Empty dispatch")?)?];
            }
            out.extend(body.iter().take_while(|t| t["kind"] != "return").cloned());
        }
        Ok(out)
    }
    pub fn dispatch(&self, address: u32) -> Result<Option<(u8, usize)>> {
        let tokens = self.messages.get(&address).ok_or("Missing message")?;
        let field_bound = tokens
            .iter()
            .any(|t| t["kind"] == "bind_external" && t["mode"] == 0);
        for token in tokens.iter().filter(|t| t["kind"] == "message_call") {
            let body = &self.messages[&va_of(&token["target_va"])?];
            if let Some(first) = body.first().filter(|t| t["kind"] == "message_dispatch") {
                let slot = match (
                    first["selector_mode"].as_u64(),
                    first["selector_index"].as_u64(),
                    first["table_mode"].as_u64(),
                ) {
                    (Some(1), Some(slot), Some(0)) => u8::try_from(slot)?,
                    (Some(3), Some(0x20), Some(0)) if field_bound => {
                        crate::sfc_field_events::FOLLOW_UP_SLOT
                    }
                    _ => {
                        return Err(format!("Unsupported message dispatch at {address:#x}").into());
                    }
                };
                let count = first["targets"].as_array().ok_or("Missing targets")?.len();
                return Ok(Some((slot, count)));
            }
        }
        Ok(None)
    }
    pub fn sign_variant(
        &self,
        address: u32,
        variant: usize,
        layout: &PageLayout,
    ) -> Result<SignPages> {
        let tokens = self.expanded(address, variant)?;
        let mut audio = Vec::new();
        let mut pages = Vec::new();
        let mut text = String::new();
        let mut color = DEFAULT_COLOR;
        let mut waiting = false;
        for token in &tokens {
            match token["kind"].as_str().ok_or("Missing B1 token kind")? {
                "text" if !waiting => push_text(&mut text, color, &token["text"])?,
                "text" => {
                    waiting = false;
                    push_text(&mut text, color, &token["text"])?
                }
                "newline" if !waiting => text.push(SOURCE_BREAK),
                "newline" => {}
                "font" if token["value"] == 2 => {}
                "palette_color" if token["value"] == 0 || token["value"] == 1 => {
                    set_color(&mut text, &mut color, &token["value"])?
                }
                "window" if token["value"] == 196608 => {}
                "position" if token["x"] == 104 && token["y"] == 344 => {}
                "delay" if token["value"] == 0 => {}
                "sound_effect" | "audio" if text.is_empty() && !waiting => {
                    audio.push((pages.len(), token.clone()));
                }
                "clear" if text.is_empty() || waiting => {
                    text.clear();
                    waiting = false;
                }
                "wait_input" if !text.is_empty() && !waiting => {
                    pages.extend(layout.pages(&text)?);
                    waiting = true;
                }
                "end" if !text.is_empty() && !waiting && pages.is_empty() => {
                    let laid = layout.pages(&text)?;
                    if laid.len() != 1 {
                        return Err(format!("Sign notice at {address:#x} overflows").into());
                    }
                    pages.extend(laid);
                    text.clear();
                }
                "yield" | "end" if text.is_empty() => {}
                kind => {
                    return Err(format!("Unsupported sign control {kind} at {address:#x}").into());
                }
            }
        }
        if pages.is_empty() || !text.is_empty() {
            return Err("Incomplete B1 sign dialogue".into());
        }
        Ok((pages, audio))
    }
    pub fn event_variant(
        &self,
        address: u32,
        variant: usize,
        layout: &PageLayout,
    ) -> Result<EventPages> {
        let tokens = self.expanded(address, variant)?;
        let mut pages = Vec::new();
        let mut text = String::new();
        let mut color = DEFAULT_COLOR;
        let mut choice = false;
        let mut pending_audio = Vec::new();
        let mut delays = Vec::new();
        let mut pending_ticks = 0;
        let mut last_wait_text = String::new();
        for token in &tokens {
            match token["kind"].as_str().ok_or("Missing B1 token kind")? {
                "text" => push_text(&mut text, color, &token["text"])?,
                "newline" => text.push(SOURCE_BREAK),
                "font" if token["value"] == 2 => {}
                "palette_color" if token["value"].as_u64().is_some_and(|v| v <= 5) => {
                    set_color(&mut text, &mut color, &token["value"])?
                }
                "window" if token["value"] == 196608 => {}
                "position" if token["x"] == 104 && token["y"] == 344 => {}
                "delay" => {}
                "bind_external" if token["mode"] == 0 => {}
                "clear" => text.clear(),
                "wait_ticks" if address == 0x49d5de && token["value"] == 24 => {
                    pending_ticks = 24;
                }
                "wait_ticks" if !text.is_empty() => {
                    let ticks = u8::try_from(token["value"].as_u64().ok_or("Wait ticks")?)?;
                    let waited = layout.pages(&text)?;
                    if waited.len() != 1 || ticks == 0 || ticks >= TIMED || pending_ticks != 0 {
                        return Err(format!("Unsupported timed page at {address:#x}").into());
                    }
                    delays.push(TIMED | ticks);
                    last_wait_text = text.clone();
                    pages.extend(waited);
                }
                "wait_ticks" if text.is_empty() && !pages.is_empty() && pending_ticks == 0 => {
                    let ticks = u8::try_from(token["value"].as_u64().ok_or("Wait ticks")?)?;
                    if ticks == 0 || ticks >= TIMED {
                        return Err(format!("Unsupported timed pause at {address:#x}").into());
                    }
                    delays.push(TIMED | ticks);
                    last_wait_text.clear();
                    pages.push(String::new());
                }
                "wait_sound_effect" if !text.is_empty() && pending_ticks == 0 => {
                    let waited = layout.pages(&text)?;
                    if waited.len() != 1 {
                        return Err(format!("Sound-timed page overflows at {address:#x}").into());
                    }
                    delays.push(EFFECT_WAIT);
                    last_wait_text = text.clone();
                    pages.extend(waited);
                }
                "wait_input" if !text.is_empty() => {
                    let waited = layout.pages(&text)?;
                    delays
                        .extend((0..waited.len()).map(|i| if i == 0 { pending_ticks } else { 0 }));
                    pending_ticks = 0;
                    last_wait_text = text.clone();
                    pages.extend(waited);
                }
                "game_call" if token["target_va"] == 0x4be0b8 && !choice => {
                    pages.push(layout.choice(&text)?);
                    delays.push(0);
                    choice = true;
                }
                "audio_control" | "sound_effect" | "wait_audio" | "audio" => {
                    pending_audio.push((pages.len(), token.clone()))
                }
                "yield" | "end" if text.is_empty() || text == last_wait_text => {}
                "end" if !pages.is_empty() && pending_ticks == 0 => {
                    let laid = layout.pages(&text)?;
                    if laid.len() != 1 {
                        return Err(format!("Standing page overflows at {address:#x}").into());
                    }
                    delays.push(STANDING);
                    last_wait_text = text.clone();
                    pages.extend(laid);
                }
                kind => {
                    return Err(
                        format!("Unsupported B1 event control {kind} at {address:#x}").into(),
                    );
                }
            }
        }
        if pages.is_empty() || (!text.is_empty() && text != last_wait_text) || pending_ticks != 0 {
            return Err("Incomplete B1 event pages".into());
        }
        Ok(EventPages {
            pages,
            choice,
            audio: pending_audio,
            delays,
        })
    }
    pub fn is_notice(&self, address: u32) -> bool {
        let count = self.dispatch(address).ok().flatten().map_or(1, |(_, n)| n);
        (0..count).all(|variant| {
            self.expanded(address, variant).is_ok_and(|tokens| {
                tokens.iter().all(|t| {
                    !matches!(
                        t["kind"].as_str(),
                        Some("wait_input" | "game_call" | "wait_ticks")
                    )
                })
            })
        })
    }
    pub fn is_silent(&self, address: u32, variant: usize) -> Result<bool> {
        Ok(self.expanded(address, variant)?.iter().all(|t| {
            !matches!(
                t["kind"].as_str(),
                Some("text" | "audio" | "sound_effect" | "audio_control")
            )
        }))
    }
    pub fn notice(&self, address: u32, layout: &PageLayout) -> Result<String> {
        let tokens = self.messages.get(&address).ok_or("Missing B1 notice")?;
        let mut text = String::new();
        let mut ended = false;
        for (i, token) in tokens.iter().enumerate() {
            if ended {
                return Err("B1 notice has tokens after end".into());
            }
            let last = tokens.get(i + 1).is_some_and(|t| t["kind"] == "end");
            match token["kind"].as_str().ok_or("Missing notice kind")? {
                "font" if token["value"] == 2 => {}
                "palette_color" if token["value"] == 1 => {}
                "window" if token["value"] == 196608 => {}
                "position" if token["x"] == 104 && token["y"] == 344 => {}
                "delay" if token["value"] == 0 => {}
                "audio" | "sound_effect" => {}
                "clear" => text.clear(),
                "yield" if text.is_empty() || last => {}
                "newline" => text.push(SOURCE_BREAK),
                "text" => text.push_str(token["text"].as_str().ok_or("Invalid notice text")?),
                "end" => ended = true,
                _ => {
                    return Err(format!(
                        "Interactive control in arrival notice {address:#x}: {token}"
                    )
                    .into());
                }
            }
        }
        if ended && text.is_empty() {
            return Ok(text);
        }
        let pages = layout.pages(&text)?;
        if !ended || pages.len() != 1 {
            return Err("Invalid B1 notice length or termination".into());
        }
        Ok(pages.concat())
    }
    pub fn notice_audio(&self, address: u32) -> Vec<Value> {
        self.messages.get(&address).map_or_else(Vec::new, |tokens| {
            tokens
                .iter()
                .filter(|t| t["kind"] == "audio" || t["kind"] == "sound_effect")
                .cloned()
                .collect()
        })
    }
}
