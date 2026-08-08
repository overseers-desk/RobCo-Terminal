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
// (1448x1086): sixteen windows punched straight into the chassis at a 61px
// pitch, a full-height selector rail 41px wide on the left, dark stamped
// numerals ending 20px short of the strip, 33px of chassis right of the
// windows before the CRT bezel.
QtObject {
    // The rail's stand-off from the bank's left edge and the groove's from
    // the top and bottom.
    readonly property int bankPadding: 29
    readonly property int topPadding: 95
    // Row 16 runs to 30px of the mock's foot; less bottom air than the rail.
    readonly property int bottomPadding: 10
    readonly property int rightPadding: 33
    // 46px of window bezel, 15px of bare chassis between windows.
    readonly property int rowSpacing: 15
    readonly property int columnGap: 20
    readonly property int numeralWidth: 63
    // The strip (18px) sits centred in a 46px window: 14px of panel above
    // and below, and the pitch lands at exactly 61.
    readonly property int stripPadding: 14
    readonly property int minRowHeight: 46
    // The full-height carrier rail, x 29..70 on the mock.
    readonly property int trackWidth: 41
}
