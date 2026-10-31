use crate::Result;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, PartialEq, Deserialize)]
pub struct Message {
    #[serde(default)]
    source_va: u32,
    id: u16,
    event_id: Option<u8>,
    scene: Scene,
    tokens: Vec<Token>,
}
#[derive(Clone, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Scene {
    B1,
    Prologue,
}
#[derive(Clone, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Token {
    Text {
        #[serde(default)]
        va: u32,
        text: String,
    },
    End,
    Newline,
    Font {
        value: u8,
    },
    Delay {
        value: u16,
    },
    WaitInput,
    WaitTicks {
        value: u16,
    },
    Audio {
        bank: String,
        channel: u8,
        index: u8,
    },
    Position {
        x: u16,
        y: u16,
    },
    PaletteColor {
        value: u8,
    },
    Clear,
    Window {
        value: u32,
    },
    Yield,
}
#[derive(Debug)]
pub struct Compiled {
    pub messages: Vec<u8>,
    pub glyphs: Vec<u8>,
    pub glyph_count: usize,
    pub message_count: usize,
}
pub fn validate_prologue_controls(original: &[Message], content: &[Message]) -> Result<()> {
    let source: Vec<_> = original
        .iter()
        .filter(|m| matches!(m.scene, Scene::Prologue))
        .collect();
    if source.len() != content.len() {
        return Err("Prologue message inventory changed".into());
    }
    for message in content {
        let previous = source
            .iter()
            .find(|m| m.id == message.id)
            .ok_or("Unknown prologue message")?;
        if message.source_va != previous.source_va
            || message.scene != previous.scene
            || message.event_id != previous.event_id
            || message.tokens.len() != previous.tokens.len()
            || message
                .tokens
                .iter()
                .zip(&previous.tokens)
                .any(|(a, b)| match (a, b) {
                    (Token::Text { va: a, .. }, Token::Text { va: b, .. }) => a != b,
                    _ => a != b,
                })
        {
            return Err(format!(
                "Prologue message {} control edit requires timeline regeneration",
                message.id
            )
            .into());
        }
    }
    Ok(())
}
pub fn prologue_text(messages: &[Message]) -> Result<BTreeMap<u32, String>> {
    let mut text = BTreeMap::new();
    for message in messages
        .iter()
        .filter(|m| matches!(m.scene, Scene::Prologue))
    {
        for token in &message.tokens {
            if let Token::Text { va, text: value } = token
                && (*va == 0 || text.insert(*va, value.clone()).is_some())
            {
                return Err("Missing or duplicate canonical prologue text address".into());
            }
        }
    }
    Ok(text)
}
pub fn entry_pages(messages: &[Message]) -> Result<Vec<String>> {
    let message = messages
        .iter()
        .find(|m| m.id == 0)
        .ok_or("Missing B1 entry message")?;
    if !matches!(message.scene, Scene::B1) || message.event_id != Some(17) {
        return Err("Unexpected B1 entry message binding".into());
    }
    let [
        Token::Font { value: 2 },
        Token::PaletteColor { value: 1 },
        Token::Window { value: 196608 },
        Token::Position { x: 104, y: 344 },
        Token::Delay { value: 0 },
        Token::Clear,
        body @ ..,
    ] = message.tokens.as_slice()
    else {
        return Err("Unsupported B1 entry message setup".into());
    };
    if !matches!(body, [.., Token::Clear, Token::Yield, Token::End]) {
        return Err("Unsupported B1 entry message completion".into());
    }
    let mut pages = Vec::new();
    let mut text = String::new();
    let mut waiting = false;
    for token in body {
        match token {
            Token::Newline if !waiting => text.push(crate::b1_dialogue::SOURCE_BREAK),
            Token::Text { text: part, .. } if !waiting => text.push_str(part),
            Token::WaitInput if !waiting && !text.is_empty() => {
                pages.push(text.clone());
                waiting = true;
            }
            Token::Clear if text.is_empty() || waiting => {
                text.clear();
                waiting = false;
            }
            Token::Yield | Token::End if text.is_empty() => {}
            _ => return Err("Unsupported B1 entry message control sequence".into()),
        }
    }
    if pages.len() != 2 || !text.is_empty() {
        return Err("Incomplete B1 entry message pages".into());
    }
    Ok(pages)
}
#[derive(Clone, serde::Serialize)]
pub struct TimedPage {
    pub cells: Vec<(char, u8)>,
    pub wait: u16,
}
#[derive(serde::Serialize)]
pub struct TimedProgram {
    pub id: u16,
    pub pages: Vec<TimedPage>,
}
pub fn timed_pages(messages: &[Message], id: u16) -> Result<(Vec<String>, Vec<u16>)> {
    let program = timed_program(messages, id)?;
    if program
        .pages
        .iter()
        .flat_map(|p| &p.cells)
        .any(|(_, color)| *color != 1)
    {
        return Err("White-only message consumer received colored text".into());
    }
    Ok((
        program
            .pages
            .iter()
            .map(|p| p.cells.iter().map(|(ch, _)| ch).collect())
            .collect(),
        program.pages.iter().map(|p| p.wait).collect(),
    ))
}
pub fn timed_program(messages: &[Message], id: u16) -> Result<TimedProgram> {
    let message = messages
        .iter()
        .find(|m| m.id == id)
        .ok_or("Missing timed message")?;
    if !matches!(message.scene, Scene::Prologue) || message.event_id.is_some() {
        return Err("Unexpected timed message binding".into());
    }
    let [
        Token::Font { value: 2 },
        Token::PaletteColor { value: 1 },
        Token::Window { value: 196612 },
        Token::Position { x: 104, y: 384 },
        Token::Delay { value: 0 },
        Token::Clear,
        body @ ..,
    ] = message.tokens.as_slice()
    else {
        return Err("Unsupported timed message setup".into());
    };
    let [body @ .., Token::Clear, Token::Yield, Token::End] = body else {
        return Err("Unsupported timed message completion".into());
    };
    let mut pages = Vec::new();
    let mut text = Vec::new();
    let mut color = 1;
    let mut has_waited = false;
    for token in body {
        match token {
            Token::Text { text: part, .. } => {
                text.extend(part.chars().map(|ch| (ch, color)));
                has_waited = false;
            }
            Token::Newline => {}
            Token::PaletteColor { value } if *value < 6 => {
                color = *value;
            }
            Token::Clear if has_waited => {
                text.clear();
                has_waited = false;
            }
            Token::WaitTicks { value } if value & 0x7fff != 0 => {
                if text.len() > 80 {
                    return Err("Timed message exceeds target panel".into());
                }
                pages.push(TimedPage {
                    cells: text.clone(),
                    wait: *value,
                });
                has_waited = true;
            }
            _ => return Err("Unsupported timed message control".into()),
        }
    }
    if !has_waited || pages.is_empty() || pages.len() > 254 {
        return Err("Incomplete timed message".into());
    }
    Ok(TimedProgram { id, pages })
}
fn short(buffer: &mut Vec<u8>, value: usize) -> Result<()> {
    buffer.extend(u16::try_from(value)?.to_le_bytes());
    Ok(())
}
pub fn compile(messages: &[Message], event_ids: &BTreeSet<u8>) -> Result<Compiled> {
    if messages.is_empty() {
        return Err("Missing messages".into());
    }
    let mut ids = BTreeSet::new();
    let mut characters = BTreeSet::new();
    for message in messages {
        if !ids.insert(message.id)
            || match message.scene {
                Scene::B1 => !message.event_id.is_some_and(|id| event_ids.contains(&id)),
                Scene::Prologue => message.event_id.is_some(),
            }
            || !matches!(message.tokens.last(), Some(Token::End))
        {
            return Err("Invalid message identity, event binding or terminator".into());
        }
        for (index, token) in message.tokens.iter().enumerate() {
            if matches!(token, Token::End) && index + 1 != message.tokens.len() {
                return Err("Message has tokens after end".into());
            }
            if let Token::Text { text, .. } = token {
                if text.is_empty() || text.chars().any(char::is_control) {
                    return Err("Empty text or unexpected control character".into());
                }
                characters.extend(text.chars());
            }
        }
    }
    let lookup: BTreeMap<_, _> = characters
        .iter()
        .enumerate()
        .map(|(i, c)| (*c, i))
        .collect();
    let mut glyphs = b"HMG1".to_vec();
    short(&mut glyphs, characters.len())?;
    for character in &characters {
        glyphs.extend((*character as u32).to_le_bytes());
    }
    let mut output = b"HMD2".to_vec();
    short(&mut output, messages.len())?;
    for message in messages {
        let mut stream = Vec::new();
        for token in &message.tokens {
            match token {
                Token::End => stream.push(0),
                Token::Text { text, .. } => {
                    stream.push(1);
                    short(&mut stream, text.chars().count())?;
                    for character in text.chars() {
                        short(&mut stream, lookup[&character])?;
                    }
                }
                Token::Newline => stream.push(2),
                Token::WaitInput => stream.push(3),
                Token::WaitTicks { value } => {
                    stream.push(11);
                    stream.extend(value.to_le_bytes());
                }
                Token::Audio {
                    bank,
                    channel,
                    index,
                } => {
                    let id = match bank.as_str() {
                        "VOICE.WLK" => 0,
                        "PCMDATA.WLK" => 1,
                        _ => return Err("Unknown audio bank".into()),
                    };
                    if *channel >= 17 {
                        return Err("Invalid source audio channel".into());
                    }
                    stream.extend([12, id, *channel, *index]);
                }
                Token::Clear => stream.push(4),
                Token::Font { value } => stream.extend([5, *value]),
                Token::Delay { value } => {
                    stream.push(6);
                    stream.extend(value.to_le_bytes());
                }
                Token::Position { x, y } => {
                    stream.push(7);
                    stream.extend(x.to_le_bytes());
                    stream.extend(y.to_le_bytes());
                }
                Token::PaletteColor { value } => stream.extend([8, *value]),
                Token::Window { value } => {
                    stream.push(9);
                    stream.extend(value.to_le_bytes());
                }
                Token::Yield => stream.push(10),
            }
        }
        output.extend(message.id.to_le_bytes());
        output.push(match message.scene {
            Scene::B1 => 0,
            Scene::Prologue => 1,
        });
        output.push(message.event_id.unwrap_or(255));
        output.extend(u32::try_from(stream.len())?.to_le_bytes());
        output.extend(stream);
    }
    Ok(Compiled {
        messages: output,
        glyphs,
        glyph_count: characters.len(),
        message_count: messages.len(),
    })
}
pub fn root_bindings(messages: &[Message]) -> Result<Vec<(u32, u8)>> {
    let mut bindings = Vec::new();
    for m in messages
        .iter()
        .filter(|m| matches!(m.scene, Scene::Prologue))
    {
        if !(1..=20).contains(&m.id)
            || m.source_va == 0
            || bindings
                .iter()
                .any(|(va, id)| *va == m.source_va || *id == m.id as u8)
        {
            return Err("Invalid root message binding".into());
        }
        bindings.push((m.source_va, m.id as u8));
    }
    if bindings.len() != 20 {
        return Err("Missing root message bindings".into());
    }
    Ok(bindings)
}
