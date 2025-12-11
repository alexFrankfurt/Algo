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

### Controls
- **Space**: Pause/Resume visualization.
- **R**: Reset the sorting array.

### Implemented Algorithms
- Bubble Sort

## Video demonstration

[watch](https://www.youtube.com/watch?v=vQolPP4WW6A)

## Web Visualization

The project also includes a web-based visualizer located in the root directory, utilizing HTML, CSS, and JavaScript for accessible, browser-based demonstrations.

### Running the Web Version
Open `index.html` in a modern web browser.
