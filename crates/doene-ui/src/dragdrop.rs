//! Mouse drag & drop planning (#20). Plain logic, no egui types.
//!
//! The UI measures the visible task rows and the pointer; this module turns
//! that into a [`DropPlan`] using the core zone geometry, and the resulting
//! [`DropOutcome`] is applied by `reorder::apply_outcome` (same path as keys).

use crate::reorder::Tree;
use doene_core::dnd::{
    DropOutcome, DropZone, GroupRect, build_zones, deepest_zone_at, drop_result,
};
use doene_core::tree::can_nest;

/// Measured vertical extent of one visible task row.
#[derive(Debug, Clone, PartialEq)]
pub struct RowRect {
    pub id: String,
    pub top: f64,
    pub bottom: f64,
}

/// Where to draw the drop feedback.
#[derive(Debug, Clone, PartialEq)]
pub enum Indicator {
    /// A line at this y (place between rows).
    Line(f64),
    /// Highlight this card (nest under it).
    Card(String),
    /// The unnest area.
    Unnest,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DropPlan {
    pub outcome: DropOutcome,
    pub indicator: Indicator,
}

/// One sibling list among the visible rows.
struct Level {
    parent: Option<String>,
    ids: Vec<String>,
    groups: Vec<GroupRect>,
    zones: Vec<DropZone>,
}

fn levels(tree: &Tree, rows: &[RowRect], dragged: &str) -> Vec<Level> {
    let parent_of = |id: &str| tree.parents.get(id).cloned().flatten();
    let mut parents: Vec<Option<String>> = Vec::new();
    for r in rows {
        let p = parent_of(&r.id);
        if !parents.contains(&p) {
            parents.push(p);
        }
    }
    // shallowest first
    let depth = |p: &Option<String>| {
        let mut d = 0;
        let mut cur = p.clone();
        while let Some(c) = cur {
            d += 1;
            cur = parent_of(&c);
        }
        d
    };
    parents.sort_by_key(depth);
    parents
        .into_iter()
        .filter(|p| {
            p.as_deref()
                .is_none_or(|p| p != dragged && can_nest(dragged, p, &tree.parents))
        })
        .map(|parent| {
            let mut ids = Vec::new();
            let mut groups = Vec::new();
            for (i, r) in rows.iter().enumerate() {
                if parent_of(&r.id) != parent {
                    continue;
                }
                // the group extends over directly following descendant rows
                let mut bottom = r.bottom;
                for n in &rows[i + 1..] {
                    let mut cur = parent_of(&n.id);
                    let mut inside = false;
                    while let Some(c) = cur {
                        if c == r.id {
                            inside = true;
                            break;
                        }
                        cur = parent_of(&c);
                    }
                    if !inside {
                        break;
                    }
                    bottom = n.bottom;
                }
                ids.push(r.id.clone());
                groups.push(GroupRect {
                    bottom,
                    card_top: r.top,
                    card_height: r.bottom - r.top,
                });
            }
            let top = groups.first().map_or(0.0, |g| g.card_top);
            let bottom = groups.last().map_or(0.0, |g| g.bottom);
            let zones = build_zones(&groups, top, bottom, parent.as_deref());
            Level {
                parent,
                ids,
                groups,
                zones,
            }
        })
        .collect()
}

/// Real sibling index (dragged excluded) for visible slot `at` of `level`.
fn real_index(tree: &Tree, level: &Level, dragged: &str, at: usize) -> usize {
    let full: Vec<&String> = tree
        .children
        .get(&level.parent)
        .map(|v| v.iter().filter(|s| *s != dragged).collect())
        .unwrap_or_default();
    let vis: Vec<&String> = level.ids.iter().filter(|s| *s != dragged).collect();
    let dragged_before = level
        .ids
        .iter()
        .position(|s| s == dragged)
        .is_some_and(|i| i < at);
    let at = at - usize::from(dragged_before);
    let pos = |id: &String| full.iter().position(|f| *f == id);
    match vis.get(at) {
        Some(t) => pos(t).unwrap_or(full.len()),
        None => vis
            .last()
            .and_then(|l| pos(l))
            .map_or(full.len(), |i| i + 1),
    }
}

/// The drop under pointer `y` while dragging `dragged`; `unnest` is true when
/// the pointer is in the unnest area. `None` = not a valid drop.
pub fn plan_drop(
    tree: &Tree,
    rows: &[RowRect],
    dragged: &str,
    y: f64,
    unnest: bool,
) -> Option<DropPlan> {
    if unnest {
        return drop_result(dragged, None, None, true, &tree.parents).map(|outcome| DropPlan {
            outcome,
            indicator: Indicator::Unnest,
        });
    }
    let lv = levels(tree, rows, dragged);
    let lists: Vec<&[DropZone]> = lv.iter().map(|l| l.zones.as_slice()).collect();
    if let Some(zone) = deepest_zone_at(&lists, y) {
        let level = lv.iter().find(|l| l.parent == zone.parent_id)?;
        let line = level.groups.get(zone.at).map_or_else(
            || level.groups.last().map_or(0.0, |g| g.bottom),
            |g| g.card_top,
        );
        let outcome = DropOutcome::Place {
            parent_id: zone.parent_id.clone(),
            index: real_index(tree, level, dragged, zone.at),
        };
        return Some(DropPlan {
            outcome,
            indicator: Indicator::Line(line),
        });
    }
    let card = rows
        .iter()
        .find(|r| (r.top..=r.bottom).contains(&y))
        .map(|r| r.id.as_str())?;
    let outcome = drop_result(dragged, None, Some(card), false, &tree.parents)?;
    Some(DropPlan {
        outcome,
        indicator: Indicator::Card(card.to_owned()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use doene_core::tree::ParentMap;
    use std::collections::HashMap;

    fn tree(pairs: &[(&str, Option<&str>)]) -> Tree {
        let mut parents = ParentMap::new();
        let mut children: HashMap<Option<String>, Vec<String>> = HashMap::new();
        for &(id, p) in pairs {
            parents.insert(id.into(), p.map(str::to_owned));
            children
                .entry(p.map(str::to_owned))
                .or_default()
                .push(id.into());
        }
        Tree { parents, children }
    }

    /// Rows 100 tall, 10 apart: row k spans [k*110, k*110+100].
    fn rows(ids: &[&str]) -> Vec<RowRect> {
        ids.iter()
            .enumerate()
            .map(|(k, id)| RowRect {
                id: (*id).into(),
                top: k as f64 * 110.0,
                bottom: k as f64 * 110.0 + 100.0,
            })
            .collect()
    }

    #[test]
    fn gap_between_cards_places_at_that_index() {
        let t = tree(&[("a", None), ("b", None), ("c", None)]);
        let p = plan_drop(&t, &rows(&["a", "b", "c"]), "c", 105.0, false).unwrap();
        assert_eq!(
            p.outcome,
            DropOutcome::Place {
                parent_id: None,
                index: 1
            }
        );
        assert_eq!(p.indicator, Indicator::Line(110.0));
    }

    #[test]
    fn dropping_below_dragged_item_accounts_for_its_removal() {
        let t = tree(&[("a", None), ("b", None), ("c", None)]);
        // gap after b (slot 2) while dragging a -> index 1 (excluding a)
        let p = plan_drop(&t, &rows(&["a", "b", "c"]), "a", 215.0, false).unwrap();
        assert_eq!(
            p.outcome,
            DropOutcome::Place {
                parent_id: None,
                index: 1
            }
        );
        // below the last card -> last position
        let p = plan_drop(&t, &rows(&["a", "b", "c"]), "a", 320.0, false).unwrap();
        assert_eq!(
            p.outcome,
            DropOutcome::Place {
                parent_id: None,
                index: 2
            }
        );
    }

    #[test]
    fn card_body_nests_and_self_or_descendant_is_refused() {
        let t = tree(&[("a", None), ("b", None), ("s", Some("a"))]);
        let r = rows(&["a", "s", "b"]);
        let p = plan_drop(&t, &r, "b", 150.0, false).unwrap(); // body of s
        assert_eq!(
            p.outcome,
            DropOutcome::Nest {
                parent_id: "s".into()
            }
        );
        assert_eq!(p.indicator, Indicator::Card("s".into()));
        assert_eq!(plan_drop(&t, &r, "a", 150.0, false), None); // own subtask: cycle
        assert_eq!(plan_drop(&t, &r, "a", 50.0, false), None); // own card
    }

    #[test]
    fn zone_inside_own_subtree_is_not_offered() {
        let t = tree(&[("a", None), ("s", Some("a")), ("t", Some("a"))]);
        let r = rows(&["a", "s", "t"]);
        // gap between s and t belongs to a's list, which a may not enter
        let p = plan_drop(&t, &r, "a", 215.0, false);
        assert!(!matches!(
            p,
            Some(DropPlan {
                outcome: DropOutcome::Place {
                    parent_id: Some(_),
                    ..
                },
                ..
            })
        ));
    }

    #[test]
    fn nested_zone_beats_parent_zone_and_unnest_area_wins() {
        let t = tree(&[("a", None), ("s", Some("a")), ("t", Some("a")), ("b", None)]);
        let r = rows(&["a", "s", "t", "b"]);
        let p = plan_drop(&t, &r, "b", 215.0, false).unwrap();
        assert_eq!(
            p.outcome,
            DropOutcome::Place {
                parent_id: Some("a".into()),
                index: 1
            }
        );
        let p = plan_drop(&t, &r, "s", 215.0, true).unwrap();
        assert_eq!(
            (p.outcome, p.indicator),
            (DropOutcome::Unnest, Indicator::Unnest)
        );
        assert_eq!(plan_drop(&t, &r, "b", 215.0, true), None); // already top level
    }
}
