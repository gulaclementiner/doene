//! System (smart) lists (Gör't `src/smartlists.js`): membership rules for the
//! six system lists and the list-preference sort.

use std::cmp::Ordering;

use crate::agenda::{
    AgendaOptions, DEFAULT_SORT, FocusMode, SortDirection, SortOrder, SortSpec, agenda_open_tasks,
    is_picked,
};
use crate::agenda_order::default_order;
use crate::dates::Date;
use crate::entities::{Importance, Task, TaskStatus};
use crate::errors::CodedError;

/// The six system lists, in Gör't's `SMART` order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmartList {
    MyDay,
    Shortlisted,
    Important,
    Planned,
    All,
    Completed,
}

/// All system lists in display order.
pub const SMART: [SmartList; 6] = [
    SmartList::MyDay,
    SmartList::Shortlisted,
    SmartList::Important,
    SmartList::Planned,
    SmartList::All,
    SmartList::Completed,
];

impl SmartList {
    /// Gör't's list id: `"my-day"`, `"shortlisted"`, ...
    pub fn id(self) -> &'static str {
        match self {
            Self::MyDay => "my-day",
            Self::Shortlisted => "shortlisted",
            Self::Important => "important",
            Self::Planned => "planned",
            Self::All => "all",
            Self::Completed => "completed",
        }
    }

    /// Parse a list id; an unknown id is `error.unknownSmartList` with `id`.
    pub fn parse(id: &str) -> Result<Self, CodedError> {
        SMART
            .iter()
            .copied()
            .find(|l| l.id() == id)
            .ok_or_else(|| CodedError {
                code: "error.unknownSmartList",
                message: format!("unknown smart list {id}"),
                params: vec![("id", id.to_owned())],
            })
    }
}

/// The settings smart lists read (Gör't's `settings`): focus mode and radar (#98).
#[derive(Debug, Clone, Copy, Default)]
pub struct SmartSettings {
    pub focus_mode: FocusMode,
    pub my_day_radar: bool,
}

/// Radar visibility (#98): when the setting is off, the `radar` key is
/// dropped from the visible order.
pub fn visible_order(order: Vec<String>, settings: SmartSettings) -> Vec<String> {
    if settings.my_day_radar {
        order
    } else {
        order.into_iter().filter(|k| k != "radar").collect()
    }
}

fn open(t: &Task) -> bool {
    t.status != TaskStatus::Completed
}

/// Tasks of a system list. `today` is the focus date for midnight expiry.
pub fn smart_tasks(
    list: SmartList,
    tasks: &[Task],
    today: Date,
    settings: SmartSettings,
) -> Vec<&Task> {
    match list {
        SmartList::Shortlisted => tasks
            .iter()
            .filter(|t| open(t) && is_picked(t, today, settings.focus_mode))
            .collect(),
        SmartList::MyDay => {
            let opts = AgendaOptions {
                today,
                order: visible_order(default_order(), settings),
                sort: DEFAULT_SORT,
                focus_mode: settings.focus_mode,
            };
            agenda_open_tasks(tasks, &opts).expect("the default order has only known keys")
        }
        SmartList::Important => tasks
            .iter()
            .filter(|t| t.importance == Importance::High)
            .collect(),
        SmartList::Planned => tasks.iter().filter(|t| t.due_date.is_some()).collect(),
        SmartList::All => tasks.iter().collect(),
        SmartList::Completed => tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Completed)
            .collect(),
    }
}

/// Sort per list preferences (Gör't's `sortTasks`): stable; `important_to_top`
/// then floats high-importance tasks above the rest, keeping their order.
pub fn sort_tasks<'a>(tasks: &[&'a Task], sort: SortSpec, important_to_top: bool) -> Vec<&'a Task> {
    fn rank(i: Importance) -> u8 {
        match i {
            Importance::High => 0,
            Importance::Normal => 1,
        }
    }
    let mut out: Vec<&'a Task> = tasks.to_vec();
    out.sort_by(|a, b| {
        let ord = match sort.order {
            SortOrder::Importance => rank(a.importance).cmp(&rank(b.importance)),
            // Undated tasks sort last ('9999').
            SortOrder::Due => match (a.due_date, b.due_date) {
                (Some(x), Some(y)) => x.cmp(&y),
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (None, None) => Ordering::Equal,
            },
            SortOrder::Alpha => a.subject.to_lowercase().cmp(&b.subject.to_lowercase()),
            SortOrder::Created => a.created_at.cmp(&b.created_at),
            SortOrder::Manual => a
                .position
                .partial_cmp(&b.position)
                .unwrap_or(Ordering::Equal),
        };
        match sort.direction {
            SortDirection::Asc => ord,
            SortDirection::Desc => ord.reverse(),
        }
    });
    if important_to_top {
        let (high, rest): (Vec<_>, Vec<_>) = out
            .into_iter()
            .partition(|t| t.importance == Importance::High);
        return high.into_iter().chain(rest).collect();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Date {
        Date::parse(s).unwrap()
    }

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

    fn ids<'a>(v: &[&'a Task]) -> Vec<&'a str> {
        v.iter().map(|t| t.id.as_str()).collect()
    }

    const NONE: SmartSettings = SmartSettings {
        focus_mode: FocusMode::Daily,
        my_day_radar: false,
    };

    // core.test.js — "My Day is the computed agenda; picks and due dates expire at midnight".
    #[test]
    fn my_day_picks_and_due_dates_expire_at_midnight() {
        let mut a = t("a");
        a.focus_date = Some(d("2025-03-12"));
        let mut b = t("b");
        b.due_date = Some(d("2025-03-12"));
        let mut c = t("c");
        c.due_date = Some(d("2025-03-13"));
        let tasks = vec![a, b, c];
        let mut day1 = ids(&smart_tasks(
            SmartList::MyDay,
            &tasks,
            d("2025-03-12"),
            NONE,
        ));
        day1.sort();
        assert_eq!(day1, ["a", "b"]);
        // Midnight: a's pick is gone, b is overdue, c is due today.
        let day2 = smart_tasks(SmartList::MyDay, &tasks, d("2025-03-13"), NONE);
        assert_eq!(ids(&day2), ["b", "c"]);
        let sl = smart_tasks(SmartList::Shortlisted, &tasks, d("2025-03-13"), NONE);
        assert!(sl.is_empty());
    }

    // core.test.js — "focus mode drives the Shortlisted list and My Day picks (#98)".
    #[test]
    fn focus_mode_drives_shortlisted_and_my_day() {
        let mut old = t("old-pick");
        old.focus_date = Some(d("2025-03-10"));
        let tasks = vec![old];
        let today = d("2025-03-12");
        let durable = SmartSettings {
            focus_mode: FocusMode::Durable,
            my_day_radar: false,
        };
        assert!(smart_tasks(SmartList::Shortlisted, &tasks, today, NONE).is_empty());
        assert_eq!(
            ids(&smart_tasks(SmartList::Shortlisted, &tasks, today, durable)),
            ["old-pick"]
        );
        assert!(smart_tasks(SmartList::MyDay, &tasks, today, NONE).is_empty());
        assert_eq!(
            ids(&smart_tasks(SmartList::MyDay, &tasks, today, durable)),
            ["old-pick"]
        );
    }

    // core.test.js — "radar: ... only when the setting is on (#98)".
    #[test]
    fn radar_joins_my_day_only_when_setting_on() {
        let mut radar = t("on-radar");
        radar.importance = Importance::High;
        let mut due = t("due-now");
        due.due_date = Some(d("2025-03-12"));
        let tasks = vec![radar, due];
        let today = d("2025-03-12");
        assert_eq!(
            ids(&smart_tasks(SmartList::MyDay, &tasks, today, NONE)),
            ["due-now"]
        );
        let on = SmartSettings {
            focus_mode: FocusMode::Daily,
            my_day_radar: true,
        };
        assert_eq!(
            ids(&smart_tasks(SmartList::MyDay, &tasks, today, on)),
            ["due-now", "on-radar"]
        );
    }

    // core.test.js — "important / planned / completed / all".
    #[test]
    fn important_planned_completed_all() {
        let mut imp = t("imp");
        imp.importance = Importance::High;
        imp.status = TaskStatus::Completed;
        let mut due = t("due");
        due.due_date = Some(d("2025-03-20"));
        let tasks = vec![imp, due];
        let today = d("2025-03-12");
        let get = |l| smart_tasks(l, &tasks, today, NONE);
        assert_eq!(
            ids(&get(SmartList::Important)),
            ["imp"],
            "closed tasks stay in Important"
        );
        assert_eq!(ids(&get(SmartList::Completed)), ["imp"]);
        assert_eq!(ids(&get(SmartList::Planned)), ["due"]);
        assert_eq!(
            get(SmartList::All).len(),
            2,
            "All holds open and closed tasks"
        );
        // Planned keeps completed dated tasks too.
        let mut done = t("done");
        done.due_date = Some(d("2025-03-01"));
        done.status = TaskStatus::Completed;
        let tasks = vec![done];
        assert_eq!(
            ids(&smart_tasks(SmartList::Planned, &tasks, today, NONE)),
            ["done"]
        );
    }

    // module-error-codes.test.js — "unknown smart list carries code and id".
    #[test]
    fn unknown_smart_list_carries_code_and_id() {
        let e = SmartList::parse("nope").unwrap_err();
        assert_eq!(e.code, "error.unknownSmartList");
        assert_eq!(e.param("id"), Some("nope"));
        for l in SMART {
            assert_eq!(SmartList::parse(l.id()).unwrap(), l);
        }
    }

    // core.test.js — "sorting".
    #[test]
    fn sorting() {
        let mut b = t("b");
        b.position = 1.0;
        let mut a = t("A");
        a.importance = Importance::High;
        a.position = 2.0;
        let tasks = [b, a];
        let refs: Vec<&Task> = tasks.iter().collect();
        let s = |order, direction, top| sort_tasks(&refs, SortSpec { order, direction }, top);
        assert_eq!(
            ids(&s(SortOrder::Alpha, SortDirection::Asc, false)),
            ["A", "b"]
        );
        assert_eq!(
            ids(&s(SortOrder::Alpha, SortDirection::Desc, false)),
            ["b", "A"]
        );
        assert_eq!(
            ids(&s(SortOrder::Manual, SortDirection::Asc, true)),
            ["A", "b"]
        );
        assert_eq!(
            ids(&s(SortOrder::Importance, SortDirection::Asc, false)),
            ["A", "b"]
        );
        // Undated last under due-asc; stable ties.
        let mut x = t("x");
        x.due_date = Some(d("2025-03-02"));
        let y = t("y");
        let mut z = t("z");
        z.due_date = Some(d("2025-03-01"));
        let ts = [y, x, z];
        let r: Vec<&Task> = ts.iter().collect();
        assert_eq!(ids(&sort_tasks(&r, DEFAULT_SORT, false)), ["z", "x", "y"]);
    }
}
