//! Reading, deserializing, and atomically writing the config file.
//!
//! Everything here operates on [`toml_edit::DocumentMut`] and on
//! `T: serde::de::DeserializeOwned`; nothing in this module knows the shape
//! of the settings themselves. That shape lives in a sibling module.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::de::DeserializeOwned;
use toml_edit::DocumentMut;

/// Everything that can go wrong reading, parsing, or writing the config file.
#[derive(Debug)]
pub enum ConfigError {
    /// The file (or its directory, or the temp file used to write it)
    /// could not be read or written.
    Io(io::Error),
    /// The file's bytes are not valid TOML.
    ///
    /// This is the "unparseable file" case from the config contract: the
    /// caller must keep its last-good in-memory value and log this loudly,
    /// never swallow it silently.
    Parse(toml_edit::TomlError),
    /// The document parsed as TOML but does not deserialize into the
    /// expected shape (wrong type for a key, etc).
    Deserialize(toml_edit::de::Error),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Io(e) => write!(f, "config I/O error: {e}"),
            ConfigError::Parse(e) => write!(f, "config file is not valid TOML: {e}"),
            ConfigError::Deserialize(e) => {
                write!(f, "config file does not match expected shape: {e}")
            }
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ConfigError::Io(e) => Some(e),
            ConfigError::Parse(e) => Some(e),
            ConfigError::Deserialize(e) => Some(e),
        }
    }
}

impl From<io::Error> for ConfigError {
    fn from(e: io::Error) -> Self {
        ConfigError::Io(e)
    }
}

impl From<toml_edit::TomlError> for ConfigError {
    fn from(e: toml_edit::TomlError) -> Self {
        ConfigError::Parse(e)
    }
}

impl From<toml_edit::de::Error> for ConfigError {
    fn from(e: toml_edit::de::Error) -> Self {
        ConfigError::Deserialize(e)
    }
}

/// Read the config file at `path` as a format-preserving TOML document.
///
/// A missing file is not an error: it is an empty document, i.e. "every
/// setting takes its built-in default," per the diff-against-defaults
/// contract. Any other I/O failure, or a file whose bytes do not parse as
/// TOML, comes back as `Err`.
pub fn read_document(path: &Path) -> Result<DocumentMut, ConfigError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(DocumentMut::new()),
        Err(e) => return Err(e.into()),
    };
    Ok(text.parse::<DocumentMut>()?)
}

/// Deserialize an already-parsed document into a typed value.
///
/// Keys the document doesn't mention are left for `T`'s own defaulting
/// (`#[serde(default)]` and friends) to fill in; this function does not
/// merge against a default value itself.
pub fn deserialize<T: DeserializeOwned>(doc: DocumentMut) -> Result<T, ConfigError> {
    Ok(toml_edit::de::from_document(doc)?)
}

/// Read and deserialize `path` in one step.
pub fn load<T: DeserializeOwned>(path: &Path) -> Result<T, ConfigError> {
    deserialize(read_document(path)?)
}

/// Atomically write `doc` to `path`.
///
/// Writes a temp file in the same directory as `path`, flushes and syncs
/// it, then renames it over `path`. The rename is the atomic step: nothing
/// ever observes a partially written file, whether it reads through this
/// crate's watcher or by any other means.
pub fn write_document(path: &Path, doc: &DocumentMut) -> Result<(), ConfigError> {
    write_atomic(path, doc.to_string().as_bytes())
}

/// Read-modify-write: parse the document at `path`, let `edit` mutate it in
/// place, then atomically write the result back.
///
/// Because [`DocumentMut`] preserves comments, formatting, and keys unknown
/// to this program, an `edit` closure that touches one key changes exactly
/// that key's bytes on disk; everything else round-trips byte for byte.
pub fn edit_document(path: &Path, edit: impl FnOnce(&mut DocumentMut)) -> Result<(), ConfigError> {
    let mut doc = read_document(path)?;
    edit(&mut doc);
    write_document(path, &doc)
}

/// One scalar, as a writer names it: the values a single settings key can
/// hold.
///
/// The narrow half of [`edit_document`]. A caller that wants to move one key
/// says which key and what it now is, and nothing else about the file is
/// reachable from here: no table it can drop, no formatting it can normalise.
/// Everything in `Config`'s schema is one of these four (the enums serialise as
/// strings), so this covers the schema without exposing the document.
#[derive(Debug, Clone, PartialEq)]
pub enum Scalar {
    Integer(i64),
    Float(f64),
    Boolean(bool),
    String(String),
}

impl From<Scalar> for toml_edit::Value {
    fn from(scalar: Scalar) -> Self {
        match scalar {
            Scalar::Integer(v) => toml_edit::Value::from(v),
            Scalar::Float(v) => toml_edit::Value::from(v),
            Scalar::Boolean(v) => toml_edit::Value::from(v),
            Scalar::String(v) => toml_edit::Value::from(v),
        }
    }
}

/// Set one dotted key (`"general.led_characters"`) to `value`, atomically,
/// leaving every other byte of the file alone.
///
/// This is the single-key machine-write path the application uses;
/// [`write_keys`] is the multi-key one, for callers (a profile save) that
/// cannot show a half-applied write. Both stay deliberately small:
/// `docs/config-format.md` holds a writer to changing only the bytes of the
/// keys it touched, and a surface that reaches named keys alone cannot break
/// that rule.
///
/// Two details carry the contract:
///
/// - a key already in the file keeps its **decor** (the whitespace and the
///   comments around the value), so `led_characters = 12  # twelve` becomes
///   `led_characters = 20  # twelve` rather than losing the note;
/// - a table the file does not have is created as a real `[table]` rather than
///   the inline one `toml_edit`'s own auto-vivification would leave, so a
///   config that grows its first `[general]` key reads the way a hand-written
///   one does.
///
/// A missing file is an empty document to edit, per the contract's own rule,
/// so the first write to a fresh install creates it.
pub fn write_key(path: &Path, key: &str, value: Scalar) -> Result<(), ConfigError> {
    let value = toml_edit::Value::from(value);
    edit_document(path, |doc| set_dotted(doc, key, value))
}

/// Set several dotted keys in one atomic write.
///
/// The narrowest widening of [`write_key`] that a profile save can be built
/// from, and it exists for exactly that: an appliance is 37 keys
/// (`profile::save_to`), and writing them one at a time would be 37 atomic
/// renames, 37 reloads, and 36 windows in which a watcher could observe half
/// a look. One document, one rename, one reload.
///
/// Every property [`write_key`] carries survives, because this *is*
/// [`write_key`]'s edit run more than once inside a single
/// [`edit_document`]: decor is kept, absent tables grow as real `[table]`s,
/// unknown keys and comments round-trip, and a path running through a
/// non-table is refused rather than overwritten. What it deliberately does
/// not become is a general mutation path: the caller still names each key
/// and hands over a scalar, so there is no way from here to drop a table,
/// reorder a document, or write anything the caller did not name.
pub fn write_keys(path: &Path, entries: &[(String, Scalar)]) -> Result<(), ConfigError> {
    edit_document(path, |doc| {
        for (key, value) in entries {
            set_dotted(doc, key, toml_edit::Value::from(value.clone()));
        }
    })
}

/// The document half of [`write_key`], split out so the round trip is testable
/// without a file.
fn set_dotted(doc: &mut DocumentMut, key: &str, value: toml_edit::Value) {
    let mut table: &mut dyn toml_edit::TableLike = doc.as_table_mut();
    let mut segments = key.split('.').peekable();
    while let Some(segment) = segments.next() {
        if segments.peek().is_none() {
            match table.get_mut(segment).and_then(|item| item.as_value_mut()) {
                // The key stands: move the value and keep everything written
                // around it.
                Some(existing) => {
                    let decor = existing.decor().clone();
                    *existing = value;
                    *existing.decor_mut() = decor;
                }
                None => {
                    table.insert(segment, toml_edit::Item::Value(value));
                }
            }
            return;
        }
        let entry = table
            .entry(segment)
            .or_insert(toml_edit::Item::Table(toml_edit::Table::new()));
        if entry.is_none() {
            *entry = toml_edit::Item::Table(toml_edit::Table::new());
        }
        match entry.as_table_like_mut() {
            Some(next) => table = next,
            // The path runs through something that is not a table (a user has
            // written `general = 3`). Refusing to write is the only edit that
            // cannot destroy what is there; the caller's reload will show the
            // setting unchanged, which is the truth.
            None => return,
        }
    }
}

/// Write `bytes` to `path` atomically: temp file in the same directory,
/// `sync_all`, then `rename` over the target.
///
/// The directory is created if it is not there. The contract already says a
/// missing config *file* is an empty document to edit rather than an error,
/// and a missing directory is the same situation one level up: it is what a
/// fresh install looks like before anything has been saved. Without this,
/// the first write on a new machine fails with `ENOENT` from the temp file
/// -- which the app happened not to hit only because `SettingsHandle::spawn`
/// creates the directory it watches, and a profile save writes to a
/// *different* file than the one being watched.
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), ConfigError> {
    if let Some(dir) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(dir)?;
    }
    let tmp_path = temp_path_for(path);
    let result = (|| -> io::Result<()> {
        let mut tmp = File::create(&tmp_path)?;
        tmp.write_all(bytes)?;
        tmp.sync_all()
    })();
    match result {
        Ok(()) => {
            fs::rename(&tmp_path, path)?;
            Ok(())
        }
        Err(e) => {
            let _ = fs::remove_file(&tmp_path);
            Err(e.into())
        }
    }
}

/// A temp-file path in the same directory as `path`, distinct on every call
/// even under concurrent writers in the same process (a monotonic counter)
/// or across processes (pid + timestamp). The leading dot keeps it out of
/// naive directory listings and out of the watcher's own file-name filter.
fn temp_path_for(path: &Path) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("config.toml");
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    dir.join(format!(".{file_name}.{pid}.{nanos}.{n}.tmp"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize, PartialEq, Default)]
    struct Settings {
        #[serde(default)]
        bloom: f64,
        #[serde(default)]
        title: String,
    }

    #[test]
    fn missing_file_reads_as_empty_document_and_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let doc = read_document(&path).expect("missing file should not error");
        assert_eq!(doc.to_string(), "");

        let settings: Settings = load(&path).expect("missing file should deserialize to defaults");
        assert_eq!(settings, Settings::default());
    }

    #[test]
    fn unparseable_file_is_a_loud_error_not_a_silent_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "this is not [valid toml").unwrap();

        let err = read_document(&path).expect_err("garbage TOML must error, not silently default");
        assert!(matches!(err, ConfigError::Parse(_)));
    }

    #[test]
    fn write_document_is_readable_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let mut doc = DocumentMut::new();
        doc["bloom"] = toml_edit::value(0.5);
        write_document(&path, &doc).unwrap();

        let read_back = read_document(&path).unwrap();
        assert_eq!(read_back.to_string(), doc.to_string());

        // No leftover temp files beside the real one.
        let leftovers: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name() != "config.toml")
            .collect();
        assert!(leftovers.is_empty(), "leftover temp files: {leftovers:?}");
    }

    #[test]
    fn single_key_edit_leaves_every_other_byte_identical() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let original = "\
# user comment above bloom\n\
bloom = 0.5   # inline comment\n\
title = \"hello\"\n\
\n\
[chassis]\n\
# another comment\n\
radius = 12\n\
";
        fs::write(&path, original).unwrap();

        edit_document(&path, |doc| {
            doc["title"] = toml_edit::value("changed");
        })
        .unwrap();

        let updated = fs::read_to_string(&path).unwrap();
        let original_lines: Vec<&str> = original.lines().collect();
        let updated_lines: Vec<&str> = updated.lines().collect();
        assert_eq!(original_lines.len(), updated_lines.len());

        let mut changed_lines = 0;
        for (a, b) in original_lines.iter().zip(updated_lines.iter()) {
            if a != b {
                changed_lines += 1;
                assert!(b.starts_with("title ="), "unexpected changed line: {b:?}");
            }
        }
        assert_eq!(changed_lines, 1, "exactly one line should differ");
    }

    #[test]
    fn write_key_moves_one_value_and_keeps_the_comment_beside_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let original = "\
# the appliance\n\
[general]\n\
# how many characters the strips hold\n\
led_characters = 12  # twelve\n\
chassis_shown = true\n\
\n\
[screen]\n\
name = \"Default Amber\"\n\
";
        fs::write(&path, original).unwrap();

        write_key(&path, "general.led_characters", Scalar::Integer(27)).unwrap();

        let updated = fs::read_to_string(&path).unwrap();
        let before: Vec<&str> = original.lines().collect();
        let after: Vec<&str> = updated.lines().collect();
        assert_eq!(before.len(), after.len());
        let changed: Vec<&&str> = before
            .iter()
            .zip(after.iter())
            .filter(|(a, b)| a != b)
            .map(|(_, b)| b)
            .collect();
        assert_eq!(changed, vec![&"led_characters = 27  # twelve"]);
    }

    #[test]
    fn write_key_grows_a_real_table_for_a_key_the_file_has_never_had() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");

        // A file that does not exist at all is an empty document to edit.
        write_key(&path, "general.led_characters", Scalar::Integer(20)).unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "[general]\nled_characters = 20\n"
        );

        // ...and a second key lands in the table the first one built.
        write_key(&path, "general.chassis_shown", Scalar::Boolean(false)).unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "[general]\nled_characters = 20\nchassis_shown = false\n"
        );
    }

    #[test]
    fn write_key_carries_every_scalar_the_schema_uses() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        write_key(&path, "general.led_characters", Scalar::Integer(20)).unwrap();
        write_key(&path, "general.chassis_shown", Scalar::Boolean(false)).unwrap();
        write_key(&path, "screen.bloom", Scalar::Float(0.25)).unwrap();
        write_key(
            &path,
            "general.led_font_name",
            Scalar::String("TERMINESS_SCALED".to_string()),
        )
        .unwrap();

        #[derive(Debug, Deserialize, PartialEq, Default)]
        struct General {
            #[serde(default)]
            led_characters: i64,
            #[serde(default)]
            chassis_shown: bool,
            #[serde(default)]
            led_font_name: String,
        }
        #[derive(Debug, Deserialize, PartialEq, Default)]
        struct Screen {
            #[serde(default)]
            bloom: f64,
        }
        #[derive(Debug, Deserialize, PartialEq, Default)]
        struct Both {
            #[serde(default)]
            general: General,
            #[serde(default)]
            screen: Screen,
        }

        let read: Both = load(&path).unwrap();
        assert_eq!(read.general.led_characters, 20);
        assert!(!read.general.chassis_shown);
        assert_eq!(read.general.led_font_name, "TERMINESS_SCALED");
        assert_eq!(read.screen.bloom, 0.25);
    }

    #[test]
    fn write_key_leaves_a_path_that_is_not_a_table_alone() {
        // A hand-edited file where `general` is a scalar: the write is refused
        // rather than replacing what the user wrote with a table.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "general = 3\n").unwrap();
        write_key(&path, "general.led_characters", Scalar::Integer(20)).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "general = 3\n");
    }

    /// A fresh install: nothing has ever been saved, so the config
    /// directory does not exist yet. The first write makes it rather than
    /// failing, which is the same rule the contract already states for a
    /// missing file.
    #[test]
    fn a_write_into_a_directory_that_does_not_exist_yet_creates_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("never/made/config.toml");
        assert!(!path.parent().unwrap().exists());

        write_key(&path, "general.led_characters", Scalar::Integer(20)).unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "[general]\nled_characters = 20\n"
        );
    }

    #[test]
    fn write_keys_moves_several_keys_in_one_file_and_keeps_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let original = "\
# kept\n\
[general]\n\
led_characters = 12  # twelve\n\
\n\
[screen]\n\
bloom = 0.5\n\
name = \"Default Amber\"\n\
";
        fs::write(&path, original).unwrap();

        write_keys(
            &path,
            &[
                ("screen.bloom".to_string(), Scalar::Float(0.9)),
                (
                    "screen.name".to_string(),
                    Scalar::String("Deep Blue".to_string()),
                ),
                ("chassis.frame_size".to_string(), Scalar::Float(0.45)),
            ],
        )
        .unwrap();

        let updated = fs::read_to_string(&path).unwrap();
        assert!(updated.contains("# kept"));
        assert!(updated.contains("led_characters = 12  # twelve"));
        assert!(updated.contains("bloom = 0.9"));
        assert!(updated.contains("name = \"Deep Blue\""));
        assert!(updated.contains("[chassis]\nframe_size = 0.45"));
    }

    #[test]
    fn edit_document_preserves_unknown_keys() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "bloom = 0.5\nsome_future_key = \"kept\"\n").unwrap();

        edit_document(&path, |doc| {
            doc["bloom"] = toml_edit::value(0.9);
        })
        .unwrap();

        let updated = fs::read_to_string(&path).unwrap();
        assert!(updated.contains("some_future_key = \"kept\""));
        assert!(updated.contains("bloom = 0.9"));
    }
}
