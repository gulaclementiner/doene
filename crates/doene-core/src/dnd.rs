//! Drag & drop zone rules (#69): where a dropped task lands (docs/plan.md §7 row 8).
//!
//! Pure geometry + outcome rules, parity with Gör't's `public/task-drop.js`.
//! The UI measures card rects and feeds them in; the mouse glue (#20) and
//! keyboard reordering (#19) consume the outcomes.

use crate::tree::{ParentMap, can_nest};

/// Fraction of a card that still belongs to the drop zone above it (Gör't: TASK_EDGE).
pub const TASK_EDGE: f64 = 0.25;

/// Measured geometry of one sibling group in a list, top to bottom.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GroupRect {
    /// Bottom edge of the whole group (card plus any open subtask list).
    pub bottom: f64,
    /// Top edge of the task card itself.
    pub card_top: f64,
    pub card_height: f64,
}

/// A drop zone between sibling groups.
#[derive(Debug, Clone, PartialEq)]
pub struct DropZone {
    pub top: f64,
    pub bottom: f64,
    /// Sibling index a drop lands at (0..=number of siblings).
    pub at: usize,
    /// Parent of this sibling list; `None` at top level.
    pub parent_id: Option<String>,
}

/// Zones for one list of siblings: one before each group plus one after the last.
///
/// A zone runs from the bottom of the group above to the top quarter
/// ([`TASK_EDGE`]) of the card below; the first and last zones use the list edges.
pub fn build_zones(
    groups: &[GroupRect],
    list_top: f64,
    list_bottom: f64,
    parent_id: Option<&str>,
) -> Vec<DropZone> {
    (0..=groups.len())
        .map(|k| DropZone {
            // k is in 0..=groups.len(), so k-1 (when it exists) is a valid index
            top: k.checked_sub(1).map_or(list_top, |j| groups[j].bottom),
            bottom: match groups.get(k) {
                Some(g) => g.card_top + g.card_height * TASK_EDGE,
                None => list_bottom,
            },
            at: k,
            parent_id: parent_id.map(str::to_owned),
        })
        .collect()
}

/// The zone under pointer `y`, or none.
///
/// First match wins; zones of one list never overlap (the gap between a zone's
/// bottom and the next zone's top is card body, not a drop target).
pub fn zone_at(zones: &[DropZone], y: f64) -> Option<&DropZone> {
    zones.iter().find(|z| (z.top..=z.bottom).contains(&y))
}

/// The zone under `y` across all sibling lists of the main view.
///
/// `lists` are shallowest-first (root list first, then each open subtask list);
/// the deepest matching zone wins — a nested zone beats its parent's at the same y
/// (Gör't parity: task-dnd.js checks sibling lists in reverse).
pub fn deepest_zone_at<'a>(lists: &[&'a [DropZone]], y: f64) -> Option<&'a DropZone> {
    lists.iter().rev().find_map(|zones| zone_at(zones, y))
}

/// The move a drop produces (Gör't parity: `dropResult`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DropOutcome {
    /// Place at `index` among the siblings under `parent_id` (`None` = top level).
    Place {
        parent_id: Option<String>,
        index: usize,
    },
    /// Nest under `parent_id`.
    Nest { parent_id: String },
    /// Unnest to top level.
    Unnest,
}

/// The outcome of a drop. An unnest area wins over a zone, a zone wins over a
/// card; a card drop that would create a cycle does nothing (`None`).
pub fn drop_result(
    task_id: &str,
    zone: Option<&DropZone>,
    card: Option<&str>,
    unnest: bool,
    parents: &ParentMap,
) -> Option<DropOutcome> {
    if unnest {
        return parents
            .get(task_id)
            .and_then(|p| p.as_deref())
            .map(|_| DropOutcome::Unnest);
    }
    if let Some(zone) = zone {
        return Some(DropOutcome::Place {
            parent_id: zone.parent_id.clone(),
            index: zone.at,
        });
    }
    if let Some(card) = card.filter(|c| can_nest(task_id, c, parents)) {
        return Some(DropOutcome::Nest {
            parent_id: card.to_owned(),
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{DropOutcome, DropZone, build_zones, deepest_zone_at, drop_result, zone_at};
    use std::collections::HashMap;

    fn parents(pairs: &[(&str, Option<&str>)]) -> HashMap<String, Option<String>> {
        pairs
            .iter()
            .map(|&(id, p)| (id.to_string(), p.map(str::to_owned)))
            .collect()
    }

    /// Three sibling cards, 100px tall, 10px apart: card k spans [k*110, k*110+100].
    fn three_groups() -> Vec<super::GroupRect> {
        (0..3)
            .map(|k| super::GroupRect {
                bottom: k as f64 * 110.0 + 100.0,
                card_top: k as f64 * 110.0,
                card_height: 100.0,
            })
            .collect()
    }

    // Parity fixtures: Gör't test/task-drop.test.js.

    #[test]
    fn zones_one_more_than_the_number_of_siblings_each_before_a_sibling_last_after_all() {
        let z = build_zones(&three_groups(), 0.0, 320.0, None);
        assert_eq!(z.len(), 4);
        assert_eq!(z.iter().map(|x| x.at).collect::<Vec<_>>(), vec![0, 1, 2, 3]);
    }

    #[test]
    fn a_zone_runs_from_the_bottom_of_the_card_above_to_the_top_quarter_of_the_card_below() {
        let z = build_zones(&three_groups(), 0.0, 320.0, None);
        // zone 1 sits between card 0 (bottom 100) and card 1 (top 110, quarter at 110 + 100*0.25 = 135)
        assert_eq!(z[1].top, 100.0);
        assert_eq!(z[1].bottom, 110.0 + 100.0 * super::TASK_EDGE);
    }

    #[test]
    fn the_first_zone_starts_at_the_list_top_the_last_ends_at_the_list_bottom() {
        let z = build_zones(&three_groups(), 0.0, 320.0, None);
        assert_eq!(z[0].top, 0.0);
        assert_eq!(z[3].bottom, 320.0);
    }

    #[test]
    fn zone_at_finds_the_zone_under_the_pointer_and_nothing_below_the_cards() {
        let z = build_zones(&three_groups(), 0.0, 320.0, None);
        assert_eq!(
            zone_at(&z, 105.0).map(|x| x.at),
            Some(1),
            "in the gap between card 0 and card 1"
        );
        assert!(
            zone_at(&z, 50.0).is_none(),
            "over card 0 middle is not a zone"
        );
        assert!(zone_at(&z, 400.0).is_none(), "outside the list");
    }

    #[test]
    fn dropping_a_card_middle_nests_dropping_a_zone_places_it_at_that_index_with_its_parent() {
        let p = parents(&[]);
        assert_eq!(
            drop_result("x", None, Some("y"), false, &p),
            Some(DropOutcome::Nest {
                parent_id: "y".into()
            })
        );
        let zone = DropZone {
            top: 0.0,
            bottom: 1.0,
            at: 2,
            parent_id: Some("p".into()),
        };
        assert_eq!(
            drop_result("x", Some(&zone), None, false, &p),
            Some(DropOutcome::Place {
                parent_id: Some("p".into()),
                index: 2
            })
        );
        let zone = DropZone {
            top: 0.0,
            bottom: 1.0,
            at: 0,
            parent_id: None,
        };
        assert_eq!(
            drop_result("x", Some(&zone), None, false, &p),
            Some(DropOutcome::Place {
                parent_id: None,
                index: 0
            })
        );
    }

    #[test]
    fn the_top_level_drop_area_unnests_a_subtask_and_does_nothing_for_a_top_level_task() {
        let sub = parents(&[("kid", Some("boss")), ("boss", None)]);
        assert_eq!(
            drop_result("kid", None, None, true, &sub),
            Some(DropOutcome::Unnest)
        );
        assert_eq!(drop_result("boss", None, None, true, &sub), None);
    }

    #[test]
    fn a_card_drop_that_would_create_a_cycle_does_nothing() {
        let p = parents(&[("c", Some("a"))]); // c is under a
        assert_eq!(drop_result("a", None, Some("c"), false, &p), None);
    }

    // Deepest-list-wins: Gör't applies this in the DOM layer (task-dnd.js checks
    // sibling lists deepest-first); no Gör't unit fixture exists, so these pin
    // the rule directly.

    /// Root list with two cards plus one open children list under card 1.
    fn nested_lists() -> (Vec<DropZone>, Vec<DropZone>) {
        let root = build_zones(
            &[
                super::GroupRect {
                    bottom: 100.0,
                    card_top: 0.0,
                    card_height: 100.0,
                },
                super::GroupRect {
                    bottom: 210.0,
                    card_top: 110.0,
                    card_height: 100.0,
                },
            ],
            0.0,
            320.0,
            None,
        );
        // children list of card "b" (group 1 spans y 110..210)
        let nested = build_zones(
            &[super::GroupRect {
                bottom: 180.0,
                card_top: 130.0,
                card_height: 40.0,
            }],
            115.0,
            205.0,
            Some("b"),
        );
        (root, nested)
    }

    #[test]
    fn deepest_list_wins_over_its_parent_at_the_same_y() {
        let (root, nested) = nested_lists();
        // y=120 is inside root zone 1 [100..135] AND nested zone 0 [115..140]
        assert_eq!(zone_at(&root, 120.0).map(|z| z.at), Some(1));
        let winner = deepest_zone_at(&[&root, &nested], 120.0).expect("a zone matches");
        assert_eq!(
            winner.parent_id.as_deref(),
            Some("b"),
            "the nested list's zone wins"
        );
        assert_eq!(winner.at, 0);
    }

    #[test]
    fn deepest_falls_back_to_a_shallower_list_when_it_has_no_zone_there() {
        let (root, nested) = nested_lists();
        // y=105 is inside root zone 1 [100..135] but above the nested list's first zone
        let winner = deepest_zone_at(&[&root, &nested], 105.0).expect("a zone matches");
        assert_eq!(winner.parent_id.as_deref(), None);
        assert_eq!(winner.at, 1);
    }

    #[test]
    fn a_single_list_behaves_like_zone_at() {
        let (root, _nested) = nested_lists();
        for y in [5.0_f64, 120.0, 250.0, 999.0] {
            assert_eq!(deepest_zone_at(&[&root], y), zone_at(&root, y));
        }
    }
}
