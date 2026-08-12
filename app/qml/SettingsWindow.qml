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

import QtQuick 2.2
import QtQuick.Controls 2.1
import QtQuick.Window 2.1
import QtQuick.Layouts 1.3
import QtQuick.Dialogs

ApplicationWindow {
    readonly property real tabButtonPadding: 10

    // One accent governs the whole window: the ground under the row on air,
    // the Modified badge and the link across to the Effects tab all take
    // their colour from here, and near-black text sits on that ground. The
    // pages hold it as a property the window fills in, so the colour is
    // written once.
    readonly property color accentColor: "#e0532c"
    readonly property color accentTextColor: "#141414"

    id: settings_window
    title: qsTr("Settings")
    width: 640
    height: 560

    Item {
        anchors { fill: parent; }

        TabBar {
            id: bar
            anchors { left: parent.left; right: parent.right; top: parent.top; }
            TabButton {
                padding: tabButtonPadding
                text: qsTr("General")
            }
            TabButton {
                padding: tabButtonPadding
                text: qsTr("Terminal")
            }
            TabButton {
                id: effectsTabButton
                padding: tabButtonPadding
                text: qsTr("Effects")
            }
            TabButton {
                padding: tabButtonPadding
                text: qsTr("Advanced")
            }
            TabButton {
                padding: tabButtonPadding
                text: qsTr("Channels")
            }
            TabButton {
                padding: tabButtonPadding
                text: qsTr("Profiles")
            }
        }

        StackLayout {
            anchors {
                top: bar.bottom
                left: parent.left
                right: parent.right
                bottom: statusBar.top
                margins: 16
            }

            currentIndex: bar.currentIndex

            SettingsGeneralTab {
                accentColor: settings_window.accentColor
                accentTextColor: settings_window.accentTextColor
                // The page names the tab it sends the user to; the bar reads
                // the button's own place in it.
                effectsTabIndex: effectsTabButton.TabBar.index
                onRequestTab: (index) => bar.currentIndex = index
            }
            SettingsTerminalTab { }
            SettingsEffectsTab { }
            SettingsAdvancedTab { }
            SettingsChannelsTab { }
            SettingsProfilesTab {
                accentColor: settings_window.accentColor
                accentTextColor: settings_window.accentTextColor
                onRequestSaveCurrent: statusBar.beginNaming()
            }
        }

        // The look on air, and the way to keep it. The bar stands outside the
        // tab stack, so whichever page is open the user reads what is running
        // and can name it as a profile from there.
        Rectangle {
            id: statusBar
            anchors { left: parent.left; right: parent.right; bottom: parent.bottom; }
            height: statusRow.implicitHeight + 16
            color: Qt.darker(palette.window, 1.1)

            // While a name is being taken the field holds the bar's right-hand
            // corner; the button returns once the profile is kept.
            property bool naming: false

            function beginNaming() {
                naming = true
                nameField.text = ""
                nameField.forceActiveFocus()
            }

            function commitName() {
                var name = nameField.text.trim()
                appSettings.saveCurrentAsProfile(name !== "" ? name : qsTr("My profile"))
                naming = false
            }

            Rectangle {
                anchors { left: parent.left; right: parent.right; top: parent.top; }
                height: 1
                color: Qt.rgba(palette.text.r, palette.text.g, palette.text.b, 0.2)
            }

            RowLayout {
                id: statusRow
                anchors { fill: parent; leftMargin: 8; rightMargin: 8 }
                spacing: 8

                Label {
                    Layout.maximumWidth: statusBar.width * 0.5
                    text: appSettings.screenName + " · " + appSettings.chassisName
                    elide: Text.ElideRight
                }
                Rectangle {
                    visible: appSettings.modified
                    implicitWidth: modifiedLabel.implicitWidth + 10
                    implicitHeight: modifiedLabel.implicitHeight + 4
                    radius: 3
                    color: Qt.rgba(accentColor.r, accentColor.g, accentColor.b, 0.18)
                    border.width: 1
                    border.color: Qt.rgba(accentColor.r, accentColor.g, accentColor.b, 0.6)
                    Label {
                        id: modifiedLabel
                        anchors.centerIn: parent
                        text: qsTr("Modified")
                        color: accentColor
                        font.pointSize: Qt.application.font.pointSize * 0.85
                    }
                }
                Item {
                    Layout.fillWidth: true
                }
                Button {
                    visible: !statusBar.naming
                    text: qsTr("Save as profile…")
                    onClicked: statusBar.beginNaming()
                }
                TextField {
                    id: nameField
                    visible: statusBar.naming
                    implicitWidth: 160
                    placeholderText: qsTr("Profile name")
                    onAccepted: statusBar.commitName()
                }
                Button {
                    visible: statusBar.naming
                    text: qsTr("Save")
                    onClicked: statusBar.commitName()
                }
            }
        }
    }
}
