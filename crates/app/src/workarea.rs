//! How wide the desktop actually is, panels and docks taken off.
//!
//! One caller, one moment: [`crate::shell::Shell`] sizing the first window.
//! A window is mapped at the size it was created with, so a floor discovered
//! afterwards can only be applied as a resize the user watches happen. Once
//! the window stands, nothing here is consulted again: the cabinet fits its
//! bank to whatever width the window actually has, whoever chose it, so a
//! compositor that hands back a size of its own is answered by the fit and
//! not by a monitor being tracked.
//!
//! That is also why no monitor change is followed. A window dragged to a
//! smaller screen keeps its size until something resizes it, and if something
//! does, the resize is what the fit reacts to. Tracking the move would buy a
//! resize nobody asked for.
//!
//! # What it can know, and where it stops
//!
//! winit reports a monitor's own rectangle, straight from the XRandR CRTC on
//! X11, with no allowance for a panel's reserved strut. The value that does
//! carry the allowance is the EWMH property `_NET_WORKAREA`, set on the root
//! window by whatever draws the panel. So:
//!
//! 1. `_NET_WORKAREA` for the current desktop, intersected with the monitor
//!    the window will open on. Catches a dock, taskbar or top bar holding a
//!    screen edge, which is invisible to winit and to XRandR alike.
//! 2. The monitor's own size, when nothing published the property. That is
//!    every Wayland session, where panel space is an exclusive zone the
//!    compositor keeps to itself and no protocol asks after it, and an X11
//!    session under a window manager that sets no work area. A window may
//!    then open under a panel, and the compositor's first configure corrects
//!    it on Wayland.
//! 3. Nothing, when there is no monitor to ask: a headless winit, or a
//!    compositor reporting no output before the first surface. The caller
//!    keeps its own default size.

use winit::event_loop::ActiveEventLoop;

/// A rectangle in physical pixels: the unit winit's `MonitorHandle` and
/// `_NET_WORKAREA` are both measured in.
type Rect = (i64, i64, i64, i64);

/// The usable desktop for a window about to be created, in physical pixels.
///
/// `None` when no monitor answers, which leaves the caller its own default.
pub fn usable_size(event_loop: &ActiveEventLoop) -> Option<(u32, u32)> {
    let monitor = event_loop
        .primary_monitor()
        .or_else(|| event_loop.available_monitors().next())?;
    let size = monitor.size();
    let position = monitor.position();
    let screen: Rect = (
        i64::from(position.x),
        i64::from(position.y),
        i64::from(size.width),
        i64::from(size.height),
    );

    let usable = work_area().and_then(|area| intersection(area, screen));
    let (_, _, width, height) = usable.unwrap_or(screen);
    Some((width.max(0) as u32, height.max(0) as u32))
}

/// The rectangle common to both, or `None` when they do not overlap.
///
/// `_NET_WORKAREA` is one rectangle spanning the whole X screen rather than
/// one per monitor, so on a desk with two screens it is wider than the one
/// the window opens on and has to be cut down to it.
fn intersection(a: Rect, b: Rect) -> Option<Rect> {
    let x = a.0.max(b.0);
    let y = a.1.max(b.1);
    let right = (a.0 + a.2).min(b.0 + b.2);
    let bottom = (a.1 + a.3).min(b.1 + b.3);
    (right > x && bottom > y).then_some((x, y, right - x, bottom - y))
}

/// The quadruple for one desktop out of `_NET_WORKAREA`'s flat list.
///
/// The property is four CARDINALs per desktop, in desktop order. A short
/// list, or an index past its end, is a window manager mid-change and reads
/// as no answer rather than as a guess at the neighbouring desktop's.
fn quadruple(values: &[u32], desktop: usize) -> Option<Rect> {
    let base = desktop.checked_mul(4)?;
    let quad = values.get(base..base + 4)?;
    Some((
        i64::from(quad[0]),
        i64::from(quad[1]),
        i64::from(quad[2]),
        i64::from(quad[3]),
    ))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn work_area() -> Option<Rect> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt};

    let (conn, screen_num) = x11rb::connect(None).ok()?;
    let root = conn.setup().roots.get(screen_num)?.root;

    // `only_if_exists`, so an atom that no window manager has ever interned
    // comes back as zero and the read stops here rather than asking for a
    // property that cannot exist.
    let work_area = conn
        .intern_atom(true, b"_NET_WORKAREA")
        .ok()?
        .reply()
        .ok()?
        .atom;
    if work_area == 0 {
        return None;
    }
    let current = conn
        .intern_atom(true, b"_NET_CURRENT_DESKTOP")
        .ok()?
        .reply()
        .ok()?
        .atom;

    let desktop = if current == 0 {
        0
    } else {
        conn.get_property(false, root, current, AtomEnum::CARDINAL, 0, 1)
            .ok()?
            .reply()
            .ok()?
            .value32()
            .and_then(|mut v| v.next())
            .unwrap_or(0) as usize
    };

    let length = 4 * (desktop as u32 + 1);
    let values: Vec<u32> = conn
        .get_property(false, root, work_area, AtomEnum::CARDINAL, 0, length)
        .ok()?
        .reply()
        .ok()?
        .value32()?
        .collect();
    quadruple(&values, desktop)
}

#[cfg(not(all(unix, not(target_os = "macos"))))]
fn work_area() -> Option<Rect> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_work_area_inside_one_screen_of_two_is_cut_to_that_screen() {
        // The property spans both screens; the window opens on the right-hand
        // one, whose own rectangle starts at 1920.
        let area = (0, 27, 3840, 1053);
        let screen = (1920, 0, 1920, 1080);
        assert_eq!(intersection(area, screen), Some((1920, 27, 1920, 1053)));
    }

    #[test]
    fn a_dock_on_the_left_takes_its_width_off() {
        assert_eq!(
            intersection((64, 0, 1856, 1080), (0, 0, 1920, 1080)),
            Some((64, 0, 1856, 1080))
        );
    }

    #[test]
    fn screens_that_do_not_meet_answer_nothing() {
        assert_eq!(intersection((0, 0, 800, 600), (1920, 0, 1920, 1080)), None);
        // Touching along an edge is not an overlap: a rectangle of no width
        // is not a desktop.
        assert_eq!(intersection((0, 0, 1920, 1080), (1920, 0, 800, 600)), None);
    }

    #[test]
    fn the_desktops_own_quadruple_is_the_one_read() {
        let values = [0, 0, 1920, 1080, 64, 27, 1856, 1053];
        assert_eq!(quadruple(&values, 0), Some((0, 0, 1920, 1080)));
        assert_eq!(quadruple(&values, 1), Some((64, 27, 1856, 1053)));
    }

    #[test]
    fn a_property_too_short_for_the_desktop_asked_for_answers_nothing() {
        assert_eq!(quadruple(&[0, 0, 1920, 1080], 1), None);
        assert_eq!(quadruple(&[], 0), None);
        assert_eq!(quadruple(&[0, 0, 1920], 0), None);
    }
}
