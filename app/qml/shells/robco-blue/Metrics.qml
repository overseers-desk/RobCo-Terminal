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
import QtQuick

// The blue appliance's fixed furniture: windows punched straight into the
// chassis at the mock's 60.8px pitch (how many fit is the window's business,
// not a number of this file's), a full-height carrier rail 41px wide drawn
// by the chassis on the left, dark stamped numerals hard against the rail,
// and the three-piece page selector always standing at the bank's foot.
// Horizontally the panel is cut close: rail, numerals and windows stand a
// sliver apart, just enough unbroken metal that the chassis reads as one
// piece.
QtObject {
    // The bank's left shoulder: 29px of chassis before the rail (the
    // bracket's ground), the rail's 41, and a sliver between rail and
    // numerals. The profile marks the current channel by glow, so no live
    // lane is reserved on top of this; the rail is the chassis's own
    // furniture.
    readonly property int bankPadding: 76
    // Row 1's window top sits at the mock's y 64.
    readonly property int topPadding: 64
    // The selector's bevel line ends at y 1023; 63px of chassis follow.
    readonly property int bottomPadding: 63
    // A sliver of chassis past the punch's cut ring before the bezel plate.
    readonly property int rightPadding: 8
    // 43px of window bezel; the mock's pitch is 60.8, so the air between
    // windows carries the fraction.
    readonly property real rowSpacing: 17.8
    // The air between the numerals and the punch: a sliver, as stamped.
    readonly property int columnGap: 8
    // Just the stamped digits' own width; no blank lane rides with them.
    readonly property int numeralWidth: 36
    // The strip fills the recessed panel (39px) inside the window;
    // minRowHeight pins the window's outer height to the mock's 43, and the
    // 2px above and below are the panel's own inset into the ring.
    readonly property int stripPadding: 2
    readonly property int minRowHeight: 43
    // The full-height carrier rail, x 29..69 on the mock. Its width stands
    // folded into bankPadding above (the glow profile reserves no lane);
    // shellMetrics always exposes trackWidth for a profile that asks.
    readonly property int trackWidth: 41

    // The chassis and frame are one casting poured from the metal shader
    // pair; both read this same light and metal color rather than keeping
    // their own copies to drift apart.
    readonly property vector2d castingLightDir: Qt.vector2d(-0.55, -0.85)
    readonly property color castingColor: "#453c2d"
}
