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
import QtQuick 2.2

// A latching preset button moulded from the chassis plastic. Rectangle and
// MouseArea only: a Control would take the focus the terminal needs.
Rectangle {
    id: presetButton

    property color plastic: "#7a7168"
    // Latched down while its channel is the one on screen.
    property bool pressed: false

    signal clicked()

    readonly property bool hovered: pointer.containsMouse
    readonly property color body: pressed
        ? Qt.darker(plastic, 1.4)
        : (hovered ? Qt.lighter(plastic, 1.12) : plastic)

    implicitWidth: 24
    implicitHeight: 18
    radius: 4
    antialiasing: true

    border.width: 1
    border.color: Qt.darker(plastic, 1.9)

    gradient: Gradient {
        GradientStop { position: 0.0; color: Qt.lighter(presetButton.body, 1.3) }
        GradientStop { position: 0.55; color: presetButton.body }
        GradientStop { position: 1.0; color: Qt.darker(presetButton.body, 1.35) }
    }

    transform: Translate { y: presetButton.pressed ? 2 : 0 }

    Rectangle {
        anchors {
            left: parent.left
            right: parent.right
            top: parent.top
            margins: 1
        }
        visible: presetButton.pressed
        height: 2
        radius: 1
        opacity: 0.7
        color: Qt.darker(presetButton.plastic, 2.4)
    }

    MouseArea {
        id: pointer
        anchors.fill: parent
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton
        onClicked: presetButton.clicked()
    }
}
