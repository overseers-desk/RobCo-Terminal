# Keys

Every key the terminal takes for itself. Everything not listed here goes to
the program on the other end, which is why the list is worth knowing: a key
that is bound here is a key your editor never sees.

The chord modifier is `Alt`, and `Cmd` on macOS.

## Channels

| Key | What it does |
|---|---|
| `Alt`+digit | Bring that channel to the screen |
| `Alt`+`Shift`+digit | Move the channel on screen to that slot |
| `Ctrl`+`Shift`+`T` | New channel |
| `Ctrl`+`Shift`+`W` | Close this channel |
| `Ctrl`+`PageUp` / `PageDown` | Previous / next channel |
| `Ctrl`+`Shift`+`Left` / `Right` | Move the channel on screen one slot |
| `Alt`+`PageUp` / `PageDown` | Page the bank without switching channel |

The digit `0` on its own means channel 10. Digits are typed one at a time,
and the chord fires the moment no longer slot number could still match what
you have typed. Let go of the chord modifier to fire it early. So on a bank
of nine, `Alt` then `3` switches immediately, while on a bank of thirty it
waits to see whether a second digit is coming. The digit chords name the
numerals engraved on the bank, and the pager keys step what the bank shows,
so both need the bank on show. The rest of the table stands whether the
chassis is drawn or not.

`Ctrl`+`Shift`+`Left` / `Right` swap the channel on screen with its
neighbour, and take an empty slot outright. The first slot and the last are
walls: a step past either leaves the bank as it stands.

## Windows

| Key | What it does |
|---|---|
| `Ctrl`+`Shift`+`N` | New window |
| `Ctrl`+`Shift`+`Q` | Close this window |
| `F11` | Fullscreen, with or without modifiers |

`F11` ignores its modifiers, so the same key works for a hand used to
Konsole's `Ctrl`+`Shift`+`F11` and one used to a bare `F11`.

## Copying and pasting

| Key | What it does |
|---|---|
| `Ctrl`+`Shift`+`C` | Copy the selection |
| `Ctrl`+`Shift`+`V` | Paste |

Selecting text with the mouse copies it already. `Ctrl`+`Shift`+`C` copies
it again, which is what you want after something else has taken the
clipboard. Middle-click pastes, as `Ctrl`+`Shift`+`V` does; holding `Ctrl`
while you middle-click forces bracketed paste onto a program that did not
ask for it. Hold `Ctrl`+`Alt` while you drag to select a rectangle instead
of a run of lines.

## Scrollback

| Key | What it does |
|---|---|
| `Shift`+`Up` / `Down` | Scroll a line |
| `Shift`+`PageUp` / `PageDown` | Scroll a screen |

These move the view only on the primary screen. A full-screen program such
as an editor or a pager is drawn on the alternate screen, which has no
history behind it, so there the same keys are the program's own.

## On an attachment

A channel attached to a tmux server through control mode is a picture to
read and copy from rather than a surface to type at, because that channel's
pty carries the protocol itself. Every key is dropped there, except `Enter`,
which detaches the client and brings the channel home.

## Not bound

Font size is a config key rather than a keystroke; see
[config.md](config.md). Nothing here binds `Ctrl`+letter, so `Ctrl`+`C`,
`Ctrl`+`D` and the readline chords reach the program untouched.
