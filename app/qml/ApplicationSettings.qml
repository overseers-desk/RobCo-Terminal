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
import QtQuick 2.4
import QtQuick.Controls 2.0
import CoolRetroTerm 1.0

import "utils.js" as Utils

QtObject {
    readonly property string version: appVersion
    readonly property int profileVersion: 1

    // STATIC CONSTANTS ////////////////////////////////////////////////////////
    readonly property real screenCurvatureSize: 0.6
    readonly property real minimumFontScaling: 0.25
    readonly property real maximumFontScaling: 2.50

    readonly property int defaultMargin: 16

    readonly property real minBurnInFadeTime: 0.16
    readonly property real maxBurnInFadeTime: 1.6

    readonly property real ledDotPitch: 1.5
    readonly property int minLedCharacters: 8
    // Rows of unlit lamps added above and below the glyph field, so the text
    // sits in a panel of lamps rather than filling the window lip to lip.
    readonly property int ledPadCells: 4
    // Unlit cells a bank strip adds at each end, outside the character count:
    // ledCharacters means visible characters, and these stand on the fixed-
    // furniture side of the width arithmetic. The display and its Metrics
    // both read this one number.
    readonly property int ledSidePadCells: 1

    property bool isMacOS: Qt.platform.os === "osx"

    // GENERAL SETTINGS ///////////////////////////////////////////////////////
    property bool showMenubar: false

    property bool showTerminalSize: true
    property real windowScaling: 1.0

    property int effectsFrameSkip: 3
    property bool verbose: false

    property real bloomQuality: 0.5
    property real burnInQuality: 0.5

    property bool blinkingCursor: false

    // How many characters of a channel title the LED windows carry. The user
    // sets it by the seam drag or the settings spinner and it is theirs, not a
    // profile's: switching look does not re-fit the bank.
    property int ledCharacters: 12

    // Whether the appliance stands at all: its casting, its bezel and the bank
    // that column carries, which come and go together. Theirs and not a
    // profile's, like the width above: a user who wants the whole window for
    // the screen says so once, and every appliance obeys until told otherwise.
    // The sessions are the store's and stand either way.
    property bool chassisShown: true

    // The face the bank's windows are lettered in. A reading choice like the
    // width above: the user picks the type they can read across the room and
    // every chassis letters its windows in it.
    property string ledFontName: "UNSCII_8_SCALED"


    // PROFILE SETTINGS ///////////////////////////////////////////////////////
    property real windowOpacity: 1.0
    property real ambientLight: 0.2
    property real contrast: 0.80
    property real brightness: 0.5

    property bool useCustomCommand: false
    property string customCommand: ""

    property string _backgroundColor: "#000000"
    property string _fontColor: "#ff8100"
    // How the bank marks the channel on screen: "glow" lights the window
    // itself, "pointer" runs a mechanical selector up the bank's left edge,
    // "switch" leaves the mark to a thrown toggle in the shell's own row
    // hardware and keeps every open window level.
    property string channelIndicator: "glow"
    // Which kit of components paints the appliance's body, and which display
    // sits in the bank's windows. Directories under shells/ and displays/;
    // a chassis that names neither wears the annunciator appliance.
    property string shell: "annunciator"
    property string channelDisplay: "led"
    property string saturatedColor: Utils.mix(Utils.strToColor(_fontColor), Utils.strToColor("#FFFFFF"), (saturationColor * 0.5))
    property color fontColor: Utils.mix(Utils.strToColor(_backgroundColor), Utils.strToColor(saturatedColor), (0.7 + (contrast * 0.3)))
    property color backgroundColor: Utils.mix(Utils.strToColor(saturatedColor), Utils.strToColor(_backgroundColor), (0.7 + (contrast * 0.3)))
    readonly property color frameColor: Utils.strToColor(_frameColor)

    property real staticNoise: 0.12
    property real screenCurvature: 0.3
    property real glowingLine: 0.2
    property real burnIn: 0.25
    property real bloom: 0.55

    property real chromaColor: 0.25
    property real saturationColor: 0.25

    property real jitter: 0.2

    property real horizontalSync: 0.08
    property real flickering: 0.1

    property real rgbShift: 0.0

    // THE BEZEL //////////////////////////////////////////////////////////////
    // Two bezels are kept at once, because two things can cut the glass. A
    // chassis is a cabinet built around a hole and carries the measurements of
    // that hole; a screen is a tube that came out of a factory in a moulding
    // of its own. Both records stand in store the whole time and travel with
    // their own profile, and which of them the renderers see is the standing
    // of the chassis: the cabinet's cut while it is around the tube, the
    // tube's own moulding once it is out.
    property real _frameShininessChassis: 0.3
    property real _frameSizeChassis: 0.45
    property real _screenRadiusChassis: 0.44
    property string _frameColorChassis: "#001735"

    property real _frameShininessScreen: 0.3
    property real _frameSizeScreen: 0.1
    property real _screenRadiusScreen: 0.1
    property string _frameColorScreen: "#cfcfcf"

    // The governing surface: the bezel the glass is actually cut with. Every
    // renderer reads these and none of them writes; the tuning UI moves the
    // bezel on view through the setters below, which route to whichever side
    // is governing, so a knob always moves the frame the user is looking at.
    readonly property real _frameShininess: chassisShown ? _frameShininessChassis : _frameShininessScreen
    readonly property real _frameSize: chassisShown ? _frameSizeChassis : _frameSizeScreen
    readonly property real _screenRadius: chassisShown ? _screenRadiusChassis : _screenRadiusScreen
    readonly property string _frameColor: chassisShown ? _frameColorChassis : _frameColorScreen

    function setFrameShininess(value) {
        if (chassisShown) _frameShininessChassis = value
        else _frameShininessScreen = value
    }
    function setFrameSize(value) {
        if (chassisShown) _frameSizeChassis = value
        else _frameSizeScreen = value
    }
    function setScreenRadius(value) {
        if (chassisShown) _screenRadiusChassis = value
        else _screenRadiusScreen = value
    }
    function setFrameColor(value) {
        if (chassisShown) _frameColorChassis = value
        else _frameColorScreen = value
    }

    readonly property real frameShininess: _frameShininess * 0.5
    readonly property real frameSize: _frameSize * 0.05
    readonly property real screenRadius: Utils.lint(4.0, 120.0, _screenRadius)

    property real _margin: 0.5
    property real margin: Utils.lint(1.0, 40.0, _margin) + (1.0 - Math.SQRT1_2) * screenRadius

    readonly property bool frameEnabled: ambientLight > 0 || _frameSize > 0 || screenCurvature > 0

    readonly property int no_rasterization: 0
    readonly property int scanline_rasterization: 1
    readonly property int pixel_rasterization: 2
    readonly property int subpixel_rasterization: 3
    readonly property int modern_rasterization: 4

    property alias rasterization: fontManager.rasterization

    readonly property int bundled_fonts: 0
    readonly property int system_fonts: 1

    property alias fontSource: fontManager.fontSource

    // FONTS //////////////////////////////////////////////////////////////////
    readonly property real baseFontScaling: 0.75
    property alias fontScaling: fontManager.fontScaling
    property real totalFontScaling: baseFontScaling * fontScaling

    property alias fontWidth: fontManager.fontWidth
    property alias lineSpacing: fontManager.lineSpacing

    property alias lowResolutionFont: fontManager.lowResolutionFont

    property alias fontName: fontManager.fontName
    property alias filteredFontList: fontManager.filteredFontList
    property alias lowResolutionFontList: fontManager.lowResolutionFontList

    property FontManager fontManager: FontManager {
        id: fontManager
        baseFontScaling: baseFontScaling
    }

    // LED strip geometry, derived from the chosen font so every strip agrees
    // on cell size.
    readonly property var _ledFontEntry: fontManager.fontByName(ledFontName)
    readonly property string ledFontFamily: (_ledFontEntry && _ledFontEntry.family) ? _ledFontEntry.family : lowResolutionFontList.get(0).family
    readonly property int ledFontPixelSize: (_ledFontEntry && _ledFontEntry.pixelSize) ? _ledFontEntry.pixelSize : 8
    property TextMetrics _ledMetrics: TextMetrics {
        font.family: ledFontFamily
        font.pixelSize: ledFontPixelSize
        text: "M"
    }
    readonly property int ledCellWidth: Math.max(1, Math.round(_ledMetrics.advanceWidth))
    readonly property int ledCellHeight: Math.max(1, Math.ceil(_ledMetrics.height))

    // The looks the build actually carries. A profile naming any other falls
    // back to the default, warned once: a look can go missing between
    // versions, and a bank that fails to paint would be worse than one that
    // paints plainly.
    readonly property var knownShells: ["annunciator", "slide-rule", "switchboard"]
    readonly property var knownDisplays: ["led", "tape"]
    property var _warnedLooks: ({})

    function _validatedLook(kind, name, known, fallback) {
        if (known.indexOf(name) !== -1)
            return name
        var key = kind + ":" + name
        if (!_warnedLooks[key]) {
            _warnedLooks[key] = true
            console.log("Unknown " + kind + " \"" + name + "\", falling back to \"" + fallback + "\"")
        }
        return fallback
    }

    // Relative URLs, resolved by the Loader against qrc:/.
    function shellUrl(part) {
        return "shells/" + _validatedLook("shell", shell, knownShells, "annunciator")
                + "/" + part + ".qml"
    }

    function displayUrl(part) {
        return "displays/" + _validatedLook("display", channelDisplay, knownDisplays, "led")
                + "/" + part + ".qml"
    }

    // Where the bezel comes from. While a chassis stands the bezel is part of
    // its body like everything else the user sees of it, and comes out of the
    // shell's own kit; a screen standing with no chassis around it wears the
    // moulding it came with.
    function frameUrl() {
        return chassisShown ? shellUrl("Frame") : "ScreenFrame.qml"
    }

    signal initializedSettings

    function incrementScaling() {
        fontScaling = Math.min(fontScaling + 0.05, maximumFontScaling)
    }

    function decrementScaling() {
        fontScaling = Math.max(fontScaling - 0.05, minimumFontScaling)
    }

    function close() {
        storeSettings()
        storeCustomProfiles()
        Qt.quit()
    }

    property Storage storage: Storage {}

    function stringify(obj) {
        var replacer = function (key, val) {
            return val.toFixed ? Number(val.toFixed(4)) : val
        }
        return JSON.stringify(obj, replacer, 2)
    }

    function composeSettingsString() {
        var settings = {
            "effectsFrameSkip": effectsFrameSkip,
            "windowScaling": windowScaling,
            "showTerminalSize": showTerminalSize,
            "fontScaling": fontScaling,
            "showMenubar": showMenubar,
            "bloomQuality": bloomQuality,
            "burnInQuality": burnInQuality,
            "useCustomCommand": useCustomCommand,
            "customCommand": customCommand,
            "ledCharacters": ledCharacters,
            "chassisShown": chassisShown,
            "ledFontName": ledFontName
        }
        return stringify(settings)
    }

    // The screen is everything behind the glass: phosphor, geometry, type and
    // the effects that age them, plus the moulding it came out of the factory
    // in, which is what cuts the glass when no cabinet does.
    function composeScreenObject() {
        var screen = {
            "backgroundColor": _backgroundColor,
            "fontColor": _fontColor,
            "flickering": flickering,
            "horizontalSync": horizontalSync,
            "staticNoise": staticNoise,
            "chromaColor": chromaColor,
            "saturationColor": saturationColor,
            "screenCurvature": screenCurvature,
            "glowingLine": glowingLine,
            "burnIn": burnIn,
            "bloom": bloom,
            "rasterization": rasterization,
            "jitter": jitter,
            "rgbShift": rgbShift,
            "brightness": brightness,
            "contrast": contrast,
            "ambientLight": ambientLight,
            "windowOpacity": windowOpacity,
            "fontName": fontName,
            "fontSource": fontSource,
            "fontWidth": fontWidth,
            "lineSpacing": lineSpacing,
            "margin": _margin,
            "blinkingCursor": blinkingCursor,
            "frameSize": _frameSizeScreen,
            "screenRadius": _screenRadiusScreen,
            "frameColor": _frameColorScreen,
            "frameShininess": _frameShininessScreen
        }
        return screen
    }

    // The chassis is the cabinet the screen is mounted in: its casting, its
    // bezel, and the way its bank marks the channel on air.
    function composeChassisObject() {
        var chassis = {
            "shell": shell,
            "channelIndicator": channelIndicator,
            "channelDisplay": channelDisplay,
            "frameSize": _frameSizeChassis,
            "screenRadius": _screenRadiusChassis,
            "frameColor": _frameColorChassis,
            "frameShininess": _frameShininessChassis
        }
        return chassis
    }

    // A whole appliance: the screen in the chassis it stands in. This is the
    // shape a user's saved profile takes and the shape the session restores.
    function composeProfileObject() {
        return {
            "screen": composeScreenObject(),
            "chassis": composeChassisObject()
        }
    }

    function composeScreenString() {
        return stringify(composeScreenObject())
    }

    function composeChassisString() {
        return stringify(composeChassisObject())
    }

    function composeProfileString() {
        return stringify(composeProfileObject())
    }

    // True when the database had a settings, screen and chassis set to
    // restore, so the caller knows whether this start is a first one.
    function loadSettings() {
        var settingsString = storage.getSetting("_CURRENT_SETTINGS")
        var screenString = storage.getSetting("_CURRENT_SCREEN")
        var chassisString = storage.getSetting("_CURRENT_CHASSIS")

        if (!settingsString)
            return false
        if (!screenString)
            return false
        if (!chassisString)
            return false

        loadSettingsString(settingsString)
        loadScreenString(screenString)
        loadChassisString(chassisString)

        if (verbose)
            console.log("Loading settings: " + settingsString + screenString + chassisString)

        return true
    }

    function storeSettings() {
        var settingsString = composeSettingsString()
        var screenString = composeScreenString()
        var chassisString = composeChassisString()

        storage.setSetting("_CURRENT_SETTINGS", settingsString)
        storage.setSetting("_CURRENT_SCREEN", screenString)
        storage.setSetting("_CURRENT_CHASSIS", chassisString)

        if (verbose) {
            console.log("Storing settings: " + settingsString)
            console.log("Storing screen: " + screenString)
            console.log("Storing chassis: " + chassisString)
        }
    }

    function loadSettingsString(settingsString) {
        var settings = JSON.parse(settingsString)

        showTerminalSize = settings.showTerminalSize
                !== undefined ? settings.showTerminalSize : showTerminalSize

        effectsFrameSkip = settings.effectsFrameSkip !== undefined ? settings.effectsFrameSkip : effectsFrameSkip
        windowScaling = settings.windowScaling
                !== undefined ? settings.windowScaling : windowScaling

        fontScaling = settings.fontScaling !== undefined ? settings.fontScaling : fontScaling

        showMenubar = settings.showMenubar !== undefined ? settings.showMenubar : showMenubar

        bloomQuality = settings.bloomQuality !== undefined ? settings.bloomQuality : bloomQuality
        burnInQuality = settings.burnInQuality
                !== undefined ? settings.burnInQuality : burnInQuality

        useCustomCommand = settings.useCustomCommand
                !== undefined ? settings.useCustomCommand : useCustomCommand
        customCommand = settings.customCommand
                !== undefined ? settings.customCommand : customCommand

        ledCharacters = settings.ledCharacters
                !== undefined ? Math.max(minLedCharacters, settings.ledCharacters) : ledCharacters

        chassisShown = settings.chassisShown
                !== undefined ? settings.chassisShown : chassisShown

        ledFontName = settings.ledFontName !== undefined ? settings.ledFontName : ledFontName
    }

    function loadScreenObject(settings) {
        _backgroundColor = settings.backgroundColor
                !== undefined ? settings.backgroundColor : _backgroundColor
        _fontColor = settings.fontColor !== undefined ? settings.fontColor : _fontColor

        horizontalSync = settings.horizontalSync
                !== undefined ? settings.horizontalSync : horizontalSync
        flickering = settings.flickering !== undefined ? settings.flickering : flickering
        staticNoise = settings.staticNoise !== undefined ? settings.staticNoise : staticNoise
        chromaColor = settings.chromaColor !== undefined ? settings.chromaColor : chromaColor
        saturationColor = settings.saturationColor
                !== undefined ? settings.saturationColor : saturationColor
        screenCurvature = settings.screenCurvature
                !== undefined ? settings.screenCurvature : screenCurvature
        glowingLine = settings.glowingLine !== undefined ? settings.glowingLine : glowingLine

        burnIn = settings.burnIn !== undefined ? settings.burnIn : burnIn
        bloom = settings.bloom !== undefined ? settings.bloom : bloom

        rasterization = settings.rasterization
                !== undefined ? settings.rasterization : rasterization

        jitter = settings.jitter !== undefined ? settings.jitter : jitter

        rgbShift = settings.rgbShift !== undefined ? settings.rgbShift : rgbShift

        ambientLight = settings.ambientLight !== undefined ? settings.ambientLight : ambientLight
        contrast = settings.contrast !== undefined ? settings.contrast : contrast
        brightness = settings.brightness !== undefined ? settings.brightness : brightness
        windowOpacity = settings.windowOpacity
                !== undefined ? settings.windowOpacity : windowOpacity

        fontSource = settings.fontSource !== undefined ? settings.fontSource : fontSource
        fontName = settings.fontName !== undefined ? settings.fontName : fontName
        fontWidth = settings.fontWidth !== undefined ? settings.fontWidth : fontWidth
        lineSpacing = settings.lineSpacing !== undefined ? settings.lineSpacing : lineSpacing

        _margin = settings.margin !== undefined ? settings.margin : _margin

        _frameSizeScreen = settings.frameSize !== undefined ? settings.frameSize : _frameSizeScreen
        _screenRadiusScreen = settings.screenRadius !== undefined ? settings.screenRadius : _screenRadiusScreen
        _frameColorScreen = settings.frameColor !== undefined ? settings.frameColor : _frameColorScreen
        _frameShininessScreen = settings.frameShininess !== undefined ? settings.frameShininess : _frameShininessScreen

        blinkingCursor = settings.blinkingCursor !== undefined ? settings.blinkingCursor : blinkingCursor
    }

    function loadScreenString(screenString) {
        loadScreenObject(JSON.parse(screenString))
    }

    function loadChassisObject(settings) {
        _frameSizeChassis = settings.frameSize !== undefined ? settings.frameSize : _frameSizeChassis
        _screenRadiusChassis = settings.screenRadius !== undefined ? settings.screenRadius : _screenRadiusChassis
        _frameColorChassis = settings.frameColor !== undefined ? settings.frameColor : _frameColorChassis
        _frameShininessChassis = settings.frameShininess !== undefined ? settings.frameShininess : _frameShininessChassis

        // The keys that fall back to a constant rather than to what is
        // standing: taking a chassis has to change the machine, so one that
        // is silent about its kit or its mark means the default appliance,
        // never whatever the last chassis left in place.
        shell = settings.shell !== undefined ? settings.shell : "annunciator"
        channelIndicator = settings.channelIndicator !== undefined ? settings.channelIndicator : "glow"
        channelDisplay = settings.channelDisplay !== undefined ? settings.channelDisplay : "led"
    }

    function loadChassisString(chassisString) {
        loadChassisObject(JSON.parse(chassisString))
    }

    function loadProfileObject(profile) {
        loadScreenObject(profile.screen)
        loadChassisObject(profile.chassis)
    }

    function loadProfileString(profileString) {
        loadProfileObject(JSON.parse(profileString))
    }

    function storeCustomProfiles() {
        storage.setSetting("_CUSTOM_PROFILES", composeCustomProfilesString())
    }

    function loadCustomProfiles() {
        var customProfileString = storage.getSetting("_CUSTOM_PROFILES")
        if (customProfileString === undefined)
            customProfileString = "[]"
        loadCustomProfilesString(customProfileString)
    }

    function loadCustomProfilesString(customProfilesString) {
        var customProfiles = JSON.parse(customProfilesString)
        for (var i = 0; i < customProfiles.length; i++) {
            var profile = customProfiles[i]

            if (verbose)
                console.log("Loading custom profile: " + stringify(profile))

            customProfilesList.append(profile)
        }
    }

    function composeCustomProfilesString() {
        var customProfiles = []
        for (var i = 0; i < customProfilesList.count; i++) {
            var profile = customProfilesList.get(i)
            customProfiles.push({
                                    "text": profile.text,
                                    "obj_string": profile.obj_string,
                                    "builtin": false
                                })
        }
        return stringify(customProfiles)
    }

    function loadScreen(index) {
        loadScreenString(screensList.get(index).obj_string)
    }

    function loadChassis(index) {
        loadChassisString(chassisList.get(index).obj_string)
    }

    function loadCustomProfile(index) {
        loadProfileString(customProfilesList.get(index).obj_string)
    }

    function appendCustomProfile(name, profileString) {
        customProfilesList.append({
                                      "text": name,
                                      "obj_string": profileString,
                                      "builtin": false
                                  })
    }

    // PROFILES ///////////////////////////////////////////////////////////////
    // The face the terminal wears when nobody has said otherwise: the screen
    // and the chassis a first start and --default-settings both land on. The
    // QML property values above are the schema's floor, not the app's look.
    readonly property string defaultScreenName: "Default Amber"
    readonly property string defaultChassisName: "Annunciator"

    // What is behind the glass. A screen says nothing about the cabinet it is
    // mounted in, so any of these can be dropped into any chassis.
    property ListModel screensList: ListModel {
        ListElement {
            text: "Default Amber"
            obj_string: '{
                "ambientLight": 0.3,
                "backgroundColor": "#000000",
                "bloom": 0.6,
                "brightness": 0.5,
                "burnIn": 0.3,
                "chromaColor": 0.2,
                "contrast": 0.8,
                "flickering": 0.1,
                "fontColor": "#ff8100",
                "fontName": "TERMINESS_SCALED",
                "fontSource": 0,
                "fontWidth": 1,
                "lineSpacing": 0.1,
                "glowingLine": 0.2,
                "horizontalSync": 0.1,
                "jitter": 0.2,
                "rasterization": 0,
                "rgbShift": 0,
                "saturationColor": 0.2,
                "screenCurvature": 0.2,
                "screenRadius": 0.1,
                "staticNoise": 0.1,
                "windowOpacity": 1,
                "margin": 0.3,
                "blinkingCursor": false,
                "frameSize": 0.1,
                "frameColor": "#cfcfcf",
                "frameShininess": 0.3
            }'
            builtin: true
        }
        ListElement {
            text: "Monochrome Green"
            obj_string: '{
                "ambientLight": 0.3,
                "backgroundColor": "#000000",
                "bloom": 0.5,
                "brightness": 0.5,
                "burnIn": 0.3,
                "chromaColor": 0.0,
                "contrast": 0.8,
                "flickering": 0.1,
                "fontColor": "#0ccc68",
                "fontName": "DEPARTURE_MONO_SCALED",
                "fontSource": 0,
                "fontWidth": 1,
                "lineSpacing": 0.1,
                "glowingLine": 0.2,
                "horizontalSync": 0.1,
                "jitter": 0.2,
                "rasterization": 0,
                "rgbShift": 0,
                "saturationColor": 0.0,
                "screenCurvature": 0.3,
                "screenRadius": 0.2,
                "staticNoise": 0.1,
                "windowOpacity": 1,
                "margin": 0.3,
                "blinkingCursor": false,
                "frameSize": 0.1,
                "frameColor": "#d4d4d4",
                "frameShininess": 0.1
            }'
            builtin: true
        }
        ListElement {
            text: "Deep Blue"
            obj_string: '{
                "ambientLight": 0.0,
                "backgroundColor": "#000000",
                "bloom": 0.6,
                "brightness": 0.5,
                "burnIn": 0.3,
                "chromaColor": 1.0,
                "contrast": 0.8,
                "flickering": 0.1,
                "fontColor": "#7fb4ff",
                "fontName": "BIGBLUE_TERMINAL_SCALED",
                "fontSource": 0,
                "fontWidth": 1,
                "lineSpacing": 0.1,
                "glowingLine": 0.2,
                "horizontalSync": 0.1,
                "jitter": 0.2,
                "rasterization": 0,
                "rgbShift": 0,
                "saturationColor": 0.2,
                "screenCurvature": 0.4,
                "screenRadius": 0.1,
                "staticNoise": 0.1,
                "windowOpacity": 1,
                "margin": 0.3,
                "blinkingCursor": false,
                "frameSize": 0.1,
                "frameColor": "#ffffff",
                "frameShininess": 0.9
            }'
            builtin: true
        }
        ListElement {
            text: "Commodore 64"
            obj_string: '{
                "ambientLight": 0.4,
                "backgroundColor": "#3b3b8f",
                "bloom": 0.4,
                "brightness": 0.6,
                "burnIn": 0.1,
                "chromaColor": 0.0,
                "contrast": 0.7,
                "flickering": 0.1,
                "fontColor": "#a9a7ff",
                "fontName": "COMMODORE_64_SCALED",
                "fontSource": 0,
                "fontWidth": 1,
                "lineSpacing": 0.1,
                "glowingLine": 0.1,
                "horizontalSync": 0.0,
                "jitter": 0.0,
                "rasterization": 1,
                "rgbShift": 0,
                "saturationColor": 0,
                "screenCurvature": 0.5,
                "screenRadius": 0.1,
                "staticNoise": 0.1,
                "windowOpacity": 1,
                "margin": 0.3,
                "blinkingCursor": false,
                "frameSize": 0.5,
                "frameColor": "#999999",
                "frameShininess": 0.0
            }'
            builtin: true
        }
        ListElement {
            text: "Commodore PET"
            obj_string: '{
                "ambientLight": 0.0,
                "backgroundColor": "#000000",
                "bloom": 0.4,
                "brightness": 0.5,
                "burnIn": 0.4,
                "chromaColor": 0,
                "contrast": 0.8,
                "flickering": 0.2,
                "fontColor": "#ffffff",
                "fontName": "COMMODORE_PET_SCALED",
                "fontSource": 0,
                "fontWidth": 1.25,
                "lineSpacing": 0.1,
                "glowingLine": 0.3,
                "horizontalSync": 0.2,
                "jitter": 0.15,
                "rasterization": 1,
                "rgbShift": 0.0,
                "saturationColor": 0,
                "screenCurvature": 0.7,
                "screenRadius": 0.3,
                "staticNoise": 0.2,
                "windowOpacity": 1,
                "margin": 0.2,
                "blinkingCursor": false,
                "frameSize": 0.5,
                "frameColor": "#000000",
                "frameShininess": 0.6
            }'
            builtin: true
        }
        ListElement {
            text: "Apple ]["
            obj_string: '{
                "ambientLight": 1.0,
                "backgroundColor": "#001100",
                "bloom": 0.3,
                "brightness": 0.5,
                "burnIn": 0.3,
                "chromaColor": 0,
                "contrast": 0.8,
                "flickering": 0.2,
                "fontColor": "#4dff6b",
                "fontName": "APPLE_II_SCALED",
                "fontSource": 0,
                "fontWidth": 1.25,
                "lineSpacing": 0.1,
                "glowingLine": 0.3,
                "horizontalSync": 0.2,
                "jitter": 0.2,
                "rasterization": 1,
                "rgbShift": 0.0,
                "saturationColor": 0,
                "screenCurvature": 0.5,
                "screenRadius": 0.3,
                "staticNoise": 0.2,
                "windowOpacity": 1,
                "margin": 0.0,
                "blinkingCursor": false,
                "frameSize": 0.2,
                "frameColor": "#ffffff",
                "frameShininess": 0.8
            }'
            builtin: true
        }
        ListElement {
            text: "Atari 400"
            obj_string: '{
                "ambientLight": 0.1,
                "backgroundColor": "#0f1f5a",
                "bloom": 0.1,
                "brightness": 0.6,
                "burnIn": 0.2,
                "chromaColor": 0,
                "contrast": 0.9,
                "flickering": 0.1,
                "fontColor": "#8ed6ff",
                "fontName": "ATARI_400_SCALED",
                "fontSource": 0,
                "fontWidth": 1,
                "lineSpacing": 0.1,
                "glowingLine": 0.1,
                "horizontalSync": 0.0,
                "jitter": 0.0,
                "rasterization": 1,
                "rgbShift": 0.0,
                "saturationColor": 0,
                "screenCurvature": 0.4,
                "screenRadius": 0.2,
                "staticNoise": 0.1,
                "windowOpacity": 1,
                "margin": 0.2,
                "blinkingCursor": false,
                "frameSize": 0.4,
                "frameColor": "#cccccc",
                "frameShininess": 0.3
            }'
            builtin: true
        }
        ListElement {
            text: "IBM VGA 8x16"
            obj_string: '{
                "ambientLight": 0.2,
                "backgroundColor": "#000000",
                "bloom": 0.2,
                "brightness": 0.6,
                "burnIn": 0.1,
                "chromaColor": 0.5,
                "contrast": 1.0,
                "flickering": 0.1,
                "fontColor": "#c0c0c0",
                "fontName": "IBM_VGA_8x16",
                "fontSource": 0,
                "fontWidth": 1,
                "lineSpacing": 0.1,
                "glowingLine": 0.1,
                "horizontalSync": 0.0,
                "jitter": 0.0,
                "rasterization": 1,
                "rgbShift": 0.1,
                "saturationColor": 0,
                "screenCurvature": 0.3,
                "screenRadius": 0.1,
                "staticNoise": 0.0,
                "windowOpacity": 1,
                "margin": 0.2,
                "blinkingCursor": false,
                "frameSize": 0.1,
                "frameColor": "#ffffff",
                "frameShininess": 0.3
            }'
            builtin: true
        }
        ListElement {
            text: "IBM 3278 Reborn"
            obj_string: '{
                "ambientLight": 0.2,
                "backgroundColor": "#000000",
                "bloom": 0.2,
                "brightness": 0.5,
                "burnIn": 0.5,
                "chromaColor": 0,
                "contrast": 0.8,
                "flickering": 0,
                "fontColor": "#3cff7a",
                "fontName": "IBM_3278",
                "fontSource": 0,
                "fontWidth": 1,
                "lineSpacing": 0.1,
                "glowingLine": 0.0,
                "horizontalSync": 0,
                "jitter": 0,
                "rasterization": 4,
                "rgbShift": 0,
                "saturationColor": 0,
                "screenCurvature": 0,
                "screenRadius": 0.0,
                "staticNoise": 0.0,
                "windowOpacity": 1,
                "margin": 0.1,
                "blinkingCursor": false,
                "frameSize": 0,
                "frameColor": "#ffffff",
                "frameShininess": 0.2
            }'
            builtin: true
        }
        ListElement {
            text: "Neon Cyan"
            obj_string: '{
                "ambientLight": 0.1,
                "backgroundColor": "#001018",
                "bloom": 0.6,
                "brightness": 0.6,
                "burnIn": 0.1,
                "chromaColor": 1,
                "contrast": 0.9,
                "flickering": 0.1,
                "fontColor": "#52f7ff",
                "fontName": "IOSEVKA",
                "fontSource": 0,
                "fontWidth": 1,
                "lineSpacing": 0.1,
                "glowingLine": 0.2,
                "horizontalSync": 0.0,
                "jitter": 0.1,
                "rasterization": 4,
                "rgbShift": 0.0,
                "saturationColor": 0.6,
                "screenCurvature": 0,
                "screenRadius": 0.0,
                "staticNoise": 0.1,
                "windowOpacity": 0.8,
                "margin": 0.1,
                "blinkingCursor": false,
                "frameSize": 0,
                "frameColor": "#c3c3c3",
                "frameShininess": 0.2
            }'
            builtin: true
        }
        ListElement {
            text: "Ghost Terminal"
            obj_string: '{
                "ambientLight": 0.3,
                "backgroundColor": "#0b1014",
                "bloom": 0.3,
                "brightness": 0.6,
                "burnIn": 0.2,
                "chromaColor": 0,
                "contrast": 0.5,
                "flickering": 0.0,
                "fontColor": "#a6b3c0",
                "fontName": "JETBRAINS_MONO",
                "fontSource": 0,
                "fontWidth": 1,
                "lineSpacing": 0.1,
                "glowingLine": 0.1,
                "horizontalSync": 0.0,
                "jitter": 0.0,
                "rasterization": 4,
                "rgbShift": 0.0,
                "saturationColor": 0.0,
                "screenCurvature": 0,
                "screenRadius": 0.0,
                "staticNoise": 0.1,
                "windowOpacity": 0.7,
                "margin": 0.1,
                "blinkingCursor": false,
                "frameSize": 0,
                "frameColor": "#a7a7a7",
                "frameShininess": 0.2
            }'
            builtin: true
        }
        ListElement {
            text: "Plasma"
            obj_string: '{
                "ambientLight": 0.1,
                "backgroundColor": "#070014",
                "bloom": 0.7,
                "brightness": 0.6,
                "burnIn": 0.1,
                "chromaColor": 1,
                "contrast": 0.8,
                "flickering": 0.1,
                "fontColor": "#ff9bd6",
                "fontName": "FIRA_CODE",
                "fontSource": 0,
                "fontWidth": 1,
                "lineSpacing": 0.1,
                "glowingLine": 0.2,
                "horizontalSync": 0.0,
                "jitter": 0.1,
                "rasterization": 4,
                "rgbShift": 0.1,
                "saturationColor": 0.8,
                "screenCurvature": 0,
                "screenRadius": 0.0,
                "staticNoise": 0.1,
                "windowOpacity": 1.0,
                "margin": 0.1,
                "blinkingCursor": false,
                "frameSize": 0,
                "frameColor": "#d0d0d0",
                "frameShininess": 0.2
            }'
            builtin: true
        }
        ListElement {
            text: "Boring"
            obj_string: '{
                "ambientLight": 0.1,
                "backgroundColor": "#000000",
                "bloom": 0.5,
                "brightness": 0.5,
                "burnIn": 0.05,
                "chromaColor": 1,
                "contrast": 0.8,
                "flickering": 0.0,
                "fontColor": "#ffffff",
                "fontName": "JETBRAINS_MONO",
                "fontSource": 0,
                "fontWidth": 1,
                "lineSpacing": 0.1,
                "glowingLine": 0.1,
                "horizontalSync": 0,
                "jitter": 0.0,
                "rasterization": 4,
                "rgbShift": 0,
                "saturationColor": 0.0,
                "screenCurvature": 0,
                "screenRadius": 0.0,
                "staticNoise": 0.0,
                "windowOpacity": 1.0,
                "margin": 0.0,
                "blinkingCursor": false,
                "frameSize": 0,
                "frameColor": "#c0c0c0",
                "frameShininess": 0.2
            }'
            builtin: true
        }
        ListElement {
            text: "E-Ink"
            obj_string: '{
                "ambientLight": 0.6,
                "backgroundColor": "#f2f2ec",
                "bloom": 0.0,
                "brightness": 1.0,
                "burnIn": 0.6,
                "chromaColor": 0,
                "contrast": 0.5,
                "flickering": 0.0,
                "fontColor": "#101010",
                "fontName": "HACK",
                "fontSource": 0,
                "fontWidth": 1,
                "lineSpacing": 0.1,
                "glowingLine": 0.0,
                "horizontalSync": 0.0,
                "jitter": 0.0,
                "rasterization": 4,
                "rgbShift": 0,
                "saturationColor": 0,
                "screenCurvature": 0,
                "screenRadius": 0.0,
                "staticNoise": 0.0,
                "windowOpacity": 1,
                "margin": 0.1,
                "blinkingCursor": false,
                "frameSize": 0,
                "frameColor": "#cdcdcd",
                "frameShininess": 0.2
            }'
            builtin: true
        }
    }

    // The cabinets a screen can be mounted in: the casting, the bezel it cuts
    // the glass with, and the way its bank marks the channel on air.
    property ListModel chassisList: ListModel {
        // Fitted to the amber appliance of the reference mock
        // (RoboCo-Amber.png, 1448x1086): a gunmetal plate of LED windows
        // beside a slim-bezelled tube, the current channel marked by its
        // window's own light. The frameColor leans cold because the base
        // colour law folds a fifth of the screen's phosphor into the plate;
        // this value lands the mix on the mock's gunmetal. frameSize and
        // screenRadius are fitted to the mock's glass opening (inset ~22px,
        // corner radius ~55).
        ListElement {
            text: "Annunciator"
            obj_string: '{
                "shell": "annunciator",
                "channelIndicator": "glow",
                "channelDisplay": "led",
                "frameSize": 0.45,
                "screenRadius": 0.44,
                "frameColor": "#001735",
                "frameShininess": 0.3
            }'
            builtin: true
        }
        // Fitted to the blue appliance of the reference mock
        // (Metalic-Blue.png, 1448x1086): warm olive-grey gunmetal (all the
        // blue in that mock is emitted light, none of it paint), windows
        // punched into the chassis, a carrier rail down the bank's left edge.
        // The mock draws that rail as a bare groove and lights its windows
        // alike; the appliance puts the groove to work, driving the carriage
        // down it to the channel on screen, so this panel marks by machinery
        // where the annunciator marks by light. frameSize sits between the
        // mock's shallow top inset and its deep bottom trough; the
        // screenRadius ceiling approximates the barrel-curved glass mouth.
        ListElement {
            text: "Slide Rule"
            obj_string: '{
                "shell": "slide-rule",
                "channelIndicator": "pointer",
                "channelDisplay": "led",
                "frameSize": 0.7,
                "screenRadius": 1.0,
                "frameColor": "#a77d37",
                "frameShininess": 0.15
            }'
            builtin: true
        }
        // Fitted to the switchboard of the reference mock (Deep-Blue.png,
        // 1448x1086): fifteen heavy toggle switches in recessed wells down a
        // near-neutral gunmetal panel, a mechanical page counter at its foot,
        // big round-cornered glass. The owner supersedes the artwork twice:
        // the label plates carry embossed punch tape rather than the mock's
        // glowing LED text, and the thrown switch is the sole mark of the
        // channel on screen (the mock's brighter row-06 plate is void), so
        // this chassis asks for the switch law and every window sits level.
        // The frameColor leans dark maroon because the base colour law folds
        // a fifth of the screen's phosphor into the plate; this value lands
        // the mix on the mock's #232830 gunmetal under cyan. frameSize and
        // screenRadius are fitted to the mock's glass opening (inset ~10px,
        // corner radius ~90).
        ListElement {
            text: "Switchboard"
            obj_string: '{
                "shell": "switchboard",
                "channelIndicator": "switch",
                "channelDisplay": "tape",
                "frameSize": 0.2,
                "screenRadius": 0.7,
                "frameColor": "#461725",
                "frameShininess": 0.2
            }'
            builtin: true
        }
    }

    // The appliances the user has built and named: a screen and a chassis
    // saved together, filled from the store at startup.
    property ListModel customProfilesList: ListModel {}

    function getProfileIndexByName(list, name) {
        for (var i = 0; i < list.count; i++) {
            if (list.get(i).text === name)
                return i
        }
        return -1
    }

    Component.onCompleted: {
        // Manage the arguments from the QML side.
        var args = Qt.application.arguments
        if (args.indexOf("--verbose") !== -1) {
            verbose = true
        }
        var restored = false
        if (args.indexOf("--default-settings") === -1) {
            restored = loadSettings()
        }

        loadCustomProfiles()

        // Nothing to restore, so the terminal comes up as itself: the amber
        // screen in the annunciator cabinet, bank and all. A profile named on
        // the command line is a later word than this one and still wins.
        if (!restored) {
            var screenIndex = getProfileIndexByName(screensList, defaultScreenName)
            if (screenIndex !== -1)
                loadScreen(screenIndex)

            var chassisIndex = getProfileIndexByName(chassisList, defaultChassisName)
            if (chassisIndex !== -1)
                loadChassis(chassisIndex)
        }

        // A name on the command line is read as a whole appliance first, since
        // that is what the user saved under a name of their own; failing that
        // it is a screen, which the cabinet standing takes without change.
        var profileArgPosition = args.indexOf("--profile")
        if (profileArgPosition !== -1) {
            var profileName = args[profileArgPosition + 1]
            var customIndex = getProfileIndexByName(customProfilesList, profileName)
            var namedScreenIndex = getProfileIndexByName(screensList, profileName)
            if (customIndex !== -1) {
                loadCustomProfile(customIndex)
            } else if (namedScreenIndex !== -1) {
                loadScreen(namedScreenIndex)
            } else {
                console.log("Warning: selected profile is not valid; ignoring it")
            }
        }

        initializedSettings()
    }

    // VARS ///////////////////////////////////////////////////////////////////
    property Label _sampleLabel: Label {
        text: "100%"
    }
    property real labelWidth: _sampleLabel.width
}
