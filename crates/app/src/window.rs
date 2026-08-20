//! What fills a shell window: the wgpu surface, the rio-vt session
//! behind it, and the input path in front of it.
//!
//! Two events change the grid, and only one of them is a resize.
//! `Resized` moves the window's pixels; `ScaleFactorChanged` moves the
//! *cell* while the window's logical size stays put. Both end at
//! `sync_geometry`, which recomputes the grid from the viewport and
//! applies it to the session, so neither can be handled correctly and
//! the other forgotten.
//!
//! The event loop itself is [`crate::shell`]'s, not this module's: the
//! PTY is not a winit event source, so [`TerminalSurface::tick`] asks
//! the shell to wake it again through [`Tick::wake_at`] rather than
//! spinning a loop of its own.
//!
//! Two paths through this module put several separately-tested pieces in a
//! fixed order, and neither order is a matter of taste. The first is the
//! redraw ([`Glass`]):
//!
//! 1. the settings snapshot reaches the chain, which decides for itself
//!    whether that is a uniform push or a rebuild;
//! 2. the renderer syncs from the session, which is where rio-vt's damage
//!    turns into instance-buffer writes;
//! 3. the clock ticks, once, and the degauss hook is sampled off the same
//!    instant;
//! 4. `Params::build` turns settings plus geometry plus that instant into
//!    uniforms, and the chain takes them;
//! 5. one encoder records the grid into the offscreen target, then the chain
//!    from that target into the screen well's rectangle of the swapchain view,
//!    then the bank column's casting over what the chain left of the rest;
//! 6. present.
//!
//! Nothing between steps 3 and 6 reads a clock again: a frame drawn on two
//! instants fades the burn-in ghost by one of them and animates by the other.
//!
//! The column comes last and not first because the chain's own last pass clears
//! the whole image before it draws its rectangle of it; [`crate::column`] says
//! more, and it is the reason the casting is composited rather than given a
//! second `Viewport` into the same view.
//!
//! # Where the window ends and the well begins
//!
//! With a chassis shown the window is two rectangles, not one
//! (`chassis::WindowLayout`): a bank column at the left and the screen well to
//! its right. Everything that used to mean "the window" means the *well* here:
//! the surface's [`Viewport`], the grid the session is resized to, the
//! offscreen target, the chain's geometry and every uniform normalised by
//! `normalizedScreenScale`. The window's own size survives as
//! `window_size`, and only two things read it: the swapchain, which is the
//! whole window, and the arithmetic that divides the two.
//!
//! A surface with no cabinet (every headless one, and any window whose
//! profile hides the chassis) has a bank of no width, so the well *is* the
//! window and none of the above is visible.
//!
//! The second path is the pointer:
//!
//! 0. the seam gets first refusal: a press, a drag or a hover on the ten pixels
//!    where the bank's plastic meets the glass is the cabinet's
//!    (`chassis::seam`), and never also the grid's;
//! 1. every pointer position that is the grid's is measured from the well's
//!    left edge and goes through [`term::distortion`] first: the click
//!    landed on the bent glass, so it is pushed back through the warp
//!    before it is anything else;
//! 2. the corrected point divides by the cell into an *absolute* grid
//!    cell, which is the coordinate [`term::selection`] works in;
//! 3. [`term::pointer`] decides what the event means (mark the screen,
//!    report to the program, or paste), because that depends on terminal
//!    state (mouse reporting, frozen glass), not on the window;
//! 4. only then does anything happen: a selection, a
//!    [`crate::mouse`]-encoded report down the PTY, or a
//!    [`crate::clipboard`] copy.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chassis::{Cabinet, SeamCursor, SeamUpdate};
use config::toml::Scalar;
use config::Config;
use crt::{Chain, Degauss, Geometry, Pacing, Params};
use term::distortion::{self, correct_distortion, DistortionParams};
use term::fonts::sizing::{ScalePolicy, SizingRequest};
use term::pointer::{self, on_press, PointerAction, PointerContext};
use term::rio_vt::crosswords::Mode;
use term::selection::{self, SelectionController};
use term::{
    CellSize, ChannelSession, ControlModeTap, FontContext, FontEntry, GridRenderer, ResolvedFont,
    RioGrid, Scheme, ScrollPosition, Session, SessionConfig, Target, TmuxPane, Viewport,
};
use tmux_cc::{PaneId, WindowId};
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{Ime, MouseButton, MouseScrollDelta};
use winit::event_loop::EventLoopProxy;
use winit::keyboard::ModifiersState;
use winit::window::{CursorIcon, Window};

use crate::badge::Badge;
use crate::bank::BankPager;
use crate::channels::{ChannelKind, Channels, Close, PageId};
use crate::chord::{Chord, ChordInput};
use crate::column::Column;
use crate::frame_stats::Mark;
use crate::gpu::Gpu;
use crate::input::{encode_winit_key, KeyAction, KeyboardModes, Modifiers};
use crate::settings::{self, SettingsHandle};
use crate::shell::{ShellEvent, Surface, Tick};
use crate::tmux::{Gateway, GatewayEvent};
use crate::{clipboard, mouse, paths};

/// How often the loop wakes to drain the PTY when nothing else is
/// happening. ~125 Hz: comfortably below a frame at 60 Hz, so output
/// never waits a frame longer than it has to, and cheap enough to idle
/// on.
const POLL_INTERVAL: Duration = Duration::from_millis(8);

/// One frame at 60 Hz, the rate `general.effects_frame_skip` counts in.
///
/// Every shader in the chain reads `time`, and it is republished only every
/// `effectsFrameSkip`th frame rather than continuously, so that division is
/// what the CRT actually animates at: 20 Hz at the shipped skip of 3.
/// Nothing here is vsync-paced -- the event loop is a timer, not a frame
/// callback -- so 60 Hz is assumed and the skip multiplies it. If a real
/// display session (R2) says the assumption costs anything, the fix is to
/// measure the present interval and divide that, not to change what the
/// setting means.
const EFFECTS_BASE_FRAME: Duration = Duration::from_micros(16_667);

/// What the badge says when a PTY channel's write queue sheds: the child this
/// program spawned has stopped reading its tty and the keystrokes aimed at it
/// are being thrown away. See [`TerminalSurface::watch_the_write_queues`] for
/// why the wording is this short.
pub const SHED_PTY: &str = "input dropped";

/// The same, for the queue on the way to a tmux server: the tmux server is
/// not reading its control wire, so what is being dropped is `send-keys`.
pub const SHED_TMUX: &str = "tmux input dropped";

/// Not a config key: a fixed multiplier applied to the stored `fontScaling`.
/// `SizingRequest::default()` carries the same 0.75, and this name exists so
/// the one place that needs the product for a shader uniform
/// (`totalFontScaling`) does not restate the number.
const BASE_FONT_SCALING: f64 = 0.75;

/// The chain's geometry, from a physical render target and the window it is
/// drawn in. The arithmetic of [`Glass::geometry`], with nothing in it that
/// needs a device, so the unit conversion can be measured without one.
fn chain_geometry(
    target: (u32, u32),
    integer_scale: u32,
    cfg: &Config,
    scale_factor: f64,
) -> Geometry {
    let scale = integer_scale.max(1) as f32;
    let (width, height) = (target.0 as f32, target.1 as f32);
    let ratio = if scale_factor > 0.0 {
        scale_factor as f32
    } else {
        1.0
    };
    let font_width = cfg.screen.font_width.max(0.01) as f32;
    Geometry {
        output_width: width / ratio,
        output_height: height / ratio,
        virtual_width: (width / (scale * font_width)).floor().max(1.0),
        virtual_height: (height / scale).floor().max(1.0),
        total_font_scaling: (BASE_FONT_SCALING * cfg.general.font_scaling) as f32,
        device_pixel_ratio: ratio,
    }
}

/// The bank column's footprint in physical pixels, held to at most one pixel
/// short of the window so the screen well never has none.
///
/// Free rather than a method because [`TerminalSurface::new`] needs it before
/// there is a surface to ask.
fn bank_physical(cabinet: &Cabinet, scale_factor: f64, window_width: u32) -> u32 {
    cabinet
        .bank_width_physical(scale_factor)
        .min(window_width.saturating_sub(1))
}

/// The catalogue entry `screen.font_name` names, or the shipped default if it
/// names nothing (a hand-edited config can, and a font that has gone missing
/// is not a reason to refuse to draw).
fn font_entry(cfg: &Config) -> &'static FontEntry {
    term::font_by_name(&cfg.screen.font_name)
        .or_else(|| term::font_by_name(&Config::default().screen.font_name))
        .expect("the bundled catalogue always contains the default font")
}

/// What a channel slot holds in this binary: a PTY session carrying the
/// tmux-detecting tap, or a tmux pane's screen the gateway feeds.
pub type AppSession = ChannelSession<ControlModeTap>;

/// Start a channel's session, or log why not. A window that cannot start a
/// second shell keeps the first one and its picture; refusing the slot is the
/// whole of the failure, which is why [`Channels`] takes a closure that may
/// answer `None` rather than a session.
fn spawn(config: &SessionConfig, size: term::TermSize) -> Option<AppSession> {
    match Session::spawn(config, size, ControlModeTap::default()) {
        Ok(s) => Some(ChannelSession::Pty(s)),
        Err(e) => {
            log::error!("could not start a session: {e}");
            None
        }
    }
}

/// The sizing knobs, as this window's settings and monitor set them.
fn sizing_request(cfg: &Config, scale_factor: f64) -> SizingRequest {
    SizingRequest {
        font_scaling: cfg.general.font_scaling,
        line_spacing: cfg.screen.line_spacing,
        font_width: cfg.screen.font_width,
        window_scaling: cfg.general.window_scaling,
        device_pixel_ratio: scale_factor,
        ..SizingRequest::default()
    }
}

/// The cell box in logical pixels, which is what [`Viewport`] takes.
///
/// The renderer's cell is `atlas.cell * integer_scale` *physical* pixels, and
/// `Viewport::physical_cell` multiplies by the DPR and rounds, so dividing
/// here and multiplying there is exact rather than approximately exact: the
/// grid the session is resized to is the grid the renderer draws.
fn logical_cell(cell: term::CellMetrics, resolved: &ResolvedFont, scale_factor: f64) -> CellSize {
    let scale = resolved.integer_scale as f64;
    CellSize {
        width: (f64::from(cell.width) * scale / scale_factor) as f32,
        height: (f64::from(cell.height) * scale / scale_factor) as f32,
    }
}

/// Everything behind the glass that needs a GPU: the grid renderer and its
/// atlas, the offscreen target the grid is drawn into, and the CRT chain that
/// takes that target to the swapchain.
///
/// It is one struct because it is one lifetime. Every field is built from the
/// same device, and the one that is not (`pacing`) is the clock that chain is
/// driven by, which has nowhere better to live: a second clock would be a
/// second answer to what time this frame is.
///
/// The degauss transient used to sit here beside it and does not any more. It
/// is *triggered* by a channel change and only *sampled* by the frame, so it
/// belongs to the surface's state and not the device's: a window whose chain
/// failed to load still switches channels, and a headless surface -- which is
/// where the channel state machines are tested -- has no `Glass` at all.
struct Glass {
    renderer: GridRenderer,
    /// What the atlas was built for. A settings edit that moves any of it
    /// means the atlas is the wrong size and has to be rebuilt, which is also
    /// one of the two events `crt-burnin`'s mount contract calls a
    /// discontinuity of the ghost.
    font_name: String,
    resolved: ResolvedFont,
    /// The chain's input, sized to the screen well rather than to the grid: the
    /// grid is centred in it at a whole-pixel origin, so a glyph still lands
    /// on the pixels it was rasterised for. Sizing it to the grid instead
    /// would leave the chain stretching a slightly-off-aspect picture over the
    /// well, which is a resample of exactly the pixel font this renderer
    /// exists to keep unresampled.
    target: Target,
    chain: Chain,
    /// The bank column's casting, mounted outside the chain. `None`
    /// when its pass could not be loaded, which is a window with a bare well
    /// rather than a process that dies.
    column: Option<Column>,
    /// The size badge's mount, outside the chain for the same reason the
    /// column is (`crate::badge`'s module doc). It holds no state of its own:
    /// what to say and how strongly are the shell's overlay's answer, pushed
    /// through [`crate::shell::Surface::set_size_badge`].
    badge: Badge,
    pacing: Pacing,
    /// The settings the chain was last given, so a redraw can tell whether
    /// there is anything to apply.
    applied: Config,
    /// Target size and grid size as last logged, so the one line that says
    /// where the picture is appears when it moves and not once a frame.
    logged_geometry: Option<(u32, u32, u32, u32)>,
}

/// Two clicks inside this window on the same cell are a double click.
/// winit does not expose the platform's own double-click interval, so this
/// is the X11/GTK desktop default an application would otherwise be handed.
const DOUBLE_CLICK_INTERVAL: Duration = Duration::from_millis(400);

/// The surface the binary runs behind the
/// [`crate::shell::Surface`] seam: the shell owns the event loop and the
/// window, this owns what is inside it.
/// What the input method has told this surface so far.
///
/// Deliberately small. The commit is written straight through and kept nowhere;
/// what is held is what a caller may need to *ask* about -- whether composition
/// is open, and what the half-typed word is. The frame reads the word every
/// redraw and draws it at the cursor, so this is the composition's one home
/// rather than a copy of one.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ImeState {
    /// The input method has taken the keyboard: `Ime::Enabled` arrived and no
    /// `Ime::Disabled` has since.
    pub enabled: bool,
    /// The uncommitted composition string, `""` when there is none.
    pub preedit: String,
    /// The cursor/selection inside the pre-edit, in byte offsets, as winit
    /// reports it; `None` is the input method asking for no caret inside the
    /// composition.
    ///
    /// Kept and not drawn: the whole pre-edit is painted as a single block, so
    /// this offset is stored for a caller that might want it but is not
    /// consulted to draw the composition.
    pub cursor: Option<(usize, usize)>,
}

pub struct TerminalSurface {
    /// `None` in a headless surface (see [`TerminalSurface::headless`]).
    /// It is read for the window's own size on a DPI change, and it is
    /// what says whether there is a display to reach a clipboard through.
    window: Option<Arc<Window>>,
    gpu: Option<Gpu>,
    /// This window's channels, one session each, and which of them is on the
    /// air (`crate::channels`). Empty only where the first session
    /// could not be spawned, which is the state the single `Option<Session>`
    /// this replaced used to carry.
    channels: Channels<AppSession>,
    /// Each tmux attachment's client half, keyed by the page it raised. The
    /// write side is a second handle onto the gateway channel's PTY
    /// (`term::Session::control_mode_writer`); the read side arrives through the
    /// gateway channel's DCS tap on every [`TerminalSurface::pump`].
    gateways: HashMap<PageId, Gateway<std::fs::File>>,
    /// How a channel this window opens later is started: every channel gets
    /// the same configuration, so the shell a `Ctrl+Shift+T` starts is the
    /// shell the window was launched with.
    session_config: SessionConfig,
    /// Where the bank's numerals stand (`crate::bank`), which is what the
    /// chord's slots and a press on a strip are resolved against.
    pager: BankPager,
    /// The digits typed against those numerals (`crate::chord`).
    chord: ChordInput,
    /// Whether the chord modifier is down, so its release can commit -- Alt
    /// everywhere but macOS, where it is Meta.
    chord_modifier: bool,
    /// The channel-change transient. Triggered here, sampled by the frame; see
    /// [`Glass`] for why it does not live there.
    degauss: Degauss,
    /// The pair the glass was last showing, so the work that follows a switch
    /// runs on a switch and not on every pump.
    on_air: (crate::channels::PageId, u32),
    /// The **screen well**, not the window: its size is what the grid, the
    /// offscreen target and the chain are all measured in. See the module doc.
    viewport: Viewport,
    /// The window's own inner size in physical pixels. The swapchain is this
    /// big; the well is this less the bank column.
    window_size: (u32, u32),
    /// The cabinet the well is set into, or `None` for a surface that stands in
    /// no chassis at all: every headless one unless a test asks for one
    /// ([`TerminalSurface::set_cabinet`]), which is what keeps a surface with
    /// no cabinet identical to the pre-cabinet behaviour rather than merely
    /// close to it.
    cabinet: Option<Cabinet>,
    /// The profile the cabinet was last measured for, so a redraw can tell
    /// whether the lamp font or the shell moved. Re-measuring reads a font
    /// face, which is not a per-frame cost worth paying for nothing.
    cabinet_cfg: Config,
    /// The seam took the press that is still down, so the release and the
    /// motions between belong to it and not to the grid.
    seam_press: bool,
    /// What the drag has landed and the settings do not carry yet. The write
    /// happens once, when the button comes up: the cabinet is already at the
    /// new count (`chassis::Cabinet::cursor_moved` takes it on the spot), so
    /// writing per motion would be a file rewrite and a reload per pixel of
    /// travel for a value the screen already shows.
    pending_led_characters: Option<i32>,
    /// Whether the pointer is currently wearing the seam's shape, so the
    /// window is only told when it changes.
    seam_cursor: bool,
    /// How this surface reaches its shell. The bank's width is the window's
    /// minimum-width hint and the shell owns every window, so a seam drag has
    /// to cross back ([`ShellEvent::SetBankWidth`]).
    shell_events: Option<EventLoopProxy<ShellEvent>>,
    modes: KeyboardModes,
    /// What the input method is composing but has not committed, and whether
    /// composition is open at all.
    ///
    /// Read by every redraw, which is what puts the composition on the glass at
    /// the cursor ([`TerminalSurface::draw_frame`], and
    /// `term::render::GridRenderer::set_preedit` under it); the commit goes
    /// straight to the child and is kept nowhere. See
    /// [`TerminalSurface::ime_input`].
    ime: ImeState,
    eof: bool,
    /// Where the view sits in the scrollback. The wheel moves it.
    scroll: ScrollPosition,
    selection: SelectionController,
    /// The absolute cell the pointer was last over.
    pointer_cell: (usize, usize),
    /// The left button is down and a move extends the selection.
    dragging: bool,
    /// When and where the last left press landed, for the double click.
    last_click: Option<(Instant, (usize, usize))>,
    /// What the last completed selection said. The clipboard is the real
    /// destination; this is what a test with no display can read.
    last_selection: Option<String>,
    /// Pixels of wheel travel not yet worth a line, for the trackpads
    /// that report in pixels rather than notches.
    wheel_pixels: f64,
    /// The live config the pointer's inverse-distortion transform reads
    /// (margin, frame size, screen curvature) on every press/release/move.
    /// `None` in a headless surface with no settings handle
    /// attached (see [`TerminalSurface::headless`] and
    /// [`TerminalSurface::set_settings`]): every distortion parameter then
    /// falls back to the neutral/identity value, matching this crate's
    /// original placeholder behavior so a test that never opts in is
    /// unaffected by a settings handle it never asked for.
    settings: Option<Arc<SettingsHandle>>,
    /// What the settings are when there is no handle to ask.
    ///
    /// `--default-settings` reads and watches no file, so no handle is built
    /// -- but `--profile <name>` still resolves a look, and that resolution
    /// is the whole config this run is meant to wear. Without somewhere to
    /// put it the surface fell back to `Config::default()` on every frame and
    /// the flag reached the log line and nothing else, which is the shape
    /// `xtask snap` screenshots by name.
    ///
    /// A plain `Config` rather than a second handle: there is no file behind
    /// it and nothing to reload, so the only thing it needs to do is be
    /// readable once a frame. Defaults until [`TerminalSurface::set_config`]
    /// says otherwise, which keeps every headless test that never sets one
    /// exactly where it was.
    base: Config,
    /// When the next effects frame is due. The CRT animates on wall time --
    /// the glowing line sweeps, the noise crawls, the ghost fades -- so a
    /// window whose child has said nothing for a minute still owes the screen
    /// a frame. Without this the picture freezes on whatever the last byte of
    /// output left behind, which is a still photograph of a CRT rather than a
    /// CRT.
    next_effects_frame: Option<Instant>,
    /// The output governor: when child output may next ask for a frame, and
    /// whether any arrived since the last one it asked for. The PTY is
    /// polled at ~125 Hz and a flood would otherwise paint
    /// frames a 60 Hz panel never shows (measured on the Intel
    /// LNL: present-interval p50 3.3 ms during a `seq` flood). Output is
    /// therefore coalesced to [`EFFECTS_BASE_FRAME`]; a byte that arrives
    /// between frames waits for the next one, never longer.
    next_output_frame: Option<Instant>,
    output_pending: bool,
    /// Everything that needs a device: the grid renderer, the offscreen
    /// target and the CRT chain. `None` in a headless surface, and also in a
    /// windowed one whose chain failed to load, which is a window that clears
    /// rather than a process that dies.
    glass: Option<Glass>,
    /// The size badge as the shell last reported it: the text and the opacity
    /// its envelope has reached. Held rather than acted on, because it arrives
    /// from the loop's clock and is spent on the next frame
    /// ([`crate::shell::Surface::set_size_badge`]).
    size_badge: (String, f32),
    /// The caret rectangle the input method was last told about, in whole
    /// physical pixels, so a caret that has not moved is not republished every
    /// turn of the loop. See [`TerminalSurface::publish_ime_cursor`].
    ime_area: Option<(i32, i32, i32, i32)>,
    /// The second badge in the stack: what this appliance has to say for itself
    /// right now (a write queue shedding, a look saved). Raised here and drawn
    /// by [`Self::draw_frame`]; see [`crate::overlay::Notice`] for why it is the
    /// rebuild's own and not a port.
    notice: crate::overlay::Notice,
    /// The shed counters as of the last [`Self::pump`]: the local children's
    /// input queues, and the gateways' command queues. A count that has moved is
    /// exactly "something the user typed was thrown away since we last looked",
    /// which is what [`Self::notice`] then says out loud.
    sheds_seen: (u64, u64),
}

impl Glass {
    /// Build the glass for a window that has a GPU.
    ///
    /// `cfg` is the settings this window opens with. It is not necessarily the
    /// user's: `TerminalSurface::new` runs before the settings handle is
    /// attached, so the first frame's redraw is what reconciles the two, and
    /// it does that through the same two paths a later edit takes
    /// ([`TerminalSurface::apply_live_settings`]).
    fn new(gpu: &Gpu, cfg: &Config, viewport: &Viewport, identity: &str) -> Option<Self> {
        let entry = font_entry(cfg);
        let request = sizing_request(cfg, viewport.scale_factor);
        let (resolved, _font, atlas) =
            term::build_font(&gpu.device, &gpu.queue, entry, &request, ScalePolicy::Floor);

        let size = viewport.term_size();
        // White on black, and the phosphor nowhere in it: the chain's last
        // pass converts a grey into the profile's two colours, so a grid
        // drawn in amber here would be tinted twice.
        let scheme = Scheme::monochrome([1.0, 1.0, 1.0, 1.0], [0.0, 0.0, 0.0, 1.0]);
        let mut renderer = GridRenderer::new(
            &gpu.device,
            &gpu.queue,
            atlas,
            size.cols(),
            size.rows(),
            scheme,
        );
        renderer.set_scale(resolved.integer_scale);

        // The well, not the window: the chain draws into the rectangle the bank
        // column leaves, and `Params::build` reads `normalizedScreenScale` off
        // the geometry this target's size produces.
        let target = Target::new(&gpu.device, viewport.width.max(1), viewport.height.max(1));

        let dir = paths::preset_dir(identity);
        let chain = match Chain::from_config(&gpu.device, &gpu.queue, &dir, cfg) {
            Ok(chain) => chain,
            Err(e) => {
                log::error!("could not load the CRT chain from {}: {e}", dir.display());
                return None;
            }
        };
        log::info!(
            "glass: {} at {}px x{} scale, {}x{} cells, preset {}",
            entry.name,
            resolved.raster_pixel_size,
            resolved.integer_scale,
            size.cols(),
            size.rows(),
            chain.preset_path().display()
        );

        Some(Self {
            renderer,
            font_name: entry.name.to_string(),
            resolved,
            target,
            chain,
            column: Column::new(&gpu.device, &gpu.queue, &dir, gpu.format()),
            badge: Badge::new(&gpu.device, gpu.format()),
            // One clock per window, started here, never read anywhere else.
            pacing: Pacing::new(Instant::now()),
            applied: cfg.clone(),
            logged_geometry: None,
        })
    }

    /// The frame's measurements, as `crt::Params` wants them.
    ///
    /// `output_*` is in **logical** pixels, which is what `crt::Geometry`
    /// documents: the render target is physical, so the window's scale
    /// factor is divided back out here. This is the one place in the tree
    /// that conversion happens. Left undone it halves `normalizedScreenScale`
    /// on a 2x display, and with it `ScreenCurvature` and `FrameSize` -- a
    /// flatter tube in a thinner moulding, on exactly the machines that show
    /// it most clearly.
    ///
    /// `virtual_*` is the terminal area in *unscaled* raster pixels, which is
    /// what the rasterization mask is spaced by. It is a raster count rather
    /// than a length, so it stays on the physical size over this renderer's
    /// integer scale. The width is additionally divided by the profile's
    /// `fontWidth`, and both axes are floored to whole pixels: a mask spaced
    /// by 639.4 stripes is not the mask spaced by 639.
    fn geometry(&self, scale_factor: f64) -> Geometry {
        chain_geometry(
            (self.target.width, self.target.height),
            self.resolved.integer_scale,
            &self.applied,
            scale_factor,
        )
    }
}

impl TerminalSurface {
    /// Builds a surface for a window the shell has already created.
    /// Failure to get a GPU or a PTY is logged and leaves an empty
    /// surface: a window that shows nothing beats a process that dies
    /// with no window at all, which is what the contract harness would
    /// see.
    pub fn new(window: &Arc<Window>, session: &SessionConfig, frame_stats_enabled: bool) -> Self {
        let window = Arc::clone(window);
        let physical = window.inner_size();

        // The cell comes from the font, and the font's metrics need no GPU:
        // the grid the session is spawned with is therefore the real grid on
        // the first frame, not a guess corrected on the second. The settings
        // are the defaults here because the handle is attached after this
        // returns; a font the user chose instead arrives through
        // `apply_live_settings` on the first redraw.
        let cfg = Config::default();
        let entry = font_entry(&cfg);
        let scale_factor = window.scale_factor();
        let request = sizing_request(&cfg, scale_factor);
        let resolved = term::resolve(entry, &request, ScalePolicy::Floor);
        let cell = logical_cell(
            FontContext::new(entry).cell_metrics(&resolved),
            &resolved,
            scale_factor,
        );

        // A window stands in a cabinet, and the cabinet is what says how much
        // of it is glass. The profile is the defaults for the same reason the
        // font is: the settings handle is attached after this returns, and
        // `set_settings` re-measures.
        let window_size = (physical.width.max(1), physical.height.max(1));
        let cabinet = Cabinet::from_config(
            &cfg,
            f64::from(window_size.0) / scale_factor.max(f64::EPSILON),
            f64::from(window_size.1) / scale_factor.max(f64::EPSILON),
        );
        let bank = bank_physical(&cabinet, scale_factor, window_size.0);
        let mut viewport = Viewport::new(
            window_size.0.saturating_sub(bank).max(1),
            window_size.1,
            scale_factor,
            cell,
        );
        viewport.margin = settings::distortion_margin(&cfg) * scale_factor;

        let gpu = match Gpu::new(Arc::clone(&window), frame_stats_enabled) {
            Ok(g) => {
                log::info!("wgpu adapter {} on {}", g.adapter_name, g.backend);
                Some(g)
            }
            Err(e) => {
                log::error!("{e}");
                None
            }
        };
        let glass = gpu
            .as_ref()
            .and_then(|gpu| Glass::new(gpu, &cfg, &viewport, &crate::identity()));

        let mut surface = Self::assemble(
            Some(window),
            gpu,
            session,
            viewport,
            window_size,
            Some(cabinet),
        );
        surface.glass = glass;
        surface
    }

    /// A surface with no window and no GPU: a session, a geometry, and
    /// everything the pointer path touches.
    ///
    /// The pointer path reads the terminal and the grid arithmetic and
    /// nothing else: the window only ever tells it how big it is. So
    /// this is what lets a press-drag-release be an ordinary `cargo
    /// test`, the same way a scripted PTY transcript already is one crate
    /// down.
    ///
    /// It stands in no chassis: the viewport it is given is the whole of it,
    /// well and window alike. [`TerminalSurface::set_cabinet`] is how a test
    /// that wants one asks, and it is the only way a headless surface gets one.
    pub fn headless(session: &SessionConfig, viewport: Viewport) -> Self {
        let window_size = (viewport.width, viewport.height);
        Self::assemble(None, None, session, viewport, window_size, None)
    }

    fn assemble(
        window: Option<Arc<Window>>,
        gpu: Option<Gpu>,
        session: &SessionConfig,
        viewport: Viewport,
        window_size: (u32, u32),
        cabinet: Option<Cabinet>,
    ) -> Self {
        let columns = viewport.term_size().cols();
        // The set comes up on channel 1, and the tube is armed only after
        // it, so the first channel is not a channel change and nothing
        // flinches.
        let mut channels = Channels::new();
        let size = viewport.term_size();
        channels.start(|| spawn(session, size));
        let on_air = (channels.current_page(), channels.current_channel());
        Self {
            window,
            gpu,
            channels,
            gateways: HashMap::new(),
            on_air,
            session_config: session.clone(),
            pager: BankPager::new(),
            chord: ChordInput::new(),
            chord_modifier: false,
            degauss: Degauss::new(),
            viewport,
            window_size,
            cabinet,
            cabinet_cfg: Config::default(),
            seam_press: false,
            pending_led_characters: None,
            seam_cursor: false,
            shell_events: None,
            modes: KeyboardModes::default(),
            ime: ImeState::default(),
            eof: false,
            scroll: ScrollPosition::default(),
            selection: SelectionController::new(columns),
            pointer_cell: (0, 0),
            dragging: false,
            last_click: None,
            last_selection: None,
            wheel_pixels: 0.0,
            settings: None,
            base: Config::default(),
            next_effects_frame: None,
            next_output_frame: None,
            output_pending: false,
            glass: None,
            size_badge: (String::new(), 0.0),
            ime_area: None,
            notice: crate::overlay::Notice::default(),
            sheds_seen: (0, 0),
        }
    }

    /// Drain every channel's PTY once. The shell calls [`Surface::tick`] for
    /// this; a test with no event loop calls it directly.
    ///
    /// Every channel and not only the one on the air: every channel keeps
    /// running while only one of them is shown, so a shell that prints while
    /// another channel is up has printed by the time you turn the knob back.
    /// Answers how many bytes the *visible* channel produced, since that is
    /// the one a redraw would show.
    pub fn pump(&mut self) -> usize {
        let current = (
            self.channels.current_page(),
            self.channels.current_channel(),
        );
        let mut visible_bytes = 0;
        let mut died: Vec<(u32, u32, ChannelKind)> = Vec::new();
        for row in self.channels.rows_mut() {
            let pumped = row.session.pump();
            if (row.page, row.channel) == current {
                visible_bytes = pumped.bytes;
            }
            if pumped.eof {
                died.push((row.page, row.channel, row.kind));
            }
            // A PTY shell's title is its own, and rio-vt keeps whatever the
            // OSC set on the terminal itself.
            if row.kind == ChannelKind::Pty {
                let title = row.session.term().title.clone();
                if title != row.title {
                    row.title = title.trim().to_string();
                }
            }
        }
        // A channel whose session finished tells the model, and the model
        // decides whether that ends the appliance.
        for (page, channel, kind) in died {
            log::info!("channel {channel} on page {page} exited");
            if kind == ChannelKind::Gateway {
                // The gateway's transport died under it; `session_died` is
                // about to collapse the page (`gateway_died`), and a gateway
                // client with no channel under it has no wire.
                self.gateways.remove(&page);
            }
            if self.channels.session_died(page, channel) == Close::CloseWindow {
                self.eof = true;
            }
        }
        // A pane row's own `pump` above read nothing and could not: its bytes
        // arrive off the gateway's wire, which is drained here, after the loop
        // that counted. Counted only there, a tmux window on the air never
        // asked for a redraw at all -- what put its output on the glass was
        // the effects clock coming round, and a window whose effects are not
        // running would have shown nothing.
        visible_bytes += self.pump_gateways();
        self.channel_changed();
        self.watch_the_write_queues();
        visible_bytes
    }

    /// Put a shed write queue on the glass.
    ///
    /// Both write queues are capped (`term::session::INPUT_CAP`,
    /// `crate::tmux::PENDING_CAP`) so a peer that has stopped reading cannot
    /// grow this process until the OOM killer arrives. What a cap does when it
    /// bites is throw away what the user just typed, and until this the only
    /// trace of that was a `log::warn!` nobody is watching: the typing simply
    /// vanished. So the counters are read once per pump and a rise raises the
    /// badge.
    ///
    /// The badge is two words wide because the plate is twice the text and
    /// the glass is only so many columns across, and the log line beside it
    /// carries the byte counts for anyone who wants them.
    ///
    /// The two wires get two words, because the remedy differs: an unread tty
    /// is a shell this program spawned, and a full gateway queue is the tmux
    /// server. Both dropping at once is one badge -- the pty one, the wire
    /// nearer the user's hand -- and not a stack of two: they are one event,
    /// "your typing is being thrown away", and the log has the detail.
    fn watch_the_write_queues(&mut self) {
        let pty: u64 = self
            .channels
            .rows_mut()
            .map(|row| row.session.sheds())
            .sum();
        let tmux: u64 = self.gateways.values().map(|gateway| gateway.sheds()).sum();
        let seen = self.sheds_seen;
        self.sheds_seen = (pty, tmux);
        if pty > seen.0 {
            self.notice.raise(SHED_PTY, Instant::now());
        } else if tmux > seen.1 {
            self.notice.raise(SHED_TMUX, Instant::now());
        }
    }

    // ---- the tmux control-mode plumbing -----------------------------
    //
    // Everything here runs synchronously, on every pump. There is no
    // listener to attach late: by the time `attach` returns, the
    // model is already wired to the gateway whose bootstrap is in flight, so
    // there is no window in which a late attachment could race it.

    /// Detection, the gateways' turn, and the model transitions their events
    /// ask for.
    /// Answers what [`Self::pump_gateway`] counted, summed over the pages.
    fn pump_gateways(&mut self) -> usize {
        // Detection: a PTY channel's program entered control mode. The
        // channel transports to a new page and its PTY becomes the
        // attachment's wire.
        let mut detected: Vec<(PageId, u32)> = Vec::new();
        for row in self.channels.rows_mut() {
            if let Some(session) = row.session.pty_mut() {
                if session.tap_mut().take_detected() {
                    detected.push((row.page, row.channel));
                }
            }
        }
        for (page, channel) in detected {
            self.attach(page, channel);
        }

        let pages: Vec<PageId> = self.gateways.keys().copied().collect();
        let mut visible = 0;
        for page in pages {
            visible += self.pump_gateway(page);
        }
        visible
    }

    /// One detection: raise the page, dup the wire, start the gateway, and
    /// tell every attachment the glass's grid.
    fn attach(&mut self, page: PageId, channel: u32) {
        // The tmux server's hostname is the bootstrap's to resolve
        // (`tmux_host_changed`); the page opens under the empty name briefly,
        // until it does.
        let Some(page_id) = self.channels.attach(page, channel, "") else {
            log::warn!("control mode detected on a slot that cannot attach ({page},{channel})");
            return;
        };
        let writer = self
            .channels
            .rows_mut()
            .find(|r| r.page == page_id && r.channel == 1)
            .and_then(|r| r.session.pty_mut())
            .map(|s| s.control_mode_writer());
        match writer {
            Some(Ok(writer)) => {
                self.gateways.insert(page_id, Gateway::new(writer));
                log::info!("tmux: attached; page {page_id} raised over channel {channel}");
                // The client-size law: the glass's grid goes to *every*
                // gateway on attach, not to the new page alone (see the
                // module doc of `crate::channels`).
                self.set_client_size();
            }
            other => {
                log::error!("tmux: no wire for the attachment: {other:?}");
                self.channels.collapse_page(page_id);
            }
        }
    }

    /// One gateway's turn: drain the gateway channel's tap, advance, apply.
    /// Answers how many bytes reached the channel on the air, for the same
    /// reason [`Self::pump`] counts them: a pane row's own `pump` reads
    /// nothing (its bytes arrive here, off the gateway's wire), so counted only
    /// there a tmux window's output never asked for a redraw at all.
    fn pump_gateway(&mut self, page: PageId) -> usize {
        let current = (
            self.channels.current_page(),
            self.channels.current_channel(),
        );
        let mut visible = 0;
        let Some(mut gateway) = self.gateways.remove(&page) else {
            return visible;
        };
        // The peeled envelope body, and whether an `ST` closed it, off the
        // gateway channel's own tap.
        let drained = self
            .channels
            .rows_mut()
            .find(|r| r.page == page && r.kind == ChannelKind::Gateway)
            .and_then(|r| r.session.pty_mut())
            .map(|s| {
                let tap = s.tap_mut();
                (tap.take_body(), tap.take_ended())
            });
        let Some((bytes, ended)) = drained else {
            // No gateway channel, no wire: the page collapsed under this
            // gateway.
            return visible;
        };

        let mut events = gateway.advance(&bytes);
        // `ST` without a preceding `%exit`: the gateway program died
        // mid-protocol.
        if ended && gateway.attached() {
            events.extend(gateway.control_mode_ended());
        }
        // The pump's own clock: the queued wire goes out, a settled resize
        // with it, and the bootstrap watchdog reads the attachment's pulse --
        // which can end it, so what it says joins the events above.
        events.extend(gateway.poll(Instant::now()));

        // The write side of the keystroke diversion: what the pane sessions
        // queued becomes `send-keys`. Which keys get queued is settled before
        // they reach here, by `key_input` and `write`: a gateway channel's are
        // swallowed, a pane channel's land in its `TmuxPane`.
        let mut inputs: Vec<(PaneId, Vec<u8>)> = Vec::new();
        for row in self.channels.rows_mut().filter(|r| r.page == page) {
            if let Some(pane_session) = row.session.tmux_pane_mut() {
                let input = pane_session.take_input();
                if !input.is_empty() {
                    if let Some(pane) = PaneId::parse(&row.tmux_pane) {
                        inputs.push((pane, input));
                    }
                }
            }
        }
        for (pane, input) in inputs {
            gateway.send_keys(&pane, &input);
        }

        let mut collapse = None;
        for event in events {
            match event {
                GatewayEvent::HostChanged(host) => self.channels.tmux_host_changed(page, &host),
                GatewayEvent::WindowAdded { window, pane, name } => {
                    let size = self.viewport.term_size();
                    let scrollback = self.session_config.scrollback;
                    let opened = self.channels.open_tmux_pane(
                        page,
                        window.as_str(),
                        pane.as_str(),
                        &name,
                        || Some(ChannelSession::TmuxPane(TmuxPane::new(size, scrollback))),
                    );
                    if opened {
                        gateway.attach_window(&window, &pane);
                    } else {
                        // A window added with no free slot to take it stays
                        // channelless: tmux keeps it, but nothing here draws
                        // it.
                        log::warn!("tmux: no slot for window {window} on page {page}");
                    }
                }
                GatewayEvent::WindowRenamed { window, name } => {
                    let channel = self.channels.channel_of_window(page, window.as_str());
                    if channel > 0 {
                        self.channels.set_title(page, channel, &name);
                    }
                }
                GatewayEvent::WindowClosed { window } => {
                    self.channels.window_closed(page, window.as_str());
                }
                GatewayEvent::WindowPaneChanged { window, pane } => {
                    // The channel keeps its emulation and scrollback; only
                    // its routing moves. The gateway's fresh capture redraws
                    // the screen through the ordinary output path.
                    for row in self.channels.rows_mut() {
                        if row.page == page && row.tmux_window == window.as_str() {
                            row.tmux_pane = pane.as_str().to_string();
                        }
                    }
                }
                GatewayEvent::Output { pane, bytes } => {
                    let row = self
                        .channels
                        .rows_mut()
                        .find(|r| r.page == page && r.tmux_pane == pane.as_str());
                    if let Some(row) = row {
                        let on_air = (row.page, row.channel) == current;
                        if let Some(pane_session) = row.session.tmux_pane_mut() {
                            pane_session.feed(&bytes);
                            if on_air {
                                visible += bytes.len();
                            }
                        }
                    }
                }
                GatewayEvent::Detached { lost_protocol } => collapse = Some(lost_protocol),
            }
        }

        if let Some(lost_protocol) = collapse {
            self.collapse_page(page, lost_protocol);
        } else {
            self.gateways.insert(page, gateway);
        }
        visible
    }

    /// Detach or gateway death: the model collapses the page
    /// (`channels::Channels::collapse_page`), and a protocol lost without an
    /// `ST` also forces the gateway channel's parsers out of the envelope no
    /// one will ever close (`term::Session::leave_control_mode`).
    fn collapse_page(&mut self, page: PageId, lost_protocol: bool) {
        let gateway_home_slot = self
            .channels
            .pages()
            .iter()
            .find(|p| p.id == page)
            .map(|p| p.gateway_home_slot);
        self.channels.collapse_page(page);
        if lost_protocol {
            if let Some(slot) = gateway_home_slot {
                let row = self
                    .channels
                    .rows_mut()
                    .find(|r| r.page == 0 && r.channel == slot);
                if let Some(session) = row.and_then(|r| r.session.pty_mut()) {
                    session.leave_control_mode();
                }
            }
        }
        log::info!("tmux: page {page} collapsed (protocol lost: {lost_protocol})");
    }

    /// The client-size law: one client, one geometry, told to every
    /// attachment. A gateway told only while its page held the air would
    /// keep a stale size that tmux would draw *other* sessions at.
    fn set_client_size(&mut self) {
        let size = self.viewport.term_size();
        let columns = size.cols().min(u16::MAX as usize) as u16;
        let rows = size.rows().min(u16::MAX as usize) as u16;
        for gateway in self.gateways.values_mut() {
            gateway.set_client_size(columns, rows);
        }
    }

    /// What the user is looking at, one row per entry.
    ///
    /// The viewport and not the live screen: scrolled back, these differ,
    /// and this is the one a renderer (or a test asking whether the
    /// screen is ready yet) wants.
    pub fn viewport_text(&self) -> Vec<String> {
        match self.channels.session() {
            Some(session) => term::viewport_text(session.term()),
            None => Vec::new(),
        }
    }

    /// The text of the last completed selection, if there was one.
    pub fn last_selection(&self) -> Option<&str> {
        self.last_selection.as_deref()
    }

    /// Lines the view is scrolled back above the bottom of the history.
    pub fn scroll_offset(&self) -> usize {
        self.scroll.offset()
    }

    /// One key press, reduced to the three things anything downstream reads:
    /// the logical key, the text winit decoded for it, and the modifiers.
    ///
    /// `text` is the text the platform produced with **every** modifier
    /// applied, Control included: winit's `text_with_all_modifiers`.
    /// [`key_text`] is where that is picked, and why; a
    /// caller passing winit's plain `text` field instead would hand `"c"` to
    /// the child where the user pressed `Ctrl+C`.
    ///
    /// [`Surface::key_pressed`] is this with a `winit::event::KeyEvent` around
    /// it. The split is not decoration: `KeyEvent` carries a crate-private
    /// platform field, so no test outside winit can build one, and the wiring
    /// below -- which key reaches the pty and which one moves the viewport --
    /// would otherwise be reachable only from a running window.
    pub fn key_input(
        &mut self,
        logical: &winit::keyboard::Key,
        text: Option<&str>,
        modifiers: ModifiersState,
    ) {
        // The bank's own keys first: they are window-level shortcuts that
        // run before the emulation ever sees the event, so the keytab is not
        // the authority on them.
        if self.channel_key(logical, modifiers) {
            return;
        }
        // Then the gateway's own keyboard, which stands between the shortcuts
        // and the keytab: it holds the focus in the terminal's place, so the
        // emulation never sees a key at all, while the window's own
        // shortcuts (above) run before any focused item either way.
        if self.gateway_key(logical) {
            return;
        }
        let mods = modifiers_from(modifiers);
        // The keytab next: it is the authority on every key it binds.
        if let Some(action) = encode_winit_key(logical, mods, self.modes) {
            match action {
                KeyAction::Bytes(bytes) => self.write(&bytes),
                other => self.scroll_key(other),
            }
            return;
        }
        // Everything the keytab does not bind is ordinary text, which is
        // the path winit's own `text` field already decoded for us.
        if let Some(text) = text {
            if !text.is_empty() {
                let bytes = text.as_bytes().to_vec();
                self.write(&bytes);
            }
        }
    }

    /// One thing the input method said, applied.
    ///
    /// `key_input`'s counterpart, and public for the same reason: winit's
    /// `KeyEvent` cannot be built outside winit, but `Ime` can, so this is the
    /// seam a test drives without a display server. [`Surface::ime`] is one
    /// line calling it.
    ///
    /// **What is here.** `Ime::Commit` is the composed text, and it is written
    /// to the PTY as UTF-8, unchanged and unencoded: a commit is text the user
    /// finished choosing, not a keystroke, so it is carried as bytes and
    /// nothing else -- it is neither run through the keytab (there is no key
    /// to bind) nor escaped. In particular it is not bracketed: a program
    /// that reads a paste bracket where the user typed a word is worse off
    /// than one that reads the word.
    ///
    /// **What the pre-edit does.** It is held here and drawn at the cursor, in
    /// the grid, by `term::render::GridRenderer::set_preedit`, so the
    /// half-typed word appears where it is being typed, through the curvature
    /// and in the phosphor, and vanishes on the commit or the abandon. The frame
    /// reads this field on every redraw ([`Self::draw_frame`]) rather than being
    /// pushed at from here: the composition is state, not an event, and the
    /// cursor it stands at can move underneath it.
    ///
    /// Winit's inner cursor offsets (`ImeState::cursor`) are kept and not
    /// drawn: the whole composition is painted as one block, so the offset
    /// inside it is not consulted.
    ///
    /// The other half is [`Self::publish_ime_cursor`], which tells the
    /// platform where the caret is so the candidate window follows it.
    pub fn ime_input(&mut self, event: &Ime) {
        match event {
            Ime::Enabled => {
                self.ime = ImeState {
                    enabled: true,
                    ..ImeState::default()
                };
            }
            Ime::Preedit(text, cursor) => {
                self.ime.preedit.clear();
                self.ime.preedit.push_str(text);
                self.ime.cursor = *cursor;
            }
            Ime::Commit(text) => {
                // The composition is over whether or not a `Preedit("")`
                // follows, and every input method sends the two in a different
                // order. Clearing here means the state is never a stale word
                // the user already committed.
                self.ime.preedit.clear();
                self.ime.cursor = None;
                if !text.is_empty() {
                    let bytes = text.as_bytes().to_vec();
                    self.write(&bytes);
                }
            }
            Ime::Disabled => self.ime = ImeState::default(),
        }
    }

    /// What the input method is composing. [`Self::draw_frame`] hands it to the
    /// glyph renderer every redraw; see [`TerminalSurface::ime_input`].
    pub fn ime_state(&self) -> &ImeState {
        &self.ime
    }

    /// Where the caret is, in this window's own physical pixels: the
    /// cursor's cell, mapped from grid coordinates into the window's own.
    ///
    /// The window shows the bent picture directly rather than a flat
    /// rendered image, so the cell is pushed through
    /// `term::distortion::forward_distort` -- the same map the pointer path
    /// inverts on every click -- and the bank column's width is added,
    /// because the well starts where the casting stops. The caret the
    /// candidate window is told about is therefore the caret the user can
    /// see, which is what the rectangle is for.
    ///
    /// `None` when there is no channel, or when the cursor is not on the visible
    /// screen at all (scrolled back into history): there is no caret on the
    /// glass to point at, and a rectangle from the wrong row would move the
    /// candidate window somewhere the user is not looking.
    pub fn ime_cursor_area(&self) -> Option<(PhysicalPosition<f64>, PhysicalSize<f64>)> {
        let session = self.channels.session()?;
        let cursor = session.term().cursor();
        let row = cursor.pos.row.0;
        if row < 0 {
            return None;
        }
        let (row, col) = (row as usize, cursor.pos.col.0);
        let size = self.viewport.term_size();
        if row >= size.rows() || col >= size.cols() {
            return None;
        }
        let (cell_w, cell_h) = (f64::from(size.cell_width), f64::from(size.cell_height));
        // The cell's rectangle in grid-texture pixels -- `cell_at`'s output
        // space -- forward through the warp into well pixels.
        let params = self.distortion_params();
        let x = col as f64 * cell_w;
        let y = row as f64 * cell_h;
        let top_left = distortion::forward_distort(x, y, &params);
        let bottom_right = distortion::forward_distort(x + cell_w, y + cell_h, &params);
        let bank = f64::from(self.bank_physical());
        Some((
            PhysicalPosition::new(top_left.x + bank, top_left.y),
            PhysicalSize::new(
                (bottom_right.x - top_left.x).max(1.0),
                (bottom_right.y - top_left.y).max(1.0),
            ),
        ))
    }

    /// Tell the platform where the caret is, if it has moved since last time.
    ///
    /// Called once per turn of the loop rather than from [`Self::ime_input`],
    /// because the caret moves for reasons the input method never hears about:
    /// the shell echoing a character, a program repainting, a resize. Rounded to
    /// whole pixels before the comparison, since that is the resolution the
    /// question is asked at, and a caret that has not moved must not cost a
    /// round trip to the input method 120 times a second.
    ///
    /// Only while an input method has the keyboard (`Ime::Enabled` arrived and
    /// no `Ime::Disabled` since), because only then is anyone reading the
    /// answer, and deriving it costs a settings snapshot. The first composition
    /// after the method takes the keyboard is one loop turn away, which is
    /// under ten milliseconds and before any candidate window is up.
    fn publish_ime_cursor(&mut self) {
        if !self.ime.enabled {
            return;
        }
        let Some(window) = self.window.as_ref() else {
            return;
        };
        let Some((position, size)) = self.ime_cursor_area() else {
            return;
        };
        let area = (
            position.x.round() as i32,
            position.y.round() as i32,
            size.width.round() as i32,
            size.height.round() as i32,
        );
        if self.ime_area == Some(area) {
            return;
        }
        self.ime_area = Some(area);
        window.set_ime_cursor_area(position, size);
    }

    /// What the appliance is saying on its own behalf right now, if anything.
    ///
    /// The badge itself needs a device and a frame; this is the state under it,
    /// so a test with neither can read what the user would have seen.
    pub fn notice(&self) -> &crate::overlay::Notice {
        &self.notice
    }

    /// The keytab's non-byte actions: `scrollLineUp`, `scrollLineDown`,
    /// `scrollPageUp`, `scrollPageDown` and `scrollLock`, which the keytab
    /// binds to Shift+Up/Down/PageUp/PageDown and the Scroll Lock key
    /// (`app::input`'s table, from `default.keytab`).
    ///
    /// The four that move go to the same [`ScrollPosition`] the wheel moves,
    /// with the same sign: positive is up and into history. Routing them
    /// anywhere else would give the keyboard a second idea of where the view
    /// is, and rio-vt's display offset is the only authority there is.
    fn scroll_key(&mut self, action: KeyAction) {
        let Some(session) = self.channels.session_mut() else {
            return;
        };
        let term = session.term_mut();
        match action {
            KeyAction::ScrollLineUp => self.scroll.scroll(term, 1),
            KeyAction::ScrollLineDown => self.scroll.scroll(term, -1),
            KeyAction::ScrollPageUp => self.scroll.page_up(term),
            KeyAction::ScrollPageDown => self.scroll.page_down(term),
            // Not a movement, and deliberately not a hold either. Konsole
            // holds output on Scroll Lock; the choice here was between
            // porting that hold and leaving the binding inert, and it
            // settled on inert: the keytab names the action, and the action
            // does nothing. The keytab row stays, because the keytab is
            // transcribed as written and the key really is bound to an
            // action that really does nothing.
            //
            // The XON/XOFF hold is a different mechanism and already exists:
            // Ctrl+S/Ctrl+Q reach the pty's `IXON` through the tty
            // discipline, not through this table.
            KeyAction::ScrollLock => {
                log::debug!("scroll lock is bound to nothing");
            }
            // `key_input` has already taken this arm.
            KeyAction::Bytes(_) => {}
        }
    }

    // ---- the channel bank's keys -------------------------------------

    /// The window-level shortcuts, which run before the keytab. Answers
    /// whether the key was one of them.
    ///
    /// | key | handler |
    /// |---|---|
    /// | `Ctrl+Shift+T` | [`Self::new_channel`] |
    /// | `Ctrl+Shift+W` | [`Self::close_channel`] |
    /// | `Ctrl+PgUp/PgDown` | [`Self::cycle_channel`] |
    /// | `Alt+PgUp/PgDown` | [`Self::step_bank`] |
    /// | `Alt+<digit>` | [`Self::chord_digit`] (select) |
    /// | `Alt+Shift+<digit>` | [`Self::chord_digit`] (store) |
    ///
    /// `Ctrl+Shift+N`/`Q` are the *shell*'s (a window, not a channel) and
    /// never reach here; [`crate::shell`] takes them first.
    fn channel_key(&mut self, logical: &winit::keyboard::Key, modifiers: ModifiersState) -> bool {
        use winit::keyboard::{Key, NamedKey};

        let ctrl = modifiers.control_key();
        let shift = modifiers.shift_key();
        // The chord modifier is Alt everywhere but macOS, where it is Meta.
        let chord_mod = if cfg!(target_os = "macos") {
            modifiers.super_key()
        } else {
            modifiers.alt_key()
        };

        match logical {
            Key::Named(NamedKey::PageUp) if chord_mod => self.step_bank(-1),
            Key::Named(NamedKey::PageDown) if chord_mod => self.step_bank(1),
            Key::Named(NamedKey::PageUp) if ctrl && !shift => {
                self.cycle_channel(-1);
                true
            }
            Key::Named(NamedKey::PageDown) if ctrl && !shift => {
                self.cycle_channel(1);
                true
            }
            Key::Character(c) if chord_mod && is_digit(c) => {
                self.chord_digit(c.as_bytes()[0], shift)
            }
            Key::Character(c) if ctrl && shift && c.eq_ignore_ascii_case("t") => {
                self.new_channel();
                true
            }
            Key::Character(c) if ctrl && shift && c.eq_ignore_ascii_case("w") => {
                self.close_channel();
                true
            }
            _ => false,
        }
    }

    /// The gateway channel's keyboard, which is the whole of what an attached
    /// channel does with typed input.
    ///
    /// Every key is accepted and dropped, and the bare Enter is turned into
    /// the empty line tmux's control mode reads as "detach". The glass under
    /// it is still a picture to read and copy from; it is no longer a
    /// surface to type at, because that pty *is* the protocol's wire.
    ///
    /// **The one subtlety, and it is protocol hygiene.** Writing the `\r`
    /// onto the wire directly would only work if a stray reply could be
    /// discarded as unsolicited; this build's codec instead pairs replies by
    /// command id off its own send queue, so a line it did not send is a
    /// block it cannot attribute. The detach therefore goes out as
    /// [`Gateway::detach`]'s `detach-client`, which is the same ask, paired,
    /// and answered by the same `%exit` coming back up the same wire to
    /// collapse the page. Nothing reaches the gateway channel's pty except
    /// through the codec (`crate::tmux`).
    ///
    /// winit folds the two Enter keys (the main one and the keypad's) into
    /// one `NamedKey::Enter` told apart only by `KeyEvent::location`, so
    /// matching the named key covers both.
    ///
    /// Answers whether the key was the gateway channel's, which on a gateway
    /// channel is always.
    fn gateway_key(&mut self, logical: &winit::keyboard::Key) -> bool {
        use winit::keyboard::{Key, NamedKey};

        let Some(row) = self.channels.current() else {
            return false;
        };
        if row.kind != ChannelKind::Gateway {
            return false;
        }
        let page = row.page;
        if matches!(logical, Key::Named(NamedKey::Enter)) {
            match self.gateways.get_mut(&page) {
                Some(gateway) => gateway.detach(),
                // The row is a gateway channel with no client standing only
                // between a teardown and the collapse that follows it; the page
                // is going home already.
                None => log::debug!("the gateway's Enter found no client on page {page}"),
            }
        }
        true
    }

    /// `Ctrl+Shift+T`. A new channel goes to the page on view: on home the
    /// lowest free slot with a shell in it, on an attachment another window of
    /// that session, which is its page's gateway's to give.
    pub fn new_channel(&mut self) {
        let (config, size) = (self.session_config.clone(), self.viewport.term_size());
        if let Some(page) = self.channels.new_channel(|| spawn(&config, size)) {
            // The model set the page's `new_window_pending` flag; the window
            // tmux answers with will take the air when it lands
            // (`open_tmux_pane`).
            match self.gateways.get_mut(&page) {
                Some(gateway) => gateway.new_window(),
                None => log::warn!("page {page} asked for a window with no gateway standing"),
            }
        }
        self.channel_changed();
    }

    /// `Ctrl+Shift+W`.
    pub fn close_channel(&mut self) {
        let (page, channel) = (
            self.channels.current_page(),
            self.channels.current_channel(),
        );
        match self.channels.close_channel(page, channel) {
            // The last channel anywhere switches the appliance off, which
            // for this surface is the same end its child's exit has.
            Close::CloseWindow => self.eof = true,
            // A gateway channel detaches its page: tmux keeps the session, the
            // channel comes home when `%exit` echoes back through the pump.
            Close::Detach(page) => {
                if let Some(gateway) = self.gateways.get_mut(&page) {
                    gateway.detach();
                }
            }
            // A tmux window is tmux's to kill; its row goes when the close
            // notification lands, not here.
            Close::KillWindow { page, window_id } => {
                if let (Some(gateway), Some(window)) =
                    (self.gateways.get_mut(&page), WindowId::parse(&window_id))
                {
                    gateway.kill_window(&window);
                }
            }
            Close::Removed | Close::Nothing => {}
        }
        self.channel_changed();
    }

    /// `Ctrl+PgUp` / `Ctrl+PgDown`.
    pub fn cycle_channel(&mut self, direction: i32) {
        self.channels.cycle_open(direction);
        self.channel_changed();
    }

    /// Whether this window has a bank at all. The chord input and the two
    /// pager shortcuts belong to the bank, and the bank does not show at all
    /// when the chassis is hidden: no bank, no numerals, no chord. The two
    /// window shortcuts (`Ctrl+Shift+T`/`W`) and `Ctrl+PgUp`/`PgDown` are the
    /// window's own and stand either way.
    fn has_bank(&self) -> bool {
        self.cabinet.as_ref().is_some_and(|c| c.is_shown())
    }

    /// `Alt+PgUp` / `Alt+PgDown`: the pager, which views a page without
    /// stealing the air. Answers whether there was a bank to step.
    pub fn step_bank(&mut self, direction: i32) -> bool {
        if !self.has_bank() {
            return false;
        }
        self.pager.step(direction, &self.channels);
        self.settle_bank();
        true
    }

    /// One digit of a chord, and whatever it commits. Answers whether there was
    /// a bank to type it against; a window with none leaves the key to the
    /// keytab.
    pub fn chord_digit(&mut self, digit: u8, store: bool) -> bool {
        if !self.has_bank() {
            return false;
        }
        self.chord_modifier = true;
        let (pager, channels) = (&self.pager, &self.channels);
        let committed = self
            .chord
            .feed_digit(digit, store, Instant::now(), |buf, store| {
                pager.slot_prefix_exists(channels, buf, store)
            });
        self.apply_chord(committed);
        true
    }

    /// The chord modifier came up, or the window went away under it: either
    /// way the chord commits.
    fn commit_chord(&mut self) {
        let committed = self.chord.commit();
        self.apply_chord(committed);
    }

    /// The chord names a key on the page the bank is showing, as the
    /// numerals engraved beside those keys read; the bank turns it into a
    /// slot.
    fn apply_chord(&mut self, committed: Option<Chord>) {
        let Some(chord) = committed else { return };
        let page = self.pager.view(&self.channels).page;
        let slot = self.pager.absolute_slot(&self.channels, chord.slot());
        match chord {
            Chord::Select(_) => {
                self.channels.select_channel(page, slot);
            }
            Chord::Store(_) => {
                self.channels.move_current_to(page, slot);
            }
        }
        self.channel_changed();
    }

    /// Everything that follows the current pair moving, in order: the tube
    /// flinches, the bank turns to the channel on the air, and a stretch of
    /// numerals that moved under the chord abandons its digits.
    fn channel_changed(&mut self) {
        if self.channels.take_degauss() {
            self.degauss.trigger(Instant::now());
        }
        let on_air = (
            self.channels.current_page(),
            self.channels.current_channel(),
        );
        if self.on_air != on_air {
            self.on_air = on_air;
            // A mark is a region of one grid and means nothing on another, the
            // same reason a resize clears it (`sync_geometry`).
            self.selection.selection.clear();
            // The view offset's authority is rio-vt's own `display_offset`,
            // which is the channel's, not the window's: `ScrollPosition` is a
            // mirror of it, so the channel coming to the screen brings its own
            // place in its own scrollback and this re-reads it at once rather
            // than a tick later.
            if let Some(session) = self.channels.session() {
                self.scroll.sync(session.term());
            }
            // This must run only when the air moves and at no other time:
            // `channel_changed` is also called from the pump, every 8ms, and
            // `ensure_visible` recomputes `page_index` from the channel on the
            // air with no memory of a manual step. Without this guard it put
            // the bank back on the air's own page within a frame of the user
            // paging away from it, which is Alt+PageUp doing nothing and a
            // chord spanning two pages never surviving to be committed.
            self.pager.ensure_visible(&self.channels);
        }
        // Outside the guard, deliberately: this is not a switch's work. The
        // page the bank *shows* is the page `Ctrl+Shift+T` acts on however it
        // came to be showing it, and `BankPager::refresh` already answers true
        // only when the view really moved, so a pump that changed nothing
        // cancels no chord.
        self.settle_bank();
    }

    /// The half of the above that a pager step also needs: the page the bank
    /// is showing is the page `Ctrl+Shift+T` acts on, and a moved stretch
    /// cancels the chord.
    fn settle_bank(&mut self) {
        let page = self.pager.view(&self.channels).page;
        self.channels.set_page_on_view(page);
        if self.pager.refresh(&self.channels) {
            self.chord.cancel();
        }
    }

    /// What the bank's furniture draws: one strip per engraved key of the page
    /// on view -- the reason
    /// [`chassis::strip`] is a type and not a private struct.
    /// Every frame draws this, so it reads the one setting it needs rather
    /// than taking a snapshot to get at it: `handle.current()` clones the
    /// whole `Config` under the settings mutex, and `redraw` has already
    /// cloned that same snapshot one call earlier in `apply_live_settings`.
    ///
    /// The no-handle arm reads `base` and not `Config::default()`, for the
    /// same reason `live_config` does: under `--default-settings --profile`
    /// the resolved profile is in `base` and nowhere else, and reading a
    /// default here showed the bank an indicator the profile had not asked
    /// for. It also built a whole `Config` per frame to do it.
    pub fn bank_strips(&self) -> chassis::BankStrips {
        let indicator = match self.settings.as_ref() {
            Some(handle) => handle.with(chassis::channel_indicator),
            None => chassis::channel_indicator(&self.base),
        };
        self.pager.strips(&self.channels, indicator)
    }

    /// A press on one of the bank's strips. The `channel` is
    /// [`chassis::StripRow::channel`], the absolute slot behind
    /// the engraved numeral, so the caller never has to know where the pager
    /// stands. A dark slot starts a session on it and an open one comes to the
    /// screen; either way the shell gets the keyboard back, which here it never
    /// gave up.
    ///
    /// The hit test is the furniture's: the window *is* the key, and
    /// only whoever drew it knows where it is.
    pub fn press_strip(&mut self, channel: u32) {
        let (config, size) = (self.session_config.clone(), self.viewport.term_size());
        let pager = self.pager.clone();
        pager.press(&mut self.channels, channel, || spawn(&config, size));
        self.channel_changed();
    }

    /// The channel state itself, for a test or for whoever mounts the bank.
    pub fn channels(&self) -> &Channels<AppSession> {
        &self.channels
    }

    /// What the degauss transient is doing at `now`. The frame samples this
    /// itself; this is how a surface with no GPU can be asked.
    pub fn degauss_state(&mut self, now: Instant) -> crt::DegaussState {
        self.degauss.sample(now)
    }

    /// The settings this frame is drawn from.
    ///
    /// The handle if there is one, because a watched file outranks anything
    /// resolved once at startup; otherwise the config the run was launched
    /// with ([`TerminalSurface::set_config`]), which under
    /// `--default-settings --profile <name>` is the named look.
    fn live_config(&self) -> Config {
        match self.settings.as_ref() {
            Some(handle) => handle.current(),
            None => self.base.clone(),
        }
    }

    /// The config this run resolved before any window existed: the CLI's
    /// `--profile` overlay on whatever the flags left standing.
    ///
    /// Only consulted when no settings handle is attached. With one, the
    /// watched file is the authority and this is never read, so `main` can
    /// set it either way without having to know which it built.
    pub fn set_config(&mut self, config: Config) {
        self.base = config;
    }

    /// Attach the live settings handle the pointer's inverse-distortion
    /// transform reads on every press/release/move.
    /// `SettingsHandle` is a thin, cheaply-`Arc`-cloned wrapper over
    /// `config::watch::ConfigWatcher`, so `handle.current()` always
    /// answers with whatever the watcher's last successful reload
    /// published -- a settings edit therefore changes the pointer mapping
    /// without restarting the process, with no polling or callback wiring
    /// needed here.
    pub fn set_settings(&mut self, settings: Arc<SettingsHandle>) {
        let cfg = settings.current();
        self.settings = Some(settings);
        // The cabinet was measured for the defaults in `new`, because the
        // handle did not exist yet. Take the real profile now rather than on
        // the first redraw, so the first frame is already the right shape.
        self.apply_cabinet_settings(&cfg);
    }

    /// The programmatic accessor: the live-preview throughput
    /// instrument's rolling p50/p99s, or `None` on a surface with no GPU
    /// (headless, or one whose `Gpu::new` failed). Reads `--frame-stats`'s
    /// same numbers the periodic log line prints, without scraping a log.
    pub fn frame_stats(&self) -> Option<crate::frame_stats::Stats> {
        self.gpu.as_ref().map(Gpu::frame_stats)
    }

    /// How this surface reaches its shell.
    ///
    /// One event travels this way today: the bank's width, which is the
    /// window's minimum-width hint and therefore the shell's to apply. A
    /// surface with no proxy (every headless one) still drags its seam; the
    /// hint is simply not a thing it has.
    pub fn set_shell_events(&mut self, proxy: EventLoopProxy<ShellEvent>) {
        self.shell_events = Some(proxy);
    }

    /// Stand this surface in a cabinet.
    ///
    /// [`TerminalSurface::new`] builds one from the profile; this is for a
    /// headless surface, which otherwise has none at all. Either way the well
    /// is re-divided on the spot and the shell is told the new bank width.
    pub fn set_cabinet(&mut self, cabinet: Cabinet) {
        self.cabinet = Some(cabinet);
        self.relayout();
        self.announce_bank_width();
    }

    /// The cabinet, for a test measuring what a drag landed.
    pub fn cabinet(&self) -> Option<&Cabinet> {
        self.cabinet.as_ref()
    }

    // ---- the window's division ---------------------------------------

    /// The bank column's width in physical pixels: zero with no cabinet, and
    /// never so wide that the well has no pixels left (a window manager is
    /// free to ignore the minimum-size hint).
    fn bank_physical(&self) -> u32 {
        match self.cabinet.as_ref() {
            Some(cabinet) => bank_physical(cabinet, self.viewport.scale_factor, self.window_size.0),
            None => 0,
        }
    }

    /// Re-divide the window into the bank column and the screen well, and
    /// carry the well's new size through to the session, the surface and the
    /// chain's target.
    fn relayout(&mut self) {
        let scale = self.viewport.scale_factor.max(f64::EPSILON);
        let (window_w, window_h) = self.window_size;
        if let Some(cabinet) = self.cabinet.as_mut() {
            cabinet.resized(f64::from(window_w) / scale, f64::from(window_h) / scale);
        }
        let bank = self.bank_physical();
        self.viewport.width = window_w.saturating_sub(bank).max(1);
        self.viewport.height = window_h.max(1);
        // The margin is physical (see `Viewport::margin`), so a DPR change
        // moves it even with the settings themselves untouched -- the same
        // reason `relayout` is what `scale_factor_changed` calls.
        let cfg = self.live_config();
        self.ensure_margin(&cfg);
        self.sync_geometry();
        self.settle_rows();
    }

    /// How many rows the bank shows, measured off the window it now stands
    /// in.
    ///
    /// This runs on every resize with no debounce: it is arithmetic over a
    /// `Vec`, and running it on the spot costs nothing. The pager's
    /// footprint comes off the foot first (`Cabinet::pager_height`); the
    /// height it divides is the window's, in logical pixels, because that is
    /// what the bank column actually stands in.
    fn settle_rows(&mut self) {
        let Some(cabinet) = self.cabinet.as_ref().filter(|c| c.is_shown()) else {
            return;
        };
        let geometry = *cabinet.geometry();
        let pager_height = cabinet.pager_height();
        let height = (f64::from(self.window_size.1) / self.viewport.scale_factor.max(f64::EPSILON))
            .round() as i32;
        self.pager
            .settle(&geometry, height, pager_height, &self.channels);
        self.settle_bank();
    }

    /// Tell the shell what the bank now measures, so the window's
    /// minimum-width hint follows the seam.
    fn announce_bank_width(&self) {
        let (Some(proxy), Some(cabinet)) = (self.shell_events.as_ref(), self.cabinet.as_ref())
        else {
            return;
        };
        if proxy
            .send_event(ShellEvent::SetBankWidth(cabinet.bank_width()))
            .is_err()
        {
            log::debug!("the shell is gone; the bank width has nowhere to go");
        }
    }

    /// Take a profile the cabinet has not seen. Re-measuring reads the lamp
    /// font, so it happens on a change and not on a frame.
    fn apply_cabinet_settings(&mut self, cfg: &Config) {
        if self.cabinet.is_none() || *cfg == self.cabinet_cfg {
            return;
        }
        self.cabinet_cfg = cfg.clone();
        if let Some(cabinet) = self.cabinet.as_mut() {
            cabinet.apply_config(cfg);
        }
        self.relayout();
        self.announce_bank_width();
    }

    // ---- the redraw path --------------------------------------------

    /// Take whatever the settings say now, before anything is recorded.
    ///
    /// Two things can move, and they are not the same thing:
    ///
    /// * the *font*, which resizes the cell and therefore the grid, so the
    ///   atlas is rebuilt and the session resized;
    /// * everything else, which the chain takes itself, deciding between a
    ///   uniform push and a rebuild by comparing the `Structure` it derives
    ///   from each `Config` (`crt::chain`). `app::settings::classify` names
    ///   the same split but is informational only.
    ///
    /// This is a poll of the handle rather than the settings *callback* an
    /// earlier sketch named, and deliberately: the callback runs on the
    /// watcher's own thread, and the chain, the device and the atlas are the
    /// event loop's. `SettingsHandle::current()` is the published snapshot,
    /// so polling it once per frame sees every edit exactly as the callback
    /// would, on the thread that is allowed to act on it.
    fn apply_live_settings(&mut self) {
        let cfg = self.live_config();

        // The cabinet first: it decides how much of the window is glass, and
        // the font sizing below is measured against the glass.
        self.apply_cabinet_settings(&cfg);

        let refonted = self.ensure_font(&cfg);
        let remargined = self.ensure_margin(&cfg);
        if refonted || remargined {
            self.sync_geometry();
        }

        let well = (self.viewport.width.max(1), self.viewport.height.max(1));
        if let (Some(gpu), Some(glass)) = (self.gpu.as_ref(), self.glass.as_mut()) {
            if cfg != glass.applied {
                match glass.chain.apply_settings(&gpu.device, &gpu.queue, &cfg) {
                    Ok(applied) => log::debug!("settings reached the chain: {applied:?}"),
                    Err(e) => log::error!("could not apply settings to the chain: {e}"),
                }
                glass.applied = cfg;
            }

            // The two discontinuities the settings cannot express, which
            // `crt-burnin`'s mount contract leaves to the application: a
            // resized target and a rebuilt atlas both mean the accumulator's
            // contents describe a picture that is no longer on screen, and a
            // ghost of the old one would otherwise decay in place over it.
            //
            // The target is the *well*, so a seam drag resizes it too: the
            // glass really is narrower afterwards, and a target still sized to
            // the old well would stretch the picture into the new one.
            let (width, height) = well;
            if (glass.target.width, glass.target.height) != (width, height) {
                log::debug!(
                    "target {}x{} -> {width}x{height}",
                    glass.target.width,
                    glass.target.height
                );
                glass.target = Target::new(&gpu.device, width, height);
                glass.chain.burn_in().restart();
            }
            if refonted {
                glass.chain.burn_in().restart();
            }
        }
    }

    /// Recompute the well's grid inset from the live settings and this
    /// window's DPR, and say whether it moved.
    ///
    /// `settings::distortion_margin` -- the same derivation
    /// [`TerminalSurface::distortion_params`] already reads for the pointer
    /// path, so there is exactly one copy of the formula -- is logical;
    /// [`Viewport::margin`] is physical, so it is scaled by
    /// `scale_factor` here, the same boundary [`Viewport::physical_cell`]
    /// crosses for the cell.
    fn ensure_margin(&mut self, cfg: &Config) -> bool {
        let margin = settings::distortion_margin(cfg) * self.viewport.scale_factor;
        if (margin - self.viewport.margin).abs() > f64::EPSILON {
            self.viewport.margin = margin;
            true
        } else {
            false
        }
    }

    /// Rebuild the atlas if the font the settings ask for is not the font the
    /// atlas holds. Returns whether anything moved.
    ///
    /// The comparison is on the *resolved* font and not on the setting: two
    /// different `font_scaling` values that floor to the same integer scale
    /// rasterise the same atlas, and rebuilding it for them would throw the
    /// burn-in ghost away for no visible reason.
    fn ensure_font(&mut self, cfg: &Config) -> bool {
        let entry = font_entry(cfg);
        let request = sizing_request(cfg, self.viewport.scale_factor);
        let resolved = term::resolve(entry, &request, ScalePolicy::Floor);

        match self.glass.as_ref() {
            Some(glass) if glass.font_name == entry.name && glass.resolved == resolved => {
                return false
            }
            // No glass means no atlas to rebuild and no cell to move: a
            // headless surface takes its viewport from whoever built it.
            None => return false,
            Some(_) => {}
        }

        self.viewport.cell = logical_cell(
            FontContext::new(entry).cell_metrics(&resolved),
            &resolved,
            self.viewport.scale_factor,
        );
        if let (Some(gpu), Some(glass)) = (self.gpu.as_ref(), self.glass.as_mut()) {
            let (_, _, atlas) =
                term::build_font(&gpu.device, &gpu.queue, entry, &request, ScalePolicy::Floor);
            glass.renderer.set_scale(resolved.integer_scale);
            glass.renderer.set_atlas(&gpu.device, &gpu.queue, atlas);
            glass.font_name = entry.name.to_string();
            glass.resolved = resolved;
            log::info!(
                "font changed to {} at {}px x{}",
                glass.font_name,
                glass.resolved.raster_pixel_size,
                glass.resolved.integer_scale
            );
        }
        true
    }

    /// Record and present one frame.
    fn draw_frame(&mut self) {
        // The window's division, taken before the glass is borrowed: the well
        // is where the chain's picture goes and the column is what the casting
        // is drawn into, and a hidden chassis makes the second of them empty.
        let bank = self.bank_physical();
        let shown = self.cabinet.as_ref().filter(|cabinet| cabinet.is_shown());
        let column_params = shown.map(|cabinet| cabinet.chassis_params());
        // What stands on the casting: the real page of the real channel model,
        // exactly what `bank_strips` hands the keyboard and the hit test.
        let strips = self.bank_strips();
        let column_pieces = shown
            .map(|cabinet| cabinet.furniture(&strips))
            .unwrap_or_default();

        let Some(gpu) = self.gpu.as_mut() else { return };
        let Some(mut frame) = gpu.acquire() else {
            return;
        };

        let (glass, session) = match (self.glass.as_mut(), self.channels.session_mut()) {
            (Some(glass), Some(session)) => (glass, session),
            // A window with no chain or no child still has to put something on
            // the screen, or the compositor shows whatever was there before.
            _ => {
                // No glass means no marks reach the query set, so timing
                // this frame is withdrawn rather than resolved half-written.
                frame.discard_timing();
                frame.clear();
                gpu.present(frame);
                return;
            }
        };

        let scale_factor = self.viewport.scale_factor;

        // One instant for the whole frame: the chain's clock and the degauss
        // transient are two readings of the same moment.
        let now = Instant::now();
        let time = glass.pacing.tick(now);
        let degauss = self.degauss.sample(now);

        glass.renderer.sync(
            &gpu.device,
            &gpu.queue,
            session.term_mut(),
            &mut self.scroll,
        );
        // What the input method is composing goes into the grid, at the
        // cursor, before the grid is drawn, so it runs through the CRT chain
        // with the rest of the tube rather than floating flat over it
        // (`term::render::GridRenderer::set_preedit`).
        glass.renderer.set_preedit(&gpu.queue, &self.ime.preedit);

        // The grid is centred in the target at a whole-pixel origin: the
        // remainder of dividing the window by the cell is padding, and half a
        // pixel of it would put every glyph between two of them.
        let (target_width, target_height) = (glass.target.width, glass.target.height);
        let (grid_width, grid_height) = glass.renderer.pixel_size();
        let geometry = (target_width, target_height, grid_width, grid_height);
        if glass.logged_geometry != Some(geometry) {
            glass.logged_geometry = Some(geometry);
            log::debug!(
                "frame {}: {}x{} cells, grid {grid_width}x{grid_height} centred in \
                 target {target_width}x{target_height}",
                time.index,
                glass.renderer.cols(),
                glass.renderer.rows(),
            );
        }
        glass.renderer.set_origin(
            (target_width as i32 - grid_width as i32).max(0) / 2,
            (target_height as i32 - grid_height as i32).max(0) / 2,
        );

        let mut params =
            Params::build(&glass.applied, &glass.geometry(scale_factor), time, degauss);
        // Every shader that reads `time` sees a value held for
        // `effectsFrameSkip` frames and then jumped, not a continuous clock,
        // so the effects run at 60/skip Hz -- 20 Hz at the shipped skip of 3
        // -- and not once per frame drawn. `EFFECTS_BASE_FRAME` is the same
        // 60 Hz assumption the redraw scheduler already makes, and its doc
        // says what to do if a real display session disagrees.
        //
        // Only the `Time` uniform. `FrameTime::now` stays continuous, because
        // the burn-in decay is measured in real seconds between real frames,
        // and quantising that would fade the ghost in steps.
        let step = EFFECTS_BASE_FRAME.as_secs_f32()
            * glass.applied.general.effects_frame_skip.max(1) as f32;
        params.set("Time", (time.elapsed / step).floor() * step);
        glass.chain.set_params(&params);

        // One encoder, three recordings: the grid into the offscreen target,
        // the chain from that target into the well's rectangle of the swapchain
        // image, and the bank column's casting over the rest. The clear
        // is opaque black because the bloom pass reads the alpha of what it
        // blurs as its own mask, and a transparent surround would mask the
        // glow away at the edges.
        //
        // The frame-stats timing marks straddle each recording plus the
        // frame as a whole; `Frame::mark` is a no-op when this frame is not being
        // GPU-timed, so every call site here is unconditional. The column's
        // marks bracket its `if` rather than sitting inside it, so the query
        // set is always eight-for-eight written even with the chassis
        // hidden -- a hidden-column frame legitimately measures ~0 there.
        frame.mark(Mark::FrameStart);
        frame.mark(Mark::GridStart);
        glass.renderer.draw(
            &gpu.queue,
            &mut frame.encoder,
            &glass.target.view,
            target_width,
            target_height,
            wgpu::LoadOp::Clear(wgpu::Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            }),
        );
        frame.mark(Mark::GridEnd);
        frame.mark(Mark::ChainStart);
        if let Err(e) = glass.chain.frame_at(
            &glass.target.texture,
            &frame.view,
            (bank, 0),
            (target_width, target_height),
            gpu.format(),
            &mut frame.encoder,
            time,
        ) {
            log::error!("could not record a chain frame: {e}");
        }
        frame.mark(Mark::ChainEnd);
        // The casting is chrome, so it goes on afterwards rather than
        // through. It also has to: the chain's last pass cleared this whole
        // image before drawing its rectangle of it, so what stands to the left
        // of the well right now is transparent black.
        frame.mark(Mark::ColumnStart);
        if let (Some(column), Some(params)) = (glass.column.as_mut(), column_params) {
            column.render(
                &gpu.device,
                &gpu.queue,
                &mut frame.encoder,
                &frame.view,
                (bank, target_height),
                (target_width, target_height),
                scale_factor,
                &params,
                &column_pieces,
                time.index,
            );
        }
        frame.mark(Mark::ColumnEnd);
        // Last of all, the size badge -- over the glass and over the casting
        // both, drawn topmost. It is centred in the *well*, so the rectangle
        // it is given starts where the column stops. `opacity` gates it: the
        // shell's overlay reports zero whenever the badge is not up, and the
        // mount draws nothing rather than a transparent quad.
        //
        // `showTerminalSize` is gated here and not at the state machine,
        // because it is live-reloadable and this is the side of the seam that
        // holds live settings: the shell reads the setting once, when it builds
        // a window, while `glass.applied` is whatever the last config edit
        // said. This stops the drawing itself rather than the timer that
        // drives it.
        //
        // Below it, in the stack's second slot, whatever the appliance itself
        // has to say this moment (`crate::overlay::Notice`): a write queue
        // that shed, a look that was saved. This is not gated by
        // `showTerminalSize` either -- the setting is about the resize
        // badge, and hiding a loss behind a cosmetic switch would be a
        // second way to lose the news.
        let badge_opacity = if glass.applied.general.show_terminal_size {
            self.size_badge.1
        } else {
            0.0
        };
        let entries = [
            crate::badge::Entry {
                text: &self.size_badge.0,
                opacity: badge_opacity,
            },
            crate::badge::Entry {
                text: self.notice.text(),
                opacity: self.notice.opacity_at(now),
            },
        ];
        glass.badge.draw(
            &gpu.device,
            &gpu.queue,
            &mut frame.encoder,
            &frame.view,
            self.window_size,
            (bank as i32, 0, target_width, target_height),
            glass.renderer.atlas(),
            glass.resolved.integer_scale,
            scale_factor,
            &entries,
        );
        // The badge is inside `FrameEnd` and outside `ColumnEnd`: it is real
        // work the frame pays for, and it is not the column's.
        frame.mark(Mark::FrameEnd);
        gpu.present(frame);
    }

    fn sync_geometry(&mut self) {
        let size = self.viewport.term_size();
        // A selection is a linear index over a grid of a given width, so
        // it means something else at a new one. Konsole cleared it on
        // resize for exactly that reason, and `set_columns` does.
        if self.selection.selection.columns() != size.cols() {
            self.selection.selection.set_columns(size.cols());
        }
        // Every channel, not only the one on the air. There is one rectangle
        // of glass in the appliance and every channel is laid into it, so a
        // channel resized only when it came back to the screen would redraw
        // its whole screen at the moment it was looked at -- resizing every
        // channel up front avoids exactly that flicker.
        for session in self.channels.sessions_mut() {
            if let Err(e) = session.resize(size) {
                log::error!("could not resize the pty: {e}");
            }
        }
        // The client-size law's resize half: the glass's grid to every
        // gateway, debounced by the gateway itself against the drag's burst.
        self.set_client_size();
        if let Some(gpu) = self.gpu.as_mut() {
            // The swapchain is the whole window; only the chain's picture is
            // the well.
            gpu.resize(self.window_size.0, self.window_size.1);
        }
    }

    /// Send bytes to the channel on the air. Public because a paste, a mouse
    /// report and a test's scripted input all mean the same thing to the child.
    pub fn write(&mut self, bytes: &[u8]) {
        // The gateway swallows every byte, and that is deliberate rather than
        // a gap in this path. On frozen glass every byte-producing path is
        // suppressed one by one: the middle button's paste is a paste like
        // any other and inert on a gateway, and the pointer is handed a
        // Shift it did not press so a drag marks the screen instead of
        // reporting to a program -- nothing the mouse does is written to the
        // control-mode wire. What is left is the keyboard, and the keyboard
        // never reaches here: [`Self::gateway_key`] holds it, and the one key
        // with a meaning goes out as `detach-client`.
        //
        // So this is the same swallow, for the same reason. Its teeth are
        // protocol hygiene: the gateway channel's pty is the control wire,
        // where tmux reads every line as a command and answers it with a block
        // the codec never asked for: one stray byte desyncs the pairing queue
        // for good.
        if self
            .channels
            .current()
            .is_some_and(|row| row.kind == ChannelKind::Gateway)
        {
            return;
        }
        if let Some(session) = self.channels.session_mut() {
            if let Err(e) = session.write(bytes) {
                log::error!("could not write to the pty: {e}");
            }
        }
    }

    // ---- the pointer path -------------------------------------------

    /// The captured state a distortion computation needs, as this window
    /// supplies it: the window's own pixel size plus whatever
    /// [`TerminalSurface::set_settings`] last attached. No handle (any
    /// pointer test written before this settings handle existed, and any
    /// headless surface that never opts in) means every derived term is the
    /// neutral/identity value: zero margin, zero frame inset, zero
    /// curvature -- the map is window pixels onto grid-texture pixels,
    /// exactly this method's original behavior.
    fn distortion_params(&self) -> DistortionParams {
        let (grid_width, grid_height) = self.viewport.term_size().pixel_size();
        let width = self.viewport.width as f64;
        let height = self.viewport.height as f64;
        let normalized_screen_scale = distortion::normalized_screen_scale(width, height);

        let (margin, frame_size, screen_curvature) = match self.settings.as_ref() {
            Some(handle) => {
                let config = handle.current();
                (
                    // Physical, like every other field of `DistortionParams`:
                    // `width` and `height` just above are the viewport's own
                    // physical pixels, and `correct_distortion` subtracts the
                    // margin from a physical pointer position. The setting is
                    // logical, so it is scaled here the way `Viewport::margin`
                    // is scaled at its own boundary (`ensure_margin`, and
                    // `new`). Unscaled, a HiDPI pointer was measured against
                    // half the inset the glass was drawn with, and the cell it
                    // landed on drifted from the cell under the cursor.
                    settings::distortion_margin(&config) * self.viewport.scale_factor,
                    settings::distortion_frame_size(&config) * normalized_screen_scale,
                    config.screen.screen_curvature,
                )
            }
            None => (0.0, 0.0, 0.0),
        };

        DistortionParams {
            margin,
            width,
            height,
            frame_size,
            screen_curvature,
            screen_curvature_size: distortion::SCREEN_CURVATURE_SIZE,
            normalized_screen_scale,
            total_width: f64::from(grid_width),
            total_height: f64::from(grid_height),
        }
    }

    /// A window pixel becomes an absolute grid cell: measured from the well's
    /// left edge, through the inverse distortion, divided by the cell, offset
    /// by where the view sits.
    ///
    /// The bank column is subtracted first because every term after it is the
    /// glass's own: the distortion is the curvature of a tube that starts where
    /// the casting ends, and a click 200 px into a window with a 247 px bank is
    /// not on the glass at all.
    fn cell_at(&self, position: PhysicalPosition<f64>) -> (usize, usize) {
        let x = position.x - f64::from(self.bank_physical());
        let point = correct_distortion(x, position.y, &self.distortion_params());
        let size = self.viewport.term_size();
        let column = (point.x / f64::from(size.cell_width)).floor();
        let row = (point.y / f64::from(size.cell_height)).floor();
        let column = column.clamp(0.0, size.cols().saturating_sub(1) as f64) as usize;
        let row = row.clamp(0.0, size.rows().saturating_sub(1) as f64) as usize;
        (column, self.top_line() + row)
    }

    /// The absolute index of the line at the top of the view.
    fn top_line(&self) -> usize {
        match self.channels.session() {
            Some(session) => session
                .term()
                .history_size()
                .saturating_sub(self.scroll.offset()),
            None => 0,
        }
    }

    fn selection_window(&self) -> selection::Window {
        let size = self.viewport.term_size();
        selection::Window {
            top_line: self.top_line(),
            lines: size.rows(),
            columns: size.cols(),
        }
    }

    fn mode_contains(&self, mode: Mode) -> bool {
        self.channels
            .session()
            .is_some_and(|s| s.term().mode().contains(mode))
    }

    /// Whether the program below asked for the pointer.
    ///
    /// Public because it is also what the pointer *shape* is set from: an
    /// I-beam shows exactly while the program is not listening, which is
    /// the renderer's to apply.
    pub fn terminal_uses_mouse(&self) -> bool {
        self.channels
            .session()
            .is_some_and(|s| s.term().mode().intersects(Mode::MOUSE_MODE))
    }

    fn pointer_context(&self) -> PointerContext {
        PointerContext {
            terminal_uses_mouse: self.terminal_uses_mouse(),
            // `frozen_glass` is one thing and one thing only: this channel is
            // the gateway. An earlier revision had no channel model
            // to read it from and stood a scrolled-back view in for it
            // instead; the model exists now, and that substitute is retired.
            //
            // What it buys: over the gateway a drag marks the screen instead
            // of reporting to the program, the pointer keeps its I-beam, and
            // the middle button's paste goes nowhere -- the glass is still a
            // picture to read and copy from; it is no longer a surface to
            // type at.
            frozen_glass: self
                .channels
                .current()
                .is_some_and(|row| row.kind == ChannelKind::Gateway),
        }
    }

    /// Report a button or wheel event to the program, in whichever of the
    /// two wire formats it asked for.
    fn report_mouse(
        &mut self,
        button: mouse::MouseButton,
        cell: (usize, usize),
        modifiers: Modifiers,
        pressed: bool,
    ) {
        let column = cell.0.min(u16::MAX as usize) as u16;
        let row = cell
            .1
            .saturating_sub(self.top_line())
            .min(u16::MAX as usize) as u16;
        let bytes = if self.mode_contains(Mode::SGR_MOUSE) {
            mouse::encode_sgr(button, column, row, modifiers, pressed)
        } else {
            // X10 has no per-button release code: every release is a 3.
            let button = if pressed {
                button
            } else {
                mouse::MouseButton::Release
            };
            mouse::encode_x10(button, column, row, modifiers)
        };
        self.write(&bytes);
    }

    // ---- the seam ----------------------------------------------------

    /// A window x in the logical pixels every chassis measure is in
    /// (`chassis::cabinet`'s "which pixel").
    fn logical_x(&self, position: PhysicalPosition<f64>) -> f64 {
        position.x / self.viewport.scale_factor.max(f64::EPSILON)
    }

    /// A left press on the grab strip is the seam's and nobody else's.
    /// Returns whether it took it.
    fn seam_pressed(&mut self, button: MouseButton, position: PhysicalPosition<f64>) -> bool {
        if button != MouseButton::Left {
            return false;
        }
        let x = self.logical_x(position);
        self.seam_press = self
            .cabinet
            .as_mut()
            .is_some_and(|cabinet| cabinet.pointer_pressed(x));
        self.seam_press
    }

    /// The window itself is the key: pressing it reaches the channel, so the
    /// row carries no separate button. A left press inside one of the bank's
    /// drawn windows is that key going down.
    ///
    /// Returns whether it took the press. The hit test is the furniture's
    /// ([`chassis::Cabinet::strip_at`], over the same rectangles it drew) and
    /// what to do about it is the bank's ([`crate::bank::BankPager::press`],
    /// through [`TerminalSurface::press_strip`]): a dark slot starts a session
    /// and an open one comes to the screen.
    fn strip_pressed(&mut self, button: MouseButton, position: PhysicalPosition<f64>) -> bool {
        if button != MouseButton::Left {
            return false;
        }
        let scale = self.viewport.scale_factor.max(f64::EPSILON);
        let (x, y) = (position.x / scale, position.y / scale);
        let strips = self.bank_strips();
        let Some(row) = self
            .cabinet
            .as_ref()
            .and_then(|cabinet| cabinet.strip_at(strips.rows.len(), x, y))
        else {
            return false;
        };
        self.press_strip(strips.rows[row].channel);
        true
    }

    /// A motion the seam claims: it is either dragging the boundary or hovering
    /// the strip, and either way the grid does not also see it.
    fn seam_moved(&mut self, position: PhysicalPosition<f64>) -> bool {
        let x = self.logical_x(position);
        let Some(cabinet) = self.cabinet.as_mut() else {
            return false;
        };
        let update = cabinet.cursor_moved(x);
        let claimed = cabinet.cursor() == SeamCursor::ResizeHorizontal;
        self.set_seam_cursor(claimed);
        if let Some(update) = update {
            self.seam_landed(update);
        }
        claimed
    }

    /// The drag landed a new character count. The cabinet has already taken it;
    /// what is left is the two obligations that are not the chassis's to
    /// discharge (`chassis::cabinet`'s "Two obligations").
    fn seam_landed(&mut self, update: SeamUpdate) {
        // The write waits for the button to come up; the count is remembered
        // here so a drag that ends outside the strip still writes what it left.
        self.pending_led_characters = Some(update.led_characters);
        self.relayout();
        if let Some(proxy) = self.shell_events.as_ref() {
            let _ = proxy.send_event(ShellEvent::SetBankWidth(update.bank_width));
        }
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }

    /// The button came up, or the window lost focus with it still down.
    fn seam_released(&mut self) {
        self.seam_press = false;
        if let Some(cabinet) = self.cabinet.as_mut() {
            cabinet.pointer_released();
        }
        let Some(characters) = self.pending_led_characters.take() else {
            return;
        };
        let Some(settings) = self.settings.as_ref() else {
            // No handle: `--default-settings`, or a headless surface that never
            // attached one. The drag stands on screen and is not persisted,
            // which is what "never touch the user's real config" means.
            log::debug!("the seam landed on {characters} characters with no config to write");
            return;
        };
        // The file is the source of truth, so this is the write that makes the
        // drag real; the watcher then delivers it back as an ordinary reload,
        // carrying the count the cabinet already holds.
        if let Err(e) = settings.write_key(
            "general.led_characters",
            Scalar::Integer(i64::from(characters)),
        ) {
            log::error!("could not write general.led_characters = {characters}: {e}");
        }
    }

    /// The shape is the only thing that says the seam is there, so it
    /// changes on hover and not only while dragging.
    fn set_seam_cursor(&mut self, claimed: bool) {
        if claimed == self.seam_cursor {
            return;
        }
        self.seam_cursor = claimed;
        if let Some(window) = self.window.as_ref() {
            window.set_cursor(if claimed {
                CursorIcon::EwResize
            } else {
                CursorIcon::Default
            });
        }
    }

    fn drag_selection_to(&mut self, cell: (usize, usize)) {
        let win = self.selection_window();
        let Some(session) = self.channels.session() else {
            return;
        };
        let grid = RioGrid::new(session.term());
        self.selection.drag_to(&grid, win, cell.0, cell.1);
    }

    fn select_word_at(&mut self, cell: (usize, usize)) -> Option<String> {
        let win = self.selection_window();
        let session = self.channels.session()?;
        let grid = RioGrid::new(session.term());
        self.selection.double_click(&grid, win, cell.0, cell.1)
    }

    fn end_selection(&mut self) -> Option<String> {
        let session = self.channels.session()?;
        let grid = RioGrid::new(session.term());
        self.selection.release(&grid)
    }

    /// Konsole copies on select rather than on a keystroke. The clipboard
    /// needs a display to talk to, so a headless surface keeps the text
    /// and skips the platform call.
    fn copy_on_select(&mut self, text: Option<String>) {
        let Some(text) = text.filter(|t| !t.is_empty()) else {
            return;
        };
        if self.window.is_some() {
            if let Err(e) = clipboard::copy(&text) {
                log::debug!("could not copy the selection: {e}");
            }
        }
        self.last_selection = Some(text);
    }

    fn paste_primary(&mut self, ctrl_held: bool) {
        if self.window.is_none() {
            return;
        }
        match clipboard::paste() {
            Ok(text) => {
                // The terminal's own DECSET 2004 decides bracketing; the
                // routing table asks for it too when Ctrl was held.
                let bracketed = ctrl_held || self.mode_contains(Mode::BRACKETED_PASTE);
                let bytes = clipboard::bracket_paste(&text, bracketed);
                self.write(&bytes);
            }
            Err(e) => log::debug!("could not paste: {e}"),
        }
    }

}

/// The text a key press produced, with **every** modifier applied.
///
/// `KeyEvent::text` is not that: on the X11/Wayland backend it is
/// `keysym_to_utf8_raw(keysym)`, the text of the keysym the layout resolved,
/// which for `Ctrl+C` is `"c"`: Control is not in it (winit
/// `platform_impl/linux/common/xkb/mod.rs:315-327`, and the trait below says so
/// outright: "Identical to `KeyEvent::text` but this is affected by Ctrl").
/// `text_with_all_modifiers` is `get_utf8_raw(keycode)` against the live xkb
/// state, so `Ctrl+C` is `"\x03"`.
///
/// The second is what reaches Konsole. On X11 that text comes from
/// `XLookupString`, which folds Control into the ASCII control range, and
/// Konsole's keytab binds no `Ctrl+<letter>` at all: `Ctrl+C` reaches the
/// emulation as the *text* of the event and nowhere else. Taking winit's plain
/// `text` here sent a literal `c` to the child for every `Ctrl+<letter>` the
/// keytab does not bind (no interrupt, no `Ctrl+D`, no readline bindings)
/// on local channels and, once the diversion existed, through `send-keys` to
/// tmux panes as well.
///
/// winit implements the trait on Windows, macOS, X11 and Wayland
/// (`platform/mod.rs:44-52`), which is every platform this appliance builds
/// for; a target without it would fail to compile here rather than quietly lose
/// its control keys.
fn key_text(event: &winit::event::KeyEvent) -> Option<&str> {
    use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
    event.text_with_all_modifiers()
}

/// Which of the three buttons the routing table knows about this is.
/// Back and forward are not bound to anything.
fn pointer_button(button: MouseButton) -> Option<pointer::Button> {
    match button {
        MouseButton::Left => Some(pointer::Button::Left),
        MouseButton::Middle => Some(pointer::Button::Middle),
        MouseButton::Right => Some(pointer::Button::Right),
        _ => None,
    }
}

fn report_button(button: pointer::Button) -> mouse::MouseButton {
    match button {
        pointer::Button::Left => mouse::MouseButton::Left,
        pointer::Button::Middle => mouse::MouseButton::Middle,
        pointer::Button::Right => mouse::MouseButton::Right,
    }
}

fn modifiers_from(modifiers: ModifiersState) -> Modifiers {
    Modifiers {
        shift: modifiers.shift_key(),
        control: modifiers.control_key(),
        alt: modifiers.alt_key(),
        meta: modifiers.super_key(),
    }
}

impl Surface for TerminalSurface {
    fn resized(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        self.window_size = (size.width.max(1), size.height.max(1));
        self.relayout();
    }

    fn scale_factor_changed(&mut self, scale_factor: f64) {
        self.viewport.scale_factor = scale_factor;
        if let Some(window) = self.window.as_ref() {
            let physical = window.inner_size();
            self.window_size = (physical.width.max(1), physical.height.max(1));
        }
        self.relayout();
    }

    fn redraw(&mut self) {
        self.apply_live_settings();
        self.draw_frame();
    }

    fn set_size_badge(&mut self, text: &str, opacity: f32) {
        if self.size_badge.0 != text {
            self.size_badge.0.clear();
            self.size_badge.0.push_str(text);
        }
        self.size_badge.1 = opacity;
    }

    fn cell_size(&self) -> winit::dpi::PhysicalSize<u32> {
        // The same arithmetic the grid is divided by, so the overlay's cell
        // count and the session's cannot disagree.
        let (width, height) = self.viewport.physical_cell();
        winit::dpi::PhysicalSize::new(u32::from(width).max(1), u32::from(height).max(1))
    }

    fn key_pressed(&mut self, event: &winit::event::KeyEvent, modifiers: ModifiersState) {
        self.key_input(&event.logical_key, key_text(event), modifiers);
    }

    fn ime(&mut self, event: &Ime) {
        self.ime_input(event);
    }

    fn mouse_pressed(
        &mut self,
        button: MouseButton,
        position: PhysicalPosition<f64>,
        modifiers: ModifiersState,
    ) {
        // The seam first: a press it took is the cabinet's and must not also
        // mark the screen (`chassis::cabinet`'s wiring sketch). The bank's own
        // windows come next, for the same reason and in the same order: the
        // grab strip lies over the bank's right edge, so the seam is the
        // narrower claim and gets first refusal.
        if self.seam_pressed(button, position) || self.strip_pressed(button, position) {
            return;
        }
        let Some(button) = pointer_button(button) else {
            return;
        };
        let mods = modifiers_from(modifiers);
        let cell = self.cell_at(position);
        self.pointer_cell = cell;

        // `over_hot_spot` is false until the renderer owns a per-frame
        // URL chain to ask: activating a link needs the chain the drawn
        // frame was built from, not one rebuilt behind the pointer.
        // `term::hotspots` is tested against that day; this call site is
        // where it plugs in.
        match on_press(self.pointer_context(), button, mods, false) {
            PointerAction::Mark | PointerAction::MarkAndActivateHotSpot => {
                let now = Instant::now();
                let doubled = self.last_click.is_some_and(|(at, at_cell)| {
                    at_cell == cell && now.duration_since(at) < DOUBLE_CLICK_INTERVAL
                });
                if doubled {
                    // The word under the pointer, and word mode stays on
                    // so a drag afterwards extends by whole words.
                    let text = self.select_word_at(cell);
                    self.copy_on_select(text);
                    self.dragging = true;
                    self.last_click = None;
                } else {
                    self.selection.preserve_line_breaks = pointer::preserve_line_breaks(mods);
                    self.selection.column_selection_mode = pointer::column_selection_mode(mods);
                    self.selection.press(cell.0, cell.1);
                    self.dragging = true;
                    self.last_click = Some((now, cell));
                }
            }
            PointerAction::ReportToProgram => {
                self.report_mouse(report_button(button), cell, mods, true)
            }
            PointerAction::PastePrimary { bracketed } => self.paste_primary(bracketed),
            PointerAction::Ignore => {}
        }
    }

    fn mouse_released(
        &mut self,
        button: MouseButton,
        position: PhysicalPosition<f64>,
        modifiers: ModifiersState,
    ) {
        if self.seam_press && button == MouseButton::Left {
            self.seam_released();
            return;
        }
        let Some(button) = pointer_button(button) else {
            return;
        };
        let mods = modifiers_from(modifiers);
        let cell = self.cell_at(position);
        self.pointer_cell = cell;

        if button == pointer::Button::Left && self.dragging {
            self.dragging = false;
            // Konsole copies here, not on a keystroke: the selection is
            // the clipboard the moment the button comes up.
            let text = self.end_selection();
            self.copy_on_select(text);
            return;
        }
        if self.terminal_uses_mouse() && !mods.shift {
            self.report_mouse(report_button(button), cell, mods, false);
        }
    }

    fn cursor_moved(&mut self, position: PhysicalPosition<f64>, modifiers: ModifiersState) {
        // Every motion goes to the seam, drag or no drag: it tracks the pointer
        // for the cursor's shape and moves nothing until it is held.
        if self.seam_moved(position) {
            return;
        }
        let cell = self.cell_at(position);
        // Every pointer path is expressed in cells, so a move within one
        // cell has nothing to say, including to a program tracking
        // motion, which is addressed in cells too.
        if cell == self.pointer_cell {
            return;
        }
        self.pointer_cell = cell;

        if self.dragging {
            self.drag_selection_to(cell);
            return;
        }
        let mods = modifiers_from(modifiers);
        if !mods.shift && self.mode_contains(Mode::MOUSE_MOTION) {
            self.report_mouse(mouse::MouseButton::Release, cell, mods, true);
        }
    }

    fn mouse_wheel(&mut self, delta: MouseScrollDelta, modifiers: ModifiersState) {
        let notches = match delta {
            MouseScrollDelta::LineDelta(_, lines) => f64::from(lines),
            MouseScrollDelta::PixelDelta(pixels) => {
                // A trackpad reports pixels. Bank them until they add up
                // to a line, or a slow scroll would be no scroll at all.
                let cell_height = f64::from(self.viewport.term_size().cell_height).max(1.0);
                self.wheel_pixels += pixels.y;
                let lines = (self.wheel_pixels / cell_height).trunc();
                self.wheel_pixels -= lines * cell_height;
                lines
            }
        };
        let notches = notches.trunc() as i32;
        if notches == 0 {
            return;
        }
        let mods = modifiers_from(modifiers);

        // Shift is the user's override: it scrolls the view even while a
        // program is tracking the mouse.
        if self.terminal_uses_mouse() && !mods.shift {
            let button = if notches > 0 {
                mouse::MouseButton::WheelUp
            } else {
                mouse::MouseButton::WheelDown
            };
            let cell = self.pointer_cell;
            for _ in 0..notches.abs() {
                self.report_mouse(button, cell, mods, true);
            }
            return;
        }

        // Positive is up and into history, which is the sign
        // `ScrollPosition::scroll` takes.
        if let Some(session) = self.channels.session_mut() {
            self.scroll.scroll_wheel(session.term_mut(), notches);
        }
    }

    fn focus_changed(&mut self, focused: bool) {
        if !focused {
            // A window that loses focus never sees the button come up,
            // so a drag left running would resume on the next move. The seam's
            // release also writes what its drag landed, which is why it is the
            // same call the button makes and not a flag cleared here.
            self.dragging = false;
            self.seam_released();
            // A window that goes away under a half-typed chord commits it
            // rather than holding the digits for whenever it comes back.
            self.chord_modifier = false;
            self.commit_chord();
        }
        if self.mode_contains(Mode::FOCUS_IN_OUT) {
            let bytes: &[u8] = if focused { b"\x1b[I" } else { b"\x1b[O" };
            self.write(bytes);
        }
    }

    fn tick(&mut self) -> Tick {
        if self.channels.is_empty() {
            return Tick::default();
        }
        if self.eof {
            return Tick {
                finished: true,
                ..Tick::default()
            };
        }
        let bytes = self.pump();
        // Output written while the view is scrolled up moves the offset,
        // because the history under the view grew; the terminal is the
        // authority and this is where the scroll position hears about it.
        if let Some(session) = self.channels.session() {
            self.scroll.sync(session.term());
        }
        // Where the caret is, for the input method's candidate window.
        self.publish_ime_cursor();
        if self.eof {
            log::info!("the last channel is gone; closing");
            return Tick {
                finished: true,
                ..Tick::default()
            };
        }
        let now = Instant::now();
        let poll = now + POLL_INTERVAL;
        // The soonest sync deadline of any channel, not only the visible one:
        // an unattended channel's synchronised update has to end on time too,
        // or its screen is half-applied when the knob turns back to it.
        let mut wake_at = self
            .channels
            .rows_mut()
            .filter_map(|row| row.session.sync_deadline())
            .fold(poll, Instant::min);
        // The chord's own 900 ms, which nothing else would wake for: a chord
        // typed and then left alone has to commit on its own.
        if let Some(chord) = self.chord.tick(now) {
            self.apply_chord(Some(chord));
        }
        if let Some(deadline) = self.chord.deadline() {
            wake_at = wake_at.min(deadline);
        }

        // The effects clock. A window with no glass has no effects to run, so
        // it keeps to drawing only what changed.
        let mut effects_due = false;
        if self.glass.is_some() {
            let skip = match self.settings.as_ref() {
                Some(handle) => handle.current().general.effects_frame_skip,
                None => Config::default().general.effects_frame_skip,
            };
            let interval = EFFECTS_BASE_FRAME * skip.max(1) as u32;
            match self.next_effects_frame {
                Some(at) if at > now => wake_at = wake_at.min(at),
                _ => {
                    effects_due = true;
                    self.next_effects_frame = Some(now + interval);
                    wake_at = wake_at.min(now + interval);
                }
            }
        }

        // The output governor (see [`Self::next_output_frame`]): output asks
        // for a frame at most once per [`EFFECTS_BASE_FRAME`], and output
        // that arrived between frames is carried as pending, so the frame it
        // waits for is always asked for. An effects frame paints everything
        // anyway, so it discharges the pending output with it.
        if bytes > 0 {
            self.output_pending = true;
        }
        let mut output_due = false;
        if self.output_pending {
            match self.next_output_frame {
                Some(at) if at > now && !effects_due => wake_at = wake_at.min(at),
                _ => {
                    output_due = true;
                    self.output_pending = false;
                    self.next_output_frame = Some(now + EFFECTS_BASE_FRAME);
                }
            }
        }

        Tick {
            redraw: output_due || effects_due,
            wake_at: Some(wake_at),
            finished: false,
        }
    }

    /// The window's title tracks the current channel's title. `None` falls
    /// back to the application's name, which the shell already holds as its
    /// identity.
    fn title(&self) -> Option<String> {
        self.channels.current_title().map(str::to_string)
    }

    /// The release of the chord modifier commits whatever digits are
    /// waiting. winit has no key-release filter of its own, so the edge is
    /// taken off the modifier state the shell already tracks: the modifier
    /// was down, and now it is not.
    fn modifiers_changed(&mut self, modifiers: ModifiersState) {
        let down = if cfg!(target_os = "macos") {
            modifiers.super_key()
        } else {
            modifiers.alt_key()
        };
        let released = self.chord_modifier && !down;
        self.chord_modifier = down;
        if released {
            self.commit_chord();
        }
    }
}

/// A single ASCII digit, which is what the chord's ten shortcuts each carry.
fn is_digit(c: &str) -> bool {
    c.len() == 1 && c.as_bytes()[0].is_ascii_digit()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crt::{DegaussState, Geometry, Params};

    /// One frame's uniforms from a geometry, with the clock and the transient
    /// held still.
    fn params(cfg: &Config, geom: &Geometry) -> Params {
        Params::build(
            cfg,
            geom,
            crt::FrameTime {
                elapsed: 0.0,
                now: 0.0,
                delta: 0.0,
                index: 0,
            },
            DegaussState::IDLE,
        )
    }

    /// `crt::Geometry`'s `output_*` is in logical pixels, and the render
    /// target is physical. On an ordinary display the two are the same number
    /// and nothing shows; at scale factor 2 the difference is the whole of
    /// `normalizedScreenScale`, and with it the curvature and the moulding.
    #[test]
    fn the_chain_is_measured_in_logical_pixels_on_a_2x_display() {
        let cfg = Config::default();

        // The same window, twice: 1024x768 logical on a 1x display, and the
        // same 1024x768 logical on a 2x one, where the target is 2048x1536.
        // The font is twice the raster scale on the 2x display for the same
        // reason the target is twice the size.
        let one = chain_geometry((1024, 768), 1, &cfg, 1.0);
        let two = chain_geometry((2048, 1536), 2, &cfg, 2.0);

        assert_eq!((one.output_width, one.output_height), (1024.0, 768.0));
        assert_eq!(
            (two.output_width, two.output_height),
            (1024.0, 768.0),
            "the same window is the same size in logical pixels whatever the \
             display's scale factor"
        );

        // Which is the point: every uniform normalised by the screen scale is
        // the same on both, rather than half on the 2x display.
        let (a, b) = (params(&cfg, &one), params(&cfg, &two));
        for name in ["ScreenCurvature", "FrameSize", "screenRadius"] {
            let (x, y) = (a.get(name).unwrap(), b.get(name).unwrap());
            assert!(
                (x - y).abs() < 1e-6,
                "{name} is {x} at 1x and {y} at 2x; the window is the same size"
            );
        }

        // The raster grid is not a length, so it is *not* logical: it is a
        // count of unscaled font pixels, and the 2x display fits the same
        // number of them across because its font is scaled up to match.
        assert_eq!(one.virtual_width, two.virtual_width);
        assert_eq!(one.virtual_height, two.virtual_height);

        // And the rasterization fade still measures device pixels per raster
        // pixel, which is where the ratio comes back in: a 2x display at
        // twice the integer scale is the same density.
        assert!(
            (a.get("RasterizationIntensity").unwrap() - b.get("RasterizationIntensity").unwrap())
                .abs()
                < 1e-6
        );
    }

    /// The virtual width is `floor(width / (scale * font_width))` and the
    /// virtual height is `floor(height / scale)`. Both terms of the width
    /// matter -- a narrowed font packs more columns of raster into the same
    /// glass, and the mask is spaced by the count.
    #[test]
    fn the_virtual_resolution_takes_the_font_width_and_floors() {
        let mut cfg = Config::default();
        assert_eq!(cfg.screen.font_width, 1.0);
        let wide = chain_geometry((1000, 750), 3, &cfg, 1.0);
        // floor(1000/3) = 333, not 333.33; floor(750/3) = 250 exactly.
        assert_eq!(wide.virtual_width, 333.0);
        assert_eq!(wide.virtual_height, 250.0);

        cfg.screen.font_width = 0.5;
        let narrow = chain_geometry((1000, 750), 3, &cfg, 1.0);
        assert_eq!(narrow.virtual_width, 666.0);
        assert_eq!(
            narrow.virtual_height, 250.0,
            "font width narrows the columns and leaves the rows alone"
        );
    }

    /// Done-test: the well's grid is inset by `settings::distortion_margin`
    /// on every edge before its own column/row count is taken -- the space
    /// the grid is sized from shrinks by twice the margin on each axis,
    /// `margin` itself being a lerp between two configured bounds plus a
    /// curvature term ([`settings::distortion_margin`], the one derivation
    /// both this and the pointer's [`TerminalSurface::distortion_params`]
    /// read).
    ///
    /// The margin is logical; [`Viewport::margin`] is physical, so it is
    /// scaled by the sampled DPR here the same way a caller wiring this up
    /// (`TerminalSurface::new`/`apply_live_settings`) has to.
    #[test]
    fn the_grid_is_inset_by_the_distortion_margin_before_dividing_by_the_cell() {
        let mut cfg = Config::default();
        cfg.general.chassis_shown = false;
        cfg.screen.margin = 0.4;
        cfg.screen.screen_radius = 0.7;

        // The formula, spelled out independently of
        // `settings::distortion_margin` so this test would notice either
        // side drifting from the other.
        let lint = |a: f64, b: f64, t: f64| a + (b - a) * t;
        let screen_radius = lint(4.0, 120.0, 0.7);
        let expected_margin_logical =
            lint(1.0, 40.0, 0.4) + (1.0 - std::f64::consts::FRAC_1_SQRT_2) * screen_radius;
        let margin_logical = settings::distortion_margin(&cfg);
        assert!(
            (margin_logical - expected_margin_logical).abs() < 1e-9,
            "{margin_logical} != {expected_margin_logical}"
        );

        // A sampled well size and DPR, not defaults, so the inset is a real
        // number of pixels rather than an edge case.
        let scale_factor = 1.5;
        let mut viewport = Viewport::new(1000, 820, scale_factor, CellSize::new(9.0, 18.0));
        viewport.margin = margin_logical * scale_factor;

        let size = viewport.term_size();
        assert_eq!(
            (size.cols(), size.rows()),
            (62, 25),
            "875x695 physical pixels left after a 125px (2 * round(62.33..)) \
             inset, divided by the 14x27 physical cell"
        );

        // Positioning: `draw_frame` centres the grid in the well at a
        // whole-pixel origin (`(target - grid).max(0) / 2`); with the grid
        // now smaller by the margin on every edge, that same centring
        // already seats it inset by (roughly) the margin rather than
        // flush against the well's own edge.
        let (grid_w, grid_h) = size.pixel_size();
        let origin = (
            (1000i32 - i32::from(grid_w)).max(0) / 2,
            (820i32 - i32::from(grid_h)).max(0) / 2,
        );
        assert_eq!(origin, (66, 72));
    }
}
