use super::*;
pub(super) fn emit(p: &mut w65c816::Assembler, tables: &Tables) -> Result<()> {
    use crate::sfc::emit;
    let Tables {
        layout,
        program,
        events,
        ..
    } = *tables;
    let section = |name: &str| -> Result<u32> { layout.cpu_address(layout.section(name)?.offset) };
    p.label("field_run").assume_mode(CpuMode::NATIVE_M8_X16);
    call(p, "field_run_events");
    abs(p, M::Lda, VIEW_DIRTY);
    branch(p, M::Beq, "field_view_clean");
    abs(p, M::Stz, VIEW_DIRTY);
    jump(p, "field_picture_view");
    p.label("field_view_clean");
    implied(p, M::Rts);
    p.label("field_run_events")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    abs(p, M::Lda, PC);
    emit(p, M::Cmp, A::Immediate, O::Word(0xffff));
    m8(p);
    branch(p, M::Bne, "field_run_steps");
    call(p, "field_select");
    m16(p);
    abs(p, M::Lda, SELECTED);
    emit(p, M::Cmp, A::Immediate, O::Word(0xffff));
    m8(p);
    branch(p, M::Bne, "field_run_selected");
    implied(p, M::Rts);
    p.label("field_run_selected");
    abs(p, M::Lda, KIND);
    emit(p, M::Cmp, A::Immediate, O::Byte(1));
    branch(p, M::Beq, "field_run_call");
    emit(p, M::Cmp, A::Immediate, O::Byte(2));
    branch(p, M::Beq, "field_run_context");
    emit(p, M::Cmp, A::Immediate, O::Byte(3));
    branch(p, M::Beq, "field_run_shop");
    implied(p, M::Rts);
    p.label("field_run_shop");
    call(p, "sound_field_song");
    abs(p, M::Sta, SHOP_RETURN);
    emit(
        p,
        M::Lda,
        A::Immediate,
        O::Byte(crate::sfc_sound::song("shop")),
    );
    call(p, "sound_play");
    p.label("field_run_context");
    write(p, CONTEXT, 1);
    p.label("field_run_call");
    abs(p, M::Stz, DEPTH);
    m16(p);
    abs(p, M::Lda, SELECTED);
    abs(p, M::Sta, PC);
    m8(p);
    p.label("field_run_script");
    write(p, STEPS, 0);
    jump(p, "field_step");
    p.label("field_run_steps");
    write(p, STEPS, STEP_LIMIT);
    p.label("field_step");
    abs(p, M::Dec, STEPS);
    branch(p, M::Bne, "field_fetch");
    implied(p, M::Rts);
    p.label("field_fetch");
    abs(p, M::Ldx, PC);
    long_x(p, M::Lda, program);
    for op in [
        0x01u8, 0x03, 0x05, 0x06, 0x07, 0x77, 0x10, 0x11, 0x12, 0x13, 0x14, 0x24, 0x25, 0x26, 0x28,
        0x29, 0x2f, 0x31, 0x32, 0x34, 0x38, 0x39, 0x3b, 0x3f, 0x4b, 0x6f, 0x70, 0x75, 0x78, 0x7a,
        0x7b, 0x7e, 0x81, 0x84, 0x02, 0x04, 0x40, 0x8c, 0x69, 0x83, 0x93, 0x3a, 0x3c, 0x8b, 0x27,
        0x86, 0x44, 0x9b, 0x35, 0x15, 0x85, 0x97, 0x4c, 0x88, 0x8f, 0x96,
    ] {
        emit(p, M::Cmp, A::Immediate, O::Byte(op));
        branch(p, M::Bne, &format!("field_not_{op:02x}"));
        jump(p, &format!("field_op_{op:02x}"));
        p.label(format!("field_not_{op:02x}"));
    }
    p.label("field_end");
    m16(p);
    emit(p, M::Lda, A::Immediate, O::Word(0xffff));
    abs(p, M::Sta, PC);
    m8(p);
    abs(p, M::Lda, KIND);
    emit(p, M::Cmp, A::Immediate, O::Byte(2));
    branch(p, M::Bcc, "field_end_unwrapped");
    abs(p, M::Stz, CONTEXT);
    p.label("field_end_unwrapped");
    implied(p, M::Rts);
    for length in [4u16, 8, 12, 16] {
        p.label(format!("field_next{length}"));
        m16(p);
        abs(p, M::Lda, PC);
        implied(p, M::Clc);
        emit(p, M::Adc, A::Immediate, O::Word(length));
        abs(p, M::Sta, PC);
        m8(p);
        jump(p, "field_step");
        p.label(format!("field_yield{length}"));
        m16(p);
        abs(p, M::Lda, PC);
        implied(p, M::Clc);
        emit(p, M::Adc, A::Immediate, O::Word(length));
        abs(p, M::Sta, PC);
        m8(p);
        implied(p, M::Rts);
    }
    for at in [4u32, 8, 12] {
        p.label(format!("field_goto{at}"));
        m16(p);
        long_x(p, M::Lda, program + at);
        abs(p, M::Sta, PC);
        m8(p);
        jump(p, "field_step");
    }
    p.label("field_op_01");
    jump(p, "field_yield4");
    let shops = section("shop_descriptors")?;
    p.label("field_op_69");
    implied(p, M::Phx);
    m16(p);
    long_x(p, M::Lda, program + 4);
    abs(p, M::Sta, TEMP);
    long_x(p, M::Lda, program + 6);
    abs(p, M::Sta, TEMP + 2);
    emit(p, M::Ldx, A::Immediate, O::Word(0));
    p.label("field_shop_row");
    long_x(p, M::Lda, shops);
    long_x(p, M::Ora, shops + 2);
    branch(p, M::Beq, "field_shop_none");
    long_x(p, M::Lda, shops);
    abs(p, M::Cmp, TEMP);
    branch(p, M::Bne, "field_shop_next");
    long_x(p, M::Lda, shops + 2);
    abs(p, M::Cmp, TEMP + 2);
    branch(p, M::Bne, "field_shop_next");
    m8(p);
    long_x(p, M::Lda, shops + 4);
    call(p, "shop_open");
    branch(p, M::Bra, "field_shop_done");
    p.label("field_shop_next")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    implied(p, M::Txa);
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Word(5));
    implied(p, M::Tax);
    branch(p, M::Bra, "field_shop_row");
    p.label("field_shop_none")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    m8(p);
    p.label("field_shop_done");
    implied(p, M::Plx);
    jump(p, "field_next8");
    p.label("field_shop_left")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, SHOP_RETURN);
    emit(p, M::Cmp, A::Immediate, O::Byte(0xff));
    branch(p, M::Beq, "field_shop_left_quiet");
    call(p, "sound_play");
    branch(p, M::Bra, "field_shop_left_song");
    p.label("field_shop_left_quiet");
    call(p, "sound_stop");
    p.label("field_shop_left_song");
    abs(p, M::Stz, CONTEXT);
    abs(p, M::Stz, BLOCK);
    call(p, "field_find");
    branch(p, M::Bcs, "field_shop_left_no_event");
    long_x(p, M::Lda, events + 4);
    branch(p, M::Bra, "field_shop_left_event");
    p.label("field_shop_left_no_event");
    emit(p, M::Lda, A::Immediate, O::Byte(255));
    p.label("field_shop_left_event");
    abs(p, M::Sta, 0x505);
    call(p, "b1_render");
    crate::sfc::write_word(p, 0x501, crate::sfc_b1_menu::EMPTY_PAGE.into());
    jump(p, "b1_page");
    Ok(())
}
