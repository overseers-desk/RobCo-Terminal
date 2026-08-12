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

// The blue appliance's chassis: bare scratched gunmetal, with the numerals
// and windows punched straight into it. There is no plate, so this is the
// frame's chassis law continued under the bank and nothing more: heavy grain,
// blotchy stains, wear streaks, the room's vignette pooling the corners dark.
// The carrier rail and its hinge bracket stand on this metal as furniture:
// the rail belongs to the chassis and survives the profile's indicator law,
// while the shoe that rides it is the bank's, raised beside the channel on
// screen.
//
// It occupies only the ground the bank stands on, never a sheet behind the
// screen: a see-through profile has to look through the tube onto the desktop,
// and any metal left under the glass would be a second veil over the picture.
ShaderEffect {
    id: chassis

    // The item the frame shader fills; this metal continues its field.
    property Item frameRegion

    // The frame's own measurements, in the units its shader works in: this
    // metal is placed in that field, not in one of its own.
    property size viewportSize: Qt.size(_fieldWidth, _fieldHeight)

    property size fieldScale: Qt.size(width / _fieldWidth, height / _fieldHeight)

    property size fieldOffset: frameRegion
        ? Qt.size((x - frameRegion.x) / _fieldWidth, (y - frameRegion.y) / _fieldHeight)
        : Qt.size(0, 0)

    readonly property real _fieldWidth: frameRegion ? Math.max(1, frameRegion.width) : 1
    readonly property real _fieldHeight: frameRegion ? Math.max(1, frameRegion.height) : 1

    // The frame's own chassis law, continued leftwards: one casting, one
    // shared light and metal color, read off this shell's Metrics.
    property vector2d lightDir: metrics.castingLightDir
    property color chassisColor: metrics.castingColor
    property real grainAmount: 0.4
    property real mottleAmount: 1.35
    property real scratchAmount: 0.7
    property real vignetteStrength: 0.3

    // The body takes the tube's translucency, so a see-through profile is one
    // set and not a screen cut into an opaque box.
    opacity: appSettings.windowOpacity * 0.3 + 0.7

    blending: false

    vertexShader: "qrc:/shaders/chassis_metal.vert.qsb"
    fragmentShader: "qrc:/shaders/chassis_metal.frag.qsb"

    onStatusChanged: if (log) console.log(log)

    // The carrier rail: a full-height proud metal rail with a deep slot
    // milled down it between machined double-groove edges, and an angular
    // sheet-metal hinge bracket bolted over its head with three slotted
    // screws. Measured off the mock: rail x 29..69, standing 29 off the
    // chassis top and foot, groove 46..55 in bank coordinates, bracket
    // [18,48]-[116,120]. The slot is cut here; what rides it is the bank's.
    Metrics { id: metrics }

    Item {
        id: rail

        x: 29
        width: metrics.trackWidth
        anchors {
            top: parent.top
            bottom: parent.bottom
            topMargin: 29
            bottomMargin: 29
        }

        readonly property color railMetal: "#2b2418"
        readonly property color grooveDark: "#030202"
        readonly property color bracketLight: "#b2a47d"
        readonly property color bracketDark: "#241e14"
        readonly property color screwGlint: "#e3dfd2"
        readonly property color machinedLight: "#5c5344"

        // The rail body: aged metal, worn bright down its milled edges.
        ShaderEffect {
            anchors.fill: parent

            property size sizePx: Qt.size(width, height)
            property vector2d lightDir: Qt.vector2d(-0.55, -0.85)
            property color baseColor: rail.railMetal
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

        // The milled slot with its machined double-groove edges: on each side
        // a bright turned line and a dark cut line, then the near-black slot
        // whose right wall alone catches the room.
        Item {
            id: grooveAssembly

            x: 15
            width: 15
            anchors {
                top: parent.top
                bottom: parent.bottom
                topMargin: 5
                bottomMargin: 12
            }

            Rectangle { x: 0; width: 1; height: parent.height; color: rail.machinedLight; opacity: 0.8 }
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
                    GradientStop { position: 0.0; color: rail.grooveDark }
                    GradientStop { position: 0.65; color: Qt.lighter(rail.grooveDark, 2.6) }
                    GradientStop { position: 1.0; color: Qt.lighter(rail.grooveDark, 6.0) }
                }
                // The lit endcap at the slot's foot.
                Rectangle {
                    anchors {
                        left: parent.left
                        right: parent.right
                        bottom: parent.bottom
                    }
                    height: 2
                    color: Qt.lighter(rail.railMetal, 2.6)
                }
            }
            Rectangle { x: 12; width: 2; height: parent.height; color: "#0a0806" }
            Rectangle { x: 14; width: 1; height: parent.height; color: rail.machinedLight; opacity: 0.6 }
        }

        // The hinge bracket over the rail's head: an angular sheet-metal
        // plate, wider at the left, its right end tapering toward the numeral
        // column with the top corner cut, three slotted screws holding it
        // down. Bank coordinates [18,48]-[78,120]: the tab ends a sliver
        // short of the stamped digits, whose column starts hard after the
        // rail. The rail starts at bank x 29, y 29.
        Item {
            id: bracket

            x: -11
            y: 19
            width: 60
            height: 72

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
                    body.addColorStop(0.0, rail.bracketLight)
                    body.addColorStop(0.45, Qt.lighter(rail.bracketDark, 3.4))
                    body.addColorStop(1.0, Qt.lighter(rail.bracketDark, 1.3))
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
                        ctx.fillStyle = dark2 ? Qt.rgba(0.05, 0.04, 0.02, 0.06 + 0.08 * rnd())
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

            // Screw centres in bank coordinates: (46,69) (64,84) (47,102);
            // the middle one rides the tab.
            ScrewHead {
                x: 46 - 18 - 11; y: 69 - 48 - 11
                width: 22; height: 22
                metalLight: rail.bracketLight
                metalMid: "#4a4234"
                metalDark: "#0d0a06"
                glint: rail.screwGlint
                slotAngle: 32
            }
            ScrewHead {
                x: 64 - 18 - 11; y: 84 - 48 - 11
                width: 22; height: 22
                metalLight: rail.bracketLight
                metalMid: "#4a4234"
                metalDark: "#0d0a06"
                glint: rail.screwGlint
                slotAngle: -63
            }
            ScrewHead {
                x: 47 - 18 - 11; y: 102 - 48 - 11
                width: 22; height: 22
                metalLight: rail.bracketLight
                metalMid: "#4a4234"
                metalDark: "#0d0a06"
                glint: rail.screwGlint
                slotAngle: 74
            }
        }
    }
}
