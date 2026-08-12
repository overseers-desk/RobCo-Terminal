#!/usr/bin/env bash
# Drive the tmux -CC page flow end to end and judge it: attach transport,
# per-page windows, the page's lifetime, detach and gateway death, and two
# simultaneous attachments. Same private-Xvfb / scratch-HOME / TMPDIR pattern
# as snap.sh, with private tmux sockets under the scratch so nothing touches
# a real server. Assertions read the window title (xprop _NET_WM_NAME echoes
# the store's currentTitle), the servers' own books (tmux list-clients,
# list-windows), and screenshots for what only the glass can say.
#
# Every scratch shell titles itself "home <nonce>" from PROMPT_COMMAND, and
# the flow retitles the slots it uses to "slotN <nonce>": a title names its
# slot, and a fresh nonce per prompt is what lets "the shell's title escape
# repaints on the next prompt" be observed as a change.
#
# The anchor's input law is read off tmux's own reading of the wire. A
# control-mode client takes its stdin as a line of tmux commands, and the
# empty line is the one that means "detach": so a swallowed `ls` followed by
# Enter detaches, while an `ls` that reached the wire would make the line a
# command tmux does not know and no detach would follow. That difference is
# the assertion; the glass is checked besides, against a floor two shots of
# one still screen set between them.
#
# One tmux fact the flow leans on: an attached session cannot stand with
# zero windows, killing the last one kills the session itself. So the rule-4
# assertion is that the page outlives every window-close short of the
# session's own death with no detach asked from our side, and the last
# close collapses the page through tmux's own %exit, landing home.
#
# usage: tmux-flow.sh <binary> [outdir]
set -uo pipefail

BIN=$(realpath "${1:?binary}")
OUTDIR=$(realpath -m "${2:-/tmp/robco-flow-out}")
W=1448
H=1086

if [ -z "${SNAP_INNER:-}" ]; then
    exec env SNAP_INNER=1 xvfb-run -a -s "-screen 0 ${W}x${H}x24" "$0" "$BIN" "$OUTDIR"
fi

mkdir -p "$OUTDIR"
FAIL=0
ok()  { echo "ok:   $*"; }
bad() { echo "FAIL: $*"; FAIL=1; }

APP_PID=""
SCRATCH=""
WID=""

cleanup() {
    [ -n "$SCRATCH" ] && tmux -S "$SCRATCH/tmp/sock1" kill-server 2>/dev/null
    [ -n "$SCRATCH" ] && tmux -S "$SCRATCH/tmp/sock2" kill-server 2>/dev/null
    [ -n "$APP_PID" ] && kill "$APP_PID" 2>/dev/null
    [ -n "$APP_PID" ] && wait "$APP_PID" 2>/dev/null
}
trap 'cleanup' EXIT

# Raise one appliance on a fresh scratch; names APP_PID, SCRATCH, WID.
launch() {
    local profile=$1 log=$2
    SCRATCH=$(mktemp -d /tmp/robco-flow.XXXXXX)
    mkdir -p "$SCRATCH/run" "$SCRATCH/tmp"
    chmod 700 "$SCRATCH/run"
    printf 'PROMPT_COMMAND=%s\n' \
        "'printf \"\\033]0;home %s\\007\" \"\$RANDOM\$RANDOM\"'" \
        > "$SCRATCH/.bashrc"
    env HOME="$SCRATCH" \
        XDG_DATA_HOME="$SCRATCH/.local/share" \
        XDG_CONFIG_HOME="$SCRATCH/.config" \
        XDG_CACHE_HOME="$SCRATCH/.cache" \
        XDG_RUNTIME_DIR="$SCRATCH/run" \
        TMPDIR="$SCRATCH/tmp" \
        QT_QPA_PLATFORM=xcb \
        LIBGL_ALWAYS_SOFTWARE=1 \
        "$BIN" --default-settings --profile "$profile" > "$OUTDIR/$log" 2>&1 &
    APP_PID=$!

    WID=""
    for _ in $(seq 60); do
        for cand in $(xdotool search --class "$(basename "$BIN")" 2>/dev/null); do
            local pid geom
            pid=$(xdotool getwindowpid "$cand" 2>/dev/null || echo 0)
            geom=$(xdotool getwindowgeometry "$cand" 2>/dev/null \
                   | sed -n 's/.*Geometry: \([0-9]*\)x.*/\1/p')
            if [ "$pid" = "$APP_PID" ] && [ "${geom:-0}" -gt 100 ]; then WID=$cand; break 2; fi
        done
        sleep 0.5
    done
    [ -n "$WID" ] || { echo "no sizable window of pid $APP_PID" >&2; exit 1; }
    xdotool windowsize --sync "$WID" "$W" "$H" || true
    xdotool windowfocus --sync "$WID"
    sleep 2
}

shutdown_app() {
    kill "$APP_PID" 2>/dev/null
    wait "$APP_PID" 2>/dev/null
    APP_PID=""
    WID=""
}

wm_name() {
    xprop -id "$WID" -notype _NET_WM_NAME 2>/dev/null \
        | sed -n 's/^_NET_WM_NAME = "\(.*\)"$/\1/p'
}

# Wait until the title matches, then log; a timeout is a failure.
wait_wm() {
    local re=$1 label=$2 tries=${3:-30}
    for _ in $(seq "$tries"); do
        if wm_name | grep -qE "$re"; then ok "$label [$(wm_name)]"; return 0; fi
        sleep 0.5
    done
    bad "$label [$(wm_name)] wanted /$re/"
    return 1
}

# The title must still match after a beat: how a refusal is observed.
still_wm() {
    local re=$1 label=$2
    sleep 2
    if wm_name | grep -qE "$re"; then ok "$label [$(wm_name)]"; else bad "$label [$(wm_name)] wanted /$re/"; fi
}

type_line() {
    xdotool type --clearmodifiers --delay 25 "$1"
    xdotool key --clearmodifiers Return
}

shot() {
    import -window "$WID" "$OUTDIR/$1"
}

# How far apart two shots of the glass are, root-mean-square over the whole
# frame. The tube burns and flickers of its own accord, so two shots of one
# still screen are never identical and the number only means anything against
# that floor.
glass_rmse() {
    compare -metric RMSE "$OUTDIR/$1" "$OUTDIR/$2" null: 2>&1 \
        | sed -n 's/.*(\([0-9.e-]*\)).*/\1/p'
}

# a <= b: awk, the shell having no floats.
le() { awk -v a="$1" -v b="$2" 'BEGIN { exit !(a <= b) }'; }

# Retitle the shell on the air so its titles name its slot from here on.
retitle() {
    type_line "PROMPT_COMMAND='printf \"\\033]0;slot$1 %s\\007\" \"\$RANDOM\$RANDOM\"'"
}

clients()  { tmux -S "$1" list-clients 2>/dev/null | wc -l; }
windows()  { tmux -S "$1" list-windows 2>/dev/null | wc -l; }

HOSTSHORT=$(hostname -s)
ANCHOR_RE="^tmux -CC # @${HOSTSHORT}\$"

echo "== phase 1: RobCo Amber, one attachment, the whole life of a page =="
launch "RobCo Amber" phase1.log
SOCK1="$SCRATCH/tmp/sock1"
tmux -S "$SOCK1" new-session -d -s one

wait_wm '^home ' "channel 1 up, shell titles itself"
xdotool key --clearmodifiers ctrl+shift+t; sleep 1
xdotool key --clearmodifiers ctrl+shift+t; sleep 1
xdotool key --clearmodifiers alt+1; sleep 1; retitle 1; wait_wm '^slot1 ' "slot 1 retitled"
xdotool key --clearmodifiers alt+2; sleep 1; retitle 2; wait_wm '^slot2 ' "slot 2 retitled"
xdotool key --clearmodifiers alt+3; sleep 1; retitle 3; wait_wm '^slot3 ' "slot 3 retitled"

echo "-- 1: attach on channel 2 of 3: transport, title, no blank frame --"
xdotool key --clearmodifiers alt+2; sleep 1
type_line "tmux -S $SOCK1 -CC attach -t one"
wait_wm "$ANCHOR_RE" "anchor on the air, titled for the machine"
[ "$(clients "$SOCK1")" -eq 1 ] && ok "control client attached" || bad "no client on sock1"
sleep 1
shot flow1-attach.png
# The glass must hold the frozen pre-attach screen: a blanked frame would
# read near black where the prompt and the typed command stand.
MEAN=$(convert "$OUTDIR/flow1-attach.png" -crop 700x160+320+60 -colorspace Gray -format "%[fx:int(mean*65535)]" info:)
[ "${MEAN:-0}" -gt 800 ] && ok "glass holds the frozen screen (crop mean $MEAN)" \
                         || bad "glass looks blanked (crop mean $MEAN)"

echo "-- 1b: the home slot is held dark and refuses --"
xdotool key --clearmodifiers alt+Prior; sleep 1
shot flow1-home-held.png
xdotool key --clearmodifiers alt+2
still_wm "$ANCHOR_RE" "chord on the held slot refused, air stays on the anchor"

echo "-- 2a: Ctrl+Shift+T on home skips the held slot --"
xdotool key --clearmodifiers ctrl+shift+t
wait_wm '^home ' "new local shell took a free slot, not the held one"
xdotool key --clearmodifiers ctrl+shift+w
wait_wm '^slot3 ' "closed again; air handed to a home neighbour"

echo "-- 2b: Ctrl+Shift+T on the session page asks tmux --"
xdotool key --clearmodifiers alt+Next; sleep 1
xdotool key --clearmodifiers alt+1
wait_wm "$ANCHOR_RE" "anchor selected across pages by chord"
xdotool key --clearmodifiers ctrl+shift+t; sleep 2
xdotool key --clearmodifiers ctrl+shift+t; sleep 2
[ "$(windows "$SOCK1")" -eq 3 ] && ok "two windows asked of tmux (3 on the server)" \
                                || bad "expected 3 tmux windows, found $(windows "$SOCK1")"
wait_wm '^bash$' "asked-for window took the air under its bare name"
shot flow2-windows.png

echo "-- 2c: a remote channel still takes the keyboard --"
type_line "tmux -S $SOCK1 rename-window typed"
wait_wm '^typed$' "typing reached the remote pane, and tmux renamed the window"

echo "-- 3: rule 4, the page outlives its windows --"
WINS=$(tmux -S "$SOCK1" list-windows -F '#{window_id}')
COUNT=$(echo "$WINS" | wc -l)
KILLED=0
for wid in $WINS; do
    tmux -S "$SOCK1" kill-window -t "$wid" 2>/dev/null
    KILLED=$((KILLED + 1))
    sleep 2
    if [ "$KILLED" -lt "$COUNT" ]; then
        [ "$(clients "$SOCK1")" -eq 1 ] && ok "window $KILLED/$COUNT killed, client still attached" \
                                        || bad "client gone after window kill $KILLED/$COUNT"
        wm_name | grep -qE '^slot' && bad "air fell off the session page early [$(wm_name)]" \
                                   || ok "page persists [$(wm_name)]"
    fi
done
# The last kill killed the session itself: tmux says %exit, the page
# collapses, the anchor lands home relit, and the shell's next prompt
# repaints the slot's own title over the model's.
wait_wm '^slot2 ' "landed home on the held slot, title repainted by the prompt"
[ "$(clients "$SOCK1")" -eq 0 ] && ok "no client left on sock1" || bad "client survived session death"
sleep 1
shot flow3-relit.png

echo "-- 4a: the anchor's input law, and Enter as its one live key --"
tmux -S "$SOCK1" new-session -d -s one
type_line "tmux -S $SOCK1 -CC attach -t one"
wait_wm "$ANCHOR_RE" "re-attached"
sleep 2; shot flow4-anchor-still.png
sleep 2; shot flow4-anchor-still2.png
FLOOR=$(glass_rmse flow4-anchor-still.png flow4-anchor-still2.png)

xdotool type --clearmodifiers --delay 25 "the anchor swallows this line"
sleep 2; shot flow4-anchor-typed.png
TYPED=$(glass_rmse flow4-anchor-still2.png flow4-anchor-typed.png)
le "$TYPED" "$(awk -v f="$FLOOR" 'BEGIN { print f * 2 + 0.0002 }')" \
    && ok "typed keys never reached the glass (rmse $TYPED, still-floor $FLOOR)" \
    || bad "the glass moved under typing (rmse $TYPED, still-floor $FLOOR)"
[ "$(clients "$SOCK1")" -eq 1 ] && ok "typed keys never reached the wire either" \
                                || bad "the client went away under plain typing"

echo "-- 4b: paste is inert on the anchor --"
# An empty line is the one thing tmux's control mode acts on, so a paste of
# one is the paste whose arrival could not go unnoticed.
printf '\n' | xclip -selection clipboard
xdotool key --clearmodifiers ctrl+shift+v; sleep 2
[ "$(clients "$SOCK1")" -eq 1 ] && ok "paste on the anchor did nothing" \
                                || bad "a pasted empty line reached the wire"

echo "-- 4c: the frozen glass can still be copied off --"
# Down the first lines of the held screen and across. The emulation extends
# a selection on motion, so the drag is walked rather than warped, and it
# starts on a row that carries text: the band above the first line belongs to
# the bezel, and a drag begun there selects nothing.
xdotool mousemove --window "$WID" 330 90; sleep 0.3
xdotool mousedown 1; sleep 0.3
xdotool mousemove --window "$WID" 700 130; sleep 0.2
xdotool mousemove --window "$WID" 1330 210; sleep 0.4
xdotool mouseup 1
sleep 1
xdotool key --clearmodifiers ctrl+shift+c; sleep 1
xclip -o -selection clipboard 2>/dev/null | grep -q "attach -t one" \
    && ok "selection off the frozen screen copied" \
    || bad "copy off the frozen screen came back empty [$(xclip -o -selection clipboard 2>/dev/null | tr '\n' '|')]"
shot flow4-anchor-copied.png

echo "-- 4d: Enter alone detaches --"
xdotool type --clearmodifiers --delay 25 "ls"
xdotool key --clearmodifiers Return
for _ in $(seq 20); do [ "$(clients "$SOCK1")" -eq 0 ] && break; sleep 0.5; done
[ "$(clients "$SOCK1")" -eq 0 ] \
    && ok "the swallowed ls left Enter to detach the client on its own" \
    || bad "client still attached after ls<Enter>"
wait_wm '^slot2 ' "landed home, relit, repainted"

echo "-- 4e: detach by detach-client on the server --"
type_line "tmux -S $SOCK1 -CC attach -t one"
wait_wm "$ANCHOR_RE" "re-attached"
tmux -S "$SOCK1" list-clients -F '#{client_name}' \
    | while read -r c; do tmux -S "$SOCK1" detach-client -t "$c"; done
for _ in $(seq 20); do [ "$(clients "$SOCK1")" -eq 0 ] && break; sleep 0.5; done
[ "$(clients "$SOCK1")" -eq 0 ] && ok "server-side detach took" || bad "client still attached"
wait_wm '^slot2 ' "landed home, relit, repainted"

echo "-- 5: gateway death --"
type_line "tmux -S $SOCK1 -CC attach -t one"
wait_wm "$ANCHOR_RE" "re-attached"
CPID=$(tmux -S "$SOCK1" list-clients -F '#{client_pid}' | head -1)
kill -9 "$CPID"
wait_wm '^slot2 ' "collapse on gateway death, landed home"
kill -0 "$APP_PID" 2>/dev/null && ok "appliance survived the death" || bad "appliance crashed"
shot flow5-death.png
grep -q "protocol lost" "$OUTDIR/phase1.log" && ok "client logged the lost protocol" \
                                             || bad "no protocol-lost line in the log"
shutdown_app

echo "== phase 2: RobCo Blue, two sockets at once =="
launch "RobCo Blue" phase2.log
SOCK1="$SCRATCH/tmp/sock1"
SOCK2="$SCRATCH/tmp/sock2"
tmux -S "$SOCK1" new-session -d -s one
tmux -S "$SOCK2" new-session -d -s two

wait_wm '^home ' "channel 1 up"
xdotool key --clearmodifiers ctrl+shift+t; sleep 1
xdotool key --clearmodifiers alt+1; sleep 1; retitle 1; wait_wm '^slot1 ' "slot 1 retitled"
type_line "tmux -S $SOCK1 -CC attach -t one"
wait_wm "$ANCHOR_RE" "page A raised"
xdotool key --clearmodifiers alt+Prior; sleep 1
xdotool key --clearmodifiers alt+2
wait_wm '^home ' "home channel 2 selected from the viewed page"
type_line "tmux -S $SOCK2 -CC attach -t two"
wait_wm "$ANCHOR_RE" "page B raised"
[ "$(clients "$SOCK1")" -eq 1 ] && [ "$(clients "$SOCK2")" -eq 1 ] \
    && ok "both sockets hold a client" || bad "a client is missing"
sleep 1
shot flow6-two-pages.png

xdotool key --clearmodifiers ctrl+shift+t; sleep 2
[ "$(windows "$SOCK2")" -eq 2 ] && [ "$(windows "$SOCK1")" -eq 1 ] \
    && ok "Ctrl+Shift+T on page B hit socket 2 alone" \
    || bad "window counts sock1=$(windows "$SOCK1") sock2=$(windows "$SOCK2")"

xdotool key --clearmodifiers alt+Prior; sleep 1
xdotool key --clearmodifiers alt+1
wait_wm "$ANCHOR_RE" "page A's anchor selected"
xdotool key --clearmodifiers ctrl+shift+t; sleep 2
[ "$(windows "$SOCK1")" -eq 2 ] && [ "$(windows "$SOCK2")" -eq 2 ] \
    && ok "Ctrl+Shift+T on page A hit socket 1 alone" \
    || bad "window counts sock1=$(windows "$SOCK1") sock2=$(windows "$SOCK2")"

# Air is on page A's asked-for window; detach page A from its anchor.
xdotool key --clearmodifiers alt+1
wait_wm "$ANCHOR_RE" "back on page A's anchor"
xdotool key --clearmodifiers Return
for _ in $(seq 20); do [ "$(clients "$SOCK1")" -eq 0 ] && break; sleep 0.5; done
[ "$(clients "$SOCK1")" -eq 0 ] && [ "$(clients "$SOCK2")" -eq 1 ] \
    && ok "detach hit page A's gateway alone" \
    || bad "clients sock1=$(clients "$SOCK1") sock2=$(clients "$SOCK2")"
wait_wm '^slot1 ' "landed home on page A's held slot"
sleep 1
shot flow6-after-detach.png
shutdown_app

echo
[ "$FAIL" -eq 0 ] && echo "ALL FLOW CHECKS PASSED" || echo "FLOW CHECKS FAILED"
exit "$FAIL"
