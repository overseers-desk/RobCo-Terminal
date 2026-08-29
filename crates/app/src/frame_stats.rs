//! A frame's GPU-timing instrument: wgpu timestamp queries straddling the
//! three recordings [`crate::window`]'s `draw_frame` puts in one encoder --
//! the grid, the CRT chain, and the bank column's casting -- plus the frame
//! as a whole, aggregated into a rolling window's p50/p99. The frame-time
//! budget question stays open until a real number lands here, and the
//! question this instrument exists to answer is whether file-watch config
//! reload holds up for live preview, or a preview socket (the named
//! fallback escalation) is worth building.
//!
//! # Why a query set per frame, owned by `Frame`
//!
//! [`crate::gpu::Frame`] already owns the one encoder every mark in a frame
//! is written into (`draw_frame`'s module doc: one encoder, three
//! recordings), and a [`wgpu::QuerySet`]'s writes and its eventual resolve
//! have to land in that same encoder before it is `finish`ed. So the query
//! set travels with the encoder rather than living on [`crate::gpu::Gpu`],
//! which only ever sees one frame's encoder at a time. What *does* live on
//! `Gpu` is the aggregate: the rolling window of durations and the
//! present-cadence samples survive across frames, which a per-frame query
//! set cannot.
//!
//! # Feature gate
//!
//! Recording an arbitrary timestamp on a plain `CommandEncoder` (not inside
//! a render pass) needs `Features::TIMESTAMP_QUERY_INSIDE_ENCODERS` on top
//! of the base `Features::TIMESTAMP_QUERY` the wgpu docs otherwise lead
//! with; `draw_frame` marks between three calls whose internals it does not
//! otherwise touch, so the encoder-level write is the only one that fits
//! without threading a query set through `term::GridRenderer`, `crt::Chain`
//! and `crate::chrome::Chrome`. Both features are requested together,
//! filtered to what the adapter actually offers (the same pattern
//! `gpu::required_features` uses); an adapter offering neither, or
//! only the base feature, gets an instrument that reports itself
//! unavailable rather than one that panics reaching for `write_timestamp`.
//!
//! Present cadence -- the CPU-side gap between one `Gpu::present` and the
//! next -- needs no GPU feature at all and is always sampled when the
//! instrument is enabled, even on an adapter with no timestamp support at
//! all: it is how the "measured present cadence vs the `EFFECTS_BASE_FRAME`
//! 60 Hz assumption" question gets answered.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Both features `CommandEncoder::write_timestamp` needs; see the module
/// doc for why the base `TIMESTAMP_QUERY` is not enough on its own.
const WANTED: wgpu::Features =
    wgpu::Features::TIMESTAMP_QUERY.union(wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS);

/// The features this instrument wants, filtered to what the adapter offers.
/// Pass the result into `required_features` alongside `gpu::required_features`'s at
/// device creation; requesting a feature the adapter lacks is a device
/// creation error, not a graceful degradation.
pub fn required_features(adapter: &wgpu::Adapter) -> wgpu::Features {
    adapter.features() & WANTED
}

/// Where in the query set each mark lands. The order matches the frame's
/// own recording order (`crate::window`'s module doc): grid, chain, column,
/// bracketed by the frame's own start and end.
#[derive(Clone, Copy, Debug)]
pub enum Mark {
    FrameStart,
    GridStart,
    GridEnd,
    ChainStart,
    ChainEnd,
    ColumnStart,
    ColumnEnd,
    FrameEnd,
}

impl Mark {
    const COUNT: u32 = 8;

    fn index(self) -> u32 {
        match self {
            Mark::FrameStart => 0,
            Mark::GridStart => 1,
            Mark::GridEnd => 2,
            Mark::ChainStart => 3,
            Mark::ChainEnd => 4,
            Mark::ColumnStart => 5,
            Mark::ColumnEnd => 6,
            Mark::FrameEnd => 7,
        }
    }
}

/// One tick per query, 8 bytes each (`wgpu::QUERY_SIZE`).
const BUFFER_BYTES: u64 = Mark::COUNT as u64 * 8;

/// One frame's timestamps: the query set the marks write into, and the two
/// small buffers a resolve needs (a resolved query set can only land in a
/// buffer, never be read directly; the readback buffer is what gets mapped).
pub struct FrameTiming {
    query_set: wgpu::QuerySet,
    resolve_buffer: wgpu::Buffer,
    readback_buffer: wgpu::Buffer,
}

impl FrameTiming {
    fn new(device: &wgpu::Device) -> Self {
        let query_set = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("frame stats timestamps"),
            ty: wgpu::QueryType::Timestamp,
            count: Mark::COUNT,
        });
        let resolve_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("frame stats resolve"),
            size: BUFFER_BYTES,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("frame stats readback"),
            size: BUFFER_BYTES,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        Self {
            query_set,
            resolve_buffer,
            readback_buffer,
        }
    }

    /// Public, not `pub(crate)`, so an integration test can drive the
    /// query-set plumbing end to end on a device it built itself, the same
    /// way `crate::gpu::Frame::mark` drives it in the shipping frame path.
    pub fn mark(&self, encoder: &mut wgpu::CommandEncoder, point: Mark) {
        encoder.write_timestamp(&self.query_set, point.index());
    }

    /// Copy the eight raw ticks into the readback buffer. Has to run in
    /// this same encoder, before it is `finish`ed: a resolved query set's
    /// data only exists once the resolve command itself has executed on
    /// the GPU, and this is the last chance to record one.
    ///
    /// All eight marks must have been written before this runs, or the
    /// resolve reads a query index the GPU never wrote -- a validation
    /// error, not a graceful zero. `Frame::discard_timing` is the escape
    /// hatch for a frame that will not draw the usual three recordings.
    pub fn resolve(&self, encoder: &mut wgpu::CommandEncoder) {
        encoder.resolve_query_set(&self.query_set, 0..Mark::COUNT, &self.resolve_buffer, 0);
        encoder.copy_buffer_to_buffer(
            &self.resolve_buffer,
            0,
            &self.readback_buffer,
            0,
            BUFFER_BYTES,
        );
    }

    /// Block until the GPU has executed this frame's submission, then read
    /// the eight ticks back and turn them into four interval durations.
    /// `submission` pins the wait to this frame specifically -- the
    /// alternative, waiting for "whatever is outstanding", would let a
    /// later frame's tail wake this wait early on a GPU deep in its queue.
    fn read(
        &self,
        device: &wgpu::Device,
        period_ns: f32,
        submission: wgpu::SubmissionIndex,
    ) -> Option<StageDurations> {
        // The same map-then-poll-then-read shape `gpu::read_back`'s and
        // `gpu::harness`'s readbacks already use: `PollType::Wait`
        // runs the mapping callback synchronously before returning, so
        // there is no callback to store and no second wakeup to wait for.
        self.readback_buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, |_| {});
        device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: None,
            })
            .ok()?;
        let ticks: Vec<u64> = {
            let view = self.readback_buffer.slice(..).get_mapped_range().ok()?;
            view.chunks_exact(8)
                .map(|c| u64::from_le_bytes(c.try_into().expect("8-byte chunk")))
                .collect()
        };
        self.readback_buffer.unmap();

        let ns = |from: Mark, to: Mark| -> f64 {
            let (from, to) = (from.index() as usize, to.index() as usize);
            ticks[to].saturating_sub(ticks[from]) as f64 * period_ns as f64
        };
        Some(StageDurations {
            grid_ms: ns(Mark::GridStart, Mark::GridEnd) / 1e6,
            chain_ms: ns(Mark::ChainStart, Mark::ChainEnd) / 1e6,
            column_ms: ns(Mark::ColumnStart, Mark::ColumnEnd) / 1e6,
            frame_ms: ns(Mark::FrameStart, Mark::FrameEnd) / 1e6,
        })
    }
}

/// One frame's four GPU interval durations, in milliseconds.
#[derive(Clone, Copy, Debug, Default)]
pub struct StageDurations {
    pub grid_ms: f64,
    pub chain_ms: f64,
    pub column_ms: f64,
    pub frame_ms: f64,
}

/// A window's p50/p99 for one series, `None` when the window has no samples
/// for it yet (or ever, on an adapter with no GPU timings).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Percentiles {
    pub p50: f64,
    pub p99: f64,
}

/// The programmatic accessor: everything a test or a log
/// line wants out of the rolling window, read without reaching into it.
#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    pub grid_ms: Option<Percentiles>,
    pub chain_ms: Option<Percentiles>,
    pub column_ms: Option<Percentiles>,
    pub frame_ms: Option<Percentiles>,
    pub present_interval_ms: Option<Percentiles>,
    /// How many GPU-timed frames are in the window right now (0 on an
    /// adapter with no timestamp support, even with the instrument on).
    pub gpu_samples: usize,
    /// How many present-interval samples are in the window.
    pub cadence_samples: usize,
}

/// Rolling window size. 8192 samples comfortably covers a 30+ second
/// measurement run at any cadence this app can reach -- from the effects
/// clock's 20 Hz idle floor up past a flood-driven redraw well over 60 Hz --
/// while staying a bounded window rather than an ever-growing log.
const WINDOW_CAPACITY: usize = 8192;

/// How often `--frame-stats` prints a log line.
const LOG_INTERVAL: Duration = Duration::from_secs(5);

struct Window {
    durations: VecDeque<StageDurations>,
    cadence_ms: VecDeque<f64>,
}

impl Window {
    fn new() -> Self {
        Self {
            durations: VecDeque::with_capacity(WINDOW_CAPACITY),
            cadence_ms: VecDeque::with_capacity(WINDOW_CAPACITY),
        }
    }

    fn push_duration(&mut self, d: StageDurations) {
        if self.durations.len() == WINDOW_CAPACITY {
            self.durations.pop_front();
        }
        self.durations.push_back(d);
    }

    fn push_cadence(&mut self, ms: f64) {
        if self.cadence_ms.len() == WINDOW_CAPACITY {
            self.cadence_ms.pop_front();
        }
        self.cadence_ms.push_back(ms);
    }

    /// Nearest-rank percentiles over an unsorted sample iterator.
    fn percentiles(values: impl Iterator<Item = f64>) -> Option<Percentiles> {
        let mut v: Vec<f64> = values.collect();
        if v.is_empty() {
            return None;
        }
        v.sort_by(|a, b| a.partial_cmp(b).expect("no NaN durations"));
        let pick = |p: f64| -> f64 {
            let n = v.len();
            let idx = ((p / 100.0) * n as f64).ceil() as usize;
            v[idx.clamp(1, n) - 1]
        };
        Some(Percentiles {
            p50: pick(50.0),
            p99: pick(99.0),
        })
    }

    fn stats(&self) -> Stats {
        Stats {
            grid_ms: Self::percentiles(self.durations.iter().map(|d| d.grid_ms)),
            chain_ms: Self::percentiles(self.durations.iter().map(|d| d.chain_ms)),
            column_ms: Self::percentiles(self.durations.iter().map(|d| d.column_ms)),
            frame_ms: Self::percentiles(self.durations.iter().map(|d| d.frame_ms)),
            present_interval_ms: Self::percentiles(self.cadence_ms.iter().copied()),
            gpu_samples: self.durations.len(),
            cadence_samples: self.cadence_ms.len(),
        }
    }
}

/// The live-preview throughput instrument. One lives on every [`crate::gpu::Gpu`],
/// whether or not `--frame-stats` was passed: a disabled instrument is a few
/// bytes and does nothing on the frame path, which is what keeps every run
/// that never asked for it exactly as fast as before this instrument existed.
pub struct Instrument {
    enabled: bool,
    gpu_supported: bool,
    period_ns: f32,
    window: Window,
    last_present: Option<Instant>,
    last_log: Instant,
}

impl Instrument {
    /// `enabled` is `--frame-stats`. GPU support is read off the device
    /// regardless, so `gpu_available` is correct even when `enabled` is
    /// false (a test can ask "would this adapter support it" without
    /// turning the instrument on).
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, enabled: bool) -> Self {
        let gpu_supported = device.features().contains(WANTED);
        if enabled && !gpu_supported {
            log::warn!(
                "--frame-stats: this adapter offers no TIMESTAMP_QUERY (+ \
                 TIMESTAMP_QUERY_INSIDE_ENCODERS); reporting present cadence only, no GPU pass timings"
            );
        }
        Self {
            enabled,
            gpu_supported,
            period_ns: queue.get_timestamp_period(),
            window: Window::new(),
            last_present: None,
            last_log: Instant::now(),
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Whether GPU pass timings are available: the instrument is on *and*
    /// the device carries both timestamp features. `false` is the
    /// documented fallback, not an error -- see the module doc.
    pub fn gpu_available(&self) -> bool {
        self.enabled && self.gpu_supported
    }

    /// A fresh per-frame query set, or `None` when this frame will not be
    /// GPU-timed.
    pub fn new_frame_timing(&self, device: &wgpu::Device) -> Option<FrameTiming> {
        self.gpu_available().then(|| FrameTiming::new(device))
    }

    /// Called from `Gpu::present` right after `queue.submit`: samples the
    /// CPU-side present interval whenever the instrument is enabled, reads
    /// back this frame's GPU timings if it was timed, and prints the
    /// periodic log line.
    pub fn after_present(
        &mut self,
        device: &wgpu::Device,
        timing: Option<(&FrameTiming, wgpu::SubmissionIndex)>,
    ) {
        if !self.enabled {
            return;
        }
        let now = Instant::now();
        if let Some(last) = self.last_present.replace(now) {
            self.window
                .push_cadence(now.duration_since(last).as_secs_f64() * 1000.0);
        }
        if let Some((timing, submission)) = timing {
            if let Some(d) = timing.read(device, self.period_ns, submission) {
                self.window.push_duration(d);
            }
        }
        if now.duration_since(self.last_log) >= LOG_INTERVAL {
            self.last_log = now;
            log::info!("{}", self.summary_line());
        }
    }

    /// The rolling window's p50/p99s: `--frame-stats`'s log line reads
    /// this, and so does a test.
    pub fn stats(&self) -> Stats {
        self.window.stats()
    }

    fn summary_line(&self) -> String {
        let stats = self.stats();
        let fmt = |p: Option<Percentiles>| match p {
            Some(p) => format!("p50={:.3}ms p99={:.3}ms", p.p50, p.p99),
            None => "n/a".to_string(),
        };
        if self.gpu_available() {
            format!(
                "frame stats ({} gpu, {} cadence samples): grid {} chain {} column {} frame {} \
                 present-interval {}",
                stats.gpu_samples,
                stats.cadence_samples,
                fmt(stats.grid_ms),
                fmt(stats.chain_ms),
                fmt(stats.column_ms),
                fmt(stats.frame_ms),
                fmt(stats.present_interval_ms),
            )
        } else {
            format!(
                "frame stats: GPU pass timings unavailable on this adapter ({} cadence samples); \
                 present-interval {}",
                stats.cadence_samples,
                fmt(stats.present_interval_ms),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentiles_of_one_to_hundred_land_where_nearest_rank_says() {
        let values = (1..=100).map(|n| n as f64);
        let p = Window::percentiles(values).expect("non-empty");
        // Nearest-rank on 1..=100: p50 -> ceil(0.5*100)=50th smallest = 50;
        // p99 -> ceil(0.99*100)=99th smallest = 99.
        assert_eq!(
            p,
            Percentiles {
                p50: 50.0,
                p99: 99.0
            }
        );
    }

    #[test]
    fn percentiles_of_one_sample_are_that_sample() {
        let p = Window::percentiles(std::iter::once(4.2)).expect("one sample");
        assert_eq!(p, Percentiles { p50: 4.2, p99: 4.2 });
    }

    #[test]
    fn percentiles_of_no_samples_is_none() {
        assert!(Window::percentiles(std::iter::empty()).is_none());
    }

    #[test]
    fn a_disabled_instrument_never_asks_a_device_for_a_query_set() {
        // `enabled: false` is checked before `gpu_available` ever reads
        // `gpu_supported`, and `new_frame_timing` is the only thing that
        // would try to build one. No device is needed to prove that:
        // gpu_available combines `enabled` with a stored bool by `&&`, so a
        // false `enabled` short-circuits regardless of what the (absent)
        // device would have said.
        struct Fake {
            enabled: bool,
            gpu_supported: bool,
        }
        impl Fake {
            fn gpu_available(&self) -> bool {
                self.enabled && self.gpu_supported
            }
        }
        let disabled_but_capable = Fake {
            enabled: false,
            gpu_supported: true,
        };
        assert!(!disabled_but_capable.gpu_available());
    }

    /// Forces the fallback path: an instrument whose adapter lacks the
    /// timestamp features (`gpu_supported: false`)
    /// still runs, reports itself unavailable, and never panics reaching
    /// for a query set. This is the pure-logic half of that test; the
    /// GPU-backed half (a real device that deliberately did not request
    /// the features) lives in `tests/suite/frame_stats.rs`.
    #[test]
    fn an_instrument_the_adapter_cannot_time_reports_unavailable_not_a_panic() {
        let window = Window::new();
        let instrument = Instrument {
            enabled: true,
            gpu_supported: false,
            period_ns: 1.0,
            window,
            last_present: None,
            last_log: Instant::now(),
        };
        assert!(instrument.enabled());
        assert!(!instrument.gpu_available());
        let stats = instrument.stats();
        assert!(stats.grid_ms.is_none());
        assert!(stats.frame_ms.is_none());
        // The line the log gets: no numbers to lie about, and no panic.
        assert!(instrument.summary_line().contains("unavailable"));
    }
}
