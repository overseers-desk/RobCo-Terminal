//! The two arithmetic primitives every layout formula in this crate runs
//! through.
//!
//! JavaScript's `Math.round` is not Rust's `f64::round`. JavaScript rounds a
//! half toward positive infinity; Rust rounds it away from zero. They differ
//! on every negative half: `Math.round(-0.5)` is `-0`, while
//! `(-0.5f64).round()` is `-1.0`.
//!
//! That difference is load-bearing here rather than academic. The seam drag
//! rounds a *signed* character delta, so a hand sitting exactly half a
//! character left of the bank's edge is the ordinary case, not a corner one,
//! and `f64::round` would step the strip one character further than this
//! convention does.

/// JavaScript's `Math.round`: halves go toward positive infinity.
pub fn round(x: f64) -> f64 {
    (x + 0.5).floor()
}

/// JavaScript's `Math.round`, delivered as an `i32`.
pub fn round_i32(x: f64) -> i32 {
    round(x) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn halves_go_up_not_away_from_zero() {
        // The four cases that separate Math.round from f64::round.
        assert_eq!(round(-0.5), 0.0);
        assert_eq!(round(-1.5), -1.0);
        assert_eq!(round(0.5), 1.0);
        assert_eq!(round(1.5), 2.0);

        // ...and the witness that Rust's own rounding would have differed.
        assert_eq!((-0.5f64).round(), -1.0);
        assert_eq!((-1.5f64).round(), -2.0);
    }

    #[test]
    fn ordinary_values_agree_with_rust() {
        for x in [-3.7, -2.2, -0.1, 0.0, 0.4, 7.6, 41.49, 41.51] {
            assert_eq!(round(x), x.round(), "disagreement at {x}");
        }
    }
}
