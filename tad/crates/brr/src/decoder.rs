use crate::{BRR_HEADER_END_FLAG, BYTES_PER_BRR_BLOCK, SAMPLES_PER_BLOCK};
#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub struct I15Sample {
    value: i16,
}
impl I15Sample {
    pub const MIN: I15Sample = I15Sample {
        value: i16::MIN >> 1,
    };
    pub fn value(self) -> i32 {
        self.value.into()
    }
    pub fn clamp_and_clip(value: i32) -> Self {
        let value = value.clamp(i16::MIN.into(), i16::MAX.into());
        let value = i16::try_from(value).unwrap();
        let value = value << 1;
        let value = value >> 1;
        Self { value }
    }
    pub fn from_sample(s: i16) -> Self {
        Self { value: s >> 1 }
    }
    pub const fn to_sample(self) -> i16 {
        self.value << 1
    }
}
pub fn filter0(_p1: I15Sample, _p2: I15Sample) -> i32 {
    0
}
pub fn filter1(p1: I15Sample, _p2: I15Sample) -> i32 {
    let p1: i32 = p1.value.into();
    p1 + ((-p1) >> 4)
}
pub fn filter2(p1: I15Sample, p2: I15Sample) -> i32 {
    let p1: i32 = p1.value.into();
    let p2: i32 = p2.value.into();
    (p1 << 1) + ((-((p1 << 1) + p1)) >> 5) - p2 + (p2 >> 4)
}
pub fn filter3(p1: I15Sample, p2: I15Sample) -> i32 {
    let p1: i32 = p1.value.into();
    let p2: i32 = p2.value.into();
    (p1 << 1) + ((-(p1 + (p1 << 2) + (p1 << 3))) >> 6) - p2 + (((p2 << 1) + p2) >> 4)
}
fn nibble_to_i32(n: u8) -> i32 {
    let n = n & 0xf;
    if n & 0b1000 == 0 {
        n.into()
    } else {
        let n = n | 0b11110000;
        i8::from_le_bytes([n]).into()
    }
}
fn decode_brr_block_data(
    data: &[u8; SAMPLES_PER_BLOCK / 2],
    p1: I15Sample,
    p2: I15Sample,
    shift: u8,
    filter: impl Fn(I15Sample, I15Sample) -> i32,
) -> [i16; SAMPLES_PER_BLOCK] {
    let mut p1 = p1;
    let mut p2 = p2;
    std::array::from_fn(|i| {
        let nibble = if (i & 1) == 0 {
            data[i / 2] >> 4
        } else {
            data[i / 2]
        };
        let s = if shift <= 12 {
            (nibble_to_i32(nibble) << shift) >> 1
        } else {
            if nibble & 0b1000 == 0 {
                0
            } else {
                -2048
            }
        };
        let s = s + filter(p1, p2);
        p2 = p1;
        p1 = I15Sample::clamp_and_clip(s);
        p1.to_sample()
    })
}
pub fn decode_brr_block(
    brr_block: &[u8; BYTES_PER_BRR_BLOCK],
    previous_sample_1: i16,
    previous_sample_2: i16,
) -> [i16; SAMPLES_PER_BLOCK] {
    let p1 = I15Sample::from_sample(previous_sample_1);
    let p2 = I15Sample::from_sample(previous_sample_2);
    let header = brr_block[0];
    let data = brr_block[1..].try_into().unwrap();
    let shift = header >> 4;
    let filter = (header >> 2) & 0b11;
    match filter {
        0 => decode_brr_block_data(data, p1, p2, shift, filter0),
        1 => decode_brr_block_data(data, p1, p2, shift, filter1),
        2 => decode_brr_block_data(data, p1, p2, shift, filter2),
        3 => decode_brr_block_data(data, p1, p2, shift, filter3),
        _ => panic!(),
    }
}
pub fn decode_brr_data(brr_data: &[u8]) -> Vec<i16> {
    let n_blocks = brr_data.len() / BYTES_PER_BRR_BLOCK;
    let mut out = Vec::with_capacity(n_blocks * SAMPLES_PER_BLOCK);
    let mut p1 = 0;
    let mut p2 = 0;
    for block in brr_data.chunks_exact(BYTES_PER_BRR_BLOCK) {
        let samples = decode_brr_block(block.try_into().unwrap(), p1, p2);
        p1 = samples[SAMPLES_PER_BLOCK - 1];
        p2 = samples[SAMPLES_PER_BLOCK - 2];
        out.extend(samples);
        if block[0] & BRR_HEADER_END_FLAG != 0 {
            break;
        }
    }
    out
}
