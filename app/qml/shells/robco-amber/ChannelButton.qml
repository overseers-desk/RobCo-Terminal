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

// The amber pager's key: the mock's own ridged metal cap, sliced with its
// shadow margin, over a dark front face. A press seats the cap two pixels
// lower. Image and MouseArea only: no Control, no stolen focus.
Item {
    id: presetButton

    property color plastic: "#241e19"
    property bool pressed: false

    signal clicked()

    readonly property bool hovered: pointer.containsMouse

    implicitWidth: 56
    implicitHeight: 40

    opacity: pressed ? 0.88 : (hovered ? 1.0 : 0.96)

    Image {
        anchors.fill: parent
        anchors.margins: -3
        source: "assets/key.png"
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
