#!/usr/bin/env bash
# Screenshot one build of the terminal at a given profile and size.
#
# Runs on a private Xvfb display (llvmpipe renders the GL shaders), so the
# session's compositor cannot clamp the window size, refuse synthetic input,
# or offer a stale window of the same class. A scratch HOME keeps the
# settings database and single-instance socket away from any real session.
#
# usage: snap.sh <binary> <profile> <out.png> [width] [height] [channels]
set -euo pipefail

BIN=$(realpath "${1:?binary}")
PROFILE=${2:?profile name}
OUT=$(realpath -m "${3:?output png}")
W=${4:-1448}
H=${5:-1086}
CHANNELS=${6:-16}

if [ -z "${SNAP_INNER:-}" ]; then
    exec env SNAP_INNER=1 xvfb-run -a -s "-screen 0 ${W}x${H}x24" \
        "$0" "$BIN" "$PROFILE" "$OUT" "$W" "$H" "$CHANNELS"
fi

SCRATCH=$(mktemp -d /tmp/robco-snap.XXXXXX)
trap 'kill $APP_PID 2>/dev/null || true; wait $APP_PID 2>/dev/null || true; rm -rf "$SCRATCH"' EXIT

mkdir -p "$SCRATCH/run"
chmod 700 "$SCRATCH/run"

env HOME="$SCRATCH" \
    XDG_DATA_HOME="$SCRATCH/.local/share" \
    XDG_CONFIG_HOME="$SCRATCH/.config" \
    XDG_CACHE_HOME="$SCRATCH/.cache" \
    XDG_RUNTIME_DIR="$SCRATCH/run" \
    LIBGL_ALWAYS_SOFTWARE=1 \
    "$BIN" --default-settings --profile "$PROFILE" &
APP_PID=$!

# The app maps a 10x10 helper window besides the real one; take the window of
# this pid with real size.
WID=""
for _ in $(seq 60); do
    for cand in $(xdotool search --class "$(basename "$BIN")" 2>/dev/null); do
        pid=$(xdotool getwindowpid "$cand" 2>/dev/null || echo 0)
        geom=$(xdotool getwindowgeometry "$cand" 2>/dev/null | sed -n 's/.*Geometry: \([0-9]*\)x.*/\1/p')
        if [ "$pid" = "$APP_PID" ] && [ "${geom:-0}" -gt 100 ]; then WID=$cand; break 2; fi
    done
    sleep 0.5
done
[ -n "$WID" ] || { echo "no sizable window of pid $APP_PID found" >&2; exit 1; }

xdotool windowsize --sync "$WID" "$W" "$H" || true
xdotool windowfocus --sync "$WID"
sleep 1

for _ in $(seq 2 "$CHANNELS"); do
    xdotool key --clearmodifiers ctrl+shift+t
    sleep 0.25
done

# Let shells print their prompts and the tube settle.
sleep 3

import -window "$WID" "$OUT"
echo "$OUT"
