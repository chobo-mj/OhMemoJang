//! 클립보이: a small window of the newest copies, opened with a global
//! shortcut (Alt+V) beside the place the user is typing in. A click on an
//! item pastes it there at once.
//!
//! The window never takes the focus, so the program the user types in keeps
//! its cursor: the paste is a Ctrl+V sent to it. Without its pin, a click or
//! a key press anywhere else closes it.
use super::{ClipboardHistory, Cmd, ItemMeta, RestoreOutcome};
use crate::icons::Icon;
use crate::ui;
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use std::collections::{HashMap, HashSet};

pub use native::note_hotkey;

/// How many of the newest items it lists.
pub const MAX_ITEMS: usize = 100;
/// The size of Windows' own clipboard window (Win+V), in points.
pub const SIZE: Vec2 = Vec2::new(360., 400.);
const ROW_H: f32 = 64.;
const THUMB: f32 = 48.;

enum Act {
    Paste(i64),
    Copy(i64),
    Pin(i64, bool),
    Delete(i64),
    Clear,
    Keep,
}

#[derive(Default)]
pub struct Clipboy {
    open: bool,
    /// The pin: stays open when the user clicks or types elsewhere.
    keep: bool,
    req: u64,
    items: Vec<ItemMeta>,
    loaded: bool,
    /// The stored items changed: list them again.
    pub(super) stale: bool,
    textures: HashMap<i64, egui::TextureHandle>,
    thumbs_asked: HashSet<i64>,
    grids: HashMap<i64, Vec<Vec<String>>>,
    grids_asked: HashSet<i64>,
    /// The item being put on the clipboard is to be pasted.
    paste: bool,
    note: Option<(String, f64)>,
    /// When a key press closed it: the shortcut pressed again right then
    /// must not open it again.
    closed_by_key: Option<f64>,
    anchor: Option<native::Anchor>,
    placed: bool,
    watch: Option<native::Watch>,
    /// The shortcut that opens it.
    key: Option<crate::keys::Combo>,
    /// Turned on or off on the page: the shortcut is registered again.
    switched: bool,
}

impl Clipboy {
    fn close(&mut self) {
        self.open = false;
        self.watch = None;
        self.placed = false;
        self.anchor = None;
        self.paste = false;
        self.note = None;
        self.textures.clear();
        self.thumbs_asked.clear();
        self.grids.clear();
        self.grids_asked.clear();
    }

    /// The shortcut: open it at `anchor`, or close it.
    fn toggle(&mut self, now: f64, anchor: Option<native::Anchor>) {
        if self.open {
            self.close();
            return;
        }
        if self.closed_by_key.take().is_some_and(|t| now - t < 0.5) {
            return;
        }
        self.open = true;
        self.anchor = anchor;
        self.loaded = false;
        self.stale = true;
    }

    /// 전체비우기 as the list shows it until the worker's answer.
    fn cleared(&mut self) {
        self.items
            .retain(|m| m.pinned || !m.comment.trim().is_empty());
    }

    pub(super) fn listed(&mut self, req: u64, items: Vec<ItemMeta>) {
        if req != self.req {
            return;
        }
        let keep: HashSet<i64> = items.iter().map(|m| m.id).collect();
        self.textures.retain(|id, _| keep.contains(id));
        self.thumbs_asked.retain(|id| keep.contains(id));
        self.grids.retain(|id, _| keep.contains(id));
        self.grids_asked.retain(|id| keep.contains(id));
        self.items = items;
        self.loaded = true;
    }

    pub(super) fn thumb(&mut self, id: i64, tex: &egui::TextureHandle) {
        if self.open && self.thumbs_asked.contains(&id) {
            self.textures.insert(id, tex.clone());
        }
    }

    pub(super) fn grid(&mut self, id: i64, cells: Vec<Vec<String>>) {
        if self.open && self.grids_asked.contains(&id) {
            self.grids.insert(id, cells);
        }
    }

    /// An item was put on the clipboard (by 클립보이 or the page).
    pub(super) fn restored(&mut self, outcome: &RestoreOutcome, now: f64) {
        if !self.open {
            return;
        }
        let ok = matches!(outcome, RestoreOutcome::Full | RestoreOutcome::Partial(_));
        if std::mem::take(&mut self.paste) && ok {
            native::paste();
            if !self.keep {
                self.close();
                return;
            }
            self.note = Some(("붙여 넣었어요".into(), now));
        } else if ok {
            self.note = Some(("복사했어요 · Ctrl+V로 붙여 넣으세요".into(), now));
        } else {
            self.note = Some((
                "클립보드에 넣지 못했어요 · 잠시 뒤 다시 눌러 주세요".into(),
                now,
            ));
        }
    }
}

impl ClipboardHistory {
    /// 클립보이 is on (its shortcut is registered).
    pub fn clipboy_enabled(&self) -> bool {
        self.settings.clipboy && self.live.is_some()
    }

    pub fn set_clipboy_key(&mut self, key: Option<crate::keys::Combo>) {
        self.boy.key = key;
    }

    /// Turned on or off on the page since the last call.
    pub fn take_clipboy_switched(&mut self) -> bool {
        std::mem::take(&mut self.boy.switched)
    }

    /// Stores a practice item (the tutorial's) as if it had been copied.
    pub fn clipboy_sample(&self, text: &str) {
        self.send(Cmd::Sample(text.to_owned()));
    }

    /// Its shortcut was pressed: open it where the user types, or close it.
    pub fn toggle_clipboy(&mut self, now: f64) {
        let anchor = native::take_anchor();
        if self.clipboy_enabled() {
            self.boy.toggle(now, anchor);
        }
    }

    /// The page's on/off button for 클립보이.
    pub(super) fn clipboy_switch(&mut self, ui: &mut egui::Ui) {
        let mut on = self.settings.clipboy;
        let key = self
            .boy
            .key
            .as_ref()
            .map_or("Alt+V".to_owned(), |k| k.to_string());
        let label = if on {
            "클립보이 켜짐"
        } else {
            "클립보이 꺼짐"
        };
        if ui
            .toggle_value(&mut on, label)
            .on_hover_text(format!(
                "켜면 {key}로 최근 복사 {MAX_ITEMS}개를 커서 옆에 띄워 바로 붙여 넣어요 · 끄면 {key}를 다른 프로그램이 쓸 수 있어요"
            ))
            .changed()
        {
            self.settings.clipboy = on;
            self.save_settings();
            self.boy.switched = true;
            if !on {
                self.boy.close();
            }
        }
    }

    /// Draws 클립보이 while it is open. `look` applies the program's theme
    /// to the window and gives its colors.
    pub fn clipboy_window(
        &mut self,
        ctx: &egui::Context,
        look: &dyn Fn(&egui::Context) -> ui::Tone,
    ) {
        if !self.boy.open {
            return;
        }
        let now = ctx.input(|i| i.time);
        if let Some((outside, typed)) = self.boy.watch.as_ref().map(native::Watch::take) {
            if (outside || typed) && !self.boy.keep {
                if typed {
                    self.boy.closed_by_key = Some(now);
                }
                self.boy.close();
                return;
            }
        }
        if std::mem::take(&mut self.boy.stale) {
            self.boy.req += 1;
            self.send(Cmd::ListClipboy { req: self.boy.req });
        }
        let place = native::rect_for(self.boy.anchor, SIZE);
        let mut builder = egui::ViewportBuilder::default()
            .with_title("클립보이")
            .with_inner_size(SIZE)
            .with_decorations(false)
            .with_resizable(false)
            .with_taskbar(false)
            .with_active(false)
            .with_window_level(egui::WindowLevel::AlwaysOnTop);
        if let Some((r, scale)) = place {
            builder = builder.with_position(Pos2::new(r[0] as f32 / scale, r[1] as f32 / scale));
        }
        let collecting = self.settings.collecting();
        let mut acts = vec![];
        let mut want_thumbs = vec![];
        let mut want_grids = vec![];
        let boy = &mut self.boy;
        ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of("clipboy"),
            builder,
            |ctx, _| {
                let tone = look(ctx);
                crate::win_style::shape(ctx, 1.);
                if !boy.placed {
                    if let Some(h) = crate::win_style::hwnd(ctx) {
                        native::style(h);
                        if let Some((r, _)) = place {
                            native::place(h, r);
                        }
                        boy.watch = native::Watch::start(ctx.clone(), h);
                        boy.placed = true;
                    }
                }
                crate::tutorial::mark(ctx, crate::tutorial::Spot::Clipboy, ctx.screen_rect());
                egui::CentralPanel::default()
                    .frame(
                        egui::Frame::NONE
                            .fill(tone.bg)
                            .stroke(Stroke::new(1.0_f32, tone.line_strong))
                            .inner_margin(egui::Margin::same(10)),
                    )
                    .show(ctx, |ui| {
                        boy.ui(ui, tone, now, collecting, &mut acts);
                        for m in &boy.items {
                            if m.kind == "image" && m.has_thumb && !boy.thumbs_asked.contains(&m.id)
                            {
                                want_thumbs.push(m.id);
                            }
                            if m.kind == "table" && !boy.grids_asked.contains(&m.id) {
                                want_grids.push(m.id);
                            }
                        }
                    });
            },
        );
        for id in want_thumbs {
            self.boy.thumbs_asked.insert(id);
            self.send(Cmd::Thumb(id));
        }
        for id in want_grids {
            self.boy.grids_asked.insert(id);
            self.send(Cmd::Grid(id));
        }
        for a in acts {
            match a {
                Act::Paste(id) => {
                    self.boy.paste = true;
                    self.send(Cmd::Restore(id));
                }
                Act::Copy(id) => {
                    self.boy.paste = false;
                    self.send(Cmd::Restore(id));
                }
                Act::Pin(id, on) => {
                    for m in self.items.iter_mut().chain(self.boy.items.iter_mut()) {
                        if m.id == id {
                            m.pinned = on;
                        }
                    }
                    self.send(Cmd::Pin(id, on));
                }
                Act::Delete(id) => {
                    self.items.retain(|m| m.id != id);
                    self.boy.items.retain(|m| m.id != id);
                    self.send(Cmd::Delete(id));
                }
                Act::Clear => {
                    let watermark = self.live.as_ref().map_or(0, |l| {
                        l.shared.seq.load(std::sync::atomic::Ordering::Relaxed)
                    });
                    self.boy.cleared();
                    self.send(Cmd::ClearUnprotected { watermark });
                }
                Act::Keep => self.boy.keep = !self.boy.keep,
            }
        }
        if self.boy.note.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }
    }
}

impl Clipboy {
    fn ui(
        &mut self,
        ui: &mut egui::Ui,
        tone: ui::Tone,
        now: f64,
        collecting: bool,
        acts: &mut Vec<Act>,
    ) {
        ui.horizontal(|ui| {
            let tip = if self.keep {
                "압정 빼기 · 다른 곳을 누르거나 키를 누르면 닫혀요"
            } else {
                "압정 꽂기 · 다른 곳을 눌러도 계속 떠 있어요 (단축키를 한 번 더 누르면 닫혀요)"
            };
            if crate::icons::toggle_icon(ui, Icon::Pin, tip, self.keep).clicked() {
                acts.push(Act::Keep);
            }
            ui.label(RichText::new("클립보이").strong().color(tone.ink));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button("전체비우기")
                    .on_hover_text("압정을 꽂은 항목과 코멘트가 있는 항목은 남겨요")
                    .clicked()
                {
                    acts.push(Act::Clear);
                }
                ui.label(
                    RichText::new(format!("({}/{MAX_ITEMS})", self.items.len())).color(tone.muted),
                );
            });
        });
        if let Some((text, at)) = &self.note {
            if now - at < 2.5 {
                ui.label(RichText::new(text).size(12.).color(ui::accent(ui)));
            } else {
                self.note = None;
            }
        }
        ui.add_space(4.);
        egui::Frame::NONE
            .stroke(Stroke::new(1.0_f32, tone.line_strong))
            .corner_radius(10.)
            .inner_margin(egui::Margin::symmetric(6, 4))
            .show(ui, |ui| {
                ui.set_min_size(ui.available_size());
                if self.items.is_empty() {
                    ui.add_space(40.);
                    ui.vertical_centered(|ui| {
                        if !self.loaded {
                            ui.label(RichText::new("불러오는 중…").color(tone.muted));
                        } else {
                            ui.label(RichText::new("아직 모은 기록이 없어요").color(tone.ink));
                            ui.add_space(4.);
                            ui.label(
                                RichText::new(if collecting {
                                    "다른 프로그램에서 Ctrl+C로 복사하면 여기에 쌓여요"
                                } else {
                                    "클립보드 히스토리에서 수집을 켜 주세요"
                                })
                                .size(12.)
                                .color(tone.muted),
                            );
                        }
                    });
                    return;
                }
                ui.spacing_mut().item_spacing.y = 0.;
                egui::ScrollArea::vertical()
                    .id_salt("clipboy_list")
                    .auto_shrink([false, false])
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                    .show_rows(ui, ROW_H, self.items.len(), |ui, rows| {
                        for i in rows {
                            self.row(ui, i, tone, acts);
                        }
                    });
            });
    }

    fn row(&self, ui: &mut egui::Ui, i: usize, tone: ui::Tone, acts: &mut Vec<Act>) {
        let m = &self.items[i];
        let width = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, ROW_H), Sense::hover());
        let painter = ui.painter_at(rect);
        let id = ui.id().with(("clipboy_row", m.id));
        // Right side: × at the top, then 복사 and the pin.
        let close = Rect::from_min_size(
            Pos2::new(rect.right() - 22., rect.top() + 3.),
            Vec2::splat(20.),
        );
        let buttons_y = rect.center().y + 7.;
        let pin =
            Rect::from_center_size(Pos2::new(rect.right() - 13., buttons_y), Vec2::splat(22.));
        let copy =
            Rect::from_center_size(Pos2::new(pin.left() - 24., buttons_y), Vec2::new(40., 22.));
        let thumb = Rect::from_min_size(
            Pos2::new(rect.left() + 4., rect.center().y - THUMB / 2.),
            Vec2::splat(THUMB),
        );
        let content = Rect::from_min_max(
            Pos2::new(thumb.right() + 10., rect.top() + 6.),
            Pos2::new(
                (copy.left() - 6.).max(thumb.right() + 30.),
                rect.bottom() - 6.,
            ),
        );
        let paste = ui
            .interact(thumb.union(content), id.with("paste"), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text("누르면 지금 커서 자리에 붙여 넣어요");
        let copy_r = ui.interact(copy, id.with("copy"), Sense::click());
        let pin_r = ui
            .interact(pin, id.with("pin"), Sense::click())
            .on_hover_text(if m.pinned {
                "고정 풀기"
            } else {
                "맨 위에 고정 · 전체비우기와 자동 정리에서 빠져요"
            });
        let close_r = ui
            .interact(close, id.with("close"), Sense::click())
            .on_hover_text("이 기록 지우기");
        if paste.hovered() {
            painter.rect_filled(rect.shrink2(Vec2::new(0., 2.)), 8., tone.hover);
        }
        painter.hline(
            rect.x_range(),
            rect.bottom() - 0.5,
            Stroke::new(1.0_f32, tone.line),
        );
        self.paint_thumb(&painter, thumb, m, tone, ui.visuals().dark_mode);
        // The content: what it is for pictures and tables, the text itself
        // for text and links.
        let (main, sub) = match m.kind.as_str() {
            "image" => ("이미지".to_owned(), m.meta.clone()),
            "table" => ("표".to_owned(), m.meta.clone()),
            _ => (m.summary.clone(), String::new()),
        };
        let comment = m.comment.trim();
        let main_color = if m.kind == "link" {
            ui::accent(ui)
        } else {
            tone.ink
        };
        let lines = if comment.is_empty() && sub.is_empty() {
            2
        } else {
            1
        };
        let galley = wrapped(ui, &main, 14., main_color, content.width(), lines);
        let mut y = content.top() + 2.;
        let h = galley.size().y;
        painter.galley(Pos2::new(content.left(), y), galley, main_color);
        y += h + 2.;
        let second = if !comment.is_empty() {
            format!("코멘트 · {comment}")
        } else {
            sub
        };
        if !second.is_empty() {
            let g = wrapped(ui, &second, 11.5, tone.muted, content.width(), 1);
            painter.galley(Pos2::new(content.left(), y), g, tone.muted);
        }
        for (r, hovered) in [
            (copy, copy_r.hovered()),
            (pin, pin_r.hovered()),
            (close, close_r.hovered()),
        ] {
            if hovered {
                painter.rect_filled(r, 6., tone.pressed);
            }
        }
        painter.text(
            copy.center(),
            Align2::CENTER_CENTER,
            "복사",
            FontId::proportional(12.),
            tone.muted,
        );
        crate::icons::paint_icon(
            &painter,
            pin.shrink(4.),
            Icon::Pin,
            if m.pinned { ui::accent(ui) } else { tone.faint },
        );
        crate::icons::paint_icon(&painter, close.shrink(4.), Icon::Close, tone.muted);
        if paste.clicked() {
            acts.push(Act::Paste(m.id));
        } else if copy_r.clicked() {
            acts.push(Act::Copy(m.id));
        } else if pin_r.clicked() {
            acts.push(Act::Pin(m.id, !m.pinned));
        } else if close_r.clicked() {
            acts.push(Act::Delete(m.id));
        }
    }

    /// The square on the left: the picture, a small drawing of the table,
    /// or the kind of content.
    fn paint_thumb(&self, p: &egui::Painter, r: Rect, m: &ItemMeta, tone: ui::Tone, dark: bool) {
        p.rect_filled(r, 8., tone.field);
        p.rect_stroke(
            r,
            8.,
            Stroke::new(1.0_f32, tone.line_strong),
            egui::StrokeKind::Inside,
        );
        if let Some(t) = self.textures.get(&m.id) {
            let s = t.size_vec2();
            let k = ((r.width() - 6.) / s.x).min((r.height() - 6.) / s.y);
            p.image(
                t.id(),
                Rect::from_center_size(r.center(), s * k),
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1., 1.)),
                Color32::WHITE,
            );
            return;
        }
        if let Some(cells) = self.grids.get(&m.id).filter(|c| !c.is_empty()) {
            let rows = cells.len();
            let cols = cells.iter().map(Vec::len).max().unwrap_or(1).max(1);
            let inner = r.shrink(5.);
            let (cw, ch) = (inner.width() / cols as f32, inner.height() / rows as f32);
            let head = if dark {
                Color32::from_rgb(40, 70, 55)
            } else {
                Color32::from_rgb(214, 238, 222)
            };
            p.rect_filled(
                Rect::from_min_size(inner.min, Vec2::new(inner.width(), ch)),
                0.,
                head,
            );
            let line = Stroke::new(0.8_f32, tone.line_strong);
            for k in 0..=rows {
                p.hline(inner.x_range(), inner.top() + ch * k as f32, line);
            }
            for k in 0..=cols {
                p.vline(inner.left() + cw * k as f32, inner.y_range(), line);
            }
            let clip = p.with_clip_rect(inner);
            for (y, row) in cells.iter().enumerate() {
                for (x, cell) in row.iter().enumerate() {
                    clip.text(
                        Pos2::new(
                            inner.left() + cw * x as f32 + 1.,
                            inner.top() + ch * (y as f32 + 0.5),
                        ),
                        Align2::LEFT_CENTER,
                        cell,
                        FontId::proportional(6.),
                        tone.ink,
                    );
                }
            }
            return;
        }
        let label = match m.kind.as_str() {
            "image" => "이미지",
            "table" => "표",
            "link" => "링크",
            _ => "텍스트",
        };
        p.text(
            r.center(),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(11.),
            tone.muted,
        );
    }
}

/// `text` laid out in `width`, at most `rows` lines, ending in … when cut.
fn wrapped(
    ui: &egui::Ui,
    text: &str,
    size: f32,
    color: Color32,
    width: f32,
    rows: usize,
) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::single_section(
        text.to_owned(),
        egui::TextFormat {
            font_id: FontId::proportional(size),
            color,
            ..Default::default()
        },
    );
    job.wrap = egui::text::TextWrapping {
        max_width: width.max(10.),
        max_rows: rows,
        break_anywhere: true,
        overflow_character: Some('…'),
    };
    ui.fonts(|f| f.layout_job(job))
}

/// The Windows side: where the user types, pasting there, a window that
/// never takes the focus, and noticing clicks and key presses elsewhere.
#[cfg(windows)]
mod native {
    use eframe::egui;
    use std::collections::HashSet;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    use std::sync::{Arc, Mutex};
    use windows_sys::Win32::Foundation::{LPARAM, LRESULT, POINT, RECT, WPARAM};
    use windows_sys::Win32::Graphics::Gdi::{
        ClientToScreen, GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::System::Threading::GetCurrentThreadId;
    use windows_sys::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, GetCursorPos, GetForegroundWindow, GetGUIThreadInfo, GetMessageW,
        GetWindowLongPtrW, GetWindowRect, GetWindowThreadProcessId, PeekMessageW,
        PostThreadMessageW, SetWindowLongPtrW, SetWindowPos, SetWindowsHookExW,
        UnhookWindowsHookEx, GUITHREADINFO, GWL_EXSTYLE, HWND_TOPMOST, KBDLLHOOKSTRUCT, MSG,
        MSLLHOOKSTRUCT, PM_NOREMOVE, SWP_NOACTIVATE, SWP_SHOWWINDOW, WH_KEYBOARD_LL, WH_MOUSE_LL,
        WM_APP, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_MBUTTONDOWN, WM_QUIT, WM_RBUTTONDOWN,
        WM_SYSKEYDOWN, WM_SYSKEYUP, WM_XBUTTONDOWN, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    };

    /// Where the user was typing when the shortcut was pressed (physical
    /// pixels): the text cursor if the program shows one, else the pointer.
    #[derive(Clone, Copy, Debug)]
    pub struct Anchor {
        caret: Option<[i32; 4]>,
        pointer: (i32, i32),
    }

    static ANCHOR: Mutex<Option<Anchor>> = Mutex::new(None);

    const VK_MENU: i32 = 0x12;
    const VK_LWIN: i32 = 0x5B;
    const VK_RWIN: i32 = 0x5C;
    /// A key no keyboard has (the one AutoHotkey uses for the same purpose).
    const VK_MASK: u16 = 0xE8;

    fn key(vk: u16, up: bool) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: if up { KEYEVENTF_KEYUP } else { 0 },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    fn send(inputs: &[INPUT]) {
        // SAFETY: a plain array of keyboard events.
        unsafe {
            SendInput(
                inputs.len() as u32,
                inputs.as_ptr(),
                std::mem::size_of::<INPUT>() as i32,
            );
        }
    }

    fn down(vk: i32) -> bool {
        // SAFETY: reads the key state only.
        unsafe { GetAsyncKeyState(vk) as u16 & 0x8000 != 0 }
    }

    /// Called right when a global shortcut is pressed, on the thread that
    /// receives the presses: remembers where the user types. Alt (or the
    /// Windows key) let go after a shortcut would otherwise open the menu
    /// of the program in front (or the Start menu): a key press nobody uses
    /// in between keeps that from happening.
    pub fn note_hotkey() {
        if down(VK_MENU) || down(VK_LWIN) || down(VK_RWIN) {
            send(&[key(VK_MASK, false), key(VK_MASK, true)]);
        }
        // SAFETY: plain queries about the window in front.
        let anchor = unsafe {
            let mut p = POINT { x: 0, y: 0 };
            GetCursorPos(&mut p);
            Anchor {
                caret: caret(),
                pointer: (p.x, p.y),
            }
        };
        *ANCHOR.lock().unwrap_or_else(|e| e.into_inner()) = Some(anchor);
    }

    pub fn take_anchor() -> Option<Anchor> {
        ANCHOR.lock().unwrap_or_else(|e| e.into_inner()).take()
    }

    unsafe fn caret() -> Option<[i32; 4]> {
        let front = GetForegroundWindow();
        if front.is_null() {
            return None;
        }
        let thread = GetWindowThreadProcessId(front, std::ptr::null_mut());
        let mut info: GUITHREADINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
        if GetGUIThreadInfo(thread, &mut info) == 0 || info.hwndCaret.is_null() {
            return None;
        }
        let r = info.rcCaret;
        let mut a = POINT {
            x: r.left,
            y: r.top,
        };
        let mut b = POINT {
            x: r.right,
            y: r.bottom,
        };
        if ClientToScreen(info.hwndCaret, &mut a) == 0
            || ClientToScreen(info.hwndCaret, &mut b) == 0
        {
            return None;
        }
        Some([a.x, a.y, b.x.max(a.x + 1), b.y.max(a.y + 1)])
    }

    /// Ctrl+V to the program in front.
    pub fn paste() {
        const VK_CONTROL: u16 = 0x11;
        const VK_V: u16 = 0x56;
        send(&[
            key(VK_CONTROL, false),
            key(VK_V, false),
            key(VK_V, true),
            key(VK_CONTROL, true),
        ]);
    }

    /// Never takes the focus (typing goes on in the program in front), no
    /// taskbar button.
    pub fn style(h: isize) {
        // SAFETY: style bits of a window of this program.
        unsafe {
            let h = h as windows_sys::Win32::Foundation::HWND;
            let ex = GetWindowLongPtrW(h, GWL_EXSTYLE);
            let want = ex | (WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW) as isize;
            if want != ex {
                SetWindowLongPtrW(h, GWL_EXSTYLE, want);
            }
        }
    }

    pub fn place(h: isize, r: [i32; 4]) {
        // SAFETY: moves a window of this program without activating it.
        unsafe {
            SetWindowPos(
                h as _,
                HWND_TOPMOST,
                r[0],
                r[1],
                r[2] - r[0],
                r[3] - r[1],
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
        }
    }

    /// Where the window goes (physical pixels) and the scale of that
    /// monitor: under the text cursor (above it when there is no room
    /// below), else next to the pointer, inside the monitor's work area.
    pub fn rect_for(anchor: Option<Anchor>, size: egui::Vec2) -> Option<([i32; 4], f32)> {
        let anchor = anchor.or_else(|| {
            let mut p = POINT { x: 0, y: 0 };
            // SAFETY: reads the pointer position.
            unsafe { GetCursorPos(&mut p) };
            Some(Anchor {
                caret: None,
                pointer: (p.x, p.y),
            })
        })?;
        let (x, top, bottom) = match anchor.caret {
            Some(c) => (c[0], c[1], c[3]),
            None => (anchor.pointer.0, anchor.pointer.1, anchor.pointer.1 + 12),
        };
        // SAFETY: plain monitor queries.
        let (work, scale) = unsafe {
            let monitor = MonitorFromPoint(POINT { x, y: top }, MONITOR_DEFAULTTONEAREST);
            let mut info: MONITORINFO = std::mem::zeroed();
            info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
            if monitor.is_null() || GetMonitorInfoW(monitor, &mut info) == 0 {
                return None;
            }
            let (mut dx, mut dy) = (0u32, 0u32);
            let scale =
                if GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dx, &mut dy) == 0 && dx > 0 {
                    dx as f32 / 96.
                } else {
                    1.
                };
            (info.rcWork, scale)
        };
        Some((
            super::fit(
                [work.left, work.top, work.right, work.bottom],
                x,
                top,
                bottom,
                size * scale,
                scale,
            ),
            scale,
        ))
    }

    /// Notices a click outside the window and a key pressed anywhere while
    /// it is open, without taking either.
    pub struct Watch {
        flags: Arc<[AtomicBool; 2]>,
        thread: Arc<AtomicU32>,
    }

    const WM_CLICK: u32 = WM_APP + 0x61;
    const WM_TYPED: u32 = WM_APP + 0x62;
    const LLKHF_INJECTED: u32 = 0x10;
    const LLMHF_INJECTED: u32 = 0x01;

    thread_local! {
        /// Keys already held when the watch began (the shortcut's own key):
        /// their repeats are not typing.
        static HELD: std::cell::RefCell<HashSet<u32>> = std::cell::RefCell::new(HashSet::new());
    }

    fn modifier(vk: u32) -> bool {
        matches!(vk, 0x10..=0x12 | 0x5B | 0x5C | 0xA0..=0xA5 | 0x14)
    }

    /// As short as can be: Windows drops a low-level hook that answers too slowly.
    unsafe extern "system" fn keys(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
        if code >= 0 {
            let k = &*(l as *const KBDLLHOOKSTRUCT);
            let msg = w as u32;
            if msg == WM_KEYUP || msg == WM_SYSKEYUP {
                HELD.with(|h| h.borrow_mut().remove(&k.vkCode));
            } else if (msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN)
                && k.flags & LLKHF_INJECTED == 0
                && !modifier(k.vkCode)
                && !HELD.with(|h| h.borrow().contains(&k.vkCode))
            {
                PostThreadMessageW(GetCurrentThreadId(), WM_TYPED, 0, 0);
            }
        }
        CallNextHookEx(std::ptr::null_mut(), code, w, l)
    }

    unsafe extern "system" fn mouse(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
        if code >= 0
            && matches!(
                w as u32,
                WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_XBUTTONDOWN
            )
        {
            let m = &*(l as *const MSLLHOOKSTRUCT);
            if m.flags & LLMHF_INJECTED == 0 {
                PostThreadMessageW(
                    GetCurrentThreadId(),
                    WM_CLICK,
                    m.pt.x as isize as WPARAM,
                    m.pt.y as LPARAM,
                );
            }
        }
        CallNextHookEx(std::ptr::null_mut(), code, w, l)
    }

    impl Watch {
        pub fn start(ctx: egui::Context, hwnd: isize) -> Option<Self> {
            let flags = Arc::new([AtomicBool::new(false), AtomicBool::new(false)]);
            let thread = Arc::new(AtomicU32::new(0));
            let (f, t) = (flags.clone(), thread.clone());
            std::thread::Builder::new()
                .name("clipboy-watch".into())
                .spawn(move || unsafe {
                    let mut msg: MSG = std::mem::zeroed();
                    PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_NOREMOVE);
                    HELD.with(|h| {
                        let mut h = h.borrow_mut();
                        for vk in 0x08..0xFF {
                            if GetAsyncKeyState(vk) as u16 & 0x8000 != 0 {
                                h.insert(vk as u32);
                            }
                        }
                    });
                    let module = GetModuleHandleW(std::ptr::null());
                    let k = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keys), module, 0);
                    let m = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse), module, 0);
                    t.store(GetCurrentThreadId(), Ordering::SeqCst);
                    while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                        let hit = match msg.message {
                            WM_TYPED => Some(1),
                            WM_CLICK => {
                                let (x, y) = (msg.wParam as isize as i32, msg.lParam as i32);
                                let mut r: RECT = std::mem::zeroed();
                                let inside = GetWindowRect(hwnd as _, &mut r) != 0
                                    && (r.left..r.right).contains(&x)
                                    && (r.top..r.bottom).contains(&y);
                                (!inside).then_some(0)
                            }
                            _ => None,
                        };
                        if let Some(k) = hit {
                            f[k].store(true, Ordering::Relaxed);
                            ctx.request_repaint_of(egui::ViewportId::ROOT);
                        }
                    }
                    if !k.is_null() {
                        UnhookWindowsHookEx(k);
                    }
                    if !m.is_null() {
                        UnhookWindowsHookEx(m);
                    }
                })
                .ok()?;
            Some(Self { flags, thread })
        }

        /// (clicked outside, typed) since the last call.
        pub fn take(&self) -> (bool, bool) {
            (
                self.flags[0].swap(false, Ordering::Relaxed),
                self.flags[1].swap(false, Ordering::Relaxed),
            )
        }
    }

    impl Drop for Watch {
        fn drop(&mut self) {
            // The thread may still be starting: wait for its id briefly.
            for _ in 0..50 {
                let id = self.thread.load(Ordering::SeqCst);
                if id != 0 {
                    // SAFETY: posting to a thread that may be gone is harmless.
                    unsafe { PostThreadMessageW(id, WM_QUIT, 0, 0) };
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
        }
    }
}

#[cfg(not(windows))]
mod native {
    use eframe::egui;

    #[derive(Clone, Copy, Debug)]
    pub struct Anchor;

    pub fn note_hotkey() {}
    pub fn take_anchor() -> Option<Anchor> {
        None
    }
    pub fn paste() {}
    pub fn style(_: isize) {}
    pub fn place(_: isize, _: [i32; 4]) {}
    pub fn rect_for(_: Option<Anchor>, _: egui::Vec2) -> Option<([i32; 4], f32)> {
        None
    }

    pub struct Watch;

    impl Watch {
        pub fn start(_: egui::Context, _: isize) -> Option<Self> {
            None
        }
        pub fn take(&self) -> (bool, bool) {
            (false, false)
        }
    }
}

/// A window of `size` (physical pixels) for text between `top` and
/// `bottom` at `x`: below it, above it when there is no room below, inside
/// `work` (left, top, right, bottom).
#[cfg_attr(not(windows), allow(dead_code))]
fn fit(work: [i32; 4], x: i32, top: i32, bottom: i32, size: Vec2, scale: f32) -> [i32; 4] {
    let (w, h) = (size.x.round() as i32, size.y.round() as i32);
    let gap = (6. * scale).round() as i32;
    let mut y = bottom + gap;
    if y + h > work[3] {
        y = top - gap - h;
    }
    let x = x.clamp(work[0], (work[2] - w).max(work[0]));
    let y = y.clamp(work[1], (work[3] - h).max(work[1]));
    [x, y, x + w, y + h]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_opens_under_the_cursor_and_stays_on_the_monitor() {
        let work = [0, 0, 2880, 1704];
        let size = SIZE * 2.;
        // Under the text, 6 points apart.
        assert_eq!(fit(work, 100, 200, 240, size, 2.), [100, 252, 820, 1052]);
        // No room below: above it.
        assert_eq!(fit(work, 100, 1500, 1540, size, 2.), [100, 688, 820, 1488]);
        // Near the right edge: pulled back in.
        assert_eq!(fit(work, 2800, 200, 240, size, 2.)[2], 2880);
        // A second monitor left of the first.
        let left = [-1920, 0, 0, 1040];
        let r = fit(left, -10, 10, 30, SIZE, 1.);
        assert!(r[0] >= -1920 && r[2] <= 0 && r[1] >= 0 && r[3] <= 1040);
    }

    fn item(id: i64, pinned: bool, comment: &str) -> ItemMeta {
        ItemMeta {
            id,
            captured_ms: id,
            kind: "text".into(),
            summary: format!("t{id}"),
            meta: String::new(),
            bytes: 1,
            formats: "text".into(),
            reduced: false,
            has_thumb: false,
            comment: comment.into(),
            pinned,
        }
    }

    #[test]
    fn clearing_keeps_what_the_user_kept_and_the_list_follows() {
        let mut boy = Clipboy {
            open: true,
            req: 3,
            ..Default::default()
        };
        boy.listed(2, vec![item(9, false, "")]);
        assert!(boy.items.is_empty(), "an older answer is ignored");
        boy.listed(
            3,
            vec![
                item(1, true, ""),
                item(2, false, "메모"),
                item(3, false, " "),
            ],
        );
        assert!(boy.loaded);
        // Drawn at the Win+V size, with nothing pressed.
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SIZE)),
            ..Default::default()
        };
        let mut acts = vec![];
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                boy.ui(ui, crate::ui::tone(false), 0., true, &mut acts);
            });
        });
        assert!(acts.is_empty());
        boy.cleared();
        assert_eq!(
            boy.items.iter().map(|m| m.id).collect::<Vec<_>>(),
            vec![1, 2]
        );
        // Closing lets go of everything it showed.
        boy.close();
        assert!(!boy.open && boy.textures.is_empty() && boy.grids.is_empty());
    }

    #[test]
    fn the_shortcut_toggles_and_a_closing_key_does_not_reopen_it() {
        let mut boy = Clipboy::default();
        boy.toggle(1., None);
        assert!(boy.open && boy.stale && !boy.loaded);
        boy.toggle(2., None);
        assert!(!boy.open);
        // A key press closed it; the shortcut's own press right after does
        // not open it again, a later one does.
        boy.toggle(3., None);
        boy.close();
        boy.closed_by_key = Some(5.);
        boy.toggle(5.2, None);
        assert!(!boy.open);
        boy.toggle(5.3, None);
        assert!(boy.open);
        // Without a running history nothing opens.
        let mut h = ClipboardHistory::default();
        h.toggle_clipboy(9.);
        assert!(!h.boy.open);
    }
}
