use asset_types::{
    CANONICAL_CHARACTER_PIXELS, CanonicalAssetError, CanonicalFourBitCharacters,
    CanonicalTwoBitCharacters, CanonicalU15PaletteWords,
};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
const CHARACTER_EDGE_PIXELS: usize = 8;
const PLANAR_2BPP_CHARACTER_BYTES: usize = 16;
const PLANAR_4BPP_CHARACTER_BYTES: usize = 32;
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SfcAssetCodecProfile {
    TwoBitPlanarCharacters,
    FourBitPlanarCharacters,
    CgramU15Words,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SfcAssetCodecError {
    UnsupportedPlanarDepth {
        bits_per_pixel: u8,
    },
    EmptyPlanarInput {
        bits_per_pixel: u8,
    },
    PlanarByteLength {
        bits_per_pixel: u8,
        actual: usize,
        character_bytes: usize,
    },
    EmptyCgramInput,
    CgramByteLength {
        actual: usize,
    },
    CgramTopBitSet {
        word_index: usize,
        value: u16,
    },
    Canonical(CanonicalAssetError),
}
impl Display for SfcAssetCodecError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPlanarDepth { bits_per_pixel } => {
                write!(
                    formatter,
                    "planar depth {bits_per_pixel} is not one of the checked two- or four-plane forms"
                )
            }
            Self::EmptyPlanarInput { bits_per_pixel } => {
                write!(formatter, "{bits_per_pixel}bpp planar input is empty")
            }
            Self::PlanarByteLength {
                bits_per_pixel,
                actual,
                character_bytes,
            } => {
                write!(
                    formatter,
                    "{bits_per_pixel}bpp planar input is {actual} bytes, expected whole {character_bytes}-byte characters"
                )
            }
            Self::EmptyCgramInput => formatter.write_str("CGRAM input is empty"),
            Self::CgramByteLength { actual } => {
                write!(
                    formatter,
                    "CGRAM input is {actual} bytes, expected whole two-byte words"
                )
            }
            Self::CgramTopBitSet { word_index, value } => {
                write!(
                    formatter,
                    "CGRAM word {word_index} is {value:#06x}, but bit 15 must be clear"
                )
            }
            Self::Canonical(error) => Display::fmt(error, formatter),
        }
    }
}
impl Error for SfcAssetCodecError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Canonical(error) => Some(error),
            _ => None,
        }
    }
}
impl From<CanonicalAssetError> for SfcAssetCodecError {
    fn from(error: CanonicalAssetError) -> Self {
        Self::Canonical(error)
    }
}
#[must_use]
pub fn supported_sfc_asset_codec_profiles() -> &'static [SfcAssetCodecProfile] {
    &[
        SfcAssetCodecProfile::TwoBitPlanarCharacters,
        SfcAssetCodecProfile::FourBitPlanarCharacters,
        SfcAssetCodecProfile::CgramU15Words,
    ]
}
pub fn decode_sfc_planar_2bpp(
    native: &[u8],
) -> Result<CanonicalTwoBitCharacters, SfcAssetCodecError> {
    CanonicalTwoBitCharacters::new(decode_planar_characters(native, 2)?).map_err(Into::into)
}
#[must_use]
pub fn encode_sfc_planar_2bpp(characters: &CanonicalTwoBitCharacters) -> Vec<u8> {
    let mut native =
        Vec::with_capacity(characters.characters().len() * PLANAR_2BPP_CHARACTER_BYTES);
    for character in characters.characters() {
        let start = native.len();
        native.resize(start + PLANAR_2BPP_CHARACTER_BYTES, 0);
        for row in 0..CHARACTER_EDGE_PIXELS {
            for column in 0..CHARACTER_EDGE_PIXELS {
                let pixel = character[row * CHARACTER_EDGE_PIXELS + column];
                let shift = 7 - column;
                for plane in 0..2 {
                    let byte_index = start + row * 2 + plane;
                    native[byte_index] |= ((pixel >> plane) & 1) << shift;
                }
            }
        }
    }
    native
}
pub fn decode_sfc_planar_4bpp(
    native: &[u8],
) -> Result<CanonicalFourBitCharacters, SfcAssetCodecError> {
    CanonicalFourBitCharacters::new(decode_planar_characters(native, 4)?).map_err(Into::into)
}
#[must_use]
pub fn encode_sfc_planar_4bpp(characters: &CanonicalFourBitCharacters) -> Vec<u8> {
    let mut native =
        Vec::with_capacity(characters.characters().len() * PLANAR_4BPP_CHARACTER_BYTES);
    for character in characters.characters() {
        let start = native.len();
        native.resize(start + PLANAR_4BPP_CHARACTER_BYTES, 0);
        for row in 0..CHARACTER_EDGE_PIXELS {
            for column in 0..CHARACTER_EDGE_PIXELS {
                let pixel = character[row * CHARACTER_EDGE_PIXELS + column];
                let shift = 7 - column;
                for plane in 0..4 {
                    let byte_index = start + row * 2 + plane % 2 + (plane / 2) * 16;
                    native[byte_index] |= ((pixel >> plane) & 1) << shift;
                }
            }
        }
    }
    native
}
pub fn decode_sfc_cgram_u15(native: &[u8]) -> Result<CanonicalU15PaletteWords, SfcAssetCodecError> {
    if native.is_empty() {
        return Err(SfcAssetCodecError::EmptyCgramInput);
    }
    if !native.len().is_multiple_of(2) {
        return Err(SfcAssetCodecError::CgramByteLength {
            actual: native.len(),
        });
    }
    let mut words = Vec::with_capacity(native.len() / 2);
    for (word_index, bytes) in native.as_chunks::<2>().0.iter().enumerate() {
        let value = u16::from_le_bytes([bytes[0], bytes[1]]);
        if value & 0x8000 != 0 {
            return Err(SfcAssetCodecError::CgramTopBitSet { word_index, value });
        }
        words.push(value);
    }
    CanonicalU15PaletteWords::new(words).map_err(Into::into)
}
#[must_use]
pub fn encode_sfc_cgram_u15(palette: &CanonicalU15PaletteWords) -> Vec<u8> {
    palette
        .words()
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect()
}
#[must_use]
pub fn compose_sfc_cgram_u15(low: u8, high: u8) -> u16 {
    u16::from(low) | (u16::from(high & 0x7f) << 8)
}
pub fn decode_planar_characters(
    native: &[u8],
    bits_per_pixel: u8,
) -> Result<Vec<Vec<u8>>, SfcAssetCodecError> {
    let bytes_per_character = match bits_per_pixel {
        2 => PLANAR_2BPP_CHARACTER_BYTES,
        4 => PLANAR_4BPP_CHARACTER_BYTES,
        _ => {
            return Err(SfcAssetCodecError::UnsupportedPlanarDepth { bits_per_pixel });
        }
    };
    if native.is_empty() {
        return Err(SfcAssetCodecError::EmptyPlanarInput { bits_per_pixel });
    }
    if !native.len().is_multiple_of(bytes_per_character) {
        return Err(SfcAssetCodecError::PlanarByteLength {
            bits_per_pixel,
            actual: native.len(),
            character_bytes: bytes_per_character,
        });
    }
    let planes = usize::from(bits_per_pixel);
    let mut characters = Vec::with_capacity(native.len() / bytes_per_character);
    for block in native.chunks_exact(bytes_per_character) {
        let mut pixels = Vec::with_capacity(CANONICAL_CHARACTER_PIXELS);
        for row in 0..CHARACTER_EDGE_PIXELS {
            for column in 0..CHARACTER_EDGE_PIXELS {
                let shift = 7 - column;
                let mut index = 0u8;
                for plane in 0..planes {
                    let byte = block[row * 2 + plane % 2 + (plane / 2) * 16];
                    index |= ((byte >> shift) & 1) << plane;
                }
                pixels.push(index);
            }
        }
        characters.push(pixels);
    }
    Ok(characters)
}
