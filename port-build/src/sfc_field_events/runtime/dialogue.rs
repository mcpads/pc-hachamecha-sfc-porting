use super::*;
pub(super) fn emit(p: &mut w65c816::Assembler, tables: &Tables, page_delays: u32) -> Result<()> {
    use crate::sfc::emit;
    let Tables {
        layout, pressed, ..
    } = *tables;
    let section = |name: &str| -> Result<u32> { layout.cpu_address(layout.section(name)?.offset) };
    p.label("b1_dialogue_tick")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, 0x518);
    branch(p, M::Beq, "b1_dialogue_ready");
    emit(
        p,
        M::Cmp,
        A::Immediate,
        O::Byte(crate::b1_dialogue::EFFECT_WAIT),
    );
    branch(p, M::Bne, "b1_page_counted");
    {
        use crate::sfc_sound::{JOB, JOB_STREAM, STREAM_PENDING, STREAM_TIMER};
        abs(p, M::Lda, STREAM_PENDING);
        emit(p, M::Cmp, A::Immediate, O::Byte(0xff));
        branch(p, M::Bne, "b1_page_sounding");
        abs(p, M::Lda, JOB);
        emit(p, M::Cmp, A::Immediate, O::Byte(JOB_STREAM));
        branch(p, M::Beq, "b1_page_sounding");
        abs(p, M::Lda, STREAM_TIMER);
        abs(p, M::Ora, STREAM_TIMER + 1);
        branch(p, M::Beq, "b1_page_timed_out");
        p.label("b1_page_sounding");
        implied(p, M::Rts);
    }
    p.label("b1_page_counted");
    emit(
        p,
        M::Cmp,
        A::Immediate,
        O::Byte(crate::b1_dialogue::TIMED | 1),
    );
    branch(p, M::Beq, "b1_page_timed_out");
    abs(p, M::Dec, 0x518);
    implied(p, M::Rts);
    p.label("b1_page_timed_out");
    abs(p, M::Stz, 0x518);
    jump(p, "b1_page_advance");
    p.label("b1_dialogue_ready");
    m16(p);
    abs(p, M::Lda, 0x501);
    emit(p, M::Inc, A::Accumulator, O::None);
    abs(p, M::Cmp, 0x50d);
    m8(p);
    branch(p, M::Bne, "b1_regular_page");
    abs(p, M::Lda, 0x511);
    branch(p, M::Beq, "b1_regular_page");
    jump(p, "b1_choose");
    p.label("b1_regular_page");
    use crate::sfc_b1::{JINGLE_HOLD, JINGLE_WAIT};
    use crate::sfc_sound::{PENDING as SONG_PENDING, TIMER as JINGLE_TIMER};
    abs(p, M::Lda, JINGLE_HOLD);
    branch(p, M::Bne, "b1_jingle_held");
    abs(p, M::Lda, pressed);
    emit(p, M::And, A::Immediate, O::Byte(0x80));
    branch(p, M::Beq, "b1_dialogue_return");
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    abs(p, M::Lda, 0x501);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Tax);
    m8(p);
    let audio = section("b1_page_audio")?;
    long_x(p, M::Lda, audio);
    emit(p, M::And, A::Immediate, O::Byte(JINGLE_WAIT));
    branch(p, M::Beq, "b1_page_advance");
    write(p, JINGLE_HOLD, 1);
    p.label("b1_jingle_held");
    abs(p, M::Lda, SONG_PENDING);
    emit(p, M::Cmp, A::Immediate, O::Byte(0xff));
    branch(p, M::Bne, "b1_dialogue_return");
    abs(p, M::Lda, JINGLE_TIMER);
    abs(p, M::Ora, JINGLE_TIMER + 1);
    branch(p, M::Bne, "b1_dialogue_return");
    abs(p, M::Stz, JINGLE_HOLD);
    p.label("b1_page_advance");
    m16(p);
    abs(p, M::Inc, 0x501);
    abs(p, M::Lda, 0x501);
    abs(p, M::Cmp, 0x50d);
    m8(p);
    branch(p, M::Beq, "b1_dialogue_end");
    m16(p);
    abs(p, M::Lda, 0x501);
    implied(p, M::Tax);
    emit(p, M::Inc, A::Accumulator, O::None);
    abs(p, M::Cmp, 0x50d);
    m8(p);
    branch(p, M::Bne, "b1_page_next");
    long_x(p, M::Lda, page_delays);
    emit(
        p,
        M::Cmp,
        A::Immediate,
        O::Byte(crate::b1_dialogue::STANDING),
    );
    branch(p, M::Bne, "b1_page_next");
    call(p, "b1_page");
    abs(p, M::Stz, 0x518);
    crate::sfc::write_word(p, 0x501, 2);
    jump(p, "field_resume");
    p.label("b1_page_next");
    jump(p, "b1_page");
    p.label("b1_dialogue_end");
    crate::sfc::write_word(p, 0x501, 2);
    call(p, "b1_page");
    jump(p, "field_resume");
    p.label("b1_dialogue_return");
    implied(p, M::Rts);
    p.label("b1_choose").assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, pressed + 1);
    emit(p, M::And, A::Immediate, O::Byte(0x80));
    branch(p, M::Bne, "b1_choose_no");
    abs(p, M::Lda, pressed);
    emit(p, M::And, A::Immediate, O::Byte(0x80));
    branch(p, M::Bne, "b1_choose_yes");
    implied(p, M::Rts);
    p.label("b1_choose_no");
    if let Some(effect) = crate::sfc_sound::effect(layout, 1)? {
        emit(p, M::Lda, A::Immediate, O::Byte(effect));
        call(p, "sound_effect");
    }
    emit(p, M::Lda, A::Immediate, O::Byte(0));
    branch(p, M::Bra, "b1_choose_answer");
    p.label("b1_choose_yes");
    if let Some(effect) = crate::sfc_sound::effect(layout, 0)? {
        emit(p, M::Lda, A::Immediate, O::Byte(effect));
        call(p, "sound_effect");
    }
    emit(p, M::Lda, A::Immediate, O::Byte(1));
    p.label("b1_choose_answer");
    abs(p, M::Sta, ANSWER);
    abs(p, M::Stz, 0x511);
    crate::sfc::write_word(p, 0x501, 2);
    call(p, "b1_page");
    p.label("field_resume");
    abs(p, M::Stz, CLICK);
    call(p, "field_run");
    call(p, "field_input");
    m16(p);
    abs(p, M::Lda, PC);
    emit(p, M::Cmp, A::Immediate, O::Word(0xffff));
    m8(p);
    branch(p, M::Bne, "field_resume_done");
    abs(p, M::Lda, crate::sfc_encounter::REQUEST);
    branch(p, M::Beq, "field_resume_done");
    jump(p, "encounter_select");
    p.label("field_resume_done");
    implied(p, M::Rts);
    p.label("field_input").assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    abs(p, M::Lda, PC);
    emit(p, M::Cmp, A::Immediate, O::Word(0xffff));
    m8(p);
    branch(p, M::Beq, "field_input_free");
    implied(p, M::Rts);
    p.label("field_input_free");
    abs(p, M::Stz, CHOSEN);
    abs(p, M::Lda, FOLLOW_UP);
    implied(p, M::Sec);
    emit(p, M::Sbc, A::Immediate, O::Byte(32));
    emit(p, M::Cmp, A::Immediate, O::Byte(8));
    branch(p, M::Bcc, "field_input_follow");
    abs(p, M::Lda, FOLLOW_UP);
    emit(p, M::Cmp, A::Immediate, O::Byte(52));
    branch(p, M::Beq, "field_input_follow");
    implied(p, M::Sec);
    emit(p, M::Sbc, A::Immediate, O::Byte(54));
    emit(p, M::Cmp, A::Immediate, O::Byte(34));
    branch(p, M::Bcc, "field_input_follow");
    abs(p, M::Stz, FOLLOW_UP);
    implied(p, M::Rts);
    p.label("field_input_follow");
    abs(p, M::Lda, FOLLOW_UP);
    implied(p, M::Sec);
    emit(p, M::Sbc, A::Immediate, O::Byte(31));
    abs(p, M::Sta, FOLLOW_UP);
    abs(p, M::Stz, DEPTH);
    crate::sfc::write_word(p, PC, FOLLOW_UP_PC);
    jump(p, "field_run_steps");
    Ok(())
}
