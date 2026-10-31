use super::*;
const SPAWN: u32 = 0x495298;
const OPCODE: u8 = 0x95;
const GHOST_ENTRY: u32 = GHOST + 13;
const GHOST_OPCODE: u8 = 0x96;
const SOURCE_START: u32 = 0x4950d0;
const SOURCE_END: u32 = 0x4954d4;
const SOURCE_HASH: &str = "10c24e62072e6d12b7c6ac75d859152ec09758d7d9ccc6fc7cb33fd851cf4659";
pub(super) fn report() -> serde_json::Value {
    serde_json::json!(
        { "spawn" : SPAWN, "opcode" : OPCODE, "source_start" : SOURCE_START, "source_end"
        : SOURCE_END, "source_sha256" : SOURCE_HASH, "visible_children_per_emitter" : 5,
        "allocated_children_per_emitter" : 20, "ghost" : GHOST_ENTRY, "ghost_opcode" :
        GHOST_OPCODE, "source_rng_divisors" : [16, 24, 48], "unused_rng_steps" : 3,
        "keep_final_child" : true, "private_object_fields" : [] }
    )
}
pub(super) fn check_source(source: &[u8]) -> Result<()> {
    let range = (SOURCE_START - SOURCE_BASE) as usize..(SOURCE_END - SOURCE_BASE) as usize;
    if source
        .get(range)
        .is_none_or(|bytes| crate::sha(bytes) != SOURCE_HASH)
    {
        return Err("Jugem emitter or leaf source changed".into());
    }
    Ok(())
}
pub(super) fn patch(out: &mut [u8], source: &[u8], commands: &[u32]) -> Result<()> {
    let at = (SPAWN - SOURCE_BASE) as usize;
    if source.get(at..at + 20)
        != Some(&[
            7, 1, 0, 0, 0xc8, 0x53, 0x49, 0, 0x1f, 0, 0, 0, 6, 0, 20, 0, 0x98, 0x52, 0x49, 0,
        ])
        || !commands.contains(&SPAWN)
        || commands.iter().any(|pc| {
            source
                .get((pc - SOURCE_BASE) as usize)
                .is_some_and(|b| [OPCODE, GHOST_OPCODE].contains(b))
        })
    {
        return Err("Jugem density spawn, loop or opcode changed".into());
    }
    let ghost = (GHOST - SOURCE_BASE) as usize;
    if out.get(ghost..ghost + 16)
        != Some(&[
            0x2b, 0, 0xff, 0xff, 0x2b, 0, 0xff, 0xff, 0x2b, 0, 0xff, 0xff, 0, 0, 0, 0,
        ])
    {
        return Err("Existing ghost terminator or retained padding changed".into());
    }
    out[at] = OPCODE;
    out[(GHOST_ENTRY - SOURCE_BASE) as usize] = GHOST_OPCODE;
    Ok(())
}
pub(super) fn emit_step(p: &mut Assembler) {
    p.label("bvm_op_95").assume_mode(CpuMode::NATIVE_M8_X16);
    dp(p, M::Ldx, OBJ);
    m16(p);
    ax(p, M::Lda, 0x3c);
    imm16(p, M::Cmp, SPAWN as u16);
    br(p, M::Bne, "bvm_jugem_ghost");
    ax(p, M::Lda, 0x3e);
    imm16(p, M::Cmp, (SPAWN >> 16) as u16);
    br(p, M::Bne, "bvm_jugem_ghost");
    ax(p, M::Lda, 0x60);
    imm16(p, M::And, 3);
    imm16(p, M::Cmp, 1);
    br(p, M::Bne, "bvm_jugem_ghost");
    m8(p);
    jmp(p, "bvm_op_07");
    p.label("bvm_jugem_ghost")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    m8(p);
    imm8(p, M::Lda, 1);
    jsr(p, "bvm_alloc");
    m16(p);
    implied(p, M::Txa);
    dp(p, M::Sta, COUNT);
    br(p, M::Bne, "bvm_jugem_allocated");
    jmp(p, "bvm_07_none");
    p.label("bvm_jugem_allocated")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    imm16(p, M::Lda, GHOST_ENTRY as u16);
    ax(p, M::Sta, 0x40);
    imm16(p, M::Lda, (GHOST_ENTRY >> 16) as u16);
    ax(p, M::Sta, 0x42);
    jmp(p, "bvm_07_link_result");
    p.label("bvm_op_96").assume_mode(CpuMode::NATIVE_M8_X16);
    implied(p, M::Phb);
    implied(p, M::Phd);
    m16(p);
    imm16(p, M::Lda, 0);
    implied(p, M::Tcd);
    m8(p);
    implied(p, M::Pha);
    implied(p, M::Plb);
    imm8(p, M::Lda, 3);
    jsr(p, "encounter_rng_skip");
    implied(p, M::Pld);
    implied(p, M::Plb);
    jmp(p, "bvm_op_00");
}
