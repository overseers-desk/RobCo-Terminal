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

// One switchboard row's furniture: two riveted plates on the chassis with a
// seam of bare metal between rows. The left plate carries the light-stamped
// numeral and the heavy toggle in its recessed well; the right one is the
// label plate, a raised moulding framing the tape well the display kit lays
// in displayRect. The thrown switch is the whole mark of the channel on
// screen: lever swung off a lit right side, the well flooding with the
// profile's own phosphor. At rest the lever lies flat over the well's left,
// dark-capped, only its machined chamfer catching the room.
Item {
    id: furniture

    property color plastic: "#232830"
    property string numeralText: ""
    property rect displayRect: Qt.rect(0, 0, 0, 0)
    property bool open: false
    property bool current: false

    // Bound from the shell's own Metrics.columnGap by the row.
    property int numeralGap: 18

    Metrics { id: metrics }

    // How far the plates stand past the row band, into the inter-row seam.
    readonly property int plateReach: 4

    readonly property color plateFace: Qt.lighter(plastic, 1.4)
    readonly property color plateHighlight: "#99a1ac"
    readonly property color plateShadow: "#040507"
    readonly property color numeralPaint: "#cfc4ba"
    readonly property color wellDark: "#0a0c10"
    readonly property color capFace: "#343a41"
    readonly property color capChamfer: "#79818c"
    readonly property color glowColor: appSettings.fontColor

    // Each plate weathers on its own seed, keyed off the stamped numeral so
    // no two rows wear alike. The numeral lands after creation and changes on
    // a page flip; the painted cap re-weathers with it.
    readonly property real rowSeed: (parseInt(numeralText, 10) || 0) * 0.137 % 1
    onRowSeedChanged: cap.requestPaint()

    // The switch plate: numeral and well together, the folded lane the
    // Metrics file hands the bank as numeralWidth.
    ShaderEffect {
        id: switchPlate

        x: 0
        y: -furniture.plateReach
        width: metrics.numeralWidth
        height: furniture.height + 2 * furniture.plateReach

        property size sizePx: Qt.size(width, height)
        property vector2d lightDir: metrics.castingLightDir
        property color baseColor: furniture.plateFace
        property color highlightColor: furniture.plateHighlight
        property color shadowColor: furniture.plateShadow
        property real cornerRadius: 5
        property real bevelPx: 2
        property real grainAmount: 0.35
        property real mottleAmount: 0.7
        property real scratchAmount: 0.5
        property real vignetteStrength: 0.3
        property real wearAmount: 0.4
        property real seamGain: 0.6
        property real seed: furniture.rowSeed

        vertexShader: "qrc:/shaders/plate_metal.vert.qsb"
        fragmentShader: "qrc:/shaders/plate_metal.frag.qsb"

        onStatusChanged: if (log) console.log(log)

        // The plate's corner rivets: small domes, a glint on the side the
        // key light lands.
        component Rivet: Rectangle {
            width: 5
            height: 5
            radius: 2.5
            antialiasing: true
            gradient: Gradient {
                GradientStop { position: 0.0; color: "#6d747e" }
                GradientStop { position: 0.55; color: "#2c3037" }
                GradientStop { position: 1.0; color: "#0a0b0e" }
            }
        }
        Rivet { x: 5; y: 5 }
        Rivet { x: parent.width - 10; y: 5 }
        Rivet { x: 5; y: parent.height - 10 }
        Rivet { x: parent.width - 10; y: parent.height - 10 }
    }

    // The stamped numeral: raised paint catching the light on its face, its
    // strike shadow thrown down and right, the inverse of the blue shell's
    // ink-in-metal stamp.
    Item {
        id: numeral

        x: 6
        width: (metrics.numeralCenterX - 8) * 2 + 4
        height: stamped.implicitHeight
        anchors.verticalCenter: parent.verticalCenter

        // The same Iosevka the font manager already registers app-wide,
        // named directly rather than loaded a second time.
        readonly property string stampFaceFamily: appSettings.fontManager.fontByName("IOSEVKA").family

        Text {
            x: 1
            y: 2
            width: numeral.width
            horizontalAlignment: Text.AlignHCenter
            font.family: numeral.stampFaceFamily
            font.pixelSize: 38
            font.bold: true
            font.letterSpacing: -2
            text: furniture.numeralText
            color: "#05060a"
            opacity: 0.75
        }
        // The paint face, struck twice a hair apart: the mock's stencil
        // strokes are heavier than any weight this face carries.
        Text {
            id: stamped
            width: numeral.width
            horizontalAlignment: Text.AlignHCenter
            font.family: numeral.stampFaceFamily
            font.pixelSize: 38
            font.bold: true
            font.letterSpacing: -2
            text: furniture.numeralText
            color: furniture.numeralPaint
        }
        Text {
            x: 0.8
            width: numeral.width
            horizontalAlignment: Text.AlignHCenter
            font: stamped.font
            text: furniture.numeralText
            color: furniture.numeralPaint
        }
    }

    // The switch well: a rounded recess standing proud of the row band top
    // and bottom, dark under its top lip, the far wall catching the light
    // along the bottom the way every punched hole on this panel does.
    Rectangle {
        id: well

        x: metrics.switchWellX
        anchors.verticalCenter: parent.verticalCenter
        width: metrics.switchWellWidth
        height: metrics.switchWellHeight
        radius: metrics.switchWellRadius
        antialiasing: true
        clip: true

        gradient: Gradient {
            GradientStop { position: 0.00; color: Qt.darker(furniture.wellDark, 1.8) }
            GradientStop { position: 0.50; color: furniture.wellDark }
            GradientStop { position: 0.88; color: Qt.lighter(furniture.wellDark, 1.9) }
            GradientStop { position: 1.00; color: Qt.lighter(furniture.wellDark, 3.2) }
        }

        // The top lip's shadow down the near wall, and the left wall falling
        // dark with the key leaning that way.
        Rectangle {
            width: parent.width
            height: 6
            gradient: Gradient {
                GradientStop { position: 0.0; color: Qt.rgba(0, 0, 0, 0.8) }
                GradientStop { position: 1.0; color: "transparent" }
            }
        }
        Rectangle {
            width: 6
            height: parent.height
            gradient: Gradient {
                orientation: Gradient.Horizontal
                GradientStop { position: 0.0; color: Qt.rgba(0, 0, 0, 0.6) }
                GradientStop { position: 1.0; color: "transparent" }
            }
        }

        // The lit floor the thrown lever uncovers: the profile's phosphor
        // flooding the well's right side, hottest against the lever's edge,
        // falling off toward the far lip.
        Rectangle {
            id: wellGlow

            x: 56
            y: 2
            width: parent.width - x - 2
            height: parent.height - 4
            radius: metrics.switchWellRadius - 2
            antialiasing: true
            opacity: furniture.current ? 1 : 0
            Behavior on opacity { NumberAnimation { duration: 150 } }

            gradient: Gradient {
                orientation: Gradient.Horizontal
                GradientStop { position: 0.00; color: Qt.lighter(furniture.glowColor, 1.5) }
                GradientStop { position: 0.14; color: furniture.glowColor }
                GradientStop { position: 0.45; color: Qt.darker(furniture.glowColor, 2.6) }
                GradientStop { position: 0.85; color: Qt.darker(furniture.glowColor, 6.0) }
                GradientStop { position: 1.00; color: Qt.darker(furniture.glowColor, 9.0) }
            }
        }

        // The pivot screw's socket in the well's right side; its rim takes
        // the glow when the floor lights.
        Rectangle {
            id: pivotSocket

            x: 78
            anchors.verticalCenter: parent.verticalCenter
            width: 15
            height: 15
            radius: 7.5
            antialiasing: true
            color: "#0c0e12"
            border.width: 2
            border.color: furniture.current ? Qt.lighter(furniture.glowColor, 1.6) : "#3a4048"
            Behavior on border.color { ColorAnimation { duration: 150 } }

            Rectangle {
                anchors.centerIn: parent
                width: 5
                height: 5
                radius: 2.5
                color: furniture.current ? "#ffffff" : "#20242a"
                Behavior on color { ColorAnimation { duration: 150 } }
            }
        }
    }

    // The bright bevel line on the plate just under the well's bottom lip,
    // where the light catches the far wall of the punch.
    Rectangle {
        x: well.x + 3
        y: well.y + well.height + 1
        width: well.width - 6
        height: 2
        radius: 1
        color: "#79818c"
        opacity: 0.45
    }

    // The soft spill the lit well throws past its own lips onto the plate.
    Rectangle {
        anchors.centerIn: well
        width: well.width + 26
        height: well.height + 26
        radius: metrics.switchWellRadius + 13
        antialiasing: true
        color: furniture.glowColor
        opacity: furniture.current ? 0.06 : 0
        Behavior on opacity { NumberAnimation { duration: 150 } }
    }

    // The lever: a heavy dark cap over the well's left. At rest it lies flat;
    // thrown it swings out and left off the lit floor, the throw landing with
    // a slight mechanical overshoot.
    Item {
        id: lever

        x: well.x + (furniture.current ? -1 : 3)
        anchors.verticalCenter: parent.verticalCenter
        width: 74
        height: 54
        rotation: furniture.current ? -3 : 0
        transformOrigin: Item.BottomLeft

        Behavior on x {
            NumberAnimation { duration: 100; easing.type: Easing.OutBack; easing.overshoot: 1.4 }
        }
        Behavior on rotation {
            NumberAnimation { duration: 100; easing.type: Easing.OutBack; easing.overshoot: 1.4 }
        }

        // The cap's drop shadow into the well.
        Rectangle {
            x: 6
            y: 8
            width: 62
            height: 44
            radius: 6
            color: Qt.rgba(0, 0, 0, 0.55)
        }

        Canvas {
            id: cap

            anchors.fill: parent
            onPaint: {
                var ctx = getContext("2d")
                ctx.reset()
                var w = width
                var h = height

                // Front face: a dark scratched slab, lit faintly from the
                // upper left.
                var face = ctx.createLinearGradient(0, 0, w * 0.8, h)
                face.addColorStop(0.0, Qt.lighter(furniture.capFace, 1.35))
                face.addColorStop(0.5, furniture.capFace)
                face.addColorStop(1.0, Qt.darker(furniture.capFace, 1.5))
                ctx.beginPath()
                ctx.roundedRect(2, 4, w - 16, h - 10, 6, 6)
                ctx.fillStyle = face
                ctx.fill()

                // Scratches and grime over the face, seeded per row.
                var s = 7 + Math.round(furniture.rowSeed * 97)
                function rnd() { s = (s * 16807) % 2147483647; return s / 2147483647 }
                ctx.save()
                ctx.beginPath()
                ctx.roundedRect(2, 4, w - 16, h - 10, 6, 6)
                ctx.clip()
                for (var i = 0; i < 90; i++) {
                    var px = 2 + rnd() * (w - 18)
                    var py = 4 + rnd() * (h - 12)
                    var dark = rnd() < 0.55
                    ctx.strokeStyle = dark ? Qt.rgba(0, 0, 0, 0.10 + 0.14 * rnd())
                                           : Qt.rgba(0.78, 0.81, 0.86, 0.06 + 0.11 * rnd())
                    ctx.lineWidth = rnd() < 0.2 ? 1.6 : 1
                    ctx.beginPath()
                    ctx.moveTo(px, py)
                    ctx.lineTo(px + (rnd() - 0.3) * 18, py + (rnd() - 0.5) * 6)
                    ctx.stroke()
                }
                // Grime pooling toward the cap's lower half.
                for (var b = 0; b < 10; b++) {
                    var bx = 2 + rnd() * (w - 18)
                    var by = h * 0.4 + rnd() * (h * 0.5)
                    var br = 3 + rnd() * 8
                    ctx.beginPath()
                    ctx.ellipse(bx - br, by - br * 0.6, br * 2, br * 1.2)
                    ctx.fillStyle = Qt.rgba(0, 0, 0, 0.05 + 0.07 * rnd())
                    ctx.fill()
                }
                ctx.restore()

                // The machined chamfer down the cap's right side, the
                // brightest metal on the row at rest.
                var cham = ctx.createLinearGradient(w - 18, 0, w - 2, h)
                cham.addColorStop(0.0, Qt.lighter(furniture.capChamfer, 1.25))
                cham.addColorStop(0.45, furniture.capChamfer)
                cham.addColorStop(1.0, Qt.darker(furniture.capChamfer, 2.6))
                ctx.beginPath()
                ctx.moveTo(w - 16, 4)
                ctx.lineTo(w - 4, 10)
                ctx.lineTo(w - 4, h - 12)
                ctx.lineTo(w - 16, h - 6)
                ctx.closePath()
                ctx.fillStyle = cham
                ctx.fill()

                // The lit sliver along the cap's top edge.
                ctx.beginPath()
                ctx.moveTo(6, 4.5)
                ctx.lineTo(w - 15, 4.5)
                ctx.lineWidth = 1.5
                ctx.strokeStyle = Qt.rgba(0.42, 0.46, 0.52, 0.9)
                ctx.stroke()

                // The cut shadow closing the cap's outline.
                ctx.beginPath()
                ctx.roundedRect(2, 4, w - 16, h - 10, 6, 6)
                ctx.lineWidth = 1.5
                ctx.strokeStyle = Qt.rgba(0, 0, 0, 0.6)
                ctx.stroke()

                // The retaining screw's recess in the cap's left half.
                ctx.beginPath()
                ctx.arc(16, h / 2 - 4, 4.5, 0, Math.PI * 2)
                ctx.fillStyle = Qt.rgba(0, 0, 0, 0.6)
                ctx.fill()
                ctx.beginPath()
                ctx.arc(16, h / 2 - 4, 4.5, Math.PI * 0.15, Math.PI * 0.85)
                ctx.lineWidth = 1.2
                ctx.strokeStyle = Qt.rgba(0.5, 0.54, 0.6, 0.7)
                ctx.stroke()
            }
        }

        // The chamfer catching the lit well when the lever is thrown.
        Rectangle {
            x: parent.width - 9
            y: 8
            width: 5
            height: parent.height - 20
            radius: 2
            antialiasing: true
            color: Qt.lighter(furniture.glowColor, 1.35)
            opacity: furniture.current ? 0.55 : 0
            Behavior on opacity { NumberAnimation { duration: 150 } }
        }
    }

    // The label plate: the raised moulding framing the tape well the display
    // kit lays in displayRect, screwed down at its corners.
    ShaderEffect {
        id: labelPlate

        x: furniture.displayRect.x - 10
        y: -furniture.plateReach
        width: furniture.displayRect.width + 25
        height: furniture.height + 2 * furniture.plateReach

        property size sizePx: Qt.size(width, height)
        property vector2d lightDir: metrics.castingLightDir
        property color baseColor: furniture.plateFace
        property color highlightColor: furniture.plateHighlight
        property color shadowColor: furniture.plateShadow
        property real cornerRadius: 5
        property real bevelPx: 2
        property real grainAmount: 0.35
        property real mottleAmount: 0.7
        property real scratchAmount: 0.5
        property real vignetteStrength: 0.3
        property real wearAmount: 0.4
        property real seamGain: 0.6
        property real seed: furniture.rowSeed + 0.41

        vertexShader: "qrc:/shaders/plate_metal.vert.qsb"
        fragmentShader: "qrc:/shaders/plate_metal.frag.qsb"

        onStatusChanged: if (log) console.log(log)

        ScrewHead {
            x: -1; y: -1
            width: 13; height: 13
            metalLight: "#8b929c"
            metalMid: "#3a3f46"
            metalDark: "#0a0b0e"
            glint: "#d8dde4"
            slotAngle: 24
            lightX: metrics.castingLightDir.x
            lightY: metrics.castingLightDir.y
        }
        ScrewHead {
            x: parent.width - 12; y: -1
            width: 13; height: 13
            metalLight: "#8b929c"
            metalMid: "#3a3f46"
            metalDark: "#0a0b0e"
            glint: "#d8dde4"
            slotAngle: -58
            lightX: metrics.castingLightDir.x
            lightY: metrics.castingLightDir.y
        }
        ScrewHead {
            x: -1; y: parent.height - 12
            width: 13; height: 13
            metalLight: "#8b929c"
            metalMid: "#3a3f46"
            metalDark: "#0a0b0e"
            glint: "#d8dde4"
            slotAngle: 81
            lightX: metrics.castingLightDir.x
            lightY: metrics.castingLightDir.y
        }
        ScrewHead {
            x: parent.width - 12; y: parent.height - 12
            width: 13; height: 13
            metalLight: "#8b929c"
            metalMid: "#3a3f46"
            metalDark: "#0a0b0e"
            glint: "#d8dde4"
            slotAngle: -13
            lightX: metrics.castingLightDir.x
            lightY: metrics.castingLightDir.y
        }
    }

    // The bevel ring dropping from the moulding to the tape well: a dark cut
    // all round, and a lit line along the lower edge where the light catches
    // the far wall of the sink.
    Rectangle {
        x: furniture.displayRect.x - metrics.stripPadding - 1
        y: furniture.displayRect.y - metrics.stripPadding - 1
        width: furniture.displayRect.width + 2 * metrics.stripPadding + 2
        height: furniture.displayRect.height + 2 * metrics.stripPadding + 2
        radius: 5
        antialiasing: true
        color: "transparent"
        border.width: 2
        border.color: "#0b0d10"
    }
    Rectangle {
        x: furniture.displayRect.x - 2
        y: furniture.displayRect.y + furniture.displayRect.height + 2
        width: furniture.displayRect.width + 4
        height: 1
        color: "#79818c"
        opacity: 0.55
    }
}
