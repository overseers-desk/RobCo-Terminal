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

// The switchboard's bezel: a slim band of the same near-neutral gunmetal as
// the chassis, dropping through a shallow dark well to a big round-cornered
// glass. The mock cuts this plate close: a few pixels of lit face along its
// own moulded edge, then the well's dark wall, with the rim light standing
// on the wall around the opening.
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

    property vector2d lightDir: metrics.castingLightDir
    property color bezelColor: "#20242b"
    property color chassisColor: metrics.castingColor
    property color ridgeColor: "#737a83"
    // Bezel plate edge insets, in px: left, top, right, bottom. The plate
    // meets the bank flush on the left and runs near the window edge all
    // round, as the mock's glass does.
    property vector4d bezelMargins: Qt.vector4d(0, 6, 10, 10)
    property real outerRadius: 26
    property real wellDepth: 30
    property real wellFloor: 0.18
    property real ridgeGain: 0.4
    property real troughGain: 0.0
    // The mock's plate is lit only in a band along its own moulding; deeper
    // in, the face is the well's dark wall however far the glass still is,
    // with the bright rim standing on the wall around the opening.
    property real faceBandPx: 10
    property real rimDistPx: 12
    property real rimGain: 1.3
    property real grainAmount: 0.35
    property real mottleAmount: 0.8
    property real scratchAmount: 0.45
    property real vignetteStrength: 0.4
    property real fillGain: 0.35

    vertexShader: "qrc:/shaders/frame_metal.vert.qsb"
    fragmentShader: "qrc:/shaders/frame_metal.frag.qsb"

    onStatusChanged: if (log) console.log(log) //Print warning messages
}
