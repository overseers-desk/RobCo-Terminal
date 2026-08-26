# The settings window

Right-clicking the glass opens `robco-settings`, a separate program shipped
in every official package beside `robco-term`. It is the graphical face of
[the config file](config.md): three tabs (General, Screen, Chassis), a
preset picker at the head of each look axis, sliders and pickers for every
key the build reads, a marker on each value the file pins away from its
preset, and a per-row reset that unpins one.

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

## How it relates to the terminal

The two are separate processes with the config file between them; there is
no other channel. The terminal spawns the app on right-click (a sibling
binary first, then `$PATH`) and declines to spawn a second while one runs.
The app asks the terminal for what the file cannot tell it, running
`robco-term --dump-settings` at startup for the defaults, the resolved
preset tables, the font catalogue and the enum value lists; it carries no
copy of its own, so it cannot drift from the binary it serves. It looks for
that binary beside its own executable, then on `$PATH`, and the environment
variable `ROBCO_TERM` overrides both. Without a reachable `robco-term` it
refuses to start and says what it looked for.

## Building and shipping it

The app is Tcl/Tk 9, under `settings/` in this repository: `lib/` is the
file surgery, dump client and value model, `ui/` the window, `tests/` the
tcltest suites (`tclsh9.0 settings/tests/all.tcl`). During development the
entry script `settings/robco-settings` runs directly against a system
Tcl/Tk 9.

Releases ship it as one self-contained executable per platform, built by
`settings/zipfs/build-selfcontained.sh` (Unix) or `.ps1` (Windows): a
static Tcl 9 + Tk with the scripts folded in by `zipfs mkimg`, no installed
Tcl required, named `robco-settings-<version>-<os>-<arch>` under `dist/`.
Its version has one home, the `ROBCO_SETTINGS_VERSION` line in the entry
script. `cargo run -p xtask -- dist --settings-binary <that file>` (and
`deb` likewise) stages it beside `robco-term`; both refuse to package
without it, because no official package omits the settings app. A
self-compiled terminal without it loses only the right-click: the press
logs one warning and the file remains yours to edit. macOS `.app`/`.dmg`
bundling and a CI matrix for all platforms are follow-up work; questlog's
release machinery, which `settings/zipfs/` is adapted from, shows the
finished shape.
