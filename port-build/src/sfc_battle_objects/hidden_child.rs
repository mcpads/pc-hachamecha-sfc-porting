use super::*;
const OPCODE: u8 = 0x9d;
pub(super) fn patch(out: &mut [u8], source: &[u8], commands: &[u32]) -> Result<()> {
    let at = (GHOST - SOURCE_BASE) as usize;
    if source.get(at..at + 16) != Some(&[0; 16])
        || out.get(at..at + 16)
            != Some(&[
                0x2b, 0, 0xff, 0xff, 0x2b, 0, 0xff, 0xff, 0x2b, 0, 0xff, 0xff, 0, 0x96, 0, 0,
            ])
        || commands
            .iter()
            .any(|pc| out.get((pc - SOURCE_BASE) as usize) == Some(&OPCODE))
    {
        return Err("Hidden child program or opcode ownership changed".into());
    }
    out[at] = OPCODE;
    Ok(())
}
pub(crate) fn report(rules: &w65c816::AssembledProgram) -> Result<serde_json::Value> {
    let at = |label: &str| -> Result<u32> {
        Ok(rules
            .label_location(label)
            .ok_or("Missing hidden child label")?
            .to_u32())
    };
    let start = at("bvm_op_9d")?;
    Ok(serde_json::json!(
        { "helper_entry" : start, "helper_bytes" : at("bvm_hidden_child_end") ? -
        start, "opcode" : OPCODE, "source_entry" : GHOST, "retained_entries" : [GHOST
        + 4, GHOST + 8, GHOST + 12, GHOST + 13], "ordered_rng_steps" : 3,
        "unused_results_elided" : 2, "private_object_fields" : [], "new_wram" : [] }
    ))
}
pub(super) fn emit_step(p: &mut Assembler) {
    p.label("bvm_op_9d").assume_mode(CpuMode::NATIVE_M8_X16);
    implied(p, M::Phb);
    implied(p, M::Phd);
    m16(p);
    imm16(p, M::Lda, 0);
    implied(p, M::Tcd);
    m8(p);
    implied(p, M::Pha);
    implied(p, M::Plb);
    jsr(p, "encounter_rng");
    jsr(p, "encounter_rng");
    implied(p, M::Pld);
    implied(p, M::Plb);
    dp(p, M::Ldx, OBJ);
    advance(p, 8);
    jmp(p, "bvm_op_2b");
    p.label("bvm_hidden_child_end");
}
