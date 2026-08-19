//! `robco-config`: RobCo Terminal settings (lib name `config`; the package
//! is named `robco-config` because a bare `config` package name is too
//! easily shadowed on crates.io, even though this crate is not published).
//!
//! The TOML config file is the single source of truth for settings; a key
//! absent from it means its built-in default, so the file is a diff
//! against defaults rather than a full dump of every setting. The rules
//! that follow from that are written up for third-party tool authors in
//! `docs/config-format.md`.
//!
//! The schema half models the settings that round-trip through the profile
//! JSON export format's four blobs (`_CURRENT_SETTINGS`, `_CURRENT_SCREEN`,
//! `_CURRENT_CHASSIS`, `_CUSTOM_PROFILES`). See `src/schema.rs` for the
//! boundary rule and the types, and `src/presets.rs` for the built-in
//! screen and chassis presets.
//!
//! The plumbing half (`src/io.rs`, `src/watch.rs`) is deliberately
//! schema-agnostic: it operates on `toml_edit::DocumentMut` and on
//! `T: serde::de::DeserializeOwned`, never on a concrete settings type.
//!
//! `_CUSTOM_PROFILES` (the user's saved screen+chassis pairs under a name)
//! is modelled in [`profile`] as the appliance split along two axes, with a
//! profile's own snapshot/modified equality, and JSON import/export against
//! that blob format. Rather than keeping the whole roster inside one JSON
//! value, the rebuild keeps one profile per TOML file beside the config
//! file, because the file is the source of truth here and a roster inside a
//! value would be a second store with its own rules.

pub mod io;
pub mod presets;
pub mod profile;
pub mod schema;
pub mod structural;
pub mod watch;

pub use profile::{Profile, Tuning};

pub use schema::{
    ChannelDisplay, ChannelIndicator, ChassisSettings, FontSource, GeneralSettings, Rasterization,
    ScreenSettings, Shell,
};

/// The three modelled settings blobs together: the shape a config file
/// written by this crate would take. Not itself a concept from the JSON
/// import format -- there these are three separate stored blobs -- but a
/// convenient unit for a single config file's `[general]` / `[screen]` /
/// `[chassis]` tables.
///
/// Deliberately excludes custom profiles; see the module comment.
///
/// `#[serde(default)]` here, and on each of `GeneralSettings`,
/// `ScreenSettings`, `ChassisSettings`, is what actually makes the
/// diff-against-defaults contract in `docs/config-format.md` hold for a
/// *partial* file, not just a missing one: without it, serde requires every
/// field of a struct present in the input, so a file containing only
/// `[general]\nfont_scaling = 2.0` would fail to deserialize entirely (an
/// `[general]` table missing ten other required keys), rather than filling
/// them in from `GeneralSettings::default()`. `read_document`/`deserialize`
/// in `io.rs` already handle a wholly-missing *file* correctly (empty
/// `DocumentMut` -> this container default fills in every field); this
/// annotation is what extends the same guarantee to a present-but-partial
/// table, key by key, which is the shape every real edit takes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub general: GeneralSettings,
    pub screen: ScreenSettings,
    pub chassis: ChassisSettings,
}

impl Config {
    /// Read a config file and resolve it: the two axes' `name` keys select
    /// their built-in preset as the base, and every other key present
    /// overrides it ([`profile::resolve_presets`]).
    ///
    /// This is the load path for *every* config file, the user's own and a
    /// `--profile` file alike, and it is one function on purpose. The same
    /// TOML text has to mean the same look wherever it sits: a rule that
    /// applied only to profile files would make a look change meaning when
    /// it was copied into `config.toml`, which is a quiet way to lose a
    /// user's settings.
    ///
    /// A missing file is the all-defaults config, per the
    /// diff-against-defaults contract; nothing here writes anything.
    pub fn load(path: &std::path::Path) -> Result<Config, io::ConfigError> {
        let mut doc = io::read_document(path)?;
        profile::resolve_presets(&mut doc);
        io::deserialize(doc)
    }
}

use serde::{Deserialize, Serialize};

/// The screen radius knob maps through `lint(4.0, 120.0, raw)` to pixels;
/// this pair is that range's one home.
pub const SCREEN_RADIUS_PX: (f64, f64) = (4.0, 120.0);

impl Config {
    /// While a chassis stands, its bezel is the frame; bare, the screen
    /// wears the moulding it came in. These four accessors are that
    /// split's one home: whichever of chassis or screen is showing
    /// governs, at the pointer's end and the picture's end alike.
    pub fn raw_frame_size(&self) -> f64 {
        if self.general.chassis_shown {
            self.chassis.frame_size
        } else {
            self.screen.frame_size
        }
    }

    pub fn raw_screen_radius(&self) -> f64 {
        if self.general.chassis_shown {
            self.chassis.screen_radius
        } else {
            self.screen.screen_radius
        }
    }

    pub fn raw_frame_color(&self) -> &str {
        if self.general.chassis_shown {
            &self.chassis.frame_color
        } else {
            &self.screen.frame_color
        }
    }

    pub fn raw_frame_shininess(&self) -> f64 {
        if self.general.chassis_shown {
            self.chassis.frame_shininess
        } else {
            self.screen.frame_shininess
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn general_settings_default_round_trips_through_toml() {
        let original = GeneralSettings::default();
        let toml_string = toml::to_string(&original).unwrap();
        let restored: GeneralSettings = toml::from_str(&toml_string).unwrap();
        assert_eq!(original, restored);
    }

    #[test]
    fn screen_settings_default_round_trips_through_toml() {
        let original = ScreenSettings::default();
        let toml_string = toml::to_string(&original).unwrap();
        let restored: ScreenSettings = toml::from_str(&toml_string).unwrap();
        assert_eq!(original, restored);
    }

    #[test]
    fn chassis_settings_default_round_trips_through_toml() {
        let original = ChassisSettings::default();
        let toml_string = toml::to_string(&original).unwrap();
        let restored: ChassisSettings = toml::from_str(&toml_string).unwrap();
        assert_eq!(original, restored);
    }

    #[test]
    fn config_default_round_trips_through_toml() {
        let original = Config::default();
        let toml_string = toml::to_string(&original).unwrap();
        let restored: Config = toml::from_str(&toml_string).unwrap();
        assert_eq!(original, restored);
    }

    /// The diff-against-defaults contract (`docs/config-format.md`) means a
    /// file naming only one key of one table must deserialize, filling
    /// every other field -- in that table and the two untouched tables --
    /// from the built-in defaults. This is what
    /// `#[serde(default)]` on `Config`/`GeneralSettings`/`ScreenSettings`/
    /// `ChassisSettings` buys: without it, a partial table is a hard
    /// deserialize error (serde requires every struct field present unless
    /// told otherwise).
    #[test]
    fn a_single_changed_key_deserializes_with_every_other_key_defaulted() {
        let partial = "[general]\nfont_scaling = 2.0\n";
        let restored: Config = toml::from_str(partial).unwrap();
        let mut expected = Config::default();
        expected.general.font_scaling = 2.0;
        assert_eq!(restored, expected);
    }

    /// An empty file (no tables at all) must also resolve to the frozen v1
    /// defaults -- the "zero configuration" launch experience.
    #[test]
    fn an_empty_file_resolves_to_every_default() {
        let restored: Config = toml::from_str("").unwrap();
        assert_eq!(restored, Config::default());
    }

    #[test]
    fn default_screen_is_default_amber() {
        // Startup loads this exact preset every first run, so it -- not just
        // the first list entry in the abstract -- is the schema default.
        assert_eq!(ScreenSettings::default().name, "Default Amber");
        assert_eq!(
            ScreenSettings::default(),
            presets::screen_presets()[0].clone()
        );
    }

    #[test]
    fn default_chassis_is_annunciator() {
        assert_eq!(ChassisSettings::default().name, "Annunciator");
        assert_eq!(
            ChassisSettings::default(),
            presets::chassis_presets()[0].clone()
        );
    }

    #[test]
    fn preset_counts_match_the_built_in_lists() {
        // Screens: 14 presets (Default Amber .. E-Ink).
        // Chassis: 3 presets (Annunciator, Slide Rule, Switchboard).
        assert_eq!(presets::screen_presets().len(), 14);
        assert_eq!(presets::chassis_presets().len(), 3);
    }

    #[test]
    fn every_screen_preset_round_trips_through_toml() {
        for preset in presets::screen_presets() {
            let toml_string = toml::to_string(&preset).unwrap();
            let restored: ScreenSettings = toml::from_str(&toml_string).unwrap();
            assert_eq!(
                preset, restored,
                "preset {:?} failed to round-trip",
                preset.name
            );
        }
    }

    #[test]
    fn every_chassis_preset_round_trips_through_toml() {
        for preset in presets::chassis_presets() {
            let toml_string = toml::to_string(&preset).unwrap();
            let restored: ChassisSettings = toml::from_str(&toml_string).unwrap();
            assert_eq!(
                preset, restored,
                "preset {:?} failed to round-trip",
                preset.name
            );
        }
    }
}
