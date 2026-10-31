use crate::decoder::I15Sample;
use crate::SAMPLES_PER_BLOCK;
pub(crate) fn test_for_gaussian_overflow_glitch(
    prev1: i16,
    prev2: i16,
    decoded_samples: &[i16; SAMPLES_PER_BLOCK],
) -> bool {
    const MIN: i16 = I15Sample::MIN.to_sample();
    let mut c: u32 = match (prev2, prev1) {
        (MIN, MIN) => 2,
        (_, MIN) => 1,
        (_, _) => 0,
    };
    for s in decoded_samples {
        if *s == MIN {
            c += 1;
            if c == 3 {
                return true;
            }
        } else {
            c = 0;
        }
    }
    false
}
