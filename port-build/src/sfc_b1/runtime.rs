use super::*;
pub fn emit(p: &mut w65c816::Assembler, layout: &CartridgeLayout, pressed: u16) -> Result<()> {
    use crate::sfc::{emit, implied, write};
    use w65c816::{AddressingMode as A, CpuMode, Mnemonic as M, Operand as O};
    let abs = |p: &mut w65c816::Assembler, m, at| emit(p, m, A::Absolute, O::Word(at));
    let branch = |p: &mut w65c816::Assembler, m, label: &str| {
        p.emit_label_ref(m, A::Relative8, label);
    };
    let call = |p: &mut w65c816::Assembler, label: &str| {
        p.emit_label_ref(M::Jsr, A::Absolute, label);
    };
    p.label("b1_enter").assume_mode(CpuMode::NATIVE_M8_X16);
    write(p, 0x22c, 1);
    write(p, 0x22d, 0);
    abs(p, M::Stz, crate::sfc_b1_map::MAP_EVENT);
    abs(p, M::Stz, crate::sfc_b1_text::STONE_SKIP);
    crate::sfc::write_word(p, crate::sfc_b1_stone::SHOWN, 0xffff);
    abs(p, M::Stz, crate::sfc_b1_stone::SHOWN_CONTEXT);
    abs(p, M::Stz, crate::sfc_battle_vm::SCENE);
    crate::sfc::write_word(p, crate::sfc_b1_stone::MP_MAX_SEEN, 0xffff);
    write(p, crate::sfc_field_events::SCREEN_DARK, 0);
    call(p, "title_ppu_restore");
    write(p, 0x651, 255);
    write(p, crate::sfc_b1_equipment::SHOWN, 255);
    write(p, crate::sfc_b1_equipment::SHOWN + 1, 255);
    write(p, 0x2100, 0x80);
    write(p, 0x4200, 1);
    abs(p, M::Lda, crate::sfc_save::LOADING);
    branch(p, M::Beq, "b1_enter_settings");
    emit(
        p,
        M::Lda,
        A::AbsoluteLong,
        O::Long(crate::sfc_save::STAGED_SETTINGS),
    );
    abs(p, M::Sta, crate::sfc_b1_menu::SETTINGS);
    p.label("b1_enter_settings");
    emit(
        p,
        M::Lda,
        A::Immediate,
        O::Byte(crate::sfc_sound::song("b1")),
    );
    call(p, "sound_music");
    write(p, 0x210b, (BG2_TILES >> 13 << 4) as u8);
    write(p, 0x2108, (TEXT_MAP_VRAM >> 9) as u8);
    write(p, 0x2101, OBJSEL);
    let [dx, _, dw, _] = DUNGEON;
    let [px, _, pw, _] = PORTRAIT;
    for (at, value) in [
        (0x2126, dx),
        (0x2127, dx + dw - 1),
        (0x2128, px),
        (0x2129, px + pw - 1),
        (0x2125, 0x0f),
        (0x212b, 1),
    ] {
        write(p, at, value as u8);
    }
    for (i, value) in BAND_TABLE.iter().enumerate() {
        write(p, BANDS + i as u16, *value);
    }
    for (at, value) in [
        (0x4310, 4),
        (0x4311, 0x2c),
        (0x4312, BANDS as u8),
        (0x4313, (BANDS >> 8) as u8),
        (0x4314, 0),
    ] {
        write(p, at, value);
    }
    for slot in 0..QUEUE_SLOTS * 5 {
        write(p, QUEUE + slot, 0);
    }
    crate::sfc_b1_portrait::emit_halves_reset(p);
    write(p, PENDING, 0);
    write(p, PENDING + 1, 0);
    call(p, "b1_view_channels");
    write(p, crate::sfc_b1_portrait::FACE_BACKGROUND, 0);
    call(p, "stats_reset");
    call(p, "b1_portrait_reset");
    call(p, "b1_menu_reset");
    call(p, "b1_map_reset");
    call(p, "floor_cells_reset");
    write(p, 0x640, 0);
    write(p, 0x641, 0);
    write(p, 0x64c, 255);
    write(p, 0x64d, 0);
    write(p, 0x500, 236);
    write(p, 0x50c, 1);
    call(p, "b1_map_visit");
    crate::sfc::write_word(p, 0x501, 2);
    crate::sfc::write_word(p, 0x50d, 2);
    write(p, 0x511, 0);
    call(p, "field_reset");
    call(p, "encounter_reset");
    call(p, "shop_reset");
    abs(p, M::Stz, crate::sfc_battle_field::ACTIVE);
    for at in [
        crate::sfc_battle_vm::MESSAGE,
        crate::sfc_battle_message::STARTED,
        crate::sfc_battle_message::DRAW,
        crate::sfc_battle_message::SOUND_REQUEST,
        crate::sfc_battle_message::SOUND_STOP,
    ] {
        abs(p, M::Stz, at);
    }
    write(p, 0x504, 0);
    write(p, 0x505, 17);
    write(p, 0x506, 0);
    use crate::sfc_save::LOADING;
    abs(p, M::Lda, LOADING);
    branch(p, M::Beq, "b1_enter_new");
    call(p, "save_scatter");
    write(p, 0x505, 255);
    call(p, "b1_map_visit");
    p.label("b1_enter_new");
    write(p, 0x212c, 3);
    write(p, 0x210d, 0);
    write(p, 0x210d, 0);
    let entry = layout.cpu_address(layout.section("b1_entry")?.offset)?;
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    emit(p, M::Lda, A::Immediate, O::Word(entry as u16));
    abs(p, M::Sta, 0xe0);
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    write(p, 0xe2, (entry >> 16) as u8);
    emit(p, M::Ldx, A::Immediate, O::Word(0));
    call(p, "b1_enqueue");
    call(p, "b1_stream_all");
    call(p, "b1_portrait_select");
    call(p, "b1_stream_all");
    for table in [
        crate::sfc_b1_text::EXTENT,
        crate::sfc_b1_text::SHOWN,
        crate::sfc_b1_text::POSTED,
    ] {
        for row in 0..crate::sfc_b1_text::ROWS as u16 {
            write(p, table + row, crate::sfc_b1_text::COLUMNS as u8);
        }
        for row in crate::sfc_b1_text::ROWS as u16..16 {
            abs(p, M::Stz, table + row);
        }
    }
    abs(p, M::Lda, LOADING);
    branch(p, M::Beq, "b1_enter_intro");
    abs(p, M::Stz, LOADING);
    call(p, "field_floor_bounds");
    call(p, "field_floor_entry");
    p.emit_label_ref(M::Bra, A::Relative8, "b1_enter_program");
    p.label("b1_enter_intro");
    call(p, "field_run");
    p.label("b1_enter_program");
    call(p, "b1_page");
    call(p, "b1_text_work");
    call(p, "b1_text_post");
    call(p, "b1_render");
    call(p, "b1_experience_tick");
    call(p, "b1_stone_tick");
    call(p, "b1_portrait_tick");
    call(p, "b1_stream_all");
    for _ in 0..2 {
        call(p, "b1_equipment_tick");
        call(p, "b1_stream_all");
    }
    write(p, 0x228, 1);
    write(p, 0x4200, 0x81);
    implied(p, M::Rts);
    p.label("b1_tick");
    abs(p, M::Lda, 0x228);
    branch(p, M::Beq, "b1_input");
    write(p, 0x228, 0);
    implied(p, M::Rts);
    p.label("b1_input");
    abs(p, M::Lda, crate::sfc_b1_text::SERIAL);
    abs(p, M::Cmp, crate::sfc_b1_text::QUEUED);
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    branch(p, M::Bne, "b1_input_busy");
    abs(p, M::Lda, QUEUE + 3);
    branch(p, M::Bne, "b1_input_busy");
    abs(p, M::Lda, QUEUE + 8);
    branch(p, M::Beq, "b1_input_ready");
    abs(p, M::Cmp, crate::sfc_b1_text::TAIL_COUNT);
    branch(p, M::Beq, "b1_input_ready");
    branch(p, M::Bcc, "b1_input_ready");
    p.label("b1_input_busy");
    abs(p, M::Lda, PENDING);
    abs(p, M::Ora, pressed);
    abs(p, M::Sta, PENDING);
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    implied(p, M::Rts);
    p.label("b1_input_ready")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    abs(p, M::Lda, PENDING);
    abs(p, M::Ora, pressed);
    abs(p, M::Sta, pressed);
    abs(p, M::Stz, PENDING);
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    abs(p, M::Lda, crate::sfc_battle_field::ACTIVE);
    branch(p, M::Beq, "b1_no_battle");
    p.emit_label_ref(M::Jmp, A::Absolute, "b1_battle_tick");
    p.label("b1_no_battle");
    abs(p, M::Lda, crate::sfc_shop::SHOP);
    branch(p, M::Beq, "b1_no_shop");
    p.emit_label_ref(M::Jmp, A::Absolute, "b1_shop_tick");
    p.label("b1_no_shop");
    abs(p, M::Lda, crate::sfc_b1_menu::MODE);
    branch(p, M::Beq, "b1_menu_closed");
    p.emit_label_ref(M::Jmp, A::Absolute, "b1_menu_tick");
    p.label("b1_menu_closed");
    abs(p, M::Lda, 0x50c);
    emit(p, M::Cmp, A::Immediate, O::Byte(BOUNDARY_FLOOR));
    branch(p, M::Bne, "b1_active_floor");
    implied(p, M::Rts);
    p.label("b1_active_floor");
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    abs(p, M::Lda, 0x501);
    emit(p, M::Cmp, A::Immediate, O::Word(2));
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    branch(p, M::Beq, "b1_navigation");
    call(p, "b1_dialogue_tick");
    implied(p, M::Rts);
    p.label("b1_navigation");
    use crate::sfc_field_events::{BLOCK, CLICK, CONTEXT, HEALING_WAIT, SIGNAL};
    abs(p, M::Lda, CONTEXT);
    branch(p, M::Bne, "b1_menu_not_opened");
    abs(p, M::Lda, pressed);
    emit(p, M::And, A::Immediate, O::Byte(0x40));
    branch(p, M::Bne, "b1_open_menu");
    abs(p, M::Lda, pressed + 1);
    emit(p, M::And, A::Immediate, O::Byte(0x10));
    branch(p, M::Beq, "b1_menu_not_opened");
    p.label("b1_open_menu");
    p.emit_label_ref(M::Jmp, A::Absolute, "b1_menu_open");
    p.label("b1_menu_not_opened");
    abs(p, M::Lda, HEALING_WAIT);
    branch(p, M::Beq, "b1_click_read");
    emit(p, M::Lda, A::Immediate, O::Byte(0));
    branch(p, M::Bra, "b1_click_store");
    p.label("b1_click_read");
    abs(p, M::Lda, pressed);
    emit(p, M::And, A::Immediate, O::Byte(0x80));
    p.label("b1_click_store");
    abs(p, M::Sta, CLICK);
    abs(p, M::Lda, crate::sfc_field_events::SCENE_REQUEST);
    branch(p, M::Beq, "b1_no_scene");
    abs(p, M::Stz, crate::sfc_field_events::SCENE_REQUEST);
    emit(p, M::Cmp, A::Immediate, O::Byte(2));
    branch(p, M::Bne, "b1_scene_phantom");
    emit(p, M::Lda, A::Immediate, O::Byte(1));
    abs(p, M::Sta, crate::sfc_ending::ACTIVE);
    implied(p, M::Rts);
    p.label("b1_scene_phantom")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    p.emit_label_ref(M::Jmp, A::Absolute, "scene_enter");
    p.label("b1_no_scene");
    call(p, "field_pictures_left");
    call(p, "field_run");
    abs(p, M::Lda, crate::sfc_shop::SHOP);
    branch(p, M::Bne, "b1_field_busy");
    call(p, "field_input");
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    abs(p, M::Lda, crate::sfc_field_events::PC);
    emit(p, M::Cmp, A::Immediate, O::Word(0xffff));
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    branch(p, M::Bne, "b1_field_busy");
    call(p, "encounter_select");
    p.label("b1_field_busy");
    abs(p, M::Stz, SIGNAL);
    abs(p, M::Stz, CLICK);
    abs(p, M::Lda, crate::sfc_shop::SHOP);
    abs(p, M::Ora, crate::sfc_battle_field::ACTIVE);
    branch(p, M::Bne, "b1_navigation_return");
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    abs(p, M::Lda, 0x501);
    emit(p, M::Cmp, A::Immediate, O::Word(2));
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    branch(p, M::Bne, "b1_navigation_return");
    abs(p, M::Lda, 0x50c);
    emit(p, M::Cmp, A::Immediate, O::Byte(BOUNDARY_FLOOR));
    branch(p, M::Bne, "b1_direction_input");
    p.label("b1_navigation_return");
    implied(p, M::Rts);
    p.label("b1_direction_input");
    abs(p, M::Lda, HEALING_WAIT);
    abs(p, M::Ora, CONTEXT);
    branch(p, M::Beq, "b1_direction_free");
    implied(p, M::Rts);
    p.label("b1_direction_free");
    use crate::sfc_floor_walls::{CD, CX, CY};
    let deltas = layout.cpu_address(layout.section("view_deltas")?.offset)?;
    for (action, mask, turn) in [(0u16, 8, 0u8), (1, 2, 3), (2, 1, 1), (3, 4, 2)] {
        abs(p, M::Lda, pressed + 1);
        emit(p, M::And, A::Immediate, O::Byte(15));
        emit(p, M::Cmp, A::Immediate, O::Byte(mask));
        branch(p, M::Bne, &format!("b1_not_{action}"));
        if action == 0 {
            abs(p, M::Lda, BLOCK);
            branch(p, M::Beq, "b1_forward_unlocked");
            emit(p, M::Cmp, A::Immediate, O::Byte(1));
            branch(p, M::Beq, "b1_forward_blocked");
            write(p, SIGNAL, 2);
            implied(p, M::Rts);
            p.label("b1_forward_unlocked");
            call(p, "floor_edge");
            emit(p, M::Bit, A::Immediate, O::Byte(8));
            branch(p, M::Beq, "b1_forward_open");
            p.label("b1_forward_blocked");
            write(p, 0x504, 3);
            write(p, IMPACT, IMPACT_TICKS);
            if let Some(effect) = crate::sfc_sound::effect(layout, 12)? {
                emit(p, M::Lda, A::Immediate, O::Byte(effect));
                call(p, "sound_effect");
            }
            write(p, SIGNAL, 1);
            implied(p, M::Rts);
            p.label("b1_forward_open");
            emit(p, M::And, A::Immediate, O::Byte(7));
            branch(p, M::Beq, "b1_forward_step");
            emit(p, M::Lda, A::Immediate, O::Byte(1));
            p.label("b1_forward_step");
            emit(p, M::Inc, A::Accumulator, O::None);
            abs(p, M::Sta, 0x504);
            emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
            abs(p, M::Lda, CD);
            emit(p, M::And, A::Immediate, O::Word(3));
            implied(p, M::Tax);
            emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
            emit(p, M::Lda, A::AbsoluteLongX, O::Long(deltas + 4));
            implied(p, M::Clc);
            abs(p, M::Adc, CY);
            for _ in 0..3 {
                emit(p, M::Asl, A::Accumulator, O::None);
            }
            abs(p, M::Sta, CY);
            emit(p, M::Lda, A::AbsoluteLongX, O::Long(deltas));
            implied(p, M::Clc);
            abs(p, M::Adc, CX);
            implied(p, M::Clc);
            abs(p, M::Adc, CY);
            emit(p, M::Asl, A::Accumulator, O::None);
            emit(p, M::Asl, A::Accumulator, O::None);
            abs(p, M::Ora, CD);
        } else {
            abs(p, M::Stz, BLOCK);
            write(p, 0x504, 0);
            let slot = crate::sfc_field_events::GLOBALS + 4 * (32 + action);
            for i in 0..4 {
                abs(p, M::Inc, slot + i);
                if i < 3 {
                    branch(p, M::Bne, &format!("b1_turn_counted_{action}"));
                }
            }
            p.label(format!("b1_turn_counted_{action}"));
            abs(p, M::Lda, 0x500);
            implied(p, M::Clc);
            emit(p, M::Adc, A::Immediate, O::Byte(turn));
            emit(p, M::And, A::Immediate, O::Byte(3));
            abs(p, M::Sta, CD);
            abs(p, M::Lda, 0x500);
            emit(p, M::And, A::Immediate, O::Byte(0xfc));
            abs(p, M::Ora, CD);
        }
        p.label(format!("b1_move_{action}"));
        abs(p, M::Sta, 0x500);
        if action == 0 {
            let step = crate::sfc_sound::effect(layout, 13)?;
            let door = crate::sfc_sound::effect(layout, 9)?;
            abs(p, M::Lda, 0x504);
            emit(p, M::Cmp, A::Immediate, O::Byte(2));
            branch(p, M::Beq, "b1_forward_door");
            if let Some(effect) = step {
                emit(p, M::Lda, A::Immediate, O::Byte(effect));
                call(p, "sound_effect");
            }
            branch(p, M::Bra, "b1_forward_sounded");
            p.label("b1_forward_door");
            if let Some(effect) = door {
                emit(p, M::Lda, A::Immediate, O::Byte(effect));
                call(p, "sound_effect");
            }
            p.label("b1_forward_sounded");
            call(p, "b1_map_path_step");
        }
        p.emit_label_ref(M::Jmp, A::Absolute, "b1_moved");
        p.label(format!("b1_not_{action}"));
    }
    implied(p, M::Rts);
    p.label("b1_moved");
    call(p, "b1_page");
    call(p, "b1_render");
    call(p, "b1_map_visit");
    call(p, "encounter_roll");
    implied(p, M::Rts);
    crate::sfc_field_events::emit(p, layout, pressed, BOUNDARY_FLOOR)?;
    crate::sfc_floor_walls::emit(p, layout)?;
    crate::sfc_encounter::emit(p, layout)?;
    crate::sfc_battle::emit(p, layout)?;
    crate::sfc_battle_field::emit_field(p, layout, pressed)?;
    crate::sfc_ending::emit_field(p)?;
    crate::sfc_shop::emit_field(p, layout, pressed)?;
    crate::sfc_far_code::emit(p, layout)?;
    crate::sfc_b1_stone::emit(p, layout)?;
    crate::sfc_b1_frame::emit(p, layout)?;
    crate::sfc_b1_equipment::emit(p, layout)?;
    crate::sfc_b1_portrait::emit(p, layout)?;
    crate::sfc_field_smoke::emit(p, layout)?;
    crate::sfc_field_shards::emit(p, layout)?;
    crate::sfc_field_flames::emit(p, layout)?;
    crate::sfc_field_guardian::emit(p, layout)?;
    crate::sfc_b1_menu::emit(p, layout, pressed)?;
    crate::sfc_b1_map::emit(p, layout, pressed)?;
    crate::sfc_sound::emit(p, layout)?;
    crate::sfc_b1_text::emit(p, layout)?;
    crate::sfc_b1_view::emit_view(p, layout, 4 * DUNGEON_COLUMNS)?;
    crate::sfc_b1_view::emit_channels(p);
    for (label, section, index) in [("b1_render", "b1_views", 0x500), ("b1_page", "", 0x501)] {
        p.label(label).assume_mode(CpuMode::NATIVE_M8_X16);
        if label == "b1_render" {
            abs(p, M::Lda, crate::sfc_b1_map::MAP_EVENT);
            branch(p, M::Beq, "b1_render_no_map");
            call(p, "b1_map_event_close");
            p.label("b1_render_no_map");
            call(p, "b1_view_find");
            emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
            abs(p, M::Lda, crate::sfc_floor_walls::BASE);
            emit(p, M::Cmp, A::Immediate, O::Word(0xffff));
            emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
            branch(p, M::Bne, "b1_render_found");
            implied(p, M::Rts);
            p.label("b1_view_find").assume_mode(CpuMode::NATIVE_M8_X16);
            abs(p, M::Lda, 0x500);
            emit(p, M::And, A::Immediate, O::Byte(3));
            abs(p, M::Sta, 0x50a);
            abs(p, M::Lda, 0x500);
            for _ in 0..2 {
                emit(p, M::Lsr, A::Accumulator, O::None);
            }
            abs(p, M::Sta, 0x50b);
            emit(p, M::And, A::Immediate, O::Byte(7));
            abs(p, M::Sta, 0x508);
            abs(p, M::Lda, 0x50b);
            for _ in 0..3 {
                emit(p, M::Lsr, A::Accumulator, O::None);
            }
            abs(p, M::Sta, 0x509);
            call(p, "floor_view_record");
            emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
            abs(p, M::Lda, crate::sfc_floor_walls::BASE);
            emit(p, M::Cmp, A::Immediate, O::Word(0xffff));
            emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
            branch(p, M::Beq, "b1_view_find_done");
            call(p, "field_picture_record");
            p.label("b1_view_find_done");
            implied(p, M::Rts);
        }
        if label == "b1_page" {
            call(p, "b1_text_request");
        } else {
            let source = layout.cpu_address(layout.section(section)?.offset)?;
            let _ = index;
            p.label("b1_render_found");
            abs(p, M::Stz, crate::sfc_field_events::VIEW_DIRTY);
            emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
            abs(p, M::Lda, 0x51a);
            abs(p, M::Sta, SHOWN_VIEW);
            emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
            abs(p, M::Stz, crate::sfc_field_events::FADE_LEVEL);
            emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
            abs(p, M::Lda, 0x51a);
            implied(p, M::Clc);
            emit(p, M::Adc, A::Immediate, O::Word(source as u16));
            abs(p, M::Sta, 0xe0);
            emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
            write(p, 0xe2, (source >> 16) as u8);
            let list = layout.cpu_address(layout.section("b1_view_list_code")?.offset)?;
            emit(p, M::Jsl, A::AbsoluteLong, O::Long(list));
            emit(p, M::Ldx, A::Immediate, O::Word(0));
            call(p, "b1_enqueue");
        }
        if label == "b1_page" {
            emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
            abs(p, M::Lda, 0x501);
            implied(p, M::Tax);
            emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
            let delays = layout.cpu_address(layout.section("b1_page_delays")?.offset)?;
            emit(p, M::Lda, A::AbsoluteLongX, O::Long(delays));
            abs(p, M::Sta, 0x518);
            let audio = layout.cpu_address(layout.section("b1_page_audio")?.offset)?;
            emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
            implied(p, M::Txa);
            emit(p, M::Asl, A::Accumulator, O::None);
            implied(p, M::Tax);
            emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
            emit(p, M::Lda, A::AbsoluteLongX, O::Long(audio));
            emit(p, M::And, A::Immediate, O::Byte(!JINGLE_WAIT));
            branch(p, M::Beq, "b1_page_no_music");
            emit(p, M::Dec, A::Accumulator, O::None);
            call(p, "sound_play");
            p.label("b1_page_no_music");
            emit(p, M::Lda, A::AbsoluteLongX, O::Long(audio + 1));
            branch(p, M::Beq, "b1_page_no_effect");
            emit(p, M::Cmp, A::Immediate, O::Byte(PAGE_PAIRS));
            branch(p, M::Bcc, "b1_page_effect");
            emit(p, M::Sbc, A::Immediate, O::Byte(PAGE_PAIRS));
            emit(p, M::Asl, A::Accumulator, O::None);
            emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
            emit(p, M::And, A::Immediate, O::Word(0xff));
            implied(p, M::Tax);
            emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
            let pairs = layout.cpu_address(layout.section("b1_page_pairs")?.offset)?;
            emit(p, M::Lda, A::AbsoluteLongX, O::Long(pairs + 1));
            implied(p, M::Pha);
            emit(p, M::Lda, A::AbsoluteLongX, O::Long(pairs));
            emit(p, M::Dec, A::Accumulator, O::None);
            call(p, "sound_effect");
            implied(p, M::Pla);
            p.label("b1_page_effect");
            emit(p, M::Dec, A::Accumulator, O::None);
            call(p, "sound_effect");
            p.label("b1_page_no_effect");
            let word = |name: &str| -> Result<u16> {
                let bytes = &layout.section(name)?.bytes;
                Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
            };
            emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
            abs(p, M::Lda, 0x501);
            emit(p, M::Cmp, A::Immediate, O::Word(word("b1_impact_page")?));
            emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
            branch(p, M::Bne, "b1_page_no_impact");
            write(p, IMPACT, IMPACT_TICKS);
            p.label("b1_page_no_impact");
        }
        implied(p, M::Rts);
    }
    p.label("b1_enqueue").assume_mode(CpuMode::NATIVE_M8_X16);
    for i in 0..5u16 {
        emit(p, M::Ldy, A::Immediate, O::Word(i));
        emit(p, M::Lda, A::DirectPageIndirectLongIndexedY, O::Byte(0xe0));
        emit(p, M::Sta, A::AbsoluteX, O::Word(QUEUE + i));
    }
    implied(p, M::Rts);
    p.label("b1_stream_all").assume_mode(CpuMode::NATIVE_M8_X16);
    for _ in 0..5 {
        call(p, "b1_view_gather");
        call(p, "b1_stream_unlimited");
    }
    implied(p, M::Rts);
    p.label("b1_stream_unlimited")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    abs(p, M::Stz, MENU_HELD);
    emit(p, M::Lda, A::Immediate, O::Word(0xffff));
    p.emit_label_ref(M::Brl, A::Relative16, "b1_stream_budget");
    p.label("b1_stream_title")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    abs(p, M::Stz, MENU_HELD);
    emit(p, M::Lda, A::Immediate, O::Word(TITLE_STREAM_BUDGET));
    p.emit_label_ref(M::Brl, A::Relative16, "b1_stream_budget");
    p.label("b1_stream").assume_mode(CpuMode::NATIVE_M8_X16);
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    emit(p, M::Lda, A::Immediate, O::Word(STREAM_BUDGET));
    abs(p, M::Sta, BUDGET);
    abs(p, M::Stz, MENU_HELD);
    abs(p, M::Lda, GROUP_WAITING);
    branch(p, M::Beq, "b1_stream_no_waiting");
    abs(p, M::Stz, GROUP_WAITING);
    abs(p, M::Lda, crate::sfc_battle_field::ACTIVE);
    emit(p, M::And, A::Immediate, O::Word(0xff));
    branch(p, M::Bne, "b1_stream_waited_battle");
    emit(p, M::Lda, A::Immediate, O::Word(1));
    abs(p, M::Sta, MENU_HELD);
    p.emit_label_ref(M::Jmp, A::Absolute, "b1_stream_slots");
    p.label("b1_stream_waited_battle");
    emit(p, M::Ldx, A::Immediate, O::Word(5));
    emit(p, M::Lda, A::AbsoluteX, O::Word(QUEUE + 3));
    branch(p, M::Beq, "b1_stream_no_waiting");
    emit(p, M::Lda, A::AbsoluteX, O::Word(QUEUE));
    abs(p, M::Sta, 0xe4);
    emit(p, M::Lda, A::AbsoluteX, O::Word(QUEUE + 2));
    abs(p, M::Sta, 0xe6);
    emit(p, M::Lda, A::DirectPageIndirectLong, O::Byte(0xe4));
    emit(p, M::And, A::Immediate, O::Word(0xff));
    emit(
        p,
        M::Cmp,
        A::Immediate,
        O::Word(u16::from(crate::sfc_b1_text::GROUP_KIND)),
    );
    branch(p, M::Bne, "b1_stream_no_waiting");
    call(p, "b1_stream_group");
    p.label("b1_stream_no_waiting");
    abs(p, M::Lda, crate::sfc_battle_field::ACTIVE);
    emit(p, M::And, A::Immediate, O::Word(0xff));
    branch(p, M::Beq, "b1_stream_menu_free");
    abs(p, M::Lda, crate::sfc_b1_menu::MODE);
    emit(p, M::And, A::Immediate, O::Word(0xff));
    branch(p, M::Beq, "b1_stream_menu_free");
    abs(p, M::Lda, crate::sfc_battle_display::BODIES_FROM);
    branch(p, M::Bne, "b1_stream_menu_free");
    emit(p, M::Lda, A::Immediate, O::Word(1));
    abs(p, M::Sta, MENU_HELD);
    branch(p, M::Bra, "b1_stream_slots");
    p.label("b1_stream_menu_free");
    abs(p, M::Lda, QUEUE + MENU_STRIP_SLOT + 3);
    branch(p, M::Beq, "b1_stream_slots");
    for slot in [MENU_STRIP_SLOT, MENU_OAM_SLOT] {
        emit(p, M::Ldx, A::Immediate, O::Word(slot));
        call(p, "b1_stream_whole");
    }
    p.emit_label_ref(M::Bra, A::Relative8, "b1_stream_slots");
    p.label("b1_stream_whole")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    emit(p, M::Lda, A::AbsoluteX, O::Word(QUEUE + 3));
    branch(p, M::Beq, "b1_stream_whole_done");
    emit(p, M::Lda, A::AbsoluteX, O::Word(QUEUE));
    abs(p, M::Sta, 0xe4);
    emit(p, M::Lda, A::AbsoluteX, O::Word(QUEUE + 2));
    abs(p, M::Sta, 0xe6);
    emit(p, M::Ldy, A::Immediate, O::Word(3));
    emit(p, M::Lda, A::DirectPageIndirectLongIndexedY, O::Byte(0xe4));
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Word(DESCRIPTOR_COST));
    abs(p, M::Sta, 0xe8);
    abs(p, M::Lda, BUDGET);
    implied(p, M::Sec);
    abs(p, M::Sbc, 0xe8);
    branch(p, M::Bcs, "b1_stream_whole_left");
    emit(p, M::Lda, A::Immediate, O::Word(0));
    p.label("b1_stream_whole_left");
    abs(p, M::Sta, BUDGET);
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    call(p, "b1_descriptor");
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    emit(p, M::Lda, A::AbsoluteX, O::Word(QUEUE));
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Word(8));
    emit(p, M::Sta, A::AbsoluteX, O::Word(QUEUE));
    emit(p, M::Lda, A::AbsoluteX, O::Word(QUEUE + 3));
    emit(p, M::Dec, A::Accumulator, O::None);
    emit(p, M::Sta, A::AbsoluteX, O::Word(QUEUE + 3));
    p.emit_label_ref(M::Bra, A::Relative8, "b1_stream_whole");
    p.label("b1_stream_whole_done");
    implied(p, M::Rts);
    p.label("b1_stream_slots")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    abs(p, M::Lda, BUDGET);
    p.label("b1_stream_budget");
    abs(p, M::Sta, BUDGET);
    emit(p, M::Ldx, A::Immediate, O::Word(0));
    p.label("b1_stream_slot");
    emit(p, M::Lda, A::AbsoluteX, O::Word(QUEUE + 3));
    branch(p, M::Beq, "b1_stream_next_slot");
    abs(p, M::Lda, MENU_HELD);
    branch(p, M::Beq, "b1_stream_not_held");
    implied(p, M::Txa);
    emit(p, M::Cmp, A::Immediate, O::Word(MENU_STRIP_SLOT));
    branch(p, M::Beq, "b1_stream_next_slot");
    emit(p, M::Cmp, A::Immediate, O::Word(MENU_OAM_SLOT));
    branch(p, M::Beq, "b1_stream_next_slot");
    p.label("b1_stream_not_held");
    emit(p, M::Lda, A::AbsoluteX, O::Word(QUEUE));
    abs(p, M::Sta, 0xe4);
    emit(p, M::Lda, A::AbsoluteX, O::Word(QUEUE + 2));
    abs(p, M::Sta, 0xe6);
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    call(p, "b1_view_unstaged");
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    branch(p, M::Bcs, "b1_stream_next_slot");
    emit(p, M::Lda, A::DirectPageIndirectLong, O::Byte(0xe4));
    emit(p, M::And, A::Immediate, O::Word(0xff));
    emit(
        p,
        M::Cmp,
        A::Immediate,
        O::Word(u16::from(crate::sfc_b1_text::GROUP_KIND)),
    );
    branch(p, M::Bne, "b1_stream_single");
    call(p, "b1_stream_group");
    branch(p, M::Bcs, "b1_stream_group_went");
    emit(p, M::Lda, A::Immediate, O::Word(1));
    abs(p, M::Sta, GROUP_WAITING);
    p.emit_label_ref(M::Jmp, A::Absolute, "b1_stream_done");
    p.label("b1_stream_group_went");
    p.emit_label_ref(M::Jmp, A::Absolute, "b1_stream_slot");
    p.label("b1_stream_single")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    emit(p, M::Ldy, A::Immediate, O::Word(3));
    emit(p, M::Lda, A::DirectPageIndirectLongIndexedY, O::Byte(0xe4));
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Word(DESCRIPTOR_COST));
    abs(p, M::Cmp, BUDGET);
    branch(p, M::Beq, "b1_stream_fits");
    branch(p, M::Bcs, "b1_stream_done");
    p.label("b1_stream_fits");
    abs(p, M::Sta, 0xe8);
    abs(p, M::Lda, BUDGET);
    implied(p, M::Sec);
    abs(p, M::Sbc, 0xe8);
    abs(p, M::Sta, BUDGET);
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    call(p, "b1_descriptor");
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    emit(p, M::Lda, A::AbsoluteX, O::Word(QUEUE));
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Word(8));
    emit(p, M::Sta, A::AbsoluteX, O::Word(QUEUE));
    emit(p, M::Lda, A::AbsoluteX, O::Word(QUEUE + 3));
    emit(p, M::Dec, A::Accumulator, O::None);
    emit(p, M::Sta, A::AbsoluteX, O::Word(QUEUE + 3));
    p.emit_label_ref(M::Brl, A::Relative16, "b1_stream_slot");
    p.label("b1_stream_next_slot");
    implied(p, M::Txa);
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Word(5));
    implied(p, M::Tax);
    emit(p, M::Cpx, A::Immediate, O::Word(QUEUE_SLOTS * 5));
    branch(p, M::Beq, "b1_stream_done");
    p.emit_label_ref(M::Brl, A::Relative16, "b1_stream_slot");
    p.label("b1_stream_done");
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    implied(p, M::Rts);
    p.label("b1_stream_group")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    emit(p, M::Ldy, A::Immediate, O::Word(3));
    emit(p, M::Lda, A::DirectPageIndirectLongIndexedY, O::Byte(0xe4));
    abs(p, M::Cmp, BUDGET);
    branch(p, M::Beq, "b1_stream_group_fits");
    branch(p, M::Bcc, "b1_stream_group_fits");
    emit(p, M::Ldy, A::Immediate, O::Word(5));
    emit(p, M::Lda, A::DirectPageIndirectLongIndexedY, O::Byte(0xe4));
    emit(p, M::And, A::Immediate, O::Word(0xff));
    branch(p, M::Beq, "b1_stream_group_waits");
    abs(p, M::Lda, BUDGET);
    emit(p, M::Cmp, A::Immediate, O::Word(STREAM_BUDGET));
    branch(p, M::Bne, "b1_stream_group_waits");
    abs(p, M::Stz, BUDGET);
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    emit(p, M::Lda, A::Immediate, O::Byte(1));
    abs(p, M::Sta, crate::sfc_b1_text::STONE_SKIP);
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    branch(p, M::Bra, "b1_stream_group_send");
    p.label("b1_stream_group_waits");
    implied(p, M::Clc);
    implied(p, M::Rts);
    p.label("b1_stream_group_fits");
    abs(p, M::Sta, 0xe8);
    abs(p, M::Lda, BUDGET);
    implied(p, M::Sec);
    abs(p, M::Sbc, 0xe8);
    abs(p, M::Sta, BUDGET);
    p.label("b1_stream_group_send");
    emit(p, M::Ldy, A::Immediate, O::Word(1));
    emit(p, M::Lda, A::DirectPageIndirectLongIndexedY, O::Byte(0xe4));
    abs(p, M::Sta, GROUP_LEFT);
    p.label("b1_stream_group_next");
    emit(p, M::Lda, A::AbsoluteX, O::Word(QUEUE));
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Word(8));
    emit(p, M::Sta, A::AbsoluteX, O::Word(QUEUE));
    abs(p, M::Sta, 0xe4);
    emit(p, M::Lda, A::AbsoluteX, O::Word(QUEUE + 3));
    emit(p, M::Dec, A::Accumulator, O::None);
    emit(p, M::Sta, A::AbsoluteX, O::Word(QUEUE + 3));
    abs(p, M::Lda, GROUP_LEFT);
    branch(p, M::Beq, "b1_stream_group_sent");
    abs(p, M::Dec, GROUP_LEFT);
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    call(p, "b1_descriptor");
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    branch(p, M::Bra, "b1_stream_group_next");
    p.label("b1_stream_group_sent");
    implied(p, M::Sec);
    implied(p, M::Rts);
    p.label("b1_descriptor").assume_mode(CpuMode::NATIVE_M8_X16);
    let field = |p: &mut w65c816::Assembler, index: u16, to: u16| {
        emit(p, M::Ldy, A::Immediate, O::Word(index));
        emit(p, M::Lda, A::DirectPageIndirectLongIndexedY, O::Byte(0xe4));
        emit(p, M::Sta, A::Absolute, O::Word(to));
    };
    emit(p, M::Lda, A::DirectPageIndirectLong, O::Byte(0xe4));
    branch(p, M::Bne, "b1_descriptor_cgram");
    write(p, 0x2115, 0x80);
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    emit(p, M::Lda, A::Immediate, O::Word(0x1801));
    abs(p, M::Sta, 0x4300);
    field(p, 1, 0x2116);
    p.emit_label_ref(M::Bra, A::Relative8, "b1_descriptor_dma");
    p.label("b1_descriptor_cgram")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    emit(p, M::Cmp, A::Immediate, O::Byte(2));
    branch(p, M::Beq, "b1_descriptor_oam");
    emit(
        p,
        M::Cmp,
        A::Immediate,
        O::Byte(crate::sfc_b1_view::VIEW_KIND),
    );
    branch(p, M::Bne, "b1_descriptor_kind_experience");
    p.emit_label_ref(M::Jmp, A::Absolute, "b1_descriptor_view");
    p.label("b1_descriptor_kind_experience");
    emit(
        p,
        M::Cmp,
        A::Immediate,
        O::Byte(crate::sfc_b1_view::EXPERIENCE_KIND),
    );
    branch(p, M::Bne, "b1_descriptor_kind_cgram");
    p.emit_label_ref(M::Jmp, A::Absolute, "b1_descriptor_experience");
    p.label("b1_descriptor_kind_cgram");
    field(p, 1, 0x2121);
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    emit(p, M::Lda, A::Immediate, O::Word(0x2200));
    abs(p, M::Sta, 0x4300);
    p.emit_label_ref(M::Bra, A::Relative8, "b1_descriptor_dma");
    p.label("b1_descriptor_oam");
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    emit(p, M::Lda, A::Immediate, O::Word(0x0400));
    abs(p, M::Sta, 0x4300);
    field(p, 1, 0x2102);
    p.label("b1_descriptor_dma")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    field(p, 3, 0x4305);
    field(p, 5, 0x4302);
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    field(p, 7, 0x4304);
    write(p, 0x420b, 1);
    implied(p, M::Rts);
    Ok(())
}
