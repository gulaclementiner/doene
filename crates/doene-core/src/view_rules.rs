//! Custom-view rules shared across the seam — a port of Gör't
//! `src/view-rules.js` (`normaliseView`, `viewMatches`, `sortTasks`).

use std::cmp::Ordering;

use crate::entities::{HydratedTask, MatchMode, Tag, TaskStatus};
use crate::errors::CodedError;

/// The sort fields a view can order by, in picker order.
pub const SORT_KEYS: [&str; 4] = ["name", "due", "created", "manual"];

/// Most chained sort levels a view may have.
pub const MAX_SORTS: usize = 6;

/// A view's sort field (Gör't's `SORT_KEYS` tokens).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    Name,
    Due,
    Created,
    Manual,
}

impl SortKey {
    /// Parse a Gör't token; `None` for unknown keys.
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "name" => Some(Self::Name),
            "due" => Some(Self::Due),
            "created" => Some(Self::Created),
            "manual" => Some(Self::Manual),
            _ => None,
        }
    }
}

/// A view's sort direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Asc,
    Desc,
}

/// One level of a view's multi-level sort.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SortLevel {
    pub by: SortKey,
    pub dir: SortDirection,
}

/// Raw (unvalidated) view shape, as stored in `views.data` JSON.
#[derive(Debug, Clone, Default)]
pub struct RawView {
    pub name: Option<String>,
    pub emoji: Option<String>,
    /// Tag words; trimmed and deduped on normalisation.
    pub tags: Vec<String>,
    /// `"any"` | `"all"`; defaults to any.
    pub match_mode: Option<String>,
    pub sorts: Vec<RawSortLevel>,
    pub show_done: bool,
}

/// A raw sort level (`by` token + optional direction).
#[derive(Debug, Clone)]
pub struct RawSortLevel {
    pub by: String,
    pub dir: Option<String>,
}

/// A normalised view rule (Gör't's `normaliseView` return value).
#[derive(Debug, Clone, PartialEq)]
pub struct ViewRules {
    pub name: String,
    pub emoji: Option<String>,
    /// Trimmed, deduped tag words; case preserved — matching is
    /// case-insensitive.
    pub tags: Vec<String>,
    pub match_mode: MatchMode,
    pub sorts: Vec<SortLevel>,
    pub show_done: bool,
}

/// Validate a raw view into rules (Gör't's `normaliseView`): trims the name
/// (required), defaults match to any, validates sort keys, forces manual
/// direction asc, and trims + dedupes tags.
pub fn normalise_view(v: &RawView) -> Result<ViewRules, CodedError> {
    let name = v.name.as_deref().unwrap_or("").trim().to_owned();
    if name.is_empty() {
        return Err(CodedError::new(
            "error.viewNameRequired",
            "view name required",
        ));
    }

    let match_mode = match v.match_mode.as_deref().unwrap_or("any") {
        "any" => MatchMode::Any,
        "all" => MatchMode::All,
        other => {
            return Err(CodedError::new(
                "error.viewMatchInvalid",
                format!("match must be any or all (got {other})"),
            ));
        }
    };

    if v.sorts.len() > MAX_SORTS {
        return Err(CodedError {
            code: "error.viewSortTooMany",
            message: format!("at most {MAX_SORTS} sort levels"),
            params: vec![("max", MAX_SORTS.to_string())],
        });
    }

    let mut sorts = Vec::with_capacity(v.sorts.len());
    for s in &v.sorts {
        let by = SortKey::from_token(&s.by).ok_or_else(|| CodedError {
            code: "error.viewSortInvalidKey",
            message: format!("unknown sort {}", s.by),
            params: vec![("by", s.by.clone())],
        })?;
        // Manual order is always ascending (Gör't forces it).
        let dir = if by == SortKey::Manual {
            SortDirection::Asc
        } else if s.dir.as_deref() == Some("desc") {
            SortDirection::Desc
        } else {
            SortDirection::Asc
        };
        sorts.push(SortLevel { by, dir });
    }

    let mut tags: Vec<String> = Vec::new();
    for tag in &v.tags {
        let t = tag.trim().to_owned();
        if !t.is_empty() && !tags.contains(&t) {
            tags.push(t);
        }
    }

    Ok(ViewRules {
        name,
        emoji: v.emoji.clone(),
        tags,
        match_mode,
        sorts,
        show_done: v.show_done,
    })
}

/// Whether a task shows in the view (Gör't's `viewMatches`): open tasks
/// always, completed only with `showDone`; then the tag rule.
pub fn view_matches(view: &ViewRules, task: &HydratedTask) -> bool {
    if !view.show_done && task.task.status == TaskStatus::Completed {
        return false;
    }
    matches_tags(&task.tags, &view.tags, view.match_mode)
}

/// The tag rule (Gör't's `matchesTags`): no tags means no filter; `any` keeps
/// a task with at least one of the view's tags, `all` needs every one.
/// Case-insensitive — tags are normalised to lowercase on write.
fn matches_tags(have: &[Tag], want: &[String], match_mode: MatchMode) -> bool {
    if want.is_empty() {
        return true;
    }
    let have: std::collections::HashSet<String> =
        have.iter().map(|t| t.as_str().to_lowercase()).collect();
    let has = |g: &String| have.contains(&g.to_lowercase());
    match match_mode {
        MatchMode::All => want.iter().all(has),
        MatchMode::Any => want.iter().any(has),
    }
}

/// The tasks in the view's sort order (Gör't's `sortTasks`): chained levels,
/// ties from an earlier level fall to the next. Stable — equal keys keep
/// their relative input order; no levels keeps the input order.
pub fn sort_tasks<'a>(tasks: &'a [HydratedTask], sorts: &[SortLevel]) -> Vec<&'a HydratedTask> {
    let mut out: Vec<&'a HydratedTask> = tasks.iter().collect();
    // Stable multi-key sort: apply the lowest-priority level first.
    for level in sorts.iter().rev() {
        let dir = level.dir;
        out.sort_by(|a, b| {
            let o = cmp_level(a, b, level.by);
            if dir == SortDirection::Desc {
                o.reverse()
            } else {
                o
            }
        });
    }
    out
}

fn cmp_level(a: &HydratedTask, b: &HydratedTask, by: SortKey) -> Ordering {
    match by {
        SortKey::Name => a
            .task
            .subject
            .to_lowercase()
            .cmp(&b.task.subject.to_lowercase()),
        // Undated sorts last (Gör't's `'9999-12-31'` sentinel).
        SortKey::Due => match (a.task.due_date, b.task.due_date) {
            (Some(x), Some(y)) => x.cmp(&y),
            (None, None) => Ordering::Equal,
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
        },
        SortKey::Created => a.task.created_at.cmp(&b.task.created_at),
        // NaN positions compare equal and fall to the next level.
        SortKey::Manual => a
            .task
            .position
            .partial_cmp(&b.task.position)
            .unwrap_or(Ordering::Equal),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dates::Date;
    use crate::entities::{Importance, Task};

    fn task(id: &str, due: Option<&str>, created: &str, position: f64) -> HydratedTask {
        let mut t = Task {
            id: id.to_owned(),
            subject: id.to_owned(),
            status: TaskStatus::Open,
            importance: Importance::Normal,
            due_date: None,
            reminder_at: None,
            body: None,
            focus_date: None,
            postponed_date: None,
            created_at: created.to_owned(),
            completed_at: None,
            position,
            spawned_from: None,
            parent_id: None,
            recurrence: None,
        };
        if let Some(d) = due {
            t.due_date = Some(Date::parse(d).expect("valid date"));
        }
        HydratedTask::bare(t)
    }

    #[test]
    fn due_level_undated_sorts_last() {
        let dated = task("a", Some("2025-01-01"), "2025-01-01T00:00:00.000Z", 0.0);
        let undated = task("b", None, "2025-01-01T00:00:00.000Z", 0.0);
        assert_eq!(cmp_level(&dated, &undated, SortKey::Due), Ordering::Less);
        assert_eq!(cmp_level(&undated, &dated, SortKey::Due), Ordering::Greater);
    }

    #[test]
    fn sort_key_tokens_round_trip() {
        for token in SORT_KEYS {
            let key = SortKey::from_token(token).expect("known key");
            assert_eq!(
                SortKey::from_token(match key {
                    SortKey::Name => "name",
                    SortKey::Due => "due",
                    SortKey::Created => "created",
                    SortKey::Manual => "manual",
                }),
                Some(key)
            );
        }
        assert_eq!(SortKey::from_token("importance"), None);
    }
}
