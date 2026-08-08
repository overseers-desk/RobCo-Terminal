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

// One blue row's furniture. The window bezel is a slice of the mock's own
// raised rim: top edge catching the light, under-shadow below, worn chassis
// around it baked in the margin, interior carved to alpha for the live
// lamps. Two source rows alternate so neighbours never repeat exactly. The
// numeral stays live text, stamped the mock's way: dark ink struck into the
// metal with a faint lit edge below the strokes.
Item {
    id: furniture

    property color plastic: "#4f4737"
    property string numeralText: ""
    property rect displayRect: Qt.rect(0, 0, 0, 0)
    property bool open: false
    property bool current: false

    // Bound from the shell's own Metrics.columnGap by the row.
    property int numeralGap: 20

    readonly property color numeralInk: "#0d0b08"
    readonly property color numeralEdge: "#4a4132"
    readonly property color panelDark: "#090d0d"

    Item {
        id: numeral

        x: 0
        width: furniture.displayRect.x - furniture.numeralGap
        height: stamped.implicitHeight
        anchors.verticalCenter: parent.verticalCenter

        // Metal catching light along the stroke's lower edge, ink above it.
        Text {
            y: 1
            width: numeral.width
            horizontalAlignment: Text.AlignRight
            font.family: "Liberation Sans"
            font.bold: false
            font.pixelSize: 36
            text: furniture.numeralText
            color: furniture.numeralEdge
        }
        Text {
            id: stamped
            width: numeral.width
            horizontalAlignment: Text.AlignRight
            font.family: "Liberation Sans"
            font.bold: false
            font.pixelSize: 36
            text: furniture.numeralText
            color: furniture.numeralInk
        }
    }

    // The dark floor of the punched hole, under the lamps; the judge carves
    // this interior out, the live display lights it.
    Rectangle {
        x: furniture.displayRect.x - 2
        y: 2
        width: furniture.displayRect.width + 4
        height: parent.height - 6
        radius: 2
        color: furniture.panelDark
    }

    // The mock's own rim around the hole, stretched only in its straights.
    BorderImage {
        x: furniture.displayRect.x - 10
        y: -4
        width: furniture.displayRect.width + 20
        height: parent.height + 8
        source: "assets/window" +
                (1 + ((parseInt(furniture.numeralText, 10) || 1) - 1) % 2) +
                ".png"
        border { left: 14; right: 14; top: 12; bottom: 12 }
        horizontalTileMode: BorderImage.Repeat
        verticalTileMode: BorderImage.Stretch
    }
}
