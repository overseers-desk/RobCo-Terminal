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

// The blue appliance's deep barrel-mouthed bezel, sliced whole from the
// mock: the moulded well, its bright top ridge, the dark right band, the
// bottom trough and the corner grime are the mock's pixels. The barrel
// glass is carved to alpha along the tuned rounded rect (its overshoot
// lands on the near-black sloped wall), so the live tube shows through and
// the terminal shader composites this image as the frame. Chassis.qml and
// this slice are two halves of the same photograph, so the bank column and
// the CRT frame keep reading as one piece.
BorderImage {
    source: "assets/frame.png"
    border { left: 150; right: 150; top: 150; bottom: 150 }
    horizontalTileMode: BorderImage.Repeat
    verticalTileMode: BorderImage.Repeat
}
