//! Native window shaping on Windows.
//!
//! Many GPU drivers give OpenGL windows no per-pixel alpha, so anything egui
//! paints transparent shows up black (including the area outside rounded
//! corners). Borderless windows are therefore painted opaque and square:
//! Windows rounds their corners and applies whole-window opacity through a
//! layered window, which every driver supports.

use eframe::egui;

/// True when the OS shapes windows, so content must be painted opaque and
/// unrounded. Elsewhere egui's own alpha and rounding are used.
pub const NATIVE: bool = cfg!(windows);

/// Round the corners of the window `ctx` draws and set its whole-window
/// opacity (`0.0..=1.0`). Cheap to call every frame: work is only done when
/// a value changes.
pub fn shape(ctx: &egui::Context, opacity: f32) {
    #[cfg(windows)]
    imp::shape(ctx, opacity);
    #[cfg(not(windows))]
    let _ = (ctx, opacity);
}

/// Keeps a mouse-operated popup from taking keyboard focus from its editor.
pub fn without_activation(ctx: &egui::Context) {
    #[cfg(windows)]
    imp::without_activation(ctx);
    #[cfg(not(windows))]
    let _ = ctx;
}

/// Use a shallow app-drawn edge for stickers instead of the system shadow.
pub fn sticker_shadow(ctx: &egui::Context, small: bool) {
    #[cfg(windows)]
    imp::sticker_shadow(ctx, small);
    #[cfg(not(windows))]
    let _ = (ctx, small);
}

/// Native handle of the window `ctx` draws, if it can be found.
pub fn hwnd(ctx: &egui::Context) -> Option<isize> {
    #[cfg(windows)]
    return unsafe { imp::find(ctx).map(|h| h as isize) };
    #[cfg(not(windows))]
    {
        let _ = ctx;
        None
    }
}

/// Stack the ordinary windows immediately below the main window in their
/// current order, without activating them.
pub fn stack_below(main: isize, windows: &[isize]) {
    #[cfg(windows)]
    desk::stack_below(main, windows);
    #[cfg(not(windows))]
    let _ = (main, windows);
}

/// Observes foreground changes so that idle stickers can sleep between events.
#[cfg(windows)]
pub struct ForegroundWatch(windows_sys::Win32::UI::Accessibility::HWINEVENTHOOK);

#[cfg(windows)]
impl ForegroundWatch {
    pub fn start() -> Self {
        Self(desk::watch_foreground())
    }
}

#[cfg(windows)]
impl Drop for ForegroundWatch {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { windows_sys::Win32::UI::Accessibility::UnhookWinEvent(self.0) };
        }
    }
}

#[cfg(windows)]
pub use desk::{
    desktop_foreground, desktop_is_raised, is_desktop_sticker_visible, keep_desktop_sticker,
    release_desktop_sticker,
};

/// Native handle of the window `ctx` draws as [`shape`] found it (no search).
pub fn shaped_hwnd(ctx: &egui::Context) -> Option<isize> {
    #[cfg(windows)]
    return imp::cached(ctx);
    #[cfg(not(windows))]
    {
        let _ = ctx;
        None
    }
}

/// A screen rectangle in physical pixels: left, top, right, bottom.
pub type PxRect = [i32; 4];

/// Put window `hwnd` on the desktop wallpaper layer at `rect`: below the
/// desktop icons, so clicks go to the desktop and icons can sit on top.
/// `over_icons`: above the icons instead, so the window takes input.
pub fn pin_to_desktop(hwnd: isize, rect: PxRect, over_icons: bool) -> bool {
    #[cfg(windows)]
    return unsafe { desk::pin(hwnd, rect, over_icons) };
    #[cfg(not(windows))]
    {
        let _ = (hwnd, rect, over_icons);
        false
    }
}

/// Make a pinned window an ordinary top-level window again at `rect`.
pub fn unpin_from_desktop(hwnd: isize, rect: PxRect) -> bool {
    #[cfg(windows)]
    return unsafe { desk::unpin(hwnd, rect) };
    #[cfg(not(windows))]
    {
        let _ = (hwnd, rect);
        false
    }
}

/// At quit: a pinned window is hidden and taken off the desktop layer
/// before it is destroyed, so no picture of it stays on the wallpaper.
pub fn let_go_of_desktop(hwnd: isize) {
    #[cfg(windows)]
    unsafe {
        desk::let_go(hwnd)
    };
    #[cfg(not(windows))]
    let _ = hwnd;
}

/// Whether the native window still exists (a window pinned under the
/// desktop icons goes with the desktop when Explorer restarts).
pub fn window_alive(hwnd: isize) -> bool {
    #[cfg(windows)]
    // SAFETY: a plain query; any handle value may be passed.
    return hwnd != 0
        && unsafe { windows_sys::Win32::UI::WindowsAndMessaging::IsWindow(hwnd as _) != 0 };
    #[cfg(not(windows))]
    {
        let _ = hwnd;
        false
    }
}

/// Has the desktop draw `rect` (screen pixels) again on all its layers.
pub fn refresh_desktop(rect: PxRect) {
    #[cfg(windows)]
    unsafe {
        desk::refresh(rect)
    };
    #[cfg(not(windows))]
    let _ = rect;
}

/// Watches, on a small background thread, for a click on `rect` while the
/// desktop (not another window) is under the pointer. The standing calendar
/// lives below the desktop icons and gets no input of its own.
pub struct DesktopClick {
    hit: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// The window is no longer where it was pinned: gone, moved out of its
    /// desktop layer, or that layer hidden.
    lost: std::sync::Arc<std::sync::atomic::AtomicBool>,
    rect: std::sync::Arc<std::sync::Mutex<PxRect>>,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    thread: u32,
}

impl DesktopClick {
    pub fn watch(hwnd: isize, rect: PxRect, ctx: egui::Context, over_icons: bool) -> Option<Self> {
        use std::sync::{atomic::AtomicBool, Arc, Mutex};
        let hit = Arc::new(AtomicBool::new(false));
        let lost = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let rect = Arc::new(Mutex::new(rect));
        #[cfg(windows)]
        let thread = desk::watch(desk::Watch {
            hwnd,
            over_icons,
            rect: rect.clone(),
            hit: hit.clone(),
            lost: lost.clone(),
            stop: stop.clone(),
            ctx,
        })?;
        #[cfg(not(windows))]
        let thread = {
            let _ = (hwnd, ctx, over_icons);
            0
        };
        Some(Self {
            hit,
            lost,
            rect,
            stop,
            thread,
        })
    }
    /// The pinned window lost its place on the desktop (see [`Self::lost`]).
    pub fn lost(&self) -> bool {
        self.lost.load(std::sync::atomic::Ordering::Relaxed)
    }
    pub fn update(&self, rect: PxRect) {
        *self.rect.lock().unwrap_or_else(|e| e.into_inner()) = rect;
    }
    pub fn take(&self) -> bool {
        self.hit.swap(false, std::sync::atomic::Ordering::Relaxed)
    }
}

impl Drop for DesktopClick {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        #[cfg(windows)]
        unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::PostThreadMessageW(
                self.thread,
                windows_sys::Win32::UI::WindowsAndMessaging::WM_QUIT,
                0,
                0,
            );
        }
    }
}

fn contains_point(rect: PxRect, x: i32, y: i32) -> bool {
    x >= rect[0] && x < rect[2] && y >= rect[1] && y < rect[3]
}

/// The mouse hook looks gone: the pointer moved since the last check but
/// the hook saw no mouse event. Windows removes a low-level hook without a
/// word when it once answers too slowly; the standing button then stopped
/// answering for good.
fn hook_seems_gone(pointer_moved: bool, calls_then: u32, calls_now: u32) -> bool {
    pointer_moved && calls_then == calls_now
}

/// A pinned window lost its place: gone, out of the layer it was put in,
/// or that layer hidden for two seconds.
fn pin_lost(alive: bool, same_parent: bool, parent_hidden_ms: u32) -> bool {
    !alive || !same_parent || parent_hidden_ms >= 2000
}

#[cfg(windows)]
mod desk {
    use super::PxRect;
    use std::cell::Cell;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    };
    use windows_sys::Win32::{
        Foundation::{
            GetLastError, SetLastError, BOOL, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM,
        },
        Graphics::{
            Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED},
            Gdi::{
                ClientToScreen, RedrawWindow, ScreenToClient, RDW_ALLCHILDREN, RDW_ERASE,
                RDW_INVALIDATE,
            },
        },
        System::{LibraryLoader::GetModuleHandleW, Threading::GetCurrentThreadId},
        UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK},
        UI::WindowsAndMessaging::{
            CallNextHookEx, EnumWindows, FindWindowExW, FindWindowW, GetAncestor, GetClassNameW,
            GetCursorPos, GetDesktopWindow, GetForegroundWindow, GetMessageW, GetTopWindow,
            GetWindow, GetWindowLongW, GetWindowRect, IsChild, IsIconic, IsWindow, IsWindowVisible,
            KillTimer, PeekMessageW, PostThreadMessageW, SendMessageTimeoutW,
            SetLayeredWindowAttributes, SetParent, SetTimer, SetWindowLongW, SetWindowPos,
            SetWindowsHookExW, ShowWindow, ShowWindowAsync, UnhookWindowsHookEx, WindowFromPoint,
            EVENT_SYSTEM_FOREGROUND, GA_PARENT, GA_ROOT, GWL_EXSTYLE, GW_HWNDNEXT, GW_HWNDPREV,
            HHOOK, HWND_NOTOPMOST, HWND_TOP, HWND_TOPMOST, LWA_ALPHA, MSG, MSLLHOOKSTRUCT,
            PM_NOREMOVE, SMTO_NORMAL, SWP_ASYNCWINDOWPOS, SWP_FRAMECHANGED, SWP_NOACTIVATE,
            SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, SW_HIDE, SW_SHOWNOACTIVATE,
            WH_MOUSE_LL, WINEVENT_OUTOFCONTEXT, WM_APP, WM_LBUTTONUP, WM_TIMER, WS_EX_LAYERED,
            WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
        },
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    unsafe fn class_of(h: HWND) -> String {
        let mut buf = [0u16; 64];
        let n = GetClassNameW(h, buf.as_mut_ptr(), buf.len() as i32).max(0) as usize;
        String::from_utf16_lossy(&buf[..n])
    }

    unsafe fn progman() -> HWND {
        FindWindowW(wide("Progman").as_ptr(), std::ptr::null())
    }

    /// The window that draws the wallpaper behind the desktop icons, on the
    /// classic desktop layout (a top-level WorkerW next to the one that holds
    /// the icons). Null on Windows 11 24H2 and later, where the wallpaper's
    /// WorkerW is a child of Progman: a window made a child of it (or of
    /// Progman) is not shown there, it just disappears.
    unsafe fn wallpaper_layer() -> HWND {
        let progman = progman();
        if progman.is_null() {
            return progman;
        }
        // Ask Explorer to create the WorkerW that sits behind the icons.
        let mut result = 0usize;
        SendMessageTimeoutW(progman, 0x052C, 0xD, 1, SMTO_NORMAL, 1000, &mut result);
        SendMessageTimeoutW(progman, 0x052C, 0, 0, SMTO_NORMAL, 1000, &mut result);
        let mut worker: HWND = std::ptr::null_mut();
        unsafe extern "system" fn visit(h: HWND, data: LPARAM) -> BOOL {
            let worker = &mut *(data as *mut HWND);
            let view = FindWindowExW(
                h,
                std::ptr::null_mut(),
                wide("SHELLDLL_DefView").as_ptr(),
                std::ptr::null(),
            );
            if !view.is_null() {
                // Classic layout: the next top-level WorkerW is the wallpaper.
                *worker = FindWindowExW(
                    std::ptr::null_mut(),
                    h,
                    wide("WorkerW").as_ptr(),
                    std::ptr::null(),
                );
            }
            1
        }
        EnumWindows(Some(visit), &mut worker as *mut HWND as LPARAM);
        worker
    }

    /// The icon layer inside Progman on the newer desktop layout (Windows 11
    /// 24H2 and later: Progman holds the icons, `SHELLDLL_DefView`, and the
    /// wallpaper, a `WorkerW`, as its own children). Null on the classic one.
    unsafe fn icon_layer(progman: HWND) -> HWND {
        FindWindowExW(
            progman,
            std::ptr::null_mut(),
            wide("SHELLDLL_DefView").as_ptr(),
            std::ptr::null(),
        )
    }

    /// Newer layout: the window becomes a child of Progman placed right
    /// below the icon layer and above the wallpaper, as wallpaper apps do,
    /// so icons can sit on top of it and clicks go to the desktop. Progman
    /// draws without a surface of its own, so a plain child window would not
    /// show: it is made a (fully opaque) layered window.
    unsafe fn pin_under_icons(h: HWND, r: PxRect) -> bool {
        let progman = progman();
        if progman.is_null() {
            return false;
        }
        let icons = icon_layer(progman);
        if icons.is_null() {
            return false;
        }
        let style = GetWindowLongW(h, GWL_EXSTYLE) as u32;
        SetWindowLongW(h, GWL_EXSTYLE, (style | WS_EX_LAYERED) as i32);
        SetLayeredWindowAttributes(h, 0, 255, LWA_ALPHA);
        SetLastError(0);
        if SetParent(h, progman).is_null() && GetLastError() != 0 {
            SetWindowLongW(h, GWL_EXSTYLE, style as i32);
            return false;
        }
        let mut p = POINT { x: r[0], y: r[1] };
        ScreenToClient(progman, &mut p);
        // "Insert after" the icon layer: directly behind it.
        SetWindowPos(
            h,
            icons,
            p.x,
            p.y,
            r[2] - r[0],
            r[3] - r[1],
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
        true
    }

    /// The window holding the desktop icons (`SHELLDLL_DefView`): Progman on
    /// the newer layout, a top-level WorkerW on the classic one.
    unsafe fn icon_host() -> HWND {
        let progman = progman();
        if !progman.is_null() && !icon_layer(progman).is_null() {
            return progman;
        }
        let mut host: HWND = std::ptr::null_mut();
        unsafe extern "system" fn visit(h: HWND, data: LPARAM) -> BOOL {
            if !icon_layer(h).is_null() {
                *(data as *mut HWND) = h;
                return 0;
            }
            1
        }
        EnumWindows(Some(visit), &mut host as *mut HWND as LPARAM);
        host
    }

    /// The window becomes the top child of the window holding the icons:
    /// over them, so it takes the pointer (the icons there are covered).
    unsafe fn pin_over_icons(h: HWND, r: PxRect) -> bool {
        let host = icon_host();
        if host.is_null() {
            return false;
        }
        let style = GetWindowLongW(h, GWL_EXSTYLE) as u32;
        SetWindowLongW(h, GWL_EXSTYLE, (style | WS_EX_LAYERED) as i32);
        SetLayeredWindowAttributes(h, 0, 255, LWA_ALPHA);
        SetLastError(0);
        if SetParent(h, host).is_null() && GetLastError() != 0 {
            SetWindowLongW(h, GWL_EXSTYLE, style as i32);
            return false;
        }
        let mut p = POINT { x: r[0], y: r[1] };
        ScreenToClient(host, &mut p);
        SetWindowPos(
            h,
            HWND_TOP,
            p.x,
            p.y,
            r[2] - r[0],
            r[3] - r[1],
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
        true
    }

    /// Without the classic layout the window stays a top-level window, just
    /// above the desktop (Progman) in the stacking order: below every other
    /// window, out of the taskbar and Alt+Tab, and shown on every Windows
    /// version. `keep_on_desktop` puts it back there when it gets raised.
    unsafe fn pin_above_desktop(h: HWND, r: PxRect) -> bool {
        let progman = progman();
        if progman.is_null() {
            return false;
        }
        let style = GetWindowLongW(h, GWL_EXSTYLE) as u32;
        SetWindowLongW(h, GWL_EXSTYLE, (style | WS_EX_TOOLWINDOW) as i32);
        SetWindowPos(
            h,
            below_visible_windows(h, progman),
            r[0],
            r[1],
            r[2] - r[0],
            r[3] - r[1],
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
        true
    }

    /// The window to insert `h` after (that is, behind) so that `h` ends up
    /// directly above the desktop: the lowest visible window above Progman,
    /// or the top when none. (`SetWindowPos` places a window *behind* its
    /// "insert after" window, so Progman itself would hide it.) Hidden
    /// windows and the wallpaper WorkerWs in between do not count. Also tells
    /// whether `h` is already there.
    unsafe fn desktop_slot(h: HWND, progman: HWND) -> (HWND, bool) {
        let mut n = GetWindow(progman, GW_HWNDPREV);
        while !n.is_null() && n != h {
            // Always-on-top windows (the taskbar) are a band of their own:
            // placed after one, the calendar would join it. Reaching them
            // means no ordinary window is above the desktop (Win+D raised
            // it): the calendar goes on top of the ordinary ones.
            if GetWindowLongW(n, GWL_EXSTYLE) as u32 & WS_EX_TOPMOST != 0 {
                return (HWND_TOP, false);
            }
            if IsWindowVisible(n) != 0 && class_of(n) != "WorkerW" {
                return (n, false);
            }
            n = GetWindow(n, GW_HWNDPREV);
        }
        if n == h {
            (h, true)
        } else {
            (HWND_TOP, false)
        }
    }

    /// The window is on screen: shown, not minimized, not empty and not
    /// cloaked (on another virtual desktop, say).
    unsafe fn on_screen(n: HWND) -> bool {
        if IsWindowVisible(n) == 0 || IsIconic(n) != 0 {
            return false;
        }
        let mut r: RECT = std::mem::zeroed();
        GetWindowRect(n, &mut r);
        if r.right <= r.left || r.bottom <= r.top {
            return false;
        }
        let mut cloaked = 0u32;
        DwmGetWindowAttribute(
            n,
            DWMWA_CLOAKED as u32,
            &mut cloaked as *mut u32 as *mut _,
            std::mem::size_of::<u32>() as u32,
        );
        cloaked == 0
    }

    /// Win+D (show desktop) raised the desktop over every ordinary window:
    /// Progman is the first ordinary window on screen from the top.
    unsafe fn desktop_raised(h: HWND, progman: HWND) -> bool {
        let mut n = GetTopWindow(std::ptr::null_mut());
        let mut left = 4096;
        while !n.is_null() && left > 0 {
            if n == progman {
                return true;
            }
            if n != h
                && GetWindowLongW(n, GWL_EXSTYLE) as u32 & WS_EX_TOPMOST == 0
                && class_of(n) != "WorkerW"
                && on_screen(n)
            {
                return false;
            }
            n = GetWindow(n, GW_HWNDNEXT);
            left -= 1;
        }
        false
    }

    /// `h` is above the desktop window in the stacking order.
    unsafe fn above(h: HWND, progman: HWND) -> bool {
        let mut n = GetWindow(h, GW_HWNDNEXT);
        while !n.is_null() {
            if n == progman {
                return true;
            }
            n = GetWindow(n, GW_HWNDNEXT);
        }
        false
    }

    /// The lowest always-on-top window (the calendar goes right below it,
    /// so it stays under the taskbar and other always-on-top windows).
    unsafe fn lowest_topmost(h: HWND, progman: HWND) -> HWND {
        let mut n = GetWindow(progman, GW_HWNDPREV);
        while !n.is_null() {
            if n != h && GetWindowLongW(n, GWL_EXSTYLE) as u32 & WS_EX_TOPMOST != 0 {
                return n;
            }
            n = GetWindow(n, GW_HWNDPREV);
        }
        HWND_TOPMOST
    }

    unsafe fn below_visible_windows(h: HWND, progman: HWND) -> HWND {
        desktop_slot(h, progman).0
    }

    fn is_top_level(h: HWND) -> bool {
        // SAFETY: plain queries on a window handle.
        unsafe { GetAncestor(h, GA_PARENT) == GetDesktopWindow() }
    }

    /// Keeps a window pinned in the top-level way right above the desktop.
    /// Returns whether it is pinned that way (and so needs this called again).
    pub unsafe fn keep_on_desktop(hwnd: isize, over_icons: bool) -> bool {
        let h = hwnd as HWND;
        let progman = progman();
        if over_icons && !is_top_level(h) {
            // Over the icons: kept the top child of their window.
            if !GetWindow(h, GW_HWNDPREV).is_null() {
                SetWindowPos(
                    h,
                    HWND_TOP,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS,
                );
            }
            return true;
        }
        if !progman.is_null() && GetAncestor(h, GA_PARENT) == progman {
            // Under the icons: keep it right behind them if Explorer
            // reordered its children (after Win+D, for instance).
            let icons = icon_layer(progman);
            if !icons.is_null() && GetWindow(h, GW_HWNDPREV) != icons {
                SetWindowPos(
                    h,
                    icons,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS,
                );
            }
            return true;
        }
        // A window already let go (no longer a tool window) is left alone:
        // the watch thread may tick between the release and its own end.
        if !is_top_level(h) || GetWindowLongW(h, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW == 0 {
            return false;
        }
        // Asynchronous calls only: this thread must never wait for the
        // program's (busy) window thread.
        if IsIconic(h) != 0 {
            // Minimized (show desktop on some PCs): back without the focus.
            ShowWindowAsync(h, SW_SHOWNOACTIVATE);
        }
        let flags = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS;
        let topmost = GetWindowLongW(h, GWL_EXSTYLE) as u32 & WS_EX_TOPMOST != 0;
        if !progman.is_null() && desktop_raised(h, progman) {
            // Win+D raised the desktop, which is now the foreground window,
            // over every window: no program in the background may put a
            // window above it with `HWND_TOP` (the call is quietly ignored).
            // Placed after the lowest always-on-top window, Windows puts it
            // at the top of the ordinary windows instead, above the desktop
            // (or, on some versions, among the always-on-top ones).
            if !topmost && !above(h, progman) {
                SetWindowPos(h, lowest_topmost(h, progman), 0, 0, 0, 0, flags);
            }
            return true;
        }
        if topmost {
            SetWindowPos(h, HWND_NOTOPMOST, 0, 0, 0, 0, flags);
        }
        let (after, placed) = if progman.is_null() {
            (h, true)
        } else {
            desktop_slot(h, progman)
        };
        if !placed || topmost {
            SetWindowPos(
                h,
                after,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS,
            );
        }
        true
    }

    /// Takes the window's button off the taskbar (`show` false) or puts it
    /// back. Explorer keeps the button of a window moved under the desktop
    /// icons, so it is taken off by hand.
    pub fn watch_foreground() -> HWINEVENTHOOK {
        unsafe extern "system" fn changed(
            _: HWINEVENTHOOK,
            _: u32,
            _: HWND,
            _: i32,
            _: i32,
            _: u32,
            _: u32,
        ) {
            crate::wake::now();
        }
        unsafe {
            SetWinEventHook(
                EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_FOREGROUND,
                std::ptr::null_mut(),
                Some(changed),
                0,
                0,
                WINEVENT_OUTOFCONTEXT,
            )
        }
    }

    pub fn desktop_foreground() -> Option<isize> {
        unsafe {
            let h = GetForegroundWindow();
            matches!(class_of(h).as_str(), "Progman" | "WorkerW").then_some(h as isize)
        }
    }

    pub fn desktop_is_raised(desktop: isize, stickers: &[isize]) -> bool {
        unsafe {
            let mut h = GetTopWindow(std::ptr::null_mut());
            while !h.is_null() {
                if h as isize == desktop {
                    return true;
                }
                if GetWindowLongW(h, GWL_EXSTYLE) as u32 & (WS_EX_TOPMOST | WS_EX_TOOLWINDOW) == 0
                    && !stickers.contains(&(h as isize))
                    && class_of(h) != "WorkerW"
                    && on_screen(h)
                {
                    return false;
                }
                h = GetWindow(h, GW_HWNDNEXT);
            }
            false
        }
    }

    pub fn is_desktop_sticker_visible(hwnd: isize) -> bool {
        unsafe { on_screen(hwnd as HWND) }
    }

    /// Show an already-open sticker with no focus change. Returns whether it
    /// acquired a temporary topmost style that the caller must remove.
    pub fn keep_desktop_sticker(desktop: isize, hwnd: isize) -> bool {
        unsafe {
            let h = hwnd as HWND;
            let desktop = desktop as HWND;
            if IsWindow(h) == 0 {
                return false;
            }
            if IsIconic(h) != 0 || IsWindowVisible(h) == 0 {
                ShowWindow(h, SW_SHOWNOACTIVATE);
            }
            let topmost = GetWindowLongW(h, GWL_EXSTYLE) as u32 & WS_EX_TOPMOST != 0;
            if !topmost && !above(h, desktop) {
                SetWindowPos(
                    h,
                    lowest_topmost(h, desktop),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                );
            }
            !topmost && GetWindowLongW(h, GWL_EXSTYLE) as u32 & WS_EX_TOPMOST != 0
        }
    }

    pub fn release_desktop_sticker(hwnd: isize) {
        unsafe {
            SetWindowPos(
                hwnd as HWND,
                HWND_NOTOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
    }

    pub fn stack_below(main: isize, windows: &[isize]) {
        unsafe {
            let mut order = Vec::with_capacity(windows.len());
            let mut h = GetTopWindow(std::ptr::null_mut());
            while !h.is_null() && order.len() < windows.len() {
                if windows.contains(&(h as isize))
                    && GetWindowLongW(h, GWL_EXSTYLE) as u32 & WS_EX_TOPMOST == 0
                    && IsIconic(h) == 0
                {
                    order.push(h);
                }
                h = GetWindow(h, GW_HWNDNEXT);
            }

            let mut after = main as HWND;
            let flags = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE;
            for h in order {
                if GetWindow(after, GW_HWNDNEXT) != h
                    && SetWindowPos(h, after, 0, 0, 0, 0, flags) == 0
                {
                    continue;
                }
                after = h;
            }
        }
    }

    pub fn taskbar_button(hwnd: isize, show: bool) {
        use windows::Win32::System::Com::{
            CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
        };
        use windows::Win32::UI::Shell::{ITaskbarList, TaskbarList};
        // SAFETY: COM calls on this (the UI) thread; the window handle is
        // only passed on, and a gone window is an error Explorer ignores.
        unsafe {
            // Already initialized by the window system: the result is ignored.
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            let Ok(list) =
                CoCreateInstance::<_, ITaskbarList>(&TaskbarList, None, CLSCTX_INPROC_SERVER)
            else {
                return;
            };
            if list.HrInit().is_err() {
                return;
            }
            let h = windows::Win32::Foundation::HWND(hwnd as *mut core::ffi::c_void);
            let _ = if show {
                list.AddTab(h)
            } else {
                list.DeleteTab(h)
            };
        }
    }

    pub unsafe fn pin(hwnd: isize, r: PxRect, over_icons: bool) -> bool {
        let top_level_only = crate::system::standing_unsupported()
            || std::env::var_os("OHMEMOJANG_STANDING_TOPLEVEL").is_some();
        // The top-level way (24H2) takes input as it is.
        let pinned = if over_icons && !top_level_only {
            pin_over_icons(hwnd as HWND, r) || pin_window(hwnd, r)
        } else {
            pin_window(hwnd, r)
        };
        if pinned {
            taskbar_button(hwnd, false);
        }
        pinned
    }

    unsafe fn pin_window(hwnd: isize, r: PxRect) -> bool {
        let layer = wallpaper_layer();
        let h = hwnd as HWND;
        if layer.is_null() {
            // Windows 11 24H2 itself does not show a window placed under
            // the icons: there it stays a top-level window above the desktop.
            // (`OHMEMOJANG_STANDING_TOPLEVEL` tries that way on other PCs.)
            let top_level_only = crate::system::standing_unsupported()
                || std::env::var_os("OHMEMOJANG_STANDING_TOPLEVEL").is_some();
            if !top_level_only && pin_under_icons(h, r) {
                return true;
            }
            return pin_above_desktop(h, r);
        }
        // A null previous parent is also a successful result for top-level windows.
        SetLastError(0);
        if SetParent(h, layer).is_null() && GetLastError() != 0 {
            return false;
        }
        let mut p = POINT { x: r[0], y: r[1] };
        ScreenToClient(layer, &mut p);
        SetWindowPos(
            h,
            std::ptr::null_mut(),
            p.x,
            p.y,
            r[2] - r[0],
            r[3] - r[1],
            SWP_NOZORDER | SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
        true
    }

    pub unsafe fn unpin(hwnd: isize, r: PxRect) -> bool {
        let back = unpin_window(hwnd, r);
        if back {
            taskbar_button(hwnd, true);
        }
        back
    }

    unsafe fn unpin_window(hwnd: isize, r: PxRect) -> bool {
        let h = hwnd as HWND;
        if is_top_level(h) {
            let style = GetWindowLongW(h, GWL_EXSTYLE) as u32;
            SetWindowLongW(h, GWL_EXSTYLE, (style & !WS_EX_TOOLWINDOW) as i32);
            if style & WS_EX_TOPMOST != 0 {
                // Let go while the desktop was raised (Win+D).
                SetWindowPos(h, HWND_NOTOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
            }
            SetWindowPos(
                h,
                HWND_TOP,
                r[0],
                r[1],
                r[2] - r[0],
                r[3] - r[1],
                SWP_SHOWWINDOW,
            );
            return true;
        }
        let parent = GetAncestor(h, GA_PARENT);
        let mut stood: RECT = std::mem::zeroed();
        let has_rect = GetWindowRect(h, &mut stood) != 0;
        // Remove the desktop surface before changing parents. On the classic
        // desktop Explorer can otherwise keep the last composited picture.
        ShowWindow(h, SW_HIDE);
        SetLastError(0);
        if SetParent(h, std::ptr::null_mut()).is_null() && GetLastError() != 0 {
            ShowWindow(h, SW_SHOWNOACTIVATE);
            return false;
        }
        // Layered only while under the icons.
        let style = GetWindowLongW(h, GWL_EXSTYLE) as u32;
        if style & WS_EX_LAYERED != 0 {
            SetWindowLongW(h, GWL_EXSTYLE, (style & !WS_EX_LAYERED) as i32);
        }
        SetWindowPos(
            h,
            std::ptr::null_mut(),
            r[0],
            r[1],
            r[2] - r[0],
            r[3] - r[1],
            SWP_NOZORDER | SWP_SHOWWINDOW | SWP_FRAMECHANGED,
        );
        if has_rect {
            let rect = [stood.left, stood.top, stood.right, stood.bottom];
            redraw_layer(parent, rect);
            refresh(rect);
        }
        true
    }

    unsafe fn redraw_layer(h: HWND, r: PxRect) {
        let mut a = POINT { x: r[0], y: r[1] };
        let mut b = POINT { x: r[2], y: r[3] };
        if h.is_null() || ScreenToClient(h, &mut a) == 0 || ScreenToClient(h, &mut b) == 0 {
            return;
        }
        let rc = RECT {
            left: a.x,
            top: a.y,
            right: b.x,
            bottom: b.y,
        };
        RedrawWindow(
            h,
            &rc,
            std::ptr::null_mut(),
            RDW_INVALIDATE | RDW_ERASE | RDW_ALLCHILDREN,
        );
    }

    /// Redraws `r` (screen pixels) on every desktop layer: Progman with its
    /// children and the top-level WorkerWs (the classic layout's wallpaper).
    /// Asynchronous: Explorer paints it when it gets to it.
    pub unsafe fn refresh(r: PxRect) {
        unsafe extern "system" fn visit(h: HWND, data: LPARAM) -> BOOL {
            let r = *(data as *const PxRect);
            if matches!(class_of(h).as_str(), "WorkerW" | "Progman") {
                redraw_layer(h, r);
            }
            1
        }
        EnumWindows(Some(visit), &r as *const PxRect as LPARAM);
    }

    pub unsafe fn let_go(hwnd: isize) {
        let h = hwnd as HWND;
        if IsWindow(h) == 0 {
            return;
        }
        let mut stood: RECT = std::mem::zeroed();
        GetWindowRect(h, &mut stood);
        ShowWindow(h, SW_HIDE);
        if !is_top_level(h) {
            SetParent(h, std::ptr::null_mut());
            refresh([stood.left, stood.top, stood.right, stood.bottom]);
        }
    }

    /// The desktop is under `p` (not some other window): the calendar
    /// itself, or any window of the desktop (icons, wallpaper layers, and
    /// whatever else Explorer or a wallpaper tool keeps in them).
    unsafe fn desktop_at(p: POINT, calendar: HWND) -> bool {
        let top = WindowFromPoint(p);
        if top.is_null() || top == calendar || IsChild(calendar, top) != 0 {
            return true;
        }
        if matches!(
            class_of(top).as_str(),
            "SysListView32" | "SHELLDLL_DefView" | "WorkerW" | "Progman"
        ) {
            return true;
        }
        matches!(
            class_of(GetAncestor(top, GA_ROOT)).as_str(),
            "WorkerW" | "Progman"
        )
    }

    pub struct Watch {
        pub hwnd: isize,
        pub over_icons: bool,
        pub rect: Arc<Mutex<PxRect>>,
        pub hit: Arc<AtomicBool>,
        pub lost: Arc<AtomicBool>,
        pub stop: Arc<AtomicBool>,
        pub ctx: eframe::egui::Context,
    }

    /// A left-button release seen by the hook (point in `wParam`/`lParam`).
    const WM_DESKTOP_CLICK: u32 = WM_APP + 0x31;
    /// Another window became the foreground one: the calendar's place is
    /// checked at once (Win+D, or a window brought back after it).
    const WM_DESKTOP_CHECK: u32 = WM_APP + 0x32;

    unsafe extern "system" fn foreground_changed(
        _: HWINEVENTHOOK,
        _: u32,
        _: HWND,
        _: i32,
        _: i32,
        _: u32,
        _: u32,
    ) {
        PostThreadMessageW(GetCurrentThreadId(), WM_DESKTOP_CHECK, 0, 0);
    }
    /// How often the watch checks the calendar's place and the hook: under
    /// 0.25 s, so the calendar is back right after Win+D.
    const TICK_MS: u32 = 150;

    thread_local! {
        /// Mouse events the hook has seen, to tell whether it still runs.
        static HOOK_CALLS: Cell<u32> = const { Cell::new(0) };
    }

    /// As short as can be: Windows silently removes a low-level hook that
    /// once answers too slowly (while the system is busy, say). The click
    /// itself is looked at in the thread's loop, after the hook returned.
    unsafe extern "system" fn hook(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
        if code >= 0 {
            HOOK_CALLS.with(|c| c.set(c.get().wrapping_add(1)));
            if w as u32 == WM_LBUTTONUP {
                let info = &*(l as *const MSLLHOOKSTRUCT);
                PostThreadMessageW(
                    GetCurrentThreadId(),
                    WM_DESKTOP_CLICK,
                    info.pt.x as isize as WPARAM,
                    info.pt.y as LPARAM,
                );
            }
        }
        CallNextHookEx(std::ptr::null_mut(), code, w, l)
    }

    unsafe fn install_hook() -> HHOOK {
        SetWindowsHookExW(
            WH_MOUSE_LL,
            Some(hook),
            GetModuleHandleW(std::ptr::null()),
            0,
        )
    }

    unsafe fn cursor() -> (i32, i32) {
        let mut p = POINT { x: 0, y: 0 };
        GetCursorPos(&mut p);
        (p.x, p.y)
    }

    /// The click at `pt` is on the standing button, with the desktop (not
    /// another window) under the pointer.
    unsafe fn on_button(watch: &Watch, pt: POINT) -> bool {
        let rect = *watch.rect.lock().unwrap_or_else(|e| e.into_inner());
        let h = watch.hwnd as HWND;
        let mut origin = POINT { x: 0, y: 0 };
        // Resolve the native client position at the instant of the click.
        // Reparenting, title-bar changes and monitor positions cannot stale it.
        ClientToScreen(h, &mut origin) != 0
            && super::contains_point(rect, pt.x - origin.x, pt.y - origin.y)
            && desktop_at(pt, h)
    }

    pub fn watch(watch: Watch) -> Option<u32> {
        let (ready, result) = std::sync::mpsc::sync_channel(1);
        let cancel = watch.stop.clone();
        let thread_id = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let started_thread = thread_id.clone();
        std::thread::Builder::new()
            .name("standing-calendar-input".into())
            .spawn(move || unsafe {
                let mut msg: MSG = std::mem::zeroed();
                // Create the queue before publishing the thread id, so Drop cannot
                // lose its quit message during a rapid unpin/pin transition.
                PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_NOREMOVE);
                let mut hook = install_hook();
                if hook.is_null() {
                    let _ = ready.send(None);
                    return;
                }
                started_thread.store(GetCurrentThreadId(), Ordering::SeqCst);
                let delivered = ready.send(Some(GetCurrentThreadId())).is_ok();
                let me = watch.hwnd as HWND;
                // The layer it was pinned to (the desktop window itself for
                // the top-level way).
                let parent = GetAncestor(me, GA_PARENT);
                let top_level = parent == GetDesktopWindow();
                // Keep the calendar in its place on the desktop from here, so
                // the program does not have to redraw just to check it.
                let timer = SetTimer(std::ptr::null_mut(), 0, TICK_MS, None);
                let foreground = if top_level {
                    SetWinEventHook(
                        EVENT_SYSTEM_FOREGROUND,
                        EVENT_SYSTEM_FOREGROUND,
                        std::ptr::null_mut(),
                        Some(foreground_changed),
                        0,
                        0,
                        WINEVENT_OUTOFCONTEXT,
                    )
                } else {
                    std::ptr::null_mut()
                };
                let mut pointer = cursor();
                let mut calls = HOOK_CALLS.with(Cell::get);
                let mut reinstalled: Option<std::time::Instant> = None;
                let mut hidden_ms = 0u32;
                let mut lost = false;
                // A second look shortly after the foreground changed: Win+D
                // raises the desktop a moment after making it the foreground.
                let mut recheck = 0usize;
                while delivered
                    && !watch.stop.load(Ordering::Relaxed)
                    && GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0
                {
                    if watch.stop.load(Ordering::Relaxed) || lost {
                        continue;
                    }
                    match msg.message {
                        WM_DESKTOP_CLICK => {
                            let pt = POINT {
                                x: msg.wParam as isize as i32,
                                y: msg.lParam as i32,
                            };
                            if on_button(&watch, pt) {
                                watch.hit.store(true, Ordering::Relaxed);
                                watch.ctx.request_repaint();
                            }
                        }
                        WM_DESKTOP_CHECK => {
                            if IsWindow(me) != 0 {
                                keep_on_desktop(watch.hwnd, watch.over_icons);
                            }
                            if recheck != 0 {
                                KillTimer(std::ptr::null_mut(), recheck);
                            }
                            recheck = SetTimer(std::ptr::null_mut(), 0, 50, None);
                        }
                        WM_TIMER if recheck != 0 && msg.wParam == recheck => {
                            KillTimer(std::ptr::null_mut(), recheck);
                            recheck = 0;
                            if IsWindow(me) != 0 {
                                keep_on_desktop(watch.hwnd, watch.over_icons);
                            }
                        }
                        WM_TIMER => {
                            let alive = IsWindow(me) != 0;
                            let same_parent = alive && GetAncestor(me, GA_PARENT) == parent;
                            hidden_ms = if same_parent && !top_level && IsWindowVisible(parent) == 0
                            {
                                hidden_ms.saturating_add(TICK_MS)
                            } else {
                                0
                            };
                            if super::pin_lost(alive, same_parent, hidden_ms) {
                                // The program takes it back and stands it again.
                                lost = true;
                                watch.lost.store(true, Ordering::Relaxed);
                                watch.ctx.request_repaint();
                                continue;
                            }
                            keep_on_desktop(watch.hwnd, watch.over_icons);
                            let now = (cursor(), HOOK_CALLS.with(Cell::get));
                            let quiet = reinstalled.is_none_or(|t| t.elapsed().as_secs() >= 2);
                            if quiet && super::hook_seems_gone(now.0 != pointer, calls, now.1) {
                                if !hook.is_null() {
                                    UnhookWindowsHookEx(hook);
                                }
                                hook = install_hook();
                                reinstalled = Some(std::time::Instant::now());
                            }
                            (pointer, calls) = now;
                        }
                        _ => {}
                    }
                }
                KillTimer(std::ptr::null_mut(), timer);
                if recheck != 0 {
                    KillTimer(std::ptr::null_mut(), recheck);
                }
                if !foreground.is_null() {
                    UnhookWinEvent(foreground);
                }
                if !hook.is_null() {
                    UnhookWindowsHookEx(hook);
                }
            })
            .ok()?;
        match result.recv_timeout(std::time::Duration::from_secs(1)) {
            Ok(thread) => thread,
            Err(_) => {
                cancel.store(true, Ordering::SeqCst);
                let id = thread_id.load(Ordering::SeqCst);
                if id != 0 {
                    unsafe {
                        PostThreadMessageW(
                            id,
                            windows_sys::Win32::UI::WindowsAndMessaging::WM_QUIT,
                            0,
                            0,
                        );
                    }
                }
                None
            }
        }
    }
}

#[cfg(windows)]
mod imp {
    use eframe::egui;
    use windows_sys::Win32::{
        Foundation::{BOOL, HWND, LPARAM, RECT},
        Graphics::{
            Dwm::{
                DwmSetWindowAttribute, DWMNCRP_DISABLED, DWMNCRP_USEWINDOWSTYLE,
                DWMWA_NCRENDERING_POLICY, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
            },
            Gdi::{CreateRoundRectRgn, SetWindowRgn},
        },
        System::Threading::GetCurrentThreadId,
        UI::WindowsAndMessaging::{
            EnumThreadWindows, GetWindowLongPtrW, GetWindowRect, GetWindowTextW, IsWindow,
            SetLayeredWindowAttributes, SetWindowLongPtrW, GWL_EXSTYLE, LWA_ALPHA, WS_EX_LAYERED,
            WS_EX_NOACTIVATE,
        },
    };

    #[derive(Clone, Copy, Default)]
    struct State {
        hwnd: isize,
        alpha: Option<u8>,
        native_round: Option<bool>,
        region: (i32, i32),
        small_shadow: bool,
    }

    pub fn cached(ctx: &egui::Context) -> Option<isize> {
        let key = egui::Id::new(("win_style", ctx.viewport_id()));
        ctx.data(|d| d.get_temp::<State>(key))
            .map(|s| s.hwnd)
            .filter(|&h| h != 0)
    }

    pub fn without_activation(ctx: &egui::Context) {
        if let Some(hwnd) = cached(ctx) {
            unsafe {
                let h = hwnd as HWND;
                let style = GetWindowLongPtrW(h, GWL_EXSTYLE);
                if style & WS_EX_NOACTIVATE as isize == 0 {
                    SetWindowLongPtrW(h, GWL_EXSTYLE, style | WS_EX_NOACTIVATE as isize);
                }
            }
        }
    }

    pub fn sticker_shadow(ctx: &egui::Context, small: bool) {
        let key = egui::Id::new(("win_style", ctx.viewport_id()));
        let Some(mut state) = ctx.data(|d| d.get_temp::<State>(key)) else {
            return;
        };
        if state.small_shadow == small {
            return;
        }
        let policy = if small {
            DWMNCRP_DISABLED
        } else {
            DWMNCRP_USEWINDOWSTYLE
        };
        let result = unsafe {
            DwmSetWindowAttribute(
                state.hwnd as HWND,
                DWMWA_NCRENDERING_POLICY as u32,
                &policy as *const _ as *const _,
                std::mem::size_of_val(&policy) as u32,
            )
        };
        if result >= 0 {
            state.small_shadow = small;
            ctx.data_mut(|d| d.insert_temp(key, state));
        }
    }

    pub fn shape(ctx: &egui::Context, opacity: f32) {
        let key = egui::Id::new(("win_style", ctx.viewport_id()));
        let mut s: State = ctx.data(|d| d.get_temp(key)).unwrap_or_default();
        // SAFETY: plain Win32 calls on a window owned by this thread.
        unsafe {
            if s.hwnd == 0 || IsWindow(s.hwnd as HWND) == 0 {
                let Some(h) = find(ctx) else {
                    return;
                };
                s = State {
                    hwnd: h as isize,
                    ..Default::default()
                };
            }
            let h = s.hwnd as HWND;
            let alpha = (opacity.clamp(0., 1.) * 255.).round() as u8;
            // Checked every frame: winit rewrites the extended style whenever
            // it changes the window (e.g. toggling resizable for the color
            // box), which silently drops the layered flag and the opacity.
            // An opaque window is left unlayered: a layered OpenGL window
            // stopped presenting new frames after growing back from a title
            // strip or color box.
            let ex = GetWindowLongPtrW(h, GWL_EXSTYLE);
            let layered = ex & WS_EX_LAYERED as isize != 0;
            if alpha == u8::MAX {
                if layered {
                    SetWindowLongPtrW(h, GWL_EXSTYLE, ex & !(WS_EX_LAYERED as isize));
                }
            } else if !layered || s.alpha != Some(alpha) {
                if !layered {
                    SetWindowLongPtrW(h, GWL_EXSTYLE, ex | WS_EX_LAYERED as isize);
                }
                SetLayeredWindowAttributes(h, 0, alpha, LWA_ALPHA);
            }
            s.alpha = Some(alpha);
            if s.native_round.is_none() {
                let pref = DWMWCP_ROUND;
                let hr = DwmSetWindowAttribute(
                    h,
                    DWMWA_WINDOW_CORNER_PREFERENCE as u32,
                    &pref as *const _ as *const _,
                    std::mem::size_of_val(&pref) as u32,
                );
                s.native_round = Some(hr >= 0);
            }
            // Windows 10 has no rounded-corner preference: clip to a region.
            if s.native_round == Some(false) {
                let mut r: RECT = std::mem::zeroed();
                GetWindowRect(h, &mut r);
                let size = (r.right - r.left, r.bottom - r.top);
                if size != s.region {
                    let d = (16. * ctx.pixels_per_point()).round() as i32;
                    let rgn = CreateRoundRectRgn(0, 0, size.0 + 1, size.1 + 1, d, d);
                    SetWindowRgn(h, rgn, 1);
                    s.region = size;
                }
            }
        }
        ctx.data_mut(|d| d.insert_temp(key, s));
    }

    struct Search {
        title: Vec<u16>,
        rect: RECT,
        best: Option<(i32, HWND)>,
    }

    /// egui does not expose native handles of child viewports, so the window
    /// is matched by its title and on-screen rectangle among this thread's
    /// top-level windows.
    pub unsafe fn find(ctx: &egui::Context) -> Option<HWND> {
        let (title, rect, ppp) = ctx.input(|i| {
            let v = i.viewport();
            (
                v.title.clone(),
                v.outer_rect,
                v.native_pixels_per_point.unwrap_or(1.),
            )
        });
        let (title, rect) = (title?, rect?);
        let mut search = Search {
            title: title.encode_utf16().collect(),
            rect: RECT {
                left: (rect.min.x * ppp).round() as i32,
                top: (rect.min.y * ppp).round() as i32,
                right: (rect.max.x * ppp).round() as i32,
                bottom: (rect.max.y * ppp).round() as i32,
            },
            best: None,
        };
        unsafe extern "system" fn visit(h: HWND, data: LPARAM) -> BOOL {
            let search = &mut *(data as *mut Search);
            let mut buf = [0u16; 512];
            let n = GetWindowTextW(h, buf.as_mut_ptr(), buf.len() as i32).max(0) as usize;
            if buf[..n] == search.title[..] {
                let mut r: RECT = std::mem::zeroed();
                GetWindowRect(h, &mut r);
                let t = search.rect;
                let d = (r.left - t.left).abs()
                    + (r.top - t.top).abs()
                    + (r.right - t.right).abs()
                    + (r.bottom - t.bottom).abs();
                if search.best.is_none_or(|(b, _)| d < b) {
                    search.best = Some((d, h));
                }
            }
            1
        }
        EnumThreadWindows(
            GetCurrentThreadId(),
            Some(visit),
            &mut search as *mut Search as LPARAM,
        );
        // A far-off match is a different window with the same title.
        search.best.filter(|(d, _)| *d < 64).map(|(_, h)| h)
    }
}

#[cfg(test)]
mod desktop_click_tests {
    #[test]
    fn a_hook_that_saw_nothing_while_the_pointer_moved_is_put_back() {
        assert!(super::hook_seems_gone(true, 41, 41));
        assert!(!super::hook_seems_gone(true, 41, 44), "it saw the moves");
        assert!(!super::hook_seems_gone(false, 41, 41), "nothing moved");
        assert!(
            !super::hook_seems_gone(true, u32::MAX, 2),
            "the count wrapped"
        );
    }

    #[test]
    fn a_pin_is_lost_when_the_window_or_its_layer_goes() {
        assert!(!super::pin_lost(true, true, 0));
        assert!(super::pin_lost(false, false, 0), "the window is gone");
        assert!(super::pin_lost(true, false, 0), "taken out of its layer");
        assert!(
            !super::pin_lost(true, true, 1950),
            "a short hide is waited out"
        );
        assert!(super::pin_lost(true, true, 2100), "the layer stayed hidden");
    }

    #[cfg(windows)]
    #[test]
    fn memos_stack_below_the_main_window_in_order() {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            CreateWindowExW, DestroyWindow, GetWindow, SetWindowPos, GW_HWNDNEXT, HWND_TOP,
            SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, WS_POPUP,
        };
        let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
        let make = || unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                std::ptr::null(),
                WS_POPUP,
                0,
                0,
                10,
                10,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
            )
        };
        let [main, a, b, c, other] = [make(), make(), make(), make(), make()];
        assert!(![main, a, b, c, other].iter().any(|h| h.is_null()));
        let flags = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE;
        unsafe {
            // From the top: c, main, a, other, b.
            SetWindowPos(main, HWND_TOP, 0, 0, 0, 0, flags);
            SetWindowPos(a, main, 0, 0, 0, 0, flags);
            SetWindowPos(other, a, 0, 0, 0, 0, flags);
            SetWindowPos(b, other, 0, 0, 0, 0, flags);
            SetWindowPos(c, HWND_TOP, 0, 0, 0, 0, flags);
        }
        let ids = |hs: &[windows_sys::Win32::Foundation::HWND]| -> Vec<isize> {
            hs.iter().map(|&h| h as isize).collect()
        };
        super::stack_below(main as isize, &ids(&[a, b, c]));
        let next = |h| unsafe { GetWindow(h, GW_HWNDNEXT) };
        assert_eq!(next(main), c);
        assert_eq!(next(c), a);
        assert_eq!(next(a), b);
        assert_eq!(next(b), other);
        unsafe {
            for h in [main, a, b, c, other] {
                DestroyWindow(h);
            }
        }
    }
    #[test]
    fn a_closed_handle_is_no_live_window() {
        assert!(!super::window_alive(0));
    }

    #[test]
    fn hit_test_uses_current_client_origin_including_negative_monitors() {
        let rect = [180, 21, 228, 69];
        for (x, y) in [(100, 200), (-1920, -500), (2560, 0)] {
            assert!(super::contains_point(rect, (x + 190) - x, (y + 30) - y));
            assert!(!super::contains_point(rect, (x + 228) - x, (y + 30) - y));
            assert!(!super::contains_point(rect, (x + 179) - x, (y + 30) - y));
        }
    }
}
