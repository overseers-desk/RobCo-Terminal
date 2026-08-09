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
# screen well. The bank's width is measured off the live window rather than
# tabulated: the window exports minimumWidth, the bank's implicitWidth plus
# the screen well's floor, as the X11 program-specified minimum size.
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
    # The screen well's floor, read from the source beside this script;
    # 320 when the script runs against a binary outside its tree.
    CRT_MIN=$(sed -n 's/.*crtMinimumWidth: \([0-9]*\).*/\1/p' \
        "$(dirname "$(realpath "$0")")/../../app/qml/TerminalWindow.qml" 2>/dev/null | head -1)
    CRT_MIN=${CRT_MIN:-320}

    # The bank's live width: the window's program-specified minimum size
    # less the well's floor.
    bank_width() {
        xprop -id "$WID" WM_NORMAL_HINTS \
            | sed -n 's/.*program specified minimum size: \([0-9]*\) by.*/\1/p' \
            | awk -v m="$CRT_MIN" '{print $1 - m}'
    }

    START_BANK=$(bank_width)
    if [ "${START_BANK:-0}" -le 0 ]; then
        echo "profile has no channel bank; UNITS is meaningless" >&2
        exit 1
    fi

    # Grab 2px into the seam's lean past the bank's edge. 12px a character
    # is the LED display's unitWidth at the default font (displays/led/
    # Metrics.qml), from the default count of 12 (ApplicationSettings.qml
    # ledCharacters), both guaranteed by --default-settings.
    START=$((START_BANK + 2))
    TARGET=$((START + 12 * (UNITS - 12)))

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

    # Verify the fit against the same instrument: a drag that missed by a
    # character or more is a loud warning, never a quietly wrong picture.
    sleep 0.5
    END_BANK=$(bank_width)
    WANT=$((START_BANK + 12 * (UNITS - 12)))
    DIFF=$((END_BANK - WANT)); [ "$DIFF" -lt 0 ] && DIFF=$((-DIFF))
    if [ "$DIFF" -ge 12 ]; then
        echo "warning: seam drag landed at bank width $END_BANK, wanted $WANT" >&2
    fi
fi

# Let shells print their prompts and the tube settle.
sleep 3

import -window "$WID" "$OUT"
echo "$OUT"
