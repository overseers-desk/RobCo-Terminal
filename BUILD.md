# Building and packaging

What the build requires, why it is arranged this way, and what each package route does underneath. The README carries what an adopter needs; the rest of that knowledge lives here.

## Toolchains

Rust 1.96.1 or newer (developed on 1.97.1), and a C++ compiler for the terminal core's SIMD dependency. The SSH stack's crypto (`ring`) uses the same C toolchain and links statically, adding no requirement of its own; on Windows its build also wants `nasm`.

## Platforms

The stack was chosen for the three desktop markets: the terminal core speaks ConPTY, the GPU layer covers Metal and D3D from the same shader source, and the config paths are implemented for all three. The terminal cannot be cross-compiled from Linux, because the C++ dependency needs a native toolchain; `.github/workflows/ci.yml` proves that build on every push instead, alongside the settings toolchain (`settings/zipfs/build-selfcontained.ps1 -Embed`), the exe that embeds its payload, and the payload proved from inside it with `--settings-selftest`. Its Linux job builds the tarball and the `.deb` on a pinned `ubuntu-24.04`, the package's glibc floor. Push a version tag and `.github/workflows/release.yml` builds that one exe and attaches it, bare, to the tag's release; its macOS job builds the terminal and the self-contained settings image on `macos-latest`, bundles them into `RobCo Terminal.app` via `packaging/macos/bundle.sh`, and attaches the result as a `.dmg`.

A local Windows build needs Rust and a C++ compiler for the terminal, plus `nasm` for `ring`'s assembly, same as the rest of this page; `cargo build --release -p robco-app` produces a `robco-term.exe` that spawns a separately built `robco-settings.exe` for right-click; the shipped shape embeds the settings program instead (`--features embedded-settings`, fed by the `-Embed` run below). Either way the settings side needs its own toolchain instead of Tcl/Tk installed anywhere: Visual Studio with the C++ toolset (for `nmake` and `cl`, found via `vswhere`), plus `tar` and `curl` (both in Windows 10 1803 and later). `settings/zipfs/build-selfcontained.ps1` runs the from-source Tcl/Tk build and the link itself; its header has the full account, the Windows twin of `build-selfcontained.sh` below.

## What is inside the binary

Fonts, shaders, presets and the noise texture are compiled in, which is what makes the installation three files and relocatable. The inventory and its reasoning live in `crates/xtask/src/install.rs`'s module doc.

## Routes

`cargo run -p xtask -- install --prefix <dir>` writes the binary, the desktop entry and the icon; `--destdir` stages under another root for packaging. Every route checks the installed copy starts in a scrubbed environment before it reports a path.

`cargo run -p xtask -- dist` rolls the same layout into a tarball, and requires `--settings-binary`: the self-contained robco-settings image that `settings/zipfs/build-selfcontained.sh` builds from static Tcl/Tk, because a tarball promises to run on a host with nothing installed.

`dpkg-buildpackage -us -uc -b` builds the Debian package through `debian/rules`, which stages the layout via `cargo run -p xtask -- stage-deb`. debhelper strips the binary and splits the symbols into `robco-term-dbgsym`; `dh_shlibdeps` reads the C dependencies out of the built binary rather than a hand-kept list; the settings window ships as Tcl sources running on the distribution's own `tcl9.0`/`tk9.0`, declared in `debian/control`. Artifacts land in the parent directory, dpkg-buildpackage's convention.
