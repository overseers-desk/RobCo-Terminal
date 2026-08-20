//! The profile model: an appliance split into two axes, snapshot equality
//! over a rendered JSON document, and JSON import/export against that same
//! document format.
//!
//! A **profile** is a whole appliance: the screen behind the glass and the
//! chassis it stands in -- `{screen: ..., chassis: ...}` and nothing else.
//! The twelve `_CURRENT_SETTINGS` keys ([`GeneralSettings`]) are
//! deliberately outside a profile: they are "the user's" rather than a
//! profile's, because switching a look must not re-fit the LED bank.
//!
//! Three things live here.
//!
//! # The snapshot, and the Modified flag
//!
//! Two strings are kept and compared: the appliance as it stands now,
//! rendered to JSON, and the appliance as it was last handed over, rendered
//! the same way. The flag is whether the two differ.
//!
//! The render is two-space indented with every real rounded to four
//! decimals. So the equality is over a *rendered document*, not over the
//! properties: two looks are the same look when their JSON text matches
//! with every real rounded to four decimals. [`Profile::snapshot`] produces
//! that text, and [`Tuning`] is the pair of strings plus the flag.
//!
//! The rounding is load-bearing, not incidental: a slider that moves by
//! less than 5e-5 leaves the Modified badge alone. See [`Profile::snapshot`]
//! for the field list the equality actually reads.
//!
//! # Preset base plus overrides
//!
//! A preset is a full blob: every measure of a screen and a chassis is
//! named, and picking a preset by index overwrites the name over
//! whatever was already selected, so "which preset was this struck from"
//! is carried by the selection rather than by the blob. The rebuild's
//! config file is a diff against defaults (`docs/config-format.md`), so it
//! carries the same fact the other way round: the `name` key *selects the
//! base*, and every other key present is an override on top of it.
//! [`crate::toml::resolve_presets`] is that rule, applied to the document
//! before it is deserialized, so
//!
//! ```toml
//! [screen]
//! name = "Deep Blue"
//! bloom = 0.9
//! ```
//!
//! is the Deep Blue preset with one measure moved, rather than the default
//! screen wearing Deep Blue's name. A file that names every measure (an
//! imported blob, say) resolves to itself, since every key is an override;
//! a `name` matching no built-in preset (a look the user saved under their
//! own name) resolves against the schema default, which is what the file
//! already meant before this rule existed.
//!
//! # JSON import and export
//!
//! Export takes the profile, adds `name` and `profileVersion`, and writes
//! it; import reads a file, requires `name`, checks `profileVersion`,
//! deletes both, and keeps the rest as the profile. [`export_json`] and
//! [`import_json`] are those two. The format keeps a fixed spelling
//! throughout: `camelCase` keys, `rasterization` and `fontSource` as
//! integers, colours as hex strings.

use std::path::Path;

use crate::schema::{ChassisSettings, ScreenSettings};
use crate::toml::{self, ConfigError, Scalar};
use crate::Config;

/// An imported profile carrying any other version is refused.
pub const PROFILE_VERSION: u32 = 1;

/// A whole appliance: the screen in the chassis it stands in.
///
/// This is the shape a user's saved profile takes, the shape the session
/// restores, and the shape the snapshot equality compares.
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

    /// Stand this appliance up in `config`, leaving the twelve general
    /// settings alone: this moves exactly these two axes and never touches
    /// `_CURRENT_SETTINGS`.
    pub fn apply_to(&self, config: &mut Config) {
        config.screen = self.screen.clone();
        config.chassis = self.chassis.clone();
    }

    /// The appliance as a JSON document, two-space indented, every real
    /// rounded to four decimals.
    ///
    /// This string *is* the equality. It reads the two axes exactly as the
    /// schema structs declare them ([`ScreenSettings`], [`ChassisSettings`]),
    /// in declaration order, spelled the JSON format's way: camelCase keys,
    /// `rasterization` and `fontSource` as integers. Nothing else. In
    /// particular the general settings are absent, so re-fitting the LED
    /// bank or hiding the chassis does not mark a profile modified.
    pub fn snapshot(&self) -> String {
        let mut out = String::new();
        write_value(&mut out, 0, &json_value(self));
        out
    }
}

/// The two strings kept for the equality, and the flag derived from them.
///
/// `loaded` is "the appliance as it was handed over". Every path that
/// *establishes* a look writes it -- a screen taken, a chassis taken, a
/// saved appliance loaded, the session restored, the defaults on a first
/// start, a look saved under a name -- and the difference between it and
/// the live snapshot is the user's own tuning since. Importing a profile
/// deliberately does not write it: an imported profile is somebody else's
/// work, it is appended to the roster rather than stood up, and it leaves
/// the mark standing.
#[derive(Debug, Clone, PartialEq)]
pub struct Tuning {
    loaded: String,
}

impl Tuning {
    /// The appliance was just handed over: the mark starts here.
    pub fn handed_over(profile: &Profile) -> Self {
        Tuning {
            loaded: profile.snapshot(),
        }
    }

    /// Re-establish the mark against `profile`. This is the same
    /// assignment as [`Self::handed_over`], named for the call sites that
    /// move an existing mark: `loadScreen`, `loadChassis`,
    /// `loadProfileObject`, and `saveCurrentAsProfile`.
    pub fn mark(&mut self, profile: &Profile) {
        self.loaded = profile.snapshot();
    }

    /// Whether the live snapshot has moved from the mark.
    pub fn is_modified(&self, profile: &Profile) -> bool {
        profile.snapshot() != self.loaded
    }

    /// The stored snapshot, for a caller that wants to show or diff it.
    pub fn loaded(&self) -> &str {
        &self.loaded
    }
}

/// A profile parsed out of an imported JSON file: the name the file
/// carried and the appliance under it.
#[derive(Debug, Clone, PartialEq)]
pub struct NamedProfile {
    pub name: String,
    pub profile: Profile,
}

/// Everything an import can refuse on.
#[derive(Debug)]
pub enum ProfileError {
    /// No `name`, or an empty one.
    Unnamed,
    /// The profile file names a version this build does not support.
    UnsupportedVersion { found: u64, expected: u32 },
    /// The bytes are not JSON, or not a JSON object of the right shape.
    Malformed(String),
}

impl std::fmt::Display for ProfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProfileError::Unnamed => write!(f, "profile doesn't have a name"),
            ProfileError::UnsupportedVersion { found, expected } => write!(
                f,
                "profile version {found} is not supported on this version \
                 of the terminal (expected {expected})"
            ),
            ProfileError::Malformed(why) => write!(f, "profile file is malformed: {why}"),
        }
    }
}

impl std::error::Error for ProfileError {}

/// The profile object with `name` and `profileVersion` added at the top
/// level, stringified two-space indented.
///
/// The two added keys come after `screen` and `chassis`, keeping a fixed
/// key order across every file this writes.
pub fn export_json(name: &str, profile: &Profile) -> String {
    let mut value = json_value(profile);
    let map = value
        .as_object_mut()
        .expect("a profile serializes as an object");
    map.insert("name".into(), serde_json::Value::from(name));
    map.insert("profileVersion".into(), serde_json::Value::from(PROFILE_VERSION));
    let mut out = String::new();
    write_value(&mut out, 0, &value);
    out
}

/// Require a name, check the version (absent means 1), drop both keys, and
/// read what is left as the appliance.
///
/// Every measure the file does not name falls back to the default
/// appliance's, never to whatever look happens to be standing. A blob is a
/// complete statement of a look, so a key it omits is a key it does not
/// have an opinion about, and the default is the only answer that does not
/// depend on when the import happened.
pub fn import_json(text: &str) -> Result<NamedProfile, ProfileError> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|e| ProfileError::Malformed(e.to_string()))?;
    let object = value
        .as_object()
        .ok_or_else(|| ProfileError::Malformed("top level is not a JSON object".to_string()))?;

    let name = object
        .get("name")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or(ProfileError::Unnamed)?
        .to_string();

    // `var version = profileObject.profileVersion !== undefined
    //      ? profileObject.profileVersion : 1`
    let version = match object.get("profileVersion") {
        None => u64::from(PROFILE_VERSION),
        Some(v) => v
            .as_u64()
            .ok_or_else(|| ProfileError::Malformed("profileVersion is not a number".to_string()))?,
    };
    if version != u64::from(PROFILE_VERSION) {
        return Err(ProfileError::UnsupportedVersion {
            found: version,
            expected: PROFILE_VERSION,
        });
    }

    Ok(NamedProfile {
        name,
        profile: Profile {
            screen: read_axis(object.get("screen"))?,
            chassis: read_axis(object.get("chassis"))?,
        },
    })
}

/// An axis the file does not carry (or carries as a non-object) is the
/// default axis; one it does carry is read with every omitted key falling
/// back to the default, per the schema structs' own `#[serde(default)]`.
/// A value of the wrong shape refuses the import.
fn read_axis<T: serde::de::DeserializeOwned + Default>(
    value: Option<&serde_json::Value>,
) -> Result<T, ProfileError> {
    match value {
        Some(value) if value.is_object() => {
            let mut value = value.clone();
            from_json_format(&mut value);
            serde_json::from_value(value).map_err(|e| ProfileError::Malformed(e.to_string()))
        }
        _ => Ok(T::default()),
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

/// The two enums the JSON format stores as integers, in code order, spelled
/// as the schema's serde (and so the config file) spells them. The pairing
/// of code to word is the JSON format's own fact and this is its one home;
/// the variants themselves live on the schema enums.
const RASTERIZATION_CODES: &[&str] = &[
    "no_rasterization",
    "scanline_rasterization",
    "pixel_rasterization",
    "subpixel_rasterization",
    "modern_rasterization",
];
const FONT_SOURCE_CODES: &[&str] = &["bundled_fonts", "system_fonts"];

/// The profile in the JSON format's shape -- camelCase keys, the two coded
/// enums as integers -- derived from the schema's own serialization, so the
/// schema structs are the only field list.
fn json_value(profile: &Profile) -> serde_json::Value {
    let mut value = serde_json::to_value(profile).expect("schema types serialize");
    to_json_format(&mut value);
    value
}

/// snake_case keys to camelCase, words to codes where the format is
/// numeric. Recursive over objects; the profile shape carries no arrays.
fn to_json_format(value: &mut serde_json::Value) {
    let serde_json::Value::Object(map) = value else {
        return;
    };
    let entries: Vec<(String, serde_json::Value)> = std::mem::take(map).into_iter().collect();
    for (key, mut value) in entries {
        to_json_format(&mut value);
        if let Some(codes) = coded_enum(&key) {
            if let Some(code) = value.as_str().and_then(|w| codes.iter().position(|c| *c == w)) {
                value = serde_json::Value::from(code as u64);
            }
        }
        map.insert(camel(&key), value);
    }
}

/// The inverse: camelCase keys to snake_case, codes back to words. A code
/// outside the table is dropped rather than kept, so the field falls back
/// to its default, the forgiveness imports have always shown out-of-range
/// codes.
fn from_json_format(value: &mut serde_json::Value) {
    let serde_json::Value::Object(map) = value else {
        return;
    };
    let entries: Vec<(String, serde_json::Value)> = std::mem::take(map).into_iter().collect();
    for (key, mut value) in entries {
        from_json_format(&mut value);
        let key = snake(&key);
        if let Some(codes) = coded_enum(&key) {
            match value.as_u64().and_then(|c| codes.get(c as usize)) {
                Some(word) => value = serde_json::Value::from(*word),
                None => continue,
            }
        }
        map.insert(key, value);
    }
}

fn coded_enum(snake_key: &str) -> Option<&'static [&'static str]> {
    match snake_key {
        "rasterization" => Some(RASTERIZATION_CODES),
        "font_source" => Some(FONT_SOURCE_CODES),
        _ => None,
    }
}

/// The JSON format spells keys in camelCase; the schema and the config
/// file spell them in snake_case. These two functions are the whole
/// translation.
fn snake(camel: &str) -> String {
    let mut out = String::with_capacity(camel.len() + 4);
    for ch in camel.chars() {
        if ch.is_ascii_uppercase() {
            out.push('_');
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

fn camel(snake: &str) -> String {
    let mut out = String::with_capacity(snake.len());
    let mut upper = false;
    for ch in snake.chars() {
        if ch == '_' {
            upper = true;
            continue;
        }
        out.push(if upper { ch.to_ascii_uppercase() } else { ch });
        upper = false;
    }
    out
}

/// `Number(val.toFixed(4))`. Rounding the decimal rendering rather than
/// scaling by 10000 is what `toFixed` does, and the two disagree on values
/// whose binary form sits just under a half-way point.
fn round4(value: f64) -> f64 {
    if !value.is_finite() {
        return value;
    }
    format!("{value:.4}").parse().unwrap_or(value)
}

/// A JSON number the way JavaScript writes one: no trailing `.0`, since
/// `JSON.stringify(1)` is `"1"`. Rust's own `f64` `Display` is already
/// shortest-round-trip and already drops the fraction on an integral
/// value, so the rounding is the only work.
fn write_number(out: &mut String, value: f64) {
    let value = round4(value);
    if value.is_finite() {
        out.push_str(&value.to_string());
    } else {
        // JSON has no infinity or NaN; `JSON.stringify` writes null.
        out.push_str("null");
    }
}

fn write_string(out: &mut String, value: &str) {
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// A value rendered the way `JSON.stringify(v, null, 2)` renders one:
/// two-space nesting, every real rounded to four decimals, no trailing
/// `.0` on integral values.
fn write_value(out: &mut String, indent: usize, value: &serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            let inner = " ".repeat(indent + 2);
            out.push_str("{\n");
            for (index, (key, value)) in map.iter().enumerate() {
                out.push_str(&inner);
                write_string(out, key);
                out.push_str(": ");
                write_value(out, indent + 2, value);
                if index + 1 < map.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            out.push_str(&" ".repeat(indent));
            out.push('}');
        }
        serde_json::Value::String(s) => write_string(out, s),
        serde_json::Value::Number(n) => write_number(out, n.as_f64().unwrap_or(f64::NAN)),
        serde_json::Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        serde_json::Value::Null => out.push_str("null"),
        serde_json::Value::Array(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                write_value(out, indent, item);
            }
            out.push(']');
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presets;
    use crate::schema::{ChannelDisplay, ChannelIndicator, FontSource, Rasterization, Shell};

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

    // THE SNAPSHOT EQUALITY //////////////////////////////////////////////

    /// The Modified flag fires when the rendered snapshot differs from the
    /// one that was handed over, and nothing else.
    #[test]
    fn tuning_is_unmodified_until_a_measure_moves() {
        let mut profile = Profile::default();
        let tuning = Tuning::handed_over(&profile);
        assert!(!tuning.is_modified(&profile));

        profile.screen.bloom += 0.25;
        assert!(tuning.is_modified(&profile));
    }

    /// Every path that re-establishes the mark restarts the tuning from
    /// there.
    #[test]
    fn marking_the_appliance_clears_the_flag() {
        let mut profile = Profile::default();
        let mut tuning = Tuning::handed_over(&profile);
        profile.chassis.frame_shininess = 0.9;
        assert!(tuning.is_modified(&profile));

        tuning.mark(&profile);
        assert!(!tuning.is_modified(&profile));
    }

    /// The general settings are outside the equality, because a profile
    /// never reads them. Re-fitting the LED bank or hiding the chassis is
    /// the user's business, not the profile's.
    #[test]
    fn general_settings_are_outside_the_equality() {
        let mut config = Config::default();
        let tuning = Tuning::handed_over(&Profile::from_config(&config));

        config.general.led_characters = 40;
        config.general.chassis_shown = false;
        config.general.font_scaling = 3.0;

        assert!(!tuning.is_modified(&Profile::from_config(&config)));
    }

    /// Both axes are inside it, which is what makes the model two-axis
    /// rather than a screen with decoration.
    #[test]
    fn both_axes_are_inside_the_equality() {
        let base = Profile::default();
        let tuning = Tuning::handed_over(&base);

        let mut screen_moved = base.clone();
        screen_moved.screen.jitter = 0.7;
        assert!(tuning.is_modified(&screen_moved));

        let mut chassis_moved = base.clone();
        chassis_moved.chassis.shell = Shell::Switchboard;
        assert!(tuning.is_modified(&chassis_moved));
    }

    /// Every serialized field of both axes is read by the snapshot:
    /// perturb each leaf of the schema's own serialization and check the
    /// flag fires. The schema structs are the only field list, so a field
    /// they gain is covered here without this test changing; a field the
    /// snapshot stopped reading would be a measure the Modified badge
    /// silently stopped noticing, which is the failure this test exists to
    /// prevent.
    #[test]
    fn every_field_moves_the_snapshot() {
        let base = Profile::default();
        let tuning = Tuning::handed_over(&base);
        // For word-enum fields a generic string perturbation would not
        // deserialize; each gets a valid variant differing from the default.
        let alternates: &[(&str, &str)] = &[
            ("shell", "switchboard"),
            ("channel_indicator", "switch"),
            ("channel_display", "tape"),
            ("rasterization", "scanline_rasterization"),
            ("font_source", "system_fonts"),
        ];
        let document = serde_json::to_value(&base).expect("profile serializes");
        for (axis, object) in document.as_object().expect("profile is an object") {
            for (key, leaf) in object.as_object().expect("axis is an object") {
                let mut moved = object.as_object().unwrap().clone();
                let perturbed = match leaf {
                    serde_json::Value::Number(n) => {
                        serde_json::Value::from(n.as_f64().unwrap() + 0.5)
                    }
                    serde_json::Value::Bool(b) => serde_json::Value::from(!b),
                    serde_json::Value::String(s) => {
                        match alternates.iter().find(|(k, _)| k == key) {
                            Some((_, alternate)) => serde_json::Value::from(*alternate),
                            None => serde_json::Value::from(format!("{s}x")),
                        }
                    }
                    other => panic!("unexpected leaf shape at {axis}.{key}: {other:?}"),
                };
                moved.insert(key.clone(), perturbed);
                let moved = serde_json::Value::Object(moved);
                let mut profile = base.clone();
                match axis.as_str() {
                    "screen" => profile.screen = serde_json::from_value(moved).expect(key),
                    "chassis" => profile.chassis = serde_json::from_value(moved).expect(key),
                    other => panic!("unexpected axis {other}"),
                }
                assert!(
                    tuning.is_modified(&profile),
                    "moving {axis}.{key} did not change the snapshot"
                );
            }
        }
    }

    /// `Number(val.toFixed(4))` in the snapshot's rendering: a move below
    /// the fifth decimal does not render, so it does not count as tuning.
    /// This is the rounding being load-bearing rather than cosmetic.
    #[test]
    fn a_move_below_the_fourth_decimal_is_not_a_modification() {
        let base = Profile::default();
        let tuning = Tuning::handed_over(&base);

        let mut nudged = base.clone();
        nudged.screen.bloom += 0.000_001;
        assert!(
            !tuning.is_modified(&nudged),
            "a sub-1e-4 move must round away, as the snapshot's own toFixed(4) rounds it away"
        );

        let mut moved = base.clone();
        moved.screen.bloom += 0.001;
        assert!(moved.snapshot() != base.snapshot());
    }

    /// The rendered document's exact shape: two-space indent, `camelCase`,
    /// integer-coded enums, and no trailing `.0` on a whole number.
    #[test]
    fn the_snapshot_renders_the_documented_json_layout() {
        let snapshot = Profile::default().snapshot();
        assert!(snapshot.starts_with("{\n  \"screen\": {\n    \"name\": \"Default Amber\","));
        assert!(snapshot.contains("\"rasterization\": 0"));
        assert!(snapshot.contains("\"fontSource\": 0"));
        assert!(
            snapshot.contains("\"fontWidth\": 1,"),
            "JSON.stringify(1) is \"1\", not \"1.0\": {snapshot}"
        );
        assert!(snapshot.contains("\"windowOpacity\": 1,"));
        assert!(snapshot.contains("\"blinkingCursor\": false"));
        assert!(snapshot.contains("\"chassis\": {\n    \"name\": \"Annunciator\","));
        assert!(snapshot.contains("\"shell\": \"annunciator\""));
        assert!(snapshot.ends_with("\n}"));
    }

    // JSON IMPORT / EXPORT ///////////////////////////////////////////////

    /// The round trip that matters most: a *modified* profile (one tuned
    /// away from any preset on both axes) exports and imports back to itself.
    #[test]
    fn a_modified_profile_round_trips_through_json() {
        let mut profile = Profile {
            screen: deep_blue(),
            chassis: slide_rule(),
        };
        // Tune it away from both presets, on every kind of field the two
        // axes carry: reals, strings, bools, and both enums.
        profile.screen.bloom = 0.4321;
        profile.screen.rasterization = Rasterization::SubpixelRasterization;
        profile.screen.font_source = FontSource::SystemFonts;
        profile.screen.font_name = "MY_OWN_FACE".to_string();
        profile.screen.blinking_cursor = true;
        profile.screen.font_color = "#abcdef".to_string();
        profile.chassis.channel_display = ChannelDisplay::Tape;
        profile.chassis.channel_indicator = ChannelIndicator::Pointer;
        profile.chassis.frame_shininess = 0.7777;

        let tuning = Tuning::handed_over(&Profile {
            screen: deep_blue(),
            chassis: slide_rule(),
        });
        assert!(
            tuning.is_modified(&profile),
            "test setup: the profile being round-tripped must be a modified one"
        );

        let json = export_json("My Own Look", &profile);
        let imported = import_json(&json).expect("our own export must import");

        assert_eq!(imported.name, "My Own Look");
        assert_eq!(imported.profile, profile);
        // And the round trip is stable at the document level too, which is
        // the level the equality works at.
        assert_eq!(imported.profile.snapshot(), profile.snapshot());
        assert_eq!(export_json("My Own Look", &imported.profile), json);
    }

    #[test]
    fn export_carries_the_name_and_the_version_at_the_top_level() {
        let json = export_json("Workshop", &Profile::default());
        assert!(json.contains("\"name\": \"Workshop\""));
        assert!(json.contains("\"profileVersion\": 1"));
        // Export assigns the two added keys after the two the object
        // already had, so they land at a fixed position in the document.
        let name_at = json.find("\"name\": \"Workshop\"").unwrap();
        let chassis_at = json.find("\"chassis\"").unwrap();
        assert!(name_at > chassis_at);
    }

    /// `throw "Profile doesn't have a name"`.
    #[test]
    fn an_unnamed_profile_is_refused() {
        let json = r#"{"screen": {}, "chassis": {}, "profileVersion": 1}"#;
        assert!(matches!(import_json(json), Err(ProfileError::Unnamed)));
        let empty = r#"{"name": "", "screen": {}, "profileVersion": 1}"#;
        assert!(matches!(import_json(empty), Err(ProfileError::Unnamed)));
    }

    /// `throw "This profile is not supported on this version of CRT."`
    #[test]
    fn a_future_profile_version_is_refused() {
        let json = r#"{"name": "Next", "profileVersion": 2, "screen": {}}"#;
        assert!(matches!(
            import_json(json),
            Err(ProfileError::UnsupportedVersion { found: 2, .. })
        ));
    }

    /// `profileObject.profileVersion !== undefined ? ... : 1`: a file
    /// without the key is a version-1 file.
    #[test]
    fn a_missing_version_reads_as_version_one() {
        let json = r#"{"name": "Old", "screen": {"bloom": 0.9}}"#;
        let imported = import_json(json).expect("a versionless profile is a v1 profile");
        assert_eq!(imported.profile.screen.bloom, 0.9);
    }

    /// A partial blob: `camelCase`, integer enums, and no `name` key inside
    /// the screen object (an imported profile's `name` lives at the top
    /// level, not the screen). The missing keys must fall back to the
    /// *default* appliance, never to what is standing.
    #[test]
    fn a_partial_blob_imports_with_defaults_for_what_it_omits() {
        let json = r##"{
          "screen": {
            "fontColor": "#7fb4ff",
            "rasterization": 2,
            "fontSource": 1,
            "blinkingCursor": true,
            "screenCurvature": 0.4
          },
          "chassis": {
            "shell": "switchboard",
            "channelDisplay": "tape"
          },
          "name": "Partial Import",
          "profileVersion": 1
        }"##;
        let imported = import_json(json).expect("a partial blob must import");
        assert_eq!(imported.name, "Partial Import");

        let screen = &imported.profile.screen;
        assert_eq!(screen.font_color, "#7fb4ff");
        assert_eq!(screen.rasterization, Rasterization::PixelRasterization);
        assert_eq!(screen.font_source, FontSource::SystemFonts);
        assert!(screen.blinking_cursor);
        assert_eq!(screen.screen_curvature, 0.4);
        // Silent about these, so they are the default appliance's.
        let default = ScreenSettings::default();
        assert_eq!(screen.name, default.name);
        assert_eq!(screen.bloom, default.bloom);
        assert_eq!(screen.font_name, default.font_name);

        let chassis = &imported.profile.chassis;
        assert_eq!(chassis.shell, Shell::Switchboard);
        assert_eq!(chassis.channel_display, ChannelDisplay::Tape);
        assert_eq!(chassis.name, ChassisSettings::default().name);
        assert_eq!(
            chassis.channel_indicator,
            ChassisSettings::default().channel_indicator
        );
    }

    #[test]
    fn malformed_json_is_refused_rather_than_defaulted() {
        assert!(matches!(
            import_json("not json at all"),
            Err(ProfileError::Malformed(_))
        ));
        assert!(matches!(
            import_json("[1, 2, 3]"),
            Err(ProfileError::Malformed(_))
        ));
    }

    /// A name with characters JSON has to escape survives the round trip,
    /// since a profile name is the user's own text.
    #[test]
    fn a_name_needing_escapes_round_trips() {
        let name = "He said \"hi\"\\ and\tstopped";
        let json = export_json(name, &Profile::default());
        assert_eq!(import_json(&json).unwrap().name, name);
    }

    // THE PROFILE SAVE ///////////////////////////////////////////////////

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

    /// Save, then load, then compare snapshots: the look a user keeps is
    /// the look they get back, at the document level the Modified flag
    /// reads.
    #[test]
    fn a_saved_look_reloads_unmodified() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.workshop.toml");

        let mut profile = Profile {
            screen: deep_blue(),
            chassis: slide_rule(),
        };
        profile.screen.jitter = 0.6543;
        profile.chassis.frame_color = "#123456".to_string();

        save_to(&path, &profile).unwrap();
        let tuning = Tuning::handed_over(&profile);

        let reloaded = Profile::from_config(&crate::Config::load(&path).unwrap());
        assert!(
            !tuning.is_modified(&reloaded),
            "a saved look reloaded as a different look:\n{}\nvs\n{}",
            tuning.loaded(),
            reloaded.snapshot()
        );
    }
}
