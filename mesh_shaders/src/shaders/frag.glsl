#version 460
#extension GL_EXT_ray_query : require

// Selection Sort Visualization Fragment Shader

layout(location = 0) in vec3 fragWorldPos;
layout(location = 1) in vec3 fragNormal;
layout(location = 2) in vec2 fragUV;
layout(location = 3) in vec3 fragTangent;
layout(location = 4) flat in int barState; // 0=normal, 1=current_i, 2=current_j, 3=min_idx, 4=sorted

layout(location = 0) out vec4 outColor;

layout(set = 0, binding = 0) uniform sampler2D albedoMap;
layout(set = 0, binding = 1) uniform sampler2D normalMap;
layout(set = 0, binding = 2) uniform sampler2D rmaMap;
layout(set = 0, binding = 3) uniform accelerationStructureEXT tlas;

layout(push_constant) uniform PushConstants {
    mat4 mvp;
    mat4 inv_view_proj;
    vec4 camera_pos;
    vec4 light_pos;
    float time;
    int current_i;
    int current_j;
    int min_idx;
    float bar_heights[8];
} pc;

const float PI = 3.14159265359;

// Trace shadow ray using hardware ray tracing
float traceShadow(vec3 origin, vec3 direction, float maxDist) {
    rayQueryEXT rayQuery;
    
    rayQueryInitializeEXT(
        rayQuery,
        tlas,
        gl_RayFlagsTerminateOnFirstHitEXT | gl_RayFlagsOpaqueEXT,
        0xFF,
        origin,
        0.01,
        direction,
        maxDist
    );
    
    while (rayQueryProceedEXT(rayQuery)) {}
    
    if (rayQueryGetIntersectionTypeEXT(rayQuery, true) != gl_RayQueryCommittedIntersectionNoneEXT) {
        return 0.0;
    }
    
    return 1.0;
}

void main() {
    vec3 N = normalize(fragNormal);
    vec3 camPos = pc.camera_pos.xyz;
    vec3 V = normalize(camPos - fragWorldPos);
    
    float NdotV = max(dot(N, V), 0.0);
    float fresnel = pow(1.0 - NdotV, 2.5);
    
    // Base colors based on bar state
    vec3 baseColor;
    float glowIntensity = 0.0;
    
    if (barState == 4) {
        // Sorted - green
        baseColor = vec3(0.2, 0.9, 0.3);
        glowIntensity = 0.3;
    } else if (barState == 1) {
        // Current position (i) - yellow/gold
        baseColor = vec3(1.0, 0.85, 0.2);
        glowIntensity = 0.6 + 0.3 * sin(pc.time * 4.0);
    } else if (barState == 3) {
        // Current minimum - red/orange
        baseColor = vec3(1.0, 0.3, 0.1);
        glowIntensity = 0.7 + 0.3 * sin(pc.time * 5.0);
    } else if (barState == 2) {
        // Currently comparing (j) - cyan
        baseColor = vec3(0.1, 0.85, 0.95);
        glowIntensity = 0.4 + 0.2 * sin(pc.time * 3.0);
    } else {
        // Normal unsorted - purple/blue
        baseColor = vec3(0.5, 0.3, 0.8);
        glowIntensity = 0.1;
    }
    
    // Add subtle height-based gradient
    float heightGrad = fragUV.y * 0.3;
    baseColor = mix(baseColor * 0.7, baseColor * 1.2, heightGrad);
    
    // Edge glow effect
    float edgeX = min(fragUV.x, 1.0 - fragUV.x);
    float edgeY = min(fragUV.y, 1.0 - fragUV.y);
    float edgeDist = min(edgeX, edgeY);
    float edgeLine = 1.0 - smoothstep(0.0, 0.06, edgeDist);
    vec3 edgeGlow = baseColor * edgeLine * (1.0 + glowIntensity);
    
    // Fresnel edge glow
    vec3 fresnelGlow = baseColor * fresnel * 0.4;
    
    // Lighting
    vec3 lightPos = pc.light_pos.xyz;
    vec3 L = lightPos - fragWorldPos;
    float lightDist = length(L);
    L = normalize(L);
    
    float shadow = traceShadow(fragWorldPos + N * 0.02, L, lightDist);
    
    // Specular
    vec3 H = normalize(V + L);
    float spec = pow(max(dot(N, H), 0.0), 64.0) * 0.5 * shadow;
    
    // Diffuse
    float NdotL = max(dot(N, L), 0.0);
    float diffuse = NdotL * shadow;
    
    // Combine
    vec3 color = baseColor * (0.3 + 0.5 * diffuse);
    color += edgeGlow;
    color += fresnelGlow;
    color += vec3(1.0) * spec;
    
    // Add glow for active bars
    color += baseColor * glowIntensity * 0.3;
    
    // Shadow influence
    color *= (0.7 + 0.3 * shadow);
    
    color = clamp(color, 0.0, 1.0);
    
    // Semi-transparent glass effect
    float alpha = 0.6 + fresnel * 0.3 + edgeLine * 0.1;
    alpha = clamp(alpha, 0.5, 0.95);
    
    outColor = vec4(color, alpha);
}
