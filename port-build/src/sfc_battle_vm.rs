use crate::Result;
use crate::sfc::{emit, implied};
use crate::sfc_battle::{
    ACTOR_BYTES, ACTOR_COUNT, ACTORS, CURRENT, CURRENT_INDEX, DIVISOR, ORDER_INDEX, RESULT, STATS,
};
use crate::sfc_battle_act::{
    ACTION, AMOUNT, FACTOR, ROLL, TARGET, TARGET_INDEX, WORK, abs, branch, byte, call, its, jump,
    m8, m16, own, registers, word,
};
use crate::sfc_battle_controller::Controller;
use sfc_builder::cartridge::{CartridgeLayout, Section};
use w65c816::{AddressingMode as A, Assembler, CpuMode, Mnemonic as M, Operand as O};
const PC: u16 = 0xce0;
const STACK: u16 = 0xce2;
const DEPTH: u16 = 0xcea;
pub const EXIT: u16 = 0xcec;
pub const BUDGET: u16 = 0xced;
const LATCH: u16 = 0xcee;
const GEMS: u16 = 0xcf0;
const WAIT: u16 = 0xcf2;
pub const GROWTH_NOTICE: u16 = 0x8c0 + 0x21;
const STEPS: u16 = 0xcf7;
const REACHED: u16 = 0xcf8;
const SPAN: u16 = 0xcfa;
pub const MESSAGE: u16 = 0xd00;
pub const MESSAGE_KIND: u16 = 0xd01;
pub const MESSAGE_WHO: u16 = 0xd02;
pub const MESSAGE_SLOT: u16 = 0xd03;
pub const MESSAGE_SOURCE: u16 = 0xd04;
pub const MUSIC: u16 = 0xd08;
pub const MUSIC_ALONE: u8 = 0x40;
pub const MUSIC_PENDING: u16 = 0xd0a;
pub const AUDIO_WAIT: u16 = 0xd0b;
pub const AUDIO_WAIT_MUSIC: u8 = 1;
pub const AUDIO_WAIT_EFFECT: u8 = 2;
pub const LAST_EFFECT: u16 = 0xcf4;
pub const EFFECT_REQUEST: u16 = 0xcf5;
pub const DIM: u16 = 0xd0c;
pub const WALL: u16 = 0xd11;
pub const MENU: u16 = 0xd0d;
pub const COMMAND: u16 = 0xd0e;
pub const RESTORE: u16 = 0xd10;
pub const KIND_DESCRIPTOR: u8 = 1;
pub const KIND_SOURCE: u8 = 2;
pub const SCENE: u16 = 0xd19;
pub const FADE: u16 = 0xd1a;
const DEFAULT_BUDGET: u8 = 64;
const GOLD: u16 = 0x974;
const INVENTORY: u16 = crate::sfc_b1_menu::INVENTORY;
mod actors;
mod commands;
mod rewards;
mod runtime;
mod tables;
pub use runtime::emit_rules;
pub use tables::{Tables, load, sections};
pub const PENDING_SCENE: u16 = crate::sfc_battle_objects::PENDING_SCENE;
fn program(p: &mut Assembler, m: M, base: u32, offset: u32) {
    emit(p, m, A::AbsoluteLongX, O::Long(base + offset));
}
fn next(p: &mut Assembler, bytes: u16, yielding: bool) {
    m16(p);
    abs(p, M::Lda, PC);
    implied(p, M::Clc);
    word(p, M::Adc, bytes);
    abs(p, M::Sta, PC);
    m8(p);
    if yielding {
        implied(p, M::Rts);
    } else {
        jump(p, "battle_vm_step");
    }
}
fn slot(p: &mut Assembler, m: M, index: u16, field: u16) {
    abs(p, m, ACTORS + index * ACTOR_BYTES + field);
}
