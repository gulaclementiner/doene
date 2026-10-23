//! Parity fixtures ported 1:1 from Gör't `test/search-recurrence-mutants.test.js`
//! (the three search tests) and the search-scoring case of
//! `test/nlp-search.test.js`.

use doene_core::entities::{HydratedTask, Importance, Step, Task, TaskStatus};
use doene_core::search::{score, search_tasks};

/// Gör't's `T` helper: an open task with the given subject.
fn t(subject: &str) -> HydratedTask {
    t_with(subject, None, &[], TaskStatus::Open)
}

fn t_with(subject: &str, body: Option<&str>, steps: &[&str], status: TaskStatus) -> HydratedTask {
    let task = Task {
        id: String::new(),
        subject: subject.to_owned(),
        status,
        importance: Importance::Normal,
        due_date: None,
        reminder_at: None,
        body: body.map(str::to_owned),
        focus_date: None,
        postponed_date: None,
        created_at: "2025-01-01T00:00:00.000Z".to_owned(),
        completed_at: None,
        position: 0.0,
        spawned_from: None,
        parent_id: None,
        recurrence: None,
    };
    let steps = steps
        .iter()
        .enumerate()
        .map(|(i, s)| Step {
            id: format!("s{i}"),
            task_id: String::new(),
            subject: (*s).to_owned(),
            status: TaskStatus::Open,
            position: (i + 1) as f64,
        })
        .collect();
    HydratedTask {
        task,
        tags: vec![],
        steps,
    }
}

#[test]
fn search_score_exact_tiers() {
    assert_eq!(score(&t("Buy milk now"), &["milk".into()]), 6, "whole word");
    assert_eq!(
        score(&t("milkshake time"), &["milk".into()]),
        5,
        "subject prefix"
    );
    assert_eq!(score(&t("a milkshake"), &["milk".into()]), 4, "word prefix");
    assert_eq!(score(&t("almilk"), &["milk".into()]), 3, "substring");
    assert_eq!(
        score(
            &t_with("x", None, &["get MILK"], TaskStatus::Open),
            &["milk".into()]
        ),
        2,
        "step"
    );
    assert_eq!(
        score(
            &t_with("x", Some("some Milk"), &[], TaskStatus::Open),
            &["milk".into()]
        ),
        1,
        "body"
    );
    assert_eq!(score(&t("x"), &["milk".into()]), 0);
    assert_eq!(
        score(
            &t_with("milk", Some("zz"), &[], TaskStatus::Open),
            &["milk".into(), "zz".into()]
        ),
        7,
        "sums"
    );
    assert_eq!(
        score(&t("milk"), &["milk".into(), "zz".into()]),
        0,
        "all words must match"
    );
    assert_eq!(
        score(&t("hello, world"), &["world".into()]),
        6,
        "punctuation splits words"
    );
    assert_eq!(score(&t("a-milk"), &["milk".into()]), 6);
    // Gör't's `score({subject:'x'}, ['x'])` — missing steps/body ok.
    let bare = HydratedTask::bare(Task {
        id: String::new(),
        subject: "x".to_owned(),
        status: TaskStatus::Open,
        importance: Importance::Normal,
        due_date: None,
        reminder_at: None,
        body: None,
        focus_date: None,
        postponed_date: None,
        created_at: String::new(),
        completed_at: None,
        position: 0.0,
        spawned_from: None,
        parent_id: None,
        recurrence: None,
    });
    assert_eq!(score(&bare, &["x".into()]), 6);
    assert_eq!(
        score(
            &t_with("zzz", None, &["a", "milk"], TaskStatus::Open),
            &["milk".into()]
        ),
        2
    );
    assert_eq!(
        score(&t("The Quick"), &["quick".into()]),
        6,
        "case-insensitive subject"
    );
    assert_eq!(score(&t("quick"), &["quick".into()]), 6);
    assert_eq!(
        score(
            &t_with("x", Some("QUICK"), &[], TaskStatus::Open),
            &["quick".into()]
        ),
        1,
        "case-insensitive body"
    );
    assert_eq!(
        score(
            &t_with("x", None, &["QUICK"], TaskStatus::Open),
            &["quick".into()]
        ),
        2,
        "case-insensitive step"
    );
    assert_eq!(
        score(&t_with("xx", None, &[], TaskStatus::Open), &["q".into()]),
        0
    );
}

#[test]
fn search_empty_blank_query_returns_nothing_and_is_case_space_tolerant() {
    let ts = [t("alpha")];
    assert_eq!(search_tasks(&ts, "", 50).len(), 0);
    assert_eq!(search_tasks(&ts, "   ", 50).len(), 0);
    // Gör't's `searchTasks(ts, null)` case: a null query is not representable
    // in Rust; the empty string covers it.
    assert_eq!(search_tasks(&ts, "  ALPHA  ", 50).len(), 1);
    assert_eq!(
        search_tasks(
            &[t_with("red apple", None, &[], TaskStatus::Open)],
            "red\tapple",
            50
        )
        .len(),
        1
    );
    assert_eq!(
        search_tasks(
            &[t_with("red apple", None, &[], TaskStatus::Open)],
            "red\napple",
            50
        )
        .len(),
        1
    );
    assert_eq!(search_tasks(&ts, "nomatch", 50).len(), 0);
}

#[test]
fn search_ranks_by_score_then_open_before_completed_then_subject_and_honours_limit() {
    let low = t("xmilk");
    let high = t("milk");
    assert_eq!(
        search_tasks(&[low.clone(), high.clone()], "milk", 50)
            .iter()
            .map(|x| x.task.subject.clone())
            .collect::<Vec<_>>(),
        vec!["milk", "xmilk"]
    );
    assert_eq!(
        search_tasks(&[high.clone(), low.clone()], "milk", 50)
            .iter()
            .map(|x| x.task.subject.clone())
            .collect::<Vec<_>>(),
        vec!["milk", "xmilk"]
    );

    let done = t_with("milk", None, &[], TaskStatus::Completed);
    let open = t("milk");
    for input in [[done.clone(), open.clone()], [open.clone(), done.clone()]] {
        assert_eq!(
            search_tasks(&input, "milk", 50)
                .iter()
                .map(|x| x.task.status)
                .collect::<Vec<_>>(),
            vec![TaskStatus::Open, TaskStatus::Completed]
        );
    }

    let b = t("milk b");
    let a = t("milk a");
    for input in [[b.clone(), a.clone()], [a.clone(), b.clone()]] {
        assert_eq!(
            search_tasks(&input, "milk", 50)
                .iter()
                .map(|x| x.task.subject.clone())
                .collect::<Vec<_>>(),
            vec!["milk a", "milk b"]
        );
    }

    // completed with higher score still beats open with lower score
    let mixed = [low, t_with("milk", None, &[], TaskStatus::Completed)];
    assert_eq!(search_tasks(&mixed, "milk", 50)[0].task.subject, "milk");

    let many: Vec<HydratedTask> = (0..10).map(|i| t(&format!("milk {i}"))).collect();
    assert_eq!(search_tasks(&many, "milk", 3).len(), 3);
    assert_eq!(search_tasks(&many, "milk", 50).len(), 10);
    assert_eq!(search_tasks(&many, "milk", 0).len(), 0);
}

#[test]
fn search_ranks_subject_over_steps_over_notes() {
    // Gör't's nlp-search fixture (ids 1..4).
    let mut tasks = vec![
        t_with("Buy milk", Some(""), &[], TaskStatus::Open),
        t_with(
            "Plan trip",
            Some("remember to buy sunscreen"),
            &["book hotel"],
            TaskStatus::Open,
        ),
        t_with("Milkshake machine", Some(""), &[], TaskStatus::Completed),
        t_with("Email Bob", Some(""), &[], TaskStatus::Open),
    ];

    // The fixture tasks carry numeric ids 1..4 in Gör't; tag them here.
    for (i, t) in tasks.iter_mut().enumerate() {
        t.task.id = (i + 1).to_string();
    }
    let ids = |q: &str| {
        search_tasks(&tasks, q, 50)
            .iter()
            .map(|x| x.task.id.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(ids("buy"), vec!["1", "2"]);
    assert_eq!(ids("hotel"), vec!["2"]);
    assert_eq!(ids("milk"), vec!["1", "3"]);
    assert_eq!(ids("BUY milk"), vec!["1"]);
    assert_eq!(ids("  "), Vec::<String>::new());
    assert_eq!(ids("zzz"), Vec::<String>::new());
}
