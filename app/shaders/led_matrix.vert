#version 440

layout(location = 0) in vec4 qt_Vertex;
layout(location = 1) in vec2 qt_MultiTexCoord0;

layout(std140, binding = 0) uniform ubuf {
    mat4 qt_Matrix;
    float qt_Opacity;
    vec2 gridSize;
    vec4 litColor;
    vec4 dimColor;
    vec4 panelColor;
    float dotRadius;
    float threshold;
    float glow;
    float pixelsPerCell;
    vec2 spillMargin;
    float spillStrength;
};

layout(location = 0) out vec2 qt_TexCoord0;

void main() {
    qt_TexCoord0 = qt_MultiTexCoord0;
    gl_Position = qt_Matrix * qt_Vertex;
}
