use super::*;
const START: u32 = 0x492bd0;
const OPCODE: u8 = 0x9b;
const MARKER: u16 = 0x9b9b;
pub(super) const SOURCE_HASH: &str =
    "1e5d48413a7609999fb5560760ffb0d22c6fc872ca6a00c592fccff160572c65";
const MAIN_OFFSET: usize = 0x6c00;
const RESERVE: usize = 0x300;
const _: () = assert!(MAIN_OFFSET + RESERVE <= 0x7400);
fn entry(layout: &CartridgeLayout) -> u32 {
    let bank = if layout.mapping() == sfc_builder::cartridge::Mapping::HiRom {
        0x80_0000
    } else {
        0
    };
    bank + 0x8000 + MAIN_OFFSET as u32
}
pub(super) fn report(layout: &CartridgeLayout) -> Result<serde_json::Value> {
    Ok(serde_json::json!(
        { "start" : START, "opcode" : OPCODE, "source_range" : [0x492474, 0x492c18],
        "source_sha256" : SOURCE_HASH, "helper_entry" : entry(layout), "main_offset"
        : MAIN_OFFSET, "helper_bytes" : code(layout) ?.bytes().len(),
        "helper_reserve" : RESERVE, "private_object_fields" : [0x68, 0x80], "cache" :
        "XY displacement, marker, angle, XY origins, speed, last amplitude",
        "fallback" : "original bf, or fused arithmetic followed by original f7",
        "hardware_state" :
        "restore original final Y multiplier and projection operands" }
    ))
}
pub(super) fn patch(out: &mut [u8], source: &[u8], commands: &[u32]) -> Result<()> {
    if source
        .get((0x492474 - SOURCE_BASE) as usize..(0x492c18 - SOURCE_BASE) as usize)
        .is_none_or(|b| crate::sha(b) != SOURCE_HASH)
        || commands
            .iter()
            .any(|pc| out.get((pc - SOURCE_BASE) as usize) == Some(&OPCODE))
    {
        return Err("Common radial source or opcode changed".into());
    }
    let mut refs = Vec::new();
    for (offset, bytes) in source.windows(4).enumerate() {
        let target = u32::from_le_bytes(bytes.try_into().unwrap());
        if (0x492aec..0x492c18).contains(&target) {
            refs.push((SOURCE_BASE + offset as u32, target));
        }
    }
    if refs
        != [
            (0x492594, 0x492aec),
            (0x4926dc, 0x492b18),
            (0x492814, 0x492b44),
            (0x49294c, 0x492b70),
            (0x492b14, 0x492b9c),
            (0x492b40, 0x492b9c),
            (0x492b6c, 0x492b9c),
            (0x492b98, 0x492b9c),
            (0x492bc4, 0x492be4),
            (0x492bdc, 0x492bc8),
            (0x492c14, 0x492c0c),
        ]
    {
        return Err("Common radial incoming references changed".into());
    }
    let at = (START - SOURCE_BASE) as usize;
    if out.get(at..at + 8) != Some(&[0xbf, 0xa1, 0x92, 0x58, 0xbf, 0xa1, 0x94, 0x58])
        || out.get(at + 8) != Some(&TRIG_LOOP)
        || [START, START + 4, START + 8, 0x492bc8, 0x492bcc, 0x492be0]
            .iter()
            .any(|pc| !commands.contains(pc))
    {
        return Err("Common radial continuation changed".into());
    }
    out[at] = OPCODE;
    Ok(())
}
pub(super) fn emit_step(p: &mut Assembler, layout: &CartridgeLayout) {
    p.label("bvm_op_9b").assume_mode(CpuMode::NATIVE_M8_X16);
    emit(p, M::Jml, A::AbsoluteLong, O::Long(entry(layout)));
}
pub(crate) fn place_main(bytes: &mut Vec<u8>, layout: &CartridgeLayout) -> Result<()> {
    if bytes.len() > MAIN_OFFSET {
        return Err("Main code overlaps common radial helper".into());
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
            .ok_or("Missing common radial target")?
            .to_u32())
    };
    let sine = layout.cpu_address(layout.section("bvm_sine")?.offset)?;
    let mut assembler = Assembler::new();
    let p = &mut assembler;
    let jump = |p: &mut Assembler, label: &str| {
        p.emit_label_ref(M::Brl, A::Relative16, label);
    };
    let reject = |p: &mut Assembler, accepted: M, next: &str, label: &str| {
        br(p, accepted, next);
        p.emit_label_ref(M::Brl, A::Relative16, label);
        p.label(next).assume_mode(CpuMode::NATIVE_M16_X16);
    };
    let project = |p: &mut Assembler| -> Result<()> {
        m8(p);
        imm8(p, M::Lda, 0x0b);
        emit(
            p,
            M::Jsl,
            A::AbsoluteLong,
            O::Long(target("bvm_inward_projection_original")?),
        );
        dp(p, M::Ldx, OBJ);
        m16(p);
        Ok(())
    };
    p.assume_mode(CpuMode::NATIVE_M8_X16);
    cbyte(p, 1);
    imm8(p, M::Cmp, 0xa1);
    br(p, M::Beq, "radial_entry");
    imm8(p, M::Cmp, 0x80);
    br(p, M::Bne, "blink_entry");
    cbyte(p, 2);
    imm8(p, M::Cmp, 0x24);
    br(p, M::Bne, "radial_birth");
    emit(
        p,
        M::Jml,
        A::AbsoluteLong,
        O::Long(common_blink_density::entry(layout)),
    );
    p.label("radial_birth").assume_mode(CpuMode::NATIVE_M8_X16);
    emit(
        p,
        M::Jml,
        A::AbsoluteLong,
        O::Long(common_density::entry(layout)),
    );
    p.label("blink_entry").assume_mode(CpuMode::NATIVE_M8_X16);
    emit(
        p,
        M::Jml,
        A::AbsoluteLong,
        O::Long(common_blink::entry(layout)),
    );
    p.label("radial_entry").assume_mode(CpuMode::NATIVE_M8_X16);
    dp(p, M::Ldx, OBJ);
    m16(p);
    ax(p, M::Lda, 0x40);
    imm16(p, M::Cmp, START as u16);
    br(p, M::Beq, "radial_pc");
    imm16(p, M::Cmp, 0x48a4);
    reject(p, M::Beq, "lightning_low", "original");
    ax(p, M::Lda, 0x42);
    imm16(p, M::Cmp, 0x49);
    reject(p, M::Beq, "lightning_bank", "original");
    m8(p);
    emit(
        p,
        M::Jml,
        A::AbsoluteLong,
        O::Long(lightning_radius::entry(layout)),
    );
    p.label("radial_pc").assume_mode(CpuMode::NATIVE_M16_X16);
    ax(p, M::Lda, 0x42);
    imm16(p, M::Cmp, 0x49);
    reject(p, M::Beq, "pc_high", "original");
    ax(p, M::Lda, 0x58);
    dp(p, M::Sta, TMP);
    for field in [0x92, 0x94] {
        ax(p, M::Lda, field);
        implied(p, M::Clc);
        dp(p, M::Adc, TMP);
        ax(p, M::Sta, field);
    }
    imm16(p, M::Lda, 0x2bd8);
    ax(p, M::Sta, 0x40);
    dp(p, M::Sta, PCP);
    ax(p, M::Lda, 0x66);
    imm16(p, M::Cmp, 0x49);
    reject(p, M::Beq, "anim_bank", "loop");
    ax(p, M::Lda, 0x64);
    imm16(p, M::Cmp, 0x2be4);
    reject(p, M::Bcs, "anim_low", "loop");
    imm16(p, M::Cmp, 0x2c18);
    reject(p, M::Bcc, "anim_high", "loop");
    ax(p, M::Lda, 0x70);
    imm16(p, M::Cmp, 1);
    br(p, M::Beq, "cache_known");
    imm16(p, M::Cmp, MARKER);
    reject(p, M::Beq, "cache_known", "prime");
    dp(p, M::Lda, BUDGET);
    imm16(p, M::Cmp, 2);
    reject(p, M::Bcs, "budget", "loop");
    ax(p, M::Lda, 0x58);
    imm16(p, M::Cmp, 4);
    reject(p, M::Bcs, "speed_low", "loop");
    imm16(p, M::Cmp, 12);
    reject(p, M::Bcc, "speed_high", "loop");
    ax(p, M::Lda, 0x92);
    ax(p, M::Cmp, 0x94);
    reject(p, M::Beq, "amplitude_equal", "loop");
    ax(p, M::Cmp, 0x58);
    reject(p, M::Bcs, "amplitude_low", "loop");
    imm16(p, M::Cmp, 221);
    reject(p, M::Bcc, "amplitude_high", "loop");
    ax(p, M::Lda, 0x8c);
    ax(p, M::Cmp, 0x8e);
    reject(p, M::Beq, "angle_equal", "loop");
    for (field, value, label) in [(0x3c, 0x2bc8, "latch_low"), (0x3e, 0x49, "latch_high")] {
        ax(p, M::Lda, field);
        imm16(p, M::Cmp, value);
        reject(p, M::Beq, label, "prime");
    }
    ax(p, M::Lda, 0x60);
    imm16(p, M::Cmp, 2);
    reject(p, M::Bcs, "count_low", "loop");
    imm16(p, M::Cmp, 21);
    reject(p, M::Bcc, "count_high", "loop");
    for (current, cached, label) in [
        (0x8c, 0x72, "cache_angle"),
        (0x80, 0x74, "cache_x_low"),
        (0x82, 0x76, "cache_x_high"),
        (0x84, 0x78, "cache_y_low"),
        (0x86, 0x7a, "cache_y_high"),
        (0x58, 0x7c, "cache_speed"),
    ] {
        ax(p, M::Lda, current);
        ax(p, M::Cmp, cached);
        reject(p, M::Beq, label, "prime");
    }
    ax(p, M::Lda, 0x7e);
    implied(p, M::Clc);
    ax(p, M::Adc, 0x58);
    ax(p, M::Cmp, 0x92);
    reject(p, M::Beq, "cache_amplitude", "prime");
    ax(p, M::Dec, 0x60);
    dp(p, M::Dec, BUDGET);
    m8(p);
    imm8(p, M::Lda, 0x0b);
    dp(p, M::Sta, AXIS);
    m16(p);
    ax(p, M::Lda, 0x70);
    imm16(p, M::Cmp, 1);
    reject(p, M::Bne, "hot", "learn");
    for (pos, delta) in [(0x1c, 0x68), (0x20, 0x6c)] {
        implied(p, M::Clc);
        for word in [0, 2] {
            ax(p, M::Lda, pos + word);
            ax(p, M::Adc, delta + word);
            ax(p, M::Sta, pos + word);
        }
    }
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
    br(p, M::Bne, "last_digit");
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(sine + 1));
    p.label("last_digit").assume_mode(CpuMode::NATIVE_M8_X16);
    emit(p, M::Sta, A::AbsoluteLong, O::Long(0x211c));
    dp(p, M::Ldx, OBJ);
    m16(p);
    jump(p, "finish");
    p.label("learn").assume_mode(CpuMode::NATIVE_M16_X16);
    for (pos, cached) in [(0x1c, 0x68), (0x1e, 0x6a), (0x20, 0x6c), (0x22, 0x6e)] {
        ax(p, M::Lda, pos);
        ax(p, M::Sta, cached);
    }
    project(p)?;
    for (pos, cached) in [(0x1c, 0x68), (0x20, 0x6c)] {
        implied(p, M::Sec);
        for word in [0, 2] {
            ax(p, M::Lda, pos + word);
            ax(p, M::Sbc, cached + word);
            ax(p, M::Sta, cached + word);
        }
    }
    imm16(p, M::Lda, MARKER);
    ax(p, M::Sta, 0x70);
    p.label("finish").assume_mode(CpuMode::NATIVE_M16_X16);
    ax(p, M::Lda, 0x92);
    ax(p, M::Sta, 0x7e);
    imm16(p, M::Lda, START as u16);
    ax(p, M::Sta, 0x40);
    imm16(p, M::Lda, 0x49);
    ax(p, M::Sta, 0x42);
    imm16(p, M::Ldy, 6);
    implied(p, M::Clc);
    implied(p, M::Clv);
    m8(p);
    emit(
        p,
        M::Jml,
        A::AbsoluteLong,
        O::Long(common_batch::entry(layout)),
    );
    p.label("prime").assume_mode(CpuMode::NATIVE_M16_X16);
    for (current, cached) in [
        (0x8c, 0x72),
        (0x80, 0x74),
        (0x82, 0x76),
        (0x84, 0x78),
        (0x86, 0x7a),
        (0x58, 0x7c),
        (0x92, 0x7e),
    ] {
        ax(p, M::Lda, current);
        ax(p, M::Sta, cached);
    }
    imm16(p, M::Lda, 1);
    ax(p, M::Sta, 0x70);
    p.label("loop").assume_mode(CpuMode::NATIVE_M16_X16);
    m8(p);
    emit(p, M::Jml, A::AbsoluteLong, O::Long(target("bvm_op_f7")?));
    p.label("original").assume_mode(CpuMode::NATIVE_M16_X16);
    m8(p);
    emit(p, M::Jml, A::AbsoluteLong, O::Long(target("bvm_op_bf")?));
    let at = entry(layout);
    let program = crate::sfc_width_check::assemble(
        p,
        w65c816::CodeLocation::new((at >> 16) as u8, at as u16),
        CpuMode::NATIVE_M8_X16,
    )?;
    if program.bytes().len() > RESERVE {
        return Err("Common radial helper exceeds first-bank reserve".into());
    }
    Ok(program)
}
