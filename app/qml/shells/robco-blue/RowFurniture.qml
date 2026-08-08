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
// strike catching a sliver of light on the lower edge, and a window punched
// straight into the chassis: a tight dark cut ring around the recessed
// panel, and a bright bevel line on the chassis just under the hole, where
// the light catches the far wall of the punch.
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
    readonly property color rimLight: "#b7a283"
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
            font.pixelSize: 33
            font.bold: true
            font.letterSpacing: -1
            text: furniture.numeralText
            color: furniture.numeralEdge
            opacity: 0.5
        }
        Text {
            id: stamped
            width: numeral.width
            horizontalAlignment: Text.AlignRight
            font.family: stampFace.name
            font.pixelSize: 33
            font.bold: true
            font.letterSpacing: -1
            text: furniture.numeralText
            color: furniture.numeralInk
        }
    }

    // The cut ring of the punch: two pixels of shadowed edge around the
    // panel, darkest under the top lip where the room cannot reach.
    Rectangle {
        id: ring

        x: furniture.displayRect.x - 3
        y: 0
        width: furniture.displayRect.width + 6
        height: parent.height
        radius: 3
        antialiasing: true

        gradient: Gradient {
            GradientStop { position: 0.00; color: furniture.rimDark }
            GradientStop { position: 0.55; color: Qt.darker(furniture.plastic, 2.2) }
            GradientStop { position: 1.00; color: Qt.darker(furniture.plastic, 1.4) }
        }
    }

    // The recessed panel: dark under the top lip, its far wall catching
    // light along the bottom.
    Rectangle {
        id: panel

        x: furniture.displayRect.x - 1
        y: 2
        width: furniture.displayRect.width + 2
        height: parent.height - 4
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

    // The bright bevel line on the chassis just under the hole.
    Rectangle {
        x: furniture.displayRect.x - 2
        y: parent.height + 1
        width: furniture.displayRect.width + 4
        height: 2
        radius: 1
        color: furniture.rimLight
        opacity: 0.9
    }
}
