use super::*;
pub(crate) const OFFSET: usize = 0x700;
const RESERVE: usize = 0x100;
const CHILDREN: [u32; 4] = [0x492aec, 0x492b18, 0x492b44, 0x492b70];
const _: () = assert!(OFFSET + RESERVE == 0x800);
pub(super) fn entry(layout: &CartridgeLayout) -> Result<u32> {
    layout.cpu_address(layout.section("guardian_code")?.offset + OFFSET)
}
pub(crate) fn report(layout: &CartridgeLayout) -> Result<serde_json::Value> {
    Ok(serde_json::json!(
        { "children" : CHILDREN, "source_sha256" : common_radial::SOURCE_HASH,
        "helper_entry" : entry(layout) ?, "helper_bytes" : code(layout) ?.bytes()
        .len(), "helper_offset" : OFFSET, "helper_reserve" : RESERVE,
        "private_object_fields" : [], "minimum_jump_budget" : 2,
        "ordered_rng_divisors" : [16384, 8], "resume_pc" : 0x492bc0, "fallback" :
        "original opcode b7" }
    ))
}
pub(crate) fn code(layout: &CartridgeLayout) -> Result<w65c816::AssembledProgram> {
    let rules = crate::sfc_battle::code(layout)?;
    let target = |name: &str| -> Result<u32> {
        Ok(rules
            .label_location(name)
            .ok_or("Missing common birth target")?
            .to_u32())
    };
    let mut assembler = Assembler::new();
    let p = &mut assembler;
    let reject = |p: &mut Assembler, accepted: M, label: &str| {
        br(p, accepted, label);
        p.emit_label_ref(M::Brl, A::Relative16, "fallback");
        p.label(label).assume_mode(CpuMode::NATIVE_M16_X16);
    };
    p.assume_mode(CpuMode::NATIVE_M8_X16);
    dp(p, M::Ldx, OBJ);
    m16(p);
    ax(p, M::Lda, 0x42);
    imm16(p, M::Cmp, 0x49);
    reject(p, M::Beq, "bank");
    ax(p, M::Lda, 0x40);
    for pc in CHILDREN {
        imm16(p, M::Cmp, pc as u16);
        br(p, M::Beq, "child");
    }
    p.emit_label_ref(M::Brl, A::Relative16, "fallback");
    p.label("child").assume_mode(CpuMode::NATIVE_M16_X16);
    dp(p, M::Lda, BUDGET);
    imm16(p, M::Cmp, 2);
    reject(p, M::Bcs, "budget");
    cword(p, 4);
    ax(p, M::Sta, 0x8c);
    ax(p, M::Sta, 0x8e);
    cword(p, 32);
    ax(p, M::Sta, 0x24);
    cword(p, 34);
    ax(p, M::Sta, 0x26);
    dp(p, M::Sta, TMP + 2);
    let random = |p: &mut Assembler, divisor: u8| -> Result<()> {
        m8(p);
        implied(p, M::Phb);
        implied(p, M::Phd);
        m16(p);
        imm16(p, M::Lda, 0);
        implied(p, M::Tcd);
        m8(p);
        implied(p, M::Pha);
        implied(p, M::Plb);
        imm8(p, M::Lda, divisor);
        emit(
            p,
            M::Jsl,
            A::AbsoluteLong,
            O::Long(target("encounter_rng_far")?),
        );
        implied(p, M::Pld);
        implied(p, M::Plb);
        dp(p, M::Ldx, OBJ);
        m16(p);
        Ok(())
    };
    random(p, 0)?;
    emit(
        p,
        M::Lda,
        A::AbsoluteLong,
        O::Long(u32::from(crate::sfc_encounter::SEED + 2)),
    );
    imm16(p, M::And, 0x3fff);
    implied(p, M::Clc);
    ax(p, M::Adc, 0x8c);
    ax(p, M::Sta, 0x8c);
    ax(p, M::Sta, 0x8e);
    ax(p, M::Stz, 0);
    imm16(p, M::Lda, 0x4000);
    ax(p, M::Sta, 2);
    imm16(p, M::Lda, 2);
    ax(p, M::Sta, 0x2c);
    ax(p, M::Stz, 0x2e);
    random(p, 8)?;
    imm16(p, M::And, 0xff);
    abs(p, M::Sta, crate::sfc_battle_act::RANDOM);
    abs(p, M::Sta, crate::sfc_battle::DIVIDEND);
    implied(p, M::Clc);
    imm16(p, M::Adc, 4);
    ax(p, M::Sta, 0x58);
    ax(p, M::Stz, 0x5a);
    imm16(p, M::Lda, 8);
    abs(p, M::Sta, crate::sfc_battle::DIVISOR);
    emit(p, M::Lda, A::AbsoluteLong, O::Long(0x4214));
    abs(p, M::Sta, crate::sfc_battle::QUOTIENT);
    imm16(p, M::Lda, 4);
    dp(p, M::Sta, TMP);
    imm16(p, M::Lda, 24);
    dp(p, M::Sta, COUNT);
    dp(p, M::Dec, BUDGET);
    imm16(p, M::Lda, 0x2bc0);
    ax(p, M::Sta, 0x40);
    dp(p, M::Sta, PCP);
    m8(p);
    emit(p, M::Jml, A::AbsoluteLong, O::Long(target("bvm_next")?));
    p.label("fallback").assume_mode(CpuMode::NATIVE_M16_X16);
    imm16(p, M::Ldx, 2 * 0xb7);
    m8(p);
    emit(p, M::Jml, A::AbsoluteLong, O::Long(target("bvm_op_b7")?));
    let at = entry(layout)?;
    let code = crate::sfc_width_check::assemble(
        p,
        w65c816::CodeLocation::new((at >> 16) as u8, at as u16),
        CpuMode::NATIVE_M8_X16,
    )?;
    if code.bytes().len() > RESERVE {
        return Err("Common birth exceeds guardian reserve".into());
    }
    Ok(code)
}
