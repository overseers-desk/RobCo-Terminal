# Competitive Landscape: terminals that ship a look

Snapshot taken August 2026.

A competitor here is a terminal that shows a retro or CRT picture to the person who installs it, and then has to be functional enough to work in. That is the pairing this project is built on, so it is the pairing the comparison is drawn against. A terminal that merely accepts a shader the user writes or sources ships no look and is not in the table; nor is a fast terminal with no picture at all. Both are recorded below under the survey, because the per-pain supply notes read the whole field and the numbers there are the basis for what is scarce.

Capabilities are as their authors describe them in READMEs, docs and launch posts, not independently re-verified; cells that could not be confirmed read "unconfirmed". Star counts and release dates are approximate and omitted where unknown. Pain numbers (P1-P13) cross-reference `use-case-survey.md`. RobCo Terminal's own coverage is not in this file; it lives in `status.md`.

Re-snapshot trigger: a terminal that ships a retro look reaching the top tier, one abandoned, a bring-your-own-shader terminal shipping a preset look of its own, or six months elapsed.

## The competitors

Four products present a retro picture without the user supplying a shader. Cells: Yes / No / Partial / unconfirmed, with a short note. Columns are the pains; P12 (maintainer conduct) and P13 (choosing) are not features and have no column.

| Tool | Platform / UI | Latency claim (P1) | GPU need, fallback (P2) | CRT / retro look (P3) | Bitmap fonts, scale (P4) | Many sessions (P5) | tmux control mode (P6) | Survival (P7) | Config model (P8) | Linux fit, packaging (P9) | Table stakes (P10) | Weight (P11) |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| cool-retro-term | Qt6/QML, Linux + macOS, X11 | none | GPU via Qt scene graph; no fallback; idle draw 17–20% (PR #946) | Yes: built-in fixed CRT chain, presets; no user shaders | unconfirmed (bitmap only by recompiling, #740) | No tabs or splits (#247) | unconfirmed | No | GUI context menu, preset themes | AppImage, distro repos, Snap; Wayland crashes open (#925, #934) | Partial (qtermwidget basics) | Qt, 121 deps per a WezTerm user |
| Cathode | Cocoa, macOS only | unconfirmed | unconfirmed | Yes (third-party accounts) | unconfirmed | unconfirmed | unconfirmed | unconfirmed | GUI | N/A | unconfirmed | Discontinued ~2017 |
| Hyper + `cool-retro-hyper` | Electron (React) | none | none | Yes via the plugin: bloom, burn-in, jitter, curvature, scanlines; an optional monitor frame | unconfirmed | Tabs; splits via plugins | Declined (#2978, "no public spec"); own headless mux | No | `~/.hyper.js` + GUI; hot reload | deb, rpm, AppImage, Snap | Partial | Electron; last release Jul 2024 |
| Windows Terminal | Win32/WinUI, Windows only | "fast and efficient" | Direct3D | Yes: `experimental.retroTerminalEffect` (glow, scanlines); custom HLSL on top | DirectWrite outline | Tabs, splits, palette, jump-to-tab-N | #3656, #5612 open | App-state tabs | GUI + `settings.json`; live reload | N/A | Yes | ~104.7k stars |

Cathode (macOS, discontinued) and Windows Terminal (Windows) are one-platform products this project cannot be installed beside. On Linux the set is cool-retro-term and an Electron plugin.

Links: github.com/Swordfish90/cool-retro-term; hyper.is with github.com/jonathanbell/cool-retro-hyper; github.com/microsoft/terminal.

## Where a look can be brought

Terminals that render a user-supplied shader and ship no picture of their own. They are the route by which someone who wants the look on a fast terminal gets it, so they bound how scarce the look is, without being products that offer it.

- **Ghostty**: `custom-shader` takes user GLSL. GTK4/OpenGL on Linux, Metal on macOS.
- **Rio**: RetroArch `.slangp` shader packs through config, on the wgpu backend and not on GL.
- **WezTerm**: a CRT WGSL pull request (#7649) is open and unmerged.
- **kitty**: declined (#4842, "Not something I am interested in"); a third-party `CRTty` LD_PRELOAD exists outside the project.

## The rest of the field, surveyed

Not competitors, and recorded because the supply notes below read the whole category and because the functional references live here. One line each: what it is, and what it is the reference for.

- **Alacritty**: Rust/OpenGL, claims "the fastest terminal emulator in existence" with no published figure; no tabs, no scrollback search, no scrollbar, all by design. ~65.5k stars.
- **foot**: C, CPU-rendered, Wayland only. The one product whose docs promise bitmap strikes from freetype. Server/footclient outlives the client. ~2.1k on Codeberg.
- **Contour**: C++, documented software rasterizer fallback, and a daemon that "interoperates with tmux in both directions" and survives window closure. ~3k stars.
- **WezTerm**: Rust, documented software front-end, FreeType bitmap strikes by default, built-in mux server, tmux control mode in nightlies since March 2025 (#336 open, ssh latency #6806). ~28.5k stars.
- **kitty**: C/Python OpenGL; author benchmark 134.55 MB/s against gnome-terminal's 61.83; scalable fonts only by design (#97, #1295); multiplexers called "a bad idea" (#2170). ~34.6k stars.
- **Ghostty**: native Zig; "same performance category" as Alacritty; tmux control mode #1935 open at 731 reactions with a parser begun; GTK4 header bar on KDE and Xfce. ~60k stars, 1.3.1 Mar 2026.
- **Rio**: Rust/wgpu, "minimal GPU" mode, tabs and split panes, Steam Deck among its targets. ~7.4k stars.
- **Warp**: Rust GPU with blocks, AI features and accounts; own SSH extension rather than control mode. 64.4k stars after open-sourcing May 2026.
- **Tabby**, **Wave Terminal**: Electron; Tabby has canvas fallback from WebGL and quake-style tabs, Wave has drag-and-drop blocks with durable ssh blocks. ~74.1k stars, and a tarball.
- **iTerm2**: Cocoa, macOS only. The tmux control-mode reference: windows to tabs, splits to splits, a tmux Dashboard, a decade old.
- **Konsole**, **GNOME Terminal / Ptyxis**: the desktop defaults, and the reference for accumulated table stakes. Ptyxis has a tab overview with previews; Ubuntu disables bitmap fonts by default.
- **Zed terminal**: a panel in an editor, Rust GPUI, ~2 ms and 120 fps by its blog; refuses llvmpipe by default; rejected embedding tmux (#50584).
- **tmux**: C, inside any terminal. The reference for three things: `-C`/`-CC` with a documented protocol, detach and reattach, and prefix+number as the numbered-slot model. ~48k stars, 3.7c Jul 2026.
- **Zellij**: Rust, inside any terminal. Tabs, panes, floating and stacked, a session manager, and Session Resurrection across reboot. ~35.1k stars.

Links: alacritty.org; codeberg.org/dnkl/foot; contour-terminal.org; wezterm.org; sw.kovidgoyal.net/kitty; ghostty.org; rioterm.com; warp.dev; tabby.sh; waveterm.dev; iterm2.com/documentation-tmux-integration.html; konsole.kde.org; gitlab.gnome.org/chergert/ptyxis; zed.dev/docs/terminal; github.com/tmux/tmux; zellij.dev.

## Field supply by pain

How much of the surveyed category serves each pain, and how. The absences are stated as absences from the surveyed set.

**P1, latency.** Every GPU terminal claims it; kitty and Zed publish numbers, Alacritty claims "fastest" without them, Contour declines the framing. The corpus disputes all of it (a benchmark showing Ghostty worst on input latency; users going back to Terminal.app). No surveyed terminal publishes keystroke-to-glyph latency under visual effects; the CRT product's latency is described as "unbearable" and has no published figure.

**P2, GPU cost and breakage.** WezTerm and Contour are the two GPU terminals that document a software rendering fallback; the rest require a working GPU stack and say nothing about VMs or VNC. cool-retro-term's idle GPU draw and Wayland crashes are open issues. Zed refuses an emulated GPU by default.

**P3, the retro look.** Four products present one: a dedicated Qt terminal, a discontinued macOS one, an Electron plugin set, and an experimental Windows flag. Ghostty and Rio render a shader the user brings; WezTerm has an unmerged pull request, and kitty declined. Phosphor persistence with a decaying ghost, a degauss on switch, or any effect beyond the in-glass post-process is documented in none of the surveyed products. A chassis, bezel or cabinet drawn around the glass is documented in none of them either; the `cool-retro-hyper` plugin's optional monitor frame is the nearest thing.

**P4, fonts.** foot is the one product whose docs promise bitmap strikes; WezTerm loads them by default; kitty refuses them by design; Ghostty, Alacritty and Rio carry open or contested issues. Blur at fractional scaling is open across the field (Ghostty #1938 closed after long debugging, kitty #7513 disputed, WezTerm #5149 open). A bitmap face drawn from its embedded strike at an integer multiple, never re-rasterised, is documented in none of the surveyed products. Ligatures are served by kitty, WezTerm, Ghostty; refused by Alacritty and foot.

**P5, many sessions.** Tabs are universal outside Alacritty and foot. Numbered-slot chords are default-bound in tmux, Zellij, Ghostty and Windows Terminal; a tab overview exists in WezTerm, Ptyxis and Windows Terminal's palette. A persistent numbered strip visible without a tab bar, with digit chords to jump and to move, is tmux's model; no surveyed GUI terminal draws one as chrome outside the text area. Among the terminals that ship a look, cool-retro-term has neither tabs nor splits.

**P6, tmux control mode.** iTerm2 is the reference and, on macOS, the only complete one. WezTerm ships control mode in nightlies (March 2025 onward) with an open ssh-latency bug and windows landing in the same tab row. Ghostty's #1935 is open at 731 reactions with a parser begun. Alacritty, kitty and Hyper declined; Tabby, Windows Terminal, Konsole and Zellij carry open requests. On Linux, the surveyed field offers one partial native implementation (WezTerm nightly), and none of the terminals that ship a look offers any.

**P7, survival.** tmux and Zellij (resurrection across reboot) are the reference; WezTerm's mux server and Contour's daemon are the independent mechanisms. No surveyed GUI terminal recovers a session's windows and screens from a tmux server after the terminal itself dies, without a daemon of its own, except through iTerm2's control mode on macOS.

**P8, configuration.** A text file with live reload is the native default (Ghostty, WezTerm, kitty, Alacritty, Rio, foot, Contour); GUI settings are the desktop and Electron default, and cool-retro-term and Cathode, the dedicated retro terminals, are both GUI-only. A config written as a diff against a named preset, so a file stays short and means the same on another machine, is claimed by none of the surveyed products; Ghostty's `+show-config --default` dump comes closest. Hot reload on save is common; a parse error keeping the last good state is documented by few.

**P9, Linux fit.** The GTK4 terminals pay the desktop-consistency cost on KDE and Xfce; the Qt and Rust ones less. Wayland-only (foot) and X11-only (cool-retro-term) both draw complaints. cool-retro-term has had an open distro-packaging request since 2014.

**P10, table stakes.** The incumbents (iTerm2, Konsole, GNOME Terminal) have them; new GPU terminals ship without scrollback search, scrollbars, or URL hints and add them later under complaint. Of the terminals that ship a look, only Windows Terminal has the full set.

**P11, weight.** Native Rust, C and Zig terminals escape it; the Electron trio, Warp's AI and accounts, and iTerm2's memory and AI features carry it. cool-retro-term (Qt, 121 dependencies) and Hyper (Electron) are the heavy ones among the terminals that ship a look.

## What the pairing costs the field

The products that ship a look are weak on exactly the functional pains, and the strong functional terminals ship no look. cool-retro-term has no tabs, no splits, an open X11-only complaint and an unbearable-latency reputation. Hyper is Electron and last released in July 2024. Windows Terminal is functional and Windows-only. Nothing in the surveyed set is both.

No surveyed product is built on a hardware metaphor beyond the glass. The CRT effect in every one of them is a flat post-process over the text area; none maps the pointer back through the curvature, draws a cabinet, or degausses on a switch.
