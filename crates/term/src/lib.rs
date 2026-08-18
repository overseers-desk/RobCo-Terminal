//! RobCo Terminal core.
//!
//! Everything that is a terminal and nothing that is a window: the rio-vt
//! session, the PTY read loop driving it, the grid read-back seam, the DCS
//! tap the tmux gateway hangs on, the remote session a tmux window feeds,
//! and the glyph path that draws a screen of cells. `crates/app` owns the
//! window, the surface and the event loop, and calls in here.
//!
//! The split is what lets the done-tests run without a display: a session
//! needs no window, so a scripted PTY transcript is an ordinary `cargo test`,
//! and the renderer draws into an offscreen texture it reads back.
//!
//! The glyph path is the one glyphon could not provide: cosmic-text shapes
//! and rasterises, this crate thresholds the coverage mask on its way into
//! an atlas it owns, and the fragment shader reads texels with
//! `textureLoad` rather than through a sampler. Magnifying is integer
//! geometry, never re-rasterisation: the counterexample
//! (Terminess re-rasterised at twice the size differs from the doubled bitmap
//! in 3960 pixels) is re-asserted in this crate's tests so the shortcut cannot
//! be taken back later by accident.
//!
//! Layering:
//!
//! * [`session`] / [`dcs`] / [`size`]: the emulation core and its PTY loop.
//! * [`grid`]: the grid seam. `GridView` is the one definition of what a
//!   line says; [`rio_grid`] adapts rio-vt's `Crosswords` onto it and carries
//!   the read-back-as-text path with it.
//! * [`selection`] / [`search`] / [`hotspots`] / [`pointer`]: what the user
//!   has selected, what a search found in scrollback, which spans are links,
//!   and whether a pointer event marks the screen or reaches the program.
//! * [`fonts`]: the bundled catalogue, and [`fonts::sizing`], the seam: a
//!   catalogue row plus the user's knobs in, a `ResolvedFont` out. Nothing
//!   downstream sees the catalogue's raw per-entry properties again.
//! * [`atlas`]: shaping, rasterising, thresholding, packing.
//! * [`cells`] / [`color`]: what a screen of text is, and the one place a
//!   rio-vt `Square` becomes a coloured cell.
//! * [`viewport`]: scrollback policy over rio-vt's display offset.
//! * [`render`]: the damage-driven instance buffer and the draw.
//! * [`gpu`]: an offscreen target and a readback. The grid is drawn into a
//!   texture, not into the swapchain, so the CRT chain can filter the grid
//!   without filtering the chassis around it.

pub mod atlas;
pub mod cells;
pub mod color;
pub mod dcs;
pub mod distortion;
pub mod fonts;
pub mod gpu;
pub mod grid;
pub mod hotspots;
pub mod pointer;
pub mod remote;
pub mod render;
pub mod rio_grid;
pub mod search;
pub mod selection;
pub mod session;
pub mod size;
pub mod viewport;

pub use atlas::{CellMetrics, FontContext, GlyphAtlas, Rasterization};
pub use cells::{Cell, CellGrid, CursorShape, CursorState};
pub use color::{Rgba, Scheme};
pub use dcs::{ControlModeTap, DcsParser, DcsTap, NoopTap};
pub use distortion::{correct_distortion, DistortionParams};
pub use fonts::sizing::{resolve, ResolvedFont, ScalePolicy, SizingRequest};
pub use fonts::{font_by_name, fonts, FontEntry};
pub use gpu::{Gpu, Image, Target};
pub use grid::{GridView, ScriptedGrid};
pub use hotspots::{HotSpot, HotSpotType, UrlFilterChain, UrlType};
pub use render::{GridRenderer, SyncStats};
// The two grid-to-text answers, both at the root because both are asked
// from outside: `live_text` is what the program below has written on the
// screen, `viewport_text` is what the user is looking at. They differ the
// moment the view is scrolled back, which is the case the renderer is in.
pub use cells::vt::viewport_text;
pub use rio_grid::{all_text, cell_char, live_text, row_cells, row_text, screen_contains, RioGrid};
pub use search::{search, SearchHit};
// `distortion::Point` and `selection::Window` stay behind their modules.
// Both are common words that mean something else one crate up (a winit
// `Window`, a pixel point), and neither is asked for often enough to be
// worth the collision at the root.
pub use remote::{ChannelSession, RemoteSession};
pub use selection::{Selection, SelectionController, TripleClickMode};
pub use session::{Pumped, Session, SessionConfig, Term, INPUT_CAP};
pub use size::{CellSize, TermSize, Viewport};
pub use viewport::{ScrollPosition, ViewportChange};

/// Re-exported so downstream crates pin the same rio-vt this crate does
/// rather than adding a second, possibly different, dependency on it.
pub use rio_vt;

/// The threshold this crate measured to work, and the one the atlas uses
/// unless a caller says otherwise: coverage at or above half is ink.
pub const DEFAULT_THRESHOLD: u8 = 128;

/// Printable ASCII. What a terminal atlas holds before anything exotic
/// arrives; the atlas grows on demand for the rest.
pub fn ascii_charset() -> String {
    (0x20u8..0x7f).map(|b| b as char).collect()
}

/// The common opening move: resolve the sizing, build an atlas over printable
/// ASCII in the mode the face is entitled to, and hand back both halves.
///
/// The mode is not a parameter, and deliberately: there is one switch that
/// decides whether a face is antialiased, at [`Rasterization::for_face`]. A
/// caller that could pass its own would be a second place the question is
/// answered.
pub fn build_font(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    spec: &FontEntry,
    request: &SizingRequest,
    policy: ScalePolicy,
) -> (ResolvedFont, FontContext, GlyphAtlas) {
    let resolved = resolve(spec, request, policy);
    let mut font = FontContext::new(spec);
    let atlas = font.build_atlas(
        device,
        queue,
        &resolved,
        &ascii_charset(),
        Rasterization::for_face(&resolved),
    );
    (resolved, font, atlas)
}
