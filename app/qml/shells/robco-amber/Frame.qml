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

// The amber appliance's slim bezel, sliced whole from the mock: the metal
// bands, the groove against the chassis, the lit right side and the plate's
// drop shadow on the left margin are the mock's pixels. The glass interior
// is carved to alpha in the asset, so the live tube shows through it and
// the terminal shader composites this image as the frame. Chassis.qml and
// this slice are two halves of the same photograph, so the bank column and
// the CRT frame keep reading as one piece.
BorderImage {
    source: "assets/frame.png"
    border { left: 100; right: 100; top: 100; bottom: 100 }
    horizontalTileMode: BorderImage.Repeat
    verticalTileMode: BorderImage.Repeat
}
