use super::*;
const SOURCE_START: u32 = 0x46ade0;
const SOURCE_END: u32 = 0x46b2f4;
const SOURCE_HASH: &str = "8d4b4b73b0f0e1128e3fb1ce4c503e7219df38bebf78871bcdf952006f2ca8a4";
const LEAVES: [(u32, u32, u32); 6] = [
    (0x46b03c, 0x46b094, 6),
    (0x46b0b0, 0x46b108, 3),
    (0x46b124, 0x46b17c, 1),
    (0x46b198, 0x46b1f0, 6),
    (0x46b20c, 0x46b264, 3),
    (0x46b280, 0x46b2d8, 1),
];
pub(super) const DENSITY: [(u32, [u8; 4], [u8; 4]); 42] = [
    (
        0x46ae08,
        0x46b03cu32.to_le_bytes(),
        (GHOST + 8).to_le_bytes(),
    ),
    (
        0x46ae10,
        0x46b0b0u32.to_le_bytes(),
        (GHOST + 8).to_le_bytes(),
    ),
    (
        0x46ae24,
        0x46b198u32.to_le_bytes(),
        (GHOST + 8).to_le_bytes(),
    ),
    (
        0x46ae2c,
        0x46b20cu32.to_le_bytes(),
        (GHOST + 8).to_le_bytes(),
    ),
    (
        0x46ae38,
        0x46b03cu32.to_le_bytes(),
        (GHOST + 8).to_le_bytes(),
    ),
    (
        0x46ae40,
        0x46b0b0u32.to_le_bytes(),
        (GHOST + 8).to_le_bytes(),
    ),
    (
        0x46ae54,
        0x46b0b0u32.to_le_bytes(),
        (GHOST + 8).to_le_bytes(),
    ),
    (
        0x46ae5c,
        0x46b124u32.to_le_bytes(),
        (GHOST + 8).to_le_bytes(),
    ),
    (
        0x46ae64,
        0x46b198u32.to_le_bytes(),
        (GHOST + 8).to_le_bytes(),
    ),
    (
        0x46ae6c,
        0x46b20cu32.to_le_bytes(),
        (GHOST + 8).to_le_bytes(),
    ),
    (
        0x46ae80,
        0x46b20cu32.to_le_bytes(),
        (GHOST + 8).to_le_bytes(),
    ),
    (
        0x46ae88,
        0x46b280u32.to_le_bytes(),
        (GHOST + 8).to_le_bytes(),
    ),
    (
        0x46ae94,
        0x46b124u32.to_le_bytes(),
        (GHOST + 8).to_le_bytes(),
    ),
    (
        0x46aea8,
        0x46b280u32.to_le_bytes(),
        (GHOST + 8).to_le_bytes(),
    ),
    (
        0x46aeb0,
        0x46b124u32.to_le_bytes(),
        (GHOST + 8).to_le_bytes(),
    ),
    (
        0x46aebc,
        0x46b280u32.to_le_bytes(),
        (GHOST + 8).to_le_bytes(),
    ),
    (
        0x46aee8,
        0x46b03cu32.to_le_bytes(),
        (GHOST + 8).to_le_bytes(),
    ),
    (
        0x46aef0,
        0x46b0b0u32.to_le_bytes(),
        (GHOST + 8).to_le_bytes(),
    ),
    (
        0x46af04,
        0x46b198u32.to_le_bytes(),
        (GHOST + 8).to_le_bytes(),
    ),
    (
        0x46af0c,
        0x46b20cu32.to_le_bytes(),
        (GHOST + 8).to_le_bytes(),
    ),
    (
        0x46af18,
        0x46b03cu32.to_le_bytes(),
        (GHOST + 8).to_le_bytes(),
    ),
    (
        0x46af20,
        0x46b0b0u32.to_le_bytes(),
        (GHOST + 8).to_le_bytes(),
    ),
    (
        0x46af34,
        0x46b0b0u32.to_le_bytes(),
        (GHOST + 8).to_le_bytes(),
    ),
    (
        0x46af3c,
        0x46b124u32.to_le_bytes(),
        (GHOST + 8).to_le_bytes(),
    ),
    (
        0x46af44,
        0x46b198u32.to_le_bytes(),
        (GHOST + 8).to_le_bytes(),
    ),
    (
        0x46af4c,
        0x46b20cu32.to_le_bytes(),
        (GHOST + 8).to_le_bytes(),
    ),
    (
        0x46af60,
        0x46b20cu32.to_le_bytes(),
        (GHOST + 8).to_le_bytes(),
    ),
    (
        0x46af68,
        0x46b280u32.to_le_bytes(),
        (GHOST + 8).to_le_bytes(),
    ),
    (
        0x46af74,
        0x46b124u32.to_le_bytes(),
        (GHOST + 8).to_le_bytes(),
    ),
    (
        0x46af88,
        0x46b280u32.to_le_bytes(),
        (GHOST + 8).to_le_bytes(),
    ),
    (
        0x46af90,
        0x46b124u32.to_le_bytes(),
        (GHOST + 8).to_le_bytes(),
    ),
    (
        0x46af9c,
        0x46b280u32.to_le_bytes(),
        (GHOST + 8).to_le_bytes(),
    ),
    (
        0x46afc8,
        0x46b03cu32.to_le_bytes(),
        (GHOST + 8).to_le_bytes(),
    ),
    (
        0x46afd0,
        0x46b0b0u32.to_le_bytes(),
        (GHOST + 8).to_le_bytes(),
    ),
    (
        0x46afe4,
        0x46b198u32.to_le_bytes(),
        (GHOST + 8).to_le_bytes(),
    ),
    (
        0x46afec,
        0x46b20cu32.to_le_bytes(),
        (GHOST + 8).to_le_bytes(),
    ),
    (
        0x46aff8,
        0x46b03cu32.to_le_bytes(),
        (GHOST + 8).to_le_bytes(),
    ),
    (
        0x46b000,
        0x46b0b0u32.to_le_bytes(),
        (GHOST + 8).to_le_bytes(),
    ),
    (
        0x46b014,
        0x46b0b0u32.to_le_bytes(),
        (GHOST + 8).to_le_bytes(),
    ),
    (
        0x46b01c,
        0x46b198u32.to_le_bytes(),
        (GHOST + 8).to_le_bytes(),
    ),
    (
        0x46b024,
        0x46b20cu32.to_le_bytes(),
        (GHOST + 8).to_le_bytes(),
    ),
    (
        0x46b030,
        0x46b20cu32.to_le_bytes(),
        (GHOST + 8).to_le_bytes(),
    ),
];
pub(super) const RETIREMENTS: [(usize, u32, u32); 4] =
    [(24, 6, 48), (25, 3, 96), (27, 6, 48), (28, 3, 96)];
pub(super) fn report() -> serde_json::Value {
    serde_json::json!(
        { "source_start" : SOURCE_START, "source_end" : SOURCE_END, "source_sha256" :
        SOURCE_HASH, "private_object_fields" : [[0x84, 0x88], [0x92, 0x96]],
        "turn_opcode" : 0x90, "turn_wait_ticks" : 1, "density" : DENSITY.iter().map(|&
        (va, from, to) | { serde_json::json!({ "va" : va, "leaf" :
        u32::from_le_bytes(from), "target" : u32::from_le_bytes(to) }) }).collect::< Vec
        < _ >> (), "retirements" : RETIREMENTS.iter().map(|& (frame, speed, update) | {
        serde_json::json!({ "frame" : frame, "speed" : speed, "update" : update }) })
        .collect::< Vec < _ >> (), "leaves" : LEAVES.iter().enumerate().map(| (i, &
        (start, motion, speed)) | { let retirement = RETIREMENTS.iter().find(|&& (frame,
        _, _) | frame == 24 + i); serde_json::json!({ "start" : start, "motion" : motion,
        "end" : motion + 28, "speed" : speed, "retire_at" : retirement.map(|& (_, _,
        update) | update) }) }).collect::< Vec < _ >> () }
    )
}
pub(super) fn check_source(source: &[u8]) -> Result<()> {
    let range = (SOURCE_START - SOURCE_BASE) as usize..(SOURCE_END - SOURCE_BASE) as usize;
    if source
        .get(range)
        .is_none_or(|bytes| crate::sha(bytes) != SOURCE_HASH)
    {
        return Err("Vertical particle emitters or leaves changed".into());
    }
    Ok(())
}
pub(super) fn patch(out: &mut [u8], source: &[u8], commands: &[u32]) -> Result<()> {
    check_source(source)?;
    if commands
        .iter()
        .any(|&pc| source[(pc - SOURCE_BASE) as usize] == 0x90)
    {
        return Err("Vertical turn opcode already used by source".into());
    }
    for (i, &(start, motion, speed)) in LEAVES.iter().enumerate() {
        for (offset, bytes) in source.windows(4).enumerate() {
            let target = u32::from_le_bytes(bytes.try_into().unwrap());
            let va = SOURCE_BASE + offset as u32;
            if (target == start && !(SOURCE_START..LEAVES[0].0).contains(&va))
                || (target == motion && va != motion + 20)
                || ((start..motion + 28).contains(&target) && target != start && target != motion)
            {
                return Err("Unexpected vertical particle reference or interior entry".into());
            }
        }
        let init = (start - SOURCE_BASE) as usize;
        let at = (motion - SOURCE_BASE) as usize;
        if out.get(init + 20..init + 28) != Some(&[3, 0x84, 0, 0, 0, 0, 0x10, 1])
            || out.get(at..at + 4) != Some(&[0x2d, 0x0b, 0, 0])
            || out.get(at + 4..at + 8) != Some(&[0x02, 0, 1, 0])
            || out.get(at + 8..at + 16) != Some(&[0xbf, 0xa1, 0x92, 0x58, 0xbf, 0xa1, 0x94, 0x58])
            || [
                start,
                motion,
                motion + 4,
                motion + 8,
                motion + 12,
                motion + 16,
            ]
            .iter()
            .any(|pc| !commands.contains(pc))
        {
            return Err("Vertical particle specialization boundary changed".into());
        }
        out[init + 21] = 0x20;
        out[at..at + 4].copy_from_slice(&[0xc1, 0xa0, 0x1c, 0x80]);
        out[at + 8..at + 12].copy_from_slice(&[0x90, 0x82, 0x20, 0]);
        out[at + 12..at + 16].copy_from_slice(&(speed << 16).to_le_bytes());
        if let Some(&(_, retirement_speed, retirement)) =
            RETIREMENTS.iter().find(|&&(frame, _, _)| frame == 24 + i)
        {
            if retirement_speed != speed {
                return Err("Vertical particle retirement speed changed".into());
            }
            let original_lifetime: u16 = if speed == 6 { 150 } else { 200 };
            if out.get(at + 18..at + 20) != Some(original_lifetime.to_le_bytes().as_slice()) {
                return Err("Vertical particle lifetime changed".into());
            }
            out[at + 18..at + 20].copy_from_slice(&(retirement as u16).to_le_bytes());
        }
    }
    Ok(())
}
pub(super) fn emit_step(p: &mut Assembler, layout: &CartridgeLayout) -> Result<()> {
    p.label("bvm_op_90").assume_mode(CpuMode::NATIVE_M8_X16);
    emit(
        p,
        M::Jsl,
        A::AbsoluteLong,
        O::Long(crate::sfc_far_code::vertical_turn_entry(layout)?),
    );
    br(p, M::Bcs, "bvm_vertical_done");
    jmp(p, "bvm_op_b6");
    p.label("bvm_vertical_done");
    yield_now(p);
    Ok(())
}
pub(super) fn emit_far(p: &mut Assembler) {
    p.label("bvm_vertical_turn_far")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    dp(p, M::Ldx, OBJ);
    m16(p);
    ax(p, M::Lda, 0x60);
    imm16(p, M::Cmp, 2);
    br(p, M::Bcc, "bvm_vertical_slow");
    dp(p, M::Lda, BUDGET);
    imm16(p, M::Cmp, 2);
    br(p, M::Bcc, "bvm_vertical_slow");
    cword(p, 12);
    ax(p, M::Cmp, 0x3c);
    br(p, M::Bne, "bvm_vertical_slow");
    cword(p, 14);
    ax(p, M::Cmp, 0x3e);
    br(p, M::Bne, "bvm_vertical_slow");
    cword(p, 6);
    dp(p, M::Sta, TMP);
    ax(p, M::Lda, 0x22);
    implied(p, M::Sec);
    dp(p, M::Sbc, TMP);
    ax(p, M::Sta, 0x22);
    for word in [0, 2] {
        ax(p, M::Lda, 0x80 + word);
        ax(p, M::Sta, 0x1c + word);
    }
    ax(p, M::Dec, 0x60);
    dp(p, M::Dec, BUDGET);
    imm16(p, M::Lda, 1);
    ax(p, M::Sta, 0x5e);
    ax(p, M::Lda, 0x40);
    implied(p, M::Sec);
    imm16(p, M::Sbc, 4);
    ax(p, M::Sta, 0x40);
    dp(p, M::Sta, PCP);
    m8(p);
    implied(p, M::Sec);
    implied(p, M::Rtl);
    p.label("bvm_vertical_slow")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    m8(p);
    implied(p, M::Clc);
    implied(p, M::Rtl);
}
