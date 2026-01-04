# Mesh Shader Demo

A Vulkan demo showcasing **mesh shaders** (`VK_EXT_mesh_shader`), the modern replacement for the traditional vertex/geometry shader pipeline.

## What are Mesh Shaders?

Mesh shaders introduce a new programmable geometry pipeline:

```
Traditional:  Input Assembly → Vertex Shader → [Geometry Shader] → Rasterizer
Mesh Shader:  Task Shader (optional) → Mesh Shader → Rasterizer
```

### Key Benefits
- **Flexible geometry generation** - Generate vertices/primitives procedurally
- **Meshlet-based rendering** - Natural fit for GPU-driven rendering
- **Compute-like programming** - Workgroups, shared memory, barriers
- **Amplification** - Task shaders can spawn variable mesh shader workgroups

## Project Structure

```
mesh_shaders/
├── src/
│   ├── main.rs              # Vulkan app with mesh shader pipeline
│   └── shaders/
│       ├── task.glsl        # Task shader (amplification stage)
│       ├── mesh.glsl        # Mesh shader (geometry generation)
│       └── frag.glsl        # Fragment shader
├── build.rs                 # Shader compilation script
└── Cargo.toml
```

## Requirements

- **GPU with mesh shader support** (NVIDIA Turing+, AMD RDNA2+, Intel Arc)
- **Vulkan SDK** with `glslc` compiler in PATH
- **Vulkan 1.3** runtime

## Building

```bash
# Compile shaders and build
cargo build --release

# Run
cargo run --release
```

## How It Works

1. **Task Shader** (`task.glsl`)
   - Runs once per dispatch
   - Decides how many meshlets to render
   - Passes payload data to mesh shaders
   - Calls `EmitMeshTasksEXT()` to spawn mesh shader workgroups

2. **Mesh Shader** (`mesh.glsl`)
   - One workgroup per meshlet
   - Generates cube geometry procedurally
   - Outputs vertices via `gl_MeshVerticesEXT`
   - Outputs primitives via `gl_PrimitiveTriangleIndicesEXT`

3. **Fragment Shader** (`frag.glsl`)
   - Standard fragment processing
   - Receives interpolated colors from mesh shader

## Demo

The demo renders 8 animated cubes arranged in a ring, demonstrating:
- Task shader workgroup amplification
- Procedural geometry generation in mesh shaders
- Push constant animation
- Per-meshlet coloring
