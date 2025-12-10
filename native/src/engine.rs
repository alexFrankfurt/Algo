
use std::time::Duration;

use crate::algorithms::merge::merge_sort_actions;
use rand::{rngs::SmallRng, Rng, SeedableRng};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionKind {
    Compare,
    Swap,
    Write,       // Write from temp to main array
    TempPush,    // Push element to temp array
    TempClear,   // Clear temp array (merge complete)
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarState {
    Idle,
    Compare,
    Swap,
    Source,  // Bar providing a value (highlighted during write)
    Sorted,
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
}

/// Temp array state for visualization
#[derive(Clone, Debug, Default)]
pub struct TempArrayState {
    pub values: Vec<u32>,    // Current values in temp array
    pub left_bound: usize,   // Left boundary of merge region
    pub right_bound: usize,  // Right boundary of merge region
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
    pub temp_array: TempArrayState,
}

impl Engine {
    pub fn new(size: usize) -> Self {
        let mut rng = SmallRng::from_entropy();
        let values: Vec<u32> = (0..size).map(|_| rng.gen_range(1..=1000)).collect();
        let max_value = values.iter().copied().max().unwrap_or(1);
        // Default to Merge Sort as requested
        let actions = merge_sort_actions(&values);
        let peak_memory = actions.iter().map(|a| a.memory).max().unwrap_or(0);
        let bars = values
            .into_iter()
            .map(|v| Bar {
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
            step_delay: 0.1, // 1 second delay per step
            current_animation: AnimationInfo::default(),
            temp_array: TempArrayState::default(),
        }
    }

    pub fn reset(&mut self) {
        let size = self.bars.len();
        let mut values: Vec<u32> = (0..size).map(|_| self.rng.gen_range(1..=1000)).collect();
        self.max_value = values.iter().copied().max().unwrap_or(1);
        // Default to Merge Sort
        self.actions = merge_sort_actions(&values);
        self.peak_memory = self.actions.iter().map(|a| a.memory).max().unwrap_or(0);
        self.cursor = 0;
        self.comparisons = 0;
        self.operations = 0;
        self.current_memory = 0;
        self.time_elapsed = Duration::ZERO;
        self.step_timer = 0.0;
        self.current_animation = AnimationInfo::default();
        self.temp_array = TempArrayState::default();
        for (bar, val) in self.bars.iter_mut().zip(values.drain(..)) {
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
            // Update current memory usage from action
            self.current_memory = action.memory;
            match action.kind {
                ActionKind::Compare => {
                    self.comparisons += 1;
                    self.current_animation.active = false;
                    self.mark(action.i, BarState::Compare);
                    self.mark(action.j, BarState::Compare);
                }
                ActionKind::Swap => {
                    self.operations += 1;
                    self.current_animation.active = false;
                    self.bars.swap(action.i, action.j);
                    self.mark(action.i, BarState::Swap);
                    self.mark(action.j, BarState::Swap);
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
                    };
                    // Add value to temp array visualization
                    self.temp_array.values.push(action.value);
                    // Mark source bar
                    self.mark(action.i, BarState::Source);
                }
                ActionKind::Write => {
                    self.operations += 1;
                    // Set up animation info for renderer
                    // The value comes from temp array, so animate a bar flying to target
                    let source_height = action.value as f32 / self.max_value as f32;
                    self.current_animation = AnimationInfo {
                        active: true,  // Always animate writes
                        source_idx: action.temp_idx,  // Source is temp array index
                        target_idx: action.i,
                        source_height,
                        is_temp_push: false,
                        temp_target_idx: action.temp_idx,
                    };
                    // Remove from temp array visualization (first element)
                    if !self.temp_array.values.is_empty() {
                        self.temp_array.values.remove(0);
                    }
                    // Mark and update target bar (where value goes)
                    if let Some(bar) = self.bars.get_mut(action.i) {
                        bar.value = action.value;
                        bar.state = BarState::Swap;
                    }
                }
                ActionKind::TempClear => {
                    // Merge complete, clear temp array
                    self.current_animation.active = false;
                    self.temp_array.values.clear();
                }
                ActionKind::Done => {
                    self.current_memory = 0;
                    self.current_animation.active = false;
                    self.temp_array.values.clear();
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
