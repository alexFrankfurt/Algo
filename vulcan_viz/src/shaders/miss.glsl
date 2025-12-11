#version 460
#extension GL_EXT_ray_tracing : require

struct RayPayload {
    vec3 color;
    uint depth;
};

layout(location = 0) rayPayloadInEXT RayPayload payload;

layout(push_constant) uniform Constants { float time; } pushC;

void main()
{
    vec3 rayDir = normalize(gl_WorldRayDirectionEXT);
    
    // Cyberpunk Grid Floor
    if (rayDir.y < -0.05) {
        float t = -2.0 / rayDir.y; // Plane at y = -2.0
        vec3 hitPos = gl_WorldRayOriginEXT + t * rayDir;
        
        // Grid logic
        float gridSize = 1.0;
        float lineWidth = 0.05;
        
        // Animate grid
        float zShift = pushC.time * 2.0;
        
        vec2 grid = abs(fract((hitPos.xz + vec2(0, zShift)) / gridSize - 0.5) - 0.5) / lineWidth;
        float line = min(grid.x, grid.y);
        
        if (line < 1.0) {
            float pulse = 0.5 + 0.5 * sin(pushC.time * 3.0 + hitPos.z * 0.5);
            payload.color = vec3(0.0, 0.8, 1.0) * (1.0 + pulse); // Pulsing cyan grid
            return;
        }
        
        // Floor reflection (dark)
        payload.color = vec3(0.05, 0.05, 0.1);
        return;
    }

    // Studio Softbox Light (Top)
    if (rayDir.y > 0.8 && abs(rayDir.x) < 0.3 && abs(rayDir.z) < 0.3) {
        payload.color = vec3(4.0, 4.0, 4.0); // Very bright white light
        return;
    }

    // Dark Sky Gradient
    float t = 0.5 * (rayDir.y + 1.0);
    payload.color = mix(vec3(0.05, 0.05, 0.1), vec3(0.0, 0.0, 0.2), t);
}
