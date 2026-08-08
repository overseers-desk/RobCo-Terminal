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
import QtQuick.Window
import QtQuick.Controls

import "menus"

ApplicationWindow {
    id: terminalWindow

    width: 1024
    height: 768

    // Show the window once it is ready.
    Component.onCompleted: {
        visible = true
    }

    // The least screen the well is ever given; the seam's travel stops here too.
    readonly property int crtMinimumWidth: 320

    minimumWidth: channelBank.implicitWidth + crtMinimumWidth
    minimumHeight: 240

    visible: false

    property bool fullscreen: false
    onFullscreenChanged: visibility = (fullscreen ? Window.FullScreen : Window.Windowed)

    menuBar: WindowMenu { }

    // A window manager free to ignore minimumWidth can hand the region no size
    // at all, and every consumer of the scale divides by it.
    property real normalizedScreenScale: 1024 / Math.max(1, 0.5 * crtRegion.width + 0.5 * crtRegion.height)

    color: "#00000000"

    title: terminalChannels.currentTitle

    Action {
        id: fullscreenAction
        text: qsTr("Fullscreen")
        enabled: !appSettings.isMacOS
        shortcut: StandardKey.FullScreen
        onTriggered: fullscreen = !fullscreen
        checkable: true
        checked: fullscreen
    }
    Action {
        id: newWindowAction
        text: qsTr("New Window")
        shortcut: appSettings.isMacOS ? "Meta+N" : "Ctrl+Shift+N"
        onTriggered: appRoot.createWindow()
    }
    Action {
        id: quitAction
        text: qsTr("Quit")
        shortcut: appSettings.isMacOS ? StandardKey.Close : "Ctrl+Shift+Q"
        onTriggered: terminalWindow.close()
    }
    Action {
        id: showsettingsAction
        text: qsTr("Settings")
        onTriggered: {
            settingsWindow.show()
            settingsWindow.requestActivate()
            settingsWindow.raise()
        }
    }
    Action {
        id: copyAction
        text: qsTr("Copy")
        shortcut: appSettings.isMacOS ? StandardKey.Copy : "Ctrl+Shift+C"
    }
    Action {
        id: pasteAction
        text: qsTr("Paste")
        shortcut: appSettings.isMacOS ? StandardKey.Paste : "Ctrl+Shift+V"
    }
    Action {
        id: zoomIn
        text: qsTr("Zoom In")
        shortcut: StandardKey.ZoomIn
        onTriggered: appSettings.incrementScaling()
    }
    Action {
        id: zoomOut
        text: qsTr("Zoom Out")
        shortcut: StandardKey.ZoomOut
        onTriggered: appSettings.decrementScaling()
    }
    Action {
        id: showAboutAction
        text: qsTr("About")
        onTriggered: {
            aboutDialog.show()
            aboutDialog.requestActivate()
            aboutDialog.raise()
        }
    }
    Action {
        id: newChannelAction
        text: qsTr("New Channel")
        shortcut: appSettings.isMacOS ? "Meta+T" : "Ctrl+Shift+T"
        // Follows the focus: on a remote channel this is a tmux window.
        onTriggered: terminalChannels.newChannel()
    }
    // The two ends of the fork the shortcut chooses between, each nameable on
    // its own, plus the way back off the session. The remote pair only exists
    // while a gateway does.
    Action {
        id: newLocalChannelAction
        text: qsTr("New local window")
        onTriggered: terminalChannels.openFirstFree()
    }
    Action {
        id: newRemoteChannelAction
        text: qsTr("New window on %1").arg(terminalChannels.tmuxHost)
        enabled: terminalChannels.tmuxGateway !== null
        onTriggered: terminalChannels.newRemoteChannel()
    }
    Action {
        id: detachAction
        text: qsTr("Detach from %1").arg(terminalChannels.tmuxHost)
        enabled: terminalChannels.tmuxGateway !== null
        onTriggered: terminalChannels.tmuxGateway.detach()
    }
    Action {
        id: closeChannelAction
        text: qsTr("Close Channel")
        shortcut: appSettings.isMacOS ? "Meta+W" : "Ctrl+Shift+W"
        onTriggered: terminalChannels.closeChannel(terminalChannels.currentChannel)
    }
    // The chord names a key on the page the bank is showing, as the numerals
    // engraved beside those keys read; the bank turns it into a slot.
    ChannelChordInput {
        id: channelChordInput
        onSelectSlot: function (slot) {
            terminalChannels.selectChannel(channelBank.absoluteSlot(slot))
        }
        onStoreToSlot: function (slot) {
            terminalChannels.moveCurrentTo(channelBank.absoluteSlot(slot))
        }
        onCycleOpen: function (direction) {
            terminalChannels.cycleOpen(direction)
        }
        slotPrefixExists: channelBank.slotPrefixExists

        // A page flip re-labels the numerals, so digits typed against the old
        // page are abandoned, never committed against the new one.
        Connections {
            target: channelBank
            function onPageIndexChanged() {
                channelChordInput.cancel()
            }
        }
    }
    Shortcut {
        sequence: appSettings.isMacOS ? "Meta+PgUp" : "Alt+PgUp"
        context: Qt.WindowShortcut
        onActivated: channelBank.step(-1)
    }
    Shortcut {
        sequence: appSettings.isMacOS ? "Meta+PgDown" : "Alt+PgDown"
        context: Qt.WindowShortcut
        onActivated: channelBank.step(1)
    }
    // The bank's plastic, continuing the frame's moulding leftwards. Nothing
    // stands between the frame and the window on the other three sides: the
    // frame is the outermost thing there, as it is upstream.
    Loader {
        id: chassisLoader

        anchors.fill: channelBank
        source: appSettings.shellUrl("Chassis")

        // The chassis lives in a file of its own, where this window's ids do
        // not reach; the frame's region is handed over explicitly.
        Binding {
            target: chassisLoader.item
            property: "frameRegion"
            value: crtRegion
        }
    }
    ChannelBank {
        id: channelBank
        anchors {
            left: parent.left
            top: parent.top
            bottom: parent.bottom
        }
    }
    Item {
        id: crtRegion
        anchors {
            left: channelBank.right
            right: parent.right
            top: parent.top
            bottom: parent.bottom
        }
        TerminalChannels {
            id: terminalChannels
            anchors.fill: parent
        }
    }
    // The seam where the bank's plastic meets the screen well. Nothing is
    // drawn: the cursor's change of shape is the only tell. A drag re-fits
    // the LED strips at the character count nearest the hand, so the seam
    // travels in whole-character steps.
    MouseArea {
        id: seam

        // The strips run to the boundary, so the grab strip leans into the
        // moulding: it takes 3 px of the LED windows' clickable right edge
        // and 7 px of inert plastic.
        x: channelBank.width - 3
        width: 10
        anchors {
            top: parent.top
            bottom: parent.bottom
        }
        z: 2
        cursorShape: Qt.SplitHCursor
        acceptedButtons: Qt.LeftButton
        onPositionChanged: function (mouse) {
            if (!pressed)
                return
            var windowX = mapToItem(null, mouse.x, 0).x
            var chars = Math.min(channelBank.charactersForWidth(windowX),
                                 channelBank.charactersForWidth(terminalWindow.width - terminalWindow.crtMinimumWidth))
            chars = Math.max(channelBank.minUnits, chars)
            if (chars !== appSettings.ledCharacters)
                appSettings.ledCharacters = chars
        }
    }
    Loader {
        anchors.centerIn: crtRegion
        active: appSettings.showTerminalSize
        sourceComponent: SizeOverlay {
            z: 3
            terminalSize: terminalChannels.terminalSize
        }
    }
    onClosing: {
        appRoot.closeWindow(terminalWindow)
    }
}
