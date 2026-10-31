use super::*;
const ENTRY: u32 = 0x4bca8c;
const OPCODE: u8 = 0x97;
const START: u32 = 0x4bc8f8;
const END: u32 = 0x4bcb84;
const HASH: &str = "a1edbb0de96b899463602c6ef1cb011d46ccfc6f4c2cdcf95d8e23864e9bbd27";
pub(crate) const OFFSET: usize = 0xe00;
pub(crate) const RESERVE: usize = 0x200;
pub(crate) fn entry(layout: &CartridgeLayout) -> Result<u32> {
    layout.cpu_address(layout.section("guardian_code")?.offset + OFFSET)
}
pub(super) fn report(layout: &CartridgeLayout) -> Result<serde_json::Value> {
    Ok(serde_json::json!(
        { "entry" : ENTRY, "opcode" : OPCODE, "source_start" : START, "source_end" :
        END, "source_sha256" : HASH, "visible_children_per_emitter" : 59,
        "allocated_children_per_emitter" : 140, "helper_entry" : entry(layout) ?,
        "helper_section" : "guardian_code", "helper_offset" : OFFSET,
        "helper_reserve" : RESERVE, "keep_final_child" : true,
        "private_object_fields" : [], "source_rng_divisors" :
        "48, 4[, 4[, 4]], 4; nonzero skips remaining weighted draws" }
    ))
}
pub(super) fn patch(out: &mut [u8], source: &[u8], commands: &[u32]) -> Result<()> {
    let range = (START - SOURCE_BASE) as usize..(END - SOURCE_BASE) as usize;
    let at = (ENTRY - SOURCE_BASE) as usize;
    if source.get(range).is_none_or(|b| crate::sha(b) != HASH)
        || !commands.contains(&ENTRY)
        || out.get(at) != Some(&0xc1)
        || commands
            .iter()
            .any(|pc| out.get((pc - SOURCE_BASE) as usize) == Some(&OPCODE))
    {
        return Err("Falling density source, copy entry or opcode changed".into());
    }
    out[at] = OPCODE;
    Ok(())
}
pub(super) fn emit_step(p: &mut Assembler, layout: &CartridgeLayout) -> Result<()> {
    p.label("bvm_op_97").assume_mode(CpuMode::NATIVE_M8_X16);
    emit(p, M::Jml, A::AbsoluteLong, O::Long(entry(layout)?));
    Ok(())
}
pub(crate) fn code(layout: &CartridgeLayout) -> Result<w65c816::AssembledProgram> {
    let rules = crate::sfc_battle::code(layout)?;
    let target = |name: &str| -> Result<u32> {
        Ok(rules
            .label_location(name)
            .ok_or("Missing falling density target")?
            .to_u32())
    };
    let mut assembler = Assembler::new();
    let p = &mut assembler;
    let far_branch = |p: &mut Assembler, opposite: M, label: &str, next: &str| {
        br(p, opposite, next);
        p.emit_label_ref(M::Brl, A::Relative16, label);
        p.label(next).assume_mode(CpuMode::NATIVE_M16_X16);
    };
    p.label("bvm_falling_density_far")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    dp(p, M::Ldx, OBJ);
    m16(p);
    ax(p, M::Lda, 0xa0);
    implied(p, M::Sec);
    imm16(p, M::Sbc, SOURCE_DELTA as u16);
    dp(p, M::Sta, TMP);
    ax(p, M::Lda, 0xa2);
    imm16(p, M::Sbc, (SOURCE_DELTA >> 16) as u16);
    far_branch(p, M::Beq, "keep", "parent_bank");
    dp(p, M::Lda, TMP);
    imm16(p, M::Cmp, POOL);
    far_branch(p, M::Bcs, "keep", "parent_lower");
    imm16(p, M::Cmp, POOL + OBJECTS * OBJECT_BYTES);
    far_branch(p, M::Bcc, "keep", "parent_upper");
    implied(p, M::Tax);
    ax(p, M::Lda, 0x42);
    imm16(p, M::Cmp, 0x4b);
    far_branch(p, M::Beq, "keep", "parent_pc_bank");
    ax(p, M::Lda, 0x40);
    for (i, pc) in [0xc9a4, 0xc9dc, 0xca14].into_iter().enumerate() {
        imm16(p, M::Cmp, pc);
        far_branch(p, M::Bne, "aligned", &format!("phase_one_{i}"));
    }
    imm16(p, M::Cmp, 0xca6c);
    far_branch(p, M::Beq, "keep", "phase_two");
    for (off, value, label) in [(0x3c, 0xca5c, "latch_low"), (0x3e, 0x4b, "latch_high")] {
        ax(p, M::Lda, off);
        imm16(p, M::Cmp, value);
        far_branch(p, M::Beq, "keep", label);
    }
    ax(p, M::Lda, 0x60);
    imm16(p, M::Cmp, 2);
    far_branch(p, M::Bcs, "keep", "count_lower");
    imm16(p, M::Cmp, 71);
    far_branch(p, M::Bcc, "keep", "count_upper");
    imm16(p, M::And, 3);
    imm16(p, M::Cmp, 1);
    far_branch(p, M::Bne, "keep", "count_omitted");
    p.label("aligned").assume_mode(CpuMode::NATIVE_M16_X16);
    implied(p, M::Txa);
    implied(p, M::Sec);
    imm16(p, M::Sbc, POOL);
    emit(p, M::Sta, A::AbsoluteLong, O::Long(0x4204));
    m8(p);
    imm8(p, M::Lda, OBJECT_BYTES as u8);
    emit(p, M::Sta, A::AbsoluteLong, O::Long(0x4206));
    for _ in 0..8 {
        implied(p, M::Nop);
    }
    m16(p);
    emit(p, M::Lda, A::AbsoluteLong, O::Long(0x4216));
    far_branch(p, M::Beq, "keep", "omit");
    m8(p);
    implied(p, M::Phb);
    imm8(p, M::Lda, 0);
    implied(p, M::Pha);
    implied(p, M::Plb);
    let rng = target("encounter_rng_far")?;
    let random = |p: &mut Assembler, divisor: u8| {
        imm8(p, M::Lda, divisor);
        emit(p, M::Jsl, A::AbsoluteLong, O::Long(rng));
    };
    random(p, 0);
    random(p, 4);
    br(p, M::Bne, "last_draw");
    random(p, 4);
    br(p, M::Bne, "last_draw");
    random(p, 0);
    p.label("last_draw").assume_mode(CpuMode::NATIVE_M8_X16);
    random(p, 0);
    implied(p, M::Plb);
    emit(p, M::Jml, A::AbsoluteLong, O::Long(target("bvm_op_00")?));
    p.label("keep").assume_mode(CpuMode::NATIVE_M16_X16);
    imm16(p, M::Lda, 2 * 0xc1);
    implied(p, M::Tax);
    m8(p);
    emit(p, M::Jml, A::AbsoluteLong, O::Long(target("bvm_op_c1")?));
    let at = entry(layout)?;
    let program = crate::sfc_width_check::assemble(
        p,
        w65c816::CodeLocation::new((at >> 16) as u8, at as u16),
        CpuMode::NATIVE_M8_X16,
    )?;
    if program.bytes().len() > RESERVE {
        return Err("Falling density helper exceeds its reserved guardian tail".into());
    }
    Ok(program)
}
