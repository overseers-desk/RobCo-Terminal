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

// The blue appliance shows no pager: its rail carries the selector the whole
// height, and the mock has sixteen windows and nothing below them. With one
// page this draws nothing and takes no height, so the bank's rows keep the
// mock's count. Should the slots ever spill onto a second page, a small dark
// rocker surfaces rather than stranding the channels beyond reach.
Item {
    id: pager

    property color plastic: "#4f4737"
    property int pageIndex: 0
    property int pageCount: 1
    property int columnGap: 20

    signal step(int direction)

    visible: pageCount > 1
    implicitHeight: pageCount > 1 ? 26 : 0
    implicitWidth: pageCount > 1 ? 2 * 30 + 4 : 0

    component RockerHalf: Rectangle {
        property int direction: -1

        width: 30
        height: 22
        radius: 3
        antialiasing: true

        gradient: Gradient {
            GradientStop { position: 0.0; color: Qt.darker(pager.plastic, 1.6) }
            GradientStop { position: 1.0; color: Qt.darker(pager.plastic, 3.0) }
        }

        Text {
            anchors.centerIn: parent
            font.pixelSize: 14
            font.bold: true
            text: parent.direction < 0 ? "◂" : "▸"
            color: Qt.lighter(pager.plastic, 1.5)
        }

        MouseArea {
            anchors.fill: parent
            acceptedButtons: Qt.LeftButton
            onClicked: pager.step(parent.direction)
        }
    }

    RockerHalf {
        x: 0
        anchors.verticalCenter: parent.verticalCenter
        direction: -1
    }
    RockerHalf {
        x: 34
        anchors.verticalCenter: parent.verticalCenter
        direction: 1
    }
}
