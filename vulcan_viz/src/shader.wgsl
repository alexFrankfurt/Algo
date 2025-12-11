struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_normal: vec3<f32>,
    @location(1) world_position: vec3<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    var positions = array<vec3<f32>, 36>(
        // Front face
        vec3<f32>(-0.5, -1.0,  0.5), vec3<f32>( 0.5, -1.0,  0.5), vec3<f32>( 0.5,  1.0,  0.5),
        vec3<f32>(-0.5, -1.0,  0.5), vec3<f32>( 0.5,  1.0,  0.5), vec3<f32>(-0.5,  1.0,  0.5),
        // Back face
        vec3<f32>(-0.5, -1.0, -0.5), vec3<f32>(-0.5,  1.0, -0.5), vec3<f32>( 0.5,  1.0, -0.5),
        vec3<f32>(-0.5, -1.0, -0.5), vec3<f32>( 0.5,  1.0, -0.5), vec3<f32>( 0.5, -1.0, -0.5),
        // Top face
        vec3<f32>(-0.5,  1.0, -0.5), vec3<f32>(-0.5,  1.0,  0.5), vec3<f32>( 0.5,  1.0,  0.5),
        vec3<f32>(-0.5,  1.0, -0.5), vec3<f32>( 0.5,  1.0,  0.5), vec3<f32>( 0.5,  1.0, -0.5),
        // Bottom face
        vec3<f32>(-0.5, -1.0, -0.5), vec3<f32>( 0.5, -1.0, -0.5), vec3<f32>( 0.5, -1.0,  0.5),
        vec3<f32>(-0.5, -1.0, -0.5), vec3<f32>( 0.5, -1.0,  0.5), vec3<f32>(-0.5, -1.0,  0.5),
        // Right face
        vec3<f32>( 0.5, -1.0, -0.5), vec3<f32>( 0.5,  1.0, -0.5), vec3<f32>( 0.5,  1.0,  0.5),
        vec3<f32>( 0.5, -1.0, -0.5), vec3<f32>( 0.5,  1.0,  0.5), vec3<f32>( 0.5, -1.0,  0.5),
        // Left face
        vec3<f32>(-0.5, -1.0, -0.5), vec3<f32>(-0.5, -1.0,  0.5), vec3<f32>(-0.5,  1.0,  0.5),
        vec3<f32>(-0.5, -1.0, -0.5), vec3<f32>(-0.5,  1.0,  0.5), vec3<f32>(-0.5,  1.0, -0.5)
    );

    var normals = array<vec3<f32>, 36>(
        // Front
        vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(0.0, 0.0, 1.0),
        vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(0.0, 0.0, 1.0),
        // Back
        vec3<f32>(0.0, 0.0, -1.0), vec3<f32>(0.0, 0.0, -1.0), vec3<f32>(0.0, 0.0, -1.0),
        vec3<f32>(0.0, 0.0, -1.0), vec3<f32>(0.0, 0.0, -1.0), vec3<f32>(0.0, 0.0, -1.0),
        // Top
        vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(0.0, 1.0, 0.0),
        vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(0.0, 1.0, 0.0),
        // Bottom
        vec3<f32>(0.0, -1.0, 0.0), vec3<f32>(0.0, -1.0, 0.0), vec3<f32>(0.0, -1.0, 0.0),
        vec3<f32>(0.0, -1.0, 0.0), vec3<f32>(0.0, -1.0, 0.0), vec3<f32>(0.0, -1.0, 0.0),
        // Right
        vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(1.0, 0.0, 0.0),
        vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(1.0, 0.0, 0.0),
        // Left
        vec3<f32>(-1.0, 0.0, 0.0), vec3<f32>(-1.0, 0.0, 0.0), vec3<f32>(-1.0, 0.0, 0.0),
        vec3<f32>(-1.0, 0.0, 0.0), vec3<f32>(-1.0, 0.0, 0.0), vec3<f32>(-1.0, 0.0, 0.0)
    );

    var out: VertexOutput;
    let pos = positions[in_vertex_index];
    let normal = normals[in_vertex_index];
    
    out.world_position = pos;
    out.world_normal = normal;

    // Simple fixed camera view matrix
    // Eye: (3.0, 3.0, 3.0), Target: (0.0, 0.0, 0.0), Up: (0.0, 1.0, 0.0)
    let view = mat4x4<f32>(
        vec4<f32>( 0.707, -0.408,  0.577, 0.0),
        vec4<f32>( 0.0,    0.816,  0.577, 0.0),
        vec4<f32>(-0.707, -0.408,  0.577, 0.0),
        vec4<f32>( 0.0,    0.0,   -5.196, 1.0)
    );

    // Perspective projection
    // fov 45, aspect 1.33, near 0.1, far 100.0
    let f = 1.0 / tan(radians(45.0) / 2.0);
    let aspect = 1.33;
    let near = 0.1;
    let far = 100.0;
    
    let proj = mat4x4<f32>(
        vec4<f32>(f / aspect, 0.0, 0.0, 0.0),
        vec4<f32>(0.0, f, 0.0, 0.0),
        vec4<f32>(0.0, 0.0, far / (near - far), -1.0),
        vec4<f32>(0.0, 0.0, (near * far) / (near - far), 0.0)
    );

    out.clip_position = proj * view * vec4<f32>(pos, 1.0);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let light_dir = normalize(vec3<f32>(1.0, 1.0, 1.0));
    let view_dir = normalize(vec3<f32>(3.0, 3.0, 3.0) - in.world_position);
    let normal = normalize(in.world_normal);

    // Ambient
    let ambient = 0.2;

    // Diffuse
    let diff = max(dot(normal, light_dir), 0.0);

    // Specular (Phong)
    let reflect_dir = reflect(-light_dir, normal);
    let spec = pow(max(dot(view_dir, reflect_dir), 0.0), 32.0);

    // Glass color (Cyan-ish)
    let base_color = vec3<f32>(0.2, 0.6, 0.8);
    
    let result = (ambient + diff) * base_color + vec3<f32>(spec);
    
    // Alpha 0.4 for translucency
    return vec4<f32>(result, 0.4);
}
