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
import QtQuick 2.2

import "utils.js" as Utils

// The appliance body the channel bank and the screen are set into. Its plastic
// is mixed from the same profile inputs the frame shader lights, so the body
// and the bezel are one moulding whichever profile is loaded.
Rectangle {
    id: chassis

    // The item occupying the screen well; the body is recessed around it.
    property Item well

    readonly property color plastic: Utils.frameBaseColor(
        appSettings.frameColor,
        appSettings.fontColor,
        appSettings.backgroundColor,
        appSettings.ambientLight
    )

    opacity: appSettings.windowOpacity * 0.3 + 0.7

    gradient: Gradient {
        GradientStop { position: 0.0; color: Utils.scaleColor(chassis.plastic, 1.12) }
        GradientStop { position: 1.0; color: Utils.scaleColor(chassis.plastic, 0.86) }
    }

    Rectangle {
        anchors { left: parent.left; right: parent.right; top: parent.top }
        height: 1
        color: Utils.scaleColor(chassis.plastic, 1.35)
    }

    Rectangle {
        anchors { left: parent.left; right: parent.right; bottom: parent.bottom }
        height: 1
        color: Utils.scaleColor(chassis.plastic, 0.62)
    }

    Rectangle {
        readonly property point origin: chassis.well
            ? chassis.mapFromItem(chassis.well.parent, chassis.well.x, chassis.well.y)
            : Qt.point(0, 0)

        visible: chassis.well !== null
        x: origin.x - border.width
        y: origin.y - border.width
        width: (chassis.well ? chassis.well.width : 0) + 2 * border.width
        height: (chassis.well ? chassis.well.height : 0) + 2 * border.width
        color: "transparent"
        border.width: 2
        border.color: Utils.scaleColor(chassis.plastic, 0.55)
    }
}
