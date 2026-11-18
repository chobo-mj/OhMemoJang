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

/// The NVIDIA Optimus switch read when the OpenGL driver is chosen.
const GPU_SWITCH: &str = "SHIM_MCCOMPAT";
/// Asks for the integrated GPU on laptops with two (NVIDIA Optimus). The
/// screen is driven by the integrated one: drawing on the discrete GPU kept
/// it awake and copied every frame across, about 16 driver threads and 50 MB
/// per window (measured: 4 windows, 408 MB private and 99 threads on NVIDIA,
/// 69 MB and 17 threads on the integrated GPU). Call before the first
/// window; a choice the user made with the same variable is kept.
pub fn prefer_integrated_gpu() {
    if std::env::var_os(GPU_SWITCH).is_none() {
        std::env::set_var(GPU_SWITCH, "0x800000000");
        GPU_ASKED.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}
static GPU_ASKED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Once the drawing context exists: programs this one starts (a browser for
/// a link, Explorer) must not inherit the choice.
pub fn forget_gpu_choice() {
    if GPU_ASKED.swap(false, std::sync::atomic::Ordering::Relaxed) {
        std::env::remove_var(GPU_SWITCH);
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
        // SAFETY: plain registry calls on a key this function opens and closes.
        unsafe {
            if RegOpenKeyExW(HKEY_CURRENT_USER, key.as_ptr(), 0, KEY_SET_VALUE, &mut h) != 0 {
                return Err("시작 프로그램 설정을 열지 못했습니다".into());
            }
            // The entry the program had under its old name must not start it twice.
            RegDeleteValueW(h, old_name.as_ptr());
            let result = if on {
                let exe = std::env::current_exe().map_err(|e| e.to_string())?;
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
    fn the_gpu_choice_is_made_once_and_not_passed_on() {
        // A choice already made (by the user) is left alone.
        std::env::set_var(super::GPU_SWITCH, "0x800000001");
        super::prefer_integrated_gpu();
        super::forget_gpu_choice();
        assert_eq!(
            std::env::var(super::GPU_SWITCH).as_deref(),
            Ok("0x800000001")
        );
        // Otherwise it is asked for, then gone for programs started later.
        std::env::remove_var(super::GPU_SWITCH);
        super::prefer_integrated_gpu();
        assert_eq!(
            std::env::var(super::GPU_SWITCH).as_deref(),
            Ok("0x800000000")
        );
        super::forget_gpu_choice();
        assert!(std::env::var_os(super::GPU_SWITCH).is_none());
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
