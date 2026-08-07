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

import "utils.js" as Utils

// The moulding the channel bank is set into: the same plastic the frame shader
// lights, drawn beyond the frame's own rectangle. Its lighting field and its
// grain lattice are the frame's, continued outwards, so the bank column and the
// CRT frame read as one piece rather than a panel butted against one.
//
// It occupies only the ground the bank stands on, never a sheet behind the
// screen: a see-through profile has to look through the tube onto the desktop,
// and any plastic left under the glass would be a second veil over the picture.
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

    // The frame's own measurements, in the units its shader works in: this
    // plastic is placed in that field, not in one of its own.
    property size viewportSize: Qt.size(_fieldWidth, _fieldHeight)

    property size fieldScale: Qt.size(width / _fieldWidth, height / _fieldHeight)

    property size fieldOffset: frameRegion
        ? Qt.size((x - frameRegion.x) / _fieldWidth, (y - frameRegion.y) / _fieldHeight)
        : Qt.size(0, 0)

    readonly property real _fieldWidth: frameRegion ? Math.max(1, frameRegion.width) : 1
    readonly property real _fieldHeight: frameRegion ? Math.max(1, frameRegion.height) : 1

    // The body takes the tube's translucency, so a see-through profile is one
    // set and not a screen cut into an opaque box.
    opacity: appSettings.windowOpacity * 0.3 + 0.7

    blending: false

    vertexShader: "qrc:/shaders/chassis_plastic.vert.qsb"
    fragmentShader: "qrc:/shaders/chassis_plastic.frag.qsb"

    onStatusChanged: if (log) console.log(log)
}
