mod assets;
mod layout;
mod publication;
mod runtime;
mod script;
use crate::Result;
use crate::sfc::{emit, implied, write};
use crate::sfc_battle_act::{abs, branch, byte, call, jump, m8, m16, word};
pub use assets::{prepare, wrap};
pub use publication::emit_field;
use publication::emit_nmi_rules;
pub use runtime::emit_rules;
pub use script::thin;
use serde_json::Value;
use sfc_builder::cartridge::{CartridgeLayout, Section};
use std::collections::BTreeMap;
use w65c816::{AddressingMode as A, Assembler, CpuMode, Mnemonic as M, Operand as O};
pub const PROFILE_LABELS: &[&str] = &[
    "ending_frame",
    "ending_tick",
    "ending_compose",
    "ending_group",
    "ending_object",
    "ending_sprite",
    "ending_piece",
    "ending_bucket",
    "ending_hidden",
    "ending_high_store",
    "ending_colours_go",
    "ending_colour_block",
    "ending_bgr",
    "ending_colour_copy",
    "ending_colours_done",
    "ending_stream",
];
pub const SCENE: u8 = 2;
pub const DESCRIPTOR: u32 = 0x4b88b8;
pub const ROOT: u32 = 0x4b8b30;
pub const RESERVED: u16 = crate::sfc_battle_objects::PENDING_SCENE;
pub const BATTLE_SCENES: [u32; 1] = [0x4ab5a8];
pub(crate) const DP: u16 = 0xf00;
pub const ACTIVE: u16 = DP;
pub const FILL: u16 = DP + 0x01;
const STAGE: u8 = 0x03;
pub const LAYOUT_WANTED: u16 = DP + 0x04;
const LAYOUT: u8 = 0x05;
const PICTURE: u8 = 0x06;
const EYES: u8 = 0x07;
const EYES_SHOWN: u8 = 0x08;
const LAYER: u8 = 0x09;
const BG1_V: u8 = 0x0a;
const BG2_H: u8 = 0x0c;
const BG2_V: u8 = 0x0e;
const SCREEN: u8 = 0x10;
const SCREEN_LAYOUT: u8 = 0x11;
const OAM_END: u8 = 0x12;
const T0: u8 = 0x14;
const T1: u8 = 0x16;
const T2: u8 = 0x18;
const PX: u8 = 0x1c;
const PY: u8 = 0x1e;
const TEXT_SERIAL: u8 = 0x20;
const TEXT_DONE: u8 = 0x21;
const TEXT_QUEUED: u8 = 0x22;
const TEXT_FOR: u8 = 0x23;
pub(crate) const PEN_X: u8 = 0x24;
pub(crate) const LINE_Y: u8 = 0x26;
pub(crate) const INK: u8 = 0x28;
const DRAWING: u8 = 0x29;
const CODE: u8 = 0x2a;
pub(crate) const GLYPH: u8 = 0x2d;
pub(crate) const TILE: u8 = 0x44;
const PTR: u8 = 0x4a;
const OBJ: u8 = 0x4e;
const N: u8 = 0x9a;
const SX: u8 = 0x8c;
const SY: u8 = 0x8e;
const PIECES: u8 = 0x90;
const GROUP_AT: u8 = 0x92;
const TAIL: u8 = 0x94;
const BUCKET: u8 = 0x96;
const DIRTY: u8 = 0x98;
const RPTR: u8 = 0x9c;
const STREAM_AT: u8 = 0xa0;
const REMAIN: u8 = 0xa2;
const LEFT_IN: u8 = 0xaa;
const HIGH: u8 = 0xb8;
const HIGH_LEFT: u8 = 0xb9;
const HIGH_AT: u8 = 0xbc;
const PENDING: u8 = 0xbe;
const BLOCK: u8 = 0xc0;
const REMAP: u8 = 0xc2;
const COPY: u8 = 0xc4;
pub const STOP: u16 = DP + 0xae;
const SONG: u16 = DP + 0xaf;
pub const SCREEN_WHITE: u16 = DP + 0xc6;
pub const SCREEN_KEPT: u16 = DP + 0xda;
pub const LINES: u16 = DP + 0xb0;
pub const FRAMES: u16 = DP + 0xd0;
const UNDRAWN: u16 = DP + 0xd8;
pub const UPDATES: u16 = DP + 0xac;
const SHEETS: u8 = 0x50;
const FIRST_SLOT: u8 = 38;
const LAST_SLOT: u8 = 54;
const MAP: u8 = 0x62;
const ALIAS: u8 = 0x72;
const FILL_SENT: u8 = 0x82;
const CGRAM: u32 = 0x7fd000;
const APPLIED: u32 = 0x7fd600;
const OBJECT_LIST: u32 = 0x7fd200;
const BUCKETS: u32 = 0x7fd400;
pub(crate) const TEXT_BUFFER: u32 = 0x7fe000;
const TEXT_BYTES: usize = 6 * 32 * 32;
const SLOT_OAM: u16 = 5;
const SLOT_CGRAM: u16 = 10;
const SLOT_EYES: u16 = 15;
const SLOT_TEXT: u16 = 20;
const SLOT_LOAD: u16 = 25;
pub const LOAD_COUNT: u16 = crate::sfc_b1::QUEUE + SLOT_LOAD + 3;
const TEXT_ROW: usize = 22;
const TEXT_PEN_X: usize = 20;
const TEXT_PEN_Y: usize = 7;
const TEXT_WIDTH: usize = 256 - TEXT_PEN_X;
const LINE: usize = crate::font::DIALOGUE_LINE;
const A_BG1_MAP: u16 = 0xf800;
const A_TEXT_MAP: u16 = 0xd800;
const A_OBJ: u16 = 0xc000;
const A_OBJ_NAMES: usize = 192;
const B_BG1_MAP: u16 = 0x1000;
const B_BG2_TILES: u16 = 0x2000;
const B_BG2_MAP: u16 = 0x5000;
const B_OBJ: u16 = 0x8000;
const B_OBJ_NAMES: usize = 512;
const B_TEXT_MAP: u16 = 0xf000;
const TEXT_TILES: u16 = 0xe000;
const CHUNK: usize = 4096;
#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    PictureA = 1,
    PictureB = 2,
    Eyes = 3,
    Layer = 4,
    Sprite = 5,
}
struct Sheet {
    width: usize,
    pixels: Vec<u8>,
    rgb: Vec<u8>,
}
struct Load {
    va: u32,
    slot: u8,
    sheet: u8,
    layout: u8,
    uploads: Vec<(Vec<u8>, u16)>,
    colours: Vec<u8>,
    base: u8,
    blocks: (u8, u8, u8),
    alias: u8,
}
type Pieces = Vec<(i16, i16, u8, u8, bool)>;
enum Frames {
    None,
    Sprite(Vec<Pieces>),
    Eyes(Vec<Vec<(u16, Vec<u8>)>>),
}
pub struct Assets {
    loads: Vec<Load>,
    sheets: Vec<(Kind, Frames)>,
    text_maps: [Vec<u8>; 2],
    text_colours: [[u8; 3]; 2],
    pub report: Value,
}
fn dpo(p: &mut Assembler, m: M, at: u8) {
    emit(p, m, A::DirectPage, O::Byte(at));
}
fn long(p: &mut Assembler, m: M, at: u32) {
    emit(p, m, A::AbsoluteLong, O::Long(at));
}
fn long_x(p: &mut Assembler, m: M, at: u32) {
    emit(p, m, A::AbsoluteLongX, O::Long(at));
}
fn ax(p: &mut Assembler, m: M, at: u16) {
    emit(p, m, A::AbsoluteX, O::Word(at));
}
fn ay(p: &mut Assembler, m: M, at: u16) {
    emit(p, m, A::AbsoluteY, O::Word(at));
}
fn ind(p: &mut Assembler, m: M, at: u8) {
    emit(p, m, A::DirectPageIndirectLongIndexedY, O::Byte(at));
}
fn acc(p: &mut Assembler, m: M) {
    emit(p, m, A::Accumulator, O::None);
}
fn far_branch(p: &mut Assembler, m: M, label: &str, wide: bool) {
    if m == M::Bra {
        p.emit_label_ref(M::Jmp, A::Absolute, label);
        return;
    }
    static SITES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let site = SITES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let skip = format!("{label}_skip_{site}");
    let inverse = match m {
        M::Beq => M::Bne,
        M::Bne => M::Beq,
        M::Bcc => M::Bcs,
        M::Bcs => M::Bcc,
        M::Bpl => M::Bmi,
        M::Bmi => M::Bpl,
        _ => unreachable!(),
    };
    p.emit_label_ref(inverse, A::Relative8, &skip);
    p.emit_label_ref(M::Jmp, A::Absolute, label);
    p.label(skip).assume_mode(if wide {
        CpuMode::NATIVE_M16_X16
    } else {
        CpuMode::NATIVE_M8_X16
    });
}
fn enqueue(p: &mut Assembler, at: u32, slot: u16) {
    for i in 0..5u32 {
        long(p, M::Lda, at + i);
        abs(p, M::Sta, crate::sfc_b1::QUEUE + slot + i as u16);
    }
}
const ORB_EVERY: usize = 8;
const GEM_EVERY: usize = 32;
pub(crate) const THIN_SPACE: u32 = 0x4ba400;
pub(crate) const THIN_END: u32 = 0x4bb800;
