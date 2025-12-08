struct Globals {
    view_proj: mat4x4<f32>,
    bar_width: f32,
    max_value: f32,
    focus_distance: f32,
    focus_range: f32,
};

@group(0) @binding(0)
var<uniform> globals: Globals;

struct VertexIn {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) offset: f32,
    @location(3) height: f32,
    @location(4) z: f32,
    @location(5) state: u32,
};

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) state: u32,
    @location(2) screen_uv: vec2<f32>,
};

fn select_state_color(state: u32) -> vec4<f32> {
    switch state {
        // idle
        case 0u: { return vec4<f32>(0.42, 0.78, 1.00, 0.55); }
        // compare
        case 1u: { return vec4<f32>(1.00, 0.75, 0.35, 0.65); }
        // swap
        case 2u: { return vec4<f32>(1.00, 0.45, 0.65, 0.70); }
        // sorted
        case 3u: { return vec4<f32>(0.65, 1.00, 0.75, 0.60); }
        default: { return vec4<f32>(0.70, 0.70, 0.90, 0.50); }
    }
}

@vertex
fn vs_main(input: VertexIn) -> VertexOut {
    // World-space bar quad in a simple strip; bar_width is already in NDC scale (2.0 / count)
    // Slightly slimmer bars to create visible gaps between them
    let width_scale = 0.7;
    let world_x = input.offset + input.position.x * globals.bar_width * width_scale;
    let world_y = input.position.y * input.height * 1.25;
    let world = vec4<f32>(world_x, world_y, input.z, 1.0);

    var out: VertexOut;
    let clip = globals.view_proj * world;
    out.position = clip;
    out.uv = input.uv;
    out.state = input.state;
    // Screen-space UV (0..1) for sampling scene texture
    out.screen_uv = 0.5 * (clip.xy / clip.w) + vec2<f32>(0.5, 0.5);
    return out;
}

@group(1) @binding(0)
var scene_tex: texture_2d<f32>;
@group(1) @binding(1)
var scene_samp: sampler;

@fragment
fn fs_main(input: VertexOut) -> @location(0) vec4<f32> {
    let state_color = select_state_color(input.state);

    // Normalized height from UV.y (0 bottom, 1 top)
    let h = clamp(input.uv.y, 0.0, 1.0);

    // Base near-white with stronger cool→warm tint by height
    let cool = vec3<f32>(0.75, 0.90, 1.10);
    let warm = vec3<f32>(1.10, 0.90, 0.80);
    let tint = mix(cool, warm, h);
    let base = mix(vec3<f32>(0.85, 0.88, 0.95), tint, 0.65);

    // Multiply by state tint for compare/swap colors
    var color = base * state_color.rgb;

    // Simple glass Fresnel rim (view ~ camera looking down -Z)
    let view_dir = normalize(vec3<f32>(0.0, 0.0, 1.0));
    let normal = vec3<f32>(0.0, 0.0, 1.0);
    let fresnel = pow(1.0 - max(dot(view_dir, normal), 0.0), 3.0);
    let rim = vec3<f32>(0.55, 0.95, 1.35) * fresnel * 1.0;

    // Specular highlight (low roughness)
    let light_dir = normalize(vec3<f32>(-0.3, 0.7, 0.6));
    let half_vec = normalize(light_dir + view_dir);
    let spec = pow(max(dot(normal, half_vec), 0.0), 96.0);
    let spec_col = vec3<f32>(1.2, 1.2, 1.3) * spec * 1.2;

    // Thin emissive top edge
    let edge_mask = smoothstep(0.85, 0.98, h);
    let edge = vec3<f32>(0.9, 1.1, 1.4) * edge_mask * 1.2;

    // Combine PBR-ish glass contributions
    var glass_rgb = color * 0.9 + rim + spec_col + edge;

    // Simple volumetric absorption toward the bar center
    let center = vec2<f32>(0.5, 0.5);
    let radial = length(input.uv - center);
    let absorb = smoothstep(0.0, 0.6, radial);
    let depth_tint = vec3<f32>(0.8, 0.9, 1.1);
    let volumetric = mix(depth_tint, vec3<f32>(1.0, 1.0, 1.0), absorb);
    glass_rgb = glass_rgb * volumetric;

    // Fake refraction: distort background scene under the bar
    let distortion = (input.uv - center) * vec2<f32>(0.03, 0.05);
    let refract_uv = input.screen_uv + distortion;
    let bg = textureSample(scene_tex, scene_samp, refract_uv).rgb;

    // Mix background and glass: more glassy near edges, more background in the middle
    let edge_factor = smoothstep(0.2, 0.9, length(input.uv - center));
    let mixed_rgb = mix(bg, glass_rgb, edge_factor);

    // More transparent, opacity driven more by fresnel
    let alpha = clamp(0.35 + 0.35 * fresnel, 0.25, 0.7);
    return vec4<f32>(mixed_rgb * alpha, alpha);
}
