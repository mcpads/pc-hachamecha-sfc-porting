use crate::Result;
use sfc_builder::cartridge::CartridgeLayout;
pub(crate) const SONGS: [(&str, &str, u8); 28] = [
    ("title", "01-title", 1),
    ("prologue_arle", "02-prologue-arle", 2),
    ("opening", "00-opening-2", 0),
    ("b1", "04-labyrinth-intro", 4),
    ("rulue", "16-rulue", 16),
    ("item", "20-item", 20),
    ("event_item", "19-event-item", 19),
    ("level_up", "22-level-up", 22),
    ("b2", "05-try-out", 5),
    ("ending", "03-ending", 3),
    ("b5_b7", "06-b5-b7", 6),
    ("phantom_zone", "07-phantom-zone", 7),
    ("song_8", "08-unused-8", 8),
    ("shop", "09-shop", 9),
    ("event", "10-event", 10),
    ("ending_2", "11-ending-2", 11),
    ("battle_12", "12-battle", 12),
    ("battle_13", "13-battle", 13),
    ("ancient", "14-ancient", 14),
    ("guardian", "15-guardian", 15),
    ("battle_17", "17-battle", 17),
    ("final_boss", "18-final-boss", 18),
    ("monster", "21-monster", 21),
    ("victory", "23-victory", 23),
    ("defeat", "24-defeat", 24),
    ("crystal", "25-crystal", 25),
    ("panotti", "26-panotti", 26),
    ("song_27", "27-unknown-27", 27),
];
pub(crate) fn effect(layout: &CartridgeLayout, source: usize) -> Result<Option<u8>> {
    let map = &layout.section("sound_effect_map")?.bytes;
    Ok(map.get(source).copied().filter(|&c| c != 0))
}
pub(crate) fn voice(layout: &CartridgeLayout, source: usize) -> Result<Option<u8>> {
    let map = &layout.section("sound_voice_map")?.bytes;
    Ok(map.get(source).copied().filter(|&c| c != 0))
}
pub(crate) fn song(name: &str) -> u8 {
    SONGS.iter().position(|(n, ..)| *n == name).expect("song") as u8
}
pub(crate) fn source_song(index: u64) -> Option<u8> {
    SONGS
        .iter()
        .position(|(.., s)| u64::from(*s) == index)
        .map(|i| i as u8)
}
pub(crate) const STREAMED: u8 = 0x10;
pub(crate) const MUSIC: u16 = 0x1714;
pub(crate) const PENDING: u16 = 0x1718;
pub(crate) const RETURN: u16 = 0x1719;
pub(crate) const TIMER: u16 = 0x171a;
pub(crate) const STREAM_PENDING: u16 = 0x171e;
pub(crate) const STREAM_TIMER: u16 = 0x1720;
pub(crate) const JOB: u16 = 0x1722;
pub(crate) const EXCHANGES: u16 = 0x172c;
pub(crate) const WANTED: u16 = 0x1733;
pub(crate) const JOB_SONG: u8 = 1;
pub(crate) const JOB_STREAM: u8 = 2;
