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

// The blue appliance's carrier. The rail, its milled slot, the hinge
// bracket and its screws are the mock's pixels, baked into the chassis
// slice behind this item; what moves is only the carriage, and that is a
// slotted screw head cut from the same photograph, riding the slot beside
// the channel on screen. It reads the panel and reports nothing back;
// there is no mouse area.
Item {
    id: track

    // Unread here: the carriage is cut from the mock's own photograph and
    // carries no live colour. Kept because ChannelBank's generic selector
    // Loader Binding assigns item.plastic for every shell's SelectorTrack
    // alike; deleting it breaks instantiation here with a non-obvious error.
    property color plastic: "#4f4737"
    // Where the carriage belongs, as a centre in this item's coordinates.
    property real targetY: height / 2

    implicitWidth: 41

    // The shadow the carriage drops into the slot beneath it.
    Rectangle {
        x: knob.x + 3
        y: knob.y + 3
        width: knob.width - 3
        height: knob.height - 2
        radius: height / 2
        color: "#000000"
        opacity: 0.5
    }

    // The carriage: a slotted bolt head astride the slot (local x 18..27).
    Image {
        id: knob

        x: 22 - width / 2
        y: Math.round(track.targetY - height / 2)
        width: 27
        height: 27
        source: "assets/knob.png"

        // The travel of a hand moving the carriage, not of a lamp fading up.
        Behavior on y {
            NumberAnimation { duration: 150; easing.type: Easing.OutCubic }
        }
    }
}
