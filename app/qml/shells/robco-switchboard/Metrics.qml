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

// The switchboard's fixed furniture, measured off Deep-Blue.png (1448x1086,
// an untracked working file at the repo root): fifteen rows of heavy toggle
// switches in recessed wells, each row a stamped numeral, the switch, and a
// framed label well, punched into near-neutral dark gunmetal. No selector
// rail anywhere on the panel: the thrown switch is the whole mark, so this
// file declares no track and the profile asks for the switch law.
//
// Row layout decision: the switch well sits between the numeral and the
// label plate, and the bank knows nothing of it. The whole lane left of the
// label, numeral and well together, is folded into numeralWidth, and
// RowFurniture owns everything left of displayRect; the well's own rect
// below is furniture's business, stated in lane coordinates.
QtObject {
    // The row plates' left edge stands at x 8; the sliver of chassis before
    // it is the bank's whole left shoulder.
    readonly property int bankPadding: 8
    // Row 1's label well, bevel ring included, starts at y 34 (its glass
    // interior at 36, read down column x 300 of the mock).
    readonly property int topPadding: 34
    // The pager rail's bottom bevel line sits at y 1065; 21px of chassis
    // follow to the mock's foot.
    readonly property int bottomPadding: 21
    // Label plate's outer right bevel at x 387-389, a seam, and chassis to
    // the screen well's trough at x 402: 29px past the glass interior's
    // right edge at 373.
    readonly property int rightPadding: 29
    // Well tops run 36, 99, 162 .. 908 down the mock: (908 - 36) / 14 rows
    // gives a 62.29px pitch, and the artwork itself drifts a couple of
    // pixels either way across the column. Pitch less the 48px row leaves
    // the fraction to the air between rows.
    readonly property real rowSpacing: 14.29
    // From the numeral plate's right seam at x 178 to the label glass at
    // x 196; the label's raised frame moulding lives in this gap and
    // RowFurniture draws it around displayRect.
    readonly property int columnGap: 18
    // The folded lane: plate x 8..178 holds the stamped numeral (strokes
    // x 27..53) and the whole switch well.
    readonly property int numeralWidth: 170
    // Label well outer 34..82 against its 36..80 glass: the bevel ring is
    // 2px of inset all round.
    readonly property int stripPadding: 2
    readonly property int minRowHeight: 48

    // The switch well, in lane coordinates (lane x 8 is well 0). Mock rect
    // x 72..174 by 52 high, standing 2px proud of the row band top and
    // bottom, centered on the row; corners rounded about 10px. Row 2 wears
    // the flat lever (cap over the well's left, pivot screw right); row 1
    // wears the thrown one, lever swung left off a lit cyan right side,
    // sampled #b6f9fe at its hottest edge (x 159, y 58).
    readonly property int switchWellX: 64
    readonly property int switchWellWidth: 102
    readonly property int switchWellHeight: 52
    readonly property int switchWellRadius: 10
    // Stamped numerals: light warm paint on the dark plate, sampled
    // #cfc4ba at the brightest stroke, glyphs about 30px tall, centered
    // near lane x 32.
    readonly property int numeralCenterX: 32

    // The pager rail across the bank's foot, plate y 963..1064 on the mock.
    readonly property int pagerHeight: 104
    // Arrow buttons flank the counter: left one x 28..102, y 974..1056,
    // the right its mirror at x 292..365.
    readonly property int pagerArrowWidth: 74
    readonly property int pagerArrowHeight: 82
    // The framed PAGE plate x 118..277, y 968..1058, "PAGE" engraved on the
    // frame over a recessed counter window, interior x 136..258 by
    // y 1002..1042. The mock's rolls carry near-white painted digits,
    // sampled #c6c6c4.
    readonly property int pageWindowWidth: 122
    readonly property int pageWindowHeight: 40
    readonly property int pageWindowRadius: 6

    // The chassis and frame are one casting poured from the metal shader
    // pair; both read this same light and metal color rather than keeping
    // their own copies to drift apart. The color is the plate face itself,
    // sampled #232830 at (300, 26), the same reading that gave the blue
    // shell its #453c2d. The light stands nearly overhead with a lean to
    // the left: recess bottom lips catch it bright, right inner walls
    // faintly, tops and lefts fall dark.
    readonly property vector2d castingLightDir: Qt.vector2d(-0.4, -0.9)
    readonly property color castingColor: "#232830"
}
