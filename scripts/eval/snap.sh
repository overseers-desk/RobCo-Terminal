#!/usr/bin/env bash
# Screenshot one build of the terminal at a given profile and size.
#
# Runs on a private Xvfb display (llvmpipe renders the GL shaders), so the
# session's compositor cannot clamp the window size, refuse synthetic input,
# or offer a stale window of the same class. A scratch HOME keeps the
# settings database and single-instance socket away from any real session.
#
# usage: snap.sh <binary> <profile> <out.png> [width] [height] [channels] [units]
#
# UNITS, when given, re-fits the LED strips to that character count the way
# the user does it: a pointer drag on the seam between the bank and the
# screen well. The seam lands where the bank's implicitWidth puts it, so the
# grab point is computed from the shell's fixed furniture plus 12px per
# visible character (the LED display's unitWidth at the default font). The
# strip's two end-pad cells sit outside the character count, so their 24px
# ride with the fixed furniture. FIXED below must be re-derived by hand if a
# shell's Metrics.qml changes.
set -euo pipefail

BIN=$(realpath "${1:?binary}")
PROFILE=${2:?profile name}
OUT=$(realpath -m "${3:?output png}")
W=${4:-1448}
H=${5:-1086}
CHANNELS=${6:-16}
UNITS=${7:-}

if [ -z "${SNAP_INNER:-}" ]; then
    exec env SNAP_INNER=1 xvfb-run -a -s "-screen 0 ${W}x${H}x24" \
        "$0" "$BIN" "$PROFILE" "$OUT" "$W" "$H" "$CHANNELS" "$UNITS"
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

if [ -n "$UNITS" ]; then
    # The bank's width at N visible characters: the shell's fixed furniture
    # (contentX + numeralWidth + columnGap + rightPadding, per its Metrics)
    # plus the strip's 24px of end-pad cells, plus 12px a character. Default
    # character count is 12.
    case "$PROFILE" in
        "RobCo Amber") FIXED=176 ;;   # 8+70+24+50 + 24
        "RobCo Blue")  FIXED=222 ;;   # 97+50+27+24 + 24
        *)             FIXED=78 ;;    # moulded-plastic, glow: 10+34+10+0 + 24
    esac
    START=$((FIXED + 12 * 12 + 2))
    TARGET=$((FIXED + 12 * UNITS + 2))

    eval "$(xdotool getwindowgeometry --shell "$WID")" # sets X, Y
    MIDY=$((Y + H / 2))

    xdotool mousemove --sync $((X + START)) "$MIDY"
    xdotool mousedown 1
    STEP=$(( TARGET > START ? 24 : -24 ))
    POS=$START
    while [ $(( (TARGET - POS) * STEP )) -gt 0 ]; do
        POS=$((POS + STEP))
        if [ $(( (TARGET - POS) * STEP )) -lt 0 ]; then POS=$TARGET; fi
        xdotool mousemove --sync $((X + POS)) "$MIDY"
        sleep 0.05
    done
    xdotool mousemove --sync $((X + TARGET)) "$MIDY"
    sleep 0.2
    xdotool mouseup 1
fi

# Let shells print their prompts and the tube settle.
sleep 3

import -window "$WID" "$OUT"
echo "$OUT"
