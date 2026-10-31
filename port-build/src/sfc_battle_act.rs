use crate::Result;
use crate::sfc::{emit, implied};
use crate::sfc_battle::{
    ACTORS, CURRENT, CURRENT_INDEX, DIVIDEND, DIVISOR, ORDER, ORDER_INDEX, RESULT, STATS,
};
use sfc_builder::cartridge::{CartridgeLayout, Section};
use w65c816::{AddressingMode as A, Assembler, CpuMode, Mnemonic as M, Operand as O};
pub(crate) const TARGET: u16 = 0xcc0;
pub(crate) const TARGET_INDEX: u16 = 0xcc2;
pub(crate) const AMOUNT: u16 = 0xcc4;
pub(crate) const WORK: u16 = 0xcc8;
pub(crate) const FACTOR: u16 = 0xccc;
pub(crate) const ROLL: u16 = 0xcce;
pub(crate) const RANDOM: u16 = 0xcd0;
pub(crate) const REMAINDER: u16 = 0xcd2;
const MULTIPLIER: u16 = 0xcd4;
const ATTACK_BONUS: u16 = 0xcd5;
const DEFENSE_BONUS: u16 = 0xcd6;
pub(crate) const ACTION: u16 = 0xcd7;
const LOSS: u16 = 0xcd8;
const STAFF: u16 = STATS + 0x1a;
const RING: u16 = STATS + 0x1b;
const STAFF_OFFSET: u8 = 0x54;
const RING_OFFSET: u8 = 0x51;
pub struct Tables {
    bonus: Vec<u8>,
    costs: Vec<u8>,
}
pub fn load(content: &crate::content::Content) -> Result<Tables> {
    let tables: serde_json::Value = content.json("battle-tables.json")?;
    let bonus = hex(tables["bonus_hex"]
        .as_str()
        .ok_or("Missing battle bonus run")?)?;
    if tables["staff_index_offset"] != STAFF_OFFSET || tables["ring_index_offset"] != RING_OFFSET {
        return Err("Battle bonus run offsets differ".into());
    }
    let items: serde_json::Value = content.json("items.json")?;
    for row in items["equipment_bonuses"]
        .as_array()
        .ok_or("Missing equipment bonuses")?
    {
        let id = row["item_id"].as_u64().ok_or("Equipment id")? as u8;
        let offset = if row["bonus_kind"] == "attack" {
            STAFF_OFFSET
        } else {
            RING_OFFSET
        };
        let at = usize::from(id.checked_sub(offset).ok_or("Equipment below the run")?);
        if bonus.get(at).map(|&b| u64::from(b)) != row["bonus"].as_u64() {
            return Err(format!("Equipment {id} bonus differs from the run").into());
        }
    }
    let actors: serde_json::Value = content.json("combat-actors.json")?;
    let mut costs = vec![0u8; 18];
    for row in actors["player_spells"]["actions"]
        .as_array()
        .ok_or("Missing spell costs")?
    {
        let action = row["action_id"].as_u64().ok_or("Spell action")? as usize;
        let cost = row["mp_cost"].as_u64().ok_or("Spell cost")? as u16;
        if !(1..=8).contains(&action) {
            return Err("Spell action out of range".into());
        }
        costs[action * 2..action * 2 + 2].copy_from_slice(&cost.to_le_bytes());
    }
    Ok(Tables { bonus, costs })
}
fn hex(text: &str) -> Result<Vec<u8>> {
    (0..text.len())
        .step_by(2)
        .map(|i| Ok(u8::from_str_radix(&text[i..i + 2], 16)?))
        .collect()
}
pub fn sections(
    sections: &mut Vec<Section>,
    space: &mut crate::sfc_rom::Space,
    tables: &Tables,
) -> Result<()> {
    crate::sfc_b1::add(sections, space, "battle_bonus".into(), tables.bonus.clone())?;
    crate::sfc_b1::add(sections, space, "battle_costs".into(), tables.costs.clone())?;
    Ok(())
}
pub(crate) fn abs(p: &mut Assembler, m: M, at: u16) {
    emit(p, m, A::Absolute, O::Word(at));
}
pub(crate) fn m16(p: &mut Assembler) {
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
}
pub(crate) fn m8(p: &mut Assembler) {
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
}
pub(crate) fn branch(p: &mut Assembler, m: M, label: &str) {
    p.emit_label_ref(m, A::Relative8, label);
}
pub(crate) fn call(p: &mut Assembler, label: &str) {
    p.emit_label_ref(M::Jsr, A::Absolute, label);
}
pub(crate) fn jump(p: &mut Assembler, label: &str) {
    p.emit_label_ref(M::Jmp, A::Absolute, label);
}
pub(crate) fn own(p: &mut Assembler, m: M, field: u16) {
    emit(p, m, A::AbsoluteY, O::Word(ACTORS + field));
}
pub(crate) fn its(p: &mut Assembler, m: M, field: u16) {
    emit(p, m, A::AbsoluteX, O::Word(ACTORS + field));
}
pub(crate) fn byte(p: &mut Assembler, m: M, value: u8) {
    emit(p, m, A::Immediate, O::Byte(value));
}
pub(crate) fn word(p: &mut Assembler, m: M, value: u16) {
    emit(p, m, A::Immediate, O::Word(value));
}
pub(crate) fn registers(p: &mut Assembler) {
    emit(p, M::Ldy, A::Absolute, O::Word(CURRENT));
    emit(p, M::Ldx, A::Absolute, O::Word(TARGET));
}
pub(crate) fn set_own(p: &mut Assembler, field: u16, bits: u8) {
    own(p, M::Lda, field);
    byte(p, M::Ora, bits);
    own(p, M::Sta, field);
}
pub(crate) fn clear_own(p: &mut Assembler, field: u16, bits: u8) {
    own(p, M::Lda, field);
    byte(p, M::And, !bits);
    own(p, M::Sta, field);
}
pub(crate) fn set_its(p: &mut Assembler, field: u16, bits: u8) {
    its(p, M::Lda, field);
    byte(p, M::Ora, bits);
    its(p, M::Sta, field);
}
pub(crate) fn clear_its(p: &mut Assembler, field: u16, bits: u8) {
    its(p, M::Lda, field);
    byte(p, M::And, !bits);
    its(p, M::Sta, field);
}
fn rng_of(p: &mut Assembler) {
    call(p, "battle_rng");
}
pub fn emit_rules(p: &mut Assembler, layout: &CartridgeLayout) -> Result<()> {
    let bonus = layout.cpu_address(layout.section("battle_bonus")?.offset)?;
    let costs = layout.cpu_address(layout.section("battle_costs")?.offset)?;
    p.label("battle_rng").assume_mode(CpuMode::NATIVE_M8_X16);
    implied(p, M::Phx);
    byte(p, M::Lda, 0);
    call(p, "encounter_rng");
    m16(p);
    abs(p, M::Stz, RANDOM);
    abs(p, M::Lda, DIVISOR);
    branch(p, M::Beq, "battle_rng_done");
    abs(p, M::Lda, crate::sfc_encounter::SEED + 2);
    abs(p, M::Sta, DIVIDEND);
    m8(p);
    call(p, "battle_divide");
    m16(p);
    abs(p, M::Lda, DIVIDEND);
    abs(p, M::Sta, RANDOM);
    p.label("battle_rng_done");
    m8(p);
    implied(p, M::Plx);
    implied(p, M::Rts);
    p.label("battle_multiply")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    abs(p, M::Stz, WORK);
    abs(p, M::Stz, WORK + 2);
    p.label("battle_multiply_bit");
    abs(p, M::Lda, FACTOR);
    branch(p, M::Beq, "battle_multiply_done");
    abs(p, M::Lsr, FACTOR);
    branch(p, M::Bcc, "battle_multiply_skip");
    abs(p, M::Lda, WORK);
    implied(p, M::Clc);
    abs(p, M::Adc, AMOUNT);
    abs(p, M::Sta, WORK);
    abs(p, M::Lda, WORK + 2);
    abs(p, M::Adc, AMOUNT + 2);
    abs(p, M::Sta, WORK + 2);
    p.label("battle_multiply_skip");
    abs(p, M::Asl, AMOUNT);
    abs(p, M::Rol, AMOUNT + 2);
    branch(p, M::Bra, "battle_multiply_bit");
    p.label("battle_multiply_done");
    abs(p, M::Lda, WORK);
    abs(p, M::Sta, AMOUNT);
    abs(p, M::Lda, WORK + 2);
    abs(p, M::Sta, AMOUNT + 2);
    m8(p);
    implied(p, M::Rts);
    p.label("battle_divide_long")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    implied(p, M::Phx);
    m16(p);
    abs(p, M::Stz, REMAINDER);
    emit(p, M::Ldx, A::Immediate, O::Word(32));
    p.label("battle_divide_long_bit");
    abs(p, M::Asl, AMOUNT);
    abs(p, M::Rol, AMOUNT + 2);
    abs(p, M::Rol, REMAINDER);
    branch(p, M::Bcs, "battle_divide_long_take");
    abs(p, M::Lda, REMAINDER);
    abs(p, M::Cmp, DIVISOR);
    branch(p, M::Bcc, "battle_divide_long_next");
    p.label("battle_divide_long_take");
    abs(p, M::Lda, REMAINDER);
    implied(p, M::Sec);
    abs(p, M::Sbc, DIVISOR);
    abs(p, M::Sta, REMAINDER);
    abs(p, M::Inc, AMOUNT);
    p.label("battle_divide_long_next");
    implied(p, M::Dex);
    branch(p, M::Bne, "battle_divide_long_bit");
    m8(p);
    implied(p, M::Plx);
    implied(p, M::Rts);
    p.label("battle_steps").assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    abs(p, M::Lda, DIVISOR);
    branch(p, M::Bne, "battle_steps_divide");
    abs(p, M::Stz, AMOUNT);
    abs(p, M::Stz, AMOUNT + 2);
    m8(p);
    byte(p, M::Lda, 8);
    implied(p, M::Rts);
    p.label("battle_steps_divide")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    for _ in 0..3 {
        abs(p, M::Asl, AMOUNT);
        abs(p, M::Rol, AMOUNT + 2);
    }
    abs(p, M::Lda, DIVISOR);
    emit(p, M::Dec, A::Accumulator, O::None);
    implied(p, M::Clc);
    abs(p, M::Adc, AMOUNT);
    abs(p, M::Sta, AMOUNT);
    word(p, M::Lda, 0);
    abs(p, M::Adc, AMOUNT + 2);
    abs(p, M::Sta, AMOUNT + 2);
    m8(p);
    call(p, "battle_divide_long");
    byte(p, M::Lda, 8);
    implied(p, M::Sec);
    abs(p, M::Sbc, AMOUNT);
    implied(p, M::Rts);
    p.label("battle_pair").assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    abs(p, M::Lda, ORDER_INDEX);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Tax);
    emit(p, M::Lda, A::AbsoluteX, O::Word(ORDER));
    abs(p, M::Sta, CURRENT_INDEX);
    m8(p);
    call(p, "battle_actor_offset");
    m16(p);
    abs(p, M::Sta, CURRENT);
    implied(p, M::Tay);
    m8(p);
    own(p, M::Lda, 0x44);
    m16(p);
    word(p, M::And, 0xff);
    abs(p, M::Sta, TARGET_INDEX);
    m8(p);
    call(p, "battle_actor_offset");
    m16(p);
    abs(p, M::Sta, TARGET);
    m8(p);
    registers(p);
    implied(p, M::Rts);
    act(p, bonus, costs);
    prepare(p);
    escape(p);
    heal(p);
    guard(p);
    reaction(p);
    apply(p);
    status(p);
    Ok(())
}
fn act(p: &mut Assembler, bonus: u32, costs: u32) {
    p.label("battle_act").assume_mode(CpuMode::NATIVE_M8_X16);
    call(p, "battle_pair");
    byte(p, M::Lda, 0);
    own(p, M::Sta, 0x4e);
    own(p, M::Sta, 0x45);
    its(p, M::Sta, 0x45);
    its(p, M::Stz, 0x4c);
    its(p, M::Stz, 0x4d);
    byte(p, M::Lda, 8);
    its(p, M::Sta, 0x51);
    own(p, M::Lda, 0);
    branch(p, M::Beq, "battle_act_arle");
    emit(p, M::Cmp, A::Immediate, O::Byte(1));
    branch(p, M::Bne, "battle_act_not_carbuncle");
    jump(p, "battle_act_carbuncle");
    p.label("battle_act_not_carbuncle");
    emit(p, M::Cmp, A::Immediate, O::Byte(2));
    branch(p, M::Bne, "battle_act_to_reaction");
    jump(p, "battle_act_enemy");
    p.label("battle_act_to_reaction");
    jump(p, "battle_act_reaction");
    p.label("battle_act_arle");
    byte(p, M::Lda, 0);
    own(p, M::Sta, 0x48);
    own(p, M::Lda, 0x40);
    emit(p, M::Cmp, A::Immediate, O::Byte(9));
    branch(p, M::Bcs, "battle_act_to_reaction");
    emit(p, M::Cmp, A::Immediate, O::Byte(0));
    branch(p, M::Bne, "battle_act_cast");
    call(p, "battle_escape");
    jump(p, "battle_act_reaction");
    p.label("battle_act_cast");
    abs(p, M::Sta, ACTION);
    own(p, M::Lda, 0x3e);
    own(p, M::Sta, 0x4b);
    its(p, M::Sta, 0x4b);
    abs(p, M::Lda, ACTION);
    emit(p, M::Cmp, A::Immediate, O::Byte(4));
    branch(p, M::Bcs, "battle_act_chosen");
    own(p, M::Lda, 0x2e);
    byte(p, M::And, 0x08);
    branch(p, M::Beq, "battle_act_chosen");
    abs(p, M::Lda, ACTION);
    emit(p, M::Inc, A::Accumulator, O::None);
    emit(p, M::Cmp, A::Immediate, O::Byte(4));
    branch(p, M::Bcc, "battle_act_turned");
    byte(p, M::Lda, 1);
    p.label("battle_act_turned");
    abs(p, M::Sta, ACTION);
    own(p, M::Sta, 0x40);
    p.label("battle_act_chosen");
    m16(p);
    abs(p, M::Lda, ACTION);
    word(p, M::And, 0xff);
    implied(p, M::Clc);
    abs(p, M::Adc, CURRENT);
    implied(p, M::Tay);
    m8(p);
    own(p, M::Lda, 0x1f);
    emit(p, M::Ldy, A::Absolute, O::Word(CURRENT));
    its(p, M::Sta, 0x4a);
    abs(p, M::Lda, ACTION);
    own(p, M::Sta, 0x4e);
    implied(p, M::Clc);
    byte(p, M::Adc, 31);
    its(p, M::Sta, 0x4e);
    m16(p);
    abs(p, M::Lda, ACTION);
    word(p, M::And, 0xff);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Tax);
    own(p, M::Lda, 8);
    implied(p, M::Sec);
    emit(p, M::Sbc, A::AbsoluteLongX, O::Long(costs));
    own(p, M::Sta, 8);
    m8(p);
    emit(p, M::Ldx, A::Absolute, O::Word(TARGET));
    byte(p, M::Lda, 1);
    abs(p, M::Sta, RESULT);
    abs(p, M::Lda, ACTION);
    emit(p, M::Cmp, A::Immediate, O::Byte(4));
    branch(p, M::Bcs, "battle_act_not_attack");
    m16(p);
    its(p, M::Lda, 0x4a);
    word(p, M::And, 0xff);
    implied(p, M::Tax);
    m8(p);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(bonus));
    abs(p, M::Sta, ATTACK_BONUS);
    abs(p, M::Lda, STAFF);
    branch(p, M::Beq, "battle_act_attack");
    implied(p, M::Sec);
    byte(p, M::Sbc, STAFF_OFFSET);
    m16(p);
    word(p, M::And, 0xff);
    implied(p, M::Tax);
    m8(p);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(bonus));
    implied(p, M::Clc);
    abs(p, M::Adc, ATTACK_BONUS);
    abs(p, M::Sta, ATTACK_BONUS);
    abs(p, M::Lda, ACTION);
    implied(p, M::Clc);
    byte(p, M::Adc, 88);
    abs(p, M::Cmp, STAFF);
    branch(p, M::Bne, "battle_act_attack");
    abs(p, M::Lda, ATTACK_BONUS);
    implied(p, M::Clc);
    byte(p, M::Adc, 4);
    abs(p, M::Sta, ATTACK_BONUS);
    p.label("battle_act_attack");
    registers(p);
    byte(p, M::Lda, 1);
    abs(p, M::Sta, MULTIPLIER);
    abs(p, M::Stz, DEFENSE_BONUS);
    call(p, "battle_attack");
    jump(p, "battle_act_after");
    p.label("battle_act_not_attack");
    branch(p, M::Bne, "battle_act_not_heal");
    call(p, "battle_heal");
    jump(p, "battle_act_after");
    p.label("battle_act_not_heal");
    emit(p, M::Cmp, A::Immediate, O::Byte(5));
    branch(p, M::Bne, "battle_act_not_5");
    abs(p, M::Stz, MULTIPLIER);
    abs(p, M::Stz, ATTACK_BONUS);
    abs(p, M::Stz, DEFENSE_BONUS);
    call(p, "battle_attack");
    registers(p);
    byte(p, M::Lda, 11);
    own(p, M::Sta, 0x48);
    clear_own(p, 0x45, 0x10);
    jump(p, "battle_act_after");
    p.label("battle_act_not_5");
    emit(p, M::Cmp, A::Immediate, O::Byte(6));
    branch(p, M::Bne, "battle_act_not_6");
    byte(p, M::Lda, 14);
    own(p, M::Sta, 0x4e);
    jump(p, "battle_act_after");
    p.label("battle_act_not_6");
    emit(p, M::Cmp, A::Immediate, O::Byte(7));
    branch(p, M::Bne, "battle_act_not_7");
    byte(p, M::Lda, 13);
    own(p, M::Sta, 0x4e);
    jump(p, "battle_act_after");
    p.label("battle_act_not_7");
    byte(p, M::Lda, 3);
    abs(p, M::Sta, MULTIPLIER);
    abs(p, M::Stz, ATTACK_BONUS);
    abs(p, M::Stz, DEFENSE_BONUS);
    call(p, "battle_attack");
    p.label("battle_act_after");
    call(p, "battle_guard");
    jump(p, "battle_act_reaction");
    p.label("battle_act_carbuncle");
    own(p, M::Lda, 0x40);
    branch(p, M::Bne, "battle_act_carbuncle_not_rest");
    own(p, M::Sta, 0x47);
    jump(p, "battle_act_reaction");
    p.label("battle_act_carbuncle_not_rest");
    emit(p, M::Cmp, A::Immediate, O::Byte(1));
    branch(p, M::Beq, "battle_act_carbuncle_attack");
    jump(p, "battle_act_reaction");
    p.label("battle_act_carbuncle_attack");
    byte(p, M::Lda, 0x62);
    its(p, M::Sta, 0x4e);
    byte(p, M::Lda, 1);
    its(p, M::Sta, 0x4b);
    its(p, M::Sta, 0x4a);
    abs(p, M::Sta, MULTIPLIER);
    byte(p, M::Lda, 4);
    own(p, M::Sta, 0x47);
    abs(p, M::Stz, ATTACK_BONUS);
    abs(p, M::Stz, DEFENSE_BONUS);
    call(p, "battle_attack");
    registers(p);
    its(p, M::Lda, 0x19);
    branch(p, M::Bne, "battle_act_reaction");
    m16(p);
    its(p, M::Lda, 6);
    its(p, M::Sta, 0x4c);
    m8(p);
    branch(p, M::Bra, "battle_act_reaction");
    p.label("battle_act_enemy");
    own(p, M::Lda, 0x47);
    branch(p, M::Beq, "battle_act_reaction");
    own(p, M::Lda, 0x48);
    emit(p, M::Cmp, A::Immediate, O::Byte(12));
    branch(p, M::Bne, "battle_act_enemy_attack");
    call(p, "battle_heal");
    branch(p, M::Bra, "battle_act_enemy_after");
    p.label("battle_act_enemy_attack");
    for (effect, kind) in [(6u8, 1u8), (7, 2), (8, 3)] {
        let next = format!("battle_act_enemy_not_{effect}");
        emit(p, M::Cmp, A::Immediate, O::Byte(effect));
        branch(p, M::Bne, &next);
        byte(p, M::Lda, kind);
        own(p, M::Sta, 0x4e);
        p.label(next);
    }
    abs(p, M::Stz, DEFENSE_BONUS);
    abs(p, M::Lda, RING);
    branch(p, M::Beq, "battle_act_enemy_hit");
    abs(p, M::Lda, TARGET_INDEX);
    branch(p, M::Bne, "battle_act_enemy_hit");
    abs(p, M::Lda, RING);
    implied(p, M::Sec);
    byte(p, M::Sbc, RING_OFFSET);
    m16(p);
    word(p, M::And, 0xff);
    implied(p, M::Tax);
    m8(p);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(bonus));
    abs(p, M::Sta, DEFENSE_BONUS);
    p.label("battle_act_enemy_hit");
    byte(p, M::Lda, 1);
    abs(p, M::Sta, MULTIPLIER);
    abs(p, M::Stz, ATTACK_BONUS);
    call(p, "battle_attack");
    p.label("battle_act_enemy_after");
    call(p, "battle_guard");
    p.label("battle_act_reaction");
    call(p, "battle_reaction");
    registers(p);
    byte(p, M::Lda, 1);
    own(p, M::Sta, 0x46);
    implied(p, M::Rts);
}
fn prepare(p: &mut Assembler) {
    p.label("battle_attack").assume_mode(CpuMode::NATIVE_M8_X16);
    registers(p);
    m16(p);
    its(p, M::Lda, 2);
    own(p, M::Cmp, 2);
    m8(p);
    branch(p, M::Bcs, "battle_attack_roll");
    set_own(p, 0x45, 0x40);
    jump(p, "battle_attack_states");
    p.label("battle_attack_roll");
    m16(p);
    own(p, M::Lda, 0x10);
    own(p, M::Cmp, 2);
    branch(p, M::Beq, "battle_attack_plain");
    branch(p, M::Bcc, "battle_attack_plain");
    implied(p, M::Sec);
    own(p, M::Sbc, 2);
    abs(p, M::Sta, DIVISOR);
    m8(p);
    rng_of(p);
    m16(p);
    abs(p, M::Lda, RANDOM);
    implied(p, M::Clc);
    own(p, M::Adc, 2);
    branch(p, M::Bra, "battle_attack_rolled");
    p.label("battle_attack_plain");
    abs(p, M::Sta, DIVISOR);
    m8(p);
    rng_of(p);
    m16(p);
    abs(p, M::Lda, RANDOM);
    p.label("battle_attack_rolled");
    abs(p, M::Sta, ROLL);
    its(p, M::Lda, 0x12);
    abs(p, M::Sta, DIVISOR);
    m8(p);
    rng_of(p);
    m16(p);
    abs(p, M::Lda, RANDOM);
    emit(p, M::Lsr, A::Accumulator, O::None);
    abs(p, M::Cmp, ROLL);
    m8(p);
    branch(p, M::Bcs, "battle_attack_missed");
    set_own(p, 0x45, 0x40);
    branch(p, M::Bra, "battle_attack_states");
    p.label("battle_attack_missed");
    set_its(p, 0x45, 0x20);
    p.label("battle_attack_states");
    own(p, M::Lda, 0x2e);
    byte(p, M::And, 0x20);
    branch(p, M::Beq, "battle_attack_open");
    own(p, M::Lda, 0x40);
    emit(p, M::Cmp, A::Immediate, O::Byte(3));
    branch(p, M::Beq, "battle_attack_open");
    clear_own(p, 0x45, 0x40);
    set_its(p, 0x45, 0x20);
    p.label("battle_attack_open");
    its(p, M::Lda, 0x2e);
    byte(p, M::And, 2);
    branch(p, M::Beq, "battle_attack_critical");
    set_own(p, 0x45, 0x40);
    clear_its(p, 0x45, 0x20);
    p.label("battle_attack_critical");
    m16(p);
    word(p, M::Lda, 100);
    abs(p, M::Sta, DIVISOR);
    m8(p);
    rng_of(p);
    m16(p);
    abs(p, M::Lda, RANDOM);
    own(p, M::Cmp, 0x14);
    m8(p);
    branch(p, M::Bcs, "battle_attack_amount");
    set_own(p, 0x45, 0x10);
    p.label("battle_attack_amount");
    m16(p);
    abs(p, M::Stz, AMOUNT);
    abs(p, M::Stz, AMOUNT + 2);
    m8(p);
    own(p, M::Lda, 0x45);
    byte(p, M::And, 0x40);
    branch(p, M::Bne, "battle_attack_hit");
    jump(p, "battle_attack_factor");
    p.label("battle_attack_hit");
    m16(p);
    abs(p, M::Lda, ATTACK_BONUS);
    word(p, M::And, 0xff);
    implied(p, M::Clc);
    own(p, M::Adc, 0x0c);
    abs(p, M::Sta, AMOUNT);
    abs(p, M::Rol, AMOUNT + 2);
    word(p, M::Lda, 2);
    abs(p, M::Sta, FACTOR);
    m8(p);
    own(p, M::Lda, 0x45);
    byte(p, M::And, 0x10);
    branch(p, M::Beq, "battle_attack_power");
    abs(p, M::Inc, FACTOR);
    p.label("battle_attack_power");
    call(p, "battle_multiply");
    m16(p);
    abs(p, M::Lda, AMOUNT);
    implied(p, M::Sec);
    its(p, M::Sbc, 0x0e);
    abs(p, M::Sta, AMOUNT);
    abs(p, M::Lda, AMOUNT + 2);
    word(p, M::Sbc, 0);
    abs(p, M::Sta, AMOUNT + 2);
    abs(p, M::Lda, DEFENSE_BONUS);
    word(p, M::And, 0xff);
    abs(p, M::Sta, FACTOR);
    abs(p, M::Lda, AMOUNT);
    implied(p, M::Sec);
    abs(p, M::Sbc, FACTOR);
    abs(p, M::Sta, AMOUNT);
    abs(p, M::Lda, AMOUNT + 2);
    word(p, M::Sbc, 0);
    abs(p, M::Sta, AMOUNT + 2);
    branch(p, M::Bpl, "battle_attack_element");
    abs(p, M::Stz, AMOUNT);
    abs(p, M::Stz, AMOUNT + 2);
    p.label("battle_attack_element");
    m8(p);
    own(p, M::Lda, 0x4e);
    branch(p, M::Beq, "battle_attack_quarter");
    element(p, "battle_attack", "its");
    p.label("battle_attack_quarter");
    shift_amount(p, 2);
    p.label("battle_attack_factor");
    registers(p);
    m16(p);
    own(p, M::Lda, 0x47);
    word(p, M::And, 0xff);
    abs(p, M::Sta, FACTOR);
    its(p, M::Lda, 0x2e);
    word(p, M::And, 0x400);
    branch(p, M::Beq, "battle_attack_scaled");
    abs(p, M::Inc, FACTOR);
    p.label("battle_attack_scaled");
    m8(p);
    call(p, "battle_multiply");
    shift_amount(p, 2);
    own(p, M::Lda, 0x2e);
    byte(p, M::And, 0x10);
    branch(p, M::Beq, "battle_attack_weak");
    shift_amount(p, 1);
    p.label("battle_attack_weak");
    m16(p);
    abs(p, M::Lda, AMOUNT);
    abs(p, M::Ora, AMOUNT + 2);
    m8(p);
    branch(p, M::Bne, "battle_attack_guarded");
    clear_own(p, 0x45, 0x10);
    p.label("battle_attack_guarded");
    its(p, M::Lda, 0x2f);
    byte(p, M::And, 0x20);
    branch(p, M::Beq, "battle_attack_charge");
    set_own(p, 0x45, 0x40);
    clear_its(p, 0x45, 0x20);
    m16(p);
    abs(p, M::Stz, AMOUNT);
    abs(p, M::Stz, AMOUNT + 2);
    m8(p);
    byte(p, M::Lda, 0);
    own(p, M::Sta, 0x48);
    p.label("battle_attack_charge");
    m16(p);
    abs(p, M::Lda, MULTIPLIER);
    word(p, M::And, 0xff);
    abs(p, M::Sta, FACTOR);
    m8(p);
    call(p, "battle_multiply");
    charge_factor(p);
    call(p, "battle_multiply");
    registers(p);
    m16(p);
    abs(p, M::Lda, AMOUNT);
    its(p, M::Sta, 0x4c);
    abs(p, M::Stz, AMOUNT + 2);
    its(p, M::Lda, 6);
    abs(p, M::Sta, DIVISOR);
    m8(p);
    call(p, "battle_steps");
    its(p, M::Sta, 0x51);
    m16(p);
    own(p, M::Lda, 0x12);
    emit(p, M::Lsr, A::Accumulator, O::None);
    abs(p, M::Sta, DIVISOR);
    m8(p);
    rng_of(p);
    m16(p);
    abs(p, M::Lda, RANDOM);
    abs(p, M::Sta, ROLL);
    its(p, M::Lda, 0x12);
    abs(p, M::Sta, DIVISOR);
    m8(p);
    rng_of(p);
    m16(p);
    abs(p, M::Lda, RANDOM);
    abs(p, M::Cmp, ROLL);
    m8(p);
    branch(p, M::Bcs, "battle_attack_done");
    set_own(p, 0x45, 0x08);
    p.label("battle_attack_done");
    implied(p, M::Rts);
}
fn element(p: &mut Assembler, prefix: &str, whose: &str) {
    m16(p);
    word(p, M::And, 0xff);
    implied(p, M::Clc);
    abs(p, M::Adc, if whose == "its" { TARGET } else { CURRENT });
    implied(p, M::Tax);
    m8(p);
    emit(p, M::Lda, A::AbsoluteX, O::Word(ACTORS + 0x15));
    abs(p, M::Sta, ROLL);
    abs(p, M::Stz, ROLL + 1);
    m16(p);
    word(p, M::Lda, 3);
    abs(p, M::Sta, DIVISOR);
    m8(p);
    rng_of(p);
    m16(p);
    abs(p, M::Lda, RANDOM);
    implied(p, M::Clc);
    word(p, M::Adc, 3);
    abs(p, M::Sta, FACTOR);
    word(p, M::Lda, 0);
    p.label(format!("{prefix}_element_add"));
    implied(p, M::Clc);
    abs(p, M::Adc, ROLL);
    abs(p, M::Dec, FACTOR);
    branch(p, M::Bne, &format!("{prefix}_element_add"));
    implied(p, M::Clc);
    abs(p, M::Adc, AMOUNT);
    abs(p, M::Sta, AMOUNT);
    word(p, M::Lda, 0);
    abs(p, M::Adc, AMOUNT + 2);
    abs(p, M::Sta, AMOUNT + 2);
    m8(p);
    registers(p);
}
fn shift_amount(p: &mut Assembler, bits: u8) {
    m16(p);
    for _ in 0..bits {
        abs(p, M::Lsr, AMOUNT + 2);
        abs(p, M::Ror, AMOUNT);
    }
    m8(p);
}
fn charge_factor(p: &mut Assembler) {
    emit(p, M::Ldy, A::Absolute, O::Word(CURRENT));
    m16(p);
    own(p, M::Lda, 0x3e);
    word(p, M::And, 0xff);
    emit(p, M::Asl, A::Accumulator, O::None);
    emit(p, M::Inc, A::Accumulator, O::None);
    abs(p, M::Sta, FACTOR);
    m8(p);
    byte(p, M::Lda, 0);
    own(p, M::Sta, 0x3e);
    clear_own(p, 0x2f, 0x40);
}
fn escape(p: &mut Assembler) {
    p.label("battle_escape").assume_mode(CpuMode::NATIVE_M8_X16);
    registers(p);
    byte(p, M::Lda, 0);
    own(p, M::Sta, 0x3e);
    clear_own(p, 0x2f, 0x40);
    its(p, M::Lda, 0x19);
    branch(p, M::Bne, "battle_escape_done");
    m16(p);
    its(p, M::Lda, 2);
    own(p, M::Cmp, 2);
    m8(p);
    branch(p, M::Bcs, "battle_escape_roll");
    set_own(p, 0x45, 0x04);
    implied(p, M::Rts);
    p.label("battle_escape_roll");
    m16(p);
    own(p, M::Lda, 0x12);
    abs(p, M::Sta, DIVISOR);
    m8(p);
    rng_of(p);
    m16(p);
    abs(p, M::Lda, RANDOM);
    abs(p, M::Sta, ROLL);
    its(p, M::Lda, 0x12);
    abs(p, M::Sta, DIVISOR);
    m8(p);
    rng_of(p);
    m16(p);
    abs(p, M::Lda, RANDOM);
    emit(p, M::Lsr, A::Accumulator, O::None);
    abs(p, M::Cmp, ROLL);
    m8(p);
    branch(p, M::Bcs, "battle_escape_done");
    set_own(p, 0x45, 0x04);
    p.label("battle_escape_done");
    implied(p, M::Rts);
}
fn heal(p: &mut Assembler) {
    p.label("battle_heal").assume_mode(CpuMode::NATIVE_M8_X16);
    registers(p);
    m16(p);
    own(p, M::Lda, 6);
    abs(p, M::Sta, DIVIDEND);
    word(p, M::Lda, 3);
    abs(p, M::Sta, DIVISOR);
    m8(p);
    call(p, "battle_divide");
    m16(p);
    abs(p, M::Lda, crate::sfc_battle::QUOTIENT);
    abs(p, M::Sta, AMOUNT);
    abs(p, M::Stz, AMOUNT + 2);
    m8(p);
    own(p, M::Lda, 0);
    call(p, "battle_actor_offset");
    m16(p);
    implied(p, M::Tax);
    abs(p, M::Lda, CURRENT);
    abs(p, M::Sta, ROLL);
    emit(p, M::Stx, A::Absolute, O::Word(CURRENT));
    m8(p);
    charge_factor(p);
    m16(p);
    abs(p, M::Lda, ROLL);
    abs(p, M::Sta, CURRENT);
    m8(p);
    call(p, "battle_multiply");
    m16(p);
    abs(p, M::Stz, AMOUNT + 2);
    abs(p, M::Lda, AMOUNT);
    abs(p, M::Sta, RANDOM);
    m8(p);
    registers(p);
    m16(p);
    own(p, M::Lda, 6);
    abs(p, M::Sta, DIVISOR);
    m8(p);
    call(p, "battle_steps");
    m16(p);
    abs(p, M::Lda, AMOUNT + 2);
    branch(p, M::Bne, "battle_heal_full");
    abs(p, M::Lda, AMOUNT);
    emit(p, M::Cmp, A::Immediate, O::Word(9));
    branch(p, M::Bcs, "battle_heal_full");
    m8(p);
    byte(p, M::Lda, 8);
    implied(p, M::Sec);
    abs(p, M::Sbc, AMOUNT);
    branch(p, M::Bra, "battle_heal_shown");
    p.label("battle_heal_full");
    m8(p);
    byte(p, M::Lda, 0);
    p.label("battle_heal_shown");
    abs(p, M::Sta, ACTORS + 0x5d);
    abs(p, M::Sta, ACTORS + 0x5f);
    byte(p, M::Lda, 8);
    abs(p, M::Sta, ACTORS + 0x5e);
    m16(p);
    own(p, M::Lda, 4);
    implied(p, M::Clc);
    abs(p, M::Adc, RANDOM);
    own(p, M::Cmp, 6);
    branch(p, M::Bcc, "battle_heal_store");
    branch(p, M::Beq, "battle_heal_store");
    own(p, M::Lda, 6);
    p.label("battle_heal_store");
    own(p, M::Sta, 4);
    m8(p);
    call(p, "battle_shortage");
    implied(p, M::Rts);
}
fn guard(p: &mut Assembler) {
    p.label("battle_guard").assume_mode(CpuMode::NATIVE_M8_X16);
    registers(p);
    own(p, M::Lda, 0x4e);
    emit(p, M::Cmp, A::Immediate, O::Byte(13));
    branch(p, M::Bne, "battle_guard_not_13");
    own(p, M::Lda, 0x3e);
    implied(p, M::Clc);
    byte(p, M::Adc, 2);
    own(p, M::Sta, 0x3d);
    set_own(p, 0x2f, 0x20);
    byte(p, M::Lda, 0);
    own(p, M::Sta, 0x3e);
    clear_own(p, 0x2f, 0x40);
    implied(p, M::Rts);
    p.label("battle_guard_not_13");
    emit(p, M::Cmp, A::Immediate, O::Byte(14));
    branch(p, M::Bne, "battle_guard_done");
    own(p, M::Lda, 0x3e);
    own(p, M::Cmp, 0x25);
    branch(p, M::Bcs, "battle_guard_done");
    emit(p, M::Inc, A::Accumulator, O::None);
    own(p, M::Sta, 0x3e);
    set_own(p, 0x2f, 0x40);
    p.label("battle_guard_done");
    implied(p, M::Rts);
}
fn reaction(p: &mut Assembler) {
    p.label("battle_reaction")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    registers(p);
    byte(p, M::Lda, 0);
    own(p, M::Sta, 0x58);
    own(p, M::Sta, 0x59);
    own(p, M::Sta, 0x5b);
    own(p, M::Lda, 0x40);
    for (action, category) in [
        (1u8, 3u8),
        (2, 3),
        (3, 3),
        (8, 3),
        (4, 2),
        (6, 2),
        (7, 2),
        (5, 1),
    ] {
        let next = format!("battle_reaction_not_{action}");
        emit(p, M::Cmp, A::Immediate, O::Byte(action));
        branch(p, M::Bne, &next);
        byte(p, M::Lda, category);
        branch(p, M::Bra, "battle_reaction_category");
        p.label(next);
    }
    byte(p, M::Lda, 0);
    p.label("battle_reaction_category");
    abs(p, M::Sta, ACTION);
    own(p, M::Lda, 0x40);
    emit(p, M::Cmp, A::Immediate, O::Byte(1));
    branch(p, M::Bcc, "battle_reaction_cursed");
    emit(p, M::Cmp, A::Immediate, O::Byte(4));
    branch(p, M::Bcs, "battle_reaction_cursed");
    own(p, M::Lda, 0x2e);
    byte(p, M::And, 0x08);
    branch(p, M::Beq, "battle_reaction_cursed");
    byte(p, M::Lda, 1);
    own(p, M::Sta, 0x58);
    p.label("battle_reaction_cursed");
    abs(p, M::Lda, ACTION);
    emit(p, M::Cmp, A::Immediate, O::Byte(3));
    branch(p, M::Bne, "battle_reaction_poison");
    own(p, M::Lda, 0x2e);
    byte(p, M::And, 0x10);
    branch(p, M::Beq, "battle_reaction_poison");
    byte(p, M::Lda, 2);
    own(p, M::Sta, 0x58);
    p.label("battle_reaction_poison");
    abs(p, M::Lda, ACTION);
    byte(p, M::And, 1);
    branch(p, M::Beq, "battle_reaction_element");
    own(p, M::Lda, 0x2e);
    byte(p, M::And, 0x20);
    branch(p, M::Beq, "battle_reaction_element");
    byte(p, M::Lda, 3);
    own(p, M::Sta, 0x58);
    p.label("battle_reaction_element");
    for (kind, harmful, removed) in [(1u8, 0x40u16, 0x80u16), (2, 0x80, 0x40), (3, 0x100, 0x20)] {
        let next = format!("battle_reaction_not_kind_{kind}");
        own(p, M::Lda, 0x4e);
        emit(p, M::Cmp, A::Immediate, O::Byte(kind));
        branch(p, M::Bne, &next);
        m16(p);
        own(p, M::Lda, 0x2e);
        word(p, M::And, harmful);
        m8(p);
        branch(p, M::Beq, &format!("battle_reaction_unharmed_{kind}"));
        byte(p, M::Lda, kind);
        own(p, M::Sta, 0x59);
        p.label(format!("battle_reaction_unharmed_{kind}"));
        m16(p);
        own(p, M::Lda, 0x2e);
        word(p, M::And, removed);
        branch(p, M::Beq, &format!("battle_reaction_kept_{kind}"));
        own(p, M::Lda, 0x2e);
        word(p, M::And, !removed);
        own(p, M::Sta, 0x2e);
        m8(p);
        byte(p, M::Lda, kind);
        own(p, M::Sta, 0x5b);
        branch(p, M::Bra, &next);
        p.label(format!("battle_reaction_kept_{kind}"));
        m8(p);
        p.label(next);
    }
    abs(p, M::Lda, ACTION);
    emit(p, M::Cmp, A::Immediate, O::Byte(3));
    branch(p, M::Bne, "battle_reaction_chain");
    own(p, M::Lda, 0x2f);
    byte(p, M::And, 0x02);
    branch(p, M::Beq, "battle_reaction_chain");
    byte(p, M::Lda, 4);
    own(p, M::Sta, 0x59);
    p.label("battle_reaction_chain");
    own(p, M::Lda, 0x2e);
    byte(p, M::And, 0x02);
    branch(p, M::Beq, "battle_reaction_guarded");
    byte(p, M::Lda, 5);
    own(p, M::Sta, 0x59);
    p.label("battle_reaction_guarded");
    its(p, M::Lda, 0x2f);
    byte(p, M::And, 0x20);
    branch(p, M::Beq, "battle_reaction_signalled");
    own(p, M::Lda, 0x47);
    branch(p, M::Beq, "battle_reaction_signalled");
    byte(p, M::Lda, 99);
    own(p, M::Sta, 0x4e);
    byte(p, M::Lda, 7);
    own(p, M::Sta, 0x59);
    p.label("battle_reaction_signalled");
    own(p, M::Lda, 0x59);
    branch(p, M::Bne, "battle_reaction_hurt");
    implied(p, M::Rts);
    p.label("battle_reaction_hurt");
    m16(p);
    own(p, M::Lda, 0x0c);
    abs(p, M::Sta, AMOUNT);
    abs(p, M::Stz, AMOUNT + 2);
    abs(p, M::Asl, AMOUNT);
    abs(p, M::Rol, AMOUNT + 2);
    abs(p, M::Lda, AMOUNT);
    implied(p, M::Sec);
    own(p, M::Sbc, 0x0e);
    abs(p, M::Sta, AMOUNT);
    abs(p, M::Lda, AMOUNT + 2);
    word(p, M::Sbc, 0);
    abs(p, M::Sta, AMOUNT + 2);
    branch(p, M::Bpl, "battle_reaction_element_term");
    abs(p, M::Stz, AMOUNT);
    abs(p, M::Stz, AMOUNT + 2);
    p.label("battle_reaction_element_term");
    m8(p);
    own(p, M::Lda, 0x4e);
    branch(p, M::Beq, "battle_reaction_eighth");
    element(p, "battle_reaction", "own");
    p.label("battle_reaction_eighth");
    shift_amount(p, 3);
    m16(p);
    abs(p, M::Lda, AMOUNT);
    abs(p, M::Sta, LOSS);
    abs(p, M::Lda, AMOUNT + 2);
    abs(p, M::Sta, LOSS + 2);
    own(p, M::Lda, 6);
    abs(p, M::Sta, DIVISOR);
    m8(p);
    call(p, "battle_steps");
    own(p, M::Sta, 0x51);
    hp_loss(p, "battle_reaction", "own", LOSS, LOSS + 2);
    implied(p, M::Rts);
}
fn hp_loss(p: &mut Assembler, prefix: &str, whose: &str, low: u16, high: u16) {
    let field = |p: &mut Assembler, m: M, f: u16| {
        if whose == "own" {
            own(p, m, f)
        } else {
            its(p, m, f)
        }
    };
    registers(p);
    m16(p);
    abs(p, M::Lda, high);
    branch(p, M::Bne, &format!("{prefix}_ended"));
    abs(p, M::Lda, low);
    field(p, M::Cmp, 4);
    branch(p, M::Bcc, &format!("{prefix}_survived"));
    p.label(format!("{prefix}_ended"));
    word(p, M::Lda, 0);
    field(p, M::Sta, 4);
    field(p, M::Sta, 0x2e);
    for f in (0x30..0x40).step_by(2) {
        field(p, M::Sta, f);
    }
    m8(p);
    field(p, M::Lda, 0x45);
    byte(p, M::Ora, 0x80);
    field(p, M::Sta, 0x45);
    p.emit_label_ref(M::Bra, A::Relative8, format!("{prefix}_hp_done"));
    p.label(format!("{prefix}_survived"))
        .assume_mode(CpuMode::NATIVE_M16_X16);
    field(p, M::Lda, 4);
    implied(p, M::Sec);
    abs(p, M::Sbc, low);
    field(p, M::Sta, 4);
    m8(p);
    for (value, out) in [(4u16, 0x4fu16), (8, 0x50)] {
        registers(p);
        m16(p);
        field(p, M::Lda, value);
        abs(p, M::Sta, AMOUNT);
        abs(p, M::Stz, AMOUNT + 2);
        field(p, M::Lda, value + 2);
        abs(p, M::Sta, DIVISOR);
        m8(p);
        call(p, "battle_steps");
        registers(p);
        field(p, M::Sta, out);
    }
    p.label(format!("{prefix}_hp_done"));
    registers(p);
}
fn apply(p: &mut Assembler) {
    p.label("battle_apply").assume_mode(CpuMode::NATIVE_M8_X16);
    call(p, "battle_pair");
    its(p, M::Lda, 0x43);
    branch(p, M::Bne, "battle_apply_busy");
    jump(p, "battle_apply_done");
    p.label("battle_apply_busy");
    its(p, M::Lda, 0x45);
    byte(p, M::And, 0x20);
    branch(p, M::Beq, "battle_apply_hit");
    jump(p, "battle_apply_done");
    p.label("battle_apply_hit");
    its(p, M::Lda, 0x49);
    branch(p, M::Beq, "battle_apply_damage");
    jump(p, "battle_apply_not_damage");
    p.label("battle_apply_damage");
    m16(p);
    its(p, M::Lda, 0x4c);
    abs(p, M::Sta, LOSS);
    abs(p, M::Stz, LOSS + 2);
    m8(p);
    hp_loss(p, "battle_apply", "its", LOSS, LOSS + 2);
    call(p, "battle_status");
    jump(p, "battle_apply_cleanup");
    p.label("battle_apply_not_damage");
    emit(p, M::Cmp, A::Immediate, O::Byte(3));
    branch(p, M::Bcs, "battle_apply_cleanup");
    implied(p, M::Pha);
    set_its(p, 0x45, 0x02);
    implied(p, M::Pla);
    emit(p, M::Cmp, A::Immediate, O::Byte(2));
    branch(p, M::Bne, "battle_apply_cleanup");
    m16(p);
    its(p, M::Lda, 4);
    implied(p, M::Clc);
    its(p, M::Adc, 0x4c);
    its(p, M::Cmp, 6);
    branch(p, M::Bcc, "battle_apply_healed");
    its(p, M::Lda, 6);
    p.label("battle_apply_healed");
    its(p, M::Sta, 4);
    m8(p);
    p.label("battle_apply_cleanup");
    registers(p);
    its(p, M::Lda, 0x2f);
    branch(p, M::Bpl, "battle_apply_done");
    m16(p);
    word(p, M::Lda, 0);
    its(p, M::Sta, 0x2e);
    for f in (0x30..0x40).step_by(2) {
        its(p, M::Sta, f);
    }
    m8(p);
    its(p, M::Stz, 0x60);
    p.label("battle_apply_done");
    registers(p);
    byte(p, M::Lda, 1);
    own(p, M::Sta, 0x46);
    implied(p, M::Rts);
}
fn status(p: &mut Assembler) {
    p.label("battle_status").assume_mode(CpuMode::NATIVE_M8_X16);
    registers(p);
    byte(p, M::Lda, 0);
    its(p, M::Sta, 0x55);
    its(p, M::Lda, 0x45);
    branch(p, M::Bmi, "battle_status_exit");
    own(p, M::Lda, 0x48);
    emit(p, M::Cmp, A::Immediate, O::Byte(15));
    branch(p, M::Bne, "battle_apply_status_guard");
    byte(p, M::Lda, 1);
    its(p, M::Sta, 0x3f);
    set_its(p, 0x2f, 0x80);
    p.label("battle_apply_status_guard");
    its(p, M::Lda, 0x2f);
    byte(p, M::And, 0x20);
    branch(p, M::Bne, "battle_status_exit");
    own(p, M::Lda, 0x48);
    branch(p, M::Beq, "battle_status_exit");
    emit(p, M::Cmp, A::Immediate, O::Byte(11));
    branch(p, M::Bcc, "battle_apply_status_one");
    branch(p, M::Bne, "battle_status_exit");
    own(p, M::Lda, 0x3e);
    emit(p, M::Inc, A::Accumulator, O::None);
    its(p, M::Sta, 0x3b);
    set_its(p, 0x2f, 0x08);
    byte(p, M::Lda, 0);
    own(p, M::Sta, 0x3e);
    clear_own(p, 0x2f, 0x40);
    byte(p, M::Lda, 11);
    its(p, M::Sta, 0x55);
    p.label("battle_status_exit");
    implied(p, M::Rts);
    p.label("battle_apply_status_one");
    abs(p, M::Sta, ACTION);
    m16(p);
    word(p, M::Lda, 0);
    for f in (0x30..0x3c).step_by(2) {
        its(p, M::Sta, f);
    }
    m8(p);
    its(p, M::Stz, 0x5a);
    abs(p, M::Lda, ACTION);
    emit(p, M::Cmp, A::Immediate, O::Byte(4));
    branch(p, M::Bcc, "battle_apply_status_lasting");
    emit(p, M::Cmp, A::Immediate, O::Byte(8));
    branch(p, M::Bcc, "battle_apply_status_bit");
    p.label("battle_apply_status_lasting");
    m16(p);
    word(p, M::Lda, 2);
    abs(p, M::Sta, DIVISOR);
    m8(p);
    rng_of(p);
    m16(p);
    abs(p, M::Lda, ACTION);
    word(p, M::And, 0xff);
    implied(p, M::Clc);
    abs(p, M::Adc, TARGET);
    implied(p, M::Tax);
    m8(p);
    abs(p, M::Lda, RANDOM);
    emit(p, M::Inc, A::Accumulator, O::None);
    emit(p, M::Sta, A::AbsoluteX, O::Word(ACTORS + 0x30));
    registers(p);
    p.label("battle_apply_status_bit");
    m16(p);
    abs(p, M::Lda, ACTION);
    word(p, M::And, 0xff);
    implied(p, M::Tay);
    word(p, M::Lda, 1);
    p.label("battle_apply_status_shift");
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Dey);
    branch(p, M::Bne, "battle_apply_status_shift");
    abs(p, M::Sta, FACTOR);
    its(p, M::Lda, 0x2e);
    word(p, M::And, 0xf000);
    abs(p, M::Ora, FACTOR);
    its(p, M::Sta, 0x2e);
    m8(p);
    registers(p);
    abs(p, M::Lda, ACTION);
    its(p, M::Sta, 0x55);
    p.label("battle_apply_status_done");
    implied(p, M::Rts);
}
