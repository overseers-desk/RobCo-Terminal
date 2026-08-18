//! The time contract for every pass in the chain.
//!
//! One clock, wall-clock, owned here, and it has to be wall-clock rather
//! than a frame counter: burn-in decay is computed from the real elapsed
//! time between accumulator updates, so a chain paced by frames would fade at a
//! rate that changed with the frame rate. Every shader in the chain reads the
//! same single source, seconds since start, as `time`.
//!
//! [`Pacing`] never calls `Instant::now()` itself. The caller passes the instant
//! it observed, which is what lets the render tests drive a hundred frames of
//! synthetic time and get the same pixels every run. The app passes
//! `Instant::now()` once per frame and nothing else in the crate reads a clock.

use std::time::{Duration, Instant};

/// One frame's worth of time, as the passes see it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameTime {
    /// Seconds since the chain's epoch. This is the `Time` uniform, the
    /// value every shader in the chain reads as `time`.
    pub elapsed: f32,
    /// The same instant at double precision, which is the clock the burn-in
    /// accumulator is ticked on.
    ///
    /// Two fields for one number because they answer to different things. The
    /// uniform is an `f32` because that is what the shader reads, and by the
    /// second day of an uptime an `f32` of seconds has lost the resolution to
    /// tell one 60 Hz frame from the next: at 86400 s its ULP is 7.8 ms, half a
    /// frame. The shader does not care, since every use of `time` there is
    /// modular. `DecayClock` does: it works in differences, and a frame delta
    /// rounded to half its size is a ghost fading at half the rate the settings
    /// asked for. Anything monotonic will do for it, as long as it is the same
    /// scale every frame, so it gets the `f64`.
    pub now: f64,
    /// Seconds since the previous frame. Zero on the first frame, since there
    /// is no previous one to measure against.
    pub delta: f32,
    /// Frames rendered since the epoch, starting at zero. Passed to librashader
    /// as its frame index, which is what drives `FrameCount` in every shader.
    pub index: usize,
}

/// The clock the chain is paced by.
pub struct Pacing {
    epoch: Instant,
    last: Option<Instant>,
    current: FrameTime,
}

impl Pacing {
    /// Start the clock at `epoch`. Elapsed time is measured from here, so this
    /// is the instant the shaders' `time` reads zero at.
    pub fn new(epoch: Instant) -> Self {
        Self {
            epoch,
            last: None,
            current: FrameTime {
                elapsed: 0.0,
                now: 0.0,
                delta: 0.0,
                index: 0,
            },
        }
    }

    /// Advance to `now` and return the frame time the passes should be given.
    ///
    /// `now` running backwards is treated as a zero delta rather than a
    /// negative one: a monotonic `Instant` cannot go backwards, but a caller
    /// replaying recorded instants can, and a negative delta would drive the
    /// burn-in decay the wrong way.
    pub fn tick(&mut self, now: Instant) -> FrameTime {
        let since_epoch = now.saturating_duration_since(self.epoch);
        let elapsed = since_epoch.as_secs_f32();
        let delta = match self.last {
            None => 0.0,
            Some(last) => now.saturating_duration_since(last).as_secs_f32(),
        };
        let index = match self.last {
            None => 0,
            Some(_) => self.current.index + 1,
        };
        self.last = Some(now);
        self.current = FrameTime {
            elapsed,
            now: since_epoch.as_secs_f64(),
            delta,
            index,
        };
        self.current
    }

    /// Advance by a fixed step. The deterministic form the tests use, and the
    /// form a fixed-rate offscreen render wants.
    pub fn tick_by(&mut self, step: Duration) -> FrameTime {
        let now = self.last.unwrap_or(self.epoch)
            + if self.last.is_none() {
                Duration::ZERO
            } else {
                step
            };
        self.tick(now)
    }

    /// The most recent frame time, without advancing.
    pub fn current(&self) -> FrameTime {
        self.current
    }

    pub fn epoch(&self) -> Instant {
        self.epoch
    }
}
