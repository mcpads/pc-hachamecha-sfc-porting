use super::*;
const START: u32 = 0x4bca90;
const END: u32 = 0x4bcb0c;
const HASH: &str = "610378836aac5ba3f09964b3c5b9bf99b5baabde9a875cd1da5c59803f7bc125";
const OPCODE: u8 = 0x99;
pub(crate) const OFFSET: usize = 0xb00;
const RESERVE: usize = 0x180;
const _: () = assert!(OFFSET + RESERVE <= inward_projection::OFFSET);
fn entry(layout: &CartridgeLayout) -> Result<u32> {
    layout.cpu_address(layout.section("guardian_code")?.offset + OFFSET)
}
pub(super) fn report(layout: &CartridgeLayout) -> Result<serde_json::Value> {
    Ok(serde_json::json!(
        { "start" : START, "end" : END, "source_sha256" : HASH, "opcode" : OPCODE,
        "helper_entry" : entry(layout) ?, "helper_offset" : OFFSET, "helper_reserve"
        : RESERVE, "minimum_jump_budget" : 3, "fallback" : "opcode e1",
        "private_object_fields" : [], "bound_reads" : [0x24, 0x1c, 0x20],
        "preserved_list_advance" : 0x48, "pc_publication" :
        "before each bound origin read and successor dispatch" }
    ))
}
pub(super) fn patch(out: &mut [u8], source: &[u8], commands: &[u32]) -> Result<()> {
    let range = (START - SOURCE_BASE) as usize..(END - SOURCE_BASE) as usize;
    if source
        .get(range.clone())
        .is_none_or(|b| crate::sha(b) != HASH)
        || !commands.contains(&START)
        || !commands.contains(&END)
        || out.get(range.start) != Some(&0xe1)
        || commands
            .iter()
            .any(|pc| out.get((pc - SOURCE_BASE) as usize) == Some(&OPCODE))
    {
        return Err("Falling setup source, boundaries or opcode changed".into());
    }
    out[range.start] = OPCODE;
    Ok(())
}
pub(super) fn emit_step(p: &mut Assembler, layout: &CartridgeLayout) -> Result<()> {
    p.label("bvm_op_99").assume_mode(CpuMode::NATIVE_M8_X16);
    emit(p, M::Jml, A::AbsoluteLong, O::Long(entry(layout)?));
    p.label("bvm_falling_setup_at_far")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    jsr(p, "bvm_at");
    implied(p, M::Rtl);
    Ok(())
}
pub(crate) fn code(layout: &CartridgeLayout) -> Result<w65c816::AssembledProgram> {
    let rules = crate::sfc_battle::code(layout)?;
    let target = |name: &str| -> Result<u32> {
        Ok(rules
            .label_location(name)
            .ok_or("Missing falling setup target")?
            .to_u32())
    };
    let mut assembler = Assembler::new();
    let p = &mut assembler;
    let reject = |p: &mut Assembler, accepted: M, next: &str| {
        br(p, accepted, next);
        p.emit_label_ref(M::Brl, A::Relative16, "fallback");
        p.label(next).assume_mode(CpuMode::NATIVE_M16_X16);
    };
    let indirect = |p: &mut Assembler, m| {
        emit(p, m, A::DirectPageIndirectIndexedY, O::Byte(PTR));
    };
    p.assume_mode(CpuMode::NATIVE_M8_X16);
    dp(p, M::Ldx, OBJ);
    m16(p);
    for (field, value, label) in [
        (0x40, START as u16, "pc_low"),
        (0x42, (START >> 16) as u16, "pc_high"),
    ] {
        ax(p, M::Lda, field);
        imm16(p, M::Cmp, value);
        reject(p, M::Beq, label);
    }
    dp(p, M::Lda, BUDGET);
    imm16(p, M::Cmp, 3);
    reject(p, M::Bcs, "budget");
    let at = target("bvm_falling_setup_at_far")?;
    let read_bound = |p: &mut Assembler, offset: u8, label: &str| {
        m8(p);
        imm8(p, M::Lda, offset);
        dp(p, M::Sta, OFF);
        imm8(p, M::Lda, 0xc0);
        emit(p, M::Jsl, A::AbsoluteLong, O::Long(at));
        m16(p);
        dp(p, M::Stz, TMP);
        dp(p, M::Stz, TMP + 2);
        m8(p);
        dp(p, M::Lda, VALID);
        br(p, M::Beq, label);
        m16(p);
        indirect(p, M::Lda);
        dp(p, M::Sta, TMP);
        implied(p, M::Iny);
        implied(p, M::Iny);
        indirect(p, M::Lda);
        dp(p, M::Sta, TMP + 2);
        m8(p);
        p.label(label).assume_mode(CpuMode::NATIVE_M8_X16);
        m16(p);
        dp(p, M::Ldx, OBJ);
    };
    read_bound(p, 0x24, "direction_read");
    dp(p, M::Dec, BUDGET);
    dp(p, M::Lda, TMP);
    br(p, M::Bne, "assign");
    dp(p, M::Lda, TMP + 2);
    imm16(p, M::Cmp, 2);
    br(p, M::Bne, "assign");
    dp(p, M::Dec, BUDGET);
    p.label("assign").assume_mode(CpuMode::NATIVE_M16_X16);
    for (field, value) in initializers::placed_words(
        layout,
        0x4bcabc,
        0x4bcb04,
        "8ee54dd37302712d55c9c53086aab3550eb3d1646edd9b50213852b2ea1add0a",
    )? {
        if value == 0 {
            ax(p, M::Stz, field);
        } else {
            imm16(p, M::Lda, value);
            ax(p, M::Sta, field);
        }
    }
    imm16(p, M::Lda, 0x48);
    dp(p, M::Sta, COUNT);
    for (source, dest, pc, label) in [
        (0x1c, 0x80, 0xcb04, "origin_x_read"),
        (0x20, 0x84, 0xcb08, "origin_y_read"),
    ] {
        imm16(p, M::Lda, pc);
        ax(p, M::Sta, 0x40);
        dp(p, M::Sta, PCP);
        read_bound(p, source, label);
        ax(p, M::Lda, dest);
        implied(p, M::Clc);
        dp(p, M::Adc, TMP);
        ax(p, M::Sta, dest);
        ax(p, M::Lda, dest + 2);
        dp(p, M::Adc, TMP + 2);
        ax(p, M::Sta, dest + 2);
    }
    imm16(p, M::Lda, END as u16);
    ax(p, M::Sta, 0x40);
    dp(p, M::Sta, PCP);
    m8(p);
    emit(p, M::Jml, A::AbsoluteLong, O::Long(target("bvm_next")?));
    p.label("fallback").assume_mode(CpuMode::NATIVE_M16_X16);
    m8(p);
    emit(p, M::Jml, A::AbsoluteLong, O::Long(target("bvm_op_e1")?));
    let at = entry(layout)?;
    let program = crate::sfc_width_check::assemble(
        p,
        w65c816::CodeLocation::new((at >> 16) as u8, at as u16),
        CpuMode::NATIVE_M8_X16,
    )?;
    if program.bytes().len() > RESERVE {
        return Err("Falling setup exceeds its reserved guardian gap".into());
    }
    Ok(program)
}
