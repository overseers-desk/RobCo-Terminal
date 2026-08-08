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

    implicitHeight: display.height + 2 * stripPadding
    implicitWidth: numeralWidth + columnGap + display.width

    // Two digits always, as the panel printer stamped them.
    readonly property string numeralText:
        label < 10 ? "0" + label : String(label)

    // Where the title display sits, in this row's coordinates: the shell's
    // furniture paints its window moulding around this rectangle.
    readonly property rect displayRect: Qt.rect(
        display.x, (height - display.height) / 2, display.width, display.height)

    // The confirmation the panel gives when the current session is stored onto this slot.
    function blink() {
        storeBlink.restart()
    }

    // The plastic around the window: numeral, dish and moulding, all the
    // shell's. It lives in a file of its own where this row's ids do not
    // reach, so everything it paints from is pushed over explicitly.
    Loader {
        id: furniture

        anchors.fill: parent
        source: appSettings.shellUrl("RowFurniture")

        Binding {
            target: furniture.item
            property: "plastic"
            value: channelRow.plastic
        }
        Binding {
            target: furniture.item
            property: "numeralText"
            value: channelRow.numeralText
        }
        Binding {
            target: furniture.item
            property: "displayRect"
            value: channelRow.displayRect
        }
        Binding {
            target: furniture.item
            property: "open"
            value: channelRow.open
        }
        Binding {
            target: furniture.item
            property: "current"
            value: channelRow.current
        }
    }

    Loader {
        id: display

        x: channelRow.numeralWidth + channelRow.columnGap
        anchors.verticalCenter: parent.verticalCenter
        source: appSettings.displayUrl("Display")

        Binding {
            target: display.item
            property: "text"
            value: channelRow.title
        }
        Binding {
            target: display.item
            property: "powered"
            value: channelRow.open
        }
        Binding {
            target: display.item
            property: "bright"
            value: channelRow.current
        }
    }

    // The window is the key. No Control and no focus: the terminal keeps the
    // keyboard, as the button this replaced was careful to. It covers the
    // whole window body, moulding lip included, as it always has.
    MouseArea {
        x: channelRow.displayRect.x - 3
        y: channelRow.displayRect.y - 3
        width: channelRow.displayRect.width + 6
        height: channelRow.displayRect.height + 6
        acceptedButtons: Qt.LeftButton
        onClicked: channelRow.activated()
    }

    SequentialAnimation {
        id: storeBlink

        loops: 2
        NumberAnimation { target: display; property: "opacity"; to: 0.0; duration: 40 }
        NumberAnimation { target: display; property: "opacity"; to: 1.0; duration: 35 }
    }
}
