use super::*;
const STEP: u32 = 0x495338;
const OPCODE: u8 = 0x91;
pub(super) fn report() -> serde_json::Value {
    serde_json::json!(
        { "step" : STEP, "opcode" : OPCODE, "angle_step" : [0x400, 0x800], "amplitudes" :
        [24, 8], "table_phases_per_axis" : 64, "private_object_fields" : [] }
    )
}
pub(super) fn patch(out: &mut [u8], source: &[u8], commands: &[u32]) -> Result<()> {
    if source
        .get((0x4952b4 - SOURCE_BASE) as usize..(0x4953c8 - SOURCE_BASE) as usize)
        .is_none_or(|b| {
            crate::sha(b) != "a7f3cfc9470c569d776f6b5e0e10dba6bb6227013a87089ab3bd9442882f369f"
        })
        || !commands.contains(&STEP)
        || commands
            .iter()
            .any(|pc| source.get((pc - SOURCE_BASE) as usize) == Some(&OPCODE))
        || out.get((STEP - SOURCE_BASE) as usize..(STEP - SOURCE_BASE + 4) as usize)
            != Some(&[0x4d, 0, 0, 0])
    {
        return Err("Orbit turn source, entry or opcode changed".into());
    }
    out[(STEP - SOURCE_BASE) as usize] = OPCODE;
    Ok(())
}
pub(super) fn emit_step(p: &mut Assembler, layout: &CartridgeLayout) -> Result<()> {
    p.label("bvm_op_91").assume_mode(CpuMode::NATIVE_M8_X16);
    emit(
        p,
        M::Jsl,
        A::AbsoluteLong,
        O::Long(crate::sfc_far_code::orbit_turn_entry(layout)?),
    );
    br(p, M::Bcs, "bvm_orbit_done");
    jmp(p, "bvm_op_4d");
    p.label("bvm_orbit_done");
    yield_now(p);
    Ok(())
}
pub(super) fn emit_far(p: &mut Assembler, layout: &CartridgeLayout) -> Result<()> {
    p.label("bvm_orbit_turn_far")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, crate::sfc_battle::ACTORS + 0x60);
    br(p, M::Bne, "bvm_orbit_active");
    implied(p, M::Clc);
    implied(p, M::Rtl);
    p.label("bvm_orbit_active");
    dp(p, M::Ldx, OBJ);
    m16(p);
    dp(p, M::Lda, BUDGET);
    imm16(p, M::Cmp, 3);
    br(p, M::Bcc, "bvm_orbit_slow");
    ax(p, M::Lda, 0x92);
    imm16(p, M::Cmp, 24);
    br(p, M::Bne, "bvm_orbit_slow");
    ax(p, M::Lda, 0x94);
    imm16(p, M::Cmp, 8);
    br(p, M::Bne, "bvm_orbit_slow");
    ax(p, M::Lda, 0x8c);
    ax(p, M::Ora, 0x8e);
    imm16(p, M::Bit, 0x03ff);
    br(p, M::Bne, "bvm_orbit_slow");
    ax(p, M::Lda, 0xa0);
    implied(p, M::Sec);
    imm16(p, M::Sbc, SOURCE_POOL as u16);
    dp(p, M::Sta, TMP);
    ax(p, M::Lda, 0xa2);
    imm16(p, M::Sbc, (SOURCE_POOL >> 16) as u16);
    br(p, M::Bne, "bvm_orbit_slow");
    dp(p, M::Lda, TMP);
    imm16(p, M::Cmp, OBJECTS * OBJECT_BYTES);
    br(p, M::Bcs, "bvm_orbit_slow");
    implied(p, M::Clc);
    imm16(p, M::Adc, POOL);
    implied(p, M::Tay);
    implied(p, M::Sec);
    dp(p, M::Sbc, OBJ);
    implied(p, M::Clc);
    imm16(p, M::Adc, OBJECT_BYTES - 1);
    imm16(p, M::Cmp, 2 * OBJECT_BYTES - 1);
    br(p, M::Bcs, "bvm_orbit_fast");
    p.label("bvm_orbit_slow")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    m8(p);
    implied(p, M::Clc);
    implied(p, M::Rtl);
    p.label("bvm_orbit_fast")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    for (angle, delta) in [(0x8c, 0x400), (0x8e, 0x800)] {
        ax(p, M::Lda, angle);
        implied(p, M::Sec);
        imm16(p, M::Sbc, delta);
        ax(p, M::Sta, angle);
    }
    for word in [0, 2] {
        ax(p, M::Lda, 0xa0 + word);
        ax(p, M::Sta, 0xa8 + word);
    }
    for (parent, origin) in [(0x1c, 0x80), (0x20, 0x84)] {
        ay(p, M::Lda, parent);
        ax(p, M::Sta, origin);
        ay(p, M::Lda, parent + 2);
        implied(p, M::Clc);
        imm16(p, M::Adc, 12);
        ax(p, M::Sta, origin + 2);
    }
    ay(p, M::Lda, 0x24);
    ax(p, M::Sta, 0x24);
    ay(p, M::Lda, 0x26);
    dp(p, M::Sta, TMP);
    ax(p, M::Lda, 0x8c);
    implied(p, M::Clc);
    imm16(p, M::Adc, 0x4000);
    br(p, M::Bmi, "bvm_orbit_behind");
    dp(p, M::Inc, TMP);
    br(p, M::Bra, "bvm_orbit_depth");
    p.label("bvm_orbit_behind")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    dp(p, M::Dec, TMP);
    p.label("bvm_orbit_depth");
    dp(p, M::Lda, TMP);
    ax(p, M::Sta, 0x26);
    dp(p, M::Ldy, OBJ);
    for (axis, angle, origin, position) in [("x", 0x8c, 0x80, 0x1c), ("y", 0x8e, 0x84, 0x20)] {
        dp(p, M::Ldx, OBJ);
        ax(p, M::Lda, angle);
        implied(p, M::Xba);
        imm16(p, M::And, 0x00ff);
        acc(p, M::Lsr);
        implied(p, M::Tax);
        for (word, name) in [(0, "low"), (2, "high")] {
            p.emit_label_ref(M::Lda, A::AbsoluteLongX, format!("bvm_orbit_{axis}_{name}"));
            if word == 0 {
                implied(p, M::Clc);
            }
            ay(p, M::Adc, origin + word);
            ay(p, M::Sta, position + word);
        }
    }
    dp(p, M::Dec, BUDGET);
    dp(p, M::Dec, BUDGET);
    m8(p);
    implied(p, M::Sec);
    implied(p, M::Rtl);
    let sine = &layout.section("bvm_sine")?.bytes;
    if sine.len() != 4096 * 4 {
        return Err("Orbit needs the exact source sine table".into());
    }
    for (axis, phase_shift, amplitude) in [("x", 0usize, 24i32), ("y", 0x400, -8)] {
        let values: Vec<i32> = (0..64)
            .map(|phase| {
                let at = ((phase * 64 + phase_shift) & 4095) * 4;
                let coefficient: i32 = (0..3)
                    .map(|j| i32::from(sine[at + j] as i8) << (j * 8))
                    .sum();
                coefficient * amplitude
            })
            .collect();
        for (word, name) in [(0, "low"), (2, "high")] {
            p.label(format!("bvm_orbit_{axis}_{name}"));
            p.data(
                values
                    .iter()
                    .flat_map(|v| v.to_le_bytes()[word..word + 2].to_vec())
                    .collect::<Vec<_>>(),
            );
        }
    }
    Ok(())
}
