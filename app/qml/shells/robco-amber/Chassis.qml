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

// The amber appliance's chassis: the mock's own bank column, sliced whole.
// The raised plate, its bevelled moulding, the four slotted corner screws
// and the patina are the mock's pixels; only the furniture that moves at
// runtime (windows, numerals, pager) was cleaned off the slice. The border
// insets keep the screws and the moulding out of the tiled middle, so the
// column can stand taller or wider than the mock without smearing them.
Item {
    id: chassis

    // The window hands the frame's region over; a slice carries its own
    // lighting, so the field is baked and the handle is kept only for the
    // interface's sake.
    property Item frameRegion

    // The tube's translucency law, as the moulded shell keeps it: the body
    // fades with windowOpacity but never below the moulding's floor.
    opacity: appSettings.windowOpacity * 0.3 + 0.7

    BorderImage {
        anchors.fill: parent
        source: "assets/bank.png"
        border { left: 52; right: 48; top: 48; bottom: 56 }
        horizontalTileMode: BorderImage.Repeat
        verticalTileMode: BorderImage.Repeat
    }
}
