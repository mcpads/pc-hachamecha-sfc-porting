mod decoder;
mod encoder;
mod gaussian_overflow_test;
mod mono_pcm_wav;
mod parse_brr_file;
pub use decoder::decode_brr_data;
pub use encoder::{encode_brr, EncodeError, Evaluator, DEFAULT_EVALUATOR};
use gaussian_overflow_test::test_for_gaussian_overflow_glitch;
pub use mono_pcm_wav::{read_mono_pcm_wave_file, MonoPcm16WaveFile, WavError};
pub use parse_brr_file::{parse_brr_file, ParseError, ValidBrrFile};
use std::str::FromStr;
pub const SAMPLES_PER_BLOCK: usize = 16;
pub const BYTES_PER_BRR_BLOCK: usize = 9;
pub const BRR_HEADER_END_FLAG: u8 = 0x01;
pub const BRR_HEADER_LOOP_FLAG: u8 = 0x02;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SampleNumber(pub usize);
impl From<usize> for SampleNumber {
    fn from(value: usize) -> Self {
        Self(value)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockNumber(pub usize);
impl From<usize> for BlockNumber {
    fn from(value: usize) -> Self {
        Self(value)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrrFilter {
    Filter0 = 0,
    Filter1 = 1,
    Filter2 = 2,
    Filter3 = 3,
}
impl FromStr for BrrFilter {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "0" => Ok(Self::Filter0),
            "1" => Ok(Self::Filter1),
            "2" => Ok(Self::Filter2),
            "3" => Ok(Self::Filter3),
            _ => Err("Invalid BRR filter (expected 0-3)"),
        }
    }
}
impl BrrFilter {
    fn as_u8(&self) -> u8 {
        *self as u8
    }
}
#[derive(Clone, Hash, Eq, PartialEq)]
pub struct BrrSample {
    loop_offset: Option<u16>,
    brr_data: Vec<u8>,
}
impl BrrSample {
    pub fn loop_offset(&self) -> Option<u16> {
        self.loop_offset
    }
    pub fn brr_data(&self) -> &[u8] {
        &self.brr_data
    }
    pub fn n_brr_blocks(&self) -> usize {
        self.brr_data.len() / BYTES_PER_BRR_BLOCK
    }
    pub fn n_samples(&self) -> usize {
        self.brr_data.len() / BYTES_PER_BRR_BLOCK * SAMPLES_PER_BLOCK
    }
    pub fn loop_point_samples(&self) -> Option<usize> {
        self.loop_offset
            .map(|lo| usize::from(lo) / BYTES_PER_BRR_BLOCK * SAMPLES_PER_BLOCK)
    }
    pub fn is_looping(&self) -> bool {
        self.loop_offset.is_some()
    }
    pub fn brr_with_loop_header(&self) -> Vec<u8> {
        let lo = self.loop_offset().unwrap_or(0);
        [&lo.to_le_bytes(), self.brr_data.as_slice()].concat()
    }
    pub fn decode_into_buffer(
        &self,
        buf: &mut [i16],
        index: usize,
        prev1: i16,
        prev2: i16,
    ) -> usize {
        assert!(buf.len() % SAMPLES_PER_BLOCK == 0);
        assert!(index % BYTES_PER_BRR_BLOCK == 0);
        let mut index = index;
        let mut prev1 = prev1;
        let mut prev2 = prev2;
        for out in buf.chunks_exact_mut(SAMPLES_PER_BLOCK) {
            if index < self.brr_data.len() {
                let block = self.brr_data[index..index + BYTES_PER_BRR_BLOCK]
                    .try_into()
                    .unwrap();
                let decoded = decoder::decode_brr_block(&block, prev1, prev2);
                out.copy_from_slice(&decoded);
                prev1 = decoded[SAMPLES_PER_BLOCK - 1];
                prev2 = decoded[SAMPLES_PER_BLOCK - 2];
                index += BYTES_PER_BRR_BLOCK;
                if index >= self.brr_data.len() {
                    if let Some(lo) = self.loop_offset {
                        index = lo.into();
                    }
                }
            } else {
                out.fill(0)
            }
        }
        index
    }
    pub fn test_for_gaussian_overflow_glitch_n_loops(&self, n_loops: u32) -> bool {
        let n_loops = match self.is_looping() {
            true => n_loops + 1,
            false => 1,
        };
        let mut loop_prev1 = 0;
        let mut loop_prev2 = 0;
        for i in 0..n_loops {
            let mut prev1 = loop_prev1;
            let mut prev2 = loop_prev2;
            let i: usize = match i {
                0 => 0,
                _ => self.loop_offset.unwrap().into(),
            };
            for block in self.brr_data[i..].chunks_exact(BYTES_PER_BRR_BLOCK) {
                let decoded_samples =
                    decoder::decode_brr_block(&block.try_into().unwrap(), prev1, prev2);
                if test_for_gaussian_overflow_glitch(prev1, prev2, &decoded_samples) {
                    return true;
                }
                prev1 = decoded_samples[SAMPLES_PER_BLOCK - 1];
                prev2 = decoded_samples[SAMPLES_PER_BLOCK - 2];
            }
            if (prev1, prev2) == (loop_prev1, loop_prev2) {
                break;
            }
            loop_prev1 = prev1;
            loop_prev2 = prev2;
        }
        false
    }
    pub fn test_for_gaussian_overflow_glitch_autoloop(&self) -> bool {
        const ONE_SECOND: u32 = 32000 / SAMPLES_PER_BLOCK as u32 * BYTES_PER_BRR_BLOCK as u32;
        let bytes = u32::try_from(self.n_brr_blocks()).unwrap_or(ONE_SECOND);
        let n_loops = (ONE_SECOND / bytes).clamp(2, 20);
        self.test_for_gaussian_overflow_glitch_n_loops(n_loops)
    }
}
