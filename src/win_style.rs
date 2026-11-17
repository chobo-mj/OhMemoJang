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

/// A screen rectangle in physical pixels: left, top, right, bottom.
pub type PxRect = [i32; 4];

/// Put window `hwnd` on the desktop wallpaper layer at `rect`: below the
/// desktop icons, so clicks go to the desktop and icons can sit on top.
pub fn pin_to_desktop(hwnd: isize, rect: PxRect) -> bool {
    #[cfg(windows)]
    return unsafe { desk::pin(hwnd, rect) };
    #[cfg(not(windows))]
    {
        let _ = (hwnd, rect);
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

/// Watches, on a small background thread, for a click on `rect` while the
/// desktop (not another window) is under the pointer. The standing calendar
/// lives below the desktop icons and gets no input of its own.
pub struct DesktopClick {
    hit: std::sync::Arc<std::sync::atomic::AtomicBool>,
    rect: std::sync::Arc<std::sync::Mutex<PxRect>>,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    thread: u32,
}

impl DesktopClick {
    pub fn watch(hwnd: isize, rect: PxRect, ctx: egui::Context) -> Option<Self> {
        use std::sync::{atomic::AtomicBool, Arc, Mutex};
        let hit = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let rect = Arc::new(Mutex::new(rect));
        #[cfg(windows)]
        let thread = desk::watch(hwnd, rect.clone(), hit.clone(), stop.clone(), ctx)?;
        #[cfg(not(windows))]
        let thread = {
            let _ = (hwnd, ctx);
            0
        };
        Some(Self {
            hit,
            rect,
            stop,
            thread,
        })
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

#[cfg(windows)]
mod desk {
    use super::PxRect;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    };
    use windows_sys::Win32::{
        Foundation::{GetLastError, SetLastError, BOOL, HWND, LPARAM, LRESULT, POINT, WPARAM},
        Graphics::Gdi::{ClientToScreen, ScreenToClient},
        System::{LibraryLoader::GetModuleHandleW, Threading::GetCurrentThreadId},
        UI::WindowsAndMessaging::{
            CallNextHookEx, EnumWindows, FindWindowExW, FindWindowW, GetClassNameW, GetMessageW,
            IsChild, PeekMessageW, SendMessageTimeoutW, SetParent, SetWindowPos, SetWindowsHookExW,
            UnhookWindowsHookEx, WindowFromPoint, MSG, MSLLHOOKSTRUCT, PM_NOREMOVE, SMTO_NORMAL,
            SWP_NOACTIVATE, SWP_NOZORDER, SWP_SHOWWINDOW, WH_MOUSE_LL, WM_LBUTTONUP,
        },
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    /// The window that draws the wallpaper, behind the desktop icons.
    unsafe fn wallpaper_layer() -> HWND {
        let progman = FindWindowW(wide("Progman").as_ptr(), std::ptr::null());
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
        if worker.is_null() {
            // Windows 11 24H2: the WorkerW is a child of Progman, under the icons.
            worker = FindWindowExW(
                progman,
                std::ptr::null_mut(),
                wide("WorkerW").as_ptr(),
                std::ptr::null(),
            );
        }
        if worker.is_null() {
            progman
        } else {
            worker
        }
    }

    pub unsafe fn pin(hwnd: isize, r: PxRect) -> bool {
        let layer = wallpaper_layer();
        if layer.is_null() {
            return false;
        }
        let h = hwnd as HWND;
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
        let h = hwnd as HWND;
        SetLastError(0);
        if SetParent(h, std::ptr::null_mut()).is_null() && GetLastError() != 0 {
            return false;
        }
        SetWindowPos(
            h,
            std::ptr::null_mut(),
            r[0],
            r[1],
            r[2] - r[0],
            r[3] - r[1],
            SWP_NOZORDER | SWP_SHOWWINDOW,
        );
        true
    }

    /// The desktop is under `p` (not some other window).
    unsafe fn desktop_at(p: POINT, calendar: HWND) -> bool {
        let top = WindowFromPoint(p);
        if top == calendar || IsChild(calendar, top) != 0 {
            return true;
        }
        let mut buf = [0u16; 64];
        let n = GetClassNameW(top, buf.as_mut_ptr(), buf.len() as i32).max(0) as usize;
        let class = String::from_utf16_lossy(&buf[..n]);
        matches!(
            class.as_str(),
            "SysListView32" | "SHELLDLL_DefView" | "WorkerW" | "Progman"
        ) || top.is_null()
    }

    struct Watch {
        hwnd: isize,
        rect: Arc<Mutex<PxRect>>,
        hit: Arc<AtomicBool>,
        ctx: eframe::egui::Context,
    }
    thread_local! {
        static WATCH: std::cell::RefCell<Option<Watch>> = const { std::cell::RefCell::new(None) };
    }

    unsafe extern "system" fn hook(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
        if code >= 0 && w as u32 == WM_LBUTTONUP {
            let info = &*(l as *const MSLLHOOKSTRUCT);
            WATCH.with(|slot| {
                let watch = slot.borrow();
                let Some(watch) = watch.as_ref() else {
                    return;
                };
                let Ok(rect) = watch.rect.try_lock().map(|r| *r) else {
                    return;
                };
                let h = watch.hwnd as HWND;
                let mut origin = POINT { x: 0, y: 0 };
                // Resolve the native client position at the instant of the click.
                // Reparenting, title-bar changes and monitor positions cannot stale it.
                if ClientToScreen(h, &mut origin) != 0
                    && super::contains_point(rect, info.pt.x - origin.x, info.pt.y - origin.y)
                    && desktop_at(info.pt, h)
                {
                    watch.hit.store(true, Ordering::Relaxed);
                    watch.ctx.request_repaint();
                }
            });
        }
        CallNextHookEx(std::ptr::null_mut(), code, w, l)
    }

    pub fn watch(
        hwnd: isize,
        rect: Arc<Mutex<PxRect>>,
        hit: Arc<AtomicBool>,
        stop: Arc<AtomicBool>,
        ctx: eframe::egui::Context,
    ) -> Option<u32> {
        let (ready, result) = std::sync::mpsc::sync_channel(1);
        let cancel = stop.clone();
        let thread_id = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let started_thread = thread_id.clone();
        std::thread::Builder::new()
            .name("standing-calendar-input".into())
            .spawn(move || unsafe {
                let mut msg: MSG = std::mem::zeroed();
                // Create the queue before publishing the thread id, so Drop cannot
                // lose its quit message during a rapid unpin/pin transition.
                PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_NOREMOVE);
                WATCH.with(|slot| {
                    *slot.borrow_mut() = Some(Watch {
                        hwnd,
                        rect,
                        hit,
                        ctx,
                    })
                });
                let module = GetModuleHandleW(std::ptr::null());
                let h = SetWindowsHookExW(WH_MOUSE_LL, Some(hook), module, 0);
                if h.is_null() {
                    let _ = ready.send(None);
                    WATCH.with(|slot| *slot.borrow_mut() = None);
                    return;
                }
                started_thread.store(GetCurrentThreadId(), Ordering::SeqCst);
                let delivered = ready.send(Some(GetCurrentThreadId())).is_ok();
                while delivered
                    && !stop.load(Ordering::Relaxed)
                    && GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0
                {}
                UnhookWindowsHookEx(h);
                WATCH.with(|slot| *slot.borrow_mut() = None);
            })
            .ok()?;
        match result.recv_timeout(std::time::Duration::from_secs(1)) {
            Ok(thread) => thread,
            Err(_) => {
                cancel.store(true, Ordering::SeqCst);
                let id = thread_id.load(Ordering::SeqCst);
                if id != 0 {
                    unsafe {
                        windows_sys::Win32::UI::WindowsAndMessaging::PostThreadMessageW(
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
            Dwm::{DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND},
            Gdi::{CreateRoundRectRgn, SetWindowRgn},
        },
        System::Threading::GetCurrentThreadId,
        UI::WindowsAndMessaging::{
            EnumThreadWindows, GetWindowLongPtrW, GetWindowRect, GetWindowTextW, IsWindow,
            SetLayeredWindowAttributes, SetWindowLongPtrW, GWL_EXSTYLE, LWA_ALPHA, WS_EX_LAYERED,
        },
    };

    #[derive(Clone, Copy, Default)]
    struct State {
        hwnd: isize,
        alpha: Option<u8>,
        native_round: Option<bool>,
        region: (i32, i32),
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
    fn hit_test_uses_current_client_origin_including_negative_monitors() {
        let rect = [180, 21, 228, 69];
        for (x, y) in [(100, 200), (-1920, -500), (2560, 0)] {
            assert!(super::contains_point(rect, (x + 190) - x, (y + 30) - y));
            assert!(!super::contains_point(rect, (x + 228) - x, (y + 30) - y));
            assert!(!super::contains_point(rect, (x + 179) - x, (y + 30) - y));
        }
    }
}
