//! Randomness: which piece, which way round, which row, and where in its
//! interval the critter arrives.
//!
//! Nothing here is adversarial and none of it is cryptographic. A stream that
//! follows from a seed is the point: it lets a test assert a whole crossing,
//! cell for cell, and a thousand simulated hours of scheduling.

/// SplitMix64: its own generator as well as its own seeder, which is why it
/// is the whole of this module.
pub fn next_u64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// A number in `0..n`, uniform enough: the modulo's bias is one part in 2^64
/// over a bound never taken above a few hundred here.
pub fn below(state: &mut u64, n: u64) -> u64 {
    if n == 0 {
        return 0;
    }
    next_u64(state) % n
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

    #[test]
    fn below_stays_below() {
        let mut state = 3;
        assert!((0..500).all(|_| below(&mut state, 8) < 8));
        assert_eq!(below(&mut state, 1), 0);
        assert_eq!(below(&mut state, 0), 0);
    }
}
