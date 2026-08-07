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

// One slot of the bank: the numeral engraved in the plastic, the preset button
// that selects it, and the LED strip carrying that session's title. The row
// reads nothing from the model; the bank feeds it and takes the press back.
Item {
    id: channelRow

    property int channel: 0
    property string title: ""
    property bool open: false
    property bool current: false
    property color plastic: "#7a7168"

    property int numeralWidth: 20
    property int columnGap: 6
    property int buttonWidth: 24
    property int buttonHeight: 18
    property int stripPadding: 3

    signal activated()

    implicitHeight: ledStrip.height + 2 * stripPadding
    implicitWidth: numeralWidth + columnGap + buttonWidth + columnGap + ledStrip.width

    Item {
        id: numeral

        x: 0
        width: channelRow.numeralWidth
        height: engraving.implicitHeight
        anchors.verticalCenter: parent.verticalCenter

        Text {
            width: numeral.width
            y: 1
            horizontalAlignment: Text.AlignRight
            font.pixelSize: 13
            font.bold: true
            font.letterSpacing: 0.5
            text: channelRow.channel
            color: Qt.lighter(channelRow.plastic, 1.55)
        }
        Text {
            id: engraving
            width: numeral.width
            horizontalAlignment: Text.AlignRight
            font.pixelSize: 13
            font.bold: true
            font.letterSpacing: 0.5
            text: channelRow.channel
            color: Qt.darker(channelRow.plastic, 2.1)
        }
    }

    ChannelButton {
        id: presetButton

        x: channelRow.numeralWidth + channelRow.columnGap
        width: channelRow.buttonWidth
        height: channelRow.buttonHeight
        anchors.verticalCenter: parent.verticalCenter
        plastic: channelRow.plastic
        pressed: channelRow.current
        onClicked: channelRow.activated()
    }

    Rectangle {
        id: recess

        x: presetButton.x + presetButton.width + channelRow.columnGap - 2
        width: ledStrip.width + 4
        height: ledStrip.height + 4
        anchors.verticalCenter: parent.verticalCenter
        color: Qt.darker(channelRow.plastic, 3.4)
        border.width: 1
        border.color: Qt.darker(channelRow.plastic, 2.2)

        Rectangle {
            anchors {
                left: parent.left
                right: parent.right
                bottom: parent.bottom
                bottomMargin: -1
            }
            height: 1
            color: Qt.lighter(channelRow.plastic, 1.3)
        }
    }

    ChannelLedStrip {
        id: ledStrip

        x: recess.x + 2
        anchors.verticalCenter: parent.verticalCenter
        text: channelRow.title
        powered: channelRow.open
    }
}
