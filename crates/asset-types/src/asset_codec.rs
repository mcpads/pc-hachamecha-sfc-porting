use serde::Serialize;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
pub const CANONICAL_CHARACTER_PIXELS: usize = 64;
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalAssetError {
    EmptyCharacterSet,
    CharacterPixelCount {
        character_index: usize,
        actual: usize,
    },
    CharacterPixelOutOfRange {
        character_index: usize,
        pixel_index: usize,
        value: u8,
    },
    TwoBitCharacterPixelOutOfRange {
        character_index: usize,
        pixel_index: usize,
        value: u8,
    },
    EmptyPalette,
    PaletteWordOutOfRange {
        word_index: usize,
        value: u16,
    },
}
impl Display for CanonicalAssetError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyCharacterSet => formatter.write_str("character set is empty"),
            Self::CharacterPixelCount {
                character_index,
                actual,
            } => {
                write!(
                    formatter,
                    "character {character_index} holds {actual} pixels, expected 64"
                )
            }
            Self::CharacterPixelOutOfRange {
                character_index,
                pixel_index,
                value,
            } => {
                write!(
                    formatter,
                    "character {character_index} pixel {pixel_index} has value {value}, expected 0..=15"
                )
            }
            Self::TwoBitCharacterPixelOutOfRange {
                character_index,
                pixel_index,
                value,
            } => {
                write!(
                    formatter,
                    "two-bit character {character_index} pixel {pixel_index} has value {value}, expected 0..=3"
                )
            }
            Self::EmptyPalette => formatter.write_str("palette word sequence is empty"),
            Self::PaletteWordOutOfRange { word_index, value } => {
                write!(
                    formatter,
                    "palette word {word_index} has value {value:#06x}, expected 0x0000..=0x7fff"
                )
            }
        }
    }
}
impl Error for CanonicalAssetError {}
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct CanonicalTwoBitCharacters(Vec<Vec<u8>>);
impl CanonicalTwoBitCharacters {
    pub fn new(characters: Vec<Vec<u8>>) -> Result<Self, CanonicalAssetError> {
        if characters.is_empty() {
            return Err(CanonicalAssetError::EmptyCharacterSet);
        }
        for (character_index, character) in characters.iter().enumerate() {
            if character.len() != CANONICAL_CHARACTER_PIXELS {
                return Err(CanonicalAssetError::CharacterPixelCount {
                    character_index,
                    actual: character.len(),
                });
            }
            if let Some((pixel_index, value)) = character
                .iter()
                .copied()
                .enumerate()
                .find(|(_, value)| *value > 0x03)
            {
                return Err(CanonicalAssetError::TwoBitCharacterPixelOutOfRange {
                    character_index,
                    pixel_index,
                    value,
                });
            }
        }
        Ok(Self(characters))
    }
    #[must_use]
    pub fn characters(&self) -> &[Vec<u8>] {
        &self.0
    }
    #[must_use]
    pub fn into_characters(self) -> Vec<Vec<u8>> {
        self.0
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct CanonicalFourBitCharacters(Vec<Vec<u8>>);
impl CanonicalFourBitCharacters {
    pub fn new(characters: Vec<Vec<u8>>) -> Result<Self, CanonicalAssetError> {
        if characters.is_empty() {
            return Err(CanonicalAssetError::EmptyCharacterSet);
        }
        for (character_index, character) in characters.iter().enumerate() {
            if character.len() != CANONICAL_CHARACTER_PIXELS {
                return Err(CanonicalAssetError::CharacterPixelCount {
                    character_index,
                    actual: character.len(),
                });
            }
            if let Some((pixel_index, value)) = character
                .iter()
                .copied()
                .enumerate()
                .find(|(_, value)| *value > 0x0f)
            {
                return Err(CanonicalAssetError::CharacterPixelOutOfRange {
                    character_index,
                    pixel_index,
                    value,
                });
            }
        }
        Ok(Self(characters))
    }
    #[must_use]
    pub fn characters(&self) -> &[Vec<u8>] {
        &self.0
    }
    #[must_use]
    pub fn into_characters(self) -> Vec<Vec<u8>> {
        self.0
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct CanonicalU15PaletteWords(Vec<u16>);
impl CanonicalU15PaletteWords {
    pub fn new(words: Vec<u16>) -> Result<Self, CanonicalAssetError> {
        if words.is_empty() {
            return Err(CanonicalAssetError::EmptyPalette);
        }
        if let Some((word_index, value)) = words
            .iter()
            .copied()
            .enumerate()
            .find(|(_, value)| *value > 0x7fff)
        {
            return Err(CanonicalAssetError::PaletteWordOutOfRange { word_index, value });
        }
        Ok(Self(words))
    }
    #[must_use]
    pub fn words(&self) -> &[u16] {
        &self.0
    }
    #[must_use]
    pub fn into_words(self) -> Vec<u16> {
        self.0
    }
}
