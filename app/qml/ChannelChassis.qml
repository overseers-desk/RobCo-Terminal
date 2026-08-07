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

import "utils.js" as Utils

// The appliance body the channel bank and the screen are set into. Its plastic
// is mixed from the same profile inputs the frame shader lights, so the body
// and the frame are one moulding whichever profile is loaded.
//
// The body is four panels around the screen well, never a sheet behind it: a
// see-through profile has to look through the tube onto the desktop, and any
// plastic left under the glass would be a second veil over the picture.
Item {
    id: chassis

    // The item occupying the screen well; the body is recessed around it.
    property Item well

    readonly property color plastic: Utils.frameBaseColor(
        appSettings.frameColor,
        appSettings.fontColor,
        appSettings.backgroundColor,
        appSettings.ambientLight
    )

    readonly property point wellOrigin: well
        ? mapFromItem(well.parent, well.x, well.y)
        : Qt.point(0, 0)
    readonly property real wellWidth: well ? well.width : 0
    readonly property real wellHeight: well ? well.height : 0

    opacity: appSettings.windowOpacity * 0.3 + 0.7

    // One moulding lit from above: each panel takes the slice of the body's
    // full-height shading that its own span covers.
    function shade(y) {
        return Utils.mix(Utils.scaleColor(plastic, 1.12),
                         Utils.scaleColor(plastic, 0.86),
                         height > 0 ? Utils.clamp(y / height, 0, 1) : 0)
    }

    Rectangle {
        width: chassis.width
        height: chassis.wellOrigin.y
        gradient: Gradient {
            GradientStop { position: 0.0; color: chassis.shade(0) }
            GradientStop { position: 1.0; color: chassis.shade(chassis.wellOrigin.y) }
        }

        Rectangle {
            anchors { left: parent.left; right: parent.right; top: parent.top }
            height: 1
            color: Utils.scaleColor(chassis.plastic, 1.35)
        }
    }

    Rectangle {
        y: chassis.wellOrigin.y
        width: chassis.wellOrigin.x
        height: chassis.wellHeight
        gradient: Gradient {
            GradientStop { position: 0.0; color: chassis.shade(chassis.wellOrigin.y) }
            GradientStop { position: 1.0; color: chassis.shade(chassis.wellOrigin.y + chassis.wellHeight) }
        }
    }

    Rectangle {
        x: chassis.wellOrigin.x + chassis.wellWidth
        y: chassis.wellOrigin.y
        width: chassis.width - x
        height: chassis.wellHeight
        gradient: Gradient {
            GradientStop { position: 0.0; color: chassis.shade(chassis.wellOrigin.y) }
            GradientStop { position: 1.0; color: chassis.shade(chassis.wellOrigin.y + chassis.wellHeight) }
        }
    }

    Rectangle {
        y: chassis.wellOrigin.y + chassis.wellHeight
        width: chassis.width
        height: chassis.height - y
        gradient: Gradient {
            GradientStop { position: 0.0; color: chassis.shade(chassis.wellOrigin.y + chassis.wellHeight) }
            GradientStop { position: 1.0; color: chassis.shade(chassis.height) }
        }

        Rectangle {
            anchors { left: parent.left; right: parent.right; bottom: parent.bottom }
            height: 1
            color: Utils.scaleColor(chassis.plastic, 0.62)
        }
    }

    Rectangle {
        visible: chassis.well !== null
        x: chassis.wellOrigin.x - border.width
        y: chassis.wellOrigin.y - border.width
        width: chassis.wellWidth + 2 * border.width
        height: chassis.wellHeight + 2 * border.width
        color: "transparent"
        border.width: 2
        border.color: Utils.scaleColor(chassis.plastic, 0.55)
    }
}
