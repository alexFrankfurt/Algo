use crate::engine::{Action, ActionKind};

pub fn bubble_sort_actions(values: &[u32]) -> Vec<Action> {
    let mut arr: Vec<u32> = values.to_vec();
    let n = arr.len();
    let mut actions = Vec::new();

    for i in 0..n {
        for j in 0..(n - 1 - i) {
            actions.push(Action {
                kind: ActionKind::Compare,
                i: j,
                j: j + 1,
                value: 0,
                memory: 0,
                temp_idx: 0,
                thread_id: 0,
            });
            if arr[j] > arr[j + 1] {
                arr.swap(j, j + 1);
                actions.push(Action {
                    kind: ActionKind::Swap,
                    i: j,
                    j: j + 1,
                    value: 0,
                    memory: 0,
                    temp_idx: 0,
                    thread_id: 0,
                });
            }
        }
        actions.push(Action {
            kind: ActionKind::Done,
            i: n - 1 - i,
            j: n - 1 - i,
            value: arr[n - 1 - i],
            memory: 0,
            temp_idx: 0,
            thread_id: 0,
        });
    }

    actions
}
