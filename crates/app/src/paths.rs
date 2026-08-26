//! Where the application's per-user files live.
//!
//! Per-user data lives at `$XDG_DATA_HOME/<identity>`, a single path
//! component: nothing here needs an organization/application split, so
//! there is no reason to double the identity in the path. Where the XDG
//! variables are unset the base comes from the platform's own per-user
//! locations through `directories` (`~/.local/share` and `~/.cache` on
//! Linux, `%LOCALAPPDATA%` on Windows); the explicit variable check stays
//! first so a scratch run can redirect these directories on every
//! platform, XDG-shaped or not (contract item 5).
//!
//! The config file's path is deliberately *not* here. It is `settings`'s to
//! decide and record, and putting a second opinion about it in this
//! module would be exactly the kind of duplicate that has to be
//! reconciled at merge.

use std::path::PathBuf;

/// `$XDG_DATA_HOME/<identity>`, falling back to the platform's per-user
/// data location. The variable is honored on every platform so the
/// harness can isolate a scratch run (contract item 5).
pub fn data_dir(identity: &str) -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .or_else(|| directories::BaseDirs::new().map(|b| b.data_local_dir().to_path_buf()))
        .unwrap_or_else(std::env::temp_dir);
    base.join(identity)
}

/// Where crash backtraces are written: the app data location plus
/// `/crashes`.
pub fn crash_dir(identity: &str) -> PathBuf {
    data_dir(identity).join("crashes")
}

/// `$XDG_CACHE_HOME/<identity>`, with the same platform fallback as
/// [`data_dir`]: everything this directory holds is generated and
/// regenerable (see [`preset_dir`]), which is what belongs in a cache
/// location rather than the data one.
pub fn cache_dir(identity: &str) -> PathBuf {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .or_else(|| directories::BaseDirs::new().map(|b| b.cache_dir().to_path_buf()))
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
