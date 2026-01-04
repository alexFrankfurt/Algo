#version 460

layout(location = 0) in vec2 inUV;
layout(location = 1) in vec3 inViewDir;

layout(location = 0) out vec4 outColor;

layout(set = 0, binding = 0) uniform samplerCube cubemapTex;

layout(push_constant) uniform PushConstants {
    mat4 mvp;
    mat4 invViewProj;
    vec4 camera_pos;
    vec4 light_pos;
    float time;
} pc;

void main() {
    vec3 viewDir = normalize(inViewDir);
    
    vec3 envColor = texture(cubemapTex, viewDir).rgb;
    
    float pulse = 0.95 + 0.05 * sin(pc.time * 0.5);
    envColor *= pulse;
    
    float vignette = 1.0 - 0.2 * length(inUV - 0.5);
    envColor *= vignette;
    
    outColor = vec4(envColor, 1.0);
}
