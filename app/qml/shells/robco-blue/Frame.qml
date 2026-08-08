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
import QtQuick 2.0

// The blue appliance's deep barrel-mouthed bezel: aged dark bronze gunmetal
// from the metal shader pair. Light from the upper left, a bright ridge
// along the bezel plate's outer moulding, a deep shaded well dropping to the
// glass, heavy grain and stains, corners pooling to black.
ShaderEffect {
    // The instantiation site used to set this; the frame is opaque metal
    // either way, so it travels with the component now.
    blending: false

    property real screenCurvature: appSettings.screenCurvature * appSettings.screenCurvatureSize * terminalWindow.normalizedScreenScale

    property real frameShininess: appSettings.frameShininess

    property real frameSize: appSettings.frameSize * terminalWindow.normalizedScreenScale

    property real screenRadius: appSettings.screenRadius

    property size viewportSize: Qt.size(width / appSettings.windowScaling, height / appSettings.windowScaling)

    property real ambientLight: appSettings.ambientLight

    property vector2d lightDir: Qt.vector2d(-0.55, -0.85)
    property color bezelColor: "#2e2820"
    property color chassisColor: "#353024"
    property color ridgeColor: "#8c8068"
    // Bezel plate edge insets, in px: left, top, right, bottom.
    property vector4d bezelMargins: Qt.vector4d(6, 11, 16, 8)
    property real outerRadius: 45
    property real wellDepth: 45
    property real wellFloor: 0.16
    property real ridgeGain: 1.15
    property real troughGain: 1.3
    property real grainAmount: 0.35
    property real mottleAmount: 0.85
    property real scratchAmount: 0.55
    property real vignetteStrength: 0.5
    property real fillGain: 0.35

    vertexShader: "qrc:/shaders/frame_metal.vert.qsb"
    fragmentShader: "qrc:/shaders/frame_metal.frag.qsb"

    onStatusChanged: if (log) console.log(log) //Print warning messages
}
