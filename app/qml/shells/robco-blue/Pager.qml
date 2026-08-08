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

// The page selector at the bank's foot, always on station as the mock keeps
// it: three holes punched into the chassis, each with the lit bevel lip
// under it. PREV and NEXT are dark metal caps carrying engraved labels and
// solid arrows; between them an LED window shows the page the bank is on,
// through the same display kit as the rows. The pieces are slices of the
// mock; only their stations flex: PREV holds the left edge, NEXT hugs the
// bank's right edge, and the window keeps the middle, so a narrower bank
// draws the group together instead of clipping NEXT away.
Item {
    id: pager

    property color plastic: "#4f4737"
    property int pageIndex: 0
    property int pageCount: 1
    property int columnGap: 25

    signal step(int direction)

    // The recesses run y 932..1021 with their bevel lips at 1022.
    implicitHeight: 91
    implicitWidth: 287

    // The keys' stations: PREV pinned 14px in, NEXT's right edge 3px off
    // the bank's; at the mock's width these are the mock's own stations.
    readonly property int prevX: 14
    readonly property int nextX: width - 70 - 3
    // The page window, centred between the keys.
    readonly property int winX: Math.round((prevX + 70 + nextX) / 2 - 45)

    // The dark floor of the page window's punched hole, under the lamps.
    Rectangle {
        x: pager.winX + 8
        y: 1
        width: 77
        height: 85
        radius: 2
        color: "#050505"
    }

    // The live page number: the LED display kit at two characters, its
    // lamps grown to the window's scale. The digits read the page as the
    // numerals do: 1-based.
    Loader {
        id: pageDisplay

        x: pager.winX + 46 - 12
        y: 34
        scale: 3.0
        // Resolved against this shell's directory, so the kit path walks
        // back to the QML root first.
        source: "../../" + appSettings.displayUrl("Display")

        Binding {
            target: pageDisplay.item
            property: "characters"
            value: 2
        }
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
            value: ("0" + (pager.pageIndex + 1)).slice(-2)
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
        // The window's recess swallows the lamps' throw on the mock; no
        // spill on the bezel.
        Binding {
            target: pageDisplay.item
            property: "spillStrength"
            value: 0.0
        }
    }

    // The mock's own pieces, engraving, recess shadow and bevel lip baked.
    Image {
        x: pager.prevX - 5
        y: -5
        source: "assets/prev.png"
    }
    Image {
        x: pager.winX
        y: -6
        source: "assets/pagewin.png"
    }
    Image {
        x: pager.nextX - 5
        y: -5
        source: "assets/next.png"
    }

    // A key's press: the cap face sinks into shadow while held. The caps
    // are bolted furniture and stay full strength on a single page; the
    // page window reading 01 with nowhere to go is the selector's own tell.
    component Key: Item {
        id: key

        property int direction: -1

        width: 70
        height: 90

        Rectangle {
            x: 5
            y: 5
            width: 61
            height: 76
            radius: 4
            color: "black"
            opacity: press.pressed && press.enabled ? 0.3 : 0.0
        }
        MouseArea {
            id: press

            anchors.fill: parent
            acceptedButtons: Qt.LeftButton
            enabled: pager.pageCount > 1
            onClicked: pager.step(key.direction)
        }
    }

    Key {
        x: pager.prevX
        direction: -1
    }
    Key {
        x: pager.nextX
        direction: 1
    }
}
