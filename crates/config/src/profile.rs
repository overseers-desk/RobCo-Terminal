//! The profile model: an appliance split into two axes.
//!
//! A **profile** is a whole appliance: the screen behind the glass and the
//! chassis it stands in -- `{screen: ..., chassis: ...}` and nothing else.
//! The app-level settings ([`GeneralSettings`]) sit outside a profile: they
//! are the user's rather than a look's, being the strip width a hand dragged
//! the seam to and whether a cabinet is drawn at all. The face a cabinet
//! letters its bank in is the cabinet's, so it rides inside the profile with
//! the rest of the axis, and taking a look therefore re-fits the bank.
//!
//! # Preset base plus overrides
//!
//! A preset is a full blob: every measure of a screen and a chassis is
//! named. The config file is a diff against defaults
//! (`docs/config-format.md`), so it carries the same fact the other way
//! round: the `name` key *selects the base*, and every other key present is
//! an override on top of it. [`crate::toml::resolve_presets`] is that rule,
//! applied to the document before it is deserialized, so
//!
//! ```toml
//! [screen]
//! name = "Deep Blue"
//! bloom = 0.9
//! ```
//!
//! is the Deep Blue preset with one measure moved, rather than the default
//! screen wearing Deep Blue's name. A file that names every measure (a saved
//! look, say) resolves to itself, since every key is an override; a `name`
//! matching no built-in preset (a look the user saved under their own name)
//! resolves against the schema default, which is what the file already
//! meant before this rule existed.
//!
//! A saved look is one profile per TOML file beside the config file
//! ([`save_to`]), because the file is the source of truth here and a roster
//! inside one value would be a second store with its own rules.

use std::path::Path;

use crate::schema::{ChassisSettings, ScreenSettings};
use crate::toml::{self, ConfigError, Scalar};
use crate::Config;

/// A whole appliance: the screen in the chassis it stands in.
///
/// This is the shape a user's saved profile takes and the shape the session
/// restores.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize)]
pub struct Profile {
    pub screen: ScreenSettings,
    pub chassis: ChassisSettings,
}

impl Profile {
    /// The two axes of a resolved [`Config`], which is where a running
    /// appliance's look lives.
    pub fn from_config(config: &Config) -> Self {
        Profile {
            screen: config.screen.clone(),
            chassis: config.chassis.clone(),
        }
    }

    /// Stand this appliance up in `config`, leaving the app-level settings
    /// alone: this moves exactly these two axes.
    pub fn apply_to(&self, config: &mut Config) {
        config.screen = self.screen.clone();
        config.chassis = self.chassis.clone();
    }
}

/// Write `profile` into the TOML file at `path` as its `[screen]` and
/// `[chassis]` tables, atomically, in one edit.
///
/// This is the profile save, and it is the one place the writer had to
/// grow: `toml::write_key` moves a single key, and an appliance is every
/// measure of both axes, which as single-key writes would be that many
/// reloads and chances for a half-applied look to be observed.
/// [`toml::write_keys`] is that widening and no more -- one document, one
/// atomic rename, and still nothing but a closed list of named scalars, so
/// there is no general mutation path into the file.
///
/// Every measure is written, not just the ones that differ from the preset
/// the profile names: a saved profile is a complete statement of a look,
/// so it must not shift underneath the user when a later version changes a
/// default. The general settings are not touched, because they are not
/// part of a profile. The schema's own serialization supplies the keys and
/// their spellings, so the config file's dotted keys carry no field list
/// here.
pub fn save_to(path: &Path, profile: &Profile) -> Result<(), ConfigError> {
    let mut keys = Vec::new();
    for (axis, settings) in [
        ("screen", serde_json::to_value(&profile.screen)),
        ("chassis", serde_json::to_value(&profile.chassis)),
    ] {
        let value = settings.expect("schema types serialize");
        let object = value.as_object().expect("an axis serializes as a table");
        for (key, value) in object {
            keys.push((format!("{axis}.{key}"), to_scalar(value)));
        }
    }
    toml::write_keys(path, &keys)
}

/// The TOML value for one serialized field. The schema's serde spelling is
/// already the config file's: snake_case keys, enums as words.
fn to_scalar(value: &serde_json::Value) -> Scalar {
    match value {
        serde_json::Value::String(s) => Scalar::String(s.clone()),
        serde_json::Value::Bool(b) => Scalar::Boolean(*b),
        serde_json::Value::Number(n) => Scalar::Float(round4(n.as_f64().unwrap_or(0.0))),
        other => Scalar::String(other.to_string()),
    }
}

/// A real as the file keeps it: four decimals, so a save does not carry a
/// slider's float noise into the document.
fn round4(value: f64) -> f64 {
    if !value.is_finite() {
        return value;
    }
    format!("{value:.4}").parse().unwrap_or(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presets;
    use crate::schema::{ChannelDisplay, Rasterization};

    fn deep_blue() -> ScreenSettings {
        presets::screen_presets()
            .into_iter()
            .find(|p| p.name == "Deep Blue")
            .expect("Deep Blue is a built-in screen preset")
    }

    fn slide_rule() -> ChassisSettings {
        presets::chassis_presets()
            .into_iter()
            .find(|p| p.name == "Slide Rule")
            .expect("Slide Rule is a built-in chassis preset")
    }
    /// A saved profile is a complete statement of a look: every measure,
    /// written in one atomic edit, and nothing outside the two axes.
    #[test]
    fn saving_writes_both_axes_whole_and_nothing_else() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.workshop.toml");

        let mut profile = Profile {
            screen: deep_blue(),
            chassis: slide_rule(),
        };
        profile.screen.bloom = 0.4321;
        profile.screen.rasterization = Rasterization::ScanlineRasterization;
        profile.chassis.channel_display = ChannelDisplay::Tape;

        save_to(&path, &profile).expect("saving a profile should succeed");

        let config = crate::Config::load(&path).expect("a saved profile should load back");
        assert_eq!(config.screen, profile.screen);
        assert_eq!(config.chassis, profile.chassis);
        // The general settings are not a profile's to write.
        assert_eq!(config.general, crate::GeneralSettings::default());
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            !text.contains("[general]"),
            "a profile save must not grow a general table: {text}"
        );
    }

    /// The save is a settings write like any other, so it keeps the config
    /// contract: a comment and an unknown key beside it survive.
    #[test]
    fn saving_preserves_comments_and_keys_it_does_not_own() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.workshop.toml");
        std::fs::write(
            &path,
            "# my workshop look\n[general]\nled_characters = 20\n\n[future]\nkey = \"kept\"\n",
        )
        .unwrap();

        save_to(&path, &Profile::default()).unwrap();

        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("# my workshop look"));
        assert!(text.contains("led_characters = 20"));
        assert!(text.contains("key = \"kept\""));
    }

    /// Save, then load: the look a user keeps is the look they get back.
    #[test]
    fn a_saved_look_reloads_as_itself() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.workshop.toml");

        let mut profile = Profile {
            screen: deep_blue(),
            chassis: slide_rule(),
        };
        profile.screen.jitter = 0.6543;
        profile.chassis.frame_color = "#123456".to_string();

        save_to(&path, &profile).unwrap();

        let reloaded = Profile::from_config(&crate::Config::load(&path).unwrap());
        assert_eq!(reloaded, profile);
    }
}
