# Invariants

What must hold across every change here. Breaking one is a redesign, and a redesign is the owner's decision: propose the change and wait, never route around.

- The user meets nothing but the simulated glass unless they ask to leave it: every runtime prompt, picker, progress line, error and secret is text on the terminal's own grid, typed through its own keyboard path. A native surface (the settings window or any other) appears only on an explicit bail-out gesture, a right-click or a chord bound to that purpose; it may hold configuration, and never holds a password, a passphrase, or a connect-time question.
- Every change compiles on every platform the project targets. Platform-specific code sits behind a gate whose other arms exist, as per-platform implementations or a deliberate visible no-op; nothing platform-bound sits unconditionally on a shared path.
- A capability built for one platform's users never costs another platform a capability it has. Each platform gets its native equivalent; uniformity is never bought by removal.
- A subsystem left unconfigured and uninvoked is indistinguishable from one not built: configuration empty, chords untouched, its startup, screen and behaviour match a build without it. First case: SSH, where an empty `[ssh]` table and no SSH chord mean a purely local terminal.
