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

// The amber appliance's fixed furniture, measured off the reference mock
// (1448x1086): a raised plate carrying 19 windows at a 45px pitch, numerals
// ending 24px short of the LED strip, and 50px of plate standing between the
// strips and the CRT bezel.
QtObject {
    readonly property int bankPadding: 8
    // The plate's headroom above row 1: screws and blank metal.
    readonly property int topPadding: 61
    readonly property int bottomPadding: 8
    readonly property int rightPadding: 50
    // Window outer bezels nearly touch on the mock: 43px of bezel, 2px of air.
    readonly property int rowSpacing: 2
    readonly property int columnGap: 24
    readonly property int numeralWidth: 70
    // The strip (18px) sits in a 43px window: the mock's outer bezel height,
    // held by minRowHeight so the pitch lands at 45 regardless of padding sums.
    readonly property int stripPadding: 12
    readonly property int minRowHeight: 43
    // Glow profile: no selector lane, but the contract names the measure.
    readonly property int trackWidth: 14
}
