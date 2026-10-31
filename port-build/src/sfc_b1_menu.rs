mod assets;
mod icons;
mod layout;
mod oam;
pub(crate) use oam::emit_compact;
mod runtime;
mod text;
use crate::sfc_b1_portrait::{EAT, EAT_TICKS, EAT_WAITING};
use crate::{Result, SOURCE, b1_dialogue::PageLayout, sha};
use icons::{pack_merging, rgb15, sheet, tiles};
pub use runtime::emit;
use serde::Deserialize;
use serde_json::Value;
use sfc_builder::cartridge::{CartridgeLayout, Section};
use std::collections::BTreeMap;
pub use text::text;
use text::{definition, message_text};
pub const MODE: u16 = 0x700;
const STRIP: u16 = 0x701;
const CURSOR: u16 = 0x702;
const COUNT: u16 = 0x703;
const MAIN_CURSOR: u16 = 0x704;
pub(crate) const SETTINGS: u16 = 0x705;
pub(crate) const SAMPLING_OFF: u8 = 1;
pub(crate) const BGM_OFF: u8 = 2;
pub(crate) const FAST_ANIMATION: u8 = 4;
const ITEM_PAGE: u16 = 0x706;
const HELD: u16 = 0x707;
const SCRATCH: u16 = 0x738;
const SLOT_PALETTES: u16 = 0x708;
const LIST: u16 = 0x710;
const CELLS: u16 = 0x718;
const SEQUENCE: u16 = 0x720;
const TIMER: u16 = 0x722;
pub use crate::sfc_battle::LEARNED;
pub const INVENTORY: u16 = 0x780;
pub const FIRST_ITEM: u8 = 40;
pub const ITEM_KINDS: usize = 58;
const STATS: usize = 2;
pub(crate) const OAM_BUFFER: u16 = 0x1000;
pub(crate) const OAM_USED: u16 = OAM_BUFFER + 544;
pub(crate) const STRIP_JOB: u16 = 0x1230;
pub(crate) const STRIP_LIST: u16 = 0x1240;
pub(crate) const ICON_NAME: u16 = 272;
pub(crate) const ICON_VRAM: u16 = 0xe000 + (ICON_NAME - 256) * 32;
const SLOTS: usize = 8;
const ICON_TILES: usize = 9;
const ICON_BYTES: usize = ICON_TILES * 32;
const DIGIT_NAME: u16 = ICON_NAME + (SLOTS * ICON_TILES) as u16;
pub(crate) const CORNER_NAME: u16 = DIGIT_NAME + 10;
const MARK_NAME: u16 = ICON_NAME - 1;
const MARK_VRAM: u16 = 0xe000 + (MARK_NAME - 256) * 32;
const _: () = assert!(MARK_NAME >> 8 == DIGIT_NAME >> 8);
const STATIC_VRAM: u16 = 0xe000 + (DIGIT_NAME - 256) * 32;
const ICON_STAGING: u32 = 0x7fc000;
const ICON_PALETTES: [u8; 2] = [0, 2];
const MARK_PALETTE: u8 = 3;
const BAR_Y: usize = 136;
const ICON: usize = 24;
const PITCH: usize = 26;
const DIGIT_SPRITES: usize = 4;
const ICON_SPRITES: usize = DIGIT_SPRITES + 2 * SLOTS;
const OAM_BYTES: usize = 544;
pub(crate) const MENU_SPRITES: usize = ICON_SPRITES + ICON_TILES * SLOTS;
const MAIN: u8 = 0;
const MAGIC: u8 = 1;
const ITEMS: u8 = 2;
const SETTINGS_STRIP: u8 = 3;
const SLOT_STRIP: u8 = 4;
const BATTLE_STRIP: u8 = 5;
const BATTLE_CELLS: [u16; 5] = [0, 1, 5, 3, 4];
const SHOP_STRIP: u8 = 6;
const SHOP_CODES: [u8; 5] = [6, 7, 8, 4, 5];
const SHOP_CELLS: [u16; 5] = [6, 7, 5, 3, 4];
const ESCAPE: u8 = 31;
const STATUS: u8 = 5;
const SLOT_ICONS: [u16; 4] = [13, 14, 15, 16];
const SAVE_MAPS: usize = 17;
const QUESTION_MODE: u8 = 4;
const SLOT_ACTION: u16 = 0x731;
const QUIT_ACTION: u8 = 2;
const QUIT_SLOT: u8 = 5;
const QUESTION_SLOT: u16 = 0x732;
const SLOT_PAGE: u16 = 0x733;
const FIRST_SETTING: u16 = 0x737;
const HOVER_ENTRY: u16 = 0x723;
const MAGIC_AVAILABLE: u16 = 0x724;
const DIACUTE_SLOT: u8 = 5;
const CHARGE: u16 = crate::sfc_battle::ACTORS + 0x3e;
const HELD_ITEMS: [u8; 58] = {
    let mut ids = [0; 58];
    let mut i = 0;
    while i < ids.len() {
        ids[i] = 40 + i as u8;
        i += 1;
    }
    ids
};
const START_INVENTORY: [(u8, u8); 4] = [(40, 3), (43, 2), (51, 1), (52, 1)];
const FIELD_ITEMS: [u8; 35] = {
    let mut ids = [52; 35];
    let mut i = 1;
    while i < ids.len() {
        ids[i] = 53 + i as u8;
        i += 1;
    }
    ids
};
const _: () = {
    let mut i = 0;
    while i < FIELD_ITEMS.len() {
        assert!(FIELD_ITEMS[i] - 31 >= 9);
        i += 1;
    }
};
const SPELL_COSTS: [u8; 8] = [0, 0, 0, 8, 20, 15, 25, 50];
const SETTINGS_ICONS: [(u16, u16, u8); 6] = [
    (9, 9, 0),
    (8, 8, 0),
    (10, 29, 1),
    (11, 30, 2),
    (27, 26, 4),
    (12, 12, 0),
];
const UNLEARNED: u16 = 19;
pub(crate) const EMPTY_PAGE: u8 = 2;
use crate::sfc_battle::MP;
const END: u8 = 0;
const PAGE: u8 = 1;
const SET: u8 = 3;
const SOUND: u8 = 5;
const CLOSE: u8 = 6;
const USE: u8 = 8;
const GROW: u8 = 9;
const BRANCH: u8 = 10;
const SAY: u8 = 11;
const STATUS_DATA: u8 = 12;
const STATUS_MESSAGE: u32 = 0x4b1376;
const USE_MESSAGE: u32 = 0x4b0268;
#[derive(Deserialize)]
struct Sheet {
    file: String,
    width: usize,
    height: usize,
    pixels_sha256: String,
    palette_sha256: String,
}
#[derive(Deserialize)]
struct ItemDrawing {
    icons: Vec<ItemIcon>,
    sheets: Vec<IconSheet>,
    quantity: Quantity,
}
#[derive(Deserialize)]
struct ItemIcon {
    item_id: u8,
    slot: u16,
    frame: u16,
}
#[derive(Deserialize)]
struct IconSheet {
    slot: u16,
    file: String,
    #[serde(default)]
    used_rectangles: Vec<UsedRectangle>,
}
#[derive(Deserialize)]
struct UsedRectangle {
    frame: u16,
    rectangle: [usize; 4],
}
#[derive(Deserialize, PartialEq)]
struct Quantity {
    minimum: u8,
    maximum: u8,
}
const SHOWN_QUANTITY: (u8, u8) = (2, 99);
const EQUIPPED: [u16; 2] = [0x1a, 0x1b];
#[derive(Deserialize)]
struct HudSheets {
    resources: Vec<Sheet>,
}
#[derive(Deserialize)]
struct Row {
    code: u16,
    icon: u16,
}
#[derive(Deserialize)]
struct StripSource {
    rows: Vec<Row>,
}
#[derive(Deserialize)]
struct Tokens {
    tokens: Vec<Value>,
}
#[derive(Deserialize)]
struct Step {
    kind: String,
    #[serde(default)]
    tokens: Vec<Value>,
    #[serde(default)]
    wait: Value,
    #[serde(flatten)]
    extra: serde_json::Map<String, Value>,
}
#[derive(Deserialize)]
struct Action {
    steps: Vec<Step>,
    #[serde(default)]
    follow_variants: Vec<Vec<Step>>,
}
#[derive(Deserialize)]
struct Definition {
    schema: u8,
    source_sha256: String,
    sheets: Vec<Sheet>,
    item_list_drawing: ItemDrawing,
    strips: BTreeMap<String, StripSource>,
    hover: BTreeMap<String, Tokens>,
    actions: BTreeMap<String, Action>,
    save_files: SaveFiles,
}
#[derive(Deserialize)]
struct SaveFiles {
    hover: Vec<String>,
    map_names: Vec<String>,
    empty: String,
    save_question: Vec<String>,
    load_question: Vec<String>,
    quit_question: Vec<String>,
}
#[derive(Deserialize)]
struct ItemSource {
    id: u8,
    description: Tokens,
}
#[derive(Deserialize)]
struct Items {
    items: Vec<ItemSource>,
}
#[derive(Deserialize)]
struct Growth {
    display_steps: u32,
}
enum Op {
    Page(usize, u8),
    Set(u16, u8),
    Sound(u8),
    Close,
    Use(u8),
    Grow,
    Unless(u16, String),
    Say(u32),
    StatusData,
}
struct Strip {
    palettes: Vec<u8>,
    cells: BTreeMap<u16, (Vec<u8>, u8)>,
}
pub struct Assets {
    strips: Vec<Strip>,
    mark_palette: Vec<u8>,
    static_tiles: Vec<u8>,
    mark_tile: Vec<u8>,
    pub pages: Vec<String>,
    pub slot_pages: Vec<String>,
    results: BTreeMap<String, Vec<Op>>,
    pub heal_follow: Vec<(Vec<String>, Vec<u8>)>,
    pub report: Value,
}
type Packed = (Vec<u8>, Vec<u8>, Vec<[u16; 2]>);
