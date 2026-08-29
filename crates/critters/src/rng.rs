//! Randomness, and the one distribution this crate draws from.
//!
//! Twenty lines rather than a dependency, for two reasons. The demand is
//! small -- a different piece, a different row, a different wait -- and none
//! of it is adversarial: nobody is attacking a duck, so nothing here wants a
//! cryptographic generator and none is used. The second reason is the tests:
//! a stream that follows from a seed is what lets a test assert a whole
//! crossing, cell for cell, and a thousand simulated hours of scheduling.

/// SplitMix64: Steele, Lea and Flood's mixer, the one a seeded `u64` is
/// usually smeared with before it is fit to be a stream. It is its own
/// generator as well as its own seeder, which is why it is the whole of this
/// module.
pub fn next_u64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// A number in `0..n`, uniform enough. The modulo's bias is one part in
/// 2^64 over a bound this crate never takes above a few hundred, which is
/// not a thing any eye watching a duck can measure.
pub fn below(state: &mut u64, n: u64) -> u64 {
    if n == 0 {
        return 0;
    }
    next_u64(state) % n
}

/// A wait drawn from the memoryless distribution with this mean, in seconds.
///
/// `-mean * ln(u)` for `u` uniform in `(0, 1]`. The point of drawing at all
/// is that a fixed timer is a metronome: a user who has seen two critters
/// knows when the third is due, and knowing is the end of the joke. Nothing
/// is clamped at the top, because a long gap is a legitimate draw and a
/// ceiling would put back the shape the distribution was chosen for not
/// having.
pub fn wait(state: &mut u64, mean: f64) -> f64 {
    // 53 bits, the mantissa's worth, shifted off the top where SplitMix64's
    // bits are best. The +1 keeps `u` off zero, whose logarithm is not a
    // duration.
    let u = ((next_u64(state) >> 11) + 1) as f64 / (1u64 << 53) as f64;
    mean * -u.ln()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stream_follows_from_the_seed() {
        let (mut a, mut b) = (7, 7);
        let run: Vec<u64> = (0..8).map(|_| next_u64(&mut a)).collect();
        let again: Vec<u64> = (0..8).map(|_| next_u64(&mut b)).collect();
        assert_eq!(run, again);
        assert!(run.windows(2).all(|w| w[0] != w[1]));
    }

    /// The two properties the schedule stands on: the mean is the mean, and
    /// the shape is the exponential's rather than a timer's. `1 - 1/e` of
    /// the draws fall below the mean, which a metronome would fail by
    /// putting all of them at it.
    #[test]
    fn the_wait_is_memoryless_with_the_mean_it_was_given() {
        let mut state = 1;
        let mean = 900.0;
        let draws: Vec<f64> = (0..10_000).map(|_| wait(&mut state, mean)).collect();
        let average = draws.iter().sum::<f64>() / draws.len() as f64;
        assert!(
            (average - mean).abs() < mean * 0.03,
            "average {average} is not the mean {mean}"
        );
        let below_mean = draws.iter().filter(|d| **d < mean).count() as f64 / draws.len() as f64;
        assert!(
            (below_mean - (1.0 - std::f64::consts::E.recip())).abs() < 0.02,
            "{below_mean} of the draws fell below the mean"
        );
        assert!(draws.iter().all(|d| *d > 0.0));
    }

    #[test]
    fn below_stays_below() {
        let mut state = 3;
        assert!((0..500).all(|_| below(&mut state, 8) < 8));
        assert_eq!(below(&mut state, 1), 0);
        assert_eq!(below(&mut state, 0), 0);
    }
}
