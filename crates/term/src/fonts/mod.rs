//! The bundled font catalogue.
//!
//! Three things live here that no text stack can supply for us:
//!
//!   * the catalogue itself, one entry per bundled font, carrying the
//!     metadata (`base_width`, `pixel_size`, `low_resolution`,
//!     `fallback_name`) that decides how a face may be rasterised;
//!   * [`metrics`], the scaled-metric arithmetic (see that module for the
//!     rule and the evidence);
//!   * [`sizing`], the sizing policy plus the integer-scale policy the
//!     renderer needs on top of it;
//!   * [`raster`], the one rule for turning a face into pixels -- the
//!     embedded bitmap strike first, the outline only as a fallback -- which
//!     both rasterising callers in this crate share.
//!
//! System fonts are [`system`]'s, enumerated through `fontdb` and appended
//! to the catalogue after the bundled faces. The `is_system` flag was
//! already here for the filtering rules; now something sets it.

pub mod led;
pub mod metrics;
pub mod raster;
pub mod sizing;
pub mod subpixel;
pub mod system;
pub mod text;

use std::path::PathBuf;
use std::sync::OnceLock;

/// The base pixel height a font's other metrics are scaled from.
pub const BASE_FONT_PIXEL_HEIGHT: f64 = 32.0;
/// The rasterization value that means "modern": it selects the
/// non-low-resolution half of the catalogue.
pub const MODERN_RASTERIZATION: i32 = 4;
/// The pixel size at which system (non-bundled) fonts render.
pub const SYSTEM_FONT_PIXEL_SIZE: u32 = 32;

/// Which half of the catalogue the font list offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontSource {
    Bundled = 0,
    System = 1,
}

/// One row of the bundled font table, before the family is resolved and
/// `base_width` is recomputed for low-resolution faces.
#[derive(Clone, Copy, Debug)]
struct BundledFont {
    name: &'static str,
    text: &'static str,
    source: &'static str,
    /// The literal `base_width` value for this entry. Used as-is for the
    /// scalable faces and as the degenerate-metrics fallback; for a
    /// low-resolution face it is normally replaced by the computed value.
    fallback_base_width: f64,
    pixel_size: u32,
    low_resolution: bool,
    fallback_name: &'static str,
    data: &'static [u8],
}

/// A catalogue entry as `fontByName()` reports it.
#[derive(Clone, Debug, PartialEq)]
pub struct FontEntry {
    /// Stable key, e.g. `"TERMINESS_SCALED"`. What settings persist.
    pub name: &'static str,
    /// Menu label, e.g. `"Terminess"`.
    pub text: &'static str,
    /// The bundled resource path, `:/fonts/<dir>/<file>`, used as the
    /// entry's stable identifier.
    pub source: &'static str,
    /// Horizontal cell-width factor. Metric-derived for a low-resolution
    /// face, the declared literal otherwise.
    pub base_width: f64,
    /// The size this face is designed at. Load bearing: a low-resolution face
    /// is rasterised here and nowhere else.
    pub pixel_size: u32,
    /// True for the bitmap/pixel faces. Gates antialiasing and integer
    /// scaling, and selects which half of the catalogue is offered.
    pub low_resolution: bool,
    /// True for the faces [`system`] enumerated off the machine, false for the
    /// bundled ones.
    pub is_system: bool,
    /// Family name as the font itself reports it.
    pub family: String,
    /// Catalogue name of the face that covers this one's gaps, or `""`.
    pub fallback_name: &'static str,
    /// Where the face's bytes come from. Private, because the two arms are not
    /// the caller's business: [`FontEntry::data`] is.
    data: FontData,
}

/// A face's bytes, or where to get them.
///
/// A bundled face is `include_bytes!`, so it is already `'static` and already
/// resident. A system face is a path the enumeration found, read the first time
/// somebody asks it to become pixels: the machine's monospace list runs to
/// dozens of files and loading all of them to build a menu would be paying
/// megabytes for a list of names.
#[derive(Clone, Debug, PartialEq)]
enum FontData {
    Bundled(&'static [u8]),
    /// The file and which face inside it. The index is carried because a `.ttc`
    /// holds several and the reader will need it once anything in the tree
    /// shapes from a collection.
    System(PathBuf, u32),
}

impl FontEntry {
    /// The face's bytes.
    ///
    /// For a bundled face this is the `include_bytes!` slice and costs nothing.
    /// For a system face it is the file, read and cached on first use
    /// ([`system::face_data`]); an unreadable file comes back empty, which the
    /// atlas reports as a face it cannot build rather than as a wrong picture.
    pub fn data(&self) -> &'static [u8] {
        match &self.data {
            FontData::Bundled(data) => data,
            FontData::System(path, _) => system::face_data(path),
        }
    }
}

macro_rules! bundled {
    ($name:literal, $text:literal, $dir:literal, $file:literal,
     $base_width:literal, $pixel_size:literal, $low:literal, $fallback:literal) => {
        BundledFont {
            name: $name,
            text: $text,
            source: concat!(":/fonts/", $dir, "/", $file),
            fallback_base_width: $base_width,
            pixel_size: $pixel_size,
            low_resolution: $low,
            fallback_name: $fallback,
            data: include_bytes!(concat!("../../assets/fonts/", $dir, "/", $file)),
        }
    };
}

/// The bundled font table, in declared order: the low-resolution faces
/// first, then the scalable ones, with Departure Mono (low-resolution)
/// sitting among the latter. The list's order is the font menu's order, so
/// the order itself is meaningful, not incidental.
#[rustfmt::skip]
const BUNDLED: &[BundledFont] = &[
    bundled!("TERMINESS_SCALED", "Terminess", "terminus", "TerminessNerdFontMono-Regular.ttf", 1.0, 12, true, ""),
    bundled!("BIGBLUE_TERMINAL_SCALED", "BigBlue Terminal", "bigblue-terminal", "BigBlueTerm437NerdFontMono-Regular.ttf", 1.0, 12, true, ""),
    bundled!("EXCELSIOR_SCALED", "Fixedsys Excelsior", "fixedsys-excelsior", "FSEX301-L2.ttf", 1.0, 16, true, "UNSCII_16_SCALED"),
    bundled!("GREYBEARD_SCALED", "Greybeard", "greybeard", "Greybeard-16px.ttf", 1.0, 16, true, "UNSCII_16_SCALED"),
    bundled!("COMMODORE_PET_SCALED", "Commodore PET", "pet-me", "PetMe.ttf", 0.5, 8, true, "UNSCII_8_SCALED"),
    bundled!("GOHU_11_SCALED", "Gohu 11", "gohu", "GohuFont11NerdFontMono-Regular.ttf", 1.0, 11, true, ""),
    bundled!("COZETTE_SCALED", "Cozette", "cozette", "CozetteVector.ttf", 1.0, 13, true, ""),
    bundled!("UNSCII_8_SCALED", "Unscii 8", "unscii", "unscii-8.ttf", 0.5, 8, true, "UNSCII_8_SCALED"),
    bundled!("UNSCII_8_THIN_SCALED", "Unscii 8 Thin", "unscii", "unscii-8-thin.ttf", 0.5, 8, true, "UNSCII_8_SCALED"),
    bundled!("UNSCII_16_SCALED", "Unscii 16", "unscii", "unscii-16-full.ttf", 1.0, 16, true, "UNSCII_16_SCALED"),
    bundled!("APPLE_II_SCALED", "Apple ][", "apple2", "PrintChar21.ttf", 0.5, 8, true, "UNSCII_8_SCALED"),
    bundled!("ATARI_400_SCALED", "Atari 400-800", "atari-400-800", "AtariClassic-Regular.ttf", 0.5, 8, true, "UNSCII_8_SCALED"),
    bundled!("COMMODORE_64_SCALED", "Commodore 64", "pet-me", "PetMe64.ttf", 0.5, 8, true, "UNSCII_8_SCALED"),
    bundled!("IBM_EGA_8x8", "IBM EGA 8x8", "oldschool-pc-fonts", "PxPlus_IBM_EGA_8x8.ttf", 0.5, 8, true, "UNSCII_8_SCALED"),
    bundled!("IBM_VGA_8x16", "IBM VGA 8x16", "oldschool-pc-fonts", "PxPlus_IBM_VGA_8x16.ttf", 1.0, 16, true, "UNSCII_16_SCALED"),
    bundled!("TERMINESS", "Terminess", "terminus", "TerminessNerdFontMono-Regular.ttf", 1.0, 32, false, ""),
    bundled!("HACK", "Hack", "hack", "HackNerdFontMono-Regular.ttf", 1.0, 32, false, ""),
    bundled!("FIRA_CODE", "Fira Code", "fira-code", "FiraCodeNerdFontMono-Regular.ttf", 1.0, 32, false, ""),
    bundled!("IOSEVKA", "Iosevka", "iosevka", "IosevkaTermNerdFontMono-Regular.ttf", 1.0, 32, false, ""),
    bundled!("JETBRAINS_MONO", "JetBrains Mono", "jetbrains-mono", "JetBrainsMonoNerdFontMono-Regular.ttf", 1.0, 32, false, ""),
    bundled!("IBM_3278", "IBM 3278", "ibm-3278", "3270NerdFontMono-Regular.ttf", 1.0, 32, false, ""),
    bundled!("SOURCE_CODE_PRO", "Source Code Pro", "source-code-pro", "SauceCodeProNerdFontMono-Regular.ttf", 1.0, 32, false, ""),
    bundled!("DEPARTURE_MONO_SCALED", "Departure Mono", "departure-mono", "DepartureMonoNerdFontMono-Regular.otf", 1.0, 11, true, ""),
    bundled!("OPENDYSLEXIC", "OpenDyslexic", "opendyslexic", "OpenDyslexicMNerdFontMono-Regular.otf", 1.0, 32, false, ""),
];

fn resolve_all() -> Vec<FontEntry> {
    let mut all = resolve_bundled();
    // Bundled, then system, in one list. The system half is appended, never
    // interleaved, so the bundled menu order this catalogue's test pins is
    // untouched by what is installed on the machine.
    let bundled_families: Vec<String> = all
        .iter()
        .map(|f| f.family.clone())
        .filter(|f| !f.is_empty())
        .collect();
    all.extend(
        system::monospace_families(&bundled_families)
            .into_iter()
            .map(|face| {
                // Field for field: the family name is the name, the label
                // and the family; the width is the literal 1.0 (there is no
                // metric-derived width on this path); the size is
                // `SYSTEM_FONT_PIXEL_SIZE`; and a system face is never
                // low-resolution, so it takes the antialiasing-on
                // rasterisation like the bundled scalable faces do.
                //
                // The name is leaked because every other entry's is `'static` and
                // `resolve_font_name` hands one back: the catalogue is a process
                // singleton built once, so its strings live as long as it does
                // whether or not the allocator is told.
                let name: &'static str = Box::leak(face.family.clone().into_boxed_str());
                FontEntry {
                    name,
                    text: name,
                    source: "",
                    base_width: 1.0,
                    pixel_size: SYSTEM_FONT_PIXEL_SIZE,
                    low_resolution: false,
                    is_system: true,
                    family: face.family,
                    fallback_name: "",
                    data: FontData::System(face.path, face.index),
                }
            }),
    );
    all
}

/// A system entry whose file is not there, for the one case the atlas has to
/// survive rather than assert away: a face the catalogue offered and the
/// machine has since lost. `FontEntry::data` answers an empty slice for it,
/// exactly as it does for a font uninstalled between enumeration and
/// selection.
#[cfg(test)]
pub(crate) fn missing_system_face(name: &'static str, path: PathBuf) -> FontEntry {
    FontEntry {
        name,
        text: name,
        source: "",
        base_width: 1.0,
        pixel_size: SYSTEM_FONT_PIXEL_SIZE,
        low_resolution: false,
        is_system: true,
        family: name.to_string(),
        fallback_name: "",
        data: FontData::System(path, 0),
    }
}

fn resolve_bundled() -> Vec<FontEntry> {
    BUNDLED
        .iter()
        .map(|b| {
            let family = metrics::family_name(b.data).unwrap_or_default();
            // addBundledFont(): a low-resolution face takes the metric-derived
            // width, a scalable one keeps the literal.
            let base_width = if b.low_resolution {
                metrics::compute_base_width(b.data, b.pixel_size, b.fallback_base_width)
            } else {
                b.fallback_base_width
            };
            FontEntry {
                name: b.name,
                text: b.text,
                source: b.source,
                base_width,
                pixel_size: b.pixel_size,
                low_resolution: b.low_resolution,
                is_system: false,
                family,
                fallback_name: b.fallback_name,
                data: FontData::Bundled(b.data),
            }
        })
        .collect()
}

/// The whole catalogue, in declared order. Resolved once.
pub fn fonts() -> &'static [FontEntry] {
    static FONTS: OnceLock<Vec<FontEntry>> = OnceLock::new();
    FONTS.get_or_init(resolve_all)
}

pub fn font_by_name(name: &str) -> Option<&'static FontEntry> {
    fonts().iter().find(|f| f.name == name)
}

/// The bundled low-resolution faces, the list the LED and tape displays
/// letter themselves from.
pub fn low_resolution_fonts() -> impl Iterator<Item = &'static FontEntry> {
    fonts().iter().filter(|f| !f.is_system && f.low_resolution)
}

/// The faces offered for a given source and rasterization mode. Modern
/// rasterization offers the scalable faces, every other mode offers the
/// low-resolution ones; system fonts are offered whatever the mode.
pub fn filtered_fonts(
    source: FontSource,
    rasterization: i32,
) -> impl Iterator<Item = &'static FontEntry> {
    let modern = rasterization == MODERN_RASTERIZATION;
    fonts().iter().filter(move |f| {
        let matches_source = match source {
            FontSource::Bundled => !f.is_system,
            FontSource::System => f.is_system,
        };
        matches_source && (f.is_system || modern == !f.low_resolution)
    })
}

/// Fallback for a font name the current filter does not offer: falls back
/// to the first face that it does.
pub fn resolve_font_name(
    name: &str,
    source: FontSource,
    rasterization: i32,
) -> Option<&'static str> {
    let mut filtered = filtered_fonts(source, rasterization).peekable();
    let first = filtered.peek().map(|f| f.name);
    match filtered.find(|f| f.name == name) {
        Some(found) => Some(found.name),
        None => first,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bundled half, which is the half that is the same on every machine.
    fn bundled() -> Vec<&'static FontEntry> {
        fonts().iter().filter(|f| !f.is_system).collect()
    }

    #[test]
    fn the_catalogue_matches_the_recorded_entries() {
        // The bundled half is pinned exactly; the system half is whatever the
        // machine has, so the count below is the count of `BUNDLED` and the
        // last bundled entry is still the last entry the bundled table
        // declares.
        let bundled = bundled();
        assert_eq!(bundled.len(), 24);
        assert_eq!(bundled[0].name, "TERMINESS_SCALED");
        assert_eq!(bundled.last().unwrap().name, "OPENDYSLEXIC");
        // System entries are appended, never interleaved: the first 24 of the
        // whole catalogue are the bundled 24 in declared order.
        assert!(fonts()[..24].iter().all(|f| !f.is_system));
        // The default font name.
        assert!(font_by_name("TERMINESS_SCALED").is_some());
        // The settings default for the channel-bank lettering.
        assert!(font_by_name("UNSCII_8_SCALED").is_some());
        assert!(font_by_name("NO_SUCH_FONT").is_none());
    }

    #[test]
    fn each_rasterization_mode_offers_its_own_half() {
        let modern: Vec<_> = filtered_fonts(FontSource::Bundled, MODERN_RASTERIZATION)
            .map(|f| f.name)
            .collect();
        let legacy: Vec<_> = filtered_fonts(FontSource::Bundled, 0)
            .map(|f| f.name)
            .collect();
        assert_eq!(modern.len() + legacy.len(), bundled().len());
        assert!(modern.contains(&"HACK"));
        assert!(!modern.contains(&"UNSCII_8_SCALED"));
        assert!(legacy.contains(&"UNSCII_8_SCALED"));
        // Departure Mono sits among the scalable faces in the list but is a
        // low-resolution face, so the legacy half is where it is offered.
        assert!(legacy.contains(&"DEPARTURE_MONO_SCALED"));
        // A system face is offered whatever the rasterization mode, so the
        // two system lists are the same list.
        let sys_legacy: Vec<_> = filtered_fonts(FontSource::System, 0)
            .map(|f| f.name)
            .collect();
        let sys_modern: Vec<_> = filtered_fonts(FontSource::System, MODERN_RASTERIZATION)
            .map(|f| f.name)
            .collect();
        assert_eq!(sys_legacy, sys_modern);
        // ...and no bundled face is in it, in either mode.
        assert!(!sys_legacy.contains(&"HACK"));
    }

    /// The system half: one entry per family, with the system-face field
    /// values, offered under the system source only, in both rasterization
    /// modes.
    ///
    /// Stated about *whatever* the machine has rather than about a named
    /// family, because a test that needs DejaVu installed is a test that fails
    /// on a machine rather than about one. The named-family evidence is
    /// `tests/system_fonts.rs`, which says so out loud when it skips.
    #[test]
    fn the_system_half_is_populate_system_fonts() {
        let system: Vec<_> = fonts().iter().filter(|f| f.is_system).collect();
        for f in &system {
            assert_eq!(f.name, f.text, "{}: name and label are the family", f.name);
            assert_eq!(f.family, f.name, "{}: family is the name", f.name);
            assert_eq!(f.source, "", "{}: a system face has no resource", f.name);
            assert_eq!(f.base_width, 1.0, "{}: the literal baseWidth", f.name);
            assert_eq!(f.pixel_size, SYSTEM_FONT_PIXEL_SIZE, "{}", f.name);
            assert!(!f.low_resolution, "{}: never low-resolution", f.name);
            assert_eq!(f.fallback_name, "", "{}: no fallback entry", f.name);
        }
        let mut names: Vec<_> = system.iter().map(|f| f.name).collect();
        let count = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), count, "a family was enumerated twice");
        // A bundled family is never offered twice.
        for f in &system {
            assert!(
                !fonts()
                    .iter()
                    .any(|b| !b.is_system && !b.family.is_empty() && b.family == f.family),
                "{} is offered as both a bundled and a system face",
                f.name
            );
        }
    }

    #[test]
    fn a_filtered_out_name_falls_back_to_the_first_offered() {
        // Switching rasterization while a now-unavailable face is selected
        // moves the selection.
        assert_eq!(
            resolve_font_name("HACK", FontSource::Bundled, 0),
            Some("TERMINESS_SCALED")
        );
        assert_eq!(
            resolve_font_name("HACK", FontSource::Bundled, MODERN_RASTERIZATION),
            Some("HACK")
        );
    }

    #[test]
    fn the_low_resolution_list_is_what_the_displays_letter_from() {
        let names: Vec<_> = low_resolution_fonts().map(|f| f.name).collect();
        assert_eq!(names.len(), 16);
        assert!(names.contains(&"UNSCII_8_SCALED"));
        assert!(!names.contains(&"OPENDYSLEXIC"));
    }
}
