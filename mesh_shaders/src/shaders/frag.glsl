#version 460
#extension GL_EXT_ray_query : require

// Holographic Glass Fragment Shader with Ray-Traced Shadows

layout(location = 0) in vec3 fragWorldPos;
layout(location = 1) in vec3 fragNormal;
layout(location = 2) in vec2 fragUV;
layout(location = 3) in vec3 fragTangent;

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
        0.01,  // tMin - small offset to avoid self-intersection
        direction,
        maxDist
    );
    
    // Traverse the acceleration structure
    while (rayQueryProceedEXT(rayQuery)) {
        // For opaque geometry, we don't need to do anything here
    }
    
    // Check if we hit anything
    if (rayQueryGetIntersectionTypeEXT(rayQuery, true) != gl_RayQueryCommittedIntersectionNoneEXT) {
        return 0.0;  // In shadow
    }
    
    return 1.0;  // Not in shadow
}

void main() {
    vec3 N = normalize(fragNormal);
    
    vec3 camPos = pc.camera_pos.xyz;
    vec3 V = normalize(camPos - fragWorldPos);
    
    float NdotV = max(dot(N, V), 0.0);
    float fresnel = pow(1.0 - NdotV, 2.5);
    
    // === RICH HOLOGRAPHIC COLORS ===
    vec3 cyan = vec3(0.1, 0.85, 0.95);
    vec3 purple = vec3(0.65, 0.25, 0.85);
    vec3 pink = vec3(0.95, 0.35, 0.65);
    vec3 green = vec3(0.2, 0.9, 0.5);
    
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
    float edgeX = min(fragUV.x, 1.0 - fragUV.x);
    float edgeY = min(fragUV.y, 1.0 - fragUV.y);
    float edgeDist = min(edgeX, edgeY);
    float edgeLine = 1.0 - smoothstep(0.0, 0.08, edgeDist);
    vec3 edgeGlow = cyan * edgeLine * 1.5;
    
    // === FRESNEL EDGE GLOW ===
    vec3 fresnelGlow = cyan * fresnel * 0.6;
    
    // === INTERNAL SHIMMER ===
    float shimmer = sin(fragWorldPos.y * 12.0 + pc.time * 2.5) * 
                    sin(fragWorldPos.x * 10.0 - pc.time * 1.5);
    shimmer = shimmer * 0.5 + 0.5;
    float shimmer2 = sin(fragWorldPos.z * 8.0 + pc.time * 1.8);
    shimmer2 = shimmer2 * 0.5 + 0.5;
    
    vec3 internalRainbow = vec3(
        sin(fragWorldPos.y * 5.0 + pc.time) * 0.5 + 0.5,
        sin(fragWorldPos.y * 5.0 + pc.time + 2.0) * 0.5 + 0.5,
        sin(fragWorldPos.y * 5.0 + pc.time + 4.0) * 0.5 + 0.5
    );
    vec3 shimmerColor = internalRainbow * shimmer * shimmer2 * 0.35;
    
    // === RAY-TRACED SHADOW ===
    vec3 lightPos = pc.light_pos.xyz;
    vec3 L = lightPos - fragWorldPos;
    float lightDist = length(L);
    L = normalize(L);
    
    // Trace shadow ray from surface toward light
    float shadow = traceShadow(fragWorldPos + N * 0.02, L, lightDist);
    
    // === SPECULAR with shadow ===
    vec3 H = normalize(V + L);
    float spec = pow(max(dot(N, H), 0.0), 96.0) * 0.4 * shadow;
    
    // === DIFFUSE LIGHTING with shadow ===
    float NdotL = max(dot(N, L), 0.0);
    float diffuse = NdotL * shadow;
    
    // === COMBINE ===
    vec3 color = baseColor * (0.25 + 0.1 * diffuse);  // Base with some shadow influence
    color += edgeGlow * (0.5 + 0.5 * shadow);         // Edge glow dimmed in shadow
    color += fresnelGlow;                              // Fresnel always visible
    color += shimmerColor * (0.5 + 0.5 * shadow);     // Internal light affected by shadow
    color += vec3(1.0) * spec;                         // Specular highlight
    
    // Add shadow visualization - darker in shadowed areas
    color *= (0.6 + 0.4 * shadow);
    
    color = clamp(color, 0.0, 1.0);
    
    // === TRANSPARENCY ===
    float alpha = 0.2 + fresnel * 0.4 + edgeLine * 0.3;
    alpha = clamp(alpha, 0.15, 0.85);
    
    outColor = vec4(color, alpha);
}
