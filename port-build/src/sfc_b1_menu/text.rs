use super::*;
pub(super) fn message_text(tokens: &[Value]) -> Result<String> {
    use crate::b1_dialogue::{SOURCE_BREAK, color_mark};
    let mut text = String::new();
    let mut color = 2u8;
    for token in tokens {
        match token["kind"].as_str().ok_or("Missing menu token kind")? {
            "text" => {
                if text.chars().all(|c| c == SOURCE_BREAK) && color != 2 {
                    text.push(color_mark(color));
                }
                text.push_str(token["text"].as_str().ok_or("Invalid menu text")?);
            }
            "newline" => text.push(SOURCE_BREAK),
            "palette_color" => {
                color = 1 + u8::try_from(token["value"].as_u64().ok_or("Invalid menu colour")?)?;
                if text.chars().any(|c| c != SOURCE_BREAK) {
                    text.push(color_mark(color));
                }
            }
            "message_call" | "wait_ticks" | "end" => {}
            kind => return Err(format!("Unsupported menu control {kind}").into()),
        }
    }
    Ok(text)
}
pub(super) fn definition(content: &crate::content::Content) -> Result<(Vec<u8>, Definition)> {
    let raw = content.read("menu/menu.json")?;
    let def: Definition = serde_json::from_slice(&raw)?;
    if def.schema != 1 || def.source_sha256 != SOURCE {
        return Err("Invalid menu source".into());
    }
    Ok((raw, def))
}
pub fn text(content: &crate::content::Content) -> Result<String> {
    let (_, def) = definition(content)?;
    let items: Items = content.json("items.json")?;
    let mut text = String::new();
    for hover in def.hover.values() {
        text.push_str(&message_text(&hover.tokens)?);
    }
    for action in def.actions.values() {
        for step in action
            .steps
            .iter()
            .chain(action.follow_variants.iter().flatten())
        {
            text.push_str(&message_text(&step.tokens)?);
        }
    }
    for item in items.items.iter().filter(|i| HELD_ITEMS.contains(&i.id)) {
        text.push_str(&message_text(&item.description.tokens)?);
    }
    let files = &def.save_files;
    for line in files
        .hover
        .iter()
        .chain(&files.map_names)
        .chain([&files.empty])
        .chain(&files.save_question)
        .chain(&files.load_question)
    {
        text.push_str(line);
    }
    text.push_str("１２３４");
    Ok(text)
}
