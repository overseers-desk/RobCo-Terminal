# SSH

`robco-term --ssh [user@]host[:port]` opens the first channel on an SSH connection the terminal itself owns, instead of a local shell. The user defaults to `$USER` and the port to 22, the same spelling `ssh` reads. The connection is a bank of its own: its channel sits at slot 1, the bank strip shows it beside the home bank, and closing its last channel hangs the connection up. Connection progress, authentication results and every failure are printed on the channel's own glass, dim and bracketed, in scrollback like everything else that happened there.

A connection that fails before it ever produces a remote byte keeps its channel, wearing the refusal, until you close it (`Ctrl`+`Shift`+`W`): the slot is the only place the reason is readable. A connection that dies after working ends its channel the way a local shell's exit does.

There is no prompt, picker or configuration for SSH yet; that operator surface is designed in the project's issue tracker (#14) and arrives separately. Until it does, the command line is the whole way in, and the sections below say exactly what this build will and will not do.

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

## Authentication

The ssh-agent, and nothing else: `SSH_AUTH_SOCK` on Unix, the agent named pipe or Pageant on Windows. Identities are tried in the order the agent lists them, silently; in the common case you never see an authentication message at all. Every failure names its remedy on the glass: an unset `SSH_AUTH_SOCK` says to run `ssh-add`, and a refused identity list says what the server would have accepted. Password and keyboard-interactive authentication need typed input before a shell exists, which is the operator surface's territory; key files and passphrases likewise.

## `~/.ssh/config` is not read

Not any of it, deliberately. Honouring `HostName` while ignoring `ProxyJump` connects, confidently, to the wrong place; `HostKeyAlias` silently changes which `known_hosts` entry applies. The file is worth reading only whole, and it enters when explicit key files do.
