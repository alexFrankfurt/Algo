struct VSOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_fullscreen(@location(0) position: vec2<f32>, @location(1) uv: vec2<f32>) -> VSOut {
    var o: VSOut;
    o.pos = vec4<f32>(position, 0.0, 1.0);
    o.uv = uv;
    return o;
}

@group(0) @binding(0) var src_tex: texture_2d<f32>;
@group(0) @binding(1) var src_samp: sampler;

// Horizontal blur (manually unrolled for WebGPU const indexing)
@fragment
fn fs_blur_h(in: VSOut) -> @location(0) vec4<f32> {
    let texel = 1.0 / vec2<f32>(textureDimensions(src_tex));
    let w0 = 0.204164;
    let w1 = 0.304005;
    let w2 = 0.193783;
    let w3 = 0.072086;
    let w4 = 0.017962;

    var acc = vec3<f32>(0.0);
    acc += textureSample(src_tex, src_samp, in.uv + vec2<f32>(-4.0 * texel.x, 0.0)).rgb * w4;
    acc += textureSample(src_tex, src_samp, in.uv + vec2<f32>(-3.0 * texel.x, 0.0)).rgb * w3;
    acc += textureSample(src_tex, src_samp, in.uv + vec2<f32>(-2.0 * texel.x, 0.0)).rgb * w2;
    acc += textureSample(src_tex, src_samp, in.uv + vec2<f32>(-1.0 * texel.x, 0.0)).rgb * w1;
    acc += textureSample(src_tex, src_samp, in.uv).rgb * w0;
    acc += textureSample(src_tex, src_samp, in.uv + vec2<f32>(1.0 * texel.x, 0.0)).rgb * w1;
    acc += textureSample(src_tex, src_samp, in.uv + vec2<f32>(2.0 * texel.x, 0.0)).rgb * w2;
    acc += textureSample(src_tex, src_samp, in.uv + vec2<f32>(3.0 * texel.x, 0.0)).rgb * w3;
    acc += textureSample(src_tex, src_samp, in.uv + vec2<f32>(4.0 * texel.x, 0.0)).rgb * w4;
    return vec4<f32>(acc, 1.0);
}

// Vertical blur (manually unrolled)
@fragment
fn fs_blur_v(in: VSOut) -> @location(0) vec4<f32> {
    let texel = 1.0 / vec2<f32>(textureDimensions(src_tex));
    let w0 = 0.204164;
    let w1 = 0.304005;
    let w2 = 0.193783;
    let w3 = 0.072086;
    let w4 = 0.017962;

    var acc = vec3<f32>(0.0);
    acc += textureSample(src_tex, src_samp, in.uv + vec2<f32>(0.0, -4.0 * texel.y)).rgb * w4;
    acc += textureSample(src_tex, src_samp, in.uv + vec2<f32>(0.0, -3.0 * texel.y)).rgb * w3;
    acc += textureSample(src_tex, src_samp, in.uv + vec2<f32>(0.0, -2.0 * texel.y)).rgb * w2;
    acc += textureSample(src_tex, src_samp, in.uv + vec2<f32>(0.0, -1.0 * texel.y)).rgb * w1;
    acc += textureSample(src_tex, src_samp, in.uv).rgb * w0;
    acc += textureSample(src_tex, src_samp, in.uv + vec2<f32>(0.0, 1.0 * texel.y)).rgb * w1;
    acc += textureSample(src_tex, src_samp, in.uv + vec2<f32>(0.0, 2.0 * texel.y)).rgb * w2;
    acc += textureSample(src_tex, src_samp, in.uv + vec2<f32>(0.0, 3.0 * texel.y)).rgb * w3;
    acc += textureSample(src_tex, src_samp, in.uv + vec2<f32>(0.0, 4.0 * texel.y)).rgb * w4;
    return vec4<f32>(acc, 1.0);
}

@group(0) @binding(0) var scene_tex: texture_2d<f32>;
@group(0) @binding(1) var bloom_tex: texture_2d<f32>;
@group(0) @binding(2) var depth_tex: texture_depth_2d;
@group(0) @binding(3) var post_samp: sampler;

fn aces_tonemap(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

fn linearize_depth(depth: f32) -> f32 {
    // Matches near/far used in renderer
    let near = 0.1;
    let far = 10.0;
    let z = depth * 2.0 - 1.0;
    return (2.0 * near * far) / (far + near - z * (far - near));
}

@fragment
fn fs_tonemap(in: VSOut) -> @location(0) vec4<f32> {
    let scene = textureSample(scene_tex, post_samp, in.uv).rgb;
    let bloom = textureSample(bloom_tex, post_samp, in.uv).rgb;
    let depth = textureSample(depth_tex, post_samp, in.uv);
    let linear_depth = linearize_depth(depth);

    let focus = 2.3;
    let focus_range = 2.5;
    let coc = clamp(abs(linear_depth - focus) / focus_range, 0.0, 1.0);

    // Clamp scene to avoid washout and tone map after reduced bloom
    let scene_clamped = clamp(scene, vec3<f32>(0.0), vec3<f32>(4.0));
    let blurred = bloom; // blur_b is already blurred scene
    let dof_mix = mix(scene_clamped, blurred, coc * 0.5);

    let color = dof_mix + bloom * 0.28;
    let mapped = aces_tonemap(color);
    // Softer vignette
    let d = length(in.uv * 2.0 - 1.0);
    let vig = mix(1.0, 0.9, smoothstep(0.92, 1.10, d));
    return vec4<f32>(mapped * vig, 1.0);
}
