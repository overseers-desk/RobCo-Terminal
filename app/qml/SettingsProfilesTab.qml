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
import QtQuick.Controls 2.4
import QtQuick.Layouts 1.1
import QtQuick.Dialogs

// Everything the user's own saved looks need: the roster of them, the buttons
// that keep, fetch and drop one, and the two that carry a profile in and out
// as a file.
ColumnLayout {
    // The window fills these in; the accent lives in one place there.
    property color accentColor
    property color accentTextColor

    // The naming field stands in the window's status bar, which this page can
    // see, so keeping the current look is asked of the window rather than done
    // here.
    signal requestSaveCurrent()

    readonly property real smallPointSize: Qt.application.font.pointSize * 0.85
    readonly property color frameColor: Qt.rgba(palette.text.r, palette.text.g,
                                                palette.text.b, 0.25)
    readonly property color dimTextColor: Qt.rgba(palette.text.r, palette.text.g,
                                                  palette.text.b, 0.6)

    id: profilesTab
    spacing: appSettings.defaultMargin / 2

    Label {
        Layout.fillWidth: true
        text: qsTr("A profile saves your whole setup (the screen, the chassis, and every effect adjustment) under one name, so you can switch between setups or share them as files.")
        color: dimTextColor
        wrapMode: Text.WordWrap
    }

    RowLayout {
        Layout.fillWidth: true
        Layout.fillHeight: true
        spacing: appSettings.defaultMargin / 2

        Rectangle {
            Layout.fillWidth: true
            Layout.fillHeight: true
            color: palette.base
            radius: 3
            border.width: 1
            border.color: frameColor

            ListView {
                id: profilesView
                anchors { fill: parent; margins: 2 }
                model: appSettings.customProfilesList
                clip: true
                currentIndex: -1
                boundsBehavior: Flickable.StopAtBounds
                ScrollBar.vertical: ScrollBar { }
                delegate: Rectangle {
                    readonly property bool selected: profilesView.currentIndex === index

                    width: profilesView.width
                    height: profileLabel.implicitHeight + 10
                    color: selected
                           ? accentColor
                           : (profileArea.containsMouse
                              ? Qt.rgba(palette.text.r, palette.text.g,
                                        palette.text.b, 0.08)
                              : "transparent")

                    RowLayout {
                        anchors { fill: parent; leftMargin: 8; rightMargin: 8 }
                        spacing: 8
                        Label {
                            id: profileLabel
                            Layout.fillWidth: true
                            text: model.text
                            elide: Text.ElideRight
                            color: selected ? accentTextColor : palette.text
                        }
                        Label {
                            text: appSettings.profileCombo(index)
                            elide: Text.ElideRight
                            font.pointSize: smallPointSize
                            color: selected
                                   ? Qt.rgba(accentTextColor.r, accentTextColor.g,
                                             accentTextColor.b, 0.75)
                                   : dimTextColor
                        }
                    }
                    MouseArea {
                        id: profileArea
                        anchors.fill: parent
                        hoverEnabled: true
                        onClicked: profilesView.currentIndex = index
                        onDoubleClicked: appSettings.loadCustomProfile(index)
                    }
                }
            }

            ColumnLayout {
                anchors.centerIn: parent
                width: parent.width - 2 * appSettings.defaultMargin
                visible: appSettings.customProfilesList.count === 0
                spacing: 2
                Label {
                    Layout.fillWidth: true
                    text: qsTr("No saved profiles yet.")
                    color: dimTextColor
                    horizontalAlignment: Text.AlignHCenter
                }
                Label {
                    Layout.fillWidth: true
                    text: qsTr("Set up a look you like on the General tab, then save it here or from the bar below.")
                    color: dimTextColor
                    font.pointSize: smallPointSize
                    wrapMode: Text.WordWrap
                    horizontalAlignment: Text.AlignHCenter
                }
            }
        }

        ColumnLayout {
            Layout.fillHeight: true
            Layout.fillWidth: false
            Layout.preferredWidth: 96
            Button {
                Layout.fillWidth: true
                text: qsTr("Save current")
                onClicked: profilesTab.requestSaveCurrent()
            }
            Button {
                Layout.fillWidth: true
                property alias currentIndex: profilesView.currentIndex
                enabled: currentIndex >= 0
                text: qsTr("Load")
                onClicked: {
                    var index = currentIndex
                    if (index >= 0)
                        appSettings.loadCustomProfile(index)
                }
            }
            Button {
                Layout.fillWidth: true
                text: qsTr("Remove")
                property alias currentIndex: profilesView.currentIndex

                enabled: currentIndex >= 0
                onClicked: {
                    appSettings.customProfilesList.remove(currentIndex)

                    // TODO This is a very ugly workaround. The view didn't update on Qt 5.3.2.
                    profilesView.model = 0
                    profilesView.model = appSettings.customProfilesList
                    profilesView.currentIndex = -1
                }
            }
            Item {
                // Spacing
                Layout.fillHeight: true
            }
            Button {
                Layout.fillWidth: true
                text: qsTr("Import…")
                onClicked: {
                    fileDialog.selectExisting = true
                    fileDialog.callBack = function (url) {
                        loadFile(url)
                    }
                    fileDialog.open()
                }
                function loadFile(url) {
                    try {
                        if (appSettings.verbose)
                            console.log("Loading file: " + url)

                        var profileObject = JSON.parse(fileIO.read(url))
                        var name = profileObject.name

                        if (!name)
                            throw "Profile doesn't have a name"

                        var version = profileObject.profileVersion
                                !== undefined ? profileObject.profileVersion : 1
                        if (version !== appSettings.profileVersion)
                            throw "This profile is not supported on this version of CRT."

                        delete profileObject.name
                        delete profileObject.profileVersion

                        appSettings.appendCustomProfile(name,
                                                        JSON.stringify(
                                                            profileObject))
                    } catch (err) {
                        messageDialog.text = qsTr(err)
                        messageDialog.open()
                    }
                }
            }
            Button {
                property alias currentIndex: profilesView.currentIndex

                Layout.fillWidth: true

                text: qsTr("Export…")
                enabled: currentIndex >= 0
                onClicked: {
                    fileDialog.selectExisting = false
                    fileDialog.callBack = function (url) {
                        storeFile(url)
                    }
                    fileDialog.open()
                }
                function storeFile(url) {
                    try {
                        var urlString = url.toString()

                        // Fix the extension if it's missing.
                        var extension = urlString.substring(
                                    urlString.length - 5, urlString.length)
                        var urlTail = (extension === ".json" ? "" : ".json")
                        url += urlTail

                        if (appSettings.verbose)
                            console.log("Storing file: " + url)

                        var profileObject = appSettings.customProfilesList.get(
                                    currentIndex)
                        var profileSettings = JSON.parse(
                                    profileObject.obj_string)
                        profileSettings["name"] = profileObject.text
                        profileSettings["profileVersion"] = appSettings.profileVersion

                        var result = fileIO.write(url, JSON.stringify(
                                                      profileSettings,
                                                      undefined, 2))
                        if (!result)
                            throw "The file could not be written."
                    } catch (err) {
                        console.log(err)
                        messageDialog.text = qsTr(
                                    "There has been an error storing the file.")
                        messageDialog.open()
                    }
                }
            }
        }
    }

    // DIALOGS ////////////////////////////////////////////////////////////////
    MessageDialog {
        id: messageDialog
        title: qsTr("File Error")
        buttons: MessageDialog.Ok
        onAccepted: {
            messageDialog.close()
        }
    }
    Loader {
        property var callBack
        property bool selectExisting: false
        id: fileDialog

        sourceComponent: FileDialog {
            nameFilters: ["Json files (*.json)"]
            fileMode: fileDialog.selectExisting ? FileDialog.OpenFile : FileDialog.SaveFile
            onAccepted: callBack(selectedFile)
        }

        onSelectExistingChanged: reload()

        function open() {
            item.open()
        }

        function reload() {
            active = false
            active = true
        }
    }
}
