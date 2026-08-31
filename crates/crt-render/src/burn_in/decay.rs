//! The wall-clock half of burn-in.
//!
//! The decay is computed on the CPU and handed to the shader as a single
//! number. Two timestamps (`last_update`, `prev_last_update`) turn into
//!
//! ```text
//! decay = clamp((last_update - prev_last_update) * burn_in_time, 0, 1)
//! ```
//!
//! where `burn_in_time = 1 / lint(MIN_FADE_TIME, MAX_FADE_TIME, burn_in)`,
//! with the two constants below (0.16 s and 1.6 s). So `burn_in_time` is a
//! rate, and its reciprocal is the fade time: the seconds a full-brightness
//! pixel takes to reach black with nothing relighting it. Larger `burn_in`
//! means a longer fade, not a faster one.
//!
//! This module is the same arithmetic against a real clock. It holds no GPU
//! state, so it is unit-testable without a device, and the value it produces is
//! pushed as the `BURNIN_DECAY_STEP` parameter every frame.

/// The fade time at `burn_in = 0`.
pub const MIN_FADE_TIME: f64 = 0.16;
/// The fade time reached at `burn_in = 1`.
pub const MAX_FADE_TIME: f64 = 1.6;

/// The parameter name the accumulator pass reads its decay from.
pub const DECAY_PARAM: &str = "BURNIN_DECAY_STEP";
/// The parameter name that switches the freshness mask. Tests move it; the
/// application leaves it at 1.
pub const MASK_PARAM: &str = "BURNIN_MASK";

/// Linear interpolation.
fn lint(a: f64, b: f64, t: f64) -> f64 {
    (1.0 - t) * a + t * b
}

/// Seconds a full-brightness pixel takes to fade to black at this `burn_in`.
///
/// `burn_in` is the settings key, in 0..=1; out-of-range values are clamped
/// rather than extrapolated, because a config file can supply one even
/// though the slider cannot, and a negative fade time would invert the ramp.
pub fn fade_time_seconds(burn_in: f64) -> f64 {
    lint(MIN_FADE_TIME, MAX_FADE_TIME, burn_in.clamp(0.0, 1.0))
}

/// The decay to subtract from the accumulator for a frame `dt` seconds long.
///
/// `burn_in = 0` returns 1.0 whatever the delta: a full decay every frame,
/// which leaves the accumulator equal to the live source and switches burn-in
/// off without touching the chain's structure. That is deliberate: spending
/// a uniform instead of a chain rebuild avoids resetting the ghost of every
/// *other* pass along with it.
pub fn decay_step(burn_in: f64, dt_seconds: f64) -> f32 {
    if burn_in <= 0.0 {
        return 1.0;
    }
    let step = dt_seconds.max(0.0) / fade_time_seconds(burn_in);
    step.clamp(0.0, 1.0) as f32
}

/// Tracks the real frame delta and turns it into the per-frame decay.
///
/// One instance per mounted accumulator. The caller ticks it once per rendered
/// frame with the frame's timestamp and pushes the result; the write itself
/// costs ~83 ns, so there is no case for skipping it on frames where the
/// value happens to be unchanged.
#[derive(Debug, Clone)]
pub struct DecayClock {
    burn_in: f64,
    /// Timestamp of the previous tick, in seconds on any monotonic scale the
    /// caller likes, as long as it is the same scale every time. `None` before
    /// the first tick and after a restart.
    last: Option<f64>,
    /// The longest frame delta that is treated as real. A frame delta larger
    /// than this comes from the process being stopped, the window being
    /// occluded, or a laptop lid, and none of those are a CRT sitting there
    /// fading. The host holding its picture while nobody is at the glass is
    /// the same case, and arrives here as one delta of however long that
    /// lasted. The clamp keeps any of them from erasing the ghost in one
    /// step: what comes back fades over the frames after it, as a tube
    /// does, rather than being gone by the first.
    max_dt: f64,
}

/// Frame deltas above this are clamped, see [`DecayClock::max_dt`].
pub const MAX_FRAME_DELTA: f64 = 0.25;

impl DecayClock {
    /// A clock for the given `burn_in` setting (the `screen.burn_in` key).
    pub fn new(burn_in: f64) -> Self {
        Self {
            burn_in,
            last: None,
            max_dt: MAX_FRAME_DELTA,
        }
    }

    pub fn burn_in(&self) -> f64 {
        self.burn_in
    }

    /// Live settings change. Does not restart the clock: the ghost currently on
    /// screen keeps fading, at the new rate, from where it is.
    pub fn set_burn_in(&mut self, burn_in: f64) {
        self.burn_in = burn_in;
    }

    /// Forget the previous timestamp, so the next tick decays by nothing.
    ///
    /// Call it after anything that discontinues the accumulator: a chain
    /// rebuild on a structural key, a font change, a resize.
    pub fn restart(&mut self) {
        self.last = None;
    }

    /// Seconds a full-brightness pixel takes to fade out at the current setting.
    pub fn fade_time_seconds(&self) -> f64 {
        fade_time_seconds(self.burn_in)
    }

    /// Advance to `now` (seconds, monotonic) and return the decay to push.
    ///
    /// The first tick after construction or a restart has no previous frame to
    /// measure against and decays by nothing, which is what keeps a resumed
    /// chain from jumping.
    pub fn tick(&mut self, now: f64) -> f32 {
        let dt = match self.last {
            Some(prev) => (now - prev).clamp(0.0, self.max_dt),
            None => 0.0,
        };
        self.last = Some(now);
        if self.burn_in <= 0.0 {
            // Off means off, including on the first frame: the accumulator has
            // to be flushed to the live image rather than held.
            return 1.0;
        }
        decay_step(self.burn_in, dt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fade_time_matches_the_recorded_endpoints() {
        assert!((fade_time_seconds(0.0) - 0.16).abs() < 1e-12);
        assert!((fade_time_seconds(1.0) - 1.6).abs() < 1e-12);
        // The default burnIn is 0.25.
        assert!((fade_time_seconds(0.25) - 0.52).abs() < 1e-12);
        // The screen presets sit at 0.3.
        assert!((fade_time_seconds(0.3) - 0.592).abs() < 1e-12);
    }

    #[test]
    fn decay_step_is_dt_over_fade_time() {
        let s = decay_step(0.25, 1.0 / 60.0);
        assert!((s as f64 - (1.0 / 60.0) / 0.52).abs() < 1e-6, "{s}");
        // A delta longer than the whole fade cannot decay by more than all.
        assert_eq!(decay_step(0.25, 10.0), 1.0);
    }

    #[test]
    fn burn_in_zero_decays_fully() {
        assert_eq!(decay_step(0.0, 1.0 / 60.0), 1.0);
        let mut c = DecayClock::new(0.0);
        assert_eq!(c.tick(0.0), 1.0);
        assert_eq!(c.tick(1.0 / 60.0), 1.0);
    }

    #[test]
    fn first_tick_decays_by_nothing() {
        let mut c = DecayClock::new(0.5);
        assert_eq!(c.tick(100.0), 0.0);
        assert!(c.tick(100.0 + 1.0 / 60.0) > 0.0);
        c.restart();
        assert_eq!(c.tick(200.0), 0.0);
    }

    #[test]
    fn long_stall_is_clamped_to_one_frame_of_decay() {
        let mut c = DecayClock::new(1.0);
        c.tick(0.0);
        // Ten seconds of suspended process: decays as 0.25 s, not as 10 s.
        let s = c.tick(10.0);
        assert!((s as f64 - MAX_FRAME_DELTA / 1.6).abs() < 1e-6, "{s}");
    }

    #[test]
    fn a_backwards_clock_decays_by_nothing() {
        let mut c = DecayClock::new(0.5);
        c.tick(10.0);
        assert_eq!(c.tick(9.0), 0.0);
    }

    #[test]
    fn changing_burn_in_does_not_restart_the_ghost() {
        let mut c = DecayClock::new(1.0);
        c.tick(0.0);
        c.set_burn_in(0.25);
        // Still measuring against the previous tick, at the new rate.
        let s = c.tick(1.0 / 60.0);
        assert!((s as f64 - (1.0 / 60.0) / 0.52).abs() < 1e-6, "{s}");
    }
}
