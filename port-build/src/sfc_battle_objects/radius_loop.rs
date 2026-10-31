use super::*;
const START: u32 = 0x43584c;
const END: u32 = START + 16;
const OPCODE: u8 = 0xac;
const TURNS: u16 = 24;
const SEQUENCE: [u8; 16] = [
    0xbf, 0xa1, 0x92, 0x96, 0xbf, 0xa1, 0x94, 0x96, 0x06, 0, 24, 0, 0x4c, 0x58, 0x43, 0,
];
pub(super) fn patch(image: &mut [u8], commands: &[u32]) -> Result<()> {
    let at = (START - SOURCE_BASE) as usize;
    if image.get(at..at + SEQUENCE.len()) != Some(SEQUENCE.as_slice())
        || [START, START + 4, START + 8, END]
            .iter()
            .any(|pc| !commands.contains(pc))
    {
        return Err("Radial initialization loop or command boundaries changed".into());
    }
    if commands.iter().any(|pc| {
        pc.checked_sub(SOURCE_BASE)
            .and_then(|offset| image.get(offset as usize))
            == Some(&OPCODE)
    }) {
        return Err("Radial initialization opcode already used".into());
    }
    image[at] = OPCODE;
    Ok(())
}
pub(super) fn emit_loop(p: &mut Assembler) {
    p.label(format!("bvm_op_{OPCODE:02x}"))
        .assume_mode(CpuMode::NATIVE_M8_X16);
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
    jmp(p, "bvm_op_bf");
    p.label("bvm_radius_fresh")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    ax(p, M::Lda, 0x96);
    dp(p, M::Sta, TMP);
    acc(p, M::Asl);
    implied(p, M::Clc);
    dp(p, M::Adc, TMP);
    for _ in 0..3 {
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
    advance_next(p, (END - START) as u16);
}
