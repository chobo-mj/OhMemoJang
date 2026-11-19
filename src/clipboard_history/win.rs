//! Windows side of the clipboard history: a thread with two hidden
//! message-only windows. The *listener* window receives
//! `WM_CLIPBOARDUPDATE`; the *owner* window is used only to publish restored
//! items, so "the clipboard owner is the owner window" means "we wrote it".
//! `unsafe` is confined to this file.
#[cfg(not(windows))]
use super::{Cmd, Event, Shared, WinCmd};
#[cfg(not(windows))]
use eframe::egui;
#[cfg(not(windows))]
use std::sync::mpsc::Sender;
#[cfg(not(windows))]
use std::sync::Arc;

#[cfg(windows)]
pub use imp::{post, spawn};

#[cfg(not(windows))]
pub fn spawn(_: Arc<Shared>, _: Sender<Cmd>, _: Sender<Event>, _: egui::Context) {}
#[cfg(not(windows))]
pub fn post(_: &Shared, _: WinCmd) {}

#[cfg(windows)]
mod imp {
    use super::super::formats::*;
    use super::super::{push_pending, Cmd, Event, Raw, RestoreOutcome, Shared, WinCmd};
    use eframe::egui;
    use std::cell::RefCell;
    use std::ptr::{null, null_mut};
    use std::sync::atomic::Ordering::{Relaxed, SeqCst};
    use std::sync::mpsc::Sender;
    use std::sync::Arc;
    use windows_sys::Win32::Foundation::{GlobalFree, HGLOBAL, HWND, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::System::DataExchange::{
        AddClipboardFormatListener, CloseClipboard, EmptyClipboard, GetClipboardData,
        GetClipboardOwner, GetClipboardSequenceNumber, IsClipboardFormatAvailable, OpenClipboard,
        RegisterClipboardFormatW, RemoveClipboardFormatListener, SetClipboardData,
    };
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::System::Memory::{
        GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, KillTimer,
        PostMessageW, PostQuitMessage, RegisterClassW, SetTimer, TranslateMessage, HWND_MESSAGE,
        MSG, WM_APP, WM_DESTROY, WM_TIMER, WNDCLASSW,
    };

    const CF_UNICODETEXT: u32 = 13;
    const CF_DIB: u32 = 8;
    const CF_DIBV5: u32 = 17;
    const WM_CLIPBOARDUPDATE: u32 = 0x031D;
    const WM_WAKE: u32 = WM_APP + 1;
    const RETRY_TIMER: usize = 1;
    /// Waits before each retry when another program holds the clipboard.
    const RETRY_MS: [u32; 4] = [10, 25, 50, 100];

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    struct Fmt {
        html: u32,
        url: u32,
        png: u32,
        token: u32,
        exclude: u32,
        can_include: u32,
    }

    struct State {
        shared: Arc<Shared>,
        tx_cmd: Sender<Cmd>,
        tx_ev: Sender<Event>,
        ctx: egui::Context,
        listener: HWND,
        owner: HWND,
        /// Random per run; published with every restore, never stored.
        token: Vec<u8>,
        fmt: Fmt,
        /// Sequence number that was on the clipboard when collection began.
        ignore_seq: u32,
        /// Sequence number of the last capture taken, so one change is
        /// never read twice (a retry and a new notice for the same copy).
        last_seq: u32,
        retry: usize,
    }

    thread_local! {
        static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
    }

    fn with_state(f: impl FnOnce(&mut State)) {
        STATE.with(|s| {
            // A re-entrant message while a clipboard call blocks is ignored.
            if let Ok(mut b) = s.try_borrow_mut() {
                if let Some(st) = b.as_mut() {
                    f(st);
                }
            }
        });
    }

    pub fn spawn(
        shared: Arc<Shared>,
        tx_cmd: Sender<Cmd>,
        tx_ev: Sender<Event>,
        ctx: egui::Context,
    ) {
        let guard = super::super::ThreadGuard::new(&shared);
        let _ = std::thread::Builder::new()
            .name("clipboard-listener".into())
            .spawn(move || {
                let _guard = guard;
                // SAFETY: the thread owns everything it creates.
                unsafe { run(shared, tx_cmd, tx_ev, ctx) }
            });
    }

    pub fn post(shared: &Shared, cmd: WinCmd) {
        shared
            .win_queue
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push_back(cmd);
        let h = shared.win_hwnd.load(SeqCst);
        if h != 0 {
            // SAFETY: posting to a window handle is harmless if it is gone.
            unsafe {
                PostMessageW(h as HWND, WM_WAKE, 0, 0);
            }
        }
    }

    fn random_token() -> Vec<u8> {
        use sha2::{Digest, Sha256};
        use std::hash::{BuildHasher, Hasher};
        let mut h = Sha256::new();
        h.update(std::process::id().to_le_bytes());
        h.update(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
                .to_le_bytes(),
        );
        h.update(
            std::collections::hash_map::RandomState::new()
                .build_hasher()
                .finish()
                .to_le_bytes(),
        );
        h.finalize()[..16].to_vec()
    }

    unsafe fn run(
        shared: Arc<Shared>,
        tx_cmd: Sender<Cmd>,
        tx_ev: Sender<Event>,
        ctx: egui::Context,
    ) {
        let hinst = GetModuleHandleW(null());
        let class = wide("OhMemoJangClipboardHistory");
        let mut wc: WNDCLASSW = std::mem::zeroed();
        wc.lpfnWndProc = Some(wndproc);
        wc.hInstance = hinst;
        wc.lpszClassName = class.as_ptr();
        RegisterClassW(&wc);
        let make = || {
            CreateWindowExW(
                0,
                class.as_ptr(),
                class.as_ptr(),
                0,
                0,
                0,
                0,
                0,
                HWND_MESSAGE,
                null_mut(),
                hinst,
                null(),
            )
        };
        let listener = make();
        let owner = make();
        if listener.is_null() || owner.is_null() {
            let _ = tx_ev.send(Event::Notice("클립보드 감시 창을 만들 수 없습니다".into()));
            return;
        }
        let reg = |n: &str| RegisterClipboardFormatW(wide(n).as_ptr());
        let fmt = Fmt {
            html: reg("HTML Format"),
            url: reg("UniformResourceLocatorW"),
            png: reg("PNG"),
            token: reg("OhMemoJang.ClipboardHistory.Restore/v1"),
            exclude: reg("ExcludeClipboardContentFromMonitorProcessing"),
            can_include: reg("CanIncludeInClipboardHistory"),
        };
        if AddClipboardFormatListener(listener) == 0 {
            let _ = tx_ev.send(Event::Notice(
                "클립보드 변경 알림을 등록할 수 없습니다".into(),
            ));
        }
        STATE.with(|s| {
            *s.borrow_mut() = Some(State {
                shared: shared.clone(),
                tx_cmd,
                tx_ev,
                ctx,
                listener,
                owner,
                token: random_token(),
                fmt,
                ignore_seq: 0,
                last_seq: 0,
                retry: 0,
            })
        });
        shared.win_hwnd.store(listener as isize, SeqCst);
        // Commands posted before the window existed.
        with_state(|s| s.on_wake());
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        shared.win_hwnd.store(0, SeqCst);
        RemoveClipboardFormatListener(listener);
        DestroyWindow(owner);
    }

    unsafe extern "system" fn wndproc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_CLIPBOARDUPDATE => {
                with_state(|s| s.on_update());
                0
            }
            WM_TIMER if wparam == RETRY_TIMER => {
                KillTimer(hwnd, RETRY_TIMER);
                with_state(|s| {
                    if s.shared.collecting.load(Relaxed) {
                        s.capture();
                    }
                });
                0
            }
            WM_WAKE => {
                with_state(|s| s.on_wake());
                0
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    enum Read {
        /// Our own write, an excluded copy, or nothing supported.
        Skip,
        /// The clipboard kept changing while it was read.
        Unstable,
        Got(Vec<(&'static str, Vec<u8>)>, bool),
    }

    enum Fail {
        Missing,
        Oversize,
    }

    /// Copies one format out of the open clipboard into owned memory.
    unsafe fn read_format(cf: u32, limit: usize) -> Result<Vec<u8>, Fail> {
        if IsClipboardFormatAvailable(cf) == 0 {
            return Err(Fail::Missing);
        }
        let h = GetClipboardData(cf);
        if h.is_null() {
            return Err(Fail::Missing);
        }
        let size = GlobalSize(h as HGLOBAL);
        if size == 0 {
            return Err(Fail::Missing);
        }
        if size > limit {
            return Err(Fail::Oversize);
        }
        let p = GlobalLock(h as HGLOBAL) as *const u8;
        if p.is_null() {
            return Err(Fail::Missing);
        }
        let v = std::slice::from_raw_parts(p, size).to_vec();
        GlobalUnlock(h as HGLOBAL);
        Ok(v)
    }

    impl State {
        unsafe fn on_update(&mut self) {
            // A newer change replaces any retry in flight.
            KillTimer(self.listener, RETRY_TIMER);
            self.retry = 0;
            if !self.shared.collecting.load(Relaxed) {
                return;
            }
            self.capture();
        }

        unsafe fn capture(&mut self) {
            // Never block the listener while a UI clipboard write is in progress.
            // The write records its final sequence before releasing this lock.
            let Ok(own_sequence) = crate::clipboard_copy::LAST_WRITE.try_lock() else {
                SetTimer(self.listener, RETRY_TIMER, 10, None);
                return;
            };
            let current = GetClipboardSequenceNumber();
            if current != 0 && (current == self.ignore_seq || current == self.last_seq) {
                return;
            }
            if current != 0 && current == *own_sequence && self.shared.ignore_own_copy.load(Relaxed)
            {
                return;
            }
            let seq = self.shared.seq.fetch_add(1, Relaxed) + 1;
            let generation = self.shared.generation.load(Relaxed);
            if OpenClipboard(self.listener) == 0 {
                if self.retry < RETRY_MS.len() {
                    SetTimer(self.listener, RETRY_TIMER, RETRY_MS[self.retry], None);
                    self.retry += 1;
                } else {
                    self.retry = 0;
                    self.shared.counters.missed_locked.fetch_add(1, Relaxed);
                }
                return;
            }
            self.retry = 0;
            let result = self.read_open();
            CloseClipboard();
            match result {
                Read::Skip => {}
                Read::Unstable => {
                    self.shared.counters.missed_locked.fetch_add(1, Relaxed);
                }
                Read::Got(formats, reduced) => {
                    self.last_seq = current;
                    let captured_ms = chrono::Utc::now().timestamp_millis();
                    push_pending(
                        &self.shared,
                        Raw {
                            seq,
                            generation,
                            captured_ms,
                            reduced,
                            formats,
                        },
                    );
                    let _ = self.tx_cmd.send(Cmd::CaptureReady);
                }
            }
        }

        /// The clipboard is open. Exclusions are checked before any content
        /// is read.
        unsafe fn read_open(&mut self) -> Read {
            let owner = GetClipboardOwner();
            if !owner.is_null() && self.shared.ignore_own_copy.load(Relaxed) {
                let mut process = 0;
                windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(
                    owner,
                    &mut process,
                );
                if process == std::process::id() {
                    return Read::Skip;
                }
            }
            if !owner.is_null() && owner == self.owner {
                return Read::Skip;
            }
            if IsClipboardFormatAvailable(self.fmt.exclude) != 0 {
                return Read::Skip;
            }
            if let Ok(b) = read_format(self.fmt.can_include, 64) {
                if b.len() >= 4 && u32::from_le_bytes([b[0], b[1], b[2], b[3]]) == 0 {
                    return Read::Skip;
                }
            }
            for _ in 0..2 {
                let before = GetClipboardSequenceNumber();
                let (formats, reduced) = self.read_formats();
                let after = GetClipboardSequenceNumber();
                if before == after || before == 0 {
                    return if formats.is_empty() {
                        Read::Skip
                    } else {
                        Read::Got(formats, reduced)
                    };
                }
            }
            Read::Unstable
        }

        unsafe fn read_formats(&self) -> (Vec<(&'static str, Vec<u8>)>, bool) {
            let c = &self.shared.counters;
            let mut out: Vec<(&'static str, Vec<u8>)> = vec![];
            let mut reduced = false;
            let mut total = 0usize;
            let mut take = |key: &'static str, cf: u32, out: &mut Vec<(&'static str, Vec<u8>)>| {
                match read_format(cf, limit_of(key)) {
                    Ok(v) if total + v.len() <= MAX_ITEM_BYTES => {
                        total += v.len();
                        out.push((key, v));
                    }
                    Ok(_) | Err(Fail::Oversize) => {
                        c.oversize.fetch_add(1, Relaxed);
                        reduced = true;
                    }
                    Err(Fail::Missing) => {}
                }
            };
            take(K_TEXT, CF_UNICODETEXT, &mut out);
            take(K_HTML, self.fmt.html, &mut out);
            take(K_URL, self.fmt.url, &mut out);
            // Cell ranges also offer a picture; it is not read for them.
            let table_like = out.iter().any(|(k, v)| match *k {
                K_HTML => v.windows(6).any(|w| w.eq_ignore_ascii_case(b"<table")),
                K_TEXT => utf16le_to_string(v).is_some_and(|t| looks_like_tsv(&t)),
                _ => false,
            });
            if !table_like {
                take(K_PNG, self.fmt.png, &mut out);
                // Windows offers DIB and DIBV5 for each other: keep one.
                if IsClipboardFormatAvailable(CF_DIBV5) != 0 {
                    take(K_DIBV5, CF_DIBV5, &mut out);
                } else {
                    take(K_DIB, CF_DIB, &mut out);
                }
            }
            (out, reduced)
        }

        unsafe fn on_wake(&mut self) {
            loop {
                let cmd = self
                    .shared
                    .win_queue
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .pop_front();
                match cmd {
                    None => break,
                    Some(WinCmd::Rebase) => self.ignore_seq = GetClipboardSequenceNumber(),
                    Some(WinCmd::Restore { formats, epoch }) => {
                        let outcome = self.restore(formats, epoch);
                        let _ = self.tx_ev.send(Event::Restored { outcome });
                        self.ctx.request_repaint();
                    }
                    Some(WinCmd::Quit) => {
                        DestroyWindow(self.listener);
                    }
                }
            }
        }

        /// Puts a stored item on the clipboard. All memory is prepared
        /// first: if that or `OpenClipboard` fails, the current clipboard is
        /// not touched. Multi-format writes are not atomic, so there is no
        /// rollback.
        unsafe fn restore(
            &mut self,
            formats: Vec<(String, Vec<u8>)>,
            epoch: u64,
        ) -> RestoreOutcome {
            let mut jobs: Vec<(u8, &'static str, u32, Vec<u8>)> = vec![];
            for (key, mut data) in formats {
                let (rank, name, cf) = match key.as_str() {
                    K_HTML => {
                        data.push(0);
                        (0, K_HTML, self.fmt.html)
                    }
                    K_PNG => (1, K_PNG, self.fmt.png),
                    K_DIBV5 => (2, K_DIBV5, CF_DIBV5),
                    K_DIB => (3, K_DIB, CF_DIB),
                    K_URL => {
                        data.extend([0, 0]);
                        (4, K_URL, self.fmt.url)
                    }
                    K_TEXT => {
                        data.extend([0, 0]);
                        (5, K_TEXT, CF_UNICODETEXT)
                    }
                    _ => continue,
                };
                jobs.push((rank, name, cf, data));
            }
            // Rich forms before plain text.
            jobs.sort_by_key(|j| j.0);
            let mut prepared: Vec<(&'static str, u32, HGLOBAL)> = vec![];
            let free_all = |p: &[(&'static str, u32, HGLOBAL)]| {
                for (_, _, h) in p {
                    GlobalFree(*h);
                }
            };
            for (_, name, cf, data) in &jobs {
                match alloc(data) {
                    Some(h) => prepared.push((name, *cf, h)),
                    None => {
                        free_all(&prepared);
                        return RestoreOutcome::NotTouched("메모리를 준비하지 못했습니다".into());
                    }
                }
            }
            // Keep Windows history and cloud sync out of it.
            let mut extras: Vec<(&'static str, u32, HGLOBAL)> = vec![];
            if let Some(h) = alloc(&self.token) {
                extras.push(("token", self.fmt.token, h));
            }
            if let Some(h) = alloc(&[0u8; 4]) {
                extras.push(("exclude", self.fmt.exclude, h));
            }
            if prepared.is_empty() {
                free_all(&extras);
                return RestoreOutcome::NotTouched("복원할 형식이 없습니다".into());
            }
            let mut opened = false;
            for _ in 0..8 {
                if OpenClipboard(self.owner) != 0 {
                    opened = true;
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(15));
            }
            if !opened {
                free_all(&prepared);
                free_all(&extras);
                return RestoreOutcome::NotTouched(
                    "다른 프로그램이 클립보드를 사용 중입니다".into(),
                );
            }
            // Deleted while waiting: publish nothing.
            if self.shared.epoch.load(Relaxed) != epoch {
                CloseClipboard();
                free_all(&prepared);
                free_all(&extras);
                return RestoreOutcome::Cancelled;
            }
            EmptyClipboard();
            let mut done: Vec<String> = vec![];
            for (name, cf, h) in &prepared {
                if SetClipboardData(*cf, *h as _).is_null() {
                    GlobalFree(*h);
                } else {
                    done.push((*name).to_owned());
                }
            }
            for (_, cf, h) in &extras {
                if SetClipboardData(*cf, *h as _).is_null() {
                    GlobalFree(*h);
                }
            }
            // Our own write is recognized by its owner window.
            self.ignore_seq = 0;
            CloseClipboard();
            if done.len() == prepared.len() {
                RestoreOutcome::Full
            } else if done.is_empty() {
                RestoreOutcome::Cleared
            } else {
                RestoreOutcome::Partial(done)
            }
        }
    }

    unsafe fn alloc(data: &[u8]) -> Option<HGLOBAL> {
        let h = GlobalAlloc(GMEM_MOVEABLE, data.len().max(1));
        if h.is_null() {
            return None;
        }
        let p = GlobalLock(h) as *mut u8;
        if p.is_null() {
            GlobalFree(h);
            return None;
        }
        std::ptr::copy_nonoverlapping(data.as_ptr(), p, data.len());
        GlobalUnlock(h);
        Some(h)
    }
}
