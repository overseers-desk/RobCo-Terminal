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

// The mechanical channel selector: a slot milled down the left edge of the
// bank with a clamp riding it, parked beside the channel on screen. It reads
// the panel and reports nothing back: the channel drives the selector, not the
// other way round, so there is no mouse area and nothing to drag.
//
// Like the rest of the column it is moulded rather than drawn: every edge here
// is a change of shade, never a line.
Item {
    id: track

    property color plastic: "#7a7168"
    // Where the clamp belongs, as a centre in this item's own coordinates.
    // The bank owns the panel's layout and works this out; the track only
    // takes the time to get there.
    property real targetY: height / 2

    // The clamp is the wider part: it sits proud of the slot and overhangs it
    // on both sides, the way a bracket bolted onto a runner does.
    readonly property int clampHeight: 16
    readonly property int grooveWidth: Math.max(5, Math.round(width * 0.4))

    implicitWidth: 14

    // The slot: plastic cut away, dark at the near wall where the light does
    // not reach and lifting towards the far one.
    Rectangle {
        id: groove

        anchors {
            top: parent.top
            bottom: parent.bottom
            horizontalCenter: parent.horizontalCenter
        }
        width: track.grooveWidth
        radius: width / 2
        antialiasing: true

        gradient: Gradient {
            orientation: Gradient.Horizontal
            GradientStop { position: 0.0; color: Qt.darker(track.plastic, 3.6) }
            GradientStop { position: 0.65; color: Qt.darker(track.plastic, 2.8) }
            GradientStop { position: 1.0; color: Qt.darker(track.plastic, 1.9) }
        }
    }

    Item {
        id: clamp

        // The travel of a hand moving the selector, not of a lamp fading up.
        y: Math.round(track.targetY - height / 2)
        width: track.width
        height: track.clampHeight

        Behavior on y {
            NumberAnimation { duration: 150; easing.type: Easing.OutCubic }
        }

        // The shadow the clamp drops into its own slot.
        Rectangle {
            x: 1
            y: 2
            width: parent.width - 1
            height: parent.height
            radius: 3
            antialiasing: true
            opacity: 0.45
            color: Qt.darker(track.plastic, 3.2)
        }

        // The body, moulded: lit along its top face, shaded under the lip.
        Rectangle {
            id: body

            anchors.fill: parent
            radius: 3
            antialiasing: true

            gradient: Gradient {
                GradientStop { position: 0.0; color: Qt.lighter(track.plastic, 1.6) }
                GradientStop { position: 0.4; color: Qt.lighter(track.plastic, 1.15) }
                GradientStop { position: 1.0; color: Qt.darker(track.plastic, 1.7) }
            }
        }

        // The nose that reads the row: a raised ridge on the bank side, caught
        // by the same light as the body's top face.
        Rectangle {
            anchors {
                right: parent.right
                verticalCenter: parent.verticalCenter
            }
            width: 3
            height: parent.height * 0.5
            radius: 1
            antialiasing: true

            gradient: Gradient {
                GradientStop { position: 0.0; color: Qt.lighter(track.plastic, 1.7) }
                GradientStop { position: 1.0; color: Qt.darker(track.plastic, 1.2) }
            }
        }
    }
}
