#version 460
#extension GL_EXT_ray_tracing : require
#extension GL_EXT_scalar_block_layout : require

struct RayPayload {
    vec3 color;
    uint depth;
};

layout(location = 0) rayPayloadInEXT RayPayload payload;

hitAttributeEXT vec2 attribs;

layout(binding = 0, set = 0) uniform accelerationStructureEXT topLevelAS;
layout(binding = 3, set = 0, scalar) buffer Vertices { vec3 v[]; } vertices;
layout(binding = 4, set = 0, scalar) buffer Indices { uint i[]; } indices;

struct Vertex { vec3 pos; };

layout(push_constant) uniform Constants { float time; } pushC;

void main()
{
    // Fetch indices
    uint ind0 = indices.i[3 * gl_PrimitiveID + 0];
    uint ind1 = indices.i[3 * gl_PrimitiveID + 1];
    uint ind2 = indices.i[3 * gl_PrimitiveID + 2];

    // Fetch vertices
    vec3 v0 = vertices.v[ind0];
    vec3 v1 = vertices.v[ind1];
    vec3 v2 = vertices.v[ind2];

    // Compute normal
    const vec3 n0 = v1 - v0;
    const vec3 n1 = v2 - v0;
    vec3 normal = normalize(cross(n0, n1));
    normal = normalize(vec3(gl_ObjectToWorldEXT * vec4(normal, 0.0)));

    // --- Surface Imperfections (Procedural Bump Map) ---
    // Simple hash-based noise
    vec3 p = (gl_WorldRayOriginEXT + gl_WorldRayDirectionEXT * gl_HitTEXT) * 10.0;
    p.y -= pushC.time * 0.5; // Animate noise downwards (like rain/flow)
    vec3 i = floor(p);
    vec3 f = fract(p);
    f = f*f*(3.0-2.0*f);
    float n = mix(mix(mix(fract(sin(dot(i + vec3(0,0,0), vec3(12.9898,78.233,45.543))) * 43758.5453), 
                          fract(sin(dot(i + vec3(1,0,0), vec3(12.9898,78.233,45.543))) * 43758.5453),f.x),
                      mix(fract(sin(dot(i + vec3(0,1,0), vec3(12.9898,78.233,45.543))) * 43758.5453), 
                          fract(sin(dot(i + vec3(1,1,0), vec3(12.9898,78.233,45.543))) * 43758.5453),f.x),f.y),
                  mix(mix(fract(sin(dot(i + vec3(0,0,1), vec3(12.9898,78.233,45.543))) * 43758.5453), 
                          fract(sin(dot(i + vec3(1,0,1), vec3(12.9898,78.233,45.543))) * 43758.5453),f.x),
                      mix(fract(sin(dot(i + vec3(0,1,1), vec3(12.9898,78.233,45.543))) * 43758.5453), 
                          fract(sin(dot(i + vec3(1,1,1), vec3(12.9898,78.233,45.543))) * 43758.5453),f.x),f.y),f.z);
    
    // Perturb normal
    normal = normalize(normal + (vec3(n) - 0.5) * 0.05);
    // ---------------------------------------------------

    // Basic lighting
    vec3 lightDir = normalize(vec3(0.5, 1.0, 0.5));
    
    // --- Material Logic ---
    vec3 absorbColor;
    float ior = 1.45;
    float emission = 0.0;
    bool isOpaque = false;
    
    if (gl_InstanceCustomIndexEXT == 0) {
        // Floor: Opaque, Dark, Reflective
        absorbColor = vec3(0.05, 0.05, 0.08);
        ior = 1.5; 
        isOpaque = true;
    } else {
        // Bars: Glassy with Cyan/Pink gradient
        float t = float(gl_InstanceCustomIndexEXT) / 12.0; // Normalized value
        
        // Palette: Cyan -> Purple -> Pink
        vec3 cyan = vec3(0.0, 0.9, 1.0);
        vec3 purple = vec3(0.6, 0.0, 1.0);
        vec3 pink = vec3(1.0, 0.2, 0.6);
        
        vec3 color;
        if (t < 0.5) {
            color = mix(cyan, purple, t * 2.0);
        } else {
            color = mix(purple, pink, (t - 0.5) * 2.0);
        }
        
        absorbColor = color;
        emission = 0.1; // Subtle glow
    }
    // ----------------------

    vec3 viewDir = -gl_WorldRayDirectionEXT;
    float NdotV = dot(normal, viewDir);
    
    // Fresnel
    float F0 = pow((1.0 - ior) / (1.0 + ior), 2.0);
    float fresnel = F0 + (1.0 - F0) * pow(1.0 - abs(NdotV), 5.0);

    // Recursion Limit Check
    if (payload.depth >= 6) {
        payload.color = vec3(0.0, 0.05, 0.1); // Darker fallback
        return;
    }

    // Save current depth
    uint currentDepth = payload.depth;

    // Reflection
    vec3 reflectDir = reflect(gl_WorldRayDirectionEXT, normal);
    
    payload.depth = currentDepth + 1;
    traceRayEXT(topLevelAS, gl_RayFlagsNoneEXT, 0xff, 0, 0, 0, gl_WorldRayOriginEXT + gl_WorldRayDirectionEXT * gl_HitTEXT, 0.001, reflectDir, 100.0, 0);
    vec3 reflectColor = payload.color;
    
    if (isOpaque) {
        // Opaque material (Floor)
        // Mix diffuse (absorbColor) and reflection based on Fresnel
        // Simple lighting for diffuse
        float diff = max(dot(normal, lightDir), 0.0);
        vec3 diffuse = absorbColor * (diff + 0.2); // Ambient
        
        payload.color = mix(diffuse, reflectColor, fresnel * 0.8); // 0.8 reflectivity
        return;
    }

    // Refraction with Dispersion (Chromatic Aberration)
    vec3 refractColor = vec3(0.0);
    
    // IORs for R, G, B (Dispersion)
    float iorR = 1.43;
    float iorG = 1.45;
    float iorB = 1.47;
    
    vec3 dirR = refract(gl_WorldRayDirectionEXT, normal, 1.0 / iorR);
    vec3 dirG = refract(gl_WorldRayDirectionEXT, normal, 1.0 / iorG);
    vec3 dirB = refract(gl_WorldRayDirectionEXT, normal, 1.0 / iorB);
    
    // Trace Red
    if (length(dirR) > 0.0) {
        payload.depth = currentDepth + 1;
        traceRayEXT(topLevelAS, gl_RayFlagsNoneEXT, 0xff, 0, 0, 0, gl_WorldRayOriginEXT + gl_WorldRayDirectionEXT * gl_HitTEXT, 0.001, dirR, 100.0, 0);
        refractColor.r = payload.color.r * absorbColor.r;
    } else {
        fresnel = 1.0; // TIR
    }
    
    // Trace Green
    if (length(dirG) > 0.0) {
        payload.depth = currentDepth + 1;
        traceRayEXT(topLevelAS, gl_RayFlagsNoneEXT, 0xff, 0, 0, 0, gl_WorldRayOriginEXT + gl_WorldRayDirectionEXT * gl_HitTEXT, 0.001, dirG, 100.0, 0);
        refractColor.g = payload.color.g * absorbColor.g;
    }
    
    // Trace Blue
    if (length(dirB) > 0.0) {
        payload.depth = currentDepth + 1;
        traceRayEXT(topLevelAS, gl_RayFlagsNoneEXT, 0xff, 0, 0, 0, gl_WorldRayOriginEXT + gl_WorldRayDirectionEXT * gl_HitTEXT, 0.001, dirB, 100.0, 0);
        refractColor.b = payload.color.b * absorbColor.b;
    }

    // Specular Highlight
    vec3 halfVec = normalize(lightDir + viewDir);
    float NdotH = max(dot(normal, halfVec), 0.0);
    float specular = pow(NdotH, 64.0);
    vec3 specularColor = vec3(1.0) * specular;

    // Iridescence / Edge Glow (Subtle rainbow)
    float edgeFactor = pow(1.0 - abs(NdotV), 3.0);
    vec3 iridescence = 0.5 + 0.5 * cos(6.28318 * (edgeFactor + vec3(0.0, 0.33, 0.67)));
    
    // Combine
    payload.color = mix(refractColor, reflectColor, fresnel);
    payload.color += specularColor;
    payload.color += iridescence * edgeFactor * 0.2;
    payload.color += absorbColor * emission;
}
