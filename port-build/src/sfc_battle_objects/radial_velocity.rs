use super::*;
const START: u32 = 0x46bee0;
const END: u32 = 0x46bf5c;
const STEP: u32 = 0x46bf4c;
const OPCODE: u8 = 0xaa;
pub(super) fn contains(pc: u32) -> bool {
    (START..END).contains(&pc)
}
fn program(compiled: bool) -> Vec<u8> {
    let arithmetic = |opcode: u8, mode: u8| {
        if compiled {
            VARIANTS
                .iter()
                .find(|v| v.0 == opcode && v.1 == (mode & 0xf0) && v.2 == Some(mode & 15))
                .expect("Known own arithmetic form")
                .3
        } else {
            opcode
        }
    };
    let mut b = vec![8, 0, 0, 0];
    for (offset, value) in [
        (2, 0x4000u16),
        (0x26, 0xfff6),
        (0x2c, 1),
        (0x2a, 36),
        (0x14, 2),
    ] {
        b.extend([2, offset]);
        b.extend(value.to_le_bytes());
    }
    b.extend([0xff, 0, 0, 0]);
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
    b.extend([0x2b, 0, 22, 0, arithmetic(0x11, 0xa1), 0xa1, 0x28, 0x58]);
    b.extend([
        0x2b,
        0,
        5,
        0,
        arithmetic(0x11, 0x81),
        0x81,
        0x58,
        0,
        3,
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
    b.extend([8, 0, 0, 0]);
    for (offset, value) in [(0x1c, 460u32 << 16), (0x20, 156u32 << 16)] {
        b.extend([3, offset, 0, 0]);
        b.extend(value.to_le_bytes());
    }
    b.extend([0xff, 0, 0, 0]);
    assert_eq!(START + b.len() as u32, STEP - 4);
    b.extend([
        1,
        0,
        0,
        0,
        if compiled { OPCODE } else { 0x1e },
        0,
        0,
        0,
        6,
        0,
        100,
        0,
    ]);
    b.extend((STEP - 4).to_le_bytes());
    b.extend([0, 0, 0, 0]);
    assert_eq!(START + b.len() as u32, END);
    b
}
pub(super) fn patch(out: &mut [u8], source: &[u8], compiled: bool) -> Result<()> {
    for (start, end, hash) in [
        (
            0x46bba0,
            0x46bc00,
            "4f45db3db5054165723898d7cf3753814bd429d08a20740b39584a36bb6e7e98",
        ),
        (
            START,
            END,
            "bfa6c0aecc8b4ff7c003b38e6eeb84dcda2dc078d211f674a7c46c50eb4a086d",
        ),
    ] {
        if source
            .get((start - SOURCE_BASE) as usize..(end - SOURCE_BASE) as usize)
            .is_none_or(|b| crate::sha(b) != hash)
        {
            return Err("Constant-velocity burst source changed".into());
        }
    }
    for (offset, bytes) in source.windows(4).enumerate() {
        let target = u32::from_le_bytes(bytes.try_into().unwrap());
        if (START..END).contains(&target) {
            let va = SOURCE_BASE + offset as u32;
            let spawn = target == START
                && (0x46bba4..=0x46bbec).contains(&va)
                && (va - 0x46bba4).is_multiple_of(8);
            let own_loop = va == 0x46bf54 && target == 0x46bf40;
            if !spawn && !own_loop {
                return Err("Constant-velocity burst has an unexpected incoming reference".into());
            }
        }
    }
    let mut b = program(false);
    let range = (START - SOURCE_BASE) as usize..(END - SOURCE_BASE) as usize;
    let expected = if compiled {
        b.as_slice()
    } else {
        &source[range.clone()]
    };
    if out.get(range) != Some(expected) {
        return Err("Constant-velocity burst overlaps another transformation".into());
    }
    if compiled {
        b = program(true);
    }
    out[(START - SOURCE_BASE) as usize..(END - SOURCE_BASE) as usize].copy_from_slice(&b);
    Ok(())
}
pub(super) fn report() -> serde_json::Value {
    serde_json::json!(
        { "start" : START, "end" : END, "step" : STEP, "opcode" : OPCODE, "lifetime" :
        100, "private_object_fields" : [[0x3c, 0x44], [0x74, 0x7c], [0x80, 0x88], [0x92,
        0x96]], "program_sha256" : crate ::sha(& program(false)) }
    )
}
pub(super) fn emit_step(p: &mut Assembler) {
    p.label("bvm_op_aa").assume_mode(CpuMode::NATIVE_M8_X16);
    dp(p, M::Ldx, OBJ);
    m16(p);
    for (pos, velocity) in [(0x1c, 0x74), (0x20, 0x78)] {
        implied(p, M::Clc);
        for word in [0, 2] {
            ax(p, M::Lda, pos + word);
            ax(p, M::Adc, velocity + word);
            ax(p, M::Sta, pos + word);
        }
    }
    m8(p);
    advance_next(p, 4);
}
