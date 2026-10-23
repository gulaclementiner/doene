//! Notification scheduling rules (port of Gör't `public/notify.js`): pure, clock injected.
//!
//! `now_local` is local wall-clock time `YYYY-MM-DDTHH:MM` (reminders carry no timezone).

use std::collections::HashSet;

use crate::dates::Date;
use crate::entities::{HydratedTask, TaskStatus};
use crate::i18n::tr;

/// Reminders older than this are skipped, so a long-closed app does not spam.
pub const GRACE_MINUTES: i64 = 24 * 60;
/// Default `dueNotifyTime` setting.
pub const DEFAULT_DUE_NOTIFY_TIME: &str = "09:00";
/// Fired keys kept after pruning.
pub const FIRED_KEEP: usize = 500;

/// A notification to deliver; `key` is the dedupe key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    pub key: String,
    pub title: String,
    pub body: String,
}

/// Minutes since epoch of a `YYYY-MM-DDTHH:MM` string (seconds/zone suffix ignored).
fn minutes(s: &str) -> Option<i64> {
    let date = Date::parse(s.get(..10)?).ok()?;
    let h: i64 = s.get(11..13)?.parse().ok()?;
    let m: i64 = s.get(14..16)?.parse().ok()?;
    Some(date.days_since_epoch() * 1440 + h * 60 + m)
}

/// Notifications due at `now_local` that are not in `fired`.
///
/// Reminders fire once when `reminder_at <= now` within [`GRACE_MINUTES`]; due-today
/// ones fire once per day at/after `due_notify_time` (`HH:MM`). Completed tasks never fire.
pub fn due_notifications(
    tasks: &[HydratedTask],
    now_local: &str,
    fired: &HashSet<String>,
    due_notify_time: &str,
    locale: &str,
) -> Vec<Notification> {
    let (Some(now), Some(day)) = (minutes(now_local), now_local.get(..10)) else {
        return vec![];
    };
    let time_reached = now_local.get(11..16).is_some_and(|t| t >= due_notify_time);
    let none = Default::default();
    let mut out = vec![];
    for h in tasks {
        let t = &h.task;
        if t.status == TaskStatus::Completed {
            continue;
        }
        if let Some(r) = t.reminder_at.as_deref().filter(|r| r.len() >= 16) {
            let key = format!("r:{}:{r}", t.id);
            if let Some(rm) = minutes(r)
                && rm <= now
                && now - rm <= GRACE_MINUTES
                && !fired.contains(&key)
            {
                out.push(Notification {
                    key,
                    title: t.subject.clone(),
                    body: tr("notify.reminder", &none, Some(locale)),
                });
            }
        }
        if time_reached && t.due_date.is_some_and(|d| d.to_string() == day) {
            let key = format!("d:{}:{day}", t.id);
            if !fired.contains(&key) {
                out.push(Notification {
                    key,
                    title: t.subject.clone(),
                    body: tr("notify.dueToday", &none, Some(locale)),
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::{Importance, Task};

    fn task(id: &str, due: Option<&str>, rem: Option<&str>, done: bool) -> HydratedTask {
        HydratedTask::bare(Task {
            id: id.into(),
            subject: format!("S{id}"),
            status: if done {
                TaskStatus::Completed
            } else {
                TaskStatus::Open
            },
            importance: Importance::Normal,
            due_date: due.map(|d| Date::parse(d).unwrap()),
            reminder_at: rem.map(Into::into),
            body: None,
            focus_date: None,
            postponed_date: None,
            created_at: String::new(),
            completed_at: None,
            position: 1.0,
            spawned_from: None,
            parent_id: None,
            recurrence: None,
        })
    }
    fn run(tasks: &[HydratedTask], now: &str, fired: &[&str]) -> Vec<String> {
        let f = fired.iter().map(|s| s.to_string()).collect();
        due_notifications(tasks, now, &f, "09:00", "en")
            .into_iter()
            .map(|n| n.key)
            .collect()
    }

    #[test]
    fn reminder_fires_once_due_and_within_grace() {
        let t = [task("a", None, Some("2025-03-10T08:30"), false)];
        assert!(run(&t, "2025-03-10T08:29", &[]).is_empty());
        assert_eq!(run(&t, "2025-03-10T08:30", &[]), ["r:a:2025-03-10T08:30"]);
        assert_eq!(run(&t, "2025-03-11T08:30", &[]).len(), 1);
        assert!(run(&t, "2025-03-11T08:31", &[]).is_empty());
        assert!(run(&t, "2025-03-10T09:00", &["r:a:2025-03-10T08:30"]).is_empty());
    }

    #[test]
    fn due_today_fires_at_or_after_configured_time_once() {
        let t = [task("a", Some("2025-03-10"), None, false)];
        assert!(run(&t, "2025-03-10T08:59", &[]).is_empty());
        assert_eq!(run(&t, "2025-03-10T09:00", &[]), ["d:a:2025-03-10"]);
        assert_eq!(run(&t, "2025-03-10T23:00", &[]).len(), 1);
        assert!(run(&t, "2025-03-10T23:00", &["d:a:2025-03-10"]).is_empty());
        assert!(run(&t, "2025-03-11T10:00", &[]).is_empty());
    }

    #[test]
    fn completed_tasks_never_fire_and_bodies_are_localised() {
        let t = [task(
            "a",
            Some("2025-03-10"),
            Some("2025-03-10T08:00"),
            true,
        )];
        assert!(run(&t, "2025-03-10T10:00", &[]).is_empty());
        let t = [task("a", Some("2025-03-10"), None, false)];
        let n = due_notifications(&t, "2025-03-10T10:00", &Default::default(), "09:00", "sv");
        assert_eq!(
            (n[0].title.as_str(), n[0].body.as_str()),
            ("Sa", "Förfaller idag")
        );
    }
}
