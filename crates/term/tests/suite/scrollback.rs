//! The scrollback viewport and the damage path, driven by a real rio-vt
//! terminal (no PTY: bytes are fed straight to the processor, which is what
//! `Processor::advance` is for and keeps the test deterministic).
//!
//! What is being pinned down here:
//!
//! * scrolling back shows history, and the renderer's cells follow it;
//! * a viewport move forces a full rebuild, because rio-vt's per-line damage
//!   is in viewport coordinates and cannot describe lines that arrived from
//!   history;
//! * ordinary output does *not* force a full rebuild, so the damage path is
//!   actually narrowing the work rather than redrawing the screen every frame.

use rio_vt::ansi::CursorShape;
use rio_vt::crosswords::grid::Dimensions;
use rio_vt::crosswords::Crosswords;
use rio_vt::event::{VoidListener, WindowId};
use rio_vt::performer::handler::Processor;

use crt_burnin::headless::GpuLock;
use term::atlas::Rasterization;
use term::color::Scheme;
use term::fonts::font_by_name;
use term::fonts::sizing::{self, ScalePolicy, SizingRequest};
use term::gpu::Gpu;
use term::render::GridRenderer;
use term::viewport::ScrollPosition;
use term::{ascii_charset, FontContext, DEFAULT_THRESHOLD};

const COLS: usize = 40;
const ROWS: usize = 6;
const SCROLLBACK: usize = 200;

#[derive(Clone, Copy)]
struct Size;

impl Dimensions for Size {
    fn total_lines(&self) -> usize {
        ROWS
    }
    fn screen_lines(&self) -> usize {
        ROWS
    }
    fn columns(&self) -> usize {
        COLS
    }
}

/// The device, and the machine-wide GPU lock it is held under.
///
/// `term::gpu::Gpu` is the shipping type and takes no lock of its own -- the
/// application is not competing with anyone for a device. These tests are, with
/// every other GPU test in the tree and in any other process running the suite,
/// which is what `crt_burnin::headless::GpuLock` exists for. The tuple's order
/// is its drop order: the device goes first, the lock after it.
fn gpu() -> Option<(Gpu, GpuLock)> {
    let lock = match GpuLock::acquire() {
        Ok(lock) => lock,
        Err(e) => panic!("cannot take the GPU lock: {e}"),
    };
    match Gpu::new() {
        Ok(gpu) => Some((gpu, lock)),
        Err(e) => {
            if std::env::var("ROBCO_SKIP_GPU_TESTS").is_ok() {
                eprintln!("skipping: no wgpu adapter ({e})");
                None
            } else {
                panic!("no wgpu adapter: {e}");
            }
        }
    }
}

fn renderer(gpu: &Gpu, scheme: &Scheme) -> GridRenderer {
    let terminess = font_by_name("TERMINESS_SCALED").expect("TERMINESS_SCALED in the catalogue");
    let resolved = sizing::resolve(terminess, &SizingRequest::default(), ScalePolicy::Floor);
    let mut font = FontContext::new(terminess);
    let atlas = font.build_atlas(
        &gpu.device,
        &gpu.queue,
        &resolved,
        &ascii_charset(),
        Rasterization::Binary {
            threshold: DEFAULT_THRESHOLD,
        },
    );
    GridRenderer::new(&gpu.device, &gpu.queue, atlas, COLS, ROWS, scheme.clone())
}

#[test]
fn scrollback_viewport_follows_history() {
    let Some((gpu, _lock)) = gpu() else { return };
    let scheme = Scheme::monochrome([1.0, 1.0, 1.0, 1.0], [0.0, 0.0, 0.0, 0.0]);
    let mut renderer = renderer(&gpu, &scheme);
    let mut viewport = ScrollPosition::default();

    let mut term = Crosswords::new(
        Size,
        CursorShape::Block,
        VoidListener {},
        WindowId::from(0u64),
        0,
        SCROLLBACK,
    );
    let mut processor = Processor::default();

    // Twenty lines through a six-row screen: fourteen end up in history.
    for i in 0..20 {
        processor.advance(&mut term, format!("LINE-{i:03}\r\n").as_bytes());
    }
    let stats = renderer.sync(&gpu.device, &gpu.queue, &mut term, &mut viewport);
    assert!(stats.rows_updated > 0, "output produced no damage at all");
    assert_eq!(
        viewport.offset(),
        0,
        "output should leave the view at the bottom"
    );
    assert!(viewport.is_following());
    assert_eq!(renderer.grid().row_text(0).trim_end(), "LINE-015");
    assert_eq!(renderer.grid().row_text(4).trim_end(), "LINE-019");

    // Scroll back five lines: the same screen, five lines earlier.
    viewport.scroll(&mut term, 5);
    assert_eq!(viewport.offset(), 5);
    assert!(!viewport.is_following());
    let stats = renderer.sync(&gpu.device, &gpu.queue, &mut term, &mut viewport);
    assert!(
        stats.full,
        "a viewport move has to rebuild every row: rio-vt's damage is in \
         viewport coordinates and says nothing about lines that came from history"
    );
    assert_eq!(renderer.grid().row_text(0).trim_end(), "LINE-010");
    assert_eq!(renderer.grid().row_text(4).trim_end(), "LINE-014");

    // Back to the bottom.
    viewport.to_bottom(&mut term);
    renderer.sync(&gpu.device, &gpu.queue, &mut term, &mut viewport);
    assert_eq!(viewport.offset(), 0);
    assert!(viewport.is_following());
    assert_eq!(renderer.grid().row_text(4).trim_end(), "LINE-019");

    // A page is a screenful, clamped by the history that exists.
    viewport.page_up(&mut term);
    assert_eq!(viewport.offset(), ROWS);
    viewport.page_down(&mut term);
    assert_eq!(viewport.offset(), 0);
    viewport.to_top(&mut term);
    assert_eq!(viewport.offset(), term.history_size());
}

#[test]
fn ordinary_output_does_not_force_a_full_rebuild() {
    let Some((gpu, _lock)) = gpu() else { return };
    let scheme = Scheme::monochrome([1.0, 1.0, 1.0, 1.0], [0.0, 0.0, 0.0, 0.0]);
    let mut renderer = renderer(&gpu, &scheme);
    let mut viewport = ScrollPosition::default();

    let mut term = Crosswords::new(
        Size,
        CursorShape::Block,
        VoidListener {},
        WindowId::from(0u64),
        0,
        SCROLLBACK,
    );
    let mut processor = Processor::default();

    // Settle: the first sync after construction is allowed to be full.
    processor.advance(&mut term, b"first\r\n");
    renderer.sync(&gpu.device, &gpu.queue, &mut term, &mut viewport);

    // One line of output on a screen with room to spare. Two rows can
    // legitimately need rewriting (the text, and the row the cursor left).
    processor.advance(&mut term, b"second\r\n");
    let stats = renderer.sync(&gpu.device, &gpu.queue, &mut term, &mut viewport);
    assert!(
        !stats.full,
        "one line of output should not repaint the screen"
    );
    assert!(
        stats.rows_updated <= 3,
        "one line of output rewrote {} of {ROWS} rows",
        stats.rows_updated
    );
    assert_eq!(renderer.grid().row_text(1).trim_end(), "second");

    // Nothing at all: nothing to do.
    let stats = renderer.sync(&gpu.device, &gpu.queue, &mut term, &mut viewport);
    assert!(!stats.full);
    assert_eq!(stats.rows_updated, 0, "an idle frame rewrote rows");
}

#[test]
fn untouched_cells_read_as_blanks_not_nuls() {
    let Some((gpu, _lock)) = gpu() else { return };
    let scheme = Scheme::monochrome([1.0, 1.0, 1.0, 1.0], [0.0, 0.0, 0.0, 0.0]);
    let mut renderer = renderer(&gpu, &scheme);
    let mut viewport = ScrollPosition::default();

    let mut term = Crosswords::new(
        Size,
        CursorShape::Block,
        VoidListener {},
        WindowId::from(0u64),
        0,
        SCROLLBACK,
    );
    let mut processor = Processor::default();
    processor.advance(&mut term, b"AB");
    renderer.sync(&gpu.device, &gpu.queue, &mut term, &mut viewport);

    // The trap: an untouched `Square` reads `'\0'`, not `' '`. If the
    // normalisation is ever dropped, this row becomes two characters followed
    // by thirty-eight NULs and the atlas quietly maps none of them.
    let row = renderer.grid().row_text(0);
    assert_eq!(row.len(), COLS);
    assert!(
        !row.contains('\0'),
        "row 0 still carries NUL cells: {row:?}"
    );
    assert_eq!(row.trim_end(), "AB");
}
