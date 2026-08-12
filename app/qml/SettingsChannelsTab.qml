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
import QtQuick.Layouts 1.1
import QtQml 2.0

import "Components"

// The LED strip's font and width are the user's own settings, not a
// profile's, so this page's choices carry across every screen and chassis:
// values set here apply the moment either is picked.
ColumnLayout {
    GroupBox {
        title: qsTr("LED Strip")
        Layout.fillWidth: true
        Layout.fillHeight: true
        padding: appSettings.defaultMargin
        GridLayout {
            anchors.fill: parent
            columns: 2
            Label {
                text: qsTr("LED Font")
            }
            ComboBox {
                id: ledFontChanger
                Layout.fillWidth: true
                model: appSettings.lowResolutionFontList
                textRole: "text"
                onActivated: {
                    appSettings.ledFontName = model.get(currentIndex).name
                }
                function updateIndex() {
                    for (var i = 0; i < appSettings.lowResolutionFontList.count; i++) {
                        var font = appSettings.lowResolutionFontList.get(i)
                        if (font.name === appSettings.ledFontName) {
                            currentIndex = i
                            return
                        }
                    }
                    currentIndex = 0
                }
                Connections {
                    target: appSettings
                    function onLedFontNameChanged() {
                        ledFontChanger.updateIndex()
                    }
                }
                Component.onCompleted: updateIndex()
            }
            Label {
                text: qsTr("Characters per strip")
            }
            SpinBox {
                id: ledCharactersChanger
                Layout.fillWidth: true
                from: appSettings.minLedCharacters
                // The widget demands a ceiling; this one sits beyond any
                // window's reach. The real bound is the drag's: the screen
                // well keeps its minimum width.
                to: 9999
                value: appSettings.ledCharacters
                onValueModified: appSettings.ledCharacters = value
                Connections {
                    target: appSettings
                    function onLedCharactersChanged() {
                        ledCharactersChanger.value = appSettings.ledCharacters
                    }
                }
            }
        }
    }
    Loader {
        Layout.alignment: Qt.AlignHCenter
        source: appSettings.displayUrl("Display")
        // The window at full power: the setting is judged against a window in
        // use, not one sitting by at part power.
        onLoaded: { item.text = "channel preview"; item.bright = true }
    }
}
