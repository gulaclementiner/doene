//! Task search scoring — a port of Gör't `src/search.js`.
//!
//! Every query word must match (case-insensitive) in the subject, notes or
//! steps; subject hits rank highest. Results sort by score desc, then open
//! before completed, then subject asc (Gör't uses `localeCompare`; plain
//! string order is identical for the fixture alphabets).

use crate::entities::{HydratedTask, TaskStatus};

/// Per-word score tiers (Gör't's `score`): 6 whole subject word, 5 subject
/// prefix, 4 subject token prefix, 3 subject substring, 2 step substring,
/// 1 body substring. `words` must be lowercased; a task that scores 0 for
/// any word scores 0 overall (all words are required).
pub fn score(task: &HydratedTask, words: &[String]) -> i64 {
    let subj = task.task.subject.to_lowercase();
    let body = task.task.body.as_deref().unwrap_or("").to_lowercase();
    let steps: Vec<String> = task.steps.iter().map(|s| s.subject.to_lowercase()).collect();

    let mut total = 0i64;
    for w in words {
        // JS `/\W+/` splits on runs of non-word chars (word = [A-Za-z0-9_]);
        // each tier re-splits, as in Gör't.
        let s = if subj.split(non_word).any(|t| t == w.as_str()) {
            6
        } else if subj.starts_with(w) {
            5
        } else if subj.split(non_word).any(|t| t.starts_with(w)) {
            4
        } else if subj.contains(w.as_str()) {
            3
        } else if steps.iter().any(|x| x.contains(w.as_str())) {
            2
        } else if body.contains(w.as_str()) {
            1
        } else {
            return 0;
        };
        total += s;
    }
    total
}

fn non_word(c: char) -> bool {
    !(c.is_ascii_alphanumeric() || c == '_')
}

/// Search (Gör't's `searchTasks`): every query word must match somewhere in
/// subject/steps/body; results sort by score desc, then open before
/// completed, then subject asc; at most `limit` tasks. Empty or blank
/// queries return nothing.
pub fn search_tasks<'a>(tasks: &'a [HydratedTask], q: &str, limit: usize) -> Vec<&'a HydratedTask> {
    let words = q.to_lowercase().split_whitespace().map(str::to_owned).collect::<Vec<_>>();
    if words.is_empty() {
        return vec![];
    }

    let mut scored: Vec<(&HydratedTask, i64)> = tasks
        .iter()
        .map(|t| (t, score(t, &words)))
        .filter(|(_, s)| *s > 0)
        .collect();
    scored.sort_by(|a, b| {
        b.1.cmp(&a.1).then_with(|| open_rank(a.0).cmp(&open_rank(b.0))).then_with(|| {
            a.0.task
                .subject
                .to_lowercase()
                .cmp(&b.0.task.subject.to_lowercase())
        })
    });
    scored.into_iter().take(limit).map(|(t, _)| t).collect()
}

/// Open tasks rank before completed ones (Gör't's `(a.status === 'completed') - …`).
fn open_rank(t: &HydratedTask) -> u8 {
    if t.task.status == TaskStatus::Completed { 1 } else { 0 }
}
