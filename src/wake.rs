//! Waking the program from other threads. The program sleeps while nothing
//! happens; whatever happens on another thread (a shortcut, the tray icon, a
//! second launch) calls `now` to run one frame.
use std::sync::{Mutex, OnceLock};

static CTX: OnceLock<eframe::egui::Context> = OnceLock::new();
static HOTKEYS: Mutex<Vec<u32>> = Mutex::new(Vec::new());

pub fn set(ctx: &eframe::egui::Context) {
    let _ = CTX.set(ctx.clone());
}

/// Run a frame soon.
pub fn now() {
    if let Some(ctx) = CTX.get() {
        ctx.request_repaint();
    }
}

/// Global shortcut presses are collected here by a thread that waits for
/// them, so the frame loop does not have to look for them.
pub fn watch_hotkeys() {
    use global_hotkey::{GlobalHotKeyEvent, HotKeyState};
    std::thread::spawn(|| {
        while let Ok(e) = GlobalHotKeyEvent::receiver().recv() {
            if e.state == HotKeyState::Pressed {
                HOTKEYS.lock().unwrap_or_else(|e| e.into_inner()).push(e.id);
                now();
            }
        }
    });
}

/// Ids of the shortcuts pressed since the last call.
pub fn take_hotkeys() -> Vec<u32> {
    std::mem::take(&mut *HOTKEYS.lock().unwrap_or_else(|e| e.into_inner()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pressed_shortcuts_are_handed_over_once() {
        HOTKEYS.lock().unwrap().extend([3, 7]);
        assert_eq!(take_hotkeys(), vec![3, 7]);
        assert!(take_hotkeys().is_empty());
        // Waking without a window to wake does nothing.
        now();
    }
}
