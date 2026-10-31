mod runtime;
use crate::{Result, floor_events::Floor};
pub use runtime::emit;
use sfc_builder::cartridge::CartridgeLayout;
use w65c816::{AddressingMode as A, CpuMode, Mnemonic as M, Operand as O};
pub const GLOBALS: u16 = 0x800;
const INPUT: u16 = 0x8c0;
pub const SHARD_DRAW_RANGE: u8 = 0x20;
const ANSWER: u16 = INPUT + 0x1d;
pub const CHOSEN: u16 = INPUT + 0x1f;
pub const FOLLOW_UP: u16 = INPUT + 0x20;
const _: () = assert!((FOLLOW_UP - GLOBALS).is_multiple_of(4) && (FOLLOW_UP - GLOBALS) / 4 < 256);
pub const FOLLOW_UP_SLOT: u8 = ((FOLLOW_UP - GLOBALS) / 4) as u8;
pub const SIGNAL: u16 = INPUT + 0x1e;
pub(crate) const PC: u16 = 0x950;
const SELECTED: u16 = 0x952;
const LOOP_TARGET: u16 = 0x954;
pub const FADE_LEVEL: u16 = 0x770;
const FADE_COUNT: u16 = 0x771;
const FADE_START: u16 = 0x772;
const FADE_UNITS: u16 = 0x774;
const FADE_TEMP: u16 = 0x77a;
const FADE_COLORS: u8 = 16;
pub const SCREEN_DARK: u16 = 0x7f1;
const FADE_BUFFER: u32 = 0x7f5000;
const SHADE_COLORS: u8 = 32;
const FADE_LIST: u32 = FADE_BUFFER + 2 * SHADE_COLORS as u32;
const FADE_JOB: u32 = FADE_LIST + 8;
const LOOP_COUNT: u16 = 0x956;
const EVENT: u16 = 0x958;
const KIND: u16 = 0x959;
const ORIGIN: u16 = 0x95a;
pub const CONTEXT: u16 = 0x95b;
pub const BLOCK: u16 = 0x95c;
pub const CLICK: u16 = 0x95d;
pub const HEALING_WAIT: u16 = 0x961;
pub const SCENE_REQUEST: u16 = 0x940;
const SHOP_RETURN: u16 = 0x941;
pub const FOLLOW_PICK: u16 = 0x944;
const _: () =
    assert!((FOLLOW_PICK - GLOBALS).is_multiple_of(4) && (FOLLOW_PICK - GLOBALS) / 4 < 256);
pub const FOLLOW_PICK_SLOT: u8 = ((FOLLOW_PICK - GLOBALS) / 4) as u8;
const MODE: u16 = 0x95e;
const STEPS: u16 = 0x95f;
const MASK: u16 = 0x960;
const TEMP: u16 = 0x962;
const EVENTS_FROM: u16 = 0x966;
const EVENTS_TO: u16 = 0x968;
const REFERENCE: u16 = 0x96a;
const NEXT_FLOOR: u16 = 0x96b;
const RESUME: u16 = 0x96c;
const END: u16 = 0x96e;
const END_SCRATCH: u16 = 0x96e;
const GAME: u16 = 0x970;
const PICTURE_X: u16 = 0x7e8;
const PICTURE_Y: u16 = PICTURE_X + 4;
const PICTURE: u16 = PICTURE_X - 0x1c;
pub const SLIDE_PIXELS: u8 = 16;
pub const SLIDE_STEPS: u8 = 10;
pub const SLIDE_KEY: u8 = 0xf0;
const _: () = assert!(
    SLIDE_PIXELS.is_power_of_two()
        && SLIDE_PIXELS as u16 * (SLIDE_STEPS as u16 + 1) <= 256
        && SLIDE_STEPS < SLIDE_KEY
);
const START_GOLD: u32 = 100;
use crate::sfc_battle::{MP, MP_MAX};
pub(crate) const LIVE: u16 = 0x980;
const LIVE_SLOTS: u16 = 0x982;
const CREATED: u16 = 0x984;
pub(crate) const LIVE_STATE: u16 = 0x985;
const KEY: u16 = 0x986;
const RETURNS: u16 = 0x988;
const DEPTH: u16 = 0x990;
pub(crate) const VIEW_DIRTY: u16 = 0x991;
const WAIT: u16 = 0x992;
const MAX_DEPTH: u8 = 4;
pub const MESSAGE_WAIT: u8 = 0;
pub const MESSAGE_NOTICE: u8 = 1;
pub const MESSAGE_QUESTION: u8 = 2;
pub const MESSAGE_DISPATCH: u8 = 3;
const STEP_LIMIT: u8 = 64;
const SCRIPT_SLACK: u8 = 0u8.wrapping_sub(STEP_LIMIT);
const FOLLOW_UP_PC: u16 = 0;
const SONGS: u64 = 64;
const EFFECTS: u8 = 64;
pub fn check(floor: &Floor) -> Result<()> {
    use crate::sfc_b1_menu::FIRST_ITEM;
    let references = floor.references();
    for (at, raw) in floor.commands() {
        let reference = references.get(&at).copied().unwrap_or(1);
        let width = match raw[0] {
            0x11 | 0x14 => 2,
            0x12 | 0x15 => 4,
            _ => 1,
        };
        let items = 8..8 + crate::sfc_b1_menu::ITEM_KINDS as u8;
        let target = match (raw[1] & 0xc0, raw[0], raw[2], reference) {
            (0x40, ..) => usize::from(raw[2]) * 4 + 4 <= GLOBALS_BYTES,
            (0xc0, 0x12, 0x1c | 0x20, 4) => true,
            (0xc0, 0x12, 4, 2) => true,
            (0xc0, 0x10 | 0x13, o, 2) => items.contains(&o),
            (0xc0, 0x10 | 0x13, 0x1d..=0x20, 1) => true,
            (0xc0, 0x10, 0x19, 1) => true,
            (0xc0, 0x10, 0x26..=0x2d, 8) => true,
            (0xc0, 0x13, 0x2b, 8) => true,
            _ => false,
        };
        let ok = match raw[0] {
            0x01 | 0x02 | 0x03 | 0x04 | 0x05 | 0x06 | 0x07 | 0x77 | 0x25 | 0x28 | 0x29 | 0x2f
            | 0x31 | 0x32 | 0x34 | 0x39 | 0x3b | 0x3f | 0x40 | 0x4b | 0x6f | 0x70 | 0x75 | 0x7a
            | 0x7b | 0x7e | 0x8c => true,
            0x4c => raw[1] == 0,
            0x93 => raw[1] < 2,
            0x27 => raw[1] == 0x22 || raw[1] < 2,
            0x96 => raw[1] < 2,
            0x86 | 0x44 | 0x9b => raw[1] == 0,
            0x35 => raw[1] < 2,
            0x3a => {
                usize::from(raw[1]) == crate::sfc_b1::VIEW_COLORS[0][0]
                    && raw[2] == SHADE_COLORS
                    && matches!(raw[3], 0x04 | 0xfc)
            }
            0x3c => true,
            0x8b => raw[1] == 1 && raw[2] == 4,
            0x69 => raw[4..8] != [0, 0, 0, 0],
            0x83 => true,
            0x88 => true,
            0x8f => true,
            0x38 => {
                let wall = usize::from(raw[2]) == crate::sfc_b1::VIEW_COLORS[0][0]
                    && raw[3] == SHADE_COLORS;
                let upper = raw[2] >= 192
                    && (1..=FADE_COLORS).contains(&raw[3])
                    && usize::from(raw[2]) + usize::from(raw[3]) <= 256;
                let screen = usize::from(raw[2]) < crate::sfc_b1::VIEW_COLORS[0][0]
                    && usize::from(raw[2]) + usize::from(raw[3]) >= 240;
                raw[1] != 1
                    || (raw[4..7] == [0, 0, 0] && (screen || ((wall || upper) && raw[7] % 8 == 0)))
            }
            0x24 => raw[3] < EFFECTS,
            0x26 => u64::from(raw[3]) < SONGS,
            0x10..=0x12 => target && raw[1] & 0x0f < 3,
            0x13 if raw[1] & 0x0f >= 2 => target && raw[1] & 0xc0 == 0xc0,
            0x13 | 0x14 => target && width <= 4 && raw[1] & 0x0f < 2,
            0x15 => target && raw[1] & 0xf0 == 0x40 && raw[1] & 0x0f < 2,
            0x85 => raw[1] == 1 && raw[2] < 8 && raw[3] < 8,
            0x97 => raw[3] == 0,
            0x78 => raw[3] != 16,
            0x84 => raw[1] < 64 && matches!(raw[2] & 0x7f, 1 | 2 | 4 | 8),
            0x81 => (FIRST_ITEM..FIRST_ITEM + 64).contains(&raw[2]),
            _ => false,
        };
        if !ok {
            return Err(format!(
                "Floor {} command {} at {at:#x} (source {:#x}) has no ROM step",
                floor.id,
                raw.iter().map(|b| format!("{b:02x}")).collect::<String>(),
                floor.sources.get(&at).copied().unwrap_or(0)
            )
            .into());
        }
    }
    Ok(())
}
const GLOBALS_BYTES: usize = 192;
pub fn tables(floors: &[Floor], follow_up: u16) -> Result<[Vec<u8>; 3]> {
    let last = floors.iter().map(|f| usize::from(f.id)).max().unwrap_or(0);
    let mut table = vec![[0u16, 0, 0xffff, 0xffff]; last + 2];
    let [lo, hi] = follow_up.to_le_bytes();
    let mut program = vec![
        0x2f, 1, 1, 0, lo, hi, 0, 0, 0x10, 0xc0, 0x20, 0, 0x05, 0, 0, 0,
    ];
    let mut events = Vec::new();
    for floor in floors {
        check(floor)?;
        let base = u16::try_from(program.len())?;
        let (code, records) = floor.relocated(base)?;
        let from = u16::try_from(events.len())?;
        program.extend(code);
        events.extend(records);
        table[usize::from(floor.id)] = [
            from,
            u16::try_from(events.len())?,
            floor.entry + base,
            floor.exit + base,
        ];
    }
    let table = table
        .iter()
        .flatten()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    Ok([program, events, table])
}
const RECORD: usize = 8;
pub fn event_lookup(events: &[u8], floors: &[u8]) -> Result<Vec<u8>> {
    let ids = floors.len() / 8;
    let (mut rows, mut counts, mut starts, mut cells) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for floor in floors.chunks(8) {
        let from = usize::from(u16::from_le_bytes([floor[0], floor[1]]));
        let to = usize::from(u16::from_le_bytes([floor[2], floor[3]]));
        let records = events.get(from..to).ok_or("Floor event range")?;
        if records.len() % RECORD != 0 || records.len() / RECORD > 0x80 {
            return Err("Floor event records do not fit the lookup".into());
        }
        let records: Vec<&[u8]> = records.chunks(RECORD).collect();
        starts.extend(u16::try_from(cells.len())?.to_le_bytes());
        let mut count = 0u8;
        for y in 0..8u8 {
            let mut bits = 0u8;
            counts.push(count);
            for x in 0..8u8 {
                let at: Vec<usize> = (0..records.len())
                    .filter(|&i| records[i][0] == x && records[i][1] == y)
                    .collect();
                if let Some(&first) = at.first() {
                    bits |= 1 << x;
                    count += 1;
                    cells.push(first as u8 | if at.len() > 1 { 0x80 } else { 0 });
                }
            }
            rows.push(bits);
        }
    }
    if rows.len() != ids * 8 {
        return Err("Floor table size".into());
    }
    Ok([rows, counts, starts, cells].concat())
}
pub fn boundary(floors: &[Floor]) -> u8 {
    floors.iter().map(|f| f.id).max().unwrap_or(0) + 1
}
pub fn songs() -> Vec<u8> {
    (0..SONGS)
        .map(|i| crate::sfc_sound::source_song(i).map_or(0, |s| s + 1))
        .collect()
}
