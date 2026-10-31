use super::*;
pub fn emit_rules(p: &mut Assembler, layout: &CartridgeLayout) -> Result<()> {
    if layout.section("battle_display")?.bytes.get(2) != Some(&1) {
        return Err("Battle display resource zero must remain tile-backed".into());
    }
    let address = |name: &str| -> Result<u32> { layout.cpu_address(layout.section(name)?.offset) };
    let records = address("battle_display")?;
    let setup = address("battle_display_setup")?;
    let restore = address("b1_portrait_restore")?;
    let oam_job = address("menu_oam_job")?;
    let slots = address("battle_display_slots")?;
    let left_x = address("battle_display_left_x")?;
    let right_x = address("battle_display_right_x")?;
    let table_y = address("battle_display_y")?;
    let arle_names = address("battle_display_arle_names")?;
    let high_bits = address("battle_display_high_bits")?;
    let oam = crate::sfc_b1_menu::OAM_BUFFER;
    let slot = |s: u16| QUEUE + s * 5;
    let long = |p: &mut Assembler, m, at: u32| emit(p, m, A::AbsoluteLong, O::Long(at));
    let long_x = |p: &mut Assembler, m, at: u32| emit(p, m, A::AbsoluteLongX, O::Long(at));
    let indirect =
        |p: &mut Assembler, m| emit(p, m, A::DirectPageIndirectLongIndexedY, O::Byte(RECORD));
    let enqueue_long = |p: &mut Assembler, record: u32, to: u16| {
        for i in 0..5u32 {
            long(p, M::Lda, record + i);
            abs(p, M::Sta, to + i as u16);
        }
    };
    let hide_bodies = |p: &mut Assembler, label: &str, first: u16| {
        emit(p, M::Ldx, A::Immediate, O::Word(first * 4));
        p.label(label);
        byte(p, M::Lda, 224);
        emit(p, M::Sta, A::AbsoluteX, O::Word(oam + 1));
        implied(p, M::Inx);
        implied(p, M::Inx);
        implied(p, M::Inx);
        implied(p, M::Inx);
        emit(p, M::Cpx, A::Immediate, O::Word(512));
        branch(p, M::Bne, label);
        for at in first / 4..32 {
            abs(p, M::Stz, oam + 512 + at);
        }
    };
    p.label("battle_display_start")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Stz, BOSS_ON);
    m16(p);
    word(p, M::Lda, NO_BOSS);
    abs(p, M::Sta, BOSS_SENT);
    m8(p);
    for s in [ARLE_SLOT, SPRITE_SLOT, 5] {
        abs(p, M::Stz, slot(s) + 3);
        abs(p, M::Stz, slot(s) + 4);
    }
    m16(p);
    abs(p, M::Lda, crate::sfc_encounter::ENEMY);
    word(p, M::And, 0xff);
    abs(p, M::Sta, CELL);
    emit(p, M::Asl, A::Accumulator, O::None);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Clc);
    abs(p, M::Adc, CELL);
    implied(p, M::Tax);
    m8(p);
    for i in 0..5u32 {
        long_x(p, M::Lda, setup + i);
        abs(p, M::Sta, slot(ARLE_SLOT) + i as u16);
    }
    m16(p);
    word(p, M::Lda, 0xffff);
    abs(p, M::Sta, ARLE_FRAME);
    abs(p, M::Sta, ARLE_SHOWN);
    abs(p, M::Stz, ARLE_BANK);
    word(p, M::Lda, 0);
    emit(p, M::Ldx, A::Immediate, O::Word(0));
    p.label("battle_display_last_clear");
    long_x(p, M::Sta, LAST_DRAWN);
    implied(p, M::Inx);
    implied(p, M::Inx);
    emit(p, M::Cpx, A::Immediate, O::Word(LAST_DRAWN_BYTES));
    branch(p, M::Bne, "battle_display_last_clear");
    emit(p, M::Ldx, A::Immediate, O::Word(0));
    p.label("battle_display_cache_clear");
    long_x(p, M::Sta, CACHE);
    implied(p, M::Inx);
    implied(p, M::Inx);
    emit(
        p,
        M::Cpx,
        A::Immediate,
        O::Word(CACHE_ENTRIES * CACHE_BYTES),
    );
    branch(p, M::Bne, "battle_display_cache_clear");
    emit(p, M::Ldx, A::Immediate, O::Word(0));
    p.label("battle_display_keys_clear");
    long_x(p, M::Sta, KEY_COUNTS);
    implied(p, M::Inx);
    implied(p, M::Inx);
    emit(p, M::Cpx, A::Immediate, O::Word(0x100));
    branch(p, M::Bne, "battle_display_keys_clear");
    word(p, M::Lda, 0);
    for r in (0..ROWS).step_by(2) {
        long(p, M::Sta, ROWS_USED + u32::from(r));
    }
    emit(p, M::Ldx, A::Immediate, O::Word(0));
    word(p, M::Lda, BLANK);
    p.label("battle_display_blank");
    long_x(p, M::Sta, SHADOW);
    implied(p, M::Inx);
    implied(p, M::Inx);
    emit(p, M::Cpx, A::Immediate, O::Word(16 * 64));
    branch(p, M::Bne, "battle_display_blank");
    m8(p);
    hide_bodies(p, "battle_display_hide", FIRST_ENTRY);
    m16(p);
    word(p, M::Lda, 512);
    abs(p, M::Sta, PREV_END);
    abs(p, M::Stz, BODIES_FROM);
    abs(p, M::Lda, crate::sfc_encounter::ENEMY);
    word(p, M::And, 0xff);
    for _ in 0..7 {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    implied(p, M::Tax);
    emit(p, M::Ldy, A::Immediate, O::Word(0));
    p.label("battle_display_slots_copy");
    long_x(p, M::Lda, slots);
    implied(p, M::Phx);
    implied(p, M::Tyx);
    long_x(p, M::Sta, SLOT_TABLE);
    implied(p, M::Plx);
    implied(p, M::Inx);
    implied(p, M::Inx);
    implied(p, M::Iny);
    implied(p, M::Iny);
    emit(p, M::Cpy, A::Immediate, O::Word(SLOTS_PER_TABLE as u16));
    branch(p, M::Bne, "battle_display_slots_copy");
    m8(p);
    p.emit_label_ref(M::Jsr, A::Absolute, "battle_display_palette_start");
    m16(p);
    abs(p, M::Lda, crate::sfc_encounter::ENEMY);
    word(p, M::And, 0xff);
    word(p, M::Cmp, SCENE_ID as u16);
    branch(p, M::Bne, "battle_display_not_scene");
    word(p, M::Lda, 0xffff);
    abs(p, M::Sta, PALETTE_DIRTY);
    p.label("battle_display_not_scene");
    m8(p);
    byte(p, M::Lda, SETTING_UP);
    abs(p, M::Sta, DISPLAY);
    implied(p, M::Rts);
    let streams = address("scene_streams")?;
    let rgb = address("battle_palette_rgb")?;
    p.label("scene_stream").assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    emit(p, M::Ldx, A::Immediate, O::Word(0));
    p.label("scene_stream_find");
    long_x(p, M::Lda, streams);
    abs(p, M::Sta, CELL);
    long_x(p, M::Lda, streams + 1);
    abs(p, M::Ora, CELL);
    branch(p, M::Beq, "scene_stream_none");
    long_x(p, M::Lda, streams);
    long(p, M::Cmp, STREAM);
    branch(p, M::Bne, "scene_stream_skip");
    long_x(p, M::Lda, streams + 2);
    word(p, M::And, 0xff);
    abs(p, M::Sta, CELL);
    long(p, M::Lda, STREAM + 2);
    word(p, M::And, 0xff);
    abs(p, M::Cmp, CELL);
    branch(p, M::Beq, "scene_stream_found");
    p.label("scene_stream_skip");
    long_x(p, M::Lda, streams + 3);
    word(p, M::And, 0xff);
    abs(p, M::Sta, CELL);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Clc);
    abs(p, M::Adc, CELL);
    implied(p, M::Clc);
    word(p, M::Adc, 4);
    abs(p, M::Sta, CELL);
    implied(p, M::Txa);
    implied(p, M::Clc);
    abs(p, M::Adc, CELL);
    implied(p, M::Tax);
    branch(p, M::Bra, "scene_stream_find");
    p.label("scene_stream_none");
    m8(p);
    implied(p, M::Rts);
    p.label("scene_stream_found")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    long_x(p, M::Lda, streams + 3);
    word(p, M::And, 0xff);
    abs(p, M::Sta, ROW);
    implied(p, M::Inx);
    implied(p, M::Inx);
    implied(p, M::Inx);
    implied(p, M::Inx);
    p.label("scene_stream_action");
    abs(p, M::Lda, ROW);
    branch(p, M::Bne, "scene_stream_act");
    m8(p);
    implied(p, M::Rts);
    p.label("scene_stream_act")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    abs(p, M::Stx, COLUMN);
    long_x(p, M::Lda, streams + 1);
    word(p, M::And, 0xff);
    abs(p, M::Sta, CELL);
    long_x(p, M::Lda, streams + 2);
    word(p, M::And, 0xff);
    abs(p, M::Sta, ATTRIBUTE);
    long_x(p, M::Lda, streams);
    word(p, M::And, 0xff);
    implied(p, M::Tax);
    m8(p);
    abs(p, M::Lda, CELL);
    long_x(p, M::Sta, SLOT_TABLE);
    m16(p);
    abs(p, M::Lda, CELL);
    branch(p, M::Beq, "scene_stream_next");
    emit(p, M::Dec, A::Accumulator, O::None);
    for _ in 0..4 {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    abs(p, M::Sta, DATA);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Clc);
    abs(p, M::Adc, DATA);
    abs(p, M::Sta, DATA);
    abs(p, M::Lda, ATTRIBUTE);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Clc);
    abs(p, M::Adc, ATTRIBUTE);
    abs(p, M::Sta, DATA + 2);
    emit(p, M::Ldy, A::Immediate, O::Word(0));
    p.label("scene_stream_colour");
    implied(p, M::Tya);
    implied(p, M::Clc);
    abs(p, M::Adc, DATA);
    implied(p, M::Tax);
    long_x(p, M::Lda, rgb);
    implied(p, M::Pha);
    implied(p, M::Tya);
    implied(p, M::Clc);
    abs(p, M::Adc, DATA + 2);
    implied(p, M::Tax);
    implied(p, M::Pla);
    long_x(p, M::Sta, PALETTE_CURRENT);
    implied(p, M::Iny);
    implied(p, M::Iny);
    emit(p, M::Cpy, A::Immediate, O::Word(48));
    branch(p, M::Bne, "scene_stream_colour");
    abs(p, M::Lda, ATTRIBUTE);
    for _ in 0..4 {
        emit(p, M::Lsr, A::Accumulator, O::None);
    }
    implied(p, M::Tax);
    word(p, M::Lda, 1);
    p.label("scene_stream_bit");
    emit(p, M::Cpx, A::Immediate, O::Word(0));
    branch(p, M::Beq, "scene_stream_mark");
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Dex);
    branch(p, M::Bra, "scene_stream_bit");
    p.label("scene_stream_mark");
    abs(p, M::Ora, PALETTE_DIRTY);
    abs(p, M::Sta, PALETTE_DIRTY);
    p.label("scene_stream_next");
    abs(p, M::Ldx, COLUMN);
    implied(p, M::Inx);
    implied(p, M::Inx);
    implied(p, M::Inx);
    abs(p, M::Dec, ROW);
    p.emit_label_ref(M::Jmp, A::Absolute, "scene_stream_action");
    p.label("battle_display_end")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, DISPLAY);
    branch(p, M::Bne, "battle_display_ending");
    implied(p, M::Rts);
    p.label("battle_display_ending");
    abs(p, M::Stz, BOSS_ON);
    byte(p, M::Lda, 0xff);
    abs(p, M::Sta, crate::sfc_b1_view::STAGED + 2);
    for s in [SPRITE_SLOT, 5] {
        abs(p, M::Stz, slot(s) + 3);
        abs(p, M::Stz, slot(s) + 4);
    }
    {
        use crate::sfc_b1_portrait::{BACKGROUNDS, FACE_BACKGROUND};
        abs(p, M::Lda, crate::sfc_battle::ACTORS + 0x4f);
        emit(p, M::Lsr, A::Accumulator, O::None);
        byte(p, M::Cmp, BACKGROUNDS as u8);
        branch(p, M::Bcc, "battle_display_background");
        byte(p, M::Lda, BACKGROUNDS as u8 - 1);
        p.label("battle_display_background");
        abs(p, M::Sta, FACE_BACKGROUND);
        emit(p, M::Asl, A::Accumulator, O::None);
        emit(p, M::Asl, A::Accumulator, O::None);
        implied(p, M::Clc);
        abs(p, M::Adc, FACE_BACKGROUND);
        m16(p);
        word(p, M::And, 255);
        implied(p, M::Tax);
        m8(p);
        for i in 0..5u32 {
            emit(p, M::Lda, A::AbsoluteLongX, O::Long(restore + i));
            abs(p, M::Sta, slot(ARLE_SLOT) + i as u16);
        }
    }
    hide_bodies(p, "battle_display_unhide", 0);
    enqueue_long(p, oam_job, crate::sfc_b1::MENU_OAM_SLOT + QUEUE);
    byte(p, M::Lda, RESTORING);
    abs(p, M::Sta, DISPLAY);
    implied(p, M::Rts);
    p.label("battle_display_tick")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, DISPLAY);
    byte(p, M::Cmp, SETTING_UP);
    branch(p, M::Bne, "battle_display_not_setup");
    abs(p, M::Lda, slot(ARLE_SLOT) + 3);
    abs(p, M::Ora, slot(ARLE_SLOT) + 4);
    branch(p, M::Bne, "battle_display_wait");
    byte(p, M::Lda, RUNNING);
    abs(p, M::Sta, DISPLAY);
    p.label("battle_display_wait");
    implied(p, M::Rts);
    p.label("battle_display_not_setup");
    byte(p, M::Cmp, RUNNING);
    branch(p, M::Beq, "battle_display_running");
    implied(p, M::Rts);
    p.label("battle_display_running");
    p.emit_label_ref(M::Jsr, A::Absolute, "battle_display_objects");
    p.emit_label_ref(M::Jsr, A::Absolute, "battle_boss_check");
    p.emit_label_ref(M::Jsr, A::Absolute, "battle_display_arle");
    p.emit_label_ref(M::Jsr, A::Absolute, "battle_display_palette");
    implied(p, M::Rts);
    emit_objects(
        p,
        records,
        SLOT_TABLE,
        [left_x, right_x, table_y, high_bits],
        [
            address("battle_display_extents")?,
            address("battle_display_extent_firsts")?,
            address(crate::sfc_body_tiles::STAGES_SECTION)?,
            crate::sfc_body_tiles::gather_address(layout)?,
            single_block::entry(layout)?,
            u32::from(layout.section("battle_arle_effect_resource")?.bytes[0]),
        ],
        oam,
        oam_job,
    )?;
    emit_arle(p, records, arle_names, indirect)?;
    emit_boss_view(p, layout, table_y)?;
    emit_palette(
        p,
        address("battle_palette_blocks")?,
        address("battle_palette_rgb")?,
    );
    Ok(())
}
