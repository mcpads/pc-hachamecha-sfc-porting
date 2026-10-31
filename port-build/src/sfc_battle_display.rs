mod arle;
mod arle_effects;
mod assets;
mod boss_view;
mod layout;
mod palette;
mod runtime;
mod single_block;
mod sprites;
use crate::Result;
use crate::sfc::{emit, implied};
use crate::sfc_b1::{QUEUE, transfer};
use crate::sfc_battle_act::{abs, branch, byte, m8, m16, word};
use arle::emit_arle;
pub use assets::load;
use boss_view::emit_boss_view;
pub use layout::sections;
use palette::emit_palette;
pub use runtime::emit_rules;
use sfc_builder::cartridge::{CartridgeLayout, Section};
pub use single_block::place_code;
use sprites::emit_objects;
use std::collections::BTreeMap;
use w65c816::{AddressingMode as A, Assembler, CpuMode, Mnemonic as M, Operand as O};
pub const PROFILE_LABELS: &[&str] = &[
    "battle_display_tick",
    "bdo_draw",
    "bdo_each",
    "bdo_skip",
    "bdo_skip8",
    "bdo_resource",
    "bdo_sprite",
    "bdo_sprite_last",
    "bdo_sprite_ready",
    "bdo_sprite_draw",
    "bdo_place",
    "bdo_cache",
    "bdo_cache_none",
    "bdo_cache_name",
    "bdo_block_take",
    "bdo_block_next",
    "bdo_hop_7",
    "bdo_finish",
    "bdo_record",
    "battle_display_arle",
    "battle_display_objects",
    "bdo_group",
    "bdo_object",
    "bdo_full_next",
    "bdo_cull",
    "bdo_reverse",
    "bdo_sort",
    "bdo_sort_inner",
    "bdo_unuse",
    "battle_boss_check",
    "battle_display_palette",
    "bdo_mask",
    "bdo_fit_row",
    "bdo_fit_column",
    "bdo_fit_found",
    "bdo_cache_search",
    "bdo_cache_find",
    "bdo_cache_key",
    "bdo_cache_missing",
    "bdo_cache_free",
    "bdo_cache_entry",
    "bdo_free",
    "bdo_chunk",
    "bdo_chunk_blocks",
    "bdo_block",
    "bdo_cache_memo",
    "bdo_cache_fail",
];
type Panel = [i32; 4];
const LEFT: Panel = [64, 32, 8, 8];
const RIGHT: Panel = [352, 32, 144, 8];
fn screen_x(panel: Panel, x: i32) -> i32 {
    panel[2] + ((x - panel[0]) * 104).div_euclid(224)
}
fn screen_y(panel: Panel, y: i32) -> i32 {
    panel[3] + ((y - panel[1]) * 128).div_euclid(240)
}
fn scale(n: usize, to: usize, from: usize) -> usize {
    (n * to).div_ceil(from)
}
const COORD_FIRST: i32 = -256;
const COORDS: usize = 1024;
pub const DISPLAY: u16 = 0xd98;
const SETTING_UP: u8 = 1;
pub(crate) const RESTORING: u8 = 2;
const RUNNING: u8 = 3;
const ARLE_FRAME: u16 = 0xd80;
const ARLE_SHOWN: u16 = 0xd82;
const ARLE_X: u16 = 0xd84;
const ARLE_Y: u16 = 0xd86;
const ARLE_SHOWN_X: u16 = 0xd88;
const ARLE_SHOWN_Y: u16 = 0xd8a;
const BOSS_SENT: u16 = 0xd8c;
const BOSS_LINE: u16 = 0xd8e;
const BOSS_ON: u16 = 0xd90;
const OVERLAY_ROW: u16 = 0xd92;
const OVERLAY_NOT: u16 = 0xd94;
const OVERLAY_TILES: u16 = 0xd96;
const NO_BOSS: u16 = 0x8000;
const BOSS_STEP: u16 = 1;
const INDEX: u16 = 0xdd2;
const COUNT: u16 = 0xda1;
const HIGH: u16 = 0xda2;
const AT_X: u16 = 0xda4;
const AT_Y: u16 = 0xda6;
const ROW: u16 = 0xda8;
const COLUMN: u16 = 0xdaa;
const CELL: u16 = 0xdac;
const OBJECT: u16 = 0xdae;
const ENTRY: u16 = 0xdb0;
const VISIBLE: u16 = 0xdb2;
const CHUNK: u16 = 0xdb4;
const BLOCKS: u16 = 0xdb6;
const RESOURCE: u16 = 0xdb8;
const SHEET_FRAME: u16 = 0xdba;
const ORIGIN_X: u16 = 0xdbc;
const ORIGIN_Y: u16 = 0xdbe;
const PANEL_LEFT: u16 = 0xdc0;
const PANEL_RIGHT: u16 = 0xdc2;
const SLOT_FOUND: u16 = 0xdc4;
const DESCRIPTORS: u16 = 0xdc6;
const ARLE_SEEN: u16 = 0xdc8;
const ATTRIBUTE: u16 = 0xdca;
const DATA: u16 = 0xdcc;
const FIND_ONLY: u16 = 0xdd0;
const MISSED: u16 = 0xdd1;
const CHUNK_ATTRIBUTE: u16 = 0xe0c;
const LAST_DRAWN: u32 = 0x7f8000;
pub(crate) const ARLE_BANK: u16 = 0xdd4;
const ARLE_BASE: u16 = 0xdd6;
const CHUNK_NAMES: u16 = 0xdd8;
const CHUNK_NAMES_MAX: usize = 16;
const PREV_END: u16 = 0xdf8;
pub(crate) const PALETTE_CURRENT: u32 = 0x7fa000;
pub(crate) const PALETTE_KEPT: u32 = 0x7fa300;
pub(crate) const PALETTE_DIRTY: u16 = 0xdfa;
const PALETTE_BUFFER: u32 = 0x7fa600;
const PALETTE_LIST: u32 = PALETTE_BUFFER + 18 * 32;
const PALETTE_SLOT: u16 = 5;
const PALETTE_TABLE: u16 = 0xdfc;
const PALETTE_BLOCK: u16 = 0xdfe;
const PALETTE_SENT: u16 = 0xe00;
const HINTS: u32 = 0x7f9c00;
const KEY_COUNTS: u32 = 0x7ef400;
const FREED_HASH: u32 = KEY_COUNTS + 0x100;
const SLOT_TABLE: u32 = 0x7fb000;
pub const STREAM: u32 = 0x7fb080;
const HASH: u16 = 0xe02;
pub(crate) const BODIES_FROM: u16 = 0xe06;
const LAST_DRAWN_BYTES: u16 =
    crate::sfc_battle_objects::OBJECTS * crate::sfc_battle_objects::OBJECT_BYTES / 8;
const VISIBLE_LIST: u32 = 0x7ef100;
const VISIBLE_MAX: u16 = 48;
const SORTED: u32 = 0x7ef3a0;
const _: () = assert!(SORTED + VISIBLE_MAX as u32 * 2 <= 0x7ef400);
const Z_SPAN: u16 = 16;
const Z_COUNTS: u32 = 0x7ef1c0;
const Z_LOW: u32 = Z_COUNTS + Z_SPAN as u32 * 2;
const Z_HIGH: u32 = Z_LOW + 2;
const Z_FROM: u32 = Z_HIGH + 2;
const Z_PREVIOUS: u32 = Z_FROM + 2;
const Z_DESCENTS: u32 = Z_PREVIOUS + 2;
const Z_MOVES: u32 = Z_DESCENTS + 2;
const Z_TRY_COUNT: u32 = Z_MOVES + 2;
const Z_FEW_OBJECTS: u16 = 16;
const Z_FEW_DESCENTS: u16 = 3;
const _: () =
    assert!(VISIBLE_LIST + VISIBLE_MAX as u32 * 4 <= Z_COUNTS && Z_TRY_COUNT + 2 <= CACHE);
const CACHE: u32 = 0x7ef200;
const CACHE_ENTRIES: u16 = 40;
const CACHE_BYTES: u16 = 6;
const fn cache(k: u32) -> u32 {
    CACHE + k * CACHE_ENTRIES as u32
}
const ROWS_USED: u32 = 0x7ef300;
const MEMO_KEYS: u16 = 8;
const MEMO_RESOURCE: u32 = 0x7ef378;
const MEMO_FRAME: u32 = MEMO_RESOURCE + MEMO_KEYS as u32;
const MEMO_CHUNK: u32 = MEMO_FRAME + MEMO_KEYS as u32;
const MEMO_COUNT: u32 = MEMO_CHUNK + MEMO_KEYS as u32;
const MEMO_NEXT: u32 = MEMO_COUNT + 1;
const FIT_FAIL: u32 = MEMO_NEXT + 1;
const NO_ENTRY: u32 = FIT_FAIL + 1;
const KNOWN: u32 = NO_ENTRY + 1;
const _: () = assert!(KNOWN < 0x7ef400);
const UPLOAD_LIST: u32 = 0x7ef318;
const UPLOAD_MAX: u16 = 12;
const SHADOW: u32 = 0x7f7400;
const SHADOW_VRAM: u16 = 0xf000 + 64;
const RECORD: u8 = 0xf0;
pub const OBJECT_PAGE: u16 = 0x0d00;
pub const OBJECT_FRAME_POINTER: u16 = DISPLAY + 1;
const _: () = assert!(OBJECT_FRAME_POINTER + 3 <= COUNT);
pub(crate) const ARLE_BUFFERS: [[(u16, usize); 2]; 2] = [
    [(0xa000, 64), (crate::sfc_b1_text::ARLE_TAIL, 14)],
    [(0xb3c0, 66), (crate::sfc_b1_text::ARLE_TAIL + 14 * 32, 12)],
];
const _: () = assert!(
    crate::sfc_b1_text::ARLE_TAIL as usize + 32 * (14 + 12)
        <= crate::sfc_b1_text::BELOW_PORTRAIT as usize
);
const ARLE_TILES: usize = 78;
const _: () = assert!(
    ARLE_BUFFERS[0][0].1 + ARLE_BUFFERS[0][1].1 == ARLE_TILES
        && ARLE_BUFFERS[1][0].1 + ARLE_BUFFERS[1][1].1 == ARLE_TILES
);
const ARLE_PALETTE: u16 = 5;
const ARLE_COLORS: u16 = 80;
const ARLE_REST: (i32, i32) = (408, 80);
const PANEL_ROWS: std::ops::RangeInclusive<i32> = 1..=16;
const PANEL_COLUMNS: std::ops::RangeInclusive<i32> = 18..=30;
const BLANK: u16 = (crate::sfc_b1::TEXT_TILES - crate::sfc_b1::BG2_TILES) / 32;
const FIRST_ENTRY: u16 = crate::sfc_b1_menu::MENU_SPRITES as u16;
const ROWS: u16 = 8;
const SPRITE_VRAM: u16 = 0x8000;
pub(crate) const ARLE_SLOT: u16 = 3;
const SPRITE_SLOT: u16 = 4;
const SPRITE_BOTTOM: i32 = 134;
const SPRITE_TOP: i32 = 8;
const PALETTE_SHEETS: [usize; 4] = [0, 5, 6, 7];
const BATTLE_SHEETS: [usize; 8] = [0, 1, 2, 3, 4, 5, 6, 7];
const SLOTS_PER_TABLE: usize = 128;
pub const BACKDROP_COLORS: u8 = 48;
pub const SCENE_ID: usize = 0;
struct Chunk {
    offsets: Vec<[u8; 2]>,
    tops: Vec<u8>,
    bottoms: Vec<u8>,
    upper: bool,
}
struct TileFrame {
    columns: usize,
    rows: usize,
    align: (usize, usize),
    cells: Vec<u16>,
    tiles: Vec<u8>,
}
enum Frames {
    Sprites(Vec<Vec<Chunk>>),
    Tiles(Vec<TileFrame>),
}
struct Resource {
    file: String,
    slot: u8,
    rects: Vec<[usize; 4]>,
    used: u16,
    rgb_upper: Option<Vec<u8>>,
    anchors: Vec<(i32, i32)>,
    colors: u16,
    palette: Vec<u8>,
    source_base: u8,
    key: u8,
    rgb: Vec<u8>,
    frames: Frames,
}
enum StreamAction {
    Load { slot: u8, resource: u8, base: u8 },
    Release { slot: u8 },
}
pub struct Assets {
    resources: Vec<Resource>,
    arle_effect_resource: usize,
    streams: Vec<(u32, Vec<StreamAction>)>,
    reserved_streams: Vec<(u32, Vec<StreamAction>)>,
    backdrop_rgb: Vec<u8>,
    enemy_loads: Vec<Vec<(u8, usize)>>,
    boss: Option<BossView>,
    report: serde_json::Value,
}
struct BossView {
    slot: u8,
    frame: u16,
    enemies: Vec<u8>,
    rows: Vec<u8>,
    height: usize,
}
const BOSS_ROW: usize = 13 * 9;
impl Assets {
    pub fn report(&self) -> &serde_json::Value {
        &self.report
    }
}
