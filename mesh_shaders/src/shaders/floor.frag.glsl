#version 460
#extension GL_EXT_ray_query : require

layout(location = 0) in vec3 fragWorldPos;
layout(location = 1) in vec2 fragUV;

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
    vec3 N = vec3(0.0, 1.0, 0.0);
    
    // Sample textures
    vec3 albedo = texture(albedoMap, fragUV).rgb;
    vec3 normalTex = texture(normalMap, fragUV).rgb * 2.0 - 1.0;
    vec3 rma = texture(rmaMap, fragUV).rgb;
    
    // Simple tangent space (floor is flat)
    vec3 T = vec3(1.0, 0.0, 0.0);
    vec3 B = vec3(0.0, 0.0, 1.0);
    N = normalize(T * normalTex.x + B * normalTex.y + N * normalTex.z);
    
    vec3 camPos = pc.camera_pos.xyz;
    vec3 V = normalize(camPos - fragWorldPos);
    
    // Lighting
    vec3 lightPos = pc.light_pos.xyz;
    vec3 L = lightPos - fragWorldPos;
    float lightDist = length(L);
    L = normalize(L);
    
    // Ray-traced shadow
    float shadow = traceShadow(fragWorldPos + vec3(0.0, 0.01, 0.0), L, lightDist);
    
    // Diffuse
    float NdotL = max(dot(N, L), 0.0);
    vec3 diffuse = albedo * NdotL;
    
    // Specular
    vec3 H = normalize(V + L);
    float roughness = rma.r;
    float spec = pow(max(dot(N, H), 0.0), mix(128.0, 8.0, roughness));
    
    // Ambient
    vec3 ambient = albedo * 0.15;
    
    // Combine with shadow
    vec3 color = ambient + (diffuse * 0.7 + vec3(spec * 0.3)) * shadow;
    
    // Add subtle grid lines
    vec2 grid = abs(fract(fragWorldPos.xz) - 0.5);
    float gridLine = 1.0 - smoothstep(0.45, 0.48, min(grid.x, grid.y));
    color += vec3(0.1, 0.4, 0.5) * gridLine * 0.3;
    
    // Distance fade
    float dist = length(fragWorldPos.xz);
    float fade = 1.0 - smoothstep(4.0, 6.0, dist);
    
    outColor = vec4(color, fade * 0.9);
}
