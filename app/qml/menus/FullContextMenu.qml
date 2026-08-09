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
import QtQuick.Controls 2.3

Menu {
    id: contextmenu
    MenuItem {
        action: copyAction
    }
    MenuItem {
        action: pasteAction
    }
    MenuItem {
        action: showsettingsAction
    }

    Menu {
        title: qsTr("File")
        MenuItem {
            action: newWindowAction
        }
        MenuItem {
            action: newChannelAction
        }
        MenuItem {
            action: newLocalChannelAction
        }
        // Off the menu entirely with no session attached: the host they name
        // does not exist then.
        MenuItem {
            action: newRemoteChannelAction
            visible: newRemoteChannelAction.enabled
            height: visible ? implicitHeight : 0
        }
        MenuItem {
            action: detachAction
            visible: detachAction.enabled
            height: visible ? implicitHeight : 0
        }
        MenuSeparator {}
        MenuItem {
            action: quitAction
        }
    }
    Menu {
        title: qsTr("Edit")
        MenuItem {
            action: copyAction
        }
        MenuItem {
            action: pasteAction
        }
        MenuSeparator {}
        MenuItem {
            action: showsettingsAction
        }
    }
    Menu {
        id: viewMenu
        title: qsTr("View")
        MenuItem {
            action: fullscreenAction
            visible: fullscreenAction.enabled
        }
        // Built only for a profile that has a bank: a menu item hidden in
        // place still holds its row, and a plain profile would carry the gap.
        // Row 1, under the fullscreen item, which stands here in the file and
        // is built before this one.
        Instantiator {
            model: appSettings.channels ? 1 : 0
            delegate: MenuItem { action: channelBankAction }
            onObjectAdded: (index, object) => viewMenu.insertItem(index + 1, object)
            onObjectRemoved: (index, object) => viewMenu.removeItem(object)
        }
        MenuItem {
            action: zoomIn
        }
        MenuItem {
            action: zoomOut
        }
    }
    Menu {
        id: profilesMenu
        title: qsTr("Profiles")
        Instantiator {
            model: appSettings.profilesList
            delegate: MenuItem {
                text: model.text
                onTriggered: {
                    appSettings.loadProfileString(obj_string)
                }
            }
            onObjectAdded: function(index, object) { profilesMenu.insertItem(index, object) }
            onObjectRemoved: function(object) { profilesMenu.removeItem(object) }
        }
    }
    Menu {
        title: qsTr("Help")
        MenuItem {
            action: showAboutAction
        }
    }
}
