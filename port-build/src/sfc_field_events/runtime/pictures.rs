use super::*;
pub(super) fn emit(p: &mut w65c816::Assembler, tables: &Tables) -> Result<()> {
    use crate::sfc::emit;
    let Tables {
        boundary,
        program,
        pictures,
        picture_songs,
        ..
    } = *tables;
    p.label("field_op_77");
    long_x(p, M::Lda, program + 4);
    emit(p, M::Inc, A::Accumulator, O::None);
    abs(p, M::Sta, CREATED);
    implied(p, M::Phx);
    call(p, "field_pictures_left");
    implied(p, M::Plx);
    abs(p, M::Lda, 0x500);
    abs(p, M::Sta, LIVE_STATE);
    call(p, "field_picture_unmoved");
    for i in 0..2 {
        abs(p, M::Lda, LIVE + i);
        abs(p, M::Cmp, CREATED);
        branch(p, M::Beq, "field_created");
    }
    for i in 0..2 {
        abs(p, M::Lda, LIVE + i);
        branch(p, M::Bne, &format!("field_create_{i}"));
        abs(p, M::Lda, CREATED);
        abs(p, M::Sta, LIVE + i);
        abs(p, M::Stz, LIVE_SLOTS + i);
        branch(p, M::Bra, "field_created");
        p.label(format!("field_create_{i}"));
    }
    p.label("field_created");
    call(p, "field_picture_view");
    jump(p, "field_next8");
    p.label("field_op_28");
    long_x(p, M::Lda, program + 1);
    branch(p, M::Beq, "field_register");
    write(p, REFERENCE, 2);
    jump(p, "field_next4");
    p.label("field_picture_unmoved")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    for i in 0..8 {
        abs(p, M::Stz, PICTURE_X + i);
    }
    implied(p, M::Rts);
    p.label("field_register");
    for i in [1u16, 0] {
        abs(p, M::Lda, LIVE + i);
        abs(p, M::Cmp, CREATED);
        branch(p, M::Bne, &format!("field_register_{i}"));
        long_x(p, M::Lda, program + 2);
        abs(p, M::Sta, LIVE_SLOTS + i);
        jump(p, "field_next4");
        p.label(format!("field_register_{i}"));
    }
    jump(p, "field_next4");
    p.label("field_op_29");
    abs(p, M::Stz, TEMP);
    for i in 0..2 {
        long_x(p, M::Lda, program + 1);
        abs(p, M::Cmp, LIVE_SLOTS + i);
        branch(p, M::Bne, &format!("field_delete_{i}"));
        abs(p, M::Lda, 0x50c);
        emit(p, M::Dec, A::Accumulator, O::None);
        implied(p, M::Xba);
        abs(p, M::Lda, LIVE + i);
        implied(p, M::Tax);
        long_x(p, M::Lda, picture_songs);
        abs(p, M::Tsb, TEMP);
        abs(p, M::Stz, LIVE + i);
        abs(p, M::Stz, LIVE_SLOTS + i);
        jump(p, "field_deleted");
        p.label(format!("field_delete_{i}"));
    }
    p.label("field_deleted");
    call(p, "field_picture_unmoved");
    abs(p, M::Bit, TEMP);
    branch(p, M::Bpl, "field_deleted_kept");
    call(p, "field_picture_view");
    p.label("field_deleted_kept");
    jump(p, "field_next4");
    p.label("field_picture_view")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, 0x50c);
    emit(p, M::Cmp, A::Immediate, O::Byte(boundary));
    branch(p, M::Bcs, "field_picture_view_kept");
    jump(p, "b1_render");
    p.label("field_picture_view_kept");
    implied(p, M::Rts);
    p.label("field_pictures_left")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, LIVE);
    abs(p, M::Ora, LIVE + 1);
    branch(p, M::Beq, "field_pictures_kept");
    abs(p, M::Lda, LIVE_STATE);
    abs(p, M::Cmp, 0x500);
    branch(p, M::Beq, "field_pictures_kept");
    for i in 0..2 {
        abs(p, M::Lda, 0x50c);
        emit(p, M::Dec, A::Accumulator, O::None);
        implied(p, M::Xba);
        abs(p, M::Lda, LIVE + i);
        implied(p, M::Tax);
        long_x(p, M::Lda, picture_songs);
        emit(p, M::And, A::Immediate, O::Byte(0x7f));
        branch(p, M::Beq, &format!("field_left_{i}"));
        emit(p, M::Dec, A::Accumulator, O::None);
        call(p, "sound_play");
        p.label(format!("field_left_{i}"));
        abs(p, M::Stz, LIVE + i);
        abs(p, M::Stz, LIVE_SLOTS + i);
    }
    p.label("field_pictures_kept");
    implied(p, M::Rts);
    p.label("field_picture_record")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, LIVE);
    abs(p, M::Ora, LIVE + 1);
    branch(p, M::Beq, "field_record_early");
    abs(p, M::Lda, LIVE_STATE);
    abs(p, M::Cmp, 0x500);
    branch(p, M::Beq, "field_record_live");
    p.label("field_record_early");
    implied(p, M::Rts);
    p.label("field_record_live");
    abs(p, M::Lda, LIVE);
    abs(p, M::Cmp, LIVE + 1);
    branch(p, M::Bcc, "field_record_ordered");
    abs(p, M::Lda, LIVE + 1);
    abs(p, M::Sta, KEY);
    abs(p, M::Lda, LIVE);
    abs(p, M::Sta, KEY + 1);
    branch(p, M::Bra, "field_record_keyed");
    p.label("field_record_ordered");
    abs(p, M::Sta, KEY);
    abs(p, M::Lda, LIVE + 1);
    abs(p, M::Sta, KEY + 1);
    p.label("field_record_keyed");
    abs(p, M::Lda, KEY);
    branch(p, M::Bne, "field_record_slid");
    abs(p, M::Lda, PICTURE_X + 2);
    abs(p, M::Ora, PICTURE_Y + 2);
    for _ in 0..SLIDE_PIXELS.trailing_zeros() {
        emit(p, M::Lsr, A::Accumulator, O::None);
    }
    branch(p, M::Beq, "field_record_slid");
    emit(p, M::Ora, A::Immediate, O::Byte(SLIDE_KEY));
    abs(p, M::Sta, KEY);
    p.label("field_record_slid");
    m16(p);
    abs(p, M::Lda, 0x50c);
    emit(p, M::And, A::Immediate, O::Word(255));
    emit(p, M::Dec, A::Accumulator, O::None);
    implied(p, M::Xba);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Pha);
    abs(p, M::Lda, 0x500);
    emit(p, M::And, A::Immediate, O::Word(255));
    emit(p, M::Asl, A::Accumulator, O::None);
    emit(p, M::Ora, A::StackRelative, O::Byte(1));
    implied(p, M::Tax);
    implied(p, M::Pla);
    long_x(p, M::Lda, pictures);
    emit(p, M::Cmp, A::Immediate, O::Word(0xffff));
    branch(p, M::Beq, "field_record_none16");
    implied(p, M::Tax);
    m8(p);
    p.label("field_record_entry");
    long_x(p, M::Lda, pictures);
    emit(p, M::Cmp, A::Immediate, O::Byte(0xff));
    branch(p, M::Beq, "field_record_none");
    abs(p, M::Cmp, KEY);
    branch(p, M::Bne, "field_record_next");
    long_x(p, M::Lda, pictures + 1);
    abs(p, M::Cmp, KEY + 1);
    branch(p, M::Bne, "field_record_next");
    m16(p);
    long_x(p, M::Lda, pictures + 2);
    abs(p, M::Cmp, crate::sfc_floor_walls::BASE);
    m8(p);
    branch(p, M::Bne, "field_record_next");
    m16(p);
    long_x(p, M::Lda, pictures + 4);
    for _ in 0..crate::sfc_floor_walls::VIEW_RECORD.trailing_zeros() {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    abs(p, M::Sta, 0x51a);
    m8(p);
    implied(p, M::Rts);
    p.label("field_record_next");
    for _ in 0..6 {
        implied(p, M::Inx);
    }
    branch(p, M::Bra, "field_record_entry");
    p.label("field_record_none16")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    m8(p);
    p.label("field_record_none");
    implied(p, M::Rts);
    Ok(())
}
