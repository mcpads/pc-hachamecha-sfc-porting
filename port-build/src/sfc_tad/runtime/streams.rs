use super::*;
use crate::sfc_tad::stream;
pub(super) fn emit(p: &mut Assembler, clips: u32, rates: u32, _chant_map: u32, _chants: u32) {
    label(p, "tad_clip_slot");
    m16(p);
    word(p, M::And, 255);
    for _ in 0..3 {
        op(p, M::Asl, A::Accumulator, O::None);
    }
    imp(p, M::Tax);
    m8(p);
    word(p, M::Ldy, 0);
    longx(p, M::Lda, clips + 3);
    byte(p, M::And, 2);
    p.emit_label_ref(M::Beq, A::Relative8, "tad_clip_slot_done");
    word(p, M::Ldy, SLOT_STRIDE);
    label(p, "tad_clip_slot_done");
    imp(p, M::Rts);
    label(p, "sound_effect");
    abs(p, M::Stz, CHANT_REQUEST);
    label(p, "sound_chant");
    abs(p, M::Inc, BUSY);
    imp(p, M::Pha);
    call(p, "tad_clip_slot");
    longx(p, M::Lda, clips + 3);
    byte(p, M::And, 4);
    p.emit_label_ref(M::Beq, A::Relative8, "tad_effect_allowed");
    abs(p, M::Lda, crate::sfc_b1_menu::SETTINGS);
    byte(p, M::And, crate::sfc_b1_menu::SAMPLING_OFF);
    br(p, M::Bne, "tad_effect_drop");
    label(p, "tad_effect_allowed");
    imp(p, M::Pla);
    imp(p, M::Pha);
    byte(p, M::Cmp, 16);
    br(p, M::Bcs, "tad_effect_wait_slot");
    imp(p, M::Phy);
    imp(p, M::Phx);
    imp(p, M::Tyx);
    call(p, "tad_clip_stop");
    imp(p, M::Plx);
    imp(p, M::Ply);
    jump(p, "tad_effect_queue");
    label(p, "tad_effect_wait_slot");
    op(p, M::Lda, A::AbsoluteY, O::Word(PENDING_CLIP));
    byte(p, M::Cmp, 0xff);
    br(p, M::Beq, "tad_effect_queue");
    longx(p, M::Lda, clips + 3);
    byte(p, M::And, 1);
    br(p, M::Bne, "tad_effect_drop");
    label(p, "tad_effect_queue");
    imp(p, M::Pla);
    op(p, M::Sta, A::AbsoluteY, O::Word(PENDING_CLIP));
    abs(p, M::Lda, CHANT_REQUEST);
    op(p, M::Sta, A::AbsoluteY, O::Word(PENDING_REPEATS));
    abs(p, M::Dec, BUSY);
    imp(p, M::Rts);
    label(p, "tad_effect_drop");
    imp(p, M::Pla);
    abs(p, M::Dec, BUSY);
    imp(p, M::Rts);
    label(p, "sound_effect_once");
    call(p, "sound_effect_sounding");
    p.emit_label_ref(M::Bcs, A::Relative8, "tad_once_done");
    call(p, "sound_effect");
    label(p, "tad_once_done");
    imp(p, M::Rts);
    label(p, "sound_effect_sounding");
    imp(p, M::Phx);
    imp(p, M::Phy);
    imp(p, M::Pha);
    call(p, "tad_clip_slot");
    imp(p, M::Pla);
    imp(p, M::Pha);
    op(p, M::Cmp, A::AbsoluteY, O::Word(PENDING_CLIP));
    br(p, M::Beq, "tad_sounding_yes");
    op(p, M::Cmp, A::AbsoluteY, O::Word(LOADED));
    br(p, M::Beq, "tad_sounding_yes");
    imp(p, M::Pla);
    imp(p, M::Ply);
    imp(p, M::Plx);
    imp(p, M::Clc);
    imp(p, M::Rts);
    label(p, "tad_sounding_yes");
    imp(p, M::Pla);
    imp(p, M::Ply);
    imp(p, M::Plx);
    imp(p, M::Sec);
    imp(p, M::Rts);
    label(p, "tad_clip_frame");
    ax(p, M::Lda, STARTED);
    br(p, M::Beq, "tad_clip_not_started");
    m16(p);
    abs(p, M::Lda, ELAPSED);
    word(p, M::And, 255);
    imp(p, M::Tay);
    l16(p, "tad_clip_time_step");
    ax(p, M::Lda, CLIP_TIMER);
    op(p, M::Dec, A::Accumulator, O::None);
    ax(p, M::Sta, CLIP_TIMER);
    br(p, M::Beq, "tad_clip_time_finished");
    ax(p, M::Lda, LEFT);
    br(p, M::Beq, "tad_clip_time_next");
    ax(p, M::Lda, AHEAD);
    imp(p, M::Sec);
    ax(p, M::Sbc, FRAME_BYTES);
    br(p, M::Bcc, "tad_clip_underrun");
    ax(p, M::Sta, AHEAD);
    l16(p, "tad_clip_time_next");
    imp(p, M::Dey);
    br(p, M::Bne, "tad_clip_time_step");
    m8(p);
    jump(p, "tad_clip_fill");
    l16(p, "tad_clip_underrun");
    m8(p);
    write(p, FAULT, 2);
    jump(p, "tad_clip_stop");
    l16(p, "tad_clip_time_finished");
    m8(p);
    jump(p, "tad_clip_stop");
    label(p, "tad_clip_not_started");
    ax(p, M::Lda, LOADED);
    byte(p, M::Cmp, 0xff);
    br(p, M::Bne, "tad_clip_fill");
    ax(p, M::Lda, PENDING_CLIP);
    byte(p, M::Cmp, 0xff);
    br(p, M::Beq, "tad_clip_return");
    ax(p, M::Sta, LOADED);
    call(p, "tad_prepare_clip");
    label(p, "tad_clip_fill");
    m16(p);
    ax(p, M::Lda, LEFT);
    br(p, M::Beq, "tad_clip_maybe_start_wide");
    ax(p, M::Lda, AHEAD);
    word(p, M::Cmp, stream::REFILL_TARGET);
    br(p, M::Bcs, "tad_clip_maybe_start_wide");
    m8(p);
    for i in 0..3 {
        ax(p, M::Lda, CLIP_PTR + i);
        op(
            p,
            M::Sta,
            A::DirectPage,
            O::Byte(TRANSFER_POINTER + i as u8),
        );
    }
    word(p, M::Ldy, 0);
    abs(p, M::Lda, ELAPSED);
    byte(p, M::Cmp, stream::MAX_SERVICE_FRAMES);
    p.emit_label_ref(M::Bcc, A::Relative8, "tad_stream_budget");
    byte(p, M::Lda, stream::MAX_SERVICE_FRAMES);
    label(p, "tad_stream_budget");
    op(p, M::Asl, A::Accumulator, O::None);
    abs(p, M::Sta, BUDGET);
    label(p, "tad_stream_burst");
    abs(p, M::Lda, TOKEN);
    call(p, "tad_wait_0");
    br(p, M::Bcs, "tad_clip_return");
    m16(p);
    ax(p, M::Lda, RING);
    imp(p, M::Clc);
    ax(p, M::Adc, POSITION);
    abs(p, M::Sta, 0x2141);
    ax(p, M::Lda, POSITION);
    word(
        p,
        M::Cmp,
        (stream::RING_BYTES - stream::TRANSFER_BYTES as usize) as u16,
    );
    m8(p);
    abs(p, M::Stz, BURST);
    p.emit_label_ref(M::Bne, A::Relative8, "tad_stream_not_ring_end");
    write(p, BURST, 3);
    label(p, "tad_stream_not_ring_end");
    abs(p, M::Stz, 0x2143);
    abs(p, M::Stz, SEQUENCE);
    abs(p, M::Lda, TOKEN);
    byte(p, M::Eor, 1);
    byte(p, M::And, 1);
    byte(p, M::Ora, 24);
    abs(p, M::Sta, TOKEN);
    abs(p, M::Sta, 0x2140);
    byte(p, M::Lda, 0x80);
    call(p, "tad_wait_0");
    br(p, M::Bcs, "tad_clip_return");
    imp(p, M::Phx);
    label(p, "tad_stream_triple");
    for port in [0x2141, 0x2142, 0x2143] {
        if port == 0x2141 {
            abs(p, M::Lda, SEQUENCE);
            byte(p, M::Cmp, 29);
            p.emit_label_ref(M::Bne, A::Relative8, "tad_stream_plain_header");
            abs(p, M::Lda, BURST);
            p.emit_label_ref(M::Beq, A::Relative8, "tad_stream_plain_header");
            op(
                p,
                M::Lda,
                A::DirectPageIndirectLongIndexedY,
                O::Byte(TRANSFER_POINTER),
            );
            imp(p, M::Pha);
            byte(p, M::And, 3);
            byte(p, M::Cmp, 1);
            p.emit_label_ref(M::Beq, A::Relative8, "tad_stream_terminal_header");
            imp(p, M::Pla);
            byte(p, M::Ora, 3);
            p.emit_label_ref(M::Bra, A::Relative8, "tad_stream_header_ready");
            label(p, "tad_stream_terminal_header");
            imp(p, M::Pla);
            p.emit_label_ref(M::Bra, A::Relative8, "tad_stream_header_ready");
            label(p, "tad_stream_plain_header");
        }
        op(
            p,
            M::Lda,
            A::DirectPageIndirectLongIndexedY,
            O::Byte(TRANSFER_POINTER),
        );
        if port == 0x2141 {
            label(p, "tad_stream_header_ready");
        }
        abs(p, M::Sta, port);
        imp(p, M::Iny);
    }
    abs(p, M::Inc, SEQUENCE);
    abs(p, M::Lda, SEQUENCE);
    abs(p, M::Sta, 0x2140);
    word(p, M::Ldx, 0xffff);
    label(p, "tad_stream_poll");
    abs(p, M::Cmp, 0x2140);
    p.emit_label_ref(M::Beq, A::Relative8, "tad_stream_acked");
    imp(p, M::Dex);
    p.emit_label_ref(M::Bne, A::Relative8, "tad_stream_poll");
    jump(p, "tad_wait_timeout");
    label(p, "tad_stream_acked");
    byte(p, M::Cmp, (stream::TRANSFER_BYTES / 3) as u8);
    p.emit_label_ref(M::Bne, A::Relative8, "tad_stream_triple");
    imp(p, M::Plx);
    abs(p, M::Stz, 0x2143);
    abs(p, M::Lda, TOKEN);
    abs(p, M::Sta, 0x2140);
    call(p, "tad_wait_0");
    br(p, M::Bcs, "tad_clip_return");
    m16(p);
    ax(p, M::Lda, POSITION);
    imp(p, M::Clc);
    word(p, M::Adc, stream::TRANSFER_BYTES);
    ax(p, M::Sta, POSITION);
    ax(p, M::Lda, AHEAD);
    imp(p, M::Clc);
    word(p, M::Adc, stream::TRANSFER_BYTES);
    ax(p, M::Sta, AHEAD);
    ax(p, M::Lda, LEFT);
    imp(p, M::Sec);
    word(p, M::Sbc, stream::TRANSFER_BYTES);
    ax(p, M::Sta, LEFT);
    ax(p, M::Lda, POSITION);
    word(p, M::Cmp, crate::sfc_tad::assets::RING_BYTES as u16);
    p.emit_label_ref(M::Bne, A::Relative8, "tad_stream_no_wrap");
    ax(p, M::Stz, POSITION);
    l16(p, "tad_stream_no_wrap");
    ax(p, M::Lda, LEFT);
    m8(p);
    p.emit_label_ref(M::Bne, A::Relative8, "tad_stream_has_segment");
    ax(p, M::Lda, REPEATS);
    br(p, M::Beq, "tad_stream_saved");
    ax(p, M::Dec, REPEATS);
    call(p, "tad_chant_segment");
    for i in 0..3 {
        ax(p, M::Lda, CLIP_PTR + i);
        op(
            p,
            M::Sta,
            A::DirectPage,
            O::Byte(TRANSFER_POINTER + i as u8),
        );
    }
    word(p, M::Ldy, 0);
    label(p, "tad_stream_has_segment");
    m16(p);
    ax(p, M::Lda, AHEAD);
    word(p, M::Cmp, stream::REFILL_TARGET);
    m8(p);
    p.emit_label_ref(M::Bcs, A::Relative8, "tad_stream_saved");
    abs(p, M::Dec, BUDGET);
    br(p, M::Bne, "tad_stream_burst");
    label(p, "tad_stream_saved");
    m16(p);
    imp(p, M::Tya);
    imp(p, M::Clc);
    ax(p, M::Adc, CLIP_PTR);
    ax(p, M::Sta, CLIP_PTR);
    m8(p);
    jump(p, "tad_clip_maybe_start");
    l16(p, "tad_clip_maybe_start_wide");
    m8(p);
    label(p, "tad_clip_maybe_start");
    ax(p, M::Lda, STARTED);
    p.emit_label_ref(M::Bne, A::Relative8, "tad_clip_return");
    m16(p);
    ax(p, M::Lda, AHEAD);
    word(p, M::Cmp, stream::START_BYTES);
    br(p, M::Bcs, "tad_clip_start_wide");
    ax(p, M::Lda, LEFT);
    m8(p);
    p.emit_label_ref(M::Bne, A::Relative8, "tad_clip_return");
    jump(p, "tad_clip_start");
    l16(p, "tad_clip_start_wide");
    m8(p);
    label(p, "tad_clip_start");
    call(p, "tad_slot_param");
    ax(p, M::Lda, PROGRAM);
    abs(p, M::Ora, PARAM);
    abs(p, M::Sta, PARAM);
    write(p, PARAM + 1, 64);
    command(p, 26);
    byte(p, M::Lda, 1);
    ax(p, M::Sta, STARTED);
    ax(p, M::Lda, LOADED);
    ax(p, M::Sta, CURRENT);
    label(p, "tad_clip_return");
    imp(p, M::Rts);
    label(p, "tad_prepare_clip");
    imp(p, M::Phx);
    m16(p);
    ax(p, M::Lda, LOADED);
    word(p, M::And, 255);
    for _ in 0..3 {
        op(p, M::Asl, A::Accumulator, O::None);
    }
    imp(p, M::Tax);
    m8(p);
    for i in 0..3 {
        longx(p, M::Lda, clips + i);
        abs(p, M::Sta, PTR + i as u16);
    }
    longx(p, M::Lda, clips + 3);
    for _ in 0..3 {
        op(p, M::Lsr, A::Accumulator, O::None);
    }
    op(p, M::Asl, A::Accumulator, O::None);
    abs(p, M::Sta, PARAM);
    m16(p);
    longx(p, M::Lda, clips + 4);
    abs(p, M::Sta, SIZE);
    longx(p, M::Lda, clips + 6);
    abs(p, M::Sta, JINGLE);
    imp(p, M::Plx);
    abs(p, M::Lda, SIZE);
    ax(p, M::Sta, LEFT);
    abs(p, M::Lda, JINGLE);
    ax(p, M::Sta, CLIP_TIMER);
    ax(p, M::Stz, AHEAD);
    ax(p, M::Stz, POSITION);
    imp(p, M::Phx);
    abs(p, M::Lda, PARAM);
    word(p, M::And, 255);
    imp(p, M::Tax);
    longx(p, M::Lda, rates);
    imp(p, M::Plx);
    ax(p, M::Sta, FRAME_BYTES);
    m8(p);
    abs(p, M::Lda, PARAM);
    ax(p, M::Sta, PROGRAM);
    for i in 0..3 {
        abs(p, M::Lda, PTR + i);
        ax(p, M::Sta, CLIP_PTR + i);
    }
    byte(p, M::Lda, 0xff);
    ax(p, M::Sta, PENDING_CLIP);
    call(p, "tad_prepare_chant");
    imp(p, M::Rts);
    label(p, "tad_slot_param");
    imp(p, M::Txa);
    for _ in 0..5 {
        op(p, M::Lsr, A::Accumulator, O::None);
    }
    abs(p, M::Sta, PARAM);
    imp(p, M::Rts);
    label(p, "tad_clip_stop");
    abs(p, M::Lda, JOB);
    br(p, M::Bne, "tad_clip_stop_state");
    call(p, "tad_slot_param");
    command(p, 28);
    label(p, "tad_clip_stop_state");
    for a in [STARTED, LEFT, LEFT + 1, CLIP_TIMER, CLIP_TIMER + 1, REPEATS] {
        ax(p, M::Stz, a);
    }
    byte(p, M::Lda, 0xff);
    ax(p, M::Sta, CURRENT);
    ax(p, M::Sta, LOADED);
    imp(p, M::Rts);
    label(p, "sound_effect_stop");
    abs(p, M::Inc, BUSY);
    for slot in SLOTS {
        word(p, M::Ldx, slot);
        call(p, "tad_clip_stop");
        byte(p, M::Lda, 0xff);
        ax(p, M::Sta, PENDING_CLIP);
    }
    abs(p, M::Dec, BUSY);
    imp(p, M::Rts);
    label(p, "sound_effect_stop_code");
    byte(p, M::Cmp, 0xff);
    br(p, M::Beq, "sound_effect_stop");
    abs(p, M::Inc, BUSY);
    imp(p, M::Pha);
    call(p, "tad_clip_slot");
    imp(p, M::Tyx);
    imp(p, M::Pla);
    imp(p, M::Pha);
    ax(p, M::Cmp, PENDING_CLIP);
    br(p, M::Bne, "tad_stop_code_loaded");
    byte(p, M::Lda, 0xff);
    ax(p, M::Sta, PENDING_CLIP);
    label(p, "tad_stop_code_loaded");
    imp(p, M::Pla);
    ax(p, M::Cmp, LOADED);
    br(p, M::Bne, "tad_stop_code_done");
    call(p, "tad_clip_stop");
    label(p, "tad_stop_code_done");
    abs(p, M::Dec, BUSY);
    imp(p, M::Rts);
}
