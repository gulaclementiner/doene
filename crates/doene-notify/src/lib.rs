//! Döne notifications: OS toast delivery (Windows backend first, cross-platform stubs).
//!
//! Standalone crate — no dependency on other Döne crates (docs/plan.md §4).

/// Delivers a native OS notification.
pub trait Notifier: Send {
    /// Show a toast; failures are reported, never panic.
    fn notify(&self, title: &str, body: &str) -> Result<(), String>;
}

/// Stub for platforms without a native backend: does nothing.
#[derive(Debug, Default)]
pub struct NullNotifier;

impl Notifier for NullNotifier {
    fn notify(&self, _title: &str, _body: &str) -> Result<(), String> {
        Ok(())
    }
}

/// The notifier for the current platform.
pub fn system_notifier() -> Box<dyn Notifier> {
    #[cfg(windows)]
    {
        Box::new(windows::WindowsNotifier)
    }
    #[cfg(not(windows))]
    {
        Box::new(NullNotifier)
    }
}

/// Current local time as `YYYY-MM-DDTHH:MM` (UTC on platforms without a local-time backend).
pub fn local_minute_now() -> String {
    #[cfg(windows)]
    {
        windows::local_minute_now()
    }
    #[cfg(not(windows))]
    {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as i64);
        format_minute(secs)
    }
}

/// `YYYY-MM-DDTHH:MM` of unix seconds (civil-from-days algorithm).
#[cfg_attr(windows, allow(dead_code))]
fn format_minute(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}",
        rem / 3600,
        rem % 3600 / 60
    )
}

#[cfg(windows)]
mod windows {
    use super::Notifier;
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    #[repr(C)]
    #[derive(Default)]
    struct SystemTime {
        year: u16,
        month: u16,
        dow: u16,
        day: u16,
        hour: u16,
        minute: u16,
        second: u16,
        millis: u16,
    }

    unsafe extern "system" {
        fn GetLocalTime(t: *mut SystemTime);
    }

    pub fn local_minute_now() -> String {
        let mut t = SystemTime::default();
        // SAFETY: `t` is a valid, correctly laid-out SYSTEMTIME out-pointer.
        unsafe { GetLocalTime(&mut t) };
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}",
            t.year, t.month, t.day, t.hour, t.minute
        )
    }

    /// Native toast through the WinRT toast API, driven by a hidden PowerShell
    /// (keeps the crate free of heavy Windows bindings).
    pub struct WindowsNotifier;

    fn xml_escape(s: &str) -> String {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;")
    }

    impl Notifier for WindowsNotifier {
        fn notify(&self, title: &str, body: &str) -> Result<(), String> {
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            let xml = format!(
                "<toast><visual><binding template=\"ToastGeneric\"><text>{}</text><text>{}</text></binding></visual></toast>",
                xml_escape(title),
                xml_escape(body)
            );
            // The XML is passed through an environment variable, never interpolated into the script.
            let script = "[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null;\
                [Windows.Data.Xml.Dom.XmlDocument, Windows.Data.Xml.Dom.XmlDocument, ContentType = WindowsRuntime] | Out-Null;\
                $d = New-Object Windows.Data.Xml.Dom.XmlDocument; $d.LoadXml($env:DONE_TOAST_XML);\
                $t = [Windows.UI.Notifications.ToastNotification]::new($d);\
                [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\\WindowsPowerShell\\v1.0\\powershell.exe').Show($t)";
            Command::new("powershell")
                .args(["-NoProfile", "-NonInteractive", "-Command", script])
                .env("DONE_TOAST_XML", xml)
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()
                .map(|_| ())
                .map_err(|e| e.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_unix_seconds_as_local_minute() {
        assert_eq!(format_minute(0), "1970-01-01T00:00");
        assert_eq!(format_minute(1_741_597_500), "2025-03-10T09:05");
    }

    #[test]
    fn null_notifier_succeeds() {
        assert!(NullNotifier.notify("t", "b").is_ok());
        assert_eq!(local_minute_now().len(), 16);
    }
}
