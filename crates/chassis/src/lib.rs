//! The chassis: the cabinet the curved screen is set into.
//!
//! The appliance is a screen well with a column of furniture to its left
//! (the channel bank) and a metal casting wrapped around both. This crate owns
//! that casting's geometry and the metal it is poured from. It owns no device
//! and draws nothing itself: it turns a window size and a profile into
//! rectangles and named shader uniforms, and whoever holds the wgpu device
//! mounts the passes.
//!
//! # What lives where
//!
//! - [`metrics`]: the two measure contracts. A shell declares its fixed
//!   furniture; a display declares how it quantises its own width.
//! - [`bank`]: the channel bank's measures, and the one number the rest of the
//!   window turns on: [`bank::BankGeometry::implicit_width`].
//! - [`layout`]: how the window divides into the bank's column and the screen
//!   well, and the screen-scale normalisation every curvature-bearing uniform
//!   is measured in.
//! - [`seam`]: the drag on the boundary between the two, which resizes the
//!   screen region in whole-character steps.
//! - [`frame`]: the bezel's and the chassis metal's uniform sets, and the
//!   derivations that sit between a stored setting and a shader.
//! - [`oracle`]: CPU forms of the three metals' closed-form math, the
//!   independent-formula side of their per-pass tests.
//! - [`displays`]: the two channel-display kits, LED and tape: the raster
//!   each one composes over `term::fonts::led`'s proven glyphs, and the
//!   appearance mapping that colours it.
//! - [`shells`]: the three shells themselves: each one's fixed geometry and
//!   its drawing recipe, i.e. which regions it paints and with which of the
//!   three metals.
//!
//! `shaders/metal/` carries the three procedural metals: `frame_metal` (the
//! bezel), `chassis_metal` (the casting under the bank) and `plate_metal` (the
//! raised plate a shell screws over it). Each ships its own `.slangp` and
//! mounts standalone. [`shaders`] compiles the three sources in, for a host
//! that writes its own preset and has no source tree to read one from.
//! `shaders/led_matrix/` and `shaders/tape_label/` are the two channel
//! displays' passes, on the same terms; both arrived from `crt-render` with
//! the metals, since the chain stops at the glass and these paint chrome.
//!
//! # The order the numbers come in
//!
//! Nothing here is a binding graph, so the composition order is worth stating
//! once. Every window size, and every settings change that touches any of it:
//!
//! 1. Build the display's [`metrics::DisplayMetrics`] from the profile.
//! 2. Build the [`bank::BankGeometry`] from shell metrics, display metrics, the
//!    character count and the indicator law. Its `implicit_width` is the bank's
//!    footprint.
//! 3. Build the [`layout::WindowLayout`] from the window size and that width.
//! 4. Draw: [`frame::frame_params`] over the well, [`frame::chassis_params`]
//!    over the bank column.
//! 5. Hand `implicit_width` to `app::shell::Shell::set_bank_width`, so the
//!    window's minimum-size hint follows.
//!
//! A seam drag re-enters at step 1 with a new character count. See [`seam`] for
//! why that loop is stable rather than drifting.
//!
//! [`cabinet::Cabinet`] is that whole order assembled, shaped so an
//! `app::shell::Surface` forwards to it method for method. Everything under it
//! stays usable alone: the geometry, the layout and the seam are plain values
//! and pure functions, testable with no window and no device.
//!
//! # Where the chassis stops
//!
//! Chassis chrome lives outside the CRT chain and composites over the
//! glass. That is why this crate is meant to depend on no part of `crt`, not
//! even for the two small functions its [`oracle`] and `crt::oracle` both
//! carry, which the shaders' own `.frag` files duplicate between themselves
//! for the same reason. The one piece that is genuinely in both worlds is
//! the bezel; [`frame`]'s module doc says what it is doing there. [`color`]
//! is the same trade made once for the whole crate: the two colour helpers
//! both halves of the chrome need, rather than a dependency on
//! `crt::color`.

pub mod bank;
pub mod cabinet;
pub mod color;
pub mod displays;
pub mod frame;
pub mod furniture;
pub mod js;
pub mod layout;
pub mod metrics;
pub mod params;
pub mod paint;
pub mod seam;
pub mod shaders;
pub mod shells;
pub mod strip;

pub use bank::{BankGeometry, ChannelIndicator};
pub use cabinet::{Cabinet, Display, SeamUpdate};
pub use furniture::{Pass, Piece};
pub use layout::{Rect, WindowLayout};
pub use metrics::{DisplayMetrics, LedMetrics, ShellMetrics, TapeMetrics};
pub use seam::{SeamContext, SeamCursor, SeamDrag};
pub use strip::{BankStrips, StripRow};

use config::Config;

/// The shell metrics the profile asks for.
pub fn shell_metrics(cfg: &Config) -> ShellMetrics {
    match cfg.chassis.shell {
        config::Shell::Annunciator => metrics::shells::annunciator(),
        config::Shell::SlideRule => metrics::shells::slide_rule(),
        config::Shell::Switchboard => metrics::shells::switchboard(),
    }
}

/// The bezel the profile asks for.
pub fn frame_style(cfg: &Config) -> frame::FrameStyle {
    match cfg.chassis.shell {
        config::Shell::Annunciator => frame::styles::annunciator_frame(),
        config::Shell::SlideRule => frame::styles::slide_rule_frame(),
        config::Shell::Switchboard => frame::styles::switchboard_frame(),
    }
}

/// The chassis surface the profile asks for.
pub fn chassis_style(cfg: &Config) -> frame::ChassisStyle {
    match cfg.chassis.shell {
        config::Shell::Annunciator => frame::styles::annunciator_chassis(),
        config::Shell::SlideRule => frame::styles::slide_rule_chassis(),
        config::Shell::Switchboard => frame::styles::switchboard_chassis(),
    }
}

/// The display kit the profile asks for, measured off the bundled font stack.
///
/// This is the one function in the crate that touches a font. Everything else
/// takes the measured kit as a value, which is what keeps the geometry testable
/// with no font stack at all; this is where a *profile* becomes such a value,
/// so a host does not have to know which of the two kits reads the user's lamp
/// font and which carries its own.
pub fn display_kit(cfg: &Config) -> Display {
    match cfg.chassis.channel_display {
        config::ChannelDisplay::Led => Display::Led(led_metrics(&cfg.general.led_font_name)),
        config::ChannelDisplay::Tape => Display::Tape(tape_metrics()),
    }
}

/// The LED cell the profile's lamp font implies: `max(1, round(advance_width))`
/// by `max(1, ceil(height))`, taken off the advance and scaled height of
/// `"M"` at the face's own catalogue pixel size ([`displays::led::cell_metrics`],
/// over the proven 26.6 path). The four remaining measures are the
/// settings' own defaults.
///
/// A `general.led_font_name` naming no bundled face falls back to the
/// shipped default rather than refusing to measure: a hand-edited config is
/// not a reason to have no bank.
pub fn led_metrics(font_name: &str) -> LedMetrics {
    let entry = term::fonts::font_by_name(font_name)
        .or_else(|| term::fonts::font_by_name(displays::led::DEFAULT_LED_FONT_NAME))
        .expect("the bundled catalogue always carries the default lamp font");
    let (cell_width, cell_height) = displays::led::cell_metrics(entry.data(), entry.pixel_size);
    LedMetrics {
        cell_width: cell_width as i32,
        cell_height: cell_height as i32,
        dot_pitch: displays::led::LED_DOT_PITCH,
        min_characters: displays::led::MIN_LED_CHARACTERS as i32,
        pad_cells: displays::led::LED_PAD_CELLS as i32,
        side_pad_cells: displays::led::LED_SIDE_PAD_CELLS as i32,
    }
}

/// The punch wheel's one letter size, so this reads no profile: the advance
/// of `"M"` in Departure Mono at 20 px, the 12 px of blank tape at either
/// end, and the same character floor the LED strip takes.
pub fn tape_metrics() -> TapeMetrics {
    let entry = term::fonts::font_by_name(displays::tape::FONT_NAME)
        .expect("the bundled catalogue always carries the tape's own face");
    TapeMetrics {
        unit_width: displays::tape::metrics::unit_width(entry.data()),
        end_pad: displays::tape::END_PAD as i32,
        min_characters: displays::tape::metrics::min_units() as i32,
    }
}

/// How the profile marks the channel on screen.
pub fn channel_indicator(cfg: &Config) -> ChannelIndicator {
    match cfg.chassis.channel_indicator {
        config::ChannelIndicator::Pointer => ChannelIndicator::Pointer,
        config::ChannelIndicator::Switch => ChannelIndicator::Switch,
        config::ChannelIndicator::Glow => ChannelIndicator::Glow,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shell_the_config_can_name_has_metrics_a_bezel_and_a_casting() {
        // The enum is closed, so this is a match-exhaustiveness check with the
        // values attached: a fourth shell added to `robco-config` fails to
        // compile here rather than falling back to the amber one at run time.
        for shell in [
            config::Shell::Annunciator,
            config::Shell::SlideRule,
            config::Shell::Switchboard,
        ] {
            let mut cfg = Config::default();
            cfg.chassis.shell = shell;
            let m = shell_metrics(&cfg);
            let f = frame_style(&cfg);
            let c = chassis_style(&cfg);
            // A shell that declares no measures would divide the bank by zero.
            assert!(m.min_row_height > 0, "{shell:?} has no row height");
            assert!(m.numeral_width > 0, "{shell:?} has no numeral column");
            assert!(f.outer_radius > 0.0, "{shell:?} has a square bezel");
            assert!(c.vignette_strength > 0.0, "{shell:?} has a flat casting");
            // The frame and the chassis are one casting, so both styles
            // read the same light and metal off the metrics.
            assert!(m.casting_light_dir[1] < 0.0, "{shell:?} is lit from below");
        }
    }

    #[test]
    fn the_stock_profile_measures_its_lamp_font_rather_than_a_fixture() {
        // `LedMetrics::default()`'s 5x8 cell is a fixture and says so; the
        // shipped profile's lamp font is UNSCII 8, whose "M" advances 8 px at
        // its own catalogue pixel size and whose scaled height ceils to 8.
        // The difference is the whole point of measuring: a bank built on the
        // fixture is 184 px wide and the appliance the user gets is 247.
        let cfg = Config::default();
        assert_eq!(cfg.general.led_font_name, "UNSCII_8_SCALED");
        let led = led_metrics(&cfg.general.led_font_name);
        assert_eq!((led.cell_width, led.cell_height), (8, 8));
        assert_eq!(led.unit_width(), 12.0); // 8 * 1.5
        assert_eq!(led.width_for_units(12), 168); // round(8 * 14 * 1.5)
        assert_ne!(led, LedMetrics::default());

        // A name no bundled face answers to falls back rather than refusing.
        assert_eq!(led_metrics("NO_SUCH_FACE"), led);

        // Departure Mono at 20 px, the tape's one letter size, measured the
        // same way.
        let tape = tape_metrics();
        assert_eq!(tape.end_pad, 12);
        assert_eq!(tape.min_characters, 8);
        assert!(
            tape.unit_width > 5.0 && tape.unit_width < 20.0,
            "unit_width={}",
            tape.unit_width
        );

        // And the profile picks between them.
        assert_eq!(display_kit(&cfg), Display::Led(led));
        let mut tape_cfg = cfg.clone();
        tape_cfg.chassis.channel_display = config::ChannelDisplay::Tape;
        assert_eq!(display_kit(&tape_cfg), Display::Tape(tape));
    }

    #[test]
    fn a_cabinet_built_from_the_shipped_profile_alone_stands_247_px_wide() {
        // The whole constructor path, profile to cabinet, with no kit handed
        // in: the sum over the annunciator's own measures -- 3 (bank_padding)
        // + 46 (numeral_width) + 16 (column_gap) + 168 (twelve characters of
        // the measured cell) + 14 (right_padding). The glow indicator draws
        // no rail, so no lane is cut for one.
        let cfg = Config::default();
        let c = Cabinet::from_config(&cfg, 1024.0, 768.0);
        assert_eq!(c.bank_width(), 3 + 46 + 16 + 168 + 14);
        assert_eq!(c.bank_width(), 247);
        assert_eq!(c.layout().crt.width, 1024.0 - 247.0);
        assert_eq!(c.min_inner_size(), (247 + 320, 240));

        // A hidden chassis is no bank at all, and the well takes the window.
        let mut bare = cfg.clone();
        bare.general.chassis_shown = false;
        let c = Cabinet::from_config(&bare, 1024.0, 768.0);
        assert_eq!(c.bank_width(), 0);
        assert_eq!(c.layout().crt.width, 1024.0);
        assert_eq!(c.min_inner_size(), (320, 240));
    }

    #[test]
    fn a_reload_re_measures_the_kit_and_not_only_the_geometry() {
        // `general.led_font_name` moves the cell the strips are cut from, so a
        // reload that only re-applied the standing kit would keep the old
        // bank. Terminess's cell is 6 px against UNSCII's 8, which takes 42 px
        // off twelve characters of strip and the same 42 off the bank.
        let cfg = Config::default();
        let mut c = Cabinet::from_config(&cfg, 1024.0, 768.0);
        assert_eq!(c.bank_width(), 247);

        let mut narrow = cfg.clone();
        narrow.general.led_font_name = "TERMINESS_SCALED".to_string();
        let width = c.apply_config(&narrow);
        assert_eq!(width, c.bank_width());
        assert_eq!(width, 247 - 42);
        assert_eq!(
            c.bank_width(),
            Cabinet::from_config(&narrow, 1024.0, 768.0).bank_width()
        );
    }

    #[test]
    fn the_stock_profile_is_the_amber_appliance_over_led_strips_under_the_glow() {
        let cfg = Config::default();
        assert_eq!(cfg.chassis.shell, config::Shell::Annunciator);
        assert_eq!(cfg.chassis.channel_display, config::ChannelDisplay::Led);
        assert_eq!(channel_indicator(&cfg), ChannelIndicator::Glow);
        assert_eq!(cfg.general.led_characters, 12);
        assert!(cfg.general.chassis_shown);

        // And the whole composition, end to end, at this crate's own
        // default window size.
        let g = BankGeometry::new(
            &shell_metrics(&cfg),
            &LedMetrics::default(),
            cfg.general.led_characters,
            channel_indicator(&cfg),
        );
        let layout = WindowLayout::new(1024.0, 768.0, g.implicit_width as f64);
        assert_eq!(g.implicit_width, 184);
        assert_eq!(layout.crt.width, 840.0);
        assert_eq!(layout::min_inner_size(g.implicit_width), (504, 240));
    }
}
