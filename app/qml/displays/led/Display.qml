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

// Dot-matrix LED strip: renders a channel title as a panel of round LEDs,
// one LED per pixel of the bundled pixel font. A dark slot (powered: false)
// shows the unlit panel only. The lamps take their colour from the profile's
// font colour, so the bank and the screen burn the same phosphor.
Item {
    id: ledStrip

    property string text: ""
    property bool powered: true
    // The channel on screen: its lamps run hotter and throw a wider halo.
    property bool bright: false
    // The window's width in visible characters: the title gets this many
    // cells whole. The bank's strips follow the setting; a fixture with a
    // window of its own (the blue shell's page counter) names its width here.
    property int characters: appSettings.ledCharacters
    // Unlit lamps ahead of and behind the text, added on top of the
    // character count. The reference mocks never light the column against
    // either of the window's lips, and a glyph standing there reads as cut
    // off by the plate, its glow thrown onto the metal; one dark cell each
    // side keeps the letters clear. Fixtures that fill their window edge to
    // edge set them to 0.
    property int padCellsLeft: appSettings.ledSidePadCells
    property int padCellsRight: appSettings.ledSidePadCells
    // Unlit lamps above and below the glyphs, counted together. A fixture
    // whose window stands taller than the text names the taller band here, so
    // the lamp field reaches the window's lips instead of leaving dead glass
    // there.
    property int padCellsY: appSettings.ledPadCells

    // The LED window's colours, all struck from the profile's font colour: the
    // lamp driven to full, and that same hue sunk towards black for the unlit
    // dots and for the panel they are set into. A dark slot sits darker still,
    // and the current channel's lamps run a shade hotter. The law is this
    // display's own; no other component reads it.
    function ledWindowColors(fontColor, powered, bright) {
        let peak = Math.max(fontColor.r, Math.max(fontColor.g, fontColor.b));
        let lit = Utils.scaleColor(fontColor, peak > 0 ? 1.0 / peak : 1.0);
        if (bright)
            lit = Utils.mix(lit, Qt.rgba(1.0, 1.0, 1.0, 1.0), 0.08);
        return {
            lit: lit,
            dim: Utils.scaleColor(lit, powered ? 0.20 : 0.13),
            panel: Utils.scaleColor(lit, powered ? 0.09 : 0.045)
        };
    }

    readonly property var colors: ledWindowColors(
        appSettings.fontColor, powered, bright)
    readonly property color litColor: colors.lit

    readonly property int gridW: appSettings.ledCellWidth
        * (characters + padCellsLeft + padCellsRight)
    // A band of unlit lamps above and below the glyphs, so the text sits in a
    // lamp field instead of running into the window's lips.
    readonly property int gridH: appSettings.ledCellHeight + padCellsY
    readonly property int topPadCells: Math.floor(padCellsY / 2)

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
        // Drawn low by half the pad, in glyph pixels, so the grid the strip
        // reads has an unlit row or two above the text as well as below.
        topPadding: ledStrip.topPadCells
        // And drawn in by the pad cells, so the first letter's lamps stand
        // clear of the window's left lip.
        leftPadding: ledStrip.padCellsLeft * appSettings.ledCellWidth
        font.family: appSettings.ledFontFamily
        font.pixelSize: appSettings.ledFontPixelSize
        color: "white"
        maximumLineCount: 1
        // Truncated from the head: a title too long for the window is a path,
        // and its tail is the part that names the session.
        text: ledStrip.powered
              ? ledStrip.text.slice(-ledStrip.characters)
              : ""
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
        property color litColor: ledStrip.colors.lit
        property color dimColor: ledStrip.colors.dim
        property color panelColor: ledStrip.colors.panel
        property real dotRadius: 0.40
        property real threshold: 0.4
        property real glow: ledStrip.bright ? 0.55 : 0.3
        property real pixelsPerCell: appSettings.ledDotPitch
        // Where the window sits inside the grown item, as a fraction of it.
        property point spillMargin: Qt.point(
            matrix.width > 0 ? ledStrip.spillMarginX / matrix.width : 0,
            matrix.height > 0 ? ledStrip.spillMarginY / matrix.height : 0)
        // The dead share of each margin: no band of unlit glass stands between
        // the lamps and the metal, so the throw starts at the window's lip.
        property point spillDead: Qt.point(0, 0)
        property real spillStrength: ledStrip.spillStrength

        // The lamps come up to heat rather than snapping, so a channel switch
        // reads as light rising in one window and falling in the other.
        Behavior on glow {
            NumberAnimation { duration: 150 }
        }

        vertexShader: "qrc:/shaders/led_matrix.vert.qsb"
        fragmentShader: "qrc:/shaders/led_matrix.frag.qsb"

        onStatusChanged: if (log) console.log(log) //Print warning messages
    }
}
