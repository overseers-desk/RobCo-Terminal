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

// The blue appliance's live carriage, kept for a profile that asks for the
// pointer law: a bolted steel shoe with a specular ridge across its cap and
// a hard shadow under its foot. The rail it once carried is the chassis's
// own furniture now (the built-in profile marks the channel by glow), so
// this item holds nothing but the shoe, centred in whatever lane the bank
// reserves. It reads the panel and reports nothing back; there is no mouse
// area.
Item {
    id: track

    property color plastic: "#4f4737"
    // Where the carriage belongs, as a centre in this item's coordinates.
    property real targetY: height / 2

    readonly property color railMetal: "#2b2418"
    readonly property color bracketLight: "#b2a47d"
    readonly property color screwGlint: "#e3dfd2"

    readonly property int clampHeight: 26

    implicitWidth: 41

    // The carriage: a bolted steel shoe, specular ridge across its cap, hard
    // shadow under its foot.
    Item {
        id: clamp

        width: 21
        x: Math.round((track.width - width) / 2)
        y: Math.round(track.targetY - height / 2)
        height: track.clampHeight

        Behavior on y {
            NumberAnimation { duration: 150; easing.type: Easing.OutCubic }
        }

        // Shadow under the shoe.
        Rectangle {
            x: 1
            y: 2
            width: parent.width
            height: parent.height
            radius: 3
            color: "#000000"
            opacity: 0.5
        }

        Rectangle {
            anchors.fill: parent
            radius: 3
            antialiasing: true
            gradient: Gradient {
                GradientStop { position: 0.0; color: track.bracketLight }
                GradientStop { position: 0.18; color: Qt.lighter(track.railMetal, 2.0) }
                GradientStop { position: 0.55; color: Qt.lighter(track.railMetal, 1.3) }
                GradientStop { position: 1.0; color: "#0a0704" }
            }

            // The specular ridge machined across the cap.
            Rectangle {
                x: 2
                y: 3
                width: parent.width - 4
                height: 1
                color: track.screwGlint
                opacity: 0.9
            }
            Rectangle {
                x: 2
                y: 4
                width: parent.width - 4
                height: 1
                color: "#000000"
                opacity: 0.5
            }
            // Side bevels.
            Rectangle { x: 0; y: 2; width: 1; height: parent.height - 4; color: track.bracketLight; opacity: 0.5 }
            Rectangle { x: parent.width - 1; y: 2; width: 1; height: parent.height - 4; color: "#000000"; opacity: 0.6 }
        }
    }
}
