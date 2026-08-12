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

// One strip of embossed punch tape: near-black glossy plastic with rounded
// ends, uppercase letters raised out of it in whitened relief, lit by the
// key the caller names so its highlights sit with the surrounding metal's.
// The glyphs are rasterised on the CPU by FontManager (the LED strip's own
// plumbing) at double the displayed size, so the downscale hands the tape
// shader soft-edged strokes to light. The caller gives text, height and
// position; width follows the text, a wheel cutting only as much tape as it
// stamps. The tape display kit's Display lays one of these in its well.
Item {
    id: tape

    property string text: ""
    // The shell's casting light, so tape and chassis agree on where the
    // room's key stands.
    property vector2d lightDir: Qt.vector2d(0.8, -0.6)
    // Departure Mono over Terminus: its rounder, wider capitals sit closer
    // to the rounded gothic a Dymo wheel stamps than Terminus's condensed
    // terminal face.
    property string fontFamily: "DepartureMono Nerd Font Mono"
    // The letters' cap height rides the strip height; the raster is cut at
    // twice the displayed size for soft stroke edges.
    property real letterScale: 0.66
    property color tapeColor: "#0d0e15"
    property color letterColor: "#f0ece0"
    // Blank plastic past the last letter, as the wheel always leaves.
    property real endPadding: height * 0.55
    // The glyph box centres cap letters a hair high (the raster carries the
    // font's descent); a small push down seats them on the strip.
    property real glyphYNudge: height * 0.04

    visible: text !== ""

    readonly property int _rasterSize: Math.max(8, Math.round(height * letterScale * 2))
    readonly property real _glyphW: glyphImage.implicitWidth / 2
    readonly property real _glyphH: glyphImage.implicitHeight / 2

    implicitWidth: Math.ceil(_glyphW + 2 * endPadding)

    Image {
        id: glyphImage

        smooth: false
        visible: false
        source: tape.text.length > 0
                ? appSettings.fontManager.ledTextImage(
                      tape.fontFamily, tape._rasterSize,
                      tape.text.toUpperCase())
                : ""
    }

    ShaderEffectSource {
        id: glyphSource

        sourceItem: glyphImage
        hideSource: true
        visible: false
        sourceRect: Qt.rect(0, 0, glyphImage.implicitWidth,
                            glyphImage.implicitHeight)
        textureSize: Qt.size(glyphImage.implicitWidth,
                             glyphImage.implicitHeight)
    }

    ShaderEffect {
        anchors.fill: parent

        property size sizePx: Qt.size(width, height)
        property vector2d lightDir: tape.lightDir
        property color tapeColor: tape.tapeColor
        property color letterColor: tape.letterColor
        property vector4d glyphRectPx: Qt.vector4d(
            (width - tape._glyphW) / 2,
            (height - tape._glyphH) / 2 + tape.glyphYNudge,
            Math.max(1, tape._glyphW),
            Math.max(1, tape._glyphH))
        property real bevelPx: 1.4
        property real dilatePx: 0.6
        property real sheenAmount: 1.0
        property real grainAmount: 0.10
        property real seed: 0.37
        property variant glyphMask: glyphSource

        vertexShader: "qrc:/shaders/tape_label.vert.qsb"
        fragmentShader: "qrc:/shaders/tape_label.frag.qsb"

        onStatusChanged: if (log) console.log(log)
    }
}
