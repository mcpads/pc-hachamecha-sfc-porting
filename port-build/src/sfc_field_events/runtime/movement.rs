use super::*;
pub(super) fn emit(p: &mut w65c816::Assembler, tables: &Tables) -> Result<()> {
    use crate::sfc::emit;
    let Tables {
        layout,
        boundary,
        program,
        events,
        ..
    } = *tables;
    let section = |name: &str| -> Result<u32> { layout.cpu_address(layout.section(name)?.offset) };
    p.label("field_op_70");
    long_x(p, M::Lda, program + 4);
    abs(p, M::Sta, NEXT_FLOOR);
    m16(p);
    abs(p, M::Lda, PC);
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Word(8));
    abs(p, M::Sta, RESUME);
    m8(p);
    call(p, "field_floor_exit");
    abs(p, M::Lda, NEXT_FLOOR);
    abs(p, M::Sta, 0x50c);
    call(p, "field_floor_bounds");
    call(p, "field_floor_entry");
    call(p, "b1_map_visit");
    m16(p);
    abs(p, M::Lda, RESUME);
    abs(p, M::Sta, PC);
    m8(p);
    write(p, 0x505, 255);
    crate::sfc::write_word(p, 0x501, 2);
    call(p, "b1_page");
    abs(p, M::Lda, 0x50c);
    emit(p, M::Cmp, A::Immediate, O::Byte(boundary));
    branch(p, M::Bcs, "field_floor_kept");
    call(p, "b1_render");
    p.label("field_floor_kept");
    jump(p, "field_step");
    p.label("field_op_75");
    abs(p, M::Stz, CONTEXT);
    abs(p, M::Lda, CLICK);
    branch(p, M::Bne, "field_click");
    jump(p, "field_next16");
    p.label("field_click");
    write(p, CONTEXT, 1);
    long_x(p, M::Lda, program + 1);
    branch(p, M::Bne, "field_click_message");
    jump(p, "field_goto12");
    p.label("field_click_message");
    m16(p);
    long_x(p, M::Lda, program + 12);
    for _ in 0..3 {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    implied(p, M::Tax);
    m8(p);
    call(p, "field_show");
    jump(p, "field_yield16");
    p.label("field_op_78");
    abs(p, M::Lda, 0x500);
    emit(p, M::And, A::Immediate, O::Byte(3));
    abs(p, M::Sta, TEMP);
    for (bit, direction) in [(1u8, 0u8), (2, 1), (4, 3), (8, 2)] {
        long_x(p, M::Lda, program + 3);
        emit(p, M::Cmp, A::Immediate, O::Byte(bit));
        branch(p, M::Bne, &format!("field_place_not_{bit}"));
        write(p, TEMP, direction);
        p.label(format!("field_place_not_{bit}"));
    }
    long_x(p, M::Lda, program + 2);
    for _ in 0..5 {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    abs(p, M::Sta, TEMP + 1);
    long_x(p, M::Lda, program + 1);
    emit(p, M::Asl, A::Accumulator, O::None);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Clc);
    abs(p, M::Adc, TEMP + 1);
    abs(p, M::Ora, TEMP);
    abs(p, M::Sta, 0x500);
    abs(p, M::Stz, BLOCK);
    p.label("field_place_shown");
    call(p, "field_find");
    branch(p, M::Bcs, "field_place_no_event");
    long_x(p, M::Lda, events + 4);
    branch(p, M::Bra, "field_place_event");
    p.label("field_place_no_event");
    emit(p, M::Lda, A::Immediate, O::Byte(255));
    p.label("field_place_event");
    abs(p, M::Sta, 0x505);
    call(p, "b1_render");
    jump(p, "field_next4");
    p.label("field_op_8f");
    abs(p, M::Lda, 0x500);
    implied(p, M::Tay);
    emit(p, M::And, A::Immediate, O::Byte(0xfc));
    abs(p, M::Sta, TEMP);
    implied(p, M::Tya);
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Byte(2));
    emit(p, M::And, A::Immediate, O::Byte(3));
    abs(p, M::Ora, TEMP);
    abs(p, M::Sta, 0x500);
    jump(p, "field_place_shown");
    p.label("field_op_7a");
    long_x(p, M::Lda, program + 1);
    branch(p, M::Bne, "field_start_named");
    m16(p);
    abs(p, M::Lda, PC);
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Word(4));
    abs(p, M::Sta, SELECTED);
    m8(p);
    jump(p, "field_next4");
    p.label("field_start_named");
    m16(p);
    long_x(p, M::Lda, program + 4);
    abs(p, M::Sta, SELECTED);
    m8(p);
    jump(p, "field_next8");
    p.label("field_op_7b");
    long_x(p, M::Lda, program + 1);
    emit(p, M::Eor, A::Immediate, O::Byte(1));
    abs(p, M::Sta, CONTEXT);
    jump(p, "field_next4");
    p.label("field_op_7e");
    long_x(p, M::Lda, program + 1);
    abs(p, M::Sta, BLOCK);
    jump(p, "field_next4");
    p.label("field_op_93");
    long_x(p, M::Lda, program + 1);
    abs(p, M::Sta, HEALING_WAIT);
    jump(p, "field_next4");
    p.label("field_op_81");
    m16(p);
    long_x(p, M::Lda, program + 2);
    emit(p, M::And, A::Immediate, O::Word(255));
    implied(p, M::Sec);
    emit(
        p,
        M::Sbc,
        A::Immediate,
        O::Word(u16::from(crate::sfc_b1_menu::FIRST_ITEM)),
    );
    implied(p, M::Tay);
    m8(p);
    let inventory = crate::sfc_b1_menu::INVENTORY;
    long_x(p, M::Lda, program + 1);
    branch(p, M::Bne, "field_item_less");
    emit(p, M::Lda, A::AbsoluteY, O::Word(inventory));
    emit(p, M::Cmp, A::Immediate, O::Byte(255));
    branch(p, M::Beq, "field_item_done");
    emit(p, M::Inc, A::Accumulator, O::None);
    branch(p, M::Bra, "field_item_store");
    p.label("field_item_less");
    emit(p, M::Lda, A::AbsoluteY, O::Word(inventory));
    branch(p, M::Beq, "field_item_done");
    emit(p, M::Dec, A::Accumulator, O::None);
    p.label("field_item_store");
    emit(p, M::Sta, A::AbsoluteY, O::Word(inventory));
    p.label("field_item_done");
    jump(p, "field_next4");
    let cells = crate::sfc_floor_walls::CELLS;
    let deltas = section("view_deltas")?;
    p.label("field_op_84");
    m16(p);
    long_x(p, M::Lda, program + 1);
    emit(p, M::And, A::Immediate, O::Word(63));
    abs(p, M::Sta, TEMP);
    m8(p);
    long_x(p, M::Lda, program + 2);
    emit(p, M::And, A::Immediate, O::Byte(0x7f));
    for (bit, facing) in [(1u8, 0u8), (2, 1), (8, 2), (4, 3)] {
        emit(p, M::Cmp, A::Immediate, O::Byte(bit));
        branch(p, M::Bne, &format!("field_wall_not_{bit}"));
        emit(p, M::Lda, A::Immediate, O::Byte(facing));
        branch(p, M::Bra, "field_wall_facing");
        p.label(format!("field_wall_not_{bit}"));
    }
    p.label("field_wall_facing");
    abs(p, M::Sta, MASK);
    call(p, "field_wall_set");
    long_x(p, M::Lda, program + 2);
    branch(p, M::Bpl, "field_wall_done");
    implied(p, M::Phx);
    m16(p);
    abs(p, M::Lda, MASK);
    emit(p, M::And, A::Immediate, O::Word(3));
    implied(p, M::Tax);
    m8(p);
    long_x(p, M::Lda, deltas + 4);
    for _ in 0..3 {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    implied(p, M::Clc);
    long_x(p, M::Adc, deltas);
    implied(p, M::Clc);
    abs(p, M::Adc, TEMP);
    emit(p, M::And, A::Immediate, O::Byte(63));
    abs(p, M::Sta, TEMP);
    abs(p, M::Lda, MASK);
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Byte(2));
    emit(p, M::And, A::Immediate, O::Byte(3));
    abs(p, M::Sta, MASK);
    implied(p, M::Plx);
    call(p, "field_wall_set");
    p.label("field_wall_done");
    write(p, VIEW_DIRTY, 1);
    jump(p, "field_next4");
    p.label("field_wall_set")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    implied(p, M::Phx);
    long_x(p, M::Lda, program + 3);
    emit(p, M::And, A::Immediate, O::Byte(15));
    m16(p);
    emit(p, M::And, A::Immediate, O::Word(15));
    abs(p, M::Sta, TEMP + 1 + 1);
    emit(p, M::Lda, A::Immediate, O::Word(0xfff0));
    abs(p, M::Sta, END_SCRATCH);
    abs(p, M::Lda, MASK);
    emit(p, M::And, A::Immediate, O::Word(3));
    implied(p, M::Tax);
    p.label("field_wall_shift");
    emit(p, M::Cpx, A::Immediate, O::Word(0));
    branch(p, M::Beq, "field_wall_shifted");
    for at in [TEMP + 2, END_SCRATCH] {
        abs(p, M::Lda, at);
        for _ in 0..4 {
            emit(p, M::Asl, A::Accumulator, O::None);
        }
        if at == END_SCRATCH {
            emit(p, M::Ora, A::Immediate, O::Word(15));
        }
        abs(p, M::Sta, at);
    }
    implied(p, M::Dex);
    branch(p, M::Bra, "field_wall_shift");
    p.label("field_wall_shifted");
    abs(p, M::Lda, 0x50c);
    emit(p, M::And, A::Immediate, O::Word(255));
    emit(p, M::Dec, A::Accumulator, O::None);
    for _ in 0..6 {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    implied(p, M::Clc);
    abs(p, M::Adc, TEMP);
    emit(p, M::And, A::Immediate, O::Word(0x3ff));
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Tax);
    long_x(p, M::Lda, cells);
    abs(p, M::And, END_SCRATCH);
    abs(p, M::Ora, TEMP + 2);
    long_x(p, M::Sta, cells);
    m8(p);
    implied(p, M::Plx);
    implied(p, M::Rts);
    Ok(())
}
