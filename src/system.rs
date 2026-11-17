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
