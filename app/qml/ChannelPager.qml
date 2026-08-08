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

// The rocker at the foot of the bank that walks the preset keys from one page
// of slots to the next, with the page count printed beside it. With a single
// page there is nowhere to go and the whole thing sits dim and dead.
Item {
    id: pager

    property color plastic: "#7a7168"
    property int pageIndex: 0
    property int pageCount: 1
    property int columnGap: 6

    readonly property int buttonWidth: 32
    readonly property int buttonHeight: 22
    readonly property int buttonGap: 4

    // One stroke of the V, from the apex out. The arm is a bar laid along the
    // segment between the apex and the corner, so the pair meets at a point.
    component ChevronFace: Item {
        id: face

        property color tint: "black"
        // -1 raises the apex, 1 drops it.
        property int direction: -1
        property real thickness: 3

        readonly property real angle: Math.atan2(height, width / 2) * 180 / Math.PI
        readonly property real armLength: Math.sqrt(
            (width / 2) * (width / 2) + height * height)

        Rectangle {
            width: face.armLength
            height: face.thickness
            radius: height / 2
            antialiasing: true
            color: face.tint
            x: face.width / 4 - width / 2
            y: face.height / 2 - height / 2
            rotation: face.angle * face.direction
        }
        Rectangle {
            width: face.armLength
            height: face.thickness
            radius: height / 2
            antialiasing: true
            color: face.tint
            x: 3 * face.width / 4 - width / 2
            y: face.height / 2 - height / 2
            rotation: -face.angle * face.direction
        }
    }

    // The arrow moulded into the keycap rather than printed on it: the same
    // plastic struck twice, a shadow a pixel low and a lit face above it.
    component Chevron: Item {
        id: chevron

        property color plastic: "#7a7168"
        property int direction: -1

        implicitWidth: 14
        implicitHeight: 7

        ChevronFace {
            width: chevron.width
            height: chevron.height
            y: 1
            direction: chevron.direction
            tint: Qt.darker(chevron.plastic, 2.6)
        }
        ChevronFace {
            width: chevron.width
            height: chevron.height
            direction: chevron.direction
            tint: Qt.lighter(chevron.plastic, 1.9)
        }
    }

    signal step(int direction)

    enabled: pageCount > 1
    opacity: enabled ? 1.0 : 0.45

    implicitHeight: buttonHeight + 4
    implicitWidth: 2 * buttonWidth + buttonGap + columnGap + counter.width

    ChannelButton {
        id: pageUp

        x: 0
        width: pager.buttonWidth
        height: pager.buttonHeight
        anchors.verticalCenter: parent.verticalCenter
        plastic: pager.plastic
        onClicked: pager.step(-1)

        Chevron {
            anchors.centerIn: parent
            plastic: pager.plastic
            direction: -1
        }
    }

    ChannelButton {
        x: pageUp.width + pager.buttonGap
        width: pager.buttonWidth
        height: pager.buttonHeight
        anchors.verticalCenter: parent.verticalCenter
        plastic: pager.plastic
        onClicked: pager.step(1)

        Chevron {
            anchors.centerIn: parent
            plastic: pager.plastic
            direction: 1
        }
    }

    Item {
        id: counter

        readonly property string label: (pager.pageIndex + 1) + "/" + pager.pageCount

        anchors {
            right: parent.right
            verticalCenter: parent.verticalCenter
        }
        width: engraving.implicitWidth
        height: engraving.implicitHeight

        Text {
            y: 1
            font.pixelSize: 16
            font.bold: true
            text: counter.label
            color: Qt.lighter(pager.plastic, 1.9)
        }
        Text {
            id: engraving

            font.pixelSize: 16
            font.bold: true
            text: counter.label
            color: Qt.darker(pager.plastic, 2.6)
        }
    }
}
