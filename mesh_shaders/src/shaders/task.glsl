#version 460
#extension GL_EXT_mesh_shader : require

// Task shader - dispatches mesh shader workgroups for selection sort bars

layout(local_size_x = 1, local_size_y = 1, local_size_z = 1) in;

struct TaskPayload {
    uint meshletCount;
    vec3 baseColor;
};

taskPayloadSharedEXT TaskPayload payload;

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

void main() {
    payload.meshletCount = 8;
    payload.baseColor = vec3(1.0);
    
    EmitMeshTasksEXT(payload.meshletCount, 1, 1);
}
