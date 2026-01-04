# Design Document: Cubemap Implementation

## Overview

This design describes the implementation of an updated cubemap texture in the mesh_shaders Vulkan demo. The cubemap provides an immersive 360° environment background using equirectangular projection. The existing architecture already supports cubemap rendering - this implementation ensures the updated `cubemap.png` asset is properly integrated.

## Architecture

The cubemap implementation follows the existing Vulkan rendering architecture:

```
┌─────────────────────────────────────────────────────────────┐
│                      VulkanApp                               │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────────┐    ┌─────────────────────────────────┐ │
│  │ Texture_Loader  │───▶│ cubemap_texture: Texture        │ │
│  │ (load_texture)  │    │  - image: vk::Image             │ │
│  └─────────────────┘    │  - view: vk::ImageView          │ │
│                         │  - allocation: Allocation        │ │
│                         └─────────────────────────────────┘ │
│                                      │                       │
│                                      ▼                       │
│  ┌─────────────────────────────────────────────────────────┐│
│  │              Background Pipeline                         ││
│  │  ┌─────────────────┐  ┌─────────────────────────────┐  ││
│  │  │ Descriptor Set  │  │ Push Constants              │  ││
│  │  │ binding 0:      │  │  - mvp                      │  ││
│  │  │ cubemap sampler │  │  - inv_view_proj            │  ││
│  │  └────────┬────────┘  │  - time                     │  ││
│  │           │           └─────────────────────────────┘  ││
│  │           ▼                                             ││
│  │  ┌─────────────────────────────────────────────────┐   ││
│  │  │ background.vert.glsl → background.frag.glsl     │   ││
│  │  │ (fullscreen triangle)  (equirect sampling)      │   ││
│  │  └─────────────────────────────────────────────────┘   ││
│  └─────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────┘
```

## Components and Interfaces

### Texture Structure

```rust
struct Texture {
    image: vk::Image,           // Vulkan image handle
    view: vk::ImageView,        // Image view for shader sampling
    allocation: Option<Allocation>, // GPU memory allocation
}
```

### load_texture Function

The existing `load_texture` function handles cubemap loading:

```rust
unsafe fn load_texture(
    device: &Device,
    allocator: &Arc<Mutex<Allocator>>,
    command_pool: vk::CommandPool,
    queue: vk::Queue,
    path: &str,           // "assets/cubemap.png"
    srgb: bool,           // true for cubemap (color data)
) -> Result<Texture, Box<dyn std::error::Error>>
```

### Background Shader Interface

**Vertex Shader Outputs:**
- `outUV: vec2` - Screen-space UV coordinates
- `outViewDir: vec3` - World-space view direction

**Fragment Shader Inputs:**
- `cubemapTex: sampler2D` - Equirectangular cubemap texture (binding 0)
- `pc.invViewProj: mat4` - Inverse view-projection for ray calculation
- `pc.time: float` - Animation time

### Equirectangular Projection

The fragment shader converts 3D view directions to 2D texture coordinates:

```glsl
vec2 dirToEquirect(vec3 dir) {
    dir = normalize(dir);
    float phi = atan(dir.z, dir.x);           // Horizontal angle: -π to π
    float theta = asin(clamp(dir.y, -1.0, 1.0)); // Vertical angle: -π/2 to π/2
    
    vec2 uv;
    uv.x = phi / (2.0 * PI) + 0.5;            // Map to 0-1
    uv.y = 0.5 - theta / PI;                   // Map to 0-1 (flipped)
    return uv;
}
```

## Data Models

### Push Constants

```rust
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PushConstants {
    mvp: [[f32; 4]; 4],          // Model-view-projection matrix
    inv_view_proj: [[f32; 4]; 4], // Inverse view-projection for ray direction
    time: f32,                    // Animation time
    _padding: [f32; 3],           // Alignment padding
}
```

### Descriptor Set Layout

| Binding | Type | Stage | Description |
|---------|------|-------|-------------|
| 0 | COMBINED_IMAGE_SAMPLER | FRAGMENT | Cubemap texture |

### Sampler Configuration

```rust
SamplerCreateInfo {
    mag_filter: LINEAR,
    min_filter: LINEAR,
    mipmap_mode: LINEAR,
    address_mode_u: REPEAT,  // Wrap horizontally for seamless panorama
    address_mode_v: REPEAT,
    address_mode_w: REPEAT,
    anisotropy_enable: true,
    max_anisotropy: 16.0,
}
```

## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system-essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees.*

### Property 1: Equirectangular Projection Bijectivity

*For any* normalized 3D direction vector, the `dirToEquirect` function SHALL produce UV coordinates in the range [0, 1], and *for any* two distinct directions that differ by more than a small epsilon, the resulting UV coordinates SHALL be distinct (injective within the valid domain).

**Validates: Requirements 2.1**

### Property 2: Equirectangular Projection Continuity

*For any* two 3D direction vectors that are close together (small angular difference), the resulting UV coordinates from `dirToEquirect` SHALL also be close together (continuous mapping), except at the horizontal seam where phi wraps from π to -π.

**Validates: Requirements 2.1**

### Property 3: Equirectangular Projection Cardinal Directions

*For any* cardinal direction (forward, back, left, right, up, down), the `dirToEquirect` function SHALL produce predictable UV coordinates:
- Forward (+Z): u ≈ 0.75
- Back (-Z): u ≈ 0.25
- Right (+X): u ≈ 0.5
- Left (-X): u ≈ 0.0 or 1.0 (seam)
- Up (+Y): v ≈ 0.0
- Down (-Y): v ≈ 1.0

**Validates: Requirements 2.1**

## Error Handling

| Error Condition | Handling Strategy |
|-----------------|-------------------|
| Cubemap file not found | Return `Err` with descriptive message including file path |
| Invalid image format | Return `Err` from image decoding library |
| GPU memory allocation failure | Return `Err` from allocator |
| Vulkan image creation failure | Return `Err` with Vulkan error code |

The `load_texture` function returns `Result<Texture, Box<dyn std::error::Error>>`, propagating errors to the caller for graceful handling.

## Testing Strategy

### Unit Tests

Unit tests verify specific examples and edge cases:

1. **Texture Loading Test**: Verify cubemap.png loads successfully and returns valid Texture struct
2. **Format Test**: Verify R8G8B8A8_SRGB format is used for sRGB cubemap
3. **Depth Test**: Verify background renders at depth 0.9999
4. **Descriptor Binding Test**: Verify cubemap bound to set 0, binding 0
5. **Sampler Config Test**: Verify linear filtering is enabled
6. **Error Handling Test**: Verify descriptive error for missing file

### Property-Based Tests

Property-based tests verify universal properties across many generated inputs using a PBT library (e.g., `proptest` for Rust):

1. **Equirectangular UV Range Property**: Generate random normalized 3D directions, verify UV output is always in [0, 1]
2. **Equirectangular Continuity Property**: Generate pairs of nearby directions, verify UV outputs are also nearby
3. **Equirectangular Cardinal Property**: Test known cardinal directions produce expected UV coordinates

Each property test should run minimum 100 iterations to ensure coverage.

**Test Annotation Format:**
```rust
// Feature: cubemap-implementation, Property 1: Equirectangular Projection Bijectivity
// Validates: Requirements 2.1
```
