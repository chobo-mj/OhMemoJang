//! Clipboard history: while the app runs, what other programs copy is stored
//! locally (SQLite) and can be put back on the system clipboard with a click.
//!
//! Threads:
//! * UI (egui): list state and drawing only; everything else is a request.
//! * `win` (Windows only): hidden windows, the clipboard listener, capture
//!   and restore. One thread so capture and restore writes are serialized.
//! * `worker`: validation, previews, SQLite, retention.
mod formats;
mod store;
mod view;
mod win;
mod worker;

use eframe::egui;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU64, AtomicUsize, Ordering::Relaxed};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard};

pub use store::{ItemMeta, Policy, Query};

/// Raw bytes of one capture, waiting for the storage worker.
pub struct Raw {
    pub seq: u64,
    pub generation: u64,
    pub captured_ms: i64,
    pub reduced: bool,
    pub formats: Vec<(&'static str, Vec<u8>)>,
}

impl Raw {
    pub fn bytes(&self) -> usize {
        self.formats.iter().map(|f| f.1.len()).sum()
    }
}

/// Work for the Windows clipboard thread.
pub enum WinCmd {
    Restore {
        formats: Vec<(String, Vec<u8>)>,
        epoch: u64,
    },
    /// Treat what is on the clipboard now as already seen.
    Rebase,
    Quit,
}

#[derive(Default)]
pub struct Counters {
    pub missed_locked: AtomicU64,
    pub oversize: AtomicU64,
    pub unsupported: AtomicU64,
    pub dropped_backlog: AtomicU64,
    pub store_errors: AtomicU64,
}

/// State shared by the three threads.
pub struct Shared {
    /// Collection is on (enabled and not paused). Checked before reading.
    pub collecting: AtomicBool,
    pub ignore_own_copy: AtomicBool,
    /// Raised whenever collection stops or restarts: older captures are dropped.
    pub generation: AtomicU64,
    /// Capture order; delete-all uses it as a watermark.
    pub seq: AtomicU64,
    /// Raised by every delete: queued restores of deleted data are cancelled.
    pub epoch: AtomicU64,
    pub pending: Mutex<VecDeque<Raw>>,
    pub counters: Counters,
    pub win_queue: Mutex<VecDeque<WinCmd>>,
    pub win_hwnd: AtomicIsize,
    /// Background threads (storage worker, clipboard listener) still running.
    pub threads: AtomicUsize,
}

/// Counts a background thread of the clipboard history while it lives; made
/// before the thread is started and moved into it.
pub struct ThreadGuard(Arc<Shared>);

impl ThreadGuard {
    pub fn new(shared: &Arc<Shared>) -> Self {
        shared
            .threads
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Self(shared.clone())
    }
}

impl Drop for ThreadGuard {
    fn drop(&mut self) {
        self.0
            .threads
            .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }
}

impl Shared {
    fn new() -> Self {
        Self {
            collecting: AtomicBool::new(false),
            ignore_own_copy: AtomicBool::new(true),
            generation: AtomicU64::new(1),
            seq: AtomicU64::new(0),
            epoch: AtomicU64::new(0),
            pending: Mutex::new(VecDeque::new()),
            counters: Counters::default(),
            win_queue: Mutex::new(VecDeque::new()),
            win_hwnd: AtomicIsize::new(0),
            threads: AtomicUsize::new(0),
        }
    }
    pub fn pending(&self) -> MutexGuard<'_, VecDeque<Raw>> {
        self.pending.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Queue and byte budget of captures waiting for storage. The oldest wait
/// is dropped first so the newest copy always survives.
pub const MAX_PENDING: usize = 8;
pub const MAX_PENDING_BYTES: usize = 64 << 20;

pub fn push_pending(shared: &Shared, raw: Raw) {
    let mut q = shared.pending();
    q.push_back(raw);
    let mut bytes: usize = q.iter().map(Raw::bytes).sum();
    while q.len() > 1 && (q.len() > MAX_PENDING || bytes > MAX_PENDING_BYTES) {
        if let Some(old) = q.pop_front() {
            bytes -= old.bytes();
            shared.counters.dropped_backlog.fetch_add(1, Relaxed);
        }
    }
}

/// UI → worker.
pub enum Cmd {
    CaptureReady,
    List {
        req: u64,
        query: Query,
    },
    Thumb(i64),
    Detail(i64),
    /// Enlarged image preview (decoded on demand).
    Full(i64),
    Restore(i64),
    /// The user's comment on an item ("" removes it).
    Comment(i64, String),
    Pin(i64, bool),
    Delete(i64),
    DeleteBefore(i64),
    DeleteAll {
        watermark: u64,
    },
    Policy {
        policy: Policy,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum RestoreOutcome {
    Full,
    Partial(Vec<String>),
    /// Prepared nothing; the clipboard was not touched.
    NotTouched(String),
    /// The clipboard was emptied but no content could be published.
    Cleared,
    Cancelled,
}

/// Worker / clipboard thread → UI.
pub enum Event {
    List {
        req: u64,
        items: Vec<ItemMeta>,
        has_more: bool,
        append: bool,
    },
    Thumb {
        id: i64,
        size: [usize; 2],
        rgba: Vec<u8>,
    },
    Detail {
        id: i64,
        text: Option<String>,
        /// Cells and the full (rows, cols).
        table: Option<(Vec<Vec<String>>, usize, usize)>,
    },
    Full {
        id: i64,
        size: [usize; 2],
        rgba: Vec<u8>,
    },
    /// Stored items changed: reload the list.
    Changed,
    Stats {
        count: i64,
        bytes: i64,
    },
    Restored {
        outcome: RestoreOutcome,
    },
    Notice(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    /// Collection is on. On from the start; a settings file written before
    /// 2.1 (no `default_on` line) is turned on once.
    pub enabled: bool,
    /// Survives restarts.
    pub paused: bool,
    /// Keep only the latest `max_items` (off: no count limit).
    pub count_limit: bool,
    pub max_items: usize,
    /// Delete items older than `retention_days` (off: no time limit).
    pub period_limit: bool,
    pub retention_days: u32,
    pub ignore_own_copy: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: true,
            paused: false,
            count_limit: false,
            max_items: 100,
            period_limit: true,
            retention_days: 30,
            ignore_own_copy: true,
        }
    }
}

impl Settings {
    pub const ITEM_CHOICES: [usize; 5] = [50, 100, 300, 500, 1000];
    pub const DAY_CHOICES: [u32; 3] = [7, 30, 90];

    pub fn collecting(&self) -> bool {
        self.enabled && !self.paused
    }
    pub fn policy(&self) -> Policy {
        Policy {
            max_items: if self.count_limit {
                self.max_items
            } else {
                usize::MAX >> 1
            },
            retention_days: if self.period_limit {
                self.retention_days
            } else {
                0
            },
            ..Policy::default()
        }
    }
    pub fn to_text(&self) -> String {
        format!(
            "enabled={}
paused={}
count_limit={}
max_items={}
period_limit={}
retention_days={}
ignore_own_copy={}
default_on=1
",
            self.enabled,
            self.paused,
            self.count_limit,
            self.max_items,
            self.period_limit,
            self.retention_days,
            self.ignore_own_copy
        )
    }
    pub fn from_text(text: &str) -> Self {
        let mut s = Self::default();
        let mut default_on = false;
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let v = v.trim();
            match k.trim() {
                "enabled" => s.enabled = v == "true",
                "paused" => s.paused = v == "true",
                "count_limit" => s.count_limit = v != "false",
                "period_limit" => s.period_limit = v != "false",
                "max_items" => {
                    s.max_items = v
                        .parse()
                        .ok()
                        .filter(|n| Self::ITEM_CHOICES.contains(n))
                        .unwrap_or(s.max_items)
                }
                "retention_days" => {
                    s.retention_days = v
                        .parse()
                        .ok()
                        .filter(|n| Self::DAY_CHOICES.contains(n))
                        .unwrap_or(s.retention_days)
                }
                "ignore_own_copy" => s.ignore_own_copy = v != "false",
                "default_on" => default_on = true,
                _ => {}
            }
        }
        if !default_on {
            // Written before collection was on by default: turned on once.
            s.enabled = true;
        }
        s
    }
    /// True for a settings file that is turned on by `from_text` this once.
    fn needs_default_on(text: &str) -> bool {
        !text
            .lines()
            .any(|l| l.trim_start().starts_with("default_on="))
    }
}

/// `%LOCALAPPDATA%\OhMemoJang\clipboard_history`
pub fn history_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(|d| PathBuf::from(d).join("OhMemoJang"))
        .or_else(|| std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("OhMemoJang")))
        .or_else(|| {
            std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share/ohmemojang"))
        })
        .unwrap_or_else(|| PathBuf::from("ohmemojang-data"))
        .join("clipboard_history")
}

#[derive(Clone)]
pub struct Detail {
    pub id: i64,
    pub text: Option<String>,
    pub table: Option<(Vec<Vec<String>>, usize, usize)>,
}

#[derive(Clone, PartialEq)]
pub enum Confirm {
    All,
    Before(i64, String),
}

/// Everything the UI thread owns.
struct Live {
    shared: Arc<Shared>,
    tx: Sender<Cmd>,
    rx: Receiver<Event>,
}

#[derive(Default)]
pub struct ClipboardHistory {
    live: Option<Live>,
    dir: PathBuf,
    settings: Settings,
    items: Vec<ItemMeta>,
    has_more: bool,
    /// Latest list request; older answers are ignored.
    req: u64,
    loading: bool,
    filter: usize,
    search: String,
    /// The search text last sent (requests are debounced).
    sent_search: String,
    search_edited: f64,
    sel: Option<usize>,
    textures: HashMap<i64, egui::TextureHandle>,
    thumb_asked: HashSet<i64>,
    detail: Option<Detail>,
    detail_big: bool,
    full: Option<(i64, egui::TextureHandle)>,
    status: Option<(String, f64)>,
    notice: String,
    confirm: Option<Confirm>,
    date_input: String,
    stats: (i64, i64),
    needs_refresh: bool,
    scroll: (f32, f32),
    scroll_to: Option<f32>,
    settings_open: bool,
    search_focused: bool,
    /// Comments being typed, by item (saved when the box loses focus).
    comment_drafts: HashMap<i64, String>,
}

impl ClipboardHistory {
    /// Starts the three threads. Without Windows nothing is collected.
    pub fn start(ctx: &egui::Context) -> Self {
        let dir = history_dir();
        let _ = std::fs::create_dir_all(&dir);
        let text = std::fs::read_to_string(dir.join("settings.txt")).ok();
        let settings = text.as_deref().map(Settings::from_text).unwrap_or_default();
        if text.as_deref().is_some_and(Settings::needs_default_on) {
            let _ = std::fs::write(dir.join("settings.txt"), settings.to_text());
        }
        let shared = Arc::new(Shared::new());
        shared.collecting.store(settings.collecting(), Relaxed);
        shared
            .ignore_own_copy
            .store(settings.ignore_own_copy, Relaxed);
        let (tx, worker_rx) = channel::<Cmd>();
        let (ev_tx, rx) = channel::<Event>();
        worker::spawn(
            dir.clone(),
            shared.clone(),
            worker_rx,
            ev_tx.clone(),
            ctx.clone(),
            settings.clone(),
        );
        win::spawn(shared.clone(), tx.clone(), ev_tx, ctx.clone());
        if settings.collecting() {
            win::post(&shared, WinCmd::Rebase);
        }
        let mut me = Self::default();
        me.live = Some(Live { shared, tx, rx });
        me.dir = dir;
        me.settings = settings;
        me.needs_refresh = true;
        me
    }

    fn send(&self, cmd: Cmd) {
        if let Some(l) = &self.live {
            let _ = l.tx.send(cmd);
        }
    }

    fn save_settings(&self) {
        let _ = std::fs::create_dir_all(&self.dir);
        let _ = std::fs::write(self.dir.join("settings.txt"), self.settings.to_text());
    }

    /// Applies a changed on/paused state to the capture side. Content read
    /// while stopped is never taken: turning it on starts from "now".
    fn apply_collecting(&mut self, was: bool) {
        let Some(l) = &self.live else { return };
        let now = self.settings.collecting();
        l.shared.collecting.store(now, Relaxed);
        l.shared.generation.fetch_add(1, Relaxed);
        if now && !was {
            win::post(&l.shared, WinCmd::Rebase);
        }
        self.save_settings();
    }

    fn policy_changed(&self) {
        if let Some(l) = &self.live {
            l.shared
                .ignore_own_copy
                .store(self.settings.ignore_own_copy, Relaxed);
        }
        self.save_settings();
        self.send(Cmd::Policy {
            policy: self.settings.policy(),
        });
    }

    fn query(&self, after: Option<(i64, i64, i64)>) -> Query {
        Query {
            kind: ["", "text", "link", "table", "image"][self.filter.min(4)].to_owned(),
            search: self.search.clone(),
            after,
            limit: 50,
        }
    }

    pub fn refresh(&mut self) {
        self.req += 1;
        self.loading = true;
        self.needs_refresh = false;
        self.sent_search = self.search.clone();
        let q = self.query(None);
        self.send(Cmd::List {
            req: self.req,
            query: q,
        });
    }

    fn load_more(&mut self) {
        if self.loading || !self.has_more {
            return;
        }
        let Some(last) = self.items.last() else {
            return;
        };
        self.req += 1;
        self.loading = true;
        let q = self.query(Some(last.cursor()));
        self.send(Cmd::List {
            req: self.req,
            query: q,
        });
    }

    /// Called every frame: moves worker answers into the UI state.
    pub fn poll(&mut self, ctx: &egui::Context) {
        let Some(l) = &self.live else { return };
        let events: Vec<Event> = l.rx.try_iter().collect();
        for e in events {
            match e {
                Event::List {
                    req,
                    items,
                    has_more,
                    append,
                } => {
                    if req != self.req {
                        continue;
                    }
                    self.loading = false;
                    self.has_more = has_more;
                    if append {
                        self.items.extend(items);
                    } else {
                        self.items = items;
                        let keep: HashSet<i64> = self.items.iter().map(|i| i.id).collect();
                        self.textures.retain(|id, _| keep.contains(id));
                        self.thumb_asked.retain(|id| keep.contains(id));
                        if self.sel.is_some_and(|s| s >= self.items.len()) {
                            self.sel = None;
                        }
                    }
                }
                Event::Thumb { id, size, rgba } => {
                    let img = egui::ColorImage::from_rgba_unmultiplied(size, &rgba);
                    let tex =
                        ctx.load_texture(format!("clip{id}"), img, egui::TextureOptions::LINEAR);
                    self.textures.insert(id, tex);
                }
                Event::Detail { id, text, table } => {
                    if self.detail.as_ref().is_some_and(|d| d.id == id) {
                        self.detail = Some(Detail { id, text, table });
                    }
                }
                Event::Full { id, size, rgba } => {
                    if self.detail.as_ref().is_some_and(|d| d.id == id) {
                        let img = egui::ColorImage::from_rgba_unmultiplied(size, &rgba);
                        let tex = ctx.load_texture(
                            format!("clipfull{id}"),
                            img,
                            egui::TextureOptions::LINEAR,
                        );
                        self.full = Some((id, tex));
                    }
                }
                Event::Changed => self.needs_refresh = true,
                Event::Stats { count, bytes } => self.stats = (count, bytes),
                Event::Restored { outcome } => {
                    let msg = match outcome {
                        RestoreOutcome::Full => "복사됨 — 붙여넣을 곳에서 Ctrl+V".to_owned(),
                        RestoreOutcome::Partial(done) => {
                            format!("일부 형식만 복사됨 ({})", done.join(", "))
                        }
                        RestoreOutcome::NotTouched(why) => {
                            format!("복사 실패: {why} (기존 클립보드는 그대로입니다)")
                        }
                        RestoreOutcome::Cleared => {
                            "복사 실패: 기존 클립보드 내용이 지워졌을 수 있음".to_owned()
                        }
                        RestoreOutcome::Cancelled => {
                            "삭제된 항목이라 복사하지 않았습니다".to_owned()
                        }
                    };
                    self.status = Some((msg, ctx.input(|i| i.time)));
                }
                Event::Notice(n) => self.notice = n,
            }
        }
    }

    /// Stops collecting and both background threads, and waits up to `wait`
    /// for them to finish what they are writing (the worker closes its
    /// database as it ends). Used when the program quits.
    pub fn shutdown(&mut self, wait: std::time::Duration) {
        let Some(l) = self.live.take() else { return };
        l.shared.collecting.store(false, Relaxed);
        l.shared.generation.fetch_add(1, Relaxed);
        win::post(&l.shared, WinCmd::Quit);
        let shared = l.shared.clone();
        // The worker ends when every sender is gone: this one now, the
        // listener's when its thread ends.
        drop(l);
        let until = std::time::Instant::now() + wait;
        while shared.threads.load(std::sync::atomic::Ordering::SeqCst) > 0
            && std::time::Instant::now() < until
        {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    /// Called while the page is not shown: textures are released.
    pub fn idle(&mut self) {
        if !self.textures.is_empty() {
            self.textures.clear();
            self.thumb_asked.clear();
        }
        self.detail = None;
        self.full = None;
        self.needs_refresh = self.live.is_some();
    }
}

impl Drop for ClipboardHistory {
    fn drop(&mut self) {
        if let Some(l) = &self.live {
            win::post(&l.shared, WinCmd::Quit);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_roundtrip_and_bad_values() {
        let s = Settings {
            enabled: true,
            paused: true,
            count_limit: false,
            max_items: 300,
            period_limit: false,
            retention_days: 90,
            ignore_own_copy: false,
        };
        assert_eq!(Settings::from_text(&s.to_text()), s);
        let bad = Settings::from_text("max_items=7\nretention_days=x\nenabled=true\nzzz");
        assert_eq!(bad.max_items, 100);
        assert!(bad.enabled && !bad.paused && !bad.count_limit && bad.period_limit);
        assert_eq!(s.policy().retention_days, 0);
        assert!(s.policy().max_items > 1_000_000);
        assert!(Settings::default().collecting(), "on by default");
    }

    #[test]
    fn an_older_settings_file_is_turned_on_once() {
        let old = "enabled=false
paused=false
max_items=100
";
        assert!(Settings::needs_default_on(old));
        let s = Settings::from_text(old);
        assert!(s.enabled, "turned on");
        // Saved with the marker: turning it off again sticks.
        let off = Settings {
            enabled: false,
            ..s
        };
        let text = off.to_text();
        assert!(!Settings::needs_default_on(&text));
        assert!(!Settings::from_text(&text).enabled);
        // Paused stays paused.
        assert!(
            Settings::from_text(
                "enabled=false
paused=true"
            )
            .paused
        );
    }

    #[test]
    fn backlog_drops_oldest_first() {
        let shared = Shared::new();
        for seq in 0..12 {
            push_pending(
                &shared,
                Raw {
                    seq,
                    generation: 1,
                    captured_ms: 0,
                    reduced: false,
                    formats: vec![("text", vec![0; 4])],
                },
            );
        }
        let q = shared.pending();
        assert_eq!(q.len(), MAX_PENDING);
        assert_eq!(q.front().unwrap().seq, 4);
        assert_eq!(shared.counters.dropped_backlog.load(Relaxed), 4);
    }
}

/// Uses the real Windows clipboard: run alone with
/// `cargo test clipboard_history::live -- --ignored --test-threads=1`.
#[cfg(all(test, windows))]
mod live {
    #[test]
    #[ignore = "uses the Windows clipboard"]
    fn own_copy_exclusion_can_be_toggled() {
        let dir = std::env::temp_dir().join(format!("omj-own-copy-{}", std::process::id()));
        std::env::set_var("LOCALAPPDATA", &dir);
        let ctx = egui::Context::default();
        crate::clipboard_copy::install(&ctx);
        let mut h = ClipboardHistory::start(&ctx);
        let mut cb = crate::clipboard_copy::Clipboard::new().unwrap();
        h.set_state(true, false);
        std::thread::sleep(Duration::from_millis(300));
        let output = ctx.run(Default::default(), |ctx| {
            ctx.copy_text("OMJ own-copy excluded test".into())
        });
        assert!(output.platform_output.commands.is_empty());
        std::thread::sleep(Duration::from_millis(400));
        h.refresh();
        std::thread::sleep(Duration::from_millis(200));
        h.poll(&ctx);
        assert!(h.items.is_empty());
        h.settings.ignore_own_copy = false;
        h.policy_changed();
        cb.set_text("OMJ own-copy included test").unwrap();
        wait(&mut h, &ctx, "own copy enabled", |h| h.items.len() == 1);
        h.settings.ignore_own_copy = true;
        h.policy_changed();
        cb.set_text("OMJ own-copy excluded again").unwrap();
        std::thread::sleep(Duration::from_millis(400));
        h.refresh();
        std::thread::sleep(Duration::from_millis(200));
        h.poll(&ctx);
        assert_eq!(h.items.len(), 1);
        h.set_state(false, false);
    }
    use super::*;
    use std::time::{Duration, Instant};

    fn wait(
        h: &mut ClipboardHistory,
        ctx: &egui::Context,
        what: &str,
        mut ok: impl FnMut(&ClipboardHistory) -> bool,
    ) {
        let t = Instant::now();
        while t.elapsed() < Duration::from_secs(5) {
            h.refresh();
            std::thread::sleep(Duration::from_millis(150));
            h.poll(ctx);
            if ok(h) {
                return;
            }
        }
        panic!("timeout: {what}");
    }

    #[test]
    #[ignore]
    fn capture_restore_and_no_recapture() {
        let dir = std::env::temp_dir().join(format!("omj-clip-{}", std::process::id()));
        std::env::set_var("LOCALAPPDATA", &dir);
        let ctx = egui::Context::default();
        let mut h = ClipboardHistory::start(&ctx);
        h.settings.ignore_own_copy = false;
        h.policy_changed();
        let mut cb = arboard::Clipboard::new().unwrap();
        cb.set_text("시작 전 내용").unwrap();
        std::thread::sleep(Duration::from_millis(300));
        h.set_state(true, false);
        std::thread::sleep(Duration::from_millis(300));
        h.poll(&ctx);
        h.refresh();
        std::thread::sleep(Duration::from_millis(200));
        h.poll(&ctx);
        assert!(
            h.items.is_empty(),
            "content present before enabling must not be collected"
        );

        cb.set_text("한글 😀 첫째\n둘째 줄").unwrap();
        wait(&mut h, &ctx, "first capture", |h| h.items.len() == 1);
        assert_eq!(h.items[0].kind, "text");

        let id = h.items[0].id;
        cb.set_text("덮어쓴 값").unwrap();
        wait(&mut h, &ctx, "second capture", |h| h.items.len() == 2);
        assert_eq!(h.items[0].summary, "덮어쓴 값");

        h.send(Cmd::Restore(id));
        let t = Instant::now();
        while h.status.is_none() && t.elapsed() < Duration::from_secs(3) {
            std::thread::sleep(Duration::from_millis(50));
            h.poll(&ctx);
        }
        assert!(
            h.status.as_ref().unwrap().0.starts_with("복사됨"),
            "{:?}",
            h.status
        );
        assert_eq!(cb.get_text().unwrap(), "한글 😀 첫째\n둘째 줄");
        std::thread::sleep(Duration::from_millis(600));
        h.refresh();
        std::thread::sleep(Duration::from_millis(200));
        h.poll(&ctx);
        assert_eq!(h.items.len(), 2, "restore must not be collected again");
        assert_eq!(h.items[0].summary, "덮어쓴 값");

        // a later external copy of the same text is collected
        cb.set_text("한글 😀 첫째\n둘째 줄").unwrap();
        wait(&mut h, &ctx, "external re-copy", |h| h.items.len() == 3);

        // paused: nothing is read
        h.set_state(true, true);
        std::thread::sleep(Duration::from_millis(200));
        cb.set_text("일시 중지 중").unwrap();
        std::thread::sleep(Duration::from_millis(600));
        h.refresh();
        std::thread::sleep(Duration::from_millis(200));
        h.poll(&ctx);
        assert_eq!(h.items.len(), 3);
        let _ = std::fs::remove_dir_all(dir);
    }
    /// What the user saw: after "전체 삭제" nothing new accumulated. Copy text,
    /// an Excel-like HTML table and an image, deleting everything in between.
    #[test]
    #[ignore]
    fn copies_after_delete_all_are_collected_again() {
        let dir = std::env::temp_dir().join(format!("omj-clip2-{}", std::process::id()));
        std::env::set_var("LOCALAPPDATA", &dir);
        let ctx = egui::Context::default();
        let mut h = ClipboardHistory::start(&ctx);
        h.settings.ignore_own_copy = false;
        h.policy_changed();
        let mut cb = arboard::Clipboard::new().unwrap();
        h.set_state(true, false);
        std::thread::sleep(Duration::from_millis(300));
        cb.set_text("첫 복사").unwrap();
        wait(&mut h, &ctx, "first", |h| h.items.len() == 1);
        // delete everything the way the page does
        let watermark = h
            .live
            .as_ref()
            .unwrap()
            .shared
            .seq
            .load(std::sync::atomic::Ordering::Relaxed);
        h.send(Cmd::DeleteAll { watermark });
        std::thread::sleep(Duration::from_millis(400));
        h.refresh();
        std::thread::sleep(Duration::from_millis(200));
        h.poll(&ctx);
        assert!(h.items.is_empty());
        // text again
        cb.set_text("삭제 뒤 복사").unwrap();
        wait(&mut h, &ctx, "text after delete", |h| h.items.len() == 1);
        // an Excel-like range (HTML + text)
        cb.set_html(
            "<table><tr><td>가</td><td>나</td></tr><tr><td>다</td><td>라</td></tr></table>",
            Some("가\t나\r\n다\t라\r\n"),
        )
        .unwrap();
        wait(&mut h, &ctx, "table after delete", |h| h.items.len() == 2);
        assert_eq!(h.items[0].kind, "table");
        // an image
        cb.set_image(arboard::ImageData {
            width: 4,
            height: 4,
            bytes: std::borrow::Cow::Owned(vec![200u8; 64]),
        })
        .unwrap();
        wait(&mut h, &ctx, "image after delete", |h| h.items.len() == 3);
        assert_eq!(h.items[0].kind, "image");
        let _ = std::fs::remove_dir_all(dir);
    }
    /// Waits for copies made by other programs (run by hand, then copy).
    #[test]
    #[ignore]
    fn wait_for_external_copies() {
        let dir = std::env::temp_dir().join(format!("omj-clip3-{}", std::process::id()));
        std::env::set_var("LOCALAPPDATA", &dir);
        let ctx = egui::Context::default();
        let mut h = ClipboardHistory::start(&ctx);
        h.settings.ignore_own_copy = false;
        h.policy_changed();
        h.set_state(true, false);
        std::thread::sleep(Duration::from_millis(500));
        println!("READY");
        let t = Instant::now();
        let mut seen = 0;
        while t.elapsed() < Duration::from_secs(75) {
            h.refresh();
            std::thread::sleep(Duration::from_millis(400));
            h.poll(&ctx);
            if h.items.len() != seen {
                seen = h.items.len();
                for m in &h.items {
                    println!(
                        "ITEM {} {} {:?} {} formats={} reduced={}",
                        m.id, m.kind, m.summary, m.bytes, m.formats, m.reduced
                    );
                }
                println!("---");
            }
        }
        let c = &h.live.as_ref().unwrap().shared.counters;
        use std::sync::atomic::Ordering::Relaxed;
        println!(
            "COUNTERS locked={} oversize={} unsupported={} backlog={} store_err={}",
            c.missed_locked.load(Relaxed),
            c.oversize.load(Relaxed),
            c.unsupported.load(Relaxed),
            c.dropped_backlog.load(Relaxed),
            c.store_errors.load(Relaxed)
        );
        println!("NOTICE {:?}", h.notice);
        let _ = std::fs::remove_dir_all(dir);
    }
    /// A screen-sized picture (what a screenshot puts on the clipboard).
    #[test]
    #[ignore]
    fn a_screen_sized_image_is_collected() {
        let dir = std::env::temp_dir().join(format!("omj-clip4-{}", std::process::id()));
        std::env::set_var("LOCALAPPDATA", &dir);
        let ctx = egui::Context::default();
        let mut h = ClipboardHistory::start(&ctx);
        h.settings.ignore_own_copy = false;
        h.policy_changed();
        let mut cb = arboard::Clipboard::new().unwrap();
        h.set_state(true, false);
        std::thread::sleep(Duration::from_millis(400));
        let (w, hgt) = (2880usize, 1800usize);
        let bytes: Vec<u8> = (0..w * hgt * 4).map(|i| ((i / 7) % 251) as u8).collect();
        let t = Instant::now();
        cb.set_image(arboard::ImageData {
            width: w,
            height: hgt,
            bytes: std::borrow::Cow::Owned(bytes),
        })
        .unwrap();
        wait(&mut h, &ctx, "big image", |h| !h.items.is_empty());
        println!("BIG IMAGE stored in {:?}: {:?}", t.elapsed(), h.items[0]);
        let _ = std::fs::remove_dir_all(dir);
    }
    /// The page itself (no manual refresh): the list follows new copies, also after "전체 삭제".
    #[test]
    #[ignore]
    fn the_page_list_follows_new_copies_after_delete_all() {
        let dir = std::env::temp_dir().join(format!("omj-clip5-{}", std::process::id()));
        std::env::set_var("LOCALAPPDATA", &dir);
        let ctx = egui::Context::default();
        let mut h = ClipboardHistory::start(&ctx);
        h.settings.ignore_own_copy = false;
        h.policy_changed();
        let mut cb = arboard::Clipboard::new().unwrap();
        h.set_state(true, false);
        let mut t = 0.0;
        let mut frame = |h: &mut ClipboardHistory| {
            t += 0.05;
            h.poll(&ctx);
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(900., 700.),
                )),
                time: Some(t),
                ..Default::default()
            };
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| h.ui(ui));
            });
            std::thread::sleep(Duration::from_millis(30));
        };
        let wait_len =
            |h: &mut ClipboardHistory, frame: &mut dyn FnMut(&mut ClipboardHistory), n: usize| {
                let t0 = Instant::now();
                while h.items.len() != n && t0.elapsed() < Duration::from_secs(5) {
                    frame(h);
                }
                assert_eq!(h.items.len(), n, "list length");
            };
        for _ in 0..10 {
            frame(&mut h);
        }
        cb.set_text("하나").unwrap();
        wait_len(&mut h, &mut frame, 1);
        cb.set_text("둘").unwrap();
        wait_len(&mut h, &mut frame, 2);
        // the delete-all of the page: confirm, then the click on 삭제
        h.confirm = Some(Confirm::All);
        h.send(Cmd::DeleteAll {
            watermark: h
                .live
                .as_ref()
                .unwrap()
                .shared
                .seq
                .load(std::sync::atomic::Ordering::Relaxed),
        });
        h.confirm = None;
        wait_len(&mut h, &mut frame, 0);
        cb.set_text("셋").unwrap();
        wait_len(&mut h, &mut frame, 1);
        cb.set_html(
            "<table><tr><td>가</td><td>나</td></tr></table>",
            Some("가\t나\r\n"),
        )
        .unwrap();
        wait_len(&mut h, &mut frame, 2);
        let _ = std::fs::remove_dir_all(dir);
    }
}
