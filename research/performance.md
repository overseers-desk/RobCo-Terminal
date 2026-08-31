# What the terminal costs to keep on screen

Every number here was measured on this project's own binary. The methods and the traps are recorded alongside, because the traps are what make a naive re-measurement disagree.

Machine: 16 cores, NVIDIA RTX 2070 SUPER, GNOME on Wayland with Xwayland. Percentages are of one core, taken from `utime + stime` in `/proc/<pid>/stat` over a fixed interval. A terminal running `sh -c 'sleep N'` with no output, on the shipped defaults, unless a row says otherwise.

## What a terminal costs, by who is attending it

The glass paces itself to whoever is in front of it. `effects_frame_skip` counts in 60ths of a second and ships at 3, so a terminal being used animates twenty times a second. With nobody in front of it -- the keyboard elsewhere, or five minutes without a keystroke, click or scroll -- it animates at nothing: the picture is held, no frame is drawn for it, and the whole chain costs nothing until somebody comes back. Output still draws while the picture is held; what stops is the animation over it.

| state | rate | cost |
|---|---|---|
| somebody at the glass | 20 Hz | 2.22% |
| nobody at the glass | none | not measured |
| minimised | none | 0.35% |

The middle row has not been measured since the window began holding its picture rather than animating it slowly. The measurement it replaces was of a screen still drawing at 5 Hz, which cost 1.56%, and a screen drawing nothing cannot cost more than that. The 8 ms poll that reads the pty runs either way, so the floor is the poll and not zero; the minimised row is the closest thing to that floor already measured, and the compositor is withholding frames there rather than the terminal deciding to stop.

A minimised window has always been free, and there is nothing there to reclaim in code.

## The compositor pays most of the bill

An idle terminal's own row understates it by about ten times.

| what | no terminal | one idle terminal |
|---|---|---|
| the terminal itself | none running | 0.5% |
| `gnome-shell` | 6.0% | 11.1% |
| `Xwayland` | 3.6% | 4.0% |

Every frame the terminal draws is a frame the compositor recomposites. Someone watching a single process's row in `top` will not see the terminal as the cause of its own cost, and anything that removes frames removes the compositor's share with them. Measured before the attention throttle existed, so these are the figures for a terminal animating at a flat twenty frames a second.

The work is single-threaded. Of 25 threads, the main one carries all of it: 4.8% of a 5.3% total on a fullscreen window. The rest are wgpu and runtime pools that stay parked.

## Where the frames go

Cost divides into a fixed floor and per-frame work. Two independent measurements bracket the floor.

| measurement | result | what it says |
|---|---|---|
| `effects_frame_skip` 3 (20 Hz) against 6 (10 Hz) | 2.42% against 1.60%, 34% less | halving the frames removes about a third, so the floor is near a third of the total |
| visible against minimised | 2.45% against 0.35% | the floor is nearer a seventh |

Take the floor as somewhere between a seventh and a third. Roughly two thirds of the cost is per-frame, and that per-frame work is the CRT chain running over the glass, which is the picture the appliance exists to draw. The frame rate is therefore the only lever large enough to halve the total, which is why it is spent on the states where nobody is looking rather than on the shipped cadence.

## What each part is worth

| part | measurement | share |
|---|---|---|
| the bank column | 2.35% with it, 2.03% without (`chassis_shown`) | 13.8% of total |
| the glass and everything else | the remainder | 86.2% |

The CRT shader chain covers only the well, not the bank: the chain's output goes to the window at the bank's right edge. So the bank's 13.8% is the cost of drawing the chrome, not of any shader pass over it. Measured before the bank's records were kept, so it is the ceiling that work aimed at rather than what the bank costs now.

## Why the bank is remembered

`Cabinet::furniture` keeps the pieces it built and the arguments it built them from. `Chrome` keeps the records built from those pieces, appending the badges to them each frame and cutting them away again, and skips any atlas upload whose bytes it already holds. `Raster` carries its bytes in an `Arc`, so handing the same pieces back is a refcount bump and the atlas compares by pointer rather than by content.

The evidence that bought this arrangement: a profile of an idle fullscreen window, before any of it, put a third of all cycles in one place.

| symbol | share |
|---|---|
| `Vec<TextureBarrier>::from_iter` from `Queue::write_textures` | 15.7% |
| `vulkan::CommandEncoder::transition_textures` | 11.6% |
| `Vec<BufferTextureCopy>::from_iter` | 2.5% |
| `DeviceTextureTracker::set_single` | 1.5% |
| `Queue::write_staging_buffer_impl` | 1.0% |

Every piece of furniture was rasterised to RGBA and written to the chrome atlas on every frame, and each write cost a texture transition. Alongside it sat the rasterising: `to_rgba8` at 4.7%, `skrifa`'s autohinting at 1.7%, `read_fonts`'s cmap iteration at 1.0%, and about 9% in the allocator feeding them. The profile now is flat and none of those symbols appear in it. What sits at the top is swapchain acquisition, the librashader filter passes, and event-loop syscalls.

The draw itself still runs every frame. A frame goes into one of several rotating swapchain images, and one that omitted the column would leave an older frame showing; wgpu does not say which image it handed over, so skipping the draw would need hal-level bookkeeping.

## What the two rounds of work came to

Measured against the release before each, both builds running at once so they share the machine.

| round | state | before | after | less |
|---|---|---|---|---|
| pieces remembered, bytes shared, uploads skipped | idle | 2.95% | 2.58% | 12.5% |
| records kept, clock watches attention | focused, being typed into | 2.72% | 2.22% | 18.3% |
| records kept, clock watches attention | unfocused and untouched | 3.00% | 1.56% | 48.1% |

Compounded, an unattended terminal costs 54.6% less than before either round, and one being typed into costs 28.5% less with no change to the picture at any moment.

## Measuring this without being misled

Five traps, each of which produced a wrong number here before it was understood.

**A second launch joins the first.** The terminal is single-instance: it finds the socket in `$XDG_RUNTIME_DIR`, hands its request over, and exits, so the measured process dies at once and a window appears in somebody else's session. Give every instance under measurement its own `XDG_RUNTIME_DIR`. The module doc in `crates/app/src/instance.rs` states the rule; separate runtime directories never collide.

**Windows that overlap stop drawing.** An occluded window reads about 0.35%, the same as a minimised one, and it looks exactly like a large improvement. A harness that leaves windows stacked reports the later runs as nearly free. Place windows so none covers another, and treat any reading near that floor as occlusion until proved otherwise.

**A window the harness never found reads the same floor.** A run that fails to resolve its window ids sizes, moves and focuses nothing, and every instance in it reads about 0.67% whatever state it was meant to be in. `xdotool` prints `BadWindow` when this happens, among output that otherwise looks like a result. Check each id is non-empty before trusting the run.

**The machine drifts.** The same binary measured 4.00% and then 2.80% with nothing changed, and a run comparing attention states one after another put the cheapest state highest while `gnome-shell` tripled underneath it. Sequential A against B cannot resolve an effect of 10 or 20%. Run every arm at once, at the same window size, and compare them within the one interval.

**Software rendering measures the wrong thing.** Under Xvfb with `LIBGL_ALWAYS_SOFTWARE=1` the shading runs on the CPU, which is the work a GPU would otherwise do. Measure on a real display and confirm the adapter: the log line reads `wgpu adapter … on Vulkan`.

A profile needs symbols, which the shipped packages do not carry. `DEB_BUILD_OPTIONS=nostrip CARGO_PROFILE_RELEASE_DEBUG=2 dpkg-buildpackage -us -uc -b` builds a package that has them; `BUILD.md` covers the rest. `perf record -F 997 -p <pid>` with a flat report is enough to find a hot symbol. DWARF call graphs on a binary of that size take longer to unwind than they are worth.

To hold a window in a chosen attention state while measuring it, `xdotool key --window <id> shift` stamps the input clock without typing anything into the shell, and reaches a window that does not hold the keyboard.
