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

// The amber pager's key, as the mock builds it: a metal cap of four machined
// ridges lit from above, each ridge a specular line over its own shadow, the
// cap's front face dropping to near-black, a worn bevel down both sides and
// a shadow the key throws onto the plate. Rectangle and MouseArea only: no
// Control, no stolen focus.
Item {
    id: presetButton

    property color plastic: "#241e19"
    property bool pressed: false

    signal clicked()

    readonly property bool hovered: pointer.containsMouse

    readonly property color ridgeHighlight: "#f7e8c4"
    readonly property color ridgeBase: "#c7a381"
    readonly property color ridgeShadow: "#5e4630"
    readonly property color frontFace: "#1c1411"

    implicitWidth: 56
    implicitHeight: 40

    // The key's shadow on the plate.
    Rectangle {
        x: 2
        y: 3
        width: cap.width
        height: cap.height
        radius: 4
        color: "#000000"
        opacity: 0.45
    }

    Rectangle {
        id: cap

        anchors.fill: parent
        radius: 3
        antialiasing: true
        color: presetButton.frontFace

        transform: Translate { y: presetButton.pressed ? 2 : 0 }
        opacity: presetButton.pressed ? 0.9 : (presetButton.hovered ? 1.0 : 0.97)

        // The ridged top: four specular ridges, each with its shadow gap.
        Column {
            id: ridges

            x: 2
            y: 2
            width: parent.width - 4
            spacing: 0

            Repeater {
                model: 4
                Item {
                    width: ridges.width
                    height: 5
                    Rectangle {
                        width: parent.width
                        height: 2
                        radius: 1
                        color: presetButton.ridgeHighlight
                    }
                    Rectangle {
                        y: 2
                        width: parent.width
                        height: 2
                        color: presetButton.ridgeBase
                    }
                    Rectangle {
                        y: 4
                        width: parent.width
                        height: 1
                        color: presetButton.ridgeShadow
                    }
                }
            }
        }

        // The cap's rolled front edge under the last ridge.
        Rectangle {
            x: 2
            y: ridges.y + ridges.height
            width: parent.width - 4
            height: 2
            color: presetButton.ridgeShadow
        }

        // Worn bevels down the cap's sides.
        Rectangle {
            x: 0
            y: 2
            width: 1
            height: parent.height - 6
            color: presetButton.ridgeBase
            opacity: 0.5
        }
        Rectangle {
            x: parent.width - 1
            y: 2
            width: 1
            height: parent.height - 6
            color: "#000000"
            opacity: 0.6
        }
        // A dim catch along the front face's foot.
        Rectangle {
            x: 3
            y: parent.height - 2
            width: parent.width - 6
            height: 1
            color: presetButton.ridgeBase
            opacity: 0.25
        }
    }

    MouseArea {
        id: pointer
        anchors.fill: parent
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton
        onClicked: presetButton.clicked()
    }
}
