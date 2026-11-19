//! Small Windows integrations: start with Windows and the alert sound.

/// Resolve a known client origin to its Z-order. Called only when dropping a memo.
pub fn window_rank_at_origin(x: f32, y: f32) -> Option<usize> {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::Foundation::POINT;
        use windows_sys::Win32::Graphics::Gdi::ClientToScreen;
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GetTopWindow, GetWindow, GetWindowThreadProcessId, IsIconic, IsWindowVisible,
            GW_HWNDNEXT,
        };
        let mut window = GetTopWindow(std::ptr::null_mut());
        let mut rank = 0;
        while !window.is_null() {
            let mut process = 0;
            GetWindowThreadProcessId(window, &mut process);
            if process == std::process::id()
                && IsWindowVisible(window) != 0
                && IsIconic(window) == 0
            {
                let mut origin = POINT { x: 0, y: 0 };
                if ClientToScreen(window, &mut origin) != 0
                    && (origin.x as f32 - x).abs() < 2.
                    && (origin.y as f32 - y).abs() < 2.
                {
                    return Some(rank);
                }
            }
            window = GetWindow(window, GW_HWNDNEXT);
            rank += 1;
        }
    }
    #[cfg(not(windows))]
    let _ = (x, y);
    None
}

/// Which of this program's windows (given by their client origins, physical
/// pixels) is really seen under the mouse pointer: windows are walked from
/// the top; our own other windows (the sticker being carried, menus) are
/// looked through, and anything else there (another program, the desktop)
/// hides what is below. `None` when no listed window shows there; `Some(None)`
/// when it cannot be told (not Windows).
#[cfg_attr(test, allow(dead_code))]
pub fn window_seen_under_pointer(origins: &[Option<(f32, f32)>]) -> Option<Option<usize>> {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::Foundation::{POINT, RECT};
        use windows_sys::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
        use windows_sys::Win32::Graphics::Gdi::ClientToScreen;
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GetCursorPos, GetTopWindow, GetWindow, GetWindowLongW, GetWindowRect,
            GetWindowThreadProcessId, IsIconic, IsWindowVisible, GWL_EXSTYLE, GW_HWNDNEXT,
            WS_EX_TRANSPARENT,
        };
        let mut p = POINT { x: 0, y: 0 };
        if GetCursorPos(&mut p) == 0 {
            return Some(None);
        }
        let mut window = GetTopWindow(std::ptr::null_mut());
        while !window.is_null() {
            let mut r: RECT = std::mem::zeroed();
            let mut cloaked = 0u32;
            DwmGetWindowAttribute(
                window,
                DWMWA_CLOAKED as u32,
                &mut cloaked as *mut u32 as *mut _,
                4,
            );
            let seen = IsWindowVisible(window) != 0
                && IsIconic(window) == 0
                && cloaked == 0
                && GetWindowLongW(window, GWL_EXSTYLE) as u32 & WS_EX_TRANSPARENT == 0
                && GetWindowRect(window, &mut r) != 0
                && p.x >= r.left
                && p.x < r.right
                && p.y >= r.top
                && p.y < r.bottom;
            if seen {
                let mut process = 0;
                GetWindowThreadProcessId(window, &mut process);
                if process != std::process::id() {
                    return None;
                }
                let mut origin = POINT { x: 0, y: 0 };
                if ClientToScreen(window, &mut origin) != 0 {
                    let hit = origins.iter().position(|o| {
                        o.is_some_and(|(x, y)| {
                            (origin.x as f32 - x).abs() < 2. && (origin.y as f32 - y).abs() < 2.
                        })
                    });
                    if hit.is_some() {
                        return Some(hit);
                    }
                }
            }
            window = GetWindow(window, GW_HWNDNEXT);
        }
        None
    }
    #[cfg(not(windows))]
    {
        let _ = origins;
        Some(None)
    }
}

/// The program was called OmniMemoJang before: its data folders move to the
/// new name once, so nothing saved is lost. Does nothing when the new folder
/// exists already or there is no old one.
pub fn migrate_old_data_dirs() {
    for var in ["APPDATA", "LOCALAPPDATA"] {
        let Some(base) = std::env::var_os(var).map(std::path::PathBuf::from) else {
            continue;
        };
        let (old, new) = (base.join("OmniMemoJang"), base.join("OhMemoJang"));
        if old.is_dir() && !new.exists() {
            let _ = std::fs::rename(&old, &new);
        }
    }
}

/// Start (or stop starting) OhMemoJang when the user signs in to Windows:
/// the per-user `Run` key, so no administrator rights are needed.
pub fn set_autostart(on: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Registry::{
            RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
            KEY_SET_VALUE, REG_SZ,
        };
        let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
        let key = wide(r"Software\Microsoft\Windows\CurrentVersion\Run");
        let name = wide("OhMemoJang");
        let old_name = wide("OmniMemoJang");
        let mut h: HKEY = std::ptr::null_mut();
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        if on && runs_from_temp(&exe) {
            // Opened straight from the zip: that folder is gone later.
            return Err(
                "압축 파일 안에서 실행 중이라 자동 실행을 켤 수 없어요 · 압축을 푼 뒤 실행해 주세요"
                    .into(),
            );
        }
        // SAFETY: plain registry calls on a key this function opens and closes.
        unsafe {
            if RegOpenKeyExW(HKEY_CURRENT_USER, key.as_ptr(), 0, KEY_SET_VALUE, &mut h) != 0 {
                return Err("시작 프로그램 설정을 열지 못했습니다".into());
            }
            // The entry the program had under its old name must not start it twice.
            RegDeleteValueW(h, old_name.as_ptr());
            let result = if on {
                // Started by Windows: straight to the tray, stickers on the desktop.
                let value = wide(&format!("\"{}\" --tray", exe.display()));
                RegSetValueExW(
                    h,
                    name.as_ptr(),
                    0,
                    REG_SZ,
                    value.as_ptr() as *const u8,
                    (value.len() * 2) as u32,
                )
            } else {
                let r = RegDeleteValueW(h, name.as_ptr());
                // Already absent is fine.
                if r == 2 {
                    0
                } else {
                    r
                }
            };
            RegCloseKey(h);
            if result != 0 {
                return Err(format!("시작 프로그램 설정을 바꾸지 못했습니다 ({result})"));
            }
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = on;
        Ok(())
    }
}

/// True for a program started from the temporary folder (a zip opened in
/// Explorer runs its files from there).
pub fn runs_from_temp(exe: &std::path::Path) -> bool {
    let Some(temp) = std::env::var_os("TEMP").or_else(|| std::env::var_os("TMP")) else {
        return false;
    };
    let lower = |p: &std::path::Path| p.to_string_lossy().to_lowercase().replace('/', "\\");
    let temp = lower(std::path::Path::new(&temp));
    !temp.is_empty() && lower(exe).starts_with(&format!("{}\\", temp.trim_end_matches('\\')))
}

/// True when the program's start-up entry is turned off in Windows (Task
/// Manager or 설정 › 앱 › 시작 프로그램): Windows then skips it at sign-in.
pub fn autostart_turned_off_in_windows() -> bool {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Registry::{
            RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_BINARY,
        };
        let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
        let key = wide(r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run");
        let name = wide("OhMemoJang");
        let mut data = [0u8; 12];
        let mut size = data.len() as u32;
        // SAFETY: reads at most `size` bytes into `data`; no pointer is kept.
        let r = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_BINARY,
                std::ptr::null_mut(),
                data.as_mut_ptr().cast(),
                &mut size,
            )
        };
        // The first byte is even when enabled (2, 6) and odd when turned off (3, 7).
        r == 0 && size > 0 && data[0] & 1 == 1
    }
    #[cfg(not(windows))]
    false
}

/// `%LOCALAPPDATA%\OhMemoJang\logs`
fn log_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .map(|d| std::path::PathBuf::from(d).join("OhMemoJang").join("logs"))
}

/// Adds a line to a small log in the logs folder, keeping its last 200 lines.
/// Which graphics driver draws the windows, one line per start in
/// `logs\gl.log`: the first thing to look at when windows come up white.
pub fn log_gl(gl: &eframe::glow::Context) {
    use eframe::glow::HasContext as _;
    // SAFETY: called on the thread that owns the current GL context, while
    // it is current (eframe hands it to the app at creation).
    let (vendor, renderer, version) = unsafe {
        (
            gl.get_parameter_string(eframe::glow::VENDOR),
            gl.get_parameter_string(eframe::glow::RENDERER),
            gl.get_parameter_string(eframe::glow::VERSION),
        )
    };
    log_line("gl.log", &format!("{vendor} | {renderer} | {version}"));
}

/// Diagnostic: with `OHMEMOJANG_GL_LOG=<file>` set, eframe/egui_glow debug
/// messages (GL renderer, chosen pixel format) go to that file.
pub fn gl_log_init() {
    struct FileLog(std::sync::Mutex<std::fs::File>);
    impl log::Log for FileLog {
        fn enabled(&self, m: &log::Metadata) -> bool {
            m.level() <= log::Level::Debug
        }
        fn log(&self, r: &log::Record) {
            if self.enabled(r.metadata()) {
                use std::io::Write;
                if let Ok(mut f) = self.0.lock() {
                    let _ = writeln!(f, "{} {}: {}", r.level(), r.target(), r.args());
                }
            }
        }
        fn flush(&self) {}
    }
    let Some(path) = std::env::var_os("OHMEMOJANG_GL_LOG") else {
        return;
    };
    let Ok(f) = std::fs::File::create(path) else {
        return;
    };
    if log::set_boxed_logger(Box::new(FileLog(std::sync::Mutex::new(f)))).is_ok() {
        log::set_max_level(log::LevelFilter::Debug);
    }
}
pub fn log_line(file: &str, line: &str) {
    let Some(dir) = log_dir() else { return };
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(file);
    let old = std::fs::read_to_string(&path).unwrap_or_default();
    let mut lines: Vec<&str> = old.lines().collect();
    let skip = lines.len().saturating_sub(199);
    lines.drain(..skip);
    let stamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    let text = format!(
        "{}{}{stamp} {line}\n",
        lines.join("\n"),
        if lines.is_empty() { "" } else { "\n" }
    );
    let _ = std::fs::write(&path, text);
}

/// A crash is written to `logs\crash.log` before the program ends, so a
/// start that fails without a window (at sign-in, say) can be told apart.
pub fn log_crashes() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let place = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_default();
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_default();
        let args: Vec<String> = std::env::args().skip(1).collect();
        log_line(
            "crash.log",
            &format!(
                "{} {place} {message} (args: {})",
                env!("CARGO_PKG_VERSION"),
                args.join(" ")
            ),
        );
        previous(info);
    }));
}

/// The Windows notification sound.
pub fn alert_sound() {
    #[cfg(windows)]
    // SAFETY: no pointers involved.
    unsafe {
        windows_sys::Win32::System::Diagnostics::Debug::MessageBeep(
            windows_sys::Win32::UI::WindowsAndMessaging::MB_ICONASTERISK,
        );
    }
}

/// The mouse pointer on the screen, in physical pixels.
pub fn cursor_pos() -> Option<(i32, i32)> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::POINT;
        let mut p = POINT { x: 0, y: 0 };
        // SAFETY: `p` is a valid out parameter.
        let ok = unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut p) };
        (ok != 0).then_some((p.x, p.y))
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// Whether the left mouse button is down right now, whatever window has it.
pub fn left_button_down() -> bool {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};
        // SAFETY: reads the key state only.
        unsafe { (GetAsyncKeyState(i32::from(VK_LBUTTON)) as u16 & 0x8000) != 0 }
    }
    #[cfg(not(windows))]
    {
        false
    }
}

pub fn shift_down() -> bool {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_SHIFT};
        // SAFETY: reads a virtual key state without pointers.
        unsafe { GetAsyncKeyState(i32::from(VK_SHIFT)) < 0 }
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Whether a standing calendar cannot work on Windows build `build`: only
/// Windows 11 24H2 (build 26100) itself; earlier and later builds support it.
pub fn standing_unsupported_on(build: u32) -> bool {
    build == 26100
}
/// [`standing_unsupported_on`] for this PC.
pub fn standing_unsupported() -> bool {
    standing_unsupported_on(windows_build())
}
/// Windows build number (26100 and up: Windows 11 24H2 or later); 0 when it
/// cannot be read.
pub fn windows_build() -> u32 {
    static BUILD: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
    *BUILD.get_or_init(|| {
        #[cfg(windows)]
        // SAFETY: reading one string value into a buffer of the given size.
        unsafe {
            use windows_sys::Win32::System::Registry::{
                RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ,
            };
            let wide = |s: &str| s.encode_utf16().chain([0]).collect::<Vec<u16>>();
            let mut buf = [0u16; 32];
            let mut len = (buf.len() * 2) as u32;
            if RegGetValueW(
                HKEY_LOCAL_MACHINE,
                wide(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion").as_ptr(),
                wide("CurrentBuild").as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                buf.as_mut_ptr() as *mut _,
                &mut len,
            ) == 0
            {
                let n = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
                return String::from_utf16_lossy(&buf[..n])
                    .trim()
                    .parse()
                    .unwrap_or(0);
            }
        }
        0
    })
}

#[cfg(test)]
mod standing_tests {
    #[test]
    fn a_program_run_from_the_temp_folder_is_told_apart() {
        let Some(temp) = std::env::var_os("TEMP").or_else(|| std::env::var_os("TMP")) else {
            return;
        };
        let temp = std::path::PathBuf::from(temp);
        assert!(super::runs_from_temp(
            &temp.join("Temp1_OhMemoJang.zip").join("OhMemoJang.exe")
        ));
        assert!(!super::runs_from_temp(std::path::Path::new(
            r"C:\Users\u\AppData\Local\Programs\OhMemoJang\OhMemoJang.exe"
        )));
    }
    #[test]
    fn only_windows_11_24h2_lacks_the_standing_calendar() {
        use super::standing_unsupported_on as no;
        assert!(no(26100), "24H2");
        assert!(!no(22631), "23H2");
        assert!(!no(26200), "25H2");
        assert!(!no(27000), "later");
        assert!(!no(0), "unknown build");
    }
}
