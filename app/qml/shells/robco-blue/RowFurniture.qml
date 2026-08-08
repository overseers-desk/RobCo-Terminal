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

// One blue row's furniture: a numeral stamped dark into the metal (the
// inverse of the amber emboss: the figure is the dark stroke, the metal
// showing between strokes the light) and a hard-edged window with a thin
// metal rim (radius 3) around a punched panel (radius 2). Measured off the
// mock: rim the full 46px row, panel 2px under the top lip and 5 above the
// bottom. Palette colours; skeleton gradients until the paint pass.
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
    readonly property color numeralEdge: "#382f24"
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
            font.pixelSize: 36
            font.bold: true
            text: furniture.numeralText
            color: furniture.numeralEdge
        }
        Text {
            id: stamped
            width: numeral.width
            horizontalAlignment: Text.AlignRight
            font.pixelSize: 36
            font.bold: true
            text: furniture.numeralText
            color: furniture.numeralInk
        }
    }

    // The thin rim standing just proud of the chassis around the hole.
    Rectangle {
        id: rim

        x: furniture.displayRect.x - 6
        y: 0
        width: furniture.displayRect.width + 12
        height: parent.height
        radius: 3
        antialiasing: true

        gradient: Gradient {
            GradientStop { position: 0.0; color: Qt.darker(furniture.plastic, 1.8) }
            GradientStop { position: 0.9; color: Qt.darker(furniture.plastic, 2.6) }
            GradientStop { position: 1.0; color: Qt.lighter(furniture.plastic, 1.3) }
        }
    }

    // The punched panel, recessed: bright bevel line on the bottom lip.
    Rectangle {
        id: panel

        x: furniture.displayRect.x - 1
        y: 2
        width: furniture.displayRect.width + 2
        height: parent.height - 7
        radius: 2
        antialiasing: true

        gradient: Gradient {
            GradientStop { position: 0.0; color: Qt.darker(furniture.panelDark, 1.6) }
            GradientStop { position: 0.5; color: furniture.panelDark }
            GradientStop { position: 1.0; color: Qt.lighter(furniture.panelDark, 1.8) }
        }
    }
}
