//! Settings popover logic (ticket #22): plain functions, no egui types.
//! Week start, due-notification time, My Day group headings, language.

use doene_core::dates::week_days_from;
use doene_core::i18n::{detect_language, locales, tr};
use doene_service::{Service, ServiceError, Settings};
use serde_json::json;
use std::collections::HashMap;

/// Locale keys the popover renders (checked for EN/SV parity in tests).
pub const KEYS: &[&str] = &[
    "nav.preferences",
    "settings.preferencesTitle",
    "settings.close",
    "settings.language",
    "settings.language.en",
    "settings.language.sv",
    "settings.startOfWeek",
    "settings.startOfWeek.mon",
    "settings.startOfWeek.sun",
    "settings.myDay",
    "settings.myDaySubHeadings",
    "settings.appearance",
    "settings.dueNotifyTime",
    "settings.data",
    "settings.dataPath",
    "settings.exportData",
    "settings.importData",
    "settings.exported",
    "settings.imported",
    "settings.fileError",
];

/// Trim and validate a typed `HH:MM`; `None` when malformed.
pub fn parse_notify_time(input: &str) -> Option<String> {
    let t = input.trim();
    let (h, m) = t.split_once(':')?;
    if h.is_empty()
        || h.len() > 2
        || m.len() != 2
        || !(h.bytes().chain(m.bytes())).all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let (h, m): (u8, u8) = (h.parse().ok()?, m.parse().ok()?);
    (h < 24 && m < 60).then(|| format!("{h:02}:{m:02}"))
}

/// Locale key of a language choice's label.
pub fn language_label_key(lang: &str) -> String {
    format!("settings.language.{lang}")
}

/// The selectable languages.
pub fn languages() -> Vec<&'static str> {
    locales()
}

/// Weekday numbers (0 = Sunday) in the picker order for the week-start setting.
pub fn weekday_order(week_start: u8) -> [u8; 7] {
    week_days_from(week_start)
}

/// Locale key of a My Day group heading, or `None` when headings are hidden.
pub fn group_heading_key(show: bool, title_key: &str) -> Option<String> {
    show.then(|| title_key.to_owned())
}

/// First run (no saved language): pick the OS locale's best supported
/// language and persist it. Returns the current settings either way.
pub fn init_language(service: &Service, os_locale: Option<&str>) -> Result<Settings, ServiceError> {
    let s = service.settings()?;
    if s.language_chosen {
        return Ok(s);
    }
    service.set_setting("language", json!(detect_language(os_locale)))
}

/// Switch language (applies on the next render; persisted).
pub fn set_language(service: &Service, lang: &str) -> Result<Settings, ServiceError> {
    service.set_setting("language", json!(lang))
}

/// Set week start (0 Sunday, 1 Monday).
pub fn set_week_start(service: &Service, start: u8) -> Result<Settings, ServiceError> {
    service.set_setting("weekStart", json!(start))
}

/// Show/hide My Day group sub-headings.
pub fn set_headings(service: &Service, show: bool) -> Result<Settings, ServiceError> {
    service.set_setting("showMyDayGroupHeadings", json!(show))
}

/// Save a typed notification time; `Err` is a localised message.
pub fn set_notify_time(service: &Service, input: &str, locale: &str) -> Result<Settings, String> {
    let Some(t) = parse_notify_time(input) else {
        return Err(tr("settings.dueNotifyTime", &HashMap::new(), Some(locale)) + " (HH:MM)");
    };
    service
        .set_setting("dueNotifyTime", json!(t))
        .map_err(|e| crate::drawer::error_message(&e, locale))
}

fn msg(key: &str, params: &[(&str, String)], locale: &str) -> String {
    let p: HashMap<String, Option<String>> = params
        .iter()
        .map(|(k, v)| ((*k).to_owned(), Some(v.clone())))
        .collect();
    tr(key, &p, Some(locale))
}

/// Export all data to the file at `path`; `Ok` is a localised confirmation.
pub fn export_to_path(service: &Service, path: &str, locale: &str) -> Result<String, String> {
    let path = path.trim();
    let file_err = |e: String| msg("settings.fileError", &[("reason", e)], locale);
    if path.is_empty() {
        return Err(file_err(msg("settings.dataPath", &[], locale)));
    }
    let text = service
        .export_json()
        .map_err(|e| crate::drawer::error_message(&e, locale))?;
    std::fs::write(path, text).map_err(|e| file_err(e.to_string()))?;
    Ok(msg(
        "settings.exported",
        &[("path", path.to_owned())],
        locale,
    ))
}

/// Replace all data with the export file at `path`; `Ok` is a localised confirmation.
pub fn import_from_path(service: &Service, path: &str, locale: &str) -> Result<String, String> {
    let path = path.trim();
    let file_err = |e: String| msg("settings.fileError", &[("reason", e)], locale);
    if path.is_empty() {
        return Err(file_err(msg("settings.dataPath", &[], locale)));
    }
    let text = std::fs::read_to_string(path).map_err(|e| file_err(e.to_string()))?;
    let sum = service
        .import_json(&text)
        .map_err(|e| crate::drawer::error_message(&e, locale))?;
    Ok(msg(
        "settings.imported",
        &[("tasks", sum.tasks.to_string())],
        locale,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notify_time_parsing() {
        assert_eq!(parse_notify_time(" 7:05 ").as_deref(), Some("07:05"));
        assert_eq!(parse_notify_time("23:59").as_deref(), Some("23:59"));
        for bad in [
            "", "24:00", "12:60", "1205", "ab:cd", "12:5", "-1:00", "123:00",
        ] {
            assert_eq!(parse_notify_time(bad), None, "{bad}");
        }
    }

    #[test]
    fn weekday_order_follows_start() {
        assert_eq!(weekday_order(1), [1, 2, 3, 4, 5, 6, 0]);
        assert_eq!(weekday_order(0)[0], 0);
    }

    #[test]
    fn headings_toggle() {
        assert_eq!(
            group_heading_key(true, "myday.group.radar").as_deref(),
            Some("myday.group.radar")
        );
        assert_eq!(group_heading_key(false, "myday.group.radar"), None);
    }

    #[test]
    fn every_settings_key_exists_in_all_locales() {
        for l in languages() {
            for k in KEYS
                .iter()
                .copied()
                .chain(languages().iter().map(|_| "settings.language"))
            {
                let v = tr(k, &HashMap::new(), Some(l));
                assert!(!v.is_empty() && v != k, "missing {k} in {l}");
            }
            let label = language_label_key(l);
            assert_ne!(tr(&label, &HashMap::new(), Some("en")), label);
            assert_ne!(tr(&label, &HashMap::new(), Some("sv")), label);
        }
    }

    #[test]
    fn export_then_import_through_files() {
        use doene_service::NewTask;
        let mk = || Service::new(doene_storage::Storage::open_in_memory().unwrap());
        let (a, b) = (mk(), mk());
        a.create_task(NewTask {
            subject: "x".into(),
            ..Default::default()
        })
        .unwrap();
        let dir = std::env::temp_dir().join(format!("doene-exp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("e.json");
        let p = f.to_str().unwrap();
        assert!(
            export_to_path(&a, p, "sv")
                .unwrap()
                .starts_with("Exporterat")
        );
        assert_eq!(import_from_path(&b, p, "en").unwrap(), "Imported 1 tasks");
        assert_eq!(b.all_tasks().unwrap().len(), 1);
        std::fs::write(&f, "garbage").unwrap();
        assert_eq!(import_from_path(&b, p, "en").unwrap_err(), "invalid JSON");
        assert!(import_from_path(&b, "", "en").is_err());
        assert!(
            import_from_path(&b, "/nonexistent/zz.json", "en")
                .unwrap_err()
                .starts_with("Cannot use file")
        );
        std::fs::remove_dir_all(dir).ok();
    }
}
