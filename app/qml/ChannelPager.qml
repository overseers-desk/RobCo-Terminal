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

// The rocker at the foot of the bank that walks the preset keys from one page
// of slots to the next, with the page count printed beside it. With a single
// page there is nowhere to go and the whole thing sits dim and dead.
Item {
    id: pager

    property color plastic: "#7a7168"
    property int pageIndex: 0
    property int pageCount: 1
    property int columnGap: 6

    readonly property int buttonWidth: 20
    readonly property int buttonHeight: 14
    readonly property int buttonGap: 4

    signal step(int direction)

    enabled: pageCount > 1
    opacity: enabled ? 1.0 : 0.45

    implicitHeight: buttonHeight + 4
    implicitWidth: 2 * buttonWidth + buttonGap + columnGap + counter.width

    ChannelButton {
        id: pageUp

        x: 0
        width: pager.buttonWidth
        height: pager.buttonHeight
        anchors.verticalCenter: parent.verticalCenter
        plastic: pager.plastic
        onClicked: pager.step(-1)

        Text {
            anchors.centerIn: parent
            font.pixelSize: 9
            text: "▲"
            color: Qt.darker(pager.plastic, 2.4)
        }
    }

    ChannelButton {
        x: pageUp.width + pager.buttonGap
        width: pager.buttonWidth
        height: pager.buttonHeight
        anchors.verticalCenter: parent.verticalCenter
        plastic: pager.plastic
        onClicked: pager.step(1)

        Text {
            anchors.centerIn: parent
            font.pixelSize: 9
            text: "▼"
            color: Qt.darker(pager.plastic, 2.4)
        }
    }

    Item {
        id: counter

        readonly property string label: (pager.pageIndex + 1) + "/" + pager.pageCount

        anchors {
            right: parent.right
            verticalCenter: parent.verticalCenter
        }
        width: engraving.implicitWidth
        height: engraving.implicitHeight

        Text {
            y: 1
            font.pixelSize: 12
            font.bold: true
            text: counter.label
            color: Qt.lighter(pager.plastic, 1.55)
        }
        Text {
            id: engraving

            font.pixelSize: 12
            font.bold: true
            text: counter.label
            color: Qt.darker(pager.plastic, 2.1)
        }
    }
}
