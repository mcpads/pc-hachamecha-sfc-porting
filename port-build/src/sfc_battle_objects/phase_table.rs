use super::*;
pub(super) const OPCODE: u8 = 0xae;
const STEP: u32 = 0x495490;
const STEP_OPCODE: u8 = 0xad;
const START: u32 = 0x495498;
const SEQUENCE: [u8; 8] = [TRIG_LOOP, 0x0a, 8, 0, 0x88, 0x54, 0x49, 0];
pub(super) fn patch(image: &mut [u8], commands: &[u32]) -> Result<()> {
    let at = (START - SOURCE_BASE) as usize;
    if image.get(at..at + 8) != Some(SEQUENCE.as_slice())
        || [START, START + 8, 0x495488, 0x49548c]
            .iter()
            .any(|pc| !commands.contains(pc))
    {
        return Err("Status-child motion loop or boundaries changed".into());
    }
    if commands
        .iter()
        .any(|pc| image.get((pc - SOURCE_BASE) as usize) == Some(&OPCODE))
    {
        return Err("Phase motion opcode already used".into());
    }
    let step = (STEP - SOURCE_BASE) as usize;
    if image.get(step..step + 8) != Some(&[0xb8, 0x81, 0x8e, 0, 0, 4, 0, 0])
        || !commands.contains(&STEP)
        || commands
            .iter()
            .any(|pc| image.get((pc - SOURCE_BASE) as usize) == Some(&STEP_OPCODE))
    {
        return Err("Status-child angle step changed or opcode occupied".into());
    }
    image[step] = STEP_OPCODE;
    image[at] = OPCODE;
    Ok(())
}
pub(super) fn fill_padding(table: &mut [u8]) -> Result<()> {
    if table.len() != 4 * 4096 || table.as_chunks::<4>().0.iter().any(|d| d[3] != 0) {
        return Err("Phase products require unused sine fourth bytes".into());
    }
    for phase in 0..16usize {
        let coefficient = (0..3)
            .map(|j| i32::from(table[4 * (0x400 + phase * 64) + j] as i8) << (8 * j))
            .sum::<i32>();
        for amplitude in 0..64usize {
            let product = coefficient * amplitude as i32;
            for (byte, value) in product.to_le_bytes().into_iter().enumerate() {
                table[3 + 16 * (phase * 64 + amplitude) + 4 * byte] = value;
            }
        }
    }
    Ok(())
}
pub(super) fn emit_y(p: &mut Assembler, sine: u32) {
    p.label("bvm_phase_y").assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    ax(p, M::Lda, 0x94);
    imm16(p, M::Cmp, 64);
    br(p, M::Bcs, "bvm_phase_fallback");
    ax(p, M::Lda, 0x8e);
    imm16(p, M::Bit, 0xc3ff);
    br(p, M::Bne, "bvm_phase_fallback");
    ax(p, M::Lda, 0x94);
    for _ in 0..4 {
        acc(p, M::Asl);
    }
    ax(p, M::Ora, 0x8e);
    implied(p, M::Tax);
    m8(p);
    for byte in 0..4u8 {
        emit(
            p,
            M::Lda,
            A::AbsoluteLongX,
            O::Long(sine + 3 + u32::from(byte) * 4),
        );
        dp(p, M::Sta, RES + byte);
    }
    dp(p, M::Ldx, OBJ);
    m16(p);
    for word in 0..2u16 {
        ax(p, M::Lda, 0x84 + word * 2);
        if word == 0 {
            implied(p, M::Sec);
        }
        dp(p, M::Sbc, RES + word as u8 * 2);
        ax(p, M::Sta, 0x20 + word * 2);
    }
    m8(p);
    implied(p, M::Rts);
    p.label("bvm_phase_fallback")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    m8(p);
    dp(p, M::Lda, AXIS);
    jmp(p, "bvm_2d_loaded");
}
pub(super) fn emit_step(p: &mut Assembler) {
    p.label("bvm_op_ad").assume_mode(CpuMode::NATIVE_M8_X16);
    dp(p, M::Ldx, OBJ);
    m16(p);
    ax(p, M::Lda, 0x8e);
    implied(p, M::Clc);
    imm16(p, M::Adc, 0x400);
    ax(p, M::Sta, 0x8e);
    ax(p, M::Lda, 0x3c);
    imm16(p, M::Cmp, 0x5488);
    br(p, M::Bne, "bvm_phase_step_fallback");
    ax(p, M::Lda, 0x3e);
    imm16(p, M::Cmp, 0x49);
    br(p, M::Bne, "bvm_phase_step_fallback");
    ax(p, M::Lda, 0x60);
    imm16(p, M::Cmp, 1);
    br(p, M::Beq, "bvm_phase_step_fallback");
    dp(p, M::Lda, BUDGET);
    imm16(p, M::Cmp, 1);
    br(p, M::Beq, "bvm_phase_step_fallback");
    ax(p, M::Dec, 0x60);
    dp(p, M::Dec, BUDGET);
    m8(p);
    imm8(p, M::Lda, 0x0a);
    dp(p, M::Sta, AXIS);
    jsr(p, "bvm_phase_y");
    yield_now(p);
    p.label("bvm_phase_step_fallback")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    imm16(p, M::Lda, START as u16);
    ax(p, M::Sta, 0x40);
    dp(p, M::Sta, PCP);
    m8(p);
    jmp(p, "bvm_op_ae");
}
