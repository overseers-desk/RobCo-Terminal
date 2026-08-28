#!/usr/bin/env bash
#
# Build the self-contained single-file robco-settings image: an executable
# that runs on a host with no Tcl installed. It stubs `zipfs mkimg` on a
# from-source Tcl 9 whose libtcl/libtk are statically linked and whose script
# library is embedded, so the result links only libc and the platform GUI
# substrate (X11 on Linux).
#
# Stages, all from source so nothing is inherited from the build host's Tcl:
#   1. static Tcl 9   (--disable-shared --enable-zipfs)
#   2. static Tk 9    (--disable-shared, against that Tcl)
#   3. a custom wish  (zipfs/appinit.c) linking the two, registering Tk as a
#                     static library. robco-settings uses no worker threads,
#                     so unlike questlog's wish this one links no Thread
#                     extension.
#   4. a runtime tree (tcl_library/, tk_library/) handed to zipfs/build.tcl,
#                     which stages robco-settings's payload and folds
#                     everything onto the custom wish.
#
# Build dependencies:
#   Linux (Debian/Ubuntu names; the CI workflow installs them):
#     build-essential libx11-dev libxext-dev libxft-dev libfontconfig1-dev \
#     libxss-dev zlib1g-dev curl
#   macOS: the Xcode command-line tools (cc, make) and curl, both present on
#     GitHub macOS runners. Tk builds against the system Aqua frameworks.
#
# Usage:
#   zipfs/build-selfcontained.sh            # builds dist/robco-settings-<ver>-linux-<arch>
#   BUILD_DIR=/path zipfs/build-selfcontained.sh
#   ROBCO_SETTINGS_DIST_DIR=/path zipfs/build-selfcontained.sh   # writes the image elsewhere

set -euo pipefail

# Dependency versions. Tcl and Tk track the same release.
TCL_VER="${TCL_VER:-9.0.2}"
TK_VER="${TK_VER:-9.0.2}"

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUILD_DIR="${BUILD_DIR:-$(mktemp -d "${TMPDIR:-/tmp}/robco-settings-selfcontained.XXXXXX")}"
SRC="$BUILD_DIR/src"
STAGE="$BUILD_DIR/interp"      # install prefix for the from-source interpreter
RUNTIME="$BUILD_DIR/runtime"   # script-library tree overlaid into the image
JOBS="$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 2)"
OS="$(uname -s)"
CC="${CC:-cc}"

mkdir -p "$SRC" "$STAGE" "$RUNTIME"
echo "build dir: $BUILD_DIR"

fetch() {
    # fetch URL OUTFILE: download and verify it untars.
    #
    # --retry-all-errors as well as --retry, because the failure seen here is
    # a transfer that closes mid-stream: curl calls that error 18, and plain
    # --retry covers timeouts and transient HTTP statuses rather than a
    # truncated body.
    local url="$1" out="$2"
    curl -fsSL --retry 3 --retry-all-errors -o "$SRC/$out" "$url"
    tar tzf "$SRC/$out" >/dev/null
}

echo "== fetching sources =="
fetch "https://prdownloads.sourceforge.net/tcl/tcl${TCL_VER}-src.tar.gz" "tcl.tar.gz"
fetch "https://prdownloads.sourceforge.net/tcl/tk${TK_VER}-src.tar.gz"   "tk.tar.gz"
for f in tcl tk; do tar xzf "$SRC/$f.tar.gz" -C "$SRC"; done

TCL_SRC="$SRC/tcl${TCL_VER}"
TK_SRC="$SRC/tk${TK_VER}"

echo "== 1. static Tcl =="
( cd "$TCL_SRC/unix"
  ./configure --disable-shared --enable-zipfs --prefix="$STAGE"
  make -j"$JOBS"
  make install )

echo "== 2. static Tk =="
# macOS Tk renders through Aqua (Cocoa frameworks), Linux Tk through X11/Xft.
if [ "$OS" = "Darwin" ]; then
    TK_CONFIG_FLAGS="--enable-aqua"
else
    TK_CONFIG_FLAGS="--enable-xft --enable-xss"
fi
( cd "$TK_SRC/unix"
  ./configure --disable-shared $TK_CONFIG_FLAGS \
      --with-tcl="$TCL_SRC/unix" --prefix="$STAGE"
  make -j"$JOBS"
  make install )

echo "== 3. custom wish =="
# Link specs (X11/font libs, zlib, pthread) come from the generated config so
# they track what Tk was actually built against.
# shellcheck disable=SC1091
. "$STAGE/lib/tclConfig.sh"
# shellcheck disable=SC1091
. "$STAGE/lib/tkConfig.sh"
WISH="$BUILD_DIR/robco-settings-wish"
$CC -o "$WISH" "$REPO_ROOT/zipfs/appinit.c" \
    -I"$STAGE/include" \
    "$STAGE/lib/libtcl9tk${TK_VER%.*}.a" \
    "$STAGE/lib/libtcl${TCL_VER%.*}.a" \
    "$STAGE/lib/libtclstub.a" \
    $TK_LIBS
# The image must carry no Tcl/Tk shared dependency.
if [ "$OS" = "Darwin" ]; then
    deps="$(otool -L "$WISH")"
else
    deps="$(ldd "$WISH")"
fi
if echo "$deps" | grep -Eiq 'libtcl|libtk9'; then
    echo "wish unexpectedly links a Tcl/Tk shared library:" >&2
    echo "$deps" >&2
    exit 1
fi

echo "== 4. runtime tree + image =="
# The static install embeds its script library only in the zip appended to
# the stock tclsh, not on disk, so the authoritative library trees are the
# source library/ dirs (which is what got zipped).
cp -a "$TCL_SRC/library" "$RUNTIME/tcl_library"
cp -a "$TK_SRC/library"  "$RUNTIME/tk_library"

ROBCO_SETTINGS_WISH="$WISH" ROBCO_SETTINGS_RUNTIME="$RUNTIME" \
    "$STAGE/bin/tclsh${TCL_VER%.*}" "$REPO_ROOT/zipfs/build.tcl"

# The launcher is the version's one home; build.tcl reads it the same way, so
# the filename this script reconstructs is the one build.tcl just wrote.
VERSION="$(sed -n 's/^set ROBCO_SETTINGS_VERSION[[:space:]]*\([^[:space:]]*\).*/\1/p' "$REPO_ROOT/robco-settings")"
ARCH="$(uname -m)"
if [ "$ARCH" = "aarch64" ]; then ARCH=arm64; fi

if [ "$OS" = "Darwin" ]; then
    IMAGE="$REPO_ROOT/dist/robco-settings-$VERSION-macos-$ARCH"
    # An image cannot be re-signed. mkimg appends its archive past the end of
    # the Mach-O's __LINKEDIT, and codesign refuses that shape outright: "main
    # executable failed strict validation". So what the image carries is the
    # ad-hoc signature the linker gave the wish, covering the bytes that
    # existed then, with the archive sitting beyond the signed region.
    #
    # Whether Apple Silicon accepts that is the question this answers. It
    # enforces a valid signature on every arm64 binary, and the test of what
    # "valid" admits is to run one: --version answers without a display, so
    # exec, mount and startup are all exercised on a headless runner. A kernel
    # that refuses the shape kills the process here, and the build stops.
    echo "== 5. run the image =="
    "$IMAGE" --version

    # A .app bundle and .dmg (as questlog's zipfs/macos-bundle.sh produces)
    # are out of scope here; wrapping this image the same way is follow-up
    # work, not done by this script.
fi

echo "done. Keep \$BUILD_DIR for reuse, or rm -rf $BUILD_DIR"
