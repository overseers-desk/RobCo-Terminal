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

    // The scale factor of the screen this window is on, kept in a property of our
    // own. Screen.devicePixelRatio reads the current value but carries no working
    // change notification, so a binding on it keeps the ratio the window was built
    // with and every size derived from it is left behind when the scale changes.
    // The screen's other measures do notify, and any change of scale or of screen
    // moves at least one of them, so they are the cue to read the ratio again.
    property real screenDevicePixelRatio: 1
    readonly property string screenState:
        Screen.name + " " + Screen.width + "x" + Screen.height + "@" + Screen.pixelDensity
    onScreenStateChanged: screenDevicePixelRatio = Screen.devicePixelRatio

    // Show the window once it is ready.
    Component.onCompleted: {
        screenDevicePixelRatio = Screen.devicePixelRatio
        visible = true
    }

    // The least screen the well is ever given; the seam's travel stops here too.
    readonly property int crtMinimumWidth: 320

    // Whether the bank stands: the profile has to carry the channel function
    // and the user has to have left the column up. The one condition the
    // bank, its chassis and their seam are all gated on.
    readonly property bool bankStanding: appSettings.channels && appSettings.channelBankShown

    // The bank's footprint, zero when no bank stands.
    readonly property int bankWidth: channelBankLoader.item ? channelBankLoader.item.implicitWidth : 0

    minimumWidth: bankWidth + crtMinimumWidth
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
        // Explicit F11: the GNOME platform theme maps StandardKey.FullScreen
        // to Ctrl+F11, leaving plain F11 unbound.
        shortcut: "F11"
        onTriggered: fullscreen = !fullscreen
        checkable: true
        checked: fullscreen
    }
    Action {
        id: channelBankAction
        text: qsTr("Channel Bank")
        // Only an appliance has a bank to put away; the menus hide the item
        // for every other profile.
        enabled: appSettings.channels
        onTriggered: appSettings.channelBankShown = !appSettings.channelBankShown
        checkable: true
        checked: appSettings.channelBankShown
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
        text: appSettings.channels ? qsTr("New Channel") : qsTr("New Tab")
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
        text: appSettings.channels ? qsTr("Close Channel") : qsTr("Close Tab")
        shortcut: appSettings.isMacOS ? "Meta+W" : "Ctrl+Shift+W"
        onTriggered: terminalChannels.closeChannel(terminalChannels.currentChannel)
    }
    Shortcut {
        sequence: "Ctrl+PgUp"
        context: Qt.WindowShortcut
        onActivated: terminalChannels.cycleOpen(-1)
    }
    Shortcut {
        sequence: "Ctrl+PgDown"
        context: Qt.WindowShortcut
        onActivated: terminalChannels.cycleOpen(1)
    }
    // The screen well stands first in the file: the selector loaders below
    // resolve terminalChannels by id while they complete, so the store has to
    // exist before either of them builds.
    Item {
        id: crtRegion
        anchors {
            left: bankColumn.right
            right: parent.right
            top: tabStripLoader.bottom
            bottom: parent.bottom
        }
        TerminalChannels {
            id: terminalChannels
            anchors.fill: parent
        }
    }
    // The channel store has two faces, one standing at a time: a profile with
    // the channel function raises the bank in its chassis, any other gets the
    // tab strip. Loaders rather than hidden items: an unraised face loads no
    // metrics, holds no width, and owns no shortcuts. An appliance whose bank
    // the user has put away shows neither face and gives the well the whole
    // window: the strip belongs to the plain profiles, never a stand-in.
    Loader {
        id: tabStripLoader

        active: !appSettings.channels
        anchors {
            left: bankColumn.right
            right: parent.right
            top: parent.top
        }
        height: item ? item.implicitHeight : 0
        source: "TerminalTabStrip.qml"
    }
    // The appliance's left column, chassis and bank together, held in one
    // cached texture. The tube's effects ask the window to repaint some
    // twenty times a second and a repaint redraws every batch standing in it,
    // this column's included, though nothing here has moved: the casting is a
    // still casting and a lamp only changes when a channel does. Cached, a
    // repaint composites one quad instead of running the metal shader over
    // the column and a lamp pass over every row. The column pays again only
    // when something in it changes.
    Item {
        id: bankColumn

        anchors {
            left: parent.left
            top: parent.top
            bottom: parent.bottom
        }
        width: terminalWindow.bankWidth
        layer.enabled: terminalWindow.bankStanding

        // The bank's plastic, continuing the frame's moulding leftwards.
        // Nothing stands between the frame and the window on the other three
        // sides: the frame is the outermost thing there, as it is upstream.
        Loader {
            id: chassisLoader

            active: terminalWindow.bankStanding
            anchors.fill: parent
            source: appSettings.shellUrl("Chassis")

            // The chassis lives in a file of its own, where this window's ids
            // do not reach; the frame's region is handed over explicitly.
            Binding {
                target: chassisLoader.item
                property: "frameRegion"
                value: crtRegion
            }
            // The tape label names whose session the channels belong to, so
            // it exists only while one is attached and goes with it. The
            // host is handed over as it is stored; the tape uppercases what
            // it stamps, that being the wheel's business and not the model's.
            Binding {
                target: chassisLoader.item
                property: "tapeText"
                value: terminalChannels.tmuxGateway ? terminalChannels.tmuxHost : ""
            }
        }
        Loader {
            id: channelBankLoader

            active: terminalWindow.bankStanding
            anchors.fill: parent
            source: "ChannelBank.qml"
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
        x: terminalWindow.bankWidth - 3
        width: 10
        anchors {
            top: parent.top
            bottom: parent.bottom
        }
        z: 2
        visible: terminalWindow.bankStanding
        enabled: visible
        cursorShape: Qt.SplitHCursor
        acceptedButtons: Qt.LeftButton
        onPositionChanged: function (mouse) {
            var bank = channelBankLoader.item
            if (!pressed || !bank)
                return
            var windowX = mapToItem(null, mouse.x, 0).x
            var chars = Math.min(bank.charactersForWidth(windowX),
                                 bank.charactersForWidth(terminalWindow.width - terminalWindow.crtMinimumWidth))
            chars = Math.max(bank.minUnits, chars)
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
