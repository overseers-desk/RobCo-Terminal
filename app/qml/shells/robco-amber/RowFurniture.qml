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

// One amber row's furniture: a struck condensed-mono numeral, worn low, and a
// machined window bezel sunk into the plate. The bezel's inner cut is dark
// along the top where the cutter shadowed it and bright along the lower and
// right lip where the light catches the machined face; each window came off
// the mill a hair different, so a hash of the numeral sways the lip.
Item {
    id: furniture

    property color plastic: "#241e19"
    property string numeralText: ""
    property rect displayRect: Qt.rect(0, 0, 0, 0)
    property bool open: false
    property bool current: false

    // Bound from the shell's own Metrics.columnGap by the row.
    property int numeralGap: 24

    readonly property color numeralFill: "#9c8168"
    readonly property color numeralShadow: "#0f0d0c"
    readonly property color panelDark: "#0d0700"
    readonly property color lipLight: "#7a6448"
    readonly property color cutDark: "#060402"

    // No two windows left the mill identical: a cheap hash off the numeral
    // sways each window's lip brightness and rim tone a few percent.
    readonly property real jitter: {
        var h = 0
        for (var i = 0; i < numeralText.length; i++)
            h = (h * 31 + numeralText.charCodeAt(i)) % 997
        return h / 997
    }

    FontLoader {
        id: struckFace
        source: "qrc:/fonts/iosevka/IosevkaTermNerdFontMono-Regular.ttf"
    }

    Item {
        id: numeral

        x: 0
        width: furniture.displayRect.x - furniture.numeralGap
        height: engraved.implicitHeight
        anchors.verticalCenter: parent.verticalCenter

        // Struck into the plate: the lit figure throws its dark shadow low
        // and right, and the whole strike sits worn, not printed.
        Text {
            x: 2
            y: 2
            width: numeral.width
            horizontalAlignment: Text.AlignRight
            font.family: struckFace.name
            font.pixelSize: 34
            font.letterSpacing: -1
            text: furniture.numeralText
            color: furniture.numeralShadow
            opacity: 0.9
        }
        Text {
            id: engraved
            width: numeral.width
            horizontalAlignment: Text.AlignRight
            font.family: struckFace.name
            font.pixelSize: 34
            font.letterSpacing: -1
            text: furniture.numeralText
            color: furniture.numeralFill
            opacity: 0.88 + 0.08 * furniture.jitter
        }
    }

    // The raised outer rim around the window: a thin moulding standing just
    // proud of the plate, its top edge lit, dropping to a dark cut inside.
    Rectangle {
        id: rim

        x: furniture.displayRect.x - 10
        y: 0
        width: furniture.displayRect.width + 19
        height: parent.height
        radius: 8
        antialiasing: true

        gradient: Gradient {
            GradientStop { position: 0.00; color: Qt.lighter(furniture.plastic, 1.9 + 0.2 * furniture.jitter) }
            GradientStop { position: 0.06; color: Qt.lighter(furniture.plastic, 1.15) }
            GradientStop { position: 0.30; color: Qt.darker(furniture.plastic, 1.25) }
            GradientStop { position: 0.90; color: Qt.darker(furniture.plastic, 1.7) }
            GradientStop { position: 1.00; color: Qt.lighter(furniture.plastic, 1.5) }
        }

        // The machined inner bevel: dark cut on the upper lip...
        Rectangle {
            x: 4
            y: 3
            width: parent.width - 8
            height: 2
            radius: 1
            color: furniture.cutDark
            opacity: 0.85
        }
        // ...bright machined face along the lower lip...
        Rectangle {
            x: 5
            y: parent.height - 4
            width: parent.width - 10
            height: 2
            radius: 1
            color: furniture.lipLight
            opacity: 0.7 + 0.3 * furniture.jitter
        }
        // ...and a fainter catch down the right lip.
        Rectangle {
            x: parent.width - 4
            y: 6
            width: 2
            height: parent.height - 12
            radius: 1
            color: furniture.lipLight
            opacity: 0.3 + 0.2 * furniture.jitter
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
            GradientStop { position: 0.0; color: Qt.darker(furniture.panelDark, 1.6) }
            GradientStop { position: 0.45; color: furniture.panelDark }
            GradientStop { position: 0.93; color: Qt.lighter(furniture.panelDark, 1.6) }
            GradientStop { position: 1.0; color: Qt.lighter(furniture.panelDark, 3.2) }
        }
    }
}
