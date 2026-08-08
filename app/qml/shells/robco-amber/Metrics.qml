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

// The amber appliance's fixed furniture: a raised plate carrying windows at
// the mock's 45px pitch, cut close at the sides the way a real panel is
// punched - numerals and windows run to the plate's edges with only a sliver
// of unbroken metal keeping the plate whole, and the corner screws overlap
// the content columns rather than owning lanes of their own.
QtObject {
    readonly property int bankPadding: 3
    // The plate's headroom above row 1: screws and blank metal.
    readonly property int topPadding: 61
    readonly property int bottomPadding: 8
    // A sliver of plate past the window bezels' right lip, enough that the
    // punch does not break the plate edge, before the CRT bezel takes over.
    readonly property int rightPadding: 14
    // Window outer bezels nearly touch on the mock: 43px of bezel, 2px of air.
    readonly property int rowSpacing: 2
    readonly property int columnGap: 24
    // Just the struck digits' own width: the column starts a sliver off the
    // plate edge and holds no blank lane of its own.
    readonly property int numeralWidth: 38
    // The strip (18px) sits in a 43px window: the mock's outer bezel height,
    // held by minRowHeight so the pitch lands at 45 regardless of padding sums.
    readonly property int stripPadding: 12
    readonly property int minRowHeight: 43
    // Dark panel above and below the strip inside the window's punched hole
    // (the hole stands 35px in the 43px bezel, the strip 18): the mock keeps
    // that glass unlit, so the lamps' throw is swallowed across it and lands
    // on the machined lip.
    readonly property int panelPadY: 8
    // Glow profile: no selector lane, but shellMetrics always exposes trackWidth.
    readonly property int trackWidth: 14

    // The chassis and frame are one casting poured from the metal shader
    // pair; both read this same light and metal color rather than keeping
    // their own copies to drift apart.
    readonly property vector2d castingLightDir: Qt.vector2d(0.8, -0.6)
    readonly property color castingColor: "#16130f"
}
