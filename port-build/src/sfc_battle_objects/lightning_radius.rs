use super::*;
const START: u32 = 0x4948a4;
const END: u32 = START + 16;
const OPCODE: u8 = 0x9b;
const TURNS: u16 = 12;
const SEQUENCE: [u8; 16] = [
    0xbf, 0xa1, 0x92, 0x96, 0xbf, 0xa1, 0x94, 0x96, 0x06, 0, 12, 0, 0xa4, 0x48, 0x49, 0,
];
pub(super) fn patch(image: &mut [u8], source: &[u8], commands: &[u32]) -> Result<()> {
    let at = (START - SOURCE_BASE) as usize;
    if image.get(at..at + SEQUENCE.len()) != Some(SEQUENCE.as_slice())
        || [START, START + 4, START + 8, END]
            .iter()
            .any(|pc| !commands.contains(pc))
    {
        return Err("Radial initialization loop or command boundaries changed".into());
    }
    if source
        .get((0x4947a4 - SOURCE_BASE) as usize..(0x4949f0 - SOURCE_BASE) as usize)
        .is_none_or(|b| {
            crate::sha(b) != "de921795bb6d09558fd3464a0f6259696427353a8c6ffaa567f3cbfdd37dfda9"
        })
    {
        return Err("Lightning source family changed".into());
    }
    image[at] = OPCODE;
    Ok(())
}
const OFFSET: usize = 0x6f00;
const RESERVE: usize = 0x100;
pub(super) fn entry(layout: &CartridgeLayout) -> u32 {
    let bank = if layout.mapping() == sfc_builder::cartridge::Mapping::HiRom {
        0x80_0000
    } else {
        0
    };
    bank + 0x8000 + OFFSET as u32
}
pub(super) fn report(layout: &CartridgeLayout) -> Result<serde_json::Value> {
    Ok(serde_json::json!(
        { "start" : START, "end" : END, "turns" : TURNS, "opcode" : OPCODE,
        "helper_entry" : entry(layout), "helper_bytes" : code(layout) ?.bytes()
        .len(), "helper_reserve" : RESERVE, "full_source_pc_guard" :
        "shared 9b dispatcher", "private_object_fields" : [], "fallback" :
        "original bf; interior and budget-resume entries retained" }
    ))
}
pub(crate) fn place_main(bytes: &mut Vec<u8>, layout: &CartridgeLayout) -> Result<()> {
    if bytes.len() > OFFSET {
        return Err("Main code overlaps lightning radius helper".into());
    }
    let helper = code(layout)?;
    bytes.resize(OFFSET, 0);
    bytes.extend_from_slice(helper.bytes());
    Ok(())
}
fn code(layout: &CartridgeLayout) -> Result<w65c816::AssembledProgram> {
    let rules = crate::sfc_battle::code(layout)?;
    let target = |name: &str| -> Result<u32> {
        Ok(rules
            .label_location(name)
            .ok_or("Missing lightning continuation")?
            .to_u32())
    };
    let mut assembler = Assembler::new();
    let p = &mut assembler;
    p.assume_mode(CpuMode::NATIVE_M8_X16);
    dp(p, M::Ldx, OBJ);
    m16(p);
    dp(p, M::Lda, BUDGET);
    imm16(p, M::Cmp, TURNS);
    br(p, M::Bcc, "bvm_radius_fallback");
    ax(p, M::Lda, 0x3c);
    imm16(p, M::Cmp, START as u16);
    br(p, M::Bne, "bvm_radius_fresh");
    ax(p, M::Lda, 0x3e);
    imm16(p, M::Cmp, (START >> 16) as u16);
    br(p, M::Bne, "bvm_radius_fresh");
    p.label("bvm_radius_fallback")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    m8(p);
    emit(p, M::Jml, A::AbsoluteLong, O::Long(target("bvm_op_bf")?));
    p.label("bvm_radius_fresh")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    ax(p, M::Lda, 0x96);
    dp(p, M::Sta, TMP);
    acc(p, M::Asl);
    implied(p, M::Clc);
    dp(p, M::Adc, TMP);
    for _ in 0..2 {
        acc(p, M::Asl);
    }
    implied(p, M::Tay);
    for field in [0x92, 0x94] {
        implied(p, M::Tya);
        implied(p, M::Clc);
        ax(p, M::Adc, field);
        ax(p, M::Sta, field);
    }
    ax(p, M::Stz, 0x3c);
    ax(p, M::Stz, 0x3e);
    ax(p, M::Stz, 0x60);
    dp(p, M::Lda, BUDGET);
    implied(p, M::Sec);
    imm16(p, M::Sbc, TURNS - 1);
    dp(p, M::Sta, BUDGET);
    imm16(p, M::Ldy, 6);
    m8(p);
    advance(p, (END - START) as u16);
    emit(p, M::Jml, A::AbsoluteLong, O::Long(target("bvm_next")?));
    let at = entry(layout);
    let code = crate::sfc_width_check::assemble(
        p,
        w65c816::CodeLocation::new((at >> 16) as u8, at as u16),
        CpuMode::NATIVE_M8_X16,
    )?;
    if code.bytes().len() > RESERVE {
        return Err("Lightning radius helper exceeds reserve".into());
    }
    Ok(code)
}
