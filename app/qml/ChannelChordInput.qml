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

// Chord input for channel slots: hold the modifier, type digits, release to
// commit. The host wires selectSlot/storeToSlot/cycleOpen and the
// slotPrefixExists predicate; this component knows nothing about the model.
Item {
    id: chordInput

    width: 0
    height: 0

    signal selectSlot(int slot)
    signal storeToSlot(int slot)
    signal cycleOpen(int direction)

    // True when some selectable slot has buf as a strict prefix; while true
    // the chord keeps waiting for more digits instead of committing.
    property var slotPrefixExists: function (buf) { return false }

    property string digitBuffer: ""
    property bool storeMode: false

    readonly property var hostWindow: Window.window

    function feedDigit(digit, store) {
        storeMode = store
        digitBuffer += digit
        if (slotPrefixExists(digitBuffer))
            commitTimer.restart()
        else
            commitChord()
    }

    function commitChord() {
        commitTimer.stop()
        if (digitBuffer === "")
            return
        var slot = digitBuffer === "0" ? 10 : parseInt(digitBuffer, 10)
        var store = storeMode
        digitBuffer = ""
        storeMode = false
        if (store)
            storeToSlot(slot)
        else
            selectSlot(slot)
    }

    // Commits the chord if the modifier release is never observed.
    Timer {
        id: commitTimer
        interval: 900
        onTriggered: chordInput.commitChord()
    }

    Connections {
        target: modifierWatcher
        function onChordModifierReleased() {
            chordInput.commitChord()
        }
    }

    Connections {
        target: chordInput.hostWindow
        function onActiveChanged() {
            if (!chordInput.hostWindow.active)
                chordInput.commitChord()
        }
    }

    Shortcut {
        sequence: appSettings.isMacOS ? "Meta+0" : "Alt+0"
        context: Qt.WindowShortcut
        autoRepeat: false
        onActivated: chordInput.feedDigit("0", false)
    }
    Shortcut {
        sequence: appSettings.isMacOS ? "Meta+1" : "Alt+1"
        context: Qt.WindowShortcut
        autoRepeat: false
        onActivated: chordInput.feedDigit("1", false)
    }
    Shortcut {
        sequence: appSettings.isMacOS ? "Meta+2" : "Alt+2"
        context: Qt.WindowShortcut
        autoRepeat: false
        onActivated: chordInput.feedDigit("2", false)
    }
    Shortcut {
        sequence: appSettings.isMacOS ? "Meta+3" : "Alt+3"
        context: Qt.WindowShortcut
        autoRepeat: false
        onActivated: chordInput.feedDigit("3", false)
    }
    Shortcut {
        sequence: appSettings.isMacOS ? "Meta+4" : "Alt+4"
        context: Qt.WindowShortcut
        autoRepeat: false
        onActivated: chordInput.feedDigit("4", false)
    }
    Shortcut {
        sequence: appSettings.isMacOS ? "Meta+5" : "Alt+5"
        context: Qt.WindowShortcut
        autoRepeat: false
        onActivated: chordInput.feedDigit("5", false)
    }
    Shortcut {
        sequence: appSettings.isMacOS ? "Meta+6" : "Alt+6"
        context: Qt.WindowShortcut
        autoRepeat: false
        onActivated: chordInput.feedDigit("6", false)
    }
    Shortcut {
        sequence: appSettings.isMacOS ? "Meta+7" : "Alt+7"
        context: Qt.WindowShortcut
        autoRepeat: false
        onActivated: chordInput.feedDigit("7", false)
    }
    Shortcut {
        sequence: appSettings.isMacOS ? "Meta+8" : "Alt+8"
        context: Qt.WindowShortcut
        autoRepeat: false
        onActivated: chordInput.feedDigit("8", false)
    }
    Shortcut {
        sequence: appSettings.isMacOS ? "Meta+9" : "Alt+9"
        context: Qt.WindowShortcut
        autoRepeat: false
        onActivated: chordInput.feedDigit("9", false)
    }
    Shortcut {
        sequence: appSettings.isMacOS ? "Meta+Shift+0" : "Alt+Shift+0"
        context: Qt.WindowShortcut
        autoRepeat: false
        onActivated: chordInput.feedDigit("0", true)
    }
    Shortcut {
        sequence: appSettings.isMacOS ? "Meta+Shift+1" : "Alt+Shift+1"
        context: Qt.WindowShortcut
        autoRepeat: false
        onActivated: chordInput.feedDigit("1", true)
    }
    Shortcut {
        sequence: appSettings.isMacOS ? "Meta+Shift+2" : "Alt+Shift+2"
        context: Qt.WindowShortcut
        autoRepeat: false
        onActivated: chordInput.feedDigit("2", true)
    }
    Shortcut {
        sequence: appSettings.isMacOS ? "Meta+Shift+3" : "Alt+Shift+3"
        context: Qt.WindowShortcut
        autoRepeat: false
        onActivated: chordInput.feedDigit("3", true)
    }
    Shortcut {
        sequence: appSettings.isMacOS ? "Meta+Shift+4" : "Alt+Shift+4"
        context: Qt.WindowShortcut
        autoRepeat: false
        onActivated: chordInput.feedDigit("4", true)
    }
    Shortcut {
        sequence: appSettings.isMacOS ? "Meta+Shift+5" : "Alt+Shift+5"
        context: Qt.WindowShortcut
        autoRepeat: false
        onActivated: chordInput.feedDigit("5", true)
    }
    Shortcut {
        sequence: appSettings.isMacOS ? "Meta+Shift+6" : "Alt+Shift+6"
        context: Qt.WindowShortcut
        autoRepeat: false
        onActivated: chordInput.feedDigit("6", true)
    }
    Shortcut {
        sequence: appSettings.isMacOS ? "Meta+Shift+7" : "Alt+Shift+7"
        context: Qt.WindowShortcut
        autoRepeat: false
        onActivated: chordInput.feedDigit("7", true)
    }
    Shortcut {
        sequence: appSettings.isMacOS ? "Meta+Shift+8" : "Alt+Shift+8"
        context: Qt.WindowShortcut
        autoRepeat: false
        onActivated: chordInput.feedDigit("8", true)
    }
    Shortcut {
        sequence: appSettings.isMacOS ? "Meta+Shift+9" : "Alt+Shift+9"
        context: Qt.WindowShortcut
        autoRepeat: false
        onActivated: chordInput.feedDigit("9", true)
    }
    Shortcut {
        sequence: "Ctrl+PgUp"
        context: Qt.WindowShortcut
        onActivated: chordInput.cycleOpen(-1)
    }
    Shortcut {
        sequence: "Ctrl+PgDown"
        context: Qt.WindowShortcut
        onActivated: chordInput.cycleOpen(1)
    }
}
