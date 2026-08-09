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

MenuBar {
    id: defaultMenuBar
    visible: appSettings.isMacOS || appSettings.showMenubar

    Menu {
        title: qsTr("File")
        MenuItem { action: newWindowAction }
        MenuItem { action: newChannelAction }
        MenuItem { action: closeChannelAction }
        MenuSeparator { }
        MenuItem { action: quitAction }
    }
    Menu {
        title: qsTr("Edit")
        MenuItem { action: copyAction }
        MenuItem { action: pasteAction }
        MenuSeparator {}
        MenuItem { action: showsettingsAction }
    }
    Menu {
        id: viewMenu
        title: qsTr("View")

        // Which row an action's item stands on, the end of the menu when
        // none carries it.
        function rowOf(what) {
            for (var i = 0; i < count; i++)
                if (itemAt(i) && itemAt(i).action === what)
                    return i
            return count
        }

        Instantiator {
            model: !appSettings.isMacOS ? 1 : 0
            delegate: MenuItem { action: fullscreenAction }
            onObjectAdded: (index, object) => viewMenu.insertItem(index, object)
            onObjectRemoved: (index, object) => viewMenu.removeItem(object)
        }
        // Built only for a profile that has a bank: a menu item hidden in
        // place still holds its row, and a plain profile would carry the
        // gap. The row is read off Zoom In rather than counted from the top:
        // the two builders complete in no fixed order, and a fixed index
        // lands the toggle between the zooms whenever this one goes first.
        Instantiator {
            model: appSettings.channels ? 1 : 0
            delegate: MenuItem { action: channelBankAction }
            onObjectAdded: (index, object) => viewMenu.insertItem(viewMenu.rowOf(zoomIn) + index, object)
            onObjectRemoved: (index, object) => viewMenu.removeItem(object)
        }
        MenuItem { action: zoomIn }
        MenuItem { action: zoomOut }
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
