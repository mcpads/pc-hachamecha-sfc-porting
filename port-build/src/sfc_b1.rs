mod assets;
mod graphics;
mod layout;
mod runtime;
mod views;
use crate::{
    Result, SOURCE,
    b1::World,
    b1_dialogue::PageLayout,
    font::{DIALOGUE_LINE, DialogueFont},
    sha,
};
pub use assets::prepare;
use graphics::{dialogue_page, planar, scale_portrait, static_screen};
pub(super) use layout::{add, add_shared, transfer};
pub use runtime::emit;
use serde::{Deserialize, Serialize};
use sfc_builder::{
    cartridge::{CartridgeLayout, Section},
    tiles::{Attributes, Depth, IndexedImage, TileImage},
};
use std::collections::{BTreeMap, BTreeSet};
use views::{draws_of, picture_jobs_of, wall_variants};
pub const UI_TEXT: &str = "지하１층위 전진좌우 회전아래 뒤돌기Ａ 다음Ｂ 예 아니오북동남서";
#[derive(Deserialize)]
struct Placement {
    slot: usize,
    destination: [usize; 2],
    source: [usize; 4],
}
#[derive(Deserialize)]
struct Resource {
    slot: u16,
    resource: String,
    width: usize,
    height: usize,
    palette_base: u8,
    key: Option<u8>,
}
#[derive(Deserialize)]
struct Definition {
    schema: u8,
    source_sha256: String,
    source_viewport: [usize; 2],
    backdrop_rectangles: Vec<[usize; 4]>,
    resource_slots: Vec<u16>,
    placements: Vec<Placement>,
    resources: Vec<Resource>,
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
struct Draw {
    slot: u16,
    source: [usize; 4],
    destination: [usize; 4],
    transparent: bool,
}
pub(crate) const BOUNDARY_FLOOR: u8 = 17;
const FIELD_SONGS: [u64; 2] = [4, 5];
pub(crate) const DUNGEON: [usize; 4] = [8, 8, 104, 128];
const PORTRAIT: [usize; 4] = [144, 8, 104, 128];
const DUNGEON_COLUMNS: usize = DUNGEON[2] / 8;
const DUNGEON_ROWS: usize = DUNGEON[3] / 8;
pub(crate) const DUNGEON_TILE: usize = 1;
const STATIC_TILE: usize = DUNGEON_TILE + DUNGEON_COLUMNS * DUNGEON_ROWS;
const TEXT_CELLS: [usize; 4] = [1, 18, 30, 9];
pub(crate) const TEXT_PALETTE: usize = 7;
const TEXT_ORIGIN: [usize; 2] = [18, 148];
const TEXT_WIDTH: usize = 220;
const TEXT_LINES: usize = 5;
pub(crate) const TEXT_TILES: u16 = 0xc000;
pub(crate) const BG2_TILES: u16 = 0x6000;
const TEXT_MAP_VRAM: u16 = 0xf000;
const TEXT_NAME: u16 = (TEXT_TILES - BG2_TILES) / 32;
const OBJSEL: u8 = (0x8000u16 >> 14) as u8 | 2 << 3;
pub(crate) const BANDS: u16 = 0xd60;
pub(crate) const MENU_BANDS: [u16; 3] = [BANDS + 1, BANDS + 11, BANDS + 16];
const BAND_TABLE: [u8; 21] = [
    8, 3, 0, 0, 0, 126, 0x13, 0, 0x10, 0, 10, 3, 0, 0, 0, 80, 3, 0, 0, 0, 0,
];
const TEXT_COLORS: usize = 6;
const TEXT_SHADOW: u8 = 5;
const FIRST_BRICK: u8 = 6;
const DEFAULT_TEXT: u8 = 2;
pub(crate) const VIEW_COLORS: [[usize; 2]; 2] = [[48, 80], [192, 256]];
pub const QUEUE: u16 = 0x740;
pub const SHOWN_VIEW: u16 = 0x1640;
const QUEUE_SLOTS: u16 = 3 + crate::sfc_b1_portrait::MAX_UNITS as u16 + 3;
pub const QUEUE_BYTES: u16 = QUEUE_SLOTS * 5;
pub const MENU_STRIP_SLOT: u16 = (3 + crate::sfc_b1_portrait::MAX_UNITS as u16) * 5;
pub const MENU_OAM_SLOT: u16 = MENU_STRIP_SLOT + 5;
pub const FADE_SLOT: u16 = MENU_OAM_SLOT + 5;
const BUDGET: u16 = QUEUE + QUEUE_BYTES;
const GROUP_LEFT: u16 = 0x189a;
const GROUP_WAITING: u16 = 0x189c;
const MENU_HELD: u16 = 0xe08;
const PENDING: u16 = 0x6d0;
pub(crate) const JINGLE_WAIT: u8 = 0x80;
const PAGE_PAIRS: u8 = 0xf1;
pub(crate) const JINGLE_HOLD: u16 = 0x519;
use crate::sfc_b1_portrait::{IMPACT, IMPACT_TICKS};
pub const DESCRIPTOR_COST: u16 = 320;
pub const STREAM_BUDGET: u16 = 3800;
pub const TITLE_STREAM_BUDGET: u16 = 5400;
const ENTRY_MESSAGE: u32 = 0x49c270;
struct View {
    rows: [usize; DUNGEON_ROWS],
    palette: Vec<u8>,
}
pub struct Assets {
    frame: crate::sfc_b1_frame::Assets,
    stone: crate::sfc_b1_stone::Assets,
    equipment: crate::sfc_b1_equipment::Assets,
    portrait: crate::sfc_b1_portrait::Assets,
    title: crate::sfc_title::Assets,
    screen: StaticScreen,
    text_map: Vec<u8>,
    palette: Vec<u8>,
    portrait_palettes: Vec<Vec<u8>>,
    view_tiles: crate::sfc_b1_view::Assets,
    views: Vec<View>,
    backdrop_view: usize,
    records: Vec<usize>,
    floor_cells: Vec<Vec<u16>>,
    ray_bytes: Vec<u8>,
    view_tables: Vec<(Vec<u8>, u16)>,
    slot_zero: u8,
    text: crate::sfc_b1_text::Assets,
    field_messages: Vec<u8>,
    field_tables: [Vec<u8>; 3],
    picture_table: Vec<u8>,
    picture_songs: Vec<u8>,
    recolours: Vec<u8>,
    page_delays: Vec<u8>,
    page_audio: Vec<[u8; 2]>,
    page_pairs: Vec<[u8; 2]>,
    impact_page: u16,
    quake_root: u16,
    quake: Vec<u16>,
    hum: Vec<u16>,
    warp_hums: Vec<u16>,
    warp_quakes: Vec<(u16, u16, u8)>,
    smoke: crate::sfc_field_smoke::Assets,
    shards: crate::sfc_field_shards::Assets,
    flames: crate::sfc_field_flames::Assets,
    guardian: crate::sfc_field_guardian::Assets,
    shard_root: u16,
    encounters: crate::sfc_encounter::Floors,
    party_stats: Vec<u8>,
    enemy_records: Vec<u8>,
    battle_scripts: crate::sfc_battle_scripts::Scripts,
    battle_tables: crate::sfc_battle_act::Tables,
    battle_vm: crate::sfc_battle_vm::Tables,
    battle_messages: crate::sfc_battle_message::Compiled,
    battle_display: crate::sfc_battle_display::Assets,
    ending: crate::sfc_ending::Assets,
    shops: crate::sfc_shop::Assets,
    body_scripts: Vec<u8>,
    pub image_unread: Vec<(usize, usize)>,
    pub image_reached: Vec<(usize, usize)>,
    body_roots: [u32; crate::sfc_battle_scripts::DESCRIPTORS],
    menu: crate::sfc_b1_menu::Assets,
    menu_first_page: u8,
    slot_first: u16,
    map: crate::sfc_b1_map::Assets,
    map_first_view: usize,
    sound: crate::sfc_sound::Assets,
    pub report: serde_json::Value,
}
impl Assets {
    pub fn text_reference(&self) -> Vec<u8> {
        self.text.reference.clone()
    }
}
struct StaticScreen {
    frame_tiles: Vec<u8>,
    portrait_tiles: Vec<Vec<u8>>,
    portrait_first: usize,
    map: Vec<u8>,
    battle_map: Vec<u8>,
    equipment_first: usize,
}
pub(crate) const VIEW_LIST: u32 = 0x7f7800;
pub(crate) const VIEW_JOB: u32 = 0x7f7840;
const VIEW_LIST_CODE: usize = 0x100;
struct PictureJob {
    state: [u8; 4],
    entries: Vec<u32>,
    shift: [i32; 2],
    key: [u8; 2],
}
#[derive(Clone, Deserialize)]
struct WallLoad {
    slot: u16,
    file: String,
    palette_base: u8,
    key: Option<u8>,
    width: usize,
    height: usize,
}
#[derive(Deserialize)]
struct WallFloor {
    floor: u8,
    loads: Vec<WallLoad>,
}
#[derive(Deserialize)]
struct WallSource {
    floors: Vec<WallFloor>,
}
struct FloorView {
    id: u8,
    map: crate::b1::World,
    sheets: BTreeMap<u16, (WallLoad, Vec<u8>)>,
    changes: Vec<crate::floor_events::WallChange>,
    choices: Vec<crate::floor_events::WallChoice>,
    markers: Vec<(u8, u8)>,
    palette: Vec<u8>,
}
