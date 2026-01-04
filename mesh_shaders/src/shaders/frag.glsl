#version 460

// Holographic Glass Fragment Shader

layout(location = 0) in vec3 fragWorldPos;
layout(location = 1) in vec3 fragNormal;
layout(location = 2) in vec2 fragUV;
layout(location = 3) in vec3 fragTangent;

layout(location = 0) out vec4 outColor;

layout(set = 0, binding = 0) uniform sampler2D albedoMap;
layout(set = 0, binding = 1) uniform sampler2D normalMap;
layout(set = 0, binding = 2) uniform sampler2D rmaMap;

layout(push_constant) uniform PushConstants {
    mat4 mvp;
    float time;
} pc;

const float PI = 3.14159265359;

void main() {
    vec3 N = normalize(fragNormal);
    
    vec3 camPos = vec3(0.0, 2.0, 5.0);
    vec3 V = normalize(camPos - fragWorldPos);
    
    float NdotV = max(dot(N, V), 0.0);
    float fresnel = pow(1.0 - NdotV, 2.5);
    
    // === RICH HOLOGRAPHIC COLORS ===
    vec3 cyan = vec3(0.1, 0.85, 0.95);
    vec3 purple = vec3(0.65, 0.25, 0.85);
    vec3 pink = vec3(0.95, 0.35, 0.65);
    vec3 green = vec3(0.2, 0.9, 0.5);
    
    // Gradient based on height + view angle for more variation
    float t = fragUV.y + fresnel * 0.2;
    vec3 baseColor;
    if (t < 0.33) {
        baseColor = mix(cyan, green, t * 3.0);
    } else if (t < 0.66) {
        baseColor = mix(green, purple, (t - 0.33) * 3.0);
    } else {
        baseColor = mix(purple, pink, (t - 0.66) * 3.0);
    }
    
    // === IRIDESCENT RAINBOW ===
    float iri = NdotV * 3.0 + fragWorldPos.y * 2.0 + pc.time * 0.2;
    vec3 rainbow = vec3(
        sin(iri) * 0.4 + 0.6,
        sin(iri + 2.1) * 0.4 + 0.6,
        sin(iri + 4.2) * 0.4 + 0.6
    );
    baseColor *= rainbow;
    
    // === EDGE OUTLINE GLOW ===
    // Detect edges using UV coordinates (near 0 or 1)
    float edgeX = min(fragUV.x, 1.0 - fragUV.x);
    float edgeY = min(fragUV.y, 1.0 - fragUV.y);
    float edgeDist = min(edgeX, edgeY);
    float edgeLine = 1.0 - smoothstep(0.0, 0.08, edgeDist);
    vec3 edgeGlow = cyan * edgeLine * 1.5;
    
    // === FRESNEL EDGE GLOW ===
    vec3 fresnelGlow = cyan * fresnel * 0.6;
    
    // === INTERNAL SHIMMER - stronger rainbow reflections ===
    float shimmer = sin(fragWorldPos.y * 12.0 + pc.time * 2.5) * 
                    sin(fragWorldPos.x * 10.0 - pc.time * 1.5);
    shimmer = shimmer * 0.5 + 0.5;
    float shimmer2 = sin(fragWorldPos.z * 8.0 + pc.time * 1.8);
    shimmer2 = shimmer2 * 0.5 + 0.5;
    
    // Rainbow internal reflections
    vec3 internalRainbow = vec3(
        sin(fragWorldPos.y * 5.0 + pc.time) * 0.5 + 0.5,
        sin(fragWorldPos.y * 5.0 + pc.time + 2.0) * 0.5 + 0.5,
        sin(fragWorldPos.y * 5.0 + pc.time + 4.0) * 0.5 + 0.5
    );
    vec3 shimmerColor = internalRainbow * shimmer * shimmer2 * 0.35;
    
    // === SPECULAR ===
    vec3 L = normalize(vec3(2.0, 4.0, 3.0) - fragWorldPos);
    vec3 H = normalize(V + L);
    float spec = pow(max(dot(N, H), 0.0), 96.0) * 0.4;
    
    // === COMBINE ===
    vec3 color = baseColor * 0.35;  // Darker base for more transparency feel
    color += edgeGlow;               // Bright edge lines
    color += fresnelGlow;            // Edge fresnel
    color += shimmerColor;           // Internal light
    color += vec3(1.0) * spec;       // Specular highlight
    
    color = clamp(color, 0.0, 1.0);
    
    // === TRANSPARENCY ===
    // Very see-through, edges more visible
    float alpha = 0.2 + fresnel * 0.4 + edgeLine * 0.3;
    alpha = clamp(alpha, 0.15, 0.85);
    
    outColor = vec4(color, alpha);
}
