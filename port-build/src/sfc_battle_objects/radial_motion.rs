use super::*;
const START: u32 = 0x46b428;
const OPCODE: u8 = 0xa5;
const SEQUENCE: [u8; 8] = [0xbf, 0xa1, 0x92, 0x58, 0xbf, 0xa1, 0x94, 0x58];
pub(super) fn patch(image: &mut [u8], source: &[u8], commands: &[u32]) -> Result<()> {
    for (start, end, hash) in [
        (
            0x46b3b4,
            0x46b450,
            "e3edbab5f8a42084fce1dd6f62956e81a45b1d3c9c1e3a746dda6f9873890759",
        ),
        (
            0x46b368,
            0x46b3b4,
            "2f43971d0bf4ec47ec2d48ee4bb753b1de0e4895a04fa219bd317592b16b4e43",
        ),
    ] {
        let bytes = image
            .get((start - SOURCE_BASE) as usize..(end - SOURCE_BASE) as usize)
            .ok_or("Radial motion source range missing")?;
        if crate::sha(bytes) != hash {
            return Err("Radial motion leaf or spawner changed".into());
        }
    }
    let at = (START - SOURCE_BASE) as usize;
    if image.get(at..at + 8) != Some(SEQUENCE.as_slice())
        || [START, START + 4, START + 8, 0x46b420, 0x46b424, 0x46b444]
            .iter()
            .any(|pc| !commands.contains(pc))
        || commands.iter().any(|pc| {
            image
                .get((pc - SOURCE_BASE) as usize)
                .is_some_and(|opcode| [OPCODE, 0xfc].contains(opcode))
        })
    {
        return Err("Radial motion entry or opcode changed".into());
    }
    for (start, end, hash) in [
        (
            0x4bc79c,
            0x4bc838,
            "54c0b9db9452813afc05c3d59fea2d2d78d8a8010a0a9da27f009888d19d1ed1",
        ),
        (
            0x4bc750,
            0x4bc79c,
            "06846a78963163e9cbf7282d37775716f6e8b75e268e2f8591c50ce1339a8717",
        ),
    ] {
        if source
            .get((start - SOURCE_BASE) as usize..(end - SOURCE_BASE) as usize)
            .is_none_or(|b| crate::sha(b) != hash)
        {
            return Err("Second radial star or linked spawner changed".into());
        }
    }
    for (offset, bytes) in source.windows(4).enumerate() {
        let target = u32::from_le_bytes(bytes.try_into().unwrap());
        if (0x4bc79c..0x4bc838).contains(&target) {
            let va = SOURCE_BASE + offset as u32;
            let spawn = target == 0x4bc79c
                && [0x4bc754, 0x4bc760, 0x4bc76c, 0x4bc778, 0x4bc784].contains(&va);
            let internal =
                (va == 0x4bc820 && target == 0x4bc82c) || (va == 0x4bc830 && target == 0x4bc808);
            if !spawn && !internal {
                return Err("Second radial star has an unexpected incoming reference".into());
            }
        }
    }
    let second = (0x4bc810 - SOURCE_BASE) as usize;
    if image.get(second..second + 8) != Some(SEQUENCE.as_slice())
        || [
            0x4bc808, 0x4bc80c, 0x4bc810, 0x4bc814, 0x4bc818, 0x4bc824, 0x4bc82c, 0x4bc834,
        ]
        .iter()
        .any(|pc| !commands.contains(pc))
    {
        return Err("Second radial star entry changed".into());
    }
    image[at] = OPCODE;
    image[second] = 0xfc;
    Ok(())
}
pub(super) fn emit_step(p: &mut Assembler) {
    p.label("bvm_op_a5").assume_mode(CpuMode::NATIVE_M8_X16);
    dp(p, M::Ldx, OBJ);
    m16(p);
    ax(p, M::Lda, 0x58);
    imm16(p, M::Cmp, 2);
    br(p, M::Bne, "bvm_star_slow");
    ax(p, M::Lda, 0x3c);
    imm16(p, M::Cmp, 0xb420);
    br(p, M::Bne, "bvm_star_slow");
    ax(p, M::Lda, 0x3e);
    imm16(p, M::Cmp, 0x46);
    br(p, M::Bne, "bvm_star_slow");
    ax(p, M::Lda, 0x60);
    imm16(p, M::Cmp, 1);
    br(p, M::Beq, "bvm_star_slow");
    dp(p, M::Lda, BUDGET);
    imm16(p, M::Cmp, 3);
    br(p, M::Bcc, "bvm_star_slow");
    ax(p, M::Lda, 0x92);
    ax(p, M::Ora, 0x94);
    imm16(p, M::Bit, 0xc000);
    br(p, M::Bne, "bvm_star_slow");
    p.label("bvm_star_fast");
    ax(p, M::Inc, 0x92);
    ax(p, M::Inc, 0x92);
    ax(p, M::Inc, 0x94);
    ax(p, M::Inc, 0x94);
    ax(p, M::Lda, 0x70);
    br(p, M::Beq, "bvm_star_cold");
    for (pos, delta) in [(0x1c, 0x68), (0x20, 0x6c)] {
        implied(p, M::Clc);
        for word in [0, 2] {
            ax(p, M::Lda, pos + word);
            ax(p, M::Adc, delta + word);
            ax(p, M::Sta, pos + word);
        }
    }
    br(p, M::Bra, "bvm_star_done");
    p.label("bvm_star_slow")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    for amp in [0x92, 0x94] {
        ax(p, M::Lda, amp);
        implied(p, M::Clc);
        ax(p, M::Adc, 0x58);
        ax(p, M::Sta, amp);
    }
    imm16(p, M::Lda, (START + 8) as u16);
    ax(p, M::Sta, 0x40);
    dp(p, M::Sta, PCP);
    m8(p);
    jmp(p, radial_branch::ENTRY);
    p.label("bvm_star_cold")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    for (pos, delta) in [(0x1c, 0x68), (0x20, 0x6c)] {
        for word in [0, 2] {
            ax(p, M::Lda, pos + word);
            ax(p, M::Sta, delta + word);
        }
    }
    m8(p);
    imm8(p, M::Lda, 0x0b);
    dp(p, M::Sta, AXIS);
    jsr(p, "bvm_2d_loaded");
    dp(p, M::Ldx, OBJ);
    m16(p);
    for (pos, delta) in [(0x1c, 0x68), (0x20, 0x6c)] {
        implied(p, M::Sec);
        for word in [0, 2] {
            ax(p, M::Lda, pos + word);
            ax(p, M::Sbc, delta + word);
            ax(p, M::Sta, delta + word);
        }
    }
    imm16(p, M::Lda, 1);
    ax(p, M::Sta, 0x70);
    p.label("bvm_star_done")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    ax(p, M::Dec, 0x60);
    dp(p, M::Dec, BUDGET);
    dp(p, M::Dec, BUDGET);
    m8(p);
    imm8(p, M::Lda, 0x0b);
    dp(p, M::Sta, AXIS);
    yield_now(p);
    p.label("bvm_op_fc").assume_mode(CpuMode::NATIVE_M8_X16);
    dp(p, M::Ldx, OBJ);
    m16(p);
    ax(p, M::Lda, 0x58);
    imm16(p, M::Cmp, 2);
    br(p, M::Bne, "bvm_star_second_slow");
    ax(p, M::Lda, 0x3c);
    imm16(p, M::Cmp, 0xc808);
    br(p, M::Bne, "bvm_star_second_slow");
    ax(p, M::Lda, 0x3e);
    imm16(p, M::Cmp, 0x4b);
    br(p, M::Bne, "bvm_star_second_slow");
    ax(p, M::Lda, 0x60);
    imm16(p, M::Cmp, 1);
    br(p, M::Beq, "bvm_star_second_slow");
    dp(p, M::Lda, BUDGET);
    imm16(p, M::Cmp, 3);
    br(p, M::Bcc, "bvm_star_second_slow");
    ax(p, M::Lda, 0x92);
    ax(p, M::Ora, 0x94);
    imm16(p, M::Bit, 0xc000);
    br(p, M::Bne, "bvm_star_second_slow");
    jmp(p, "bvm_star_fast");
    p.label("bvm_star_second_slow")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    for amp in [0x92, 0x94] {
        ax(p, M::Lda, amp);
        implied(p, M::Clc);
        ax(p, M::Adc, 0x58);
        ax(p, M::Sta, amp);
    }
    imm16(p, M::Lda, 0xc818);
    ax(p, M::Sta, 0x40);
    dp(p, M::Sta, PCP);
    m8(p);
    next(p);
}
