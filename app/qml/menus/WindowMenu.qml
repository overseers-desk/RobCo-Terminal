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
        MenuItem { action: newLocalChannelAction }
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

        Instantiator {
            model: !appSettings.isMacOS ? 1 : 0
            delegate: MenuItem { action: fullscreenAction }
            onObjectAdded: (index, object) => viewMenu.insertItem(index, object)
            onObjectRemoved: (index, object) => viewMenu.removeItem(object)
        }
        MenuItem { action: chassisAction }
        MenuItem { action: zoomIn }
        MenuItem { action: zoomOut }
    }
    Menu {
        id: screensMenu
        title: qsTr("Screens")
        Instantiator {
            model: appSettings.screensList
            delegate: MenuItem {
                text: model.text
                onTriggered: {
                    appSettings.loadScreenString(obj_string)
                }
            }
            onObjectAdded: function(index, object) { screensMenu.insertItem(index, object) }
            onObjectRemoved: function(index, object) { screensMenu.removeItem(object) }
        }
    }
    Menu {
        id: chassisMenu
        title: qsTr("Chassis")
        Instantiator {
            model: appSettings.chassisList
            delegate: MenuItem {
                text: model.text
                onTriggered: {
                    appSettings.loadChassisString(obj_string)
                }
            }
            onObjectAdded: function(index, object) { chassisMenu.insertItem(index, object) }
            onObjectRemoved: function(index, object) { chassisMenu.removeItem(object) }
        }
    }
    Menu {
        id: profilesMenu
        title: qsTr("Profiles")
        // Greyed until the user has saved one. A submenu's own visible flag is
        // what opens and closes it as a popup, so standing-or-not is said with
        // enabled: the row keeps its place and tells the user the shelf is
        // there to be filled.
        enabled: appSettings.customProfilesList.count > 0
        Instantiator {
            model: appSettings.customProfilesList
            delegate: MenuItem {
                text: model.text
                onTriggered: {
                    appSettings.loadProfileString(obj_string)
                }
            }
            onObjectAdded: function(index, object) { profilesMenu.insertItem(index, object) }
            onObjectRemoved: function(index, object) { profilesMenu.removeItem(object) }
        }
    }
    Menu {
        title: qsTr("Help")
        MenuItem {
            action: showAboutAction
        }
    }
}
