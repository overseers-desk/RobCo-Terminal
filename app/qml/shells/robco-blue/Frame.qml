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

import "../../utils.js" as Utils

// The blue appliance's deep barrel-mouthed bezel. Geometry skeleton: the
// moulded-plastic lighting law stands in for the scratched gunmetal until the
// paint pass; the well's depth comes from the profile's frameSize and the
// barrel sweep from its screenRadius and curvature.
ShaderEffect {
    // The instantiation site used to set this; the frame is opaque plastic
    // either way, so it travels with the component now.
    blending: false

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

    property size viewportSize: Qt.size(width / appSettings.windowScaling, height / appSettings.windowScaling)

    property real ambientLight: appSettings.ambientLight

    vertexShader: "qrc:/shaders/terminal_frame.vert.qsb"
    fragmentShader: "qrc:/shaders/terminal_frame.frag.qsb"

    onStatusChanged: if (log) console.log(log) //Print warning messages
}
