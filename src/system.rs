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

/// The NVIDIA Optimus switch, read when a window's drawing context is made.
const GPU_SWITCH: &str = "SHIM_MCCOMPAT";
const INTEGRATED: &str = "0x800000000";
/// The values a person sets by hand (integrated, NVIDIA). Others are
/// inherited: a program running on the NVIDIA GPU passes `0x810000001` to
/// the programs it starts (seen from a terminal opened by such a program).
fn chosen_by_hand(value: Option<&std::ffi::OsStr>) -> bool {
    value.is_some_and(|v| v == INTEGRATED || v == "0x800000001")
}
/// The switch as it was before [`prefer_integrated_gpu`], to put back.
static GPU_BEFORE: std::sync::Mutex<Option<Option<std::ffi::OsString>>> =
    std::sync::Mutex::new(None);
static GPU_ASKED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// On laptops with two GPUs (NVIDIA Optimus) the program draws on the
/// integrated one, which drives the screen. Drawing on the NVIDIA GPU kept it
/// awake and copied every frame across; its driver also took about 16
/// threads and 50 MB per window (measured with 17 stickers and the main
/// window: 1036 MB private and 323 threads, against 72 MB and 17 threads).
/// Call before the first window. A value set by hand is kept.
pub fn prefer_integrated_gpu() {
    let before = std::env::var_os(GPU_SWITCH);
    if chosen_by_hand(before.as_deref()) {
        return;
    }
    std::env::set_var(GPU_SWITCH, INTEGRATED);
    if let Ok(mut b) = GPU_BEFORE.lock() {
        *b = Some(before);
    }
    GPU_ASKED.store(true, std::sync::atomic::Ordering::Relaxed);
}
/// Once the first windows have their drawing contexts: programs this one
/// starts (a browser for a link) get the switch as it was.
pub fn forget_gpu_choice() {
    if !GPU_ASKED.swap(false, std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    let before = GPU_BEFORE.lock().ok().and_then(|mut b| b.take());
    match before.flatten() {
        Some(v) => std::env::set_var(GPU_SWITCH, v),
        None => std::env::remove_var(GPU_SWITCH),
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
    fn the_integrated_gpu_is_asked_for_unless_chosen_by_hand_and_then_put_back() {
        use super::{forget_gpu_choice, prefer_integrated_gpu, GPU_SWITCH};
        let now = || std::env::var(GPU_SWITCH).ok();
        // Set by hand: left alone.
        for hand in ["0x800000001", "0x800000000"] {
            std::env::set_var(GPU_SWITCH, hand);
            prefer_integrated_gpu();
            assert_eq!(now().as_deref(), Some(hand));
            forget_gpu_choice();
            assert_eq!(now().as_deref(), Some(hand));
        }
        // Inherited from a program on the NVIDIA GPU: asked for, then put back.
        std::env::set_var(GPU_SWITCH, "0x810000001");
        prefer_integrated_gpu();
        assert_eq!(now().as_deref(), Some("0x800000000"));
        forget_gpu_choice();
        assert_eq!(now().as_deref(), Some("0x810000001"));
        // Not set: asked for, then gone again.
        std::env::remove_var(GPU_SWITCH);
        prefer_integrated_gpu();
        assert_eq!(now().as_deref(), Some("0x800000000"));
        forget_gpu_choice();
        assert_eq!(now(), None);
        // Only once.
        forget_gpu_choice();
        assert_eq!(now(), None);
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
