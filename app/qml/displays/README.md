# The display kits

One directory a kit, named by a profile's `channelDisplay` and listed in
`ApplicationSettings.knownDisplays`; `appSettings.displayUrl("Display")`
resolves the file. Both files below are required, and their qrc entries with
them, or the Loader fails in silence.

`Display.qml` renders one channel's title. Its whole surface, and who sets
each part of it:

| property | set by |
| --- | --- |
| `text` | the bank row (ChannelRow.qml), the page counter, the settings preview |
| `powered` | the bank row: false is an empty slot |
| `bright` | the bank row: the channel on screen, under the glow indicator law |
| `padCellsY` | the bank: how tall a hole the shell punched |
| `characters` | its own default is the setting; a fixture with a window of its own (robco-blue/Pager.qml) names its width |
| `padCellsLeft`, `padCellsRight` | the same fixtures, to fill their window edge to edge |
| `spillStrength` | the same fixtures, to quiet the light thrown on the plate |

A kit that has no use for one of them declares it anyway and ignores it: the
five consumers bind blind.

`Metrics.qml` (a QtObject) is the width contract, read by ChannelBank.qml and
by the seam drag in TerminalWindow.qml: `unitWidth` (one character, unrounded,
the drag's divisor), `minUnits`, `widthForUnits(n)`, and the height pair
`padCellsForHole(holeHeight)` → `heightForPadCells(padCells)`, which the bank
only ever composes in that order.
