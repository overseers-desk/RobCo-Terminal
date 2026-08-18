//! Where the application's per-user files live.
//!
//! Per-user data lives at `$XDG_DATA_HOME/<identity>`, a single path
//! component: nothing here needs an organization/application split, so
//! there is no reason to double the identity in the path.
//!
//! The config file's path is deliberately *not* here. It is `settings`'s to
//! decide and record, and putting a second opinion about it in this
//! module would be exactly the kind of duplicate that has to be
//! reconciled at merge.

use std::path::PathBuf;

/// `$XDG_DATA_HOME/<identity>`, falling back to `$HOME/.local/share` per
/// the XDG base-directory spec. Both are honored so the harness can
/// isolate a scratch run (contract item 5).
pub fn data_dir(identity: &str) -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .filter(|h| !h.is_empty())
                .map(|home| PathBuf::from(home).join(".local").join("share"))
        })
        .unwrap_or_else(std::env::temp_dir);
    base.join(identity)
}

/// Where crash backtraces are written: the app data location plus
/// `/crashes`.
pub fn crash_dir(identity: &str) -> PathBuf {
    data_dir(identity).join("crashes")
}

/// `$XDG_CACHE_HOME/<identity>`, with the same `$HOME` fallback as
/// [`data_dir`]: everything this directory holds is generated and
/// regenerable (see [`preset_dir`]), which is what belongs in a cache
/// location rather than the data one.
pub fn cache_dir(identity: &str) -> PathBuf {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .filter(|h| !h.is_empty())
                .map(|home| PathBuf::from(home).join(".cache"))
        })
        .unwrap_or_else(std::env::temp_dir);
    base.join(identity)
}

/// Where `crt::preset::materialize` writes the generated `.slangp`, its
/// shader bodies and its noise texture.
///
/// The cache and not the data directory, and that is the whole reason
/// [`cache_dir`] exists: every byte in here is generated from constants
/// compiled into the binary, is rewritten whenever a structural setting
/// moves, and is regenerated from nothing at the next start if it is
/// deleted. A user who empties it loses no state.
pub fn preset_dir(identity: &str) -> PathBuf {
    cache_dir(identity).join("preset")
}
