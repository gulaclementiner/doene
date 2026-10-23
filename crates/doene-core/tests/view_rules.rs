//! Parity fixtures ported 1:1 from Gör't `test/view-rules.test.js` (the view
//! rules shared across the seam): normaliseView, viewMatches, sortTasks.

use doene_core::entities::{HydratedTask, Importance, MatchMode, Tag, Task, TaskStatus};
use doene_core::view_rules::{
    RawSortLevel, RawView, SORT_KEYS, SortDirection, SortKey, SortLevel, normalise_view,
    sort_tasks, view_matches,
};

/// Gör't's `t` helper: an open task with the given id and defaults.
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
        created_at: "2025-01-01T00:00:00.000Z".to_owned(),
        completed_at: None,
        position: 0.0,
        spawned_from: None,
        parent_id: None,
        recurrence: None,
    }
}

fn ht(task: Task) -> HydratedTask {
    HydratedTask::bare(task)
}

/// An open task carrying the given tags (Gör't's `t(id, {tags})`).
fn tagged(id: &str, tags: &[&str]) -> HydratedTask {
    let mut h = ht(t(id));
    h.tags = tags
        .iter()
        .map(|s| Tag::new(s).expect("valid tag"))
        .collect();
    h
}

#[test]
fn sort_keys_is_the_picker_order() {
    assert_eq!(SORT_KEYS, ["name", "due", "created", "manual"]);
}

#[test]
fn normalise_trims_name_defaults_match_and_showed_dedupes_tags_falls_back_to_asc() {
    let v = normalise_view(&RawView {
        name: Some("  Work  ".to_owned()),
        tags: vec![" home ".to_owned(), "home".to_owned(), "".to_owned()],
        sorts: vec![RawSortLevel {
            by: "name".to_owned(),
            dir: Some("sideways".to_owned()),
        }],
        ..Default::default()
    })
    .unwrap();
    assert_eq!(v.name, "Work");
    assert_eq!(v.emoji, None);
    assert_eq!(v.tags, vec!["home".to_owned()]);
    assert_eq!(v.match_mode, MatchMode::Any);
    assert_eq!(
        v.sorts,
        vec![SortLevel {
            by: SortKey::Name,
            dir: SortDirection::Asc
        }]
    );
    assert!(!v.show_done);
}

#[test]
fn normalise_throws_coded_errors_for_blank_name_bad_match_and_unknown_sort_key() {
    let e = normalise_view(&RawView {
        name: Some("  ".to_owned()),
        ..Default::default()
    })
    .unwrap_err();
    assert_eq!(e.code, "error.viewNameRequired");
    assert!(e.message.contains("name required"));

    let e = normalise_view(&RawView {
        name: Some("v".to_owned()),
        match_mode: Some("some".to_owned()),
        ..Default::default()
    })
    .unwrap_err();
    assert_eq!(e.code, "error.viewMatchInvalid");
    assert!(e.message.contains("match"));

    let e = normalise_view(&RawView {
        name: Some("v".to_owned()),
        sorts: vec![RawSortLevel {
            by: "importance".to_owned(),
            dir: None,
        }],
        ..Default::default()
    })
    .unwrap_err();
    assert_eq!(e.code, "error.viewSortInvalidKey");
    assert_eq!(e.param("by"), Some("importance"));
}

#[test]
fn normalise_keeps_manual_direction_asc() {
    let v = normalise_view(&RawView {
        name: Some("v".to_owned()),
        sorts: vec![RawSortLevel {
            by: "manual".to_owned(),
            dir: Some("desc".to_owned()),
        }],
        ..Default::default()
    })
    .unwrap();
    assert_eq!(
        v.sorts,
        vec![SortLevel {
            by: SortKey::Manual,
            dir: SortDirection::Asc
        }]
    );
}

#[test]
fn view_matches_no_tags_means_no_filter_completed_hidden_unless_show_done() {
    let open = ht(t("a"));
    let mut done_task = t("b");
    done_task.status = TaskStatus::Completed;
    let done = ht(done_task);

    for show_done in [false, true] {
        let v = normalise_view(&RawView {
            name: Some("v".to_owned()),
            show_done,
            ..Default::default()
        })
        .unwrap();
        assert!(view_matches(&v, &open), "an open task always matches");
        assert_eq!(view_matches(&v, &done), show_done);
    }
}

#[test]
fn view_matches_any_keeps_a_task_with_at_least_one_tag_all_needs_every_tag() {
    let home = tagged("a", &["home"]);
    let both = tagged("b", &["home", "errand"]);
    let work = tagged("c", &["work"]);
    let none = ht(t("d"));

    let v_any = normalise_view(&RawView {
        name: Some("v".to_owned()),
        tags: vec!["home".to_owned(), "errand".to_owned()],
        ..Default::default()
    })
    .unwrap();
    assert!(view_matches(&v_any, &home));
    assert!(view_matches(&v_any, &both));
    assert!(!view_matches(&v_any, &work));

    let v_all = normalise_view(&RawView {
        name: Some("v".to_owned()),
        tags: vec!["home".to_owned(), "errand".to_owned()],
        match_mode: Some("all".to_owned()),
        ..Default::default()
    })
    .unwrap();
    assert!(view_matches(&v_all, &both));
    assert!(!view_matches(&v_all, &home));

    let v_x = normalise_view(&RawView {
        name: Some("v".to_owned()),
        tags: vec!["x".to_owned()],
        ..Default::default()
    })
    .unwrap();
    assert!(
        !view_matches(&v_x, &none),
        "a task with no tags never matches a tag filter"
    );
}

#[test]
fn view_matches_ignores_tag_case() {
    let v = normalise_view(&RawView {
        name: Some("v".to_owned()),
        tags: vec!["WORK".to_owned()],
        ..Default::default()
    })
    .unwrap();
    assert!(view_matches(&v, &tagged("a", &["work"])));
}

#[test]
fn sort_tasks_name_due_undated_last_and_created_ascending_and_descending() {
    let mut b = t("b");
    b.due_date = Some(d("2025-03-02"));
    b.created_at = "2025-01-02T00:00:00.000Z".to_owned();
    let mut a = t("a");
    a.due_date = Some(d("2025-03-01"));
    a.created_at = "2025-01-03T00:00:00.000Z".to_owned();
    let mut c = t("c");
    c.due_date = None;
    c.created_at = "2025-01-01T00:00:00.000Z".to_owned();
    let tasks = vec![ht(b), ht(a), ht(c)];

    let ids = |sorts: &[SortLevel]| {
        sort_tasks(&tasks, sorts)
            .iter()
            .map(|x| x.task.id.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        ids(&[SortLevel {
            by: SortKey::Name,
            dir: SortDirection::Asc
        }]),
        vec!["a", "b", "c"]
    );
    assert_eq!(
        ids(&[SortLevel {
            by: SortKey::Name,
            dir: SortDirection::Desc
        }]),
        vec!["c", "b", "a"]
    );
    assert_eq!(
        ids(&[SortLevel {
            by: SortKey::Due,
            dir: SortDirection::Asc
        }]),
        vec!["a", "b", "c"],
        "undated sorts last"
    );
    assert_eq!(
        ids(&[SortLevel {
            by: SortKey::Created,
            dir: SortDirection::Asc
        }]),
        vec!["c", "b", "a"]
    );
}

#[test]
fn sort_tasks_multi_level_ties_from_the_first_level_fall_to_the_next() {
    let mut x1 = t("x1");
    x1.due_date = Some(d("2025-01-01"));
    let mut a2 = t("a2");
    a2.due_date = Some(d("2025-01-01"));
    let mut z0 = t("z0");
    z0.due_date = Some(d("2024-01-01"));
    let tasks = vec![ht(x1), ht(a2), ht(z0)];

    assert_eq!(
        sort_tasks(
            &tasks,
            &[
                SortLevel {
                    by: SortKey::Due,
                    dir: SortDirection::Asc
                },
                SortLevel {
                    by: SortKey::Name,
                    dir: SortDirection::Desc
                },
            ]
        )
        .iter()
        .map(|x| x.task.id.clone())
        .collect::<Vec<_>>(),
        vec!["z0", "x1", "a2"]
    );
}

#[test]
fn sort_tasks_manual_uses_position_and_no_levels_keeps_input_order_without_mutating() {
    let mut p2 = t("p2");
    p2.position = 2.0;
    let mut p1 = t("p1");
    p1.position = 1.0;
    let mut p3 = t("p3");
    p3.position = 3.0;
    let tasks = vec![ht(p2), ht(p1), ht(p3)];

    let sorts = normalise_view(&RawView {
        name: Some("v".to_owned()),
        sorts: vec![RawSortLevel {
            by: "manual".to_owned(),
            dir: Some("desc".to_owned()),
        }],
        ..Default::default()
    })
    .unwrap()
    .sorts;
    assert_eq!(
        sorts,
        vec![SortLevel {
            by: SortKey::Manual,
            dir: SortDirection::Asc
        }],
        "normaliseView forces manual to asc, so the direction is ignored"
    );

    let ids = |sorts: &[SortLevel]| {
        sort_tasks(&tasks, sorts)
            .iter()
            .map(|x| x.task.id.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(ids(&sorts), vec!["p1", "p2", "p3"]);
    assert_eq!(
        ids(&[]),
        vec!["p2", "p1", "p3"],
        "no sort levels keeps the input order"
    );

    sort_tasks(
        &tasks,
        &[SortLevel {
            by: SortKey::Name,
            dir: SortDirection::Asc,
        }],
    );
    assert_eq!(
        tasks.iter().map(|x| x.task.id.clone()).collect::<Vec<_>>(),
        vec!["p2", "p1", "p3"],
        "the input array is not mutated"
    );
}

fn d(s: &str) -> doene_core::dates::Date {
    doene_core::dates::Date::parse(s).expect("valid date")
}
