//! Task tree guards (#90): a task cannot be its own subtask, and nesting must
//! never create a cycle (docs/plan.md §7 row 7).
//!
//! Pure rules over plain data: the parent map is `task id -> parent id`
//! (`None` at top level). Consumed by keyboard reordering (#19), mouse DnD
//! (#20) and the service layer.

use std::collections::HashMap;

/// The task tree as plain data: every task id mapped to its parent id
/// (`None` for top-level tasks).
pub type ParentMap = HashMap<String, Option<String>>;

/// Maximum ancestor steps walked before a chain is assumed corrupt (Gör't parity: 1000).
const ANCESTOR_WALK_LIMIT: usize = 1000;

/// Whether `task_id` sits below `ancestor_id` anywhere in its parent chain.
///
/// A task is never its own descendant. The walk stops at a top-level task, a
/// missing id, or after [`ANCESTOR_WALK_LIMIT`] steps (corrupt data).
pub fn descends_from(task_id: &str, parents: &ParentMap, ancestor_id: &str) -> bool {
    if task_id == ancestor_id {
        return false; // a task is not its own descendant
    }
    let mut cur = Some(task_id);
    for _ in 0..ANCESTOR_WALK_LIMIT {
        let id = match cur {
            Some(id) => id,
            None => break,
        };
        match parents.get(id) {
            Some(parent) if parent.as_deref() == Some(ancestor_id) => return true,
            Some(parent) => cur = parent.as_deref(),
            None => break, // missing task: the chain ends here
        }
    }
    false
}

/// Whether `task_id` may be nested under `parent_id`: not itself, and not one
/// of its own subtasks (any depth).
///
/// Existence is checked by the caller — this rule only guards against cycles
/// (Gör't parity: the store rejects a missing parent separately).
pub fn can_nest(task_id: &str, parent_id: &str, parents: &ParentMap) -> bool {
    task_id != parent_id && !descends_from(parent_id, parents, task_id)
}

#[cfg(test)]
mod tests {
    use super::{can_nest, descends_from};
    use std::collections::HashMap;

    fn parents(pairs: &[(&str, Option<&str>)]) -> HashMap<String, Option<String>> {
        pairs
            .iter()
            .map(|&(id, p)| (id.to_string(), p.map(str::to_owned)))
            .collect()
    }

    // Parity fixtures: Gör't test/tree.test.js.

    #[test]
    fn direct_child_descends_from_its_parent() {
        let by_id = parents(&[("a", None), ("b", Some("a"))]);
        assert!(descends_from("b", &by_id, "a"));
    }

    #[test]
    fn grandchild_descends_from_roots_two_levels_up() {
        let by_id = parents(&[("a", None), ("b", Some("a")), ("c", Some("b"))]);
        assert!(descends_from("c", &by_id, "a"));
        assert!(descends_from("c", &by_id, "b"));
    }

    #[test]
    fn a_task_is_not_its_own_descendant() {
        let by_id = parents(&[("a", None), ("b", Some("a"))]);
        assert!(!descends_from("b", &by_id, "b"));
    }

    #[test]
    fn a_sibling_is_not_a_descendant() {
        let by_id = parents(&[("a", None), ("b", Some("a")), ("c", Some("a"))]);
        assert!(!descends_from("b", &by_id, "c"));
    }

    #[test]
    fn a_task_with_no_parent_is_not_a_descendant_of_anything() {
        let by_id = parents(&[("a", None)]);
        assert!(!descends_from("a", &by_id, "x"));
    }

    #[test]
    fn a_missing_ancestor_is_not_a_descendant() {
        let by_id = parents(&[("a", None), ("b", Some("a"))]);
        assert!(!descends_from("b", &by_id, "nope"));
    }

    #[test]
    fn a_corrupt_parent_chain_that_loops_terminates() {
        let looped = parents(&[("a", Some("b")), ("b", Some("a"))]);
        assert!(!descends_from("a", &looped, "x"));
        // even a self-loop: a task is never its own descendant
        let self_loop = parents(&[("a", Some("a"))]);
        assert!(!descends_from("a", &self_loop, "a"));
    }

    #[test]
    fn a_task_cannot_nest_under_itself() {
        let by_id = parents(&[("a", None)]);
        assert!(!can_nest("a", "a", &by_id));
    }

    #[test]
    fn a_task_cannot_nest_under_one_of_its_own_descendants_no_cycle() {
        // chain: c's parent is b, b's parent is a. Dropping a under c would make a cycle.
        let by_id = parents(&[("c", Some("b")), ("b", Some("a")), ("a", None)]);
        assert!(!can_nest("a", "c", &by_id));
        assert!(!can_nest("a", "b", &by_id));
        assert!(
            can_nest("c", "a", &by_id),
            "a deeper task may move up under a root"
        );
    }

    #[test]
    fn a_task_can_nest_under_an_unrelated_task() {
        let by_id = parents(&[]);
        assert!(can_nest("x", "y", &by_id));
    }

    #[test]
    fn a_deep_chain_is_walked_for_cycles_and_a_cycle_at_the_bottom_is_caught() {
        // Gör't subtasks.test.js: 30-deep chain; nesting the top under the bottom is a cycle.
        let ids: Vec<String> = (0..30).map(|i| format!("n{i}")).collect();
        let mut by_id = std::collections::HashMap::new();
        for (i, id) in ids.iter().enumerate() {
            let parent = i
                .checked_sub(1)
                .and_then(|j| ids.get(j))
                .map(String::as_str);
            by_id.insert(id.clone(), parent.map(str::to_owned));
        }
        assert!(!can_nest(&ids[0], &ids[29], &by_id));
    }
}
