#version 460

layout(location = 0) in vec2 inUV;
layout(location = 1) in vec3 inViewDir;

layout(location = 0) out vec4 outColor;

// Use samplerCube for proper cubemap sampling
layout(set = 0, binding = 0) uniform samplerCube cubemapTex;

layout(push_constant) uniform PushConstants {
    mat4 mvp;
    mat4 invViewProj;
    float time;
} pc;

void main() {
    // Normalize view direction and sample cubemap directly
    vec3 viewDir = normalize(inViewDir);
    
    // Sample cubemap using 3D direction vector
    vec3 envColor = texture(cubemapTex, viewDir).rgb;
    
    // Optional: Add subtle animation/glow effects
    float pulse = 0.95 + 0.05 * sin(pc.time * 0.5);
    envColor *= pulse;
    
    // Slight vignette for depth
    float vignette = 1.0 - 0.2 * length(inUV - 0.5);
    envColor *= vignette;
    
    outColor = vec4(envColor, 1.0);
}
