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

// The amber appliance's slim bezel: aged gunmetal built by the metal shader
// pair. Low-key room light from high and slightly left, a soft fill from the
// right lighting the right bezel band, dark corners; the chassis margin
// around the bezel is near-black and almost matte, as the mock's is.
ShaderEffect {
    // The instantiation site used to set this; the frame is opaque metal
    // either way, so it travels with the component now.
    blending: false

    Metrics { id: metrics }

    property real screenCurvature: appSettings.screenCurvature * appSettings.screenCurvatureSize * terminalWindow.normalizedScreenScale

    property real frameShininess: appSettings.frameShininess

    property real frameSize: appSettings.frameSize * terminalWindow.normalizedScreenScale

    property real screenRadius: appSettings.screenRadius

    property size viewportSize: Qt.size(width / appSettings.windowScaling, height / appSettings.windowScaling)

    property real ambientLight: appSettings.ambientLight

    // The fill from the right is what models the bezel bands, so the shader's
    // key leans right; the vignette still pools the corners dark.
    property vector2d lightDir: metrics.castingLightDir
    property color bezelColor: "#26211c"
    property color chassisColor: metrics.castingColor
    property color ridgeColor: "#6e5c48"
    // Bezel plate edge insets, in px: left, top, right, bottom. The plate
    // meets the bank flush on the left.
    property vector4d bezelMargins: Qt.vector4d(0, 12, 10, 9)
    property real outerRadius: 60
    property real wellDepth: 9
    property real wellFloor: 0.4
    property real ridgeGain: 0.7
    property real troughGain: 0.35
    property real grainAmount: 0.16
    property real mottleAmount: 0.5
    property real scratchAmount: 0.15
    property real vignetteStrength: 0.42
    property real fillGain: 0.95
    // The shader's face-band law, switched off for this shell. Declared as
    // zeros rather than left to the runtime's fill-in: every uniform the
    // shader names gets a property here, so the UBO is written whole.
    property real faceBandPx: 0
    property real rimDistPx: 0
    property real rimGain: 0

    vertexShader: "qrc:/shaders/frame_metal.vert.qsb"
    fragmentShader: "qrc:/shaders/frame_metal.frag.qsb"

    onStatusChanged: if (log) console.log(log) //Print warning messages
}
