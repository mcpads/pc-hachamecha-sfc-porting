use super::*;
const START: u32 = 0x4bd750;
const END: u32 = 0x4bd7d8;
const LOOP: u32 = 0x4bd7bc;
pub(super) fn contains(pc: u32) -> bool {
    (START..END).contains(&pc)
}
fn program(compiled: bool) -> Vec<u8> {
    let arithmetic = |opcode: u8, mode: u8| {
        if compiled {
            VARIANTS
                .iter()
                .find(|v| v.0 == opcode && v.1 == mode & 0xf0 && v.2 == Some(mode & 15))
                .expect("Known linked velocity arithmetic")
                .3
        } else {
            opcode
        }
    };
    let mut b = vec![8, 0, 0, 0];
    for (offset, value) in [
        (2, 0x4000u16),
        (0x26, 0xffff),
        (0x2c, 2),
        (0x2a, 36),
        (0x14, 2),
    ] {
        b.extend([2, offset]);
        b.extend(value.to_le_bytes());
    }
    b.extend([0xff, 0, 0, 0, arithmetic(0x12, 0xa0), 0xa0, 0xa8, 0xa0]);
    b.extend([
        0x2b,
        0,
        0xff,
        0xff,
        arithmetic(0x11, 0xa0),
        0xa0,
        0x8c,
        0x58,
        arithmetic(0x11, 0xa0),
        0xa0,
        0x8e,
        0x58,
    ]);
    b.extend([0x2b, 0, 12, 0, arithmetic(0x11, 0xa1), 0xa1, 0x28, 0x58]);
    b.extend([
        0x2b,
        0,
        16,
        0,
        arithmetic(0x11, 0x81),
        0x81,
        0x58,
        0,
        1,
        0,
        0,
        0,
    ]);
    b.extend([
        arithmetic(0x11, 0xa0),
        0xa0,
        0x92,
        0x58,
        arithmetic(0x11, 0xa0),
        0xa0,
        0x94,
        0x58,
        0x2d,
        0x0b,
        0,
        0,
    ]);
    b.extend([
        arithmetic(0x12, 0xa0),
        0xa0,
        0x74,
        0x1c,
        arithmetic(0x12, 0xa0),
        0xa0,
        0x78,
        0x20,
    ]);
    b.extend([
        arithmetic(0x12, 0xb0),
        0xb0,
        0x1c,
        0x80,
        arithmetic(0x12, 0xb0),
        0xb0,
        0x20,
        0x84,
    ]);
    for (pos, value) in [(0x1c, 96u32 << 16), (0x20, 64u32 << 16)] {
        b.extend([arithmetic(0x12, 0x81), 0x81, pos, 0]);
        b.extend(value.to_le_bytes());
    }
    assert_eq!(START + b.len() as u32, LOOP);
    b.extend([
        1,
        0,
        0,
        0,
        if compiled { 0xaa } else { 0x1e },
        0,
        0,
        0,
        6,
        0,
        50,
        0,
    ]);
    b.extend(LOOP.to_le_bytes());
    b.extend([0, 0, 0, 0]);
    b.resize((END - START) as usize, 0);
    b
}
pub(super) fn patch(out: &mut [u8], source: &[u8], compiled: bool) -> Result<()> {
    if source
        .get((0x4bd4c8 - SOURCE_BASE) as usize..(END - SOURCE_BASE) as usize)
        .is_none_or(|b| {
            crate::sha(b) != "0ad033223f81da522964c2d40513ae273ccf3f761321863198b5374cc9580150"
        })
    {
        return Err("Linked burst parent or leaf changed".into());
    }
    for (offset, bytes) in source.windows(4).enumerate() {
        let target = u32::from_le_bytes(bytes.try_into().unwrap());
        if contains(target) {
            let va = SOURCE_BASE + offset as u32;
            let spawn = target == START
                && (0x4bd4ec..=0x4bd738).contains(&va)
                && (va - 0x4bd4ec).is_multiple_of(12);
            let own_loop = va == 0x4bd7d0 && target == LOOP;
            if !spawn && !own_loop {
                return Err("Linked burst has an unexpected incoming reference".into());
            }
        }
    }
    let range = (START - SOURCE_BASE) as usize..(END - SOURCE_BASE) as usize;
    let mut expected = if compiled {
        program(false)
    } else {
        source[range.clone()].to_vec()
    };
    if !compiled {
        expected[(0x4bd7ce - START) as usize..(0x4bd7d0 - START) as usize]
            .copy_from_slice(&50u16.to_le_bytes());
    }
    if out.get(range.clone()) != Some(expected.as_slice()) {
        return Err("Linked burst overlaps another transformation".into());
    }
    out[range].copy_from_slice(&program(compiled));
    Ok(())
}
pub(super) fn report() -> serde_json::Value {
    serde_json::json!(
        { "start" : START, "end" : END, "step" : LOOP + 4, "opcode" : 0xaa, "lifetime" :
        50, "private_object_fields" : [[0x3c, 0x44], [0x74, 0x7c], [0x80, 0x88], [0x92,
        0x96]], "program_sha256" : crate ::sha(& program(false)) }
    )
}
