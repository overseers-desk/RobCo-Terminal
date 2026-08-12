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
import QtQuick.Controls
import QtQuick.Layouts

// The dense face of the sparse channel space: one tab per open session of
// the page on the air, in slot order, none for the dark slots. Closing a tab
// darkens its slot; the others keep their numbers. The strip only stands
// when there is a choice to make; a single session gets the whole window.
Item {
    id: strip

    readonly property int innerPadding: 6
    readonly property bool tabsShown: terminalChannels.pageChannels.count > 1

    implicitHeight: tabsShown ? tabRow.implicitHeight : 0

    // The bar writes its own currentIndex on a click, so a binding would go
    // stale at the first tap; the sync is owned by hand, the inequality test
    // breaking the loop. The bar's index is a position among the tabs on
    // show, found from the page list rather than taken from the store's row
    // index, which counts every page's rows.
    function syncCurrent() {
        var air = terminalChannels.pageChannels
        for (var i = 0; i < air.count; i++) {
            if (air.get(i).channel === terminalChannels.currentChannel) {
                if (tabBar.currentIndex !== i)
                    tabBar.currentIndex = i
                return
            }
        }
    }

    Connections {
        target: terminalChannels
        function onCurrentIndexChanged() {
            strip.syncCurrent()
        }
    }

    Component.onCompleted: syncCurrent()

    // Positional jumps: the Nth key is the Nth tab on show, the strip's own
    // reading of the rows. The slot arithmetic stays the bank's.
    Instantiator {
        model: 9
        delegate: Shortcut {
            sequence: (appSettings.isMacOS ? "Meta+" : "Alt+") + (index + 1)
            context: Qt.WindowShortcut
            autoRepeat: false
            onActivated: terminalChannels.selectAt(index)
        }
    }

    Rectangle {
        id: tabRow

        anchors.fill: parent
        implicitHeight: rowLayout.implicitHeight
        color: palette.window
        visible: strip.tabsShown

        RowLayout {
            id: rowLayout
            anchors.fill: parent
            spacing: 0

            TabBar {
                id: tabBar
                Layout.fillWidth: true
                Layout.fillHeight: true
                focusPolicy: Qt.NoFocus

                Repeater {
                    model: terminalChannels.pageChannels
                    TabButton {
                        id: tabButton

                        focusPolicy: Qt.NoFocus
                        onClicked: terminalChannels.selectChannel(terminalChannels.currentPage, model.channel)
                        // The slot number the bank would engrave beside this
                        // session, still reachable from the strip.
                        ToolTip.visible: hovered
                        ToolTip.delay: 500
                        ToolTip.text: qsTr("Channel %1").arg(model.channel)

                        contentItem: RowLayout {
                            anchors.fill: parent
                            anchors { leftMargin: strip.innerPadding; rightMargin: strip.innerPadding }
                            spacing: strip.innerPadding

                            Label {
                                text: model.title
                                elide: Text.ElideRight
                                Layout.fillWidth: true
                                Layout.alignment: Qt.AlignVCenter
                            }

                            ToolButton {
                                text: "×"
                                focusPolicy: Qt.NoFocus
                                padding: strip.innerPadding
                                Layout.alignment: Qt.AlignVCenter
                                onClicked: terminalChannels.closeChannel(terminalChannels.currentPage, model.channel)
                            }
                        }
                    }
                }
            }

            ToolButton {
                id: addTabButton
                text: "+"
                focusPolicy: Qt.NoFocus
                Layout.fillHeight: true
                padding: strip.innerPadding
                Layout.alignment: Qt.AlignVCenter
                // Dark slots may all be taken: a full house disables the key
                // instead of swallowing the press. The new tab goes to the
                // page on show the way Ctrl+Shift+T does: another window on
                // an attachment's session, a local shell on home.
                enabled: terminalChannels.firstFreeChannel > 0
                onClicked: terminalChannels.newChannel()
            }
        }
    }
}
