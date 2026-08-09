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
import QtQuick.Layouts
import QtQml.Models

// Sessions occupy numbered channel slots. A slot whose shell has exited goes
// dark and stays dark: no renumbering. New channels take the lowest free slot.
Item {
    id: channelsRoot

    readonly property int channelCap: 99

    // Dense array indexed by slot number; the title string of the open channel
    // at that slot, undefined where the slot is dark.
    property var channelState: []
    property int currentChannel: 0
    property int currentIndex: -1

    readonly property int firstFreeChannel: {
        for (var n = 1; n <= channelCap; n++) {
            if (channelState[n] === undefined)
                return n
        }
        return 0
    }
    readonly property int highestOpenChannel: {
        for (var n = channelCap; n >= 1; n--) {
            if (channelState[n] !== undefined)
                return n
        }
        return 0
    }
    readonly property string currentTitle: {
        var title = channelState[currentChannel]
        return title ? title : "cool-retro-term"
    }
    property size terminalSize: Qt.size(0, 0)

    // The set only flinches once it is on: bringing up the first channel is
    // not a channel change.
    property bool _degaussArmed: false

    // A session has landed on a slot: the bank acknowledges it on that row.
    signal channelStored(int channel)

    // The open channels as a dense list, sorted by slot: the sparse slot
    // space seen without its dark slots. A consumer keys rows by their
    // channel role, never by position, because closing a channel shifts the
    // rows below it while the slots themselves never renumber.
    readonly property alias openChannels: channelsModel

    // channelsModel rows are kept sorted ascending by channel.
    ListModel {
        id: channelsModel
    }

    function normalizeTitle(rawTitle) {
        if (rawTitle === undefined || rawTitle === null) {
            return ""
        }
        return String(rawTitle).trim()
    }

    function _rowOf(channel) {
        for (var i = 0; i < channelsModel.count; i++) {
            if (channelsModel.get(i).channel === channel)
                return i
        }
        return -1
    }

    function _rebuildState() {
        var state = []
        for (var i = 0; i < channelsModel.count; i++) {
            var row = channelsModel.get(i)
            state[row.channel] = row.title
        }
        // Reassigned wholesale rather than mutated in place: only the
        // assignment notifies bindings on channelState.
        channelState = state
        currentIndex = _rowOf(currentChannel)
    }

    function openChannel(channel) {
        if (channel < 1 || channel > channelCap)
            return
        if (channelState[channel] !== undefined)
            return
        var dest = 0
        while (dest < channelsModel.count && channelsModel.get(dest).channel < channel)
            dest++
        channelsModel.insert(dest, { channel: channel, title: "" })
        currentChannel = channel
        _rebuildState()
    }

    function openFirstFree() {
        if (firstFreeChannel > 0)
            openChannel(firstFreeChannel)
    }

    function closeChannel(channel) {
        var row = _rowOf(channel)
        if (row < 0)
            return
        if (channelsModel.count <= 1) {
            terminalWindow.close()
            return
        }
        var wasCurrent = channel === currentChannel
        channelsModel.remove(row)
        if (wasCurrent) {
            var next = row < channelsModel.count ? row : channelsModel.count - 1
            currentChannel = channelsModel.get(next).channel
        }
        _rebuildState()
        activateCurrent()
    }

    function selectChannel(channel) {
        if (channelState[channel] === undefined)
            return
        currentChannel = channel
        currentIndex = _rowOf(channel)
        activateCurrent()
    }

    // Select by position among the open channels: the tab strip's own
    // vocabulary, where the Nth tab is the Nth open session.
    function selectAt(row) {
        if (row >= 0 && row < channelsModel.count)
            selectChannel(channelsModel.get(row).channel)
    }

    function moveCurrentTo(channel) {
        if (channel < 1 || channel > channelCap || channel === currentChannel)
            return
        var origin = currentChannel
        var from = _rowOf(origin)
        if (from < 0)
            return
        // The session on screen stays the same; its slot number moves. The LED
        // blink is the store's acknowledgement, so the tube holds steady.
        _degaussArmed = false
        var to = _rowOf(channel)
        if (to >= 0) {
            // Occupied slot: the two sessions swap slots. Swap the rows, then
            // restore the channel numbers by position so each session carries
            // its new slot.
            var lower = Math.min(from, to)
            var upper = Math.max(from, to)
            var lowerChannel = channelsModel.get(lower).channel
            var upperChannel = channelsModel.get(upper).channel
            channelsModel.move(lower, upper, 1)
            if (upper - lower > 1)
                channelsModel.move(upper - 1, lower, 1)
            channelsModel.setProperty(lower, "channel", lowerChannel)
            channelsModel.setProperty(upper, "channel", upperChannel)
        } else {
            channelsModel.setProperty(from, "channel", channel)
            var dest = 0
            for (var i = 0; i < channelsModel.count; i++) {
                if (i !== from && channelsModel.get(i).channel < channel)
                    dest++
            }
            if (dest !== from)
                channelsModel.move(from, dest, 1)
        }
        currentChannel = channel
        _rebuildState()
        _degaussArmed = true
        channelStored(channel)
        // A swap lands two sessions, and the displaced one gets its own say.
        if (to >= 0)
            channelStored(origin)
        activateCurrent()
    }

    function cycleOpen(direction) {
        if (channelsModel.count === 0)
            return
        var row = _rowOf(currentChannel)
        if (row < 0)
            return
        var next = (row + direction + channelsModel.count) % channelsModel.count
        selectChannel(channelsModel.get(next).channel)
    }

    // True when buf is a strict prefix of some open slot of the page rooted at
    // base: the chord keeps waiting only for digits that can still land.
    function pageSlotPrefixExists(buf, base, count) {
        for (var n = 1; n <= count; n++) {
            if (channelState[base + n] === undefined)
                continue
            var s = String(n)
            if (s.length > buf.length && s.indexOf(buf) === 0)
                return true
        }
        return false
    }

    function setTitle(channel, rawTitle) {
        var row = _rowOf(channel)
        if (row < 0)
            return
        channelsModel.setProperty(row, "title", normalizeTitle(rawTitle))
        _rebuildState()
    }

    function activateCurrent() {
        var item = channelRepeater.itemAt(currentIndex)
        if (item)
            item.activate()
    }

    Component.onCompleted: {
        openChannel(1)
        _degaussArmed = true
    }

    // Turning the knob makes the tube flinch. Re-selecting the current channel
    // never reaches here: currentChannel does not change.
    onCurrentChannelChanged: {
        if (_degaussArmed)
            degauss.restart()
    }

    Item {
        id: picture

        anchors.fill: parent
        transform: Scale {
            id: pinch
            origin.x: picture.width / 2
            origin.y: picture.height / 2
        }

        StackLayout {
            id: stack
            anchors.fill: parent
            currentIndex: channelsRoot.currentIndex

            Repeater {
                id: channelRepeater
                model: channelsModel
                TerminalContainer {
                    property int channelNumber: model.channel
                    property bool shouldHaveFocus: terminalWindow.active && StackLayout.isCurrentItem
                    isActive: StackLayout.isCurrentItem
                    onShouldHaveFocusChanged: {
                        if (shouldHaveFocus) {
                            activate()
                        }
                    }
                    onTitleChanged: channelsRoot.setTitle(channelNumber, title)
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    onSessionFinished: channelsRoot.closeChannel(channelNumber)
                    onTerminalSizeChanged: publishTerminalSize()
                    StackLayout.onIsCurrentItemChanged: publishTerminalSize()

                    function publishTerminalSize() {
                        if (StackLayout.isCurrentItem)
                            channelsRoot.terminalSize = terminalSize
                    }
                }
            }
        }

        // The phosphor flood, cut to the tube face the frame leaves exposed so
        // it never lights the bezel's corners. Idle it is transparent and out
        // of the scene, so nothing of the flinch survives the animation.
        Rectangle {
            id: flood

            anchors.fill: parent
            anchors.margins: appSettings.frameSize * Math.min(picture.width, picture.height)
            radius: appSettings.screenRadius
            color: appSettings.fontColor
            opacity: 0
            visible: opacity > 0
        }
    }

    // The mockup's degauss keyframe: brightness 2.6, scaleY 0.97, 200 ms
    // ease-out; the 0.25 flood over the phosphor lands the same peak. The
    // flood is behind the same glass as the picture, so it takes the same
    // translucency a see-through profile asks for.
    ParallelAnimation {
        id: degauss

        NumberAnimation {
            target: pinch
            property: "yScale"
            from: 0.97
            to: 1.0
            duration: 200
            easing.type: Easing.OutQuad
        }
        NumberAnimation {
            target: flood
            property: "opacity"
            from: 0.25 * (appSettings.windowOpacity * 0.3 + 0.7)
            to: 0.0
            duration: 200
            easing.type: Easing.OutQuad
        }
    }
}
