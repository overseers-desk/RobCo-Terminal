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

// The blue appliance's page selector, always standing at the bank's foot as
// the mock builds it: three recesses punched into the chassis like the LED
// windows above them, a bright bevel line under each. PREV and NEXT are dark
// metal key caps sunk in the outer two, each engraved with its label over a
// solid triangle pointing off the bank; between them an LED window shows the
// page the bank is turned to, two lamps' worth of digits.
//
// Stations measured off the mock in the bank content's coordinates (content
// x 97 at 1448x1086): PREV hole at 12, 71 wide; page window at 107, 82 wide;
// NEXT at width-22-71; holes 90 tall, y 932..1021 on the mock. A narrower
// bank squeezes the whole group proportionally rather than pinning any piece
// to a mock pixel.
Item {
    id: pager

    property color plastic: "#4f4737"
    property int pageIndex: 0
    property int pageCount: 1
    property int columnGap: 27

    signal step(int direction)

    readonly property color holeDark: "#060504"
    readonly property color capFace: "#221e16"
    readonly property color capHighlight: "#aa9a82"
    readonly property color labelMetal: "#8a7c64"
    readonly property color arrowMetal: "#786b56"
    readonly property color bevelLight: "#b7a283"
    readonly property color panelDark: "#0b0a07"

    // The group's natural span; narrower content squeezes every measure.
    readonly property real squeeze: Math.min(1, width > 0 ? width / 305 : 1)
    readonly property real keyWidth: 71 * squeeze
    readonly property real windowWidth: 82 * squeeze
    readonly property real holeHeight: 90 * squeeze

    readonly property real prevX: 12 * squeeze
    readonly property real nextX: width - 22 * squeeze - keyWidth
    readonly property real windowX: (prevX + keyWidth + nextX - windowWidth) / 2

    // Two digits always, as the bank's numerals are stamped.
    readonly property string pageLabel: {
        var n = pageIndex + 1
        return n < 10 ? "0" + n : String(n)
    }

    implicitHeight: Math.round(holeHeight) + 3

    // One recess of the selector, the window vocabulary continued: a tight
    // dark cut ring, a recessed dark floor, and the bright bevel line on the
    // chassis under the hole.
    component Recess: Item {
        id: recess

        y: 1
        width: 10
        height: pager.holeHeight

        Rectangle {
            anchors.fill: parent
            radius: 4
            antialiasing: true
            gradient: Gradient {
                GradientStop { position: 0.00; color: "#12100b" }
                GradientStop { position: 0.10; color: pager.holeDark }
                GradientStop { position: 0.85; color: Qt.lighter(pager.holeDark, 1.6) }
                GradientStop { position: 1.00; color: Qt.lighter(pager.holeDark, 2.4) }
            }
        }
        Rectangle {
            x: 1
            y: parent.height + 1
            width: parent.width - 2
            height: 2
            radius: 1
            color: pager.bevelLight
            opacity: 0.7
        }
    }

    // A dark metal key cap sunk in its recess: the plate law's bevels catch
    // the room on the top edge, the label engraved above a solid triangle.
    component PagerKey: Item {
        id: key

        property string label: ""
        property int direction: -1
        property real wearSeed: 0.31

        width: pager.keyWidth
        height: pager.holeHeight + 3

        Recess {
            width: key.width
        }

        ShaderEffect {
            id: cap

            x: 5 * pager.squeeze
            y: 6 * pager.squeeze
            width: key.width - 10 * pager.squeeze
            height: pager.holeHeight - 14 * pager.squeeze

            property size sizePx: Qt.size(width, height)
            property vector2d lightDir: Qt.vector2d(-0.55, -0.85)
            property color baseColor: pager.capFace
            property color highlightColor: pager.capHighlight
            property color shadowColor: "#040302"
            property real cornerRadius: 4
            property real bevelPx: 4
            property real grainAmount: 0.3
            property real mottleAmount: 0.6
            property real scratchAmount: 0.4
            property real vignetteStrength: 0.3
            property real wearAmount: 0.3
            property real seamGain: 0.7
            property real seed: key.wearSeed

            vertexShader: "qrc:/shaders/plate_metal.vert.qsb"
            fragmentShader: "qrc:/shaders/plate_metal.frag.qsb"

            onStatusChanged: if (log) console.log(log)

            // The machined ridge across the cap's shoulder, the brightest
            // thing on the key, with its cut shadow under it.
            Rectangle {
                x: 3
                y: Math.round(6 * pager.squeeze)
                width: parent.width - 6
                height: Math.max(2, Math.round(4 * pager.squeeze))
                radius: 1
                color: pager.capHighlight
                opacity: 0.95
            }
            Rectangle {
                x: 3
                y: Math.round(6 * pager.squeeze) + Math.max(2, Math.round(4 * pager.squeeze))
                width: parent.width - 6
                height: 1
                color: "#000000"
                opacity: 0.45
            }

            // The label engraved in the cap: the dark cut first, then the
            // light the cut's lower edge catches, laid over it.
            Item {
                anchors.horizontalCenter: parent.horizontalCenter
                y: Math.round(14 * pager.squeeze)
                width: engraving.implicitWidth
                height: engraving.implicitHeight

                Text {
                    y: -1
                    font: engraving.font
                    text: engraving.text
                    color: "#000000"
                    opacity: 0.55
                }
                Text {
                    id: engraving

                    font.pixelSize: Math.max(9, Math.round(16 * pager.squeeze))
                    font.bold: true
                    font.letterSpacing: 2 * pager.squeeze
                    text: key.label
                    color: pager.labelMetal
                }
            }

            // The solid triangle struck under the label, pointing off the
            // bank the way its key walks the pages.
            Canvas {
                id: arrow

                anchors.horizontalCenter: parent.horizontalCenter
                y: Math.round(40 * pager.squeeze)
                width: Math.round(22 * pager.squeeze)
                height: Math.round(24 * pager.squeeze)

                onPaint: {
                    var ctx = getContext("2d")
                    ctx.reset()
                    ctx.beginPath()
                    if (key.direction < 0) {
                        ctx.moveTo(width, 0)
                        ctx.lineTo(width, height)
                        ctx.lineTo(0, height / 2)
                    } else {
                        ctx.moveTo(0, 0)
                        ctx.lineTo(0, height)
                        ctx.lineTo(width, height / 2)
                    }
                    ctx.closePath()
                    ctx.fillStyle = pager.arrowMetal
                    ctx.fill()
                }
            }
        }

        MouseArea {
            anchors.fill: parent
            acceptedButtons: Qt.LeftButton
            onClicked: pager.step(key.direction)
        }
    }

    PagerKey {
        x: pager.prevX
        label: "PREV"
        direction: -1
        wearSeed: 0.31
    }

    // The page window: the same recess, with the shared LED display kit
    // burning the page number behind it.
    Item {
        x: pager.windowX
        width: pager.windowWidth
        height: pager.holeHeight + 3

        Recess {
            width: pager.windowWidth
        }

        Rectangle {
            id: pagePanel

            x: 3
            y: 3
            width: parent.width - 6
            height: pager.holeHeight - 5
            radius: 2
            antialiasing: true
            clip: true

            gradient: Gradient {
                GradientStop { position: 0.0; color: Qt.darker(pager.panelDark, 1.7) }
                GradientStop { position: 0.5; color: pager.panelDark }
                GradientStop { position: 0.94; color: Qt.lighter(pager.panelDark, 1.7) }
                GradientStop { position: 1.0; color: Qt.lighter(pager.panelDark, 2.8) }
            }

            Loader {
                id: pageDisplay

                anchors.centerIn: parent
                // displayUrl speaks from the qml root; this file lives two
                // directories deeper, so the path walks back up first.
                source: "../../" + appSettings.displayUrl("Display")
                scale: item ? Math.min(2.0,
                    (pagePanel.width - 8) / Math.max(1, item.implicitWidth)) : 1

                Binding {
                    target: pageDisplay.item
                    property: "characters"
                    value: 2
                }
                // Both lamps carry a digit; the counter fills its window.
                Binding {
                    target: pageDisplay.item
                    property: "padCellsLeft"
                    value: 0
                }
                Binding {
                    target: pageDisplay.item
                    property: "padCellsRight"
                    value: 0
                }
                Binding {
                    target: pageDisplay.item
                    property: "text"
                    value: pager.pageLabel
                }
                Binding {
                    target: pageDisplay.item
                    property: "powered"
                    value: true
                }
                Binding {
                    target: pageDisplay.item
                    property: "bright"
                    value: false
                }
                // The scaled lamps would wash the bezel; the page window
                // glows quieter than a channel window.
                Binding {
                    target: pageDisplay.item
                    property: "spillStrength"
                    value: 0.12
                }
            }
        }
    }

    PagerKey {
        x: pager.nextX
        label: "NEXT"
        direction: 1
        wearSeed: 0.67
    }
}
