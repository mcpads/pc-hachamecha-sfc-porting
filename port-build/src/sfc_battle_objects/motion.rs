use super::*;
pub fn sine_digits(value: i64) -> Result<[u8; 4]> {
    let low = |v: i64| (v + 128).rem_euclid(256) - 128;
    let d0 = low(value);
    let rest = (value - d0) / 256;
    let d1 = low(rest);
    let d2 = (rest - d1) / 256;
    if !(-1..=1).contains(&d2) {
        return Err("Sine value out of range".into());
    }
    Ok([d0 as i8 as u8, d1 as i8 as u8, d2 as i8 as u8, 0])
}
pub(super) fn emit_trig(p: &mut Assembler, sine: u32) {
    p.label(format!("bvm_op_{INIT_LOOP_MOTION:02x}"))
        .assume_mode(CpuMode::NATIVE_M8_X16);
    dp(p, M::Ldx, OBJ);
    m16(p);
    dp(p, M::Lda, BUDGET);
    imm16(p, M::Cmp, 2);
    br(p, M::Bcc, "bvm_init_motion_draw");
    ax(p, M::Lda, 0x60);
    imm16(p, M::Cmp, 2);
    br(p, M::Bcc, "bvm_init_motion_draw");
    m8(p);
    advance_next(p, 4);
    p.label("bvm_init_motion_draw")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    m8(p);
    jmp(p, "bvm_op_2d");
    p.label("bvm_op_2d").assume_mode(CpuMode::NATIVE_M8_X16);
    dp(p, M::Ldx, OBJ);
    cbyte(p, 1);
    dp(p, M::Sta, AXIS);
    jsr(p, "bvm_2d_loaded");
    advance_next(p, 4);
    for (opcode, prefix, motion) in [
        (TRIG_LOOP, "bvm_trig_loop", "bvm_2d_loaded"),
        (phase_table::OPCODE, "bvm_phase_loop", "bvm_phase_y"),
    ] {
        p.label(format!("bvm_op_{opcode:02x}"))
            .assume_mode(CpuMode::NATIVE_M8_X16);
        dp(p, M::Ldx, OBJ);
        m16(p);
        cword(p, 4);
        ax(p, M::Cmp, 0x3c);
        p.emit_label_ref(M::Bne, A::Relative8, format!("{prefix}_new"));
        cword(p, 6);
        ax(p, M::Cmp, 0x3e);
        p.emit_label_ref(M::Bne, A::Relative8, format!("{prefix}_new"));
        ax(p, M::Dec, 0x60);
        p.emit_label_ref(M::Bne, A::Relative8, format!("{prefix}_turn"));
        jmp(p, "bvm_06_end");
        p.label(format!("{prefix}_new"))
            .assume_mode(CpuMode::NATIVE_M16_X16);
        jmp(p, "bvm_06_new");
        p.label(format!("{prefix}_turn"))
            .assume_mode(CpuMode::NATIVE_M16_X16);
        dp(p, M::Dec, BUDGET);
        p.emit_label_ref(M::Bne, A::Relative8, format!("{prefix}_move"));
        cword(p, 4);
        ax(p, M::Sta, 0x40);
        cword(p, 6);
        ax(p, M::Sta, 0x42);
        m8(p);
        jmp(p, "bvm_budget_out");
        p.label(format!("{prefix}_move"))
            .assume_mode(CpuMode::NATIVE_M16_X16);
        m8(p);
        cbyte(p, 1);
        dp(p, M::Sta, AXIS);
        jsr(p, motion);
        dp(p, M::Ldx, OBJ);
        m16(p);
        cword(p, 4);
        implied(p, M::Clc);
        imm16(p, M::Adc, 8);
        ax(p, M::Sta, 0x40);
        cword(p, 6);
        imm16(p, M::Adc, 0);
        ax(p, M::Sta, 0x42);
        m8(p);
        yield_now(p);
    }
    phase_table::emit_y(p, sine);
    phase_table::emit_step(p);
    radial_motion::emit_step(p);
    p.label("bvm_2d_axes").assume_mode(CpuMode::NATIVE_M8_X16);
    dp(p, M::Lda, AXIS);
    p.label("bvm_2d_loaded").assume_mode(CpuMode::NATIVE_M8_X16);
    imm8(p, M::Eor, 0x0b);
    br(p, M::Bne, "bvm_2d_try_y");
    for (xy, name) in [(true, "bvm_2d_xy"), (false, "bvm_2d_generic")] {
        if !xy {
            p.label("bvm_2d_try_y").assume_mode(CpuMode::NATIVE_M8_X16);
            imm8(p, M::Eor, 1);
            br(p, M::Bne, "bvm_2d_generic");
            m16(p);
            jmp(p, "bvm_2d_absolute_y");
        }
        p.label(name).assume_mode(CpuMode::NATIVE_M8_X16);
        for (bit, angle, amp, origin, pos, sine, add) in [
            (1u8, 0x8cu16, 0x92u16, 0x80u16, 0x1cu16, true, true),
            (2, 0x8e, 0x94, 0x84, 0x20, false, false),
            (4, 0x90, 0x96, 0x88, 0x24, false, true),
        ] {
            if xy && bit == 4 {
                dp(p, M::Lda, AXIS);
                imm8(p, M::And, bit);
                continue;
            }
            let skip = format!("{name}_{bit}_skip");
            if !xy {
                dp(p, M::Lda, AXIS);
                imm8(p, M::And, bit);
                br(p, M::Beq, &skip);
            }
            if xy && bit == 2 {
                p.label("bvm_2d_absolute_y")
                    .assume_mode(CpuMode::NATIVE_M16_X16);
            }
            if !xy || bit == 1 {
                m16(p);
            }
            ax(p, M::Lda, amp);
            m8(p);
            emit(p, M::Sta, A::AbsoluteLong, O::Long(0x211b));
            implied(p, M::Xba);
            emit(p, M::Sta, A::AbsoluteLong, O::Long(0x211b));
            m16(p);
            ax(p, M::Lda, angle);
            implied(p, M::Clc);
            imm16(p, M::Adc, if sine { 8 } else { 0x4008 });
            for _ in 0..2 {
                acc(p, M::Lsr);
            }
            imm16(p, M::And, 0xfffc);
            implied(p, M::Tax);
            m8(p);
            jsr(p, "bvm_product");
            dp(p, M::Ldx, OBJ);
            if !xy {
                dp(p, M::Lda, AXIS);
                imm8(p, M::And, 0xf8);
                imm8(p, M::Cmp, 8);
            }
            m16(p);
            let apply = |p: &mut Assembler, base: u16| {
                for word in 0..2u16 {
                    ax(p, M::Lda, base + word * 2);
                    dp(p, M::Sta, LHS + word as u8 * 2);
                    if word == 0 {
                        implied(p, if add { M::Clc } else { M::Sec });
                    }
                    dp(p, if add { M::Adc } else { M::Sbc }, RES + word as u8 * 2);
                    ax(p, M::Sta, pos + word * 2);
                }
            };
            if xy {
                apply(p, origin);
            } else {
                br(p, M::Beq, &format!("{name}_{bit}_abs"));
                apply(p, pos);
                br(p, M::Bra, &format!("{name}_{bit}_base"));
                p.label(format!("{name}_{bit}_abs"))
                    .assume_mode(CpuMode::NATIVE_M16_X16);
                apply(p, origin);
                p.label(format!("{name}_{bit}_base"));
            }
            if !xy || bit == 2 {
                m8(p);
            }
            p.label(skip);
        }
        implied(p, M::Rts);
    }
    let reg = |p: &mut Assembler, m: M, at: u32| emit(p, m, A::AbsoluteLong, O::Long(at));
    p.label("bvm_product").assume_mode(CpuMode::NATIVE_M8_X16);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(sine));
    reg(p, M::Sta, 0x211c);
    m16(p);
    reg(p, M::Lda, 0x2134);
    dp(p, M::Sta, RES);
    m8(p);
    reg(p, M::Lda, 0x2136);
    dp(p, M::Sta, RES + 2);
    br(p, M::Bpl, "bvm_product_sign_clear");
    imm8(p, M::Lda, 0xff);
    dp(p, M::Sta, RES + 3);
    br(p, M::Bra, "bvm_product_sign_done");
    p.label("bvm_product_sign_clear");
    dp(p, M::Stz, RES + 3);
    p.label("bvm_product_sign_done");
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(sine + 1));
    reg(p, M::Sta, 0x211c);
    m16(p);
    reg(p, M::Lda, 0x2134);
    implied(p, M::Clc);
    dp(p, M::Adc, RES + 1);
    dp(p, M::Sta, RES + 1);
    m8(p);
    reg(p, M::Lda, 0x2136);
    dp(p, M::Adc, RES + 3);
    dp(p, M::Sta, RES + 3);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(sine + 2));
    br(p, M::Beq, "bvm_product_done");
    reg(p, M::Sta, 0x211c);
    m16(p);
    reg(p, M::Lda, 0x2134);
    implied(p, M::Clc);
    dp(p, M::Adc, RES + 2);
    dp(p, M::Sta, RES + 2);
    m8(p);
    p.label("bvm_product_done")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    implied(p, M::Rts);
}
pub(super) fn emit_toward(p: &mut Assembler) {
    let neg32 = |p: &mut Assembler, at: u8| {
        m16(p);
        imm16(p, M::Lda, 0);
        implied(p, M::Sec);
        dp(p, M::Sbc, at);
        dp(p, M::Sta, at);
        imm16(p, M::Lda, 0);
        dp(p, M::Sbc, at + 2);
        dp(p, M::Sta, at + 2);
        m8(p);
    };
    p.label("bvm_op_80").assume_mode(CpuMode::NATIVE_M8_X16);
    dp(p, M::Ldx, OBJ);
    for (axis, (pos, inline, origin, amp, k, keep)) in [
        (0x1cu16, 4u16, 0x80u16, 0x92u16, 1u16, DX),
        (0x20, 8, 0x84, 0x94, 2, DY),
    ]
    .into_iter()
    .enumerate()
    {
        m16(p);
        cword(p, inline);
        implied(p, M::Clc);
        ax(p, M::Adc, pos);
        dp(p, M::Sta, TMP);
        cword(p, inline + 2);
        ax(p, M::Adc, pos + 2);
        acc(p, M::Lsr);
        ax(p, M::Sta, origin + 2);
        dp(p, M::Lda, TMP);
        acc(p, M::Ror);
        ax(p, M::Sta, origin);
        cword(p, inline);
        implied(p, M::Sec);
        ax(p, M::Sbc, pos);
        dp(p, M::Sta, keep);
        cword(p, inline + 2);
        ax(p, M::Sbc, pos + 2);
        dp(p, M::Sta, keep + 2);
        dp(p, M::Sta, LHS);
        dp(p, M::Stz, LHS + 2);
        m8(p);
        dp(p, M::Stz, SIGN);
        m16(p);
        dp(p, M::Lda, keep + 2);
        br(p, M::Bpl, &format!("bvm_80_{axis}_t"));
        dp(p, M::Inc, SIGN);
        m8(p);
        neg32(p, keep);
        m16(p);
        dp(p, M::Lda, keep + 2);
        dp(p, M::Sta, LHS);
        dp(p, M::Stz, LHS + 2);
        m8(p);
        neg32(p, LHS);
        neg32(p, keep);
        m16(p);
        p.label(format!("bvm_80_{axis}_t"));
        m8(p);
        cbyte(p, k);
        dp(p, M::Sta, SRC);
        dp(p, M::Stz, SRC + 1);
        m16(p);
        dp(p, M::Stz, SRC + 2);
        m8(p);
        jsr(p, "bvm_multiply");
        m16(p);
        dp(p, M::Lda, RES);
        implied(p, M::Clc);
        imm16(p, M::Adc, 1);
        dp(p, M::Sta, RES);
        dp(p, M::Lda, RES + 2);
        imm16(p, M::Adc, 0);
        dp(p, M::Sta, RES + 2);
        br(p, M::Bpl, &format!("bvm_80_{axis}_half_pos"));
        m8(p);
        neg32(p, RES);
        m16(p);
        dp(p, M::Lsr, RES + 2);
        dp(p, M::Ror, RES);
        m8(p);
        neg32(p, RES);
        m16(p);
        br(p, M::Bra, &format!("bvm_80_{axis}_half"));
        p.label(format!("bvm_80_{axis}_half_pos"))
            .assume_mode(CpuMode::NATIVE_M16_X16);
        dp(p, M::Lsr, RES + 2);
        dp(p, M::Ror, RES);
        p.label(format!("bvm_80_{axis}_half"));
        dp(p, M::Lda, RES);
        br(p, M::Bpl, &format!("bvm_80_{axis}_amp"));
        imm16(p, M::Eor, 0xffff);
        acc(p, M::Inc);
        p.label(format!("bvm_80_{axis}_amp"));
        ax(p, M::Sta, amp);
        m8(p);
    }
    for (keep, k, to) in [(DX, 1u16, QX), (DY, 2, QY)] {
        copy32(p, LHS, keep);
        cbyte(p, k);
        dp(p, M::Sta, SRC);
        dp(p, M::Stz, SRC + 1);
        m16(p);
        dp(p, M::Stz, SRC + 2);
        m8(p);
        jsr(p, "bvm_divide");
        p.emit_label_ref(M::Bcs, A::Relative8, "bvm_80_fault");
        copy32(p, to, QUO);
    }
    jsr(p, "bvm_atan2");
    p.emit_label_ref(M::Bcs, A::Relative8, "bvm_80_fault");
    dp(p, M::Ldx, OBJ);
    m16(p);
    dp(p, M::Lda, RES);
    implied(p, M::Clc);
    imm16(p, M::Adc, 0x8000);
    ax(p, M::Sta, 0x8c);
    ax(p, M::Sta, 0x8e);
    m8(p);
    advance_next(p, 12);
    p.label("bvm_80_fault").assume_mode(CpuMode::NATIVE_M8_X16);
    dp(p, M::Ldx, OBJ);
    yield_now(p);
    p.label("bvm_atan2").assume_mode(CpuMode::NATIVE_M8_X16);
    copy32(p, LHS, QX);
    copy32(p, SRC, QY);
    jsr(p, "bvm_atan_core");
    p.emit_label_ref(M::Bcc, A::Relative8, "bvm_atan2_ok");
    implied(p, M::Rts);
    p.label("bvm_atan2_ok");
    m16(p);
    dp(p, M::Lda, QX + 2);
    p.emit_label_ref(M::Bmi, A::Relative8, "bvm_atan2_xneg");
    dp(p, M::Lda, QY + 2);
    p.emit_label_ref(M::Bmi, A::Relative8, "bvm_atan2_done");
    imm16(p, M::Lda, 0x8000);
    implied(p, M::Sec);
    dp(p, M::Sbc, RES);
    dp(p, M::Sta, RES);
    p.emit_label_ref(M::Bra, A::Relative8, "bvm_atan2_done");
    p.label("bvm_atan2_xneg")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    dp(p, M::Lda, QY + 2);
    p.emit_label_ref(M::Bmi, A::Relative8, "bvm_atan2_negc");
    dp(p, M::Lda, RES);
    implied(p, M::Clc);
    imm16(p, M::Adc, 0x8000);
    dp(p, M::Sta, RES);
    p.emit_label_ref(M::Bra, A::Relative8, "bvm_atan2_done");
    p.label("bvm_atan2_negc")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    imm16(p, M::Lda, 0);
    implied(p, M::Sec);
    dp(p, M::Sbc, RES);
    dp(p, M::Sta, RES);
    p.label("bvm_atan2_done")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    m8(p);
    implied(p, M::Clc);
    implied(p, M::Rts);
    p.label("bvm_atan_core").assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    dp(p, M::Stz, RES);
    dp(p, M::Lda, LHS);
    dp(p, M::Ora, LHS + 2);
    p.emit_label_ref(M::Bne, A::Relative8, "bvm_atan_core_a");
    m8(p);
    implied(p, M::Clc);
    implied(p, M::Rts);
    p.label("bvm_atan_core_a")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    imm16(p, M::Lda, 0x4000);
    dp(p, M::Sta, RES);
    dp(p, M::Lda, SRC);
    dp(p, M::Ora, SRC + 2);
    p.emit_label_ref(M::Bne, A::Relative8, "bvm_atan_core_b");
    m8(p);
    implied(p, M::Clc);
    implied(p, M::Rts);
    p.label("bvm_atan_core_b")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    m8(p);
    for at in [LHS, SRC] {
        m16(p);
        dp(p, M::Lda, at + 2);
        let positive = format!("bvm_atan_abs_{at:02x}");
        p.emit_label_ref(M::Bpl, A::Relative8, &positive);
        m8(p);
        neg32(p, at);
        m16(p);
        p.label(positive);
        m8(p);
    }
    copy32(p, ATA, LHS);
    copy32(p, ATB, SRC);
    m16(p);
    dp(p, M::Lda, ATB);
    implied(p, M::Sec);
    dp(p, M::Sbc, ATA);
    dp(p, M::Lda, ATB + 2);
    dp(p, M::Sbc, ATA + 2);
    p.emit_label_ref(M::Bvc, A::Relative8, "bvm_atan_core_nv");
    imm16(p, M::Eor, 0x8000);
    p.label("bvm_atan_core_nv");
    m8(p);
    p.emit_label_ref(M::Bmi, A::Relative8, "bvm_atan_core_less");
    copy32(p, LHS, ATA);
    copy32(p, SRC, ATB);
    imm8(p, M::Lda, 0);
    dp(p, M::Sta, COUNT);
    p.emit_label_ref(M::Bra, A::Relative8, "bvm_atan_core_index");
    p.label("bvm_atan_core_less");
    copy32(p, LHS, ATB);
    copy32(p, SRC, ATA);
    imm8(p, M::Lda, 1);
    dp(p, M::Sta, COUNT);
    p.label("bvm_atan_core_index");
    m16(p);
    for _ in 0..2 {
        dp(p, M::Asl, LHS);
        dp(p, M::Rol, LHS + 2);
    }
    for _ in 0..8 {
        dp(p, M::Lda, SRC + 2);
        acc(p, M::Asl);
        dp(p, M::Ror, SRC + 2);
        dp(p, M::Ror, SRC);
    }
    m8(p);
    jsr(p, "bvm_idiv");
    p.emit_label_ref(M::Bcc, A::Relative8, "bvm_atan_core_table");
    implied(p, M::Rts);
    p.label("bvm_atan_core_table");
    m16(p);
    dp(p, M::Lda, QUO);
    acc(p, M::Asl);
    dp(p, M::Sta, TMP);
    dp(p, M::Lda, QUO + 2);
    acc(p, M::Rol);
    dp(p, M::Sta, TMP + 2);
    dp(p, M::Lda, TMP);
    implied(p, M::Clc);
    imm16(p, M::Adc, 0xbbd0);
    dp(p, M::Sta, TMP);
    dp(p, M::Lda, TMP + 2);
    imm16(p, M::Adc, 0x0045);
    dp(p, M::Sta, TMP + 2);
    m8(p);
    jsr(p, "bvm_rom");
    m16(p);
    imm16(p, M::Ldy, 0);
    ind(p, M::Lda, ROMP);
    dp(p, M::Sta, RES);
    m8(p);
    dp(p, M::Lda, COUNT);
    p.emit_label_ref(M::Beq, A::Relative8, "bvm_atan_core_done");
    m16(p);
    imm16(p, M::Lda, 0x4000);
    implied(p, M::Sec);
    dp(p, M::Sbc, RES);
    dp(p, M::Sta, RES);
    m8(p);
    p.label("bvm_atan_core_done");
    implied(p, M::Clc);
    implied(p, M::Rts);
    p.label("bvm_idiv").assume_mode(CpuMode::NATIVE_M8_X16);
    dp(p, M::Stz, SIGN);
    for at in [LHS, SRC] {
        m16(p);
        dp(p, M::Lda, at + 2);
        let positive = format!("bvm_idiv_abs_{at:02x}");
        p.emit_label_ref(M::Bpl, A::Relative8, &positive);
        m8(p);
        dp(p, M::Lda, SIGN);
        imm8(p, M::Eor, 1);
        dp(p, M::Sta, SIGN);
        neg32(p, at);
        m16(p);
        p.label(positive);
        m8(p);
    }
    jsr(p, "bvm_divide");
    p.emit_label_ref(M::Bcs, A::Relative8, "bvm_idiv_done");
    dp(p, M::Lda, SIGN);
    p.emit_label_ref(M::Beq, A::Relative8, "bvm_idiv_ok");
    neg32(p, QUO);
    p.label("bvm_idiv_ok");
    implied(p, M::Clc);
    p.label("bvm_idiv_done");
    implied(p, M::Rts);
}
pub(super) fn emit_variants(p: &mut Assembler) {
    let ind16 = |p: &mut Assembler, m: M, at: u8| {
        emit(p, m, A::DirectPageIndirectIndexedY, O::Byte(at));
    };
    let self_y = |p: &mut Assembler, k: u16| {
        cbyte(p, k);
        m16(p);
        imm16(p, M::And, 0xff);
        implied(p, M::Tay);
        m8(p);
    };
    p.label("bvm_op_af").assume_mode(CpuMode::NATIVE_M8_X16);
    dp(p, M::Ldx, OBJ);
    imm8(p, M::Lda, 4);
    dp(p, M::Sta, WIDTH);
    imm8(p, M::Lda, 0x83);
    dp(p, M::Sta, MODE);
    imm8(p, M::Lda, 1);
    dp(p, M::Sta, VALID);
    cbyte(p, 2);
    dp(p, M::Sta, OFF);
    m16(p);
    imm16(p, M::And, 0xff);
    implied(p, M::Tay);
    dp(p, M::Lda, OBJ);
    dp(p, M::Sta, PTR);
    for word in 0..2u8 {
        ind16(p, M::Lda, PTR);
        dp(p, M::Sta, TMP + word * 2);
        dp(p, M::Sta, LHS + word * 2);
        if word == 0 {
            implied(p, M::Iny);
            implied(p, M::Iny);
        }
    }
    cword(p, 4);
    dp(p, M::Sta, SRC);
    cword(p, 6);
    dp(p, M::Sta, SRC + 2);
    m8(p);
    implied(p, M::Sec);
    jsr(p, "bvm_multiply");
    m16(p);
    dp(p, M::Lda, OFF);
    imm16(p, M::And, 0xff);
    implied(p, M::Tay);
    m8(p);
    imm8(p, M::Lda, 4);
    jsr(p, "bvm_write");
    p.label("bvm_af_done");
    advance_next(p, 8);
    for (v, selector, word) in [
        (0xb0u8, 0xc0u8, false),
        (0xb1, 0x80, false),
        (0xb2, 0xc0, true),
        (0xb3, 0x80, true),
    ] {
        let l = |s: &str| format!("bvm_{v:02x}_{s}");
        p.label(format!("bvm_op_{v:02x}"))
            .assume_mode(CpuMode::NATIVE_M8_X16);
        dp(p, M::Ldx, OBJ);
        cbyte(p, 2);
        dp(p, M::Sta, OFF);
        imm8(p, M::Lda, selector);
        jsr(p, "bvm_at");
        m16(p);
        dp(p, M::Stz, LHS);
        dp(p, M::Stz, LHS + 2);
        dp(p, M::Stz, SRC);
        dp(p, M::Stz, SRC + 2);
        m8(p);
        dp(p, M::Lda, VALID);
        br(p, M::Beq, &l("read"));
        if word {
            m16(p);
            ind16(p, M::Lda, PTR);
            dp(p, M::Sta, LHS);
            m8(p);
        } else {
            ind16(p, M::Lda, PTR);
            dp(p, M::Sta, LHS);
        }
        p.label(l("read"));
        if word {
            m16(p);
            cword(p, 4);
            dp(p, M::Sta, SRC);
            m8(p);
        } else {
            cbyte(p, 3);
            dp(p, M::Sta, SRC);
        }
        cbyte(p, 1);
        dp(p, M::Sta, MODE);
        jsr(p, "bvm_compare");
        dp(p, M::Ldx, OBJ);
        dp(p, M::Lda, COND);
        br(p, M::Beq, &l("false"));
        goto(p, if word { 8 } else { 4 });
        next(p);
        p.label(l("false")).assume_mode(CpuMode::NATIVE_M8_X16);
        advance_next(p, if word { 12 } else { 8 });
    }
    for command in [0x13u8, 0x14, 0x15] {
        let width: u16 = match command {
            0x13 => 1,
            0x14 => 2,
            _ => 4,
        };
        let (immediate, target, length) = if width == 1 { (3, 4, 8) } else { (4, 8, 12) };
        for own in [false, true] {
            for relation in 0..COMPARE_RELATIONS {
                let v = compare_variant(command, own, relation);
                let l = |s: &str| format!("bvm_{v:02x}_{s}");
                p.label(format!("bvm_op_{v:02x}"))
                    .assume_mode(CpuMode::NATIVE_M8_X16);
                if own && width == 1 {
                    self_y(p, 2);
                    ind16(p, M::Lda, OBJ);
                    dp(p, M::Sta, LHS);
                } else if own {
                    self_y(p, 2);
                    m16(p);
                    ind16(p, M::Lda, OBJ);
                    dp(p, M::Sta, LHS);
                    if width == 4 {
                        implied(p, M::Iny);
                        implied(p, M::Iny);
                        ind16(p, M::Lda, OBJ);
                        dp(p, M::Sta, LHS + 2);
                    }
                    m8(p);
                } else {
                    dp(p, M::Ldx, OBJ);
                    m16(p);
                    ax(p, M::Lda, 0xa8);
                    dp(p, M::Cmp, REFSRC);
                    br(p, M::Bne, &l("slow"));
                    ax(p, M::Lda, 0xaa);
                    dp(p, M::Cmp, REFSRC + 2);
                    br(p, M::Bne, &l("slow"));
                    m8(p);
                    dp(p, M::Lda, REFFAST);
                    br(p, M::Beq, &l("slow8"));
                    cbyte(p, 2);
                    m16(p);
                    imm16(p, M::And, 0xff);
                    implied(p, M::Tay);
                    if width == 1 {
                        m8(p);
                    }
                    ind16(p, M::Lda, REFBASE);
                    dp(p, M::Sta, LHS);
                    if width == 4 {
                        implied(p, M::Iny);
                        implied(p, M::Iny);
                        ind16(p, M::Lda, REFBASE);
                        dp(p, M::Sta, LHS + 2);
                    }
                    m8(p);
                    br(p, M::Bra, &l("read"));
                    p.label(l("slow")).assume_mode(CpuMode::NATIVE_M16_X16);
                    m8(p);
                    p.label(l("slow8"));
                    cbyte(p, 2);
                    dp(p, M::Sta, OFF);
                    imm8(p, M::Lda, 0xc0);
                    jsr(p, "bvm_at");
                    m16(p);
                    dp(p, M::Stz, LHS);
                    if width == 4 {
                        dp(p, M::Stz, LHS + 2);
                    }
                    m8(p);
                    dp(p, M::Lda, VALID);
                    br(p, M::Beq, &l("read"));
                    if width > 1 {
                        m16(p);
                    }
                    ind16(p, M::Lda, PTR);
                    dp(p, M::Sta, LHS);
                    if width == 4 {
                        implied(p, M::Iny);
                        implied(p, M::Iny);
                        ind16(p, M::Lda, PTR);
                        dp(p, M::Sta, LHS + 2);
                    }
                    m8(p);
                    p.label(l("read"));
                }
                let test = if relation == 6 { M::And } else { M::Cmp };
                if width == 1 {
                    imm16(p, M::Ldy, immediate);
                    dp(p, M::Lda, LHS);
                    ind(p, test, PCP);
                } else {
                    m16(p);
                    if width == 4 {
                        imm16(p, M::Ldy, immediate + 2);
                        dp(p, M::Lda, LHS + 2);
                        ind(p, test, PCP);
                        br(p, M::Bne, &l("flags"));
                    }
                    imm16(p, M::Ldy, immediate);
                    dp(p, M::Lda, LHS);
                    ind(p, test, PCP);
                    p.label(l("flags"));
                    m8(p);
                }
                match relation {
                    0 | 6 => br(p, M::Bne, &l("true")),
                    1 => br(p, M::Beq, &l("true")),
                    2 => {
                        br(p, M::Beq, &l("false"));
                        br(p, M::Bcs, &l("true"));
                    }
                    3 => br(p, M::Bcc, &l("true")),
                    4 => br(p, M::Bcs, &l("true")),
                    _ => {
                        br(p, M::Beq, &l("true"));
                        br(p, M::Bcc, &l("true"));
                    }
                }
                p.label(l("false"));
                dp(p, M::Ldx, OBJ);
                advance_next(p, length);
                p.label(l("true")).assume_mode(CpuMode::NATIVE_M8_X16);
                dp(p, M::Ldx, OBJ);
                goto(p, target);
                next(p);
            }
        }
    }
    for (v, dword, operation) in [
        (0xb4u8, true, 0u8),
        (0xb5, true, 1),
        (0xb6, true, 2),
        (0xb7, false, 0),
        (0xb8, false, 1),
        (0xb9, false, 2),
    ] {
        p.label(format!("bvm_op_{v:02x}"))
            .assume_mode(CpuMode::NATIVE_M8_X16);
        dp(p, M::Ldx, OBJ);
        m16(p);
        cword(p, 4);
        dp(p, M::Sta, TMP);
        if dword {
            cword(p, 6);
            dp(p, M::Sta, TMP + 2);
        }
        m8(p);
        self_y(p, 2);
        m16(p);
        match operation {
            0 => {
                dp(p, M::Lda, TMP);
                ind16(p, M::Sta, OBJ);
                if dword {
                    implied(p, M::Iny);
                    implied(p, M::Iny);
                    dp(p, M::Lda, TMP + 2);
                    ind16(p, M::Sta, OBJ);
                }
            }
            _ => {
                let (first, carry) = if operation == 1 {
                    (M::Clc, M::Adc)
                } else {
                    (M::Sec, M::Sbc)
                };
                ind16(p, M::Lda, OBJ);
                implied(p, first);
                dp(p, carry, TMP);
                ind16(p, M::Sta, OBJ);
                if dword {
                    implied(p, M::Iny);
                    implied(p, M::Iny);
                    ind16(p, M::Lda, OBJ);
                    dp(p, carry, TMP + 2);
                    ind16(p, M::Sta, OBJ);
                }
            }
        }
        m8(p);
        advance_next(p, 8);
    }
    for (v, operation) in [(0xbau8, 0u8), (0xbb, 6), (0xbc, 7)] {
        let l = |s: &str| format!("bvm_{v:02x}_{s}");
        p.label(format!("bvm_op_{v:02x}"))
            .assume_mode(CpuMode::NATIVE_M8_X16);
        dp(p, M::Ldx, OBJ);
        cbyte(p, 3);
        dp(p, M::Sta, SRC);
        cbyte(p, 2);
        dp(p, M::Sta, OFF);
        imm8(p, M::Lda, 0xc0);
        jsr(p, "bvm_at");
        dp(p, M::Lda, VALID);
        br(p, M::Beq, &l("done"));
        dp(p, M::Lda, SRC);
        match operation {
            6 => ind16(p, M::And, PTR),
            7 => ind16(p, M::Ora, PTR),
            _ => {}
        }
        ind16(p, M::Sta, PTR);
        p.label(l("done"));
        dp(p, M::Ldx, OBJ);
        advance_next(p, 4);
    }
    p.label("bvm_op_bd").assume_mode(CpuMode::NATIVE_M8_X16);
    dp(p, M::Ldx, OBJ);
    cbyte(p, 2);
    dp(p, M::Sta, OFF);
    imm8(p, M::Lda, 0xc0);
    jsr(p, "bvm_at");
    dp(p, M::Lda, VALID);
    br(p, M::Beq, "bvm_bd_done");
    self_y(p, 3);
    m16(p);
    ind16(p, M::Lda, OBJ);
    dp(p, M::Sta, TMP);
    implied(p, M::Iny);
    implied(p, M::Iny);
    ind16(p, M::Lda, OBJ);
    dp(p, M::Sta, TMP + 2);
    dp(p, M::Lda, OFF);
    imm16(p, M::And, 0xff);
    implied(p, M::Tay);
    dp(p, M::Lda, TMP);
    ind16(p, M::Sta, PTR);
    implied(p, M::Iny);
    implied(p, M::Iny);
    dp(p, M::Lda, TMP + 2);
    ind16(p, M::Sta, PTR);
    m8(p);
    p.label("bvm_bd_done");
    dp(p, M::Ldx, OBJ);
    advance_next(p, 4);
    for (v, dword, operation) in [
        (0xbeu8, false, 0u8),
        (0xbf, false, 1),
        (0xc0, false, 2),
        (0xc1, true, 0),
        (0xc2, true, 1),
        (0xc3, true, 2),
    ] {
        p.label(format!("bvm_op_{v:02x}"))
            .assume_mode(CpuMode::NATIVE_M8_X16);
        dp(p, M::Ldx, OBJ);
        self_y(p, 3);
        m16(p);
        ind16(p, M::Lda, OBJ);
        dp(p, M::Sta, TMP);
        if dword {
            implied(p, M::Iny);
            implied(p, M::Iny);
            ind16(p, M::Lda, OBJ);
            dp(p, M::Sta, TMP + 2);
        }
        m8(p);
        self_y(p, 2);
        m16(p);
        match operation {
            0 => {
                dp(p, M::Lda, TMP);
                ind16(p, M::Sta, OBJ);
                if dword {
                    implied(p, M::Iny);
                    implied(p, M::Iny);
                    dp(p, M::Lda, TMP + 2);
                    ind16(p, M::Sta, OBJ);
                }
            }
            _ => {
                let (first, carry) = if operation == 1 {
                    (M::Clc, M::Adc)
                } else {
                    (M::Sec, M::Sbc)
                };
                ind16(p, M::Lda, OBJ);
                implied(p, first);
                dp(p, carry, TMP);
                ind16(p, M::Sta, OBJ);
                if dword {
                    implied(p, M::Iny);
                    implied(p, M::Iny);
                    ind16(p, M::Lda, OBJ);
                    dp(p, carry, TMP + 2);
                    ind16(p, M::Sta, OBJ);
                }
            }
        }
        m8(p);
        advance_next(p, 4);
    }
    for (v, dword, operation) in [
        (0xf1u8, false, 0u8),
        (0xf2, false, 1),
        (0xf3, false, 2),
        (0xf4, true, 0),
        (0xf5, true, 1),
        (0xf6, true, 2),
    ] {
        let l = |s: &str| format!("bvm_{v:02x}_{s}");
        p.label(format!("bvm_op_{v:02x}"))
            .assume_mode(CpuMode::NATIVE_M8_X16);
        dp(p, M::Ldx, OBJ);
        cbyte(p, 3);
        dp(p, M::Sta, OFF);
        imm8(p, M::Lda, 0xc0);
        jsr(p, "bvm_at");
        m16(p);
        dp(p, M::Stz, TMP);
        dp(p, M::Stz, TMP + 2);
        m8(p);
        dp(p, M::Lda, VALID);
        br(p, M::Beq, &l("read"));
        m16(p);
        ind16(p, M::Lda, PTR);
        dp(p, M::Sta, TMP);
        if dword {
            implied(p, M::Iny);
            implied(p, M::Iny);
            ind16(p, M::Lda, PTR);
            dp(p, M::Sta, TMP + 2);
        }
        m8(p);
        p.label(l("read"));
        self_y(p, 2);
        m16(p);
        match operation {
            0 => {
                dp(p, M::Lda, TMP);
                ind16(p, M::Sta, OBJ);
                if dword {
                    implied(p, M::Iny);
                    implied(p, M::Iny);
                    dp(p, M::Lda, TMP + 2);
                    ind16(p, M::Sta, OBJ);
                }
            }
            _ => {
                let (first, carry) = if operation == 1 {
                    (M::Clc, M::Adc)
                } else {
                    (M::Sec, M::Sbc)
                };
                ind16(p, M::Lda, OBJ);
                implied(p, first);
                dp(p, carry, TMP);
                ind16(p, M::Sta, OBJ);
                if dword {
                    implied(p, M::Iny);
                    implied(p, M::Iny);
                    ind16(p, M::Lda, OBJ);
                    dp(p, carry, TMP + 2);
                    ind16(p, M::Sta, OBJ);
                }
            }
        }
        m8(p);
        dp(p, M::Ldx, OBJ);
        advance_next(p, 4);
    }
}
