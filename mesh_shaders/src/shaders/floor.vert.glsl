#version 460

layout(location = 0) out vec3 fragWorldPos;
layout(location = 1) out vec2 fragUV;

layout(push_constant) uniform PushConstants {
    mat4 mvp;
    mat4 inv_view_proj;
    vec4 camera_pos;
    vec4 light_pos;
    float time;
} pc;

void main() {
    // Large floor quad at y = -1.0, extending toward camera (positive Z)
    vec2 positions[6] = vec2[](
        vec2(-5.0, -2.0),
        vec2( 5.0, -2.0),
        vec2( 5.0, 10.0),
        vec2(-5.0, -2.0),
        vec2( 5.0, 10.0),
        vec2(-5.0, 10.0)
    );
    
    vec2 uvs[6] = vec2[](
        vec2(0.0, 0.0),
        vec2(4.0, 0.0),
        vec2(4.0, 4.0),
        vec2(0.0, 0.0),
        vec2(4.0, 4.0),
        vec2(0.0, 4.0)
    );
    
    vec2 pos = positions[gl_VertexIndex];
    vec3 worldPos = vec3(pos.x, -1.0, pos.y);
    
    gl_Position = pc.mvp * vec4(worldPos, 1.0);
    fragWorldPos = worldPos;
    fragUV = uvs[gl_VertexIndex];
}
