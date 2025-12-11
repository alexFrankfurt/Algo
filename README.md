# Algo

**Algo** is a project dedicated to visualizing sorting algorithms. It features both a web-based implementation and a high-performance native application using Rust and `wgpu`.

## Native Application (`native/`)

The native version is a high-fidelity 3D visualizer built with Rust. It leverages `wgpu` for cross-platform GPU rendering.

### Features
- **High-Performance Rendering**: Uses `wgpu` for efficient graphics API access (Vulkan, Metal, DX12, WebGPU).
- **Visual Effects**: Includes HDR (High Dynamic Range) rendering and Bloom for a polished aesthetic.
- **Interactive**: Real-time control over the visualization.

### Getting Started

1. Ensure you have [Rust and Cargo](https://rustup.rs/) installed.
2. Navigate to the native directory:
   ```bash
   cd native
   ```
3. Run the application:
   ```bash
   cargo run --release
   ```


## Video demonstration

[watch](https://www.youtube.com/watch?v=vQolPP4WW6A)

# Algo

Algo is a collection of visualizers for sorting algorithms — a lightweight web demo plus two native Rust renderers that showcase GPU-based rendering and shader-driven effects.

**Repository highlights**
- Web UI: simple browser demo (`index.html`, `public/`, `src/main.js`).
- Native Rust visualizer: `native/` (Rust + `wgpu`) with 3D rendering, post-processing, and sorting algorithm implementations.
- Vulkan-style visualizer: `vulcan_viz/` (separate Rust example with dedicated shaders under `shaders/` and `shader.wgsl`).

Project structure (important folders)
- `index.html` / `public/` — Web front-end and static assets.
- `src/` — JavaScript UI and logic for the web demo.
- `native/` — Rust native app using `wgpu`.
   - `native/src/main.rs` — application entry.
   - `native/src/engine.rs`, `native/src/renderer.rs` — core render pipeline and scene logic.
   - `native/src/bar.wgsl`, `native/src/floor.wgsl`, `native/src/post.wgsl` — WGSL shaders used by the native visualizer.
   - `native/src/algorithms/` — sorting algorithm implementations (e.g. `bubble.rs`, `merge.rs`, `mod.rs`).
- `vulcan_viz/` — an alternate Rust renderer and shader examples.
   - `vulcan_viz/src/main.rs` — entrypoint for the vulcan visualizer.
   - `vulcan_viz/shaders/` — GLSL shaders used by the example renderer.

Implemented algorithms
- Bubble Sort (`native/src/algorithms/bubble.rs`)
- Merge Sort (`native/src/algorithms/merge.rs`)

Build & run

- Web demo (quick): open `index.html` in a modern browser, or serve `public/` with a static server:
   ```powershell
   # from repository root
   python -m http.server 8000 --directory public
   # then open http://localhost:8000
   ```

- Native visualizer (`native/`): requires Rust toolchain. From repository root:
   ```powershell
   cd native
   cargo run --release
   ```

- Vulkan example (`vulcan_viz/`): also a Rust project — build/run similarly:
   ```powershell
   cd vulcan_viz
   cargo run --release
   ```

Notes
- Shaders: native uses WGSL files in `native/src/` (bar, floor, post). `vulcan_viz/` contains GLSL shader examples.
- Adding algorithms: new algorithm modules go in `native/src/algorithms/` and should be wired into the engine via `mod.rs`.
- Assets: `native/assets/` holds runtime assets used by the native app.

If you'd like, I can add quick HOWTO sections for adding a new algorithm module or for configuring the renderer (e.g., toggling bloom/HDR). 
