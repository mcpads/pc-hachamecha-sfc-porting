use super::*;
pub(super) fn descriptor_message(p: &mut Assembler, who: u8, slot: u8) {
    byte(p, M::Lda, who);
    abs(p, M::Sta, MESSAGE_WHO);
    byte(p, M::Lda, slot);
    abs(p, M::Sta, MESSAGE_SLOT);
    byte(p, M::Lda, KIND_DESCRIPTOR);
    abs(p, M::Sta, MESSAGE_KIND);
    byte(p, M::Lda, 1);
    abs(p, M::Sta, MESSAGE);
}
pub(super) fn pose(p: &mut Assembler, index: u16, busy: u8, selector: u8) {
    byte(p, M::Lda, 1);
    slot(p, M::Sta, index, 0x41);
    byte(p, M::Lda, busy);
    slot(p, M::Sta, index, 0x43);
    byte(p, M::Lda, selector);
    slot(p, M::Sta, index, 0x42);
}
pub(super) fn field_steps(p: &mut Assembler, record: u16) {
    for (value, out) in [(4u16, 0x4fu16), (8, 0x50)] {
        m16(p);
        abs(p, M::Lda, record + value);
        abs(p, M::Sta, AMOUNT);
        abs(p, M::Stz, AMOUNT + 2);
        abs(p, M::Lda, record + value + 2);
        abs(p, M::Sta, DIVISOR);
        m8(p);
        call(p, "battle_steps");
        slot(p, M::Sta, 0, out);
    }
}
pub(super) fn choose_arle(p: &mut Assembler) {
    p.label("battle_choose_arle")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    abs(p, M::Lda, COMMAND);
    word(p, M::Cmp, 31);
    m8(p);
    branch(p, M::Bcs, "battle_choose_arle_action");
    emit(p, M::Cmp, A::Immediate, O::Byte(5));
    branch(p, M::Bne, "battle_choose_arle_not_status");
    byte(p, M::Lda, 1);
    branch(p, M::Bra, "battle_choose_arle_result");
    p.label("battle_choose_arle_not_status");
    emit(p, M::Cmp, A::Immediate, O::Byte(20));
    branch(p, M::Bne, "battle_choose_arle_wait");
    byte(p, M::Lda, 2);
    p.label("battle_choose_arle_result");
    abs(p, M::Sta, RESULT);
    m16(p);
    abs(p, M::Stz, COMMAND);
    m8(p);
    implied(p, M::Clc);
    implied(p, M::Rts);
    p.label("battle_choose_arle_wait");
    byte(p, M::Lda, 1);
    abs(p, M::Sta, MENU);
    implied(p, M::Sec);
    implied(p, M::Rts);
    p.label("battle_choose_arle_action");
    m16(p);
    abs(p, M::Lda, COMMAND);
    word(p, M::Cmp, 40);
    m8(p);
    branch(p, M::Bcc, "battle_choose_arle_take");
    slot(p, M::Lda, 0, 0x2e);
    byte(p, M::And, 4);
    branch(p, M::Beq, "battle_choose_arle_take");
    byte(p, M::Lda, 3);
    branch(p, M::Bra, "battle_choose_arle_result");
    p.label("battle_choose_arle_take");
    abs(p, M::Lda, COMMAND);
    implied(p, M::Sec);
    byte(p, M::Sbc, 31);
    own(p, M::Sta, 0x40);
    implied(p, M::Clc);
    byte(p, M::Adc, 9);
    own(p, M::Sta, 0x42);
    byte(p, M::Lda, 2);
    own(p, M::Sta, 0x44);
    m16(p);
    abs(p, M::Lda, COMMAND);
    word(p, M::Cmp, 40);
    branch(p, M::Bcc, "battle_choose_arle_m16");
    word(p, M::Lda, ACTORS);
    abs(p, M::Sta, crate::sfc_item_use::RECORD);
    m8(p);
    abs(p, M::Lda, COMMAND);
    call(p, "item_use");
    emit(p, M::Ldy, A::Absolute, O::Word(CURRENT));
    byte(p, M::Lda, 0x12);
    own(p, M::Sta, 0x42);
    branch(p, M::Bra, "battle_choose_arle_taken");
    p.label("battle_choose_arle_m16")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    m8(p);
    p.label("battle_choose_arle_taken");
    abs(p, M::Stz, MENU);
    implied(p, M::Clc);
    implied(p, M::Rts);
}
pub(super) fn durations(p: &mut Assembler) {
    p.label("battle_tick_durations")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    byte(p, M::Lda, 0);
    own(p, M::Sta, 0x5a);
    m16(p);
    word(p, M::Lda, 1);
    abs(p, M::Sta, FACTOR);
    abs(p, M::Lda, CURRENT);
    abs(p, M::Sta, WORK);
    m8(p);
    abs(p, M::Stz, ROLL);
    p.label("battle_tick_durations_slot");
    abs(p, M::Lda, ROLL);
    emit(p, M::Cmp, A::Immediate, O::Byte(14));
    branch(p, M::Beq, "battle_tick_durations_next");
    emit(p, M::Ldx, A::Absolute, O::Word(WORK));
    emit(p, M::Lda, A::AbsoluteX, O::Word(ACTORS + 0x30));
    branch(p, M::Beq, "battle_tick_durations_next");
    emit(p, M::Dec, A::AbsoluteX, O::Word(ACTORS + 0x30));
    branch(p, M::Bne, "battle_tick_durations_next");
    m16(p);
    own(p, M::Lda, 0x2e);
    abs(p, M::And, FACTOR);
    branch(p, M::Beq, "battle_tick_durations_kept");
    own(p, M::Lda, 0x2e);
    abs(p, M::Eor, FACTOR);
    own(p, M::Sta, 0x2e);
    m8(p);
    abs(p, M::Lda, ROLL);
    own(p, M::Sta, 0x5a);
    p.label("battle_tick_durations_kept");
    m8(p);
    p.label("battle_tick_durations_next");
    m16(p);
    abs(p, M::Asl, FACTOR);
    abs(p, M::Inc, WORK);
    m8(p);
    abs(p, M::Inc, ROLL);
    abs(p, M::Lda, ROLL);
    emit(p, M::Cmp, A::Immediate, O::Byte(16));
    branch(p, M::Bne, "battle_tick_durations_slot");
    implied(p, M::Rts);
}
pub(super) fn message(p: &mut Assembler, base: u32) {
    p.label("vm_message").assume_mode(CpuMode::NATIVE_M8_X16);
    call(p, "battle_pair");
    emit(p, M::Ldx, A::Absolute, O::Word(PC));
    program(p, M::Lda, base, 1);
    byte(p, M::And, 0x7f);
    abs(p, M::Sta, MESSAGE_SLOT);
    abs(p, M::Lda, CURRENT_INDEX);
    abs(p, M::Sta, MESSAGE_WHO);
    registers(p);
    emit(p, M::Ldx, A::Absolute, O::Word(PC));
    program(p, M::Lda, base, 1);
    emit(p, M::Cmp, A::Immediate, O::Byte(0x80));
    branch(p, M::Bne, "vm_message_not_80");
    call(p, "battle_assess");
    abs(p, M::Stz, MESSAGE_WHO);
    jump(p, "vm_message_play");
    p.label("vm_message_not_80");
    registers(p);
    abs(p, M::Lda, MESSAGE_SLOT);
    branch(p, M::Bne, "vm_message_not_0");
    byte(p, M::Lda, 2);
    abs(p, M::Sta, MESSAGE_WHO);
    jump(p, "vm_message_play");
    p.label("vm_message_not_0");
    emit(p, M::Cmp, A::Immediate, O::Byte(2));
    branch(p, M::Bne, "vm_message_not_2");
    abs(p, M::Lda, TARGET_INDEX);
    abs(p, M::Sta, MESSAGE_WHO);
    its(p, M::Lda, 0x43);
    branch(p, M::Beq, "vm_message_none");
    its(p, M::Lda, 0x4e);
    emit(p, M::Cmp, A::Immediate, O::Byte(36));
    branch(p, M::Bne, "vm_message_play");
    its(p, M::Lda, 0x45);
    byte(p, M::And, 0x20);
    branch(p, M::Beq, "vm_message_none");
    jump(p, "vm_message_play");
    p.label("vm_message_not_2");
    emit(p, M::Cmp, A::Immediate, O::Byte(3));
    branch(p, M::Bne, "vm_message_not_3");
    own(p, M::Lda, 0x45);
    branch(p, M::Bmi, "vm_message_play");
    abs(p, M::Lda, TARGET_INDEX);
    abs(p, M::Sta, MESSAGE_WHO);
    jump(p, "vm_message_play");
    p.label("vm_message_not_3");
    emit(p, M::Cmp, A::Immediate, O::Byte(5));
    branch(p, M::Bne, "vm_message_not_5");
    own(p, M::Lda, 0x59);
    branch(p, M::Beq, "vm_message_none");
    emit(p, M::Cmp, A::Immediate, O::Byte(7));
    branch(p, M::Bne, "vm_message_play");
    byte(p, M::Lda, 2);
    abs(p, M::Sta, MESSAGE_WHO);
    abs(p, M::Sta, MESSAGE_SLOT);
    jump(p, "vm_message_play");
    p.label("vm_message_not_5");
    emit(p, M::Cmp, A::Immediate, O::Byte(8));
    branch(p, M::Bne, "vm_message_play");
    abs(p, M::Stz, MESSAGE_WHO);
    p.label("vm_message_play");
    byte(p, M::Lda, KIND_DESCRIPTOR);
    abs(p, M::Sta, MESSAGE_KIND);
    byte(p, M::Lda, 1);
    abs(p, M::Sta, MESSAGE);
    jump(p, "vm_yield4");
    p.label("vm_message_none");
    jump(p, "vm_next4");
}
type Term = (u16, u16, u8, u16);
pub(super) fn assess(p: &mut Assembler) {
    let rows: [(Term, Term, u8); 8] = [
        ((2, 2, 1, 0), (0, 2, 1, 4), 0),
        ((0, 2, 1, 0), (2, 2, 1, 4), 4),
        ((2, 0x0c, 2, 0), (0, 0x0c, 3, 0), 5),
        ((2, 0x0e, 2, 0), (0, 0x0e, 3, 0), 6),
        ((2, 4, 1, 0), (0, 4, 2, 0), 7),
        ((2, 0x12, 1, 0), (0, 0x12, 2, 0), 8),
        ((2, 2, 1, 0), (0, 2, 1, 1), 1),
        ((0, 2, 1, 0), (2, 2, 1, 1), 3),
    ];
    let value = |p: &mut Assembler, to: u16, (index, field, times, plus): (u16, u16, u8, u16)| {
        abs(p, M::Stz, to);
        abs(p, M::Stz, to + 2);
        for _ in 0..times {
            slot(p, M::Lda, index, field);
            implied(p, M::Clc);
            abs(p, M::Adc, to);
            abs(p, M::Sta, to);
            word(p, M::Lda, 0);
            abs(p, M::Adc, to + 2);
            abs(p, M::Sta, to + 2);
        }
        word(p, M::Lda, plus);
        implied(p, M::Clc);
        abs(p, M::Adc, to);
        abs(p, M::Sta, to);
        word(p, M::Lda, 0);
        abs(p, M::Adc, to + 2);
        abs(p, M::Sta, to + 2);
    };
    p.label("battle_assess").assume_mode(CpuMode::NATIVE_M8_X16);
    for (i, (lhs, rhs, index)) in rows.into_iter().enumerate() {
        let next = format!("battle_assess_not_{i}");
        m16(p);
        value(p, AMOUNT, lhs);
        value(p, WORK, rhs);
        abs(p, M::Lda, WORK + 2);
        abs(p, M::Cmp, AMOUNT + 2);
        branch(p, M::Bcc, &format!("battle_assess_yes_{i}"));
        branch(p, M::Bne, &next);
        abs(p, M::Lda, WORK);
        abs(p, M::Cmp, AMOUNT);
        branch(p, M::Bcs, &next);
        p.label(format!("battle_assess_yes_{i}"));
        m8(p);
        byte(p, M::Lda, index);
        slot(p, M::Sta, 0, 0x53);
        implied(p, M::Rts);
        p.label(next).assume_mode(CpuMode::NATIVE_M16_X16);
        m8(p);
    }
    byte(p, M::Lda, 2);
    slot(p, M::Sta, 0, 0x53);
    implied(p, M::Rts);
}
pub(super) fn status(p: &mut Assembler, base: u32) {
    p.label("vm_status").assume_mode(CpuMode::NATIVE_M8_X16);
    program(p, M::Lda, base, 1);
    branch(p, M::Bne, "vm_status_battle");
    call(p, "status_prepare");
    jump(p, "vm_next4");
    p.label("vm_status_battle");
    field_steps(p, ACTORS);
    m16(p);
    slot(p, M::Lda, 0, 0x2e);
    call(p, "status_presentation");
    jump(p, "vm_next4");
    p.label("status_prepare")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    field_steps(p, STATS);
    m16(p);
    abs(p, M::Lda, STATS + 0x2e);
    p.label("status_presentation")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    word(p, M::And, 0x7ff);
    abs(p, M::Sta, FACTOR);
    emit(p, M::Ldx, A::Immediate, O::Word(1));
    word(p, M::Lda, 2);
    p.label("vm_status_bit");
    abs(p, M::Cmp, FACTOR);
    branch(p, M::Beq, "vm_status_found");
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Inx);
    emit(p, M::Cpx, A::Immediate, O::Word(11));
    branch(p, M::Bne, "vm_status_bit");
    emit(p, M::Ldx, A::Immediate, O::Word(0));
    p.label("vm_status_found");
    m8(p);
    implied(p, M::Txa);
    slot(p, M::Sta, 0, 0x55);
    for (from, out, minus) in [(STATS + 0x1a, 0x56u16, 0x57u8), (STATS + 0x1b, 0x57, 0x5c)] {
        let none = format!("vm_status_none_{out:x}");
        abs(p, M::Lda, from);
        branch(p, M::Beq, &none);
        implied(p, M::Sec);
        byte(p, M::Sbc, minus);
        p.label(none);
        slot(p, M::Sta, 0, out);
    }
    implied(p, M::Rts);
}
