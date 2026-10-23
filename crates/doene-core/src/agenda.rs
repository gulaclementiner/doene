//! My Day as a computed agenda (Gör't `src/agenda.js`): the pure computation
//! behind the My Day view. The group order is a parameter — the spec default
//! and the cleaned user-saved order both live in `agenda_order`.

use std::collections::HashSet;
use std::fmt;

use crate::agenda_order::{SECTION_BREAK, default_order};
use crate::dates::Date;
use crate::entities::{Importance, Task, TaskStatus};

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

/// The two agenda sections split by the section break (Gör't's
/// `"do-first"` / `"continue-with"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionId {
    DoFirst,
    ContinueWith,
}

impl SectionId {
    /// Gör't's section id: `"do-first"` / `"continue-with"`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DoFirst => "do-first",
            Self::ContinueWith => "continue-with",
        }
    }
}

/// One non-empty group inside a section.
#[derive(Debug, Clone, PartialEq)]
pub struct Group<'a> {
    pub key: GroupKey,
    /// The group's tasks in within-group order.
    pub tasks: Vec<&'a Task>,
}

/// A section of the agenda ("Do first" / "Continue with").
#[derive(Debug, Clone, PartialEq)]
pub struct Section<'a> {
    pub id: SectionId,
    /// Non-empty groups in order; empty groups are omitted.
    pub groups: Vec<Group<'a>>,
}

/// The My Day agenda (Gör't's `agenda` result).
#[derive(Debug, Clone, PartialEq)]
pub struct Agenda<'a> {
    /// Sections in order; an empty section keeps its slot.
    pub sections: Vec<Section<'a>>,
    /// Tasks completed today, ascending by completion time (first at the top).
    pub completed: Vec<&'a Task>,
}

/// Options for [`agenda`] (Gör't's `{ today, order, sort, focusMode }`).
#[derive(Debug, Clone)]
pub struct AgendaOptions {
    /// The agenda's "today" — midnight expiry is relative to this date.
    pub today: Date,
    /// Group order: group keys plus at most one section break (Gör't tokens).
    pub order: Vec<String>,
    /// Within-group sort; default [`DEFAULT_SORT`].
    pub sort: SortSpec,
    /// Focus mode for the shortlisted group; default `FocusMode::Daily`.
    pub focus_mode: FocusMode,
}

impl AgendaOptions {
    /// Options with the spec defaults (default order, due-asc sort, daily focus).
    pub fn new(today: Date) -> Self {
        Self {
            today,
            order: default_order(),
            sort: DEFAULT_SORT,
            focus_mode: FocusMode::Daily,
        }
    }
}

/// An unknown key in an agenda order (Gör't throws `unknown agenda group: <key>`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownGroup(pub String);

impl fmt::Display for UnknownGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown agenda group: {}", self.0)
    }
}

impl std::error::Error for UnknownGroup {}

/// The My Day agenda (Gör't's `agenda`): open tasks are assigned to the first
/// group in `order` whose filter matches — each task appears exactly once;
/// the break splits the groups into two sections; empty groups are omitted
/// (an empty section keeps its slot). `completed` lists today's completions
/// ascending by completion time.
pub fn agenda<'a>(tasks: &'a [Task], opts: &AgendaOptions) -> Result<Agenda<'a>, UnknownGroup> {
    let mut sections: Vec<Section<'a>> = vec![];
    // Tasks already assigned to an earlier group (first match wins).
    let mut used: HashSet<&'a str> = HashSet::new();

    for key in &opts.order {
        if key == SECTION_BREAK {
            let id = if sections.is_empty() { SectionId::DoFirst } else { SectionId::ContinueWith };
            sections.push(Section { id, groups: vec![] });
            continue;
        }
        let group_key = GroupKey::from_token(key).ok_or(UnknownGroup(key.clone()))?;
        if sections.is_empty() {
            sections.push(Section { id: SectionId::DoFirst, groups: vec![] });
        }
        // Open tasks only; each task goes to the first matching group.
        let matched: Vec<&'a Task> = tasks
            .iter()
            .filter(|t| {
                t.status != TaskStatus::Completed
                    && !used.contains(t.id.as_str())
                    && group_key.matches(t, opts.today, opts.focus_mode)
            })
            .collect();
        if !matched.is_empty() {
            for t in &matched {
                used.insert(t.id.as_str());
            }
            let matched = crate::smartlists::sort_tasks(&matched, opts.sort, false);
            sections.last_mut().expect("a section exists after the push above").groups.push(Group {
                key: group_key,
                tasks: matched,
            });
        }
    }

    // Completed today, ascending by completion time (first at the top).
    let today = opts.today.to_string();
    let mut completed: Vec<&'a Task> = tasks
        .iter()
        .filter(|t| {
            t.status == TaskStatus::Completed
                && t.completed_at
                    .as_deref()
                    .is_some_and(|ts| ts.starts_with(&today))
        })
        .collect();
    completed.sort_by(|a, b| a.completed_at.cmp(&b.completed_at));

    Ok(Agenda { sections, completed })
}

/// The open tasks the agenda shows, in agenda order (sections, then groups,
/// then within-group order).
pub fn agenda_open_tasks<'a>(tasks: &'a [Task], opts: &AgendaOptions) -> Result<Vec<&'a Task>, UnknownGroup> {
    let a = agenda(tasks, opts)?;
    Ok(a.sections
        .iter()
        .flat_map(|s| s.groups.iter().flat_map(|g| g.tasks.iter()))
        .copied()
        .collect())
}

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

    /// Gör't's `group(a, section, id)` accessor.
    fn group_ids<'t>(a: &Agenda<'t>, section: SectionId, key: GroupKey) -> Vec<&'t str> {
        a.sections
            .iter()
            .find(|s| s.id == section)
            .and_then(|s| s.groups.iter().find(|g| g.key == key))
            .map(|g| g.tasks.iter().map(|t| t.id.as_str()).collect())
            .unwrap_or_default()
    }

    /// Gör't's `all_group_tasks(a)`.
    fn all_group_tasks<'t>(a: &Agenda<'t>) -> Vec<&'t str> {
        a.sections
            .iter()
            .flat_map(|s| s.groups.iter().flat_map(|g| g.tasks.iter()))
            .map(|t| t.id.as_str())
            .collect()
    }

    // Gör't test/agenda.test.js — "group filters: each group holds exactly its open tasks".
    #[test]
    fn group_filters_each_group_holds_exactly_its_open_tasks() {
        let today = d(TODAY);
        let tasks: Vec<Task> = vec![
            task!("overdue", due_date => Some(d("2025-03-05"))),
            task!(
                "overdue-done",
                due_date => Some(d("2025-03-05")),
                status => TaskStatus::Completed
            ),
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
        let a = agenda(&tasks, &AgendaOptions::new(today)).unwrap();
        assert_eq!(group_ids(&a, SectionId::DoFirst, GroupKey::Overdue), ["overdue"]);
        assert_eq!(group_ids(&a, SectionId::DoFirst, GroupKey::StarredDue), ["starred"]);
        assert_eq!(group_ids(&a, SectionId::ContinueWith, GroupKey::Shortlisted), ["pick"]);
        // `starred` was already taken by starred-due (first match wins).
        assert_eq!(group_ids(&a, SectionId::ContinueWith, GroupKey::DueToday), ["due"]);
        let mut all = all_group_tasks(&a);
        all.sort();
        assert_eq!(all, ["due", "overdue", "pick", "starred"], "the union of groups is exactly the open tasks");
    }

    // Gör't — "first-match dedup: a task appears in exactly one group".
    #[test]
    fn first_match_dedup_a_task_appears_in_exactly_one_group() {
        let today = d(TODAY);
        let tasks: Vec<Task> = vec![
            task!("pick-due", focus_date => Some(today), due_date => Some(today)),
            task!("starred", importance => Importance::High, due_date => Some(today)),
            task!("overdue-starred", importance => Importance::High, due_date => Some(d("2025-03-01"))),
            task!("plain"),
        ];
        let opts = AgendaOptions::new(today);
        let a = agenda(&tasks, &opts).unwrap();
        assert_eq!(group_ids(&a, SectionId::DoFirst, GroupKey::Overdue), ["overdue-starred"]);
        assert_eq!(group_ids(&a, SectionId::DoFirst, GroupKey::StarredDue), ["starred"]);
        assert_eq!(group_ids(&a, SectionId::ContinueWith, GroupKey::Shortlisted), ["pick-due"]);
        // due-today is empty: both of its candidates were taken earlier.
        assert_eq!(
            group_ids(&a, SectionId::ContinueWith, GroupKey::DueToday),
            Vec::<&str>::new()
        );
        let all = all_group_tasks(&a);
        let unique: std::collections::HashSet<&str> = all.iter().copied().collect();
        assert_eq!(all.len(), unique.len(), "no task in two groups");
        // The open tasks the agenda shows, in agenda order.
        let shown = agenda_open_tasks(&tasks, &opts).unwrap();
        assert_eq!(
            shown.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(),
            ["overdue-starred", "starred", "pick-due"]
        );
    }

    // Gör't — "empty groups are omitted; empty sections keep their slot".
    #[test]
    fn empty_groups_are_omitted_empty_sections_keep_their_slot() {
        let today = d(TODAY);
        let tasks: Vec<Task> = vec![task!("pick", focus_date => Some(today))];
        let a = agenda(&tasks, &AgendaOptions::new(today)).unwrap();
        assert_eq!(
            a.sections.iter().map(|s| s.id).collect::<Vec<_>>(),
            [SectionId::DoFirst, SectionId::ContinueWith]
        );
        assert!(a.sections[0].groups.is_empty());
        let cw = &a.sections[1];
        assert_eq!(cw.groups.len(), 1);
        assert_eq!(cw.groups[0].key, GroupKey::Shortlisted);
        // An empty task list yields two empty sections.
        let a = agenda(&[], &AgendaOptions::new(today)).unwrap();
        assert_eq!(
            a.sections.iter().map(|s| s.id).collect::<Vec<_>>(),
            [SectionId::DoFirst, SectionId::ContinueWith]
        );
        assert!(a.sections[0].groups.is_empty() && a.sections[1].groups.is_empty());
        assert!(a.completed.is_empty());
    }

    // Gör't — "completed today only, ascending by completion time".
    #[test]
    fn completed_today_only_ascending_by_completion_time() {
        let today = d(TODAY);
        let tasks: Vec<Task> = vec![
            task!("open", status => TaskStatus::Open),
            task!(
                "late",
                status => TaskStatus::Completed,
                completed_at => Some(format!("{TODAY}T12:00:00.000Z"))
            ),
            task!(
                "yesterday",
                status => TaskStatus::Completed,
                completed_at => Some("2025-03-11T09:00:00.000Z".to_owned())
            ),
            task!(
                "early",
                status => TaskStatus::Completed,
                completed_at => Some(format!("{TODAY}T08:00:00.000Z"))
            ),
            task!(
                "mid",
                status => TaskStatus::Completed,
                completed_at => Some(format!("{TODAY}T10:00:00.000Z"))
            ),
        ];
        let a = agenda(&tasks, &AgendaOptions::new(today)).unwrap();
        assert_eq!(
            a.completed.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(),
            ["early", "mid", "late"]
        );
        // The open task matches no group.
        assert!(all_group_tasks(&a).is_empty());
    }

    // Gör't — "order is a parameter: custom orders and break positions are honored".
    #[test]
    fn order_is_a_parameter_custom_orders_and_break_positions_are_honored() {
        let today = d(TODAY);
        let tasks: Vec<Task> = vec![
            task!("pick-due", focus_date => Some(today), due_date => Some(today)),
            task!("starred", importance => Importance::High, due_date => Some(today)),
            task!("overdue", due_date => Some(d("2025-03-01"))),
        ];
        let opts = AgendaOptions {
            order: vec![
                "due-today".into(),
                SECTION_BREAK.into(),
                "shortlisted".into(),
                "overdue".into(),
                "starred-due".into(),
            ],
            ..AgendaOptions::new(today)
        };
        let a = agenda(&tasks, &opts).unwrap();
        assert_eq!(
            a.sections
                .iter()
                .map(|s| s.groups.iter().map(|g| g.key.as_token()).collect::<Vec<_>>())
                .collect::<Vec<_>>(),
            vec![vec!["due-today"], vec!["overdue"]]
        );
        assert_eq!(group_ids(&a, SectionId::DoFirst, GroupKey::DueToday), ["pick-due", "starred"]);
        // shortlisted is empty (its only candidate was taken) and omitted.
        assert_eq!(
            group_ids(&a, SectionId::ContinueWith, GroupKey::Shortlisted),
            Vec::<&str>::new()
        );
        // An order without a break yields one section.
        let opts = AgendaOptions {
            order: vec!["overdue".into(), "shortlisted".into()],
            ..AgendaOptions::new(today)
        };
        let a = agenda(&tasks, &opts).unwrap();
        assert_eq!(a.sections.len(), 1);
        assert_eq!(a.sections[0].id, SectionId::DoFirst);
    }

    // Gör't — "focus mode: daily picks expire at midnight; durable keeps them".
    #[test]
    fn focus_mode_daily_picks_expire_at_midnight_durable_keeps_them() {
        let today = d(TODAY);
        let tasks: Vec<Task> = vec![
            task!("today-pick", focus_date => Some(today)),
            task!("old-pick", focus_date => Some(d("2025-03-10"))),
        ];
        // Daily is the default.
        let a = agenda(&tasks, &AgendaOptions::new(today)).unwrap();
        assert_eq!(group_ids(&a, SectionId::ContinueWith, GroupKey::Shortlisted), ["today-pick"]);
        // Explicit daily agrees; durable keeps old picks.
        let opts = AgendaOptions { focus_mode: FocusMode::Daily, ..AgendaOptions::new(today) };
        assert_eq!(
            group_ids(&agenda(&tasks, &opts).unwrap(), SectionId::ContinueWith, GroupKey::Shortlisted),
            ["today-pick"]
        );
        let opts = AgendaOptions { focus_mode: FocusMode::Durable, ..AgendaOptions::new(today) };
        assert_eq!(
            group_ids(&agenda(&tasks, &opts).unwrap(), SectionId::ContinueWith, GroupKey::Shortlisted),
            ["today-pick", "old-pick"]
        );
    }

    // Gör't — "radar group: important undated tasks, below due today (#98)".
    #[test]
    fn radar_group_important_undated_tasks_below_due_today() {
        let today = d(TODAY);
        let tasks: Vec<Task> = vec![
            task!("on-radar", importance => Importance::High),
            task!("starred-dated", importance => Importance::High, due_date => Some(today)),
            task!("normal-undated"),
        ];
        let a = agenda(&tasks, &AgendaOptions::new(today)).unwrap();
        assert_eq!(group_ids(&a, SectionId::ContinueWith, GroupKey::DueToday), Vec::<&str>::new());
        assert_eq!(group_ids(&a, SectionId::ContinueWith, GroupKey::Radar), ["on-radar"]);
        assert_eq!(group_ids(&a, SectionId::DoFirst, GroupKey::StarredDue), ["starred-dated"]);
    }

    // Gör't — "radar dedup: a high+undated pick goes to shortlisted first;
    // reordering lets radar win".
    #[test]
    fn radar_dedup_shortlisted_wins_by_default_reorder_lets_radar_win() {
        let today = d(TODAY);
        let tasks: Vec<Task> =
            vec![task!("pick-radar", importance => Importance::High, focus_date => Some(today))];
        // Default order: shortlisted (before radar) wins.
        let a = agenda(&tasks, &AgendaOptions::new(today)).unwrap();
        assert_eq!(group_ids(&a, SectionId::ContinueWith, GroupKey::Shortlisted), ["pick-radar"]);
        assert_eq!(group_ids(&a, SectionId::ContinueWith, GroupKey::Radar), Vec::<&str>::new());
        // Reordered: radar first.
        let opts = AgendaOptions {
            order: vec![SECTION_BREAK.into(), "radar".into(), "shortlisted".into()],
            ..AgendaOptions::new(today)
        };
        let a = agenda(&tasks, &opts).unwrap();
        assert_eq!(group_ids(&a, SectionId::DoFirst, GroupKey::Radar), ["pick-radar"]);
        assert_eq!(group_ids(&a, SectionId::DoFirst, GroupKey::Shortlisted), Vec::<&str>::new());
    }

    // Gör't — "unknown group keys are refused; input is not mutated".
    #[test]
    fn unknown_group_keys_are_refused_input_is_not_mutated() {
        let today = d(TODAY);
        let tasks: Vec<Task> = vec![task!("a"), task!("b")];
        let opts = AgendaOptions {
            order: vec!["bogus".into(), SECTION_BREAK.into()],
            ..AgendaOptions::new(today)
        };
        let err = agenda(&tasks, &opts).unwrap_err();
        assert_eq!(err.to_string(), "unknown agenda group: bogus");
        // Input is not mutated.
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].id, "a");
    }
}
