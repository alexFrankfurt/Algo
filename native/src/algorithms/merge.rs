use crate::engine::{Action, ActionKind};

/// Size of u32 in bytes for memory tracking
const ELEMENT_SIZE: usize = 4;

/// Sequential merge sort - generates actions for single-threaded visualization
pub fn merge_sort_actions(values: &[u32]) -> Vec<Action> {
    let mut actions = Vec::new();
    let mut arr = values.to_vec();
    let n = arr.len();
    let mut current_memory: usize = 0;
    merge_sort_recursive(&mut arr, 0, n, &mut actions, &mut current_memory, 0);
    
    // Mark all as done
    actions.push(Action {
        kind: ActionKind::Done,
        i: 0,
        j: 0,
        value: 0,
        memory: 0,
        temp_idx: 0,
        thread_id: 0,
    });
    
    actions
}

fn merge_sort_recursive(
    arr: &mut [u32], 
    left: usize, 
    right: usize, 
    actions: &mut Vec<Action>, 
    current_memory: &mut usize,
    thread_id: usize,
) {
    if right - left <= 1 {
        return;
    }

    let mid = left + (right - left) / 2;
    merge_sort_recursive(arr, left, mid, actions, current_memory, thread_id);
    merge_sort_recursive(arr, mid, right, actions, current_memory, thread_id);
    merge(arr, left, mid, right, actions, current_memory, thread_id);
}

fn merge(
    arr: &mut [u32], 
    left: usize, 
    mid: usize, 
    right: usize, 
    actions: &mut Vec<Action>, 
    current_memory: &mut usize,
    thread_id: usize,
) {
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
            thread_id,
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
                thread_id,
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
                thread_id,
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
            thread_id,
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
            thread_id,
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
            thread_id,
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
        thread_id,
    });
    
    // Free temporary array - subtract from current memory
    *current_memory -= temp_bytes;
}

/// Parallel merge sort - simulates multi-threaded execution with interleaved actions
/// Each thread processes a portion of the array, then threads merge their results
pub fn parallel_merge_sort_actions(values: &[u32], num_threads: usize) -> Vec<Action> {
    let n = values.len();
    if n == 0 {
        return vec![Action {
            kind: ActionKind::Done,
            i: 0, j: 0, value: 0, memory: 0, temp_idx: 0, thread_id: 0,
        }];
    }
    
    let num_threads = num_threads.min(n).max(1);
    
    // Phase 1: Split array into chunks, each thread sorts its chunk independently
    // We'll generate actions for each thread, then interleave them to simulate parallelism
    let chunk_size = (n + num_threads - 1) / num_threads;
    
    // Generate actions for each thread sorting its chunk
    let mut thread_actions: Vec<Vec<Action>> = Vec::with_capacity(num_threads);
    let mut arr = values.to_vec();
    
    for thread_id in 0..num_threads {
        let start = thread_id * chunk_size;
        let end = (start + chunk_size).min(n);
        
        if start >= n {
            thread_actions.push(Vec::new());
            continue;
        }
        
        let mut actions = Vec::new();
        let mut current_memory = 0usize;
        
        // Sort this chunk
        let mut chunk = arr[start..end].to_vec();
        parallel_merge_sort_chunk(&mut chunk, start, &mut actions, &mut current_memory, thread_id);
        
        // Copy sorted chunk back
        arr[start..end].copy_from_slice(&chunk);
        
        thread_actions.push(actions);
    }
    
    // Interleave actions from all threads to simulate parallel execution
    let mut interleaved = interleave_actions(&thread_actions);
    
    // Phase 2: Merge sorted chunks together (this happens sequentially with fewer threads)
    // Use a tree-based merge pattern
    let mut step = chunk_size;
    let mut merge_level = 1usize; // Level 0 is the initial chunk sorting, level 1+ are merge phases
    
    while step < n {
        // Signal the start of a new merge phase
        interleaved.push(Action {
            kind: ActionKind::MergePhase,
            i: 0,
            j: 0,
            value: merge_level as u32,
            memory: 0,
            temp_idx: 0,
            thread_id: 0,
        });
        
        let mut merge_thread_actions: Vec<Vec<Action>> = Vec::new();
        let mut thread_id = 0;
        
        let mut left = 0;
        while left < n {
            let mid = (left + step).min(n);
            let right = (left + 2 * step).min(n);
            
            if mid < right {
                let mut actions = Vec::new();
                let mut current_memory = 0usize;
                
                // Merge arr[left..mid] with arr[mid..right]
                parallel_merge(&mut arr, left, mid, right, &mut actions, &mut current_memory, thread_id % num_threads);
                
                merge_thread_actions.push(actions);
                thread_id += 1;
            }
            
            left += 2 * step;
        }
        
        // Interleave merge actions
        let merge_interleaved = interleave_actions(&merge_thread_actions);
        interleaved.extend(merge_interleaved);
        
        step *= 2;
        merge_level += 1;
    }
    
    // Mark done
    interleaved.push(Action {
        kind: ActionKind::Done,
        i: 0,
        j: 0,
        value: 0,
        memory: 0,
        temp_idx: 0,
        thread_id: 0,
    });
    
    interleaved
}

/// Sort a chunk of the array (used by each thread)
fn parallel_merge_sort_chunk(
    arr: &mut [u32],
    global_offset: usize, // Offset in the main array
    actions: &mut Vec<Action>,
    current_memory: &mut usize,
    thread_id: usize,
) {
    let n = arr.len();
    if n <= 1 {
        return;
    }
    
    parallel_merge_sort_chunk_recursive(arr, 0, n, global_offset, actions, current_memory, thread_id);
}

fn parallel_merge_sort_chunk_recursive(
    arr: &mut [u32],
    left: usize,
    right: usize,
    global_offset: usize,
    actions: &mut Vec<Action>,
    current_memory: &mut usize,
    thread_id: usize,
) {
    if right - left <= 1 {
        return;
    }
    
    let mid = left + (right - left) / 2;
    parallel_merge_sort_chunk_recursive(arr, left, mid, global_offset, actions, current_memory, thread_id);
    parallel_merge_sort_chunk_recursive(arr, mid, right, global_offset, actions, current_memory, thread_id);
    parallel_merge_chunk(arr, left, mid, right, global_offset, actions, current_memory, thread_id);
}

fn parallel_merge_chunk(
    arr: &mut [u32],
    left: usize,
    mid: usize,
    right: usize,
    global_offset: usize,
    actions: &mut Vec<Action>,
    current_memory: &mut usize,
    thread_id: usize,
) {
    let temp_size = right - left;
    let temp_bytes = temp_size * ELEMENT_SIZE;
    *current_memory += temp_bytes;
    
    let mut temp: Vec<(u32, usize)> = Vec::with_capacity(temp_size);
    let mut i = left;
    let mut j = mid;
    let mut temp_idx = 0;
    
    while i < mid && j < right {
        let global_i = global_offset + i;
        let global_j = global_offset + j;
        
        actions.push(Action {
            kind: ActionKind::Compare,
            i: global_i,
            j: global_j,
            value: 0,
            memory: *current_memory,
            temp_idx: 0,
            thread_id,
        });
        
        if arr[i] <= arr[j] {
            actions.push(Action {
                kind: ActionKind::TempPush,
                i: global_i,
                j: 0,
                value: arr[i],
                memory: *current_memory,
                temp_idx,
                thread_id,
            });
            temp.push((arr[i], i));
            temp_idx += 1;
            i += 1;
        } else {
            actions.push(Action {
                kind: ActionKind::TempPush,
                i: global_j,
                j: 0,
                value: arr[j],
                memory: *current_memory,
                temp_idx,
                thread_id,
            });
            temp.push((arr[j], j));
            temp_idx += 1;
            j += 1;
        }
    }
    
    while i < mid {
        let global_i = global_offset + i;
        actions.push(Action {
            kind: ActionKind::TempPush,
            i: global_i,
            j: 0,
            value: arr[i],
            memory: *current_memory,
            temp_idx,
            thread_id,
        });
        temp.push((arr[i], i));
        temp_idx += 1;
        i += 1;
    }
    
    while j < right {
        let global_j = global_offset + j;
        actions.push(Action {
            kind: ActionKind::TempPush,
            i: global_j,
            j: 0,
            value: arr[j],
            memory: *current_memory,
            temp_idx,
            thread_id,
        });
        temp.push((arr[j], j));
        temp_idx += 1;
        j += 1;
    }
    
    for (k, &(val, _)) in temp.iter().enumerate() {
        let target_idx = global_offset + left + k;
        arr[left + k] = val;
        actions.push(Action {
            kind: ActionKind::Write,
            i: target_idx,
            j: 0,
            value: val,
            memory: *current_memory,
            temp_idx: 0,
            thread_id,
        });
    }
    
    actions.push(Action {
        kind: ActionKind::TempClear,
        i: 0,
        j: 0,
        value: 0,
        memory: *current_memory,
        temp_idx: 0,
        thread_id,
    });
    
    *current_memory -= temp_bytes;
}

/// Merge two sorted regions in the main array
fn parallel_merge(
    arr: &mut [u32],
    left: usize,
    mid: usize,
    right: usize,
    actions: &mut Vec<Action>,
    current_memory: &mut usize,
    thread_id: usize,
) {
    let temp_size = right - left;
    let temp_bytes = temp_size * ELEMENT_SIZE;
    *current_memory += temp_bytes;
    
    let mut temp: Vec<u32> = Vec::with_capacity(temp_size);
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
            thread_id,
        });
        
        if arr[i] <= arr[j] {
            actions.push(Action {
                kind: ActionKind::TempPush,
                i,
                j: 0,
                value: arr[i],
                memory: *current_memory,
                temp_idx,
                thread_id,
            });
            temp.push(arr[i]);
            temp_idx += 1;
            i += 1;
        } else {
            actions.push(Action {
                kind: ActionKind::TempPush,
                i: j,
                j: 0,
                value: arr[j],
                memory: *current_memory,
                temp_idx,
                thread_id,
            });
            temp.push(arr[j]);
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
            thread_id,
        });
        temp.push(arr[i]);
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
            thread_id,
        });
        temp.push(arr[j]);
        temp_idx += 1;
        j += 1;
    }
    
    for (k, &val) in temp.iter().enumerate() {
        let target_idx = left + k;
        arr[target_idx] = val;
        actions.push(Action {
            kind: ActionKind::Write,
            i: target_idx,
            j: 0,
            value: val,
            memory: *current_memory,
            temp_idx: 0,
            thread_id,
        });
    }
    
    actions.push(Action {
        kind: ActionKind::TempClear,
        i: 0,
        j: 0,
        value: 0,
        memory: *current_memory,
        temp_idx: 0,
        thread_id,
    });
    
    *current_memory -= temp_bytes;
}

/// Interleave actions from multiple threads to simulate parallel execution
fn interleave_actions(thread_actions: &[Vec<Action>]) -> Vec<Action> {
    let mut result = Vec::new();
    let mut indices: Vec<usize> = vec![0; thread_actions.len()];
    
    loop {
        let mut any_remaining = false;
        
        // Take one action from each thread that still has actions
        for (thread_idx, actions) in thread_actions.iter().enumerate() {
            if indices[thread_idx] < actions.len() {
                result.push(actions[indices[thread_idx]]);
                indices[thread_idx] += 1;
                any_remaining = true;
            }
        }
        
        if !any_remaining {
            break;
        }
    }
    
    result
}
