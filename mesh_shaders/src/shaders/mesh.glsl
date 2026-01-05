#version 460
#extension GL_EXT_mesh_shader : require

// Mesh shader - generates bars for selection sort visualization

layout(local_size_x = 1, local_size_y = 1, local_size_z = 1) in;
layout(triangles, max_vertices = 64, max_primitives = 32) out;

layout(location = 0) out vec3 fragWorldPos[];
layout(location = 1) out vec3 fragNormal[];
layout(location = 2) out vec2 fragUV[];
layout(location = 3) out vec3 fragTangent[];
layout(location = 4) flat out int barState[]; // 0=normal, 1=current_i, 2=current_j, 3=min_idx, 4=sorted

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
    uint barIndex = gl_WorkGroupID.x;
    
    // Get height from push constants
    float height = pc.bar_heights[barIndex];
    
    // Determine bar state for coloring
    int state = 0; // normal
    if (int(barIndex) < pc.current_i) {
        state = 4; // sorted (green)
    } else if (int(barIndex) == pc.current_i) {
        state = 1; // current position being filled (yellow)
    } else if (int(barIndex) == pc.min_idx) {
        state = 3; // current minimum (red)
    } else if (int(barIndex) == pc.current_j) {
        state = 2; // currently comparing (cyan)
    }
    
    // Position bars in a row along X axis
    float xOffset = (float(barIndex) - 3.5) * 0.7;
    vec3 offset = vec3(xOffset, -1.0, 0.0);
    
    float halfWidth = 0.25;
    float halfDepth = 0.25;
    
    // Bar vertices (8 corners)
    vec3 positions[8] = vec3[8](
        vec3(-halfWidth, 0.0, -halfDepth),
        vec3( halfWidth, 0.0, -halfDepth),
        vec3( halfWidth, height, -halfDepth),
        vec3(-halfWidth, height, -halfDepth),
        vec3(-halfWidth, 0.0,  halfDepth),
        vec3( halfWidth, 0.0,  halfDepth),
        vec3( halfWidth, height,  halfDepth),
        vec3(-halfWidth, height,  halfDepth)
    );
    
    // Face indices for 24 vertices (4 per face)
    int faceIndices[24] = int[24](
        0, 1, 2, 3,  // Front (Z-)
        5, 4, 7, 6,  // Back (Z+)
        4, 0, 3, 7,  // Left (X-)
        1, 5, 6, 2,  // Right (X+)
        3, 2, 6, 7,  // Top (Y+)
        4, 5, 1, 0   // Bottom (Y-)
    );
    
    vec3 faceNormals[6] = vec3[6](
        vec3( 0,  0, -1),
        vec3( 0,  0,  1),
        vec3(-1,  0,  0),
        vec3( 1,  0,  0),
        vec3( 0,  1,  0),
        vec3( 0, -1,  0)
    );
    
    vec3 faceTangents[6] = vec3[6](
        vec3( 1,  0,  0),
        vec3(-1,  0,  0),
        vec3( 0,  0,  1),
        vec3( 0,  0, -1),
        vec3( 1,  0,  0),
        vec3( 1,  0,  0)
    );
    
    vec2 faceUVs[4] = vec2[4](
        vec2(0, 0),
        vec2(1, 0),
        vec2(1, 1),
        vec2(0, 1)
    );
    
    // Output 24 vertices (4 per face)
    for (int face = 0; face < 6; face++) {
        for (int v = 0; v < 4; v++) {
            int vertIdx = face * 4 + v;
            int posIdx = faceIndices[vertIdx];
            
            vec3 worldPos = positions[posIdx] + offset;
            gl_MeshVerticesEXT[vertIdx].gl_Position = pc.mvp * vec4(worldPos, 1.0);
            
            fragWorldPos[vertIdx] = worldPos;
            fragNormal[vertIdx] = faceNormals[face];
            fragTangent[vertIdx] = faceTangents[face];
            fragUV[vertIdx] = faceUVs[v];
            barState[vertIdx] = state;
        }
    }
    
    // Output 12 triangles (2 per face)
    for (int face = 0; face < 6; face++) {
        int base = face * 4;
        gl_PrimitiveTriangleIndicesEXT[face * 2 + 0] = uvec3(base + 0, base + 1, base + 2);
        gl_PrimitiveTriangleIndicesEXT[face * 2 + 1] = uvec3(base + 0, base + 2, base + 3);
    }
    
    SetMeshOutputsEXT(24, 12);
}
