#version 460

// Fullscreen triangle - no vertex buffer needed
// Generates a triangle that covers the entire screen

layout(location = 0) out vec2 outUV;
layout(location = 1) out vec3 outViewDir;

layout(push_constant) uniform PushConstants {
    mat4 mvp;           // Not used for background
    mat4 invViewProj;   // Inverse view-projection for ray direction
    float time;
} pc;

void main() {
    // Fullscreen triangle trick: 3 vertices cover entire screen
    vec2 positions[3] = vec2[](
        vec2(-1.0, -1.0),
        vec2( 3.0, -1.0),
        vec2(-1.0,  3.0)
    );
    
    vec2 pos = positions[gl_VertexIndex];
    gl_Position = vec4(pos, 0.9999, 1.0); // Far depth
    
    // UV for screen-space effects (0 to 1)
    outUV = pos * 0.5 + 0.5;
    
    // Calculate view direction - unproject from clip space to world
    // Use near and far points to get proper ray direction
    vec4 nearPoint = pc.invViewProj * vec4(pos, -1.0, 1.0);
    vec4 farPoint = pc.invViewProj * vec4(pos, 1.0, 1.0);
    nearPoint /= nearPoint.w;
    farPoint /= farPoint.w;
    outViewDir = normalize(farPoint.xyz - nearPoint.xyz);
}
