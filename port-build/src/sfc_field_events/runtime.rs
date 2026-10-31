use super::*;
use crate::sfc::{implied, write};
mod control;
mod dialogue;
mod dispatch;
mod effects;
mod messages;
mod movement;
mod palette;
mod pictures;
mod selection;
mod sound_map;
mod values;
#[derive(Clone, Copy)]
struct Tables<'a> {
    layout: &'a CartridgeLayout,
    pressed: u16,
    boundary: u8,
    program: u32,
    events: u32,
    floors: u32,
    lookup: u32,
    counts: u32,
    starts: u32,
    cells: u32,
    pictures: u32,
    picture_songs: u32,
    recolours: u32,
    messages: u32,
    masks: u32,
    bits: u32,
    songs: u32,
    effects: u32,
}
fn abs(p: &mut w65c816::Assembler, m: M, at: u16) {
    crate::sfc::emit(p, m, A::Absolute, O::Word(at));
}
fn long_x(p: &mut w65c816::Assembler, m: M, at: u32) {
    crate::sfc::emit(p, m, A::AbsoluteLongX, O::Long(at));
}
fn branch(p: &mut w65c816::Assembler, m: M, label: &str) {
    p.emit_label_ref(m, A::Relative8, label);
}
fn call(p: &mut w65c816::Assembler, label: &str) {
    p.emit_label_ref(M::Jsr, A::Absolute, label);
}
fn jump(p: &mut w65c816::Assembler, label: &str) {
    p.emit_label_ref(M::Jmp, A::Absolute, label);
}
fn m16(p: &mut w65c816::Assembler) {
    crate::sfc::emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
}
fn m8(p: &mut w65c816::Assembler) {
    crate::sfc::emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
}
pub fn emit(
    p: &mut w65c816::Assembler,
    layout: &CartridgeLayout,
    pressed: u16,
    boundary: u8,
) -> Result<()> {
    let section = |name: &str| -> Result<u32> { layout.cpu_address(layout.section(name)?.offset) };
    let program = section("field_program")?;
    let events = section("field_events")?;
    let floors = section("field_floors")?;
    let lookup = section("field_event_lookup")?;
    let ids = layout.section("field_floors")?.bytes.len() as u32 / 8;
    let (counts, starts, cells) = (ids * 8, ids * 16, ids * 18);
    let pictures = section("b2_pictures")?;
    let picture_songs = section("field_picture_songs")?;
    let recolours = section("field_recolours")?;
    let messages = section("field_messages")?;
    let masks = section("field_direction_masks")?;
    let bits = section("field_bits")?;
    let songs = section("field_songs")?;
    let effects = section("sound_effect_map")?;
    if layout.section("sound_effect_map")?.bytes.len() != EFFECTS as usize {
        return Err("Sound effect map size changed".into());
    }
    let tables = Tables {
        layout,
        pressed,
        boundary,
        program,
        events,
        floors,
        lookup,
        counts,
        starts,
        cells,
        pictures,
        picture_songs,
        recolours,
        messages,
        masks,
        bits,
        songs,
        effects,
    };
    selection::emit(p, &tables)?;
    dispatch::emit(p, &tables)?;
    control::emit(p, &tables)?;
    effects::emit(p, &tables)?;
    pictures::emit(p, &tables)?;
    values::emit(p, &tables)?;
    sound_map::emit(p, &tables)?;
    let page_delays = palette::emit(p, &tables)?;
    messages::emit(p, &tables)?;
    movement::emit(p, &tables)?;
    dialogue::emit(p, &tables, page_delays)?;
    Ok(())
}
