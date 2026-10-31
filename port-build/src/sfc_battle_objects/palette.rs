use super::*;
mod screen;
pub(super) fn emit_palette(p: &mut Assembler) {
    screen::emit(p);
    use crate::sfc_battle_display::{PALETTE_CURRENT, PALETTE_DIRTY, PALETTE_KEPT};
    let long_x = |p: &mut Assembler, m: M, at: u32| emit(p, m, A::AbsoluteLongX, O::Long(at));
    let op = |n: u8| format!("bvm_op_{n:02x}");
    let (start, count, other) = (ATA, ATA + 1, ATA + 2);
    let (step, target) = (ATB, ATB + 2);
    p.label("bvm_pal_index").assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    imm16(p, M::And, 0xff);
    dp(p, M::Sta, target);
    acc(p, M::Asl);
    implied(p, M::Clc);
    dp(p, M::Adc, target);
    implied(p, M::Tax);
    m8(p);
    implied(p, M::Rts);
    p.label("bvm_pal_mark").assume_mode(CpuMode::NATIVE_M8_X16);
    dp(p, M::Lda, count);
    br(p, M::Beq, "bvm_pal_mark_done");
    dp(p, M::Lda, start);
    implied(p, M::Clc);
    dp(p, M::Adc, count);
    acc(p, M::Dec);
    for _ in 0..4 {
        acc(p, M::Lsr);
    }
    dp(p, M::Sta, other);
    dp(p, M::Lda, start);
    for _ in 0..4 {
        acc(p, M::Lsr);
    }
    p.label("bvm_pal_mark_block");
    implied(p, M::Pha);
    m16(p);
    imm16(p, M::And, 0x0f);
    acc(p, M::Asl);
    implied(p, M::Tax);
    p.emit_label_ref(M::Lda, A::AbsoluteLongX, "bvm_pal_bits");
    abs(p, M::Ora, PALETTE_DIRTY);
    abs(p, M::Sta, PALETTE_DIRTY);
    m8(p);
    implied(p, M::Pla);
    dp(p, M::Cmp, other);
    br(p, M::Beq, "bvm_pal_mark_done");
    acc(p, M::Inc);
    imm8(p, M::And, 0x0f);
    br(p, M::Bra, "bvm_pal_mark_block");
    p.label("bvm_pal_mark_done");
    implied(p, M::Rts);
    p.label("bvm_pal_bits");
    p.data(
        (0..16)
            .flat_map(|b| (1u16 << b).to_le_bytes())
            .collect::<Vec<u8>>(),
    );
    p.label("bvm_pal_approach")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    dp(p, M::Sta, target);
    dp(p, M::Stz, target + 1);
    long_x(p, M::Lda, PALETTE_CURRENT);
    dp(p, M::Cmp, target);
    br(p, M::Beq, "bvm_pal_approach_done");
    br(p, M::Bcs, "bvm_pal_approach_down");
    implied(p, M::Clc);
    dp(p, M::Adc, step);
    br(p, M::Bcs, "bvm_pal_approach_clamp");
    dp(p, M::Cmp, target);
    br(p, M::Bcc, "bvm_pal_approach_store");
    br(p, M::Bra, "bvm_pal_approach_clamp");
    p.label("bvm_pal_approach_down");
    implied(p, M::Sec);
    dp(p, M::Sbc, step);
    br(p, M::Bcc, "bvm_pal_approach_clamp");
    dp(p, M::Cmp, target);
    br(p, M::Bcs, "bvm_pal_approach_store");
    p.label("bvm_pal_approach_clamp");
    dp(p, M::Lda, target);
    p.label("bvm_pal_approach_store");
    long_x(p, M::Sta, PALETTE_CURRENT);
    p.label("bvm_pal_approach_done");
    implied(p, M::Rts);
    p.label("bvm_fade_batch")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, crate::sfc_battle_vm::SCENE);
    imm8(p, M::Cmp, crate::sfc_ending::SCENE);
    br(p, M::Bne, "bvm_fade_batch_none");
    for (units, mask, batch) in [(1u8, 7u8, 8u8), (2, 7, 16), (4, 3, 16)] {
        let next = format!("bvm_fade_batch_not_{units}");
        dp(p, M::Lda, step);
        imm8(p, M::Cmp, units);
        br(p, M::Bne, &next);
        abs(p, M::Lda, crate::sfc_ending::UPDATES);
        imm8(p, M::And, mask);
        br(p, M::Bne, "bvm_fade_batch_skip");
        imm8(p, M::Lda, batch);
        dp(p, M::Sta, step);
        br(p, M::Bra, "bvm_fade_batch_none");
        p.label(&next);
    }
    p.label("bvm_fade_batch_none");
    implied(p, M::Clc);
    implied(p, M::Rts);
    p.label("bvm_fade_batch_skip");
    implied(p, M::Sec);
    implied(p, M::Rts);
    for (n, fade) in [(0x38u8, true), (0x39, false)] {
        let l = |s: &str| format!("bvm_{n:02x}_{s}");
        p.label(op(n)).assume_mode(CpuMode::NATIVE_M8_X16);
        dp(p, M::Ldx, OBJ);
        cbyte(p, 1);
        br(p, M::Beq, &l("table"));
        p.emit_label_ref(M::Jmp, A::Absolute, l("modes"));
        p.label(l("table"));
        if fade {
            cbyte(p, 2);
            br(p, M::Beq, &l("done"));
            dp(p, M::Sta, step);
            dp(p, M::Stz, step + 1);
            jsr(p, "bvm_fade_batch");
            br(p, M::Bcs, &l("done"));
        }
        m16(p);
        cword(p, 4);
        dp(p, M::Sta, TMP);
        cword(p, 6);
        dp(p, M::Sta, TMP + 2);
        m8(p);
        jsr(p, "bvm_rom");
        imm16(p, M::Ldy, 0);
        p.label(l("entry"));
        ind(p, M::Lda, ROMP);
        imm8(p, M::Cmp, 0xff);
        br(p, M::Beq, &l("done"));
        dp(p, M::Sta, start);
        imm8(p, M::Lda, 1);
        dp(p, M::Sta, count);
        implied(p, M::Phy);
        jsr(p, "bvm_pal_mark");
        dp(p, M::Lda, start);
        jsr(p, "bvm_pal_index");
        implied(p, M::Ply);
        for _ in 0..3 {
            implied(p, M::Iny);
            ind(p, M::Lda, ROMP);
            for _ in 0..4 {
                acc(p, M::Asl);
            }
            if fade {
                implied(p, M::Phy);
                jsr(p, "bvm_pal_approach");
                implied(p, M::Ply);
            } else {
                long_x(p, M::Sta, PALETTE_CURRENT);
            }
            implied(p, M::Inx);
        }
        implied(p, M::Iny);
        br(p, M::Bra, &l("entry"));
        p.label(l("done"));
        dp(p, M::Ldx, OBJ);
        advance_next(p, 8);
        p.label(l("modes"));
        imm8(p, M::Cmp, 1);
        br(p, M::Beq, &l("inline"));
        imm8(p, M::Cmp, 2);
        br(p, M::Bne, &l("modes_done"));
        p.emit_label_ref(M::Jmp, A::Absolute, l(if fade { "kept" } else { "keep" }));
        p.label(l("modes_done"));
        p.emit_label_ref(M::Jmp, A::Absolute, l("done"));
        for (mode, steps_at) in [("inline", 7u16), ("kept", 4), ("keep", 0)] {
            if (mode == "kept" && !fade) || (mode == "keep" && fade) {
                continue;
            }
            p.label(l(mode)).assume_mode(CpuMode::NATIVE_M8_X16);
            cbyte(p, 2);
            dp(p, M::Sta, start);
            cbyte(p, 3);
            dp(p, M::Sta, count);
            if fade {
                cbyte(p, steps_at);
                br(p, M::Beq, &l(&format!("{mode}_finish")));
                dp(p, M::Sta, step);
                dp(p, M::Stz, step + 1);
                jsr(p, "bvm_fade_batch");
                br(p, M::Bcs, &l(&format!("{mode}_finish")));
            }
            if fade && mode == "inline" {
                let no = l("inline_screen_no");
                dp(p, M::Lda, start);
                imm8(p, M::Cmp, 48);
                br(p, M::Bcs, &no);
                implied(p, M::Clc);
                dp(p, M::Adc, count);
                br(p, M::Bcs, &l("inline_screen"));
                imm8(p, M::Cmp, 240);
                br(p, M::Bcc, &no);
                p.label(l("inline_screen"));
                implied(p, M::Phy);
                cbyte(p, 4);
                imm8(p, M::And, 0x80);
                abs(p, M::Sta, crate::sfc_ending::SCREEN_WHITE);
                implied(p, M::Ply);
                abs(p, M::Lda, crate::sfc_field_events::SCREEN_DARK);
                implied(p, M::Clc);
                dp(p, M::Adc, step);
                br(p, M::Bcs, &l("inline_screen_cap"));
                imm8(p, M::Cmp, 248);
                br(p, M::Bcc, &l("inline_screen_store"));
                p.label(l("inline_screen_cap"));
                imm8(p, M::Lda, 248);
                p.label(l("inline_screen_store"));
                abs(p, M::Sta, crate::sfc_field_events::SCREEN_DARK);
                p.label(no);
            }
            if fade && mode == "kept" {
                jsr(p, "bvm_screen_restore_fade");
            }
            if mode != "keep" {
                jsr(p, "bvm_pal_mark");
            }
            br(p, M::Bra, &l(&format!("{mode}_each")));
            p.label(l(&format!("{mode}_finish")));
            p.emit_label_ref(M::Jmp, A::Absolute, l("done"));
            p.label(l(&format!("{mode}_each")));
            if fade {
                let e = |s: &str| l(&format!("{mode}_fast_{s}"));
                if mode == "inline" {
                    for c in 0..3u16 {
                        cbyte(p, 4 + c);
                        dp(p, M::Sta, DX + c as u8);
                    }
                }
                dp(p, M::Lda, count);
                br(p, M::Beq, &l(&format!("{mode}_finish")));
                dp(p, M::Lda, start);
                jsr(p, "bvm_pal_index");
                implied(p, M::Phb);
                imm8(p, M::Lda, (PALETTE_CURRENT >> 16) as u8);
                implied(p, M::Pha);
                implied(p, M::Plb);
                p.label(e("index"));
                for c in 0..3u16 {
                    let f = |s: &str| e(&format!("{s}_{c}"));
                    if mode == "kept" {
                        ax(p, M::Lda, PALETTE_KEPT as u16);
                    } else {
                        dp(p, M::Lda, DX + c as u8);
                    }
                    dp(p, M::Sta, target);
                    ax(p, M::Lda, PALETTE_CURRENT as u16);
                    dp(p, M::Cmp, target);
                    br(p, M::Beq, &f("same"));
                    br(p, M::Bcs, &f("down"));
                    implied(p, M::Clc);
                    dp(p, M::Adc, step);
                    br(p, M::Bcs, &f("clamp"));
                    dp(p, M::Cmp, target);
                    br(p, M::Bcc, &f("store"));
                    br(p, M::Bra, &f("clamp"));
                    p.label(f("down"));
                    implied(p, M::Sec);
                    dp(p, M::Sbc, step);
                    br(p, M::Bcc, &f("clamp"));
                    dp(p, M::Cmp, target);
                    br(p, M::Bcs, &f("store"));
                    p.label(f("clamp"));
                    dp(p, M::Lda, target);
                    p.label(f("store"));
                    ax(p, M::Sta, PALETTE_CURRENT as u16);
                    p.label(f("same"));
                    implied(p, M::Inx);
                }
                emit(p, M::Cpx, A::Immediate, O::Word(768));
                br(p, M::Bne, &e("wrapped"));
                emit(p, M::Ldx, A::Immediate, O::Word(0));
                p.label(e("wrapped"));
                dp(p, M::Dec, count);
                br(p, M::Beq, &e("end"));
                p.emit_label_ref(M::Jmp, A::Absolute, e("index"));
                p.label(e("end"));
                implied(p, M::Plb);
                p.emit_label_ref(M::Jmp, A::Absolute, l(&format!("{mode}_finish")));
                continue;
            }
            dp(p, M::Lda, count);
            br(p, M::Beq, &l(&format!("{mode}_finish")));
            dp(p, M::Lda, start);
            jsr(p, "bvm_pal_index");
            for c in 0..3u16 {
                if mode == "kept" {
                    long_x(p, M::Lda, PALETTE_KEPT);
                } else {
                    cbyte(p, 4 + c);
                }
                if fade {
                    jsr(p, "bvm_pal_approach");
                } else if mode == "keep" {
                    long_x(p, M::Sta, PALETTE_KEPT);
                } else {
                    long_x(p, M::Sta, PALETTE_CURRENT);
                }
                implied(p, M::Inx);
            }
            dp(p, M::Inc, start);
            dp(p, M::Dec, count);
            br(p, M::Bra, &l(&format!("{mode}_each")));
        }
    }
    p.label(op(0x3a)).assume_mode(CpuMode::NATIVE_M8_X16);
    cbyte(p, 3);
    br(p, M::Beq, "bvm_3a_done");
    dp(p, M::Sta, step);
    cbyte(p, 1);
    imm8(p, M::Cmp, 0x30);
    br(p, M::Bne, "bvm_3a_shadow");
    cbyte(p, 2);
    imm8(p, M::Cmp, 0x20);
    br(p, M::Bne, "bvm_3a_shadow");
    abs(p, M::Lda, crate::sfc_battle_vm::WALL);
    implied(p, M::Sec);
    dp(p, M::Sbc, step);
    abs(p, M::Sta, crate::sfc_battle_vm::WALL);
    imm8(p, M::Lda, 1);
    abs(p, M::Sta, crate::sfc_battle_vm::DIM);
    p.label("bvm_3a_shadow");
    cbyte(p, 2);
    br(p, M::Beq, "bvm_3a_done");
    m16(p);
    imm16(p, M::And, 0xff);
    dp(p, M::Sta, TMP);
    acc(p, M::Asl);
    implied(p, M::Clc);
    dp(p, M::Adc, TMP);
    dp(p, M::Sta, TMP);
    m8(p);
    cbyte(p, 1);
    jsr(p, "bvm_pal_index");
    dp(p, M::Ldy, TMP);
    p.label("bvm_3a_byte");
    dp(p, M::Lda, step);
    br(p, M::Bmi, "bvm_3a_right");
    dp(p, M::Sta, other);
    long_x(p, M::Lda, PALETTE_CURRENT);
    p.label("bvm_3a_left");
    acc(p, M::Asl);
    dp(p, M::Dec, other);
    br(p, M::Bne, "bvm_3a_left");
    br(p, M::Bra, "bvm_3a_store");
    p.label("bvm_3a_right");
    imm8(p, M::Eor, 0xff);
    acc(p, M::Inc);
    dp(p, M::Sta, other);
    long_x(p, M::Lda, PALETTE_CURRENT);
    p.label("bvm_3a_shift_right");
    acc(p, M::Lsr);
    dp(p, M::Dec, other);
    br(p, M::Bne, "bvm_3a_shift_right");
    p.label("bvm_3a_store");
    long_x(p, M::Sta, PALETTE_CURRENT);
    implied(p, M::Inx);
    implied(p, M::Dey);
    br(p, M::Bne, "bvm_3a_byte");
    p.label("bvm_3a_done");
    dp(p, M::Ldx, OBJ);
    advance_next(p, 4);
    for n in [0x3bu8, 0x3c, 0x3d] {
        let l = |s: &str| format!("bvm_{n:02x}_{s}");
        let (from, to) = match n {
            0x3b => (PALETTE_CURRENT, PALETTE_KEPT),
            0x3c => (PALETTE_KEPT, PALETTE_CURRENT),
            _ => (PALETTE_CURRENT, PALETTE_CURRENT),
        };
        p.label(op(n)).assume_mode(CpuMode::NATIVE_M8_X16);
        dp(p, M::Ldx, OBJ);
        cbyte(p, 1);
        dp(p, M::Sta, start);
        if n == 0x3d {
            cbyte(p, 2);
            dp(p, M::Sta, other);
            cbyte(p, 3);
        } else {
            dp(p, M::Sta, other);
            cbyte(p, 2);
        }
        dp(p, M::Sta, count);
        br(p, M::Beq, &l("done"));
        if n == 0x3b {
            jsr(p, "bvm_screen_keep");
        } else if n == 0x3c {
            jsr(p, "bvm_screen_restore");
        }
        if n != 0x3b {
            dp(p, M::Lda, other);
            implied(p, M::Pha);
            jsr(p, "bvm_pal_mark");
            implied(p, M::Pla);
            dp(p, M::Sta, other);
        }
        p.label(l("each"));
        dp(p, M::Lda, other);
        jsr(p, "bvm_pal_index");
        for i in 0..3u32 {
            long_x(p, M::Lda, from + i);
            dp(p, M::Sta, TMP + i as u8);
        }
        dp(p, M::Lda, start);
        jsr(p, "bvm_pal_index");
        for i in 0..3u32 {
            dp(p, M::Lda, TMP + i as u8);
            long_x(p, M::Sta, to + i);
        }
        dp(p, M::Inc, start);
        dp(p, M::Inc, other);
        dp(p, M::Dec, count);
        br(p, M::Bne, &l("each"));
        p.label(l("done"));
        dp(p, M::Ldx, OBJ);
        advance_next(p, 4);
    }
}
