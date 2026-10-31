use super::*;
pub fn emit_rules(p: &mut Assembler, layout: &CartridgeLayout) -> Result<()> {
    let base = layout.cpu_address(layout.section("battle_program")?.offset)?;
    let entry = layout.cpu_address(layout.section("battle_entry")?.offset)?;
    let thresholds = layout.cpu_address(layout.section("battle_thresholds")?.offset)?;
    let growth = layout.cpu_address(layout.section("battle_growth")?.offset)?;
    let songs = layout.cpu_address(layout.section("battle_songs")?.offset)?;
    let ending_scene = layout.cpu_address(layout.section("battle_ending_scene")?.offset)?;
    p.label("battle_start").assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Stz, crate::sfc_battle_message::SOUND_STOP);
    m16(p);
    emit(p, M::Lda, A::AbsoluteLong, O::Long(entry));
    abs(p, M::Sta, PC);
    abs(p, M::Stz, DEPTH);
    abs(p, M::Stz, WAIT);
    word(p, M::Lda, 0xffff);
    abs(p, M::Sta, LATCH);
    abs(p, M::Stz, COMMAND);
    m8(p);
    emit(p, M::Ldx, A::Immediate, O::Word(0));
    p.label("battle_start_clear");
    emit(p, M::Stz, A::AbsoluteX, O::Word(MESSAGE));
    implied(p, M::Inx);
    emit(
        p,
        M::Cpx,
        A::Immediate,
        O::Word(crate::sfc_battle_message::BODY_REQUEST - MESSAGE + 1),
    );
    branch(p, M::Bne, "battle_start_clear");
    abs(p, M::Stz, EXIT);
    abs(p, M::Stz, EFFECT_REQUEST);
    call(p, "bvm_reset");
    byte(p, M::Lda, DEFAULT_BUDGET);
    abs(p, M::Sta, BUDGET);
    implied(p, M::Rts);
    p.label("battle_growth_start")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    call(p, "battle_start");
    m16(p);
    emit(p, M::Lda, A::AbsoluteLong, O::Long(entry + 2));
    abs(p, M::Sta, PC);
    word(
        p,
        M::Lda,
        0xff00 | u16::from(crate::sfc_battle_controller::EXIT_FIELD),
    );
    abs(p, M::Sta, STACK);
    word(p, M::Lda, 2);
    abs(p, M::Sta, DEPTH);
    m8(p);
    implied(p, M::Rts);
    p.label("battle_tick").assume_mode(CpuMode::NATIVE_M8_X16);
    call(p, "battle_controller_tick");
    jump(p, "bvm_update");
    p.label("battle_controller_tick")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, MESSAGE);
    abs(p, M::Ora, AUDIO_WAIT);
    abs(p, M::Ora, EXIT);
    abs(p, M::Ora, SCENE);
    branch(p, M::Beq, "battle_tick_run");
    implied(p, M::Rts);
    p.label("battle_tick_run");
    abs(p, M::Lda, BUDGET);
    abs(p, M::Sta, STEPS);
    p.label("battle_vm_step");
    abs(p, M::Lda, STEPS);
    branch(p, M::Bne, "battle_vm_budget");
    implied(p, M::Rts);
    p.label("battle_vm_budget");
    abs(p, M::Dec, STEPS);
    m16(p);
    abs(p, M::Lda, PC);
    word(p, M::Cmp, 0xff00);
    branch(p, M::Bcc, "battle_vm_fetch");
    m8(p);
    abs(p, M::Sta, EXIT);
    emit(
        p,
        M::Cmp,
        A::Immediate,
        O::Byte(crate::sfc_battle_controller::EXIT_FINAL),
    );
    branch(p, M::Bne, "battle_vm_exit_done");
    abs(p, M::Stz, crate::sfc_encounter::REQUEST);
    for i in 0..4u16 {
        emit(
            p,
            M::Lda,
            A::AbsoluteLong,
            O::Long(ending_scene + u32::from(i)),
        );
        abs(p, M::Sta, PENDING_SCENE + i);
    }
    p.label("battle_vm_exit_done");
    implied(p, M::Rts);
    p.label("battle_vm_fetch")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    implied(p, M::Tax);
    m8(p);
    program(p, M::Lda, base, 0);
    let handlers: &[(u8, &str)] = &[
        (0x01, "vm_yield4"),
        (0x03, "vm_branch"),
        (0x06, "vm_loop"),
        (0x38, "vm_fade"),
        (0x04, "vm_call"),
        (0x05, "vm_return"),
        (0x0a, "vm_dispatch"),
        (0x13, "vm_branch"),
        (0x26, "vm_music"),
        (0x24, "vm_effect"),
        (0x27, "vm_effect_wait"),
        (0x2f, "vm_source_message"),
        (0x07, "vm_child"),
        (0x5e, "vm_spawn"),
        (0x28, "vm_slot"),
        (0x29, "vm_unslot"),
        (0x8b, "vm_sweep"),
        (0x3f, "vm_next8"),
        (0x89, "vm_next8"),
        (0x3a, "vm_dim"),
        (0x49, "vm_menu_mode"),
        (0x7c, "vm_menu"),
        (0x7b, "vm_menu"),
        (0x02, "vm_wait_ticks"),
        (0x4b, "vm_field_bind"),
        (0x4f, "vm_field_steps"),
        (0x51, "vm_setup"),
        (0x52, "vm_order_index"),
        (0x53, "vm_order"),
        (0x54, "vm_script"),
        (0x55, "vm_choose"),
        (0x56, "vm_act"),
        (0x57, "vm_request"),
        (0x58, "vm_wait"),
        (0x59, "vm_terminal"),
        (0x5a, "vm_after"),
        (0x5c, "vm_skip"),
        (0x5f, "vm_message"),
        (0x60, "vm_escape"),
        (0x61, "vm_alive"),
        (0x62, "vm_defeat"),
        (0x63, "vm_keep"),
        (0x64, "vm_experience"),
        (0x65, "vm_level"),
        (0x66, "vm_gems"),
        (0x4e, "vm_gems_clear"),
        (0x72, "vm_restore"),
        (0x92, "vm_status"),
        (0xa2, "vm_enemy_is"),
        (0xa3, "vm_clear_arle"),
        (0xa5, "vm_final"),
        (0x32, "vm_flag"),
        (0x83, "vm_battle_request"),
        (0xaa, "vm_reward"),
    ];
    for (i, (op, handler)) in handlers.iter().enumerate() {
        let skip = format!("battle_vm_not_{i}");
        emit(p, M::Cmp, A::Immediate, O::Byte(*op));
        branch(p, M::Bne, &skip);
        jump(p, handler);
        p.label(skip);
    }
    jump(p, "vm_next4");
    p.label("vm_next4").assume_mode(CpuMode::NATIVE_M8_X16);
    next(p, 4, false);
    let roots = layout.cpu_address(layout.section("battle_body_roots")?.offset)?;
    p.label("vm_spawn").assume_mode(CpuMode::NATIVE_M8_X16);
    program(p, M::Lda, base, 1);
    emit(p, M::Cmp, A::Immediate, O::Byte(0xff));
    branch(p, M::Bne, "vm_spawn_id");
    abs(p, M::Lda, crate::sfc_encounter::ENEMY);
    p.label("vm_spawn_id");
    m16(p);
    word(p, M::And, 0xff);
    branch(p, M::Beq, "vm_spawn_none");
    emit(p, M::Asl, A::Accumulator, O::None);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Phx);
    implied(p, M::Tax);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(roots));
    abs(p, M::Sta, crate::sfc_battle_objects::SPAWN_PC);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(roots + 2));
    abs(p, M::Sta, crate::sfc_battle_objects::SPAWN_PC + 2);
    implied(p, M::Plx);
    m8(p);
    call(p, "bvm_spawn");
    jump(p, "vm_next4");
    p.label("vm_spawn_none")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    m8(p);
    jump(p, "vm_next4");
    p.label("vm_child").assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    abs(p, M::Stz, crate::sfc_battle_objects::LAST_BODY);
    abs(p, M::Stz, crate::sfc_battle_objects::LAST_BODY + 2);
    m8(p);
    jump(p, "vm_next8");
    p.label("vm_slot").assume_mode(CpuMode::NATIVE_M8_X16);
    program(p, M::Lda, base, 1);
    branch(p, M::Bne, "vm_slot_done");
    program(p, M::Lda, base, 2);
    call(p, "bvm_register");
    p.label("vm_slot_done");
    jump(p, "vm_next4");
    p.label("vm_unslot").assume_mode(CpuMode::NATIVE_M8_X16);
    program(p, M::Lda, base, 1);
    call(p, "bvm_unslot");
    jump(p, "vm_next4");
    p.label("vm_sweep").assume_mode(CpuMode::NATIVE_M8_X16);
    program(p, M::Lda, base, 2);
    m16(p);
    word(p, M::And, 0xff);
    implied(p, M::Tay);
    m8(p);
    program(p, M::Lda, base, 1);
    call(p, "bvm_sweep");
    jump(p, "vm_next4");
    p.label("vm_next8").assume_mode(CpuMode::NATIVE_M8_X16);
    next(p, 8, false);
    p.label("vm_yield4").assume_mode(CpuMode::NATIVE_M8_X16);
    next(p, 4, true);
    p.label("vm_yield8").assume_mode(CpuMode::NATIVE_M8_X16);
    next(p, 8, true);
    p.label("vm_branch").assume_mode(CpuMode::NATIVE_M8_X16);
    emit(p, M::Ldx, A::Absolute, O::Word(PC));
    m16(p);
    program(p, M::Lda, base, 4);
    abs(p, M::Sta, PC);
    m8(p);
    jump(p, "battle_vm_step");
    p.label("vm_branch_yield")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    emit(p, M::Ldx, A::Absolute, O::Word(PC));
    m16(p);
    program(p, M::Lda, base, 4);
    abs(p, M::Sta, PC);
    m8(p);
    implied(p, M::Rts);
    p.label("vm_call").assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    emit(p, M::Ldx, A::Absolute, O::Word(DEPTH));
    abs(p, M::Lda, PC);
    implied(p, M::Clc);
    word(p, M::Adc, 8);
    emit(p, M::Sta, A::AbsoluteX, O::Word(STACK));
    implied(p, M::Inx);
    implied(p, M::Inx);
    emit(p, M::Stx, A::Absolute, O::Word(DEPTH));
    m8(p);
    jump(p, "vm_branch");
    p.label("vm_return").assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    emit(p, M::Ldx, A::Absolute, O::Word(DEPTH));
    implied(p, M::Dex);
    implied(p, M::Dex);
    emit(p, M::Stx, A::Absolute, O::Word(DEPTH));
    emit(p, M::Lda, A::AbsoluteX, O::Word(STACK));
    abs(p, M::Sta, PC);
    m8(p);
    jump(p, "battle_vm_step");
    p.label("vm_dispatch").assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, RESULT);
    program(p, M::Cmp, base, 2);
    branch(p, M::Bcc, "vm_dispatch_in");
    program(p, M::Lda, base, 2);
    emit(p, M::Dec, A::Accumulator, O::None);
    p.label("vm_dispatch_in");
    m16(p);
    word(p, M::And, 0xff);
    emit(p, M::Asl, A::Accumulator, O::None);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Clc);
    abs(p, M::Adc, PC);
    implied(p, M::Tax);
    program(p, M::Lda, base, 4);
    abs(p, M::Sta, PC);
    m8(p);
    jump(p, "battle_vm_step");
    p.label("vm_music").assume_mode(CpuMode::NATIVE_M8_X16);
    program(p, M::Lda, base, 1);
    emit(p, M::Cmp, A::Immediate, O::Byte(0x20));
    branch(p, M::Beq, "vm_audio_wait");
    program(p, M::Lda, base, 2);
    abs(p, M::Sta, MUSIC);
    program(p, M::Lda, base, 3);
    abs(p, M::Sta, MUSIC + 1);
    abs(p, M::Lda, MUSIC);
    branch(p, M::Bne, "vm_music_slot");
    byte(p, M::Lda, MUSIC_ALONE);
    abs(p, M::Sta, MUSIC);
    p.label("vm_music_slot");
    abs(p, M::Lda, MUSIC);
    branch(p, M::Bpl, "vm_music_request");
    m16(p);
    abs(p, M::Lda, crate::sfc_encounter::ENEMY);
    word(p, M::And, 0xff);
    implied(p, M::Tax);
    m8(p);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(songs));
    abs(p, M::Sta, MUSIC + 1);
    p.label("vm_music_request");
    byte(p, M::Lda, 1);
    abs(p, M::Sta, MUSIC_PENDING);
    jump(p, "vm_next8");
    p.label("vm_audio_wait").assume_mode(CpuMode::NATIVE_M8_X16);
    byte(p, M::Lda, AUDIO_WAIT_MUSIC);
    abs(p, M::Sta, AUDIO_WAIT);
    jump(p, "vm_yield4");
    let effects = layout.cpu_address(layout.section("sound_effect_map")?.offset)?;
    p.label("vm_effect").assume_mode(CpuMode::NATIVE_M8_X16);
    program(p, M::Lda, base, 1);
    emit(p, M::Cmp, A::Immediate, O::Byte(1));
    branch(p, M::Bne, "vm_effect_done");
    program(p, M::Lda, base, 2);
    branch(p, M::Bne, "vm_effect_done");
    program(p, M::Lda, base, 3);
    implied(p, M::Phx);
    m16(p);
    word(p, M::And, 0xff);
    implied(p, M::Tax);
    m8(p);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(effects));
    branch(p, M::Beq, "vm_effect_none");
    abs(p, M::Sta, LAST_EFFECT);
    abs(p, M::Sta, EFFECT_REQUEST);
    p.label("vm_effect_none");
    implied(p, M::Plx);
    p.label("vm_effect_done");
    jump(p, "vm_next4");
    p.label("vm_effect_wait")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    byte(p, M::Lda, AUDIO_WAIT_EFFECT);
    abs(p, M::Sta, AUDIO_WAIT);
    jump(p, "vm_yield4");
    p.label("vm_loop").assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    program(p, M::Lda, base, 4);
    abs(p, M::Cmp, LATCH);
    branch(p, M::Bne, "vm_loop_new");
    abs(p, M::Dec, GEMS);
    branch(p, M::Beq, "vm_loop_end");
    abs(p, M::Lda, LATCH);
    abs(p, M::Sta, PC);
    m8(p);
    jump(p, "battle_vm_step");
    p.label("vm_loop_end").assume_mode(CpuMode::NATIVE_M16_X16);
    word(p, M::Lda, 0xffff);
    abs(p, M::Sta, LATCH);
    m8(p);
    jump(p, "vm_next8");
    p.label("vm_loop_new").assume_mode(CpuMode::NATIVE_M16_X16);
    abs(p, M::Sta, LATCH);
    program(p, M::Lda, base, 2);
    abs(p, M::Sta, GEMS);
    m8(p);
    jump(p, "battle_vm_step");
    p.label("vm_wait_ticks").assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    abs(p, M::Lda, WAIT);
    branch(p, M::Bne, "vm_ticks_count");
    emit(p, M::Ldx, A::Absolute, O::Word(PC));
    program(p, M::Lda, base, 2);
    abs(p, M::Sta, WAIT);
    m8(p);
    implied(p, M::Rts);
    p.label("vm_ticks_count")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    abs(p, M::Dec, WAIT);
    m8(p);
    branch(p, M::Beq, "vm_ticks_done");
    implied(p, M::Rts);
    p.label("vm_ticks_done");
    jump(p, "vm_next4");
    p.label("vm_fade").assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, FADE);
    implied(p, M::Clc);
    program(p, M::Adc, base, 7);
    branch(p, M::Bcs, "vm_fade_black");
    emit(p, M::Cmp, A::Immediate, O::Byte(249));
    branch(p, M::Bcc, "vm_fade_store");
    p.label("vm_fade_black");
    byte(p, M::Lda, 248);
    p.label("vm_fade_store");
    abs(p, M::Sta, FADE);
    jump(p, "vm_next8");
    p.label("vm_source_message")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    for i in 0..4u16 {
        program(p, M::Lda, base, 4 + u32::from(i));
        abs(p, M::Sta, MESSAGE_SOURCE + i);
    }
    byte(p, M::Lda, KIND_SOURCE);
    abs(p, M::Sta, MESSAGE_KIND);
    byte(p, M::Lda, 1);
    abs(p, M::Sta, MESSAGE);
    jump(p, "vm_yield8");
    p.label("vm_dim").assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, WALL);
    implied(p, M::Sec);
    program(p, M::Sbc, base, 3);
    abs(p, M::Sta, WALL);
    byte(p, M::Lda, 1);
    abs(p, M::Sta, DIM);
    jump(p, "vm_next4");
    p.label("vm_menu_mode").assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    abs(p, M::Stz, COMMAND);
    m8(p);
    jump(p, "vm_next4");
    p.label("vm_menu").assume_mode(CpuMode::NATIVE_M8_X16);
    program(p, M::Lda, base, 1);
    abs(p, M::Sta, MENU);
    branch(p, M::Beq, "vm_menu_done");
    m16(p);
    abs(p, M::Stz, COMMAND);
    m8(p);
    p.label("vm_menu_done");
    jump(p, "vm_next4");
    p.label("vm_field_bind").assume_mode(CpuMode::NATIVE_M8_X16);
    jump(p, "vm_next4");
    p.label("vm_restore").assume_mode(CpuMode::NATIVE_M8_X16);
    byte(p, M::Lda, 1);
    abs(p, M::Sta, RESTORE);
    jump(p, "vm_next4");
    let bits = layout.cpu_address(layout.section("field_bits")?.offset)?;
    commands::rules(p, thresholds, growth, base, bits);
    Ok(())
}
