use crate::decoder::{filter0, filter1, filter2, filter3, I15Sample};
use crate::{
    BlockNumber, BrrFilter, BrrSample, SampleNumber, BRR_HEADER_END_FLAG, BRR_HEADER_LOOP_FLAG,
    BYTES_PER_BRR_BLOCK, SAMPLES_PER_BLOCK,
};
const MAX_SAMPLES: usize = u16::MAX as usize;
const MAX_SHIFT: u8 = 12;
const I4_MIN: i32 = -8;
const I4_MAX: i32 = 7;
#[derive(Debug, Clone)]
pub enum EncodeError {
    NoSamples,
    InvalidNumberOfSamples(usize),
    TooManySamples(usize),
    InvalidLoopPointSamples(SampleNumber),
    LoopPointTooLarge(SampleNumber, usize),
    DupeBlockHackNotAllowedWithLoopResetsFilter,
    DupeBlockHackTooLarge,
}
impl std::fmt::Display for EncodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EncodeError::NoSamples => write!(f, "no samples"),
            EncodeError::InvalidNumberOfSamples(s) => {
                write!(
                    f,
                    "number of samples ({s}) is not a multiple of {SAMPLES_PER_BLOCK}"
                )
            }
            EncodeError::TooManySamples(s) => {
                write!(f, "too many samples ({s} samples, max {MAX_SAMPLES}))")
            }
            EncodeError::InvalidLoopPointSamples(lp) => {
                write!(
                    f,
                    "loop_point ({}) is not a multiple of {SAMPLES_PER_BLOCK}",
                    lp.0
                )
            }
            EncodeError::LoopPointTooLarge(lp, s_len) => {
                write!(
                    f,
                    "loop_point is too large ({}, max {})",
                    lp.0,
                    s_len - SAMPLES_PER_BLOCK
                )
            }
            EncodeError::DupeBlockHackNotAllowedWithLoopResetsFilter => {
                write!(
                    f,
                    "dupe_block_hack does nothing when loop_resets_filter is set"
                )
            }
            EncodeError::DupeBlockHackTooLarge => {
                write!(f, "dupe_block_hack value is too large")
            }
        }
    }
}
trait Scorer {
    fn score_sample(source: I15Sample, decoded: I15Sample, prev: (I15Sample, I15Sample)) -> i32;
    fn score_block(
        samples: &[I15Sample; SAMPLES_PER_BLOCK],
        block: &BrrBlock,
        prev: (I15Sample, I15Sample),
    ) -> i64;
}
struct SquaredError {}
impl SquaredError {
    fn sample_squared_error(s1: I15Sample, s2: I15Sample) -> i32 {
        let delta = s1.value() - s2.value();
        delta * delta
    }
}
impl Scorer for SquaredError {
    fn score_sample(source: I15Sample, decoded: I15Sample, _prev: (I15Sample, I15Sample)) -> i32 {
        Self::sample_squared_error(source, decoded)
    }
    fn score_block(
        samples: &[I15Sample; SAMPLES_PER_BLOCK],
        block: &BrrBlock,
        _prev: (I15Sample, I15Sample),
    ) -> i64 {
        assert!(block.decoded_samples.len() == samples.len());
        let mut square_error = 0;
        for (b, s) in block.decoded_samples.iter().zip(samples) {
            square_error += i64::from(Self::sample_squared_error(*b, *s));
        }
        square_error
    }
}
struct SquaredErrorAvoidGaussianOverflow {}
impl Scorer for SquaredErrorAvoidGaussianOverflow {
    fn score_sample(source: I15Sample, decoded: I15Sample, prev: (I15Sample, I15Sample)) -> i32 {
        const MAX_NEGATIVE_PENALTY_1: i32 = 100_000_000;
        const MAX_NEGATIVE_PENALTY_2: i32 = 1_000_000_000;
        const _MAX_DELTA: i32 = (i16::MAX as i32 >> 1) - (i16::MIN as i32 >> 1);
        const _: () = assert!(MAX_NEGATIVE_PENALTY_1 + (_MAX_DELTA * _MAX_DELTA) < i32::MAX);
        const _: () = assert!(MAX_NEGATIVE_PENALTY_2 + (_MAX_DELTA * _MAX_DELTA) < i32::MAX);
        let penalty = match (decoded, prev.0, prev.1) {
            (I15Sample::MIN, I15Sample::MIN, I15Sample::MIN) => return i32::MAX,
            (I15Sample::MIN, I15Sample::MIN, _) => MAX_NEGATIVE_PENALTY_2,
            (I15Sample::MIN, _, _) => MAX_NEGATIVE_PENALTY_1,
            (_, _, _) => 0,
        };
        let delta = source.value() - decoded.value();
        delta * delta + penalty
    }
    fn score_block(
        samples: &[I15Sample; SAMPLES_PER_BLOCK],
        block: &BrrBlock,
        prev: (I15Sample, I15Sample),
    ) -> i64 {
        assert!(block.decoded_samples.len() == samples.len());
        let mut error = 0;
        let mut prev = prev;
        for (s, b) in samples.iter().zip(block.decoded_samples) {
            error += i64::from(Self::score_sample(*s, b, prev));
            prev.1 = prev.0;
            prev.0 = b;
        }
        error
    }
}
struct BrrBlock {
    filter: BrrFilter,
    shift: u8,
    nibbles: [i8; 16],
    decoded_samples: [I15Sample; SAMPLES_PER_BLOCK],
}
fn build_block<S: Scorer>(
    samples: &[I15Sample; SAMPLES_PER_BLOCK],
    shift: u8,
    filter: BrrFilter,
    filter_fn: fn(I15Sample, I15Sample) -> i32,
    prev1: I15Sample,
    prev2: I15Sample,
) -> BrrBlock {
    assert!(shift <= MAX_SHIFT);
    let mut nibbles = [0; SAMPLES_PER_BLOCK];
    let mut decoded_samples: [I15Sample; SAMPLES_PER_BLOCK] = Default::default();
    let div: i32 = 1 << shift;
    let mut prev1 = prev1;
    let mut prev2 = prev2;
    for (i, s) in samples.iter().enumerate() {
        let offset = filter_fn(prev1, prev2);
        let n = (((s.value() - offset) << 1) / div).clamp(I4_MIN, I4_MAX);
        let (n, d) = (n - 1..=n + 1)
            .filter(|n| (I4_MIN..=I4_MAX).contains(n))
            .map(|n| {
                let d = I15Sample::clamp_and_clip(((n << shift) >> 1) + offset);
                (n, d)
            })
            .min_by_key(|(_, d)| S::score_sample(*s, *d, (prev1, prev2)))
            .unwrap();
        prev2 = prev1;
        prev1 = d;
        nibbles[i] = n.try_into().unwrap();
        decoded_samples[i] = d;
    }
    BrrBlock {
        filter,
        shift,
        nibbles,
        decoded_samples,
    }
}
fn find_best_block<S: Scorer>(
    samples: &[I15Sample; SAMPLES_PER_BLOCK],
    prev1: I15Sample,
    prev2: I15Sample,
) -> BrrBlock {
    let mut best_block = None;
    let mut best_block_score = i64::MAX;
    let mut test_filter = |filter, filter_fn| {
        for shift in 0..=MAX_SHIFT {
            let block = build_block::<S>(samples, shift, filter, filter_fn, prev1, prev2);
            let score = S::score_block(samples, &block, (prev1, prev2));
            if score < best_block_score {
                best_block = Some(block);
                best_block_score = score;
            }
        }
    };
    test_filter(BrrFilter::Filter0, filter0);
    test_filter(BrrFilter::Filter1, filter1);
    test_filter(BrrFilter::Filter2, filter2);
    test_filter(BrrFilter::Filter3, filter3);
    best_block.unwrap()
}
fn find_best_block_filter<S: Scorer>(
    samples: &[I15Sample; SAMPLES_PER_BLOCK],
    filter: BrrFilter,
    prev1: I15Sample,
    prev2: I15Sample,
) -> BrrBlock {
    let test_filter = |filter, filter_fn| {
        (0..=MAX_SHIFT)
            .map(|shift| build_block::<S>(samples, shift, filter, filter_fn, prev1, prev2))
            .min_by_key(|block| S::score_block(samples, block, (prev1, prev2)))
            .unwrap()
    };
    match filter {
        BrrFilter::Filter0 => test_filter(BrrFilter::Filter0, filter0),
        BrrFilter::Filter1 => test_filter(BrrFilter::Filter1, filter1),
        BrrFilter::Filter2 => test_filter(BrrFilter::Filter2, filter2),
        BrrFilter::Filter3 => test_filter(BrrFilter::Filter3, filter3),
    }
}
fn encode_block(block: BrrBlock, end_flag: bool, loop_flag: bool) -> [u8; BYTES_PER_BRR_BLOCK] {
    assert!(block.shift <= MAX_SHIFT);
    let mut out = [0; BYTES_PER_BRR_BLOCK];
    let mut header = ((block.shift & 0xf) << 4) | ((block.filter.as_u8()) << 2);
    if end_flag {
        header |= BRR_HEADER_END_FLAG;
        if loop_flag {
            header |= BRR_HEADER_LOOP_FLAG;
        }
    }
    out[0] = header;
    for (i, o) in out.iter_mut().skip(1).enumerate() {
        let nibble0 = block.nibbles[i * 2].to_le_bytes()[0];
        let nibble1 = block.nibbles[i * 2 + 1].to_le_bytes()[0];
        *o = ((nibble0 & 0xf) << 4) | (nibble1 & 0xf);
    }
    out
}
fn loop_point_to_block_number(
    lp: SampleNumber,
    samples: &[i16],
) -> Result<BlockNumber, EncodeError> {
    if lp.0 % SAMPLES_PER_BLOCK != 0 {
        return Err(EncodeError::InvalidLoopPointSamples(lp));
    }
    if lp.0 >= samples.len() {
        return Err(EncodeError::LoopPointTooLarge(lp, samples.len()));
    }
    Ok(BlockNumber(lp.0 / SAMPLES_PER_BLOCK))
}
fn encode_brr_with_scorer<S: Scorer>(
    samples: &[i16],
    loop_offset: Option<SampleNumber>,
    dupe_block_hack: Option<BlockNumber>,
    loop_filter: Option<BrrFilter>,
) -> Result<BrrSample, EncodeError> {
    if samples.is_empty() {
        return Err(EncodeError::NoSamples);
    }
    if samples.len() % SAMPLES_PER_BLOCK != 0 {
        return Err(EncodeError::InvalidNumberOfSamples(samples.len()));
    }
    if samples.len() > MAX_SAMPLES {
        return Err(EncodeError::TooManySamples(samples.len()));
    }
    let (loop_block, dbh_samples, loop_offset) = match (loop_offset, dupe_block_hack) {
        (None, None) => (BlockNumber(usize::MAX), [].as_slice(), None),
        (Some(lp), None) => {
            let loop_block = loop_point_to_block_number(lp, samples)?;
            let loop_offset = u16::try_from(loop_block.0 * BYTES_PER_BRR_BLOCK).unwrap();
            (loop_block, [].as_slice(), Some(loop_offset))
        }
        (lp, Some(dbh)) => {
            let lp_block = match lp {
                Some(lp) => loop_point_to_block_number(lp, samples)?,
                None => BlockNumber(0),
            };
            let loop_block = BlockNumber(lp_block.0 + dbh.0);
            if dbh.0 > 64 {
                return Err(EncodeError::DupeBlockHackTooLarge);
            }
            if loop_filter == Some(BrrFilter::Filter0) {
                return Err(EncodeError::DupeBlockHackNotAllowedWithLoopResetsFilter);
            }
            let loop_offset = match u16::try_from(loop_block.0 * BYTES_PER_BRR_BLOCK) {
                Ok(lo) => lo,
                Err(_) => return Err(EncodeError::DupeBlockHackTooLarge),
            };
            (
                loop_block,
                &samples[lp_block.0 * SAMPLES_PER_BLOCK..],
                Some(loop_offset),
            )
        }
    };
    let n_blocks = samples.len() / SAMPLES_PER_BLOCK + dupe_block_hack.unwrap_or(BlockNumber(0)).0;
    let last_block = BlockNumber(n_blocks - 1);
    let mut brr_data = Vec::with_capacity(n_blocks * BYTES_PER_BRR_BLOCK);
    let mut prev1 = I15Sample::default();
    let mut prev2 = I15Sample::default();
    for (bn, samples) in samples
        .chunks_exact(SAMPLES_PER_BLOCK)
        .chain(dbh_samples.chunks_exact(SAMPLES_PER_BLOCK).cycle())
        .take(n_blocks)
        .enumerate()
        .map(|(i, s)| (BlockNumber(i), s))
    {
        let samples: [i16; SAMPLES_PER_BLOCK] = samples.try_into().unwrap();
        let samples = samples.map(I15Sample::from_sample);
        let block = if bn == BlockNumber(0) {
            find_best_block_filter::<S>(&samples, BrrFilter::Filter0, prev1, prev2)
        } else if bn == loop_block {
            match loop_filter {
                None => find_best_block::<S>(&samples, prev1, prev2),
                Some(loop_filter) => {
                    find_best_block_filter::<S>(&samples, loop_filter, prev1, prev2)
                }
            }
        } else {
            find_best_block::<S>(&samples, prev1, prev2)
        };
        prev1 = block.decoded_samples[SAMPLES_PER_BLOCK - 1];
        prev2 = block.decoded_samples[SAMPLES_PER_BLOCK - 2];
        brr_data.extend(encode_block(block, bn == last_block, loop_offset.is_some()));
    }
    if let Some(lo) = loop_offset {
        assert!(usize::from(lo) < brr_data.len());
    }
    Ok(BrrSample {
        loop_offset,
        brr_data,
    })
}
pub enum Evaluator {
    SquaredError,
    SquaredErrorAvoidGaussianOverflow,
}
pub const DEFAULT_EVALUATOR: Evaluator = Evaluator::SquaredErrorAvoidGaussianOverflow;
pub fn encode_brr(
    samples: &[i16],
    evaluator: Evaluator,
    loop_offset: Option<SampleNumber>,
    dupe_block_hack: Option<BlockNumber>,
    loop_filter: Option<BrrFilter>,
) -> Result<BrrSample, EncodeError> {
    match evaluator {
        Evaluator::SquaredError => encode_brr_with_scorer::<SquaredError>(
            samples,
            loop_offset,
            dupe_block_hack,
            loop_filter,
        ),
        Evaluator::SquaredErrorAvoidGaussianOverflow => {
            encode_brr_with_scorer::<SquaredErrorAvoidGaussianOverflow>(
                samples,
                loop_offset,
                dupe_block_hack,
                loop_filter,
            )
        }
    }
}
