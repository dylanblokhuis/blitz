#version 450
#extension GL_ARB_separate_shader_objects : enable
#extension GL_ARB_shading_language_420pack : enable

// layout (set = 0, binding = 0) uniform sampler2D textureLinearYUV420P;
// layout (set = 0, binding = 1) uniform Uniform {
//     int index;
// } ubo;

layout (location = 0) in vec4 o_color;
layout (location = 0) out vec4 a_frag_color;

void main() {
    a_frag_color = o_color;
}