use super::*;
const ENTRIES: [(u32, u8); 2] = [(0x46be9c, 0xa6), (0x46bec0, 0xa7)];
pub(super) fn report() -> serde_json::Value {
    serde_json::json!(
        { "entries" : ENTRIES, "private_object_fields" : [0x68, 0x72], "source_range" :
        [0x46be24, 0x46bee0], "source_sha256" :
        "f8c4db551c4823ec1b7bf375b1b2a52bcf20bdddf70231c8322a455a0e0eb8c6", "projection"
        : "source-owned inward displacement reuse; see inward_projection", "fallback" :
        "compiled arithmetic pair then source comparison" }
    )
}
pub(super) fn patch(out: &mut [u8], source: &[u8], commands: &[u32]) -> Result<()> {
    if source
        .get((0x46be24 - SOURCE_BASE) as usize..(0x46bee0 - SOURCE_BASE) as usize)
        .is_none_or(|b| {
            crate::sha(b) != "f8c4db551c4823ec1b7bf375b1b2a52bcf20bdddf70231c8322a455a0e0eb8c6"
        })
        || out
            .get((0x46be24 - SOURCE_BASE) as usize..(0x46bee0 - SOURCE_BASE) as usize)
            .is_none_or(|b| {
                crate::sha(b) != "f73af008139bd9a58783524adc587fa5c31abff23d25f5129f6f938e5248b3bf"
            })
        || commands.iter().any(|pc| {
            out.get((pc - SOURCE_BASE) as usize)
                .is_some_and(|b| ENTRIES.iter().any(|(_, tag)| tag == b))
        })
    {
        return Err("Inward particle source or opcode changed".into());
    }
    for (pc, tag) in ENTRIES {
        let at = (pc - SOURCE_BASE) as usize;
        if ![pc, pc + 4, pc + 8].iter().all(|p| commands.contains(p))
            || out.get(at..at + 8) != Some(&[0xc0, 0xa2, 0x92, 0x58, 0xc0, 0xa2, 0x94, 0x58])
            || out.get(at + 8) != Some(&0xdf)
        {
            return Err("Inward particle arithmetic or comparison changed".into());
        }
        out[at] = tag;
        out[at + 1] = 0xc0;
    }
    Ok(())
}
pub(super) fn emit_step(p: &mut Assembler, layout: &CartridgeLayout) -> Result<()> {
    for (pc, tag) in ENTRIES {
        p.label(format!("bvm_op_{tag:02x}"))
            .assume_mode(CpuMode::NATIVE_M8_X16);
        dp(p, M::Ldx, OBJ);
        m16(p);
        imm16(p, M::Ldy, (pc + 8) as u16);
        if tag == 0xa6 {
            br(p, M::Bra, "bvm_inward_common");
        }
    }
    p.label("bvm_inward_common");
    emit(
        p,
        M::Jsl,
        A::AbsoluteLong,
        O::Long(crate::sfc_far_code::inward_turn_entry(layout)?),
    );
    br(p, M::Bcc, "bvm_inward_slow");
    m8(p);
    imm8(p, M::Lda, 0x0b);
    dp(p, M::Sta, AXIS);
    jsr(p, "bvm_inward_projection");
    yield_now(p);
    p.label("bvm_inward_slow")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    implied(p, M::Tya);
    ax(p, M::Sta, 0x40);
    dp(p, M::Sta, PCP);
    ax(p, M::Lda, 0x58);
    dp(p, M::Sta, TMP);
    for amp in [0x92, 0x94] {
        ax(p, M::Lda, amp);
        implied(p, M::Sec);
        dp(p, M::Sbc, TMP);
        ax(p, M::Sta, amp);
    }
    imm16(p, M::Ldy, 0x94);
    implied(p, M::Clc);
    implied(p, M::Clv);
    imm16(p, M::Lda, 0x01be);
    implied(p, M::Tax);
    m8(p);
    jmp(p, "bvm_op_df");
    inward_projection::emit_wrappers(p, layout)?;
    Ok(())
}
pub(super) fn emit_far(p: &mut Assembler) {
    p.label("bvm_inward_turn_far")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    ax(p, M::Lda, 0x42);
    imm16(p, M::Cmp, 0x46);
    br(p, M::Bne, "bvm_inward_reject");
    implied(p, M::Tya);
    implied(p, M::Sec);
    imm16(p, M::Sbc, 8);
    ax(p, M::Cmp, 0x40);
    br(p, M::Bne, "bvm_inward_reject");
    ax(p, M::Lda, 0x94);
    implied(p, M::Sec);
    ax(p, M::Sbc, 0x58);
    br(p, M::Bmi, "bvm_inward_reject");
    imm16(p, M::Cpy, 0xbea4);
    br(p, M::Beq, "bvm_inward_move");
    dp(p, M::Lda, BUDGET);
    imm16(p, M::Cmp, 2);
    br(p, M::Bcs, "bvm_inward_move");
    p.label("bvm_inward_reject");
    implied(p, M::Clc);
    implied(p, M::Rtl);
    p.label("bvm_inward_move");
    for amp in [0x92, 0x94] {
        ax(p, M::Lda, amp);
        implied(p, M::Sec);
        ax(p, M::Sbc, 0x58);
        ax(p, M::Sta, amp);
    }
    dp(p, M::Stz, TMP);
    imm16(p, M::Lda, 4);
    dp(p, M::Sta, TMP + 2);
    ax(p, M::Lda, 0x86);
    imm16(p, M::Cpy, 0xbea4);
    br(p, M::Beq, "bvm_inward_down");
    implied(p, M::Sec);
    imm16(p, M::Sbc, 4);
    ax(p, M::Sta, 0x86);
    dp(p, M::Dec, BUDGET);
    imm16(p, M::Lda, 0xbe9c);
    br(p, M::Bra, "bvm_inward_publish");
    p.label("bvm_inward_down");
    implied(p, M::Clc);
    imm16(p, M::Adc, 4);
    ax(p, M::Sta, 0x86);
    imm16(p, M::Lda, 0xbec0);
    p.label("bvm_inward_publish");
    ax(p, M::Sta, 0x40);
    dp(p, M::Sta, PCP);
    implied(p, M::Sec);
    implied(p, M::Rtl);
}
