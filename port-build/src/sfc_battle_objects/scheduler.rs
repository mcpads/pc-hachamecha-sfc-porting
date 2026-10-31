use super::*;
pub(super) fn emit_scheduler(p: &mut Assembler, bank_add: u16) -> Result<()> {
    p.label("bvm_update").assume_mode(CpuMode::NATIVE_M8_X16);
    implied(p, M::Phd);
    implied(p, M::Phb);
    m16(p);
    imm16(p, M::Lda, DP);
    implied(p, M::Tcd);
    m8(p);
    imm8(p, M::Lda, 0x7e);
    implied(p, M::Pha);
    implied(p, M::Plb);
    imm8(p, M::Lda, 1);
    dp(p, M::Sta, (IN_UPDATE - DP) as u8);
    m16(p);
    imm16(p, M::Lda, 0xffff);
    dp(p, M::Sta, REFSRC + 2);
    m8(p);
    for (tick, anim) in [("normal", false), ("anim", true)] {
        m16(p);
        dp(p, M::Stz, GROUP);
        p.label(format!("bvm_{tick}_group"));
        dp(p, M::Lda, GROUP);
        for _ in 0..5 {
            acc(p, M::Asl);
        }
        implied(p, M::Clc);
        imm16(p, M::Adc, SENTINELS);
        implied(p, M::Tax);
        implied(p, M::Clc);
        imm16(p, M::Adc, 16);
        dp(p, M::Sta, TAIL);
        ax(p, M::Lda, 0x08);
        implied(p, M::Tax);
        p.label(format!("bvm_{tick}_object"));
        dp(p, M::Cpx, TAIL);
        br(p, M::Beq, &format!("bvm_{tick}_group_done"));
        dp(p, M::Stx, OBJ);
        if !anim {
            ax(p, M::Lda, 0x02);
            br(p, M::Bmi, &format!("bvm_{tick}_skip"));
            m8(p);
            jsr(p, "bvm_run");
            p.label("bvm_normal_return")
                .assume_mode(CpuMode::NATIVE_M8_X16);
        } else {
            ax(p, M::Lda, 0x02);
            imm16(p, M::And, 0x1000);
            br(p, M::Beq, &format!("bvm_{tick}_skip"));
            ax(p, M::Dec, 0x62);
            br(p, M::Bne, &format!("bvm_{tick}_skip"));
            for (a, b) in [(0x40u16, 0x64u16), (0x42, 0x66)] {
                ax(p, M::Lda, a);
                dp(p, M::Sta, SAVE + (a - 0x40) as u8);
                ax(p, M::Lda, b);
                ax(p, M::Sta, a);
            }
            m8(p);
            jsr(p, "bvm_run");
            dp(p, M::Ldx, OBJ);
            m16(p);
            for (a, b) in [(0x40u16, 0x64u16), (0x42, 0x66)] {
                ax(p, M::Lda, a);
                ax(p, M::Sta, b);
                dp(p, M::Lda, SAVE + (a - 0x40) as u8);
                ax(p, M::Sta, a);
            }
        }
        if !anim {
            dp(p, M::Ldx, OBJ);
            m16(p);
        }
        p.label(format!("bvm_{tick}_skip"))
            .assume_mode(CpuMode::NATIVE_M16_X16);
        ax(p, M::Lda, 0x08);
        implied(p, M::Tax);
        p.emit_label_ref(M::Brl, A::Relative16, format!("bvm_{tick}_object"));
        p.label(format!("bvm_{tick}_group_done"))
            .assume_mode(CpuMode::NATIVE_M16_X16);
        dp(p, M::Inc, GROUP);
        dp(p, M::Lda, GROUP);
        imm16(p, M::Cmp, GROUPS);
        br(p, M::Bcs, &format!("bvm_{tick}_done"));
        p.emit_label_ref(M::Brl, A::Relative16, format!("bvm_{tick}_group"));
        p.label(format!("bvm_{tick}_done"))
            .assume_mode(CpuMode::NATIVE_M16_X16);
        m8(p);
    }
    dp(p, M::Stz, (IN_UPDATE - DP) as u8);
    implied(p, M::Plb);
    implied(p, M::Pld);
    implied(p, M::Rts);
    p.label("bvm_run").assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    p.label("bvm_run_wide").assume_mode(CpuMode::NATIVE_M16_X16);
    imm16(p, M::Lda, COMMAND_LIMIT);
    dp(p, M::Sta, BUDGET);
    dp(p, M::Ldx, OBJ);
    ax(p, M::Lda, 0x40);
    dp(p, M::Sta, PCP);
    m8(p);
    ax(p, M::Lda, 0x42);
    implied(p, M::Clc);
    imm8(p, M::Adc, bank_add as u8);
    dp(p, M::Sta, PCP + 2);
    p.label("bvm_fetch");
    p.label("bvm_next");
    next(p);
    p.label("bvm_budget_out")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Inc, FAULTS + 1);
    p.label("bvm_run_done");
    implied(p, M::Rts);
    p.label("bvm_repoint_bank")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    ax(p, M::Lda, 0x42);
    implied(p, M::Clc);
    imm8(p, M::Adc, bank_add as u8);
    dp(p, M::Sta, PCP + 2);
    implied(p, M::Rts);
    Ok(())
}
pub(super) fn emit_pool(p: &mut Assembler, zero: u32) {
    p.label("bvm_reset").assume_mode(CpuMode::NATIVE_M8_X16);
    implied(p, M::Phb);
    imm8(p, M::Lda, 0x7e);
    implied(p, M::Pha);
    implied(p, M::Plb);
    m16(p);
    imm16(p, M::Ldx, POOL);
    p.label("bvm_reset_clear");
    ax(p, M::Stz, 0);
    implied(p, M::Inx);
    implied(p, M::Inx);
    imm16(p, M::Cpx, POOL + OBJECTS * OBJECT_BYTES);
    br(p, M::Bcc, "bvm_reset_clear");
    imm16(p, M::Ldx, POOL);
    p.label("bvm_reset_free");
    implied(p, M::Txa);
    implied(p, M::Clc);
    imm16(p, M::Adc, OBJECT_BYTES);
    ax(p, M::Sta, 0x0c);
    implied(p, M::Tax);
    imm16(p, M::Cpx, POOL + (OBJECTS - 1) * OBJECT_BYTES);
    br(p, M::Bcc, "bvm_reset_free");
    ax(p, M::Stz, 0x0c);
    imm16(p, M::Lda, POOL);
    abs(p, M::Sta, FREE);
    for g in 0..GROUPS {
        let head = SENTINELS + 32 * g;
        imm16(p, M::Lda, head + 16);
        abs(p, M::Sta, head + 8);
        imm16(p, M::Lda, head);
        abs(p, M::Sta, head + 16 + 4);
        abs(p, M::Stz, head + 4);
        abs(p, M::Stz, head + 16 + 8);
    }
    imm16(p, M::Ldx, 0);
    p.label("bvm_reset_slots");
    ax(p, M::Stz, SLOTS);
    implied(p, M::Inx);
    implied(p, M::Inx);
    imm16(p, M::Cpx, FAULTS + 8 - SLOTS);
    br(p, M::Bcc, "bvm_reset_slots");
    m8(p);
    implied(p, M::Plb);
    implied(p, M::Rts);
    p.label("bvm_alloc").assume_mode(CpuMode::NATIVE_M8_X16);
    dp(p, M::Sta, COUNT);
    dp(p, M::Stz, COUNT + 1);
    m16(p);
    abs(p, M::Ldx, FREE);
    br(p, M::Bne, "bvm_alloc_some");
    m8(p);
    abs(p, M::Inc, FAULTS);
    implied(p, M::Rts);
    p.label("bvm_alloc_some")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    ax(p, M::Lda, 0x0c);
    abs(p, M::Sta, FREE);
    ax(p, M::Stz, 0x0c);
    dp(p, M::Lda, COUNT);
    ax(p, M::Sta, 0x1a);
    for _ in 0..5 {
        acc(p, M::Asl);
    }
    implied(p, M::Clc);
    imm16(p, M::Adc, SENTINELS + 16);
    implied(p, M::Tay);
    ay(p, M::Lda, 4);
    ax(p, M::Sta, 4);
    implied(p, M::Tya);
    ax(p, M::Sta, 8);
    implied(p, M::Txa);
    ay(p, M::Sta, 4);
    ax(p, M::Ldy, 4);
    ay(p, M::Sta, 8);
    m8(p);
    implied(p, M::Rts);
    p.label("bvm_delete").assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    ax(p, M::Lda, 4);
    dp(p, M::Sta, TMP);
    ax(p, M::Lda, 8);
    dp(p, M::Sta, TMP + 2);
    dp(p, M::Ldy, TMP);
    ay(p, M::Sta, 8);
    dp(p, M::Ldy, TMP + 2);
    dp(p, M::Lda, TMP);
    ay(p, M::Sta, 4);
    let long = |p: &mut Assembler, m: M, at: u32| emit(p, m, A::AbsoluteLong, O::Long(at));
    implied(p, M::Txa);
    long(p, M::Sta, 0x2181);
    imm16(p, M::Lda, 0x8008);
    long(p, M::Sta, 0x4300);
    imm16(p, M::Lda, zero as u16);
    long(p, M::Sta, 0x4302);
    imm16(p, M::Lda, OBJECT_BYTES);
    long(p, M::Sta, 0x4305);
    m8(p);
    imm8(p, M::Lda, (zero >> 16) as u8);
    long(p, M::Sta, 0x4304);
    imm8(p, M::Lda, 0);
    long(p, M::Sta, 0x2183);
    imm8(p, M::Lda, 1);
    long(p, M::Sta, 0x420b);
    m16(p);
    dp(p, M::Lda, TMP);
    ax(p, M::Sta, 4);
    dp(p, M::Lda, TMP + 2);
    ax(p, M::Sta, 8);
    abs(p, M::Lda, FREE);
    ax(p, M::Sta, 0x0c);
    implied(p, M::Txa);
    abs(p, M::Sta, FREE);
    m8(p);
    implied(p, M::Rts);
    p.label("bvm_spawn_root")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    imm8(p, M::Lda, 0);
    br(p, M::Bra, "bvm_spawn_group");
    p.label("bvm_spawn").assume_mode(CpuMode::NATIVE_M8_X16);
    imm8(p, M::Lda, 1);
    p.label("bvm_spawn_group");
    abs(p, M::Sta, DP + u16::from(SPAWN_GROUP));
    implied(p, M::Phd);
    implied(p, M::Phb);
    m16(p);
    imm16(p, M::Lda, DP);
    implied(p, M::Tcd);
    m8(p);
    imm8(p, M::Lda, 0x7e);
    implied(p, M::Pha);
    implied(p, M::Plb);
    dp(p, M::Lda, SPAWN_GROUP);
    jsr(p, "bvm_alloc");
    m16(p);
    dp(p, M::Stz, WORK);
    dp(p, M::Stz, WORK + 2);
    implied(p, M::Txa);
    br(p, M::Beq, "bvm_spawn_none");
    dp(p, M::Lda, (SPAWN_PC - DP) as u8);
    ax(p, M::Sta, 0x40);
    dp(p, M::Lda, (SPAWN_PC - DP) as u8 + 2);
    ax(p, M::Sta, 0x42);
    implied(p, M::Txa);
    implied(p, M::Clc);
    imm16(p, M::Adc, SOURCE_DELTA as u16);
    dp(p, M::Sta, WORK);
    imm16(p, M::Lda, (SOURCE_DELTA >> 16) as u16);
    imm16(p, M::Adc, 0);
    dp(p, M::Sta, WORK + 2);
    p.label("bvm_spawn_none");
    m8(p);
    implied(p, M::Plb);
    implied(p, M::Pld);
    implied(p, M::Rts);
}
pub(super) fn emit_controller_side(p: &mut Assembler) {
    let enter = |p: &mut Assembler| {
        implied(p, M::Phd);
        implied(p, M::Phb);
        implied(p, M::Pha);
        m16(p);
        imm16(p, M::Lda, DP);
        implied(p, M::Tcd);
        m8(p);
        imm8(p, M::Lda, 0x7e);
        implied(p, M::Pha);
        implied(p, M::Plb);
        implied(p, M::Pla);
    };
    let leave = |p: &mut Assembler| {
        implied(p, M::Plb);
        implied(p, M::Pld);
        implied(p, M::Rts);
    };
    p.label("bvm_register").assume_mode(CpuMode::NATIVE_M8_X16);
    enter(p);
    m16(p);
    imm16(p, M::And, 0xff);
    acc(p, M::Asl);
    acc(p, M::Asl);
    implied(p, M::Tax);
    dp(p, M::Lda, WORK);
    ax(p, M::Sta, SLOTS);
    dp(p, M::Lda, WORK + 2);
    ax(p, M::Sta, SLOTS + 2);
    m8(p);
    leave(p);
    p.label("bvm_unslot").assume_mode(CpuMode::NATIVE_M8_X16);
    enter(p);
    m16(p);
    imm16(p, M::And, 0xff);
    acc(p, M::Asl);
    acc(p, M::Asl);
    implied(p, M::Tax);
    ax(p, M::Lda, SLOTS);
    dp(p, M::Sta, TMP);
    ax(p, M::Lda, SLOTS + 2);
    dp(p, M::Sta, TMP + 2);
    ax(p, M::Stz, SLOTS);
    ax(p, M::Stz, SLOTS + 2);
    dp(p, M::Lda, TMP);
    implied(p, M::Sec);
    imm16(p, M::Sbc, SOURCE_POOL as u16);
    implied(p, M::Tax);
    dp(p, M::Lda, TMP + 2);
    imm16(p, M::Sbc, (SOURCE_POOL >> 16) as u16);
    br(p, M::Bne, "bvm_unslot_none");
    implied(p, M::Txa);
    imm16(p, M::Cmp, OBJECTS * OBJECT_BYTES);
    br(p, M::Bcs, "bvm_unslot_none");
    implied(p, M::Clc);
    imm16(p, M::Adc, POOL);
    implied(p, M::Tax);
    m8(p);
    jsr(p, "bvm_delete");
    m16(p);
    p.label("bvm_unslot_none")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    m8(p);
    leave(p);
    p.label("bvm_sweep").assume_mode(CpuMode::NATIVE_M8_X16);
    enter(p);
    dp(p, M::Sta, COUNT);
    implied(p, M::Tya);
    dp(p, M::Sta, COUNT + 1);
    m16(p);
    dp(p, M::Lda, COUNT);
    imm16(p, M::And, 0xff);
    for _ in 0..5 {
        acc(p, M::Asl);
    }
    implied(p, M::Clc);
    imm16(p, M::Adc, SENTINELS);
    implied(p, M::Tax);
    implied(p, M::Clc);
    imm16(p, M::Adc, 16);
    dp(p, M::Sta, TAIL);
    ax(p, M::Lda, 8);
    implied(p, M::Tax);
    p.label("bvm_sweep_object");
    implied(p, M::Txa);
    dp(p, M::Cmp, TAIL);
    br(p, M::Beq, "bvm_sweep_done");
    m8(p);
    ax(p, M::Lda, 0x14);
    dp(p, M::And, COUNT + 1);
    br(p, M::Beq, "bvm_sweep_next");
    jsr(p, "bvm_delete");
    p.label("bvm_sweep_next");
    m16(p);
    ax(p, M::Lda, 8);
    implied(p, M::Tax);
    br(p, M::Bra, "bvm_sweep_object");
    p.label("bvm_sweep_done")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    m8(p);
    leave(p);
}
