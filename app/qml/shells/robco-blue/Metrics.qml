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
// number of this file's), a full-height selector rail 41px wide on the left,
// dark stamped numerals ending 27px short of the strip, and the three-piece
// page selector always standing at the bank's foot.
QtObject {
    // The rail's stand-off from the bank's left edge and the groove's from
    // the top and bottom.
    readonly property int bankPadding: 29
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
    // The full-height carrier rail, x 29..69 on the mock.
    readonly property int trackWidth: 41
}
