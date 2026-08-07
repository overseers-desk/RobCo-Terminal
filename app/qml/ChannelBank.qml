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

// The bank of channel rows set into the chassis, left of the screen well. It
// draws no background of its own: the chassis plastic behind it is the panel.
// Its width follows the LED strip settings alone and never the window's size,
// so dragging the window edge cannot move the screen well's left edge.
Item {
    id: bank

    readonly property color plastic: Utils.frameBaseColor(
        appSettings.frameColor,
        appSettings.fontColor,
        appSettings.backgroundColor,
        appSettings.ambientLight
    )

    readonly property int bankPadding: 10
    readonly property int rowSpacing: 6
    readonly property int columnGap: 6
    readonly property int numeralWidth: 20
    readonly property int buttonWidth: 24
    readonly property int buttonHeight: 18
    // Sized so a channel row's pitch sits near two terminal text rows
    // (25 px at the default profile), the bank's readability target.
    readonly property int stripPadding: 10
    readonly property int minRowHeight: 26

    readonly property int stripWidth: appSettings.ledCellWidth * appSettings.ledCharacters * appSettings.ledDotPitch
    readonly property int stripHeight: appSettings.ledCellHeight * appSettings.ledDotPitch
    readonly property int rowHeight: Math.max(minRowHeight, stripHeight + 2 * stripPadding)

    implicitWidth: 2 * bankPadding + numeralWidth + columnGap + buttonWidth + columnGap + stripWidth

    readonly property int rowCount: Math.max(1, Math.floor(
        (height - 2 * bankPadding + rowSpacing) / (rowHeight + rowSpacing)))

    clip: true

    // The press of a preset: a dark slot starts a session on it, an open one
    // comes to the screen, and either way the shell gets the keyboard back.
    function press(channel) {
        if (terminalChannels.channelState[channel] === undefined)
            terminalChannels.openChannel(channel)
        else
            terminalChannels.selectChannel(channel)
        terminalChannels.activateCurrent()
    }

    Column {
        x: bank.bankPadding
        y: bank.bankPadding
        spacing: bank.rowSpacing

        Repeater {
            model: bank.rowCount

            ChannelRow {
                readonly property var slotTitle: terminalChannels.channelState[channel]

                channel: index + 1
                width: bank.width - 2 * bank.bankPadding
                height: bank.rowHeight
                plastic: bank.plastic
                numeralWidth: bank.numeralWidth
                columnGap: bank.columnGap
                buttonWidth: bank.buttonWidth
                buttonHeight: bank.buttonHeight
                stripPadding: bank.stripPadding
                open: slotTitle !== undefined
                title: slotTitle !== undefined ? slotTitle : ""
                current: terminalChannels.currentChannel === channel
                onActivated: bank.press(channel)
            }
        }
    }
}
