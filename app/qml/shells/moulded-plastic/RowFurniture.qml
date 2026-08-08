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

// The plastic a row is moulded from: the numeral stamped beside the window
// and the dish the window is sunk into. The row keeps the layout and the
// interaction; this paints around the display it is handed the place of.
Item {
    id: furniture

    property color plastic: "#7a7168"
    property string numeralText: ""
    // Where the title display sits, in this item's own coordinates.
    property rect displayRect: Qt.rect(0, 0, 0, 0)
    property bool open: false
    property bool current: false

    // The breathing room between the numeral column and the window, this
    // shell's own measure (Metrics.columnGap).
    readonly property int numeralGap: 10

    Item {
        id: numeral

        x: 0
        width: furniture.displayRect.x - furniture.numeralGap
        height: engraving.implicitHeight
        anchors.verticalCenter: parent.verticalCenter

        Text {
            width: numeral.width
            y: 1
            horizontalAlignment: Text.AlignRight
            font.pixelSize: 22
            font.bold: true
            font.letterSpacing: 0.5
            text: furniture.numeralText
            color: Qt.lighter(furniture.plastic, 1.9)
        }
        Text {
            id: engraving
            width: numeral.width
            horizontalAlignment: Text.AlignRight
            font.pixelSize: 22
            font.bold: true
            font.letterSpacing: 0.5
            text: furniture.numeralText
            color: Qt.darker(furniture.plastic, 2.6)
        }
    }

    // The dish the window is sunk into: shading alone, no outline. It tints
    // whatever plastic lies behind rather than laying down a sheet of its own,
    // so a translucent profile still sees through the bank.
    Rectangle {
        id: dish

        x: recess.x - 4
        y: recess.y - 4
        width: recess.width + 8
        height: recess.height + 8
        radius: 6
        antialiasing: true

        gradient: Gradient {
            GradientStop { position: 0.0; color: Qt.rgba(0, 0, 0, 0.34) }
            GradientStop { position: 0.62; color: Qt.rgba(0, 0, 0, 0.10) }
            GradientStop { position: 1.0; color: Qt.rgba(1, 1, 1, 0.07) }
        }
    }

    // The window body. Darker under its top lip, where the moulding shades it.
    Rectangle {
        id: recess

        x: furniture.displayRect.x - 3
        y: furniture.displayRect.y - 3
        width: furniture.displayRect.width + 6
        height: furniture.displayRect.height + 6
        radius: 3
        antialiasing: true

        gradient: Gradient {
            GradientStop { position: 0.0; color: Qt.darker(furniture.plastic, 5.0) }
            GradientStop { position: 0.45; color: Qt.darker(furniture.plastic, 3.6) }
            GradientStop { position: 1.0; color: Qt.darker(furniture.plastic, 2.9) }
        }
    }
}
