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

// The LED display's width-quantisation contract: a strip grows and shrinks
// in whole characters, and these are the measures of one.
QtObject {
    // One character's width on the panel, unrounded: the seam drag divides
    // by this to find the count nearest the hand.
    readonly property real unitWidth: appSettings.ledCellWidth * appSettings.ledDotPitch
    readonly property int stripHeight: Math.round(
        (appSettings.ledCellHeight + appSettings.ledPadCells) * appSettings.ledDotPitch)
    readonly property int minUnits: appSettings.minLedCharacters

    // Rounded, not truncated: at a fractional dot pitch a floored strip would
    // leave the row half a pixel short of the window it has to hold. n counts
    // visible characters; the bank strip's end-pad cells ride on top, part of
    // the fixed furniture rather than the count.
    function widthForUnits(n) {
        return Math.round(appSettings.ledCellWidth
                          * (n + 2 * appSettings.ledSidePadCells)
                          * appSettings.ledDotPitch)
    }
}
