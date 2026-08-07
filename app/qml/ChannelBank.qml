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

import "utils.js" as Utils

// The bank of channel rows set into the chassis, left of the screen well. It
// draws no background of its own: the chassis plastic behind it is the panel.
// Its width follows the LED strip settings alone and never the window's size,
// so dragging the window edge cannot move the screen well's left edge.
//
// The rows are a fixed-size window onto the slot space, paged: the numerals
// read 1..N on every page, the way a car stereo reuses its preset keys across
// FM1/FM2/FM3.
Item {
    id: bank

    readonly property color plastic: Utils.frameBaseColor(
        appSettings.frameColor,
        appSettings.fontColor,
        appSettings.backgroundColor,
        appSettings.ambientLight
    )

    readonly property int bankPadding: 10
    readonly property int rowSpacing: 6
    readonly property int columnGap: 6
    readonly property int numeralWidth: 20
    readonly property int buttonWidth: 24
    readonly property int buttonHeight: 18
    // Sized so a channel row's pitch sits near two terminal text rows
    // (25 px at the default profile), the bank's readability target.
    readonly property int stripPadding: 10
    readonly property int minRowHeight: 26

    readonly property int stripWidth: appSettings.ledCellWidth * appSettings.ledCharacters * appSettings.ledDotPitch
    readonly property int stripHeight: appSettings.ledCellHeight * appSettings.ledDotPitch
    readonly property int rowHeight: Math.max(minRowHeight, stripHeight + 2 * stripPadding)

    implicitWidth: 2 * bankPadding + numeralWidth + columnGap + buttonWidth + columnGap + stripWidth

    // Rows per page, measured rather than bound: a live count would reflow the
    // bank on every frame of a window drag.
    property int rowsVisible: 1
    property int pageIndex: 0

    readonly property int pageBase: pageIndex * rowsVisible
    // Every open slot has a page, and so does the next free one: the slot a new
    // channel will take is always one the mouse can reach.
    readonly property int pageCount: Math.min(
        Math.max(1, Math.ceil(Math.max(terminalChannels.highestOpenChannel,
                                       terminalChannels.firstFreeChannel) / rowsVisible)),
        Math.ceil(terminalChannels.channelCap / rowsVisible))

    readonly property int currentChannel: terminalChannels.currentChannel

    clip: true

    onHeightChanged: settleTimer.restart()
    onRowHeightChanged: settleTimer.restart()
    onCurrentChannelChanged: ensureVisible(currentChannel)
    onPageCountChanged: pageIndex = Math.min(pageIndex, pageCount - 1)

    Component.onCompleted: settle()

    function settle() {
        var rowsHeight = height - 2 * bankPadding - pager.height - rowSpacing
        rowsVisible = Math.max(1, Math.floor((rowsHeight + rowSpacing) / (rowHeight + rowSpacing)))
        ensureVisible(currentChannel)
    }

    // A page slot as the chord and the numerals read it, to its absolute slot;
    // 0 where this page has no such row.
    function absoluteSlot(pageSlot) {
        return pageSlot >= 1 && pageSlot <= rowsVisible ? pageBase + pageSlot : 0
    }

    function slotPrefixExists(buf) {
        return terminalChannels.pageSlotPrefixExists(buf, pageBase, rowsVisible)
    }

    function step(direction) {
        pageIndex = Math.max(0, Math.min(pageCount - 1, pageIndex + direction))
    }

    function ensureVisible(channel) {
        if (channel >= 1)
            pageIndex = Math.floor((channel - 1) / rowsVisible)
    }

    // The press of a preset: a dark slot starts a session on it, an open one
    // comes to the screen, and either way the shell gets the keyboard back.
    function press(channel) {
        if (terminalChannels.channelState[channel] === undefined)
            terminalChannels.openChannel(channel)
        else
            terminalChannels.selectChannel(channel)
        terminalChannels.activateCurrent()
    }

    // A drag walks the height a frame at a time; the bank reflows once it stops.
    Timer {
        id: settleTimer
        interval: 150
        onTriggered: bank.settle()
    }

    Connections {
        target: terminalChannels
        function onChannelStored(channel) {
            var row = rows.itemAt(channel - bank.pageBase - 1)
            if (row)
                row.blink()
        }
    }

    Column {
        x: bank.bankPadding
        y: bank.bankPadding
        spacing: bank.rowSpacing

        Repeater {
            id: rows

            model: bank.rowsVisible

            ChannelRow {
                readonly property var slotTitle: terminalChannels.channelState[channel]

                channel: bank.pageBase + index + 1
                label: index + 1
                width: bank.width - 2 * bank.bankPadding
                height: bank.rowHeight
                plastic: bank.plastic
                numeralWidth: bank.numeralWidth
                columnGap: bank.columnGap
                buttonWidth: bank.buttonWidth
                buttonHeight: bank.buttonHeight
                stripPadding: bank.stripPadding
                open: slotTitle !== undefined
                title: slotTitle !== undefined ? slotTitle : ""
                current: bank.currentChannel === channel
                onActivated: bank.press(channel)
            }
        }
    }

    ChannelPager {
        id: pager

        x: bank.bankPadding
        width: bank.width - 2 * bank.bankPadding
        anchors {
            bottom: parent.bottom
            bottomMargin: bank.bankPadding
        }
        plastic: bank.plastic
        columnGap: bank.columnGap
        pageIndex: bank.pageIndex
        pageCount: bank.pageCount
        onStep: function (direction) { bank.step(direction) }
    }
}
