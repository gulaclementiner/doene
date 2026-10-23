//! Plain view-model logic: no egui types, fully unit-testable.

use doene_core::labels::due_label;
use doene_core::{DEFAULT_SORT, Date, HydratedTask, Importance, SmartList, SmartSettings, TaskStatus};
use doene_core::{smart_tasks, smartlists::sort_tasks};
use doene_service::{Service, ServiceError, TaskPatch};

/// The user's theme choice (persisted as the `theme` setting).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Light,
    Dark,
    System,
}

impl Theme {
    pub const ALL: [Theme; 3] = [Theme::Light, Theme::Dark, Theme::System];

    /// The persisted setting value.
    pub fn id(self) -> &'static str {
        match self {
            Theme::Light => "light",
            Theme::Dark => "dark",
            Theme::System => "system",
        }
    }

    /// Parse a persisted value; anything unknown is `System`.
    pub fn parse(s: &str) -> Theme {
        match s {
            "light" => Theme::Light,
            "dark" => Theme::Dark,
            _ => Theme::System,
        }
    }

    /// Locale key of the theme's label.
    pub fn label_key(self) -> &'static str {
        match self {
            Theme::Light => "settings.theme.light",
            Theme::Dark => "settings.theme.dark",
            Theme::System => "settings.theme.system",
        }
    }
}

/// One rendered task row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRow {
    pub id: String,
    pub subject: String,
    pub completed: bool,
    pub important: bool,
    /// Localised due label ("Today", "12 Mar"), if dated.
    pub due: Option<String>,
    pub overdue: bool,
    /// Tags with a leading `#`.
    pub tags: Vec<String>,
}

/// Rows of a system list in core sort order (due ascending, undated last).
pub fn list_rows(
    list: SmartList,
    tasks: &[HydratedTask],
    today: Date,
    settings: SmartSettings,
    locale: &str,
) -> Vec<TaskRow> {
    let bare: Vec<_> = tasks.iter().map(|h| h.task.clone()).collect();
    let picked = smart_tasks(list, &bare, today, settings);
    let sorted = sort_tasks(&picked, DEFAULT_SORT, false);
    let today_s = today.to_string();
    sorted
        .into_iter()
        .filter_map(|t| tasks.iter().find(|h| h.task.id == t.id))
        .map(|h| {
            let t = &h.task;
            TaskRow {
                id: t.id.clone(),
                subject: t.subject.clone(),
                completed: t.status == TaskStatus::Completed,
                important: t.importance == Importance::High,
                due: t.due_date.map(|d| due_label(&d.to_string(), &today_s, Some(locale))),
                overdue: t.due_date.is_some_and(|d| d < today) && t.status != TaskStatus::Completed,
                tags: h.tags.iter().map(|g| format!("#{}", g.as_str())).collect(),
            }
        })
        .collect()
}

/// What a completion toggle did, for the caller's feedback (toast in #12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Toggled {
    /// Completed; a repeating task spawned its next occurrence due at the label.
    Completed { next_due: Option<String> },
    /// Reopened (any spawned occurrence was removed by the service).
    Reopened,
}

/// Flip a task between open and completed through the service (which logs the
/// activity, spawns/removes the next occurrence and cascades to descendants).
pub fn toggle_complete(
    service: &Service,
    id: &str,
    completed: bool,
    locale: &str,
) -> Result<Toggled, ServiceError> {
    let status = if completed { TaskStatus::Open } else { TaskStatus::Completed };
    let out = service.update_task(id, TaskPatch { status: Some(status), ..Default::default() })?;
    if completed {
        return Ok(Toggled::Reopened);
    }
    let today = service.today().to_string();
    let next_due = out
        .next
        .and_then(|n| n.task.due_date)
        .map(|d| due_label(&d.to_string(), &today, Some(locale)));
    Ok(Toggled::Completed { next_due })
}

#[cfg(test)]
mod tests {
    use super::*;
    use doene_core::Task;

    fn task(id: &str, subject: &str, due: Option<&str>, imp: Importance) -> HydratedTask {
        let mut t = sample(id, subject);
        t.due_date = due.map(|d| Date::parse(d).unwrap());
        t.importance = imp;
        HydratedTask::bare(t)
    }

    fn sample(id: &str, subject: &str) -> Task {
        Task {
            id: id.into(),
            subject: subject.into(),
            status: TaskStatus::Open,
            importance: Importance::Normal,
            due_date: None,
            reminder_at: None,
            body: None,
            focus_date: None,
            postponed_date: None,
            created_at: "2025-03-10T09:00:00.000Z".into(),
            completed_at: None,
            position: 1.0,
            spawned_from: None,
            parent_id: None,
            recurrence: None,
        }
    }

    fn rows(tasks: &[HydratedTask]) -> Vec<TaskRow> {
        let today = Date::parse("2025-03-10").unwrap();
        list_rows(SmartList::All, tasks, today, SmartSettings::default(), "en")
    }

    #[test]
    fn rows_follow_core_sort_due_ascending_undated_last() {
        let r = rows(&[
            task("a", "undated", None, Importance::Normal),
            task("b", "later", Some("2025-03-20"), Importance::Normal),
            task("c", "today", Some("2025-03-10"), Importance::High),
        ]);
        let order: Vec<_> = r.iter().map(|x| x.id.as_str()).collect();
        assert_eq!(order, ["c", "b", "a"]);
        assert_eq!(r[0].due.as_deref(), Some("Today"));
        assert!(r[0].important && !r[1].important);
    }

    #[test]
    fn overdue_and_tags_are_flagged() {
        let mut h = task("a", "x", Some("2025-03-01"), Importance::Normal);
        h.tags = vec![doene_core::Tag::new("home").unwrap()];
        let r = rows(&[h]);
        assert!(r[0].overdue);
        assert_eq!(r[0].tags, ["#home"]);
    }

    #[test]
    fn theme_round_trips_and_unknown_is_system() {
        for t in Theme::ALL {
            assert_eq!(Theme::parse(t.id()), t);
        }
        assert_eq!(Theme::parse("pink"), Theme::System);
    }

    fn svc() -> (Service, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let storage = doene_storage::Storage::open(&dir.path().join("d.db")).unwrap();
        let clock = Box::new(|| "2025-03-10T09:00:00.000Z".to_owned());
        (Service::with_clock(storage, clock), dir)
    }

    #[test]
    fn toggling_recurring_task_spawns_then_reopen_restores() {
        let (s, _d) = svc();
        let t = s
            .quick_add("water plants", None, false)
            .unwrap();
        s.update_task(
            &t.task.id,
            TaskPatch {
                due_date: Some(Some(Date::parse("2025-03-10").unwrap())),
                recurrence: Some(Some(doene_core::recurrence::RawRecurrence {
                    kind: Some("daily".into()),
                    interval: None,
                    unit: None,
                    days: None
                })),
                ..Default::default()
            },
        )
        .unwrap();
        let r = toggle_complete(&s, &t.task.id, false, "en").unwrap();
        assert_eq!(r, Toggled::Completed { next_due: Some("Tomorrow".into()) });
        let all = s.all_tasks().unwrap();
        assert_eq!(all.len(), 2);
        let spawn = all.iter().find(|h| h.task.id != t.task.id).unwrap();
        assert_eq!(spawn.task.spawned_from.as_deref(), Some(t.task.id.as_str()));
        let acts: Vec<_> = s.activity(&t.task.id).unwrap().into_iter().map(|a| a.action).collect();
        assert!(acts.contains(&"completed".to_owned()));

        assert_eq!(toggle_complete(&s, &t.task.id, true, "en").unwrap(), Toggled::Reopened);
        let all = s.all_tasks().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].task.status, TaskStatus::Open);
        assert_eq!(all[0].task.completed_at, None);
    }

    #[test]
    fn toggling_plain_task_has_no_next() {
        let (s, _d) = svc();
        let t = s.quick_add("plain", None, false).unwrap();
        assert_eq!(
            toggle_complete(&s, &t.task.id, false, "en").unwrap(),
            Toggled::Completed { next_due: None }
        );
    }
}
