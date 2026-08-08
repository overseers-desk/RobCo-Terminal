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

// One amber row's furniture: an embossed light numeral throwing a dark drop
// shadow low and right, and a bezelled LED window sunk into the plate. The
// window's outer rim stands the full row height (43px on the mock, radius 8);
// the inner panel is the punched hole (5px under the top lip, 3 above the
// bottom, radius 5). Colours are the mock's palette; the modelling is the
// skeleton's gradient law until the paint pass.
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
    readonly property color numeralShadow: "#121010"
    readonly property color panelDark: "#0d0700"

    Item {
        id: numeral

        x: 0
        width: furniture.displayRect.x - furniture.numeralGap
        height: engraved.implicitHeight
        anchors.verticalCenter: parent.verticalCenter

        // Embossed, so struck the other way round from the moulded shell:
        // the dark shadow lies under and right of the lit figure.
        Text {
            x: 1
            y: 1
            width: numeral.width
            horizontalAlignment: Text.AlignRight
            font.pixelSize: 33
            font.bold: true
            text: furniture.numeralText
            color: furniture.numeralShadow
        }
        Text {
            id: engraved
            width: numeral.width
            horizontalAlignment: Text.AlignRight
            font.pixelSize: 33
            font.bold: true
            text: furniture.numeralText
            color: furniture.numeralFill
        }
    }

    // The raised outer rim around the window: full row height, lit along the
    // top edge, falling to shadow at the bottom lip.
    Rectangle {
        id: rim

        x: furniture.displayRect.x - 10
        y: 0
        width: furniture.displayRect.width + 19
        height: parent.height
        radius: 8
        antialiasing: true

        gradient: Gradient {
            GradientStop { position: 0.0; color: Qt.lighter(furniture.plastic, 1.8) }
            GradientStop { position: 0.15; color: furniture.plastic }
            GradientStop { position: 0.85; color: Qt.darker(furniture.plastic, 1.6) }
            GradientStop { position: 1.0; color: Qt.lighter(furniture.plastic, 1.4) }
        }
    }

    // The punched hole the lamps live in: recessed, so its bright bevel line
    // sits on the bottom lip where light catches the far wall.
    Rectangle {
        id: panel

        x: furniture.displayRect.x - 4
        y: 5
        width: furniture.displayRect.width + 8
        height: parent.height - 8
        radius: 5
        antialiasing: true

        gradient: Gradient {
            GradientStop { position: 0.0; color: Qt.darker(furniture.panelDark, 1.5) }
            GradientStop { position: 0.5; color: furniture.panelDark }
            GradientStop { position: 1.0; color: Qt.lighter(furniture.panelDark, 2.2) }
        }
    }
}
