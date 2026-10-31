use super::*;
pub(super) fn emit(p: &mut Assembler, clips: u32, map: u32, chants: u32) {
    label(p, "tad_prepare_chant");
    ax(p, M::Stz, REPEATS);
    imp(p, M::Phx);
    m16(p);
    ax(p, M::Lda, LOADED);
    word(p, M::And, 255);
    imp(p, M::Tax);
    m8(p);
    longx(p, M::Lda, map);
    imp(p, M::Plx);
    ax(p, M::Sta, CHANT_ROW);
    byte(p, M::Cmp, 0);
    br(p, M::Beq, "tad_chant_prepared");
    imp(p, M::Phx);
    m16(p);
    word(p, M::And, 255);
    for _ in 0..3 {
        op(p, M::Asl, A::Accumulator, O::None);
    }
    imp(p, M::Tax);
    m8(p);
    longx(p, M::Lda, chants + 6);
    abs(p, M::Sta, COUNTER);
    longx(p, M::Lda, chants + 3);
    abs(p, M::Sta, SIZE);
    abs(p, M::Stz, SIZE + 1);
    imp(p, M::Plx);
    ax(p, M::Lda, PENDING_REPEATS);
    abs(p, M::Cmp, COUNTER);
    p.emit_label_ref(M::Bcc, A::Relative8, "tad_chant_count");
    abs(p, M::Lda, COUNTER);
    label(p, "tad_chant_count");
    ax(p, M::Sta, REPEATS);
    byte(p, M::Cmp, 0);
    br(p, M::Beq, "tad_chant_prepared");
    m16(p);
    word(p, M::And, 255);
    imp(p, M::Tay);
    ax(p, M::Lda, CLIP_TIMER);
    l16(p, "tad_chant_duration");
    imp(p, M::Clc);
    abs(p, M::Adc, SIZE);
    imp(p, M::Dey);
    p.emit_label_ref(M::Bne, A::Relative8, "tad_chant_duration");
    ax(p, M::Sta, CLIP_TIMER);
    m8(p);
    call(p, "tad_chant_segment");
    label(p, "tad_chant_prepared");
    ax(p, M::Stz, PENDING_REPEATS);
    imp(p, M::Rts);
    label(p, "tad_chant_segment");
    imp(p, M::Phx);
    ax(p, M::Lda, REPEATS);
    br(p, M::Beq, "tad_chant_full_voice");
    m16(p);
    ax(p, M::Lda, CHANT_ROW);
    word(p, M::And, 255);
    for _ in 0..3 {
        op(p, M::Asl, A::Accumulator, O::None);
    }
    imp(p, M::Tax);
    m8(p);
    for i in 0..3 {
        longx(p, M::Lda, chants + i);
        abs(p, M::Sta, PTR + i as u16);
    }
    m16(p);
    longx(p, M::Lda, chants + 4);
    jump(p, "tad_chant_segment_store");
    label(p, "tad_chant_full_voice");
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
    m16(p);
    longx(p, M::Lda, clips + 4);
    l16(p, "tad_chant_segment_store");
    imp(p, M::Plx);
    ax(p, M::Sta, LEFT);
    m8(p);
    for i in 0..3 {
        abs(p, M::Lda, PTR + i);
        ax(p, M::Sta, CLIP_PTR + i);
    }
    imp(p, M::Rts);
}
