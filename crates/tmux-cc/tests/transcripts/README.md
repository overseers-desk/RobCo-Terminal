# Recorded tmux control-mode transcripts

Every `.txt` here is the exact byte stream a `tmux -CC attach` client
received on its PTY, DCS envelope (`ESC P 1000 p` ... `ESC \`)
included. Every `.cmds` beside one is the wire lines that client sent,
in order: the pairing queue, replayable.

Recorded from **tmux 3.5a** by `cargo run -p robco-tmux-cc --example
record`, which is the only thing that should ever write them. A
protocol claim is only true of a version, so a transcript that changes
under a re-record is the news, not the noise.

| file | what it holds |
|---|---|
| `01-fresh-session` | Attach to a one-window session, the client's bootstrap (host, list-panes, list-windows), a client resize, a client-side detach. The attach burst's blocks and the replies to those commands are both here, which is where the `%begin` flags field gives itself away. |
| `02-second-window` | `new-window` through the codec: `%window-add`, `%session-window-changed`, `%layout-change`, and the re-listing that finds the new window's pane, which `%window-add` does not name. |
| `03-rename` | Renames from the server, one awkward name at a time: two consecutive spaces, a backslash, a tab and a BEL, valid UTF-8, and bytes that are not UTF-8. The evidence behind `escape::unvis` -- a name is C-escaped by `vis(3)`, not octal-escaped like a payload -- and behind `%session-renamed` carrying a session id the manual does not mention. |
| `04-output-octal` | Two windows producing output, one of them writing all 256 byte values through a raw tty. The escaping set is measured here: `0x00..=0x1f` and `\`, and nothing else. |
| `05-kill-window` | A background window killed from the server and the current one killed through the codec. Both arrive as `%unlinked-window-close`; `%window-close` never appears. |
| `06-detach` | The server throwing this client off with `detach-client -t`. |
| `07a-before-the-kill`, `07b-reattach` | The killed-client re-attach shape. The first client builds a three-window session and is SIGKILLed mid-protocol (no `%exit`, no `ST`). The second attaches onto that session: its burst announces **no** window, so a client that learns windows from notifications alone comes up empty. |
| `08-error-and-extended-output` | A command tmux refuses (`%error`), and the `%extended-output` form that the `pause-after` client flag turns `%output` into. |
| `09-quoting` | tmux's command lexer, probed: unquoted `#{...}` swallowed as a comment, the same quoted, `${VAR}` expanded inside double quotes, `\"` and `\\` unescaped, and single quotes not stopping format expansion. |
| `10-notification-zoo` | As many notification names as one session can be made to emit: `%sessions-changed`, `%unlinked-window-add`, `%session-renamed`, the paste-buffer pair, `%message` (inside a reply block, which the manual says cannot happen), `%layout-change`, `%window-pane-changed`, `%pane-mode-changed`, `%client-session-changed`, and `%exit` under a dying server. |
