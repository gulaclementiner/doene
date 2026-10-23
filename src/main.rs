//! Döne composition root (docs/plan.md §4): resolves the data directory,
//! creates it, then wires and runs the eframe app.

use std::env;
use std::path::{Path, PathBuf};

/// Environment variable that overrides the default data directory location.
const DATA_DIR_ENV: &str = "DONE_DATA_DIR";

/// Resolve the Döne data directory: the `DONE_DATA_DIR` override when set to a
/// non-blank value, otherwise `<platform app-data dir>/Döne`.
fn resolve_data_dir(override_dir: Option<&str>, base: &Path) -> PathBuf {
    match override_dir {
        Some(dir) if !dir.trim().is_empty() => PathBuf::from(dir),
        _ => base.join("Döne"),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let base = dirs::data_dir().ok_or("could not determine the platform data directory")?;
    // Data-dir resolution happens before any window creation.
    let data_dir = resolve_data_dir(env::var(DATA_DIR_ENV).ok().as_deref(), &base);
    std::fs::create_dir_all(&data_dir)?;

    let storage = doene_storage::Storage::open(&data_dir.join("done.db"))?;
    let service = doene_service::Service::new(storage);

    eframe::run_native(
        "Döne",
        eframe::NativeOptions::default(),
        Box::new(move |_cc| {
            // First run: adopt the OS locale (a saved choice always wins).
            if let Err(e) =
                doene_ui::settings::init_language(&service, sys_locale::get_locale().as_deref())
            {
                eprintln!("language detection failed: {e:?}");
            }
            let mut app = doene_ui::DöneApp::new(service);
            let notifier = doene_notify::system_notifier();
            app.set_notifier(
                Box::new(move |title, body| {
                    if let Err(e) = notifier.notify(title, body) {
                        eprintln!("notification failed: {e}");
                    }
                }),
                Box::new(doene_notify::local_minute_now),
            );
            Ok(Box::new(app))
        }),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::resolve_data_dir;
    use std::path::{Path, PathBuf};

    #[test]
    fn env_override_is_honoured() {
        let dir = resolve_data_dir(Some("/tmp/done-data"), Path::new("/home/u/.local/share"));
        assert_eq!(dir, PathBuf::from("/tmp/done-data"));
    }

    #[test]
    fn falls_back_to_appdata_done_without_override() {
        let dir = resolve_data_dir(None, Path::new("/home/u/.local/share"));
        assert_eq!(dir, PathBuf::from("/home/u/.local/share/Döne"));
    }

    #[test]
    fn blank_override_is_treated_as_absent() {
        for blank in ["", "   "] {
            let dir = resolve_data_dir(Some(blank), Path::new("/base"));
            assert_eq!(dir, PathBuf::from("/base/Döne"), "blank override {blank:?}");
        }
    }
}
