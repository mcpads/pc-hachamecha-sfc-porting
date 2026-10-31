pub(crate) const INPUT_RATE: f64 = 5000.0;
pub(crate) const RATES: [f64; 4] = [5000.0, 8125.0, 11025.0, 16250.0];
pub(crate) const PITCHES: [u16; 4] = [640, 1040, 1411, 2080];
pub(crate) const fn frame_bytes(profile: usize) -> u16 {
    (32040_u64 * PITCHES[profile] as u64 * 9).div_ceil(4096 * 16 * 60) as u16
}
pub(crate) const PITCH: u16 = 640;
pub(crate) const PLAYBACK_RATE: f64 = 32000.0 * PITCH as f64 / 4096.0;
pub(crate) const FRAMES_PER_SECOND: f64 = 60.0988;
pub(crate) const RING_BYTES: usize = 256 * 9;
pub(crate) const IO_VERSION: u64 = 23;
pub(crate) const TRANSFER_BYTES: u16 = 96;
pub(crate) const BURSTS_PER_FRAME: u8 = 2;
pub(crate) const MAX_SERVICE_FRAMES: u8 = 4;
pub(crate) const START_BYTES: u16 = 768;
pub(crate) const REFILL_TARGET: u16 = 1024;
pub(crate) const FRAME_BYTES: u16 = frame_bytes(3);
pub(crate) const MAX_CLIP_FRAMES: u16 = 480;
const DECODE_LOOKAHEAD_BYTES: u16 = 18;
const MAX_CONSUMPTION_OVERCOUNT: u16 = MAX_CLIP_FRAMES;
const _: () = {
    let mut profile = 0;
    while profile < RATES.len() {
        let minimum =
            (32000.0 * PITCHES[profile] as f64 / 4096.0 * 9.0 / 16.0 / FRAMES_PER_SECOND) as u16;
        assert!(frame_bytes(profile) - minimum <= 1);
        profile += 1;
    }
    assert!(RING_BYTES % 9 == 0 && RING_BYTES % TRANSFER_BYTES as usize == 0);
    assert!(TRANSFER_BYTES % 3 == 0 && TRANSFER_BYTES < 256);
    assert!(FRAME_BYTES < TRANSFER_BYTES * BURSTS_PER_FRAME as u16);
    assert!(START_BYTES <= REFILL_TARGET);
    assert!(BURSTS_PER_FRAME == 2);
    assert!(FRAME_BYTES * (MAX_SERVICE_FRAMES as u16) + DECODE_LOOKAHEAD_BYTES < START_BYTES);
    assert!(
        REFILL_TARGET as usize
            + TRANSFER_BYTES as usize * BURSTS_PER_FRAME as usize * (MAX_SERVICE_FRAMES as usize)
            + MAX_CONSUMPTION_OVERCOUNT as usize
            + (DECODE_LOOKAHEAD_BYTES as usize)
            < RING_BYTES
    );
};

pub(crate) fn effect_program(slot: u8, profile: usize) -> String {
    let ticks = (f64::from(MAX_CLIP_FRAMES) / FRAMES_PER_SECOND * 125.0).ceil() as u32;
    let first = ticks.min(256);
    let pitch = PITCHES[profile];
    let mut program = format!(
        "=== stream{slot}_{profile} === interruptible one\nset_instrument stream{slot}\nset_volume 200\nplay_pitch {pitch} no_keyoff {first}\n"
    );
    let mut left = ticks - first;
    while left > 0 {
        let part = left.min(256);
        program.push_str(&format!("wait {part}\n"));
        left -= part;
    }
    program
}
