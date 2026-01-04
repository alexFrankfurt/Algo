#version 460
#extension GL_EXT_mesh_shader : require

// Task shader - dispatches mesh shader workgroups
// This is the "amplification" stage that decides how many mesh shaders to spawn

layout(local_size_x = 1, local_size_y = 1, local_size_z = 1) in;

// Shared payload passed to mesh shaders
struct TaskPayload {
    uint meshletCount;
    vec3 baseColor;
};

taskPayloadSharedEXT TaskPayload payload;

layout(push_constant) uniform PushConstants {
    mat4 mvp;
    float time;
} pc;

void main() {
    // Determine how many meshlets to render
    // In a real application, this could do frustum culling, LOD selection, etc.
    payload.meshletCount = 8; // Render 8 meshlets (forming a ring)
    
    // Animate color based on time
    payload.baseColor = vec3(
        0.5 + 0.5 * sin(pc.time),
        0.5 + 0.5 * sin(pc.time + 2.094),
        0.5 + 0.5 * sin(pc.time + 4.188)
    );
    
    // Emit mesh shader workgroups - one per meshlet
    EmitMeshTasksEXT(payload.meshletCount, 1, 1);
}
