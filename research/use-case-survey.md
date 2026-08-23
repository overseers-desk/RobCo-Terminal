# Use Case Survey: desktop terminal emulators, 2017–2026

Snapshot taken August 2026. Re-snapshot trigger: a mainstream terminal shipping tmux control mode on Linux, a new retro/CRT terminal reaching the top tier, or six months elapsed.

Sources: Hacker News (launch and comparison threads of cool-retro-term, Ghostty, WezTerm, Alacritty, kitty, Rio, foot, Contour, Warp, Tabby, Hyper, Zutty; the typing-latency threads), the issue trackers of cool-retro-term, Ghostty, WezTerm, kitty, Alacritty, Rio, Contour, Zellij and tmux, Reddit (r/commandline, r/linux, r/unixporn, r/archlinux, r/KittyTerminal, r/wezterm, r/Fedora, r/tmux, r/selfhosted, r/zellij, r/ClaudeCode, r/ClaudeAI), Lobsters, The Register forums, and developer blogs.

This file is the index for the folder. It defines the pain taxonomy: the distinct pains people voice about living with a desktop terminal emulator, numbered once here. `voices.md` maps each quote to a pain number, `competitive-landscape.md` records what the field supplies for each, and `status.md` records which RobCo Terminal covers. The numbers are stable identifiers, not a ranking.

## The pain taxonomy

- **P1 Input latency and stutter.** The keystroke arrives a frame late, the window takes seconds to resize, a large `cat` locks the terminal, scrolling a big file in an editor lags. Felt, measured by hand, and disputed; the thing people switch terminals over and the thing they misattribute to their own tooling.
- **P2 GPU rendering that costs or breaks.** A terminal that needs a GPU and has no software fallback: 30-second lag under llvmpipe, unusable over VNC, overheating a laptop, fighting another GPU application for the driver, crashing on Wayland with a given driver, a memory footprint ten times a VTE terminal's. The "why does a text grid need a GPU" question recurs on every launch.
- **P3 The retro CRT look, wanted and resented.** Wanted: phosphor, curvature, scanlines, glow, inside a terminal fast and complete enough to work in, since the one product that has the look is slow, dependency-heavy, and breaks full-screen programs. Resented: "lipstick on a pig", "the CLI version of an Instagram filter", eye strain after twenty minutes, a toy for tourists.
- **P4 Font rendering: crispness, scale, and ligatures.** Bitmap faces that stay pixel-sharp against TrueType that blurs; blur at 125% and 150% fractional scaling; shader artefacts at non-integer scale; the same font rendering differently in two terminals; ligatures wanted by some and refused by others; a maintainer's bold-weight or glyph-override decision with no opt-out.
- **P5 Many sessions at once.** Tab labels shrunk to two characters, twenty terminal windows diluting attention, no way to say "go to the third one" without reading a bar; in 2025–26, one shell per AI agent and "no idea which one needed my attention".
- **P6 tmux control mode outside iTerm2.** The session's windows as the terminal's own tabs, with tmux's persistence underneath, which iTerm2 has had for a decade and Linux terminals mostly do not: the most-upvoted open issue on Ghostty, declined by Alacritty and kitty, shipped by WezTerm with latency over ssh, and a protocol its implementers call abysmal.
- **P7 Sessions surviving the terminal.** Keeping a shell and its windows alive when the terminal dies or the machine reboots, without hand-rolled tmux scripting; being forced into tmux just to get scrollback.
- **P8 Configuration: a file or a panel, and what happens when you save.** Config-file-only as an adoption barrier, an hour of fiddling before a terminal is usable, options that cannot be found, a restart to see a change, an XML dump no one can diff; against the dotfile-friendly text file people want to carry between machines.
- **P9 Platform reach and desktop fit on Linux.** A GTK4 header bar on KDE or Xfce, a GTK version an LTS distro does not have, Wayland-only or X11-only, nVidia plus Wayland, a 121-dependency build, no distro package for a decade-old project.
- **P10 Table-stakes missing.** Scrollback search, a scrollbar, URL hints, per-machine profiles, the smart-selection and paste conveniences an incumbent accumulated: each one a "dealbreaker" that reverses a switch to a faster terminal.
- **P11 Weight, bloat, and creep.** Electron memory against a 250-kilobyte native terminal; feature bloat; AI features, forced accounts, telemetry, and pricing models arriving in what was a free local tool.
- **P12 Maintainer conduct.** A maintainer who closes the tmux question with "don't", refuses a font option, or answers a security report brusquely, named across unrelated threads as the reason for leaving.
- **P13 Choosing at all.** Thirty-four terminals to evaluate, a paradox of choice, "whatever ships with the distro is fine" at the top of every thread, no trusted up-to-date comparison, and a name that repels adoption.

## Demand by pain

Demand is the frequency and intensity with which the pain is voiced in the corpus. Read it with the caveats below.

| Pain | Demand signal | Evidence (voices.md) | Field supply (competitive-landscape.md) |
|---|---|---|---|
| P1 Latency | Dominant; the largest single cluster on HN, argued on every launch | §1 | Every GPU terminal claims it; two publish numbers |
| P2 GPU cost/breakage | High; the counter-argument to every GPU terminal, with real breakage reports | §2 | Two of the surveyed GPU terminals document a software fallback |
| P3 Retro look | Real on both sides: feature requests at 21–23 reactions on WezTerm, declined on kitty; dismissal at scores 20–34 | §3 | One dedicated product; bring-your-own-shader in three |
| P4 Fonts | High; the richest Reddit cluster, 344-reaction Alacritty RFC | §4 | Bitmap strikes promised by one; fractional-scale blur open across the field |
| P5 Many sessions | High in its 2025–26 form: one shell per AI agent, "which one was thinking vs waiting" at scores 105–318; one measured tab-label issue | §5 | Universal tabs; numbered chords in a few; overview in three; the agent-era answers are tmux overlays |
| P6 Control mode | Very high and concentrated: 731 reactions on one issue, declines on two, a decade of open requests | §6 | iTerm2 only; WezTerm partial |
| P7 Survival | High; "PSA: Use TMUX" at 872, and voiced as tmux's own friction (scrolling, copy, resurrection crashes) | §7 | tmux, Zellij, WezTerm mux, Contour daemon |
| P8 Configuration | High; recurs as the real cost in "speed doesn't matter" replies | §8 | Text file plus live reload is the native default; no diff-against-defaults |
| P9 Linux fit | High on Linux launches (Ghostty 1.0 GTK complaints) | §9 | Uneven; GTK4 terminals pay it, Qt and Rust ones less |
| P10 Table stakes | High; the stated reason switches reverse | §10 | Incumbents have them; new terminals ship without |
| P11 Bloat and creep | High; Warp threads, Electron memory, iTerm2 AI | §11 | Native terminals escape it |
| P12 Maintainers | Moderate; three unrelated threads name the same maintainer | §12 | Not a feature |
| P13 Choosing | High by score (top comments at 121–230), low by intensity | §13 | Not a feature |

## Caveats on reading demand

- **The posting population is switchers and builders.** Someone content with the distro's terminal posts once, at the top of a thread, and is the most-upvoted voice in it; the people whose pain is private (the developer who stares at one terminal all day and never posts) leave no trace. Frequency undercounts satisfaction and overcounts the enthusiast's grievance.
- **The retro-look corpus is one product's.** Almost every quote about CRT terminals is about cool-retro-term, so its bugs (Qt, performance, Wayland) read as the category's limits. The demand for the look in a fast terminal is voiced on other products' trackers, where it is a minority request with a maintainer's answer attached.
- **Control mode demand is one issue's.** The 731 reactions sit on one Ghostty issue; the Linux-wide demand is inferred from that, from the open requests elsewhere, and from a few Reddit threads asking "anybody know of any Linux or Windows terminal" with it, not from a spread of voices.
- **Latency is argued, not measured, by most voices.** The corpus holds one OBS recording and a few published benchmarks; the rest is felt. The counter-segment (P13) answers every latency claim with "I never noticed", at higher scores.

## Sources

- HN, Ghostty 1.0 launch thread, https://news.ycombinator.com/item?id=42517447 (comments cited individually in voices.md)
- HN, "Okay, I Like WezTerm", https://news.ycombinator.com/item?id=41223934
- HN, Alacritty Show HN, https://news.ycombinator.com/item?id=13338592
- HN, cool-retro-term threads, https://news.ycombinator.com/item?id=36798774 and https://news.ycombinator.com/item?id=30734137
- HN, Rio launch, https://news.ycombinator.com/item?id=36057687
- HN, "Measured: Typing latency of Zutty", https://news.ycombinator.com/item?id=35807660
- GitHub, ghostty-org/ghostty #1935 (tmux control mode), https://github.com/ghostty-org/ghostty/issues/1935
- GitHub, alacritty/alacritty #2410, kovidgoyal/kitty #4842 and #2422, wezterm/wezterm #336, #5182, #6985, #6806, #8052, Swordfish90/cool-retro-term #117, #235, #347, #740, #925, #934
- Reddit, r/commandline "why is kitty's font rendering so weird", https://old.reddit.com/r/commandline/comments/1iysy73/
- Reddit, r/commandline "Microsoft's Windows Terminal is getting retro-style", https://old.reddit.com/r/commandline/comments/emjo8p/
- Reddit, r/linux "GPU based terminal and is there really an advantage", https://old.reddit.com/r/linux/comments/1ibs1nq/
- Lobsters, "Rio Terminal: a hardware-accelerated GPU terminal emulator", https://lobste.rs/s/7a4lle/
- Jeff Quast, "State of Terminal Emulators in 2025", https://www.jeffquast.com/post/state-of-terminal-emulation-2025/
