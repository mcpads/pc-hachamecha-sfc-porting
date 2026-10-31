mod commands;
mod motion;
mod palette;
mod scheduler;
mod script_image;
mod support;
use commands::emit_commands;
pub use motion::sine_digits;
use motion::{emit_toward, emit_trig, emit_variants};
use palette::emit_palette;
use scheduler::{emit_controller_side, emit_pool, emit_scheduler};
pub use script_image::{reached, sections, specialized, thinned, thinning_report, unread};
use support::emit_support;
mod blocks;
pub(crate) mod bound_child_copy;
pub(crate) mod common_batch;
pub(crate) mod common_birth;
pub(crate) mod common_blink;
pub(crate) mod common_blink_density;
pub(crate) mod common_cohort;
pub(crate) mod common_density;
pub(crate) mod common_radial;
pub(crate) mod falling_birth;
pub(crate) mod falling_density;
pub(crate) mod falling_setup;
mod falling_turn;
pub(crate) mod hidden_child;
mod initializers;
pub(crate) mod inward_projection;
mod inward_turn;
mod jugem_density;
mod lightning_density;
pub(crate) mod lightning_radius;
pub(crate) mod lightning_turn;
mod linked_velocity;
mod orbit_turn;
mod phase_table;
mod radial_branch;
mod radial_motion;
mod radial_velocity;
mod radius_loop;
mod vertical_motion;
pub fn compiled_initializers_report() -> serde_json::Value {
    initializers::report()
}
pub fn inward_projection_report(layout: &CartridgeLayout) -> Result<serde_json::Value> {
    inward_projection::report(layout)
}
pub fn inward_turn_report() -> serde_json::Value {
    inward_turn::report()
}
pub fn jugem_density_report() -> serde_json::Value {
    jugem_density::report()
}
pub fn falling_density_report(layout: &CartridgeLayout) -> Result<serde_json::Value> {
    falling_density::report(layout)
}
pub fn common_blink_density_report(layout: &CartridgeLayout) -> Result<serde_json::Value> {
    common_blink_density::report(layout)
}
pub fn lightning_radius_report(layout: &CartridgeLayout) -> Result<serde_json::Value> {
    lightning_radius::report(layout)
}
pub fn lightning_density_report() -> serde_json::Value {
    lightning_density::report()
}
pub fn common_density_report(layout: &CartridgeLayout) -> Result<serde_json::Value> {
    common_density::report(layout)
}
pub fn common_batch_report(layout: &CartridgeLayout) -> Result<serde_json::Value> {
    common_batch::report(layout)
}
pub fn common_blink_report(layout: &CartridgeLayout) -> Result<serde_json::Value> {
    common_blink::report(layout)
}
pub fn common_radial_report(layout: &CartridgeLayout) -> Result<serde_json::Value> {
    common_radial::report(layout)
}
pub fn bound_child_copy_report(layout: &CartridgeLayout) -> Result<serde_json::Value> {
    bound_child_copy::report(layout)
}
pub fn falling_setup_report(layout: &CartridgeLayout) -> Result<serde_json::Value> {
    falling_setup::report(layout)
}
pub fn falling_birth_report(layout: &CartridgeLayout) -> Result<serde_json::Value> {
    falling_birth::report(layout)
}
pub fn falling_turn_report() -> serde_json::Value {
    falling_turn::report()
}
pub fn orbit_turn_report() -> serde_json::Value {
    orbit_turn::report()
}
pub fn linked_velocity_report() -> serde_json::Value {
    linked_velocity::report()
}
pub fn radial_velocity_report() -> serde_json::Value {
    radial_velocity::report()
}
pub fn vertical_motion_report() -> serde_json::Value {
    vertical_motion::report()
}
pub fn vertical_particle_retirements() -> &'static [(usize, u32, u32)] {
    &vertical_motion::RETIREMENTS
}
pub fn compiled_blocks_report() -> serde_json::Value {
    blocks::report()
}
use crate::Result;
use crate::sfc::{emit, implied};
use sfc_builder::cartridge::{CartridgeLayout, Section};
use std::collections::BTreeSet;
use w65c816::{AddressingMode as A, Assembler, CpuMode, Mnemonic as M, Operand as O};
pub const SOURCE_BASE: u32 = 0x428000;
pub const SOURCE_END: u32 = 0x4be000;
pub const IMAGE_OFFSET: usize = 0x368000;
const _: () = assert!(IMAGE_OFFSET & 0xffff == (SOURCE_BASE & 0xffff) as usize);
const _: () = assert!(IMAGE_OFFSET + (SOURCE_END - SOURCE_BASE) as usize <= 0x400000);
pub const OBJECT_BYTES: u16 = 176;
pub const OBJECTS: u16 = 300;
pub const POOL: u16 = 0x2000;
pub const SOURCE_POOL: u32 = 0x4c4770;
pub const SCENE_ROOT: u32 = 0x47eb58;
const SOURCE_DELTA: u32 = SOURCE_POOL - POOL as u32;
pub const PENDING_SCENE: u16 = 0xd1c;
pub const SENTINELS: u16 = 0xef00;
const GROUPS: u16 = 5;
pub const FREE: u16 = 0xefc0;
pub const SLOTS: u16 = 0xf000;
const LAST_HANDLE: u16 = 0xf0a0;
const SAVED_HANDLES: u16 = 0xf0a2;
const COMMAND_LIMIT: u16 = 400;
const EFFECTS: u64 = 64;
pub const FAULTS: u16 = 0xf0e4;
const DP: u16 = 0x1900;
const PCP: u8 = 0x00;
const PTR: u8 = 0x03;
const ROMP: u8 = 0x06;
const VALID: u8 = 0x0a;
const BUDGET: u8 = 0x0b;
const LHS: u8 = 0x10;
const SRC: u8 = 0x14;
const RES: u8 = 0x18;
const TMP: u8 = 0x1c;
const WIDTH: u8 = 0x20;
const MODE: u8 = 0x21;
const COND: u8 = 0x22;
const SIGN: u8 = 0x23;
const GROUP: u8 = 0x26;
const SAVE: u8 = 0x28;
const OBJ: u8 = 0x2c;
const COUNT: u8 = 0x2e;
const WORK: u8 = 0x30;
const QUO: u8 = 0x38;
const REM: u8 = 0x3c;
const TAIL: u8 = 0x40;
const AXIS: u8 = 0x42;
const RW: u8 = 0x44;
const REFFAST: u8 = 0x45;
const DX: u8 = 0x48;
const DY: u8 = 0x4c;
const QX: u8 = 0x50;
const QY: u8 = 0x54;
const ATA: u8 = 0x58;
const ATB: u8 = 0x5c;
const OFF: u8 = 0x67;
const REFSRC: u8 = 0x68;
const REFBASE: u8 = 0x6c;
const REFOK: u8 = 0x6e;
const REFSPLIT: u8 = 0x6f;
pub const SPAWN_PC: u16 = DP + 0x70;
const SPAWN_GROUP: u8 = 0x74;
pub const IN_UPDATE: u16 = DP + 0x7f;
pub const LAST_BODY: u16 = DP + WORK as u16;
fn places() -> [(u32, u32, u16); 8] {
    [
        (
            0x4b4848,
            crate::sfc_battle::STATS_BYTES as u32,
            crate::sfc_battle::STATS,
        ),
        (SOURCE_POOL, OBJECTS as u32 * OBJECT_BYTES as u32, POOL),
        (0x4c43c0, 4 * 0x68, crate::sfc_battle::ACTORS),
        (0x4c4260, 0xc0, 0x800),
        (0x4c4330, 0x90, SLOTS),
        (0x4c46a0, 0x40, 0x8c0),
        (0x4b47c0, 8, 0x970),
        (
            0x4b47c8,
            crate::sfc_b1_menu::ITEM_KINDS as u32,
            crate::sfc_b1_menu::INVENTORY,
        ),
    ]
}
pub const PROFILE_LABELS: &[&str] = &[
    "battle_display_end",
    "battle_display_unhide",
    "bvm_screen_range",
    "bvm_screen_read",
    "bvm_screen_write",
    "bvm_screen_keep",
    "bvm_screen_restore",
    "bvm_screen_restore_fade",
    "bvm_af_done",
    "bvm_02_count",
    "bvm_02_done",
    "bvm_06_end",
    "bvm_06_new",
    "bvm_07_none",
    "bvm_08_dword",
    "bvm_08_end",
    "bvm_08_row",
    "bvm_08_skip",
    "bvm_08_word",
    "bvm_0a_index",
    "bvm_24_done",
    "bvm_27_done",
    "bvm_27_restore",
    "bvm_28_bind",
    "bvm_28_done",
    "bvm_32_bit",
    "bvm_32_no",
    "bvm_32_shift",
    "bvm_5b_done",
    "bvm_5b_mask",
    "bvm_67_wait",
    "bvm_80_fault",
    "bvm_81_done",
    "bvm_81_down",
    "bvm_81_write",
    "bvm_a2_no",
    "bvm_a_mul",
    "bvm_a_neg",
    "bvm_a_not",
    "bvm_a_set",
    "bvm_actor_offset",
    "bvm_alloc",
    "bvm_alloc_some",
    "bvm_arith",
    "bvm_at",
    "bvm_at_bound",
    "bvm_at_done",
    "bvm_at_not_self",
    "bvm_atan2",
    "bvm_atan2_done",
    "bvm_atan2_negc",
    "bvm_atan2_ok",
    "bvm_atan2_xneg",
    "bvm_atan_core",
    "bvm_atan_core_a",
    "bvm_atan_core_b",
    "bvm_atan_core_done",
    "bvm_atan_core_index",
    "bvm_atan_core_less",
    "bvm_atan_core_nv",
    "bvm_atan_core_table",
    "bvm_bind",
    "bvm_bind_done",
    "bvm_bind_new",
    "bvm_compare",
    "bvm_compare_ne",
    "bvm_delete",
    "bvm_destination",
    "bvm_destination_width",
    "bvm_divide",
    "bvm_divide_bit",
    "bvm_divide_less",
    "bvm_divide_some",
    "bvm_fetch",
    "bvm_idiv",
    "bvm_idiv_done",
    "bvm_idiv_ok",
    "bvm_multiply",
    "bvm_multiply_bit",
    "bvm_multiply_skip",
    "bvm_next",
    "bvm_op_2d",
    "bvm_op_80",
    "bvm_operands",
    "bvm_operands_done",
    "bvm_operands_place",
    "bvm_operands_read",
    "bvm_operands_wide",
    "bvm_operands_width",
    "bvm_ops",
    "bvm_read",
    "bvm_read_done",
    "bvm_read_dword",
    "bvm_register",
    "bvm_rel_eq",
    "bvm_rel_ge",
    "bvm_rel_gt",
    "bvm_rel_gt_no",
    "bvm_rel_le",
    "bvm_rel_le_no",
    "bvm_rel_le_yes",
    "bvm_rel_lt",
    "bvm_rel_ne",
    "bvm_rel_nz",
    "bvm_rel_nz_no",
    "bvm_reset",
    "bvm_reset_clear",
    "bvm_reset_free",
    "bvm_reset_slots",
    "bvm_rom",
    "bvm_run",
    "bvm_run_done",
    "bvm_product",
    "bvm_radial_compare",
    "bvm_2d_axes",
    "bvm_2d_loaded",
    "bvm_2d_xy",
    "bvm_2d_generic",
    "bvm_phase_y",
    "bvm_spawn",
    "bvm_spawn_none",
    "bvm_store",
    "bvm_store_truncated",
    "bvm_sweep",
    "bvm_sweep_done",
    "bvm_sweep_next",
    "bvm_sweep_object",
    "bvm_translate",
    "bvm_unknown",
    "bvm_unslot",
    "bvm_unslot_none",
    "bvm_update",
    "bvm_write",
    "bvm_write_done",
    "bvm_write_wide",
];
const VARIANTS: [(u8, u8, Option<u8>, u8); 27] = [
    (0x12, 0x80, Some(3), 0xaf),
    (0x13, 0xc0, None, 0xb0),
    (0x13, 0x80, None, 0xb1),
    (0x14, 0xc0, None, 0xb2),
    (0x14, 0x80, None, 0xb3),
    (0x12, 0x80, Some(0), 0xb4),
    (0x12, 0x80, Some(1), 0xb5),
    (0x12, 0x80, Some(2), 0xb6),
    (0x11, 0x80, Some(0), 0xb7),
    (0x11, 0x80, Some(1), 0xb8),
    (0x11, 0x80, Some(2), 0xb9),
    (0x10, 0xc0, Some(0), 0xba),
    (0x10, 0xc0, Some(6), 0xbb),
    (0x10, 0xc0, Some(7), 0xbc),
    (0x12, 0xe0, Some(0), 0xbd),
    (0x11, 0xa0, Some(0), 0xbe),
    (0x11, 0xa0, Some(1), 0xbf),
    (0x11, 0xa0, Some(2), 0xc0),
    (0x12, 0xa0, Some(0), 0xc1),
    (0x12, 0xa0, Some(1), 0xc2),
    (0x12, 0xa0, Some(2), 0xc3),
    (0x11, 0xb0, Some(0), 0xf1),
    (0x11, 0xb0, Some(1), 0xf2),
    (0x11, 0xb0, Some(2), 0xf3),
    (0x12, 0xb0, Some(0), 0xf4),
    (0x12, 0xb0, Some(1), 0xf5),
    (0x12, 0xb0, Some(2), 0xf6),
];
const COMPARES: u8 = 0xc4;
const COMPARE_RELATIONS: u8 = 7;
const fn compare_variant(command: u8, own: bool, relation: u8) -> u8 {
    COMPARES + ((command - 0x13) * 2 + own as u8) * COMPARE_RELATIONS + relation
}
const _: () = assert!(compare_variant(0x15, true, COMPARE_RELATIONS - 1) < 0xf0);
const THINNING: [(u32, [u8; 4], [u8; 4]); 5] = [
    (0x490b8c, [1, 0, 0, 0], [2, 0, 2, 0]),
    (0x490b98, [6, 0, 0x14, 0], [6, 0, 0x0a, 0]),
    (0x491678, [0x60, 0, 0, 0], [0xc0, 0, 0, 0]),
    (0x49167c, [2, 0, 1, 0], [2, 0, 2, 0]),
    (0x491680, [6, 0, 0x10, 0], [6, 0, 0x08, 0]),
];
const SCENE_THINNING: [(u32, [u8; 4], [u8; 4]); 35] = [
    (0x482184, [0x02, 0x00, 0x01, 0x00], [0x02, 0x00, 0x02, 0x00]),
    (0x482188, [0x06, 0x00, 0x20, 0x00], [0x06, 0x00, 0x10, 0x00]),
    (0x4822b4, [0x02, 0x00, 0x01, 0x00], [0x02, 0x00, 0x04, 0x00]),
    (0x4822b8, [0x06, 0x00, 0x20, 0x00], [0x06, 0x00, 0x08, 0x00]),
    (0x4824a8, [0x00, 0x08, 0x00, 0x00], [0x00, 0x10, 0x00, 0x00]),
    (0x4824b0, [0x00, 0x00, 0x10, 0x00], [0x00, 0x00, 0x20, 0x00]),
    (0x4824b4, [0x02, 0x00, 0x02, 0x00], [0x02, 0x00, 0x04, 0x00]),
    (0x4824b8, [0x06, 0x00, 0x10, 0x00], [0x06, 0x00, 0x08, 0x00]),
    (0x482a64, [0x01, 0x00, 0x00, 0x00], [0x02, 0x00, 0x02, 0x00]),
    (0x482a68, [0x06, 0x00, 0x20, 0x00], [0x06, 0x00, 0x10, 0x00]),
    (0x482ad0, [0x01, 0x00, 0x00, 0x00], [0x02, 0x00, 0x02, 0x00]),
    (0x482ad4, [0x06, 0x00, 0x20, 0x00], [0x06, 0x00, 0x10, 0x00]),
    (0x48214c, [0x07, 0x01, 0x00, 0x00], [0x03, 0x00, 0x00, 0x00]),
    (0x482150, [0x94, 0x21, 0x48, 0x00], [0x84, 0x21, 0x48, 0x00]),
    (0x482284, [0x07, 0x01, 0x00, 0x00], [0x03, 0x00, 0x00, 0x00]),
    (0x482288, [0xc4, 0x22, 0x48, 0x00], [0xb4, 0x22, 0x48, 0x00]),
    (0x482a2c, [0x07, 0x01, 0x00, 0x00], [0x03, 0x00, 0x00, 0x00]),
    (0x482a30, [0xd4, 0x2c, 0x48, 0x00], [0x64, 0x2a, 0x48, 0x00]),
    (0x482a88, [0x07, 0x01, 0x00, 0x00], [0x03, 0x00, 0x00, 0x00]),
    (0x482a8c, [0x80, 0x2d, 0x48, 0x00], [0xd0, 0x2a, 0x48, 0x00]),
    (0x481ed4, [0x06, 0x00, 0x20, 0x00], [0x06, 0x00, 0x08, 0x00]),
    (0x481ec8, [0xff, 0x07, 0x00, 0x00], [0xfc, 0x1f, 0x00, 0x00]),
    (0x481ed0, [0xff, 0x07, 0x00, 0x00], [0xfc, 0x1f, 0x00, 0x00]),
    (0x481f1c, [0x06, 0x00, 0x20, 0x00], [0x06, 0x00, 0x08, 0x00]),
    (0x481f10, [0xff, 0x07, 0x00, 0x00], [0xfc, 0x1f, 0x00, 0x00]),
    (0x481f18, [0xff, 0x07, 0x00, 0x00], [0xfc, 0x1f, 0x00, 0x00]),
    (0x481f64, [0x06, 0x00, 0x20, 0x00], [0x06, 0x00, 0x08, 0x00]),
    (0x481f58, [0xff, 0x07, 0x00, 0x00], [0xfc, 0x1f, 0x00, 0x00]),
    (0x481f60, [0xff, 0x07, 0x00, 0x00], [0xfc, 0x1f, 0x00, 0x00]),
    (0x481fac, [0x06, 0x00, 0x20, 0x00], [0x06, 0x00, 0x08, 0x00]),
    (0x481fa0, [0xff, 0x07, 0x00, 0x00], [0xfc, 0x1f, 0x00, 0x00]),
    (0x481fa8, [0xff, 0x07, 0x00, 0x00], [0xfc, 0x1f, 0x00, 0x00]),
    (0x481ff4, [0x06, 0x00, 0x20, 0x00], [0x06, 0x00, 0x08, 0x00]),
    (0x481fe8, [0xff, 0x07, 0x00, 0x00], [0xfc, 0x1f, 0x00, 0x00]),
    (0x481ff0, [0xff, 0x07, 0x00, 0x00], [0xfc, 0x1f, 0x00, 0x00]),
];
const LATE_THINNING: [(u32, [u8; 4], [u8; 4]); 36] = [
    (0x42de38, [0x07, 0x01, 0x00, 0x00], [0x03, 0x00, 0x00, 0x00]),
    (0x42de3c, [0xe4, 0xde, 0x42, 0x00], [0x6c, 0xde, 0x42, 0x00]),
    (0x42deb0, [0x07, 0x01, 0x00, 0x00], [0x03, 0x00, 0x00, 0x00]),
    (0x42deb4, [0xe4, 0xde, 0x42, 0x00], [0xc8, 0xde, 0x42, 0x00]),
    (0x49f70c, [0xff, 0x01, 0x00, 0x00], [0xfc, 0x07, 0x00, 0x00]),
    (0x49f714, [0xff, 0x01, 0x00, 0x00], [0xfc, 0x07, 0x00, 0x00]),
    (0x49f718, [0x06, 0x00, 0x80, 0x00], [0x06, 0x00, 0x20, 0x00]),
    (0x46b18c, [0x06, 0x00, 0x90, 0x01], [0x06, 0x00, 0x64, 0x00]),
    (0x46b2e8, [0x06, 0x00, 0x90, 0x01], [0x06, 0x00, 0x64, 0x00]),
    (0x4bc82c, [0x06, 0x00, 0x96, 0x00], [0x06, 0x00, 0x26, 0x00]),
    (0x4bcb78, [0x06, 0x00, 0x14, 0x00], [0x06, 0x00, 0x05, 0x00]),
    (0x46b444, [0x06, 0x00, 0x96, 0x00], [0x06, 0x00, 0x4b, 0x00]),
    (0x495498, [0x06, 0x00, 0x10, 0x00], [0x06, 0x00, 0x08, 0x00]),
    (GHOST, [0; 4], [0x2b, 0x00, 0xff, 0xff]),
    (GHOST + 4, [0; 4], [0x2b, 0x00, 0xff, 0xff]),
    (GHOST + 8, [0; 4], [0x2b, 0x00, 0xff, 0xff]),
    (0x46b378, [0xb4, 0xb3, 0x46, 0x00], [0x00, 0xca, 0x46, 0x00]),
    (0x46b390, [0xb4, 0xb3, 0x46, 0x00], [0x00, 0xca, 0x46, 0x00]),
    (0x46bb50, [0x24, 0xbe, 0x46, 0x00], [0x00, 0xca, 0x46, 0x00]),
    (0x46bb60, [0x24, 0xbe, 0x46, 0x00], [0x00, 0xca, 0x46, 0x00]),
    (0x46bb70, [0x24, 0xbe, 0x46, 0x00], [0x00, 0xca, 0x46, 0x00]),
    (0x46bb80, [0x24, 0xbe, 0x46, 0x00], [0x00, 0xca, 0x46, 0x00]),
    (0x46bbac, [0xe0, 0xbe, 0x46, 0x00], [0x00, 0xca, 0x46, 0x00]),
    (0x46bbbc, [0xe0, 0xbe, 0x46, 0x00], [0x00, 0xca, 0x46, 0x00]),
    (0x46bbcc, [0xe0, 0xbe, 0x46, 0x00], [0x00, 0xca, 0x46, 0x00]),
    (0x46bbdc, [0xe0, 0xbe, 0x46, 0x00], [0x00, 0xca, 0x46, 0x00]),
    (0x46bbec, [0xe0, 0xbe, 0x46, 0x00], [0x00, 0xca, 0x46, 0x00]),
    (0x42b078, [0x06, 0x00, 0x80, 0x00], [0x06, 0x00, 0x30, 0x00]),
    (0x431028, [0x06, 0x00, 0x80, 0x00], [0x06, 0x00, 0x30, 0x00]),
    (0x43be8c, [0x06, 0x00, 0x80, 0x00], [0x06, 0x00, 0x30, 0x00]),
    (0x478f44, [0x06, 0x00, 0x80, 0x00], [0x06, 0x00, 0x30, 0x00]),
    (0x4948ac, [0x06, 0x00, 0x18, 0x00], [0x06, 0x00, 0x0c, 0x00]),
    (0x4948c4, [0x06, 0x00, 0x18, 0x00], [0x06, 0x00, 0x0c, 0x00]),
    (0x494944, [0x06, 0x00, 0x14, 0x00], [0x06, 0x00, 0x0a, 0x00]),
    (0x4bd7cc, [0x06, 0x00, 0x64, 0x00], [0x06, 0x00, 0x32, 0x00]),
    (0x4bd3a0, [0x06, 0x00, 0xf8, 0x00], [0x06, 0x00, 0x40, 0x00]),
];
const GHOST: u32 = 0x46ca00;
const FINAL_BURST_THINNING: [(u32, [u8; 4], [u8; 4]); 25] = {
    let mut patches = [(0, [0; 4], [0; 4]); 25];
    let mut i = 0;
    while i < patches.len() {
        patches[i] = (
            0x4bd4ec + i as u32 * 24,
            0x4bd750u32.to_le_bytes(),
            GHOST.to_le_bytes(),
        );
        i += 1;
    }
    patches
};
const INWARD_BURST_THINNING: [(u32, [u8; 4], [u8; 4]); 2] = [
    (0x46bb58, 0x46be24u32.to_le_bytes(), GHOST.to_le_bytes()),
    (0x46bb78, 0x46be24u32.to_le_bytes(), GHOST.to_le_bytes()),
];
const GUARDED_SPAWN: u8 = 0xf0;
const TRIG_LOOP: u8 = 0xf7;
const INIT_LOOP_MOTION: u8 = 0xa8;
const UNREAD_MARGIN: usize = 4;
const UNREAD_LEAST: usize = 16;
fn dp(p: &mut Assembler, m: M, at: u8) {
    emit(p, m, A::DirectPage, O::Byte(at));
}
fn abs(p: &mut Assembler, m: M, at: u16) {
    emit(p, m, A::Absolute, O::Word(at));
}
fn ax(p: &mut Assembler, m: M, at: u16) {
    emit(p, m, A::AbsoluteX, O::Word(at));
}
fn ay(p: &mut Assembler, m: M, at: u16) {
    emit(p, m, A::AbsoluteY, O::Word(at));
}
fn imm8(p: &mut Assembler, m: M, v: u8) {
    emit(p, m, A::Immediate, O::Byte(v));
}
fn imm16(p: &mut Assembler, m: M, v: u16) {
    emit(p, m, A::Immediate, O::Word(v));
}
fn m16(p: &mut Assembler) {
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
}
fn m8(p: &mut Assembler) {
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
}
fn br(p: &mut Assembler, m: M, label: &str) {
    p.emit_label_ref(m, A::Relative8, label);
}
fn jmp(p: &mut Assembler, label: &str) {
    p.emit_label_ref(M::Jmp, A::Absolute, label);
}
fn jsr(p: &mut Assembler, label: &str) {
    p.emit_label_ref(M::Jsr, A::Absolute, label);
}
fn ind(p: &mut Assembler, m: M, at: u8) {
    emit(p, m, A::DirectPageIndirectLongIndexedY, O::Byte(at));
}
fn acc(p: &mut Assembler, m: M) {
    emit(p, m, A::Accumulator, O::None);
}
fn cbyte(p: &mut Assembler, k: u16) {
    imm16(p, M::Ldy, k);
    ind(p, M::Lda, PCP);
}
fn cword(p: &mut Assembler, k: u16) {
    imm16(p, M::Ldy, k);
    ind(p, M::Lda, PCP);
}
fn advance(p: &mut Assembler, n: u16) {
    static SITES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let site = SITES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let same = format!("bvm_advance_{site}");
    m16(p);
    ax(p, M::Lda, 0x40);
    implied(p, M::Clc);
    imm16(p, M::Adc, n);
    ax(p, M::Sta, 0x40);
    dp(p, M::Sta, PCP);
    p.emit_label_ref(M::Bcc, A::Relative8, &same);
    ax(p, M::Inc, 0x42);
    m8(p);
    dp(p, M::Inc, PCP + 2);
    m16(p);
    p.label(same);
    m8(p);
}
fn advance_next(p: &mut Assembler, n: u16) {
    static SITES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let site = SITES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let same = format!("bvm_advance_next_{site}");
    m16(p);
    ax(p, M::Lda, 0x40);
    implied(p, M::Clc);
    imm16(p, M::Adc, n);
    ax(p, M::Sta, 0x40);
    dp(p, M::Sta, PCP);
    p.emit_label_ref(M::Bcc, A::Relative8, &same);
    ax(p, M::Inc, 0x42);
    m8(p);
    dp(p, M::Inc, PCP + 2);
    m16(p);
    p.label(same);
    emit(p, M::Lda, A::DirectPageIndirectLong, O::Byte(PCP));
    imm16(p, M::And, 0xff);
    acc(p, M::Asl);
    implied(p, M::Tax);
    m8(p);
    p.emit_label_ref(M::Jmp, A::AbsoluteIndexedIndirectX, "bvm_ops");
}
fn goto(p: &mut Assembler, k: u16) {
    m16(p);
    cword(p, k);
    implied(p, M::Pha);
    cword(p, k + 2);
    ax(p, M::Sta, 0x42);
    implied(p, M::Pla);
    ax(p, M::Sta, 0x40);
    dp(p, M::Sta, PCP);
    m8(p);
    p.emit_label_ref(M::Jsr, A::Absolute, "bvm_repoint_bank");
    spend(p);
}
fn yield_now(p: &mut Assembler) {
    implied(p, M::Rts);
}
fn next(p: &mut Assembler) {
    emit(p, M::Lda, A::DirectPageIndirectLong, O::Byte(PCP));
    m16(p);
    imm16(p, M::And, 0xff);
    acc(p, M::Asl);
    implied(p, M::Tax);
    m8(p);
    p.emit_label_ref(M::Jmp, A::AbsoluteIndexedIndirectX, "bvm_ops");
}
fn spend(p: &mut Assembler) {
    static SITES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let site = SITES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let left = format!("bvm_spend_{site}");
    m16(p);
    dp(p, M::Dec, BUDGET);
    m8(p);
    p.emit_label_ref(M::Bne, A::Relative8, &left);
    jmp(p, "bvm_budget_out");
    p.label(left);
}
fn copy32(p: &mut Assembler, to: u8, from: u8) {
    m16(p);
    dp(p, M::Lda, from);
    dp(p, M::Sta, to);
    dp(p, M::Lda, from + 2);
    dp(p, M::Sta, to + 2);
    m8(p);
}
fn set_field(p: &mut Assembler, field: u16, value: u32) {
    m16(p);
    imm16(p, M::Lda, value as u16);
    ax(p, M::Sta, field);
    imm16(p, M::Lda, (value >> 16) as u16);
    ax(p, M::Sta, field + 2);
    m8(p);
}
pub fn dispatch_table(program: &w65c816::AssembledProgram) -> Result<Vec<u8>> {
    let unknown = program
        .label_location("bvm_unknown")
        .ok_or("Missing body VM handler")?;
    Ok((0..=255u32)
        .flat_map(|n| {
            let at = program
                .label_location(&format!("bvm_op_{n:02x}"))
                .unwrap_or(unknown);
            (at.to_u32() as u16).to_le_bytes()
        })
        .collect())
}
pub fn emit_rules(p: &mut Assembler, layout: &CartridgeLayout, table: Option<&[u8]>) -> Result<()> {
    let image = layout.cpu_address(IMAGE_OFFSET)?;
    if image & 0xffff != SOURCE_BASE & 0xffff {
        return Err("Battle script image is not at its source bank offset".into());
    }
    let bank_add = ((image >> 16) - (SOURCE_BASE >> 16)) as u16;
    emit_scheduler(p, bank_add)?;
    emit_pool(p, layout.cpu_address(layout.section("bvm_zero")?.offset)?);
    emit_controller_side(p);
    emit_support(p, bank_add);
    emit_commands(p, layout, table)?;
    common_cohort::emit_parent(p);
    Ok(())
}
pub(crate) fn emit_vertical_far(p: &mut Assembler) {
    vertical_motion::emit_far(p);
}
pub(crate) fn emit_orbit_far(p: &mut Assembler, layout: &CartridgeLayout) -> Result<()> {
    orbit_turn::emit_far(p, layout)
}
pub(crate) fn emit_falling_far(p: &mut Assembler, layout: &CartridgeLayout) -> Result<()> {
    falling_turn::emit_far(p, layout)
}
pub(crate) fn emit_inward_far(p: &mut Assembler) {
    inward_turn::emit_far(p);
}
pub(crate) fn emit_arithmetic_block_far(p: &mut Assembler, layout: &CartridgeLayout) -> Result<()> {
    blocks::emit_far(p, layout)
}
pub(crate) fn emit_initializers_far(p: &mut Assembler, layout: &CartridgeLayout) -> Result<()> {
    initializers::emit_far(p, layout)
}
