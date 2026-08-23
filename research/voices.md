# Voices

Verbatim quotes from public discussion about living with a desktop terminal emulator. Snapshot built August 2026, drawn from Hacker News, the issue trackers of the mainstream and retro terminals, Reddit, Lobsters, The Register forums and developer blogs. Each entry maps to a pain number from `use-case-survey.md`.

The corpus exists so feature copy speaks in words users already use. When writing about a feature, grep this file for the pain number and read the user's own language before drafting. The categories below are the pains people voice; `use-case-survey.md` defines the taxonomy (P1-P13) they map to.

An issue a maintainer declined ("not something I am interested in", "no desire to implement") is noted as such: it shows a maintainer turning away a need users plainly feel, which is the opening a third-party product fills.

Venues use their own engagement figure: Reddit and HN scores, GitHub reaction counts.

---

## 1. Input latency and stutter → P1
> "I always assumed that it was my bloated vim and tmux configs that made it feel a bit sluggish sometimes, but it turns out i was the terminal. Now everything feels instantaneous."
> — uvesten, Hacker News, January 2017
> https://news.ycombinator.com/item?id=13343402
> Context: Alacritty Show HN thread.
> Maps to: P1

> "Something about my tmux, or vim setup didn't play particularly well in Hyper. The end result was cusor movement in Hyper/tmux/vim was painfully slow."
> — igreulich, Hacker News, January 2017
> https://news.ycombinator.com/item?id=13521807
> Context: Hyper (Electron terminal) launch thread.
> Maps to: P1, P11

> "I believe the most important thing when it comes to performance of a terminal, is the startup performance. And here, Alacritty just sucks, I literally see a white screen for 0.5 to 0.8 seconds each time I start a new instance."
> — u/Glum_Juggernaut_1759, r/rust, June 2022 (score 3)
> https://old.reddit.com/r/rust/comments/hach2x/wezterm_a_gpuaccelerated_crossplatform_terminal/
> Context: on switching from Alacritty to WezTerm.
> Maps to: P1

> "I can only offer anecdote and no concrete measurements. But I've used bad terminal emulators and good ones, and I can really notice the difference. When the letters appear even a few frames after they should, there's a definite (if mild) feeling of discomfort. Like some kind of sensory-motor 'friction'. I can still get work done, but I'm just a little less happy."
> — _dain_, Hacker News, May 2023
> https://news.ycombinator.com/item?id=35812619
> Context: "Measured: Typing latency of Zutty compared to other terminal emulators".
> Maps to: P1

> "I found the input latency unbearable."
> — blueflow, Hacker News, July 2023
> https://news.ycombinator.com/item?id=36801046
> Context: cool-retro-term discussion thread; the one-line verdict on the CRT terminal as a working tool.
> Maps to: P1, P3

> "Long term Gnome Terminal user, I recently tried Alacritty, ugh, massive difference. Went back to Gnome after a couple of days. Like typing through molasses. Terminals can be slow."
> — roydivision, Hacker News, October 2023
> https://news.ycombinator.com/item?id=37817759
> Context: Contour launch thread.
> Maps to: P1

> "I recorded my screen + microphone input using OBS to compare they recorded keystroke sound and appearance of the character on my screen. I was able to confirm my suspicion in that WezTerm yielded higher input latency. This was especially noticeable when using helix inside of WSL..."
> — adem, Hacker News, August 2024
> https://news.ycombinator.com/item?id=41227089
> Context: "Okay, I Like WezTerm" thread; a user measuring latency by hand.
> Maps to: P1

> "The reason I switched from iTerm2 is it took multiple seconds to resize a window on macOS. ... iTerm2 would beachball while I waited multiple seconds for it to resize the window. Wezterm moves instantly."
> — 1-more, Hacker News, December 2024
> https://news.ycombinator.com/item?id=42523772
> Context: Ghostty 1.0 launch thread.
> Maps to: P1

> "After testing Ghostty out for a while, though, I've realized that input lag is higher than xfce4-terminal, font rendering is blurrier with equivalent settings, the UI is still less consistent with my desktop, and it has three to four times the memory footprint on top of all that. ... Disabling the UI cruft just turns it into a less performant version of Xterm, so unfortunately, this is going to be an uninstall for me."
> — Gormo, Hacker News, December 2024
> https://news.ycombinator.com/item?id=42550819
> Context: Ghostty 1.0 launch thread, after tuning.
> Maps to: P1, P4, P9

> "I noticed the CPU load is quite a bit higher than the Windows Terminal when typing... I only began to test this because I noticed the input latency when typing felt more sluggish than the Windows Terminal."
> — @nickjj, wezterm/wezterm issue #6818, March 2025
> https://github.com/wezterm/wezterm/issues/6818
> Context: "15% CPU usage on Wezterm vs 1% CPU usage on Windows Terminal when holding down a key"; open.
> Maps to: P1

> "I use Neovim on 5k and 6k displays, and Roxterm/GnomeTerminal/Apple Terminal/iterm2/tmux all add noticeable lag when scrolling large files."
> — jitl, Lobsters, September 2025
> https://lobste.rs/s/7a4lle/rio_terminal_hardware_accelerated_gpu
> Context: reply to a commenter asking what problem fast terminals solve.
> Maps to: P1

> "Also I don't need high FPS when I accidentally cat a giant file or whatever. What I really want is minimal input latency. I switched from iTerm2 to Ghostty and then eventually back to Terminal.app because terminal input latency really bothers me."
> — nerdponx, Hacker News, October 2025
> https://news.ycombinator.com/item?id=45441781
> Context: Rio-vt launch thread.
> Maps to: P1

> "According to this (at least 11 months old) benchmark, Ghostty has the worst input latency across all contenders"
> — ivanjermakov, Hacker News, March 2026
> https://news.ycombinator.com/item?id=47207375
> Context: pushback against the claim that Ghostty is the fastest terminal.
> Maps to: P1

---

## 2. GPU rendering that costs or breaks → P2
> "The terminal flashes every few seconds if a program with a continually updating multi-line output is running (e.g. `saldl`). I consider this one of the ultimate performance tests of terminal emulators ;)"
> — u/acc_test, r/rust, January 2017 (score 10)
> https://old.reddit.com/r/rust/comments/5mf2yh/announcing_alacritty_a_gpuaccelerated_terminal/
> Context: the Alacritty announcement; render glitches on first try.
> Maps to: P2

> "cool-retro-term was maxing out the integrated Intel GPU which caused the entire system to stutter and lag... both the MBP and my current XPS 15 are unable to drive cool-retro-term on a 4k display with the CPU integrated graphics, and they both overheat and throttle if I use the nvidia graphics card"
> — the_pwner224, Hacker News, November 2019
> https://news.ycombinator.com/item?id=21415465
> Context: a laptop performance mystery traced to the CRT terminal.
> Maps to: P2, P3

> "I love Alacritty, but it has some awful bugs (in the rendering or windowing library they use, not Alacritty itself) with nVidia + Wayland."
> — packetlost, Hacker News, December 2021
> https://news.ycombinator.com/item?id=29566408
> Context: foot launch thread.
> Maps to: P2, P9

> "For me, GPU acceleration is a con because it means I can't use it under a VNC session. I just stick to Xterm."
> — u/[deleted], r/commandline, December 2021 (score 2)
> https://old.reddit.com/r/commandline/comments/rehc8g/kitty_the_fast_featureful_gpu_based_terminal/
> Context: kitty release thread.
> Maps to: P2

> "I'm running this on a Raspberry Pi4 with OS lite... CPU hovers around 45-56%. You can drop it down to <10% by turning the effects off"
> — @wile1411, Swordfish90/cool-retro-term issue #235, December 2022
> https://github.com/Swordfish90/cool-retro-term/issues/235
> Context: the performance thread, open since 2015.
> Maps to: P2, P3

> "In this picture, ghostty (GTK) uses 812 MiB compared to 84 MiB used by xfce4-terminal (also GTK). To reproduce, open 20 ghostty terminals and then close all of them except for 1... it still only used 84 MiB of RSS."
> — @andrewrk, ghostty-org/ghostty issue #254, August 2023
> https://github.com/ghostty-org/ghostty/issues/254
> Context: "tracking issue: ghostty uses too much memory"; maintainer: "GPU-based terminals definitely have to use a bit more RAM." Closed after fixes.
> Maps to: P2, P11

> "I used WezTerm for a while and loved it, but then I discovered it had some strange interactions with other programs that use the GPU or OpenGL. In my case, when running WezTerm, the robotics simulation tool Gazebo Classic would only launch properly 1/3 to 1/2 of the time..."
> — el_memorioso, Hacker News, August 2024
> https://news.ycombinator.com/item?id=41227738
> Context: "Okay, I Like WezTerm" thread.
> Maps to: P2

> "I started to go over the settings of Kitty and found linux_display_server auto... Now just playing with it I changed it to wayland, BIG surprise no changes still lagged. However I changed it to x11 and restarted Kitty.... it's smooth as butter, it's just as smooth as the other terminals. Now the big question is why is it lagging with the wayland display server?"
> — u/NaraDesho, r/kde, October 2024 (score 1)
> https://old.reddit.com/r/kde/comments/1gfz4i6/kittyfish_resize_lag_vs_alacrittykonsole_why_is/
> Context: resize lag under Wayland, gone under X11.
> Maps to: P2, P9

> "I have a traumatic experience with GPU-based GUI. ... the system fell back to something called 'llvmpipe'. The result was that the widgets reacted approximately with 30 second lag to every action. So what I am asking, if you are making a GPU-based rendering toolkit, please write also SIMD software fallback without shaders."
> — codedokode, Hacker News, December 2024
> https://news.ycombinator.com/item?id=42525001
> Context: Ghostty 1.0 launch thread.
> Maps to: P2

> "The GPU requirements of a terminal are minuscule even under heavy load. We're not building AAA games here, we're building a thing that draws a text grid. There is no integrated GPU on the planet that wouldn't be able to keep a terminal going at an associated monitor's refresh rate."
> — mitchellh, Hacker News, December 2024
> https://news.ycombinator.com/item?id=42524717
> Context: Ghostty's author answering the GPU objection; the argument the CRT corner cannot make, since its effects are not a text grid.
> Maps to: P2

> "GPU accelerated graphics for performance reasons but is written in Python? Wat. That sounds very misguided."
> — u/HolyGarbage, r/linux, January 2025 (score 43)
> https://old.reddit.com/r/linux/comments/1ibs1nq/gpu_based_terminal_and_is_there_really_an/
> Context: "GPU based terminal and is there really an advantage" thread.
> Maps to: P2

> "This seems related to Qt6/QtQuick rendering + Mesa (Gallium) on an AMD Renoir iGPU... crashes immediately with SIGABRT."
> — @Primvin, Swordfish90/cool-retro-term issue #934, February 2026
> https://github.com/Swordfish90/cool-retro-term/issues/934
> Context: crash on launch under Hyprland; open, reproduced on other GPUs by June 2026.
> Maps to: P2, P9

> "The app crashes if I resize the window in any way. It also kills other instances of the app... I can make it fullscreen though, but there is no going back - it dies if I try to unfullscreen it"
> — @3x7r4-d1p, Swordfish90/cool-retro-term issue #925, February 2026
> https://github.com/Swordfish90/cool-retro-term/issues/925
> Context: Hyprland, niri, River; open.
> Maps to: P2, P9

> "I noticed GPU usage is quite high using this terminal, even after turning OFF all continually animated effects... It reduces GPU usage from ~17-20% to ~7-11% on my ThinkPad P14s G6 when the terminal is fully idle."
> — @DavDood, Swordfish90/cool-retro-term PR #946, May 2026
> https://github.com/Swordfish90/cool-retro-term/pull/946
> Context: idle GPU draw; PR open, maintainer asked for a simpler approach.
> Maps to: P2, P3

---

## 3. The retro CRT look, wanted and resented → P3
> "when I run one command seems so fast! Maybe would be good add an option to simulate the old hardware.. a 'line lag' time"
> — AlanJAS, Swordfish90/cool-retro-term issue #153, October 2014
> https://github.com/Swordfish90/cool-retro-term/issues/153
> Context: a request for simulated slowness; the look is not retro enough for some.
> Maps to: P3

> "I would like to be able to go even smaller. On a 1600x900 laptop, even 50% is a very large window to get a day-to-day number of columns in."
> — @tomchiverton, Swordfish90/cool-retro-term issue #117, January 2015
> https://github.com/Swordfish90/cool-retro-term/issues/117
> Context: maintainer: "scanline and pixel mode look terrible when the number of 'virtual pixels' is too close to the number of 'real pixels'". Open: the effect fights text density.
> Maps to: P3, P4

> "Lipstick on a pig."
> — u/cybereality, r/commandline, January 2020 (score 34)
> https://old.reddit.com/r/commandline/comments/emjo8p/microsofts_windows_terminal_is_getting_retrostyle/
> Context: Windows Terminal adding retro CRT effects.
> Maps to: P3

> "I'm sure this will be great for the tourists, but for people who use a terminal daily, I can't see how this wouldn't negatively impact usability and readability."
> — u/jhchrist, r/commandline, January 2020 (score 20)
> https://old.reddit.com/r/commandline/comments/emjo8p/microsofts_windows_terminal_is_getting_retrostyle/
> Context: same thread.
> Maps to: P3

> "The CLI version of an Instagram filter."
> — u/sobercelibacy, r/commandline, January 2020 (score 8)
> https://old.reddit.com/r/commandline/comments/emjo8p/microsofts_windows_terminal_is_getting_retrostyle/
> Context: same thread.
> Maps to: P3

> "I have found that Kitty comes VERY close to my ideal/dream terminal emulator for Wayland... However, I am missing an small/big detail: CRT effects. Since Kitty does everything regarding graphics on the GPU, can it please support GLSL shaders? The RetroArch project has hundreds of them, with curvature effects, scanlines, masks..."
> — @vanfanel, kovidgoyal/kitty issue #4842, March 2022 (6 reactions)
> https://github.com/kovidgoyal/kitty/issues/4842
> Context: closed by the maintainer: "Not something I am interested in, sorry." Declined.
> Maps to: P3, P12

> "For Windows folks, Windows Terminal has a similar effect available. Definitely nostalgic, definitely fun for a minute, definitely not how I would want to work in the terminal."
> — timoteostewart, Hacker News, March 2022
> https://news.ycombinator.com/item?id=30738123
> Context: cool-retro-term thread.
> Maps to: P3

> "I toyed with both, Cathode on the Mac as well as Cool-retro-term on Linux, for video production. The effect and quality is exactly what I want. My only pain is that they both don't fit well into my workflow."
> — weinzierl, Hacker News, March 2022
> https://news.ycombinator.com/item?id=30745427
> Context: same thread: the look wanted, the tool not fitting the work.
> Maps to: P3

> "I get the nostalgia appeal, it would be neat for a few minutes, but it doesn't seem like something you could use as your daily driver terminal. If someone has, what's your experience with it?"
> — mysterydip, Hacker News, July 2023
> https://news.ycombinator.com/item?id=36799287
> Context: cool-retro-term thread.
> Maps to: P3

> "That's it. I know that's supposed to be Cool Retro Term's job, but it is TOO damn slow for me to run in my iGPU i3 of 7th gen laptop, and it pulls 121 QT dependencies just to install the term. I love Wezterm and I just wish we had custom/built-in shaders support"
> — @xplshn, wezterm/wezterm issue #5182 "[Feature Request] Cool Retro Term Shaders?", March 2024 (21 reactions)
> https://github.com/wezterm/wezterm/issues/5182
> Context: open; a WGSL shader PR with "barrel distortion, chromatic aberration, vignette, and phosphor glow" arrived two years later.
> Maps to: P3, P2, P9

> "Some terminals have fixed shader effects like cool-retro-term, others like ghostty allow for shader effect programming on the user end. I was wondering if it would be possible to add an ability to affect the look of the terminal with custom programmable shader effects? So that a user could do things like retro scan lines, shaking text, animated backgrounds..."
> — @LordUbuntu, wezterm/wezterm issue #6985, May 2025 (23 reactions)
> https://github.com/wezterm/wezterm/issues/6985
> Context: open; a commenter: "+1 to this I used cool retro term, but it doesn't have much features and kinda broken when using something like btop++."
> Maps to: P3

> "It's doing something with the colors which makes stuff hard to read. Are there settings or options to make it a little less meddlesome?"
> — u/Optimal-Savings-4505, r/commandline, December 2025 (score 2)
> https://old.reddit.com/r/commandline/comments/1p7gimc/a_modern_rust_retrostyled_terminal_multiplexer/
> Context: feedback on term39, a retro-styled multiplexer with scanlines and glow.
> Maps to: P3

> "I'm compelled to ask why terminal emulators have shaders now."
> — u/Stunning_Macaron6133, r/unixporn, February 2026 (score 1)
> https://old.reddit.com/r/unixporn/comments/1r32pfq/oc_crtty_retro_crt_shader_for_kitty_also_a/
> Context: CRTty, a CRT shader injector for kitty; the thread's other reply called it bloat.
> Maps to: P3

> "The CRT look nearly broke me. I wanted that heavy analog vibe, but the second I pushed screen curvature + chromatic aberration + scanlines, reading anything on the in-game terminals turned into actual pain... how do you keep heavy filters like VHS/CRT from frying people's eyes after 20 min?"
> — u/Flat_File_907, r/Unity3D, July 2026 (score 6)
> https://old.reddit.com/r/Unity3D/comments/1v0vry8/getting_this_retro_terminal_ui_and_crt_aesthetic/
> Context: a game developer building a retro-terminal UI; the eye-strain ceiling stated by someone who wants the look.
> Maps to: P3

---

## 4. Font rendering: crispness, scale, and ligatures → P4
> "It's not that featureful. It can't even handle bitmap fonts, only slow and blurry truetype fonts."
> — snvzz, Hacker News, September 2018
> https://news.ycombinator.com/item?id=17926772
> Maps to: P4

> "Thanks, I have tried Nerd fonts, but they are only TTF and therefore look slightly blurry (anti-aliased) in terminal. It looks okay, but not nearly as nice as a crisp looking bitmap font :)"
> — u/peder2tm, r/archlinux, February 2020 (score 2)
> https://old.reddit.com/r/archlinux/comments/f5ciqa/terminus_bitmap_font_with_powerline_symbols/
> Context: wanting Terminus with powerline glyphs.
> Maps to: P4

> "You could use bitmap fonts to get truly sharp text. I'm a big fan of terminus at 9pt. It makes non-bitmap fonts look blurry and awful by comparison. A real shame that Pango broke bitmap support and your choice of terminal emulator and other stuff is limited now."
> — opan, Hacker News, December 2020
> https://news.ycombinator.com/item?id=25295895
> Maps to: P4

> "The onny difference I see is that the font rendering is slightly bolder and more blurry in Kitty, and I really hope I'll solve this, 'cause I prefer the crispness I get in Alacritty."
> — u/Hexalyse, r/archlinux, June 2021 (score 6)
> https://old.reddit.com/r/archlinux/comments/n9noje/alacritty_vs_kitty/
> Context: identical font config across both terminals.
> Maps to: P4

> "foot is really nice. It supports bitmap fonts and wayland, both are musts for me. It also has a cool url hints mode like termite did. I always missed that while using alacritty (after termite, or rather pango, dropped bitmap support and I had to switch)."
> — opan, Hacker News, December 2021
> https://news.ycombinator.com/item?id=29562230
> Context: foot launch thread; bitmap support as a switching reason.
> Maps to: P4, P10

> "CRT already supports these if you add them to the QML and resources. Bitmap fonts have the benefit of being pixel-perfect by default, so they work with the low-resolution rasterization methods without requiring hinting/alignment tweaks. It would be nice if there was a way to use them like this without having to recompile the program."
> — @ali1234, Swordfish90/cool-retro-term issue #740, July 2022
> https://github.com/Swordfish90/cool-retro-term/issues/740
> Context: open, no maintainer response; bitmap fonts in a CRT terminal only by recompiling.
> Maps to: P4, P3

> "If there's one thing I don't want, it's ligatures. I don't need a terminal that breaks the grid pattern by gluing random items together. Besides, ⇐, ≤ and <= are different things, thank you very much."
> — u/LvS, r/linux, February 2024 (score 20)
> https://old.reddit.com/r/linux/comments/1aud0lb/whats_the_best_terminal_emulator_and_why_is/
> Maps to: P4

> "I use 125% scaling on my laptop display, and Ghosty exhibits blurry font rendering. At 100% or 200% scaling, the rendering is crisp, and at 150% or 175% it looks blurry but not as pronounced as at 125%."
> — @190n, ghostty-org/ghostty issue #1938, July 2024 (12 reactions)
> https://github.com/ghostty-org/ghostty/issues/1938
> Context: closed after 54 comments of fractional-scaling debugging.
> Maps to: P4

> "However, using shaders like Newpixie-CRT and running certain programs with light blue backgrounds (like Syncterm, for example), cause 'patterns' on the solid color areas: it means, scanlines with different width... This is a very well known issue with shaders + non-integer scaling, so my question is: Is there a way for RIO to do integer scaling..."
> — @vanfanel, raphamorim/rio issue #738, October 2024
> https://github.com/raphamorim/rio/issues/738
> Context: open; CRT shader artefacts at non-integer scale.
> Maps to: P4, P3

> "The dev thinks all other terminals are wrong and is entirely unwilling to offer a configuration option for the behavior. The terminal ignores colors 8-15 in favor of using the bold variant of a given font... The dev is sort of famously unreceptive and prickly."
> — u/kin_of_the_caves, r/commandline, February 2025 (score 22)
> https://old.reddit.com/r/commandline/comments/1iysy73/why_is_kittys_font_rendering_so_weird_compared_to/
> Context: side-by-side screenshots of kitty against Alacritty, Konsole, Ghostty and foot.
> Maps to: P4, P12

> "i'm one of those simple girls that uses cozette in her terminal. only problem is i can only use the vector version which has artifactiong, unlike the bitmap version. could you perhaps add support for it in the future?"
> — @tungstengmd, raphamorim/rio issue #971, February 2025 (2 reactions)
> https://github.com/raphamorim/rio/issues/971
> Context: open; a later commenter: "I would like terminus bitmap font to work. Much better than any ttf font for your eyes."
> Maps to: P4

> "Non-bitmap fonts really are blurry, comparatively. AFAICT the non-bitmap fonts just hide this thanks to people using HiDPI displays now, which is basically like saying 'well, it looks fine from far away'."
> — opan, Hacker News, September 2025
> https://news.ycombinator.com/item?id=45090404
> Maps to: P4

> "Blurry fonts was my main issue with WPF. I get headaches from blurry text and the colour bleeding from ClearType just makes the headache worse... My trick has been to use bitmap fonts with no AA, but that broke in recent versions of electron, where bitmap fonts are now rendered blurry."
> — VorpalWay, Hacker News, April 2026
> https://news.ycombinator.com/item?id=47659062
> Maps to: P4, P11

---

## 5. Many sessions at once → P5
> "I suffer from the same problem of too many terminal windows and browser tabs, and honestly it doesn't seem to help me much. It just dilutes my attention."
> — meowface, Hacker News, ~2014
> https://news.ycombinator.com/item?id=7998521
> Maps to: P5

> "I looked at alacritty but I really like using terminal tabs and the alacritty dev is really really against them and I found the dev's attitude to be more than a bit abrasive."
> — skrtskrt, Hacker News, November 2024
> https://news.ycombinator.com/item?id=42248813
> Maps to: P5, P12

> "too many terminal tabs, constantly forgetting which session was thinking vs waiting for input"
> — u/asheshgoplani, r/ClaudeCode, December 2025 (score 318)
> https://old.reddit.com/r/ClaudeCode/comments/1pxyn37/i_got_tired_of_managing_15_terminal_tabs_for_my/
> Context: the post behind Agent Deck, a tmux dashboard for agent sessions; the many-sessions pain in its 2025 form, one shell per AI agent.
> Maps to: P5

> "I've bounced off of tmux so many times because I forget to check in on other sessions, and have never loved the interaction model."
> — u/attabui, r/ClaudeCode, December 2025 (score 13)
> https://old.reddit.com/r/ClaudeCode/comments/1pxyn37/i_got_tired_of_managing_15_terminal_tabs_for_my/
> Maps to: P5, P7

> "with only 3–4 Claude tabs in my terminal I already feel overwhelmed"
> — u/Ana8567, r/ClaudeCode, December 2025 (score 2)
> https://old.reddit.com/r/ClaudeCode/comments/1pxyn37/i_got_tired_of_managing_15_terminal_tabs_for_my/
> Maps to: P5

> "having a keyboard shortcut to switch between sessions without going via the menu would be awseome"
> — u/bzBetty, r/ClaudeCode, December 2025 (score 1)
> https://old.reddit.com/r/ClaudeCode/comments/1pxyn37/i_got_tired_of_managing_15_terminal_tabs_for_my/
> Maps to: P5

> "I kept getting lost whenever I worked with multiple coding agents. I'd start a few sessions in tmux, open another to test something, spin up one more for a different repo… and after a while I had no idea: which session was still running, which one was waiting for input, where that 'good' conversation actually lived"
> — u/Frayo44, r/ClaudeAI, February 2026 (score 105)
> https://old.reddit.com/r/ClaudeAI/comments/1rb4jvs/i_got_tired_of_managing_10_terminal_tabs_for_my/
> Context: the post behind agent-view, another tmux overlay.
> Maps to: P5

> "I use Profiles in iTerm2. Double click on one and I'm cd'd in the correct folder. I rename the tabs and apply colors to them. I create multiple windows when too many tabs are spawned."
> — u/Conscious-Drawer-364, r/ClaudeAI, February 2026 (score 2)
> https://old.reddit.com/r/ClaudeAI/comments/1rb4jvs/i_got_tired_of_managing_10_terminal_tabs_for_my/
> Context: the coping mechanism: rename and colour by hand.
> Maps to: P5

> "What do you people do that requires 10+ agent sessions? I usually have 1 session per project, so 3 parallel sessions at most."
> — u/NekoLu, r/ClaudeAI, February 2026 (score 21)
> https://old.reddit.com/r/ClaudeAI/comments/1rb4jvs/i_got_tired_of_managing_10_terminal_tabs_for_my/
> Context: the counter-segment inside the thread.
> Maps to: P5, P13

> "I was running multiple agents across multiple tmux sessions and have no idea which one needed my attention"
> — u/Palanikannan_M, r/tmux, March 2026 (score 317)
> https://old.reddit.com/r/tmux/comments/1s6oze9/built_an_agent_orchestrator_within_tmux/
> Context: the post behind a tmux sidebar for agent sessions.
> Maps to: P5

> "Do you guys actually work this way? I feel like my brain would be so fried at the end of the day trying to maintain this level of orchestration and management."
> — u/Orlandocollins, r/tmux, March 2026 (score 10)
> https://old.reddit.com/r/tmux/comments/1s6oze9/built_an_agent_orchestrator_within_tmux/
> Maps to: P5, P13

> "With many tabs open, tab labels become unreadable, and there is no configuration that recovers them — the bar always shrinks tabs to fit the window, without limit... Every increase in legibility costs characters, because the width per tab..."
> — @mmcc007, wezterm/wezterm issue #8052, August 2026
> https://github.com/wezterm/wezterm/issues/8052
> Context: open; 24 tabs at 1360 px leave two or three legible characters per label.
> Maps to: P5

No verbatim complaint found for tab-bar or status-bar visual clutter as such; the closest is the iTerm2 rename-and-colour workaround above and the WezTerm label issue.

---

## 6. tmux control mode outside iTerm2 → P6
> "Now that multi-window support is on the roadmap, how about support for control mode in tmux? (tmux -CC) Essentially this allows the user to attach to a tmux session but instead of showing the ncurses UI in the same terminal window the terminal emulator opens actual windows for each window in the tmux session."
> — @jansol, alacritty/alacritty issue #2410, May 2019 (42 reactions)
> https://github.com/alacritty/alacritty/issues/2410
> Context: maintainer: "there is absolutely no desire to implement features like tabs/splits/panes in Alacritty"; in 2022, "I don't see this issue playing well with Alacritty's goals". Declined in effect.
> Maps to: P6, P12

> "This tmux integration might seem like a small thing but once you have it it's hard to give up. basic crap like scrolling and copying and pasting from a tmux pane is straight up nastiness. Iterm2 fixes all those pain points amazingly"
> — u/locusofself, r/devops, January 2020 (score 1)
> https://old.reddit.com/r/devops/comments/eh13br/do_any_windows_terminals_support_tmux_cc_like/
> Context: looking for a Windows terminal with iTerm2's `-CC` support.
> Maps to: P6

> "Well macOS makes things difficult. But lets see what we can do."
> — @kovidgoyal, kovidgoyal/kitty issue #2422 "kitty as a tmux replacement", March 2020 (25 reactions)
> https://github.com/kovidgoyal/kitty/issues/2422
> Context: the reporter called tmux "insanely slow on macOS"; closed via workarounds, not control mode.
> Maps to: P6, P7

> "I prefer to use actual tmux since that's portable. Otherwise you're basically just using an alternate pane split solution which is limited to a single terminal application."
> — u/cicatrix1, r/tmux, July 2020 (score 3)
> https://old.reddit.com/r/tmux/comments/hwllvp/iterm2_integration/
> Context: the counter-segment on control mode: portability over integration.
> Maps to: P6, P13

> "For many years, I've been using iTerm2's tmux integration as a way to transparently maintain multiple per-project sets of terminal windows/tabs, both locally and remotely... Sadly, I haven't seen any other terminal emulator that uses it, which is a total shame... I had wanted to try out warp, but the lack of tmux -CC integration threw off my workflow so much, I couldn't even give it a fair try."
> — u/FullyHalfBaked, r/commandline, February 2024 (score 5)
> https://old.reddit.com/r/commandline/comments/1ar7c9z/anybody_know_of_any_linux_or_windows_terminal/
> Context: "Anybody know of any Linux or Windows terminal..." with control mode; a reply: "I still look for this from time to time. Terminator has git issues open about it, but I never find anything else."
> Maps to: P6

> "tmux has a feature called Control Mode, which allows a client i.e a terminal emulator such as Ghostty to manage tmux panes. This has the benefit of enabling native integration with the terminal emulator..."
> — @yankcrime, ghostty-org/ghostty issue #1935, July 2024 (731 reactions)
> https://github.com/ghostty-org/ghostty/issues/1935
> Context: open; the most-reacted issue in this sweep. A commenter: "consider this a gigantic +1 (+1M). I'm so dependent on that feature that I'll probably continue to use iTerm2 until Ghostty or another terminal emulator supports it."
> Maps to: P6

> "e.g. IIRC his answer to 'How do I set up tmux with kitty?' was something like 'Don't, tmux is dumb' and closing it. Eventually I gave up. ... Heh, I switched from Kitty to Wezterm due to the exact same types of comments from the maintainer."
> — bramhaag, Hacker News, August 2024
> https://news.ycombinator.com/item?id=41228168
> Maps to: P6, P12

> "Thanks for this long awaited feature. It is really helpful. One quick feedback... this latency is not coming when using tmux -CC inside wsl2, so it is only happening with ssh connection to remote VMs with tmux."
> — @souradeep100, wezterm/wezterm issue #6806, March 2025
> https://github.com/wezterm/wezterm/issues/6806
> Context: WezTerm's nightly control-mode support; open, still reported in January 2026: "significantly impacting my experience".
> Maps to: P6

> "It's a terrible protocol. Absolutely abysmal design which leads to a plethora of edge case bugs. At some point I'll replace tmux control mode entirely but for the moment it solves the immediate problem."
> — hnlmorg, Hacker News, April 2026
> https://news.ycombinator.com/item?id=47755404
> Context: the author of ttyphoon on implementing control mode.
> Maps to: P6

> "I'm trying to replicate how good tmux is inside iTerm, but it's tough."
> — csheaff, Hacker News, June 2026
> https://news.ycombinator.com/item?id=48532539
> Context: announcing a control-mode client for Emacs.
> Maps to: P6

---

## 7. Sessions surviving the terminal → P7
> "To me, forcing me to use tmux to get scrollback is precisely violating the idea of 'one tool doing one thing'. You're forcing me to compose a Swiss army knife (tmux) into situations where all I need is a knife (a terminal that works well with the idioms of my environment)."
> — stormbrew, Hacker News, January 2017
> https://news.ycombinator.com/item?id=13342428
> Context: Alacritty Show HN.
> Maps to: P7, P10

> "I got disconnected from my VPS when I was doing a 'do-release-upgrade'"
> — u/auron_py, r/selfhosted, March 2024 (score 872)
> https://old.reddit.com/r/selfhosted/comments/1bbw6ta/psa_use_tmux/
> Context: the post behind "PSA: Use TMUX".
> Maps to: P7

> "I like the idea of using tmux because of all its benefits. But what bugs me the most about it is the scrolling, I find it so hard to get it working."
> — u/llamedo, r/selfhosted, March 2024 (score 6)
> https://old.reddit.com/r/selfhosted/comments/1bbw6ta/psa_use_tmux/
> Maps to: P7, P10

> "Kovid really bugs me and is a reason I turned away from kitty too. ... Kovid seems to think tmux is really about splitting panes and peoples' main draw to it isn't about persistence... Oh, and kitty phones home"
> — godelski, Hacker News, August 2024
> https://news.ycombinator.com/item?id=41233328
> Context: "Okay, I Like WezTerm" thread; persistence named as tmux's real draw.
> Maps to: P7, P12, P11

> "Resurrect was really exciting for me when I started using it, but in practice every session I've resurrected I'm unable to really use the shell afterwards. It will always crash, and I'm unable to add tabs or panes."
> — u/wyldstallionesquire, r/zellij, September 2024 (score 3)
> https://old.reddit.com/r/zellij/comments/1fo675i/im_starting_to_wonder_of_i_should_switch_to_tmux/
> Maps to: P7

> "I loathe iTerm2 because it has no way to customize key bindings, which conflict with how I want to use my shell. Alacritty is enough for me. I need no frills since I run everything in tmux."
> — u/tblancher, r/commandline, October 2025 (score 3)
> https://old.reddit.com/r/commandline/comments/1o6tkgz/recommend_me_terminal_emulators/
> Context: the counter-segment: tmux underneath, any terminal on top.
> Maps to: P7, P13

> "I use tmux -Cc with iterm2 then I have a Claude code script that just reattach them all when I get disconnected, saves tab layout panels everything"
> — u/Ultramen, r/tmux, April 2026 (score 2)
> https://old.reddit.com/r/tmux/comments/1sujraw/managing_tmux_across_multiple_ssh_servers/
> Context: survival solved by control mode plus a reattach script, on macOS.
> Maps to: P7, P6

---

## 8. Configuration → P8
> "headsup, pretty sure one still has to recompile foot to change their font or font size. yeah it's that minimalistic."
> — rektide, Hacker News, December 2021
> https://news.ycombinator.com/item?id=29559078
> Maps to: P8

> "However, the default configuration is annoying."
> — u/RealNC, r/kde, May 2022 (score 9)
> https://old.reddit.com/r/kde/comments/utk096/what_is_the_advantage_to_a_drop_down_terminal_vs/
> Context: praising Yakuake's drop-down toggle and posting a full custom config to make it bearable.
> Maps to: P8

> "I don't think I've ever encountered a problem where terminal emulation would be too slow. What I do have encountered are terminal emulators that go bonkers and require some arcane configuration magic to figure out what to emulate. GNOME terminal works and looks pleasant to boot."
> — jampekka, Hacker News, September 2023
> https://news.ycombinator.com/item?id=37625284
> Context: foot thread; configuration named as the real cost.
> Maps to: P8, P13

> "...the first thing I noticed was how snappy everything feels, especially when resizing the window. The straight-forward configuration was extremely nice as well and can be stored in my dotfiles now (iTerm was a giant dump of XML). A few things that keep me from switching to it full time: Missing search scrollback (cmd+f)..."
> — denolfe, Hacker News, December 2024
> https://news.ycombinator.com/item?id=42534796
> Context: Ghostty 1.0 thread.
> Maps to: P8, P10

> "I tried it for an hour - lots of glitches and not easily discoverable config options... I'm too busy firing up Konsole and sometimes Kitty."
> — u/ben2talk, r/commandline, January 2025 (score 1)
> https://old.reddit.com/r/commandline/comments/1htimkk/kitty_vs_ghostty_terminal_emulators/
> Maps to: P8

> "It would take an hour or so fiddling with configuration files and command line settings to whip it into what I'd consider shape, as inured as I am to `xfce4-terminal` with a side order of `xterm`. … I suspect that the lack of a graphical configuration option will be a hindrance to adoption for a lot of folks and, for the nonce, I'm not sure it brings enough to the party for me to spend the time fussing with settings, especially considering the lack of a title bar, the presence of which I personally find useful."
> — Philo T Farnsworth, The Register forums, January 2025
> https://forums.theregister.com/forum/all/2025/01/08/ghostty_1/
> Context: Ghostty 1.0 as The Register's readers met it.
> Maps to: P8

> "It has a pronounced console flash when starting in Windows that is infuriating - mintty avoids this completely... It has a text config which could be a positive but to have a change take place you have to restart"
> — dkh, Lobsters, September 2025
> https://lobste.rs/s/7a4lle/rio_terminal_hardware_accelerated_gpu
> Maps to: P8

> "I hadn't changed terminals on my work devices (Macs) from iterm until they decided they needed to include 'AI' in the terminal, at which point I switched to wezterm, primarily because then I could also switch my personal devices (fedora) as well. Now I have a unified terminal config across all my devices despite the different OS"
> — sjsadowski, Lobsters, September 2025
> https://lobste.rs/s/7a4lle/rio_terminal_hardware_accelerated_gpu#c_sjsadowski
> Maps to: P8, P11

---

## 9. Platform reach and desktop fit on Linux → P9
> "It would be great to have built packages for debian environment, namely Ubuntu. There are lot of build requirements that would make it harder to build yourself."
> — @timfallmk, Swordfish90/cool-retro-term issue #136, October 2014
> https://github.com/Swordfish90/cool-retro-term/issues/136
> Context: still open twelve years on.
> Maps to: P9

> "Because of scaling problems on HighDPI displays the following problems occur: Selection doesn't match cursor location; Must use minimum Screen scaling settings; No auto scroll... If one sets QT_SCALE_FACTOR to 0.5 everything works, but the settings menu is using the scaling setting as well and appears extremely tiny."
> — @itay-grudev, Swordfish90/cool-retro-term issue #347, May 2017
> https://github.com/Swordfish90/cool-retro-term/issues/347
> Context: open nine years; traced to a Qt bug.
> Maps to: P9, P4

> "This application failed to start because it could not find or load the Qt platform plugin 'wayland-egl'... Aborted (core dumped)"
> — @StuPagely, Swordfish90/cool-retro-term issue #539, July 2019
> https://github.com/Swordfish90/cool-retro-term/issues/539
> Context: the snap on GNOME Wayland; open.
> Maps to: P9

> "I'm still missing the quake mode that's unavailable under wayland."
> — okramcivokram, Hacker News, November 2021
> https://news.ycombinator.com/item?id=29094984
> Maps to: P9

> "I'm using Linux with Xfce, but it seems to be locked into a Gnome-like look and feel, with header bars and CSDs that can't be disabled in favor of standard title bars and menus, so it's actually very inconsistent with the rest of my desktop environment."
> — Gormo, Hacker News, December 2024
> https://news.ycombinator.com/item?id=42523078
> Context: Ghostty 1.0 launch thread.
> Maps to: P9

> "Looks pretty cool! Unfortunately I can't use it yet, as I am on a Ubuntu-based distro (Pop! OS 22.04), so my GTK version is not high enough. I imaging that's the case for a lot of people who stick to LTS versions."
> — graynk, Hacker News, December 2024
> https://news.ycombinator.com/item?id=42530796
> Maps to: P9

> "It using gtk4 and libadwaita is [bad]. That makes it a gnome application, not a generic Linux application. It makes it look like crap and out of place on anything except gnome."
> — u/throttlemeister, r/linux, January 2025 (score 6)
> https://old.reddit.com/r/linux/comments/1hskl9f/is_ghostty_using_gtk_for_linux_a_drawback/
> Maps to: P9

> "Ghostty looks nice but seems to require ridiculous volume of dependencies, since it needs a special compiler, and that one apparently pulls in llvm and clang (!)."
> — u/arjuna93, r/commandline, May 2025 (score 3)
> https://old.reddit.com/r/commandline/comments/1htimkk/kitty_vs_ghostty_terminal_emulators/
> Maps to: P9

> "doesn't work on X11 so you still need some GPU compositing somewhere in your pipeline"
> — jcelerier, Lobsters, November 2025
> https://lobste.rs/s/flln5g/state_terminal_emulators_2025_errant
> Context: on foot being Wayland-only.
> Maps to: P9

---

## 10. Table-stakes missing → P10
> "If this gets a few extra little perks like tabs, scrollback and in-app configuration, I could see myself switching to this for sure."
> — u/dada_, r/rust, January 2017 (score 12)
> https://old.reddit.com/r/rust/comments/5mf2yh/announcing_alacritty_a_gpuaccelerated_terminal/
> Context: the Alacritty announcement; what is missing before it is a daily driver.
> Maps to: P10

> "Unfortunately there is no support for ligatures. A good programming font like fira or source code pro makes for such a quality of life improvement. At least easier on the eyes."
> — WD-42, Hacker News, September 2023
> https://news.ycombinator.com/item?id=37625635
> Maps to: P10, P4

> "Really need scrollback search though. Was a bit surprised it was launched without that."
> — ilrwbwrkhv, Hacker News, December 2024
> https://news.ycombinator.com/item?id=42524792
> Context: Ghostty 1.0 launch thread.
> Maps to: P10

> "Very impressed with ghostty so far, but: Main reasons for sticking with ITerm2 (for the moment at least): 'Seamless' cut & paste with paste on '2 X right click' + 'Trim trailing LF' / Quadruple click 'Smart selection' / Brillant search with highlighting in text and scrollbar / Support for OSC-1 'icon titles' in tabs, as opposed to OSC-0 'header title'"
> — themadsens, Hacker News, December 2024
> https://news.ycombinator.com/item?id=42526090
> Maps to: P10

> "It turns out those 6 features I was using were more like 8 features, and kitty doesn't have them... pressing command+f to 'find' text in your scroll back buffer... Kitty does not have this functionality, at all... I'm starting to reconsider my choices."
> — u/Cool-Engineer4408, r/commandline, August 2025 (score 2)
> https://old.reddit.com/r/commandline/comments/1htimkk/kitty_vs_ghostty_terminal_emulators/
> Maps to: P10

> "Ghostty has no scroll bar. This isn't always important for me, but when it is, it's a dealbreaker."
> — u/garblesnarky, r/commandline, October 2025 (score 2)
> https://old.reddit.com/r/commandline/comments/1o6tkgz/recommend_me_terminal_emulators/
> Maps to: P10

---

## 11. Weight, bloat, and creep → P11
> "So is my current terminal app, which clocks in at 250 kilobyte. And its memory and CPU footprint is so low, I can't find it in htop without filtering."
> — usrbinbash, Hacker News, December 2021
> https://news.ycombinator.com/item?id=29556773
> Context: Tabby (Electron) launch thread.
> Maps to: P11

> "Looks like they recently turned on quota for free users as I started hitting a paywall. I've immediately uninstalled it as there is no way to disable it."
> — turtlebits, Hacker News, May 2024
> https://news.ycombinator.com/item?id=40458635
> Context: Warp's AI feature.
> Maps to: P11

> "Deal breaker: I do not like normalizing the deviancy of sending shell input to a third party. F opt-out telemetry and a 'trust us' privacy policy."
> — magic_smoke_ee, Hacker News, November 2024
> https://news.ycombinator.com/item?id=42256127
> Maps to: P11

> "Those sounds like terminal that your typical tech bro would use. For me i skip those, I don't see why I would salvage half of my ram to run couple ssh sessions and see some logs."
> — u/Mezutelni, r/linux, April 2025 (score 230)
> https://old.reddit.com/r/linux/comments/1k5t3om/does_anyone_use_electron_based_terminal_emulators/
> Context: top comment on "does anyone use Electron-based terminal emulators?"
> Maps to: P11, P13

> "iterm2 is slow, uses a ton of memory, and keeps adding weird bloat/AI garbo"
> — trial3, Hacker News, September 2025
> https://news.ycombinator.com/item?id=45352709
> Maps to: P11, P1

> "I use foot because I got tired of KDE having too many features in the terminal."
> — WilhelmVonWeiner, Lobsters, September 2025
> https://lobste.rs/s/7a4lle/rio_terminal_hardware_accelerated_gpu#c_WilhelmVonWeiner
> Maps to: P11

> "Pricing model for a terminal. What a time to be alive."
> — gray_-_wolf, Hacker News, October 2025
> https://news.ycombinator.com/item?id=45772660
> Context: Warp's pricing change.
> Maps to: P11

> "I highly dislike forcefed AI, and Warp is AI vibecode shit. I'll stick to kitty, thank you very much"
> — u/lefunat0r, r/commandline, October 2025 (score 0)
> https://old.reddit.com/r/commandline/comments/1o6tkgz/recommend_me_terminal_emulators/
> Maps to: P11

---

## 12. Maintainer conduct → P12
> "Usually I agree but the maintainer of kitty is bonkers rude. There have been several issues concerning privacy and security regarding kitty. The responses of the maintainer regarding these very valid concerns made me switch to wezterm/sakura/foot in an instant."
> — razemio, Hacker News, April 2023
> https://news.ycombinator.com/item?id=35588591
> Maps to: P12

> "Meanwhile Kovid or the alacritty devs are just better-than-thou toxic assholes (maybe not all alacritty devs are, just the ones I've experienced)"
> — u/pvnrt1234, r/linux, December 2024 (score 6)
> https://old.reddit.com/r/linux/comments/1hn700x/what_are_the_meaningful_differences_between/
> Maps to: P12

The kitty and Alacritty entries under §3, §4 and §6 carry the same pain; it is the most cross-cutting one in the corpus.

---

## 13. Choosing at all → P13
> "I just use tmux, I don't want my terminal emulator to do anything it doesn't have to for that end."
> — u/IGTHSYCGTH, r/tmux, July 2020 (score 2)
> https://old.reddit.com/r/tmux/comments/hwllvp/iterm2_integration/
> Maps to: P13, P6

> "As someone who still just uses whatever terminal emulator my desktop environment provides, what are the advantages of choosing another terminal emulator application? ... I've never experienced latency or any problems otherwise related to rendering, so I wonder why some terminals nowadays pride themselves in using GPU rendering."
> — doodlesdev, Hacker News, May 2023
> https://news.ycombinator.com/item?id=36060222
> Context: Rio launch thread.
> Maps to: P13, P2

> "I've used urxvt almost every day for years (I remember using it on a laptop with 32 MB of memory), and have never once had a problem with typing latency. I didn't even realize it was a thing people bothered to measure until I saw this article."
> — jlarocco, Hacker News, May 2023
> https://news.ycombinator.com/item?id=35811173
> Maps to: P13, P1

> "Can I do terminal things in it? Then I don't really care."
> — u/Mysterious_Bit6882, r/linux, February 2024 (score 207)
> https://old.reddit.com/r/linux/comments/1aud0lb/whats_the_best_terminal_emulator_and_why_is/
> Context: top comment on "What's the best terminal emulator, and why isn't gnome-terminal sufficient?"
> Maps to: P13

> "Is it weird that I never considered foot because of the name? At least when you say 'alacritty' i understand that it's a terminal. When I tell someone I use 'foot'... I don't know, it doesn't give a good feeling."
> — ramon156, Hacker News, December 2024
> https://news.ycombinator.com/item?id=42523626
> Maps to: P13

> "I used Linux for 25 years and all I care is that I can type commands and get results back. Whatever is the default terminal is fine to me."
> — u/TomDuhamel, r/linux, December 2024 (score 121)
> https://old.reddit.com/r/linux/comments/1hn700x/what_are_the_meaningful_differences_between/
> Maps to: P13

> "I'm probably the weird one, but i just use whatever terminal ships with Debian. I don't even know what it's called. It starts instantly and I've never had performance issues."
> — u/crazedizzled, r/linux, January 2025 (score 187)
> https://old.reddit.com/r/linux/comments/1ibs1nq/gpu_based_terminal_and_is_there_really_an/
> Maps to: P13

> "I haven't seen anything from ghostty that I've ever wanted that isn't already available in kitty... The only reason I can imagine to want to switch on ghostty are hype, the joy of writing a new config file, and glsl shaders lol."
> — u/emi89ro, r/commandline, January 2025 (score 55)
> https://old.reddit.com/r/commandline/comments/1htimkk/kitty_vs_ghostty_terminal_emulators/
> Maps to: P13

> "I now have a list of 34 terminal emulators to at least cursorily evaluate." / "I've done the first survey and installed 30 terminal emulators."
> — dsr, Lobsters, September–October 2025
> https://lobste.rs/s/7a4lle/rio_terminal_hardware_accelerated_gpu
> Maps to: P13

> "I haven't changed terminal emulators in a decade. I see all these new options but because I mostly feel like what I use is good enough, I try to sell myself on trying another on a certain feature and get bogged down by in the paradox of choice."
> — swifthand, Lobsters, September 2025
> https://lobste.rs/s/7a4lle/rio_terminal_hardware_accelerated_gpu
> Maps to: P13

> "Every time I look at the problems they're solving with new terminal emulators I never see problems I have. 🤷‍♂️ ... So, clearly I'm not the user they have in mind."
> — trevorflowers, Lobsters, September 2025
> https://lobste.rs/s/7a4lle/rio_terminal_hardware_accelerated_gpu
> Maps to: P13

> "Honest question, as this puzzles me. Has anyone of you ever hit any graphical performance issues with terminal emulators? ... What problem are we trying to solve here? They all start immediately and run with no lag whatsoever even on low end machines."
> — pm, Lobsters, September 2025
> https://lobste.rs/s/7a4lle/rio_terminal_hardware_accelerated_gpu
> Maps to: P13, P1

> "Is tmux really that hard to use? or screen?"
> — u/Inevitable_Mistake32, r/ClaudeAI, February 2026 (score 3)
> https://old.reddit.com/r/ClaudeAI/comments/1rb4jvs/i_got_tired_of_managing_10_terminal_tabs_for_my/
> Maps to: P13, P5

---

## How to add an entry

1. Identify the category. If none fits, leave the entry in the closest sibling and add a TODO at the bottom. Promote to a new section once a second entry arrives.
2. Capture verbatim where possible: double quotes plus an attribution line beginning with an em-dash (the only attribution-line em-dash exception). Carry the venue's engagement figure in the attribution line.
3. If only a paraphrase is available, mark `[paraphrase]` and link the source so the next contributor can recover the verbatim text.
4. Add the date. If approximate, write `~YYYY`; do not invent precision.
5. Map to a pain number from `use-case-survey.md`. If it maps to more than one, list them.
6. Within the section, sort by date oldest-first.
