// D:\Dev\Algo\src\main.js

import { Canvas2DRenderer } from './renderers/canvas.js';
import { WebGLRenderer } from './renderers/webgl.js';
import { AlgorithmEngine } from './engine/engine.js';
import { BubbleSort } from './algorithms/bubbleSort.js';
import { SelectionSort } from './algorithms/selectionSort.js';

const algorithmSelect = document.getElementById('algorithm-select');
const arraySizeSlider = document.getElementById('array-size-slider');
const arraySizeLabel = document.getElementById('array-size-label');
const speedSlider = document.getElementById('speed-slider');
const speedLabel = document.getElementById('speed-label');
const generateArrayBtn = document.getElementById('generate-array-btn');
const startBtn = document.getElementById('start-btn');
const pauseBtn = document.getElementById('pause-btn');
const resetBtn = document.getElementById('reset-btn');
const rendererSelect = document.getElementById('renderer-select');
const visualizationContainer = document.getElementById('visualization-container');

let currentRenderer = null;
let algorithmEngine = null;
let currentArray = [];
let arraySize = parseInt(arraySizeSlider.value);
let animationSpeed = parseInt(speedSlider.value);

const algorithms = {
    bubbleSort: BubbleSort,
    selectionSort: SelectionSort,
    // Add more algorithms here
};

// Initialize algorithm options
for (const key in algorithms) {
    if (!algorithmSelect.querySelector(`option[value="${key}"]`)) {
        const option = document.createElement('option');
        option.value = key;
        option.textContent = algorithms[key].name; // Assuming algorithm classes have a static name property or similar
        algorithmSelect.appendChild(option);
    }
}

function generateRandomArray(size) {
    return Array.from({ length: size }, (_, i) => i + 1).sort(() => Math.random() - 0.5);
}

function initializeRenderer(rendererType) {
    if (currentRenderer) {
        currentRenderer.destroy();
        visualizationContainer.innerHTML = ''; // Clear previous canvas
    }

    const canvas = document.createElement('canvas');
    visualizationContainer.appendChild(canvas);

    if (rendererType === 'canvas2d') {
        currentRenderer = new Canvas2DRenderer(canvas);
    } else if (rendererType === 'webgl') {
        currentRenderer = new WebGLRenderer(canvas);
    }
    // Add other renderers here as they are implemented

    if (currentRenderer) {
        currentRenderer.initialize(arraySize);
        currentRenderer.render(currentArray, [], []); // Initial render
    }
}

function initializeApplication() {
    currentArray = generateRandomArray(arraySize);
    arraySizeLabel.textContent = `Array Size: ${arraySize}`;
    speedLabel.textContent = `Speed: ${animationSpeed}`;

    initializeRenderer(rendererSelect.value);

    // Initialize the algorithm engine
    algorithmEngine = new AlgorithmEngine(currentRenderer, animationSpeed);
    algorithmEngine.loadAlgorithm(new algorithms[algorithmSelect.value]());
    algorithmEngine.setArray(currentArray);
}

// Event Listeners
algorithmSelect.addEventListener('change', () => {
    algorithmEngine.loadAlgorithm(new algorithms[algorithmSelect.value]());
    algorithmEngine.setArray(currentArray); // Reset algorithm with current array
    algorithmEngine.reset();
    currentRenderer.render(currentArray, [], []); // Render initial state
});

arraySizeSlider.addEventListener('input', (event) => {
    arraySize = parseInt(event.target.value);
    arraySizeLabel.textContent = `Array Size: ${arraySize}`;
    currentArray = generateRandomArray(arraySize);
    algorithmEngine.setArray(currentArray); // Update engine with new array
    algorithmEngine.loadAlgorithm(new algorithms[algorithmSelect.value]()); // Re-load algorithm to clear steps
    algorithmEngine.reset();
    initializeRenderer(rendererSelect.value); // Re-initialize renderer with new array size
});

speedSlider.addEventListener('input', (event) => {
    animationSpeed = parseInt(event.target.value);
    speedLabel.textContent = `Speed: ${animationSpeed}`;
    algorithmEngine.setSpeed(animationSpeed);
});

generateArrayBtn.addEventListener('click', () => {
    currentArray = generateRandomArray(arraySize);
    algorithmEngine.setArray(currentArray);
    algorithmEngine.loadAlgorithm(new algorithms[algorithmSelect.value]()); // Re-load algorithm to clear steps
    algorithmEngine.reset();
    currentRenderer.render(currentArray, [], []); // Render initial state
});

startBtn.addEventListener('click', () => {
    algorithmEngine.start();
});

pauseBtn.addEventListener('click', () => {
    algorithmEngine.pause();
});

resetBtn.addEventListener('click', () => {
    algorithmEngine.reset();
    currentArray = generateRandomArray(arraySize); // Generate a new array on reset
    algorithmEngine.setArray(currentArray);
    algorithmEngine.loadAlgorithm(new algorithms[algorithmSelect.value]()); // Re-load algorithm to clear steps
    currentRenderer.render(currentArray, [], []); // Render initial state
});

rendererSelect.addEventListener('change', (event) => {
    initializeRenderer(event.target.value);
    algorithmEngine.setRenderer(currentRenderer); // Update engine with new renderer
    currentRenderer.render(currentArray, [], []); // Render initial state
});

// Initial application setup
initializeApplication();

// Handle window resizing
window.addEventListener('resize', () => {
    if (currentRenderer) {
        currentRenderer.resize();
        currentRenderer.render(currentArray, algorithmEngine.getComparedIndices(), algorithmEngine.getSwappedIndices());
    }
});
