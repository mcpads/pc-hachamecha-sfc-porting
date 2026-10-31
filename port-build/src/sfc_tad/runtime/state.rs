pub(super) const BURST: u16 = 0x1715;
pub(super) const SEQUENCE: u16 = 0x1716;
pub(super) const ELAPSED: u16 = 0x1717;
pub(super) const TICK: u16 = 0x170e;
pub(super) const LAST_TICK: u16 = 0x170f;
pub(super) const COMMON: u16 = 0x1724;
pub(super) const PENDING_REPEATS: u16 = 0x1727;
pub(super) const REPEATS: u16 = 0x1728;
pub(super) const CHANT_ROW: u16 = 0x1729;
pub(crate) const CHANT_REQUEST: u16 = 0x1764;
pub(super) const READY_WAIT: u16 = 0x1760;
pub(super) const READY_LAST: u16 = 0x1762;
pub(super) const SYNCHRONOUS: u16 = 0x1763;
pub(super) const READY: u16 = 0x1700;
pub(super) const TOKEN: u16 = 0x1701;
pub(super) const PTR: u16 = 0x1702;
pub(super) const SIZE: u16 = 0x1705;
pub(super) const OFFSET: u16 = 0x1707;
pub(super) const COUNTER: u16 = 0x1709;
pub(super) const TYPE: u16 = 0x170a;
pub(super) const STAGE: u16 = 0x170b;
pub(super) const BUDGET: u16 = 0x170c;
pub(super) const COMMAND: u16 = 0x1710;
pub(super) const PARAM: u16 = 0x1711;
pub(super) const SONG: u16 = 0x1713;
pub(super) const JINGLE: u16 = 0x171c;
pub(super) const PENDING_CLIP: u16 = 0x171e;
pub(super) const LOADED: u16 = 0x171f;
pub(super) const CLIP_TIMER: u16 = 0x1720;
pub(super) const BUSY: u16 = 0x172a;
pub(super) const CURRENT: u16 = 0x172e;
pub(super) const POSITION: u16 = 0x172f;
pub(super) const AHEAD: u16 = 0x1731;
pub(super) const PROGRAM: u16 = 0x172b;
pub(super) const FRAME_BYTES: u16 = 0x1736;
pub(super) const RING: u16 = 0x1734;
pub(super) const LEFT: u16 = 0x1738;
pub(super) const CLIP_PTR: u16 = 0x173a;
pub(super) const STARTED: u16 = 0x173d;
pub(super) const FAULT: u16 = 0x175e;
pub(super) const STOPPED: u16 = 0x175f;
pub(super) const STATE_START: u16 = READY;
pub(super) const STATE_END: u16 = CHANT_REQUEST + 1;
pub(super) const SLOT_STRIDE: u16 = 32;
pub(super) const SLOTS: [u16; 2] = [0, SLOT_STRIDE];
pub(super) const TRANSFER_POINTER: u8 = 0xf0;
pub(super) const TRANSFER_POINTER_BYTES: u8 = 3;
const _: () = {
    const GLOBALS: &[(u16, u16)] = &[
        (READY, 1),
        (TOKEN, 1),
        (PTR, 3),
        (SIZE, 2),
        (OFFSET, 2),
        (COUNTER, 1),
        (TYPE, 1),
        (STAGE, 1),
        (BUDGET, 1),
        (TICK, 1),
        (LAST_TICK, 1),
        (COMMAND, 1),
        (PARAM, 2),
        (SONG, 1),
        (BURST, 1),
        (SEQUENCE, 1),
        (ELAPSED, 1),
        (JINGLE, 2),
        (COMMON, 3),
        (BUSY, 1),
        (READY_WAIT, 2),
        (READY_LAST, 1),
        (SYNCHRONOUS, 1),
        (CHANT_REQUEST, 1),
        (FAULT, 1),
        (STOPPED, 1),
        (super::MUSIC, 1),
        (super::PENDING, 1),
        (super::RETURN, 1),
        (super::TIMER, 2),
        (super::JOB, 1),
        (super::EXCHANGES, 1),
        (super::WANTED, 1),
    ];
    const SLOT_FIELDS: &[(u16, u16)] = &[
        (PENDING_CLIP, 1),
        (LOADED, 1),
        (CLIP_TIMER, 2),
        (CURRENT, 1),
        (POSITION, 2),
        (AHEAD, 2),
        (RING, 2),
        (PROGRAM, 1),
        (FRAME_BYTES, 2),
        (LEFT, 2),
        (CLIP_PTR, 3),
        (STARTED, 1),
        (PENDING_REPEATS, 1),
        (REPEATS, 1),
        (CHANT_ROW, 1),
    ];
    let mut ranges = [(0u16, 0u16); GLOBALS.len() + SLOT_FIELDS.len() * SLOTS.len()];
    let mut n = 0;
    let mut i = 0;
    while i < GLOBALS.len() {
        ranges[n] = GLOBALS[i];
        n += 1;
        i += 1;
    }
    let mut slot = 0;
    while slot < SLOTS.len() {
        i = 0;
        while i < SLOT_FIELDS.len() {
            ranges[n] = (SLOT_FIELDS[i].0 + SLOTS[slot], SLOT_FIELDS[i].1);
            n += 1;
            i += 1;
        }
        slot += 1;
    }
    i = 0;
    while i < n {
        assert!(ranges[i].0 >= STATE_START && ranges[i].0 + ranges[i].1 <= STATE_END);
        let mut j = i + 1;
        while j < n {
            assert!(
                ranges[i].0 + ranges[i].1 <= ranges[j].0
                    || ranges[j].0 + ranges[j].1 <= ranges[i].0
            );
            j += 1;
        }
        i += 1;
    }
};
