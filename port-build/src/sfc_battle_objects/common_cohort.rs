use super::*;
const OPCODE: u8 = 0x9c;
pub(super) fn patch(out: &mut [u8], source: &[u8], commands: &[u32]) -> Result<()> {
    if source
        .get((0x492474 - SOURCE_BASE) as usize..(0x492c18 - SOURCE_BASE) as usize)
        .is_none_or(|bytes| crate::sha(bytes) != common_radial::SOURCE_HASH)
        || commands
            .iter()
            .any(|pc| out.get((pc - SOURCE_BASE) as usize) == Some(&OPCODE))
    {
        return Err("Common cohort source or opcode ownership changed".into());
    }
    for pc in WAITS {
        let pc = 0x490000 + u32::from(pc);
        let at = (pc - SOURCE_BASE) as usize;
        if !commands.contains(&pc) || out.get(at..at + 4) != Some(&[2, 0, 1, 0]) {
            return Err("Common cohort wait entry changed".into());
        }
        out[at] = OPCODE;
    }
    Ok(())
}
pub(crate) fn report(rules: &w65c816::AssembledProgram) -> Result<serde_json::Value> {
    let at = |label: &str| -> Result<u32> {
        Ok(rules
            .label_location(label)
            .ok_or("Missing common cohort label")?
            .to_u32())
    };
    let start = at("bvm_cohort_run")?;
    Ok(serde_json::json!(
        { "helper_entry" : start, "helper_bytes" : at("bvm_cohort_end") ? - start,
        "opcode" : OPCODE, "source_sha256" : common_radial::SOURCE_HASH, "waits" :
        WAITS.map(| pc | 0x490000 + u32::from(pc)), "private_object_fields" : [],
        "new_wram" : [], "fallback" :
        "original wait handler; transitions and insufficient free records",
        "contract" :
        "pool and gameplay state retained; VM operand scratch and return registers may differ"
        }
    ))
}
const WAITS: [u16; 8] = [
    0x251c, 0x25c4, 0x2668, 0x270c, 0x27a0, 0x2844, 0x28d8, 0x297c,
];
const LOOPS: [u16; 8] = [
    0x24d8, 0x253c, 0x2624, 0x2684, 0x275c, 0x27bc, 0x2894, 0x28f4,
];
const BLINK: [u16; 8] = [
    0x298c, 0x298c, 0x299c, 0x299c, 0x29ac, 0x29ac, 0x29bc, 0x29bc,
];
const RADIAL: [u16; 8] = [
    0x2aec, 0x2aec, 0x2b18, 0x2b18, 0x2b44, 0x2b44, 0x2b70, 0x2b70,
];
pub(super) fn emit_parent(p: &mut Assembler) {
    let reject = |p: &mut Assembler, accepted: M, label: &str| {
        br(p, accepted, label);
        jmp(p, "bvm_cohort_fallback");
        p.label(label).assume_mode(CpuMode::NATIVE_M16_X16);
    };
    p.label("bvm_op_9c").assume_mode(CpuMode::NATIVE_M8_X16);
    p.label("bvm_cohort_run")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    dp(p, M::Ldx, OBJ);
    ax(p, M::Lda, 0x42);
    imm16(p, M::Cmp, 0x49);
    reject(p, M::Beq, "bvm_cohort_bank");
    ax(p, M::Lda, 0x40);
    for (index, pc) in WAITS.iter().enumerate() {
        imm16(p, M::Cmp, *pc);
        br(p, M::Beq, &format!("bvm_cohort_parent_{index}"));
    }
    jmp(p, "bvm_cohort_fallback");
    for index in 0..WAITS.len() {
        p.label(format!("bvm_cohort_parent_{index}"))
            .assume_mode(CpuMode::NATIVE_M16_X16);
        imm16(p, M::Lda, index as u16 * 2);
        dp(p, M::Sta, WORK);
        jmp(p, "bvm_cohort_admit");
    }
    p.label("bvm_cohort_admit")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    ax(p, M::Lda, 0x5e);
    imm16(p, M::Cmp, 1);
    reject(p, M::Beq, "bvm_cohort_wait_one");
    ax(p, M::Lda, 0x3e);
    imm16(p, M::Cmp, 0x49);
    reject(p, M::Beq, "bvm_cohort_latch_bank");
    dp(p, M::Ldx, WORK);
    p.emit_label_ref(M::Lda, A::AbsoluteLongX, "bvm_cohort_loops");
    dp(p, M::Ldx, OBJ);
    ax(p, M::Cmp, 0x3c);
    reject(p, M::Beq, "bvm_cohort_latch");
    ax(p, M::Lda, 0x60);
    imm16(p, M::Cmp, 2);
    reject(p, M::Bcs, "bvm_cohort_continue");
    imm16(p, M::Cmp, 14);
    reject(p, M::Bcc, "bvm_cohort_count");
    imm16(p, M::Lda, 1);
    dp(p, M::Sta, COUNT);
    dp(p, M::Lda, WORK);
    imm16(p, M::And, 2);
    br(p, M::Beq, "bvm_cohort_free");
    ax(p, M::Lda, 0x30);
    imm16(p, M::And, 0xff);
    br(p, M::Beq, "bvm_cohort_free");
    imm16(p, M::Lda, 6);
    dp(p, M::Sta, COUNT);
    p.label("bvm_cohort_free");
    abs(p, M::Ldx, FREE);
    p.label("bvm_cohort_free_link");
    imm16(p, M::Cpx, POOL);
    reject(p, M::Bcs, "bvm_cohort_free_lower");
    imm16(p, M::Cpx, POOL + OBJECTS * OBJECT_BYTES);
    reject(p, M::Bcc, "bvm_cohort_free_upper");
    ax(p, M::Lda, 0x0c);
    implied(p, M::Tax);
    dp(p, M::Dec, COUNT);
    br(p, M::Bne, "bvm_cohort_free_link");
    dp(p, M::Ldx, OBJ);
    ax(p, M::Dec, 0x60);
    m8(p);
    jsr(p, "bvm_cohort_allocate");
    p.assume_mode(CpuMode::NATIVE_M16_X16);
    dp(p, M::Ldx, WORK);
    p.emit_label_ref(M::Lda, A::AbsoluteLongX, "bvm_cohort_blinks");
    ay(p, M::Sta, 0x40);
    dp(p, M::Ldx, OBJ);
    for field in [0x80, 0x82, 0x84, 0x86, 0x92, 0x94, 0x96, 0x8c, 0x8e] {
        ax(p, M::Lda, field);
        ay(p, M::Sta, field);
    }
    ax(p, M::Lda, 0x96);
    imm16(p, M::Cmp, 23);
    br(p, M::Bcs, "bvm_cohort_blink_kept");
    imm16(p, M::And, 1);
    br(p, M::Beq, "bvm_cohort_blink_kept");
    imm16(p, M::Lda, 0xd35d);
    ay(p, M::Sta, 0x68);
    p.label("bvm_cohort_blink_kept");
    dp(p, M::Lda, WORK);
    imm16(p, M::And, 12);
    br(p, M::Beq, "bvm_cohort_rotate_up");
    imm16(p, M::Cmp, 12);
    br(p, M::Beq, "bvm_cohort_rotate_up");
    for field in [0x8c, 0x8e] {
        ax(p, M::Lda, field);
        implied(p, M::Sec);
        ax(p, M::Sbc, 0x90);
        ax(p, M::Sta, field);
    }
    br(p, M::Bra, "bvm_cohort_rotated");
    p.label("bvm_cohort_rotate_up");
    for field in [0x8c, 0x8e] {
        ax(p, M::Lda, field);
        implied(p, M::Clc);
        ax(p, M::Adc, 0x90);
        ax(p, M::Sta, field);
    }
    p.label("bvm_cohort_rotated");
    ax(p, M::Inc, 0x96);
    ax(p, M::Lda, 0x90);
    implied(p, M::Clc);
    imm16(p, M::Adc, 96);
    ax(p, M::Sta, 0x90);
    dp(p, M::Lda, WORK);
    imm16(p, M::And, 2);
    br(p, M::Beq, "bvm_cohort_done");
    m8(p);
    ax(p, M::Lda, 0x30);
    br(p, M::Beq, "bvm_cohort_done8");
    ax(p, M::Dec, 0x30);
    imm8(p, M::Lda, 5);
    ax(p, M::Sta, 0x34);
    p.label("bvm_cohort_radial")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    jsr(p, "bvm_cohort_allocate");
    p.assume_mode(CpuMode::NATIVE_M16_X16);
    dp(p, M::Ldx, WORK);
    p.emit_label_ref(M::Lda, A::AbsoluteLongX, "bvm_cohort_radials");
    ay(p, M::Sta, 0x40);
    dp(p, M::Ldx, OBJ);
    for (position, origin) in [(0x1c, 0x80), (0x20, 0x84)] {
        ax(p, M::Lda, position);
        ay(p, M::Sta, origin);
        ax(p, M::Lda, position + 2);
        implied(p, M::Clc);
        imm16(p, M::Adc, 16);
        ay(p, M::Sta, origin + 2);
    }
    ax(p, M::Lda, 0x34);
    imm16(p, M::Cmp, 4);
    br(p, M::Beq, "bvm_cohort_radial_omitted");
    imm16(p, M::Cmp, 2);
    br(p, M::Bne, "bvm_cohort_radial_kept");
    p.label("bvm_cohort_radial_omitted");
    imm16(p, M::Lda, 0xd35e);
    ay(p, M::Sta, 0x70);
    p.label("bvm_cohort_radial_kept");
    m8(p);
    ax(p, M::Dec, 0x34);
    br(p, M::Bne, "bvm_cohort_radial");
    p.label("bvm_cohort_done8")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    p.label("bvm_cohort_done")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    imm16(p, M::Lda, 1);
    ax(p, M::Sta, 0x5e);
    imm16(p, M::Ldy, 2);
    m8(p);
    implied(p, M::Rts);
    p.label("bvm_cohort_fallback")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    m8(p);
    jmp(p, "bvm_op_02");
    p.label("bvm_cohort_allocate")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    m8(p);
    imm8(p, M::Lda, 1);
    jsr(p, "bvm_alloc");
    m16(p);
    implied(p, M::Txa);
    implied(p, M::Tay);
    implied(p, M::Clc);
    imm16(p, M::Adc, SOURCE_DELTA as u16);
    dp(p, M::Ldx, OBJ);
    for field in [0x58, 0xa4, 0xa8] {
        ax(p, M::Sta, field);
    }
    imm16(p, M::Lda, (SOURCE_DELTA >> 16) as u16);
    imm16(p, M::Adc, 0);
    for field in [0x5a, 0xa6, 0xaa] {
        ax(p, M::Sta, field);
    }
    implied(p, M::Txa);
    implied(p, M::Clc);
    imm16(p, M::Adc, SOURCE_DELTA as u16);
    ay(p, M::Sta, 0xa0);
    imm16(p, M::Lda, (SOURCE_DELTA >> 16) as u16);
    imm16(p, M::Adc, 0);
    ay(p, M::Sta, 0xa2);
    imm16(p, M::Lda, 0x49);
    ay(p, M::Sta, 0x42);
    implied(p, M::Rts);
    for (name, words) in [
        ("bvm_cohort_loops", LOOPS),
        ("bvm_cohort_blinks", BLINK),
        ("bvm_cohort_radials", RADIAL),
    ] {
        p.label(name);
        p.data(
            words
                .into_iter()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>(),
        );
    }
    p.label("bvm_cohort_end");
}
