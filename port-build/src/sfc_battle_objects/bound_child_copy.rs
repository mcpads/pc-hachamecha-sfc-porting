use super::*;
const BULK: [u32; 8] = [
    0x4924e8, 0x49254c, 0x492634, 0x492694, 0x49276c, 0x4927cc, 0x4928a4, 0x492904,
];
const ORIGINS: [u32; 4] = [0x4925a0, 0x4926e8, 0x492820, 0x492958];
const BULK_HASH: &str = "ed787a4128c38103e3a40e51ccc2b00b62827e236dff97692a32e8469920783f";
const ORIGINS_HASH: &str = "180b9b2ff04348dfb72dbcab93e425b532917bbe69257aaa0c31e835c2a46249";
const OPCODE: u8 = 0x9a;
pub(crate) const OFFSET: usize = 0x960;
const RESERVE: usize = 0x1a0;
const _: () = assert!(OFFSET + RESERVE <= falling_setup::OFFSET);
fn entry(layout: &CartridgeLayout) -> Result<u32> {
    layout.cpu_address(layout.section("guardian_code")?.offset + OFFSET)
}
pub(super) fn report(layout: &CartridgeLayout) -> Result<serde_json::Value> {
    Ok(serde_json::json!(
        { "bulk_starts" : BULK, "bulk_bytes" : 28, "bulk_source_sha256" : BULK_HASH,
        "origin_starts" : ORIGINS, "origin_bytes" : 24, "origin_source_sha256" :
        ORIGINS_HASH, "opcode" : OPCODE, "helper_entry" : entry(layout) ?,
        "helper_offset" : OFFSET, "helper_reserve" : RESERVE, "fallback" :
        "opcode bd", "private_object_fields" : [], "binding" :
        "valid, unsplit, aligned 176-byte pool record" }
    ))
}
pub(super) fn patch(out: &mut [u8], source: &[u8], commands: &[u32]) -> Result<()> {
    if commands
        .iter()
        .any(|pc| out.get((pc - SOURCE_BASE) as usize) == Some(&OPCODE))
    {
        return Err("Bound-child copy opcode already used".into());
    }
    for (starts, size, hash) in [(&BULK[..], 28, BULK_HASH), (&ORIGINS[..], 24, ORIGINS_HASH)] {
        for &start in starts {
            let at = (start - SOURCE_BASE) as usize;
            if source
                .get(at..at + size)
                .is_none_or(|b| crate::sha(b) != hash)
                || !commands.contains(&start)
                || !commands.contains(&(start + size as u32))
                || out.get(at) != Some(&0xbd)
            {
                return Err("Bound-child copy source or boundaries changed".into());
            }
            out[at] = OPCODE;
        }
    }
    Ok(())
}
pub(super) fn emit_step(p: &mut Assembler, layout: &CartridgeLayout) -> Result<()> {
    p.label("bvm_op_9a").assume_mode(CpuMode::NATIVE_M8_X16);
    emit(p, M::Jml, A::AbsoluteLong, O::Long(entry(layout)?));
    Ok(())
}
pub(crate) fn code(layout: &CartridgeLayout) -> Result<w65c816::AssembledProgram> {
    let rules = crate::sfc_battle::code(layout)?;
    let target = |name: &str| -> Result<u32> {
        Ok(rules
            .label_location(name)
            .ok_or("Missing bound-child copy target")?
            .to_u32())
    };
    let mut assembler = Assembler::new();
    let p = &mut assembler;
    let ind16 = |p: &mut Assembler, m, at| {
        emit(p, m, A::DirectPageIndirectIndexedY, O::Byte(at));
    };
    let reject = |p: &mut Assembler, accepted: M, next: &str| {
        br(p, accepted, next);
        p.emit_label_ref(M::Brl, A::Relative16, "fallback");
        p.label(next).assume_mode(CpuMode::NATIVE_M16_X16);
    };
    p.assume_mode(CpuMode::NATIVE_M8_X16);
    dp(p, M::Ldx, OBJ);
    m16(p);
    ax(p, M::Lda, 0x42);
    imm16(p, M::Cmp, 0x49);
    reject(p, M::Beq, "source_bank");
    m8(p);
    imm8(p, M::Lda, 0x80);
    dp(p, M::Sta, OFF);
    imm8(p, M::Lda, 0xc0);
    emit(
        p,
        M::Jsl,
        A::AbsoluteLong,
        O::Long(target("bvm_falling_setup_at_far")?),
    );
    dp(p, M::Lda, REFFAST);
    m16(p);
    imm16(p, M::And, 0xff);
    reject(p, M::Bne, "valid_binding");
    dp(p, M::Lda, REFBASE);
    imm16(p, M::Cmp, POOL);
    reject(p, M::Bcs, "pool_lower");
    imm16(p, M::Cmp, POOL + OBJECTS * OBJECT_BYTES);
    reject(p, M::Bcc, "pool_upper");
    implied(p, M::Sec);
    imm16(p, M::Sbc, POOL);
    for (i, multiple) in (0..9).rev().map(|i| (i, OBJECT_BYTES << i)) {
        let next = format!("remainder_{i}");
        imm16(p, M::Cmp, multiple);
        br(p, M::Bcc, &next);
        imm16(p, M::Sbc, multiple);
        p.label(next).assume_mode(CpuMode::NATIVE_M16_X16);
    }
    imm16(p, M::Cmp, 0);
    reject(p, M::Beq, "aligned_binding");
    ax(p, M::Lda, 0x40);
    for start in BULK {
        imm16(p, M::Cmp, start as u16);
        br(p, M::Beq, "bulk");
    }
    for start in ORIGINS {
        imm16(p, M::Cmp, start as u16);
        br(p, M::Beq, "origins");
    }
    p.emit_label_ref(M::Brl, A::Relative16, "fallback");
    p.label("bulk").assume_mode(CpuMode::NATIVE_M16_X16);
    for field in [0x80, 0x82, 0x84, 0x86, 0x92, 0x94, 0x96, 0x8c, 0x8e] {
        ax(p, M::Lda, field);
        imm16(p, M::Ldy, field);
        ind16(p, M::Sta, PTR);
    }
    m8(p);
    emit(
        p,
        M::Jml,
        A::AbsoluteLong,
        O::Long(common_blink_density::mark_entry(layout)),
    );
    p.label("origins").assume_mode(CpuMode::NATIVE_M16_X16);
    for (src, dest) in [(0x1c, 0x80), (0x1e, 0x82), (0x20, 0x84), (0x22, 0x86)] {
        ax(p, M::Lda, src);
        imm16(p, M::Ldy, dest);
        ind16(p, M::Sta, PTR);
    }
    for field in [0x82, 0x86] {
        imm16(p, M::Ldy, field);
        ind16(p, M::Lda, PTR);
        implied(p, M::Clc);
        imm16(p, M::Adc, 16);
        ind16(p, M::Sta, PTR);
    }
    m8(p);
    emit(
        p,
        M::Jml,
        A::AbsoluteLong,
        O::Long(common_density::mark_entry(layout)),
    );
    p.label("fallback").assume_mode(CpuMode::NATIVE_M16_X16);
    m8(p);
    emit(p, M::Jml, A::AbsoluteLong, O::Long(target("bvm_op_bd")?));
    let at = entry(layout)?;
    let program = crate::sfc_width_check::assemble(
        p,
        w65c816::CodeLocation::new((at >> 16) as u8, at as u16),
        CpuMode::NATIVE_M8_X16,
    )?;
    if program.bytes().len() > RESERVE {
        return Err(format!(
            "Bound-child copy uses {} bytes, reserve {RESERVE}",
            program.bytes().len()
        )
        .into());
    }
    Ok(program)
}
