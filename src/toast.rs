//! Memo alarms as Windows notifications (알림 센터 토스트).
//!
//! The program has no installer-registered identity, so it registers one for
//! the current user (`HKCU\Software\Classes\AppUserModelId\<id>`: a name and
//! an icon) and uses it for its own process. Each alarm becomes a reminder
//! toast with 열기 · 5분 뒤 · 확인 buttons; a press comes back to the program
//! through `take_answers` (it is woken up for it). If Windows refuses the
//! toast, `show` fails and the alarm uses the program's own popup instead.
use std::sync::Mutex;

pub const APP_ID: &str = "OhMemoJang.App";

/// What was pressed on an alarm's notification.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Answer {
    Open(usize, Option<char>),
    Snooze(usize, Option<char>),
    Done(usize, Option<char>),
}

static ANSWERS: Mutex<Vec<Answer>> = Mutex::new(Vec::new());

/// Buttons open `ohmemojang:<what>:<memo>` links: Windows starts the program
/// with the link, whether or not it is running, so a press is never lost.
pub const SCHEME: &str = "ohmemojang:";

/// The notification press this launch was started for, if any.
pub fn launch_answer() -> Option<String> {
    std::env::args().find(|a| a.starts_with(SCHEME))
}

/// Presses handed over by a later launch, waiting for the running copy.
fn queue_file() -> Option<std::path::PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|d| {
        std::path::PathBuf::from(d)
            .join("OhMemoJang")
            .join("notification_answers.txt")
    })
}

/// A second launch (started for a press) leaves it for the running copy.
pub fn queue_for_running(arguments: &str) {
    use std::io::Write;
    if let Some(path) = queue_file() {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let _ = writeln!(f, "{arguments}");
        }
    }
}

/// Take the presses other launches left; true when there were any.
pub fn take_queued() -> bool {
    let Some(path) = queue_file() else {
        return false;
    };
    let Ok(text) = std::fs::read_to_string(&path) else {
        return false;
    };
    let _ = std::fs::remove_file(&path);
    let mut any = false;
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        answer(line.trim());
        any = true;
    }
    any
}

/// Presses since the last call.
pub fn take_answers() -> Vec<Answer> {
    std::mem::take(&mut *ANSWERS.lock().unwrap_or_else(|e| e.into_inner()))
}

pub fn answer(arguments: &str) {
    let arguments = arguments.strip_prefix(SCHEME).unwrap_or(arguments);
    // "<what>:<memo>" or "<what>:<memo><which alarm>" (m, p, x).
    let parsed = arguments.split_once(':').and_then(|(what, token)| {
        let digits: String = token.chars().take_while(char::is_ascii_digit).collect();
        let note: usize = digits.parse().ok()?;
        let letter = token[digits.len()..].chars().next();
        Some(match what {
            "open" => Answer::Open(note, letter),
            "snooze" => Answer::Snooze(note, letter),
            "done" => Answer::Done(note, letter),
            _ => return None,
        })
    });
    if let Some(a) = parsed {
        ANSWERS.lock().unwrap_or_else(|e| e.into_inner()).push(a);
        crate::wake::now();
    }
}

/// Characters XML would read as markup.
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The notification for `note`'s alarm.
pub fn xml(note: usize, which: char, title: &str, when: &str, sound: bool) -> String {
    let id = format!("{note}{which}");
    let title = if title.trim().is_empty() {
        "제목 없음"
    } else {
        title
    };
    let audio = if sound {
        r#"<audio src="ms-winsoundevent:Notification.Reminder"/>"#
    } else {
        r#"<audio silent="true"/>"#
    };
    format!(
        r#"<toast launch="{SCHEME}open:{id}" activationType="protocol" scenario="reminder"><visual><binding template="ToastGeneric"><text>{}</text><text>{} · OhMemoJang 알람</text></binding></visual><actions><action content="열기" arguments="{SCHEME}open:{id}" activationType="protocol"/><action content="5분 뒤" arguments="{SCHEME}snooze:{id}" activationType="protocol"/><action content="확인" arguments="{SCHEME}done:{id}" activationType="protocol"/></actions>{audio}</toast>"#,
        escape(title),
        escape(when),
    )
}

#[cfg(windows)]
mod imp {
    use super::*;
    use windows::core::{Interface, HSTRING};
    use windows::Data::Xml::Dom::XmlDocument;
    use windows::Foundation::TypedEventHandler;
    use windows::UI::Notifications::{
        ToastActivatedEventArgs, ToastDismissalReason, ToastNotification, ToastNotificationManager,
    };

    /// Shown notifications are kept so their buttons keep answering.
    static SHOWN: Mutex<Vec<ToastNotification>> = Mutex::new(Vec::new());
    static READY: Mutex<Option<bool>> = Mutex::new(None);

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain([0]).collect()
    }

    /// Give the program a notification identity: a name and an icon under
    /// the current user, used by this process. Done once.
    fn register() -> bool {
        use windows_sys::Win32::System::Registry::*;
        use windows_sys::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
        let icon = std::env::var_os("LOCALAPPDATA")
            .map(|d| std::path::PathBuf::from(d).join("OhMemoJang"))
            .and_then(|dir| {
                std::fs::create_dir_all(&dir).ok()?;
                let path = dir.join("notification_icon.png");
                std::fs::write(&path, crate::app_icon::png(128)).ok()?;
                Some(path)
            });
        // SAFETY: plain registry calls on a key of the current user.
        unsafe {
            let key_path = wide(&format!("Software\\Classes\\AppUserModelId\\{APP_ID}"));
            let mut key = std::ptr::null_mut();
            if RegCreateKeyExW(
                HKEY_CURRENT_USER,
                key_path.as_ptr(),
                0,
                std::ptr::null(),
                0,
                KEY_SET_VALUE,
                std::ptr::null(),
                &mut key,
                std::ptr::null_mut(),
            ) != 0
            {
                return false;
            }
            let set = |name: &str, value: &str| {
                let v = wide(value);
                RegSetValueExW(
                    key,
                    wide(name).as_ptr(),
                    0,
                    REG_SZ,
                    v.as_ptr() as *const u8,
                    (v.len() * 2) as u32,
                );
            };
            set("DisplayName", "OhMemoJang");
            // The `ohmemojang:` links the notification buttons open.
            if let Ok(exe) = std::env::current_exe() {
                let put = |path: &str, name: Option<&str>, value: &str| {
                    let mut k = std::ptr::null_mut();
                    if RegCreateKeyExW(
                        HKEY_CURRENT_USER,
                        wide(path).as_ptr(),
                        0,
                        std::ptr::null(),
                        0,
                        KEY_SET_VALUE,
                        std::ptr::null(),
                        &mut k,
                        std::ptr::null_mut(),
                    ) == 0
                    {
                        let v = wide(value);
                        let n = name.map(wide);
                        RegSetValueExW(
                            k,
                            n.as_ref().map_or(std::ptr::null(), |n| n.as_ptr()),
                            0,
                            REG_SZ,
                            v.as_ptr() as *const u8,
                            (v.len() * 2) as u32,
                        );
                        RegCloseKey(k);
                    }
                };
                let base = r"Software\Classes\ohmemojang";
                put(base, None, "URL:OhMemoJang 알림");
                put(base, Some("URL Protocol"), "");
                put(
                    &format!(r"{base}\shell\open\command"),
                    None,
                    &format!("\"{}\" \"%1\"", exe.display()),
                );
            }
            if let Some(icon) = &icon {
                set("IconUri", &icon.to_string_lossy());
            }
            RegCloseKey(key);
            SetCurrentProcessExplicitAppUserModelID(wide(APP_ID).as_ptr());
        }
        shortcut().is_ok()
    }

    /// Windows shows notifications only for an identity it finds on a Start
    /// menu shortcut: put it on the program's shortcut (the installer's, or
    /// one made here for a copy that was not installed).
    fn shortcut() -> windows::core::Result<()> {
        use windows::Win32::Storage::EnhancedStorage::PKEY_AppUserModel_ID;
        use windows::Win32::System::Com::{
            CoCreateInstance, CoInitializeEx, IPersistFile, CLSCTX_INPROC_SERVER,
            COINIT_APARTMENTTHREADED, STGM_READWRITE,
        };
        use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
        use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
        // The real Start menu folder (not from the environment).
        // SAFETY: the returned string is freed right after use.
        let programs = unsafe {
            use windows::Win32::UI::Shell::{
                FOLDERID_Programs, SHGetKnownFolderPath, KF_FLAG_DEFAULT,
            };
            let p = SHGetKnownFolderPath(&FOLDERID_Programs, KF_FLAG_DEFAULT, None)?;
            let s = p.to_string().unwrap_or_default();
            windows::Win32::System::Com::CoTaskMemFree(Some(p.0 as *const _));
            std::path::PathBuf::from(s).join("OhMemoJang")
        };
        let path = programs.join("OhMemoJang.lnk");
        let exe = std::env::current_exe().unwrap_or_default();
        // SAFETY: COM calls on this (the UI) thread, which winit set up.
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
            let file: IPersistFile = link.cast()?;
            if path.exists() {
                file.Load(&HSTRING::from(path.as_os_str()), STGM_READWRITE)?;
            } else {
                let _ = std::fs::create_dir_all(&programs);
                link.SetPath(&HSTRING::from(exe.as_os_str()))?;
                if let Some(dir) = exe.parent() {
                    link.SetWorkingDirectory(&HSTRING::from(dir.as_os_str()))?;
                }
            }
            let store: IPropertyStore = link.cast()?;
            store.SetValue(
                &PKEY_AppUserModel_ID,
                &windows::core::PROPVARIANT::from(APP_ID),
            )?;
            store.Commit()?;
            file.Save(&HSTRING::from(path.as_os_str()), true)?;
        }
        Ok(())
    }

    /// Register the notification identity now: Windows drops the first
    /// notification sent right after its Start menu shortcut appears.
    pub fn prepare() -> bool {
        *READY
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get_or_insert_with(register)
    }

    pub fn show(
        note: usize,
        which: char,
        title: &str,
        when: &str,
        sound: bool,
    ) -> Result<(), String> {
        let ready = prepare();
        if !ready {
            return Err("알림 등록 실패".into());
        }
        let err = |e: windows::core::Error| e.message().to_string();
        let id = format!("{note}{which}");
        let id2 = id.clone();
        let doc = XmlDocument::new().map_err(err)?;
        doc.LoadXml(&HSTRING::from(xml(note, which, title, when, sound)))
            .map_err(err)?;
        let toast = ToastNotification::CreateToastNotification(&doc).map_err(err)?;
        // The same memo's newer alarm replaces its older notification.
        toast
            .SetTag(&HSTRING::from(note.to_string()))
            .map_err(err)?;
        toast.SetGroup(&HSTRING::from("alarm")).map_err(err)?;
        toast
            .Activated(&TypedEventHandler::new(
                move |_: &Option<ToastNotification>, args: &Option<windows::core::IInspectable>| {
                    let pressed = args
                        .as_ref()
                        .and_then(|a| a.cast::<ToastActivatedEventArgs>().ok())
                        .and_then(|a| a.Arguments().ok())
                        .map(|a| a.to_string())
                        .unwrap_or_else(|| format!("open:{id}"));
                    answer(&pressed);
                    Ok(())
                },
            ))
            .map_err(err)?;
        toast
            .Dismissed(&TypedEventHandler::new(
                move |_: &Option<ToastNotification>,
                      args: &Option<windows::UI::Notifications::ToastDismissedEventArgs>| {
                    // Swiped away: the same as 확인. A notification that only
                    // timed out stays in the notification center.
                    if args
                        .as_ref()
                        .and_then(|a| a.Reason().ok())
                        .is_some_and(|r| r == ToastDismissalReason::UserCanceled)
                    {
                        answer(&format!("done:{id2}"));
                    }
                    Ok(())
                },
            ))
            .map_err(err)?;
        let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(APP_ID))
            .map_err(err)?;

        // Notifications turned off for the program (or by the user or a
        // policy) are accepted and then never shown: the popup is used.
        let setting = notifier.Setting().map_err(err)?;
        if setting != windows::UI::Notifications::NotificationSetting::Enabled {
            return Err("Windows 알림이 꺼져 있음".into());
        }
        notifier.Show(&toast).map_err(err)?;
        let mut shown = SHOWN.lock().unwrap_or_else(|e| e.into_inner());
        shown.push(toast);
        if shown.len() > 50 {
            shown.remove(0);
        }
        Ok(())
    }

    /// Take a memo's alarm notification out of the notification center.
    pub fn remove(note: usize) {
        if READY.lock().map(|r| *r != Some(true)).unwrap_or(true) {
            return;
        }
        if let Ok(history) = ToastNotificationManager::History() {
            let _ = history.RemoveGroupedTagWithId(
                &HSTRING::from(note.to_string()),
                &HSTRING::from("alarm"),
                &HSTRING::from(APP_ID),
            );
        }
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn prepare() -> bool {
        false
    }
    pub fn show(_: usize, _: char, _: &str, _: &str, _: bool) -> Result<(), String> {
        Err("Windows only".into())
    }
    pub fn remove(_: usize) {}
}

pub use imp::{prepare, remove, show};

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn notification_text_is_escaped_and_buttons_carry_the_memo() {
        let x = xml(7, 'm', "a<b & \"c\"", "11월 18일 09:00", true);
        assert!(x.contains("a&lt;b &amp; &quot;c&quot;"));
        for a in [
            "ohmemojang:open:7",
            "ohmemojang:snooze:7",
            "ohmemojang:done:7",
        ] {
            assert!(x.contains(a));
        }
        assert!(x.contains("Notification.Reminder"));
        assert!(xml(1, 'm', "", "", false).contains(r#"silent="true""#));
        assert!(xml(1, 'm', "", "", false).contains("제목 없음"));
    }
    #[test]
    fn presses_come_back_as_answers() {
        let _ = take_answers();
        answer("open:3");
        answer("snooze:4");
        answer("done:5");
        answer("bogus");
        answer("ohmemojang:done:6");
        answer("done:8p");
        assert_eq!(
            take_answers(),
            vec![
                Answer::Open(3, None),
                Answer::Snooze(4, None),
                Answer::Done(5, None),
                Answer::Done(6, None),
                Answer::Done(8, Some('p'))
            ]
        );
    }
}
