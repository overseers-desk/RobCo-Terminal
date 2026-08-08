/*******************************************************************************
* Copyright (c) 2013-2021 "Filippo Scognamiglio"
* https://github.com/Swordfish90/cool-retro-term
*
* This file is part of cool-retro-term.
*
* cool-retro-term is free software: you can redistribute it and/or modify
* it under the terms of the GNU General Public License as published by
* the Free Software Foundation, either version 3 of the License, or
* (at your option) any later version.
*
* This program is distributed in the hope that it will be useful,
* but WITHOUT ANY WARRANTY; without even the implied warranty of
* MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
* GNU General Public License for more details.
*
* You should have received a copy of the GNU General Public License
* along with this program.  If not, see <http://www.gnu.org/licenses/>.
*******************************************************************************/
import QtQml

// The blue appliance's fixed furniture, measured off the reference mock
// Metalic-Blue.png (1448x1086): windows punched straight into the chassis at
// the mock's 60.8px pitch (how many fit is the window's business, not a
// number of this file's), a full-height carrier rail 41px wide drawn by the
// chassis on the left, dark stamped numerals ending 27px short of the strip,
// and the three-piece page selector always standing at the bank's foot.
QtObject {
    // The bank's left shoulder: 29px of chassis before the rail, the rail's
    // 41, and the 27 between rail and numerals. The profile marks the
    // current channel by glow, so no live lane is reserved on top of this;
    // the rail is the chassis's own furniture.
    readonly property int bankPadding: 97
    // Row 1's window top sits at the mock's y 64.
    readonly property int topPadding: 64
    // The selector's bevel line ends at y 1023; 63px of chassis follow.
    readonly property int bottomPadding: 63
    // Strip right edge 378, bezel plate edge 408, plate inset 6: 24 of bank.
    readonly property int rightPadding: 24
    // 43px of window bezel; the mock's pitch is 60.8, so the air between
    // windows carries the fraction.
    readonly property real rowSpacing: 17.8
    readonly property int columnGap: 27
    readonly property int numeralWidth: 50
    // The strip (18px) sits centred in the window; minRowHeight pins the
    // window's outer height to the mock's 43.
    readonly property int stripPadding: 12
    readonly property int minRowHeight: 43
    // Dark panel above and below the strip inside the punched hole (the
    // recessed panel stands 39px in the 43px window, the strip 18): that
    // glass stays unlit on the mock, so the lamps' throw is swallowed
    // across it and lands on the cut ring.
    readonly property int panelPadY: 10
    // The full-height carrier rail, x 29..69 on the mock. Its width stands
    // folded into bankPadding above (the glow profile reserves no lane);
    // shellMetrics always exposes trackWidth for a profile that asks.
    readonly property int trackWidth: 41
}
