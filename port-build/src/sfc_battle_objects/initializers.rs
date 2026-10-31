use super::*;
const OPCODE: u8 = 0x93;
const LISTS: [(u32, u32, &str); 3] = [
    (
        0x4bcabc,
        0x4bcb04,
        "8ee54dd37302712d55c9c53086aab3550eb3d1646edd9b50213852b2ea1add0a",
    ),
    (
        0x49561c,
        0x49565c,
        "e9630621e32e755aa9307d3b2d511985c91676356d600d93090b081a781135d5",
    ),
    (
        0x495ae8,
        0x495b30,
        "d3656c9eb8f0225a95287658aec3cfcfc48607d01dbd373d196625cf291bc76b",
    ),
];
pub(super) fn report() -> serde_json::Value {
    serde_json::json!(
        { "opcode" : OPCODE, "lists" : LISTS.iter().map(|& (start, end, sha) |
        serde_json::json!({ "start" : start, "end" : end, "source_sha256" : sha }))
        .collect::< Vec < _ >> (), "private_object_fields" : [], "fallback" :
        "opcode 08", "full_source_pc_guard" : true }
    )
}
fn parse(bytes: &[u8]) -> Result<Vec<(u16, u16)>> {
    if bytes.get(..4) != Some(&[8, 0, 0, 0]) {
        return Err("Initializer header changed".into());
    }
    let mut at = 4;
    let mut words = Vec::new();
    loop {
        let row = bytes.get(at..at + 4).ok_or("Truncated initializer")?;
        if row == [0xff, 0, 0, 0] {
            if at + 4 != bytes.len() {
                return Err("Trailing initializer data".into());
            }
            return Ok(words);
        }
        let width = match row[0] {
            2 => 2,
            3 if row[2..] == [0, 0] => 4,
            _ => return Err("Unsupported initializer row".into()),
        };
        let offset = u16::from(row[1]);
        if !(offset == 0 || (0x14..=0x30).contains(&offset) || (0x80..=0x94).contains(&offset))
            || offset % 2 != 0
            || offset + width > OBJECT_BYTES
        {
            return Err("Initializer writes a control field".into());
        }
        let value_at = at + if width == 4 { 4 } else { 2 };
        let value = bytes
            .get(value_at..value_at + width as usize)
            .ok_or("Truncated initializer value")?;
        for (i, word) in value.as_chunks::<2>().0.iter().enumerate() {
            words.push((offset + 2 * i as u16, u16::from_le_bytes(*word)));
        }
        at += if width == 4 { 8 } else { 4 };
    }
}
pub(super) fn patch(out: &mut [u8], source: &[u8], commands: &[u32]) -> Result<()> {
    if commands
        .iter()
        .any(|pc| out.get((pc - SOURCE_BASE) as usize) == Some(&OPCODE))
    {
        return Err("Compiled initializer opcode already used".into());
    }
    for (start, end, sha) in LISTS {
        let range = (start - SOURCE_BASE) as usize..(end - SOURCE_BASE) as usize;
        let bytes = source
            .get(range.clone())
            .ok_or("Missing initializer source")?;
        if crate::sha(bytes) != sha
            || out.get(range.clone()) != Some(bytes)
            || !commands.contains(&start)
            || !commands.contains(&end)
        {
            return Err("Compiled initializer source or boundary changed".into());
        }
        parse(bytes)?;
        out[range.start] = OPCODE;
    }
    Ok(())
}
pub(super) fn emit_step(p: &mut Assembler, layout: &CartridgeLayout) -> Result<()> {
    p.label("bvm_op_93").assume_mode(CpuMode::NATIVE_M8_X16);
    emit(
        p,
        M::Jsl,
        A::AbsoluteLong,
        O::Long(crate::sfc_far_code::initializer_entry(layout)?),
    );
    br(p, M::Bcs, "bvm_initializer_done");
    jmp(p, "bvm_op_08");
    p.label("bvm_initializer_done");
    jmp(p, "bvm_08_end");
    Ok(())
}
pub(super) fn placed_words(
    layout: &CartridgeLayout,
    start: u32,
    end: u32,
    sha: &str,
) -> Result<Vec<(u16, u16)>> {
    let file = IMAGE_OFFSET + (start - SOURCE_BASE) as usize;
    let length = (end - start) as usize;
    let mut bytes = None;
    let mut part = 0;
    while let Ok(section) = layout.section(&format!("battle_scripts_{part}")) {
        if file >= section.offset && file + length <= section.offset + section.bytes.len() {
            bytes =
                Some(section.bytes[file - section.offset..file - section.offset + length].to_vec());
            break;
        }
        part += 1;
    }
    let mut bytes = bytes.ok_or("Compiled initializer absent from kept scripts")?;
    if bytes[0] != OPCODE {
        return Err("Compiled initializer tag changed".into());
    }
    bytes[0] = 8;
    if crate::sha(&bytes) != sha {
        return Err("Kept initializer data changed".into());
    }
    parse(&bytes)
}
pub(super) fn emit_far(p: &mut Assembler, layout: &CartridgeLayout) -> Result<()> {
    p.label("bvm_initializer_far")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    dp(p, M::Ldx, OBJ);
    m16(p);
    for (i, (start, end, sha)) in LISTS.into_iter().enumerate() {
        let next = format!("bvm_initializer_next_{i}");
        for (field, value) in [(0x40, start as u16), (0x42, (start >> 16) as u16)] {
            ax(p, M::Lda, field);
            imm16(p, M::Cmp, value);
            br(p, M::Bne, &next);
        }
        for (offset, value) in placed_words(layout, start, end, sha)? {
            if value == 0 {
                ax(p, M::Stz, offset);
            } else {
                imm16(p, M::Lda, value);
                ax(p, M::Sta, offset);
            }
        }
        imm16(p, M::Ldy, (end - start - 4) as u16);
        m8(p);
        implied(p, M::Sec);
        implied(p, M::Rtl);
        p.label(next).assume_mode(CpuMode::NATIVE_M16_X16);
    }
    m8(p);
    implied(p, M::Clc);
    implied(p, M::Rtl);
    Ok(())
}
