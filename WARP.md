# WARP.md

This file provides guidance to WARP (warp.dev) when working with code in this repository.

## Project Overview

Single-file HTML application that provides interactive visualizations of sorting algorithms. All code (HTML, CSS, and JavaScript) is contained in `index.html`.

**Visualizations included:**
- Selection Sort with step-by-step visual indicator
- Merge Sort v4 (traditional recursive implementation with animated merging)
- Merge Sort v5 (tree visualization showing divide and conquer phases separately)

## Architecture

### Core Components

**Visual Elements:**
- `.box` - Visual bars representing array elements (height = value)
- `.arrow` and `#minIndexMarker` - Selection sort visual indicators
- `#merge-area` - Temporary display area for merge operations
- `.level-container` - Hierarchical tree display for merge sort divide phase

**Animation System:**
- `sleep()` - Respects user-controlled speed slider (100-2000ms)
- `checkPause()` - Allows pausing/resuming during any animation
- Transform-based animations for swapping and flying boxes
- CSS transitions (0.5s ease-in-out) for smooth visual effects

**Algorithm Implementations:**

1. **Selection Sort (`selectionSort()`):**
   - Visual markers show current position and minimum element
   - Comparison highlighting shows algorithmic decisions
   - DOM manipulation after each swap to maintain synchronization

2. **Merge Sort v4 (`mergeSort()`, `merge()`):**
   - Recursive divide phase is implicit
   - Merge phase uses "flying box" animations between containers
   - Clones elements to animate between merge area and main container

3. **Merge Sort v5 (`runMergeSortV5_Divide()`, `runMergeSortV5_Conquer()`):**
   - Two-phase visualization (divide, then conquer)
   - Tree structure (`rootV5`) built with nested containers
   - Each level displayed in separate horizontal row

### Key Technical Details

**DOM Manipulation Pattern:**
- Clone nodes when animating between containers
- Apply CSS transforms for positional animations
- Rebuild container innerHTML after logical changes
- Use `getBoundingClientRect()` to calculate animation distances

**State Management:**
- `boxes` array maintains logical order for selection sort
- `rootV5` tree stores hierarchical structure for merge sort v5
- `isPaused` flag for animation control
- Speed controlled via `#speedSlider` value

**Animation Coordination:**
- All animations use `await sleep()` for timing
- Pause checks inserted after each visual step
- Highlight classes applied/removed to show comparisons
- Fade in/out effects for element transitions

## Development Commands

**Run locally:**
Open `index.html` directly in any browser - no build step required.

**Quick test:**
```pwsh
Start-Process index.html
```

## Code Modification Guidelines

**When adding new sorting algorithms:**
1. Create new section with descriptive heading and algorithm explanation
2. Add new container div with unique ID
3. Implement sorting function following async/await pattern with `sleep()` and `checkPause()`
4. Add control buttons and wire up onclick handlers
5. Use existing CSS classes (`.box`, `.highlight-compare`, `.highlight-winner`) for consistency

**When modifying animations:**
- All timing uses `sleep()` which respects the speed slider
- Transform-based animations should have corresponding CSS transitions
- Always call `await checkPause()` after visual changes to respect pause state
- Use `getBoundingClientRect()` for position calculations, not hardcoded offsets

**When changing visual styles:**
- Dark theme is default (`.dark-theme` on body)
- Box width is fixed at 50px with 5px margins (used in arrow positioning calculations)
- Container height is 200px for main visualizations
- All transitions are 0.5s ease-in-out for consistency
