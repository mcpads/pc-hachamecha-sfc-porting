use crate::Result;
use crate::sfc::{emit, implied, write};
use crate::sfc_battle_vm::{
    AUDIO_WAIT, COMMAND, DIM, EXIT, MENU, MESSAGE, MUSIC, MUSIC_PENDING, RESTORE, WALL,
};
use sfc_builder::cartridge::CartridgeLayout;
use w65c816::{AddressingMode as A, Assembler, CpuMode, Mnemonic as M, Operand as O};
pub const ACTIVE: u16 = 0xd12;
pub const LAG: u16 = 0xe04;
const UNDRAWN: u16 = 0xe05;
const MAX_UNDRAWN: u8 = 1;
pub(crate) const FAST_UNDRAWN: u8 = 2;
const CATCH_UP: u16 = 0xe0a;
pub(crate) const UPDATES: u16 = 0xe0b;
const DIM_BUFFER: u32 = 0x7f5100;
const DIM_LIST: u32 = DIM_BUFFER + 64;
const DIM_JOB: u32 = DIM_LIST + 8;
const WALL_FIRST: u8 = 48;
const WALL_COLORS: u16 = 32;
const LIST_END: u16 = 0xd15;
const SHIFTS: u16 = 0xd17;
const EXIT_DEFEAT: u8 = crate::sfc_battle_controller::EXIT_DEFEAT;
pub fn emit_field(p: &mut Assembler, layout: &CartridgeLayout, pressed: u16) -> Result<()> {
    let abs = |p: &mut Assembler, m, at: u16| emit(p, m, A::Absolute, O::Word(at));
    let branch = |p: &mut Assembler, m, label: &str| {
        p.emit_label_ref(m, A::Relative8, label);
    };
    let call = |p: &mut Assembler, label: &str| {
        p.emit_label_ref(M::Jsr, A::Absolute, label);
    };
    let songs = layout.cpu_address(layout.section("field_songs")?.offset)?;
    p.label("battle_enter").assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Stz, WALL);
    abs(p, M::Stz, LAG);
    abs(p, M::Stz, CATCH_UP);
    abs(p, M::Stz, UNDRAWN);
    for at in [
        crate::sfc_battle_message::STARTED,
        crate::sfc_battle_message::DRAW,
        crate::sfc_battle_message::SOUND_REQUEST,
        crate::sfc_battle_message::SOUND_REPEATS,
        crate::sfc_battle_message::SOUND_WAIT,
        crate::sfc_battle_message::BODY_REQUEST,
    ] {
        abs(p, M::Stz, at);
    }
    call(p, "battle_start");
    call(p, "battle_display_start");
    write(p, ACTIVE, 1);
    implied(p, M::Rts);
    p.label("scene_enter").assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Stz, WALL);
    abs(p, M::Stz, LAG);
    abs(p, M::Stz, CATCH_UP);
    abs(p, M::Stz, UNDRAWN);
    for at in [
        crate::sfc_battle_message::STARTED,
        crate::sfc_battle_message::DRAW,
        crate::sfc_battle_message::SOUND_REQUEST,
        crate::sfc_battle_message::SOUND_REPEATS,
        crate::sfc_battle_message::SOUND_WAIT,
        crate::sfc_battle_message::BODY_REQUEST,
    ] {
        abs(p, M::Stz, at);
    }
    write(
        p,
        crate::sfc_encounter::ENEMY,
        crate::sfc_battle_display::SCENE_ID as u8,
    );
    call(p, "battle_start");
    write(p, crate::sfc_battle_vm::SCENE, 1);
    for (i, b) in crate::sfc_battle_objects::SCENE_ROOT
        .to_le_bytes()
        .iter()
        .enumerate()
    {
        write(p, crate::sfc_battle_objects::SPAWN_PC + i as u16, *b);
    }
    call(p, "bvm_spawn_root");
    call(p, "battle_display_start");
    let backdrop = layout.cpu_address(layout.section("scene_backdrop")?.offset)?;
    for i in 0..5u32 {
        emit(p, M::Lda, A::AbsoluteLong, O::Long(backdrop + i));
        abs(p, M::Sta, crate::sfc_b1::QUEUE + i as u16);
    }
    write(p, ACTIVE, 1);
    implied(p, M::Rts);
    p.label("battle_enter_growth")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    call(p, "battle_growth_start");
    write(p, ACTIVE, 1);
    implied(p, M::Rts);
    p.label("b1_battle_tick")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, crate::sfc_b1_menu::MODE);
    branch(p, M::Beq, "b1_battle_no_bar");
    call(p, "b1_menu_tick");
    abs(p, M::Lda, ACTIVE);
    branch(p, M::Bne, "b1_battle_no_bar");
    implied(p, M::Rts);
    p.label("b1_battle_no_bar");
    abs(p, M::Inc, UPDATES);
    call(p, "battle_tick");
    abs(p, M::Lda, crate::sfc_b1_menu::SETTINGS);
    emit(
        p,
        M::And,
        A::Immediate,
        O::Byte(crate::sfc_b1_menu::FAST_ANIMATION),
    );
    branch(p, M::Beq, "b1_battle_normal_speed");
    abs(p, M::Lda, UNDRAWN);
    branch(p, M::Beq, "b1_battle_fast_behind");
    abs(p, M::Dec, UNDRAWN);
    abs(p, M::Stz, LAG);
    branch(p, M::Bra, "b1_battle_drawn");
    p.label("b1_battle_fast_behind");
    abs(p, M::Lda, LAG);
    branch(p, M::Beq, "b1_battle_draw");
    emit(p, M::Lda, A::Immediate, O::Byte(FAST_UNDRAWN - 1));
    abs(p, M::Sta, UNDRAWN);
    abs(p, M::Stz, LAG);
    branch(p, M::Bra, "b1_battle_drawn");
    p.label("b1_battle_normal_speed");
    abs(p, M::Lda, LAG);
    branch(p, M::Beq, "b1_battle_draw");
    abs(p, M::Lda, UNDRAWN);
    emit(p, M::Cmp, A::Immediate, O::Byte(MAX_UNDRAWN));
    branch(p, M::Bcs, "b1_battle_draw");
    abs(p, M::Inc, UNDRAWN);
    abs(p, M::Stz, LAG);
    branch(p, M::Bra, "b1_battle_drawn");
    p.label("b1_battle_draw");
    abs(p, M::Stz, UNDRAWN);
    abs(p, M::Stz, LAG);
    call(p, "battle_display_tick");
    p.label("b1_battle_drawn");
    abs(p, M::Lda, crate::sfc_ending::STOP);
    branch(p, M::Beq, "b1_battle_song_on");
    abs(p, M::Stz, crate::sfc_ending::STOP);
    call(p, "sound_stop");
    p.label("b1_battle_song_on");
    abs(p, M::Lda, MUSIC_PENDING);
    branch(p, M::Beq, "b1_battle_music_done");
    abs(p, M::Stz, MUSIC_PENDING);
    abs(p, M::Lda, MUSIC);
    emit(
        p,
        M::Cmp,
        A::Immediate,
        O::Byte(crate::sfc_battle_vm::MUSIC_ALONE),
    );
    branch(p, M::Bne, "b1_battle_music_slot");
    call(p, "sound_stop");
    emit(p, M::Lda, A::Immediate, O::Byte(0xff));
    abs(p, M::Sta, crate::sfc_sound::RETURN);
    p.label("b1_battle_music_slot");
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    abs(p, M::Lda, MUSIC + 1);
    emit(p, M::And, A::Immediate, O::Word(0xff));
    implied(p, M::Tax);
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(songs));
    branch(p, M::Beq, "b1_battle_music_done");
    emit(p, M::Dec, A::Accumulator, O::None);
    call(p, "sound_play");
    p.label("b1_battle_music_done");
    abs(p, M::Lda, crate::sfc_battle_vm::EFFECT_REQUEST);
    branch(p, M::Beq, "b1_battle_effect_done");
    abs(p, M::Lda, crate::sfc_sound::PENDING);
    emit(p, M::Cmp, A::Immediate, O::Byte(0xff));
    branch(p, M::Bne, "b1_battle_effect_done");
    abs(p, M::Lda, crate::sfc_battle_vm::EFFECT_REQUEST);
    abs(p, M::Stz, crate::sfc_battle_vm::EFFECT_REQUEST);
    call(p, "sound_effect_once");
    p.label("b1_battle_effect_done");
    abs(p, M::Lda, AUDIO_WAIT);
    branch(p, M::Beq, "b1_battle_audio_done");
    emit(
        p,
        M::Cmp,
        A::Immediate,
        O::Byte(crate::sfc_battle_vm::AUDIO_WAIT_EFFECT),
    );
    branch(p, M::Bne, "b1_battle_audio_music");
    abs(p, M::Lda, crate::sfc_battle_vm::LAST_EFFECT);
    call(p, "sound_effect_sounding");
    branch(p, M::Bcs, "b1_battle_audio_done");
    abs(p, M::Stz, AUDIO_WAIT);
    branch(p, M::Bra, "b1_battle_audio_done");
    p.label("b1_battle_audio_music");
    abs(p, M::Lda, crate::sfc_sound::PENDING);
    emit(p, M::Cmp, A::Immediate, O::Byte(0xff));
    branch(p, M::Bne, "b1_battle_audio_done");
    abs(p, M::Lda, crate::sfc_sound::TIMER);
    abs(p, M::Ora, crate::sfc_sound::TIMER + 1);
    branch(p, M::Bne, "b1_battle_audio_done");
    abs(p, M::Stz, AUDIO_WAIT);
    p.label("b1_battle_audio_done");
    abs(p, M::Stz, crate::sfc_battle_message::INPUT);
    abs(p, M::Lda, CATCH_UP);
    branch(p, M::Bne, "b1_battle_no_b");
    abs(p, M::Lda, pressed);
    emit(p, M::And, A::Immediate, O::Byte(0x80));
    branch(p, M::Beq, "b1_battle_no_a");
    abs(p, M::Inc, crate::sfc_battle_message::INPUT);
    p.label("b1_battle_no_a");
    abs(p, M::Lda, pressed + 1);
    emit(p, M::And, A::Immediate, O::Byte(0x80));
    branch(p, M::Beq, "b1_battle_no_b");
    emit(p, M::Lda, A::Immediate, O::Byte(2));
    abs(p, M::Ora, crate::sfc_battle_message::INPUT);
    abs(p, M::Sta, crate::sfc_battle_message::INPUT);
    p.label("b1_battle_no_b");
    abs(p, M::Lda, crate::sfc_battle_message::SOUND_WAIT);
    branch(p, M::Beq, "b1_battle_chant_checked");
    call(p, "sound_effect_sounding");
    branch(p, M::Bcs, "b1_battle_chant_checked");
    abs(p, M::Stz, crate::sfc_battle_message::SOUND_WAIT);
    p.label("b1_battle_chant_checked");
    call(p, "battle_message_tick");
    abs(p, M::Lda, crate::sfc_battle_message::SOUND_STOP);
    branch(p, M::Beq, "b1_battle_no_stop");
    abs(p, M::Stz, crate::sfc_battle_message::SOUND_STOP);
    call(p, "sound_effect_stop_code");
    p.label("b1_battle_no_stop");
    for (request, none) in [
        (
            crate::sfc_battle_message::BODY_REQUEST,
            "b1_battle_no_body_sound",
        ),
        (
            crate::sfc_battle_message::SOUND_REQUEST,
            "b1_battle_no_sound",
        ),
    ] {
        abs(p, M::Lda, request);
        branch(p, M::Beq, none);
        abs(p, M::Stz, request);
        if request == crate::sfc_battle_message::SOUND_REQUEST
            && layout.section("tad_loader").is_ok()
        {
            implied(p, M::Pha);
            abs(p, M::Lda, crate::sfc_battle_message::SOUND_REPEATS);
            abs(p, M::Sta, crate::sfc_tad::CHANT_REQUEST);
            abs(p, M::Stz, crate::sfc_battle_message::SOUND_REPEATS);
            implied(p, M::Pla);
            call(p, "sound_chant");
        } else {
            call(p, "sound_effect");
        }
        p.label(none);
    }
    abs(p, M::Lda, crate::sfc_battle_message::DRAW);
    branch(p, M::Beq, "b1_battle_message_done");
    abs(p, M::Stz, crate::sfc_battle_message::DRAW);
    crate::sfc::write_word(p, 0x501, crate::sfc_b1_text::RUNTIME_PAGE);
    call(p, "b1_text_request");
    p.label("b1_battle_message_done");
    abs(p, M::Lda, DIM);
    branch(p, M::Beq, "b1_battle_dim_done");
    abs(p, M::Stz, DIM);
    abs(p, M::Lda, crate::sfc_battle_vm::SCENE);
    branch(p, M::Bne, "b1_battle_dim_done");
    call(p, "battle_dim");
    p.label("b1_battle_dim_done");
    abs(p, M::Lda, MENU);
    branch(p, M::Beq, "b1_battle_bar_done");
    abs(p, M::Lda, COMMAND);
    abs(p, M::Ora, COMMAND + 1);
    abs(p, M::Ora, MESSAGE);
    abs(p, M::Ora, crate::sfc_b1_menu::MODE);
    branch(p, M::Bne, "b1_battle_bar_done");
    call(p, "b1_menu_open_battle");
    p.label("b1_battle_bar_done");
    abs(p, M::Lda, EXIT);
    branch(p, M::Bne, "b1_battle_exit");
    abs(p, M::Lda, CATCH_UP);
    branch(p, M::Bne, "b1_battle_caught_up");
    abs(p, M::Lda, LAG);
    branch(p, M::Beq, "b1_battle_on_time");
    abs(p, M::Lda, ACTIVE);
    branch(p, M::Beq, "b1_battle_on_time");
    write(p, CATCH_UP, 1);
    p.emit_label_ref(M::Jmp, A::Absolute, "b1_battle_no_bar");
    p.label("b1_battle_caught_up");
    abs(p, M::Stz, CATCH_UP);
    p.label("b1_battle_on_time");
    implied(p, M::Rts);
    p.label("b1_battle_exit");
    emit(p, M::Cmp, A::Immediate, O::Byte(EXIT_DEFEAT));
    branch(p, M::Bne, "b1_battle_leave");
    abs(p, M::Stz, ACTIVE);
    abs(p, M::Stz, crate::sfc_battle_vm::FADE);
    abs(p, M::Stz, crate::sfc_b1_menu::MODE);
    write(p, 0x4200, 0x01);
    call(p, "title_enter");
    write(p, 0x4200, 0x81);
    implied(p, M::Rts);
    p.label("b1_battle_leave");
    call(p, "scene_reserved");
    branch(p, M::Bcc, "b1_battle_leave_now");
    implied(p, M::Rts);
    p.label("b1_battle_leave_now");
    abs(p, M::Stz, ACTIVE);
    abs(p, M::Stz, crate::sfc_battle_vm::SCENE);
    abs(p, M::Lda, crate::sfc_ending::ACTIVE);
    branch(p, M::Beq, "b1_battle_to_field");
    implied(p, M::Rts);
    p.label("b1_battle_to_field");
    abs(p, M::Stz, crate::sfc_field_events::SCREEN_DARK);
    abs(p, M::Stz, crate::sfc_ending::SCREEN_WHITE);
    abs(p, M::Lda, crate::sfc_b1_menu::MODE);
    branch(p, M::Beq, "b1_battle_bar_closed");
    call(p, "b1_menu_close");
    p.label("b1_battle_bar_closed");
    call(p, "battle_display_end");
    call(p, "b1_render");
    abs(p, M::Lda, RESTORE);
    branch(p, M::Beq, "b1_battle_left");
    call(p, "field_floor_entry");
    p.label("b1_battle_left");
    crate::sfc::write_word(p, 0x501, crate::sfc_b1_menu::EMPTY_PAGE.into());
    p.emit_label_ref(M::Jmp, A::Absolute, "b1_page");
    let views = crate::sfc_b1::VIEW_JOB;
    let long = |p: &mut Assembler, m, at: u32| emit(p, m, A::AbsoluteLong, O::Long(at));
    let indirect = |p: &mut Assembler, m, dp: u8| {
        emit(p, m, A::DirectPageIndirectLongIndexedY, O::Byte(dp));
    };
    p.label("battle_dim").assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Stz, SHIFTS + 1);
    abs(p, M::Lda, WALL);
    branch(p, M::Bpl, "battle_dim_level");
    emit(p, M::Lda, A::Immediate, O::Byte(0));
    p.label("battle_dim_level");
    abs(p, M::Sta, SHIFTS);
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    emit(p, M::Lda, A::Immediate, O::Word(views as u16));
    abs(p, M::Sta, 0xe0);
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    write(p, 0xe2, (views >> 16) as u8);
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    emit(p, M::Ldy, A::Immediate, O::Word(0));
    indirect(p, M::Lda, 0xe0);
    abs(p, M::Sta, 0xe4);
    emit(p, M::Ldy, A::Immediate, O::Word(2));
    indirect(p, M::Lda, 0xe0);
    abs(p, M::Sta, 0xe6);
    emit(p, M::Ldy, A::Immediate, O::Word(3));
    indirect(p, M::Lda, 0xe0);
    emit(p, M::And, A::Immediate, O::Word(0xff));
    for _ in 0..3 {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    abs(p, M::Sta, LIST_END);
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    emit(p, M::Ldy, A::Immediate, O::Word(0));
    p.label("battle_dim_find");
    emit(p, M::Cpy, A::Absolute, O::Word(LIST_END));
    branch(p, M::Bcc, "battle_dim_look");
    implied(p, M::Rts);
    p.label("battle_dim_look");
    indirect(p, M::Lda, 0xe4);
    emit(p, M::Cmp, A::Immediate, O::Byte(1));
    branch(p, M::Bne, "battle_dim_next");
    implied(p, M::Iny);
    indirect(p, M::Lda, 0xe4);
    implied(p, M::Dey);
    emit(p, M::Cmp, A::Immediate, O::Byte(WALL_FIRST));
    branch(p, M::Beq, "battle_dim_found");
    p.label("battle_dim_next");
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    implied(p, M::Tya);
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Word(8));
    implied(p, M::Tay);
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    branch(p, M::Bra, "battle_dim_find");
    p.label("battle_dim_found");
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    for (i, at) in [(5u16, 0xe8u16), (6, 0xe9)] {
        implied(p, M::Phy);
        implied(p, M::Tya);
        implied(p, M::Clc);
        emit(p, M::Adc, A::Immediate, O::Word(i));
        implied(p, M::Tay);
        indirect(p, M::Lda, 0xe4);
        abs(p, M::Sta, at);
        implied(p, M::Ply);
    }
    emit(p, M::Ldy, A::Immediate, O::Word(0));
    p.label("battle_dim_colour");
    indirect(p, M::Lda, 0xe8);
    abs(p, M::Ldx, SHIFTS);
    branch(p, M::Beq, "battle_dim_store");
    p.label("battle_dim_shift");
    emit(p, M::Lsr, A::Accumulator, O::None);
    emit(p, M::And, A::Immediate, O::Word(0x3def));
    implied(p, M::Dex);
    branch(p, M::Bne, "battle_dim_shift");
    p.label("battle_dim_store");
    implied(p, M::Tyx);
    emit(p, M::Sta, A::AbsoluteLongX, O::Long(DIM_BUFFER));
    implied(p, M::Iny);
    implied(p, M::Iny);
    emit(p, M::Cpy, A::Immediate, O::Word(2 * WALL_COLORS));
    branch(p, M::Bne, "battle_dim_colour");
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    let descriptor =
        crate::sfc_b1::transfer(DIM_BUFFER, WALL_FIRST.into(), 2 * WALL_COLORS as usize, 1);
    for (i, b) in descriptor.iter().enumerate() {
        emit(p, M::Lda, A::Immediate, O::Byte(*b));
        long(p, M::Sta, DIM_LIST + i as u32);
    }
    for (i, b) in DIM_LIST.to_le_bytes()[..3]
        .iter()
        .chain(&[1, 0])
        .enumerate()
    {
        emit(p, M::Lda, A::Immediate, O::Byte(*b));
        long(p, M::Sta, DIM_JOB + i as u32);
    }
    for (i, b) in DIM_JOB.to_le_bytes()[..3].iter().enumerate() {
        write(p, 0xe0 + i as u16, *b);
    }
    emit(p, M::Ldx, A::Immediate, O::Word(crate::sfc_b1::FADE_SLOT));
    p.emit_label_ref(M::Jmp, A::Absolute, "b1_enqueue");
    Ok(())
}
