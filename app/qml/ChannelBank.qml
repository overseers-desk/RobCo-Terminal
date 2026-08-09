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
// draws no background of its own: the chassis plastic behind it shows through.
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

    // The shell's rule for the column's fixed measures, the display's for the
    // strips'. Synchronous loads: the bank's very first layout divides by
    // these, so they cannot arrive a frame late. A shell declares all of its
    // measures; the fallbacks are safe zeros for the beat before the items
    // exist, the same convention the display metrics keep.
    Loader {
        id: shellMetrics
        source: appSettings.shellUrl("Metrics")
        onLoaded: bank.settle()
    }
    Loader {
        id: displayMetrics
        source: appSettings.displayUrl("Metrics")
        onLoaded: bank.settle()
    }

    readonly property int bankPadding: shellMetrics.item?.bankPadding ?? 0
    // A shell whose furniture is heavier at one edge than another may split
    // the padding; one that says nothing gets the even air instead.
    readonly property int topPadding: shellMetrics.item?.topPadding ?? bankPadding
    readonly property int bottomPadding: shellMetrics.item?.bottomPadding ?? bankPadding
    // Plate standing between the strips' right edge and the frame's moulding;
    // zero for a shell whose strips run to the boundary.
    readonly property int rightPadding: shellMetrics.item?.rightPadding ?? 0
    // Real, not int: a shell whose mock's row pitch is fractional carries the
    // fraction here, or fourteen rows drift three pixels by the bank's foot.
    readonly property real rowSpacing: shellMetrics.item?.rowSpacing ?? 0
    readonly property int columnGap: shellMetrics.item?.columnGap ?? 0
    readonly property int numeralWidth: shellMetrics.item?.numeralWidth ?? 0
    readonly property int stripPadding: shellMetrics.item?.stripPadding ?? 0
    readonly property int minRowHeight: shellMetrics.item?.minRowHeight ?? 0
    // Unlit lamp rows above and below the glyphs, counted together: as many as
    // the shell's punched hole holds beyond the text, so the lamp field runs
    // to the window's lips. The hole is what the window's outer height leaves
    // inside the bezel; a shell whose windows hug their strips has no fixed
    // hole and takes the display's own band.
    readonly property int windowHoleHeight: minRowHeight - 2 * stripPadding
    readonly property int padCellsY: displayMetrics.item
        ? displayMetrics.item.padCellsForHole(windowHoleHeight) : 0

    readonly property int stripWidth: displayMetrics.item
        ? displayMetrics.item.widthForUnits(appSettings.ledCharacters) : 0
    readonly property int stripHeight: displayMetrics.item
        ? displayMetrics.item.heightForPadCells(padCellsY) : 0
    // The fewest characters the display will hold; the seam drag's floor.
    readonly property int minUnits: displayMetrics.item?.minUnits ?? 1
    readonly property int rowHeight: Math.max(minRowHeight, stripHeight + 2 * stripPadding)

    // The profile decides how the channel on screen is marked. The pointer look
    // stands a mechanical selector down the left edge; the glow look drives the
    // window of the channel on screen hotter than the rest.
    readonly property bool pointerIndicator: appSettings.channelIndicator === "pointer"
    readonly property bool glowIndicator: !pointerIndicator
    readonly property int trackWidth: pointerIndicator ? (shellMetrics.item?.trackWidth ?? 0) : 0
    // A shell whose chassis carries the rail as fixed furniture has the lane
    // inside its own padding already, and names where it runs; the bank cuts no
    // second one out of the rows. A shell that draws its slot in the selector
    // itself says nothing and gets the lane taken off the content ground.
    readonly property bool laneInChassis: shellMetrics.item?.chassisCarriesTrack ?? false
    readonly property int trackX: laneInChassis ? (shellMetrics.item?.trackX ?? 0) : bankPadding
    readonly property int trackGap: (pointerIndicator && !laneInChassis) ? columnGap : 0

    // The ground left to the rows once the selector has taken its lane. It
    // runs to the bank's right edge: the frame's moulding is the strips'
    // right frame, so no padding of the bank's own stands before it.
    readonly property int contentX: bankPadding + (laneInChassis ? 0 : trackWidth) + trackGap
    readonly property int contentWidth: width - contentX

    implicitWidth: contentX + numeralWidth + columnGap + stripWidth + rightPadding

    // Rows per page, measured rather than bound: a live count would reflow the
    // bank on every frame of a window drag.
    property int rowsVisible: 1
    property int pageIndex: 0

    readonly property int pageBase: pageIndex * rowsVisible
    // Every open slot has a page, and so does the next free one: the slot a new
    // channel will take is always one the mouse can reach.
    readonly property int pageCount: Math.max(1, Math.ceil(
        Math.max(terminalChannels.highestOpenChannel,
                 terminalChannels.firstFreeChannel) / rowsVisible))
    // The last page stops at the cap. No key is engraved for a slot that no
    // channel can ever take.
    readonly property int rowsOnPage: Math.min(rowsVisible, terminalChannels.channelCap - pageBase)

    readonly property int currentChannel: terminalChannels.currentChannel

    // Where the selector stands, in the track's own coordinates: beside the row
    // of the channel on screen, or down by the pager when that channel is on a
    // page this one is not showing.
    readonly property real pointerY: (currentChannel > pageBase && currentChannel <= pageBase + rowsOnPage)
        ? topPadding - bankPadding + (currentChannel - pageBase - 1) * (rowHeight + rowSpacing) + rowHeight / 2
        : pager.y + pager.height / 2 - bankPadding

    // The pager and the selector are shell components in files of their own,
    // where this file's ids do not reach; every property they take is pushed
    // over explicitly below.

    clip: true

    onHeightChanged: settleTimer.restart()
    onRowHeightChanged: settleTimer.restart()
    onCurrentChannelChanged: ensureVisible(currentChannel)
    onPageCountChanged: pageIndex = Math.min(pageIndex, pageCount - 1)
    // A page flip re-labels the numerals, so digits typed against the old
    // page are abandoned, never committed against the new one.
    onPageIndexChanged: chordInput.cancel()

    Component.onCompleted: settle()

    // A reflow that leaves the row count where it was leaves the page there
    // too: a hand-picked page survives the window being nudged.
    function settle() {
        // Zero metrics are the pre-load beat; a measurement against them
        // would divide by nothing and explode the row count.
        if (rowHeight <= 0)
            return
        var rowsHeight = height - topPadding - bottomPadding - pager.height - rowSpacing
        var measured = Math.max(1, Math.floor((rowsHeight + rowSpacing) / (rowHeight + rowSpacing)))
        if (measured === rowsVisible)
            return
        rowsVisible = measured
        ensureVisible(currentChannel)
    }

    // The character count whose bank width sits nearest the given width,
    // measured from the current width so no layout arithmetic is repeated.
    function charactersForWidth(w) {
        var perChar = displayMetrics.item ? displayMetrics.item.unitWidth : 1
        return appSettings.ledCharacters + Math.round((w - implicitWidth) / perChar)
    }

    // A page slot as the chord and the numerals read it, to its absolute slot;
    // 0 where this page has no such row.
    function absoluteSlot(pageSlot) {
        return pageSlot >= 1 && pageSlot <= rowsOnPage ? pageBase + pageSlot : 0
    }

    // A store targets any page slot, dark included; a select waits only on
    // open ones.
    function slotPrefixExists(buf, store) {
        if (store) {
            for (var n = 1; n <= rowsOnPage; n++) {
                var s = String(n)
                if (s.length > buf.length && s.indexOf(buf) === 0)
                    return true
            }
            return false
        }
        return terminalChannels.pageSlotPrefixExists(buf, pageBase, rowsOnPage)
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

    // The chord names a key on the page the bank is showing, as the numerals
    // engraved beside those keys read; the bank turns it into a slot. It
    // lives with the bank: no bank, no numerals, no chord.
    ChannelChordInput {
        id: chordInput
        onSelectSlot: function (slot) {
            terminalChannels.selectChannel(bank.absoluteSlot(slot))
        }
        onStoreToSlot: function (slot) {
            terminalChannels.moveCurrentTo(bank.absoluteSlot(slot))
        }
        slotPrefixExists: bank.slotPrefixExists
    }
    Shortcut {
        sequence: appSettings.isMacOS ? "Meta+PgUp" : "Alt+PgUp"
        context: Qt.WindowShortcut
        onActivated: bank.step(-1)
    }
    Shortcut {
        sequence: appSettings.isMacOS ? "Meta+PgDown" : "Alt+PgDown"
        context: Qt.WindowShortcut
        onActivated: bank.step(1)
    }

    Connections {
        target: terminalChannels
        function onChannelStored(channel) {
            var row = rows.itemAt(channel - bank.pageBase - 1)
            if (row)
                row.blink()
        }
    }

    Loader {
        id: selector

        active: bank.pointerIndicator
        x: bank.trackX
        y: bank.bankPadding
        width: bank.trackWidth
        height: Math.max(0, bank.height - 2 * bank.bankPadding)

        source: appSettings.shellUrl("SelectorTrack")

        Binding {
            target: selector.item
            property: "plastic"
            value: bank.plastic
        }
        Binding {
            target: selector.item
            property: "targetY"
            value: bank.pointerY
        }
    }

    Column {
        x: bank.contentX
        y: bank.topPadding
        spacing: bank.rowSpacing

        Repeater {
            id: rows

            model: bank.rowsOnPage

            ChannelRow {
                required property int index
                readonly property var slotTitle: terminalChannels.channelState[channel]

                channel: bank.pageBase + index + 1
                label: index + 1
                width: bank.contentWidth
                height: bank.rowHeight
                plastic: bank.plastic
                numeralWidth: bank.numeralWidth
                columnGap: bank.columnGap
                stripPadding: bank.stripPadding
                padCellsY: bank.padCellsY
                open: slotTitle !== undefined
                title: slotTitle !== undefined ? slotTitle : ""
                current: bank.currentChannel === channel
                glowMarks: bank.glowIndicator
                onActivated: bank.press(channel)
            }
        }
    }

    Loader {
        id: pager

        x: bank.contentX
        width: bank.contentWidth
        anchors {
            bottom: parent.bottom
            bottomMargin: bank.bottomPadding
        }

        source: appSettings.shellUrl("Pager")
        onLoaded: item.step.connect(bank.step)

        Binding {
            target: pager.item
            property: "plastic"
            value: bank.plastic
        }
        Binding {
            target: pager.item
            property: "columnGap"
            value: bank.columnGap
        }
        Binding {
            target: pager.item
            property: "pageIndex"
            value: bank.pageIndex
        }
        Binding {
            target: pager.item
            property: "pageCount"
            value: bank.pageCount
        }
    }
}
