//! Settings schema for the terminal's three persisted configuration groups.
//!
//! The schema boundary follows the JSON import format's own three top-level
//! groups, which are the single place that says what is persisted:
//!
//! - [`GeneralSettings`] mirrors the `_CURRENT_SETTINGS` storage key.
//! - [`ScreenSettings`] mirrors the `_CURRENT_SCREEN` storage key.
//! - [`ChassisSettings`] mirrors the `_CURRENT_CHASSIS` storage key.
//!
//! Everything else in that format -- values derived from other properties
//! (e.g. a frame color computed from a hex string, a screen radius computed
//! from a slider) -- is runtime/derived state, not schema, and is
//! deliberately not modelled here.
//!
//! `_CUSTOM_PROFILES` (the user's saved screen+chassis pairs) is
//! deliberately deferred; see the module comment in `lib.rs`.

use serde::{Deserialize, Serialize};

/// The `_CURRENT_SETTINGS` blob. These are the app-level knobs that stand
/// apart from any screen or chassis -- window chrome, performance budget,
/// and the three properties (`ledCharacters`, `chassisShown`, `ledFontName`)
/// that belong to "the user" rather than to a profile, because switching a
/// look must not re-fit the LED bank.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralSettings {
    /// How many frames the shader-effect chain skips between redraws.
    pub effects_frame_skip: i32,
    pub window_scaling: f64,
    pub show_terminal_size: bool,
    pub font_scaling: f64,
    /// Carried for schema parity with the frozen v1 shape; this build has
    /// no menubar and nothing reads the key (docs/config.md, `[general]`).
    /// Wiring it is a deliberately open design fork, not a gap in this
    /// port.
    pub show_menubar: bool,
    pub bloom_quality: f64,
    pub burn_in_quality: f64,
    /// Carried for schema parity; nothing reads it here (docs/config.md,
    /// `[general]`). `--program`/`-e` are this build's way to run something
    /// other than the shell. Wiring it is a deliberately open design fork.
    pub use_custom_command: bool,
    /// Carried for schema parity alongside `use_custom_command`; nothing
    /// reads it (docs/config.md, `[general]`).
    pub custom_command: String,
    /// Visible characters in the channel bank strip. The user's own
    /// setting, not a profile's.
    pub led_characters: i32,
    /// Whether the chassis casting/bezel/bank is drawn around the screen at
    /// all. The user's own setting, not a profile's.
    pub chassis_shown: bool,
    /// The face the channel bank's windows are lettered in. The user's own
    /// setting, not a profile's.
    pub led_font_name: String,
}

/// Which family of rasterization the shader pipeline applies to glyphs,
/// stored as the screen's `rasterization` integer. Variant names mirror the
/// stored integers' own spelling so the TOML value reads the same word.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rasterization {
    NoRasterization,
    ScanlineRasterization,
    PixelRasterization,
    SubpixelRasterization,
    ModernRasterization,
}

/// Where the glyph font comes from, stored as the screen's `fontSource`
/// integer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FontSource {
    BundledFonts,
    SystemFonts,
}

/// The `_CURRENT_SCREEN` blob. "Everything behind the glass": phosphor,
/// geometry, type, the effects that age them, and the moulding the tube
/// itself came out of the factory in (used as the frame when no chassis is
/// shown). A screen says nothing about the cabinet it may be mounted in, so
/// any screen can pair with any [`ChassisSettings`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScreenSettings {
    /// Identity, not a measure. Loading an unnamed screen object falls back
    /// to the default screen's name, never to whatever name was standing
    /// (see `loadScreenObject`).
    pub name: String,
    /// Hex color string, e.g. `"#000000"`, stored under the key
    /// `backgroundColor`.
    pub background_color: String,
    /// Hex color string, stored under the key `fontColor`.
    pub font_color: String,
    pub flickering: f64,
    pub horizontal_sync: f64,
    pub static_noise: f64,
    pub chroma_color: f64,
    pub saturation_color: f64,
    pub screen_curvature: f64,
    pub glowing_line: f64,
    pub burn_in: f64,
    pub bloom: f64,
    pub rasterization: Rasterization,
    pub jitter: f64,
    pub rgb_shift: f64,
    pub brightness: f64,
    pub contrast: f64,
    pub ambient_light: f64,
    pub window_opacity: f64,
    pub font_name: String,
    pub font_source: FontSource,
    pub font_width: f64,
    pub line_spacing: f64,
    pub margin: f64,
    /// Carried for schema parity with the frozen v1 shape; the cursor does
    /// not blink in this build regardless of the value (docs/config.md,
    /// `[screen]`). Wiring it is a deliberately open design fork.
    pub blinking_cursor: bool,
    /// The screen's own moulding, stored under the key `frameSize`, used as
    /// the frame only when no chassis is shown.
    pub frame_size: f64,
    pub screen_radius: f64,
    pub frame_color: String,
    pub frame_shininess: f64,
}

/// How the chassis marks which channel is on screen: one of `"glow"` /
/// `"pointer"` / `"switch"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelIndicator {
    Glow,
    Pointer,
    Switch,
}

/// What the channel bank's windows are made of: one of `"led"` / `"tape"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelDisplay {
    Led,
    Tape,
}

/// Which component kit paints the chassis body: one of `"annunciator"` /
/// `"slide-rule"` / `"switchboard"`. Kebab-case matches the stored string
/// literals exactly (`"slide-rule"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Shell {
    Annunciator,
    SlideRule,
    Switchboard,
}

/// The `_CURRENT_CHASSIS` blob. "The cabinet the screen is mounted in": its
/// casting, its bezel, and the way its bank marks the channel on air.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ChassisSettings {
    /// Identity, not a measure. Loading an unnamed chassis object falls
    /// back to the default chassis's name (see `loadChassisObject`).
    pub name: String,
    pub shell: Shell,
    pub channel_indicator: ChannelIndicator,
    pub channel_display: ChannelDisplay,
    pub frame_size: f64,
    pub screen_radius: f64,
    pub frame_color: String,
    pub frame_shininess: f64,
}

impl Default for GeneralSettings {
    /// The values in force before a screen/chassis preset is loaded over
    /// them. Unlike screen/chassis, general settings have no preset list to
    /// fall back to, so these are the real default.
    fn default() -> Self {
        GeneralSettings {
            effects_frame_skip: 3,
            window_scaling: 1.0,
            show_terminal_size: true,
            font_scaling: 1.0,
            show_menubar: false,
            bloom_quality: 0.5,
            burn_in_quality: 0.5,
            use_custom_command: false,
            custom_command: String::new(),
            led_characters: 12,
            chassis_shown: true,
            led_font_name: "UNSCII_8_SCALED".to_string(),
        }
    }
}

impl Default for ScreenSettings {
    /// The "Default Amber" preset, first entry of the built-in screen list
    /// and the terminal's default screen name. Startup loads this screen on
    /// every first start (and on `--default-settings`), so it is the screen
    /// the terminal actually ships wearing, not just the first list entry.
    fn default() -> Self {
        crate::presets::screen_presets()
            .into_iter()
            .next()
            .expect("screen_presets() always yields at least Default Amber")
    }
}

impl Default for ChassisSettings {
    /// The "Annunciator" preset, first entry of the built-in chassis list
    /// and the terminal's default chassis name. Startup loads this chassis
    /// on every first start.
    fn default() -> Self {
        crate::presets::chassis_presets()
            .into_iter()
            .next()
            .expect("chassis_presets() always yields at least Annunciator")
    }
}
