//! Ticket #17: palette task results follow core search scoring.

use doene_service::{NewTask, Service};
use doene_storage::Storage;
use doene_ui::palette::{Action, build_entries};

#[test]
fn task_entries_ordered_by_core_score() {
    let s = Service::with_clock(
        Storage::open_in_memory().unwrap(),
        Box::new(|| "2025-03-10T09:00:00.000Z".into()),
    );
    let mk = |subject: &str, body: Option<&str>| {
        s.create_task(NewTask {
            subject: subject.into(),
            body: body.map(Into::into),
            ..Default::default()
        })
        .unwrap()
        .task
        .id
    };
    let body_hit = mk("Zebra", Some("buy milk later"));
    let substr = mk("Smilkway", None);
    let whole = mk("Get milk", None);
    let hits = s.search("milk", 8).unwrap();
    let e = build_entries("milk", &s.views().unwrap(), &hits, false, "en");
    let tasks: Vec<_> = e
        .iter()
        .filter_map(|x| {
            if let Action::OpenTask(id) = &x.action {
                Some(id.clone())
            } else {
                None
            }
        })
        .collect();
    assert_eq!(tasks, vec![whole, substr, body_hit]);
}
