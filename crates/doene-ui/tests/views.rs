//! Ticket #16: view form -> service -> rail rows and view task lists.

use doene_core::{Date, SortKey};
use doene_service::{NewTask, Service};
use doene_storage::Storage;
use doene_ui::views::{
    SortDraft, ViewDraft, describe, drop_index, rail_rows, view_color, view_rows,
};

fn svc() -> Service {
    Service::with_clock(
        Storage::open_in_memory().unwrap(),
        Box::new(|| "2025-03-10T09:00:00.000Z".into()),
    )
}

fn today() -> Date {
    Date::parse("2025-03-10").unwrap()
}

fn task(s: &Service, subject: &str, tags: &[&str], due: Option<&str>) -> String {
    s.create_task(NewTask {
        subject: subject.into(),
        tags: tags.iter().map(|t| (*t).into()).collect(),
        due_date: due.map(|d| Date::parse(d).unwrap()),
        ..Default::default()
    })
    .unwrap()
    .task
    .id
}

#[test]
fn form_validates_and_caps_sorts_at_six() {
    let mut d = ViewDraft::new();
    assert!(d.validate("en").unwrap_err().contains("name"));
    d.name = "Work".into();
    assert!(d.validate("en").is_ok());
    for _ in 0..10 {
        d.add_sort();
    }
    assert_eq!(d.sorts.len(), 6);
    assert!(!d.add_sort());
    d.remove_sort(0);
    assert_eq!(d.sorts.len(), 5);
}

#[test]
fn saved_view_shows_matching_tasks_sorted_and_round_trips_to_form() {
    let s = svc();
    task(&s, "B task", &["work"], Some("2025-03-12"));
    task(&s, "A task", &["work", "home"], Some("2025-03-20"));
    task(&s, "C task", &["home"], None);
    let done = task(&s, "D done", &["work"], None);
    s.update_task(
        &done,
        doene_service::TaskPatch {
            status: Some(doene_core::TaskStatus::Completed),
            ..Default::default()
        },
    )
    .unwrap();

    let mut d = ViewDraft::new();
    d.name = "Work".into();
    d.emoji = "💼".into();
    d.new_tag = "#Work".into();
    d.commit_tag();
    d.sorts = vec![SortDraft {
        by: SortKey::Due,
        desc: true,
    }];
    let id = d.save(&s).unwrap();

    let views = s.views().unwrap();
    let names = |show: &doene_service::SavedView| {
        view_rows(show, &s.all_tasks().unwrap(), today(), "en")
            .into_iter()
            .map(|r| r.subject)
            .collect::<Vec<_>>()
    };
    assert_eq!(names(&views[0]), ["A task", "B task"]);

    // edit via the round-tripped form: include completed, all-match, sort name
    let mut e = ViewDraft::from_view(&views[0]);
    assert_eq!(e.id.as_deref(), Some(id.as_str()));
    assert_eq!(e.tags, ["work"]);
    e.show_done = true;
    e.sorts = vec![SortDraft {
        by: SortKey::Name,
        desc: false,
    }];
    e.save(&s).unwrap();
    let v = &s.views().unwrap()[0];
    assert_eq!(names(v), ["A task", "B task", "D done"]);
}

#[test]
fn rail_rows_carry_emoji_and_stable_colour() {
    let s = svc();
    let mut d = ViewDraft::new();
    d.name = "Home".into();
    d.emoji = "🏠".into();
    d.save(&s).unwrap();
    d.id = None;
    d.name = "Plain".into();
    d.emoji = String::new();
    d.save(&s).unwrap();
    let rows = rail_rows(&s.views().unwrap());
    assert_eq!(rows[0].icon, "🏠");
    assert_eq!(rows[1].icon, "●");
    assert_eq!(rows[0].color, view_color(&rows[0].id));
}

#[test]
fn drop_index_counts_rows_above_the_pointer() {
    let rows = [(0.0, 10.0), (10.0, 20.0), (20.0, 30.0)];
    assert_eq!(drop_index(&rows, 0, 29.0), 2);
    assert_eq!(drop_index(&rows, 2, 1.0), 0);
    assert_eq!(drop_index(&rows, 1, 12.0), 1);
}

#[test]
fn describe_summarises_rules() {
    let mut d = ViewDraft::new();
    assert_eq!(describe(&d, "en"), "every task");
    d.tags = vec!["a".into(), "b".into()];
    d.sorts = vec![
        SortDraft {
            by: SortKey::Name,
            desc: false,
        },
        SortDraft {
            by: SortKey::Due,
            desc: false,
        },
    ];
    assert_eq!(
        describe(&d, "en"),
        "Tagged #a or #b, by name, then due date"
    );
}
