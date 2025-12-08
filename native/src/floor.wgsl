struct Globals {
    view_proj: mat4x4<f32>,
    bar_width: f32,
    max_value: f32,
    focus_distance: f32,
    focus_range: f32,
};

@group(0) @binding(0)
var<uniform> globals: Globals;

struct VSIn {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
};

struct VSOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@group(1) @binding(0)
var floor_albedo: texture_2d<f32>;
@group(1) @binding(1)
var floor_normal: texture_2d<f32>;
@group(1) @binding(2)
var floor_rma: texture_2d<f32>;
@group(1) @binding(3)
var floor_sampler: sampler;

@vertex
fn vs_main(input: VSIn) -> VSOut {
    var o: VSOut;
    let world = vec4<f32>(input.position, 1.0);
    o.pos = globals.view_proj * world;
    o.uv = input.uv;
    return o;
}

@fragment
fn fs_main(in: VSOut) -> @location(0) vec4<f32> {
    let uv = in.uv;

    // Sample PBR textures
    let albedo = textureSample(floor_albedo, floor_sampler, uv).rgb;
    let rma = textureSample(floor_rma, floor_sampler, uv).rgb;
    let rough = clamp(rma.r, 0.04, 1.0);
    let metal = rma.g;
    let ao = rma.b;

    // Decode normal (assume +Y up, X/Z in-plane)
    let n_tex = textureSample(floor_normal, floor_sampler, uv).xyz * 2.0 - 1.0;
    let N = normalize(vec3<f32>(n_tex.x, n_tex.z, n_tex.y));

    let L = normalize(vec3<f32>(-0.4, 1.0, 0.2));
    let V = normalize(vec3<f32>(0.0, 1.0, 1.0));
    let H = normalize(L + V);

    let NdotL = max(dot(N, L), 0.0);
    let diff = albedo * (0.1 + 0.9 * NdotL) * ao;

    let spec_power = mix(16.0, 96.0, 1.0 - rough);
    let spec = pow(max(dot(N, H), 0.0), spec_power);
    let spec_col = mix(vec3<f32>(0.08), albedo, metal) * spec * 1.5;

    var color = diff + spec_col;

    // Emissive from bright traces
    let glow_src = max(albedo.r, max(albedo.g, albedo.b));
    let glow = smoothstep(0.7, 0.95, glow_src);
    color += glow * vec3<f32>(0.10, 0.40, 0.90);

    return vec4<f32>(color, 1.0);
}
