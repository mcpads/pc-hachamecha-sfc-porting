use super::*;
const MAIN_OFFSET: usize = 0x7100;
const RESERVE: usize = 0x300;
const _: () = assert!(MAIN_OFFSET + RESERVE == 0x7400);
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
        { "source_range" : [0x492474, 0x492c18], "source_sha256" :
        common_radial::SOURCE_HASH, "helper_entry" : entry(layout), "helper_bytes" :
        code(layout) ?.bytes().len(), "main_offset" : MAIN_OFFSET, "helper_reserve" :
        RESERVE, "private_object_fields" : [], "admission" :
        "after an ordinary effect; normal-call return guard; at least two independently admitted following leaves, including mixed phases",
        "fallback" : "original bvm_run; final member always runs normally", "scratch"
        :
        "retain the flag-store temporary high word across mixed kinds; final member executes normally"
        }
    ))
}
pub(crate) fn place_main(bytes: &mut Vec<u8>, layout: &CartridgeLayout) -> Result<()> {
    if bytes.len() > MAIN_OFFSET {
        return Err("Main code overlaps common effect batch".into());
    }
    let helper = code(layout)?;
    bytes.resize(MAIN_OFFSET, 0);
    bytes.extend_from_slice(helper.bytes());
    Ok(())
}
fn code(layout: &CartridgeLayout) -> Result<w65c816::AssembledProgram> {
    let rules = crate::sfc_battle::code(layout)?;
    let original = rules
        .label_location("bvm_run_wide")
        .ok_or("Missing wide VM entry")?
        .to_u32();
    let mut assembler = Assembler::new();
    let p = &mut assembler;
    let reject = |p: &mut Assembler, accepted: M, next: &str| {
        br(p, accepted, next);
        p.emit_label_ref(M::Brl, A::Relative16, "invalid");
        p.label(next).assume_mode(CpuMode::NATIVE_M16_X16);
    };
    let returned = rules
        .label_location("bvm_normal_return")
        .ok_or("Missing normal return")?
        .to_u32() as u16
        - 1;
    let done = rules
        .label_location("bvm_run_done")
        .ok_or("Missing VM return")?
        .to_u32();
    p.assume_mode(CpuMode::NATIVE_M8_X16);
    implied(p, M::Php);
    m16(p);
    implied(p, M::Pha);
    implied(p, M::Phx);
    emit(p, M::Lda, A::StackRelative, O::Byte(6));
    imm16(p, M::Cmp, returned);
    br(p, M::Beq, "normal_caller");
    p.label("restore").assume_mode(CpuMode::NATIVE_M16_X16);
    implied(p, M::Plx);
    implied(p, M::Pla);
    implied(p, M::Plp);
    p.assume_mode(CpuMode::NATIVE_M8_X16);
    emit(p, M::Jml, A::AbsoluteLong, O::Long(done));
    p.label("normal_caller")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    ax(p, M::Lda, 0x08);
    implied(p, M::Tax);
    dp(p, M::Cpx, TAIL);
    br(p, M::Beq, "restore");
    ax(p, M::Lda, 0x40);
    implied(p, M::Pha);
    jsr(p, "validate");
    br(p, M::Bcs, "second_member");
    p.label("no_phase").assume_mode(CpuMode::NATIVE_M16_X16);
    implied(p, M::Pla);
    br(p, M::Bra, "restore");
    p.label("second_member")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    implied(p, M::Phx);
    ax(p, M::Lda, 0x08);
    implied(p, M::Tax);
    dp(p, M::Cpx, TAIL);
    br(p, M::Beq, "no_third");
    jsr(p, "validate");
    br(p, M::Bcs, "start_batch");
    p.label("no_third").assume_mode(CpuMode::NATIVE_M16_X16);
    implied(p, M::Plx);
    br(p, M::Bra, "no_phase");
    p.label("start_batch").assume_mode(CpuMode::NATIVE_M16_X16);
    implied(p, M::Plx);
    dp(p, M::Stx, OBJ);
    implied(p, M::Pla);
    implied(p, M::Plx);
    implied(p, M::Pla);
    implied(p, M::Plp);
    p.assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    dp(p, M::Ldx, OBJ);
    ax(p, M::Lda, 0x40);
    p.emit_label_ref(M::Brl, A::Relative16, "apply");
    p.label("original").assume_mode(CpuMode::NATIVE_M16_X16);
    emit(p, M::Jml, A::AbsoluteLong, O::Long(original));
    p.label("peek").assume_mode(CpuMode::NATIVE_M16_X16);
    ax(p, M::Lda, 0x40);
    implied(p, M::Pha);
    implied(p, M::Phx);
    ax(p, M::Lda, 0x08);
    implied(p, M::Tax);
    dp(p, M::Cpx, TAIL);
    br(p, M::Beq, "last");
    jsr(p, "validate");
    br(p, M::Bcs, "advance");
    p.label("last").assume_mode(CpuMode::NATIVE_M16_X16);
    implied(p, M::Plx);
    implied(p, M::Pla);
    br(p, M::Bra, "original");
    p.label("advance").assume_mode(CpuMode::NATIVE_M16_X16);
    implied(p, M::Plx);
    implied(p, M::Pla);
    p.label("apply").assume_mode(CpuMode::NATIVE_M16_X16);
    imm16(p, M::Cmp, 0x2bd0);
    br(p, M::Beq, "radial");
    imm16(p, M::Cmp, 0x2a58);
    br(p, M::Bne, "linear");
    ax(p, M::Dec, 0x60);
    imm16(p, M::Lda, 0x2a28);
    br(p, M::Bra, "visible");
    p.label("linear").assume_mode(CpuMode::NATIVE_M16_X16);
    implied(p, M::Clc);
    imm16(p, M::Adc, 0x10);
    imm16(p, M::Cmp, 0x2a38);
    br(p, M::Beq, "visible");
    ax(p, M::Sta, 0x40);
    imm16(p, M::Lda, 0x1000);
    br(p, M::Bra, "flags");
    p.label("visible").assume_mode(CpuMode::NATIVE_M16_X16);
    ax(p, M::Sta, 0x40);
    imm16(p, M::Lda, 0x5000);
    p.label("flags").assume_mode(CpuMode::NATIVE_M16_X16);
    dp(p, M::Sta, TMP + 2);
    ax(p, M::Sta, 0x02);
    ax(p, M::Stz, 0);
    br(p, M::Bra, "next");
    p.label("radial").assume_mode(CpuMode::NATIVE_M16_X16);
    ax(p, M::Lda, 0x92);
    implied(p, M::Clc);
    ax(p, M::Adc, 0x58);
    ax(p, M::Sta, 0x92);
    ax(p, M::Sta, 0x94);
    ax(p, M::Sta, 0x7e);
    ax(p, M::Dec, 0x60);
    for (pos, delta) in [(0x1c, 0x68), (0x20, 0x6c)] {
        implied(p, M::Clc);
        for word in [0, 2] {
            ax(p, M::Lda, pos + word);
            ax(p, M::Adc, delta + word);
            ax(p, M::Sta, pos + word);
        }
    }
    p.label("next").assume_mode(CpuMode::NATIVE_M16_X16);
    ax(p, M::Lda, 0x08);
    implied(p, M::Tax);
    dp(p, M::Stx, OBJ);
    p.emit_label_ref(M::Brl, A::Relative16, "peek");
    p.label("validate").assume_mode(CpuMode::NATIVE_M16_X16);
    ax(p, M::Lda, 0x42);
    imm16(p, M::Cmp, 0x49);
    reject(p, M::Beq, "bank");
    ax(p, M::Lda, 0x02);
    reject(p, M::Bpl, "awake");
    ax(p, M::Lda, 0x40);
    imm16(p, M::Cmp, 0x2bd0);
    br(p, M::Beq, "radial_guard");
    for pc in [0x2a28, 0x2a38, 0x2a48] {
        imm16(p, M::Cmp, pc);
        br(p, M::Beq, "blink_guard");
    }
    imm16(p, M::Cmp, 0x2a58);
    reject(p, M::Beq, "blink_loop");
    for (field, value, label) in [
        (0x3c, 0x2a18, "blink_latch"),
        (0x3e, 0x49, "blink_latch_bank"),
    ] {
        ax(p, M::Lda, field);
        imm16(p, M::Cmp, value);
        reject(p, M::Beq, label);
    }
    ax(p, M::Lda, 0x60);
    imm16(p, M::Cmp, 2);
    reject(p, M::Bcs, "blink_count_low");
    imm16(p, M::Cmp, 6);
    reject(p, M::Bcc, "blink_count_high");
    p.label("blink_guard").assume_mode(CpuMode::NATIVE_M16_X16);
    ax(p, M::Lda, 0x68);
    imm16(p, M::Cmp, 0x9c9c);
    reject(p, M::Beq, "blink_marker");
    ax(p, M::Lda, 0x66);
    imm16(p, M::Cmp, 0x49);
    reject(p, M::Beq, "blink_anim_bank");
    ax(p, M::Lda, 0x64);
    imm16(p, M::Cmp, 0x2ab8);
    reject(p, M::Bcs, "blink_anim_low");
    imm16(p, M::Cmp, 0x2aec);
    reject(p, M::Bcc, "blink_anim_high");
    implied(p, M::Sec);
    implied(p, M::Rts);
    p.label("radial_guard").assume_mode(CpuMode::NATIVE_M16_X16);
    for (field, value, label) in [
        (0x70, 0x9b9b, "radial_marker"),
        (0x3c, 0x2bc8, "radial_latch"),
        (0x3e, 0x49, "radial_latch_bank"),
        (0x66, 0x49, "radial_anim_bank"),
    ] {
        ax(p, M::Lda, field);
        imm16(p, M::Cmp, value);
        reject(p, M::Beq, label);
    }
    ax(p, M::Lda, 0x64);
    imm16(p, M::Cmp, 0x2be4);
    reject(p, M::Bcs, "radial_anim_low");
    imm16(p, M::Cmp, 0x2c18);
    reject(p, M::Bcc, "radial_anim_high");
    ax(p, M::Lda, 0x60);
    imm16(p, M::Cmp, 2);
    reject(p, M::Bcs, "radial_count_low");
    imm16(p, M::Cmp, 21);
    reject(p, M::Bcc, "radial_count_high");
    ax(p, M::Lda, 0x58);
    imm16(p, M::Cmp, 4);
    reject(p, M::Bcs, "speed_low");
    imm16(p, M::Cmp, 12);
    reject(p, M::Bcc, "speed_high");
    ax(p, M::Lda, 0x92);
    ax(p, M::Cmp, 0x94);
    reject(p, M::Beq, "amp_equal");
    ax(p, M::Cmp, 0x7e);
    reject(p, M::Beq, "amp_cache");
    implied(p, M::Clc);
    ax(p, M::Adc, 0x58);
    reject(p, M::Bcc, "amp_no_wrap");
    imm16(p, M::Cmp, 221);
    reject(p, M::Bcc, "amp_high");
    ax(p, M::Lda, 0x8c);
    ax(p, M::Cmp, 0x8e);
    reject(p, M::Beq, "angle_equal");
    for (field, cached, label) in [
        (0x8c, 0x72, "angle"),
        (0x80, 0x74, "x_low"),
        (0x82, 0x76, "x_high"),
        (0x84, 0x78, "y_low"),
        (0x86, 0x7a, "y_high"),
        (0x58, 0x7c, "speed"),
    ] {
        ax(p, M::Lda, field);
        ax(p, M::Cmp, cached);
        reject(p, M::Beq, label);
    }
    implied(p, M::Sec);
    implied(p, M::Rts);
    p.label("invalid").assume_mode(CpuMode::NATIVE_M16_X16);
    implied(p, M::Clc);
    implied(p, M::Rts);
    let at = entry(layout);
    let program = crate::sfc_width_check::assemble(
        p,
        w65c816::CodeLocation::new((at >> 16) as u8, at as u16),
        CpuMode::NATIVE_M8_X16,
    )?;
    if program.bytes().len() > RESERVE {
        return Err("Common batch exceeds main reservation".into());
    }
    Ok(program)
}
