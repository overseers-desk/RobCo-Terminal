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

// The amber pager's key: a ridged metal cap over a dark front face, as the
// mock's PREV/NEXT keys are built (four lit ridges on top, the cap's front
// dropping to near-black). Gradient stops stand in for the ridges until the
// paint pass. Rectangle and MouseArea only: no Control, no stolen focus.
Rectangle {
    id: presetButton

    property color plastic: "#241e19"
    property bool pressed: false

    signal clicked()

    readonly property bool hovered: pointer.containsMouse

    readonly property color ridgeHighlight: "#f7e8c4"
    readonly property color ridgeBase: "#c7a381"
    readonly property color frontFace: "#1c1411"

    implicitWidth: 56
    implicitHeight: 40
    radius: 3
    antialiasing: true

    opacity: pressed ? 0.85 : (hovered ? 1.0 : 0.96)

    // Top half: four ridges catching the light. Bottom half: the front face.
    gradient: Gradient {
        GradientStop { position: 0.00; color: presetButton.ridgeHighlight }
        GradientStop { position: 0.08; color: presetButton.ridgeBase }
        GradientStop { position: 0.14; color: presetButton.ridgeHighlight }
        GradientStop { position: 0.22; color: presetButton.ridgeBase }
        GradientStop { position: 0.28; color: presetButton.ridgeHighlight }
        GradientStop { position: 0.36; color: presetButton.ridgeBase }
        GradientStop { position: 0.42; color: presetButton.ridgeHighlight }
        GradientStop { position: 0.50; color: presetButton.ridgeBase }
        GradientStop { position: 0.58; color: presetButton.frontFace }
        GradientStop { position: 1.00; color: presetButton.frontFace }
    }

    transform: Translate { y: presetButton.pressed ? 2 : 0 }

    MouseArea {
        id: pointer
        anchors.fill: parent
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton
        onClicked: presetButton.clicked()
    }
}
