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

// The blue appliance's carrier: a full-height proud metal rail with a deep
// dark slot milled down it, a hinged bracket bolted over its head with three
// screws, and the carriage riding the slot beside the channel on screen.
// Measured off the mock: rail x 29..70 (this item's width), slot 18..27
// within it, slot running 4px below the item's top to 11px above its foot,
// bracket [18,78]-[118,152] in bank coordinates. It reads the panel and
// reports nothing back; there is no mouse area.
Item {
    id: track

    property color plastic: "#4f4737"
    // Where the carriage belongs, as a centre in this item's coordinates.
    property real targetY: height / 2

    readonly property color railMetal: "#231e16"
    readonly property color grooveDark: "#030202"
    readonly property color bracketLight: "#8e8a6e"
    readonly property color screwGlint: "#e3dfd2"

    readonly property int clampHeight: 26

    implicitWidth: 41

    // The rail body: a proud strip, lit down its left edge.
    Rectangle {
        anchors.fill: parent
        radius: 4
        antialiasing: true

        gradient: Gradient {
            orientation: Gradient.Horizontal
            GradientStop { position: 0.0; color: Qt.lighter(track.railMetal, 1.9) }
            GradientStop { position: 0.12; color: track.railMetal }
            GradientStop { position: 0.85; color: track.railMetal }
            GradientStop { position: 1.0; color: Qt.darker(track.railMetal, 1.8) }
        }
    }

    // The slot: near-black, with a lit endcap at its foot on the mock.
    Rectangle {
        id: groove

        x: 18
        width: 9
        anchors {
            top: parent.top
            bottom: parent.bottom
            topMargin: 4
            bottomMargin: 11
        }
        radius: 4
        antialiasing: true

        gradient: Gradient {
            orientation: Gradient.Horizontal
            GradientStop { position: 0.0; color: track.grooveDark }
            GradientStop { position: 0.7; color: Qt.lighter(track.grooveDark, 3.0) }
            GradientStop { position: 1.0; color: Qt.lighter(track.grooveDark, 6.0) }
        }

        Rectangle {
            anchors {
                left: parent.left
                right: parent.right
                bottom: parent.bottom
            }
            height: 2
            color: Qt.lighter(track.railMetal, 2.4)
        }
    }

    // The carriage riding the slot, wider than the slot as a bolted shoe is.
    Rectangle {
        id: clamp

        x: groove.x - 6
        y: Math.round(track.targetY - height / 2)
        width: groove.width + 12
        height: track.clampHeight
        radius: 3
        antialiasing: true

        Behavior on y {
            NumberAnimation { duration: 150; easing.type: Easing.OutCubic }
        }

        gradient: Gradient {
            GradientStop { position: 0.0; color: track.bracketLight }
            GradientStop { position: 0.35; color: Qt.lighter(track.railMetal, 1.6) }
            GradientStop { position: 1.0; color: Qt.darker(track.railMetal, 1.6) }
        }
    }

    // A bolt head with its glint on the upper left, as the mock lights them.
    component RailScrew: Rectangle {
        width: 22
        height: 22
        radius: width / 2
        antialiasing: true

        gradient: Gradient {
            GradientStop { position: 0.0; color: track.screwGlint }
            GradientStop { position: 0.4; color: track.bracketLight }
            GradientStop { position: 1.0; color: Qt.darker(track.railMetal, 1.4) }
        }
    }

    // The hinge bracket over the rail's head: a proud tab reaching from left
    // of the rail toward the numeral column, three screws holding it.
    // Bank coordinates [18,78]-[118,152]; the rail starts at bank x 29, y 29.
    Item {
        id: bracket

        x: -11
        y: 49
        width: 100
        height: 74

        Rectangle {
            anchors.fill: parent
            radius: 6
            antialiasing: true

            gradient: Gradient {
                GradientStop { position: 0.0; color: track.bracketLight }
                GradientStop { position: 0.25; color: Qt.lighter(track.railMetal, 1.8) }
                GradientStop { position: 1.0; color: track.railMetal }
            }
        }

        // Screw centres in bank coordinates: (45,103) (102,115) (47,134).
        RailScrew { x: 45 - 18 - 11; y: 103 - 78 - 11 }
        RailScrew { x: 102 - 18 - 11; y: 115 - 78 - 11 }
        RailScrew { x: 47 - 18 - 11; y: 134 - 78 - 11 }
    }
}
