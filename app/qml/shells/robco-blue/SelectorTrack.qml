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

// The blue appliance's carrier: a full-height proud metal rail with a deep
// slot milled down it between machined double-groove edges, an angular
// sheet-metal hinge bracket bolted over its head with three slotted screws,
// and the carriage shoe riding the slot. Measured off the mock: rail x 29..70
// (this item's width), slot 18..27 within it, bracket [18,78]-[118,152] in
// bank coordinates. It reads the panel and reports nothing back; there is no
// mouse area.
Item {
    id: track

    property color plastic: "#4f4737"
    // Where the carriage belongs, as a centre in this item's coordinates.
    property real targetY: height / 2

    readonly property color railMetal: "#231e16"
    readonly property color grooveDark: "#030202"
    readonly property color bracketLight: "#8e8a6e"
    readonly property color bracketDark: "#241e14"
    readonly property color screwGlint: "#e3dfd2"
    readonly property color machinedLight: "#5c5344"

    readonly property int clampHeight: 26

    implicitWidth: 41

    // The rail body: aged metal, worn bright down its milled edges.
    ShaderEffect {
        anchors.fill: parent

        property size sizePx: Qt.size(width, height)
        property vector2d lightDir: Qt.vector2d(-0.55, -0.85)
        property color baseColor: track.railMetal
        property color highlightColor: "#7d735c"
        property color shadowColor: "#050403"
        property real cornerRadius: 4
        property real bevelPx: 2
        property real grainAmount: 0.35
        property real mottleAmount: 0.7
        property real scratchAmount: 0.5
        property real vignetteStrength: 0.35
        property real wearAmount: 0.35
        property real seamGain: 0.6
        property real seed: 0.53

        vertexShader: "qrc:/shaders/plate_metal.vert.qsb"
        fragmentShader: "qrc:/shaders/plate_metal.frag.qsb"

        onStatusChanged: if (log) console.log(log)
    }

    // The milled slot with its machined double-groove edges: on each side a
    // bright turned line and a dark cut line, then the near-black slot whose
    // right wall alone catches the room.
    Item {
        id: grooveAssembly

        x: 15
        width: 15
        anchors {
            top: parent.top
            bottom: parent.bottom
            topMargin: 4
            bottomMargin: 11
        }

        Rectangle { x: 0; width: 1; height: parent.height; color: track.machinedLight; opacity: 0.8 }
        Rectangle { x: 1; width: 2; height: parent.height; color: "#0a0806" }
        Rectangle {
            id: groove
            x: 3
            width: 9
            height: parent.height
            radius: 3
            antialiasing: true
            gradient: Gradient {
                orientation: Gradient.Horizontal
                GradientStop { position: 0.0; color: track.grooveDark }
                GradientStop { position: 0.65; color: Qt.lighter(track.grooveDark, 2.6) }
                GradientStop { position: 1.0; color: Qt.lighter(track.grooveDark, 6.0) }
            }
            // The lit endcap at the slot's foot.
            Rectangle {
                anchors {
                    left: parent.left
                    right: parent.right
                    bottom: parent.bottom
                }
                height: 2
                color: Qt.lighter(track.railMetal, 2.6)
            }
        }
        Rectangle { x: 12; width: 2; height: parent.height; color: "#0a0806" }
        Rectangle { x: 14; width: 1; height: parent.height; color: track.machinedLight; opacity: 0.6 }
    }

    // The carriage: a bolted steel shoe, specular ridge across its cap, hard
    // shadow under its foot.
    Item {
        id: clamp

        x: grooveAssembly.x + groove.x - 6
        y: Math.round(track.targetY - height / 2)
        width: groove.width + 12
        height: track.clampHeight

        Behavior on y {
            NumberAnimation { duration: 150; easing.type: Easing.OutCubic }
        }

        // Shadow under the shoe.
        Rectangle {
            x: 1
            y: 2
            width: parent.width
            height: parent.height
            radius: 3
            color: "#000000"
            opacity: 0.5
        }

        Rectangle {
            anchors.fill: parent
            radius: 3
            antialiasing: true
            gradient: Gradient {
                GradientStop { position: 0.0; color: track.bracketLight }
                GradientStop { position: 0.18; color: Qt.lighter(track.railMetal, 2.0) }
                GradientStop { position: 0.55; color: Qt.lighter(track.railMetal, 1.3) }
                GradientStop { position: 1.0; color: "#0a0704" }
            }

            // The specular ridge machined across the cap.
            Rectangle {
                x: 2
                y: 3
                width: parent.width - 4
                height: 1
                color: track.screwGlint
                opacity: 0.9
            }
            Rectangle {
                x: 2
                y: 4
                width: parent.width - 4
                height: 1
                color: "#000000"
                opacity: 0.5
            }
            // Side bevels.
            Rectangle { x: 0; y: 2; width: 1; height: parent.height - 4; color: track.bracketLight; opacity: 0.5 }
            Rectangle { x: parent.width - 1; y: 2; width: 1; height: parent.height - 4; color: "#000000"; opacity: 0.6 }
        }
    }

    // The hinge bracket over the rail's head: an angular sheet-metal plate,
    // wider at the left, its right end tapering toward the numeral column
    // with the top corner cut, three slotted screws holding it down.
    // Bank coordinates [18,78]-[118,152]; the rail starts at bank x 29, y 29.
    Item {
        id: bracket

        x: -11
        y: 49
        width: 100
        height: 74

        // The bracket's shadow on rail and chassis.
        Canvas {
            anchors.fill: parent
            anchors.margins: -4
            opacity: 0.45
            onPaint: {
                var ctx = getContext("2d")
                ctx.reset()
                ctx.translate(7, 9)
                ctx.fillStyle = "#000000"
                bracket.tracePlate(ctx, bracket.width, bracket.height)
                ctx.fill()
            }
        }

        Canvas {
            id: bracketPlate
            anchors.fill: parent
            onPaint: {
                var ctx = getContext("2d")
                ctx.reset()
                var w = width
                var h = height

                // Body: brushed sheet metal lit from the upper left.
                var body = ctx.createLinearGradient(0, 0, w * 0.9, h)
                body.addColorStop(0.0, track.bracketLight)
                body.addColorStop(0.35, Qt.lighter(track.bracketDark, 1.9))
                body.addColorStop(1.0, track.bracketDark)
                bracket.tracePlate(ctx, w, h)
                ctx.fillStyle = body
                ctx.fill()

                // Weathering in two registers: broad patina blotches first,
                // then a dense scatter of grime and glint specks over them.
                var s = 43
                function rnd() { s = (s * 16807) % 2147483647; return s / 2147483647 }
                bracket.tracePlate(ctx, w, h)
                ctx.save()
                ctx.clip()
                for (var b = 0; b < 26; b++) {
                    var bx = rnd() * w
                    var by = rnd() * h
                    var br = 4 + rnd() * 14
                    var dark2 = rnd() < 0.55
                    ctx.beginPath()
                    ctx.ellipse(bx - br, by - br * (0.5 + rnd() * 0.5), br * 2, br * (1.0 + rnd()))
                    ctx.fillStyle = dark2 ? Qt.rgba(0.05, 0.04, 0.02, 0.10 + 0.10 * rnd())
                                          : Qt.rgba(0.75, 0.72, 0.58, 0.05 + 0.07 * rnd())
                    ctx.fill()
                }
                for (var i = 0; i < 700; i++) {
                    var px = rnd() * w
                    var py = rnd() * h
                    var dark = rnd() < 0.72
                    ctx.fillStyle = dark ? Qt.rgba(0, 0, 0, 0.10 + 0.14 * rnd())
                                         : Qt.rgba(0.9, 0.88, 0.8, 0.05 + 0.10 * rnd())
                    ctx.fillRect(px, py, 1 + rnd() * 1.5, 1)
                }
                ctx.restore()

                // Cut edges: lit along top and left, dark along bottom/right.
                bracket.tracePlate(ctx, w, h)
                ctx.lineWidth = 2
                ctx.strokeStyle = Qt.rgba(0, 0, 0, 0.65)
                ctx.stroke()
                ctx.beginPath()
                ctx.moveTo(3, h - 6)
                ctx.lineTo(3, 6)
                ctx.lineTo(w * 0.58, 1.5)
                ctx.lineTo(w - 8, h * 0.28)
                ctx.lineWidth = 1.5
                ctx.strokeStyle = Qt.rgba(0.92, 0.9, 0.82, 0.75)
                ctx.stroke()

                // The pressed hinge knuckle down the plate's left side.
                ctx.beginPath()
                ctx.moveTo(12, 4)
                ctx.lineTo(12, h - 4)
                ctx.lineWidth = 2
                ctx.strokeStyle = Qt.rgba(0, 0, 0, 0.4)
                ctx.stroke()
                ctx.beginPath()
                ctx.moveTo(14, 5)
                ctx.lineTo(14, h - 5)
                ctx.lineWidth = 1
                ctx.strokeStyle = Qt.rgba(0.9, 0.88, 0.8, 0.35)
                ctx.stroke()
            }
        }

        // The plate's outline: full-height at the left, the right end
        // tapering to a tab with its top corner cut off.
        function tracePlate(ctx, w, h) {
            ctx.beginPath()
            ctx.moveTo(2, 8)
            ctx.lineTo(w * 0.58, 3)
            ctx.lineTo(w - 2, h * 0.30)
            ctx.lineTo(w - 2, h * 0.62)
            ctx.lineTo(w * 0.62, h - 3)
            ctx.lineTo(2, h - 6)
            ctx.closePath()
        }

        // Screw centres in bank coordinates: (45,103) (102,115) (47,134).
        ScrewHead {
            x: 45 - 18 - 11; y: 103 - 78 - 11
            width: 22; height: 22
            metalLight: track.bracketLight
            metalMid: "#4a4234"
            metalDark: "#0d0a06"
            glint: track.screwGlint
            slotAngle: 32
        }
        ScrewHead {
            x: 102 - 18 - 11; y: 115 - 78 - 11
            width: 22; height: 22
            metalLight: track.bracketLight
            metalMid: "#4a4234"
            metalDark: "#0d0a06"
            glint: track.screwGlint
            slotAngle: -63
        }
        ScrewHead {
            x: 47 - 18 - 11; y: 134 - 78 - 11
            width: 22; height: 22
            metalLight: track.bracketLight
            metalMid: "#4a4234"
            metalDark: "#0d0a06"
            glint: track.screwGlint
            slotAngle: 74
        }
    }
}
