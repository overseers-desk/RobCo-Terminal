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

// Dot-matrix LED strip: renders a channel title as a panel of round LEDs,
// one LED per pixel of the bundled pixel font. A dark slot (powered: false)
// shows the unlit panel only.
Item {
    id: ledStrip

    property string text: ""
    property bool powered: true
    // The current channel: its window is the one the eye should find first.
    property bool bright: false

    readonly property int gridW: appSettings.ledCellWidth * appSettings.ledCharacters
    readonly property int gridH: appSettings.ledCellHeight

    implicitWidth: gridW * appSettings.ledDotPitch
    implicitHeight: gridH * appSettings.ledDotPitch

    // The light this window throws on the plastic around it. The strip draws
    // past its own bounds by these margins and the shader fades the lit dots
    // out across them, so the spill dies inside a row's height and never
    // reaches the window above or below.
    readonly property int spillMarginY: Math.round(implicitHeight * 0.8)
    readonly property int spillMarginX: Math.round(implicitHeight * 1.1)
    // A lit window always glows a little on its surround; the current one
    // glows enough to mark itself. The switch is a fade, not a cut.
    property real spillStrength: powered ? (bright ? 1.0 : 0.3) : 0.0

    Behavior on spillStrength {
        NumberAnimation { duration: 150; easing.type: Easing.InOutQuad }
    }

    Text {
        id: ledText

        renderType: Text.NativeRendering
        font.family: appSettings.ledFontFamily
        font.pixelSize: appSettings.ledFontPixelSize
        color: "white"
        maximumLineCount: 1
        text: ledStrip.powered ? ledStrip.text.substring(0, appSettings.ledCharacters) : ""
    }

    ShaderEffectSource {
        id: ledSource

        sourceItem: ledText
        hideSource: true
        visible: false
        sourceRect: Qt.rect(0, 0, ledStrip.gridW, ledStrip.gridH)
        // One texel per logical pixel, regardless of devicePixelRatio.
        textureSize: Qt.size(ledStrip.gridW, ledStrip.gridH)
    }

    ShaderEffect {
        id: matrix

        anchors {
            fill: parent
            leftMargin: -ledStrip.spillMarginX
            rightMargin: -ledStrip.spillMarginX
            topMargin: -ledStrip.spillMarginY
            bottomMargin: -ledStrip.spillMarginY
        }
        // The margin is plastic seen through added light, so it has to blend.
        blending: true

        property variant source: ledSource
        property size gridSize: Qt.size(ledStrip.gridW, ledStrip.gridH)
        property color litColor: "#ff4a26"
        property color dimColor: ledStrip.powered ? "#38100c" : "#1a0806"
        property color panelColor: ledStrip.powered ? "#1e0505" : "#0e0202"
        property real dotRadius: 0.36
        property real threshold: 0.4
        property real glow: 0.35
        property real pixelsPerCell: appSettings.ledDotPitch
        // Where the window sits inside the grown item, as a fraction of it.
        property point spillMargin: Qt.point(
            matrix.width > 0 ? ledStrip.spillMarginX / matrix.width : 0,
            matrix.height > 0 ? ledStrip.spillMarginY / matrix.height : 0)
        property real spillStrength: ledStrip.spillStrength

        vertexShader: "qrc:/shaders/led_matrix.vert.qsb"
        fragmentShader: "qrc:/shaders/led_matrix.frag.qsb"

        onStatusChanged: if (log) console.log(log) //Print warning messages
    }
}
