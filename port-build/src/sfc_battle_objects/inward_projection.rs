use super::*;
pub(crate) const OFFSET: usize = 0xc80;
const RESERVE: usize = 0x180;
const _: () = assert!(OFFSET + RESERVE <= falling_density::OFFSET);
pub(super) fn entry(layout: &CartridgeLayout) -> Result<u32> {
    layout.cpu_address(layout.section("guardian_code")?.offset + OFFSET)
}
pub(super) fn report(layout: &CartridgeLayout) -> Result<serde_json::Value> {
    let code = code(layout)?;
    Ok(serde_json::json!(
        { "helper_entry" : entry(layout) ?, "helper_bytes" : code.bytes().len(),
        "helper_offset" : OFFSET, "helper_reserve" : RESERVE, "private_object_fields"
        : [0x68, 0x72], "source_range" : [0x46be24, 0x46bee0], "source_sha256" :
        "f8c4db551c4823ec1b7bf375b1b2a52bcf20bdddf70231c8322a455a0e0eb8c6",
        "cold_state" : "source-produced previous XY; fixed speed/angles/origin X",
        "hot_state" :
        "owned displacement and marker; original turn guards still apply", "release"
        : "complete object deletion clears private fields" }
    ))
}
pub(super) fn emit_wrappers(p: &mut Assembler, layout: &CartridgeLayout) -> Result<()> {
    p.label("bvm_inward_projection")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    emit(p, M::Jsl, A::AbsoluteLong, O::Long(entry(layout)?));
    implied(p, M::Rts);
    p.label("bvm_inward_projection_original")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    jsr(p, "bvm_2d_loaded");
    implied(p, M::Rtl);
    Ok(())
}
pub(crate) fn code(layout: &CartridgeLayout) -> Result<w65c816::AssembledProgram> {
    let rules = crate::sfc_battle::code(layout)?;
    let original = rules
        .label_location("bvm_inward_projection_original")
        .ok_or("Missing inward projection wrapper")?
        .to_u32();
    let mut assembler = Assembler::new();
    let p = &mut assembler;
    let branch = |p: &mut Assembler, taken: M, label: &str, next: &str| {
        let opposite = match taken {
            M::Beq => M::Bne,
            M::Bne => M::Beq,
            M::Bcc => M::Bcs,
            M::Bcs => M::Bcc,
            _ => unreachable!(),
        };
        br(p, opposite, next);
        p.emit_label_ref(M::Brl, A::Relative16, label);
        p.label(next).assume_mode(CpuMode::NATIVE_M16_X16);
    };
    let jump = |p: &mut Assembler, label: &str| {
        p.emit_label_ref(M::Brl, A::Relative16, label);
    };
    let compare = |p: &mut Assembler, field, value, next: &str| {
        ax(p, M::Lda, field);
        imm16(p, M::Cmp, value);
        branch(p, M::Bne, "fallback", next);
    };
    let project = |p: &mut Assembler| {
        m8(p);
        imm8(p, M::Lda, 0x0b);
        emit(p, M::Jsl, A::AbsoluteLong, O::Long(original));
        dp(p, M::Ldx, OBJ);
        m16(p);
    };
    p.assume_mode(CpuMode::NATIVE_M8_X16);
    dp(p, M::Ldx, OBJ);
    m16(p);
    ax(p, M::Lda, 0x70);
    imm16(p, M::Cmp, 0xa6a7);
    branch(p, M::Beq, "hot", "cold_entry");
    compare(p, 0x42, 0x46, "pc_bank");
    ax(p, M::Lda, 0x40);
    imm16(p, M::Cmp, 0xbe9c);
    branch(p, M::Beq, "pc_good", "pc_second");
    imm16(p, M::Cmp, 0xbec0);
    branch(p, M::Bne, "fallback", "pc_good");
    ax(p, M::Lda, 2);
    imm16(p, M::Bit, 0x1000);
    branch(p, M::Bne, "fallback", "unlinked");
    ax(p, M::Lda, 0x92);
    ax(p, M::Cmp, 0x94);
    branch(p, M::Bne, "fallback", "same_amplitude");
    imm16(p, M::Cmp, 101);
    branch(p, M::Bcs, "fallback", "amplitude_range");
    ax(p, M::Lda, 0x58);
    imm16(p, M::Cmp, 2);
    branch(p, M::Bcc, "fallback", "speed_lower");
    imm16(p, M::Cmp, 6);
    branch(p, M::Bcs, "fallback", "speed_upper");
    ax(p, M::Lda, 0x8c);
    ax(p, M::Cmp, 0x8e);
    branch(p, M::Bne, "fallback", "same_angle");
    compare(p, 0x84, 0, "origin_fraction");
    ax(p, M::Lda, 0x40);
    imm16(p, M::Cmp, 0xbec0);
    branch(p, M::Beq, "origin_down", "origin_up");
    compare(p, 0x86, 152, "origin_up_good");
    jump(p, "origin_good");
    p.label("origin_down");
    compare(p, 0x86, 156, "origin_good");
    jump(p, "cold");
    p.label("hot").assume_mode(CpuMode::NATIVE_M16_X16);
    for (pos, delta) in [(0x1c, 0x68), (0x20, 0x6c)] {
        implied(p, M::Clc);
        for k in [0, 2] {
            ax(p, M::Lda, pos + k);
            ax(p, M::Adc, delta + k);
            ax(p, M::Sta, pos + k);
        }
    }
    ax(p, M::Lda, 0x40);
    imm16(p, M::Cmp, 0xbec0);
    branch(p, M::Beq, "hot_down", "hot_up");
    ax(p, M::Lda, 0x22);
    implied(p, M::Sec);
    imm16(p, M::Sbc, 4);
    jump(p, "hot_y");
    p.label("hot_down");
    ax(p, M::Lda, 0x22);
    implied(p, M::Clc);
    imm16(p, M::Adc, 4);
    p.label("hot_y");
    ax(p, M::Sta, 0x22);
    jump(p, "publish");
    p.label("cold").assume_mode(CpuMode::NATIVE_M16_X16);
    ax(p, M::Stz, 0x70);
    for (pos, delta) in [(0x1c, 0x68), (0x20, 0x6c)] {
        for k in [0, 2] {
            ax(p, M::Lda, pos + k);
            ax(p, M::Sta, delta + k);
        }
    }
    project(p);
    for (pos, delta) in [(0x1c, 0x68), (0x20, 0x6c)] {
        implied(p, M::Sec);
        for k in [0, 2] {
            ax(p, M::Lda, pos + k);
            ax(p, M::Sbc, delta + k);
            ax(p, M::Sta, delta + k);
        }
    }
    ax(p, M::Lda, 0x40);
    imm16(p, M::Cmp, 0xbec0);
    branch(p, M::Beq, "cold_down", "cold_up");
    ax(p, M::Lda, 0x6e);
    implied(p, M::Clc);
    imm16(p, M::Adc, 4);
    jump(p, "cold_y");
    p.label("cold_down");
    ax(p, M::Lda, 0x6e);
    implied(p, M::Sec);
    imm16(p, M::Sbc, 4);
    p.label("cold_y");
    ax(p, M::Sta, 0x6e);
    imm16(p, M::Lda, 0xa6a7);
    ax(p, M::Sta, 0x70);
    p.label("publish");
    m8(p);
    imm8(p, M::Lda, 0);
    implied(p, M::Rtl);
    p.label("fallback").assume_mode(CpuMode::NATIVE_M16_X16);
    ax(p, M::Stz, 0x70);
    project(p);
    m8(p);
    imm8(p, M::Lda, 0);
    implied(p, M::Rtl);
    let at = entry(layout)?;
    let code = crate::sfc_width_check::assemble(
        p,
        w65c816::CodeLocation::new((at >> 16) as u8, at as u16),
        CpuMode::NATIVE_M8_X16,
    )?;
    if code.bytes().len() > RESERVE {
        return Err("Inward projection exceeds its guardian reservation".into());
    }
    Ok(code)
}
