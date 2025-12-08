use crate::algorithms::bubble::bubble_sort_actions;
use rand::{rngs::SmallRng, Rng, SeedableRng};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionKind {
    Compare,
    Swap,
    Write,
    Done,
}

#[derive(Clone, Copy, Debug)]
pub struct Action {
    pub kind: ActionKind,
    pub i: usize,
    pub j: usize,
    pub value: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarState {
    Idle,
    Compare,
    Swap,
    Sorted,
}

#[derive(Clone, Copy, Debug)]
pub struct Bar {
    pub value: u32,
    pub state: BarState,
}

pub struct Engine {
    bars: Vec<Bar>,
    actions: Vec<Action>,
    cursor: usize,
    rng: SmallRng,
    max_value: u32,
}

impl Engine {
    pub fn new(size: usize) -> Self {
        let mut rng = SmallRng::from_entropy();
        let values: Vec<u32> = (0..size).map(|_| rng.gen_range(1..=1000)).collect();
        let max_value = values.iter().copied().max().unwrap_or(1);
        let actions = bubble_sort_actions(&values);
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
        }
    }

    pub fn reset(&mut self) {
        let size = self.bars.len();
        let mut values: Vec<u32> = (0..size).map(|_| self.rng.gen_range(1..=1000)).collect();
        self.max_value = values.iter().copied().max().unwrap_or(1);
        self.actions = bubble_sort_actions(&values);
        self.cursor = 0;
        for (bar, val) in self.bars.iter_mut().zip(values.drain(..)) {
            bar.value = val;
            bar.state = BarState::Idle;
        }
    }

    pub fn bars(&self) -> (&[Bar], u32) {
        (&self.bars, self.max_value)
    }

    pub fn step(&mut self) {
        if self.cursor >= self.actions.len() {
            // Mark sorted once done
            for bar in &mut self.bars {
                bar.state = BarState::Sorted;
            }
            return;
        }

        // Clear transient states
        for bar in &mut self.bars {
            if bar.state != BarState::Sorted {
                bar.state = BarState::Idle;
            }
        }

        // Apply a small batch per frame to keep things moving
        let batch = 8usize.min(self.actions.len() - self.cursor);
        let end = self.cursor + batch;
        for idx in self.cursor..end {
            let action = self.actions[idx];
            match action.kind {
                ActionKind::Compare => {
                    self.mark(action.i, BarState::Compare);
                    self.mark(action.j, BarState::Compare);
                }
                ActionKind::Swap => {
                    self.bars.swap(action.i, action.j);
                    self.mark(action.i, BarState::Swap);
                    self.mark(action.j, BarState::Swap);
                }
                ActionKind::Write => {
                    if let Some(bar) = self.bars.get_mut(action.i) {
                        bar.value = action.value;
                        bar.state = BarState::Swap;
                    }
                }
                ActionKind::Done => {
                    for bar in &mut self.bars {
                        bar.state = BarState::Sorted;
                    }
                }
            }
        }
        self.cursor = end;
    }

    fn mark(&mut self, idx: usize, state: BarState) {
        if let Some(bar) = self.bars.get_mut(idx) {
            if bar.state != BarState::Sorted {
                bar.state = state;
            }
        }
    }
}
