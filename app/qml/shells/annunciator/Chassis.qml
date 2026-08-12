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

import "../common"

// The amber appliance's chassis: the frame's near-black metal continued under
// the bank, with the raised bank plate screwed over it. The plate is aged
// patinated gunmetal from the plate shader: blotchy mottling, grain, worn
// bright edges, a lit top bevel and four slotted screws on their bosses.
ShaderEffect {
    id: chassis

    Metrics { id: metrics }

    // The item the frame shader fills; this metal continues its field.
    property Item frameRegion

    property size viewportSize: Qt.size(_fieldWidth, _fieldHeight)

    property size fieldScale: Qt.size(width / _fieldWidth, height / _fieldHeight)

    property size fieldOffset: frameRegion
        ? Qt.size((x - frameRegion.x) / _fieldWidth, (y - frameRegion.y) / _fieldHeight)
        : Qt.size(0, 0)

    readonly property real _fieldWidth: frameRegion ? Math.max(1, frameRegion.width) : 1
    readonly property real _fieldHeight: frameRegion ? Math.max(1, frameRegion.height) : 1

    // The frame's own chassis law, continued leftwards: one casting, one
    // shared light and metal color, read off this shell's Metrics.
    property vector2d lightDir: metrics.castingLightDir
    property color chassisColor: metrics.castingColor
    property real grainAmount: 0.16
    property real mottleAmount: 0.4
    property real scratchAmount: 0.08
    property real vignetteStrength: 0.42

    opacity: appSettings.windowOpacity * 0.3 + 0.7

    blending: false

    vertexShader: "qrc:/shaders/chassis_metal.vert.qsb"
    fragmentShader: "qrc:/shaders/chassis_metal.frag.qsb"

    onStatusChanged: if (log) console.log(log)

    // The plate the bank is punched into, sitting proud of the chassis.
    // Measured rect: 8..343 x 2..1077 on the mock. Key light high and
    // slightly left: top bevel brightest, right edge dropping to shadow,
    // corners pooling dark.
    ShaderEffect {
        id: plate

        anchors {
            fill: parent
            leftMargin: 8
            topMargin: 2
            rightMargin: 0
            bottomMargin: 8
        }

        property size sizePx: Qt.size(width, height)
        property vector2d lightDir: Qt.vector2d(-0.22, -0.98)
        property color baseColor: "#2b241c"
        property color highlightColor: "#c1a585"
        property color shadowColor: "#0e0905"
        property real cornerRadius: 6
        property real bevelPx: 2.5
        property real grainAmount: 0.3
        property real mottleAmount: 1.0
        property real scratchAmount: 0.5
        property real vignetteStrength: 0.42
        property real wearAmount: 0.7
        property real seamGain: 1.0
        property real seed: 0.17

        vertexShader: "qrc:/shaders/plate_metal.vert.qsb"
        fragmentShader: "qrc:/shaders/plate_metal.frag.qsb"

        onStatusChanged: if (log) console.log(log)
    }

    // Screw centres from the mock: (32,30) (317,29) top, and 39px above the
    // bottom edge at (32,·) (315,·); the lower pair rides the plate's foot.
    // The right-hand pair keeps its distance from the plate's right edge
    // (26px centre inset on the mock), so a narrower window keeps its screws.
    ScrewHead { x: 32 - 14; y: 30 - 14; slotAngle: 24 }
    ScrewHead { anchors.right: parent.right; anchors.rightMargin: 12; y: 29 - 14; slotAngle: -49 }
    ScrewHead { x: 32 - 14; anchors.bottom: parent.bottom; anchors.bottomMargin: 25; slotAngle: 78 }
    ScrewHead { anchors.right: parent.right; anchors.rightMargin: 14; anchors.bottom: parent.bottom; anchors.bottomMargin: 25; slotAngle: -11 }
}
