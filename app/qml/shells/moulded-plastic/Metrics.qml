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
import QtQml

// This shell's rule for the bank column: how much air the moulding leaves
// around the rows, and how wide its fixed furniture stands.
QtObject {
    readonly property int bankPadding: 10
    readonly property int rowSpacing: 6
    readonly property int columnGap: 10
    readonly property int numeralWidth: 34
    // Sized so a channel row's pitch sits near two terminal text rows
    // (25 px at the default profile), the bank's readability target.
    readonly property int stripPadding: 13
    readonly property int minRowHeight: 26
    // The selector's lane, when the profile asks for one.
    readonly property int trackWidth: 14
}
