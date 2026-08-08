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

import "../../utils.js" as Utils

// The amber appliance's chassis: the frame's lighting field continued under
// the bank, with the raised bank plate screwed onto it. The field is the
// moulded-plastic law for now; the plate, its bevel and its four corner
// screws are the mock's geometry, painted in the mock's plate palette.
ShaderEffect {
    id: chassis

    // The item the frame shader fills; this plastic continues its field.
    property Item frameRegion

    property color frameColor: Utils.frameBaseColor(
        appSettings.frameColor,
        appSettings.fontColor,
        appSettings.backgroundColor,
        appSettings.ambientLight
    )

    property real screenCurvature: appSettings.screenCurvature * appSettings.screenCurvatureSize * terminalWindow.normalizedScreenScale

    property real frameShininess: appSettings.frameShininess

    property real frameSize: appSettings.frameSize * terminalWindow.normalizedScreenScale

    property real screenRadius: appSettings.screenRadius

    property size viewportSize: Qt.size(_fieldWidth, _fieldHeight)

    property size fieldScale: Qt.size(width / _fieldWidth, height / _fieldHeight)

    property size fieldOffset: frameRegion
        ? Qt.size((x - frameRegion.x) / _fieldWidth, (y - frameRegion.y) / _fieldHeight)
        : Qt.size(0, 0)

    readonly property real _fieldWidth: frameRegion ? Math.max(1, frameRegion.width) : 1
    readonly property real _fieldHeight: frameRegion ? Math.max(1, frameRegion.height) : 1

    opacity: appSettings.windowOpacity * 0.3 + 0.7

    blending: false

    vertexShader: "qrc:/shaders/chassis_plastic.vert.qsb"
    fragmentShader: "qrc:/shaders/chassis_plastic.frag.qsb"

    onStatusChanged: if (log) console.log(log)

    // The plate the bank is punched into, sitting proud of the chassis: base
    // colour from the mock, a lit top bevel, a shaded right edge where it
    // drops back to the chassis. Measured rect: 8..343 x 2..1077 on the mock.
    readonly property color plateBase: "#241e19"
    readonly property color plateHighlight: "#c1a585"
    readonly property color plateShadow: "#1a110a"

    Rectangle {
        id: plate

        anchors {
            fill: parent
            leftMargin: 8
            topMargin: 2
            rightMargin: 0
            bottomMargin: 8
        }
        radius: 6
        antialiasing: true

        gradient: Gradient {
            GradientStop { position: 0.0; color: Qt.lighter(chassis.plateBase, 1.5) }
            GradientStop { position: 0.04; color: chassis.plateBase }
            GradientStop { position: 0.96; color: chassis.plateBase }
            GradientStop { position: 1.0; color: chassis.plateShadow }
        }

        // The top bevel edge that catches the key light.
        Rectangle {
            anchors {
                left: parent.left
                right: parent.right
                top: parent.top
                leftMargin: parent.radius
                rightMargin: parent.radius
            }
            height: 1
            color: chassis.plateHighlight
            opacity: 0.55
        }
    }

    // A slotted screw head: a shaded disc, glint on the upper left where the
    // mock's key light lands.
    component Screw: Rectangle {
        width: 28
        height: 28
        radius: width / 2
        antialiasing: true

        gradient: Gradient {
            GradientStop { position: 0.0; color: chassis.plateHighlight }
            GradientStop { position: 0.35; color: Qt.lighter(chassis.plateBase, 1.7) }
            GradientStop { position: 1.0; color: chassis.plateShadow }
        }

        Rectangle {
            anchors.centerIn: parent
            width: parent.width * 0.7
            height: 2
            rotation: 40
            color: chassis.plateShadow
        }
    }

    // Screw centres from the mock: (32,30) (317,29) top, and 39px above the
    // bottom edge at (32,·) (315,·); the lower pair rides the plate's foot.
    Screw { x: 32 - 14; y: 30 - 14 }
    Screw { x: 317 - 14; y: 29 - 14 }
    Screw { x: 32 - 14; anchors.bottom: parent.bottom; anchors.bottomMargin: 25 }
    Screw { x: 315 - 14; anchors.bottom: parent.bottom; anchors.bottomMargin: 25 }
}
