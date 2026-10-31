use crate::{
    Result,
    sfc::{emit as op, implied as imp, write},
    sfc_sound::{EXCHANGES, JOB, MUSIC, PENDING, RETURN, TIMER, WANTED},
};
use sfc_builder::cartridge::CartridgeLayout;
use w65c816::{AddressingMode as A, Assembler, CpuMode, Mnemonic as M, Operand as O};
mod state;
pub(crate) use state::CHANT_REQUEST;
use state::*;
fn abs(p: &mut Assembler, m: M, a: u16) {
    op(p, m, A::Absolute, O::Word(a));
}
fn ax(p: &mut Assembler, m: M, a: u16) {
    op(p, m, A::AbsoluteX, O::Word(a));
}
fn byte(p: &mut Assembler, m: M, b: u8) {
    op(p, m, A::Immediate, O::Byte(b));
}
fn word(p: &mut Assembler, m: M, b: u16) {
    op(p, m, A::Immediate, O::Word(b));
}
fn longx(p: &mut Assembler, m: M, b: u32) {
    op(p, m, A::AbsoluteLongX, O::Long(b));
}
fn call(p: &mut Assembler, s: &str) {
    p.emit_label_ref(M::Jsr, A::Absolute, s);
}
fn jump(p: &mut Assembler, s: &str) {
    p.emit_label_ref(M::Jmp, A::Absolute, s);
}
fn br(p: &mut Assembler, m: M, s: &str) {
    let inverse = match m {
        M::Beq => M::Bne,
        M::Bne => M::Beq,
        M::Bcc => M::Bcs,
        M::Bcs => M::Bcc,
        _ => panic!("branch"),
    };
    op(p, inverse, A::Relative8, O::Relative8(3));
    p.emit_label_ref(M::Brl, A::Relative16, s);
}
fn m8(p: &mut Assembler) {
    op(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
}
fn m16(p: &mut Assembler) {
    op(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
}
fn label(p: &mut Assembler, s: &str) {
    p.label(s).assume_mode(CpuMode::NATIVE_M8_X16);
}
fn l16(p: &mut Assembler, s: &str) {
    p.label(s).assume_mode(CpuMode::NATIVE_M16_X16);
}
fn pointer(p: &mut Assembler, a: u32, n: u16) {
    for (i, b) in a.to_le_bytes()[..3].iter().enumerate() {
        write(p, n + i as u16, *b);
    }
}
fn command(p: &mut Assembler, cmd: u8) {
    write(p, COMMAND, cmd);
    call(p, "tad_command");
}
mod chants;
mod streams;
mod transport;
pub fn emit(p: &mut Assembler, layout: &CartridgeLayout) -> Result<()> {
    let at = |s: &str| -> Result<u32> { layout.cpu_address(layout.section(s)?.offset) };
    let loader = at("tad_loader")?;
    let driver = at("tad_driver")?;
    let loader_size = u16::try_from(layout.section("tad_loader")?.bytes.len())?;
    let driver_size = u16::try_from(layout.section("tad_driver")?.bytes.len())?;
    let songs = at("tad_songs")?;
    let frames = at("sound_song_frames")?;
    let clips = at("tad_clips")?;
    transport::emit(p, loader, driver, loader_size, driver_size);
    label(p, "sound_play");
    abs(p, M::Sta, PENDING);
    imp(p, M::Rts);
    label(p, "sound_music");
    abs(p, M::Sta, PENDING);
    write(p, SYNCHRONOUS, 1);
    abs(p, M::Inc, BUSY);
    call(p, "tad_start_song");
    label(p, "tad_music_wait");
    abs(p, M::Lda, READY);
    br(p, M::Beq, "tad_music_done");
    abs(p, M::Lda, JOB);
    br(p, M::Beq, "tad_music_done");
    call(p, "tad_song_job");
    jump(p, "tad_music_wait");
    label(p, "tad_music_done");
    abs(p, M::Stz, SYNCHRONOUS);
    abs(p, M::Dec, BUSY);
    imp(p, M::Rts);
    label(p, "tad_start_song");
    abs(p, M::Lda, PENDING);
    byte(p, M::Cmp, 28);
    br(p, M::Bcs, "tad_start_done");
    abs(p, M::Sta, SONG);
    abs(p, M::Lda, TIMER);
    abs(p, M::Ora, TIMER + 1);
    br(p, M::Bne, "tad_keep_return");
    abs(p, M::Lda, MUSIC);
    byte(p, M::Cmp, 28);
    br(p, M::Bcs, "tad_keep_return");
    abs(p, M::Sta, RETURN);
    label(p, "tad_keep_return");
    write(p, PENDING, 0xff);
    abs(p, M::Stz, STOPPED);
    for slot in SLOTS {
        for a in [STARTED, LEFT, LEFT + 1, CLIP_TIMER, CLIP_TIMER + 1] {
            abs(p, M::Stz, a + slot);
        }
        for a in [LOADED, CURRENT] {
            write(p, a + slot, 0xff);
        }
    }
    abs(p, M::Lda, 0x2141);
    byte(p, M::Cmp, 0x4c);
    br(p, M::Bne, "tad_restart_running");
    write(p, 0x2143, 0xa0);
    jump(p, "tad_restart_ready");
    label(p, "tad_restart_running");
    write(p, 0x2143, 0x20);
    label(p, "tad_restart_ready");
    m16(p);
    word(p, M::Lda, 300);
    abs(p, M::Sta, READY_WAIT);
    m8(p);
    write(p, READY_LAST, 0xff);
    write(p, JOB, 1);
    write(p, STAGE, 0);
    call(p, "tad_song_index");
    m16(p);
    longx(p, M::Lda, songs + 12);
    abs(p, M::Sta, RING);
    longx(p, M::Lda, songs + 14);
    abs(p, M::Sta, RING + SLOT_STRIDE);
    longx(p, M::Lda, songs);
    abs(p, M::Cmp, COMMON);
    m8(p);
    br(p, M::Bne, "tad_reload_common");
    longx(p, M::Lda, songs + 2);
    abs(p, M::Cmp, COMMON + 2);
    br(p, M::Bne, "tad_reload_common");
    write(p, STAGE, 1);
    call(p, "tad_score_pointer");
    write(p, TYPE, 0xe2);
    call(p, "tad_begin_upload");
    jump(p, "tad_start_done");
    label(p, "tad_reload_common");
    for a in [COMMON, COMMON + 1, COMMON + 2] {
        abs(p, M::Stz, a);
    }
    call(p, "tad_common_pointer");
    write(p, TYPE, 1);
    call(p, "tad_begin_upload");
    label(p, "tad_start_done");
    imp(p, M::Rts);
    label(p, "tad_song_index");
    m16(p);
    abs(p, M::Lda, SONG);
    word(p, M::And, 255);
    for _ in 0..4 {
        op(p, M::Asl, A::Accumulator, O::None);
    }
    imp(p, M::Tax);
    m8(p);
    imp(p, M::Rts);
    for (label_name, offset) in [("tad_common_pointer", 0), ("tad_score_pointer", 6)] {
        label(p, label_name);
        for i in 0..3 {
            longx(p, M::Lda, songs + offset + i);
            abs(p, M::Sta, PTR + i as u16);
        }
        m16(p);
        longx(p, M::Lda, songs + offset + 4);
        abs(p, M::Sta, SIZE);
        m8(p);
        imp(p, M::Rts);
    }
    label(p, "tad_song_job");
    abs(p, M::Lda, STAGE);
    byte(p, M::Cmp, 2);
    br(p, M::Beq, "tad_song_ready");
    call(p, "tad_upload_chunk");
    m16(p);
    abs(p, M::Lda, OFFSET);
    abs(p, M::Cmp, SIZE);
    m8(p);
    br(p, M::Bcc, "tad_job_return");
    abs(p, M::Inc, STAGE);
    abs(p, M::Lda, STAGE);
    byte(p, M::Cmp, 2);
    br(p, M::Beq, "tad_job_return");
    call(p, "tad_song_index");
    for i in 0..3 {
        longx(p, M::Lda, songs + i);
        abs(p, M::Sta, COMMON + i as u16);
    }
    call(p, "tad_score_pointer");
    write(p, TYPE, 0xe2);
    call(p, "tad_begin_upload");
    jump(p, "tad_job_return");
    label(p, "tad_song_ready");
    abs(p, M::Lda, 0x2141);
    byte(p, M::Cmp, 0x61);
    br(p, M::Bne, "tad_ready_waiting");
    abs(p, M::Lda, 0x2140);
    br(p, M::Bne, "tad_ready_waiting");
    abs(p, M::Stz, TOKEN);
    abs(p, M::Stz, JOB);
    abs(p, M::Lda, SONG);
    abs(p, M::Sta, MUSIC);
    m16(p);
    word(p, M::And, 255);
    op(p, M::Asl, A::Accumulator, O::None);
    imp(p, M::Tax);
    longx(p, M::Lda, frames);
    abs(p, M::Sta, TIMER);
    m8(p);
    abs(p, M::Lda, crate::sfc_b1_menu::SETTINGS);
    byte(p, M::And, crate::sfc_b1_menu::BGM_OFF);
    abs(p, M::Ora, STOPPED);
    br(p, M::Beq, "tad_job_return");
    abs(p, M::Lda, MUSIC);
    abs(p, M::Sta, WANTED);
    call(p, "sound_stop");
    jump(p, "tad_job_return");
    label(p, "tad_ready_waiting");
    abs(p, M::Lda, SYNCHRONOUS);
    br(p, M::Beq, "tad_ready_async");
    abs(p, M::Lda, 0x4212);
    byte(p, M::And, 0x80);
    abs(p, M::Cmp, READY_LAST);
    br(p, M::Beq, "tad_job_return");
    abs(p, M::Sta, READY_LAST);
    byte(p, M::Cmp, 0);
    br(p, M::Beq, "tad_job_return");
    jump(p, "tad_ready_count");
    label(p, "tad_ready_async");
    abs(p, M::Lda, TICK);
    abs(p, M::Cmp, READY_LAST);
    br(p, M::Beq, "tad_job_return");
    abs(p, M::Sta, READY_LAST);
    label(p, "tad_ready_count");
    m16(p);
    abs(p, M::Dec, READY_WAIT);
    m8(p);
    br(p, M::Bne, "tad_job_return");
    write(p, FAULT, 3);
    abs(p, M::Stz, READY);
    label(p, "tad_job_return");
    imp(p, M::Rts);
    label(p, "sound_frame_owed");
    imp(p, M::Rts);
    label(p, "sound_frame_now");
    for i in 0..TRANSFER_POINTER_BYTES {
        op(p, M::Lda, A::DirectPage, O::Byte(TRANSFER_POINTER + i));
        imp(p, M::Pha);
    }
    call(p, "sound_frame");
    for i in (0..TRANSFER_POINTER_BYTES).rev() {
        imp(p, M::Pla);
        op(p, M::Sta, A::DirectPage, O::Byte(TRANSFER_POINTER + i));
    }
    imp(p, M::Rts);
    label(p, "sound_frame");
    abs(p, M::Inc, TICK);
    abs(p, M::Lda, BUSY);
    br(p, M::Bne, "tad_frame_done");
    abs(p, M::Lda, READY);
    br(p, M::Beq, "tad_frame_done");
    abs(p, M::Inc, BUSY);
    abs(p, M::Inc, EXCHANGES);
    abs(p, M::Lda, TICK);
    imp(p, M::Pha);
    imp(p, M::Sec);
    abs(p, M::Sbc, LAST_TICK);
    abs(p, M::Sta, ELAPSED);
    imp(p, M::Pla);
    abs(p, M::Sta, LAST_TICK);
    abs(p, M::Lda, JOB);
    br(p, M::Beq, "tad_frame_playing");
    call(p, "tad_song_job");
    jump(p, "tad_frame_leave");
    label(p, "tad_frame_playing");
    m16(p);
    abs(p, M::Lda, ELAPSED);
    word(p, M::And, 255);
    imp(p, M::Tay);
    l16(p, "tad_music_timer_step");
    abs(p, M::Lda, TIMER);
    br(p, M::Beq, "tad_timer_done");
    abs(p, M::Dec, TIMER);
    br(p, M::Beq, "tad_music_timer_expired");
    imp(p, M::Dey);
    br(p, M::Bne, "tad_music_timer_step");
    jump(p, "tad_timer_done");
    l16(p, "tad_music_timer_expired");
    m8(p);
    abs(p, M::Lda, RETURN);
    abs(p, M::Sta, PENDING);
    jump(p, "tad_frame_pending");
    l16(p, "tad_timer_done");
    m8(p);
    label(p, "tad_frame_pending");
    abs(p, M::Lda, PENDING);
    byte(p, M::Cmp, 28);
    br(p, M::Bcs, "tad_frame_clips");
    abs(p, M::Lda, crate::sfc_b1_menu::SETTINGS);
    byte(p, M::And, crate::sfc_b1_menu::BGM_OFF);
    br(p, M::Beq, "tad_pending_audible");
    m16(p);
    abs(p, M::Lda, PENDING);
    word(p, M::And, 255);
    op(p, M::Asl, A::Accumulator, O::None);
    imp(p, M::Tax);
    longx(p, M::Lda, frames);
    m8(p);
    br(p, M::Bne, "tad_pending_silent");
    abs(p, M::Lda, PENDING);
    abs(p, M::Sta, WANTED);
    label(p, "tad_pending_silent");
    write(p, PENDING, 0xff);
    jump(p, "tad_frame_clips");
    label(p, "tad_pending_audible");
    abs(p, M::Lda, PENDING);
    abs(p, M::Cmp, MUSIC);
    br(p, M::Bne, "tad_frame_new_song");
    write(p, PENDING, 0xff);
    jump(p, "tad_frame_clips");
    label(p, "tad_frame_new_song");
    call(p, "tad_start_song");
    jump(p, "tad_frame_leave");
    label(p, "tad_frame_clips");
    word(p, M::Ldx, 32);
    call(p, "tad_clip_frame");
    word(p, M::Ldx, 0);
    call(p, "tad_clip_frame");
    label(p, "tad_frame_leave");
    abs(p, M::Dec, BUSY);
    label(p, "tad_frame_done");
    imp(p, M::Rts);
    streams::emit(
        p,
        clips,
        at("tad_stream_rates")?,
        at("tad_chant_map")?,
        at("tad_chants")?,
    );
    chants::emit(p, clips, at("tad_chant_map")?, at("tad_chants")?);
    label(p, "sound_stop");
    abs(p, M::Inc, BUSY);
    write(p, STOPPED, 1);
    write(p, MUSIC, 0xff);
    write(p, PENDING, 0xff);
    m16(p);
    abs(p, M::Stz, TIMER);
    m8(p);
    abs(p, M::Lda, JOB);
    br(p, M::Bne, "tad_stop_done");
    command(p, 2);
    label(p, "tad_stop_done");
    abs(p, M::Dec, BUSY);
    imp(p, M::Rts);
    label(p, "sound_settings");
    abs(p, M::Lda, crate::sfc_b1_menu::SETTINGS);
    byte(p, M::And, crate::sfc_b1_menu::BGM_OFF);
    br(p, M::Beq, "tad_settings_on");
    abs(p, M::Lda, MUSIC);
    byte(p, M::Cmp, 0xff);
    br(p, M::Beq, "tad_settings_stopping");
    abs(p, M::Lda, TIMER);
    abs(p, M::Ora, TIMER + 1);
    br(p, M::Beq, "tad_settings_keep_music");
    abs(p, M::Lda, RETURN);
    jump(p, "tad_settings_keep");
    label(p, "tad_settings_keep_music");
    abs(p, M::Lda, MUSIC);
    label(p, "tad_settings_keep");
    abs(p, M::Sta, WANTED);
    label(p, "tad_settings_stopping");
    jump(p, "sound_stop");
    label(p, "tad_settings_on");
    abs(p, M::Lda, WANTED);
    byte(p, M::Cmp, 28);
    br(p, M::Bcs, "tad_settings_done");
    abs(p, M::Sta, PENDING);
    write(p, WANTED, 0xff);
    label(p, "tad_settings_done");
    imp(p, M::Rts);
    label(p, "sound_field_song");
    abs(p, M::Lda, PENDING);
    byte(p, M::Cmp, 0xff);
    br(p, M::Bne, "tad_field_done");
    abs(p, M::Lda, MUSIC);
    byte(p, M::Cmp, 0xff);
    br(p, M::Bne, "tad_field_done");
    abs(p, M::Lda, WANTED);
    label(p, "tad_field_done");
    imp(p, M::Rts);
    Ok(())
}
