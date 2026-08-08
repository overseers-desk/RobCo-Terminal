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

// One slot of the bank: the numeral printed in the plastic beside the LED
// window carrying that session's title. The window itself is the key: pressing
// it reaches the channel, so the row carries no separate button. The row
// reads nothing from the model; the bank feeds it and takes the press back.
// The numeral is the label, counted within the page; the channel behind it is
// absolute.
Item {
    id: channelRow

    property int channel: 0
    property int label: 0
    property string title: ""
    property bool open: false
    property bool current: false
    property color plastic: "#7a7168"

    // The bank owns the panel's layout; a row carries no opinion of its own.
    required property int numeralWidth
    required property int columnGap
    required property int stripPadding

    signal activated()

    implicitHeight: ledStrip.height + 2 * stripPadding
    implicitWidth: numeralWidth + columnGap + ledStrip.width

    // Two digits always, as the panel printer stamped them.
    readonly property string numeralText:
        label < 10 ? "0" + label : String(label)

    // The confirmation the panel gives when the current session is stored onto this slot.
    function blink() {
        storeBlink.restart()
    }

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
            text: channelRow.numeralText
            color: Qt.lighter(channelRow.plastic, 1.55)
        }
        Text {
            id: engraving
            width: numeral.width
            horizontalAlignment: Text.AlignRight
            font.pixelSize: 13
            font.bold: true
            font.letterSpacing: 0.5
            text: channelRow.numeralText
            color: Qt.darker(channelRow.plastic, 2.1)
        }
    }

    // The dish the window is sunk into: shading alone, no outline. It tints
    // whatever plastic lies behind rather than laying down a sheet of its own,
    // so a translucent profile still sees through the bank.
    Rectangle {
        id: dish

        x: recess.x - 4
        y: recess.y - 4
        width: recess.width + 8
        height: recess.height + 8
        radius: 6
        antialiasing: true

        gradient: Gradient {
            GradientStop { position: 0.0; color: Qt.rgba(0, 0, 0, 0.34) }
            GradientStop { position: 0.62; color: Qt.rgba(0, 0, 0, 0.10) }
            GradientStop { position: 1.0; color: Qt.rgba(1, 1, 1, 0.07) }
        }
    }

    // The window body. Darker under its top lip, where the moulding shades it.
    Rectangle {
        id: recess

        x: channelRow.numeralWidth + channelRow.columnGap - 3
        width: ledStrip.width + 6
        height: ledStrip.height + 6
        radius: 3
        antialiasing: true
        anchors.verticalCenter: parent.verticalCenter

        gradient: Gradient {
            GradientStop { position: 0.0; color: Qt.darker(channelRow.plastic, 5.0) }
            GradientStop { position: 0.45; color: Qt.darker(channelRow.plastic, 3.6) }
            GradientStop { position: 1.0; color: Qt.darker(channelRow.plastic, 2.9) }
        }
    }

    ChannelLedStrip {
        id: ledStrip

        x: recess.x + 3
        anchors.verticalCenter: parent.verticalCenter
        text: channelRow.title
        powered: channelRow.open
        bright: channelRow.current
    }

    // The window is the key. No Control and no focus: the terminal keeps the
    // keyboard, as the button this replaced was careful to.
    MouseArea {
        anchors.fill: recess
        acceptedButtons: Qt.LeftButton
        onClicked: channelRow.activated()
    }

    SequentialAnimation {
        id: storeBlink

        loops: 2
        NumberAnimation { target: ledStrip; property: "opacity"; to: 0.0; duration: 40 }
        NumberAnimation { target: ledStrip; property: "opacity"; to: 1.0; duration: 35 }
    }
}
