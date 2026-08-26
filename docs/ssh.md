# SSH

`robco-term --ssh [user@]host[:port]` opens the first channel on an SSH connection the terminal itself owns, instead of a local shell. The user defaults to `$USER` and the port to 22, the same spelling `ssh` reads. The connection is a bank of its own: its channel sits at slot 1, the bank strip shows it beside the home bank, and closing its last channel hangs the connection up. Connection progress, authentication results and every failure are printed on the channel's own glass, dim and bracketed, in scrollback like everything else that happened there.

A connection that fails before it ever produces a remote byte keeps its channel, wearing the refusal, until you close it (`Ctrl`+`Shift`+`W`): the slot is the only place the reason is readable. A connection that dies after working ends its channel the way a local shell's exit does.

Where a session starts is configuration: the `[ssh]` table in `config.toml` holds the pre-configured servers as `[[ssh.host]]` rows and names which of them is the default (`docs/config.md`). The settings window is to list those rows as its SSH tab's radios, localhost first, the checked radio being the default; the terminal reads the table at launch, so a change applies to the next session started. `--ssh` on the command line outranks the configured default for its own invocation, and `Shift`+`Alt`+`T` raises a picker on the glass: the same rows and localhost, a digit opening one channel there, the default untouched. The sections below say exactly what this build will and will not do.

## Host keys

Trust is read from `~/.ssh/known_hosts`, then `/etc/ssh/ssh_known_hosts`. The files are read-only to this program: nothing here ever writes an entry, which is the no-trust-on-first-use decision expressed as a property of the code. Three outcomes:

* **Match**: a recorded key for the host matches the presented one, and the connection proceeds.
* **Unknown**: no key is recorded. Refused, printing the `SHA256:` fingerprint and the `ssh` command that records it. Accepting a first key is a trust decision, and a build with no prompt would be making it for you; `ssh` itself is the prompt, on every box this program runs on.
* **Mismatch**: a key is recorded and the presented one differs. Refused, always, with both fingerprints and the offending `file:line`; no flag, key or environment variable overrides it.

The host-key algorithms recorded for a host lead the negotiation order, so a host known only under `ssh-rsa` connects rather than reading as unknown when the server also holds a newer key.

Ceilings of the reader, each of which costs a spurious refusal and never a false accept, because the policy refuses by default:

* Glob and negation host patterns (`*.example.com`, `!host`) are compared literally.
* `@cert-authority` lines never match, and a server presenting a certificate host key is refused outright.
* A tab-separated or double-spaced line does not parse.
* `@revoked` does better than the ceiling: any presented key a revocation line names is refused, whatever host it is filed under.

## tmux over the connection

Type `tmux -CC` on an SSH channel and the attachment arrives exactly as it does locally: the channel becomes the gateway, the session's windows fill a bank of their own, and a detach brings the channel home to its slot on the connection's bank. Because the connection multiplexes, each SSH channel can carry its own attachment: several remote tmux sessions over one wire, one `tmux -CC` each. Sessions on the remote server are not discovered automatically; the discovery mechanism is local by construction.

## Authentication

Public keys, from three sources, in intent order: the key a `[[ssh.host]]` row names (`key`, with `~/` expanded to home), the ssh-agent (`SSH_AUTH_SOCK` on Unix; the OpenSSH Authentication Agent's named pipe, then Pageant, on Windows), and, only when no key is named, the default files `ssh` itself would try: `~/.ssh/id_ed25519`, `id_ecdsa`, `id_rsa`. Agent identities are tried in the order the agent lists them, silently; in the common case you never see an authentication message at all. A named or default key that is encrypted is announced and skipped: its passphrase is typed input before a shell exists, which is the operator surface's territory (#14), and password and keyboard-interactive authentication wait on the same surface. A lost cause closes with a line naming what the server would have accepted.

## `~/.ssh/config` is not read

Not any of it, deliberately. Honouring `HostName` while ignoring `ProxyJump` connects, confidently, to the wrong place; `HostKeyAlias` silently changes which `known_hosts` entry applies. The file is worth reading only whole, and reading it whole is open work beside the prompts (#14).
