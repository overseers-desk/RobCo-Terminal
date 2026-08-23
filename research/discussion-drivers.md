# What drives discussion volume on terminal-emulator posts

Snapshot taken August 2026. Companion to `use-case-survey.md`. That file surveys the pains people voice about terminal emulators. This one asks what a terminal launch draws on Hacker News, which kinds of post draw replies rather than upvotes, and what the precedent is for a retro CRT terminal in particular.

The data is the Hacker News front page as the Algolia search API returns it (stories matching the tool names in `competitive-landscape.md` plus "terminal emulator", "tmux control mode", "terminal emulator latency", with 40 points or more; August 2026). Score, comment count, and comments-per-score are recorded together because comments-per-score is the discussion intensity signal: upvotes measure passing approval, comments measure engagement. Reddit is tabulated for the sixteen threads the voices were drawn from, not as a search of the venue.

## Posts in the corpus

### The retro CRT terminal's own record

| Post | Date | Score | Comments | C/S |
|---|---|---|---|---|
| [Cool-retro-term: A terminal emulator which mimics the old cathode display](https://news.ycombinator.com/item?id=8399461) | 2014-10-02 | 246 | 85 | 0.35 |
| [Cool-retro-term](https://news.ycombinator.com/item?id=9093545) | 2015-02-23 | 176 | 47 | 0.27 |
| [Cool Retro Terminal](https://news.ycombinator.com/item?id=17413911) | 2018-06-28 | 107 | 33 | 0.31 |
| [Cool-retro-term: A terminal emulator which mimics the old cathode display](https://news.ycombinator.com/item?id=30734137) | 2022-03-19 | 282 | 71 | 0.25 |
| [Cool Retro Terminal](https://news.ycombinator.com/item?id=36798774) | 2023-07-20 | 305 | 89 | 0.29 |
| [Cool-retro-term: terminal emulator which mimics look and feel of CRTs](https://news.ycombinator.com/item?id=46036895) | 2025-11-24 | 298 | 118 | 0.40 |
| [EDEX-UI: science fiction terminal emulator](https://news.ycombinator.com/item?id=23747721) | 2020-07-06 | 204 | 35 | 0.17 |
| [Ratty – A terminal emulator with inline 3D graphics](https://news.ycombinator.com/item?id=48093100) | 2026-05-11 | 678 | 245 | 0.36 |

One product, six front pages in twelve years, each at 100–300 points and 33–118 comments, with no new feature behind any of them. The look re-launches itself. The comment ratio sits at 0.25–0.40: people upvote the picture and a third of them say something, which on reading (voices.md §3) is "neat for a few minutes", "not a daily driver", and one person reporting it overheated a laptop.

### Mainstream terminal launches and releases

| Post | Date | Score | Comments | C/S |
|---|---|---|---|---|
| [Ghostty 1.0](https://news.ycombinator.com/item?id=42517447) | 2024-12-26 | 2,319 | 681 | 0.29 |
| [Show HN: Alacritty, a GPU-accelerated terminal emulator written in Rust](https://news.ycombinator.com/item?id=13338592) | 2017-01-06 | 1,170 | 476 | 0.41 |
| [Show HN: Warp, a Rust-based terminal](https://news.ycombinator.com/item?id=30921231) | 2022-04-05 | 946 | 726 | 0.77 |
| [Ghostty – Terminal Emulator](https://news.ycombinator.com/item?id=47206009) | 2026-03-01 | 863 | 359 | 0.42 |
| [Okay, I Like WezTerm](https://news.ycombinator.com/item?id=41223934) | 2024-08-12 | 488 | 275 | 0.56 |
| [Kitty – a fast, featureful, GPU based terminal emulator](https://news.ycombinator.com/item?id=17915829) | 2018-09-05 | 452 | 315 | 0.70 |
| [Zellij – A Terminal Workspace and Multiplexer](https://news.ycombinator.com/item?id=26902430) | 2021-04-22 | 414 | 239 | 0.58 |
| [Kitty – A fast, featureful, GPU based terminal emulator](https://news.ycombinator.com/item?id=24643008) | 2020-09-30 | 385 | 195 | 0.51 |
| [Rio: Terminal app built over WebGPU, WebAssembly and Rust](https://news.ycombinator.com/item?id=36057687) | 2023-05-24 | 207 | 124 | 0.60 |
| [Contour: Modern and fast terminal emulator](https://news.ycombinator.com/item?id=37809834) | 2023-10-08 | 181 | 191 | 1.06 |
| [WezTerm – A GPU-accelerated cross-platform terminal emulator and multiplexer](https://news.ycombinator.com/item?id=26633708) | 2021-03-30 | 181 | 123 | 0.68 |
| [Foot – A fast, lightweight and minimalistic Wayland terminal emulator](https://news.ycombinator.com/item?id=37622997) | 2023-09-23 | 179 | 136 | 0.76 |
| [Alacritty – A fast, cross-platform, OpenGL terminal emulator](https://news.ycombinator.com/item?id=40437535) | 2024-05-22 | 152 | 167 | 1.10 |
| [Show HN: Wave – Modern Open-Source Terminal](https://news.ycombinator.com/item?id=38701899) | 2023-12-19 | 82 | 90 | 1.10 |
| [Rio Terminal: A hardware-accelerated GPU terminal emulator](https://news.ycombinator.com/item?id=45432977) | 2025-10-01 | 73 | 57 | 0.78 |
| [Attyx – tiny and fast GPU-accelerated terminal emulator written in Zig](https://news.ycombinator.com/item?id=47155772) | 2026-02-25 | 19 | 16 | 0.84 |

### Electron terminals

| Post | Date | Score | Comments | C/S |
|---|---|---|---|---|
| [Hyper 2, Electron based terminal](https://news.ycombinator.com/item?id=16900941) | 2018-04-23 | 106 | 226 | 2.13 |
| [Black Screen – Both a terminal emulator and an interactive shell](https://news.ycombinator.com/item?id=14253906) | 2017-05-03 | 163 | 163 | 1.00 |
| [Tabby – A Terminal for the Modern Age](https://news.ycombinator.com/item?id=29553767) | 2021-12-14 | 68 | 103 | 1.51 |
| [Tabby is a customizable cross-platform terminal app](https://news.ycombinator.com/item?id=35111397) | 2023-03-11 | 98 | 90 | 0.92 |

### Grievance, governance, and comparison posts

| Post | Date | Score | Comments | C/S |
|---|---|---|---|---|
| [Ghostty is leaving GitHub](https://news.ycombinator.com/item?id=47939579) | 2026-04-28 | 3,521 | 1,051 | 0.30 |
| [iTerm2 critical security release](https://news.ycombinator.com/item?id=42579472) | 2025-01-02 | 671 | 438 | 0.65 |
| [iTerm2 and AI Hype Overload](https://news.ycombinator.com/item?id=40432446) | 2024-05-21 | 175 | 307 | 1.75 |
| [Use Alacritty instead of Termite](https://news.ycombinator.com/item?id=27075304) | 2021-05-07 | 249 | 323 | 1.30 |
| [Rust maintainer perfectionism, or, the tragedy of Alacritty (2020)](https://news.ycombinator.com/item?id=29349240) | 2021-11-26 | 267 | 233 | 0.87 |
| [State of Terminal Emulators in 2025: The Errant Champions](https://news.ycombinator.com/item?id=45799478) | 2025-11-03 | 267 | 273 | 1.02 |
| [Measured: Typing latency of Zutty compared to other terminal emulators (2021)](https://news.ycombinator.com/item?id=35807660) | 2023-05-03 | 89 | 76 | 0.85 |
| [Warp terminal – no more login required](https://news.ycombinator.com/item?id=42247583) | 2024-11-26 | 73 | 107 | 1.47 |
| [Warp Terminal changes pricing model](https://news.ycombinator.com/item?id=45772558) | 2025-10-31 | 39 | 85 | 2.18 |
| [iTerm2 removes AI feature from core, creates separate plugin](https://news.ycombinator.com/item?id=40458135) | 2024-05-23 | 104 | 138 | 1.33 |
| [ITerm2 is now integrated with tmux terminal multiplexer](https://news.ycombinator.com/item?id=3498163) | 2012-01-22 | 111 | 27 | 0.24 |

### Reddit, the threads the voices were drawn from

Sixteen Reddit posts whose threads supplied quotes in `voices.md`, fetched individually in August 2026 (score and comment count as Reddit shows them).

| Post | Subreddit | Date | Score | Comments | C/S |
|---|---|---|---|---|---|
| [PSA: Use TMUX.](https://old.reddit.com/r/selfhosted/comments/1bbw6ta/psa_use_tmux/) | selfhosted | 2024-03-11 | 871 | 233 | 0.27 |
| [built an agent orchestrator within tmux](https://old.reddit.com/r/tmux/comments/1s6oze9/built_an_agent_orchestrator_within_tmux/) | tmux | 2026-03-29 | 319 | 53 | 0.17 |
| [I got tired of managing 15 terminal tabs for my Claude sessions, so I built Agent Deck](https://old.reddit.com/r/ClaudeCode/comments/1pxyn37/i_got_tired_of_managing_15_terminal_tabs_for_my/) | ClaudeCode | 2025-12-28 | 318 | 93 | 0.29 |
| [Alacritty vs Kitty](https://old.reddit.com/r/archlinux/comments/n9noje/alacritty_vs_kitty/) | archlinux | 2021-05-11 | 231 | 162 | 0.70 |
| [[OC] CRTty - Retro CRT shader for kitty (also a framework)](https://old.reddit.com/r/unixporn/comments/1r32pfq/oc_crtty_retro_crt_shader_for_kitty_also_a/) | unixporn | 2026-02-12 | 165 | 10 | 0.06 |
| [What are the meaningful differences between modern terminal emulators?](https://old.reddit.com/r/linux/comments/1hn700x/what_are_the_meaningful_differences_between/) | linux | 2024-12-27 | 152 | 117 | 0.77 |
| [kitty - the fast, featureful, GPU based terminal emulator](https://old.reddit.com/r/commandline/comments/rehc8g/kitty_the_fast_featureful_gpu_based_terminal/) | commandline | 2021-12-12 | 136 | 76 | 0.56 |
| [GPU based terminal and is there really an advantage.](https://old.reddit.com/r/linux/comments/1ibs1nq/gpu_based_terminal_and_is_there_really_an/) | linux | 2025-01-28 | 116 | 92 | 0.79 |
| [What's the best terminal emulator? and why is gnome-terminal (default in ubuntu), not sufficient?](https://old.reddit.com/r/linux/comments/1aud0lb/whats_the_best_terminal_emulator_and_why_is/) | linux | 2024-02-19 | 115 | 225 | 1.96 |
| [I got tired of managing 10+ terminal tabs for my Claude sessions, so I built agent-view](https://old.reddit.com/r/ClaudeAI/comments/1rb4jvs/i_got_tired_of_managing_10_terminal_tabs_for_my/) | ClaudeAI | 2026-02-21 | 102 | 25 | 0.25 |
| [GitHub - Swordfish90/cool-retro-term: A good looking terminal emulator which mimics the old cathode display...](https://old.reddit.com/r/commandline/comments/8iqg5g/github_swordfish90coolretroterm_a_good_looking/) | commandline | 2018-05-11 | 97 | 12 | 0.12 |
| [Does anyone use electron based terminal emulators?](https://old.reddit.com/r/linux/comments/1k5t3om/does_anyone_use_electron_based_terminal_emulators/) | linux | 2025-04-23 | 72 | 182 | 2.53 |
| [Microsoft's Windows Terminal is getting retro-style CRT effects, search, and more](https://old.reddit.com/r/commandline/comments/emjo8p/microsofts_windows_terminal_is_getting_retrostyle/) | commandline | 2020-01-10 | 61 | 43 | 0.70 |
| [Kitty vs Ghostty - Terminal Emulators](https://old.reddit.com/r/commandline/comments/1htimkk/kitty_vs_ghostty_terminal_emulators/) | commandline | 2025-01-04 | 56 | 57 | 1.02 |
| [Why is kitty's font rendering so weird compared to every other terminal](https://old.reddit.com/r/commandline/comments/1iysy73/why_is_kittys_font_rendering_so_weird_compared_to/) | commandline | 2025-02-26 | 28 | 28 | 1.00 |
| [Is Ghostty using GTK for linux a drawback?](https://old.reddit.com/r/linux/comments/1hskl9f/is_ghostty_using_gtk_for_linux_a_drawback/) | linux | 2025-01-03 | 0 | 59 | n/a |

The same shape as HN. The two retro-look posts (CRTty, cool-retro-term) are the lowest-ratio rows at 0.06 and 0.12: upvoted pictures. The questions that take a side ("why isn't gnome-terminal sufficient", "does anyone use Electron terminals", "Kitty vs Ghostty") sit at 1.0–2.5. The agent-session dashboards draw 300-point approval at 0.17–0.29.

## What pulls comments

Four patterns recur in the high-ratio posts, and one absence in the low-ratio ones.

**1. A grievance against a named product or practice.** Warp's pricing (2.18), Hyper's Electron (2.13), iTerm2's AI (1.75), Warp's login (1.47), iTerm2's AI plugin (1.33), Termite's maintainer (1.30). The grievance gives the reader a side to take; "I uninstalled it too" is a complete reply.

**2. A benchmark or comparison that invites self-comparison.** "State of Terminal Emulators" (1.02), the Zutty latency measurement (0.85), the Alacritty tragedy post (0.87), Contour's launch framed against the fast-terminal claim (1.06). The number or ranking becomes a test the comment section runs on its own machine.

**3. A contested premise.** Every GPU-terminal launch draws the "why does a text grid need a GPU" reply (voices.md §2, §13) and the "I've never noticed latency" reply, and those two arguments fill the thread. Kitty (0.70), foot (0.76), WezTerm (0.68), Rio (0.60–0.78) all sit in the 0.5–0.8 band for that reason; the bare speed claim is an argument starter.

**4. A maintainer.** The kitty and Alacritty maintainers are named as reasons to switch across unrelated threads (voices.md §12), and a launch post for either product carries that discussion into its comments.

The absence on the low-ratio side: a picture with nothing to argue about. The CRT terminal's six front pages sit at 0.25–0.40; EDEX-UI at 0.17; the tmux-integration announcement of 2012 at 0.24; on Reddit, the two retro-look posts at 0.06 and 0.12. People upvote the look, the "neat for a minute" remark gets written once, and the thread ends. The score is high and the discussion is short.

## What a retro CRT terminal can expect

The precedent is specific. A CRT terminal reaches the HN front page on its looks alone, repeatedly, at 250–300 points and 70–120 comments, without a grievance hook and without a new feature. No other product in the corpus re-launches on an unchanged repository that often. The ceiling is also specific: the comments are the same each time (novelty, not a daily driver, the GPU cost), and the product that drew them has not converted the attention into a daily-use userbase the corpus can see; its issue tracker carries decade-old requests for packages and performance.

Three of the four comment drivers are available to a CRT terminal that is also a working terminal: a benchmark (keystroke-to-glyph latency under the effects, which no surveyed product publishes, P1), a contested premise stated plainly ("a CRT terminal you can work in all day", against the corpus's "not a daily driver"), and a comparison against the one incumbent (cool-retro-term's open issues as the list of what is fixed). The fourth driver, a grievance, is the tmux control mode gap on Linux (P6): 731 reactions on one open issue, two maintainers' refusals, and a decade since iTerm2 shipped it, which is a thread that fills itself.

The positioning moves these findings imply are recorded in `status.md`, since they are product strategy rather than market fact.

## Method note

HN figures from the Algolia API (`/api/v1/search?tags=story`, `/api/v1/items/<id>`), August 2026, comment counts as Algolia reports them. Reddit figures fetched per post in August 2026 through the reddit.com skill's two-step path, one post at a time. The Reddit multi-session sweep failed on browser contention and was re-run serially; its entries are in `voices.md`.
