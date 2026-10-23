//! Keyboard reordering (#19): Alt+arrows move, nest and unnest the selected task.
//!
//! Plain logic, no egui types. A key press is turned into the same
//! [`DropOutcome`] mouse drag & drop (#20) produces, then applied through
//! `Service::place_task` (one undoable move, gap-free renumbering).

use crate::drawer::error_message;
use doene_core::i18n::tr;
use doene_core::dnd::DropOutcome;
use doene_core::tree::{ParentMap, can_nest};
use doene_service::Service;
use std::collections::HashMap;

/// An Alt+arrow press.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveKey {
    Up,
    Down,
    /// Nest under the previous sibling (as its last subtask).
    Nest,
    /// Move out to the parent's level, right after the parent.
    Unnest,
}

/// Why a key press cannot move the task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reject {
    /// Already first/last, or already top level: silently nothing to do.
    Edge,
    /// Nesting has no previous sibling to nest under (feedback shown).
    NoPreviousSibling,
    /// Nesting would create a cycle (feedback shown).
    Cycle,
}

/// Sibling lists in manual order plus the parent map, as plain data.
pub struct Tree {
    pub parents: ParentMap,
    /// parent id (`None` = top level) -> child ids in position order.
    pub children: HashMap<Option<String>, Vec<String>>,
}

impl Tree {
    pub fn load(service: &Service) -> Tree {
        let mut all: Vec<_> = service.all_tasks().unwrap_or_default().into_iter().map(|h| h.task).collect();
        all.sort_by(|a, b| a.position.total_cmp(&b.position));
        let mut parents = ParentMap::new();
        let mut children: HashMap<Option<String>, Vec<String>> = HashMap::new();
        for t in all {
            parents.insert(t.id.clone(), t.parent_id.clone());
            children.entry(t.parent_id).or_default().push(t.id);
        }
        Tree { parents, children }
    }
}

/// The move a key press produces for `id`.
pub fn plan_move(tree: &Tree, id: &str, key: MoveKey) -> Result<DropOutcome, Reject> {
    let parent = tree.parents.get(id).cloned().ok_or(Reject::Edge)?;
    let sibs = tree.children.get(&parent).map(Vec::as_slice).unwrap_or_default();
    let idx = sibs.iter().position(|s| s == id).ok_or(Reject::Edge)?;
    match key {
        MoveKey::Up => idx
            .checked_sub(1)
            .map(|i| DropOutcome::Place { parent_id: parent, index: i })
            .ok_or(Reject::Edge),
        MoveKey::Down => (idx + 1 < sibs.len())
            .then(|| DropOutcome::Place { parent_id: parent, index: idx + 1 })
            .ok_or(Reject::Edge),
        MoveKey::Nest => {
            let prev = idx.checked_sub(1).map(|i| &sibs[i]).ok_or(Reject::NoPreviousSibling)?;
            if can_nest(id, prev, &tree.parents) {
                Ok(DropOutcome::Nest { parent_id: prev.clone() })
            } else {
                Err(Reject::Cycle)
            }
        }
        MoveKey::Unnest => {
            let p = parent.ok_or(Reject::Edge)?;
            let grand = tree.parents.get(&p).cloned().flatten();
            let at = tree.children.get(&grand).and_then(|g| g.iter().position(|x| *x == p)).ok_or(Reject::Edge)?;
            Ok(DropOutcome::Place { parent_id: grand, index: at + 1 })
        }
    }
}

/// Apply a drop outcome with one undoable `place_task` (nest = last child).
/// Shared by keyboard (#19) and mouse drag & drop (#20).
pub fn apply_outcome(service: &Service, id: &str, outcome: &DropOutcome, locale: &str) -> Result<(), String> {
    let (parent, index) = match outcome {
        DropOutcome::Place { parent_id, index } => (parent_id.as_deref(), Some(*index)),
        DropOutcome::Nest { parent_id } => (Some(parent_id.as_str()), None),
        DropOutcome::Unnest => (None, None),
    };
    let index = index.unwrap_or_else(|| {
        service
            .all_tasks()
            .map(|a| a.iter().filter(|h| h.task.parent_id.as_deref() == parent && h.task.id != id).count())
            .unwrap_or(0)
    });
    service.place_task(id, parent, index).map(|_| ()).map_err(|e| error_message(&e, locale))
}

/// Handle a key press: `Ok(true)` moved, `Ok(false)` nothing to do (edge),
/// `Err(message)` rejected with localised feedback.
pub fn key_move(service: &Service, id: &str, key: MoveKey, locale: &str) -> Result<bool, String> {
    match plan_move(&Tree::load(service), id, key) {
        Ok(o) => apply_outcome(service, id, &o, locale).map(|()| true),
        Err(Reject::Edge) => Ok(false),
        Err(Reject::NoPreviousSibling) => Err(tr("reorder.cannotNest", &HashMap::new(), Some(locale))),
        Err(Reject::Cycle) => Err(tr("error.subtaskCycle", &HashMap::new(), Some(locale))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use doene_storage::Storage;

    fn svc() -> (Service, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let storage = Storage::open(&dir.path().join("d.db")).unwrap();
        (Service::with_clock(storage, Box::new(|| "2025-03-10T09:00:00.000Z".into())), dir)
    }

    fn add(s: &Service, name: &str) -> String {
        s.quick_add(name, None, true).unwrap().task.id
    }

    /// Subjects of `parent`'s children in order.
    fn order(s: &Service, parent: Option<&str>) -> Vec<String> {
        let t = Tree::load(s);
        let names: HashMap<_, _> = s.all_tasks().unwrap().into_iter().map(|h| (h.task.id, h.task.subject)).collect();
        t.children.get(&parent.map(str::to_owned)).into_iter().flatten().map(|i| names[i].clone()).collect()
    }

    #[test]
    fn up_and_down_swap_with_neighbours_and_stop_at_edges() {
        let (s, _d) = svc();
        let (a, b, c) = (add(&s, "a"), add(&s, "b"), add(&s, "c"));
        assert_eq!(key_move(&s, &c, MoveKey::Up, "en"), Ok(true));
        assert_eq!(order(&s, None), ["a", "c", "b"]);
        assert_eq!(key_move(&s, &a, MoveKey::Up, "en"), Ok(false));
        assert_eq!(key_move(&s, &a, MoveKey::Down, "en"), Ok(true));
        assert_eq!(order(&s, None), ["c", "a", "b"]);
        assert_eq!(key_move(&s, &b, MoveKey::Down, "en"), Ok(false));
    }

    #[test]
    fn nest_goes_under_previous_sibling_unnest_lands_after_parent() {
        let (s, _d) = svc();
        let (a, b, c) = (add(&s, "a"), add(&s, "b"), add(&s, "c"));
        assert_eq!(key_move(&s, &b, MoveKey::Nest, "en"), Ok(true));
        assert_eq!(order(&s, Some(&a)), ["b"]);
        assert_eq!(key_move(&s, &c, MoveKey::Nest, "en"), Ok(true)); // under a, after b
        assert_eq!(order(&s, Some(&a)), ["b", "c"]);
        assert_eq!(key_move(&s, &b, MoveKey::Unnest, "en"), Ok(true));
        assert_eq!(order(&s, None), ["a", "b"]);
        assert_eq!(order(&s, Some(&a)), ["c"]);
        assert_eq!(key_move(&s, &b, MoveKey::Unnest, "en"), Ok(false)); // already top level
    }

    #[test]
    fn nest_without_previous_sibling_and_cycles_give_feedback() {
        let (s, _d) = svc();
        let a = add(&s, "a");
        assert_eq!(key_move(&s, &a, MoveKey::Nest, "en").unwrap_err(), "There is no task above to nest under");
        assert!(key_move(&s, &a, MoveKey::Nest, "sv").unwrap_err().contains("ovan"));
        // a cycle is refused: the previous sibling listed is a descendant of the task
        let t = Tree {
            parents: HashMap::from([("a".to_owned(), None), ("p".to_owned(), Some("a".to_owned()))]),
            children: HashMap::from([(None, vec!["p".to_owned(), "a".to_owned()])]),
        };
        assert_eq!(plan_move(&t, "a", MoveKey::Nest), Err(Reject::Cycle));
    }

    #[test]
    fn moves_are_gap_free_undoable_and_persist() {
        let (s, d) = svc();
        let ids: Vec<_> = ["a", "b", "c", "d"].iter().map(|n| add(&s, n)).collect();
        // repeated moves squeeze the gap between neighbours until renumbering kicks in
        for _ in 0..60 {
            key_move(&s, &ids[3], MoveKey::Up, "en").unwrap();
            key_move(&s, &ids[3], MoveKey::Down, "en").unwrap();
        }
        key_move(&s, &ids[3], MoveKey::Up, "en").unwrap();
        assert_eq!(order(&s, None), ["a", "b", "d", "c"]);
        let mut pos: Vec<f64> = s.all_tasks().unwrap().iter().map(|h| h.task.position).collect();
        pos.sort_by(f64::total_cmp);
        assert!(pos.windows(2).all(|w| w[0] < w[1]));
        s.undo().unwrap();
        assert_eq!(order(&s, None), ["a", "b", "c", "d"]);
        key_move(&s, &ids[0], MoveKey::Down, "en").unwrap();
        drop(s);
        let s2 = Service::new(Storage::open(&d.path().join("d.db")).unwrap());
        assert_eq!(order(&s2, None), ["b", "a", "c", "d"]);
    }
}
