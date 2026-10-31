use super::*;
const START: u32 = 0x4bcb0c;
const END: u32 = 0x4bcb50;
const HASH: &str = "3748e2688f828eb3281fc29842270a5c9d50cc6522b836099ebe60ed9d572bbc";
const OPCODE: u8 = 0x98;
pub(crate) const OFFSET: usize = 0x600;
const RESERVE: usize = 0x100;
const _: () = assert!(OFFSET + RESERVE <= common_birth::OFFSET);
fn entry(layout: &CartridgeLayout) -> Result<u32> {
    layout.cpu_address(layout.section("guardian_code")?.offset + OFFSET)
}
pub(super) fn report(layout: &CartridgeLayout) -> Result<serde_json::Value> {
    Ok(serde_json::json!(
        { "start" : START, "end" : END, "source_sha256" : HASH, "opcode" : OPCODE,
        "helper_entry" : entry(layout) ?, "helper_offset" : OFFSET, "helper_reserve"
        : RESERVE, "private_object_fields" : [], "minimum_jump_budget" : 2,
        "fallback" : "opcode 2b", "ordered_rng_divisors" : "48, 4[, 4[, 4]], 4",
        "scratch" : "operand pointer, arithmetic operands/results, operand offset" }
    ))
}
pub(super) fn patch(out: &mut [u8], source: &[u8], commands: &[u32]) -> Result<()> {
    let range = (START - SOURCE_BASE) as usize..(END - SOURCE_BASE) as usize;
    if source
        .get(range.clone())
        .is_none_or(|bytes| crate::sha(bytes) != HASH)
        || !commands.contains(&START)
        || !commands.contains(&END)
        || out.get(range.start) != Some(&0x2b)
        || commands
            .iter()
            .any(|pc| out.get((pc - SOURCE_BASE) as usize) == Some(&OPCODE))
    {
        return Err("Falling birth source, boundaries or opcode changed".into());
    }
    out[range.start] = OPCODE;
    Ok(())
}
pub(super) fn emit_step(p: &mut Assembler, layout: &CartridgeLayout) -> Result<()> {
    p.label("bvm_op_98").assume_mode(CpuMode::NATIVE_M8_X16);
    emit(p, M::Jml, A::AbsoluteLong, O::Long(entry(layout)?));
    Ok(())
}
pub(crate) fn code(layout: &CartridgeLayout) -> Result<w65c816::AssembledProgram> {
    let rules = crate::sfc_battle::code(layout)?;
    let target = |name: &str| -> Result<u32> {
        Ok(rules
            .label_location(name)
            .ok_or("Missing falling birth target")?
            .to_u32())
    };
    let mut assembler = Assembler::new();
    let p = &mut assembler;
    let reject = |p: &mut Assembler, accepted: M, next: &str| {
        br(p, accepted, next);
        p.emit_label_ref(M::Brl, A::Relative16, "fallback");
        p.label(next).assume_mode(CpuMode::NATIVE_M16_X16);
    };
    p.label("bvm_falling_birth_far")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    dp(p, M::Ldx, OBJ);
    m16(p);
    for (field, value, label) in [
        (0x40, START as u16, "source_low"),
        (0x42, (START >> 16) as u16, "source_high"),
    ] {
        ax(p, M::Lda, field);
        imm16(p, M::Cmp, value);
        reject(p, M::Beq, label);
    }
    dp(p, M::Lda, BUDGET);
    imm16(p, M::Cmp, 2);
    reject(p, M::Bcs, "budget");
    let rng = target("encounter_rng_far")?;
    let random = |p: &mut Assembler, divisor: u8| {
        m8(p);
        implied(p, M::Phb);
        imm8(p, M::Lda, 0);
        implied(p, M::Pha);
        implied(p, M::Plb);
        imm8(p, M::Lda, divisor);
        emit(p, M::Jsl, A::AbsoluteLong, O::Long(rng));
        implied(p, M::Plb);
        m16(p);
        imm16(p, M::And, 0xff);
        dp(p, M::Ldx, OBJ);
        imm16(p, M::Cmp, 0);
    };
    random(p, 48);
    implied(p, M::Clc);
    ax(p, M::Adc, 0x82);
    ax(p, M::Sta, 0x82);
    random(p, 4);
    br(p, M::Bne, "weighted_done");
    random(p, 4);
    br(p, M::Bne, "weighted_done");
    random(p, 4);
    br(p, M::Bra, "add_frame");
    p.label("weighted_done")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    dp(p, M::Dec, BUDGET);
    p.label("add_frame").assume_mode(CpuMode::NATIVE_M16_X16);
    implied(p, M::Clc);
    ax(p, M::Adc, 0x28);
    ax(p, M::Sta, 0x28);
    ax(p, M::Lda, 0x2a);
    imm16(p, M::Adc, 0);
    ax(p, M::Sta, 0x2a);
    random(p, 4);
    abs(p, M::Sta, crate::sfc_battle_act::RANDOM);
    abs(p, M::Sta, crate::sfc_battle::DIVIDEND);
    implied(p, M::Clc);
    imm16(p, M::Adc, 2);
    ax(p, M::Sta, 0x58);
    ax(p, M::Stz, 0x5a);
    imm16(p, M::Lda, 4);
    abs(p, M::Sta, crate::sfc_battle::DIVISOR);
    emit(p, M::Lda, A::AbsoluteLong, O::Long(0x4214));
    abs(p, M::Sta, crate::sfc_battle::QUOTIENT);
    imm16(p, M::Lda, END as u16);
    ax(p, M::Sta, 0x40);
    dp(p, M::Sta, PCP);
    m8(p);
    emit(p, M::Jml, A::AbsoluteLong, O::Long(target("bvm_next")?));
    p.label("fallback").assume_mode(CpuMode::NATIVE_M16_X16);
    m8(p);
    emit(p, M::Jml, A::AbsoluteLong, O::Long(target("bvm_op_2b")?));
    let at = entry(layout)?;
    let program = crate::sfc_width_check::assemble(
        p,
        w65c816::CodeLocation::new((at >> 16) as u8, at as u16),
        CpuMode::NATIVE_M8_X16,
    )?;
    if program.bytes().len() > RESERVE {
        return Err("Falling birth helper exceeds its reserved guardian gap".into());
    }
    Ok(program)
}
