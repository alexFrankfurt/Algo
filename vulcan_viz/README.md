# Vulcan Viz - QuickSort Visualization

A real-time ray-traced visualization of the QuickSort algorithm using Vulkan Ray Tracing.

## Features

- **Ray-traced rendering**: Beautiful glass-like bars with realistic reflections and refractions
- **QuickSort algorithm**: Step-by-step visualization of the Lomuto partition scheme
- **Animated sorting**: Watch as the algorithm partitions and sorts the array
- **Cyberpunk aesthetics**: Glowing grid floor and studio lighting
- **Automatic restart**: Continuously shuffles and re-sorts for endless visualization

## Algorithm Details

The implementation uses the **Lomuto partition scheme** for QuickSort:

1. **Partitioning**: Selects the last element as pivot and partitions the array
2. **Recursive sorting**: Uses a stack to manage sub-arrays iteratively
3. **Visual feedback**: Each step is animated with deliberate delays for educational clarity

## Timing & Pacing

The visualization is extremely slow-paced for maximum educational clarity:
- **Algorithm Steps**: ~0.5 seconds between each comparison/swap operation
- **Swap Animation**: Each swap takes ~8.3 seconds to complete (125 frames at 60fps)
- **Swap Highlighting**: Visual effects last 5+ seconds to clearly show what changed
- **Pivot Placement**: Extra emphasis (6.7 seconds) when pivot finds its final position
- **Completion Pause**: 5-second pause to appreciate the fully sorted result before reshuffling

## Controls

- The visualization runs automatically
- Close the window to exit
- Window can be resized

## Technical Implementation

- **Language**: Rust
- **Graphics API**: Vulkan with Ray Tracing extensions
- **Shaders**: GLSL ray generation, closest hit, and miss shaders
- **Rendering**: Real-time ray tracing with acceleration structures

## Building and Running

```bash
cd vulcan_viz
cargo run
```

**Requirements:**
- Rust toolchain
- Vulkan-compatible GPU with ray tracing support
- Updated graphics drivers

## Visualization Elements

- **Bars**: Glass-like cubes with height representing array values
- **Base Colors**: Cyan to purple to pink gradient based on values
- **Floor**: Reflective surface with animated grid pattern
- **Lighting**: Studio-style softbox lighting from above
- **Effects**: Chromatic dispersion, fresnel reflections, and iridescence

## Visual Algorithm Cues

The visualization includes dynamic visual indicators to show algorithm progress:

### Floating Arrow Markers (Above the Bars):
These bright, glowing markers float above the bars to clearly indicate pointer positions:

- **🟢 GREEN Arrow**: Pointer `i` - The partition boundary marker
- **🔵 CYAN Arrow**: Pointer `j` - The scanner/comparison marker  
- **🟡 GOLD Arrow (Larger)**: Pivot element marker

### Bar Color Indicators:
- **🟡 Pivot Element**: Bright gold/yellow with strong pulsing - shows the current pivot being used for partitioning
- **🟢 Pointer `i` (GREEN)**: The partition boundary - everything before this is smaller than the pivot
- **🔵 Pointer `j` (CYAN)**: The scanner - currently examining this element and comparing it to the pivot
- **🟡 Both Pointers (YELLOW-GREEN)**: When `i` and `j` point to the same element

### Action Indicators:
- **⚪ Comparing Elements**: Bright white/blue pulsing - highlights elements currently being compared to the pivot
- **🟣 Swapping Animation**: Bright magenta bars that move through a "swap zone" above the array - shows the actual swapping process in real-time
- **🔴 Recently Swapped**: Bright red/orange with intense glow - shows elements that were just swapped (after animation completes)

### Context Indicators:
- **✨ Active Range**: Slightly brighter base colors - indicates the current sub-array being processed
- **🔵 Normal Elements**: Standard gradient colors - elements not currently involved in the algorithm

## Understanding the Two-Pointer Mechanism

QuickSort uses two pointers (`i` and `j`) during partitioning. For a detailed explanation of how they work and when swaps occur, see **[TWO_POINTER_GUIDE.md](TWO_POINTER_GUIDE.md)**.

**Quick Summary:**
- **`i` (GREEN)**: Marks the boundary between smaller and larger elements
- **`j` (CYAN)**: Scans through the array examining each element
- **Swap occurs when**: `j` finds an element smaller than the pivot AND `i ≠ j`

## Swap Animation System

When elements need to be swapped, they don't just instantly change positions. Instead, they perform a detailed 4-phase animation:

1. **🔺 Rise Phase (30%)**: Both bars slowly rise up to a "swap zone" 12 units above the main array
2. **⏸️ Pause Phase (20%)**: Bars pause at the top, clearly visible in the swap zone
3. **↔️ Move Phase (30%)**: Bars move horizontally to each other's target positions while staying elevated
4. **🔻 Descend Phase (20%)**: Bars slowly descend to their new positions in the main array
5. **✨ Highlight Phase**: The swapped bars glow red/orange to confirm the swap completed

**Animation Duration**: Each swap takes about 8.3 seconds (125 frames at 60fps) to complete, making every movement extremely clear and easy to follow.

This makes it crystal clear which elements are being swapped and helps users understand the partitioning process step by step.

These visual cues make it easy to follow the QuickSort algorithm's divide-and-conquer approach as it partitions the array and recursively sorts sub-arrays.