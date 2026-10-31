use super::*;
const ENTRIES: [(u32, u8, u16, u32); 2] = [
    (0x4948c4, 0x9e, 12, 0x4948b4),
    (0x494944, 0x9f, 10, 0x494934),
];
pub(super) fn patch(out: &mut [u8], source: &[u8], commands: &[u32]) -> Result<()> {
    let at = |pc: u32| (pc - SOURCE_BASE) as usize;
    if crate::sha(&source[at(0x4947a4)..at(0x4949f0)])
        != "de921795bb6d09558fd3464a0f6259696427353a8c6ffaa567f3cbfdd37dfda9"
        || commands
            .iter()
            .any(|pc| out.get(at(*pc)).is_some_and(|op| [0x9e, 0x9f].contains(op)))
    {
        return Err("Lightning turn source or opcode changed".into());
    }
    for (pc, opcode, count, target) in ENTRIES {
        let mut expected = vec![0x06, 0, count as u8, 0];
        expected.extend(target.to_le_bytes());
        let motion = if opcode == 0x9e { 0xc0 } else { 0xbf };
        if out.get(at(pc)..at(pc) + 8) != Some(expected.as_slice())
            || out.get(at(target)..at(pc))
                != Some(&[
                    motion,
                    0xa2 - u8::from(opcode == 0x9f),
                    0x92,
                    0x96,
                    motion,
                    0xa2 - u8::from(opcode == 0x9f),
                    0x94,
                    0x96,
                    0x2d,
                    0x0b,
                    0,
                    0,
                    1,
                    0,
                    0,
                    0,
                ])
            || [target, target + 4, target + 8, target + 12, pc, pc + 8]
                .iter()
                .any(|entry| !commands.contains(entry))
        {
            return Err("Lightning loop or interior commands changed".into());
        }
        out[at(pc)] = opcode;
    }
    Ok(())
}
pub(crate) fn report(rules: &w65c816::AssembledProgram) -> Result<serde_json::Value> {
    let at = |name: &str| -> Result<u32> {
        Ok(rules
            .label_location(name)
            .ok_or("Missing Lightning turn label")?
            .to_u32())
    };
    let start = at("bvm_op_9e")?;
    Ok(serde_json::json!(
        { "source_entries" : ENTRIES.map(| entry | entry.0), "opcodes" : [0x9e,
        0x9f], "helper_entry" : start, "helper_bytes" : at("bvm_lightning_turn_end")
        ? - start, "private_object_fields" : [], "new_wram" : [], "projection" :
        "original absolute XY on every turn", "fallback" :
        "original loop for changed latch, count zero/one or budget zero/one",
        "interior_entries" :
        "original radius, projection and yield commands retained" }
    ))
}
pub(super) fn emit_step(p: &mut Assembler) {
    for (pc, opcode, _, target) in ENTRIES {
        let label = |name: &str| format!("bvm_lightning_{opcode:02x}_{name}");
        p.label(format!("bvm_op_{opcode:02x}"))
            .assume_mode(CpuMode::NATIVE_M8_X16);
        dp(p, M::Ldx, OBJ);
        m16(p);
        ax(p, M::Lda, 0x3c);
        imm16(p, M::Cmp, target as u16);
        br(p, M::Bne, &label("fallback"));
        ax(p, M::Lda, 0x3e);
        imm16(p, M::Cmp, (target >> 16) as u16);
        br(p, M::Bne, &label("fallback"));
        ax(p, M::Lda, 0x60);
        imm16(p, M::Cmp, 2);
        br(p, M::Bcc, &label("fallback"));
        dp(p, M::Lda, BUDGET);
        imm16(p, M::Cmp, 2);
        br(p, M::Bcs, &label("turn"));
        p.label(label("fallback"))
            .assume_mode(CpuMode::NATIVE_M16_X16);
        m8(p);
        jmp(p, "bvm_op_06");
        p.label(label("turn")).assume_mode(CpuMode::NATIVE_M16_X16);
        ax(p, M::Dec, 0x60);
        dp(p, M::Dec, BUDGET);
        ax(p, M::Lda, 0x96);
        dp(p, M::Sta, TMP);
        for field in [0x92, 0x94] {
            ax(p, M::Lda, field);
            implied(p, if opcode == 0x9e { M::Sec } else { M::Clc });
            dp(p, if opcode == 0x9e { M::Sbc } else { M::Adc }, TMP);
            ax(p, M::Sta, field);
        }
        imm16(p, M::Ldy, 1);
        m8(p);
        imm8(p, M::Lda, 0x0b);
        dp(p, M::Sta, AXIS);
        jsr(p, "bvm_2d_loaded");
        dp(p, M::Ldx, OBJ);
        m16(p);
        imm16(p, M::Lda, pc as u16);
        implied(p, M::Clc);
        implied(p, M::Clv);
        m8(p);
        yield_now(p);
    }
    p.label("bvm_lightning_turn_end");
}
