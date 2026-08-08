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

// The amber appliance's PREV/NEXT rocker, at the mock's own stations: labels
// engraved in the plate above the keys, solid arrows pointing outward beside
// them, ridged key caps below. The whole block keeps the height the mock
// gives it, so the bank fits its nineteen rows above.
Item {
    id: pager

    property color plastic: "#241e19"
    property int pageIndex: 0
    property int pageCount: 1
    property int columnGap: 24

    signal step(int direction)

    readonly property color labelColor: "#6a5642"

    enabled: pageCount > 1
    opacity: enabled ? 1.0 : 0.55

    // Key tops sit 38px into the block; the plate runs on below them.
    implicitHeight: 130

    // A solid triangle at label height, pointing away from its key and
    // standing clear of its label, as the mock prints them.
    component Arrow: Text {
        font.pixelSize: 14
        color: pager.labelColor
    }

    Arrow {
        x: 56
        y: 6
        text: "\u25C0"
    }
    Text {
        x: 102 - width / 2
        y: 5
        font.pixelSize: 15
        font.bold: true
        font.letterSpacing: 2
        text: "PREV"
        color: pager.labelColor
    }

    Text {
        x: 247 - width / 2
        y: 5
        font.pixelSize: 15
        font.bold: true
        font.letterSpacing: 2
        text: "NEXT"
        color: pager.labelColor
    }
    Arrow {
        x: 274
        y: 6
        text: "\u25B6"
    }

    ChannelButton {
        x: 64
        y: 38
        width: 56
        height: 40
        plastic: pager.plastic
        onClicked: pager.step(-1)
    }

    ChannelButton {
        x: 212
        y: 38
        width: 56
        height: 40
        plastic: pager.plastic
        onClicked: pager.step(1)
    }
}
