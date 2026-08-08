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

// A slotted screw head on its boss, shared by the RobCo shells: a recessed
// countersink ring, a raised domed head with its glint on the side the key
// light lands, a dark slot with a lit lower edge, and a shadow arc opposite
// the glint. All measures scale with the item's size; the slot angle is the
// caller's, as no two screws left a factory aligned.
Canvas {
    id: screw

    property color metalLight: "#c1a585"
    property color metalMid: "#5c4f40"
    property color metalDark: "#171008"
    property color glint: "#f5efe2"
    property real slotAngle: 20
    // Unit vector-ish direction the light comes from, in item coordinates.
    property real lightX: -0.6
    property real lightY: -0.8

    width: 28
    height: 28

    onPaint: {
        var ctx = getContext("2d")
        ctx.reset()
        var r = width / 2
        var cx = r
        var cy = r
        var ll = Math.sqrt(lightX * lightX + lightY * lightY) || 1
        var lx = lightX / ll
        var ly = lightY / ll

        // Countersink: a dark ring the boss sinks into.
        var sink = ctx.createRadialGradient(cx, cy, r * 0.55, cx, cy, r)
        sink.addColorStop(0.0, Qt.rgba(0, 0, 0, 0))
        sink.addColorStop(0.75, Qt.rgba(0, 0, 0, 0.55))
        sink.addColorStop(1.0, Qt.rgba(0, 0, 0, 0.1))
        ctx.beginPath()
        ctx.arc(cx, cy, r, 0, Math.PI * 2)
        ctx.fillStyle = sink
        ctx.fill()

        // The head: domed, glinting where the light lands.
        var hr = r * 0.78
        var gx = cx + lx * hr * 0.45
        var gy = cy + ly * hr * 0.45
        var dome = ctx.createRadialGradient(gx, gy, hr * 0.08, cx, cy, hr)
        dome.addColorStop(0.0, screw.glint)
        dome.addColorStop(0.22, screw.metalLight)
        dome.addColorStop(0.62, screw.metalMid)
        dome.addColorStop(1.0, screw.metalDark)
        ctx.beginPath()
        ctx.arc(cx, cy, hr, 0, Math.PI * 2)
        ctx.fillStyle = dome
        ctx.fill()

        // Rim: a lit arc toward the light, a shadow arc away from it.
        var la = Math.atan2(ly, lx)
        ctx.lineWidth = Math.max(1, r * 0.09)
        ctx.beginPath()
        ctx.arc(cx, cy, hr - ctx.lineWidth / 2, la - 1.2, la + 1.2)
        ctx.strokeStyle = Qt.rgba(screw.glint.r, screw.glint.g, screw.glint.b, 0.7)
        ctx.stroke()
        ctx.beginPath()
        ctx.arc(cx, cy, hr - ctx.lineWidth / 2, la + Math.PI - 1.3, la + Math.PI + 1.3)
        ctx.strokeStyle = Qt.rgba(0, 0, 0, 0.55)
        ctx.stroke()

        // The slot, cut through the dome, its far wall catching light.
        ctx.save()
        ctx.translate(cx, cy)
        ctx.rotate(slotAngle * Math.PI / 180)
        var sw = hr * 1.5
        var sh = Math.max(2, r * 0.2)
        ctx.fillStyle = screw.metalDark
        ctx.fillRect(-sw / 2, -sh / 2, sw, sh)
        ctx.fillStyle = Qt.rgba(screw.metalLight.r, screw.metalLight.g,
                                screw.metalLight.b, 0.8)
        ctx.fillRect(-sw / 2, sh / 2 - 1, sw, 1)
        ctx.fillStyle = Qt.rgba(0, 0, 0, 0.8)
        ctx.fillRect(-sw / 2, -sh / 2, sw, 1)
        ctx.restore()
    }

    onMetalLightChanged: requestPaint()
    onSlotAngleChanged: requestPaint()
}
