#version 460
#extension GL_EXT_mesh_shader : require

// Mesh shader with UV output for texturing

layout(local_size_x = 1, local_size_y = 1, local_size_z = 1) in;
layout(triangles, max_vertices = 64, max_primitives = 32) out;

// Outputs to fragment shader
layout(location = 0) out vec3 fragWorldPos[];
layout(location = 1) out vec3 fragNormal[];
layout(location = 2) out vec2 fragUV[];
layout(location = 3) out vec3 fragTangent[];

struct TaskPayload {
    uint meshletCount;
    vec3 baseColor;
};

taskPayloadSharedEXT TaskPayload payload;

layout(push_constant) uniform PushConstants {
    mat4 mvp;
    float time;
} pc;

// Hash for per-instance variation
float hash(float n) {
    return fract(sin(n) * 43758.5453);
}

void main() {
    uint meshletIndex = gl_WorkGroupID.x;
    
    // Position in ring
    float angle = float(meshletIndex) * 6.28318 / float(payload.meshletCount);
    float radius = 2.0;
    vec3 offset = vec3(cos(angle) * radius, sin(pc.time + angle) * 0.3, sin(angle) * radius);
    
    // Per-cube variation
    float sizeVar = 0.8 + 0.4 * hash(float(meshletIndex));
    float size = 0.35 * sizeVar;
    
    // Cube vertices
    vec3 positions[8] = vec3[8](
        vec3(-size, -size, -size),
        vec3( size, -size, -size),
        vec3( size,  size, -size),
        vec3(-size,  size, -size),
        vec3(-size, -size,  size),
        vec3( size, -size,  size),
        vec3( size,  size,  size),
        vec3(-size,  size,  size)
    );
    
    // We need 24 vertices for proper per-face normals and UVs
    // 6 faces * 4 vertices each
    
    // Face data: positions indices, normal, tangent, UVs
    // Front face (Z-)
    int faceIndices[24] = int[24](
        0, 1, 2, 3,  // Front
        5, 4, 7, 6,  // Back
        4, 0, 3, 7,  // Left
        1, 5, 6, 2,  // Right
        3, 2, 6, 7,  // Top
        4, 5, 1, 0   // Bottom
    );
    
    vec3 faceNormals[6] = vec3[6](
        vec3( 0,  0, -1),  // Front
        vec3( 0,  0,  1),  // Back
        vec3(-1,  0,  0),  // Left
        vec3( 1,  0,  0),  // Right
        vec3( 0,  1,  0),  // Top
        vec3( 0, -1,  0)   // Bottom
    );
    
    vec3 faceTangents[6] = vec3[6](
        vec3( 1,  0,  0),  // Front
        vec3(-1,  0,  0),  // Back
        vec3( 0,  0,  1),  // Left
        vec3( 0,  0, -1),  // Right
        vec3( 1,  0,  0),  // Top
        vec3( 1,  0,  0)   // Bottom
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
