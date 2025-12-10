use std::time::Duration;

use crate::algorithms::merge::{merge_sort_actions, parallel_merge_sort_actions};
use rand::{rngs::SmallRng, Rng, SeedableRng};

/// Execution mode for sorting visualization
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortMode {
    Sequential,
    Parallel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionKind {
    Compare,
    Swap,
    Write,       // Write from temp to main array
    TempPush,    // Push element to temp array
    TempClear,   // Clear temp array (merge complete)
    MergePhase,  // Signal new merge phase (merge_level stored in value)
    Done,
}

#[derive(Clone, Copy, Debug)]
pub struct Action {
    pub kind: ActionKind,
    pub i: usize,        // Main array index (target for Write, source for TempPush)
    pub j: usize,        // Secondary index
    pub value: u32,
    pub memory: usize,
    pub temp_idx: usize, // Index in temp array (for TempPush/Write)
    pub thread_id: usize, // Thread ID for parallel visualization (0-7)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarState {
    Idle,      // 0
    Compare,   // 1
    Swap,      // 2
    Sorted,    // 3
    Source,    // 4
    TempArray, // 5
    // Thread-specific states for parallel visualization (6-13)
    Thread0,   // 6
    Thread1,   // 7
    Thread2,   // 8
    Thread3,   // 9
    Thread4,   // 10
    Thread5,   // 11
    Thread6,   // 12
    Thread7,   // 13
}

impl BarState {
    pub fn from_thread_id(thread_id: usize) -> Self {
        match thread_id {
            0 => BarState::Thread0,
            1 => BarState::Thread1,
            2 => BarState::Thread2,
            3 => BarState::Thread3,
            4 => BarState::Thread4,
            5 => BarState::Thread5,
            6 => BarState::Thread6,
            7 => BarState::Thread7,
            _ => BarState::Thread0,
        }
    }
    
    pub fn temp_array_for_thread(thread_id: usize) -> u32 {
        // Return state values 14-21 for thread temp arrays
        14 + (thread_id as u32).min(7)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Bar {
    pub value: u32,
    pub state: BarState,
}

/// Info about the current animation for the renderer
#[derive(Clone, Debug, Default)]
pub struct AnimationInfo {
    pub active: bool,
    pub source_idx: usize,
    pub target_idx: usize,
    pub source_height: f32,  // Normalized height of source bar
    pub is_temp_push: bool,  // True if pushing to temp array
    pub temp_target_idx: usize, // Target index in temp array
    pub thread_id: usize,    // Thread ID for coloring
}

/// Temp array state for visualization (per thread)
#[derive(Clone, Debug, Default)]
pub struct TempArrayState {
    pub values: Vec<u32>,    // Current values in temp array
    pub left_bound: usize,   // Left boundary of merge region
    pub right_bound: usize,  // Right boundary of merge region
}

/// Multi-thread temp arrays state
#[derive(Clone, Debug, Default)]
pub struct MultiTempArrayState {
    pub arrays: Vec<TempArrayState>, // One per thread
}

impl MultiTempArrayState {
    pub fn new(num_threads: usize) -> Self {
        Self {
            arrays: (0..num_threads).map(|_| TempArrayState::default()).collect(),
        }
    }
    
    pub fn clear_all(&mut self) {
        for arr in &mut self.arrays {
            arr.values.clear();
        }
    }
    
    /// Calculate total memory usage across all temp arrays (in bytes)
    pub fn total_memory(&self) -> usize {
        self.arrays.iter().map(|arr| arr.values.len() * 4).sum()
    }
}

pub struct Engine {
    bars: Vec<Bar>,
    actions: Vec<Action>,
    cursor: usize,
    rng: SmallRng,
    max_value: u32,
    pub comparisons: usize,
    pub operations: usize, // Swaps or Writes
    pub current_memory: usize,
    pub peak_memory: usize,
    pub time_elapsed: Duration,
    step_timer: f32,
    step_delay: f32,
    pub current_animation: AnimationInfo,
    pub temp_array: TempArrayState,        // For sequential mode
    pub multi_temp_arrays: MultiTempArrayState, // For parallel mode
    pub mode: SortMode,
    pub num_threads: usize,
    initial_values: Vec<u32>,  // Store initial values for mode switching
    pub merge_level: usize,    // Current merge phase level (segment size = chunk * 2^merge_level)
}

impl Engine {
    pub fn new(size: usize) -> Self {
        let mut rng = SmallRng::from_entropy();
        let values: Vec<u32> = (0..size).map(|_| rng.gen_range(1..=1000)).collect();
        let max_value = values.iter().copied().max().unwrap_or(1);
        let mode = SortMode::Sequential;
        let num_threads = 8;
        
        let actions = merge_sort_actions(&values);
        let peak_memory = actions.iter().map(|a| a.memory).max().unwrap_or(0);
        let bars = values
            .iter()
            .map(|&v| Bar {
                value: v,
                state: BarState::Idle,
            })
            .collect();

        Self {
            bars,
            actions,
            cursor: 0,
            rng,
            max_value,
            comparisons: 0,
            operations: 0,
            current_memory: 0,
            peak_memory,
            time_elapsed: Duration::ZERO,
            step_timer: 0.0,
            step_delay: 1.0,
            current_animation: AnimationInfo::default(),
            temp_array: TempArrayState::default(),
            multi_temp_arrays: MultiTempArrayState::new(num_threads),
            mode,
            num_threads,
            initial_values: values,
            merge_level: 0,
        }
    }

    pub fn set_mode(&mut self, mode: SortMode) {
        if self.mode != mode {
            self.mode = mode;
            self.regenerate_actions();
        }
    }

    fn regenerate_actions(&mut self) {
        self.cursor = 0;
        self.comparisons = 0;
        self.operations = 0;
        self.current_memory = 0;
        self.time_elapsed = Duration::ZERO;
        self.step_timer = 0.0;
        self.current_animation = AnimationInfo::default();
        self.temp_array = TempArrayState::default();
        self.multi_temp_arrays = MultiTempArrayState::new(self.num_threads);
        self.merge_level = 0;
        
        // Restore bars to initial values
        for (bar, &val) in self.bars.iter_mut().zip(self.initial_values.iter()) {
            bar.value = val;
            bar.state = BarState::Idle;
        }
        
        // Generate actions based on mode
        self.actions = match self.mode {
            SortMode::Sequential => merge_sort_actions(&self.initial_values),
            SortMode::Parallel => parallel_merge_sort_actions(&self.initial_values, self.num_threads),
        };
        self.peak_memory = self.actions.iter().map(|a| a.memory).max().unwrap_or(0);
    }

    pub fn reset(&mut self) {
        let size = self.bars.len();
        let values: Vec<u32> = (0..size).map(|_| self.rng.gen_range(1..=1000)).collect();
        self.max_value = values.iter().copied().max().unwrap_or(1);
        self.initial_values = values.clone();
        
        // Generate actions based on current mode
        self.actions = match self.mode {
            SortMode::Sequential => merge_sort_actions(&values),
            SortMode::Parallel => parallel_merge_sort_actions(&values, self.num_threads),
        };
        self.peak_memory = self.actions.iter().map(|a| a.memory).max().unwrap_or(0);
        self.cursor = 0;
        self.comparisons = 0;
        self.operations = 0;
        self.current_memory = 0;
        self.time_elapsed = Duration::ZERO;
        self.step_timer = 0.0;
        self.current_animation = AnimationInfo::default();
        self.temp_array = TempArrayState::default();
        self.multi_temp_arrays = MultiTempArrayState::new(self.num_threads);
        
        for (bar, val) in self.bars.iter_mut().zip(values.into_iter()) {
            bar.value = val;
            bar.state = BarState::Idle;
        }
    }

    pub fn bars(&self) -> (&[Bar], u32) {
        (&self.bars, self.max_value)
    }

    pub fn step(&mut self, dt: Duration) {
        if self.cursor >= self.actions.len() {
            // Mark sorted once done
            for bar in &mut self.bars {
                bar.state = BarState::Sorted;
            }
            return;
        }

        self.time_elapsed += dt;
        self.step_timer += dt.as_secs_f32();

        if self.step_timer < self.step_delay {
            return;
        }
        self.step_timer = 0.0;

        // Clear transient states
        for bar in &mut self.bars {
            if bar.state != BarState::Sorted {
                bar.state = BarState::Idle;
            }
        }

        // Process exactly one action
        if self.cursor < self.actions.len() {
            let action = self.actions[self.cursor];
            let thread_id = action.thread_id;
            
            // Update current memory usage
            // In parallel mode, compute from actual temp arrays; in sequential mode, use action.memory
            if self.mode == SortMode::Parallel {
                self.current_memory = self.multi_temp_arrays.total_memory();
            } else {
                self.current_memory = action.memory;
            }
            
            match action.kind {
                ActionKind::Compare => {
                    self.comparisons += 1;
                    self.current_animation.active = false;
                    let state = if self.mode == SortMode::Parallel {
                        BarState::from_thread_id(thread_id)
                    } else {
                        BarState::Compare
                    };
                    self.mark(action.i, state);
                    self.mark(action.j, state);
                }
                ActionKind::Swap => {
                    self.operations += 1;
                    self.current_animation.active = false;
                    self.bars.swap(action.i, action.j);
                    let state = if self.mode == SortMode::Parallel {
                        BarState::from_thread_id(thread_id)
                    } else {
                        BarState::Swap
                    };
                    self.mark(action.i, state);
                    self.mark(action.j, state);
                }
                ActionKind::TempPush => {
                    // Element is being added to temp array
                    let source_height = action.value as f32 / self.max_value as f32;
                    self.current_animation = AnimationInfo {
                        active: true,
                        source_idx: action.i,  // Source in main array
                        target_idx: action.temp_idx,
                        source_height,
                        is_temp_push: true,
                        temp_target_idx: action.temp_idx,
                        thread_id,
                    };
                    
                    // Add value to appropriate temp array
                    if self.mode == SortMode::Parallel {
                        if let Some(arr) = self.multi_temp_arrays.arrays.get_mut(thread_id) {
                            arr.values.push(action.value);
                        }
                        // Update current and peak memory from actual temp arrays
                        self.current_memory = self.multi_temp_arrays.total_memory();
                        if self.current_memory > self.peak_memory {
                            self.peak_memory = self.current_memory;
                        }
                    } else {
                        self.temp_array.values.push(action.value);
                    }
                    
                    // Mark source bar
                    let state = if self.mode == SortMode::Parallel {
                        BarState::from_thread_id(thread_id)
                    } else {
                        BarState::Source
                    };
                    self.mark(action.i, state);
                }
                ActionKind::Write => {
                    self.operations += 1;
                    let source_height = action.value as f32 / self.max_value as f32;
                    self.current_animation = AnimationInfo {
                        active: true,
                        source_idx: action.temp_idx,
                        target_idx: action.i,
                        source_height,
                        is_temp_push: false,
                        temp_target_idx: action.temp_idx,
                        thread_id,
                    };
                    
                    // Remove from appropriate temp array (first element)
                    if self.mode == SortMode::Parallel {
                        if let Some(arr) = self.multi_temp_arrays.arrays.get_mut(thread_id) {
                            if !arr.values.is_empty() {
                                arr.values.remove(0);
                            }
                        }
                    } else if !self.temp_array.values.is_empty() {
                        self.temp_array.values.remove(0);
                    }
                    
                    // Mark and update target bar
                    if let Some(bar) = self.bars.get_mut(action.i) {
                        bar.value = action.value;
                        bar.state = if self.mode == SortMode::Parallel {
                            BarState::from_thread_id(thread_id)
                        } else {
                            BarState::Swap
                        };
                    }
                }
                ActionKind::TempClear => {
                    // Merge complete, clear temp array for this thread
                    self.current_animation.active = false;
                    if self.mode == SortMode::Parallel {
                        if let Some(arr) = self.multi_temp_arrays.arrays.get_mut(thread_id) {
                            arr.values.clear();
                        }
                    } else {
                        self.temp_array.values.clear();
                    }
                }
                ActionKind::MergePhase => {
                    // Transition to new merge phase - value contains the merge level
                    self.merge_level = action.value as usize;
                    self.current_animation.active = false;
                }
                ActionKind::Done => {
                    self.current_memory = 0;
                    self.current_animation.active = false;
                    self.temp_array.values.clear();
                    self.multi_temp_arrays.clear_all();
                    for bar in &mut self.bars {
                        bar.state = BarState::Sorted;
                    }
                }
            }
            self.cursor += 1;
        }
    }

    fn mark(&mut self, idx: usize, state: BarState) {
        if let Some(bar) = self.bars.get_mut(idx) {
            if bar.state != BarState::Sorted {
                bar.state = state;
            }
        }
    }
}
