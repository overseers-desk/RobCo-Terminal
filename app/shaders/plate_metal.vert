#version 440

layout(location = 0) in vec4 qt_Vertex;
layout(location = 1) in vec2 qt_MultiTexCoord0;

layout(std140, binding = 0) uniform ubuf {
    mat4 qt_Matrix;
    float qt_Opacity;
    vec2 sizePx;
    vec2 lightDir;
    vec4 baseColor;
    vec4 highlightColor;
    vec4 shadowColor;
    float cornerRadius;
    float bevelPx;
    float grainAmount;
    float mottleAmount;
    float scratchAmount;
    float vignetteStrength;
    float wearAmount;
    float seamGain;
    float seed;
};

layout(location = 0) out vec2 qt_TexCoord0;

void main() {
    qt_TexCoord0 = qt_MultiTexCoord0;
    gl_Position = qt_Matrix * qt_Vertex;
}
