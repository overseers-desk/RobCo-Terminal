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
// (1448x1086): windows punched straight into the chassis at a 60.8px pitch
// (61 in whole pixels), a full-height selector rail 41px wide on the left,
// dark stamped numerals ending 25px short of the strip, and the recessed
// page selector standing at the bank's foot. How many windows show is the
// window height's business, never a number of this file's.
QtObject {
    // The rail's stand-off from the bank's left edge.
    readonly property int bankPadding: 29
    // Row 1's window top on the mock.
    readonly property int topPadding: 64
    // Chassis under the selector group's bevel line (1022) to the foot.
    readonly property int bottomPadding: 63
    // The window bezels run to 4px of the bank's right edge.
    readonly property int rightPadding: 4
    // 43px of window bezel, 18px of bare chassis between windows: pitch 61.
    readonly property int rowSpacing: 18
    readonly property int columnGap: 25
    readonly property int numeralWidth: 54
    // The strip (18px) sits centred in a 43px window; minRowHeight pins the
    // mock's bezel height, since padding alone lands one short.
    readonly property int stripPadding: 12
    readonly property int minRowHeight: 43
    // The full-height carrier rail, x 29..69 on the mock.
    readonly property int trackWidth: 41
}
