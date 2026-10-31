use super::*;
const SITES: [u32; 7] = [
    0x492a20, 0x492a30, 0x492a40, 0x492a50, 0x492a74, 0x492a90, 0x492aac,
];
const MAIN_OFFSET: usize = 0x7000;
const RESERVE: usize = 0x100;
const MARKER: u16 = 0x9c9c;
pub(super) fn entry(layout: &CartridgeLayout) -> u32 {
    let bank = if layout.mapping() == sfc_builder::cartridge::Mapping::HiRom {
        0x80_0000
    } else {
        0
    };
    bank + 0x8000 + MAIN_OFFSET as u32
}
pub(super) fn report(layout: &CartridgeLayout) -> Result<serde_json::Value> {
    Ok(serde_json::json!(
        { "sites" : SITES, "opcode" : 0x9b, "source_range" : [0x492474, 0x492c18],
        "source_sha256" : common_radial::SOURCE_HASH, "helper_entry" : entry(layout),
        "helper_bytes" : code(layout) ?.bytes().len(), "main_offset" : MAIN_OFFSET,
        "helper_reserve" : RESERVE, "private_object_fields" : [0x68, 0x6a],
        "first_projection" :
        "492a20 stays unprimed to support post-yield parameter writes", "fallback" :
        "original opcode 2d", "hardware_state" :
        "original final Y multiplier and projection operands" }
    ))
}
pub(super) fn patch(out: &mut [u8], source: &[u8], commands: &[u32]) -> Result<()> {
    if source
        .get((0x492474 - SOURCE_BASE) as usize..(0x492c18 - SOURCE_BASE) as usize)
        .is_none_or(|b| crate::sha(b) != common_radial::SOURCE_HASH)
    {
        return Err("Common blink source changed".into());
    }
    let incoming: Vec<_> = source
        .windows(4)
        .enumerate()
        .filter_map(|(i, b)| {
            let pc = u32::from_le_bytes(b.try_into().unwrap());
            (0x49298c..0x492aec)
                .contains(&pc)
                .then_some((SOURCE_BASE + i as u32, pc))
        })
        .collect();
    if incoming
        .iter()
        .any(|(at, _)| !(0x492474..0x492aec).contains(at))
    {
        return Err("Foreign common blink entry".into());
    }
    for pc in SITES {
        let at = (pc - SOURCE_BASE) as usize;
        if out.get(at..at + 8) != Some(&[0x2d, 0x0b, 0, 0, 1, 0, 0, 0])
            || !commands.contains(&pc)
            || !commands.contains(&(pc + 4))
        {
            return Err("Common blink projection or yield changed".into());
        }
        out[at] = 0x9b;
    }
    Ok(())
}
pub(crate) fn place_main(bytes: &mut Vec<u8>, layout: &CartridgeLayout) -> Result<()> {
    if bytes.len() > MAIN_OFFSET {
        return Err("Main code overlaps common blink helper".into());
    }
    let helper = code(layout)?;
    bytes.resize(MAIN_OFFSET, 0);
    bytes.extend_from_slice(helper.bytes());
    Ok(())
}
fn code(layout: &CartridgeLayout) -> Result<w65c816::AssembledProgram> {
    let rules = crate::sfc_battle::code(layout)?;
    let target = |name: &str| -> Result<u32> {
        Ok(rules
            .label_location(name)
            .ok_or("Missing common blink target")?
            .to_u32())
    };
    let sine = layout.cpu_address(layout.section("bvm_sine")?.offset)?;
    let mut assembler = Assembler::new();
    let p = &mut assembler;
    let reject = |p: &mut Assembler, accepted: M, next: &str| {
        br(p, accepted, next);
        p.emit_label_ref(M::Brl, A::Relative16, "original");
        p.label(next).assume_mode(CpuMode::NATIVE_M16_X16);
    };
    p.assume_mode(CpuMode::NATIVE_M8_X16);
    dp(p, M::Ldx, OBJ);
    m16(p);
    ax(p, M::Lda, 0x42);
    imm16(p, M::Cmp, 0x49);
    reject(p, M::Beq, "bank");
    ax(p, M::Lda, 0x40);
    for pc in SITES {
        imm16(p, M::Cmp, pc as u16);
        br(p, M::Beq, "site");
    }
    p.emit_label_ref(M::Brl, A::Relative16, "original");
    p.label("site").assume_mode(CpuMode::NATIVE_M16_X16);
    ax(p, M::Lda, 0x66);
    imm16(p, M::Cmp, 0x49);
    reject(p, M::Beq, "anim_bank");
    ax(p, M::Lda, 0x64);
    imm16(p, M::Cmp, 0x2ab8);
    reject(p, M::Bcs, "anim_low");
    imm16(p, M::Cmp, 0x2aec);
    reject(p, M::Bcc, "anim_high");
    ax(p, M::Lda, 0x68);
    imm16(p, M::Cmp, MARKER);
    br(p, M::Beq, "cached");
    ax(p, M::Lda, 0x40);
    imm16(p, M::Cmp, 0x2a20);
    reject(p, M::Bne, "prime");
    imm16(p, M::Lda, MARKER);
    ax(p, M::Sta, 0x68);
    p.emit_label_ref(M::Brl, A::Relative16, "original");
    p.label("cached").assume_mode(CpuMode::NATIVE_M16_X16);
    m8(p);
    imm8(p, M::Lda, 0x0b);
    dp(p, M::Sta, AXIS);
    m16(p);
    implied(p, M::Sec);
    for word in [0, 2] {
        ax(p, M::Lda, 0x84 + word);
        dp(p, M::Sta, LHS + word as u8);
        ax(p, M::Sbc, 0x20 + word);
        dp(p, M::Sta, RES + word as u8);
    }
    ax(p, M::Lda, 0x94);
    m8(p);
    emit(p, M::Sta, A::AbsoluteLong, O::Long(0x211b));
    implied(p, M::Xba);
    emit(p, M::Sta, A::AbsoluteLong, O::Long(0x211b));
    m16(p);
    ax(p, M::Lda, 0x8e);
    implied(p, M::Clc);
    imm16(p, M::Adc, 0x4008);
    acc(p, M::Lsr);
    acc(p, M::Lsr);
    imm16(p, M::And, 0xfffc);
    implied(p, M::Tax);
    m8(p);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(sine + 2));
    br(p, M::Bne, "digit");
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(sine + 1));
    p.label("digit").assume_mode(CpuMode::NATIVE_M8_X16);
    emit(p, M::Sta, A::AbsoluteLong, O::Long(0x211c));
    dp(p, M::Ldx, OBJ);
    m16(p);
    imm16(p, M::Ldy, 1);
    ax(p, M::Lda, 0x40);
    implied(p, M::Clc);
    imm16(p, M::Adc, 8);
    ax(p, M::Sta, 0x40);
    dp(p, M::Sta, PCP);
    m8(p);
    emit(
        p,
        M::Jml,
        A::AbsoluteLong,
        O::Long(common_batch::entry(layout)),
    );
    p.label("original").assume_mode(CpuMode::NATIVE_M16_X16);
    m8(p);
    emit(p, M::Jml, A::AbsoluteLong, O::Long(target("bvm_op_2d")?));
    let at = entry(layout);
    let program = crate::sfc_width_check::assemble(
        p,
        w65c816::CodeLocation::new((at >> 16) as u8, at as u16),
        CpuMode::NATIVE_M8_X16,
    )?;
    if program.bytes().len() > RESERVE {
        return Err("Common blink helper exceeds main reservation".into());
    }
    Ok(program)
}
