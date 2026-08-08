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

    // Scans the rows, not channelState: a buried row is dark on the bank yet
    // still holds its slot. channelState is read only so a model change
    // re-evaluates this.
    readonly property int firstFreeChannel: {
        var stateDep = channelState
        var taken = {}
        for (var i = 0; i < channelsModel.count; i++)
            taken[channelsModel.get(i).channel] = true
        for (var n = 1; n <= channelCap; n++) {
            if (!taken[n])
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

    // The tmux control-mode client while a channel is attached, with the host
    // its titles carry and the slot the gateway gets back on detach.
    property var tmuxGateway: null
    property string tmuxHost: ""
    property int gatewayChannel: 0

    // The set only flinches once it is on: bringing up the first channel is
    // not a channel change.
    property bool _degaussArmed: false

    // A window this set asked tmux for, still on its way. Asking for a channel
    // puts you in it whether the shell is local or a session's away, so the
    // next window to arrive takes the air; the ones tmux volunteers do not.
    property bool _followNextRemote: false

    // A session has landed on a slot: the bank acknowledges it on that row.
    signal channelStored(int channel)

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
            if (!row.buried)
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
        if (_rowOf(channel) >= 0)
            return
        _insertRow({ channel: channel, title: "", kind: "local",
                     windowId: "", paneId: "", buried: false })
        currentChannel = channel
        _rebuildState()
    }

    // A tmux window becomes an ordinary channel on the lowest free slot. Of the
    // windows an attach lists, only the first pulls selection off the buried
    // gateway; the rest line up behind it. A window this set asked for is the
    // exception and takes the air outright.
    function openRemoteChannel(windowId, paneId, name) {
        var channel = firstFreeChannel
        if (channel < 1)
            return
        // Whatever arrives next answers the request, and nothing after it does.
        var asked = _followNextRemote
        _followNextRemote = false
        _insertRow({ channel: channel,
                     title: normalizeTitle(name + "@" + tmuxHost),
                     kind: "remote", windowId: windowId, paneId: paneId,
                     buried: false })
        var currentRow = _rowOf(currentChannel)
        if (asked || currentRow < 0 || channelsModel.get(currentRow).buried)
            currentChannel = channel
        _rebuildState()
        if (currentChannel === channel)
            activateCurrent()
    }

    function _insertRow(row) {
        var dest = 0
        while (dest < channelsModel.count && channelsModel.get(dest).channel < row.channel)
            dest++
        channelsModel.insert(dest, row)
    }

    function openFirstFree() {
        if (firstFreeChannel > 0)
            openChannel(firstFreeChannel)
    }

    // A new channel follows the focus: asked for from a remote channel it is
    // another window on that tmux session, from a local one the lowest free
    // slot with a shell in it.
    function newChannel() {
        var row = _rowOf(currentChannel)
        if (row >= 0 && channelsModel.get(row).kind === "remote" && tmuxGateway)
            newRemoteChannel()
        else
            openFirstFree()
    }

    // Ask the session for another window. The one door for it: the shortcut
    // arrives here when the focus is remote, the menu item when it is asked
    // for by name, and either way the bank goes to the window when it lands.
    function newRemoteChannel() {
        if (!tmuxGateway)
            return
        _followNextRemote = true
        tmuxGateway.newWindow()
    }

    function closeChannel(channel) {
        var row = _rowOf(channel)
        if (row < 0)
            return
        var target = channelsModel.get(row)
        if (target.buried) {
            // Only the gateway is buried, and it only closes when the program
            // holding the session died: no %exit is coming and there is
            // nothing to restore the slot to.
            _gatewayDied(channel)
            return
        }
        if (_visibleCount() <= 1) {
            // The last thing on the air: with a gateway buried behind it the
            // set detaches rather than going dark; without one the appliance
            // switches off.
            if (tmuxGateway)
                tmuxGateway.detach()
            else
                terminalWindow.close()
            return
        }
        if (target.kind === "remote") {
            // tmux owns the window: ask for the kill and remove the row when
            // its %window-close comes back, same as a kill from anywhere else.
            if (tmuxGateway)
                tmuxGateway.killWindow(target.windowId)
            return
        }
        _removeRow(channel)
    }

    // The gateway's program is gone (ssh killed, tmux -CC never said %exit).
    // Its windows are unreachable and its own slot has no shell behind it any
    // more, so the whole set goes, remote rows first: their sessions are the
    // gateway client's wiring, and it dies with the row that owns it.
    function _gatewayDied(channel) {
        console.log("channel " + channel + ": tmux gateway died, dropping "
                    + tmuxHost + " and its windows")
        for (var i = channelsModel.count - 1; i >= 0; i--) {
            if (channelsModel.get(i).kind === "remote")
                channelsModel.remove(i)
        }
        tmuxGateway = null
        tmuxHost = ""
        gatewayChannel = 0
        // Whatever was on the air went with the windows, so the dying row is
        // made current and _removeRow hands the air to its nearest neighbour.
        currentChannel = channel
        _removeRow(channel)
        // Nothing survived it: the appliance has no channel left to show.
        if (channelsModel.count === 0)
            terminalWindow.close()
    }

    // Where a single row goes: local closes and kill-window echoes land here.
    // The bulk sweeps (detach, gateway death) clear their remote rows in their
    // own loops and settle the bank once at the end.
    function _removeRow(channel) {
        var row = _rowOf(channel)
        if (row < 0)
            return
        var wasCurrent = channel === currentChannel
        channelsModel.remove(row)
        if (wasCurrent) {
            var next = _nearestVisibleRow(row)
            if (next >= 0)
                currentChannel = channelsModel.get(next).channel
        }
        _rebuildState()
        activateCurrent()
    }

    // The visible row nearest the hole a removed row left: the one that slid
    // into its place, else the nearest one before it.
    function _nearestVisibleRow(row) {
        for (var after = row; after < channelsModel.count; after++) {
            if (!channelsModel.get(after).buried)
                return after
        }
        for (var before = Math.min(row, channelsModel.count) - 1; before >= 0; before--) {
            if (!channelsModel.get(before).buried)
                return before
        }
        return -1
    }

    function _visibleCount() {
        var count = 0
        for (var i = 0; i < channelsModel.count; i++) {
            if (!channelsModel.get(i).buried)
                count++
        }
        return count
    }

    function selectChannel(channel) {
        if (channelState[channel] === undefined)
            return
        currentChannel = channel
        currentIndex = _rowOf(channel)
        activateCurrent()
    }

    function moveCurrentTo(channel) {
        if (channel < 1 || channel > channelCap || channel === currentChannel)
            return
        var origin = currentChannel
        var from = _rowOf(origin)
        if (from < 0)
            return
        var to = _rowOf(channel)
        // A buried row is off the air but still holds its slot: there is
        // nothing there to swap with, so the store finds the slot taken by
        // something it cannot move and does nothing at all.
        if (to >= 0 && channelsModel.get(to).buried)
            return
        // The session on screen stays the same; its slot number moves. The LED
        // blink is the store's acknowledgement, so the tube holds steady.
        _degaussArmed = false
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
        // Buried rows are off the air, so the wheel rolls past them.
        for (var step = 1; step <= channelsModel.count; step++) {
            var next = ((row + direction * step) % channelsModel.count
                        + channelsModel.count) % channelsModel.count
            if (!channelsModel.get(next).buried) {
                selectChannel(channelsModel.get(next).channel)
                return
            }
        }
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

    // A channel's program has entered tmux control mode and handed up its
    // gateway: the channel leaves the air (row and delegate stay alive, its
    // LED goes dark) and the gateway's windows run the bank until detach
    // hands the slot back. A null arrival is the detach restore, which has its
    // own signal. One gateway at a time is the law: a second channel entering
    // control mode is detached where it stands and comes back a plain shell.
    function attachGateway(channel, gateway) {
        if (!gateway)
            return
        if (tmuxGateway) {
            console.log("channel " + channel + ": second tmux gateway refused, "
                        + tmuxHost + " already has the bank")
            gateway.detach()
            return
        }
        var row = _rowOf(channel)
        if (row < 0)
            return
        tmuxGateway = gateway
        tmuxHost = gateway.host
        gatewayChannel = channel
        // The bank is empty only until the bootstrap listing answers, which is
        // sub-second and never empty: a tmux session always has a window. The
        // buried row's last screen stands there in the meantime.
        channelsModel.setProperty(row, "buried", true)
        _rebuildState()
        // Every channel is the same rectangle of glass, so the bank's grid is
        // the whole client's size; tmux has to hear it once before its windows
        // are drawn at somebody else's.
        _publishClientSize()
    }

    // One client, one geometry: the grid of whichever channel is on the air.
    // QMLTermWidget's terminalSize is QSize(lines, columns), so the width of
    // that size is the number of rows and its height the number of columns;
    // tmux is told columns first, the way it says them back.
    function _publishClientSize() {
        if (tmuxGateway && terminalSize.width > 0 && terminalSize.height > 0)
            tmuxGateway.setClientSize(terminalSize.height, terminalSize.width)
    }

    onTerminalSizeChanged: _publishClientSize()

    // The gateway speaks; the bank moves. Remote titles belong to tmux: they
    // are set here from its notifications, never by the delegate.
    Connections {
        target: channelsRoot.tmuxGateway

        function onHostChanged() {
            channelsRoot.tmuxHost = channelsRoot.tmuxGateway.host
        }
        function onWindowAdded(windowId, paneId, name) {
            channelsRoot.openRemoteChannel(windowId, paneId, name)
        }
        function onWindowRenamed(windowId, name) {
            var channel = channelsRoot._channelOfWindow(windowId)
            if (channel > 0)
                channelsRoot.setTitle(channel, name + "@" + channelsRoot.tmuxHost)
        }
        function onWindowClosed(windowId) {
            var channel = channelsRoot._channelOfWindow(windowId)
            if (channel > 0)
                channelsRoot._removeRow(channel)
            // The last window died under the bank's feet: nothing visible is
            // left, so give the air back to the gateway.
            if (channelsRoot.tmuxGateway && channelsRoot._visibleCount() === 0)
                channelsRoot.tmuxGateway.detach()
        }
        function onDetached() {
            channelsRoot._restoreGateway()
        }
    }

    function _channelOfWindow(windowId) {
        for (var i = 0; i < channelsModel.count; i++) {
            var row = channelsModel.get(i)
            if (row.kind === "remote" && row.windowId === windowId)
                return row.channel
        }
        return 0
    }

    // Detach: the remote channels vanish, the gateway comes back on the air
    // at the slot it never gave up.
    function _restoreGateway() {
        for (var i = channelsModel.count - 1; i >= 0; i--) {
            if (channelsModel.get(i).kind === "remote")
                channelsModel.remove(i)
        }
        var home = gatewayChannel
        for (var j = 0; j < channelsModel.count; j++) {
            if (channelsModel.get(j).buried) {
                channelsModel.setProperty(j, "buried", false)
                home = channelsModel.get(j).channel
            }
        }
        tmuxGateway = null
        tmuxHost = ""
        gatewayChannel = 0
        if (home > 0 && _rowOf(home) >= 0)
            currentChannel = home
        _rebuildState()
        activateCurrent()
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
                    property string rowKind: model.kind
                    property bool shouldHaveFocus: terminalWindow.active && StackLayout.isCurrentItem
                    isActive: StackLayout.isCurrentItem
                    channelKind: model.kind
                    tmuxWindowId: model.windowId
                    tmuxPaneId: model.paneId
                    remoteGateway: model.kind === "remote" ? channelsRoot.tmuxGateway : null
                    onShouldHaveFocusChanged: {
                        if (shouldHaveFocus) {
                            activate()
                        }
                    }
                    // A remote channel's title is tmux's to give; the
                    // emulation's own ideas stay local.
                    onTitleChanged: {
                        if (rowKind === "local")
                            channelsRoot.setTitle(channelNumber, title)
                    }
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    onSessionFinished: channelsRoot.closeChannel(channelNumber)
                    onTmuxGateway: (gateway) => channelsRoot.attachGateway(channelNumber, gateway)
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
