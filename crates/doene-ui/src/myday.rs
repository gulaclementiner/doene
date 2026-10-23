//! My Day view-model: the computed agenda as renderable sections, and the
//! persisted group-order layout (global pref `myDayGroupOrder`). No egui types.

use doene_core::agenda::{AgendaOptions, agenda};
use doene_core::{
    DEFAULT_SORT, Date, HydratedTask, SECTION_BREAK, SmartSettings, agenda_layout_save, agenda_order_for,
    visible_order,
};
use doene_service::{Service, ServiceError};

use crate::model::{TaskRow, task_row};

/// One rendered group.
#[derive(Debug, Clone, PartialEq)]
pub struct GroupRows {
    /// Locale key of the group heading.
    pub title_key: String,
    pub rows: Vec<TaskRow>,
}

/// One rendered section ("Do first" / "Continue with").
#[derive(Debug, Clone, PartialEq)]
pub struct SectionRows {
    pub title_key: &'static str,
    pub groups: Vec<GroupRows>,
}

/// The whole My Day view.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MyDayView {
    /// Sections with at least one task.
    pub sections: Vec<SectionRows>,
    /// Completed today, in completion order, pinned below the sections.
    pub completed: Vec<TaskRow>,
}

impl MyDayView {
    pub fn is_empty(&self) -> bool {
        self.sections.is_empty() && self.completed.is_empty()
    }
}

/// Compute the My Day view for a saved (full) group order.
pub fn my_day_view(
    tasks: &[HydratedTask],
    today: Date,
    settings: SmartSettings,
    order: &[String],
    locale: &str,
) -> MyDayView {
    let bare: Vec<_> = tasks.iter().map(|h| h.task.clone()).collect();
    let opts = AgendaOptions {
        today,
        order: visible_order(order.to_vec(), settings),
        sort: DEFAULT_SORT,
        focus_mode: settings.focus_mode,
    };
    let Ok(a) = agenda(&bare, &opts) else { return MyDayView::default() };
    let today_s = today.to_string();
    let row = |id: &str| {
        tasks.iter().find(|h| h.task.id == id).map(|h| task_row(h, today, &today_s, locale))
    };
    let sections = a
        .sections
        .iter()
        .filter(|s| !s.groups.is_empty())
        .map(|s| SectionRows {
            title_key: match s.id {
                doene_core::agenda::SectionId::DoFirst => "myday.doFirst",
                doene_core::agenda::SectionId::ContinueWith => "myday.continueWith",
            },
            groups: s
                .groups
                .iter()
                .map(|g| GroupRows {
                    title_key: format!("myday.group.{}", g.key.as_token()),
                    rows: g.tasks.iter().filter_map(|t| row(&t.id)).collect(),
                })
                .collect(),
        })
        .collect();
    let completed = a.completed.iter().filter_map(|t| row(&t.id)).collect();
    MyDayView { sections, completed }
}

/// The saved group order (global prefs), cleaned; the default when unsaved.
pub fn load_my_day_order(service: &Service) -> Vec<String> {
    let saved: Option<Vec<Option<String>>> = service
        .pref("global")
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_str::<serde_json::Value>(&v).ok())
        .and_then(|v| {
            v.get("myDayGroupOrder")?.as_array().map(|a| a.iter().map(|k| k.as_str().map(str::to_owned)).collect())
        });
    agenda_order_for(saved.as_deref())
}

/// Move the order item (group or break) at `index` by `delta` (clamped) and
/// persist; returns the new order.
pub fn move_my_day_item(service: &Service, index: usize, delta: i32) -> Result<Vec<String>, ServiceError> {
    let mut order = load_my_day_order(service);
    let to = (index as i32 + delta).clamp(0, order.len() as i32 - 1) as usize;
    if index < order.len() && to != index {
        let k = order.remove(index);
        order.insert(to, k);
        let keys: Vec<Option<String>> = order.into_iter().map(Some).collect();
        let save = agenda_layout_save(&keys);
        let value = serde_json::json!({ "myDayGroupOrder": save.my_day_group_order });
        service.set_pref(save.scope, &value.to_string())?;
        return Ok(save.my_day_group_order);
    }
    Ok(order)
}

/// Locale key of an order item in the layout editor.
pub fn order_item_key(key: &str) -> String {
    if key == SECTION_BREAK { "myday.break".into() } else { format!("myday.group.{key}") }
}

#[cfg(test)]
mod tests {
    use super::*;
    use doene_core::{Importance, Task, TaskStatus};
    use doene_core::default_order;
    use doene_storage::Storage;

    fn d(s: &str) -> Date {
        Date::parse(s).unwrap()
    }

    fn task(id: &str, due: Option<&str>, status: TaskStatus, done: Option<&str>) -> HydratedTask {
        HydratedTask {
            task: Task {
                id: id.into(),
                subject: id.into(),
                status,
                importance: Importance::Normal,
                due_date: due.map(d),
                reminder_at: None,
                body: None,
                focus_date: None,
                postponed_date: None,
                created_at: "2025-03-01T00:00:00.000Z".into(),
                completed_at: done.map(Into::into),
                position: 1.0,
                spawned_from: None,
                parent_id: None,
                recurrence: None,
            },
            tags: vec![],
            steps: vec![],
        }
    }

    const TODAY: &str = "2025-03-12";

    fn view(tasks: &[HydratedTask], order: &[String]) -> MyDayView {
        my_day_view(tasks, d(TODAY), SmartSettings::default(), order, "en")
    }

    #[test]
    fn groups_split_by_break_and_completed_pinned_in_completion_order() {
        let tasks = vec![
            task("late", Some("2025-03-10"), TaskStatus::Open, None),
            task("now", Some(TODAY), TaskStatus::Open, None),
            task("b", None, TaskStatus::Completed, Some("2025-03-12T15:00:00.000Z")),
            task("a", None, TaskStatus::Completed, Some("2025-03-12T08:00:00.000Z")),
        ];
        let v = view(&tasks, &default_order());
        assert_eq!(v.sections.len(), 2);
        assert_eq!(v.sections[0].title_key, "myday.doFirst");
        assert_eq!(v.sections[0].groups[0].title_key, "myday.group.overdue");
        assert_eq!(v.sections[1].title_key, "myday.continueWith");
        assert_eq!(v.sections[1].groups[0].rows[0].id, "now");
        let ids: Vec<_> = v.completed.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["a", "b"]);
    }

    #[test]
    fn moving_the_break_changes_sections() {
        let tasks = vec![
            task("late", Some("2025-03-10"), TaskStatus::Open, None),
            task("now", Some(TODAY), TaskStatus::Open, None),
        ];
        let order: Vec<String> = ["overdue", "starred-due", "shortlisted", "due-today", "break", "radar"]
            .map(String::from)
            .to_vec();
        let v = view(&tasks, &order);
        assert_eq!(v.sections.len(), 1);
        assert_eq!(v.sections[0].groups.len(), 2);
        assert_eq!(v.sections[0].title_key, "myday.doFirst");
    }

    #[test]
    fn midnight_expiry_drops_yesterdays_completions() {
        let tasks = vec![task("old", None, TaskStatus::Completed, Some("2025-03-11T23:59:00.000Z"))];
        assert!(view(&tasks, &default_order()).is_empty());
    }

    #[test]
    fn order_persists_and_break_is_movable() {
        let dir = tempfile::tempdir().unwrap();
        let storage = Storage::open(&dir.path().join("t.db")).unwrap();
        let svc = Service::with_clock(storage, Box::new(|| "2025-03-12T09:00:00.000Z".into()));
        assert_eq!(load_my_day_order(&svc), default_order());
        // The break sits at index 2; move it to the top.
        let moved = move_my_day_item(&svc, 2, -2).unwrap();
        assert_eq!(moved[0], "break");
        assert_eq!(load_my_day_order(&svc), moved);
        // Moving a group down persists too.
        let moved = move_my_day_item(&svc, 1, 1).unwrap();
        assert_eq!(load_my_day_order(&svc), moved);
        assert_eq!(moved.len(), 6);
    }
}
