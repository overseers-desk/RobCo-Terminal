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

// The two things a look is made of, side by side: the screen on the left with
// its own knobs, the chassis around it on the right with its own. A row is
// picked with a single click, and the row standing out is the one on air, not
// the one last touched.
RowLayout {
    // The window fills these in; the accent lives in one place there.
    property color accentColor
    property color accentTextColor
    property int effectsTabIndex

    signal requestTab(int index)

    readonly property real dimOpacity: 0.6
    readonly property real smallPointSize: Qt.application.font.pointSize * 0.85
    readonly property color frameColor: Qt.rgba(palette.text.r, palette.text.g,
                                                palette.text.b, 0.25)
    readonly property color dimTextColor: Qt.rgba(palette.text.r, palette.text.g,
                                                  palette.text.b, dimOpacity)

    id: generalTab
    spacing: appSettings.defaultMargin

    ColumnLayout {
        Layout.fillWidth: true
        Layout.fillHeight: true
        // The screen column carries the longer names, so it takes the wider
        // share of the page.
        Layout.preferredWidth: 1150
        spacing: 6

        RowLayout {
            Layout.fillWidth: true
            Label {
                text: qsTr("Screen")
            }
            Item {
                Layout.fillWidth: true
            }
            Label {
                text: appSettings.screensList.count + " " + qsTr("available")
                color: dimTextColor
                font.pointSize: smallPointSize
            }
        }

        Rectangle {
            Layout.fillWidth: true
            Layout.fillHeight: true
            color: palette.base
            radius: 3
            border.width: 1
            border.color: frameColor

            ColumnLayout {
                anchors { fill: parent; margins: 1 }
                spacing: 0

                ListView {
                    id: screensView
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    Layout.margins: 1
                    model: appSettings.screensList
                    clip: true
                    boundsBehavior: Flickable.StopAtBounds
                    ScrollBar.vertical: ScrollBar { }
                    delegate: Rectangle {
                        readonly property bool onAir: appSettings.screenName === model.text

                        width: screensView.width
                        height: screenLabel.implicitHeight + 10
                        color: onAir
                               ? accentColor
                               : (screenArea.containsMouse
                                  ? Qt.rgba(palette.text.r, palette.text.g,
                                            palette.text.b, 0.08)
                                  : "transparent")

                        RowLayout {
                            anchors { fill: parent; leftMargin: 8; rightMargin: 8 }
                            spacing: 8
                            Rectangle {
                                implicitWidth: 10
                                implicitHeight: 10
                                radius: 2
                                color: appSettings.screenSwatch(index)
                                border.width: 1
                                border.color: Qt.rgba(1, 1, 1, 0.35)
                            }
                            Label {
                                id: screenLabel
                                Layout.fillWidth: true
                                text: model.text
                                elide: Text.ElideRight
                                color: onAir ? accentTextColor : palette.text
                            }
                        }
                        MouseArea {
                            id: screenArea
                            anchors.fill: parent
                            hoverEnabled: true
                            onClicked: appSettings.loadScreen(index)
                        }
                    }
                }

                Rectangle {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 1
                    color: frameColor
                }

                GridLayout {
                    Layout.fillWidth: true
                    Layout.margins: appSettings.defaultMargin / 2
                    columns: 2
                    Label {
                        text: qsTr("Brightness")
                    }
                    SimpleSlider {
                        onValueChanged: appSettings.brightness = value
                        value: appSettings.brightness
                    }
                    Label {
                        text: qsTr("Contrast")
                    }
                    SimpleSlider {
                        onValueChanged: appSettings.contrast = value
                        value: appSettings.contrast
                    }
                    Label {
                        text: qsTr("Margin")
                    }
                    SimpleSlider {
                        onValueChanged: appSettings._margin = value
                        value: appSettings._margin
                    }
                    Label {
                        text: qsTr("Opacity")
                        visible: !appSettings.isMacOS
                    }
                    SimpleSlider {
                        onValueChanged: appSettings.windowOpacity = value
                        value: appSettings.windowOpacity
                        visible: !appSettings.isMacOS
                    }
                }

                // The rest of the screen's tuning lives on another page, and
                // this line is the way there.
                Label {
                    Layout.fillWidth: true
                    Layout.margins: appSettings.defaultMargin / 2
                    Layout.topMargin: 0
                    text: qsTr("Bloom, noise, curvature & more → Effects")
                    color: accentColor
                    font.pointSize: smallPointSize
                    elide: Text.ElideRight
                    MouseArea {
                        anchors.fill: parent
                        cursorShape: Qt.PointingHandCursor
                        onClicked: generalTab.requestTab(effectsTabIndex)
                    }
                }
            }
        }
    }

    ColumnLayout {
        Layout.fillWidth: true
        Layout.fillHeight: true
        Layout.preferredWidth: 1000
        spacing: 6

        RowLayout {
            Layout.fillWidth: true
            Label {
                text: qsTr("Chassis")
            }
            Item {
                Layout.fillWidth: true
            }
            Label {
                text: appSettings.chassisList.count + " " + qsTr("available")
                color: dimTextColor
                font.pointSize: smallPointSize
            }
        }

        Rectangle {
            Layout.fillWidth: true
            Layout.fillHeight: true
            color: palette.base
            radius: 3
            border.width: 1
            border.color: frameColor

            ColumnLayout {
                anchors { fill: parent; margins: 1 }
                spacing: 0

                ListView {
                    id: chassisView
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    Layout.margins: 1
                    model: appSettings.chassisList
                    clip: true
                    boundsBehavior: Flickable.StopAtBounds
                    ScrollBar.vertical: ScrollBar { }
                    delegate: Rectangle {
                        readonly property bool onAir: appSettings.chassisName === model.text

                        width: chassisView.width
                        height: chassisColumn.implicitHeight + 10
                        color: onAir
                               ? accentColor
                               : (chassisArea.containsMouse
                                  ? Qt.rgba(palette.text.r, palette.text.g,
                                            palette.text.b, 0.08)
                                  : "transparent")

                        ColumnLayout {
                            id: chassisColumn
                            anchors { fill: parent; leftMargin: 8; rightMargin: 8 }
                            spacing: 1
                            Label {
                                Layout.fillWidth: true
                                text: model.text
                                elide: Text.ElideRight
                                color: onAir ? accentTextColor : palette.text
                            }
                            Label {
                                Layout.fillWidth: true
                                text: model.description
                                elide: Text.ElideRight
                                font.pointSize: smallPointSize
                                // On the accent the dimmed tone would wash
                                // out, so the line goes darker instead.
                                color: onAir
                                       ? Qt.rgba(accentTextColor.r, accentTextColor.g,
                                                 accentTextColor.b, 0.75)
                                       : dimTextColor
                            }
                        }
                        MouseArea {
                            id: chassisArea
                            anchors.fill: parent
                            hoverEnabled: true
                            onClicked: appSettings.loadChassis(index)
                        }
                    }
                }

                Rectangle {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 1
                    color: frameColor
                }

                GridLayout {
                    Layout.fillWidth: true
                    Layout.margins: appSettings.defaultMargin / 2
                    columns: 2
                    Label {
                        text: qsTr("Radius")
                    }
                    SimpleSlider {
                        onValueChanged: appSettings.setScreenRadius(value)
                        value: appSettings._screenRadius
                    }
                    Label {
                        text: qsTr("Frame size")
                    }
                    SimpleSlider {
                        onValueChanged: appSettings.setFrameSize(value)
                        value: appSettings._frameSize
                    }
                }
            }
        }
    }
}
