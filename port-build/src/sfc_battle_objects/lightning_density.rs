use super::*;
const START: u32 = 0x46ca20;
const AFTERGLOW: u32 = START + 32;
const END: u32 = START + 60;
const POINTERS: [u32; 2] = [0x4947d0, 0x4947e8];
fn program() -> Vec<u8> {
    let mut b = vec![0x2b, 0, 0xff, 0xff, 0x2b, 0, 8, 0, 1, 0, 0, 0, 6, 0, 12, 0];
    b.extend((START + 8).to_le_bytes());
    b.extend([7, 1, 0, 0]);
    b.extend(AFTERGLOW.to_le_bytes());
    b.extend([0, 0, 0, 0]);
    assert_eq!(START + b.len() as u32, AFTERGLOW);
    b.extend([0x2b, 0, 0xff, 0xff, 0x2b, 0, 8, 0, 0x2b, 0, 0, 0x10]);
    b.extend([1, 0, 0, 0, 6, 0, 10, 0]);
    b.extend((AFTERGLOW + 12).to_le_bytes());
    b.extend([0, 0, 0, 0]);
    assert_eq!(START + b.len() as u32, END);
    b
}
pub(super) fn patch(out: &mut [u8], source: &[u8]) -> Result<()> {
    let at = |pc: u32| (pc - SOURCE_BASE) as usize;
    if crate::sha(&source[at(0x4947a4)..at(0x4949f0)])
        != "de921795bb6d09558fd3464a0f6259696427353a8c6ffaa567f3cbfdd37dfda9"
        || source[at(START)..at(END)].iter().any(|b| *b != 0)
        || out[at(START)..at(END)].iter().any(|b| *b != 0)
    {
        return Err("Lightning density source or unused script interval changed".into());
    }
    for (offset, bytes) in source.windows(4).enumerate() {
        let target = u32::from_le_bytes(bytes.try_into().unwrap());
        if (START..END).contains(&target) {
            return Err("Existing pointer to Lightning ghost interval".into());
        }
        if (0x494854..0x4949f0).contains(&target)
            && !(0x4947a4..0x4949f0).contains(&(SOURCE_BASE + offset as u32))
        {
            return Err("Foreign Lightning child consumer".into());
        }
    }
    for pc in POINTERS {
        if source[at(pc - 4)..at(pc)] != [7, 1, 0, 0]
            || source[at(pc)..at(pc + 4)] != 0x494854u32.to_le_bytes()
            || out[at(pc)..at(pc + 4)] != 0x494854u32.to_le_bytes()
        {
            return Err("Lightning density spawn changed".into());
        }
        out[at(pc)..at(pc + 4)].copy_from_slice(&START.to_le_bytes());
    }
    out[at(START)..at(END)].copy_from_slice(&program());
    Ok(())
}
pub(super) fn report() -> serde_json::Value {
    serde_json::json!(
        { "spawn_pointers" : POINTERS, "kept_ordinals" : [0, 2, 3, 5], "omitted_ordinals"
        : [1, 4], "ghost_start" : START, "afterglow_start" : AFTERGLOW, "ghost_end" :
        END, "ghost_bytes" : END - START, "inward_yields" : 12, "afterglow_yields" : 10,
        "rng_draws" : [2, 3], "allocations_retained" : true, "policy" :
        "four visible sparks per six; invisible records retain RNG and allocation timing"
        }
    )
}
