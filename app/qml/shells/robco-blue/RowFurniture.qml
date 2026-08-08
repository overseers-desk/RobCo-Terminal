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

// One blue row's furniture: a numeral stamped dark into the metal, its
// strike catching a sliver of light on the lower edge, and a raised window
// bezel: top edge lit by the room, an under-shadow thrown on the chassis
// below, the punched panel inside recessed with its bright line on the
// bottom lip where light reaches the far wall.
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
    readonly property color numeralEdge: "#4a4030"
    readonly property color panelDark: "#090d0d"
    readonly property color rimLight: "#6e6350"
    readonly property color rimDark: "#12100b"

    FontLoader {
        id: stampFace
        source: "qrc:/fonts/iosevka/IosevkaTermNerdFontMono-Regular.ttf"
    }

    Item {
        id: numeral

        x: 0
        width: furniture.displayRect.x - furniture.numeralGap
        height: stamped.implicitHeight
        anchors.verticalCenter: parent.verticalCenter

        // Metal catching light along the stroke's lower edge, ink above it.
        Text {
            y: 2
            x: 1
            width: numeral.width
            horizontalAlignment: Text.AlignRight
            font.family: stampFace.name
            font.pixelSize: 37
            font.bold: true
            font.letterSpacing: -1
            text: furniture.numeralText
            color: furniture.numeralEdge
            opacity: 0.8
        }
        Text {
            id: stamped
            width: numeral.width
            horizontalAlignment: Text.AlignRight
            font.family: stampFace.name
            font.pixelSize: 37
            font.bold: true
            font.letterSpacing: -1
            text: furniture.numeralText
            color: furniture.numeralInk
        }
    }

    // The under-shadow the raised bezel throws on the chassis below it.
    Rectangle {
        x: furniture.displayRect.x - 4
        y: parent.height - 1
        width: furniture.displayRect.width + 10
        height: 3
        radius: 2
        color: "#000000"
        opacity: 0.4
    }

    // The raised rim standing proud of the chassis around the hole: lit
    // along its top edge, falling dark down its sides.
    Rectangle {
        id: rim

        x: furniture.displayRect.x - 6
        y: 0
        width: furniture.displayRect.width + 12
        height: parent.height
        radius: 3
        antialiasing: true

        gradient: Gradient {
            GradientStop { position: 0.00; color: furniture.rimLight }
            GradientStop { position: 0.10; color: Qt.darker(furniture.plastic, 1.15) }
            GradientStop { position: 0.60; color: Qt.darker(furniture.plastic, 1.55) }
            GradientStop { position: 1.00; color: furniture.rimDark }
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
        clip: true

        gradient: Gradient {
            GradientStop { position: 0.0; color: Qt.darker(furniture.panelDark, 1.7) }
            GradientStop { position: 0.5; color: furniture.panelDark }
            GradientStop { position: 0.92; color: Qt.lighter(furniture.panelDark, 1.5) }
            GradientStop { position: 1.0; color: Qt.lighter(furniture.panelDark, 2.6) }
        }
    }
}
