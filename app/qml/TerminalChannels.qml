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

// Sessions occupy numbered channel slots on machine-scoped pages. The home
// page holds the local shells; every tmux -CC attachment is a page of its
// own, anchored at channel 1 by the very channel that attached. Within a
// page, a slot whose shell has exited goes dark and stays dark: no
// renumbering. New channels take the lowest free slot of their page.
Item {
    id: channelsRoot

    readonly property int channelCap: 99

    // The machine selector's model: row 0 is home and never leaves; a tmux
    // attachment appends {kind, host, homeSlot, pageId, follow} and collapse
    // removes it. homeSlot is the home slot the anchor holds while abroad;
    // follow marks a window this page asked tmux for, still on its way:
    // asking for a channel puts you in it, so the next window to arrive on
    // the page takes the air, while the ones tmux volunteers do not.
    ListModel {
        id: pagesModel
        ListElement { kind: "home"; host: ""; homeSlot: 0; pageId: 0; follow: false }
    }
    property int _nextPageId: 1

    // The gateways in a parallel map keyed by pageId: a ListModel cannot hold
    // a QObject. Reassigned wholesale on every change, so bindings notice.
    property var _gateways: ({})

    // Which page the air is on, and which slot of it. currentIndex is the
    // model row of that pair, the StackLayout's cue.
    property int currentPage: 0
    property int currentChannel: 0
    property int currentIndex: -1

    // The page the user is looking at: the bank binds its viewed page here
    // while it steps the pager without stealing the air; a profile with no
    // bank leaves it on the page the air is on. Ctrl+Shift+T acts here.
    property int pageOnView: currentPage

    // Every page's slots at once, pageId to a dense array indexed by slot
    // number holding the open channel's title there; channelState is the
    // current page's face of it, what the window title reads. Both are
    // derived caches with _rebuildState their single writer, reassigned
    // wholesale rather than mutated: only the assignment notifies bindings.
    property var slotStates: ({})
    property var channelState: []

    readonly property int firstFreeChannel: {
        var stateDep = slotStates
        var pagesDep = pagesModel.count
        return _firstFree(currentPage)
    }
    readonly property string currentTitle: {
        var title = channelState[currentChannel]
        return title ? title : "cool-retro-term"
    }
    property size terminalSize: Qt.size(0, 0)

    // The current page's machinery, for the menus: its host's name and its
    // gateway, both empty on home. An action is enabled exactly while the
    // page the air is on is an attachment.
    readonly property string currentPageHost: {
        var gatewayDep = _gateways
        var row = _pageRowOf(currentPage)
        return row >= 0 ? pagesModel.get(row).host : ""
    }
    readonly property var currentPageGateway: _gateways[currentPage] ?? null

    // The set only flinches once it is on: bringing up the first channel is
    // not a channel change.
    property bool _degaussArmed: false

    // A session has landed on a slot: the bank acknowledges it on that row.
    signal channelStored(int channel)

    // The current page's channels on the air as a dense list, sorted by
    // slot: the sparse slot space seen without its dark slots. A derived
    // cache like channelState, with _rebuildState its single writer, so the
    // tab strip can bind a Repeater to it. Consumers key rows by their
    // channel role, never by position: closing a channel shifts the rows
    // below it while the slots themselves never renumber.
    readonly property alias pageChannels: pageList

    ListModel {
        id: pageList
    }

    // channelsModel rows {page, channel, title, kind, windowId, paneId} are
    // kept sorted ascending by page then channel; pageIds only ever grow, so
    // the page order is the attach order with home first. kind is "local"
    // for a shell of this machine's, "remote" for a tmux window fed through
    // its page's gateway, "anchor" for the transported channel standing at a
    // page's slot 1. Transport and collapse mutate a row in place rather
    // than remove it: the row's delegate is the living session, and the
    // glass never blanks.
    ListModel {
        id: channelsModel
    }

    function normalizeTitle(rawTitle) {
        if (rawTitle === undefined || rawTitle === null) {
            return ""
        }
        return String(rawTitle).trim()
    }

    function _rowOf(page, channel) {
        for (var i = 0; i < channelsModel.count; i++) {
            var row = channelsModel.get(i)
            if (row.page === page && row.channel === channel)
                return i
        }
        return -1
    }

    function _pageRowOf(pageId) {
        for (var i = 0; i < pagesModel.count; i++) {
            if (pagesModel.get(i).pageId === pageId)
                return i
        }
        return -1
    }

    // A transported channel's home slot stays dark and held while it is
    // abroad: the hold lives on the page object, not on any row.
    function _slotHeld(channel) {
        for (var i = 0; i < pagesModel.count; i++) {
            var page = pagesModel.get(i)
            if (page.kind === "tmux" && page.homeSlot === channel)
                return true
        }
        return false
    }

    // The lowest free slot of a page. On home the held slots count as taken;
    // on an attachment the anchor holds 1, so windows fill from 2.
    function _firstFree(page) {
        var taken = {}
        for (var i = 0; i < channelsModel.count; i++) {
            var row = channelsModel.get(i)
            if (row.page === page)
                taken[row.channel] = true
        }
        if (page === 0) {
            for (var j = 0; j < pagesModel.count; j++) {
                var p = pagesModel.get(j)
                if (p.kind === "tmux")
                    taken[p.homeSlot] = true
            }
        }
        for (var n = 1; n <= channelCap; n++) {
            if (!taken[n])
                return n
        }
        return 0
    }

    function _highestOpen(page) {
        var highest = 0
        for (var i = 0; i < channelsModel.count; i++) {
            var row = channelsModel.get(i)
            if (row.page === page && row.channel > highest)
                highest = row.channel
        }
        return highest
    }

    // The bank's pager walks one flattened space over every page: each page
    // unrolls into as many bank pages as its slots need, every open slot
    // reachable and so is the next free one, the slot a new channel will
    // take. Returns {page, count} per page in selector order.
    function viewPages(rowsVisible) {
        var stateDep = slotStates
        var pagesDep = pagesModel.count
        var rows = Math.max(1, rowsVisible)
        var pages = []
        for (var i = 0; i < pagesModel.count; i++) {
            var pageId = pagesModel.get(i).pageId
            var span = Math.max(_highestOpen(pageId), _firstFree(pageId), 1)
            pages.push({ page: pageId, count: Math.ceil(span / rows) })
        }
        return pages
    }

    function _rebuildState() {
        var states = {}
        for (var i = 0; i < channelsModel.count; i++) {
            var row = channelsModel.get(i)
            var state = states[row.page]
            if (state === undefined)
                state = states[row.page] = []
            state[row.channel] = row.title
        }
        slotStates = states
        channelState = states[currentPage] ?? []
        currentIndex = _rowOf(currentPage, currentChannel)

        // Merge the current page's rows into pageList in place: both are
        // sorted by slot, and a title-only change touches one row's
        // property, so the strip's delegates survive a prompt changing a
        // title.
        var pos = 0
        for (var j = 0; j < channelsModel.count; j++) {
            var r = channelsModel.get(j)
            if (r.page !== currentPage)
                continue
            while (pos < pageList.count && pageList.get(pos).channel < r.channel)
                pageList.remove(pos)
            if (pos < pageList.count && pageList.get(pos).channel === r.channel) {
                if (pageList.get(pos).title !== r.title)
                    pageList.setProperty(pos, "title", r.title)
            } else {
                pageList.insert(pos, { channel: r.channel, title: r.title })
            }
            pos++
        }
        while (pageList.count > pos)
            pageList.remove(pos)
    }

    // Local shells live on home alone: an attachment's channels are tmux's
    // to give. A held slot refuses; it is a transported channel's berth.
    function openChannel(page, channel) {
        if (page !== 0)
            return
        if (channel < 1 || channel > channelCap)
            return
        if (_rowOf(page, channel) >= 0 || _slotHeld(channel))
            return
        _insertRow({ page: 0, channel: channel, title: "", kind: "local",
                     windowId: "", paneId: "" })
        currentPage = 0
        currentChannel = channel
        _rebuildState()
    }

    // A tmux window becomes an ordinary channel on the lowest free slot of
    // its page, from 2 up: the anchor holds 1. Windows the attach lists line
    // up behind the anchor without taking the air; a window this set asked
    // for is the exception and takes it outright.
    function openRemoteChannel(page, windowId, paneId, name) {
        var channel = _firstFree(page)
        if (channel < 1)
            return
        var pageRow = _pageRowOf(page)
        if (pageRow < 0)
            return
        // Whatever arrives next answers the request, and nothing after it does.
        var asked = pagesModel.get(pageRow).follow
        if (asked)
            pagesModel.setProperty(pageRow, "follow", false)
        _insertRow({ page: page, channel: channel,
                     title: normalizeTitle(name),
                     kind: "remote", windowId: windowId, paneId: paneId })
        if (asked) {
            currentPage = page
            currentChannel = channel
        }
        _rebuildState()
        if (asked)
            activateCurrent()
    }

    function _insertRow(row) {
        var dest = 0
        while (dest < channelsModel.count && _lessThan(channelsModel.get(dest), row))
            dest++
        channelsModel.insert(dest, row)
    }

    function _lessThan(a, b) {
        return a.page !== b.page ? a.page < b.page : a.channel < b.channel
    }

    // A row whose page or channel was rewritten in place slides to where the
    // sort order wants it. A move, never a remove: the delegate survives.
    function _resortRow(index) {
        var row = channelsModel.get(index)
        var dest = 0
        for (var i = 0; i < channelsModel.count; i++) {
            if (i !== index && _lessThan(channelsModel.get(i), row))
                dest++
        }
        if (dest !== index)
            channelsModel.move(index, dest, 1)
    }

    function openFirstFree() {
        var slot = _firstFree(0)
        if (slot > 0)
            openChannel(0, slot)
    }

    // A new channel goes to the page on view: on home the lowest free slot
    // with a shell in it, on an attachment another window of that session.
    function newChannel() {
        var pageRow = _pageRowOf(pageOnView)
        var gateway = _gateways[pageOnView]
        if (pageRow >= 0 && pagesModel.get(pageRow).kind === "tmux" && gateway) {
            pagesModel.setProperty(pageRow, "follow", true)
            gateway.newWindow()
        } else {
            openFirstFree()
        }
    }

    // Ask the current page's session for another window. The one door for
    // it: the shortcut arrives here when that page is on view, the menu item
    // when it is asked for by name, and either way the bank goes to the
    // window when it lands.
    function newRemoteChannel() {
        var gateway = currentPageGateway
        if (!gateway)
            return
        pagesModel.setProperty(_pageRowOf(currentPage), "follow", true)
        gateway.newWindow()
    }

    // What the user's close asks for, by what the row is. An anchor detaches
    // its page: tmux keeps the session, the channel comes home. A remote
    // window is tmux's to kill, and the row goes when %window-close comes
    // back. A local shell's row just goes; the last one anywhere switches
    // the appliance off.
    function closeChannel(page, channel) {
        var row = _rowOf(page, channel)
        if (row < 0)
            return
        var target = channelsModel.get(row)
        var gateway = _gateways[page]
        if (target.kind === "anchor") {
            if (gateway)
                gateway.detach()
            return
        }
        if (target.kind === "remote") {
            if (gateway)
                gateway.killWindow(target.windowId)
            return
        }
        if (channelsModel.count <= 1) {
            terminalWindow.close()
            return
        }
        _removeRow(page, channel)
    }

    // A row's own program died. For a local shell that is the ordinary end
    // of a channel. For an anchor it is the gateway dying under the session
    // (ssh killed, tmux -CC never said %exit): its windows are unreachable
    // and the slot it would come home to has no shell behind it any more,
    // so the page collapses and the returned row goes too.
    function sessionDied(page, channel) {
        var row = _rowOf(page, channel)
        if (row < 0)
            return
        if (channelsModel.get(row).kind === "anchor") {
            _anchorDied(page)
            return
        }
        if (channelsModel.count <= 1) {
            terminalWindow.close()
            return
        }
        _removeRow(page, channel)
    }

    function _anchorDied(page) {
        var pageRow = _pageRowOf(page)
        if (pageRow < 0)
            return
        var host = pagesModel.get(pageRow).host
        var homeSlot = pagesModel.get(pageRow).homeSlot
        console.log("page " + page + ": tmux gateway died, dropping "
                    + host + " and its windows")
        _collapsePage(page)
        _removeRow(0, homeSlot)
        // Nothing survived it: the appliance has no channel left to show.
        if (channelsModel.count === 0)
            terminalWindow.close()
    }

    // Where a single row goes: local closes and kill-window echoes land
    // here. The bulk sweep (collapse) clears its remote rows in its own loop
    // and settles the bank once at the end.
    function _removeRow(page, channel) {
        var row = _rowOf(page, channel)
        if (row < 0)
            return
        var wasCurrent = page === currentPage && channel === currentChannel
        channelsModel.remove(row)
        if (wasCurrent) {
            var next = _nearestRow(row, page)
            if (next >= 0) {
                currentPage = channelsModel.get(next).page
                currentChannel = channelsModel.get(next).channel
            }
        }
        _rebuildState()
        activateCurrent()
    }

    // The row nearest the hole a removed row left, its own page's rows
    // first: the one that slid into its place, else the nearest one before
    // it, else whatever another page still holds.
    function _nearestRow(row, page) {
        for (var after = row; after < channelsModel.count; after++) {
            if (channelsModel.get(after).page === page)
                return after
        }
        for (var before = Math.min(row, channelsModel.count) - 1; before >= 0; before--) {
            if (channelsModel.get(before).page === page)
                return before
        }
        if (channelsModel.count > 0)
            return Math.min(row, channelsModel.count - 1)
        return -1
    }

    function selectChannel(page, channel) {
        var state = slotStates[page]
        if (!state || state[channel] === undefined)
            return
        currentPage = page
        currentChannel = channel
        _rebuildState()
        activateCurrent()
    }

    // Select by position among the channels on the air: the tab strip's own
    // vocabulary, where the Nth tab is the Nth visible session.
    function selectAt(row) {
        if (row >= 0 && row < pageList.count)
            selectChannel(currentPage, pageList.get(row).channel)
    }

    // The session on screen keeps its screen; its slot number moves. Slots
    // are a page's own numerals, so a store never crosses pages, and an
    // anchor never moves off 1: the transported channel is the page's
    // reason for existing. On home a held slot refuses the way it refuses
    // everything: there is a channel abroad that owns it.
    function moveCurrentTo(page, channel) {
        // The bank can be viewing one machine's page while another holds the
        // air, and a store chord lands on the page on view: that is a store
        // asked for on a page the session is not on, and there is no slot on
        // it that the session could take without leaving its own machine.
        // Nothing happens, which is the whole of the answer.
        if (page !== currentPage)
            return
        if (channel < 1 || channel > channelCap || channel === currentChannel)
            return
        var origin = currentChannel
        var from = _rowOf(page, origin)
        if (from < 0 || channelsModel.get(from).kind === "anchor")
            return
        if (page === 0 && _slotHeld(channel))
            return
        var to = _rowOf(page, channel)
        if (to >= 0 && channelsModel.get(to).kind === "anchor")
            return
        // The LED blink is the store's acknowledgement, so the tube holds
        // steady.
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
            _resortRow(from)
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

    // Cycling walks the current page: the other machines' channels are a
    // pager step away, not a knob turn.
    function cycleOpen(direction) {
        var rows = []
        var pos = -1
        for (var i = 0; i < channelsModel.count; i++) {
            var row = channelsModel.get(i)
            if (row.page !== currentPage)
                continue
            if (row.channel === currentChannel)
                pos = rows.length
            rows.push(row.channel)
        }
        if (rows.length === 0 || pos < 0)
            return
        var next = ((pos + direction) % rows.length + rows.length) % rows.length
        selectChannel(currentPage, rows[next])
    }

    // True when buf is a strict prefix of some open slot of the named page's
    // stretch rooted at base: the chord keeps waiting only for digits that
    // can still land.
    function pageSlotPrefixExists(page, buf, base, count) {
        var state = slotStates[page] ?? []
        for (var n = 1; n <= count; n++) {
            if (state[base + n] === undefined)
                continue
            var s = String(n)
            if (s.length > buf.length && s.indexOf(buf) === 0)
                return true
        }
        return false
    }

    function setTitle(page, channel, rawTitle) {
        var row = _rowOf(page, channel)
        if (row < 0)
            return
        channelsModel.setProperty(row, "title", normalizeTitle(rawTitle))
        _rebuildState()
    }

    // A channel's program has entered tmux control mode and handed up its
    // gateway: the channel transports to slot 1 of a new page, titled for
    // the machine the session is on, and its home slot is held dark behind
    // it. The row mutates in place, never removes, so its delegate survives
    // and the glass keeps the screen it was showing; nothing blanks. A null
    // arrival is teardown's echo; collapse rides the detached signal.
    function attachGateway(page, channel, gateway) {
        if (!gateway)
            return
        var row = _rowOf(page, channel)
        if (row < 0 || channelsModel.get(row).kind !== "local")
            return
        var pageId = _nextPageId++
        _setGateway(pageId, gateway)
        pagesModel.append({ kind: "tmux", host: gateway.host, homeSlot: channel,
                            pageId: pageId, follow: false })
        var wasCurrent = page === currentPage && channel === currentChannel
        // The session on screen stays the same session: transport is a
        // renumbering, not a channel change, so the tube holds steady.
        _degaussArmed = false
        channelsModel.setProperty(row, "page", pageId)
        channelsModel.setProperty(row, "channel", 1)
        channelsModel.setProperty(row, "kind", "anchor")
        channelsModel.setProperty(row, "title", "tmux -CC # @" + gateway.host)
        _resortRow(row)
        if (wasCurrent) {
            currentPage = pageId
            currentChannel = 1
        }
        _rebuildState()
        _degaussArmed = true
        // Every channel is the same rectangle of glass, so the bank's grid is
        // the whole client's size; tmux has to hear it once before its windows
        // are drawn at somebody else's.
        _publishClientSize()
    }

    function _setGateway(pageId, gateway) {
        var gateways = {}
        for (var key in _gateways)
            gateways[key] = _gateways[key]
        if (gateway)
            gateways[pageId] = gateway
        else
            delete gateways[pageId]
        _gateways = gateways
    }

    // Detach or gateway death: the page's remote rows vanish, the anchor
    // transports home to the slot it never gave up and relights, the user
    // lands on it, and the shell's own title escape repaints the row on its
    // next prompt: abroad the title was the model's, home it is the shell's
    // again.
    function _collapsePage(pageId) {
        var pageRow = _pageRowOf(pageId)
        if (pageRow < 0)
            return
        var homeSlot = pagesModel.get(pageRow).homeSlot
        _degaussArmed = false
        for (var i = channelsModel.count - 1; i >= 0; i--) {
            var row = channelsModel.get(i)
            if (row.page === pageId && row.kind === "remote")
                channelsModel.remove(i)
        }
        var anchor = -1
        for (var j = 0; j < channelsModel.count; j++) {
            if (channelsModel.get(j).page === pageId) {
                anchor = j
                break
            }
        }
        if (anchor >= 0) {
            channelsModel.setProperty(anchor, "page", 0)
            channelsModel.setProperty(anchor, "channel", homeSlot)
            channelsModel.setProperty(anchor, "kind", "local")
            _resortRow(anchor)
        }
        _setGateway(pageId, null)
        pagesModel.remove(pageRow)
        currentPage = 0
        currentChannel = homeSlot
        _rebuildState()
        _degaussArmed = true
        activateCurrent()
    }

    // One client, one geometry: the grid of whichever channel is on the air,
    // told to every attachment and not only to the page holding it. There is
    // one rectangle of glass in the appliance and every channel is laid into
    // it, so an attachment's idea of its client is never its own; a gateway
    // told only while its page held the air would keep whatever size it was
    // last given across a window drag done on another page, and tmux sizes a
    // session to the clients attached to it, so the stale number would not
    // stay private but would be drawn into that session's windows on the
    // user's return. A resize is rare and a machine is one line of protocol,
    // so the broadcast is the cheap end of the bargain.
    // QMLTermWidget's terminalSize is QSize(lines, columns), so the width of
    // that size is the number of rows and its height the number of columns;
    // tmux is told columns first, the way it says them back.
    function _publishClientSize() {
        if (terminalSize.width <= 0 || terminalSize.height <= 0)
            return
        for (var key in _gateways)
            _gateways[key].setClientSize(terminalSize.height, terminalSize.width)
    }

    onTerminalSizeChanged: _publishClientSize()

    // Each page's gateway speaks; the bank moves. One listener per page,
    // raised and dropped with the page itself, its pageId closed over, so
    // two attachments' notifications never cross. Remote titles belong to
    // tmux: they are set here from its notifications, never by the delegate.
    Instantiator {
        model: pagesModel

        delegate: Connections {
            readonly property int page: model.pageId

            target: model.kind === "tmux"
                ? (channelsRoot._gateways[model.pageId] ?? null) : null

            function onHostChanged() {
                channelsRoot._hostChanged(page)
            }
            function onWindowAdded(windowId, paneId, name) {
                channelsRoot.openRemoteChannel(page, windowId, paneId, name)
            }
            function onWindowRenamed(windowId, name) {
                var channel = channelsRoot._channelOfWindow(page, windowId)
                if (channel > 0)
                    channelsRoot.setTitle(page, channel, name)
            }
            function onWindowClosed(windowId) {
                var channel = channelsRoot._channelOfWindow(page, windowId)
                if (channel > 0)
                    channelsRoot._removeRow(page, channel)
            }
            function onDetached() {
                channelsRoot._collapsePage(page)
            }
        }
    }

    // The host can resolve after the handshake: the page and its anchor's
    // title follow it.
    function _hostChanged(page) {
        var gateway = _gateways[page]
        var pageRow = _pageRowOf(page)
        if (!gateway || pageRow < 0)
            return
        pagesModel.setProperty(pageRow, "host", gateway.host)
        for (var i = 0; i < channelsModel.count; i++) {
            var row = channelsModel.get(i)
            if (row.page === page && row.kind === "anchor")
                channelsModel.setProperty(i, "title", "tmux -CC # @" + gateway.host)
        }
        _rebuildState()
    }

    function _channelOfWindow(page, windowId) {
        for (var i = 0; i < channelsModel.count; i++) {
            var row = channelsModel.get(i)
            if (row.page === page && row.kind === "remote" && row.windowId === windowId)
                return row.channel
        }
        return 0
    }

    function activateCurrent() {
        var item = channelRepeater.itemAt(currentIndex)
        if (item)
            item.activate()
    }

    Component.onCompleted: {
        openChannel(0, 1)
        _degaussArmed = true
    }

    // Turning the knob makes the tube flinch, and turning it to another
    // machine's page no less. Re-selecting the current channel never reaches
    // here: nothing changes.
    onCurrentChannelChanged: {
        if (_degaussArmed)
            degauss.restart()
    }
    onCurrentPageChanged: {
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
                    property int channelPage: model.page
                    property string rowKind: model.kind
                    property bool shouldHaveFocus: terminalWindow.active && StackLayout.isCurrentItem
                    isActive: StackLayout.isCurrentItem
                    channelKind: model.kind
                    tmuxWindowId: model.windowId
                    tmuxPaneId: model.paneId
                    remoteGateway: model.kind === "remote"
                        ? (channelsRoot._gateways[model.page] ?? null) : null
                    onShouldHaveFocusChanged: {
                        if (shouldHaveFocus) {
                            activate()
                        }
                    }
                    // A local shell's title is its own; abroad the model owns
                    // it, and a remote channel's title is tmux's to give.
                    onTitleChanged: {
                        if (rowKind === "local")
                            channelsRoot.setTitle(channelPage, channelNumber, title)
                    }
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    onSessionFinished: channelsRoot.sessionDied(channelPage, channelNumber)
                    onTmuxGateway: (gateway) => channelsRoot.attachGateway(channelPage, channelNumber, gateway)
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
