use super::*;
const CHILDREN: [u32; 4] = [0x49298c, 0x49299c, 0x4929ac, 0x4929bc];
const PARENTS: [u32; 8] = [
    0x4924e8, 0x49254c, 0x492634, 0x492694, 0x49276c, 0x4927cc, 0x4928a4, 0x492904,
];
const MARKER: u16 = 0xd35d;
const OFFSET: usize = 0x6980;
const MARK_OFFSET: usize = 0x6b00;
pub(super) fn entry(layout: &CartridgeLayout) -> u32 {
    let bank = if layout.mapping() == sfc_builder::cartridge::Mapping::HiRom {
        0x80_0000
    } else {
        0
    };
    bank + 0x8000 + OFFSET as u32
}
pub(super) fn mark_entry(layout: &CartridgeLayout) -> u32 {
    entry(layout) + (MARK_OFFSET - OFFSET) as u32
}
pub(super) fn report(layout: &CartridgeLayout) -> Result<serde_json::Value> {
    Ok(serde_json::json!(
        { "children" : CHILDREN, "parents" : PARENTS, "helper_entry" : entry(layout),
        "mark_entry" : mark_entry(layout), "helper_bytes" : child_code(layout)
        ?.bytes().len(), "mark_bytes" : mark_code(layout) ?.bytes().len(),
        "source_sha256" : common_radial::SOURCE_HASH, "marker_field" : [0x68, 0x6a],
        "marker" : MARKER, "kept_ordinals" : "even 0..22 and final 23",
        "omitted_ordinals" : "odd 1..21", "rng_advances_per_omission" : 0, "policy" :
        "decorative blink density; parent timing, both visibility phases and final linkage retained"
        }
    ))
}
pub(super) fn patch(out: &mut [u8], source: &[u8], commands: &[u32]) -> Result<()> {
    if source
        .get((0x492474 - SOURCE_BASE) as usize..(0x492c18 - SOURCE_BASE) as usize)
        .is_none_or(|b| crate::sha(b) != common_radial::SOURCE_HASH)
    {
        return Err("Blink density source changed".into());
    }
    for pc in CHILDREN {
        let at = (pc - SOURCE_BASE) as usize;
        if !commands.contains(&pc)
            || out.get(at) != Some(&0xb4)
            || source.get(at..at + 4) != Some(&[0x12, 0x80, 0x24, 0])
        {
            return Err("Common blink birth entry changed".into());
        }
        out[at] = 0x9b;
    }
    Ok(())
}
pub(crate) fn place_main(bytes: &mut Vec<u8>, layout: &CartridgeLayout) -> Result<()> {
    let child = child_code(layout)?;
    let mark = mark_code(layout)?;
    if bytes.len() < OFFSET + 0x80
        || bytes.len() > MARK_OFFSET
        || bytes[OFFSET..OFFSET + 0x80].iter().any(|byte| *byte != 0)
    {
        return Err("Blink density overlaps preceding main placement".into());
    }
    bytes[OFFSET..OFFSET + child.bytes().len()].copy_from_slice(child.bytes());
    bytes.resize(MARK_OFFSET, 0);
    bytes.extend_from_slice(mark.bytes());
    Ok(())
}
fn child_code(layout: &CartridgeLayout) -> Result<w65c816::AssembledProgram> {
    let rules = crate::sfc_battle::code(layout)?;
    let target = |name: &str| -> Result<u32> {
        Ok(rules
            .label_location(name)
            .ok_or("Missing density target")?
            .to_u32())
    };
    let mut assembler = Assembler::new();
    let p = &mut assembler;
    p.assume_mode(CpuMode::NATIVE_M8_X16);
    dp(p, M::Ldx, OBJ);
    m16(p);
    ax(p, M::Lda, 0x42);
    imm16(p, M::Cmp, 0x49);
    br(p, M::Bne, "keep");
    ax(p, M::Lda, 0x40);
    for pc in CHILDREN {
        imm16(p, M::Cmp, pc as u16);
        br(p, M::Beq, "child");
    }
    br(p, M::Bra, "keep");
    p.label("child").assume_mode(CpuMode::NATIVE_M16_X16);
    ax(p, M::Lda, 0x68);
    imm16(p, M::Cmp, MARKER);
    br(p, M::Bne, "keep");
    m8(p);
    emit(p, M::Jml, A::AbsoluteLong, O::Long(target("bvm_op_00")?));
    p.label("keep").assume_mode(CpuMode::NATIVE_M16_X16);
    imm16(p, M::Ldx, 2 * 0xb4);
    m8(p);
    emit(p, M::Jml, A::AbsoluteLong, O::Long(target("bvm_op_b4")?));
    let at = entry(layout);
    let code = crate::sfc_width_check::assemble(
        p,
        w65c816::CodeLocation::new((at >> 16) as u8, at as u16),
        CpuMode::NATIVE_M8_X16,
    )?;
    if code.bytes().len() > 0x80 {
        return Err("Blink density child exceeds reserve".into());
    }
    Ok(code)
}
fn mark_code(layout: &CartridgeLayout) -> Result<w65c816::AssembledProgram> {
    let rules = crate::sfc_battle::code(layout)?;
    let next = rules
        .label_location("bvm_next")
        .ok_or("Missing density continuation")?
        .to_u32();
    let mut assembler = Assembler::new();
    let p = &mut assembler;
    let read = |p: &mut Assembler, field| {
        imm16(p, M::Ldy, field);
        emit(p, M::Lda, A::DirectPageIndirectIndexedY, O::Byte(PTR));
    };
    let reject = |p: &mut Assembler, accepted: M, label: &str| {
        br(p, accepted, label);
        p.emit_label_ref(M::Brl, A::Relative16, "done");
        p.label(label).assume_mode(CpuMode::NATIVE_M16_X16);
    };
    p.assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    dp(p, M::Ldx, OBJ);
    ax(p, M::Lda, 0x96);
    imm16(p, M::Cmp, 23);
    reject(p, M::Bcc, "interior");
    imm16(p, M::And, 1);
    reject(p, M::Bne, "selected");
    read(p, 0x68);
    reject(p, M::Beq, "unmarked");
    read(p, 0x42);
    imm16(p, M::Cmp, 0x49);
    reject(p, M::Beq, "child_bank");
    ax(p, M::Lda, 0x40);
    for (i, pc) in PARENTS.into_iter().enumerate() {
        imm16(p, M::Cmp, pc as u16);
        br(p, M::Beq, &format!("parent_{}", i / 2));
    }
    p.emit_label_ref(M::Brl, A::Relative16, "done");
    for (i, pc) in CHILDREN.into_iter().enumerate() {
        p.label(format!("parent_{i}"))
            .assume_mode(CpuMode::NATIVE_M16_X16);
        read(p, 0x40);
        imm16(p, M::Cmp, pc as u16);
        reject(p, M::Beq, &format!("linked_{i}"));
        p.emit_label_ref(M::Brl, A::Relative16, "link");
    }
    p.label("link").assume_mode(CpuMode::NATIVE_M16_X16);
    read(p, 0xa0);
    implied(p, M::Sec);
    imm16(p, M::Sbc, SOURCE_DELTA as u16);
    implied(p, M::Pha);
    read(p, 0xa2);
    imm16(p, M::Sbc, (SOURCE_DELTA >> 16) as u16);
    br(p, M::Beq, "link_bank");
    implied(p, M::Pla);
    p.emit_label_ref(M::Brl, A::Relative16, "done");
    p.label("link_bank").assume_mode(CpuMode::NATIVE_M16_X16);
    implied(p, M::Pla);
    dp(p, M::Cmp, OBJ);
    reject(p, M::Beq, "mark");
    imm16(p, M::Lda, MARKER);
    imm16(p, M::Ldy, 0x68);
    emit(p, M::Sta, A::DirectPageIndirectIndexedY, O::Byte(PTR));
    p.label("done").assume_mode(CpuMode::NATIVE_M16_X16);
    m8(p);
    advance(p, 28);
    emit(p, M::Jml, A::AbsoluteLong, O::Long(next));
    let at = mark_entry(layout);
    let code = crate::sfc_width_check::assemble(
        p,
        w65c816::CodeLocation::new((at >> 16) as u8, at as u16),
        CpuMode::NATIVE_M8_X16,
    )?;
    if code.bytes().len() > 0x6c00 - MARK_OFFSET {
        return Err("Blink density marker exceeds reserve".into());
    }
    Ok(code)
}
