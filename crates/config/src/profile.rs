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
//! A preset is a full blob: all 28 measures of a screen (8 of a chassis)
//! are named, and picking a preset by index overwrites the name over
//! whatever was already selected, so "which preset was this struck from"
//! is carried by the selection rather than by the blob. The rebuild's
//! config file is a diff against defaults (`docs/config-format.md`), so it
//! carries the same fact the other way round: the `name` key *selects the
//! base*, and every other key present is an override on top of it.
//! [`resolve_presets`] is that rule, applied to the document before it is
//! deserialized, so
//!
//! ```toml
//! [screen]
//! name = "Deep Blue"
//! bloom = 0.9
//! ```
//!
//! is the Deep Blue preset with one measure moved, rather than the default
//! screen wearing Deep Blue's name. A file that names all 28 measures (an
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

use crate::io::{self, ConfigError, Scalar};
use crate::presets;
use crate::schema::{
    ChannelDisplay, ChannelIndicator, ChassisSettings, FontSource, Rasterization, ScreenSettings,
    Shell,
};
use crate::Config;

/// An imported profile carrying any other version is refused.
pub const PROFILE_VERSION: u32 = 1;

/// A whole appliance: the screen in the chassis it stands in.
///
/// This is the shape a user's saved profile takes, the shape the session
/// restores, and the shape the snapshot equality compares.
#[derive(Debug, Clone, PartialEq, Default)]
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
    /// This string *is* the equality. What it reads, in this fixed key
    /// order:
    ///
    /// - **screen** (29 keys): `name`,
    ///   `backgroundColor`, `fontColor`, `flickering`, `horizontalSync`,
    ///   `staticNoise`, `chromaColor`, `saturationColor`,
    ///   `screenCurvature`, `glowingLine`, `burnIn`, `bloom`,
    ///   `rasterization`, `jitter`, `rgbShift`, `brightness`, `contrast`,
    ///   `ambientLight`, `windowOpacity`, `fontName`, `fontSource`,
    ///   `fontWidth`, `lineSpacing`, `margin`, `blinkingCursor`,
    ///   `frameSize`, `screenRadius`, `frameColor`, `frameShininess`.
    /// - **chassis** (8 keys): `name`, `shell`,
    ///   `channelIndicator`, `channelDisplay`, `frameSize`, `screenRadius`,
    ///   `frameColor`, `frameShininess`.
    ///
    /// Nothing else. In particular the general settings are absent, so
    /// re-fitting the LED bank or hiding the chassis does not mark a
    /// profile modified.
    pub fn snapshot(&self) -> String {
        let mut out = String::new();
        out.push_str("{\n");
        out.push_str("  \"screen\": ");
        write_object(&mut out, 2, &screen_fields(&self.screen));
        out.push_str(",\n");
        out.push_str("  \"chassis\": ");
        write_object(&mut out, 2, &chassis_fields(&self.chassis));
        out.push('\n');
        out.push('}');
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
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str("  \"screen\": ");
    write_object(&mut out, 2, &screen_fields(&profile.screen));
    out.push_str(",\n");
    out.push_str("  \"chassis\": ");
    write_object(&mut out, 2, &chassis_fields(&profile.chassis));
    out.push_str(",\n  \"name\": ");
    write_string(&mut out, name);
    out.push_str(",\n  \"profileVersion\": ");
    out.push_str(&PROFILE_VERSION.to_string());
    out.push('\n');
    out.push('}');
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
            screen: read_screen(object.get("screen")),
            chassis: read_chassis(object.get("chassis")),
        },
    })
}

/// Resolve each axis's `name` key to the built-in preset it selects, then
/// let every key the document already carries stand as an override.
///
/// See the module comment for why the config file spells "which preset was
/// this struck from" as a key rather than as a row. Applied to the parsed
/// document before deserialization, so the file on disk stays the diff it
/// is: nothing here writes anything.
///
/// A name that matches no preset leaves the axis exactly as the document
/// has it, which resolves to the schema default plus the file's own keys --
/// the meaning the file had before this rule existed, so a user's own
/// profile name is not a way to lose settings.
pub fn resolve_presets(doc: &mut toml_edit::DocumentMut) {
    resolve_axis(doc, "screen", |name| {
        presets::screen_presets()
            .into_iter()
            .find(|p| p.name == name)
            .and_then(|p| toml_edit::ser::to_document(&p).ok())
    });
    resolve_axis(doc, "chassis", |name| {
        presets::chassis_presets()
            .into_iter()
            .find(|p| p.name == name)
            .and_then(|p| toml_edit::ser::to_document(&p).ok())
    });
}

fn resolve_axis(
    doc: &mut toml_edit::DocumentMut,
    axis: &str,
    lookup: impl Fn(&str) -> Option<toml_edit::DocumentMut>,
) {
    let name = match doc
        .get(axis)
        .and_then(|item| item.as_table_like())
        .and_then(|table| table.get("name"))
        .and_then(|item| item.as_str())
    {
        Some(name) => name.to_string(),
        None => return,
    };
    let Some(base) = lookup(&name) else { return };
    let Some(table) = doc.get_mut(axis).and_then(|item| item.as_table_like_mut()) else {
        return;
    };
    for (key, value) in base.as_table().iter() {
        if table.get(key).is_none() {
            table.insert(key, value.clone());
        }
    }
}

/// Write `profile` into the TOML file at `path` as its `[screen]` and
/// `[chassis]` tables, atomically, in one edit.
///
/// This is the profile save, and it is the one place the writer had to
/// grow: `io::write_key` moves a single key, and an appliance is 37 of
/// them, which as 37 writes would be 37 reloads and 36 chances for a
/// half-applied look to be observed. [`io::write_keys`] is that widening
/// and no more -- one document, one atomic rename, and still nothing but a
/// closed list of named scalars, so there is no general mutation path into
/// the file.
///
/// Every measure is written, not just the ones that differ from the preset
/// the profile names: a saved profile is a complete statement of a look,
/// so it must not shift underneath the user when a later version changes a
/// default. The general settings are not touched, because they are not
/// part of a profile.
pub fn save_to(path: &Path, profile: &Profile) -> Result<(), ConfigError> {
    io::write_keys(path, &profile_keys(profile))
}

/// The 37 dotted keys and values a profile save writes: the two axes, in
/// schema order, spelled as the config file spells them.
fn profile_keys(profile: &Profile) -> Vec<(String, Scalar)> {
    let mut keys = Vec::with_capacity(37);
    for (name, value) in screen_fields(&profile.screen) {
        keys.push((format!("screen.{}", snake(name)), scalar(value)));
    }
    for (name, value) in chassis_fields(&profile.chassis) {
        keys.push((format!("chassis.{}", snake(name)), scalar(value)));
    }
    keys
}

/// The config file spells the JSON format's `camelCase` keys in
/// `snake_case`, so the one field list in this module serves both formats:
/// the JSON export, and the TOML config file.
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

/// The TOML value for a field, keeping the *config file's* spelling of the
/// two enums: `rasterization` and `font_source` are words there, though
/// the JSON format stores them as integers. `Json::Enum` carries both.
fn scalar(value: Json) -> Scalar {
    match value {
        Json::Str(s) => Scalar::String(s),
        Json::Num(n) => Scalar::Float(round4(n)),
        Json::Bool(b) => Scalar::Boolean(b),
        Json::Enum { word, .. } => Scalar::String(word.to_string()),
    }
}

/// One field's value, in the two spellings the two formats need.
#[derive(Debug, Clone)]
enum Json {
    Str(String),
    Num(f64),
    Bool(bool),
    /// An enum: `code` is the JSON format's integer, `word` is the config
    /// file's serde spelling (the TOML's).
    Enum {
        code: i64,
        word: &'static str,
    },
}

/// The screen object's 29 keys, in JSON export order.
fn screen_fields(screen: &ScreenSettings) -> Vec<(&'static str, Json)> {
    vec![
        ("name", Json::Str(screen.name.clone())),
        (
            "backgroundColor",
            Json::Str(screen.background_color.clone()),
        ),
        ("fontColor", Json::Str(screen.font_color.clone())),
        ("flickering", Json::Num(screen.flickering)),
        ("horizontalSync", Json::Num(screen.horizontal_sync)),
        ("staticNoise", Json::Num(screen.static_noise)),
        ("chromaColor", Json::Num(screen.chroma_color)),
        ("saturationColor", Json::Num(screen.saturation_color)),
        ("screenCurvature", Json::Num(screen.screen_curvature)),
        ("glowingLine", Json::Num(screen.glowing_line)),
        ("burnIn", Json::Num(screen.burn_in)),
        ("bloom", Json::Num(screen.bloom)),
        ("rasterization", rasterization_json(screen.rasterization)),
        ("jitter", Json::Num(screen.jitter)),
        ("rgbShift", Json::Num(screen.rgb_shift)),
        ("brightness", Json::Num(screen.brightness)),
        ("contrast", Json::Num(screen.contrast)),
        ("ambientLight", Json::Num(screen.ambient_light)),
        ("windowOpacity", Json::Num(screen.window_opacity)),
        ("fontName", Json::Str(screen.font_name.clone())),
        ("fontSource", font_source_json(screen.font_source)),
        ("fontWidth", Json::Num(screen.font_width)),
        ("lineSpacing", Json::Num(screen.line_spacing)),
        ("margin", Json::Num(screen.margin)),
        ("blinkingCursor", Json::Bool(screen.blinking_cursor)),
        ("frameSize", Json::Num(screen.frame_size)),
        ("screenRadius", Json::Num(screen.screen_radius)),
        ("frameColor", Json::Str(screen.frame_color.clone())),
        ("frameShininess", Json::Num(screen.frame_shininess)),
    ]
}

/// The chassis object's 8 keys, in JSON export order.
fn chassis_fields(chassis: &ChassisSettings) -> Vec<(&'static str, Json)> {
    vec![
        ("name", Json::Str(chassis.name.clone())),
        ("shell", Json::Str(shell_word(chassis.shell).to_string())),
        (
            "channelIndicator",
            Json::Str(indicator_word(chassis.channel_indicator).to_string()),
        ),
        (
            "channelDisplay",
            Json::Str(display_word(chassis.channel_display).to_string()),
        ),
        ("frameSize", Json::Num(chassis.frame_size)),
        ("screenRadius", Json::Num(chassis.screen_radius)),
        ("frameColor", Json::Str(chassis.frame_color.clone())),
        ("frameShininess", Json::Num(chassis.frame_shininess)),
    ]
}

fn rasterization_json(value: Rasterization) -> Json {
    let (code, word) = match value {
        Rasterization::NoRasterization => (0, "no_rasterization"),
        Rasterization::ScanlineRasterization => (1, "scanline_rasterization"),
        Rasterization::PixelRasterization => (2, "pixel_rasterization"),
        Rasterization::SubpixelRasterization => (3, "subpixel_rasterization"),
        Rasterization::ModernRasterization => (4, "modern_rasterization"),
    };
    Json::Enum { code, word }
}

fn rasterization_from_code(code: i64) -> Option<Rasterization> {
    Some(match code {
        0 => Rasterization::NoRasterization,
        1 => Rasterization::ScanlineRasterization,
        2 => Rasterization::PixelRasterization,
        3 => Rasterization::SubpixelRasterization,
        4 => Rasterization::ModernRasterization,
        _ => return None,
    })
}

fn font_source_json(value: FontSource) -> Json {
    let (code, word) = match value {
        FontSource::BundledFonts => (0, "bundled_fonts"),
        FontSource::SystemFonts => (1, "system_fonts"),
    };
    Json::Enum { code, word }
}

fn font_source_from_code(code: i64) -> Option<FontSource> {
    Some(match code {
        0 => FontSource::BundledFonts,
        1 => FontSource::SystemFonts,
        _ => return None,
    })
}

fn shell_word(shell: Shell) -> &'static str {
    match shell {
        Shell::Annunciator => "annunciator",
        Shell::SlideRule => "slide-rule",
        Shell::Switchboard => "switchboard",
    }
}

fn shell_from_word(word: &str) -> Option<Shell> {
    Some(match word {
        "annunciator" => Shell::Annunciator,
        "slide-rule" => Shell::SlideRule,
        "switchboard" => Shell::Switchboard,
        _ => return None,
    })
}

fn indicator_word(indicator: ChannelIndicator) -> &'static str {
    match indicator {
        ChannelIndicator::Glow => "glow",
        ChannelIndicator::Pointer => "pointer",
        ChannelIndicator::Switch => "switch",
    }
}

fn indicator_from_word(word: &str) -> Option<ChannelIndicator> {
    Some(match word {
        "glow" => ChannelIndicator::Glow,
        "pointer" => ChannelIndicator::Pointer,
        "switch" => ChannelIndicator::Switch,
        _ => return None,
    })
}

fn display_word(display: ChannelDisplay) -> &'static str {
    match display {
        ChannelDisplay::Led => "led",
        ChannelDisplay::Tape => "tape",
    }
}

fn display_from_word(word: &str) -> Option<ChannelDisplay> {
    Some(match word {
        "led" => ChannelDisplay::Led,
        "tape" => ChannelDisplay::Tape,
        _ => return None,
    })
}

/// Every key falls back to the default screen's, and the name falls back
/// to the default screen's name rather than to whatever is standing.
fn read_screen(value: Option<&serde_json::Value>) -> ScreenSettings {
    let mut screen = ScreenSettings::default();
    let Some(object) = value.and_then(|v| v.as_object()) else {
        return screen;
    };
    let string = |key: &str| object.get(key).and_then(|v| v.as_str()).map(str::to_string);
    let number = |key: &str| object.get(key).and_then(serde_json::Value::as_f64);
    let boolean = |key: &str| object.get(key).and_then(serde_json::Value::as_bool);
    let code = |key: &str| object.get(key).and_then(serde_json::Value::as_i64);

    if let Some(v) = string("name") {
        screen.name = v;
    }
    if let Some(v) = string("backgroundColor") {
        screen.background_color = v;
    }
    if let Some(v) = string("fontColor") {
        screen.font_color = v;
    }
    if let Some(v) = number("flickering") {
        screen.flickering = v;
    }
    if let Some(v) = number("horizontalSync") {
        screen.horizontal_sync = v;
    }
    if let Some(v) = number("staticNoise") {
        screen.static_noise = v;
    }
    if let Some(v) = number("chromaColor") {
        screen.chroma_color = v;
    }
    if let Some(v) = number("saturationColor") {
        screen.saturation_color = v;
    }
    if let Some(v) = number("screenCurvature") {
        screen.screen_curvature = v;
    }
    if let Some(v) = number("glowingLine") {
        screen.glowing_line = v;
    }
    if let Some(v) = number("burnIn") {
        screen.burn_in = v;
    }
    if let Some(v) = number("bloom") {
        screen.bloom = v;
    }
    if let Some(v) = code("rasterization").and_then(rasterization_from_code) {
        screen.rasterization = v;
    }
    if let Some(v) = number("jitter") {
        screen.jitter = v;
    }
    if let Some(v) = number("rgbShift") {
        screen.rgb_shift = v;
    }
    if let Some(v) = number("brightness") {
        screen.brightness = v;
    }
    if let Some(v) = number("contrast") {
        screen.contrast = v;
    }
    if let Some(v) = number("ambientLight") {
        screen.ambient_light = v;
    }
    if let Some(v) = number("windowOpacity") {
        screen.window_opacity = v;
    }
    if let Some(v) = string("fontName") {
        screen.font_name = v;
    }
    if let Some(v) = code("fontSource").and_then(font_source_from_code) {
        screen.font_source = v;
    }
    if let Some(v) = number("fontWidth") {
        screen.font_width = v;
    }
    if let Some(v) = number("lineSpacing") {
        screen.line_spacing = v;
    }
    if let Some(v) = number("margin") {
        screen.margin = v;
    }
    if let Some(v) = boolean("blinkingCursor") {
        screen.blinking_cursor = v;
    }
    if let Some(v) = number("frameSize") {
        screen.frame_size = v;
    }
    if let Some(v) = number("screenRadius") {
        screen.screen_radius = v;
    }
    if let Some(v) = string("frameColor") {
        screen.frame_color = v;
    }
    if let Some(v) = number("frameShininess") {
        screen.frame_shininess = v;
    }
    screen
}

/// Every key falls back to the default chassis's: one that is silent about
/// a measure or its kit means the annunciator's, never whatever the last
/// chassis left in place.
fn read_chassis(value: Option<&serde_json::Value>) -> ChassisSettings {
    let mut chassis = ChassisSettings::default();
    let Some(object) = value.and_then(|v| v.as_object()) else {
        return chassis;
    };
    let string = |key: &str| object.get(key).and_then(|v| v.as_str()).map(str::to_string);
    let number = |key: &str| object.get(key).and_then(serde_json::Value::as_f64);

    if let Some(v) = string("name") {
        chassis.name = v;
    }
    if let Some(v) = string("shell").as_deref().and_then(shell_from_word) {
        chassis.shell = v;
    }
    if let Some(v) = string("channelIndicator")
        .as_deref()
        .and_then(indicator_from_word)
    {
        chassis.channel_indicator = v;
    }
    if let Some(v) = string("channelDisplay")
        .as_deref()
        .and_then(display_from_word)
    {
        chassis.channel_display = v;
    }
    if let Some(v) = number("frameSize") {
        chassis.frame_size = v;
    }
    if let Some(v) = number("screenRadius") {
        chassis.screen_radius = v;
    }
    if let Some(v) = string("frameColor") {
        chassis.frame_color = v;
    }
    if let Some(v) = number("frameShininess") {
        chassis.frame_shininess = v;
    }
    chassis
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

/// One JSON object at `indent` spaces, two-space nested, `JSON.stringify`-shaped
/// layout.
fn write_object(out: &mut String, indent: usize, fields: &[(&'static str, Json)]) {
    let inner = " ".repeat(indent + 2);
    out.push_str("{\n");
    for (index, (key, value)) in fields.iter().enumerate() {
        out.push_str(&inner);
        write_string(out, key);
        out.push_str(": ");
        match value {
            Json::Str(s) => write_string(out, s),
            Json::Num(n) => write_number(out, *n),
            Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Json::Enum { code, .. } => out.push_str(&code.to_string()),
        }
        if index + 1 < fields.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str(&" ".repeat(indent));
    out.push('}');
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// Every field the snapshot documents is actually read by it: move one
    /// at a time and check the flag fires for each. A field that fell out
    /// of `screen_fields`/`chassis_fields` would be a measure the Modified
    /// badge silently stopped noticing, which is exactly the failure this
    /// test exists to prevent.
    /// One documented key's nudge: the smallest edit to that field that a
    /// rendered snapshot must notice.
    type ScreenMover = fn(&mut ScreenSettings);
    type ChassisMover = fn(&mut ChassisSettings);

    #[test]
    fn every_documented_field_moves_the_snapshot() {
        let base = Profile::default();
        let tuning = Tuning::handed_over(&base);

        // The 29 screen keys, each nudged in its own type's way.
        let screen_movers: Vec<(&str, ScreenMover)> = vec![
            ("name", |s| s.name = "moved".into()),
            ("backgroundColor", |s| s.background_color = "#010203".into()),
            ("fontColor", |s| s.font_color = "#040506".into()),
            ("flickering", |s| s.flickering += 0.5),
            ("horizontalSync", |s| s.horizontal_sync += 0.5),
            ("staticNoise", |s| s.static_noise += 0.5),
            ("chromaColor", |s| s.chroma_color += 0.5),
            ("saturationColor", |s| s.saturation_color += 0.5),
            ("screenCurvature", |s| s.screen_curvature += 0.5),
            ("glowingLine", |s| s.glowing_line += 0.5),
            ("burnIn", |s| s.burn_in += 0.5),
            ("bloom", |s| s.bloom += 0.25),
            ("rasterization", |s| {
                s.rasterization = Rasterization::PixelRasterization
            }),
            ("jitter", |s| s.jitter += 0.5),
            ("rgbShift", |s| s.rgb_shift += 0.5),
            ("brightness", |s| s.brightness += 0.25),
            ("contrast", |s| s.contrast += 0.1),
            ("ambientLight", |s| s.ambient_light += 0.5),
            ("windowOpacity", |s| s.window_opacity -= 0.5),
            ("fontName", |s| s.font_name = "MOVED".into()),
            ("fontSource", |s| s.font_source = FontSource::SystemFonts),
            ("fontWidth", |s| s.font_width += 0.5),
            ("lineSpacing", |s| s.line_spacing += 0.5),
            ("margin", |s| s.margin += 0.5),
            ("blinkingCursor", |s| s.blinking_cursor = !s.blinking_cursor),
            ("frameSize", |s| s.frame_size += 0.5),
            ("screenRadius", |s| s.screen_radius += 0.5),
            ("frameColor", |s| s.frame_color = "#070809".into()),
            ("frameShininess", |s| s.frame_shininess += 0.5),
        ];
        assert_eq!(
            screen_movers.len(),
            29,
            "composeScreenObject() writes 29 keys"
        );
        for (key, move_it) in &screen_movers {
            let mut moved = base.clone();
            move_it(&mut moved.screen);
            assert!(
                tuning.is_modified(&moved),
                "moving screen.{key} did not change the snapshot"
            );
        }

        let chassis_movers: Vec<(&str, ChassisMover)> = vec![
            ("name", |c| c.name = "moved".into()),
            ("shell", |c| c.shell = Shell::Switchboard),
            ("channelIndicator", |c| {
                c.channel_indicator = ChannelIndicator::Switch
            }),
            ("channelDisplay", |c| {
                c.channel_display = ChannelDisplay::Tape
            }),
            ("frameSize", |c| c.frame_size += 0.5),
            ("screenRadius", |c| c.screen_radius += 0.5),
            ("frameColor", |c| c.frame_color = "#0a0b0c".into()),
            ("frameShininess", |c| c.frame_shininess += 0.5),
        ];
        assert_eq!(
            chassis_movers.len(),
            8,
            "composeChassisObject() writes 8 keys"
        );
        for (key, move_it) in &chassis_movers {
            let mut moved = base.clone();
            move_it(&mut moved.chassis);
            assert!(
                tuning.is_modified(&moved),
                "moving chassis.{key} did not change the snapshot"
            );
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

    // PRESET BASE PLUS OVERRIDES /////////////////////////////////////////

    fn resolved(text: &str) -> Config {
        let mut doc: toml_edit::DocumentMut = text.parse().expect("test TOML should parse");
        resolve_presets(&mut doc);
        io::deserialize(doc).expect("resolved document should deserialize")
    }

    /// The heart of "a profile names a preset pair plus overrides": naming
    /// a preset takes that preset's measures, not the default's under a
    /// borrowed name.
    #[test]
    fn naming_a_preset_takes_that_presets_measures() {
        let config = resolved("[screen]\nname = \"Deep Blue\"\n");
        assert_eq!(config.screen, deep_blue());
        assert_ne!(config.screen, ScreenSettings::default());
    }

    #[test]
    fn keys_beside_the_name_override_the_preset() {
        let config = resolved("[screen]\nname = \"Deep Blue\"\nbloom = 0.9\n");
        let mut expected = deep_blue();
        expected.bloom = 0.9;
        assert_eq!(config.screen, expected);
    }

    #[test]
    fn the_two_axes_resolve_independently() {
        let config = resolved(
            "[screen]\nname = \"Deep Blue\"\n\n[chassis]\nname = \"Slide Rule\"\nframe_shininess = 0.2\n",
        );
        assert_eq!(config.screen, deep_blue());
        let mut expected = slide_rule();
        expected.frame_shininess = 0.2;
        assert_eq!(config.chassis, expected);
    }

    /// A name that is nobody's preset -- a look the user saved under their
    /// own name -- resolves the way the file already meant, so this rule
    /// cannot lose a setting.
    #[test]
    fn an_unknown_name_leaves_the_file_meaning_what_it_meant() {
        let config = resolved("[screen]\nname = \"My Own Look\"\nbloom = 0.9\n");
        let expected = ScreenSettings {
            name: "My Own Look".to_string(),
            bloom: 0.9,
            ..ScreenSettings::default()
        };
        assert_eq!(config.screen, expected);
    }

    /// The zero-config launch: no name, nothing to resolve, the frozen v1
    /// default. This rule must be invisible to a file that does not use it.
    #[test]
    fn an_empty_document_resolves_to_the_frozen_default() {
        assert_eq!(resolved(""), Config::default());
        assert_eq!(
            resolved("[general]\nfont_scaling = 2.0\n").screen,
            ScreenSettings::default()
        );
    }

    /// A full blob (every measure named) resolves to itself: the base is
    /// entirely overridden, so importing a full 28-key screen object and
    /// writing it out as TOML is a fixed point.
    #[test]
    fn a_full_blob_resolves_to_itself() {
        let mut screen = deep_blue();
        screen.name = "Default Amber".to_string(); // names one preset, is another
        screen.bloom = 0.123;
        let text = toml_edit::ser::to_document(&screen).unwrap().to_string();
        let config = resolved(&format!("[screen]\n{text}"));
        assert_eq!(config.screen, screen);
    }

    /// Resolution is a read-time transform, so it must not rewrite the
    /// file: the document the user keeps stays the diff they wrote.
    #[test]
    fn resolution_does_not_touch_the_document_the_user_keeps() {
        let original = "[screen]\nname = \"Deep Blue\"  # my favourite\nbloom = 0.9\n";
        let doc: toml_edit::DocumentMut = original.parse().unwrap();
        // The saved file is whatever was parsed; only the *copy* handed to
        // deserialization is resolved.
        let mut copy = doc.clone();
        resolve_presets(&mut copy);
        assert_eq!(doc.to_string(), original);
        assert!(copy.to_string().len() > original.len());
    }

    // THE PROFILE SAVE ///////////////////////////////////////////////////

    /// A saved profile is a complete statement of a look: all 37 measures,
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
