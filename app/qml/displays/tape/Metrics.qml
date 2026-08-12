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

// The tape display's width-quantisation contract: a well grows and shrinks in
// whole characters, and these are the measures of one. The letters are a
// fixed size the kit carries with it, unlike the LED strip's, which follows
// the profile's chosen lamp font: a punch wheel stamps the one size of
// letter into every tape it ever cuts.
QtObject {
    id: tapeMetrics

    // The stamped letters' size. The switchboard mock's label glass is 177px
    // wide (x 196..373 of Deep-Blue.png, the A2 measurement), and twelve
    // characters, the settings' default count, of Departure Mono at 20px with
    // 12px of blank tape at either end fill it to the pixel.
    readonly property int letterPixelSize: 20
    // The blank the wheel always leaves past the last letter, at each end.
    readonly property int endPad: 12
    // The well the kit cuts for itself when the fixture names no height: the
    // switchboard's own label glass, 44px of it (A2), the well these letters
    // were sized against.
    readonly property int naturalHeight: 44

    // One character's width, unrounded: the seam drag divides pixels by this
    // to find the count nearest the hand. Departure Mono is monospaced, so
    // one advance measures them all. It is measured rather than tabulated,
    // and measured here and now: a TextMetrics cannot stand bare among a
    // QtObject's children, so it is held as a declared property and read
    // synchronously, the way the LED cell's own size is taken
    // (ApplicationSettings.qml).
    property TextMetrics _letterMetrics: TextMetrics {
        font.family: "DepartureMono Nerd Font Mono"
        font.pixelSize: tapeMetrics.letterPixelSize
        text: "M"
    }
    readonly property real unitWidth: _letterMetrics.advanceWidth
    readonly property int minUnits: appSettings.minLedCharacters

    function widthForUnits(n) {
        return Math.round(n * tapeMetrics.unitWidth + 2 * tapeMetrics.endPad)
    }

    // The height pair, a pass-through. The fixture asks what band its punched
    // hole holds and hands the answer straight back for the strip's height;
    // the two are only ever composed that way (ChannelBank.qml). The LED
    // panel answers in lamp rows because its glass has to fill with lamps;
    // tape has nothing to count, so the hole's own height travels through
    // both and the tape lies in exactly the well the shell punched.
    function padCellsForHole(holeHeight) {
        return Math.round(Math.max(0, holeHeight))
    }

    function heightForPadCells(padCells) {
        return padCells
    }
}
