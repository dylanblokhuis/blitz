#version 450
#extension GL_ARB_separate_shader_objects : enable
#extension GL_ARB_shading_language_420pack : enable

layout (location = 0) in vec2 point;
layout (location = 1) in vec4 color;

layout (location = 0) out vec4 o_color;

void main() {
    o_color = color;
    gl_Position = vec4(point, 0.0, 1.0);
}