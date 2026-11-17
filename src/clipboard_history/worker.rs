//! The storage worker: validates raw captures, builds previews, owns the
//! SQLite connection and applies retention. Deletes and stores are handled in
//! one serial loop, so a delete can never be overtaken by a late insert.
use super::formats::*;
use super::store::{FormatRow, InsertError, ItemMeta, NewItem, Query, Store};
use super::{Cmd, Event, Policy, Raw, RestoreOutcome, Settings, Shared, WinCmd};
use eframe::egui;
use std::path::PathBuf;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::time::Duration;

pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

pub fn spawn(
    dir: PathBuf,
    shared: Arc<Shared>,
    rx: Receiver<Cmd>,
    tx: Sender<Event>,
    ctx: egui::Context,
    settings: Settings,
) {
    let _ = std::thread::Builder::new()
        .name("clipboard-store".into())
        .spawn(move || {
            let mut w = Worker::open(dir, shared, tx, ctx, settings);
            w.run(rx);
        });
}

struct Worker {
    store: Option<Store>,
    shared: Arc<Shared>,
    tx: Sender<Event>,
    ctx: egui::Context,
    policy: Policy,
    last_tidy: Option<chrono::NaiveDate>,
    tidy_path: PathBuf,
    /// Captures numbered up to this were made before the last delete-all.
    watermark: u64,
}

impl Worker {
    fn open(
        dir: PathBuf,
        shared: Arc<Shared>,
        tx: Sender<Event>,
        ctx: egui::Context,
        settings: Settings,
    ) -> Self {
        let store = match Store::open(&dir.join("clipboard.db")) {
            Ok((s, note)) => {
                if let Some(n) = note {
                    let _ = tx.send(Event::Notice(n));
                }
                Some(s)
            }
            Err(e) => {
                let _ = tx.send(Event::Notice(format!(
                    "클립보드 기록 저장소를 열 수 없습니다: {e}"
                )));
                None
            }
        };
        let mut w = Self {
            store,
            shared,
            tx,
            ctx,
            policy: settings.policy(),
            last_tidy: std::fs::read_to_string(dir.join("last-tidy.txt"))
                .ok()
                .and_then(|s| s.trim().parse().ok()),
            tidy_path: dir.join("last-tidy.txt"),
            watermark: 0,
        };
        w.tidy_if_due();
        w
    }

    fn emit(&self, e: Event) {
        let _ = self.tx.send(e);
        self.ctx.request_repaint();
    }

    fn tidy(&mut self) {
        let now = now_ms();
        let slot = noon_slot(chrono::Local::now().naive_local());
        let policy = self.policy;
        let mut changed = false;
        if let Some(s) = &mut self.store {
            match s.apply_retention(&policy, now) {
                Ok(n) => {
                    changed = n > 0;
                    self.last_tidy = Some(slot);
                    let _ = std::fs::write(&self.tidy_path, slot.to_string());
                }
                Err(_) => {
                    self.shared.counters.store_errors.fetch_add(1, Relaxed);
                }
            }
        }
        self.stats();
        if changed {
            self.emit(Event::Changed);
        }
    }

    fn tidy_if_due(&mut self) {
        let slot = noon_slot(chrono::Local::now().naive_local());
        if self.last_tidy.is_none_or(|last| last < slot) {
            self.tidy();
        }
    }
    fn stats(&self) {
        if let Some(s) = &self.store {
            if let (Ok(count), Ok(bytes)) = (s.count(), s.total_bytes()) {
                self.emit(Event::Stats { count, bytes });
            }
        }
    }

    fn run(&mut self, rx: Receiver<Cmd>) {
        loop {
            self.tidy_if_due();
            let now = chrono::Local::now().naive_local();
            let next_noon = (noon_slot(now) + chrono::Duration::days(1))
                .and_hms_opt(12, 0, 0)
                .unwrap();
            let wait_ms = (next_noon - now).num_milliseconds().clamp(1, 60_000) as u64;
            match rx.recv_timeout(Duration::from_millis(wait_ms)) {
                // One bad item or request must not end the worker: nothing
                // would be collected after it.
                Ok(cmd) => {
                    let caught =
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.handle(cmd)));
                    if caught.is_err() {
                        self.shared.counters.store_errors.fetch_add(1, Relaxed);
                        let _ = self.tx.send(Event::Notice(
                            "클립보드 기록 처리 중 오류가 있었습니다. 수집은 계속됩니다".into(),
                        ));
                    }
                }
                Err(RecvTimeoutError::Timeout) => {
                    self.tidy_if_due();
                }
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
    }

    fn handle(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::CaptureReady => self.drain(),
            Cmd::List { req, query } => self.list(req, query),
            Cmd::Thumb(id) => self.thumb(id),
            Cmd::Detail(id) => self.detail(id),
            Cmd::Full(id) => self.full(id),
            Cmd::Restore(id) => self.restore(id),
            Cmd::Delete(id) => {
                self.shared.epoch.fetch_add(1, Relaxed);
                if let Some(s) = &mut self.store {
                    let _ = s.delete(id);
                }
                self.after_delete();
            }
            Cmd::DeleteBefore(ms) => {
                self.shared.epoch.fetch_add(1, Relaxed);
                // Captures still waiting that match the range go too.
                self.shared.pending().retain(|r| r.captured_ms >= ms);
                if let Some(s) = &mut self.store {
                    let _ = s.delete_before(ms);
                }
                self.after_delete();
            }
            Cmd::DeleteAll { watermark } => {
                self.shared.epoch.fetch_add(1, Relaxed);
                self.watermark = self.watermark.max(watermark);
                self.shared.pending().clear();
                if let Some(s) = &mut self.store {
                    let _ = s.delete_all();
                }
                self.after_delete();
            }
            Cmd::Policy { policy } => {
                self.policy = policy;
                self.tidy();
                self.emit(Event::Changed);
            }
        }
    }

    fn after_delete(&mut self) {
        self.stats();
        self.emit(Event::Changed);
    }

    fn drain(&mut self) {
        let mut stored = false;
        loop {
            let raw = self.shared.pending().pop_front();
            let Some(raw) = raw else { break };
            stored |= self.store_one(raw);
        }
        if stored {
            self.stats();
            self.emit(Event::Changed);
        }
    }

    fn store_one(&mut self, raw: Raw) -> bool {
        // Stale: collection was stopped or restarted, or a delete-all came
        // after this capture was read.
        if raw.generation != self.shared.generation.load(Relaxed)
            || !self.shared.collecting.load(Relaxed)
            || raw.seq <= self.watermark
        {
            return false;
        }
        let Some(item) = build_item(raw) else {
            self.shared.counters.unsupported.fetch_add(1, Relaxed);
            return false;
        };
        let Some(store) = &mut self.store else {
            return false;
        };
        match store.insert(
            &item,
            &Policy {
                retention_days: 0,
                ..self.policy
            },
            now_ms(),
        ) {
            Ok(_) => true,
            Err(InsertError::TooLarge) => {
                self.shared.counters.oversize.fetch_add(1, Relaxed);
                false
            }
            Err(InsertError::Db(e)) => {
                self.shared.counters.store_errors.fetch_add(1, Relaxed);
                // The message names the failure, never the content.
                let _ = self.tx.send(Event::Notice(format!(
                    "저장하지 못했습니다: {e} · 원인을 해결하면 다시 저장됩니다"
                )));
                false
            }
        }
    }

    fn list(&mut self, req: u64, mut query: Query) {
        let want = query.limit;
        query.limit = want + 1;
        let append = query.after.is_some();
        let mut items = match &self.store {
            Some(s) => s.list(&query).unwrap_or_default(),
            None => vec![],
        };
        let has_more = items.len() > want;
        items.truncate(want);
        self.emit(Event::List {
            req,
            items: items as Vec<ItemMeta>,
            has_more,
            append,
        });
    }

    fn thumb(&mut self, id: i64) {
        let Some(s) = &self.store else { return };
        if let Ok(Some(png)) = s.preview(id, "thumb") {
            if let Some((w, h, rgba)) = png_to_rgba(&png) {
                self.emit(Event::Thumb {
                    id,
                    size: [w as usize, h as usize],
                    rgba,
                });
            }
        }
    }

    fn detail(&mut self, id: i64) {
        let Some(s) = &self.store else { return };
        let text_full = s
            .format(id, K_TEXT)
            .ok()
            .flatten()
            .and_then(|b| utf16le_to_string(&b));
        // A larger table is read from the stored HTML (or cell text) now,
        // not kept in the database.
        let from_html =
            s.format(id, K_HTML)
                .ok()
                .flatten()
                .and_then(|b| validate_html(&b))
                .and_then(|h| {
                    html_table_with(&h.fragment, DETAIL_ROWS, DETAIL_COLS, DETAIL_CELL_CHARS)
                        .or_else(|| {
                            h.document.as_deref().and_then(|d| {
                                html_table_with(d, DETAIL_ROWS, DETAIL_COLS, DETAIL_CELL_CHARS)
                            })
                        })
                });
        let table = from_html
            .or_else(|| {
                text_full.as_deref().filter(|t| looks_like_tsv(t)).map(|t| {
                    trim_grid_with(parse_tsv(t), DETAIL_ROWS, DETAIL_COLS, DETAIL_CELL_CHARS)
                })
            })
            .or_else(|| {
                s.preview(id, "table")
                    .ok()
                    .flatten()
                    .and_then(|b| serde_json::from_slice::<Vec<Vec<String>>>(&b).ok())
                    .map(|g| {
                        let (r, c) = (g.len(), g.iter().map(Vec::len).max().unwrap_or(0));
                        (g, r, c)
                    })
            });
        let text = text_full.map(|t| t.chars().take(20_000).collect());
        self.emit(Event::Detail { id, text, table });
    }

    /// Decodes the stored image once, scaled to the preview size.
    fn full(&mut self, id: i64) {
        let Some(s) = &self.store else { return };
        let decoded = s
            .format(id, K_PNG)
            .ok()
            .flatten()
            .and_then(|b| png_to_rgba(&b))
            .or_else(|| {
                [K_DIBV5, K_DIB].into_iter().find_map(|k| {
                    let b = s.format(id, k).ok().flatten()?;
                    let d = validate_dib(&b)?;
                    Some((d.width, d.height, dib_to_rgba(&b, &d)))
                })
            });
        if let Some((w, h, px)) = decoded {
            if let Some((w, h, px)) = fit_rgba(w, h, px, FULL_PREVIEW_SIDE) {
                self.emit(Event::Full {
                    id,
                    size: [w as usize, h as usize],
                    rgba: px,
                });
            }
        }
    }

    fn restore(&mut self, id: i64) {
        // Read now; the clipboard thread checks the epoch again right
        // before it publishes anything.
        let epoch = self.shared.epoch.load(Relaxed);
        match self.store.as_ref().map(|s| s.formats(id)) {
            Some(Ok(f)) if !f.is_empty() => {
                super::win::post(&self.shared, WinCmd::Restore { formats: f, epoch })
            }
            _ => self.emit(Event::Restored {
                outcome: RestoreOutcome::NotTouched("항목을 읽을 수 없습니다".into()),
            }),
        }
    }
}

/// Validates a raw capture and builds the item to store. `None` when no
/// supported, valid format is left.
pub fn build_item(raw: Raw) -> Option<NewItem> {
    let mut reduced = raw.reduced;
    let mut text: Option<String> = None;
    let mut html: Option<Html> = None;
    let mut url: Option<String> = None;
    let mut png: Option<(Vec<u8>, u32, u32)> = None;
    let mut dib: Option<(&'static str, Vec<u8>, Dib)> = None;
    for (key, data) in raw.formats {
        match key {
            K_TEXT => match utf16le_to_string(&data) {
                Some(s) if s.is_empty() => {}
                Some(s) => text = Some(s),
                None => reduced = true,
            },
            K_HTML => match validate_html(&data) {
                Some(h) => html = Some(h),
                None => reduced = true,
            },
            K_URL => url = utf16le_to_string(&data).filter(|u| is_link(u)),
            K_PNG => match validate_png(&data) {
                Some((w, h)) => png = Some((data, w, h)),
                None => reduced = true,
            },
            K_DIBV5 | K_DIB => match validate_dib(&data) {
                Some(d) => {
                    if dib.is_none() || key == K_DIBV5 {
                        dib = Some((key, data, d));
                    }
                }
                None => reduced = true,
            },
            _ => {}
        }
    }

    // Table: HTML table, else text that looks like a copied cell range.
    let table = html
        .as_ref()
        .and_then(|h| {
            html_table(&h.fragment).or_else(|| h.document.as_deref().and_then(html_table))
        })
        .or_else(|| {
            text.as_deref()
                .filter(|t| looks_like_tsv(t))
                .map(|t| trim_grid(parse_tsv(t)))
        });
    let is_table = table.is_some();

    // Fallback text for HTML-only and URL-only copies.
    let mut derived_text = false;
    if text.is_none() {
        if let Some(h) = &html {
            let t = html_to_text(&h.fragment);
            if !t.is_empty() {
                text = Some(t);
                derived_text = true;
            }
        }
    }
    if text.is_none() {
        if let Some(u) = &url {
            text = Some(u.clone());
            derived_text = true;
        }
    }

    // Images are not kept for table copies (Excel offers a picture too).
    let image = !is_table && (png.is_some() || dib.is_some());
    let mut formats: Vec<FormatRow> = vec![];
    let mut previews: Vec<(&'static str, Vec<u8>)> = vec![];
    let mut dims = (0u32, 0u32);
    if let Some(h) = &html {
        formats.push(FormatRow {
            key: K_HTML.into(),
            derived: false,
            data: h.bytes.clone(),
        });
    }
    if image {
        let mut rgba: Option<(u32, u32, Vec<u8>)> = None;
        if let Some((_, w, h)) = &png {
            dims = (*w, *h);
        }
        if let Some((_, data, d)) = &dib {
            dims = (d.width, d.height);
            rgba = Some((d.width, d.height, dib_to_rgba(data, d)));
        }
        if let Some((data, _, _)) = &png {
            formats.push(FormatRow {
                key: K_PNG.into(),
                derived: false,
                data: data.clone(),
            });
            if dib.is_none() {
                if let Some((w, h, px)) = png_to_rgba(data) {
                    formats.push(FormatRow {
                        key: K_DIBV5.into(),
                        derived: true,
                        data: rgba_to_dibv5(w, h, &px),
                    });
                    rgba = Some((w, h, px));
                } else {
                    reduced = true;
                }
            }
        }
        if let Some((key, data, _)) = &dib {
            formats.push(FormatRow {
                key: (*key).into(),
                derived: false,
                data: data.clone(),
            });
        }
        if let Some((w, h, px)) = rgba {
            if let Some(t) = thumbnail_png(w, h, px) {
                previews.push(("thumb", t));
            }
        }
    }
    if let Some(u) = &url {
        formats.push(FormatRow {
            key: K_URL.into(),
            derived: false,
            data: string_to_utf16le(u),
        });
    }
    if let Some(t) = &text {
        formats.push(FormatRow {
            key: K_TEXT.into(),
            derived: derived_text,
            data: string_to_utf16le(t),
        });
    }
    if formats.is_empty() {
        return None;
    }

    let (kind, summary, meta) = if let Some((grid, r, c)) = &table {
        let first = grid.first().map(|row| row.join(" | ")).unwrap_or_default();
        ("table", summarize(&first, 120), format!("{r}×{c}"))
    } else if image {
        (
            "image",
            format!("이미지 {}×{}", dims.0, dims.1),
            format!("{}×{}", dims.0, dims.1),
        )
    } else if text.as_deref().is_some_and(is_link) || url.is_some() {
        let u = url.clone().or(text.clone()).unwrap_or_default();
        ("link", summarize(&u, 160), String::new())
    } else {
        let t = text.as_deref().unwrap_or("");
        (
            "text",
            summarize(t, 120),
            format!("{}자", t.chars().count()),
        )
    };
    if let Some((grid, _, _)) = &table {
        if let Ok(j) = serde_json::to_vec(grid) {
            previews.push(("table", j));
        }
    }
    let search_text = match (&text, kind) {
        (Some(t), _) => t.chars().take(2000).collect(),
        _ => summary.clone(),
    };
    Some(NewItem {
        captured_ms: raw.captured_ms,
        kind,
        summary,
        search_text,
        meta,
        reduced,
        formats,
        previews,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(formats: Vec<(&'static str, Vec<u8>)>) -> Raw {
        Raw {
            seq: 1,
            generation: 1,
            captured_ms: 5,
            reduced: false,
            formats,
        }
    }
    fn u16s(s: &str) -> Vec<u8> {
        let mut v = string_to_utf16le(s);
        v.extend([0, 0]);
        v
    }

    #[test]
    fn plain_text_and_link() {
        let it = build_item(raw(vec![(K_TEXT, u16s("안녕 😀\n둘째 줄"))])).unwrap();
        assert_eq!(it.kind, "text");
        assert_eq!(it.summary, "안녕 😀");
        let it = build_item(raw(vec![(K_TEXT, u16s("https://example.com/x"))])).unwrap();
        assert_eq!(it.kind, "link");
    }

    #[test]
    fn tsv_text_is_a_table_and_keeps_raw_text() {
        let it = build_item(raw(vec![(K_TEXT, u16s("a\tb\r\n1\t2\r\n"))])).unwrap();
        assert_eq!(it.kind, "table");
        assert_eq!(it.meta, "2×2");
        let stored =
            utf16le_to_string(&it.formats.iter().find(|f| f.key == "text").unwrap().data).unwrap();
        assert_eq!(stored, "a\tb\r\n1\t2\r\n");
    }

    #[test]
    fn invalid_formats_are_dropped_and_flagged() {
        let it = build_item(raw(vec![(K_TEXT, u16s("ok")), (K_DIB, vec![1, 2, 3])])).unwrap();
        assert!(it.reduced);
        assert_eq!(it.formats.len(), 1);
        assert!(build_item(raw(vec![(K_DIB, vec![1, 2, 3])])).is_none());
    }

    #[test]
    fn png_only_gets_a_derived_dib() {
        let png = thumbnail_png(2, 1, vec![1, 2, 3, 255, 4, 5, 6, 255]).unwrap();
        let it = build_item(raw(vec![(K_PNG, png)])).unwrap();
        assert_eq!(it.kind, "image");
        assert!(it.formats.iter().any(|f| f.key == "dibv5" && f.derived));
        assert_eq!(it.previews[0].0, "thumb");
    }
}

/// The most recent local noon, including the previous day before noon.
fn noon_slot(now: chrono::NaiveDateTime) -> chrono::NaiveDate {
    use chrono::Timelike;
    now.date() - chrono::Duration::days(i64::from(now.hour() < 12))
}

#[cfg(test)]
mod noon_tests {
    use super::*;
    #[test]
    fn noon_rollover_and_startup_catchup() {
        let day = chrono::NaiveDate::from_ymd_opt(2026, 11, 17).unwrap();
        assert_eq!(
            noon_slot(day.and_hms_opt(11, 59, 59).unwrap()),
            day.pred_opt().unwrap()
        );
        assert_eq!(noon_slot(day.and_hms_opt(12, 0, 0).unwrap()), day);
        assert_eq!(noon_slot(day.and_hms_opt(23, 59, 59).unwrap()), day);
    }
}
