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

// The switchboard's chassis: near-neutral dark gunmetal, the ground the row
// plates and the pager rail are riveted onto. Most of it hides behind that
// furniture; what shows is the seams between the plates and the slivers at
// the column's edges, so the metal is kept even and fine-grained, with none
// of the blue appliance's heavy staining. No rail and no bracket: the thrown
// switch is the whole mark on this panel, and nothing rides the chassis.
//
// It occupies only the ground the bank stands on, never a sheet behind the
// screen: a see-through profile has to look through the tube onto the desktop,
// and any metal left under the glass would be a second veil over the picture.
ShaderEffect {
    id: chassis

    // The item the frame shader fills; this metal continues its field.
    property Item frameRegion

    // The frame's own measurements, in the units its shader works in: this
    // metal is placed in that field, not in one of its own.
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
    property real grainAmount: 0.45
    property real mottleAmount: 0.7
    property real scratchAmount: 0.45
    property real vignetteStrength: 0.22

    // The body takes the tube's translucency, so a see-through profile is one
    // set and not a screen cut into an opaque box.
    opacity: appSettings.windowOpacity * 0.3 + 0.7

    blending: false

    vertexShader: "qrc:/shaders/chassis_metal.vert.qsb"
    fragmentShader: "qrc:/shaders/chassis_metal.frag.qsb"

    onStatusChanged: if (log) console.log(log)

    Metrics { id: metrics }
}
