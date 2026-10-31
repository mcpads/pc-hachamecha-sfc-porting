use super::*;
pub(super) fn emit(p: &mut w65c816::Assembler, tables: &Tables) -> Result<()> {
    use crate::sfc::emit;
    let Tables {
        program,
        songs,
        effects,
        ..
    } = *tables;
    p.label("field_op_24");
    m16(p);
    long_x(p, M::Lda, program + 3);
    emit(p, M::And, A::Immediate, O::Word(255));
    implied(p, M::Tax);
    m8(p);
    long_x(p, M::Lda, effects);
    branch(p, M::Beq, "field_sound_none");
    call(p, "sound_effect");
    p.label("field_sound_none");
    jump(p, "field_next4");
    p.label("field_op_25");
    long_x(p, M::Lda, program + 1);
    branch(p, M::Bne, "field_effect_stop");
    call(p, "sound_stop");
    jump(p, "field_next4");
    p.label("field_effect_stop");
    call(p, "sound_effect_stop");
    jump(p, "field_next4");
    {
        use crate::sfc_b1_map::{PATH, ROUTE};
        p.label("field_op_96");
        long_x(p, M::Lda, program + 1);
        branch(p, M::Bne, "field_route_check");
        for y in 0..8 {
            abs(p, M::Stz, PATH + y);
        }
        jump(p, "field_next4");
        p.label("field_route_check");
        for (y, mask) in ROUTE.iter().enumerate() {
            if *mask == 0 {
                continue;
            }
            abs(p, M::Lda, PATH + y as u16);
            emit(p, M::And, A::Immediate, O::Byte(*mask));
            emit(p, M::Cmp, A::Immediate, O::Byte(*mask));
            branch(p, M::Bne, "field_route_missing");
        }
        jump(p, "field_next8");
        p.label("field_route_missing");
        jump(p, "field_goto4");
    }
    {
        use crate::sfc_b1_map::SCRATCH;
        p.label("field_op_85");
        long_x(p, M::Lda, program + 2);
        abs(p, M::Sta, SCRATCH);
        long_x(p, M::Lda, program + 3);
        abs(p, M::Sta, SCRATCH + 1);
        call(p, "b1_map_mark_cell");
        jump(p, "field_next4");
        p.label("field_op_97");
        long_x(p, M::Lda, program + 1);
        abs(p, M::Sta, SCRATCH);
        long_x(p, M::Lda, program + 2);
        abs(p, M::Sta, SCRATCH + 2);
        call(p, "b1_map_marker_set");
        jump(p, "field_next4");
    }
    p.label("field_op_86");
    abs(p, M::Lda, 0x50c);
    abs(p, M::Sta, crate::sfc_b1_map::VIEWED);
    jump(p, "field_next4");
    p.label("field_op_44");
    implied(p, M::Phx);
    implied(p, M::Phy);
    call(p, "b1_map_event_open");
    implied(p, M::Ply);
    implied(p, M::Plx);
    jump(p, "field_next4");
    p.label("field_op_9b");
    jump(p, "field_next4");
    p.label("field_op_27");
    long_x(p, M::Lda, program + 1);
    emit(p, M::Cmp, A::Immediate, O::Byte(2));
    branch(p, M::Bcs, "field_pcm_wait");
    jump(p, "field_next4");
    p.label("field_pcm_wait");
    m16(p);
    abs(p, M::Lda, WAIT);
    m8(p);
    branch(p, M::Bne, "field_pcm_waiting");
    write(p, WAIT, 1);
    implied(p, M::Rts);
    p.label("field_pcm_waiting");
    {
        use crate::sfc_sound::{JOB, JOB_STREAM, STREAM_PENDING, STREAM_TIMER};
        abs(p, M::Lda, STREAM_PENDING);
        emit(p, M::Cmp, A::Immediate, O::Byte(0xff));
        branch(p, M::Bne, "field_pcm_playing");
        abs(p, M::Lda, JOB);
        emit(p, M::Cmp, A::Immediate, O::Byte(JOB_STREAM));
        branch(p, M::Beq, "field_pcm_playing");
        abs(p, M::Lda, STREAM_TIMER);
        abs(p, M::Ora, STREAM_TIMER + 1);
        branch(p, M::Bne, "field_pcm_playing");
    }
    abs(p, M::Stz, WAIT);
    abs(p, M::Stz, WAIT + 1);
    jump(p, "field_next4");
    p.label("field_pcm_playing");
    implied(p, M::Rts);
    p.label("field_op_26");
    m16(p);
    long_x(p, M::Lda, program + 3);
    emit(p, M::And, A::Immediate, O::Word(255));
    implied(p, M::Tax);
    m8(p);
    long_x(p, M::Lda, songs);
    branch(p, M::Beq, "field_song_none");
    emit(p, M::Dec, A::Accumulator, O::None);
    call(p, "sound_play");
    p.label("field_song_none");
    jump(p, "field_next8");
    for op in [0x34u8, 0x3b, 0x3c, 0x8b, 0x35] {
        p.label(format!("field_op_{op:02x}"));
        jump(p, "field_next4");
    }
    p.label("field_op_3f");
    jump(p, "field_next8");
    Ok(())
}
