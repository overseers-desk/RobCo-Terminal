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

Item {
    id: shaderRoot

    // The profile driving this terminal's appearance. Defaults to the global
    // profile; a tab may override it with its own ProfileSettings instance.
    property QtObject settings: appSettings

    function dynamicFragmentPath() {
        var rasterMode = settings.rasterization;
        var burnInOn = settings.burnIn > 0 ? 1 : 0;
        var frameOn = settings.frameEnabled ? 1 : 0;
        var chromaOn = settings.chromaColor > 0 ? 1 : 0;
        return "qrc:/shaders/terminal_dynamic_raster" + rasterMode +
               "_burn" + burnInOn +
               "_frame" + frameOn +
               "_chroma" + chromaOn +
               ".frag.qsb";
    }

    function staticFragmentPath() {
        var rgbShiftOn = settings.rgbShift > 0 ? 1 : 0;
        var bloomOn = settings.bloom > 0 ? 1 : 0;
        var curvatureOn = (settings.screenCurvature > 0 || settings.frameSize > 0) ? 1 : 0;
        var shineOn = settings.frameShininess > 0 ? 1 : 0;
        return "qrc:/shaders/terminal_static_rgb" + rgbShiftOn +
               "_bloom" + bloomOn +
               "_curve" + curvatureOn +
               "_shine" + shineOn +
               ".frag.qsb";
    }

    property ShaderEffectSource source
    property BurnInEffect burnInEffect
    property ShaderEffectSource bloomSource

    property color fontColor: settings.fontColor
    property color backgroundColor: settings.backgroundColor

    property real screenCurvature: settings.screenCurvature * appSettings.screenCurvatureSize * terminalWindow.normalizedWindowScale
    property real frameSize: settings.frameSize * terminalWindow.normalizedWindowScale

    property real chromaColor: settings.chromaColor

    property real ambientLight: settings.ambientLight * 0.2

    property size virtualResolution
    property size screenResolution

    property real _screenDensity: Math.min(
        screenResolution.width / virtualResolution.width,
        screenResolution.height / virtualResolution.height
    )

    ShaderEffect {
        id: dynamicShader

        property ShaderEffectSource screenBuffer: frameBuffer
        property ShaderEffectSource burnInSource: burnInEffect.effectSource
        property ShaderEffectSource frameSource: terminalFrameLoader.item

        property color fontColor: parent.fontColor
        property color backgroundColor: parent.backgroundColor
        property real screenCurvature: parent.screenCurvature
        property real chromaColor: parent.chromaColor
        property real ambientLight: parent.ambientLight

        property real flickering: settings.flickering
        property real horizontalSync: settings.horizontalSync
        property real horizontalSyncStrength: Utils.lint(0.05, 0.35, horizontalSync)
        property real glowingLine: settings.glowingLine * 0.2

        // Fast burnin properties
        property real burnIn: settings.burnIn
        property real burnInLastUpdate: burnInEffect.lastUpdate
        property real burnInTime: burnInEffect.burnInFadeTime

        property real jitter: settings.jitter
        property size jitterDisplacement: Qt.size(0.007 * jitter, 0.002 * jitter)
        property real staticNoise: settings.staticNoise
        property size scaleNoiseSize: Qt.size((width * 0.75) / (noiseTexture.width * appSettings.windowScaling * settings.totalFontScaling),
                                              (height * 0.75) / (noiseTexture.height * appSettings.windowScaling * settings.totalFontScaling))

        property size virtualResolution: parent.virtualResolution

        // Rasterization might display oversamping issues if virtual resolution is close to physical display resolution.
        // We progressively disable rasterization from 4x up to 2x resolution.
        property real rasterizationIntensity: Utils.smoothstep(2.0, 4.0, _screenDensity)

        property real time: timeManager ? timeManager.time : 0
        property ShaderEffectSource noiseSource: noiseShaderSource

        property real frameSize: parent.frameSize
        property real frameShininess: settings.frameShininess
        property real bloom: parent.bloomSource ? settings.bloom * 2.5 : 0

        anchors.fill: parent
        blending: false

        Image {
            id: noiseTexture
            source: "images/allNoise512.png"
            width: 512
            height: 512
            fillMode: Image.Tile
            visible: false
        }
        ShaderEffectSource {
            id: noiseShaderSource
            sourceItem: noiseTexture
            wrapMode: ShaderEffectSource.Repeat
            visible: false
            smooth: true
        }

        vertexShader: "qrc:/shaders/terminal_dynamic.vert.qsb"
        fragmentShader: dynamicFragmentPath()

        onStatusChanged: if (log) console.log(log)
    }

    Loader {
        id: terminalFrameLoader

        active: settings.frameEnabled

        width: staticShader.width
        height: staticShader.height

        sourceComponent: ShaderEffectSource {

            sourceItem: terminalFrame
            hideSource: true
            visible: false
            format: ShaderEffectSource.RGBA

            TerminalFrame {
                id: terminalFrame
                settings: shaderRoot.settings
                blending: false
                anchors.fill: parent
            }
        }
    }

    ShaderEffect {
        id: staticShader

        width: parent.width * appSettings.windowScaling
        height: parent.height * appSettings.windowScaling

        property ShaderEffectSource source: parent.source
        property ShaderEffectSource bloomSource: parent.bloomSource

        property color fontColor: parent.fontColor
        property color backgroundColor: parent.backgroundColor
        property real bloom: bloomSource ? settings.bloom * 2.5 : 0

        property real screenCurvature: parent.screenCurvature

        property real chromaColor: settings.chromaColor;

        property real rgbShift: settings.rgbShift * (4.0 / width) * settings.totalFontScaling

        property real screen_brightness: Utils.lint(0.5, 1.5, settings.brightness)
        property real frameShininess: settings.frameShininess
        property real frameSize: parent.frameSize

        blending: false
        visible: false

        vertexShader: "qrc:/shaders/terminal_static.vert.qsb"
        fragmentShader: staticFragmentPath()

        onStatusChanged: if (log) console.log(log)
    }

    ShaderEffectSource {
        id: frameBuffer
        visible: false
        sourceItem: staticShader
        hideSource: true
    }
}
