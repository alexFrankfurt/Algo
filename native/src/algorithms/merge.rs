use crate::engine::{Action, ActionKind};

/// Size of u32 in bytes for memory tracking
const ELEMENT_SIZE: usize = 4;

pub fn merge_sort_actions(values: &[u32]) -> Vec<Action> {
    let mut actions = Vec::new();
    let mut arr = values.to_vec();
    let n = arr.len();
    let mut current_memory: usize = 0;
    merge_sort_recursive(&mut arr, 0, n, &mut actions, &mut current_memory);
    
    // Mark all as done
    actions.push(Action {
        kind: ActionKind::Done,
        i: 0,
        j: 0,
        value: 0,
        memory: 0,
        temp_idx: 0,
    });
    
    actions
}

fn merge_sort_recursive(arr: &mut [u32], left: usize, right: usize, actions: &mut Vec<Action>, current_memory: &mut usize) {
    if right - left <= 1 {
        return;
    }

    let mid = left + (right - left) / 2;
    merge_sort_recursive(arr, left, mid, actions, current_memory);
    merge_sort_recursive(arr, mid, right, actions, current_memory);
    merge(arr, left, mid, right, actions, current_memory);
}

fn merge(arr: &mut [u32], left: usize, mid: usize, right: usize, actions: &mut Vec<Action>, current_memory: &mut usize) {
    let temp_size = right - left;
    let temp_bytes = temp_size * ELEMENT_SIZE;
    
    // Allocate temporary array - add to current memory
    *current_memory += temp_bytes;
    
    // Store (value, original_index) pairs so we can track where values came from
    let mut temp: Vec<(u32, usize)> = Vec::with_capacity(temp_size);
    let mut i = left;
    let mut j = mid;
    let mut temp_idx = 0;

    while i < mid && j < right {
        actions.push(Action {
            kind: ActionKind::Compare,
            i,
            j,
            value: 0,
            memory: *current_memory,
            temp_idx: 0,
        });
        
        if arr[i] <= arr[j] {
            // Push to temp array with animation
            actions.push(Action {
                kind: ActionKind::TempPush,
                i,  // Source index in main array
                j: 0,
                value: arr[i],
                memory: *current_memory,
                temp_idx,
            });
            temp.push((arr[i], i));
            temp_idx += 1;
            i += 1;
        } else {
            // Push to temp array with animation
            actions.push(Action {
                kind: ActionKind::TempPush,
                i: j,  // Source index in main array
                j: 0,
                value: arr[j],
                memory: *current_memory,
                temp_idx,
            });
            temp.push((arr[j], j));
            temp_idx += 1;
            j += 1;
        }
    }

    while i < mid {
        actions.push(Action {
            kind: ActionKind::TempPush,
            i,
            j: 0,
            value: arr[i],
            memory: *current_memory,
            temp_idx,
        });
        temp.push((arr[i], i));
        temp_idx += 1;
        i += 1;
    }

    while j < right {
        actions.push(Action {
            kind: ActionKind::TempPush,
            i: j,
            j: 0,
            value: arr[j],
            memory: *current_memory,
            temp_idx,
        });
        temp.push((arr[j], j));
        temp_idx += 1;
        j += 1;
    }

    for (k, &(val, _source_idx)) in temp.iter().enumerate() {
        let target_idx = left + k;
        arr[target_idx] = val;
        // Write from temp array to main array
        actions.push(Action {
            kind: ActionKind::Write,
            i: target_idx,  // Target in main array
            j: 0,
            value: val,
            memory: *current_memory,
            temp_idx: 0,  // Always read from front of temp (FIFO)
        });
    }
    
    // Clear temp array after merge
    actions.push(Action {
        kind: ActionKind::TempClear,
        i: 0,
        j: 0,
        value: 0,
        memory: *current_memory,
        temp_idx: 0,
    });
    
    // Free temporary array - subtract from current memory
    *current_memory -= temp_bytes;
}
