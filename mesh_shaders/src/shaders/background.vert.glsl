#version 460

// Fullscreen triangle - no vertex buffer needed

layout(location = 0) out vec2 outUV;
layout(location = 1) out vec3 outViewDir;

layout(push_constant) uniform PushConstants {
    mat4 mvp;
    mat4 invViewProj;
    vec4 camera_pos;
    vec4 light_pos;
    float time;
} pc;

void main() {
    vec2 positions[3] = vec2[](
        vec2(-1.0, -1.0),
        vec2( 3.0, -1.0),
        vec2(-1.0,  3.0)
    );
    
    vec2 pos = positions[gl_VertexIndex];
    gl_Position = vec4(pos, 0.9999, 1.0);
    
    outUV = pos * 0.5 + 0.5;
    
    vec4 nearPoint = pc.invViewProj * vec4(pos, -1.0, 1.0);
    vec4 farPoint = pc.invViewProj * vec4(pos, 1.0, 1.0);
    nearPoint /= nearPoint.w;
    farPoint /= farPoint.w;
    outViewDir = normalize(farPoint.xyz - nearPoint.xyz);
}
