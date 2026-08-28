# Invariants

What must hold across every change here. Breaking one is a redesign, and a redesign is the owner's decision: propose the change and wait, never route around.

- Only configuration may break the immersion, on an explicit bail-out gesture: a right-click or the platform's application menu. What users normally believe they are doing within the terminal stays inside it, such as, if a new channel is configured to use SSH, the typing of the password, the acceptance of a key, and an SSH connection error. The terminal itself can print to stderr or log like normal terminal emulators do.
- Every change compiles on every platform the project targets. Platform-specific code sits behind a gate whose other arms exist, as per-platform implementations or a deliberate visible no-op; nothing platform-bound sits unconditionally on a shared path.
- A capability built for one platform's users doesn't cost another platform a capability it has. e.g. having russh doesn't prevent users from running ssh-agent or their system's own ssh.
- A subsystem left unconfigured and uninvoked is indistinguishable from one not built: configuration empty, chords untouched, its startup, screen and behaviour match a build without it. First case: SSH, where an empty `[ssh]` table and no SSH chord mean a purely local terminal.
- The settings window wears the platform's stock widget look, never a theme of its own: the appliance's look is the user's choice at runtime, so no palette chosen at build time can match it; we do not even know what colour the cabinet is.
