//! Ticket #23: JSON export/import round trip and malformed-input rejection.

use doene_core::recurrence::RawRecurrence;
use doene_core::{RawView, Tag};
use doene_service::{NewTask, Service, StepPatch, TaskPatch};
use doene_storage::Storage;
use serde_json::{Value, json};

fn svc() -> Service {
    Service::with_clock(Storage::open_in_memory().unwrap(), Box::new(|| "2025-03-10T09:00:00.000Z".to_owned()))
}

fn populated() -> Service {
    let s = svc();
    let a = s.create_task(NewTask { subject: "parent".into(), ..Default::default() }).unwrap();
    s.create_task(NewTask { subject: "child".into(), parent_id: Some(a.task.id.clone()), ..Default::default() }).unwrap();
    s.create_step(&a.task.id, "step one").unwrap();
    let r = s.create_task(NewTask { subject: "daily".into(), ..Default::default() }).unwrap();
    s.update_task(&r.task.id, TaskPatch {
        tags: Some(vec!["home".into(), "x".into()]),
        recurrence: Some(Some(RawRecurrence { kind: Some("daily".into()), interval: None, unit: None, days: None })),
        due_date: Some(Some(doene_core::Date::parse("2025-03-11").unwrap())),
        ..Default::default()
    }).unwrap();
    let gone = s.create_task(NewTask { subject: "trashed".into(), ..Default::default() }).unwrap();
    s.delete_task(&gone.task.id).unwrap();
    s.create_view(RawView { name: Some("V".into()), tags: vec!["home".into()], match_mode: Some("all".into()), ..Default::default() }).unwrap();
    let v2 = s.create_view(RawView { name: Some("W".into()), ..Default::default() }).unwrap();
    s.delete_view(&v2.view.id).unwrap();
    s.set_setting("language", json!("sv")).unwrap();
    s.set_setting("weekStart", json!(0)).unwrap();
    s.set_pref("global", "[\"a\"]").unwrap();
    s.set_pref(&format!("drawer:{}", a.task.id), "{\"x\":1}").unwrap();
    let _ = (Tag::new("q"), StepPatch::default());
    s
}

fn state(s: &Service) -> Value {
    let mut v: Value = serde_json::from_str(&s.export_json().unwrap()).unwrap();
    v.as_object_mut().unwrap().remove("exported_at");
    v
}

#[test]
fn round_trip_into_fresh_db_is_identical() {
    let a = populated();
    let text = a.export_json().unwrap();
    let doc: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(doc["version"], 1);
    assert_eq!(doc["tasks"].as_array().unwrap().len(), 3);
    assert_eq!(doc["trash"].as_array().unwrap().len(), 2);
    assert!(doc.get("fired_notifications").is_none());

    let b = svc();
    let sum = b.import_json(&text).unwrap();
    assert_eq!((sum.tasks, sum.views, sum.trash), (3, 1, 2));
    let (sa, sb) = (state(&a), state(&b));
    for k in sa.as_object().unwrap().keys() { assert_eq!(sa[k], sb[k], "section {k}"); }
    assert_eq!(sa, sb);
    assert_eq!(a.all_tasks().unwrap(), b.all_tasks().unwrap());
    assert_eq!(a.views().unwrap(), b.views().unwrap());
    assert_eq!(a.settings().unwrap(), b.settings().unwrap());
}

#[test]
fn import_wipes_existing_data() {
    let src = populated();
    let dst = svc();
    dst.create_task(NewTask { subject: "old".into(), ..Default::default() }).unwrap();
    dst.set_pref("global", "old").unwrap();
    dst.import_json(&src.export_json().unwrap()).unwrap();
    assert!(dst.all_tasks().unwrap().iter().all(|h| h.task.subject != "old"));
    assert_eq!(dst.pref("global").unwrap().as_deref(), Some("[\"a\"]"));
    assert_eq!(dst.undo_peek(), None, "undo history is cleared");
    // restored trash entries still work
    let t = dst.trash().unwrap();
    assert_eq!(t.len(), 2);
}

fn code(s: &Service, text: &str) -> &'static str {
    s.import_json(text).unwrap_err().code().expect("coded")
}

fn doc() -> Value {
    serde_json::from_str(&populated().export_json().unwrap()).unwrap()
}

#[test]
fn malformed_input_is_rejected_and_changes_nothing() {
    let s = populated();
    let before = state(&s);
    assert_eq!(code(&s, "not json"), "error.invalidJson");
    assert_eq!(code(&s, "[]"), "error.importInvalid");
    assert_eq!(code(&s, "{}"), "error.importInvalid");
    let mut d = doc();
    d["version"] = json!(99);
    assert_eq!(code(&s, &d.to_string()), "error.exportVersion");
    let mut d = doc();
    d.as_object_mut().unwrap().remove("version");
    assert_eq!(code(&s, &d.to_string()), "error.exportVersion");

    type Break = fn(&mut Value);
    let cases: [(&str, Break); 11] = [
        ("tasks not array", |d| d["tasks"] = json!("x")),
        ("task missing subject", |d| { d["tasks"][0].as_object_mut().unwrap().remove("subject"); }),
        ("duplicate task id", |d| d["tasks"][1]["id"] = d["tasks"][0]["id"].clone()),
        ("dangling parent", |d| d["tasks"][0]["parent_id"] = json!("nope")),
        ("self parent", |d| { let id = d["tasks"][0]["id"].clone(); d["tasks"][0]["parent_id"] = id; }),
        ("bad tag", |d| d["tasks"][0]["tags"] = json!(["bad tag!"])),
        ("view without name", |d| d["views"][0]["name"] = json!("  ")),
        ("trash bad kind", |d| d["trash"][0]["kind"] = json!("zzz")),
        ("unknown setting", |d| d["settings"]["bogus"] = json!(1)),
        ("bad setting value", |d| d["settings"]["weekStart"] = json!(7)),
        ("bad pref scope", |d| d["prefs"]["weird"] = json!("v")),
    ];
    for (name, f) in cases {
        let mut d = doc();
        f(&mut d);
        let c = s.import_json(&d.to_string()).unwrap_err().code().unwrap_or("storage");
        assert!(c.starts_with("error."), "{name}: {c}");
        assert_eq!(state(&s), before, "{name}: db changed");
    }
}

#[test]
fn storage_failure_mid_import_rolls_back() {
    // Duplicate activity-free path: a step id repeated across tasks is caught up front.
    let s = populated();
    let before = state(&s);
    let mut d = doc();
    let tasks = d["tasks"].as_array().unwrap().clone();
    let with_step = tasks.iter().position(|t| !t["steps"].as_array().unwrap().is_empty()).unwrap();
    let other = (with_step + 1) % tasks.len();
    d["tasks"][other]["steps"] = tasks[with_step]["steps"].clone();
    assert_eq!(code(&s, &d.to_string()), "error.importInvalid");
    assert_eq!(state(&s), before);
}
