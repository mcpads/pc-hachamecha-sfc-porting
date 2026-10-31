pub mod battery;
pub mod cartridge;
pub mod emit;
pub mod memory_map;
pub mod runtime;
pub mod sprite;
pub mod sprite_frame;
pub mod tiles;
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
pub fn sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub mod background;
pub mod glyph;
pub mod publication;
pub mod text;
pub mod transfer;
