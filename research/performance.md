# What the terminal costs to keep on screen

Every number here was measured on this project's own binary. The methods and the traps are recorded alongside, because the traps are what make a naive re-measurement disagree.

Machine: 16 cores, NVIDIA RTX 2070 SUPER, GNOME on Wayland with Xwayland. Percentages are of one core, taken from `utime + stime` in `/proc/<pid>/stat` over a fixed interval. A terminal running `sh -c 'sleep N'` with no output, on the shipped defaults, unless a row says otherwise.

## The shape of the cost

An idle terminal costs roughly 2.5% to 3% of one core, and about nine tenths as much again in the compositor.

| what | no terminal | one idle terminal |
|---|---|---|
| the terminal itself | — | 0.5% |
| `gnome-shell` | 6.0% | 11.1% |
| `Xwayland` | 3.6% | 4.0% |

The terminal redraws its whole window twenty times a second whether or not anything changed, because the CRT effects animate. The compositor recomposites the screen each time. Someone watching a single process's row in `top` will not see the terminal as the cause of its own cost.

The work is single-threaded. Of 25 threads, the main one carries all of it: 4.8% of a 5.3% total on a fullscreen window. The rest are wgpu and runtime pools that stay parked.

## Where the frames go

Cost divides into a fixed floor and per-frame work. Two independent measurements bracket the floor.

| measurement | result | what it says |
|---|---|---|
| `effects_frame_skip` 3 (20 Hz) against 6 (10 Hz) | 2.42% against 1.60%, 34% less | halving the frames removes about a third, so the floor is near a third of the total |
| visible against minimised | 2.45% against 0.35% | the floor is nearer a seventh |

Take the floor as somewhere between a seventh and a third. Roughly two thirds of the cost is per-frame, and that per-frame work is the CRT chain running over the glass, which is the picture the appliance exists to draw.

`effects_frame_skip` counts in 60ths of a second and ships at 3, so the glass animates twenty times a second.

## What each part is worth

| part | measurement | share |
|---|---|---|
| the bank column | 2.35% with it, 2.03% without (`chassis_shown`) | 13.8% of total |
| the glass and everything else | the remainder | 86.2% |

The CRT shader chain covers only the well, not the bank: the chain's output goes to the window at the bank's right edge. So the bank's 13.8% is the cost of drawing the chrome, not of any shader pass over it.

## States that cost full price

| state | cost | throttled? |
|---|---|---|
| focused, visible | 2.88% | no |
| unfocused, visible | 3.16% | no |
| minimised | 0.35% | yes, by the compositor |

The difference between focused and unfocused is noise: nothing in the code reads focus, and the two runs bracket each other. A terminal sitting in view that nobody has touched for an hour animates at the same rate as one being typed into. A minimised one costs nothing, but that is the compositor withholding frames rather than the terminal deciding to stop.

## What the bank used to spend, and on what

Before the pieces were remembered, a profile of an idle fullscreen window put a third of all cycles in one place:

| symbol | share |
|---|---|
| `Vec<TextureBarrier>::from_iter` from `Queue::write_textures` | 15.7% |
| `vulkan::CommandEncoder::transition_textures` | 11.6% |
| `Vec<BufferTextureCopy>::from_iter` | 2.5% |
| `DeviceTextureTracker::set_single` | 1.5% |
| `Queue::write_staging_buffer_impl` | 1.0% |

Every piece of furniture was rasterised to RGBA and written to the chrome atlas on every frame, and each write cost a texture transition. Alongside it sat the rasterising itself: `to_rgba8` at 4.7%, `skrifa`'s autohinting at 1.7%, `read_fonts`'s cmap iteration at 1.0%, and about 9% in the allocator feeding them.

Remembering the pieces, sharing the raster bytes, and skipping an upload the atlas already holds took 12.5% off the total: 2.95% against 2.58%, both builds running at once. The profile afterwards is flat, and the symbols above are gone from it. What sits at the top now is swapchain acquisition, the librashader filter passes, and event-loop syscalls.

## Measuring this without being misled

Four traps, each of which produced a wrong number here before it was understood.

**A second launch joins the first.** The terminal is single-instance: it finds the socket in `$XDG_RUNTIME_DIR`, hands its request over, and exits, so the measured process dies at once and a window appears in somebody else's session. Give every instance under measurement its own `XDG_RUNTIME_DIR`. The module doc in `crates/app/src/instance.rs` states the rule; separate runtime directories never collide.

**Windows that overlap stop drawing.** An occluded window reads about 0.35%, the same as a minimised one, and it looks exactly like a large improvement. A harness that leaves windows stacked will report the second and third runs as free. Place windows so none covers another, and treat any reading near 0.35% as occlusion until proved otherwise.

**The machine drifts.** The same binary measured 4.00% and then 2.80% with nothing changed. Sequential A against B cannot resolve an effect of 10 or 20%. Run both builds at once, at the same window size, and compare them within the run.

**Software rendering measures the wrong thing.** Under Xvfb with `LIBGL_ALWAYS_SOFTWARE=1` the shading runs on the CPU, which is the work a GPU would otherwise do. Measure on a real display and confirm the adapter: the log line reads `wgpu adapter … on Vulkan`.

A profile needs symbols, which the shipped packages do not carry. `DEB_BUILD_OPTIONS=nostrip CARGO_PROFILE_RELEASE_DEBUG=2 dpkg-buildpackage -us -uc -b` builds a package that has them; `BUILD.md` covers the rest. `perf record -F 997 -p <pid>` with a flat report is enough to find a hot symbol. DWARF call graphs on a binary of that size take longer to unwind than they are worth.
