use crate::Result;
use crate::sfc::{emit, implied};
use crate::sfc_battle::{ACTORS, LEVEL, STATS};
use crate::sfc_battle_act::{AMOUNT, abs, branch, byte, call, jump, m8, m16, word};
use sfc_builder::cartridge::CartridgeLayout;
use w65c816::{AddressingMode as A, Assembler, CpuMode, Mnemonic as M, Operand as O};
pub const RECORD: u16 = 0xd52;
const ITEM: u16 = 0xd50;
const HP_GAIN: u16 = 0xd54;
const MP_GAIN: u16 = 0xd56;
const FIELD_FLAG: (u16, u8) = (crate::sfc_field_events::GLOBALS + 0x12, 0x20);
const HEAL_COST: u16 = 8;
const INVENTORY: u16 = crate::sfc_b1_menu::INVENTORY;
const FIRST_ITEM: u8 = crate::sfc_b1_menu::FIRST_ITEM;
fn field(p: &mut Assembler, m: M, offset: u16) {
    emit(p, m, A::AbsoluteY, O::Word(offset));
}
pub fn emit_rules(p: &mut Assembler, layout: &CartridgeLayout) -> Result<()> {
    let thresholds = layout.cpu_address(layout.section("battle_thresholds")?.offset)?;
    p.label("item_use").assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Sta, ITEM);
    implied(p, M::Sec);
    byte(p, M::Sbc, 31);
    abs(p, M::Sta, ACTORS + 0x40);
    m16(p);
    abs(p, M::Stz, HP_GAIN);
    abs(p, M::Stz, MP_GAIN);
    m8(p);
    emit(p, M::Ldy, A::Absolute, O::Word(RECORD));
    abs(p, M::Lda, ITEM);
    let first: &[(u8, &str)] = &[
        (35, "item_heal_spell"),
        (40, "item_hp_32"),
        (41, "item_hp_64"),
        (42, "item_hp_full"),
        (43, "item_mp_32"),
        (44, "item_mp_64"),
        (45, "item_mp_full"),
        (46, "item_both_32"),
        (47, "item_both_64"),
        (48, "item_both_full"),
        (49, "item_speed"),
        (50, "item_accuracy"),
        (51, "item_antidote"),
        (53, "item_apple"),
        (75, "item_flag"),
    ];
    for (i, (item, label)) in first.iter().enumerate() {
        let next = format!("item_first_not_{i}");
        emit(p, M::Cmp, A::Immediate, O::Byte(*item));
        branch(p, M::Bne, &next);
        jump(p, label);
        p.label(next);
    }
    jump(p, "item_second");
    let gain = |p: &mut Assembler, label: &str, hp: Option<u16>, mp: Option<u16>| {
        p.label(label).assume_mode(CpuMode::NATIVE_M8_X16);
        m16(p);
        for (to, v, full) in [(HP_GAIN, hp, 6u16), (MP_GAIN, mp, 0xa)] {
            match v {
                Some(0) => {
                    field(p, M::Lda, full);
                    abs(p, M::Sta, to);
                }
                Some(n) => {
                    word(p, M::Lda, n);
                    abs(p, M::Sta, to);
                }
                None => {}
            }
        }
        m8(p);
        jump(p, "item_second");
    };
    gain(p, "item_hp_32", Some(32), None);
    gain(p, "item_hp_64", Some(64), None);
    gain(p, "item_hp_full", Some(0), None);
    gain(p, "item_mp_32", None, Some(32));
    gain(p, "item_mp_64", None, Some(64));
    gain(p, "item_mp_full", None, Some(0));
    gain(p, "item_both_32", Some(32), Some(32));
    gain(p, "item_both_64", Some(64), Some(64));
    gain(p, "item_both_full", Some(0), Some(0));
    for (label, offset) in [("item_speed", 0x12u16), ("item_accuracy", 0x10)] {
        p.label(label).assume_mode(CpuMode::NATIVE_M8_X16);
        m16(p);
        field(p, M::Lda, offset);
        emit(p, M::Inc, A::Accumulator, O::None);
        field(p, M::Sta, offset);
        m8(p);
        jump(p, "item_second");
    }
    p.label("item_antidote").assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    abs(p, M::Lda, RECORD);
    word(p, M::Cmp, ACTORS);
    branch(p, M::Beq, "item_antidote_battle");
    m8(p);
    abs(p, M::Stz, ITEM);
    jump(p, "item_second");
    p.label("item_antidote_battle")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    field(p, M::Lda, 0x2e);
    word(p, M::And, !0x10);
    field(p, M::Sta, 0x2e);
    m8(p);
    jump(p, "item_second");
    p.label("item_apple").assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    field(p, M::Lda, 6);
    abs(p, M::Sta, HP_GAIN);
    field(p, M::Lda, 0xa);
    abs(p, M::Sta, MP_GAIN);
    abs(p, M::Lda, RECORD);
    word(p, M::Cmp, ACTORS);
    branch(p, M::Beq, "item_apple_done");
    abs(p, M::Lda, LEVEL);
    word(p, M::Cmp, 99);
    branch(p, M::Bcs, "item_apple_done");
    emit(p, M::Asl, A::Accumulator, O::None);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Tax);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(thresholds));
    abs(p, M::Sta, STATS + 0x20);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(thresholds + 2));
    abs(p, M::Sta, STATS + 0x22);
    p.label("item_apple_done");
    m8(p);
    jump(p, "item_second");
    p.label("item_flag").assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    abs(p, M::Lda, RECORD);
    word(p, M::Cmp, ACTORS);
    m8(p);
    branch(p, M::Beq, "item_flag_done");
    abs(p, M::Lda, FIELD_FLAG.0);
    byte(p, M::Ora, FIELD_FLAG.1);
    abs(p, M::Sta, FIELD_FLAG.0);
    p.label("item_flag_done");
    jump(p, "item_second");
    p.label("item_heal_spell")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    field(p, M::Lda, 4);
    field(p, M::Cmp, 6);
    branch(p, M::Bne, "item_heal_spell_cast");
    m8(p);
    byte(p, M::Lda, 9);
    abs(p, M::Sta, ACTORS + 0x5d);
    jump(p, "item_heal_spell_steps");
    p.label("item_heal_spell_cast")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    field(p, M::Lda, 6);
    abs(p, M::Sta, crate::sfc_battle::DIVIDEND);
    word(p, M::Lda, 3);
    abs(p, M::Sta, crate::sfc_battle::DIVISOR);
    m8(p);
    call(p, "battle_divide");
    m16(p);
    abs(p, M::Lda, crate::sfc_battle::QUOTIENT);
    abs(p, M::Sta, HP_GAIN);
    m8(p);
    call(p, "item_display");
    call(p, "item_add_hp");
    emit(p, M::Ldy, A::Absolute, O::Word(RECORD));
    m16(p);
    field(p, M::Lda, 8);
    implied(p, M::Sec);
    word(p, M::Sbc, HEAL_COST);
    field(p, M::Sta, 8);
    m8(p);
    p.label("item_heal_spell_steps");
    emit(p, M::Ldy, A::Absolute, O::Word(RECORD));
    call(p, "item_shortage");
    abs(p, M::Lda, ACTORS + 0x5d);
    emit(p, M::Cmp, A::Immediate, O::Byte(9));
    branch(p, M::Bne, "item_heal_spell_healed");
    byte(p, M::Lda, 8);
    branch(p, M::Bra, "item_heal_spell_pick");
    p.label("item_heal_spell_healed");
    abs(p, M::Lda, ACTORS + 0x4f);
    emit(p, M::Cmp, A::Immediate, O::Byte(7));
    branch(p, M::Bcc, "item_heal_spell_pick");
    byte(p, M::Lda, 7);
    p.label("item_heal_spell_pick");
    abs(p, M::Sta, crate::sfc_field_events::FOLLOW_PICK);
    jump(p, "item_second");
    p.label("item_second").assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Stz, ACTORS + 0x5c);
    emit(p, M::Ldy, A::Absolute, O::Word(RECORD));
    abs(p, M::Lda, ITEM);
    for (from, to, kind) in [(40u8, 42u8, 1u8), (43, 45, 2), (46, 48, 3), (53, 53, 0)] {
        let next = format!("item_second_not_{from}");
        emit(p, M::Cmp, A::Immediate, O::Byte(from));
        branch(p, M::Bcc, &next);
        emit(p, M::Cmp, A::Immediate, O::Byte(to + 1));
        branch(p, M::Bcs, &next);
        byte(p, M::Lda, kind);
        abs(p, M::Sta, ACTORS + 0x5c);
        jump(p, "item_recover");
        p.label(next);
    }
    emit(p, M::Cmp, A::Immediate, O::Byte(49));
    branch(p, M::Bcc, "item_second_not_herb");
    emit(p, M::Cmp, A::Immediate, O::Byte(52));
    branch(p, M::Bcs, "item_second_not_herb");
    jump(p, "item_consume");
    p.label("item_second_not_herb");
    for (from, slot, display, minus) in [(88u8, 0x1au16, 0x56u16, 0x57u8), (93, 0x1b, 0x57, 0x5c)] {
        let next = format!("item_second_not_{from}");
        emit(p, M::Cmp, A::Immediate, O::Byte(from));
        branch(p, M::Bcc, &next);
        emit(p, M::Cmp, A::Immediate, O::Byte(from + 5));
        branch(p, M::Bcs, &next);
        abs(p, M::Cmp, STATS + slot);
        branch(p, M::Bne, &format!("item_wear_{from}"));
        byte(p, M::Lda, 0);
        p.label(format!("item_wear_{from}"));
        abs(p, M::Sta, STATS + slot);
        abs(p, M::Lda, ITEM);
        implied(p, M::Sec);
        byte(p, M::Sbc, minus);
        abs(p, M::Sta, ACTORS + display);
        implied(p, M::Rts);
        p.label(next);
    }
    implied(p, M::Rts);
    p.label("item_recover").assume_mode(CpuMode::NATIVE_M8_X16);
    call(p, "item_display");
    call(p, "item_add");
    emit(p, M::Ldy, A::Absolute, O::Word(RECORD));
    call(p, "item_shortage");
    p.label("item_consume");
    abs(p, M::Lda, ITEM);
    implied(p, M::Sec);
    byte(p, M::Sbc, FIRST_ITEM);
    m16(p);
    word(p, M::And, 0xff);
    implied(p, M::Tax);
    m8(p);
    emit(p, M::Dec, A::AbsoluteX, O::Word(INVENTORY));
    implied(p, M::Rts);
    p.label("item_add").assume_mode(CpuMode::NATIVE_M8_X16);
    call(p, "item_add_hp");
    p.label("item_add_mp_only");
    emit(p, M::Ldy, A::Absolute, O::Word(RECORD));
    m16(p);
    add_to(p, 8, 0xa, MP_GAIN);
    m8(p);
    implied(p, M::Rts);
    p.label("item_add_hp").assume_mode(CpuMode::NATIVE_M8_X16);
    emit(p, M::Ldy, A::Absolute, O::Word(RECORD));
    m16(p);
    add_to(p, 4, 6, HP_GAIN);
    m8(p);
    implied(p, M::Rts);
    fn add_to(p: &mut Assembler, value: u16, max: u16, gain: u16) {
        let kept = format!("item_add_kept_{value}");
        field(p, M::Lda, value);
        implied(p, M::Clc);
        abs(p, M::Adc, gain);
        field(p, M::Cmp, max);
        branch(p, M::Bcc, &kept);
        branch(p, M::Beq, &kept);
        field(p, M::Lda, max);
        p.label(kept);
        field(p, M::Sta, value);
    }
    p.label("item_display").assume_mode(CpuMode::NATIVE_M8_X16);
    for (gain, max, outs) in [
        (HP_GAIN, 6u16, &[0x5du16, 0x5f][..]),
        (MP_GAIN, 0xa, &[0x5e][..]),
    ] {
        let tag = max;
        emit(p, M::Ldy, A::Absolute, O::Word(RECORD));
        m16(p);
        abs(p, M::Lda, gain);
        abs(p, M::Sta, AMOUNT);
        abs(p, M::Stz, AMOUNT + 2);
        field(p, M::Lda, max);
        abs(p, M::Sta, crate::sfc_battle::DIVISOR);
        m8(p);
        call(p, "battle_steps");
        m16(p);
        abs(p, M::Lda, AMOUNT + 2);
        branch(p, M::Bne, &format!("item_display_full_{tag}"));
        abs(p, M::Lda, AMOUNT);
        word(p, M::Cmp, 9);
        branch(p, M::Bcs, &format!("item_display_full_{tag}"));
        m8(p);
        byte(p, M::Lda, 8);
        implied(p, M::Sec);
        abs(p, M::Sbc, AMOUNT);
        branch(p, M::Bra, &format!("item_display_shown_{tag}"));
        p.label(format!("item_display_full_{tag}"))
            .assume_mode(CpuMode::NATIVE_M16_X16);
        m8(p);
        byte(p, M::Lda, 0);
        p.label(format!("item_display_shown_{tag}"));
        for out in outs {
            abs(p, M::Sta, ACTORS + out);
        }
    }
    implied(p, M::Rts);
    p.label("item_shortage").assume_mode(CpuMode::NATIVE_M8_X16);
    for (value, out) in [(4u16, 0x4fu16), (8, 0x50)] {
        let stats = format!("item_shortage_stats_{out:x}");
        let done = format!("item_shortage_done_{out:x}");
        emit(p, M::Ldy, A::Absolute, O::Word(RECORD));
        m16(p);
        field(p, M::Lda, value);
        abs(p, M::Sta, AMOUNT);
        abs(p, M::Stz, AMOUNT + 2);
        field(p, M::Lda, value + 2);
        abs(p, M::Sta, crate::sfc_battle::DIVISOR);
        m8(p);
        call(p, "battle_steps");
        implied(p, M::Pha);
        m16(p);
        abs(p, M::Lda, RECORD);
        word(p, M::Cmp, STATS);
        m8(p);
        branch(p, M::Beq, &stats);
        implied(p, M::Pla);
        field(p, M::Sta, out);
        branch(p, M::Bra, &done);
        p.label(stats);
        implied(p, M::Pla);
        abs(p, M::Sta, ACTORS + out);
        p.label(done);
    }
    implied(p, M::Rts);
    Ok(())
}
