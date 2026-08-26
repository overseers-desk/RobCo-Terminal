# RobCo Terminal

A terminal emulator that behaves like a piece of hardware.

Not a terminal with a scanline filter over it. The picture is a curved tube:
phosphor that keeps glowing after the pixel goes dark, bloom that spills off
bright type, a scan line travelling down the glass, and a click that lands
where you aimed it because the pointer is mapped back through the curvature
the same way the type is mapped forward. Around the tube is a chassis, and
the chassis has a channel bank down one side with a numbered strip per
session. You pick a channel the way you would pick a station.

The CRT look draws its visual inspiration from
[cool-retro-term](https://github.com/Swordfish90/cool-retro-term). The
terminal itself is built on the rio emulation core. The chassis, the channel
bank, and the tmux integration below are this project's own.

## What you actually get

**The glass.** A phosphor screen you configure rather than choose from a
menu: colour, curvature, bloom, burn-in persistence, static, flicker,
horizontal sync wobble, jitter, and which scanline or pixel grid is laid over
the type. Presets come built in, from `Default Amber` through `Commodore 64`,
`Apple ][`, `IBM VGA 8x16` and `E-Ink`, and each one is a starting point you
adjust rather than a fixed skin.

The type is real work rather than a font pick. Bitmap faces are drawn from
their embedded strikes at integer scale, so an 8-pixel face renders as
8-pixel pixels and not as a blurred outline pretending to be one.

**The chassis.** The cabinets that ship are `Annunciator`, `Slide Rule` and
`Switchboard`. Each has its own casting, bezel, furniture and channel bank,
and each marks the live channel its own way (a glow, a pointer, a thrown
switch). The chassis is drawn outside the CRT chain, so the cabinet stays
straight while the picture behind the glass bulges. Turn it off and the tube
stands bare in its own moulding.

**Channels.** Every session gets a numbered slot on the bank. `Alt`+digit
brings a slot to the screen; `Alt`+`Shift`+digit moves what is on screen onto
a slot. `Ctrl`+`Shift`+`T` opens a new one. Switching channels degausses, the
way changing input on real hardware did.

**tmux as channels.** Type `tmux -CC` in any channel, on this machine or
over ssh. The terminal notices tmux's control mode, and the session you
attached arrives as a bank of channels of its own: the channel you typed in
becomes its gateway and stops taking keystrokes, the session's windows fill
the slots after it, and they are switched with the same chords as any other.
On a local server every other session gets a bank too, each with a gateway
the terminal starts for it; over ssh you get each session you attach, one
per `--ssh` channel you type the attach into. There
is no status bar to read, because the bank is doing that job. `Enter` on a
gateway detaches: a channel you typed in comes home, one the terminal started
closes. Nothing is lost if the terminal dies mid-session: run `tmux -CC`
again and every session comes back with its windows and their titles.

**Built-in SSH, from the command line.** `robco-term --ssh user@host` opens
the first channel on an SSH connection the terminal itself owns, PuTTY's
`-ssh` for a cabinet: the connection is a bank of its own, and further
channels of it will multiplex over the one wire. Two things will surprise
you in the first minute, both deliberate: it authenticates with your
ssh-agent and nothing else, and an unknown host key is refused with its
fingerprint and the command that records it rather than prompted about,
because there is no prompt UI yet and a program should not make trust
decisions for you. `docs/ssh.md` has the whole contract, including what it
does not read (`~/.ssh/config`).

## Will it run here

**Linux, today.** X11 is what is wired and measured; on Wayland the window
runs.

**macOS and Windows are planned, not built.** The stack was chosen for them
(the terminal core speaks ConPTY, the GPU layer covers Metal and D3D from the
same shader source), and the config paths for both are already implemented.
But neither has been built or run, and neither can be cross-compiled from
Linux, because the terminal core drags a C++ dependency that needs a native
toolchain. Treat them as intended, not available.

You need a GPU the graphics layer can reach. It picks a backend on its own,
and reports which one it chose in the first lines of its log. `WGPU_BACKEND`
overrides the choice if the automatic one misbehaves, which on Mesa is worth
trying with `vulkan` before anything else.

The window never goes under an 80 by 24 terminal. The floor is the room that
grid takes at the font and screen margin you have configured, plus the
channel bank beside it, so a larger face raises it: on the shipped profile it
comes to 1265 by 730, and the first window opens no smaller.

To build from source you need:

- Rust 1.96.1 or newer (developed on 1.97.1),
- a C++ compiler, for the terminal core's SIMD dependency; the SSH stack's
  crypto (`ring`) uses the same C toolchain and links statically, adding no
  requirement of its own.

At run time the installed binary wants libstdc++. On Debian and Ubuntu the
`.deb` below works the exact list out from the built binary and declares it
itself, so you do not need to keep one.

## Installing

There are no published packages yet. Every route starts from a checkout.

```console
$ git clone https://github.com/overseers-desk/RobCo-Terminal
$ cd RobCo-Terminal
```

Each of the three commands below builds a release binary first if you have
not already, which is the slow part on a cold checkout. Each then runs the
copy it just installed in a scrubbed environment and checks it starts, so a
command that prints a path has already proved that path works.

**Into a prefix.** The plain install, for `~/.local` or `/usr/local` or
anywhere else:

```console
$ cargo run -p xtask -- install --prefix ~/.local
installed robco-term 0.1.0 to /home/you/.local
  /home/you/.local/bin/robco-term
  /home/you/.local/share/applications/robco-term.desktop
  /home/you/.local/share/icons/hicolor/256x256/apps/robco-term.png
  checked: robco-term 0.1.0 runs from the prefix with a clean HOME
```

Three files, and that is the whole installation. Fonts, shaders, presets and
the noise texture are compiled in, and the binary unpacks its shader preset
under `~/.cache` at startup and runs from wherever you put it. `--destdir`
stages under another root if you are packaging. The command warns you if the
prefix's `bin` is off your `PATH`, since the desktop entry launches by name.

**As a tarball**, to move to another machine of the same architecture:

```console
$ cargo run -p xtask -- dist --out-dir dist
wrote dist/robco-term-0.1.0-linux-x86_64.tar.gz (32.5 MiB)
```

It unpacks into one directory holding the same three files. Unpack it
wherever you like and run `bin/robco-term`.

**As a Debian package:**

```console
$ cargo run -p xtask -- deb --out-dir dist
wrote dist/robco-term_0.1.0_amd64.deb
  Depends: libc6 (>= 2.39), libgcc-s1 (>= 4.2), libstdc++6 (>= 13.1), ...
```

Then `sudo dpkg -i dist/robco-term_0.1.0_amd64.deb`. It builds without root,
and its dependencies are read out of the binary rather than maintained by
hand. One caveat worth stating: those versions are the ones on the machine
that built the package, so a package built on a newer distribution declares
bounds an older one cannot satisfy. Build it where you will install it.

To check the install on a machine with no desktop session:

```console
$ unset WAYLAND_DISPLAY
$ xvfb-run -a ~/.local/bin/robco-term -e sleep 5
[INFO app::gpu] swapchain format Bgra8Unorm
[INFO app::window] wgpu adapter NVIDIA GeForce RTX 2070 SUPER on Vulkan
[INFO app::window] glass: TERMINESS_SCALED at 12px x2 scale, 59x25 cells, preset /home/you/.cache/robco-term/preset/robco.slangp
[INFO app::window] channel 1 on page 0 exited
[INFO app::window] the last channel is gone; closing
```

Two of those lines are the ones to look for, and yours will name your own
hardware. `wgpu adapter` is the appliance reporting which GPU and backend it
got. `glass:` is it reporting that it built a grid, in which face, at how many
columns and rows. The rest is startup and shutdown noise. Unsetting
`WAYLAND_DISPLAY` matters: with a live Wayland socket in the environment the
window goes to your real desktop instead of the virtual display.

## First run, and configuring it

Just run `robco-term`. There is no configuration to do first, and no config
file exists until you write one.

When you do want to change something, there is no settings window: the
terminal reads one TOML file, watches it, and reloads the moment you save.
Editing the file is the settings UI. Open it in your editor, change a
number, save, and the glass changes under you while the editor is still
open. A file that does not parse costs you the edit and not the session,
because the terminal keeps the settings it already had and logs the error.

The file lives at `$XDG_CONFIG_HOME/robco-term/config.toml` on Linux, under
`~/Library/Application Support/robco-term/` on macOS, and in `%APPDATA%` on
Windows.

It is a diff against the defaults, so a real one is short:

```toml
[general]
font_scaling = 1.2

[screen]
name = "Deep Blue"
bloom = 0.9

[chassis]
name = "Slide Rule"
```

The `name` key in each of those two tables is the part worth knowing about.
It is not a label. It picks which built-in preset the rest of that table is
measured against, so the file above means the Deep Blue screen with its bloom
turned up, standing in the Slide Rule cabinet. Everything not named comes
from those two presets, which is what keeps the file this short and what
keeps it meaning the same thing on another machine.

Keep a look under a name of your own by putting the two tables in
`config.<name>.toml` beside your config file, then start under it:

```console
$ robco-term --profile workshop
```

The name is read as one of your saved looks first, then as a built-in screen,
so `--profile "Deep Blue"` works without saving anything. A name that is
neither is refused rather than quietly ignored, so you never get the wrong
picture under the right name.

**[docs/config.md](docs/config.md) is the full reference**: every key, its
default, and what it does. If you are writing a program that edits the file
on a user's behalf, [docs/config-format.md](docs/config-format.md) states the
rules it has to obey.

## Keys

`Alt`+digit brings a channel to the screen, `Ctrl`+`Shift`+`T` opens one,
and copy and paste are `Ctrl`+`Shift`+`C` and `Ctrl`+`Shift`+`V` where any
other terminal puts them. [docs/keys.md](docs/keys.md) is the full list, and
it also says which keys this terminal leaves alone.

## Command line

`robco-term --help` lists them all. The ones worth knowing before you read
it: `-e <cmd>` runs a command instead of your shell and swallows every
argument after itself, so put it last; `--program` does the same for a plain
program with no arguments; `--workdir` sets the starting directory;
`--fullscreen` and `--profile` do what they say; and `--default-settings`
starts from the built-in defaults without reading your config file.

A second `robco-term` does not start a second application. It hands its
request to the one already running, which opens another window and exits.

## Status

Version 0.1.0, unreleased. Channels and tmux control mode work against live
tmux; the core passes the conformance suite bar the 8-bit cases xterm fails.

Known gaps, so you find them here rather than by hitting them. Anything but
printable ASCII (box drawing, icons, accents, CJK, emoji) draws a blank cell,
with no box and no log line. A selection can be made and copied, but it is
never drawn. Font size has no keyboard binding, only a config key. The cursor
does not blink, the icon is a placeholder, and a licence file is to come.
Built-in SSH authenticates with your ssh-agent and nothing else, reads none
of `~/.ssh/config`, and refuses an unknown host key rather than asking, so a
password-only host or a config alias stops at a printed refusal; `docs/ssh.md`
carries the full contract.

`cargo run -p xtask -- verify <path-to-binary>` walks a built binary through
the window and CLI contract item by item and tells you which parts this
machine honours. It is the fastest honest answer to "does this work here".
