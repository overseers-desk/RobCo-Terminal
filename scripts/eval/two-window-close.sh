#!/usr/bin/env bash
# Two windows on one process, then the first dies: the process must not.
# The second window arrives through the single-instance handoff, which is
# how a resource wrongly held per-window gets its second copy; the first
# case was a per-window VkInstance that NVIDIA's Wayland driver answered
# with a segfault at the first window's close. Needs a live display and
# the real driver; the suite's one_instance test is the CI-side twin.
#
# The binary is copied under a scratch name so the handoff meets this
# run's own socket, never a terminal the user has open.
# usage: two-window-close.sh <binary>
set -u
BIN=$(realpath "${1:?binary}")
SCRATCH=$(mktemp -d)
trap 'rm -rf "$SCRATCH"' EXIT
cp "$BIN" "$SCRATCH/two-window-close-probe"

"$SCRATCH/two-window-close-probe" -e sleep 12 > "$SCRATCH/first.log" 2>&1 &
FIRST=$!
sleep 4
"$SCRATCH/two-window-close-probe" -e sleep 25 > "$SCRATCH/handoff.log" 2>&1 \
    || { echo "FAIL: handoff refused"; exit 1; }
# The first window's command ends at t=12 and takes the window with it;
# the survivor holds the process until t=29 or so.
for _ in $(seq 35); do
    kill -0 "$FIRST" 2>/dev/null || break
    sleep 1
done
if kill -0 "$FIRST" 2>/dev/null; then
    kill "$FIRST"
    echo "FAIL: process still running at t=39; the survivor never let go"
    exit 1
fi
wait "$FIRST"
RC=$?
if [ "$RC" -ne 0 ]; then
    echo "FAIL: process died with the first window (rc=$RC)"
    grep -m3 -iE "fatal|signal|panic" "$SCRATCH/first.log"
    exit 1
fi
echo "ok: first window closed, survivor carried the process to a clean exit"
