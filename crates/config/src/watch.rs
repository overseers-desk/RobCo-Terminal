//! Live-reloading the config file.
//!
//! [`ConfigWatcher`] watches the *directory* containing the config file,
//! not the file itself. Every writer in this design (this crate's own
//! [`crate::toml::write_document`], the Tk settings editor, `$EDITOR`, or any
//! third-party tool honoring the machine-write contract) writes atomically
//! via write-temp-then-rename. On several platforms a watch held on a
//! file's inode does not follow a rename that replaces it, so a watch on
//! the file itself can silently go dead the first time it is edited.
//! Watching the parent directory and filtering events by file name survives
//! the rename-replace pattern unconditionally.
//!
//! An event whose reload parses to the value already held fires nothing:
//! the backend delivers one rename-replace as more than one qualifying
//! event, and a writer may touch the file without changing it, so `on_reload`
//! (and whatever logging a caller hangs on it) means "the value changed",
//! never "the file was touched".

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::de::DeserializeOwned;

use crate::toml;

/// A live handle onto a typed config value that is kept up to date from the
/// file on disk.
///
/// Holding this alive keeps the underlying filesystem watch alive; dropping
/// it stops watching.
pub struct ConfigWatcher<T> {
    path: PathBuf,
    state: Arc<Mutex<T>>,
    loader: Loader<T>,
    // Never read directly; keeping the watcher alive is what keeps the
    // background watch running. Dropping it stops delivery.
    _watcher: RecommendedWatcher,
}

/// How a watcher turns the file at a path into its typed value. See
/// [`ConfigWatcher::spawn_with_loader`] for why this is a parameter rather
/// than always [`toml::load`].
type Loader<T> = Arc<dyn Fn(&Path) -> Result<T, toml::ConfigError> + Send + Sync>;

impl<T> ConfigWatcher<T>
where
    T: DeserializeOwned + Clone + PartialEq + Send + Sync + 'static,
{
    /// Start watching `path`. `initial` seeds the in-memory value; it is
    /// used as-is until the first reload fires; if you want the file's
    /// *current* contents rather than a hardcoded default, call
    /// [`crate::toml::load`] yourself and pass the result in as `initial`.
    pub fn spawn(path: impl AsRef<Path>, initial: T) -> notify::Result<Self> {
        Self::spawn_with(path, initial, |_| {})
    }

    /// Like [`spawn`](Self::spawn), plus `on_reload` runs with the new
    /// value every time the file is successfully reparsed. `on_reload` does
    /// not run on a failed reload: the contract is keep-last-good, and the
    /// failure is logged instead via the `log` crate at `error` level.
    pub fn spawn_with(
        path: impl AsRef<Path>,
        initial: T,
        on_reload: impl FnMut(&T) + Send + 'static,
    ) -> notify::Result<Self> {
        Self::spawn_with_loader(path, initial, toml::load::<T>, on_reload)
    }

    /// Like [`spawn_with`](Self::spawn_with), but every reload goes through
    /// `loader` instead of a plain [`crate::toml::load`].
    ///
    /// This exists because reading the config file is not always just
    /// deserializing it: [`crate::Config::load`] resolves each axis's named
    /// preset first, and a `--profile` run overlays the preset the command
    /// line named on top. Both are part of what the file *means*, so both
    /// have to happen on every reload and not only at startup -- otherwise
    /// the first live edit of a profile file would drop the resolution and
    /// the look would jump to the defaults mid-session.
    pub fn spawn_with_loader(
        path: impl AsRef<Path>,
        initial: T,
        loader: impl Fn(&Path) -> Result<T, toml::ConfigError> + Send + Sync + 'static,
        on_reload: impl FnMut(&T) + Send + 'static,
    ) -> notify::Result<Self> {
        let path = path.as_ref().to_path_buf();
        // Shared rather than moved: the watcher thread reloads on a file
        // event and `force_reload` reloads on the caller's thread (SIGUSR1),
        // so both need to reach the same loader.
        let loader: Loader<T> = Arc::new(loader);
        let dir = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let state = Arc::new(Mutex::new(initial));

        let watch_path = path.clone();
        let watch_state = Arc::clone(&state);
        let watch_loader = Arc::clone(&loader);
        let on_reload = Arc::new(Mutex::new(on_reload));
        let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
            let event = match res {
                Ok(event) => event,
                Err(err) => {
                    log::error!(
                        "robco-config: filesystem watcher error for {}: {err}",
                        watch_path.display()
                    );
                    return;
                }
            };
            if !event_touches(&event, &watch_path) {
                return;
            }
            let mut on_reload = on_reload.lock().expect("on_reload callback poisoned");
            reload_into(&watch_path, &watch_state, &*watch_loader, &mut *on_reload);
        })?;

        watcher.watch(&dir, RecursiveMode::NonRecursive)?;

        Ok(Self {
            path,
            state,
            loader,
            _watcher: watcher,
        })
    }

    /// The current in-memory value, kept fresh by the background watch.
    pub fn current(&self) -> T {
        self.state.lock().expect("config state poisoned").clone()
    }

    /// Read one thing off the current value without copying the rest of it.
    ///
    /// [`Self::current`] hands back a whole clone, which is what a caller
    /// wants when it is about to compare or keep the snapshot. A caller that
    /// only wants a field -- and does it once a frame -- pays for the whole
    /// structure to answer a question about one enum. This holds the lock for
    /// the length of `read` and copies nothing.
    ///
    /// `read` runs under the lock, so it must not reach back into this
    /// watcher.
    pub fn with<R>(&self, read: impl FnOnce(&T) -> R) -> R {
        read(&self.state.lock().expect("config state poisoned"))
    }

    /// Force an immediate reload, bypassing the filesystem watch.
    ///
    /// Wire this to whatever "reload now" trigger the platform offers, per
    /// the config contract's SIGUSR1 (Unix) requirement: install a signal
    /// handler that calls this method. This crate does not install the
    /// handler itself, since that is a process-wide, one-per-binary
    /// decision that belongs with the binary's startup code, not with a
    /// library that may be linked more than once.
    pub fn force_reload(&self) {
        let mut noop = |_: &T| {};
        reload_into(&self.path, &self.state, &*self.loader, &mut noop);
    }
}

/// Does this event concern the file we care about, and is it a kind we act
/// on? Directory watches deliver events for every entry in the directory,
/// so most events are filtered out here.
fn event_touches(event: &Event, path: &Path) -> bool {
    if !matches!(
        event.kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    ) {
        return false;
    }
    let target_name = path.file_name();
    event.paths.iter().any(|p| p.file_name() == target_name)
}

/// Re-read and re-parse `path`, updating `state` on success and logging
/// loudly (never silently) on failure, per the keep-last-good contract.
/// A parse equal to the held value is dropped whole (see the module doc).
fn reload_into<T: Clone + PartialEq>(
    path: &Path,
    state: &Mutex<T>,
    loader: &(dyn Fn(&Path) -> Result<T, toml::ConfigError> + Send + Sync),
    on_reload: &mut dyn FnMut(&T),
) {
    match loader(path) {
        Ok(value) => {
            {
                let mut guard = state.lock().expect("config state poisoned");
                if *guard == value {
                    return;
                }
                *guard = value.clone();
            }
            on_reload(&value);
        }
        Err(err) => {
            log::error!(
                "robco-config: keeping last-good config; reload of {} failed: {err}",
                path.display()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use std::fs;
    use std::sync::mpsc;
    use std::time::Duration;

    #[derive(Debug, Clone, Deserialize, PartialEq, Default)]
    struct Settings {
        #[serde(default)]
        bloom: f64,
    }

    /// Wait for `on_reload` to fire with a value matching `predicate`, up to
    /// a generous timeout, so the test does not depend on exact filesystem
    /// event timing.
    fn wait_for(rx: &mpsc::Receiver<Settings>, predicate: impl Fn(&Settings) -> bool) -> bool {
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if let Ok(value) = rx.recv_timeout(remaining.min(Duration::from_millis(500))) {
                if predicate(&value) {
                    return true;
                }
            }
        }
        false
    }

    /// A value that says how often it was copied.
    ///
    /// `Deserialize` reads it like [`Settings`]; the counter is not on the
    /// wire, and every clone bumps the shared tally. That is what makes the
    /// difference between the two readers measurable rather than asserted.
    #[derive(Debug, Deserialize, Default)]
    struct Counted {
        #[serde(default)]
        bloom: f64,
        #[serde(skip)]
        clones: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    }

    impl Clone for Counted {
        fn clone(&self) -> Self {
            self.clones
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Self {
                bloom: self.bloom,
                clones: self.clones.clone(),
            }
        }
    }

    impl PartialEq for Counted {
        fn eq(&self, other: &Self) -> bool {
            self.bloom == other.bloom
        }
    }

    /// `current` copies the whole value; `with` copies none of it.
    ///
    /// The distinction is worth a test rather than a comment because the
    /// callers that want `with` are the once-a-frame ones, where a whole
    /// `Config` clone under the settings mutex buys one field.
    #[test]
    fn with_reads_a_field_without_copying_the_value_and_current_copies_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "bloom = 0.25\n").unwrap();

        let initial = toml::load::<Counted>(&path).unwrap();
        let tally = initial.clones.clone();
        let watcher = ConfigWatcher::spawn_with(&path, initial, |_: &Counted| {})
            .expect("watcher should start");

        // Whatever the plumbing above cost, measure from here.
        let before = tally.load(std::sync::atomic::Ordering::Relaxed);
        for _ in 0..100 {
            assert_eq!(watcher.with(|c| c.bloom), 0.25);
        }
        assert_eq!(
            tally.load(std::sync::atomic::Ordering::Relaxed),
            before,
            "a hundred `with` reads copied the value"
        );

        // And the snapshot reader still does what it says.
        let snapshot = watcher.current();
        assert_eq!(snapshot.bloom, 0.25);
        assert_eq!(
            tally.load(std::sync::atomic::Ordering::Relaxed),
            before + 1,
            "`current` hands back a copy"
        );
    }

    #[test]
    fn watcher_fires_on_rename_replace() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "bloom = 0.1\n").unwrap();

        let (tx, rx) = mpsc::channel();
        let initial = toml::load::<Settings>(&path).unwrap();
        let watcher = ConfigWatcher::spawn_with(&path, initial, move |value: &Settings| {
            let _ = tx.send(value.clone());
        })
        .expect("watcher should start");

        // Simulate the atomic write-temp-then-rename pattern every writer
        // in this design uses (this crate's own `toml::write_document`, a Tk
        // editor, or any third-party tool following the machine-write
        // contract), rather than an in-place write.
        let tmp_path = dir.path().join(".config.toml.tmp");
        fs::write(&tmp_path, "bloom = 0.9\n").unwrap();
        fs::rename(&tmp_path, &path).unwrap();

        assert!(
            wait_for(&rx, |v| v.bloom == 0.9),
            "watcher did not observe the rename-replace within the timeout"
        );
        assert_eq!(watcher.current().bloom, 0.9);
    }

    /// One atomic save is delivered by the backend as more than one
    /// qualifying event, and each used to reload and fire; the equality
    /// gate in `reload_into` is what holds this at exactly one.
    #[test]
    fn one_save_fires_one_reload() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "bloom = 0.1\n").unwrap();

        let (tx, rx) = mpsc::channel();
        let initial = toml::load::<Settings>(&path).unwrap();
        let _watcher = ConfigWatcher::spawn_with(&path, initial, move |value: &Settings| {
            let _ = tx.send(value.clone());
        })
        .expect("watcher should start");

        let tmp_path = dir.path().join(".config.toml.tmp");
        fs::write(&tmp_path, "bloom = 0.9\n").unwrap();
        fs::rename(&tmp_path, &path).unwrap();

        assert!(
            wait_for(&rx, |v| v.bloom == 0.9),
            "the save did not reload at all"
        );
        // Give any sibling event of the same save time to arrive; a second
        // callback means the file was touched, not that the value changed.
        std::thread::sleep(Duration::from_millis(500));
        assert!(
            rx.try_recv().is_err(),
            "one save fired more than one reload"
        );
    }

    #[test]
    fn last_good_kept_on_parse_failure() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "bloom = 0.4\n").unwrap();

        let (tx, rx) = mpsc::channel();
        let initial = toml::load::<Settings>(&path).unwrap();
        assert_eq!(initial.bloom, 0.4);
        let watcher = ConfigWatcher::spawn_with(&path, initial, move |value: &Settings| {
            let _ = tx.send(value.clone());
        })
        .expect("watcher should start");

        // Write garbage via the same atomic rename pattern.
        let tmp_path = dir.path().join(".config.toml.tmp");
        fs::write(&tmp_path, "this is not [valid toml").unwrap();
        fs::rename(&tmp_path, &path).unwrap();

        // Nothing else has been written yet, so the only value that could
        // have replaced the in-memory one is a parse of the garbage.
        assert_eq!(
            watcher.current().bloom,
            0.4,
            "in-memory value must still be the last-good one"
        );

        // Now a good write, and take the *first* event that arrives. The
        // events are delivered in write order, so if the unparseable file
        // had produced a callback it would be sitting ahead of this one in
        // the channel. Asserting on the head of the queue is the same
        // claim as "the bad write fired nothing", and it costs one event's
        // latency rather than a timeout spent proving a negative.
        let tmp_path2 = dir.path().join(".config.toml.tmp2");
        fs::write(&tmp_path2, "bloom = 0.7\n").unwrap();
        fs::rename(&tmp_path2, &path).unwrap();

        let first = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("the good write should have produced a reload");
        assert_eq!(
            first.bloom, 0.7,
            "on_reload fired for an unparseable file; last-good must be kept silently in-memory, not replaced"
        );
        assert_eq!(watcher.current().bloom, 0.7);
    }

    #[test]
    fn force_reload_bypasses_the_watch() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "bloom = 0.2\n").unwrap();

        let initial = toml::load::<Settings>(&path).unwrap();
        let watcher = ConfigWatcher::spawn(&path, initial).expect("watcher should start");

        // Overwrite in place (not via the watch's event path under test);
        // force_reload must still pick it up on demand.
        fs::write(&path, "bloom = 0.6\n").unwrap();
        watcher.force_reload();
        assert_eq!(watcher.current().bloom, 0.6);
    }
}
