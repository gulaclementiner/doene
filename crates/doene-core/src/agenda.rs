//! My Day as a computed agenda (Gör't `src/agenda.js`): the pure computation
//! behind the My Day view. The group order is a parameter — the spec default
//! and the cleaned user-saved order both live in `agenda_order`.

use crate::dates::Date;
use crate::entities::{Importance, Task};

/// Focus mode (Gör't #98): daily — a task is picked while its focus date is
/// today; durable — it stays picked as long as it has any focus date.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FocusMode {
    #[default]
    Daily,
    Durable,
}

/// A My Day agenda group (Gör't's `agendaGroups` registry). Every group is
/// open tasks only — the status filter is applied by [`agenda`], not here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupKey {
    /// Due date before today (overdue carry-over).
    Overdue,
    /// High importance and due today.
    StarredDue,
    /// Picked under the focus mode (the shortlisted group).
    Shortlisted,
    /// Due today.
    DueToday,
    /// High importance without a due date. Hidden by default — visibility is
    /// a settings concern; the filter itself has no notion of settings.
    Radar,
}

impl GroupKey {
    /// The Gör't token: `"overdue"`, `"starred-due"`, …
    pub fn as_token(self) -> &'static str {
        match self {
            Self::Overdue => "overdue",
            Self::StarredDue => "starred-due",
            Self::Shortlisted => "shortlisted",
            Self::DueToday => "due-today",
            Self::Radar => "radar",
        }
    }

    /// Parse a Gör't token; `None` for unknown keys.
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "overdue" => Some(Self::Overdue),
            "starred-due" => Some(Self::StarredDue),
            "shortlisted" => Some(Self::Shortlisted),
            "due-today" => Some(Self::DueToday),
            "radar" => Some(Self::Radar),
            _ => None,
        }
    }

    /// The group filter (Gör't's per-group predicate). No status check: the
    /// agenda applies open-only separately.
    pub fn matches(self, t: &Task, today: Date, focus_mode: FocusMode) -> bool {
        match self {
            Self::Overdue => t.due_date.is_some_and(|d| d < today),
            Self::StarredDue => t.importance == Importance::High && t.due_date == Some(today),
            Self::Shortlisted => is_picked(t, today, focus_mode),
            Self::DueToday => t.due_date == Some(today),
            Self::Radar => t.importance == Importance::High && t.due_date.is_none(),
        }
    }
}

/// Is this task picked under a focus mode (Gör't's `isPicked`): daily — its
/// focus date is today; durable — it has any focus date. The single pick
/// predicate: the agenda's shortlisted group, the Shortlisted list, and the
/// store's hydrated `shortlisted` flag all agree on it.
pub fn is_picked(t: &Task, today: Date, focus_mode: FocusMode) -> bool {
    match focus_mode {
        FocusMode::Daily => t.focus_date == Some(today),
        FocusMode::Durable => t.focus_date.is_some(),
    }
}

/// Within-group sort order (Gör't's `sortTasks` keys).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    Due,
    Alpha,
    Created,
    Manual,
    Importance,
}

/// Sort direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Asc,
    Desc,
}

/// The user's list sort preference (Gör't's `{ order, direction }`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SortSpec {
    pub order: SortOrder,
    pub direction: SortDirection,
}

/// The default within-group sort: due date ascending.
pub const DEFAULT_SORT: SortSpec = SortSpec {
    order: SortOrder::Due,
    direction: SortDirection::Asc,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agenda_order::{DEFAULT_ORDER, GROUP_KEYS, SECTION_BREAK};
    use crate::entities::TaskStatus;

    fn d(s: &str) -> Date {
        Date::parse(s).expect("valid fixture date")
    }

    /// Gör't's `T(id, o)` task builder: open, normal importance, undated.
    fn t(id: &str) -> Task {
        Task {
            id: id.to_owned(),
            subject: id.to_owned(),
            status: TaskStatus::Open,
            importance: Importance::Normal,
            due_date: None,
            reminder_at: None,
            body: None,
            focus_date: None,
            postponed_date: None,
            created_at: "2025-03-01T00:00:00.000Z".to_owned(),
            completed_at: None,
            position: 1.0,
            spawned_from: None,
            parent_id: None,
            recurrence: None,
        }
    }

    macro_rules! task {
        ($id:expr) => {
            t($id)
        };
        ($id:expr, $($field:ident => $val:expr),* $(,)?) => {{
            let mut t = t($id);
            $(t.$field = $val;)*
            t
        }};
    }

    const TODAY: &str = "2025-03-12";

    // Gör't test/agenda.test.js — "default order is the spec order with one
    // section break; radar sits below due today (#98)".
    #[test]
    fn default_order_is_the_spec_order_with_one_section_break() {
        assert_eq!(
            DEFAULT_ORDER,
            [
                "overdue",
                "starred-due",
                SECTION_BREAK,
                "shortlisted",
                "due-today",
                "radar"
            ]
        );
        assert_eq!(
            DEFAULT_SORT,
            SortSpec {
                order: SortOrder::Due,
                direction: SortDirection::Asc
            }
        );
        // The group registry is exactly the five groups in this order — for
        // both focus modes (the filters take a mode; the registry does not).
        let keys = [
            GroupKey::Overdue,
            GroupKey::StarredDue,
            GroupKey::Shortlisted,
            GroupKey::DueToday,
            GroupKey::Radar,
        ];
        assert_eq!(keys.map(|k| k.as_token()), GROUP_KEYS);
        for key in keys {
            assert_eq!(GroupKey::from_token(key.as_token()), Some(key));
        }
    }

    // Gör't — "group filters: each group holds exactly its open tasks" (the
    // filter half; the open-only half is pinned at agenda level). The raw
    // filters have no notion of status, so completed tasks match too.
    #[test]
    fn group_filters_match_exactly_their_tasks() {
        let today = d(TODAY);
        let tasks: Vec<Task> = vec![
            task!("overdue", due_date => Some(d("2025-03-05"))),
            task!("overdue-done", due_date => Some(d("2025-03-05")), status => TaskStatus::Completed),
            task!("starred", importance => Importance::High, due_date => Some(today)),
            task!(
                "starred-done",
                importance => Importance::High,
                due_date => Some(today),
                status => TaskStatus::Completed
            ),
            task!("starred-later", importance => Importance::High, due_date => Some(d("2025-03-13"))),
            task!("pick", focus_date => Some(today)),
            task!("pick-yesterday", focus_date => Some(d("2025-03-11"))),
            task!("due", due_date => Some(today)),
            task!("due-later", due_date => Some(d("2025-03-13"))),
            task!("plain"),
        ];
        let matching = |key: GroupKey| -> Vec<&str> {
            tasks
                .iter()
                .filter(|t| key.matches(t, today, FocusMode::Daily))
                .map(|t| t.id.as_str())
                .collect()
        };
        assert_eq!(matching(GroupKey::Overdue), ["overdue", "overdue-done"]);
        assert_eq!(matching(GroupKey::StarredDue), ["starred", "starred-done"]);
        assert_eq!(
            matching(GroupKey::Shortlisted),
            ["pick"],
            "daily: focus date is today"
        );
        assert_eq!(
            matching(GroupKey::DueToday),
            ["starred", "starred-done", "due"]
        );
        assert_eq!(
            matching(GroupKey::Radar),
            Vec::<&str>::new(),
            "no high+undated task in the table"
        );
    }

    // Gör't's `isPicked` (the single pick predicate, #98).
    #[test]
    fn is_picked_daily_requires_todays_focus_date_durable_any() {
        let today = d(TODAY);
        let picked_today = task!("pick", focus_date => Some(today));
        let picked_old = task!("old-pick", focus_date => Some(d("2025-03-10")));
        let undated = task!("undated");
        assert!(is_picked(&picked_today, today, FocusMode::Daily));
        assert!(
            !is_picked(&picked_old, today, FocusMode::Daily),
            "daily: old pick expired at midnight"
        );
        assert!(!is_picked(&undated, today, FocusMode::Daily));
        assert!(is_picked(&picked_today, today, FocusMode::Durable));
        assert!(
            is_picked(&picked_old, today, FocusMode::Durable),
            "durable keeps old picks"
        );
        assert!(!is_picked(&undated, today, FocusMode::Durable));
    }
}
