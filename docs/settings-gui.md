# The settings window

Right-clicking the glass opens `robco-settings`, a separate program shipped
in every official package beside `robco-term`. On macOS the same window is
also under the application menu, at **Settings…** or `Cmd`+`,`, where a Mac
looks for it. It is the graphical face of
[the config file](config.md): four tabs (General, Screen, Chassis, SSH), a
preset picker at the head of each look axis, sliders and pickers for every
key the build reads, a marker on each value the file pins away from its
preset, and a per-row reset that unpins one.

The **SSH** tab is a list rather than a form, `[[ssh.host]]` being a table
per server. The checked radio is where a new session starts: localhost at
the top, then one row per server with its host, user, port and key beside
it, `+` to add a row and `✕` to take one away. Checking a row writes its
host into `ssh.default`; renaming the checked row rewrites `ssh.default` in
the same edit, because that key names a row by its host string and would
otherwise be left pointing at a name nothing answers to. A field left at
its default (an empty user, port 22) is not written out, the same
diff-against-defaults rule the other tabs follow, while the host is written
whatever it holds, being what the row is identified by. The terminal reads
the table when it launches, so a change here reaches the next session
started rather than the ones already running.

## How it behaves

Every change is applied the moment it is made. There is no Apply button
because there is nothing to apply: each change is one minimal edit to
`config.toml`, written under the contract in
[`config-format.md`](config-format.md), and the running terminal's own file
watch carries it to the glass. The terminal is the preview.

**OK** closes the window and keeps what you see. **Cancel** writes back the
exact bytes the file held when the window opened (or removes the file it
created), and the glass follows. An edit made by another hand while the
window is open, your editor included, is picked up when the window regains
focus.

Switching presets while some keys are pinned raises the one dialog the app
has: **Switch** writes the new `name` alone and drops the table's pinned
keys, so the whole preset shows; **Keep my look** writes the `name` and pins
whatever the new base would move, so nothing visible changes. This is the
switch-or-rename decision `config-format.md` requires a preset picker to
make deliberately, put to the person who owns the answer.

Comments, formatting and keys the app does not know stay byte-identical
through every edit; the tests in `settings/tests/` hold it to that.

A failure the window cannot continue past is written to a log as well as
shown and printed, because the shipped Windows image has no console to
print to and a message box needs a Tk that may be the thing that failed.
The log is `$XDG_STATE_HOME/robco-term/settings.log`, or
`%LOCALAPPDATA%\robco-term\settings.log` on Windows, or the same pair of
names under the temporary directory where neither variable is set; a window
embedded in the terminal writes to the log the terminal names instead, so
the two accounts of one run stay together. Every uncaught background error
goes there too. A launch that shows nothing has left a reason behind.

## How it relates to the terminal

The two are separate processes with the config file between them; there is
no other channel. The terminal spawns the app on right-click, or on the
macOS menu's Settings item, which names the focused window and reaches the
same spawn (a sibling binary first, then `$PATH`); a second is declined
while one runs.
The app knows the schema the way its coder does: the defaults, the preset
diffs, the font catalogue and the enum value lists are literal data in
`settings/lib/model.tcl`, so the window opens without running or parsing
anything. The Rust source stays the authority on those values:
`robco-term --dump-settings` still prints them, and `tests/schema.test`
holds the literals to that dump, so a drift fails the tests rather than a
user's file. The one question left for the running terminal is the
machine's installed faces, asked over `robco-term --list-renderable-fonts`
when the user opts into system fonts. For that call the spawning terminal
names its own binary in `ROBCO_SETTINGS_TERMINAL`; a hand-launched window
falls back to the sibling executable, under the name the platform gives it
(`robco-term.exe` on Windows, `robco-term` everywhere else), then to
`$PATH`, and where the window is embedded in the terminal's own executable
the terminal is this process and needs no finding.

## Building and shipping it

The app is Tcl/Tk 9, under `settings/` in this repository: `lib/` is the
schema-bearing value model and the vendored tomledit module (the
byte-preserving TOML editor), `ui/` the window, `tests/` the tcltest
suites (`tclsh9.0 settings/tests/all.tcl`). tomledit's home is the
teatotal module shelf, <https://github.com/teatotal/teatotal>, where its
own test suite, man page and updates live; the copy in `lib/` is vendored
byte-identical from there. During development the entry script
`settings/robco-settings` runs directly against a system Tcl/Tk 9.

Releases ship it as one self-contained executable per platform, built by
`settings/zipfs/build-selfcontained.sh` (Unix) or `.ps1` (Windows): a
static Tcl 9 + Tk with the scripts folded in by `zipfs mkimg`, no installed
Tcl required, named `robco-settings-<version>-<os>-<arch>` under `dist/`.
Its version has one home, the `ROBCO_SETTINGS_VERSION` line in the entry
script. `cargo run -p xtask -- dist --settings-binary <that file>` (and
`deb` likewise) stages it beside `robco-term`; both refuse to package
without it, because no official package omits the settings app. A
self-compiled terminal without it loses only the right-click: the press
logs one warning and the file remains yours to edit. On macOS the pair
ships inside `RobCo Terminal.app` on a disk image
(`packaging/macos/bundle.sh`); on Windows the settings archive rides
inside the terminal exe itself. `.github/workflows/ci.yml` proves the
Windows and macOS builds on every push, Linux being proven on the
development machines before each push; `release.yml` builds the shipped
artifacts on tag.
