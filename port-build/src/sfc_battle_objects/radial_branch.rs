use super::*;
pub(super) const PC: u32 = 0x46b430;
pub(super) const ENTRY: &str = "bvm_radial_compare";
const TARGET: u32 = 0x46b444;
const SEQUENCE: [u8; 28] = [
    0xda, 0x81, 0x58, 0, 2, 0, 0, 0, 0x44, 0xb4, 0x46, 0, 0xb9, 0x82, 0x58, 0, 1, 0, 0, 0, 0xf7,
    0x0b, 0x4b, 0, 0x20, 0xb4, 0x46, 0,
];
pub(super) fn validate(image: &[u8], commands: &[u32]) -> Result<()> {
    let at = (PC - SOURCE_BASE) as usize;
    if image.get(at..at + SEQUENCE.len()) != Some(SEQUENCE.as_slice())
        || [PC, PC + 12, TARGET]
            .iter()
            .any(|pc| !commands.contains(pc))
    {
        return Err("Radial growth comparison or successor boundaries changed".into());
    }
    Ok(())
}
pub(super) fn emit_compare(p: &mut Assembler, layout: &CartridgeLayout) -> Result<()> {
    let bank = layout.cpu_address(IMAGE_OFFSET + (TARGET - SOURCE_BASE) as usize)? >> 16;
    p.label(ENTRY).assume_mode(CpuMode::NATIVE_M8_X16);
    dp(p, M::Ldx, OBJ);
    m16(p);
    ax(p, M::Lda, 0x58);
    dp(p, M::Sta, LHS);
    imm16(p, M::Cmp, 2);
    br(p, M::Bne, "bvm_radial_compare_false");
    imm16(p, M::Lda, TARGET as u16);
    ax(p, M::Sta, 0x40);
    dp(p, M::Sta, PCP);
    implied(p, M::Clc);
    implied(p, M::Clv);
    imm16(p, M::Lda, (TARGET as u16 & 0xff00) | bank as u16);
    imm16(p, M::Ldy, 10);
    m8(p);
    spend(p);
    m16(p);
    imm16(p, M::Lda, u16::from(TRIG_LOOP) * 2);
    implied(p, M::Tax);
    m8(p);
    jmp(p, "bvm_op_f7");
    p.label("bvm_radial_compare_false")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    imm16(p, M::Lda, (PC + 12) as u16);
    ax(p, M::Sta, 0x40);
    dp(p, M::Sta, PCP);
    implied(p, M::Clc);
    implied(p, M::Clv);
    imm16(p, M::Ldy, 4);
    imm16(p, M::Lda, 0xb9 * 2);
    implied(p, M::Tax);
    m8(p);
    jmp(p, "bvm_op_b9");
    Ok(())
}
