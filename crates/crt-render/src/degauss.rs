//! The degauss transient, as a render hook.
//!
//! On a channel change the picture is squeezed to 97% of its height and a
//! white flood is laid over the phosphor, both easing out over 200 ms. The
//! mockup's keyframe, and the equivalence the flood stands in for:
//! "brightness 2.6, scaleY 0.97, 200 ms ease-out; the 0.25 flood over the
//! phosphor lands the same peak".
//!
//! What is here is the hook and nothing else. Deciding *when* to degauss is the
//! channel-bank state machine's job; this crate only knows how to run one
//! when told, and how to report what the passes should be given while it
//! runs. The channel-bank state machine calls [`Degauss::trigger`] and the
//! render loop calls [`Degauss::sample`] every frame.

use std::time::{Duration, Instant};

/// The mockup's keyframe, fixed rather than configurable.
pub const DURATION: Duration = Duration::from_millis(200);
pub const PEAK_BRIGHTNESS: f32 = 2.6;
pub const PEAK_SCALE_Y: f32 = 0.97;

/// What the transient is doing this frame. Both fields are pass uniforms.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DegaussState {
    /// Multiplier on the finished picture. 1.0 when nothing is running.
    pub brightness: f32,
    /// Vertical squeeze of the picture about its centre. 1.0 when nothing is
    /// running.
    pub scale_y: f32,
}

impl DegaussState {
    pub const IDLE: Self = Self {
        brightness: 1.0,
        scale_y: 1.0,
    };

    /// Whether the transient is doing anything the eye could see.
    pub fn is_active(&self) -> bool {
        *self != Self::IDLE
    }
}

impl Default for DegaussState {
    fn default() -> Self {
        Self::IDLE
    }
}

/// The hook. Holds at most one transient; triggering during one restarts it
/// rather than queuing or ignoring the new trigger.
#[derive(Debug, Default, Clone, Copy)]
pub struct Degauss {
    started: Option<Instant>,
}

impl Degauss {
    pub fn new() -> Self {
        Self::default()
    }

    /// Start a transient at `now`, or restart one already running.
    pub fn trigger(&mut self, now: Instant) {
        self.started = Some(now);
    }

    /// Cut a running transient short.
    pub fn cancel(&mut self) {
        self.started = None;
    }

    pub fn is_running(&self, now: Instant) -> bool {
        self.progress(now).is_some()
    }

    /// The uniforms for this frame, and the end of the transient once it is
    /// past. `sample` is what the render loop calls; it takes `&mut self` so a
    /// finished transient releases its start instant rather than being
    /// recomputed for the life of the process.
    pub fn sample(&mut self, now: Instant) -> DegaussState {
        match self.progress(now) {
            None => {
                self.started = None;
                DegaussState::IDLE
            }
            Some(t) => {
                // The `OutQuad` ease-out curve: 1 - (1-t)^2, decelerating into
                // the resting value.
                let eased = 1.0 - (1.0 - t) * (1.0 - t);
                DegaussState {
                    brightness: PEAK_BRIGHTNESS + (1.0 - PEAK_BRIGHTNESS) * eased,
                    scale_y: PEAK_SCALE_Y + (1.0 - PEAK_SCALE_Y) * eased,
                }
            }
        }
    }

    /// Normalised position through the transient, or `None` if none is running.
    fn progress(&self, now: Instant) -> Option<f32> {
        let started = self.started?;
        let elapsed = now.saturating_duration_since(started);
        if elapsed >= DURATION {
            return None;
        }
        Some(elapsed.as_secs_f32() / DURATION.as_secs_f32())
    }
}
