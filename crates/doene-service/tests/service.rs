//! Ticket #09 service integration tests against a temporary on-disk database.

use std::sync::{Arc, Mutex};

use doene_core::quickadd::QuickAddView;
use doene_core::recurrence::RawRecurrence;
use doene_core::{Date, Importance, TaskStatus, TrashKind};
use doene_service::{NewTask, Restored, Service, StepPatch, TaskPatch};
use doene_storage::Storage;
use serde_json::json;

struct Fixture {
    svc: Service,
    now: Arc<Mutex<String>>,
    _dir: tempfile::TempDir,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let storage = Storage::open(&dir.path().join("d.db")).unwrap();
    let now = Arc::new(Mutex::new("2025-03-10T09:00:00.000Z".to_owned()));
    let n = now.clone();
    let svc = Service::with_clock(storage, Box::new(move || n.lock().unwrap().clone()));
    Fixture {
        svc,
        now,
        _dir: dir,
    }
}

fn new(subject: &str) -> NewTask {
    NewTask {
        subject: subject.into(),
        ..Default::default()
    }
}

fn d(s: &str) -> Date {
    Date::parse(s).unwrap()
}

fn actions(f: &Fixture, id: &str) -> Vec<String> {
    f.svc
        .activity(id)
        .unwrap()
        .into_iter()
        .map(|a| a.action)
        .collect()
}

fn code<T: std::fmt::Debug>(r: Result<T, doene_service::ServiceError>) -> &'static str {
    r.unwrap_err().code().expect("coded error")
}

fn daily() -> RawRecurrence {
    RawRecurrence {
        kind: Some("daily".into()),
        interval: None,
        unit: None,
        days: None,
    }
}

#[test]
fn create_assigns_uuid_utc_timestamp_trimmed_subject_and_logs() {
    let f = fixture();
    let t = f
        .svc
        .create_task(NewTask {
            tags: vec!["#Home".into(), "home".into(), "@Big Job".into()],
            ..new("  Buy milk ")
        })
        .unwrap();
    assert_eq!(t.task.subject, "Buy milk");
    assert_eq!(t.task.id.len(), 36);
    assert_eq!(t.task.id.as_bytes()[14], b'4');
    assert_eq!(t.task.created_at, "2025-03-10T09:00:00.000Z");
    let tags: Vec<_> = t.tags.iter().map(|t| t.as_str()).collect();
    assert_eq!(tags, ["big-job", "home"]);
    assert!(actions(&f, &t.task.id).contains(&"created".to_owned()));
}

#[test]
fn create_errors_carry_codes_and_params() {
    let f = fixture();
    assert_eq!(code(f.svc.create_task(new("   "))), "error.subjectRequired");
    let e = f
        .svc
        .create_task(NewTask {
            parent_id: Some("nope".into()),
            ..new("x")
        })
        .unwrap_err();
    match e {
        doene_service::ServiceError::Coded(c) => {
            assert_eq!(c.code, "error.notFound");
            assert_eq!(c.param("what"), Some("what.parentTask"));
        }
        other => panic!("{other:?}"),
    }
    let bad = RawRecurrence {
        kind: Some("nope".into()),
        interval: None,
        unit: None,
        days: None,
    };
    assert_eq!(
        code(f.svc.create_task(NewTask {
            recurrence: Some(bad),
            ..new("x")
        })),
        "error.recurrenceTypeInvalid"
    );
    assert_eq!(code(f.svc.task("missing")), "error.notFound");
    assert!(
        f.svc.all_tasks().unwrap().is_empty(),
        "failed creates leave nothing behind"
    );
}

#[test]
fn recurring_create_defaults_due_to_today_and_positions_follow_add_to_top() {
    let f = fixture();
    let r = f
        .svc
        .create_task(NewTask {
            recurrence: Some(daily()),
            ..new("r")
        })
        .unwrap();
    assert_eq!(r.task.due_date, Some(d("2025-03-10")));
    let a = f.svc.create_task(new("a")).unwrap();
    let b = f.svc.create_task(new("b")).unwrap();
    assert!(b.task.position > a.task.position);
    f.svc.set_setting("addTasksToTop", json!(true)).unwrap();
    let c = f.svc.create_task(new("c")).unwrap();
    assert!(c.task.position < r.task.position);
}

#[test]
fn update_patches_fields_and_logs_changes() {
    let f = fixture();
    let t = f.svc.create_task(new("one")).unwrap();
    let id = t.task.id.clone();
    let out = f
        .svc
        .update_task(
            &id,
            TaskPatch {
                subject: Some("two".into()),
                importance: Some(Importance::High),
                due_date: Some(Some(d("2025-04-01"))),
                body: Some(Some("notes".into())),
                tags: Some(vec!["x".into()]),
                shortlisted: Some(true),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(out.task.task.subject, "two");
    assert_eq!(out.task.task.importance, Importance::High);
    assert_eq!(out.task.task.due_date, Some(d("2025-04-01")));
    assert_eq!(out.task.task.focus_date, Some(d("2025-03-10")));
    assert_eq!(out.task.tags.len(), 1);
    let acts = actions(&f, &id);
    for a in [
        "subject changed",
        "importance changed",
        "due_date changed",
        "tags changed",
    ] {
        assert!(acts.contains(&a.to_owned()), "{a} in {acts:?}");
    }
    let cleared = f
        .svc
        .update_task(
            &id,
            TaskPatch {
                due_date: Some(None),
                shortlisted: Some(false),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(cleared.task.task.due_date, None);
    assert_eq!(cleared.task.task.focus_date, None);
}

#[test]
fn body_only_update_still_writes_an_activity_row() {
    let f = fixture();
    let t = f.svc.create_task(new("one")).unwrap();
    let before = f.svc.activity(&t.task.id).unwrap().len();
    f.svc
        .update_task(
            &t.task.id,
            TaskPatch {
                body: Some(Some("b".into())),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(f.svc.activity(&t.task.id).unwrap().len(), before + 1);
}

#[test]
fn completing_recurring_task_spawns_next_and_reopen_removes_it() {
    let f = fixture();
    let t = f
        .svc
        .create_task(NewTask {
            due_date: Some(d("2025-03-01")),
            recurrence: Some(daily()),
            ..new("water")
        })
        .unwrap();
    f.svc.create_step(&t.task.id, "fetch can").unwrap();
    let done = f
        .svc
        .update_task(
            &t.task.id,
            TaskPatch {
                status: Some(TaskStatus::Completed),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(done.task.task.status, TaskStatus::Completed);
    assert_eq!(
        done.task.task.completed_at.as_deref(),
        Some("2025-03-10T09:00:00.000Z")
    );
    let next = done.next.expect("spawned");
    assert_eq!(next.task.due_date, Some(d("2025-03-10")));
    assert_eq!(next.task.spawned_from.as_deref(), Some(t.task.id.as_str()));
    assert_eq!(next.steps.len(), 1);
    assert_eq!(next.steps[0].status, TaskStatus::Open);
    assert_ne!(next.steps[0].id, done.task.steps[0].id);

    let re = f
        .svc
        .update_task(
            &t.task.id,
            TaskPatch {
                status: Some(TaskStatus::Open),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(re.task.task.completed_at, None);
    assert_eq!(
        f.svc.all_tasks().unwrap().len(),
        1,
        "spawn removed on reopen"
    );
    assert!(actions(&f, &t.task.id).contains(&"reopened".to_owned()));
}

#[test]
fn completing_a_parent_completes_open_descendants() {
    let f = fixture();
    let p = f.svc.create_task(new("p")).unwrap();
    let c = f
        .svc
        .create_task(NewTask {
            parent_id: Some(p.task.id.clone()),
            ..new("c")
        })
        .unwrap();
    let g = f
        .svc
        .create_task(NewTask {
            parent_id: Some(c.task.id.clone()),
            ..new("g")
        })
        .unwrap();
    f.svc
        .update_task(
            &p.task.id,
            TaskPatch {
                status: Some(TaskStatus::Completed),
                ..Default::default()
            },
        )
        .unwrap();
    for id in [&c.task.id, &g.task.id] {
        assert_eq!(f.svc.task(id).unwrap().task.status, TaskStatus::Completed);
        assert!(actions(&f, id).contains(&"completed".to_owned()));
    }
}

#[test]
fn subtask_guards() {
    let f = fixture();
    let a = f.svc.create_task(new("a")).unwrap();
    let b = f
        .svc
        .create_task(NewTask {
            parent_id: Some(a.task.id.clone()),
            ..new("b")
        })
        .unwrap();
    let own = f.svc.update_task(
        &a.task.id,
        TaskPatch {
            parent_id: Some(Some(a.task.id.clone())),
            ..Default::default()
        },
    );
    assert_eq!(code(own), "error.ownSubtask");
    let cycle = f.svc.update_task(
        &a.task.id,
        TaskPatch {
            parent_id: Some(Some(b.task.id.clone())),
            ..Default::default()
        },
    );
    assert_eq!(code(cycle), "error.subtaskCycle");
    let cycle = f.svc.place_task(&a.task.id, Some(&b.task.id), 0);
    assert_eq!(code(cycle), "error.subtaskCycle");
}

#[test]
fn delete_moves_subtree_to_trash_and_restore_brings_it_back() {
    let f = fixture();
    let p = f
        .svc
        .create_task(NewTask {
            tags: vec!["t".into()],
            recurrence: Some(daily()),
            ..new("p")
        })
        .unwrap();
    f.svc.create_step(&p.task.id, "s").unwrap();
    let c = f
        .svc
        .create_task(NewTask {
            parent_id: Some(p.task.id.clone()),
            tags: vec!["k".into()],
            ..new("c")
        })
        .unwrap();
    let before = f.svc.all_tasks().unwrap();
    let trash_id = f.svc.delete_task(&p.task.id).unwrap();
    assert!(f.svc.all_tasks().unwrap().is_empty());
    let listed = f.svc.trash().unwrap();
    assert_eq!(
        (
            listed[0].id.as_str(),
            listed[0].kind,
            listed[0].label.as_deref()
        ),
        (trash_id.as_str(), TrashKind::Task, Some("p"))
    );
    assert!(actions(&f, &p.task.id).contains(&"deleted".to_owned()));

    match f.svc.restore(&trash_id).unwrap() {
        Restored::Task(t) => assert_eq!(t.task.id, p.task.id),
        other => panic!("{other:?}"),
    }
    let mut after = f.svc.all_tasks().unwrap();
    let mut before = before;
    before.sort_by(|a, b| a.task.id.cmp(&b.task.id));
    after.sort_by(|a, b| a.task.id.cmp(&b.task.id));
    assert_eq!(
        after, before,
        "snapshot round-trips tasks, tags, steps, recurrence"
    );
    assert_eq!(
        f.svc.task(&c.task.id).unwrap().task.parent_id.as_deref(),
        Some(p.task.id.as_str())
    );
    assert!(f.svc.trash().unwrap().is_empty());
    assert!(actions(&f, &p.task.id).contains(&"restored".to_owned()));
    assert_eq!(code(f.svc.restore(&trash_id)), "error.notFound");
}

#[test]
fn trash_purges_after_30_days_and_can_be_emptied() {
    let f = fixture();
    let a = f.svc.create_task(new("a")).unwrap();
    let b = f.svc.create_task(new("b")).unwrap();
    f.svc.delete_task(&a.task.id).unwrap();
    *f.now.lock().unwrap() = "2025-04-20T09:00:00.000Z".into();
    f.svc.delete_task(&b.task.id).unwrap();
    let labels: Vec<_> = f
        .svc
        .trash()
        .unwrap()
        .into_iter()
        .filter_map(|t| t.label)
        .collect();
    assert_eq!(labels, ["b"], "a, 41 days old, is purged");
    f.svc.empty_trash().unwrap();
    assert!(f.svc.trash().unwrap().is_empty());
}

#[test]
fn place_task_uses_midpoints_and_renumbers_when_no_gap() {
    let f = fixture();
    let ids: Vec<String> = ["a", "b", "c"]
        .iter()
        .map(|s| f.svc.create_task(new(s)).unwrap().task.id)
        .collect();
    let moved = f.svc.place_task(&ids[2], None, 0).unwrap();
    assert!(moved.task.position < f.svc.task(&ids[0]).unwrap().task.position);
    let moved = f.svc.place_task(&ids[2], None, 1).unwrap();
    let (a, b) = (
        f.svc.task(&ids[0]).unwrap().task.position,
        f.svc.task(&ids[1]).unwrap().task.position,
    );
    assert!(moved.task.position > a && moved.task.position < b);
    // exhaust the gap, then force renumbering
    for _ in 0..80 {
        f.svc.place_task(&ids[2], None, 1).unwrap();
        f.svc.place_task(&ids[1], None, 0).unwrap();
        f.svc.place_task(&ids[1], None, 1).unwrap();
    }
    let order: Vec<_> = f
        .svc
        .all_tasks()
        .unwrap()
        .into_iter()
        .map(|t| (t.task.position, t.task.id))
        .collect();
    let mut sorted = order.clone();
    sorted.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap());
    assert!(
        sorted.windows(2).all(|w| w[0].0 < w[1].0),
        "positions stay strictly ordered"
    );
    assert!(actions(&f, &ids[2]).contains(&"moved".to_owned()));
}

#[test]
fn steps_crud_and_promotion() {
    let f = fixture();
    let t = f.svc.create_task(new("t")).unwrap();
    assert_eq!(
        code(f.svc.create_step(&t.task.id, " ")),
        "error.subjectRequired"
    );
    assert_eq!(code(f.svc.create_step("zz", "s")), "error.notFound");
    let s1 = f.svc.create_step(&t.task.id, "one").unwrap();
    let s2 = f.svc.create_step(&t.task.id, "two").unwrap();
    assert!(s2.position > s1.position);
    let up = f
        .svc
        .update_step(
            &s1.id,
            StepPatch {
                status: Some(TaskStatus::Completed),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(up.status, TaskStatus::Completed);
    let sub = f.svc.step_to_task(&s2.id).unwrap();
    assert_eq!(sub.task.parent_id.as_deref(), Some(t.task.id.as_str()));
    assert_eq!(sub.task.subject, "two");
    f.svc.delete_step(&s1.id).unwrap();
    assert!(f.svc.task(&t.task.id).unwrap().steps.is_empty());
    assert_eq!(code(f.svc.delete_step(&s1.id)), "error.notFound");
    assert!(
        actions(&f, &t.task.id)
            .iter()
            .filter(|a| a.starts_with("step"))
            .count()
            >= 5
    );
}

#[test]
fn quick_add_parses_nlp_and_applies_view_defaults() {
    let f = fixture();
    let t = f
        .svc
        .quick_add("Pay rent tomorrow !high #bills", None, false)
        .unwrap();
    assert_eq!(t.task.subject, "Pay rent");
    assert_eq!(t.task.due_date, Some(d("2025-03-11")));
    assert_eq!(t.task.importance, Importance::High);
    assert_eq!(t.tags[0].as_str(), "bills");

    let lit = f.svc.quick_add("Pay rent tomorrow", None, true).unwrap();
    assert_eq!(lit.task.subject, "Pay rent tomorrow");
    assert_eq!(lit.task.due_date, None);

    let md = f
        .svc
        .quick_add("plan", Some(&QuickAddView::Smart("my-day".into())), false)
        .unwrap();
    assert_eq!(md.task.focus_date, Some(d("2025-03-10")));
    let planned = f
        .svc
        .quick_add("x", Some(&QuickAddView::Smart("planned".into())), false)
        .unwrap();
    assert_eq!(planned.task.due_date, Some(d("2025-03-10")));
    assert_eq!(
        code(f.svc.quick_add("  ", None, false)),
        "error.textRequired"
    );
    assert!(actions(&f, &t.task.id).contains(&"created".to_owned()));
}

#[test]
fn quick_add_uses_the_language_setting() {
    let f = fixture();
    f.svc.set_setting("language", json!("sv")).unwrap();
    let t = f.svc.quick_add("Handla imorgon", None, false).unwrap();
    assert_eq!(t.task.subject, "Handla");
    assert_eq!(t.task.due_date, Some(d("2025-03-11")));
}

#[test]
fn search_uses_core_scoring() {
    let f = fixture();
    f.svc.create_task(new("Buy milk")).unwrap();
    f.svc
        .create_task(NewTask {
            body: Some("remember milk".into()),
            ..new("Errand")
        })
        .unwrap();
    f.svc.create_task(new("Walk dog")).unwrap();
    let hits = f.svc.search("milk", 10).unwrap();
    let subjects: Vec<_> = hits.iter().map(|h| h.task.subject.as_str()).collect();
    assert_eq!(subjects, ["Buy milk", "Errand"]);
    assert!(
        f.svc.search("milk dog", 10).unwrap().is_empty(),
        "all words required"
    );
}

#[test]
fn settings_validate_with_coded_errors() {
    let f = fixture();
    assert_eq!(
        code(f.svc.set_setting("language", json!("xx"))),
        "error.unsupportedLanguage"
    );
    assert_eq!(
        code(f.svc.set_setting("focusMode", json!("weekly"))),
        "error.unsupportedFocusMode"
    );
    assert_eq!(
        code(f.svc.set_setting("nope", json!(1))),
        "error.unknownSetting"
    );
    let s = f.svc.set_setting("focusMode", json!("durable")).unwrap();
    assert_eq!(s.focus_mode, doene_core::FocusMode::Durable);
    assert_eq!(f.svc.settings().unwrap().language, "en");
}

#[test]
fn data_survives_reopening_the_database() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("d.db");
    let id = {
        let svc = Service::new(Storage::open(&path).unwrap());
        svc.create_task(new("persist")).unwrap().task.id
    };
    let svc = Service::new(Storage::open(&path).unwrap());
    assert_eq!(svc.task(&id).unwrap().task.subject, "persist");
}

#[test]
fn theme_setting_defaults_to_system_validates_and_persists() {
    let f = fixture();
    assert_eq!(f.svc.settings().unwrap().theme, "system");
    assert_eq!(
        f.svc.set_setting("theme", json!("dark")).unwrap().theme,
        "dark"
    );
    assert_eq!(
        code(f.svc.set_setting("theme", json!("pink"))),
        "error.unsupportedTheme"
    );
    assert_eq!(f.svc.settings().unwrap().theme, "dark");
}

// ---- ticket #12: undo

#[test]
fn undo_restores_deleted_subtree_and_trash() {
    let f = fixture();
    let p = f.svc.create_task(new("parent")).unwrap();
    let c = f
        .svc
        .create_task(NewTask {
            subject: "child".into(),
            parent_id: Some(p.task.id.clone()),
            ..Default::default()
        })
        .unwrap();
    f.svc.create_step(&c.task.id, "step").unwrap();
    f.svc.delete_task(&p.task.id).unwrap();
    assert!(f.svc.all_tasks().unwrap().is_empty());
    assert_eq!(f.svc.undo_peek(), Some(doene_service::UndoKind::Delete));
    assert_eq!(f.svc.undo().unwrap(), Some(doene_service::UndoKind::Delete));
    let all = f.svc.all_tasks().unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(f.svc.task(&c.task.id).unwrap().steps.len(), 1);
    assert_eq!(
        f.svc.task(&c.task.id).unwrap().task.parent_id.as_deref(),
        Some(p.task.id.as_str())
    );
    assert!(f.svc.trash().unwrap().is_empty());
}

#[test]
fn undo_spans_operation_types_in_order() {
    let f = fixture();
    let t = f.svc.create_task(new("a")).unwrap();
    f.svc
        .update_task(
            &t.task.id,
            TaskPatch {
                subject: Some("b".into()),
                ..Default::default()
            },
        )
        .unwrap();
    f.svc.create_step(&t.task.id, "s").unwrap();
    f.svc.undo().unwrap();
    assert!(f.svc.task(&t.task.id).unwrap().steps.is_empty());
    f.svc.undo().unwrap();
    assert_eq!(f.svc.task(&t.task.id).unwrap().task.subject, "a");
    f.svc.undo().unwrap();
    assert!(f.svc.all_tasks().unwrap().is_empty());
    assert_eq!(f.svc.undo().unwrap(), None);
}

#[test]
fn undo_restore_and_permanent_delete() {
    let f = fixture();
    let t = f.svc.create_task(new("a")).unwrap();
    let tid = f.svc.delete_task(&t.task.id).unwrap();
    f.svc.restore(&tid).unwrap();
    assert_eq!(f.svc.all_tasks().unwrap().len(), 1);
    f.svc.undo().unwrap();
    assert!(f.svc.all_tasks().unwrap().is_empty());
    assert_eq!(f.svc.trash().unwrap().len(), 1);
    f.svc.delete_forever(&tid).unwrap();
    assert!(f.svc.trash().unwrap().is_empty());
    f.svc.undo().unwrap();
    assert_eq!(f.svc.trash().unwrap().len(), 1);
}

#[test]
fn failed_mutation_records_no_undo() {
    let f = fixture();
    assert!(f.svc.delete_task("nope").is_err());
    assert_eq!(f.svc.undo_peek(), None);
}

#[test]
fn drawer_pref_persists_and_validates_scope() {
    let f = fixture();
    assert_eq!(f.svc.pref("drawer:abc").unwrap(), None);
    f.svc.set_pref("drawer:abc", r#"["notes","tags"]"#).unwrap();
    f.svc.set_pref("drawer:abc", r#"["tags","notes"]"#).unwrap();
    assert_eq!(
        f.svc.pref("drawer:abc").unwrap().as_deref(),
        Some(r#"["tags","notes"]"#)
    );
    f.svc
        .set_pref("global", r#"{"myDayGroupOrder":["radar"]}"#)
        .unwrap();
    assert_eq!(
        f.svc.pref("global").unwrap().as_deref(),
        Some(r#"{"myDayGroupOrder":["radar"]}"#)
    );
    assert_eq!(
        f.svc.set_pref("other", "x").unwrap_err().code(),
        Some("error.unknownPrefScope")
    );
    assert_eq!(
        f.svc.set_pref("drawer:abc", " ").unwrap_err().code(),
        Some("error.prefValueRequired")
    );
}

// ---- ticket #16: views

fn raw_view(name: &str) -> doene_core::RawView {
    doene_core::RawView {
        name: Some(name.into()),
        ..Default::default()
    }
}

fn sort(by: &str, dir: &str) -> doene_core::RawSortLevel {
    doene_core::RawSortLevel {
        by: by.into(),
        dir: Some(dir.into()),
    }
}

#[test]
fn view_create_validates_and_persists_all_fields() {
    let f = fixture();
    assert_eq!(
        code(f.svc.create_view(raw_view("  "))),
        "error.viewNameRequired"
    );
    let mut bad = raw_view("x");
    bad.match_mode = Some("some".into());
    assert_eq!(code(f.svc.create_view(bad)), "error.viewMatchInvalid");
    let mut bad = raw_view("x");
    bad.sorts = vec![sort("zzz", "asc")];
    assert_eq!(code(f.svc.create_view(bad)), "error.viewSortInvalidKey");
    let mut bad = raw_view("x");
    bad.sorts = (0..7).map(|_| sort("name", "asc")).collect();
    assert_eq!(code(f.svc.create_view(bad)), "error.viewSortTooMany");
    assert!(f.svc.views().unwrap().is_empty());

    let mut r = raw_view(" Work ");
    r.emoji = Some("💼".into());
    r.tags = vec!["#Work".into(), "work".into(), "Big Job".into()];
    r.match_mode = Some("all".into());
    r.show_done = true;
    r.sorts = (0..6)
        .map(|i| {
            sort(
                ["name", "due", "created", "name", "due", "created"][i],
                "desc",
            )
        })
        .collect();
    let v = f.svc.create_view(r).unwrap();
    let got = &f.svc.views().unwrap()[0];
    assert_eq!(*got, v);
    assert_eq!(got.view.name, "Work");
    assert_eq!(got.view.emoji.as_deref(), Some("💼"));
    let tags: Vec<_> = got.view.tags.iter().map(|t| t.as_str()).collect();
    assert_eq!(tags, ["big-job", "work"]);
    assert!(got.view.show_done);
    assert_eq!(got.sorts.len(), 6);
}

#[test]
fn view_update_delete_restore_and_undo() {
    let f = fixture();
    let a = f.svc.create_view(raw_view("A")).unwrap();
    let mut r = raw_view("A2");
    r.sorts = vec![sort("manual", "desc")];
    let u = f.svc.update_view(&a.view.id, r).unwrap();
    assert_eq!(u.view.name, "A2");
    assert_eq!(u.sorts[0].dir, doene_core::SortDirection::Asc);
    assert_eq!(
        code(f.svc.update_view("nope", raw_view("x"))),
        "error.notFound"
    );
    assert_eq!(
        code(f.svc.update_view(&a.view.id, raw_view(""))),
        "error.viewNameRequired"
    );

    f.svc.undo().unwrap();
    assert_eq!(f.svc.views().unwrap()[0].view.name, "A");

    let trash_id = f.svc.delete_view(&a.view.id).unwrap();
    assert!(f.svc.views().unwrap().is_empty());
    assert_eq!(f.svc.trash().unwrap()[0].kind, TrashKind::View);
    assert_eq!(code(f.svc.delete_view(&a.view.id)), "error.notFound");
    f.svc.undo().unwrap();
    assert_eq!(f.svc.views().unwrap().len(), 1);

    f.svc.delete_view(&a.view.id).unwrap();
    let id = f.svc.trash().unwrap()[0].id.clone();
    assert_ne!(id, trash_id);
    assert!(matches!(f.svc.restore(&id).unwrap(), Restored::View { .. }));
    assert_eq!(f.svc.views().unwrap()[0].view.name, "A");
    assert!(f.svc.trash().unwrap().is_empty());
}

#[test]
fn view_move_renumbers_gap_free() {
    let f = fixture();
    let ids: Vec<String> = ["A", "B", "C"]
        .iter()
        .map(|n| f.svc.create_view(raw_view(n)).unwrap().view.id)
        .collect();
    f.svc.move_view(&ids[2], 0).unwrap();
    let vs = f.svc.views().unwrap();
    let names: Vec<_> = vs.iter().map(|v| v.view.name.as_str()).collect();
    assert_eq!(names, ["C", "A", "B"]);
    let pos: Vec<_> = vs.iter().map(|v| v.view.position).collect();
    assert_eq!(pos, [1.0, 2.0, 3.0]);
    f.svc.move_view(&ids[2], 99).unwrap();
    let names: Vec<_> = f
        .svc
        .views()
        .unwrap()
        .iter()
        .map(|v| v.view.name.clone())
        .collect();
    assert_eq!(names, ["A", "B", "C"]);
    assert_eq!(code(f.svc.move_view("nope", 0)), "error.notFound");
    f.svc.undo().unwrap();
    assert_eq!(f.svc.views().unwrap()[0].view.name, "C");
}

#[test]
fn due_notifications_are_persisted_deduped_and_pruned() {
    let f = fixture();
    let t = f
        .svc
        .create_task(NewTask {
            subject: "Pay".into(),
            due_date: Some(d("2025-03-10")),
            ..Default::default()
        })
        .unwrap();
    assert!(
        f.svc
            .take_due_notifications("2025-03-10T08:00")
            .unwrap()
            .is_empty()
    );
    let got = f.svc.take_due_notifications("2025-03-10T09:00").unwrap();
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].key, format!("d:{}:2025-03-10", t.task.id));
    assert!(
        f.svc
            .take_due_notifications("2025-03-10T10:00")
            .unwrap()
            .is_empty()
    );
    f.svc.set_setting("dueNotifyTime", json!("12:00")).unwrap();
    assert!(f.svc.set_setting("dueNotifyTime", json!("25:00")).is_err());
}

#[test]
fn fired_set_is_pruned_to_last_500_keys() {
    let dir = tempfile::tempdir().unwrap();
    let st = Storage::open(&dir.path().join("d.db")).unwrap();
    for i in 0..510 {
        doene_storage::aux::mark_fired(st.conn(), &format!("k{i}"), "x").unwrap();
    }
    doene_storage::aux::prune_fired(st.conn(), 500).unwrap();
    let keys = doene_storage::aux::fired_keys(st.conn()).unwrap();
    assert_eq!((keys.len(), keys[0].as_str()), (500, "k10"));
}

#[test]
fn settings_week_start_notify_time_headings_round_trip_and_validate() {
    let f = fixture();
    let s = f.svc.settings().unwrap();
    assert_eq!(
        (
            s.week_start,
            s.due_notify_time.as_str(),
            s.show_my_day_headings,
            s.language_chosen
        ),
        (0, "09:00", true, false)
    );
    f.svc.set_setting("language", json!("sv")).unwrap();
    assert_eq!(f.svc.settings().unwrap().week_start, 1);
    f.svc.set_setting("weekStart", json!(0)).unwrap();
    f.svc.set_setting("dueNotifyTime", json!("07:30")).unwrap();
    f.svc
        .set_setting("showMyDayGroupHeadings", json!(false))
        .unwrap();
    let s = f.svc.settings().unwrap();
    assert_eq!(
        (
            s.week_start,
            s.due_notify_time.as_str(),
            s.show_my_day_headings,
            s.language_chosen
        ),
        (0, "07:30", false, true)
    );
    assert!(f.svc.set_setting("weekStart", json!(2)).is_err());
    assert!(
        f.svc
            .set_setting("showMyDayGroupHeadings", json!("x"))
            .is_err()
    );
}
