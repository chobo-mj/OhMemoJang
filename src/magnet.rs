//! Folded memo windows (title lines and color boxes) snap to each other's
//! edges while one of them is dragged, like magnets. Windows moves the
//! window itself (see `native_window`): the rectangle it proposes in
//! `WM_MOVING` is moved onto the nearest edge on the way.

/// Default snap distance (points).
pub const SNAP: f32 = 12.;
/// The range the snap distance can be set in (points).
pub const SNAP_MIN: f32 = 4.;
pub const SNAP_MAX: f32 = 40.;

/// A screen rectangle in physical pixels: left, top, right, bottom.
pub type Px = [i32; 4];

/// `moving` shifted, by at most `reach` px on each axis, so that one of its
/// edges meets an edge of a rectangle of `others` it is near: side by side
/// (left to right, top to bottom) or lined up (left with left, top with top).
/// On each axis the shortest such shift wins; with none in reach that axis
/// stays as it is.
pub fn snap(moving: Px, others: &[Px], reach: i32) -> Px {
    fn closer(best: &mut Option<i32>, d: i32, reach: i32) {
        if d.abs() <= reach && best.is_none_or(|b| d.abs() < b.abs()) {
            *best = Some(d);
        }
    }
    let [l, t, r, b] = moving;
    let (mut dx, mut dy) = (None, None);
    for &[ol, ot, or, ob] in others {
        // Only a window this one touches or nearly touches pulls it.
        let near = l <= or + reach && r >= ol - reach && t <= ob + reach && b >= ot - reach;
        if !near {
            continue;
        }
        for d in [or - l, ol - r, ol - l, or - r] {
            closer(&mut dx, d, reach);
        }
        for d in [ob - t, ot - b, ot - t, ob - b] {
            closer(&mut dy, d, reach);
        }
    }
    let (dx, dy) = (dx.unwrap_or(0), dy.unwrap_or(0));
    [l + dx, t + dy, r + dx, b + dy]
}

/// Turn the magnets on or off and set their reach (points). Cheap: called
/// every frame.
pub fn configure(on: bool, reach: f32) {
    #[cfg(windows)]
    imp::configure(on, reach);
    #[cfg(not(windows))]
    let _ = (on, reach);
}

/// Says whether the window `hwnd` is folded (a title line or a color box)
/// now. A folded window is pulled to the other folded windows while it is
/// dragged, and pulls them.
pub fn track(hwnd: isize, folded: bool) {
    #[cfg(windows)]
    imp::track(hwnd, folded);
    #[cfg(not(windows))]
    let _ = (hwnd, folded);
}

#[cfg(windows)]
mod imp {
    use super::Px;
    use std::sync::Mutex;
    use windows_sys::Win32::{
        Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
        UI::{
            HiDpi::GetDpiForWindow,
            Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
            WindowsAndMessaging::{
                GetWindowRect, IsIconic, IsWindow, IsWindowVisible, WM_MOVING, WM_NCDESTROY,
            },
        },
    };

    const ID: usize = 0x4d41_474e; // "MAGN"

    struct Magnets {
        on: bool,
        reach: f32,
        /// Folded windows now.
        folded: Vec<isize>,
        /// Windows the move hook is on (it stays until the window goes).
        hooked: Vec<isize>,
    }
    static STATE: Mutex<Magnets> = Mutex::new(Magnets {
        on: true,
        reach: super::SNAP,
        folded: Vec::new(),
        hooked: Vec::new(),
    });

    fn state() -> std::sync::MutexGuard<'static, Magnets> {
        STATE.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn configure(on: bool, reach: f32) {
        let mut s = state();
        s.on = on;
        s.reach = reach.clamp(super::SNAP_MIN, super::SNAP_MAX);
    }

    pub fn track(hwnd: isize, folded: bool) {
        if hwnd == 0 {
            return;
        }
        let mut s = state();
        let listed = s.folded.contains(&hwnd);
        if folded == listed && (!folded || s.hooked.contains(&hwnd)) {
            return;
        }
        // SAFETY: plain Win32 calls on windows of this thread (or gone ones).
        unsafe {
            s.folded.retain(|&h| h != hwnd && IsWindow(h as HWND) != 0);
            s.hooked.retain(|&h| IsWindow(h as HWND) != 0);
            if folded {
                s.folded.push(hwnd);
                if !s.hooked.contains(&hwnd)
                    && SetWindowSubclass(hwnd as HWND, Some(moving), ID, 0) != 0
                {
                    s.hooked.push(hwnd);
                }
            }
        }
    }

    /// The other folded windows on screen, if `h` is a folded window being
    /// moved with the magnets on, and their reach in pixels.
    unsafe fn targets(h: HWND) -> Option<(Vec<Px>, i32)> {
        let (others, reach) = {
            let s = state();
            if !s.on || !s.folded.contains(&(h as isize)) {
                return None;
            }
            let others: Vec<isize> = s
                .folded
                .iter()
                .copied()
                .filter(|&o| o != h as isize)
                .collect();
            (others, s.reach)
        };
        let rects = others
            .into_iter()
            .filter_map(|o| {
                let o = o as HWND;
                if IsWindow(o) == 0 || IsWindowVisible(o) == 0 || IsIconic(o) != 0 {
                    return None;
                }
                let mut r: RECT = std::mem::zeroed();
                (GetWindowRect(o, &mut r) != 0).then_some([r.left, r.top, r.right, r.bottom])
            })
            .collect();
        let dpi = GetDpiForWindow(h).max(96) as f32;
        Some((rects, (reach * dpi / 96.).round() as i32))
    }

    unsafe extern "system" fn moving(
        h: HWND,
        msg: u32,
        w: WPARAM,
        l: LPARAM,
        _id: usize,
        _data: usize,
    ) -> LRESULT {
        if msg == WM_MOVING && l != 0 {
            if let Some((others, reach)) = targets(h) {
                let rc = &mut *(l as *mut RECT);
                let [left, top, right, bottom] =
                    super::snap([rc.left, rc.top, rc.right, rc.bottom], &others, reach);
                *rc = RECT {
                    left,
                    top,
                    right,
                    bottom,
                };
            }
        } else if msg == WM_NCDESTROY {
            RemoveWindowSubclass(h, Some(moving), ID);
        }
        DefSubclassProc(h, msg, w, l)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_title_line_dropped_near_another_lines_up_beneath_it() {
        let other = [100, 100, 400, 144];
        // 10 px below and 7 px to the right: sits right under it, edges lined up.
        assert_eq!(
            snap([107, 154, 407, 198], &[other], 12),
            [100, 144, 400, 188]
        );
        // Beside it, a little lower: joins its right edge, tops lined up.
        assert_eq!(
            snap([409, 105, 457, 153], &[other], 12),
            [400, 100, 448, 148]
        );
    }
    #[test]
    fn nothing_moves_out_of_reach_or_far_away_on_the_other_axis() {
        let other = [100, 100, 400, 144];
        // 13 px below with a 12 px reach.
        assert_eq!(
            snap([100, 157, 400, 201], &[other], 12),
            [100, 157, 400, 201]
        );
        // Its left edge is near the other's right edge, but it is far below.
        assert_eq!(
            snap([405, 400, 453, 448], &[other], 12),
            [405, 400, 453, 448]
        );
        assert_eq!(snap([1, 2, 3, 4], &[], 12), [1, 2, 3, 4]);
    }
    #[test]
    fn the_nearest_edge_wins_on_each_axis() {
        let a = [0, 0, 100, 40];
        let b = [0, 60, 100, 100];
        // Between the two: 5 px from a's bottom, 8 px from b's top.
        assert_eq!(snap([3, 45, 103, 52], &[a, b], 12), [0, 40, 100, 47]);
    }
}
