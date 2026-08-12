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

// Embossed tape: a channel title stamped on a strip of punch tape lying in a
// recessed well. The well is furniture and is drawn always; the tape is only
// there when a channel is. An empty slot is the bare well, which is what a
// switchboard row nobody has labelled looks like.
//
// Tape is paint, not light: nothing here reads the profile's phosphor, throws
// glow on the plate around it, or brightens for the channel on screen. A
// profile carrying this kit marks the channel with its own row hardware.
Item {
    id: tapeStrip

    Metrics { id: metrics }

    property string text: ""
    property bool powered: true
    // Declared for the kit contract and ignored: tape cannot be highlighted.
    property bool bright: false
    // The well's width in characters: the seam drag steps this count and the
    // well follows it, holding the whole count whether the title fills it or
    // not. A fixture with a window of its own names its own width here.
    property int characters: appSettings.ledCharacters
    // No-ops. The LED panel frames its glyphs with unlit cells against the
    // window's lips; the tape's own blank ends do that job, and they belong
    // to the tape, not to the well.
    property int padCellsLeft: 0
    property int padCellsRight: 0
    // The height of the hole the fixture punched, come straight through the
    // Metrics pass-through pair. Nought is no height at all here, where in
    // lamp rows it only means no padding, so a fixture that names none, and
    // the preview that stands alone with nobody to name one, gets the kit's
    // own well instead of a slot with no floor.
    property int padCellsY: 0
    // No-op: a tape lying in a slot lights nothing.
    property real spillStrength: 0
    // The room's key, so the emboss's bright rims sit with the casting's.
    // The default is the switchboard's, this kit's home profile; no fixture
    // binds it.
    property vector2d lightDir: Qt.vector2d(-0.4, -0.9)

    // The well is fixed furniture, unlike the tape in it.
    implicitWidth: metrics.widthForUnits(characters)
    implicitHeight: padCellsY > 0 ? padCellsY : metrics.naturalHeight

    // The lip the tape lies inside of, all round.
    readonly property int wellInset: 3
    // The slot floor, near-black and a shade colder than the tape's plastic,
    // so a bare well never reads as a tape with nothing stamped on it.
    readonly property color slotDark: "#0a0c10"

    Rectangle {
        id: slotFloor

        anchors.fill: parent
        radius: 3
        antialiasing: true
        clip: true

        // The punched-panel vocabulary the blue shell's rows already speak:
        // dark under the top lip where the room cannot reach, the far wall
        // catching the light along the bottom.
        gradient: Gradient {
            GradientStop { position: 0.00; color: Qt.darker(tapeStrip.slotDark, 1.7) }
            GradientStop { position: 0.50; color: tapeStrip.slotDark }
            GradientStop { position: 0.90; color: Qt.lighter(tapeStrip.slotDark, 1.8) }
            GradientStop { position: 1.00; color: Qt.lighter(tapeStrip.slotDark, 3.4) }
        }

        // The shadow the top lip throws down the near wall.
        Rectangle {
            width: parent.width
            height: 4
            gradient: Gradient {
                GradientStop { position: 0.0; color: Qt.rgba(0, 0, 0, 0.75) }
                GradientStop { position: 1.0; color: "transparent" }
            }
        }

        // The left wall falls dark with the key leaning that way; the right
        // one catches a little of it.
        Rectangle {
            width: 5
            height: parent.height
            gradient: Gradient {
                orientation: Gradient.Horizontal
                GradientStop { position: 0.0; color: Qt.rgba(0, 0, 0, 0.65) }
                GradientStop { position: 1.0; color: "transparent" }
            }
        }
        Rectangle {
            x: parent.width - width
            width: 3
            height: parent.height
            opacity: 0.5
            gradient: Gradient {
                orientation: Gradient.Horizontal
                GradientStop { position: 0.0; color: "transparent" }
                GradientStop { position: 1.0; color: Qt.lighter(tapeStrip.slotDark, 3.0) }
            }
        }

        // The shadow the tape casts on the floor it lies on, thrown down and
        // to the right of it by the key: a hair of it shows past the strip's
        // lower edge, which is what says the tape is in the well rather than
        // printed on its floor.
        Rectangle {
            visible: label.visible
            x: label.x + 1
            y: label.y + 2
            width: label.width
            height: label.height
            radius: height * 0.28
            color: Qt.rgba(0, 0, 0, 0.55)
        }

        // The tape, laid in from the left end of the well as a hand lays it,
        // and cut off by the far lip if it runs the whole length. A title too
        // long for the well is a path, and its tail is the part that names
        // the session, so the head is what the wheel drops.
        TapeLabel {
            id: label

            x: tapeStrip.wellInset
            anchors.verticalCenter: parent.verticalCenter
            height: tapeStrip.height - 2 * tapeStrip.wellInset
            lightDir: tapeStrip.lightDir
            endPadding: metrics.endPad
            // The strip carries the kit's one letter size whatever height of
            // well it lies in; the label states the size as a share of its
            // own height.
            letterScale: metrics.letterPixelSize / Math.max(1, height)
            text: tapeStrip.powered
                  ? tapeStrip.text.slice(-tapeStrip.characters) : ""
        }
    }
}
