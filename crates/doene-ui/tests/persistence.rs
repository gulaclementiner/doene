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

#[test]
fn settings_and_language_persist_across_reopen_and_first_run_detects_os_locale() {
    use doene_ui::settings;
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("done.db");
    {
        let svc = open(&db);
        let s = settings::init_language(&svc, Some("sv-SE")).unwrap();
        assert_eq!((s.language.as_str(), s.week_start), ("sv", 1));
        // Later OS locale changes do not override a saved choice.
        assert_eq!(settings::init_language(&svc, Some("en-US")).unwrap().language, "sv");
        settings::set_language(&svc, "en").unwrap();
        settings::set_week_start(&svc, 1).unwrap();
        settings::set_headings(&svc, false).unwrap();
        settings::set_notify_time(&svc, "8:15", "en").unwrap();
        assert!(settings::set_notify_time(&svc, "nope", "en").is_err());
    }
    let svc = open(&db);
    let s = settings::init_language(&svc, Some("sv")).unwrap();
    assert_eq!(s.language, "en");
    assert_eq!((s.week_start, s.due_notify_time.as_str(), s.show_my_day_headings), (1, "08:15", false));
}

#[test]
fn language_switch_rerenders_rows_immediately() {
    let dir = tempfile::tempdir().unwrap();
    let svc = open(&dir.path().join("done.db"));
    svc.quick_add("Buy milk tomorrow", None, false).unwrap();
    let en = all_rows(&svc);
    doene_ui::settings::set_language(&svc, "sv").unwrap();
    let sv = all_rows(&svc);
    assert_eq!(en[0].due.as_deref(), Some("Tomorrow"));
    assert_ne!(sv[0].due, en[0].due);
}
