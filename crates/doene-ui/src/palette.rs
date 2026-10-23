//! Ctrl+K command palette: plain entry-building and navigation logic.
//!
//! Order: system lists, views, commands (all filtered by case-insensitive
//! substring on their label), then tasks in the order given (core search
//! scoring, see `doene_core::search`).

use std::collections::HashMap;

use doene_core::{HydratedTask, SMART, SmartList, i18n::tr};
use doene_service::SavedView;

/// Maximum task results shown.
pub const TASK_LIMIT: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    NewTask,
    NewView,
    ToggleTheme,
}

/// What selecting an entry does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    OpenList(SmartList),
    OpenView(String),
    OpenTask(String),
    Run(Command),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub action: Action,
    pub label: String,
    /// Section heading key (`palette.*`), or None for lists.
    pub section: Option<&'static str>,
}

fn nav_key(l: SmartList) -> &'static str {
    match l {
        SmartList::MyDay => "nav.myDay",
        SmartList::Shortlisted => "nav.shortlisted",
        SmartList::Important => "nav.important",
        SmartList::Planned => "nav.planned",
        SmartList::All => "nav.all",
        SmartList::Completed => "nav.completed",
    }
}

fn matches(label: &str, q: &str) -> bool {
    q.is_empty() || label.to_lowercase().contains(q)
}

/// Build the palette entries for `query`. `tasks` are the already-ranked
/// search results for the same query. `dark` is the current effective theme.
pub fn build_entries(
    query: &str,
    views: &[SavedView],
    tasks: &[HydratedTask],
    dark: bool,
    locale: &str,
) -> Vec<Entry> {
    let q = query.trim().to_lowercase();
    let t = |k: &str| tr(k, &HashMap::new(), Some(locale));
    let go = |name: String| {
        tr(
            "palette.goTo",
            &HashMap::from([("name".to_owned(), Some(name))]),
            Some(locale),
        )
    };
    let mut out = vec![];
    for l in SMART {
        let name = t(nav_key(l));
        if matches(&name, &q) {
            out.push(Entry {
                action: Action::OpenList(l),
                label: go(name),
                section: None,
            });
        }
    }
    for v in views {
        if matches(&v.view.name, &q) {
            out.push(Entry {
                action: Action::OpenView(v.view.id.clone()),
                label: go(v.view.name.clone()),
                section: Some("palette.views"),
            });
        }
    }
    let theme = if dark {
        (Command::ToggleTheme, "palette.lightMode")
    } else {
        (Command::ToggleTheme, "palette.darkMode")
    };
    for (c, key) in [
        (Command::NewTask, "palette.newTask"),
        (Command::NewView, "palette.newView"),
        theme,
    ] {
        let label = t(key);
        if matches(&label, &q) {
            out.push(Entry {
                action: Action::Run(c),
                label,
                section: Some("palette.commands"),
            });
        }
    }
    if !q.is_empty() {
        for task in tasks {
            out.push(Entry {
                action: Action::OpenTask(task.task.id.clone()),
                label: task.task.subject.clone(),
                section: Some("palette.tasks"),
            });
        }
    }
    out
}

/// Move the highlighted index by `delta`, wrapping; 0 when empty.
pub fn move_sel(sel: usize, len: usize, delta: i32) -> usize {
    if len == 0 {
        return 0;
    }
    (sel as i64 + delta as i64).rem_euclid(len as i64) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(e: &[Entry]) -> Vec<&Action> {
        e.iter().map(|x| &x.action).collect()
    }

    #[test]
    fn empty_query_lists_lists_and_commands_only() {
        let e = build_entries("", &[], &[], false, "en");
        assert_eq!(e.len(), SMART.len() + 3);
        assert!(matches!(e[0].action, Action::OpenList(_)));
        assert_eq!(e[0].label, "Go to My Day");
        assert!(e.iter().any(|x| x.action == Action::Run(Command::NewTask)));
    }

    #[test]
    fn filters_by_label_substring() {
        let e = build_entries("plan", &[], &[], false, "en");
        assert_eq!(kinds(&e), vec![&Action::OpenList(SmartList::Planned)]);
        let e = build_entries("new", &[], &[], false, "en");
        assert_eq!(e.len(), 2);
    }

    #[test]
    fn theme_command_depends_on_mode_and_locale() {
        let e = build_entries("light", &[], &[], true, "en");
        assert_eq!(kinds(&e), vec![&Action::Run(Command::ToggleTheme)]);
        assert!(build_entries("light", &[], &[], false, "en").is_empty());
        let sv = build_entries("", &[], &[], false, "sv");
        assert!(
            sv.iter()
                .any(|x| x.label == tr("palette.darkMode", &HashMap::new(), Some("sv")))
        );
    }

    #[test]
    fn move_sel_wraps() {
        assert_eq!(move_sel(0, 3, -1), 2);
        assert_eq!(move_sel(2, 3, 1), 0);
        assert_eq!(move_sel(1, 3, 1), 2);
        assert_eq!(move_sel(5, 0, 1), 0);
    }
}
