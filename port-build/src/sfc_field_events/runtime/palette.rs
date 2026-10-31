use super::*;
pub(super) fn emit(p: &mut w65c816::Assembler, tables: &Tables) -> Result<u32> {
    use crate::sfc::emit;
    let Tables {
        layout,
        program,
        recolours,
        ..
    } = *tables;
    let section = |name: &str| -> Result<u32> { layout.cpu_address(layout.section(name)?.offset) };
    p.label("field_op_39");
    long_x(p, M::Lda, program + 1);
    branch(p, M::Bne, "field_recolour_done");
    m16(p);
    long_x(p, M::Lda, program + 4);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Tay);
    m8(p);
    implied(p, M::Phx);
    implied(p, M::Tyx);
    long_x(p, M::Lda, recolours);
    branch(p, M::Beq, "field_recolour_none");
    for i in 0..2 {
        long_x(p, M::Lda, recolours);
        abs(p, M::Cmp, LIVE + i);
        branch(p, M::Bne, &format!("field_recolour_{i}"));
        long_x(p, M::Lda, recolours + 1);
        abs(p, M::Sta, LIVE + i);
        p.label(format!("field_recolour_{i}"));
    }
    implied(p, M::Plx);
    call(p, "field_picture_view");
    p.label("field_recolour_done");
    jump(p, "field_next8");
    p.label("field_recolour_none");
    implied(p, M::Plx);
    jump(p, "field_next8");
    let views = crate::sfc_b1::VIEW_JOB;
    let page_delays = section("b1_page_delays")?;
    p.label("field_wall_colours")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    emit(p, M::Lda, A::Immediate, O::Word(views as u16));
    abs(p, M::Sta, 0xe0);
    m8(p);
    write(p, 0xe2, (views >> 16) as u8);
    m16(p);
    emit(p, M::Ldy, A::Immediate, O::Word(0));
    emit(p, M::Lda, A::DirectPageIndirectLongIndexedY, O::Byte(0xe0));
    abs(p, M::Sta, 0xe4);
    emit(p, M::Ldy, A::Immediate, O::Word(2));
    emit(p, M::Lda, A::DirectPageIndirectLongIndexedY, O::Byte(0xe0));
    abs(p, M::Sta, 0xe6);
    emit(p, M::Ldy, A::Immediate, O::Word(3));
    emit(p, M::Lda, A::DirectPageIndirectLongIndexedY, O::Byte(0xe0));
    emit(p, M::And, A::Immediate, O::Word(0xff));
    for _ in 0..3 {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    abs(p, M::Sta, FADE_TEMP);
    emit(p, M::Ldy, A::Immediate, O::Word(0));
    p.label("field_wall_find");
    emit(p, M::Lda, A::DirectPageIndirectLongIndexedY, O::Byte(0xe4));
    emit(
        p,
        M::Cmp,
        A::Immediate,
        O::Word((crate::sfc_b1::VIEW_COLORS[0][0] as u16) << 8 | 1),
    );
    branch(p, M::Beq, "field_wall_found");
    implied(p, M::Tya);
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Word(8));
    implied(p, M::Tay);
    abs(p, M::Cpy, FADE_TEMP);
    branch(p, M::Bne, "field_wall_find");
    m8(p);
    implied(p, M::Sec);
    implied(p, M::Rts);
    p.label("field_wall_found")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    for _ in 0..5 {
        implied(p, M::Iny);
    }
    emit(p, M::Lda, A::DirectPageIndirectLongIndexedY, O::Byte(0xe4));
    abs(p, M::Sta, 0xe8);
    implied(p, M::Iny);
    emit(p, M::Lda, A::DirectPageIndirectLongIndexedY, O::Byte(0xe4));
    abs(p, M::Sta, 0xe9);
    m8(p);
    implied(p, M::Clc);
    implied(p, M::Rts);
    p.label("field_op_3a");
    long_x(p, M::Lda, program + 1);
    abs(p, M::Sta, FADE_START);
    long_x(p, M::Lda, program + 2);
    abs(p, M::Sta, FADE_COUNT);
    long_x(p, M::Lda, program + 3);
    abs(p, M::Sta, FADE_TEMP + 3);
    implied(p, M::Phx);
    call(p, "field_wall_colours");
    branch(p, M::Bcc, "field_shade_found");
    implied(p, M::Plx);
    jump(p, "field_next4");
    p.label("field_shade_found")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    emit(p, M::Ldy, A::Immediate, O::Word(0));
    abs(p, M::Lda, FADE_TEMP + 3);
    m16(p);
    branch(p, M::Bpl, "field_shade_left");
    for (name, right) in [("right", true), ("left", false)] {
        p.label(format!("field_shade_{name}"))
            .assume_mode(CpuMode::NATIVE_M16_X16);
        emit(p, M::Lda, A::DirectPageIndirectLongIndexedY, O::Byte(0xe8));
        if right {
            for _ in 0..4 {
                emit(p, M::Lsr, A::Accumulator, O::None);
            }
            emit(p, M::And, A::Immediate, O::Word(0x0421));
        } else {
            emit(p, M::And, A::Immediate, O::Word(0x7bde));
        }
        implied(p, M::Tyx);
        emit(p, M::Sta, A::AbsoluteLongX, O::Long(FADE_BUFFER));
        implied(p, M::Iny);
        implied(p, M::Iny);
        emit(p, M::Cpy, A::Immediate, O::Word(2 * SHADE_COLORS as u16));
        branch(p, M::Bne, &format!("field_shade_{name}"));
        jump(p, "field_shade_send");
    }
    p.label("field_shade_send")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    m8(p);
    call(p, "field_fade_send");
    implied(p, M::Plx);
    jump(p, "field_next4");
    p.label("field_op_38");
    long_x(p, M::Lda, program + 1);
    emit(p, M::Cmp, A::Immediate, O::Byte(1));
    branch(p, M::Beq, "field_fade");
    emit(p, M::Cmp, A::Immediate, O::Byte(2));
    branch(p, M::Beq, "field_light");
    jump(p, "field_next8");
    p.label("field_light");
    long_x(p, M::Lda, program + 2);
    emit(
        p,
        M::Cmp,
        A::Immediate,
        O::Byte(crate::sfc_b1::VIEW_COLORS[0][0] as u8),
    );
    branch(p, M::Bcs, "field_light_done");
    abs(p, M::Lda, SCREEN_DARK);
    implied(p, M::Sec);
    long_x(p, M::Sbc, program + 4);
    branch(p, M::Bcs, "field_light_set");
    emit(p, M::Lda, A::Immediate, O::Byte(0));
    p.label("field_light_set");
    abs(p, M::Sta, SCREEN_DARK);
    p.label("field_light_done");
    jump(p, "field_next8");
    p.label("field_fade");
    abs(p, M::Stz, crate::sfc_ending::SCREEN_WHITE);
    long_x(p, M::Lda, program + 2);
    emit(
        p,
        M::Cmp,
        A::Immediate,
        O::Byte(crate::sfc_b1::VIEW_COLORS[0][0] as u8),
    );
    branch(p, M::Bcs, "field_fade_view");
    long_x(p, M::Lda, program + 7);
    implied(p, M::Clc);
    abs(p, M::Adc, SCREEN_DARK);
    branch(p, M::Bcs, "field_fade_black");
    emit(p, M::Cmp, A::Immediate, O::Byte(249));
    branch(p, M::Bcc, "field_fade_dark");
    p.label("field_fade_black");
    emit(p, M::Lda, A::Immediate, O::Byte(248));
    p.label("field_fade_dark");
    abs(p, M::Sta, SCREEN_DARK);
    jump(p, "field_next8");
    p.label("field_fade_view");
    long_x(p, M::Lda, program + 7);
    for _ in 0..3 {
        emit(p, M::Lsr, A::Accumulator, O::None);
    }
    implied(p, M::Clc);
    abs(p, M::Adc, FADE_LEVEL);
    emit(p, M::Cmp, A::Immediate, O::Byte(32));
    branch(p, M::Bcc, "field_fade_level");
    emit(p, M::Lda, A::Immediate, O::Byte(31));
    p.label("field_fade_level");
    abs(p, M::Sta, FADE_LEVEL);
    long_x(p, M::Lda, program + 3);
    abs(p, M::Sta, FADE_COUNT);
    long_x(p, M::Lda, program + 2);
    abs(p, M::Sta, FADE_START);
    implied(p, M::Phx);
    emit(
        p,
        M::Cmp,
        A::Immediate,
        O::Byte(crate::sfc_b1::VIEW_COLORS[0][0] as u8),
    );
    branch(p, M::Bne, "field_fade_upper");
    call(p, "field_wall_colours");
    m16(p);
    branch(p, M::Bcc, "field_fade_source");
    m8(p);
    implied(p, M::Plx);
    jump(p, "field_next8");
    p.label("field_fade_upper")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    emit(p, M::Lda, A::Immediate, O::Word(views as u16));
    abs(p, M::Sta, 0xe0);
    m8(p);
    write(p, 0xe2, (views >> 16) as u8);
    m16(p);
    emit(p, M::Ldy, A::Immediate, O::Word(0));
    emit(p, M::Lda, A::DirectPageIndirectLongIndexedY, O::Byte(0xe0));
    abs(p, M::Sta, 0xe4);
    emit(p, M::Ldy, A::Immediate, O::Word(2));
    emit(p, M::Lda, A::DirectPageIndirectLongIndexedY, O::Byte(0xe0));
    abs(p, M::Sta, 0xe6);
    emit(p, M::Ldy, A::Immediate, O::Word(3));
    emit(p, M::Lda, A::DirectPageIndirectLongIndexedY, O::Byte(0xe0));
    emit(p, M::Dec, A::Accumulator, O::None);
    for _ in 0..3 {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Word(5));
    implied(p, M::Tay);
    emit(p, M::Lda, A::DirectPageIndirectLongIndexedY, O::Byte(0xe4));
    abs(p, M::Sta, 0xe8);
    implied(p, M::Iny);
    emit(p, M::Lda, A::DirectPageIndirectLongIndexedY, O::Byte(0xe4));
    abs(p, M::Sta, 0xe9);
    abs(p, M::Lda, FADE_START);
    emit(p, M::And, A::Immediate, O::Word(0xff));
    implied(p, M::Sec);
    emit(p, M::Sbc, A::Immediate, O::Word(192));
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Clc);
    abs(p, M::Adc, 0xe8);
    abs(p, M::Sta, 0xe8);
    p.label("field_fade_source")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    abs(p, M::Lda, FADE_LEVEL);
    emit(p, M::And, A::Immediate, O::Word(0xff));
    abs(p, M::Sta, FADE_UNITS);
    for i in 1..3u16 {
        for _ in 0..5 {
            emit(p, M::Asl, A::Accumulator, O::None);
        }
        abs(p, M::Sta, FADE_UNITS + 2 * i);
    }
    emit(p, M::Ldy, A::Immediate, O::Word(0));
    p.label("field_fade_colour");
    emit(p, M::Lda, A::DirectPageIndirectLongIndexedY, O::Byte(0xe8));
    abs(p, M::Sta, FADE_TEMP);
    abs(p, M::Stz, FADE_TEMP + 2);
    for (i, mask) in [0x001fu16, 0x03e0, 0x7c00].into_iter().enumerate() {
        abs(p, M::Lda, FADE_TEMP);
        emit(p, M::And, A::Immediate, O::Word(mask));
        implied(p, M::Sec);
        abs(p, M::Sbc, FADE_UNITS + 2 * i as u16);
        branch(p, M::Bcs, &format!("field_fade_kept_{i}"));
        emit(p, M::Lda, A::Immediate, O::Word(0));
        p.label(format!("field_fade_kept_{i}"));
        abs(p, M::Ora, FADE_TEMP + 2);
        abs(p, M::Sta, FADE_TEMP + 2);
    }
    implied(p, M::Tyx);
    emit(p, M::Sta, A::AbsoluteLongX, O::Long(FADE_BUFFER));
    implied(p, M::Iny);
    implied(p, M::Iny);
    m8(p);
    implied(p, M::Tya);
    emit(p, M::Lsr, A::Accumulator, O::None);
    abs(p, M::Cmp, FADE_COUNT);
    m16(p);
    branch(p, M::Bne, "field_fade_colour");
    m8(p);
    call(p, "field_fade_send");
    implied(p, M::Plx);
    jump(p, "field_next8");
    p.label("field_fade_send")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    emit(p, M::Lda, A::Immediate, O::Byte(1));
    emit(p, M::Sta, A::AbsoluteLong, O::Long(FADE_LIST));
    abs(p, M::Lda, FADE_START);
    emit(p, M::Sta, A::AbsoluteLong, O::Long(FADE_LIST + 1));
    emit(p, M::Lda, A::Immediate, O::Byte(0));
    emit(p, M::Sta, A::AbsoluteLong, O::Long(FADE_LIST + 2));
    emit(p, M::Sta, A::AbsoluteLong, O::Long(FADE_LIST + 4));
    emit(p, M::Sta, A::AbsoluteLong, O::Long(FADE_JOB + 4));
    abs(p, M::Lda, FADE_COUNT);
    emit(p, M::Asl, A::Accumulator, O::None);
    emit(p, M::Sta, A::AbsoluteLong, O::Long(FADE_LIST + 3));
    for (i, b) in FADE_BUFFER.to_le_bytes()[..3].iter().enumerate() {
        emit(p, M::Lda, A::Immediate, O::Byte(*b));
        emit(
            p,
            M::Sta,
            A::AbsoluteLong,
            O::Long(FADE_LIST + 5 + i as u32),
        );
    }
    for (i, b) in FADE_LIST.to_le_bytes()[..3].iter().enumerate() {
        emit(p, M::Lda, A::Immediate, O::Byte(*b));
        emit(p, M::Sta, A::AbsoluteLong, O::Long(FADE_JOB + i as u32));
    }
    for (i, b) in FADE_JOB.to_le_bytes()[..3].iter().enumerate() {
        write(p, 0xe0 + i as u16, *b);
    }
    emit(p, M::Lda, A::Immediate, O::Byte(1));
    emit(p, M::Sta, A::AbsoluteLong, O::Long(FADE_JOB + 3));
    emit(p, M::Ldx, A::Immediate, O::Word(crate::sfc_b1::FADE_SLOT));
    jump(p, "b1_enqueue");
    Ok(page_delays)
}
