//! Ticket #10: composer line -> task in All list, and restart keeps data + theme.

use doene_core::{Date, SmartList, SmartSettings};
use doene_service::Service;
use doene_storage::Storage;
use doene_ui::{Theme, list_rows};
use serde_json::json;

fn open(path: &std::path::Path) -> Service {
    Service::with_clock(Storage::open(path).unwrap(), Box::new(|| "2025-03-10T09:00:00.000Z".into()))
}

fn all_rows(svc: &Service) -> Vec<doene_ui::TaskRow> {
    let s = svc.settings().unwrap();
    let settings = SmartSettings { focus_mode: s.focus_mode, ..Default::default() };
    list_rows(SmartList::All, &svc.all_tasks().unwrap(), Date::parse("2025-03-10").unwrap(), settings, &s.language)
}

#[test]
fn composer_line_persists_across_reopen_with_theme() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("done.db");
    let before = {
        let svc = open(&db);
        svc.quick_add("Pay rent tomorrow #home !high", None, false).unwrap();
        svc.quick_add("Buy milk", None, false).unwrap();
        svc.set_setting("theme", json!(Theme::Dark.id())).unwrap();
        all_rows(&svc)
    };
    assert_eq!(before.len(), 2);
    let rent = &before[0];
    assert_eq!(rent.subject, "Pay rent");
    assert_eq!(rent.due.as_deref(), Some("Tomorrow"));
    assert_eq!(rent.tags, ["#home"]);
    assert!(rent.important);

    let svc = open(&db);
    assert_eq!(all_rows(&svc), before);
    assert_eq!(Theme::parse(&svc.settings().unwrap().theme), Theme::Dark);
}
