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

import "../common"

// The switchboard's pager, the rail across the bank's foot: two square arrow
// keys flanking the framed PAGE plate, all riveted onto one raised plate.
// "PAGE" is engraved on the plate's frame over a recessed counter window,
// and the count turns on mechanical rolls behind it: near-neutral white
// painted digits, one roll a character, as the mock draws them. The rolls
// are their own counter, not the profile's display kit: no tape and no lamps
// stand in this window, paint on steel does.
//
// Stations measured off the mock in the pager's own coordinates (the bank
// content's x 8 at 1448x1086): rail plate 381 wide by 101; arrow keys 74x82
// at x 20 and mirrored; PAGE plate 159x90 centered; counter window 122x40 at
// the plate's y 34. A narrower bank squeezes the whole group proportionally
// rather than pinning any piece to a mock pixel.
Item {
    id: pager

    property color plastic: "#232830"
    property int pageIndex: 0
    property int pageCount: 1
    property int columnGap: 18

    signal step(int direction)

    Metrics { id: metrics }

    readonly property color plateFace: Qt.lighter(plastic, 1.18)
    readonly property color keyFace: Qt.lighter(plastic, 2.0)
    readonly property color plateHighlight: "#99a1ac"
    readonly property color plateShadow: "#040507"
    readonly property color engraveInk: "#101318"
    readonly property color engraveLight: "#8d949e"
    readonly property color rollDark: "#0b0c0e"
    readonly property color digitPaint: "#c6c6c4"

    // The group's natural span; narrower content squeezes every measure.
    readonly property real squeeze: Math.min(1, width > 0 ? width / 381 : 1)
    readonly property real keyWidth: metrics.pagerArrowWidth * squeeze
    readonly property real keyHeight: metrics.pagerArrowHeight * squeeze
    readonly property real plateWidth: 159 * squeeze
    readonly property real plateHeight: 90 * squeeze

    readonly property real prevX: 20 * squeeze
    readonly property real nextX: width - 20 * squeeze - keyWidth
    readonly property real plateX: (width - plateWidth) / 2

    // Two digits a side always, as the counter's rolls are painted.
    readonly property string pageLabel: _zeroPad(pageIndex + 1) + "/" + _zeroPad(pageCount)

    function _zeroPad(n) {
        return n < 10 ? "0" + n : String(n)
    }

    // The height the pager admits to is less than the rail it paints: the
    // bank's row arithmetic reserves a full rowSpacing of air above this
    // item where the mock leaves seven pixels, and charging the plate's
    // whole 104 on top of that air would push the fifteenth row off the
    // panel. The body keeps the mock's measure and stands up into the
    // reserved air instead.
    implicitHeight: metrics.pagerHeight - 15

    // The rail plate everything is riveted onto, with its bright bevel line
    // on the chassis underneath. It keeps the mock's measured height and
    // stands up into the reserved air, past this item's own top.
    ShaderEffect {
        id: rail

        width: parent.width
        y: parent.height - height - 3
        height: metrics.pagerHeight - 3

        property size sizePx: Qt.size(width, height)
        property vector2d lightDir: metrics.castingLightDir
        property color baseColor: pager.plateFace
        property color highlightColor: pager.plateHighlight
        property color shadowColor: pager.plateShadow
        property real cornerRadius: 8
        property real bevelPx: 3
        property real grainAmount: 0.35
        property real mottleAmount: 0.7
        property real scratchAmount: 0.5
        property real vignetteStrength: 0.35
        property real wearAmount: 0.35
        property real seamGain: 0.6
        property real seed: 0.23

        vertexShader: "qrc:/shaders/plate_metal.vert.qsb"
        fragmentShader: "qrc:/shaders/plate_metal.frag.qsb"

        onStatusChanged: if (log) console.log(log)

        ScrewHead {
            x: 3; y: 3
            width: 13; height: 13
            metalLight: "#8b929c"; metalMid: "#3a3f46"; metalDark: "#0a0b0e"
            glint: "#d8dde4"; slotAngle: 47
            lightX: metrics.castingLightDir.x; lightY: metrics.castingLightDir.y
        }
        ScrewHead {
            x: parent.width - 16; y: 3
            width: 13; height: 13
            metalLight: "#8b929c"; metalMid: "#3a3f46"; metalDark: "#0a0b0e"
            glint: "#d8dde4"; slotAngle: -21
            lightX: metrics.castingLightDir.x; lightY: metrics.castingLightDir.y
        }
        ScrewHead {
            x: 3; y: parent.height - 16
            width: 13; height: 13
            metalLight: "#8b929c"; metalMid: "#3a3f46"; metalDark: "#0a0b0e"
            glint: "#d8dde4"; slotAngle: 68
            lightX: metrics.castingLightDir.x; lightY: metrics.castingLightDir.y
        }
        ScrewHead {
            x: parent.width - 16; y: parent.height - 16
            width: 13; height: 13
            metalLight: "#8b929c"; metalMid: "#3a3f46"; metalDark: "#0a0b0e"
            glint: "#d8dde4"; slotAngle: -74
            lightX: metrics.castingLightDir.x; lightY: metrics.castingLightDir.y
        }
    }
    Rectangle {
        y: parent.height - 2
        x: 2
        width: parent.width - 4
        height: 2
        radius: 1
        color: "#79818c"
        opacity: 0.5
    }

    // One square arrow key: a raised cap screwed to the rail, a heavy solid
    // arrow engraved dark into its face, the cut's lower edge catching a
    // sliver of the room. A press seats the cap a couple of pixels deeper.
    component ArrowKey: Item {
        id: key

        property int direction: -1
        property real wearSeed: 0.31

        width: pager.keyWidth
        height: pager.keyHeight
        anchors.verticalCenter: rail.verticalCenter

        ShaderEffect {
            id: keyCap

            anchors.fill: parent
            anchors.topMargin: press.pressed ? 2 : 0
            anchors.bottomMargin: press.pressed ? -2 : 0

            property size sizePx: Qt.size(width, height)
            property vector2d lightDir: metrics.castingLightDir
            property color baseColor: press.pressed
                ? Qt.darker(pager.keyFace, 1.15) : pager.keyFace
            property color highlightColor: pager.plateHighlight
            property color shadowColor: pager.plateShadow
            property real cornerRadius: 7
            property real bevelPx: 3
            property real grainAmount: 0.35
            property real mottleAmount: 0.65
            property real scratchAmount: 0.5
            property real vignetteStrength: 0.3
            property real wearAmount: 0.35
            property real seamGain: 0.7
            property real seed: key.wearSeed

            vertexShader: "qrc:/shaders/plate_metal.vert.qsb"
            fragmentShader: "qrc:/shaders/plate_metal.frag.qsb"

            onStatusChanged: if (log) console.log(log)

            ScrewHead {
                x: 2; y: 2
                width: 11; height: 11
                metalLight: "#8b929c"; metalMid: "#3a3f46"; metalDark: "#0a0b0e"
                glint: "#d8dde4"; slotAngle: 12 + key.wearSeed * 90
                lightX: metrics.castingLightDir.x; lightY: metrics.castingLightDir.y
            }
            ScrewHead {
                x: parent.width - 13; y: 2
                width: 11; height: 11
                metalLight: "#8b929c"; metalMid: "#3a3f46"; metalDark: "#0a0b0e"
                glint: "#d8dde4"; slotAngle: -40 - key.wearSeed * 70
                lightX: metrics.castingLightDir.x; lightY: metrics.castingLightDir.y
            }
            ScrewHead {
                x: 2; y: parent.height - 13
                width: 11; height: 11
                metalLight: "#8b929c"; metalMid: "#3a3f46"; metalDark: "#0a0b0e"
                glint: "#d8dde4"; slotAngle: 77 - key.wearSeed * 50
                lightX: metrics.castingLightDir.x; lightY: metrics.castingLightDir.y
            }
            ScrewHead {
                x: parent.width - 13; y: parent.height - 13
                width: 11; height: 11
                metalLight: "#8b929c"; metalMid: "#3a3f46"; metalDark: "#0a0b0e"
                glint: "#d8dde4"; slotAngle: -8 + key.wearSeed * 60
                lightX: metrics.castingLightDir.x; lightY: metrics.castingLightDir.y
            }

            Canvas {
                id: arrow

                anchors.centerIn: parent
                width: Math.round(50 * pager.squeeze)
                height: Math.round(36 * pager.squeeze)

                onPaint: {
                    var ctx = getContext("2d")
                    ctx.reset()
                    var w = width
                    var h = height
                    var stem = h * 0.36

                    function trace(dx) {
                        // A solid arrow with a stem; dx mirrors it for NEXT.
                        function px(x) { return key.direction < 0 ? x : w - x }
                        ctx.beginPath()
                        ctx.moveTo(px(0) + dx, h / 2)
                        ctx.lineTo(px(w * 0.42) + dx, 0)
                        ctx.lineTo(px(w * 0.42) + dx, (h - stem) / 2)
                        ctx.lineTo(px(w) + dx, (h - stem) / 2)
                        ctx.lineTo(px(w) + dx, (h + stem) / 2)
                        ctx.lineTo(px(w * 0.42) + dx, (h + stem) / 2)
                        ctx.lineTo(px(w * 0.42) + dx, h)
                        ctx.closePath()
                    }

                    // The lit lower edge of the cut, then the ink over it.
                    ctx.save()
                    ctx.translate(0.8, 1.2)
                    trace(0)
                    ctx.fillStyle = Qt.rgba(0.55, 0.58, 0.63, 0.55)
                    ctx.fill()
                    ctx.restore()
                    trace(0)
                    ctx.fillStyle = pager.engraveInk
                    ctx.fill()
                }
            }
        }

        MouseArea {
            id: press
            anchors.fill: parent
            acceptedButtons: Qt.LeftButton
            onClicked: pager.step(key.direction)
        }
    }

    ArrowKey {
        x: pager.prevX
        direction: -1
        wearSeed: 0.31
    }

    // The framed PAGE plate: "PAGE" engraved on the frame, the counter
    // window recessed under it with the rolls behind the glass line.
    ShaderEffect {
        id: pagePlate

        x: pager.plateX
        width: pager.plateWidth
        height: pager.plateHeight
        anchors.verticalCenter: rail.verticalCenter

        property size sizePx: Qt.size(width, height)
        property vector2d lightDir: metrics.castingLightDir
        property color baseColor: pager.keyFace
        property color highlightColor: pager.plateHighlight
        property color shadowColor: pager.plateShadow
        property real cornerRadius: 7
        property real bevelPx: 3
        property real grainAmount: 0.35
        property real mottleAmount: 0.65
        property real scratchAmount: 0.5
        property real vignetteStrength: 0.3
        property real wearAmount: 0.3
        property real seamGain: 0.7
        property real seed: 0.53

        vertexShader: "qrc:/shaders/plate_metal.vert.qsb"
        fragmentShader: "qrc:/shaders/plate_metal.frag.qsb"

        onStatusChanged: if (log) console.log(log)

        ScrewHead {
            x: 2; y: 2
            width: 11; height: 11
            metalLight: "#8b929c"; metalMid: "#3a3f46"; metalDark: "#0a0b0e"
            glint: "#d8dde4"; slotAngle: 33
            lightX: metrics.castingLightDir.x; lightY: metrics.castingLightDir.y
        }
        ScrewHead {
            x: parent.width - 13; y: 2
            width: 11; height: 11
            metalLight: "#8b929c"; metalMid: "#3a3f46"; metalDark: "#0a0b0e"
            glint: "#d8dde4"; slotAngle: -66
            lightX: metrics.castingLightDir.x; lightY: metrics.castingLightDir.y
        }
        ScrewHead {
            x: 2; y: parent.height - 13
            width: 11; height: 11
            metalLight: "#8b929c"; metalMid: "#3a3f46"; metalDark: "#0a0b0e"
            glint: "#d8dde4"; slotAngle: 59
            lightX: metrics.castingLightDir.x; lightY: metrics.castingLightDir.y
        }
        ScrewHead {
            x: parent.width - 13; y: parent.height - 13
            width: 11; height: 11
            metalLight: "#8b929c"; metalMid: "#3a3f46"; metalDark: "#0a0b0e"
            glint: "#d8dde4"; slotAngle: -27
            lightX: metrics.castingLightDir.x; lightY: metrics.castingLightDir.y
        }

        // "PAGE", engraved: the dark cut with the light its lower edge
        // catches laid under it.
        Item {
            anchors.horizontalCenter: parent.horizontalCenter
            y: Math.round(8 * pager.squeeze)
            width: engraving.implicitWidth
            height: engraving.implicitHeight

            Text {
                x: 0.5
                y: 1
                font: engraving.font
                text: engraving.text
                color: pager.engraveLight
                opacity: 0.6
            }
            Text {
                id: engraving

                font.pixelSize: Math.max(9, Math.round(19 * pager.squeeze))
                font.bold: true
                font.letterSpacing: 4 * pager.squeeze
                text: "PAGE"
                color: pager.engraveInk
            }
        }

        // The counter window: a bevel ring dropping to the rolls, dark under
        // its top lip like every sink on this panel.
        Rectangle {
            id: windowRing

            x: (parent.width - width) / 2
            y: Math.round(34 * pager.squeeze)
            width: metrics.pageWindowWidth * pager.squeeze + 6
            height: metrics.pageWindowHeight * pager.squeeze + 6
            radius: metrics.pageWindowRadius + 2
            antialiasing: true
            color: "#0b0d10"

            Rectangle {
                id: rolls

                anchors.fill: parent
                anchors.margins: 3
                radius: metrics.pageWindowRadius
                antialiasing: true
                clip: true
                color: "#000000"

                Row {
                    anchors.fill: parent
                    spacing: 1

                    Repeater {
                        model: pager.pageLabel.length

                        // One roll, a painted character on its drum: lit at
                        // the drum's belly, falling dark toward the window's
                        // lips where the cylinder turns away.
                        Rectangle {
                            required property int index

                            width: (rolls.width - (pager.pageLabel.length - 1)) / pager.pageLabel.length
                            height: rolls.height

                            gradient: Gradient {
                                GradientStop { position: 0.00; color: Qt.darker(pager.rollDark, 1.8) }
                                GradientStop { position: 0.30; color: Qt.lighter(pager.rollDark, 2.6) }
                                GradientStop { position: 0.55; color: Qt.lighter(pager.rollDark, 3.0) }
                                GradientStop { position: 1.00; color: Qt.darker(pager.rollDark, 1.6) }
                            }

                            Text {
                                anchors.centerIn: parent
                                font.family: "serif"
                                font.bold: true
                                font.pixelSize: Math.max(10, Math.round(30 * pager.squeeze))
                                text: pager.pageLabel.charAt(parent.index)
                                color: pager.digitPaint
                            }
                        }
                    }
                }

                // The window lip's shadow over the top of the rolls.
                Rectangle {
                    width: parent.width
                    height: 5
                    gradient: Gradient {
                        GradientStop { position: 0.0; color: Qt.rgba(0, 0, 0, 0.7) }
                        GradientStop { position: 1.0; color: "transparent" }
                    }
                }
            }
        }
    }

    ArrowKey {
        x: pager.nextX
        direction: 1
        wearSeed: 0.67
    }
}
