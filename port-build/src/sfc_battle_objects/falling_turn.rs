use super::*;
const OPCODE: u8 = 0x92;
const ENTRIES: [u32; 4] = [0x4bcb50, 0x4bcb54, 0x4bcb60, 0x4bcb64];
pub(super) fn report() -> serde_json::Value {
    serde_json::json!(
        { "entries" : ENTRIES, "opcode" : OPCODE, "angles" : [0x8000, 0x8000],
        "private_object_fields" : [] }
    )
}
pub(super) fn patch(out: &mut [u8], source: &[u8], commands: &[u32]) -> Result<()> {
    if source
        .get((0x4bca8c - SOURCE_BASE) as usize..(0x4bcb84 - SOURCE_BASE) as usize)
        .is_none_or(|b| {
            crate::sha(b) != "389b569fcfa99eea4556bcf5c5f95db3deadf39db70932fc20738de446f23592"
        })
        || commands
            .iter()
            .any(|pc| out.get((pc - SOURCE_BASE) as usize) == Some(&OPCODE))
    {
        return Err("Falling spark source or opcode changed".into());
    }
    for pc in ENTRIES {
        let at = (pc - SOURCE_BASE) as usize;
        let expected = if pc & 4 == 0 {
            [0x2d, 0x0b, 0, 0]
        } else {
            [2, 0, 1, 0]
        };
        if !commands.contains(&pc) || out.get(at..at + 4) != Some(&expected) {
            return Err("Falling spark entry changed".into());
        }
        out[at] = OPCODE;
    }
    Ok(())
}
pub(super) fn emit_step(p: &mut Assembler, layout: &CartridgeLayout) -> Result<()> {
    p.label("bvm_op_92").assume_mode(CpuMode::NATIVE_M8_X16);
    emit(
        p,
        M::Jsl,
        A::AbsoluteLong,
        O::Long(crate::sfc_far_code::falling_turn_entry(layout)?),
    );
    br(p, M::Bcs, "bvm_falling_done");
    p.emit_label_ref(M::Jmp, A::AbsoluteIndexedIndirectX, "bvm_ops");
    p.label("bvm_falling_done");
    yield_now(p);
    Ok(())
}
pub(super) fn emit_far(p: &mut Assembler, layout: &CartridgeLayout) -> Result<()> {
    let sine = &layout.section("bvm_sine")?.bytes;
    for (angle, expected) in [(0x8000usize, [0, 0, 0]), (0xc000, [0, 0, 0xff])] {
        if sine.get(angle / 16 * 4..angle / 16 * 4 + 3) != Some(&expected) {
            return Err("Falling spark half-turn coefficients changed".into());
        }
    }
    p.label("bvm_falling_turn_far")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    dp(p, M::Ldx, OBJ);
    m16(p);
    imm16(p, M::Ldy, 2 * 0x2d);
    ax(p, M::Lda, 0x40);
    imm16(p, M::Bit, 4);
    br(p, M::Beq, "bvm_falling_angles");
    imm16(p, M::Ldy, 2 * 0x02);
    p.label("bvm_falling_angles");
    for angle in [0x8c, 0x8e] {
        ax(p, M::Lda, angle);
        imm16(p, M::Cmp, 0x8000);
        br(p, M::Bne, "bvm_falling_slow");
    }
    ax(p, M::Lda, 0x40);
    imm16(p, M::Bit, 4);
    br(p, M::Bne, "bvm_falling_wait");
    ax(p, M::Lda, 0x5e);
    br(p, M::Bne, "bvm_falling_slow");
    ax(p, M::Lda, 0x40);
    implied(p, M::Clc);
    imm16(p, M::Adc, 4);
    dp(p, M::Sta, TMP);
    jmp(p, "bvm_falling_project");
    p.label("bvm_falling_wait");
    ax(p, M::Lda, 0x5e);
    imm16(p, M::Cmp, 1);
    br(p, M::Bne, "bvm_falling_slow");
    ax(p, M::Lda, 0x40);
    imm16(p, M::Cmp, 0xcb54);
    br(p, M::Beq, "bvm_falling_move");
    dp(p, M::Lda, BUDGET);
    imm16(p, M::Cmp, 2);
    br(p, M::Bcc, "bvm_falling_slow");
    ax(p, M::Lda, 0x3c);
    imm16(p, M::Cmp, 0xcb50);
    br(p, M::Bne, "bvm_falling_latch");
    ax(p, M::Lda, 0x3e);
    imm16(p, M::Cmp, 0x4b);
    br(p, M::Bne, "bvm_falling_latch");
    ax(p, M::Lda, 0x60);
    br(p, M::Beq, "bvm_falling_slow");
    br(p, M::Bra, "bvm_falling_move");
    p.label("bvm_falling_latch");
    imm16(p, M::Lda, 0xcb50);
    ax(p, M::Sta, 0x3c);
    imm16(p, M::Lda, 0x4b);
    ax(p, M::Sta, 0x3e);
    imm16(p, M::Lda, 5);
    ax(p, M::Sta, 0x60);
    br(p, M::Bra, "bvm_falling_move");
    p.label("bvm_falling_slow")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    implied(p, M::Tyx);
    m8(p);
    implied(p, M::Clc);
    implied(p, M::Rtl);
    p.label("bvm_falling_move")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    for amp in [0x92, 0x94] {
        ax(p, M::Lda, amp);
        implied(p, M::Clc);
        ax(p, M::Adc, 0x58);
        ax(p, M::Sta, amp);
    }
    ax(p, M::Lda, 0x40);
    imm16(p, M::Cmp, 0xcb54);
    br(p, M::Beq, "bvm_falling_first");
    ax(p, M::Inc, 0x58);
    ax(p, M::Lda, 0x60);
    imm16(p, M::Cmp, 1);
    br(p, M::Bne, "bvm_falling_loop");
    ax(p, M::Stz, 0x5e);
    imm16(p, M::Lda, 0xcb78);
    ax(p, M::Sta, 0x40);
    dp(p, M::Sta, PCP);
    imm16(p, M::Ldx, 2 * 0x06);
    m8(p);
    implied(p, M::Clc);
    implied(p, M::Rtl);
    p.label("bvm_falling_loop")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    ax(p, M::Dec, 0x60);
    dp(p, M::Dec, BUDGET);
    imm16(p, M::Lda, 0xcb54);
    br(p, M::Bra, "bvm_falling_target");
    p.label("bvm_falling_first");
    imm16(p, M::Lda, 0xcb64);
    p.label("bvm_falling_target");
    dp(p, M::Sta, TMP);
    p.label("bvm_falling_project")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    for word in [0, 2] {
        ax(p, M::Lda, 0x80 + word);
        ax(p, M::Sta, 0x1c + word);
    }
    ax(p, M::Lda, 0x84);
    ax(p, M::Sta, 0x20);
    ax(p, M::Lda, 0x86);
    implied(p, M::Clc);
    ax(p, M::Adc, 0x94);
    ax(p, M::Sta, 0x22);
    dp(p, M::Lda, TMP);
    ax(p, M::Sta, 0x40);
    dp(p, M::Sta, PCP);
    imm16(p, M::Lda, 1);
    ax(p, M::Sta, 0x5e);
    m8(p);
    imm8(p, M::Lda, 0x0b);
    dp(p, M::Sta, AXIS);
    implied(p, M::Sec);
    implied(p, M::Rtl);
    Ok(())
}
