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

// One amber row's furniture. The window bezel is a slice of the mock's own
// machined rim: bright lower lip, dark upper cut, its shadow margin baked,
// its interior carved to alpha for the live lamps. Three source rows rotate
// by numeral so no two neighbours are identical, as the mock's windows are
// not. The numeral stays live text, embossed the mock's way: a light figure
// in a condensed face over its own dark drop shadow, low and right.
Item {
    id: furniture

    property color plastic: "#241e19"
    property string numeralText: ""
    property rect displayRect: Qt.rect(0, 0, 0, 0)
    property bool open: false
    property bool current: false

    // Bound from the shell's own Metrics.columnGap by the row.
    property int numeralGap: 24

    readonly property color numeralFill: "#a78a72"
    readonly property color numeralShadow: "#14100c"
    readonly property color panelDark: "#0d0700"

    Item {
        id: numeral

        x: 0
        width: furniture.displayRect.x - furniture.numeralGap
        height: engraved.implicitHeight
        anchors.verticalCenter: parent.verticalCenter

        // Embossed: the dark shadow lies under and right of the lit figure.
        Text {
            x: 1
            y: 2
            width: numeral.width
            horizontalAlignment: Text.AlignRight
            font.family: "Liberation Sans Narrow"
            font.bold: false
            font.pixelSize: 33
            text: furniture.numeralText
            color: furniture.numeralShadow
        }
        Text {
            id: engraved
            width: numeral.width
            horizontalAlignment: Text.AlignRight
            font.family: "Liberation Sans Narrow"
            font.bold: false
            font.pixelSize: 33
            text: furniture.numeralText
            color: furniture.numeralFill
        }
    }

    // The dark floor of the punched hole, under the lamps; the judge carves
    // this interior out, the live display lights it.
    Rectangle {
        x: furniture.displayRect.x - 6
        y: 4
        width: furniture.displayRect.width + 12
        height: parent.height - 8
        radius: 5
        color: furniture.panelDark
    }

    // The mock's own rim around the hole, stretched only in its straights.
    BorderImage {
        x: furniture.displayRect.x - 13
        y: -3
        width: furniture.displayRect.width + 25
        height: parent.height + 6
        source: "assets/window" +
                (1 + ((parseInt(furniture.numeralText, 10) || 1) - 1) % 3) +
                ".png"
        border { left: 16; right: 16; top: 13; bottom: 13 }
        horizontalTileMode: BorderImage.Repeat
        verticalTileMode: BorderImage.Stretch
    }
}
