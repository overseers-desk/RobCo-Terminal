//! Done-test for the composite: the chrome pass is drawn onto a frame that is
//! already there, and it leaves the rest of that frame alone.
//!
//! This turns the "chassis chrome sits outside the CRT chain" rule into a
//! measurement rather than a claim. The casting is chrome: it is not a stage
//! the picture passes through, it goes on afterwards. Two things follow, and
//! both are checked here on real GPU pixels:
//!
//! - the column's own rectangle carries the shell's metal, matching
//!   `oracle::chassis_metal` at sampled points, which is only true if
//!   the two sizes went into the mount the right way round (the drawn rectangle
//!   is the bank's, `viewport_size` is the screen well's; see `app::chrome`);
//! - every pixel right of the seam is exactly what was on the frame before, to
//!   the bit. That is what the pass's `LoadOp::Load` and its scissor at the
//!   column's rectangle buy, and it is the claim that has to be measured
//!   rather than read: a piece is allowed to hang off the column and one
//!   routinely does, so what keeps a spill margin off the glass is the
//!   scissor and nothing else.
//!
//! The furniture standing on the casting is here as composition -- a plate
//! over the casting, lamps in their own cells, a row's description over the
//! plate, a screw whose corners leave the plate showing. What each piece
//! *draws* is `bank_furniture.rs`, which reads one piece at a time over
//! nothing.
//!
//! And the hidden-chassis state, which is not a column of no width but no
//! column: nothing is drawn at all, and the frame comes back untouched.

use app::chrome::Chrome;
use chassis::Cabinet;
use config::Config;
use gpu::harness::{px_index, Locked};

/// The window, at the default window size.
const WINDOW_W: u32 = 1024;
const WINDOW_H: u32 = 768;
/// The shipped profile's bank, and the well it leaves.
const BANK: u32 = 205;

/// What the frame carries before the column goes on: a colour no metal
/// produces, so "untouched" is unambiguous.
const SENTINEL: [f32; 4] = [0.25, 0.5, 0.75, 1.0];

/// A boot page whose channel on the air has been named, which is the appliance
/// a second after launch: `BankStrips::cold_start` is the beat before the shell
/// writes its OSC and the window is lit but blank -- an empty title at that
/// point -- and a blank window puts no glyph on the strip for a pixel test to
/// find.
fn titled(rows: usize, title: &str) -> chassis::BankStrips {
    let mut strips = chassis::BankStrips::cold_start(rows);
    strips.rows[0].title = title.to_string();
    strips
}

/// The uniforms of a piece the caller knows is a plate. The struct is the
/// oracle's own input, so the mount's block goes straight to the CPU
/// reimplementation the pixels are measured against.
fn plate_params(piece: &chassis::Piece) -> oracle::PlateMetalParams {
    match piece.params {
        Some(chassis::PieceParams::Plate(p)) => p,
        other => panic!("not a plate: {other:?}"),
    }
}

/// The uniforms of a piece the caller knows is an LED window.
fn led_params(piece: &chassis::Piece) -> chassis::params::LedMetalParams {
    match piece.params {
        Some(chassis::PieceParams::Led(p)) => p,
        other => panic!("not an LED window: {other:?}"),
    }
}

/// The uniforms of a piece the caller knows is a tape label.
fn tape_params(piece: &chassis::Piece) -> chassis::params::TapeMetalParams {
    match piece.params {
        Some(chassis::PieceParams::Tape(p)) => p,
        other => panic!("not a tape label: {other:?}"),
    }
}

/// Clear a frame to [`SENTINEL`], composite the column over it, read it back.
fn frame_with_column(
    gpu: &Locked,
    chrome: &mut Chrome,
    bank: u32,
    scale_factor: f64,
    params: &chassis::params::ChassisMetalParams,
) -> Vec<[f32; 4]> {
    frame_with_furniture(gpu, chrome, bank, scale_factor, params, &[])
}



/// The same, with furniture on the casting.
fn frame_with_furniture(
    gpu: &Locked,
    chrome: &mut Chrome,
    bank: u32,
    scale_factor: f64,
    params: &chassis::params::ChassisMetalParams,
    pieces: &[chassis::Piece],
) -> Vec<[f32; 4]> {
    let output = gpu.make_output(WINDOW_W, WINDOW_H);
    let view = output.create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("bank column test"),
        });

    // Whatever the chain would have left on this image.
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("the frame the chain drew"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color {
                    r: f64::from(SENTINEL[0]),
                    g: f64::from(SENTINEL[1]),
                    b: f64::from(SENTINEL[2]),
                    a: f64::from(SENTINEL[3]),
                }),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });

    chrome.render(
        &gpu.device,
        &gpu.queue,
        &mut encoder,
        &view,
        (WINDOW_W, WINDOW_H),
        (bank, WINDOW_H),
        (WINDOW_W - bank, WINDOW_H),
        scale_factor,
        Some(params),
        &std::sync::Arc::from(pieces.to_vec()),
        None,
    );

    let index = gpu.queue.submit([encoder.finish()]);
    gpu.device
        .poll(wgpu::PollType::Wait {
            submission_index: Some(index),
            timeout: None,
        })
        .expect("device poll");
    gpu.read_output(&output, WINDOW_W, WINDOW_H)
        .expect("readback")
}

#[test]
fn the_column_lands_on_the_frame_and_leaves_the_glass_untouched() {
    let cfg = Config::default();
    let cabinet = Cabinet::from_config(&cfg, f64::from(WINDOW_W), f64::from(WINDOW_H));
    assert_eq!(cabinet.bank_width(), BANK);
    let params = cabinet.chassis_params();

    let gpu = Locked::new().expect("headless wgpu device");
    let mut column = Chrome::new(&gpu.device, gpu::harness::OUTPUT_FORMAT);

    let frame = frame_with_column(&gpu, &mut column, BANK, 1.0, &params);

    // The casting, against the closed form. `viewportSize` is the *well's*
    // size, which is what `fieldScale`/`fieldOffset` are expressed against:
    // the metal field is the bezel's, continued leftwards, so the grain runs
    // across the boundary rather than restarting.
    // At this scale factor the well's physical and logical sizes are the same
    // number; the test below separates them.
    let oracle_params = params;
    let well = [(WINDOW_W - BANK) as f32, WINDOW_H as f32];
    for &(x, y) in &[
        (0u32, 0u32),
        (BANK / 2, WINDOW_H / 2),
        (BANK - 1, 700),
        (8, 383),
    ] {
        let uv = [
            (x as f32 + 0.5) / BANK as f32,
            (y as f32 + 0.5) / WINDOW_H as f32,
        ];
        let expected = oracle::chassis_metal(uv, well, &oracle_params);
        let got = frame[px_index(WINDOW_W, x, y)];
        let tol = 0.01;
        assert!(
            (got[0] - expected[0]).abs() < tol
                && (got[1] - expected[1]).abs() < tol
                && (got[2] - expected[2]).abs() < tol,
            "the casting at ({x},{y}) reads {:?} where the oracle says {expected:?}",
            &got[0..3]
        );
    }

    // ...and it is metal rather than a flat fill: the field varies across the
    // column, so a mount that had dropped every uniform would not pass above
    // by accident.
    let a = frame[px_index(WINDOW_W, 4, 4)];
    let b = frame[px_index(WINDOW_W, 200, 600)];
    assert!(
        (a[0] - b[0]).abs() > 1e-4 || (a[1] - b[1]).abs() > 1e-4,
        "the column is a flat fill: {a:?} against {b:?}"
    );

    // The frame the chain drew, everywhere the column is not. Bit-exact,
    // because a load-op the column got wrong would not be subtle.
    for &(x, y) in &[
        (BANK, 0),
        (BANK, WINDOW_H / 2),
        (BANK + 1, 400),
        (WINDOW_W / 2, WINDOW_H / 2),
        (WINDOW_W - 1, WINDOW_H - 1),
    ] {
        assert_eq!(
            frame[px_index(WINDOW_W, x, y)],
            SENTINEL,
            "the column reached ({x},{y}), which is the glass's"
        );
    }
}

/// The same well, on a 2x display: the field the casting is measured in is the
/// well's *logical* size, so the metal at a given point on the column is the
/// metal the oracle gives for a well of half the pixels.
///
/// This is the bank half of the frame's ruler. `frame_metal` divides its own
/// `OutputSize` by `windowScaling * DPR` to land on logical pixels
/// (`crt::params::Params::build`); the column has no window scaling to undo and
/// reaches the same ruler by declaring the well over the ratio
/// (`app::chrome::well_ruler`). Get one of the two wrong and the grain changes
/// scale at the seam on every HiDPI display, which no test at ratio 1 can see.
#[test]
fn the_castings_field_is_the_wells_logical_size_on_a_hidpi_display() {
    let cfg = Config::default();
    let cabinet = Cabinet::from_config(&cfg, f64::from(WINDOW_W), f64::from(WINDOW_H));
    let params = cabinet.chassis_params();

    let gpu = Locked::new().expect("headless wgpu device");
    let mut column = Chrome::new(&gpu.device, gpu::harness::OUTPUT_FORMAT);

    let frame = frame_with_column(&gpu, &mut column, BANK, 2.0, &params);

    let oracle_params = params;
    // Rounded, not halved: `well_ruler` hands the pass whole pixels, and this
    // well is an odd 777 wide.
    let logical_well = [
        ((WINDOW_W - BANK) as f32 / 2.0).round(),
        (WINDOW_H as f32 / 2.0).round(),
    ];
    let physical_well = [(WINDOW_W - BANK) as f32, WINDOW_H as f32];

    // Only at the points where the two candidate wells actually disagree:
    // procedural metal is not uniformly sensitive to its field, and a point
    // where both answers are the same colour would pass whichever size the
    // mount declared. The margins are narrow because this casting is *dark*
    // (the switchboard's #232830 under a vignette reads around 0.05), so a
    // visible change of field is a small change of number: the two wells part
    // by at most 0.016 anywhere on the column. `TELLING` is more than three
    // times `TOL`, so a pass is the logical well rather than a tolerance wide
    // enough to swallow the difference. The sibling test above, which
    // does not need to tell two wells apart, keeps the ordinary 0.01.
    const TOL: f32 = 0.003;
    const TELLING: f32 = 0.010;
    let mut telling = 0;
    // The step is the sampling density, not a property: it wants enough
    // telling points on a column this wide to make the count below mean
    // something, and the column's width follows the face the cabinet letters
    // its bank in.
    for x in (0..BANK).step_by(7) {
        for y in (0..WINDOW_H).step_by(29) {
            let uv = [
                (x as f32 + 0.5) / BANK as f32,
                (y as f32 + 0.5) / WINDOW_H as f32,
            ];
            let expected = oracle::chassis_metal(uv, logical_well, &oracle_params);
            let physical = oracle::chassis_metal(uv, physical_well, &oracle_params);
            if (physical[0] - expected[0]).abs() < TELLING {
                continue;
            }
            telling += 1;
            let got = frame[px_index(WINDOW_W, x, y)];
            assert!(
                (got[0] - expected[0]).abs() < TOL
                    && (got[1] - expected[1]).abs() < TOL
                    && (got[2] - expected[2]).abs() < TOL,
                "the casting at ({x},{y}) reads {:?}; the logical well {logical_well:?} \
                 says {expected:?} and the physical one {physical_well:?} says {physical:?}",
                &got[0..3]
            );
        }
    }
    assert!(
        telling >= 10,
        "only {telling} sampled points tell the two wells apart, which is too few to \
         prove which one the mount declared"
    );
}

/// The furniture, piece by piece, on real pixels: the plate against its own
/// oracle, a strip's lamps against the raster that was composed for it, and
/// the glass still bit-identical to the frame that was there before.
///
/// This is the non-interference property the sibling test above proves for the
/// bare casting, re-proved with the bank fully dressed, which is the state
/// that can break it, since a strip's spill margin deliberately reaches past
/// the column's own edges and the scissor cuts it there.
#[test]
fn the_furniture_lands_on_the_casting_and_still_leaves_the_glass_untouched() {
    let cfg = Config::default();
    let cabinet = Cabinet::from_config(&cfg, f64::from(WINDOW_W), f64::from(WINDOW_H));
    let rows = cabinet.rows_visible();
    assert!(rows > 1, "a 768px bank shows more than one row, not {rows}");
    let view = titled(rows as usize, "channel-1");
    let pieces = cabinet.furniture(&view);
    // The plate, the chassis's four screws, a row's furniture and a row's
    // window per engraved key, and the pager: this order is pinned in
    // `chassis::furniture`'s own tests and relied on here.
    assert_eq!(pieces.len(), 1 + 4 + 2 * rows as usize + 1);
    let row_furniture = |i: usize| &pieces[5 + 2 * i];
    let strip_of = |i: usize| &pieces[6 + 2 * i];

    let gpu = Locked::new().expect("headless wgpu device");
    let mut column = Chrome::new(&gpu.device, gpu::harness::OUTPUT_FORMAT);

    let bare = frame_with_column(&gpu, &mut column, BANK, 1.0, &cabinet.chassis_params());
    let frame = frame_with_furniture(
        &gpu,
        &mut column,
        BANK,
        1.0,
        &cabinet.chassis_params(),
        &pieces,
    );

    // --- the plate ----------------------------------------------------
    //
    // Deep inside the plate, away from the bevel band and the rounded
    // corners the standalone `plate_metal` test already pins, the composite
    // is the plate's own closed form, which is only true if the mount
    // handed the pass the same `sizePx` the recipe was built with and put
    // the result at the recipe's own rectangle.
    let plate = &pieces[0];
    assert_eq!(plate.pass, chassis::Pass::Plate);
    let plate_oracle = plate_params(plate);
    let (px, py) = (plate.rect.x as u32, plate.rect.y as u32);
    let (pw, ph) = (plate.rect.width as u32, plate.rect.height as u32);
    // Points on the plate that no strip covers. The strips stand at the
    // content ground past the numeral lane and reach a spill margin further
    // in every direction, so the clear plate is the shoulder to their left and
    // the headroom above row 1 (61px of it).
    let strip0 = strip_of(0);
    let clear_x = (strip0.rect.x - plate.rect.x) as u32;
    let clear_y = (strip0.rect.y - plate.rect.y) as u32;
    assert!(clear_x > 4 && clear_y > 4, "no clear plate to sample");
    // ...and clear of the plate's *own* edge too: `plate_metal`'s coverage
    // falls off across its 2.5px bevel and its 6px corner radius, which is
    // the band the standalone `plate_metal` test deliberately does not pin.
    // ...and clear of the furniture that now stands on the plate. The headroom
    // above row 1 is the one band with nothing in it, and even there the two
    // top screws own x 10..38 and 199..227 of the plate at y 13..42, so the
    // samples sit between them or above them.
    let _ = clear_y;
    for &(dx, dy) in &[(pw / 2, 8u32), (60, 8), (100, 50), (149, 55)] {
        let uv = [(dx as f32 + 0.5) / pw as f32, (dy as f32 + 0.5) / ph as f32];
        let (color, coverage) = oracle::plate_metal(uv, &plate_oracle);
        let got = frame[px_index(WINDOW_W, px + dx, py + dy)];
        // Premultiplied source-over onto the casting, which is what the
        // mount's blend state does; deep inside the plate the coverage is 1
        // and the casting under it contributes nothing.
        let under = bare[px_index(WINDOW_W, px + dx, py + dy)];
        let expect = |i: usize| color[i] * coverage + under[i] * (1.0 - coverage);
        let tol = 0.01;
        assert!(
            (got[0] - expect(0)).abs() < tol
                && (got[1] - expect(1)).abs() < tol
                && (got[2] - expect(2)).abs() < tol,
            "the plate at ({dx},{dy}) reads {:?}; the oracle says {:?} at coverage {coverage}",
            &got[0..3],
            color
        );
    }
    // ...and the plate is not the casting: it is a *raised* piece, so it has
    // to differ from what was underneath.
    let inside = px_index(WINDOW_W, px + pw / 2, py + 8);
    assert!(
        (frame[inside][0] - bare[inside][0]).abs() > 1e-3,
        "the plate left the casting unchanged at its own centre"
    );

    // --- a strip's lamps ----------------------------------------------
    //
    // The one powered window is row 1's. Its numeral pixels are the raster
    // `chassis::furniture` composed, and the mount is right only if each
    // lamp of that raster lands in its own cell of the drawn rectangle: a
    // lit cell reads the lit colour and an unlit one the dim colour, the
    // same law `chassis/tests/led_display.rs` proves against the shader
    // alone.
    let strip = strip_of(0);
    assert_eq!(strip.pass, chassis::Pass::LedMatrix);
    let raster = strip.source.as_ref().expect("a lamp grid");
    let colors =
        chassis::displays::led::window_colors(chassis::furniture::font_color(&cfg), true, true);
    let led = led_params(strip);
    let grid = (led.grid_size[0] as u32, led.grid_size[1] as u32);
    assert_eq!((raster.width, raster.height), grid);
    // The window inside the grown rectangle: the spill margin is a fraction
    // of that rectangle and the lamps live between the two margins.
    let spill = (
        f64::from(led.spill_margin[0]) * strip.rect.width,
        f64::from(led.spill_margin[1]) * strip.rect.height,
    );
    let win = (
        strip.rect.x + spill.0,
        strip.rect.y + spill.1,
        strip.rect.width - 2.0 * spill.0,
        strip.rect.height - 2.0 * spill.1,
    );

    let mut lit_cells = 0;
    let mut dim_cells = 0;
    for gy in 0..grid.1 {
        for gx in 0..grid.0 {
            // The cell's centre, in column pixels.
            let cx = win.0 + (f64::from(gx) + 0.5) * win.2 / f64::from(grid.0);
            let cy = win.1 + (f64::from(gy) + 0.5) * win.3 / f64::from(grid.1);
            if cx < 0.0 || cy < 0.0 || cx >= f64::from(BANK) || cy >= f64::from(WINDOW_H) {
                continue;
            }
            let lit = raster.rgba[((gy * grid.0 + gx) * 4) as usize] >= 128;
            let expected = if lit { colors.lit } else { colors.dim };
            let got = frame[px_index(WINDOW_W, cx as u32, cy as u32)];
            let tol = 0.08;
            assert!(
                (got[0] - expected.r).abs() < tol
                    && (got[1] - expected.g).abs() < tol
                    && (got[2] - expected.b).abs() < tol,
                "lamp ({gx},{gy}) lit={lit} at column ({cx:.1},{cy:.1}) reads {:?}, \
                 the raster says {expected:?}",
                &got[0..3]
            );
            if lit {
                lit_cells += 1;
            } else {
                dim_cells += 1;
            }
        }
    }
    // A raster of nothing but dark lamps would pass the loop above without
    // proving a numeral reached the glass at all.
    assert!(
        lit_cells > 0 && dim_cells > 0,
        "the strip's raster carried {lit_cells} lit lamps and {dim_cells} dark ones"
    );

    // A dark slot's window is dark: row 2 has no session, so its lamps read
    // the unpowered dim rather than the powered one.
    let dark = strip_of(1);
    let dark_colors =
        chassis::displays::led::window_colors(chassis::furniture::font_color(&cfg), false, false);
    assert!(dark_colors.dim.r < colors.dim.r);
    let (dx, dy) = (
        (dark.rect.x + dark.rect.width / 2.0) as u32,
        (dark.rect.y + dark.rect.height / 2.0) as u32,
    );
    let got = frame[px_index(WINDOW_W, dx, dy)];
    assert!(
        (got[0] - dark_colors.dim.r).abs() < 0.08,
        "the dark slot's window at ({dx},{dy}) reads {:?}, not the unpowered dim {:?}",
        &got[0..3],
        dark_colors.dim
    );

    // --- the drawn furniture ------------------------------------------
    //
    // A row's numerals and moulding, and one of the plate's screws, are
    // `Pass::Painted`: every operation of the description is its own instance
    // of the same pass, composited source-over in the painting's own order.
    // So the proof is where the description landed: the row covers its own
    // band of plate and stops at the piece's rectangle, which is what says
    // the mount placed the operations at the piece's origin and at the
    // window's own ratio.
    // The "before" is not the bare casting but the column with everything the
    // plan draws *ahead of* this piece on it (the plate and the screws)
    // because that is what a source-over lands on.
    let before_row = frame_with_furniture(
        &gpu,
        &mut column,
        BANK,
        1.0,
        &cabinet.chassis_params(),
        &pieces[..5],
    );
    let with_row = frame_with_furniture(
        &gpu,
        &mut column,
        BANK,
        1.0,
        &cabinet.chassis_params(),
        &pieces[..6],
    );
    let painted = row_furniture(0);
    assert_eq!(painted.pass, chassis::Pass::Painted);
    let (rx, ry) = (painted.rect.x as u32, painted.rect.y as u32);
    let (rw, rh) = (painted.rect.width as u32, painted.rect.height as u32);
    let moved = |x: u32, y: u32| {
        let i = px_index(WINDOW_W, x, y);
        (0..3).any(|c| (with_row[i][c] - before_row[i][c]).abs() > 1.0 / 255.0)
    };
    let mut struck = 0;
    for y in 0..rh {
        for x in 0..rw {
            if rx + x >= BANK || ry + y >= WINDOW_H {
                continue;
            }
            if moved(rx + x, ry + y) {
                struck += 1;
            }
        }
    }
    // A row that drew nothing, or one whose operations all landed off the
    // piece, leaves the plate exactly as it found it.
    assert!(
        struck > 500,
        "the row moved only {struck} pixels of the plate"
    );

    // ...and it stays inside its own rectangle: every pixel of the bank the
    // piece does not cover is what it was before the piece went on.
    for y in 0..WINDOW_H {
        for x in 0..BANK {
            if x >= rx && x < rx + rw && y >= ry && y < ry + rh {
                continue;
            }
            assert!(
                !moved(x, y),
                "the row at ({rx},{ry}) {rw}x{rh} reached ({x},{y})"
            );
        }
    }

    // One screw, the same way, and it is round: the item's corners are
    // outside the countersink and have to leave the plate showing.
    let screw = &pieces[1];
    assert_eq!(screw.pass, chassis::Pass::Painted);
    let (sx, sy) = (screw.rect.x as u32, screw.rect.y as u32);
    let (sw, sh) = (screw.rect.width as u32, screw.rect.height as u32);
    let plate_only = frame_with_furniture(
        &gpu,
        &mut column,
        BANK,
        1.0,
        &cabinet.chassis_params(),
        &pieces[..1],
    );
    let with_screw = frame_with_furniture(
        &gpu,
        &mut column,
        BANK,
        1.0,
        &cabinet.chassis_params(),
        &pieces[..2],
    );
    assert_eq!(
        with_screw[px_index(WINDOW_W, sx, sy)],
        plate_only[px_index(WINDOW_W, sx, sy)],
        "the screw's transparent corner changed the plate under it"
    );
    let centre = px_index(WINDOW_W, sx + sw / 2, sy + sh / 2);
    assert!(
        (0..3).any(|c| (with_screw[centre][c] - plate_only[centre][c]).abs() > 4.0 / 255.0),
        "the screw's head is not on the plate: {:?} over {:?}",
        &with_screw[centre][0..3],
        &plate_only[centre][0..3]
    );

    // --- the glass ----------------------------------------------------
    //
    // Bit-identical to the frame the chain drew, exactly as before the
    // furniture went on, including the row of pixels just past the bank,
    // which is where a spill margin would land if the clip were missing.
    for &(x, y) in &[
        (BANK, 0),
        (BANK, WINDOW_H / 2),
        (BANK + 1, 400),
        (BANK + 2, (strip.rect.y + strip.rect.height / 2.0) as u32),
        (WINDOW_W / 2, WINDOW_H / 2),
        (WINDOW_W - 1, WINDOW_H - 1),
    ] {
        assert_eq!(
            frame[px_index(WINDOW_W, x, y)],
            SENTINEL,
            "the furniture reached ({x},{y}), which is the glass's"
        );
    }
}

/// The switchboard over tape: a shell with no plate at all, and a label whose
/// letters are on the strip.
#[test]
fn the_tape_shell_stamps_its_label_and_screws_on_no_plate() {
    let mut cfg = Config::default();
    cfg.chassis.shell = config::Shell::Switchboard;
    cfg.chassis.channel_display = config::ChannelDisplay::Tape;
    cfg.chassis.channel_indicator = config::ChannelIndicator::Switch;
    let cabinet = Cabinet::from_config(&cfg, f64::from(WINDOW_W), f64::from(WINDOW_H));
    let bank = cabinet.bank_width();
    let pieces = cabinet.furniture(&titled(4, "channel-1"));
    // The switchboard shell has no plate region on the casting and no
    // screws on it either, so the first piece is already a row. This shell's
    // row is four: its own `plate_metal` plate, the painting that stands on it
    // (rivets, numeral, switch well), the tape kit's painted slot, and the
    // label lying in it.
    assert_eq!(
        pieces[..4].iter().map(|p| p.pass).collect::<Vec<_>>(),
        vec![
            chassis::Pass::Plate,
            chassis::Pass::Painted,
            chassis::Pass::Painted,
            chassis::Pass::TapeLabel
        ]
    );

    let gpu = Locked::new().expect("headless wgpu device");
    let mut column = Chrome::new(&gpu.device, gpu::harness::OUTPUT_FORMAT);
    let frame = frame_with_furniture(
        &gpu,
        &mut column,
        bank,
        1.0,
        &cabinet.chassis_params(),
        &pieces,
    );

    // The tape body: the tape kit's own plastic colour, in the blank end pad
    // where no glyph box reaches.
    let label = &pieces[3];
    let tape = chassis::displays::tape::tape_color();
    let body = frame[px_index(
        WINDOW_W,
        label.rect.x as u32 + 2,
        (label.rect.y + label.rect.height / 2.0) as u32,
    )];
    assert!(
        (body[0] - tape.r).abs() < 0.12 && (body[2] - tape.b).abs() < 0.12,
        "the tape body reads {:?}, not {tape:?}",
        &body[0..3]
    );

    // The letters: somewhere inside the glyph rectangle the label is much
    // brighter than the tape it is punched into. Scanning it is the honest
    // form: a single hand-picked pixel would depend on which glyph the
    // placeholder title happens to be.
    let letter = chassis::displays::tape::letter_color();
    let glyph = tape_params(label).glyph_rect_px;
    let gx = label.rect.x + f64::from(glyph[0]);
    let gy = label.rect.y + f64::from(glyph[1]);
    let gw = f64::from(glyph[2]);
    let gh = f64::from(glyph[3]);
    assert!(gw > 1.0 && gh > 1.0, "the label has no glyph box");
    let raster = label.source.as_ref().expect("a punched raster");
    let inked = raster
        .rgba
        .iter()
        .skip(3)
        .step_by(4)
        .filter(|&&a| a > 0)
        .count();
    assert!(
        inked > 0,
        "the punch wheel struck nothing: raster {}x{}, glyph box {gw}x{gh} at ({gx},{gy}) \
         in a {}x{} label",
        raster.width,
        raster.height,
        label.rect.width,
        label.rect.height
    );
    // "Struck" is anything at least halfway from the tape to the letter face
    // `tape_label` mixes to. The upper end is left open on purpose: the
    // shader's sheen is specular and runs past 1.0, which an 8-bit swapchain
    // clamps and this float measurement rig does not.
    let face = letter.r * 0.74;
    let struck_at = (tape.r + face) / 2.0;
    let mut struck = 0;
    let mut total = 0;
    for y in 0..gh as u32 {
        for x in 0..gw as u32 {
            let px = frame[px_index(WINDOW_W, gx as u32 + x, gy as u32 + y)];
            total += 1;
            if px[0] >= struck_at {
                struck += 1;
            }
        }
    }
    assert!(
        struck > 0,
        "nothing in the glyph box reached {struck_at}; box {gw}x{gh} at ({gx},{gy}), \
         label {}x{} at ({},{}), raster {}x{} with {inked} inked texels",
        label.rect.width,
        label.rect.height,
        label.rect.x,
        label.rect.y,
        raster.width,
        raster.height,
    );
    // ...and it is a letter rather than a flood: the wheel leaves most of the
    // box unstruck, the way a numeral in a box of its own advance does.
    assert!(
        struck < total / 2,
        "{struck} of {total} pixels in the glyph box are struck, which is a fill, \
         not a letter"
    );
}

#[test]
fn a_hidden_chassis_draws_no_column_at_all() {
    // A hidden chassis takes the bank with it, so this is not a bank of zero
    // width but no bank: the well takes the whole window and nothing is
    // composited over it.
    let mut cfg = Config::default();
    cfg.general.chassis_shown = false;
    let cabinet = Cabinet::from_config(&cfg, f64::from(WINDOW_W), f64::from(WINDOW_H));
    assert_eq!(cabinet.bank_width(), 0);
    assert!(!cabinet.is_shown());

    let gpu = Locked::new().expect("headless wgpu device");
    let mut column = Chrome::new(&gpu.device, gpu::harness::OUTPUT_FORMAT);

    let frame = frame_with_column(&gpu, &mut column, 0, 1.0, &cabinet.chassis_params());
    for &(x, y) in &[(0u32, 0u32), (1, 1), (WINDOW_W / 2, WINDOW_H / 2)] {
        assert_eq!(
            frame[px_index(WINDOW_W, x, y)],
            SENTINEL,
            "something was drawn at ({x},{y}) for a chassis that is not shown"
        );
    }
}
