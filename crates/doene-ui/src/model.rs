//! Plain view-model logic: no egui types, fully unit-testable.

use doene_core::labels::due_label;
use doene_core::{DEFAULT_SORT, Date, HydratedTask, Importance, SmartList, SmartSettings, TaskStatus};
use doene_core::{smart_tasks, smartlists::sort_tasks};

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
}
