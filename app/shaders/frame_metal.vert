#version 440

layout(location = 0) in vec4 qt_Vertex;
layout(location = 1) in vec2 qt_MultiTexCoord0;

layout(std140, binding = 0) uniform ubuf {
    mat4 qt_Matrix;
    float qt_Opacity;
    float screenCurvature;
    float frameSize;
    float screenRadius;
    float ambientLight;
    float frameShininess;
    vec2 viewportSize;
    vec2 lightDir;
    vec4 bezelColor;
    vec4 chassisColor;
    vec4 ridgeColor;
    vec4 bezelMargins;
    float outerRadius;
    float wellDepth;
    float wellFloor;
    float ridgeGain;
    float grainAmount;
    float mottleAmount;
    float scratchAmount;
    float vignetteStrength;
    float fillGain;
    float troughGain;
};

layout(location = 0) out vec2 qt_TexCoord0;

void main() {
    qt_TexCoord0 = qt_MultiTexCoord0;
    gl_Position = qt_Matrix * qt_Vertex;
}
