use super::*;
pub fn emit_field(p: &mut Assembler) -> Result<()> {
    p.label("ending_nmi").assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, ACTIVE);
    byte(p, M::Cmp, 1);
    branch(p, M::Bne, "ending_nmi_running");
    call(p, "ending_setup");
    call(p, "b1_stream_all");
    call(p, "ending_start");
    write(p, 0x2100, 0x0f);
    call(p, "sound_frame");
    jump(p, "sample");
    p.label("ending_nmi_running")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    call(p, "b1_stream_title");
    call(p, "ending_registers");
    call(p, "sound_frame");
    call(p, "sample");
    call(p, "ending_frame");
    abs(p, M::Lda, STOP);
    branch(p, M::Beq, "ending_nmi_no_stop");
    abs(p, M::Stz, STOP);
    call(p, "sound_stop");
    byte(p, M::Lda, 0xff);
    abs(p, M::Sta, crate::sfc_sound::RETURN);
    p.label("ending_nmi_no_stop");
    abs(p, M::Lda, SONG);
    branch(p, M::Beq, "ending_nmi_no_song");
    abs(p, M::Stz, SONG);
    emit(p, M::Dec, A::Accumulator, O::None);
    call(p, "sound_play");
    p.label("ending_nmi_no_song");
    for (request, none) in [
        (
            crate::sfc_battle_message::BODY_REQUEST,
            "ending_nmi_no_body_sound",
        ),
        (
            crate::sfc_battle_message::SOUND_REQUEST,
            "ending_nmi_no_sound",
        ),
    ] {
        abs(p, M::Lda, request);
        branch(p, M::Beq, none);
        abs(p, M::Stz, request);
        call(p, "sound_effect");
        p.label(none);
    }
    abs(p, M::Lda, crate::sfc_battle_message::SOUND_STOP);
    branch(p, M::Beq, "ending_nmi_no_effect_stop");
    abs(p, M::Stz, crate::sfc_battle_message::SOUND_STOP);
    call(p, "sound_effect_stop_code");
    p.label("ending_nmi_no_effect_stop");
    abs(p, M::Lda, ACTIVE);
    branch(p, M::Beq, "ending_nmi_title");
    implied(p, M::Rts);
    p.label("ending_nmi_title");
    write(p, 0x4200, 0x01);
    call(p, "title_enter");
    write(p, 0x4200, 0x81);
    implied(p, M::Rts);
    Ok(())
}
pub(super) fn emit_nmi_rules(p: &mut Assembler, layout: &CartridgeLayout) -> Result<()> {
    let address = |name: &str| -> Result<u32> { layout.cpu_address(layout.section(name)?.offset) };
    let songs = address("field_songs")?;
    let song_frames = address("sound_song_frames")?;
    let zero = address("ending_zero")?;
    let entry_job = address("ending_entry_job")?;
    let hidden = address("menu_hidden_oam")?;
    let pressed = sfc_builder::runtime::PadStorage::new(0x300)?.pressed();
    use crate::sfc_battle_vm::{MUSIC, MUSIC_PENDING};
    let queue = crate::sfc_b1::QUEUE;
    p.label("scene_reserved")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    abs(p, M::Lda, RESERVED);
    word(p, M::Cmp, ROOT as u16);
    branch(p, M::Bne, "scene_reserved_battle");
    abs(p, M::Lda, RESERVED + 2);
    word(p, M::Cmp, (ROOT >> 16) as u16);
    branch(p, M::Bne, "scene_reserved_battle");
    abs(p, M::Stz, RESERVED);
    abs(p, M::Stz, RESERVED + 2);
    m8(p);
    abs(p, M::Stz, crate::sfc_battle_vm::FADE);
    byte(p, M::Lda, 1);
    abs(p, M::Sta, ACTIVE);
    implied(p, M::Clc);
    implied(p, M::Rts);
    p.label("scene_reserved_battle")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    for (i, root) in BATTLE_SCENES.iter().enumerate() {
        let next = format!("scene_reserved_not_{i}");
        abs(p, M::Lda, RESERVED);
        word(p, M::Cmp, *root as u16);
        branch(p, M::Bne, &next);
        abs(p, M::Lda, RESERVED + 2);
        word(p, M::Cmp, (*root >> 16) as u16);
        branch(p, M::Bne, &next);
        abs(p, M::Stz, RESERVED);
        abs(p, M::Stz, RESERVED + 2);
        m8(p);
        call(p, "bvm_reset");
        byte(p, M::Lda, 1);
        abs(p, M::Sta, crate::sfc_battle_vm::SCENE);
        abs(p, M::Stz, crate::sfc_battle_vm::EXIT);
        abs(p, M::Stz, STOP);
        for (k, b) in root.to_le_bytes().iter().enumerate() {
            write(p, crate::sfc_battle_objects::SPAWN_PC + k as u16, *b);
        }
        call(p, "bvm_spawn_root");
        implied(p, M::Sec);
        implied(p, M::Rts);
        p.label(&next).assume_mode(CpuMode::NATIVE_M16_X16);
    }
    m8(p);
    implied(p, M::Clc);
    implied(p, M::Rts);
    p.label("ending_setup").assume_mode(CpuMode::NATIVE_M8_X16);
    write(p, 0x2100, 0x80);
    write(p, 0x420c, 0);
    for (at, value) in [
        (0x2105u16, 0x03u8),
        (0x2106, 0),
        (0x2107, (A_BG1_MAP >> 9) as u8),
        (0x2108, (A_TEXT_MAP >> 9) as u8),
        (0x2109, (B_TEXT_MAP >> 9) as u8),
        (0x210b, (TEXT_TILES >> 13 << 4) as u8),
        (0x210c, (TEXT_TILES >> 13) as u8),
        (0x2101, (A_OBJ >> 14) as u8 | 0x20),
        (0x212c, 0x12),
        (0x212d, 0),
        (0x212e, 0),
        (0x212f, 0),
        (0x2130, 0),
        (0x2131, 0),
        (0x2133, 0),
        (0x2123, 0),
        (0x2124, 0),
        (0x2125, 0),
    ] {
        write(p, at, value);
    }
    for at in [0x210du16, 0x210f, 0x2111] {
        write(p, at, 0);
        write(p, at, 0);
    }
    for at in [0x210eu16, 0x2110, 0x2112] {
        write(p, at, 0xff);
        write(p, at, 0xff);
    }
    write(p, 0x2115, 0x80);
    crate::sfc::write_word(p, 0x2116, 0);
    crate::sfc::write_word(p, 0x4300, 0x1809);
    crate::sfc::write_word(p, 0x4302, zero as u16);
    write(p, 0x4304, (zero >> 16) as u8);
    crate::sfc::write_word(p, 0x4305, 0);
    write(p, 0x420b, 1);
    write(p, 0x2121, 0);
    crate::sfc::write_word(p, 0x4300, 0x2208);
    crate::sfc::write_word(p, 0x4305, 512);
    write(p, 0x420b, 1);
    crate::sfc::write_word(p, 0x2102, 0);
    crate::sfc::write_word(p, 0x4300, 0x0400);
    crate::sfc::write_word(p, 0x4302, hidden as u16);
    write(p, 0x4304, (hidden >> 16) as u8);
    crate::sfc::write_word(p, 0x4305, 544);
    write(p, 0x420b, 1);
    emit(p, M::Ldx, A::Immediate, O::Word(0));
    p.label("ending_hidden_oam");
    long_x(p, M::Lda, hidden);
    ax(p, M::Sta, crate::sfc_b1_menu::OAM_BUFFER);
    implied(p, M::Inx);
    emit(p, M::Cpx, A::Immediate, O::Word(544));
    branch(p, M::Bne, "ending_hidden_oam");
    emit(p, M::Ldx, A::Immediate, O::Word(0));
    p.label("ending_queue_clear");
    ax(p, M::Stz, queue);
    implied(p, M::Inx);
    emit(p, M::Cpx, A::Immediate, O::Word(crate::sfc_b1::QUEUE_BYTES));
    branch(p, M::Bne, "ending_queue_clear");
    enqueue(p, entry_job, SLOT_LOAD);
    byte(p, M::Lda, 0xff);
    abs(p, M::Sta, crate::sfc_sound::RETURN);
    abs(p, M::Stz, crate::sfc_battle_field::ACTIVE);
    abs(p, M::Stz, crate::sfc_b1_menu::MODE);
    byte(p, M::Lda, 2);
    abs(p, M::Sta, ACTIVE);
    implied(p, M::Rts);
    p.label("ending_registers")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, DP + u16::from(SCREEN_LAYOUT));
    byte(p, M::Cmp, 1);
    branch(p, M::Bne, "ending_registers_b");
    for (at, value) in [
        (0x2105u16, 0x03u8),
        (0x2107, (A_BG1_MAP >> 9) as u8),
        (0x2108, (A_TEXT_MAP >> 9) as u8),
        (0x210b, (TEXT_TILES >> 13 << 4) as u8),
        (0x2101, (A_OBJ >> 14) as u8 | 0x20),
    ] {
        write(p, at, value);
    }
    branch(p, M::Bra, "ending_registers_scroll");
    p.label("ending_registers_b")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    for (at, value) in [
        (0x2105u16, 0x09u8),
        (0x2107, (B_BG1_MAP >> 9) as u8 | 2),
        (0x2108, (B_BG2_MAP >> 9) as u8 | 2),
        (0x2109, (B_TEXT_MAP >> 9) as u8),
        (0x210b, (B_BG2_TILES >> 13 << 4) as u8),
        (0x210c, (TEXT_TILES >> 13) as u8),
        (0x2101, (B_OBJ >> 14) as u8 | 0x60),
    ] {
        write(p, at, value);
    }
    p.label("ending_registers_scroll");
    abs(p, M::Lda, DP + u16::from(SCREEN));
    abs(p, M::Sta, 0x212c);
    for (at, reg) in [(BG1_V, 0x210eu16), (BG2_H, 0x210f), (BG2_V, 0x2110)] {
        abs(p, M::Lda, DP + u16::from(at));
        abs(p, M::Sta, reg);
        abs(p, M::Lda, DP + u16::from(at) + 1);
        abs(p, M::Sta, reg);
    }
    implied(p, M::Rts);
    p.label("ending_frame").assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, DP + u16::from(STAGE));
    branch(p, M::Beq, "ending_frame_body");
    abs(p, M::Lda, crate::sfc_sound::MUSIC);
    byte(p, M::Cmp, 0xff);
    branch(p, M::Beq, "ending_frame_stopped");
    abs(p, M::Lda, crate::sfc_sound::PENDING);
    byte(p, M::Cmp, 0xff);
    branch(p, M::Bne, "ending_frame_wait");
    abs(p, M::Lda, crate::sfc_sound::TIMER);
    abs(p, M::Ora, crate::sfc_sound::TIMER + 1);
    branch(p, M::Bne, "ending_frame_wait");
    abs(p, M::Lda, crate::sfc_sound::JOB);
    byte(p, M::Cmp, crate::sfc_sound::JOB_SONG);
    branch(p, M::Beq, "ending_frame_wait");
    m16(p);
    abs(p, M::Lda, crate::sfc_sound::MUSIC);
    word(p, M::And, 0xff);
    acc(p, M::Asl);
    implied(p, M::Tax);
    long_x(p, M::Lda, song_frames);
    m8(p);
    branch(p, M::Bne, "ending_frame_stopped");
    p.label("ending_frame_wait");
    implied(p, M::Rts);
    p.label("ending_frame_stopped")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Stz, ACTIVE);
    abs(p, M::Stz, crate::sfc_battle_vm::SCENE);
    abs(p, M::Stz, crate::sfc_field_events::SCREEN_DARK);
    write(p, 0x2100, 0x80);
    for (at, value) in [(0x2105u16, 3u8), (0x2123, 3), (0x2109, 0), (0x210c, 0)] {
        write(p, at, value);
    }
    write(p, 0x210f, 0);
    write(p, 0x210f, 0);
    write(p, 0x2110, 0xff);
    write(p, 0x2110, 0xff);
    write(p, 0x420c, 2);
    implied(p, M::Rts);
    p.label("ending_frame_body")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Stz, crate::sfc_battle_message::INPUT);
    abs(p, M::Lda, pressed);
    byte(p, M::And, 0x80);
    branch(p, M::Beq, "ending_frame_no_a");
    abs(p, M::Inc, crate::sfc_battle_message::INPUT);
    p.label("ending_frame_no_a");
    abs(p, M::Lda, pressed + 1);
    byte(p, M::And, 0x80);
    branch(p, M::Beq, "ending_frame_no_b");
    byte(p, M::Lda, 2);
    abs(p, M::Ora, crate::sfc_battle_message::INPUT);
    abs(p, M::Sta, crate::sfc_battle_message::INPUT);
    p.label("ending_frame_no_b");
    call(p, "ending_tick");
    abs(p, M::Lda, MUSIC_PENDING);
    branch(p, M::Beq, "ending_frame_music_done");
    abs(p, M::Stz, MUSIC_PENDING);
    abs(p, M::Lda, MUSIC + 1);
    m16(p);
    word(p, M::And, 0xff);
    implied(p, M::Tax);
    m8(p);
    long_x(p, M::Lda, songs);
    abs(p, M::Sta, SONG);
    p.label("ending_frame_music_done");
    abs(p, M::Lda, crate::sfc_battle_message::DRAW);
    branch(p, M::Beq, "ending_frame_no_page");
    abs(p, M::Stz, crate::sfc_battle_message::DRAW);
    abs(p, M::Inc, DP + u16::from(TEXT_SERIAL));
    p.label("ending_frame_no_page");
    implied(p, M::Rts);
    Ok(())
}
