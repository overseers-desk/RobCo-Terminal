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

    minimumWidth: channelBank.implicitWidth + 320
    minimumHeight: 240

    // The plastic left standing between the screen well and the window edges.
    readonly property int chassisMargin: 16

    visible: false

    property bool fullscreen: false
    onFullscreenChanged: visibility = (fullscreen ? Window.FullScreen : Window.Windowed)

    menuBar: WindowMenu { }

    property real normalizedWindowScale: 1024 / ((0.5 * crtRegion.width + 0.5 * crtRegion.height))

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
        onTriggered: terminalChannels.openFirstFree()
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
    ChannelChassis {
        anchors.fill: parent
        well: crtRegion
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
            rightMargin: terminalWindow.chassisMargin
            topMargin: terminalWindow.chassisMargin
            bottomMargin: terminalWindow.chassisMargin
        }
        TerminalChannels {
            id: terminalChannels
            anchors.fill: parent
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
