use crate::icons::{icon_button, Icon};
use crate::*;
use egui::{Pos2, Rect, Sense};
use std::collections::BTreeMap;

/// The color of the "중요 메모" star shown before titles.
pub const STAR: Color32 = Color32::from_rgb(236, 168, 32);
/// The smallest sticker: its top bar still shows whole (its buttons with a
/// little blank space to drag it by).
pub const STICKER_MIN: Vec2 = Vec2::new(290., 220.);
/// Width of the title line a memo folds to (step 1) unless set otherwise,
/// and the range it can be set in.
pub const TITLE_WIDTH: f32 = 360.;
pub const TITLE_MIN: f32 = 160.;
pub const TITLE_MAX: f32 = 1600.;
/// Size of the color box a memo folds to (step 2) unless set otherwise, and
/// the range it can be set in.
pub const SWATCH_SIZE: f32 = 48.;
pub const SWATCH_MIN: f32 = 16.;
pub const SWATCH_MAX: f32 = 640.;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Editor,
    Title,
    Swatch,
    Sticker,
}
pub struct Window {
    pub mode: Mode,
    pub sticker_preferred: bool,
    pub open: bool,
    pub occurrence: Option<NaiveDate>,
    pub schedule_start: Option<NaiveDate>,
    full_size: Vec2,
    title_width: f32,
    skip_title: bool,
    /// Folding stops at the title line (the setting "컬러박스 생략").
    skip_swatch: bool,
    /// The title line width a fold into step 1 starts with (the setting).
    title_default: f32,
    /// The color box size a fold into step 2 starts with (the setting).
    swatch_default: Vec2,
    pub sticker_size: Vec2,
    swatch_size: Vec2,
    applied: Option<(Mode, bool)>,
    focus: bool,
    center: bool,
    tutorial_center: Option<Vec2>,
    reopening: bool,
    help: bool,
    native: crate::native_window::State,
    pub(crate) place: Option<Pos2>,
    swatch_drag: Option<(Vec2, Pos2, Vec2)>,
    calendar_bar: bool,
    menu: Option<Menu>,
    /// Where the window was last seen on screen, for reopening it there.
    pub(crate) last_pos: Option<Pos2>,
    /// `place` is where a closed window was last seen: kept on that
    /// monitor's work area, since it may come back larger (as the editor).
    fit_place: bool,
    /// The native window, once drawn.
    hwnd: Option<isize>,
    /// Set when Enter is pressed in the sticker title; applied at the start
    /// of the *next* frame, never the one Enter was pressed on. The title's
    /// return_key handling does not remove that Enter (or a pending IME
    /// commit) from this frame's input, so requesting focus on the body in
    /// the same frame let its own TextEdit see the same event again and
    /// insert a stray newline (or duplicate the just-composed character).
    move_caret_to_body: bool,
    /// When the window was last built in full, and `lists::CHANGES` then.
    drawn_at: f64,
    drawn_changes: u64,
}
/// A window resting (see [`may_rest`]) is built in full again at least this
/// often, and once more this long after the last rest.
pub(crate) const REST_MAX: f64 = 0.25;
/// The main window behind a memo window rests only while the pointer is at
/// least this far (points) inside that memo window: leaving it, the pointer
/// meets a main window that was built for it.
const MAIN_REST_INSET: f32 = 24.;
/// The pointer this close to a window (points) keeps it built every frame:
/// a window it enters has its widgets laid out, so the first click counts.
const REST_NEAR: f32 = 160.;
/// What decides whether a window may skip building its contents this
/// frame and keep showing the last one.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Quiet {
    /// Input, focus or the pointer in this window, or a menu, dialog,
    /// drag or window command of its own under way.
    pub busy: bool,
    pub pointer_near: bool,
    /// Data changed by input since the window was last built.
    pub changed: bool,
    pub since_drawn: f64,
}
/// Pointer moves over one window make every window run a frame (they share
/// one). A window none of that concerns keeps its last picture, so moving
/// the mouse costs about what one window costs, not all of them.
pub(crate) fn may_rest(q: Quiet) -> bool {
    !q.busy && !q.pointer_near && !q.changed && (0. ..REST_MAX).contains(&q.since_drawn)
}
/// An area floats over the window (egui's own background layer, there in
/// every frame, aside).
fn floating_area_shown(ctx: &egui::Context) -> bool {
    ctx.memory(|m| {
        m.areas()
            .visible_layer_ids()
            .iter()
            .any(|l| *l != egui::LayerId::background())
    })
}
/// Desktop arrangement requested from a sticker's context menu.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Layout {
    Right,
    Left,
    Cascade,
}
#[derive(Clone, Copy, Debug)]
pub struct Arrange {
    pub preserve_sticker_order: bool,
    pub layout: Layout,
    pub fold: Option<Mode>,
    pub area: Rect,
}
impl Window {
    pub(crate) fn new(mode: Mode) -> Self {
        Self {
            mode,
            sticker_preferred: mode == Mode::Sticker,
            open: true,
            occurrence: None,
            schedule_start: None,
            full_size: Vec2::new(620., 720.),
            title_width: 360.,
            skip_title: false,
            skip_swatch: false,
            swatch_default: Vec2::splat(SWATCH_SIZE),
            title_default: TITLE_WIDTH,
            sticker_size: Vec2::splat(360.),
            swatch_size: Vec2::splat(48.),
            applied: None,
            focus: true,
            center: true,
            tutorial_center: None,
            reopening: false,
            help: false,
            native: Default::default(),
            place: None,
            swatch_drag: None,
            calendar_bar: false,
            menu: None,
            last_pos: None,
            fit_place: false,
            hwnd: None,
            move_caret_to_body: false,
            drawn_at: f64::NEG_INFINITY,
            drawn_changes: 0,
        }
    }
    /// What is needed to reopen this window after a restart.
    pub fn session(&self, note: usize) -> crate::storage::SessionRec {
        crate::storage::SessionRec {
            note,
            mode: match self.mode {
                Mode::Editor => 0,
                Mode::Title => 1,
                Mode::Swatch => 2,
                Mode::Sticker => 3,
            },
            sticker_preferred: self.sticker_preferred,
            pos: self.last_pos.map(|p| [p.x, p.y]),
            full_size: self.full_size.into(),
            sticker_size: self.sticker_size.into(),
            title_width: self.title_width,
            swatch_size: self.swatch_size.into(),
            closed: !self.open,
        }
    }
    pub(crate) fn center_for_tutorial(&mut self, offset: Vec2) {
        self.tutorial_center = Some(offset);
        self.center = true;
        self.place = None;
        self.fit_place = false;
        self.reopening = false;
    }

    /// Whether the window has been somewhere on screen (reopening puts it
    /// back there).
    pub fn has_place(&self) -> bool {
        self.place.or(self.last_pos).is_some()
    }
    /// A window reopened where it was, without taking focus.
    pub fn from_session(r: &crate::storage::SessionRec) -> Self {
        let mode = match r.mode {
            1 => Mode::Title,
            2 => Mode::Swatch,
            3 => Mode::Sticker,
            _ => Mode::Editor,
        };
        let mut w = Self::new(mode);
        w.sticker_preferred = r.sticker_preferred;
        let size =
            |s: [f32; 2], min: Vec2| (s[0] > 0. && s[1] > 0.).then(|| Vec2::from(s).max(min));
        if let Some(s) = size(r.full_size, Vec2::new(480., 480.)) {
            w.full_size = s;
        }
        if let Some(s) = size(r.sticker_size, STICKER_MIN) {
            w.sticker_size = s;
        }
        if r.title_width > 0. {
            w.title_width = r.title_width.max(160.);
        }
        if let Some(s) = size(r.swatch_size, Vec2::splat(16.)) {
            w.swatch_size = s;
        }
        w.focus = false;
        if let Some(p) = r.pos {
            w.center = false;
            w.place = Some(Pos2::new(p[0], p[1]));
            w.last_pos = w.place;
        }
        w
    }
    fn reconcile(&mut self, n: &Note) {
        if let (Some(old), Some(new), Some(d)) = (self.schedule_start, n.date, self.occurrence) {
            if old != new {
                self.occurrence = Some(d + (new - old));
            }
        }
        self.schedule_start = n.date;
        if n.duration > 1 {
            if let (Some(d), Some(start)) = (self.occurrence, n.date) {
                let end = start + chrono::Duration::days(n.duration.max(1) as i64 - 1);
                self.occurrence = Some(d.clamp(start, end));
            }
        }
    }
    pub fn fold(&mut self) {
        self.mode = match self.mode {
            Mode::Editor | Mode::Sticker if self.skip_title => self.fold_to_swatch(),
            Mode::Editor | Mode::Sticker => {
                // Folded into the title line, at the width set for it.
                self.title_width = self.title_default;
                Mode::Title
            }
            // No color box: the title line opens up again.
            Mode::Title if self.skip_swatch => self.unfolded(),
            Mode::Title => self.fold_to_swatch(),
            Mode::Swatch => self.unfolded(),
        };
    }
    /// Folded into the color box, at the size set for it.
    fn fold_to_swatch(&mut self) -> Mode {
        self.swatch_size = self.swatch_default;
        Mode::Swatch
    }
    fn unfolded(&self) -> Mode {
        if self.sticker_preferred {
            Mode::Sticker
        } else {
            Mode::Editor
        }
    }
    pub fn sticker(&mut self) {
        self.sticker_preferred = true;
        self.mode = Mode::Sticker;
    }
    pub fn edit(&mut self) {
        self.mode = Mode::Editor;
    }
    fn outer_size(&self, locked: bool) -> Vec2 {
        if locked {
            Vec2::splat(64.)
        } else {
            self.size()
        }
    }
    fn size(&self) -> Vec2 {
        match self.mode {
            Mode::Editor => self.full_size,
            Mode::Title => Vec2::new(self.title_width, 44.),
            Mode::Swatch => self.swatch_size,
            Mode::Sticker => self.sticker_size,
        }
    }
}
/// A sticker being carried over a calendar: dropped on a date it takes that
/// date, then it goes back where it was.
pub struct StickerDrag {
    pub shift: bool,
    pub note: usize,
    /// Where the window was (outer position, points) and what it showed.
    pub origin: Pos2,
    pub mode: Mode,
    /// Return the window to `origin`/`mode` wherever it is let go (a sticker
    /// pulled out by its calendar icon only borrowed the title-only look).
    pub always_restore: bool,
    pub started: f64,
    pub saw_down: bool,
    pub(crate) carry: Option<CarriedSticker>,
}

pub(crate) struct CarriedSticker {
    hwnd: isize,
    grab: Vec2,
    pub(crate) pointer: Option<Pos2>,
    pub(crate) preview: Option<crate::dragging::ScheduleHit>,
}

#[derive(Default)]
pub struct Studio {
    raw_date_drop: Option<(usize, f64)>,
    pub windows: BTreeMap<usize, Window>,
    pub help: bool,
    pub drag: Option<StickerDrag>,
    /// When the main window was last built in full, and `lists::CHANGES` then.
    pub(crate) main_drawn: Option<(f64, u64)>,
    pub(crate) main_focused: bool,
    stack_below_main: Option<isize>,
    #[cfg(windows)]
    desktop_watch: Option<crate::win_style::ForegroundWatch>,
    #[cfg(windows)]
    desktop_entered: Option<f64>,
    #[cfg(windows)]
    desktop_stickers: Vec<isize>,
    #[cfg(windows)]
    desktop_promoted: Vec<isize>,
    /// The "글꼴" dialog of a memo window.
    pub font_dialog: Option<FontDialog>,
}
/// The "글꼴" dialog, opened from a memo window's right-click menus.
pub struct FontDialog {
    pub note: usize,
    pub choice: crate::composer::FontChoice,
    /// What the preview shows (the selected text, or a sample).
    pub sample: String,
}
impl Studio {
    pub fn open(&mut self, i: usize, occurrence: Option<NaiveDate>) {
        let w = self.windows.entry(i).or_insert_with(|| {
            let mut w = Window::new(Mode::Editor);
            w.reopening = true;
            w
        });
        if !w.open {
            w.reopening = true;
            w.applied = None;
            w.native.cancel();
            // Back where it was last seen, wherever the user had dragged it:
            // only a memo never shown takes a new-memo place (right-aligned
            // when that is on).
            if w.place.is_none() {
                w.place = w.last_pos;
            }
            if w.place.is_some() {
                w.center = false;
                w.fit_place = true;
            }
        }
        w.open = true;
        w.mode = Mode::Editor;
        w.occurrence = occurrence;
        w.focus = true;
    }
}
/// Positions for windows of the given sizes, packed in columns from one edge
/// of `area` (or cascaded), keeping a small gap and margin.
pub fn plan(layout: Layout, area: Rect, sizes: &[Vec2]) -> Vec<Pos2> {
    let margin = 14.;
    let gap = 8.;
    if layout == Layout::Cascade {
        return sizes
            .iter()
            .enumerate()
            .map(|(k, size)| {
                let step = 30. * (k % 12) as f32;
                crate::native_window::fit_position(
                    area.min + Vec2::new(80. + step, 60. + step),
                    *size,
                    area,
                )
            })
            .collect();
    }
    let mut out = Vec::with_capacity(sizes.len());
    let mut y = area.top() + margin;
    let mut edge = if layout == Layout::Right {
        area.right() - margin
    } else {
        area.left() + margin
    };
    let mut column = 0f32;
    for size in sizes {
        if y + size.y > area.bottom() - margin && y > area.top() + margin {
            edge += if layout == Layout::Right {
                -(column + gap)
            } else {
                column + gap
            };
            y = area.top() + margin;
            column = 0.;
        }
        let x = if layout == Layout::Right {
            edge - size.x
        } else {
            edge
        };
        out.push(Pos2::new(x, y));
        y += size.y + gap;
        column = column.max(size.x);
    }
    out
}
/// Where a new window of `size` goes on the right edge of `area`: the
/// topmost spot in the rightmost column not covered by `others`, moving to the
/// next column to the left when a column is full.
pub fn right_slot(area: Rect, size: Vec2, others: &[Rect]) -> Pos2 {
    let margin = 14.;
    let gap = 8.;
    let mut x = area.right() - margin - size.x;
    while x >= area.left() + margin - 0.5 {
        let mut y = area.top() + margin;
        while y + size.y <= area.bottom() - margin + 0.5 {
            let at = Rect::from_min_size(Pos2::new(x, y), size);
            match others.iter().find(|o| o.shrink(1.).intersects(at)) {
                None => return at.min,
                Some(hit) => y = (hit.bottom() + gap).max(y + 1.),
            }
        }
        x -= size.x + gap;
    }
    // Everything is covered: stack down the right edge, shifted a little.
    let step = 30. * (others.len() % 8) as f32;
    Pos2::new(area.right() - margin - size.x, area.top() + margin + step)
}
impl App {
    /// The window for choosing the size of new stickers: a sticker-colored
    /// window to resize with the mouse. Closing it enters its size.
    pub fn size_probe_window(&mut self, ctx: &egui::Context) {
        if !self.desktop.size_probe {
            return;
        }
        let start = self.desktop.sticker_size;
        let builder = egui::ViewportBuilder::default()
            .with_title("새 스티커 메모 크기 설정")
            .with_inner_size(start)
            .with_min_inner_size(STICKER_MIN)
            .with_max_inner_size([900., 900.])
            .with_taskbar(false)
            .with_window_level(egui::WindowLevel::AlwaysOnTop);
        let fill = self.desktop.next_new_color();
        let mut closed = false;
        let mut seen = None;
        ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of("size_probe"),
            builder,
            |ctx, _| {
                self.theme(ctx);
                if ctx.input(|i| i.viewport().close_requested() || i.key_pressed(egui::Key::Escape))
                {
                    closed = true;
                }
                seen = ctx.input(|i| i.viewport().inner_rect.map(|r| r.size()));
                egui::CentralPanel::default()
                    .frame(
                        egui::Frame::NONE
                            .fill(fill)
                            .inner_margin(egui::Margin::same(18)),
                    )
                    .show(ctx, |ui| {
                        ui.label(
                            RichText::new("창의 가장자리를 끌어 원하는 크기로 바꾼 뒤, 창을 닫으세요.")
                                .color(INK),
                        );
                        ui.add_space(6.);
                        ui.label(
                            RichText::new(
                                "닫으면 이 크기가 새 스티커 메모의 크기가 됩니다 (Esc로도 닫힙니다).",
                            )
                            .color(MUTED),
                        );
                        ui.add_space(14.);
                        if let Some(s) = seen {
                            ui.label(
                                RichText::new(format!(
                                    "너비 {} px  ×  높이 {} px",
                                    s.x.round(),
                                    s.y.round()
                                ))
                                .size(20.)
                                .strong()
                                .color(INK),
                            );
                        }
                    });
            },
        );
        if let Some(s) = seen {
            self.desktop.size_probe_last = Some(s);
        }
        if closed {
            self.desktop.size_probe = false;
            if let Some(s) = self.desktop.size_probe_last.take() {
                let size = Vec2::new(
                    s.x.round().clamp(STICKER_MIN.x, 900.),
                    s.y.round().clamp(220., 900.),
                );
                self.desktop.sticker_size = size;
                self.toast = format!("새 스티커 메모 크기를 {} × {}로 정했습니다", size.x, size.y);
            }
        }
    }
    /// The window for choosing how wide a memo's title line is when folded
    /// once: a folded memo to stretch sideways. Closing it keeps its width.
    pub fn title_probe_window(&mut self, ctx: &egui::Context) {
        if !self.desktop.title_probe {
            return;
        }
        let builder = egui::ViewportBuilder::default()
            .with_title("1단 접기 크기 설정 · 너비를 바꾸고 닫으세요")
            .with_inner_size([self.desktop.title_width, 44.])
            .with_min_inner_size([TITLE_MIN, 44.])
            .with_max_inner_size([TITLE_MAX, 44.])
            .with_taskbar(false)
            .with_window_level(egui::WindowLevel::AlwaysOnTop);
        let fill = mix(self.desktop.next_new_color(), Color32::WHITE, 0.15);
        let mut closed = false;
        let mut seen = None;
        ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of("title_probe"),
            builder,
            |ctx, _| {
                self.theme(ctx);
                if ctx.input(|i| i.viewport().close_requested() || i.key_pressed(egui::Key::Escape))
                {
                    closed = true;
                }
                seen = ctx.input(|i| i.viewport().inner_rect.map(|r| r.width()));
                egui::CentralPanel::default()
                    .frame(
                        egui::Frame::NONE
                            .fill(fill)
                            .inner_margin(egui::Margin::symmetric(6, 5)),
                    )
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            // Looks like a folded memo: done circle, title.
                            let mut done = false;
                            crate::ui::round_check(ui, &mut done, None);
                            ui.add(
                                egui::Label::new(
                                    RichText::new(format!(
                                        "제목줄 너비 {} px",
                                        seen.unwrap_or(self.desktop.title_width).round()
                                    ))
                                    .color(INK),
                                )
                                .truncate(),
                            );
                        });
                    });
            },
        );
        if let Some(width) = seen {
            self.desktop.title_probe_last = Some(width);
        }
        if closed {
            self.desktop.title_probe = false;
            if let Some(width) = self.desktop.title_probe_last.take() {
                self.desktop.title_width = width.round().clamp(TITLE_MIN, TITLE_MAX);
                self.toast = format!(
                    "1단 접기 제목줄 너비를 {} px로 정했습니다",
                    self.desktop.title_width
                );
            }
        }
    }
    /// The window for setting the color box size: a box whose corner is
    /// dragged to the size wanted; closing the window keeps that size.
    pub fn swatch_probe_window(&mut self, ctx: &egui::Context) {
        if !self.desktop.swatch_probe {
            return;
        }
        let mut size = self
            .desktop
            .swatch_probe_last
            .unwrap_or(self.desktop.swatch_size);
        let fill = self.desktop.next_new_color();
        let mut open = true;
        let mut escape = false;
        crate::ui::dialog(
            ctx,
            "swatch_probe",
            "컬러박스 크기 설정",
            [460., 460.],
            true,
            &mut open,
            |ui| {
                escape = ui.input(|i| i.key_pressed(egui::Key::Escape));
                ui.label("상자의 오른쪽 아래 모서리를 끌어 크기를 바꾼 뒤 창을 닫으세요.");
                crate::ui::caption(
                    ui,
                    "닫으면 이 크기가 2단 접기(색상 상자)의 크기가 됩니다 (Esc로도 닫힙니다).",
                );
                ui.horizontal(|ui| {
                    for (label, v) in [("너비", &mut size.x), ("높이", &mut size.y)] {
                        ui.label(label);
                        ui.add(
                            egui::DragValue::new(v)
                                .range(SWATCH_MIN..=SWATCH_MAX)
                                .speed(1.)
                                .fixed_decimals(0)
                                .suffix(" px"),
                        );
                    }
                });
                ui.add_space(8.);
                egui::ScrollArea::both()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let (rect, _) =
                            ui.allocate_exact_size(size + Vec2::splat(12.), Sense::hover());
                        let rect = Rect::from_min_size(rect.min, size);
                        ui.painter().rect_filled(rect, 6., fill);
                        let grip = Rect::from_center_size(rect.max, Vec2::splat(18.));
                        let r = ui
                            .interact(grip, ui.id().with("swatch_grip"), Sense::drag())
                            .on_hover_cursor(egui::CursorIcon::ResizeNwSe);
                        if r.dragged() {
                            size += r.drag_delta();
                        }
                        let ink =
                            INK.gamma_multiply(if r.hovered() || r.dragged() { 0.9 } else { 0.5 });
                        for k in [4., 8.] {
                            ui.painter().line_segment(
                                [rect.max - Vec2::new(k, 1.), rect.max - Vec2::new(1., k)],
                                egui::Stroke::new(1.5_f32, ink),
                            );
                        }
                    });
            },
        );
        let size = size
            .round()
            .clamp(Vec2::splat(SWATCH_MIN), Vec2::splat(SWATCH_MAX));
        self.desktop.swatch_probe_last = Some(size);
        if !open || escape {
            self.desktop.swatch_probe = false;
            if let Some(s) = self.desktop.swatch_probe_last.take() {
                self.desktop.swatch_size = s;
                self.toast = format!("색상 상자 크기를 {} × {} px로 정했습니다", s.x, s.y);
            }
        }
    }
    pub fn finish_completed_stickers(&mut self, ctx: &egui::Context) {
        let completed = std::mem::take(&mut self.completed_stickers);
        if !self.prefs.close_completed_sticker {
            return;
        }
        let mut rearrange = false;
        for i in completed {
            if let Some(w) = self.studio.windows.get_mut(&i) {
                if w.open && matches!(w.mode, Mode::Sticker | Mode::Title) {
                    w.open = false;
                    ctx.send_viewport_cmd_to(viewport(i), egui::ViewportCommand::Close);
                    rearrange = true;
                }
            }
        }
        if rearrange && self.prefs.rearrange_completed_stickers {
            self.arrange = Some(Arrange {
                preserve_sticker_order: true,
                layout: Layout::Right,
                fold: Some(Mode::Title),
                area: self.arrange_area(ctx),
            });
        }
    }
    /// Apply a pending desktop arrangement to every open sticker window.
    pub fn apply_arrangement(&mut self, _ctx: &egui::Context) {
        let Some(a) = self.arrange.take() else {
            return;
        };
        let mut ids: Vec<_> = self
            .studio
            .windows
            .iter()
            .filter(|(&i, w)| {
                w.open
                    && !self.data.notes[i].deleted
                    && self.visible(i)
                    && (!a.preserve_sticker_order || matches!(w.mode, Mode::Sticker | Mode::Title))
            })
            .map(|(&i, _)| i)
            .collect();
        if a.preserve_sticker_order {
            ids.sort_by(|a, b| {
                let position = |i: &usize| {
                    self.studio.windows[i]
                        .place
                        .or(self.studio.windows[i].last_pos)
                        .unwrap_or(Pos2::ZERO)
                };
                let a = position(a);
                let b = position(b);
                a.y.total_cmp(&b.y).then_with(|| b.x.total_cmp(&a.x))
            });
        }
        let mut sizes = vec![];
        for &i in &ids {
            let locked = self.data.notes[i].locked;
            let w = self.studio.windows.get_mut(&i).unwrap();
            match a.fold {
                Some(Mode::Title) if !locked => w.mode = Mode::Title,
                Some(Mode::Swatch) if w.mode != Mode::Swatch => {
                    w.swatch_size = self.desktop.swatch_size;
                    w.mode = Mode::Swatch
                }
                Some(Mode::Sticker) if !locked => {
                    w.mode = if w.sticker_preferred {
                        Mode::Sticker
                    } else {
                        Mode::Editor
                    }
                }
                _ => {}
            }
            if w.mode == Mode::Swatch && self.desktop.unpin_minimized {
                self.data.notes[i].topmost = false;
            }
            sizes.push(w.outer_size(locked));
        }
        for (i, p) in ids.iter().zip(plan(a.layout, a.area, &sizes)) {
            let w = self.studio.windows.get_mut(i).unwrap();
            w.place = Some(p);
            w.center = false;
        }
        self.toast = format!("스티커 {}개를 정렬했습니다", ids.len());
    }
    /// Arrangement and window actions. Returns true once an item was used.
    /// Where stickers are arranged: the monitor chosen in the settings, or
    /// the one the current window is on.
    pub fn arrange_area(&self, ctx: &egui::Context) -> Rect {
        self.desktop
            .arrange_monitor
            .and_then(|m| crate::native_window::monitors(ctx).get(m).copied())
            .or_else(|| crate::native_window::work_area(ctx))
            .unwrap_or_else(|| ctx.screen_rect())
    }
    /// Once a day from 05:00, open unfinished past memos and today's memos as
    /// titles stacked on the right of the arrange monitor.
    pub fn daily_today(&mut self, ctx: &egui::Context) {
        if !self.desktop.daily_today {
            return;
        }
        let today = daily_day(daily_now());
        if self.desktop.daily_shown == Some(today) {
            return;
        }
        self.desktop.daily_shown = Some(today);
        let due = self.daily_due(today);
        if due.is_empty() {
            return;
        }
        for i in due {
            self.open_sticker(i, Some(today));
        }
        self.arrange = Some(Arrange {
            preserve_sticker_order: false,
            layout: Layout::Right,
            fold: Some(Mode::Title),
            area: self.arrange_area(ctx),
        });
    }
    /// Uncompleted memos of earlier days plus the memos of `today`.
    fn daily_due(&self, today: NaiveDate) -> Vec<usize> {
        self.data
            .notes
            .iter()
            .enumerate()
            .filter(|(i, n)| {
                self.visible(*i)
                    && !n.locked
                    && ((n.deadline_date().is_some() && !n.done)
                        || n.date.is_some_and(|d| {
                            (d < today && !n.done_on(Some(d)))
                                || n.occurs(today) && !n.done_on(Some(today))
                        }))
            })
            .map(|(i, _)| i)
            .collect()
    }
    /// Big numbered windows on every monitor; clicking one makes it the
    /// monitor stickers are arranged on.
    pub fn monitor_picker(&mut self, ctx: &egui::Context) {
        if !self.desktop.monitor_pick {
            return;
        }
        let monitors = crate::native_window::monitors(ctx);
        if monitors.len() < 2 {
            self.desktop.monitor_pick = false;
            return;
        }
        let mut picked = None;
        let mut cancel = false;
        for (k, area) in monitors.iter().enumerate() {
            let size = Vec2::new(360., 300.);
            let builder = egui::ViewportBuilder::default()
                .with_title(format!("모니터 {}", k + 1))
                .with_position(area.center() - size / 2.)
                .with_inner_size(size)
                .with_decorations(false)
                .with_resizable(false)
                .with_taskbar(false)
                .with_transparent(true)
                .with_window_level(egui::WindowLevel::AlwaysOnTop);
            ctx.show_viewport_immediate(
                egui::ViewportId::from_hash_of(("monitor_pick", k)),
                builder,
                |ctx, _| {
                    crate::win_style::shape(ctx, 0.78);
                    egui::CentralPanel::default()
                        .frame(egui::Frame::NONE.fill(Color32::WHITE))
                        .show(ctx, |ui| {
                            let rect = ui.max_rect();
                            let r = ui
                                .interact(rect, ui.id().with("pick"), Sense::click())
                                .on_hover_cursor(egui::CursorIcon::PointingHand);
                            ui.painter().text(
                                rect.center() - Vec2::new(0., 18.),
                                egui::Align2::CENTER_CENTER,
                                (k + 1).to_string(),
                                egui::FontId::proportional(170.),
                                if r.hovered() { ACCENT } else { INK },
                            );
                            ui.painter().text(
                                Pos2::new(rect.center().x, rect.bottom() - 34.),
                                egui::Align2::CENTER_CENTER,
                                "이 모니터에 스티커를 정렬하려면 누르세요",
                                egui::FontId::proportional(14.),
                                MUTED,
                            );
                            if r.clicked() {
                                picked = Some(k);
                            }
                            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                cancel = true;
                            }
                        });
                },
            );
        }
        if let Some(k) = picked {
            self.desktop.arrange_monitor = Some(k);
            self.desktop.monitor_pick = false;
            self.toast = format!("스티커 정렬 모니터를 {}번으로 정했습니다", k + 1);
        } else if cancel {
            self.desktop.monitor_pick = false;
        }
    }
    fn sticker_menu(
        &mut self,
        ui: &mut egui::Ui,
        i: usize,
        w: &mut Window,
        own: bool,
        sub2: &mut Option<(Layout, f32)>,
    ) -> bool {
        let ctx = ui.ctx().clone();
        let area = self.arrange_area(&ctx);
        let locked = self.data.notes[i].locked;
        ui.set_min_width(210.);
        let mut done = false;
        if own && w.mode != Mode::Sticker && w.mode != Mode::Editor {
            if locked {
                if ui.button("잠금 해제…").clicked() {
                    self.ask_unlock(i, true);
                    done = true;
                }
            } else if ui.button("스티커로 펼치기").clicked() {
                w.sticker();
                done = true;
            }
            ui.separator();
        }
        if own && !locked {
            if ui.button("글꼴…").clicked() {
                self.open_font_dialog(&ctx, i);
                done = true;
            }
            let mut opacity = self.data.notes[i].document.opacity;
            crate::composer::opacity_row(ui, &mut opacity);
            if opacity != self.data.notes[i].document.opacity {
                self.data.notes[i].document.opacity = opacity;
            }
            ui.separator();
        }
        crate::ui::caption(ui, "스티커 정렬");
        // "화면 오른쪽/왼쪽에 정렬" each open their own popup (모두 제목만
        // 보이게 / 모두 상자로 접기 / 모두펼치기, specific to that side)
        // instead of arranging immediately, mirroring how "스티커 정렬"
        // itself opens beside the 더보기 메뉴.
        let origin = ctx
            .input(|v| v.viewport().inner_rect.map(|r| r.min))
            .unwrap_or_default();
        for (label, layout) in [
            ("화면 오른쪽에 정렬", Layout::Right),
            ("화면 왼쪽에 정렬", Layout::Left),
        ] {
            let open = sub2.is_some_and(|(l, _)| l == layout);
            let r = submenu_row(ui, label, open);
            if r.hovered() || r.clicked() {
                *sub2 = Some((layout, origin.y + r.rect.top() - 6.));
            }
        }
        if ui.button("계단식으로 정렬").clicked() {
            self.arrange = Some(Arrange {
                preserve_sticker_order: false,
                layout: Layout::Cascade,
                fold: None,
                area,
            });
            done = true;
        }
        // "닫기" belongs to the sticker's own right-click menu, not to the
        // "더보기 › 스티커 정렬" submenu: that submenu is specifically about
        // arranging stickers, and repeating the close actions there just
        // duplicated what the main context menu already offers.
        if own {
            ui.separator();
            let mut close_locked = self.data.notes[i].close_locked;
            if menu_check(ui, &mut close_locked, "닫기 버튼 잠금")
                .on_hover_text(
                    "켜면 X 버튼으로 닫히지 않아요. 이 메뉴의 ‘이 스티커 닫기’로 닫아요.",
                )
                .changed()
            {
                self.data.notes[i].close_locked = close_locked;
            }
            if ui.button("이 스티커 닫기").clicked() {
                w.open = false;
                done = true;
            }
            if ui.button("모든 스티커 닫기  Ctrl+Shift+H").clicked() {
                w.open = false;
                for other in self.studio.windows.values_mut() {
                    other.open = false;
                }
                done = true;
            }
        }
        done
    }
    /// The popup beside "화면 오른쪽/왼쪽에 정렬": fold-mode options specific
    /// to that arrangement side.
    fn arrange_side_menu(&mut self, ui: &mut egui::Ui, layout: Layout) -> bool {
        let ctx = ui.ctx().clone();
        let area = self.arrange_area(&ctx);
        let mut done = false;
        for (label, fold) in [
            ("모두 제목만 보이게", Mode::Title),
            ("모두 상자로 접기", Mode::Swatch),
            ("모두펼치기", Mode::Sticker),
        ] {
            if ui.button(label).clicked() {
                self.arrange = Some(Arrange {
                    preserve_sticker_order: false,
                    layout,
                    fold: Some(fold),
                    area,
                });
                done = true;
            }
        }
        done
    }
    /// Shows the sticker menu as its own small window at the pointer, so it
    /// is never clipped by a title bar or a tiny color box.
    fn sticker_menu_window(&mut self, ctx: &egui::Context, i: usize, w: &mut Window) {
        let Some(mut m) = w.menu.take() else {
            return;
        };
        // A click back on the sticker itself dismisses the menu.
        if !std::mem::take(&mut m.fresh) && ctx.input(|v| v.pointer.any_pressed()) {
            return;
        }
        let area = crate::native_window::work_area(ctx).unwrap_or_else(|| ctx.screen_rect());
        let pos = fit_in(m.at, m.size, area);
        let mut sub = m.sub;
        let mut sub2 = m.sub2;
        let mut done = false;
        let paper = (m.kind == MenuKind::Linked).then_some(self.data.notes[i].color);
        let shown = menu_window(
            ctx,
            egui::ViewportId::from_hash_of(("sticker_menu", i)),
            pos,
            m.size,
            paper,
            |ui| match m.kind {
                MenuKind::Context => self.sticker_menu(ui, i, w, true, &mut sub2),
                MenuKind::More => self.more_items(ui, i, w, pos, &mut sub),
                MenuKind::Project => self.project_items(ui, i),
                MenuKind::Linked => self.linked_items(ui, i),
            },
        );
        m.size = shown.size;
        done |= shown.result == Some(true);
        let mut focused = shown.focused;
        let mut escape = shown.escape;
        // Rect of whichever window's "스티커 정렬" section is active, so the
        // level-3 arrange-side popup below can anchor beside the right one:
        // the Context menu's own window, or the 더보기 › 스티커 정렬 submenu.
        let mut arrange_host =
            (m.kind == MenuKind::Context).then(|| Rect::from_min_size(pos, m.size));
        if let Some((kind, top)) = sub {
            let parent = Rect::from_min_size(pos, m.size);
            let spos = submenu_pos(parent, m.sub_size, top, area);
            let shown = menu_window(
                ctx,
                egui::ViewportId::from_hash_of(("sticker_submenu", i)),
                spos,
                m.sub_size,
                None,
                |ui| match kind {
                    SubMenu::Project => self.project_items(ui, i),
                    SubMenu::Arrange => self.sticker_menu(ui, i, w, false, &mut sub2),
                },
            );
            m.sub_size = shown.size;
            done |= shown.result == Some(true);
            focused |= shown.focused;
            escape |= shown.escape;
            if kind == SubMenu::Arrange {
                arrange_host = Some(Rect::from_min_size(spos, m.sub_size));
            }
        }
        m.sub = sub;
        if let Some((layout, top)) = sub2 {
            if let Some(parent) = arrange_host {
                let spos = submenu_pos(parent, m.sub2_size, top, area);
                let shown = menu_window(
                    ctx,
                    egui::ViewportId::from_hash_of(("sticker_arrange_side", i)),
                    spos,
                    m.sub2_size,
                    None,
                    |ui| self.arrange_side_menu(ui, layout),
                );
                m.sub2_size = shown.size;
                done |= shown.result == Some(true);
                focused |= shown.focused;
                escape |= shown.escape;
            } else {
                sub2 = None;
            }
        }
        m.sub2 = sub2;
        // Focus moves between the menu and its submenu window; only close when
        // neither has had it for a moment.
        let now = ctx.input(|v| v.time);
        if focused {
            m.seen_focus = true;
            m.away_since = None;
        } else if m.seen_focus {
            let since = *m.away_since.get_or_insert(now);
            done |= now - since > 0.3;
        }
        if !(done || escape) {
            w.menu = Some(m);
        }
    }
    /// Opens the "글꼴" dialog for memo `i`, near the pointer, on what is
    /// selected in it now.
    pub fn open_font_dialog(&mut self, ctx: &egui::Context, i: usize) {
        let doc = &self.data.notes[i].document;
        let choice = doc.font_choice();
        let sample = choice.sample(doc);
        let pointer = ctx.input(|v| v.pointer.latest_pos()).unwrap_or_default();
        let at = crate::float::screen_origin(ctx) + pointer.to_vec2() + Vec2::new(12., 12.);
        let area = crate::native_window::work_area(ctx).unwrap_or_else(|| ctx.screen_rect());
        let at = fit_in(at, Vec2::from(FONT_DIALOG_SIZE) + Vec2::new(16., 40.), area);
        // Where `crate::ui::dialog` looks for the window's place.
        let vid = egui::ViewportId::from_hash_of(("dialog", FONT_DIALOG));
        ctx.data_mut(|d| d.insert_temp(egui::Id::new(("dialog_position", vid)), at));
        crate::ui::raise(ctx, FONT_DIALOG);
        self.studio.font_dialog = Some(FontDialog {
            note: i,
            choice,
            sample,
        });
    }
    /// The "글꼴" dialog: "확인" (or Enter) applies it to its memo; "취소",
    /// Escape or closing the window leaves the memo as it was.
    pub fn font_dialog_window(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.studio.font_dialog.take() else {
            return;
        };
        if !matches!(self.data.notes.get(d.note), Some(n) if !n.deleted && !n.locked) {
            return;
        }
        let mut open = true;
        let (mut apply, mut cancel) = (false, false);
        crate::ui::dialog(
            ctx,
            FONT_DIALOG,
            "글꼴",
            FONT_DIALOG_SIZE,
            true,
            &mut open,
            |ui| {
                crate::composer::font_panel(ui, &mut d.choice, &d.sample);
                let (ok, no) = crate::ui::dialog_buttons(ui, "확인", "취소");
                apply = ok || ui.input(|v| v.key_pressed(egui::Key::Enter));
                cancel = no || ui.input(|v| v.key_pressed(egui::Key::Escape));
            },
        );
        if apply {
            self.data.notes[d.note]
                .document
                .apply_font_choice(&d.choice);
            ctx.request_repaint_of(viewport(d.note));
        } else if open && !cancel {
            self.studio.font_dialog = Some(d);
        }
    }
    /// The memo's "더보기" items. Rows with a submenu open it beside this
    /// menu window instead of inside it.
    fn more_items(
        &mut self,
        ui: &mut egui::Ui,
        i: usize,
        w: &mut Window,
        origin: Pos2,
        sub: &mut Option<(SubMenu, f32)>,
    ) -> bool {
        let ctx = ui.ctx().clone();
        let mut done = false;
        let mut hovered_plain = false;
        let mut plain = |r: &egui::Response| {
            hovered_plain |= r.hovered();
            r.clicked()
        };
        if plain(&ui.button("일정 설정…")) {
            self.open_schedule(i);
            done = true;
        }
        if plain(&ui.button("알람 설정…")) {
            self.open_reminder(i);
            done = true;
        }
        if w.mode == Mode::Sticker && plain(&ui.button("메모 편집 창 열기")) {
            w.edit();
            done = true;
        }
        if plain(&ui.button("메모 잠금")) {
            self.ask_lock(i);
            done = true;
        }
        let mut wiki = self.data.notes[i].wiki;
        let r = menu_check(ui, &mut wiki, "위키에 보관");
        plain(&r);
        if r.changed() {
            self.data.notes[i].wiki = wiki;
        }
        for (label, kind) in [
            ("프로젝트 설정", SubMenu::Project),
            ("스티커 정렬", SubMenu::Arrange),
        ] {
            let open = sub.is_some_and(|(k, _)| k == kind);
            let r = submenu_row(ui, label, open);
            if r.hovered() || r.clicked() {
                *sub = Some((kind, origin.y + r.rect.top() - 6.));
            }
        }
        if plain(&ui.button("동일 프로젝트 보기")) {
            self.project_filter = Some(self.data.notes[i].project.unwrap_or(NO_PROJECT));
            self.page = 0;
            self.main_open = true;
            ctx.send_viewport_cmd_to(main_viewport(), egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd_to(main_viewport(), egui::ViewportCommand::Focus);
            done = true;
        }
        if plain(&ui.button("메모 복제본 생성")) {
            self.duplicate_note(i);
            done = true;
        }
        ui.separator();
        if plain(&ui.button("메모 삭제 · 휴지통")) {
            self.trash_note(i);
            w.open = false;
            done = true;
        }
        if plain(&ui.button(RichText::new("메모 영구삭제…").color(ACCENT))) {
            self.request_purge(i);
            done = true;
        }
        ui.separator();
        if plain(&ui.button("도움말")) {
            w.help = true;
            done = true;
        }
        if plain(&ui.button("메모창 설정")) {
            // The settings are a part of the main window: bring it forward.
            self.settings = true;
            self.desktop.settings_tab = 2;
            self.main_open = true;
            ctx.send_viewport_cmd_to(main_viewport(), egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd_to(main_viewport(), egui::ViewportCommand::Focus);
            done = true;
        }
        if hovered_plain {
            *sub = None;
        }
        done
    }
    /// Memos linked to `i`, each in a bar of its own color; a click opens it
    /// as a sticker.
    pub(crate) fn linked_items(&mut self, ui: &mut egui::Ui, i: usize) -> bool {
        let linked: Vec<_> = self
            .data
            .notes
            .iter()
            .enumerate()
            .filter(|(j, n)| {
                self.visible(*j)
                    && *j != i
                    && (n.parents.contains(&i)
                        || self.data.notes[i].parents.contains(j)
                        || self.data.links.contains(&(i.min(*j), i.max(*j))))
            })
            .map(|(j, _)| j)
            .collect();
        ui.horizontal(|ui| {
            ui.label(RichText::new("연결된 메모").strong().size(13.).color(INK));
            ui.label(
                RichText::new(linked.len().to_string())
                    .size(12.)
                    .color(MUTED),
            );
        });
        ui.add_space(4.);
        if linked.is_empty() {
            crate::ui::hint(ui, "연결된 메모가 없습니다");
            crate::ui::hint(ui, "전체메모 › 관계 보드에서 연결할 수 있습니다");
            ui.add_space(4.);
            return false;
        }
        let mut open = None;
        ui.spacing_mut().item_spacing.y = 6.;
        for j in linked {
            let n = &self.data.notes[j];
            let (rect, r) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 36.), Sense::click());
            let fill = if r.hovered() {
                mix(n.color, Color32::BLACK, 0.1)
            } else {
                n.color
            };
            ui.painter().rect(
                rect,
                8.,
                fill,
                egui::Stroke::new(
                    1.0_f32,
                    Color32::from_black_alpha(if r.hovered() { 40 } else { 22 }),
                ),
                egui::StrokeKind::Inside,
            );
            let title = if n.title.is_empty() {
                "새 메모"
            } else {
                &n.title
            };
            let text = if n.starred {
                format!("★  {title}")
            } else {
                title.to_string()
            };
            let painter = ui.painter().with_clip_rect(rect.shrink(4.));
            painter.text(
                Pos2::new(rect.left() + 12., rect.center().y),
                egui::Align2::LEFT_CENTER,
                text,
                egui::FontId::proportional(13.5),
                INK,
            );
            let r = r
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text("스티커로 열기");
            if r.clicked() {
                open = Some(j);
            }
        }
        if let Some(j) = open {
            self.open_sticker(j, self.data.notes[j].date);
        }
        open.is_some()
    }
    /// Pick the memo's project. Returns true once one is chosen.
    pub(crate) fn project_items(&mut self, ui: &mut egui::Ui, i: usize) -> bool {
        crate::ui::caption(ui, "프로젝트");
        let mut picked = false;
        let current = self.data.notes[i].project;
        let mut choices = vec![(None, "프로젝트 없음".to_string())];
        for (p, name) in self.data.projects.iter().enumerate() {
            if !self.extra.hidden_projects.contains(&p) && !self.extra.deleted_projects.contains(&p)
            {
                choices.push((Some(p), name.clone()));
            }
        }
        for (p, name) in choices {
            let label = if p.is_some() {
                format!("#  {name}")
            } else {
                name
            };
            if ui.selectable_label(current == p, label).clicked() {
                self.data.notes[i].project = p;
                picked = true;
            }
        }
        picked
    }
}

/// The day the daily memos belong to: a new day starts at 05:00.
fn daily_day(now: chrono::NaiveDateTime) -> NaiveDate {
    (now - chrono::Duration::hours(5)).date()
}
/// Local time for the daily memos. `OMNIMEMOJANG_TEST_NOW`
/// (`YYYY-MM-DD HH:MM:SS`) starts a test clock at that time instead of the
/// system clock, so the 05:00 rollover can be tried without changing Windows.
fn daily_now() -> chrono::NaiveDateTime {
    static START: std::sync::OnceLock<Option<(chrono::NaiveDateTime, std::time::Instant)>> =
        std::sync::OnceLock::new();
    let start = START.get_or_init(|| {
        let text = std::env::var("OMNIMEMOJANG_TEST_NOW").ok()?;
        let at = chrono::NaiveDateTime::parse_from_str(text.trim(), "%Y-%m-%d %H:%M:%S").ok()?;
        Some((at, std::time::Instant::now()))
    });
    match start {
        Some((at, since)) => *at + chrono::Duration::from_std(since.elapsed()).unwrap_or_default(),
        None => Local::now().naive_local(),
    }
}
/// Which menu a sticker shows in its menu window.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MenuKind {
    /// Right click: arrange and close stickers.
    Context,
    /// The "더보기" button.
    More,
    /// The footer's project picker.
    Project,
    /// The footer's linked memos, in the sticker's own color.
    Linked,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SubMenu {
    Project,
    Arrange,
}
/// A sticker menu shown in its own always-on-top window, so short stickers
/// never clip it; its submenu gets a second window beside it.
#[derive(Clone, Copy, Debug)]
pub struct Menu {
    kind: MenuKind,
    at: Pos2,
    size: Vec2,
    sub: Option<(SubMenu, f32)>,
    sub_size: Vec2,
    /// "화면 오른쪽/왼쪽에 정렬" popup, a third window beside whichever
    /// window's "스티커 정렬" section opened it (the Context menu itself, or
    /// the 더보기 › 스티커 정렬 submenu).
    sub2: Option<(Layout, f32)>,
    sub2_size: Vec2,
    seen_focus: bool,
    away_since: Option<f64>,
    /// Opened this frame: the click that opened it may still read as a press.
    fresh: bool,
}
impl Menu {
    fn new(kind: MenuKind, at: Pos2) -> Self {
        Self {
            kind,
            at,
            size: Vec2::new(228., 120.),
            sub: None,
            sub_size: Vec2::new(228., 120.),
            sub2: None,
            sub2_size: Vec2::new(200., 110.),
            seen_focus: false,
            away_since: None,
            fresh: true,
        }
    }
}
/// Toggle `kind` opening below `anchor` (a rect in the sticker's points).
fn toggle_menu(ctx: &egui::Context, w: &mut Window, kind: MenuKind, anchor: Rect) {
    if w.menu.is_some_and(|m| m.kind == kind) {
        w.menu = None;
        return;
    }
    let origin = ctx
        .input(|v| v.viewport().inner_rect.map(|r| r.min))
        .unwrap_or_default();
    w.menu = Some(Menu::new(
        kind,
        origin + anchor.left_bottom().to_vec2() + Vec2::new(0., 2.),
    ));
}
/// Keep a window of `size` at `at` inside `area`.
fn fit_in(at: Pos2, size: Vec2, area: Rect) -> Pos2 {
    Pos2::new(
        at.x.min(area.right() - size.x).max(area.left()),
        at.y.min(area.bottom() - size.y).max(area.top()),
    )
}
/// Place a submenu beside `parent`: on the right, or on the left when the
/// right side would leave the work area.
fn submenu_pos(parent: Rect, size: Vec2, top: f32, area: Rect) -> Pos2 {
    let right = parent.right() + 2.;
    let x = if right + size.x <= area.right() {
        right
    } else {
        (parent.left() - 2. - size.x).max(area.left())
    };
    fit_in(Pos2::new(x, top), size, area)
}
fn submenu_row(ui: &mut egui::Ui, label: &str, open: bool) -> egui::Response {
    let r = ui.add(egui::Button::new(label).selected(open));
    ui.painter().text(
        Pos2::new(r.rect.right() - 10., r.rect.center().y),
        egui::Align2::RIGHT_CENTER,
        "›",
        egui::FontId::proportional(15.),
        crate::ui::t(ui).muted,
    );
    r
}
pub(crate) struct Shown<R> {
    pub size: Vec2,
    pub focused: bool,
    pub escape: bool,
    pub result: Option<R>,
}
/// One borderless always-on-top menu window at `pos`; it resizes itself to
/// its content.
pub(crate) fn menu_window<R>(
    ctx: &egui::Context,
    id: egui::ViewportId,
    pos: Pos2,
    size: Vec2,
    paper: Option<Color32>,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> Shown<R> {
    // A borderless always-on-top window is placed by raw screen coordinates
    // with no OS-level clamping, so a menu opened near a monitor's edge
    // would otherwise open partly or fully off-screen and be unreachable.
    let area = crate::native_window::work_area(ctx).unwrap_or_else(|| ctx.screen_rect());
    let pos = fit_in(pos, size, area);
    let builder = egui::ViewportBuilder::default()
        .with_title("메모 메뉴")
        .with_position(pos)
        .with_inner_size(size)
        .with_decorations(false)
        .with_resizable(false)
        .with_taskbar(false)
        .with_transparent(true)
        .with_window_level(egui::WindowLevel::AlwaysOnTop);
    let mut shown = Shown {
        size,
        focused: false,
        escape: false,
        result: None,
    };
    let mut add = Some(add);
    ctx.show_viewport_immediate(id, builder, |ctx, _| {
        crate::tutorial::mark(ctx, crate::tutorial::Spot::Popup, ctx.screen_rect());
        shown.focused = ctx.input(|v| v.viewport().focused == Some(true));
        shown.escape = ctx.input(|v| v.key_pressed(egui::Key::Escape));
        crate::win_style::shape(ctx, 1.);
        let mut frame = egui::Frame::popup(&ctx.style())
            .shadow(egui::epaint::Shadow::NONE)
            .inner_margin(egui::Margin::same(6));
        if crate::win_style::NATIVE {
            frame = frame.corner_radius(0.);
        }
        if let Some(paper) = paper {
            frame = frame
                .fill(paper)
                .stroke(egui::Stroke::new(1.0_f32, Color32::from_black_alpha(28)));
        }
        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
            if let Some(paper) = paper {
                paper_style(ui, paper);
            }
            let v = &mut ui.style_mut().visuals.widgets;
            v.inactive.weak_bg_fill = Color32::TRANSPARENT;
            v.inactive.bg_fill = Color32::TRANSPARENT;
            ui.style_mut().spacing.button_padding = Vec2::new(10., 5.);
            ui.style_mut().spacing.item_spacing.y = 2.;
            let r = ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
                ui.set_width(size.x - 12.);
                add.take().map(|f| f(ui))
            });
            shown.result = r.inner;
            let want = Vec2::new(size.x, (r.response.rect.height() + 14.).ceil());
            if (want.y - size.y).abs() > 1. {
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(want));
            }
            shown.size = want;
        });
    });
    shown
}
fn open_menu(ctx: &egui::Context, w: &mut Window) {
    let (origin, pointer) = ctx.input(|v| {
        (
            v.viewport().inner_rect.map(|r| r.min),
            v.pointer.interact_pos(),
        )
    });
    if let (Some(origin), Some(pointer)) = (origin, pointer) {
        w.menu = Some(Menu::new(MenuKind::Context, origin + pointer.to_vec2()));
    }
}
pub fn viewport(i: usize) -> egui::ViewportId {
    egui::ViewportId::from_hash_of(("memo", i))
}
impl App {
    pub fn begin_sticker_drag(
        &mut self,
        ctx: &egui::Context,
        note: usize,
        mode: Mode,
        restore: bool,
    ) -> bool {
        self.drag = None;
        let (origin, grab) = ctx.input(|i| {
            let v = i.viewport();
            let origin = v.outer_rect.map_or(Pos2::ZERO, |r| r.min);
            let grab = if v.maximized == Some(true) {
                None
            } else {
                v.inner_rect
                    .zip(i.pointer.press_origin())
                    .map(|(inner, press)| inner.min - origin + press.to_vec2())
            };
            (origin, grab)
        });
        let carry = if crate::system::left_button_down() {
            crate::win_style::shaped_hwnd(ctx)
                .zip(grab)
                .map(|(hwnd, grab)| CarriedSticker {
                    hwnd,
                    grab: grab * ctx.pixels_per_point(),
                    pointer: None,
                    preview: None,
                })
        } else {
            None
        };
        let tracked = carry.is_some();
        self.studio.drag = Some(StickerDrag {
            shift: ctx.input(|v| v.modifiers.shift) || crate::system::shift_down(),
            note,
            origin,
            mode,
            always_restore: restore,
            started: ctx.input(|v| v.time),
            saw_down: tracked,
            carry,
        });
        ctx.request_repaint();
        tracked
    }

    /// Watches a sticker being carried: when the button is let go over a date
    /// of a calendar window, the memo gets that date and the sticker goes
    /// back to its place and size.
    pub fn finish_sticker_drag(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.studio.drag.take() else {
            return;
        };
        if !self.studio.windows.get(&d.note).is_some_and(|w| w.open) {
            return;
        }
        let now = ctx.input(|v| v.time);
        let at = crate::system::cursor_pos().map(|(x, y)| Pos2::new(x as f32, y as f32));
        if crate::system::left_button_down() {
            if let Some(carry) = &mut d.carry {
                carry.preview = at.and_then(|p| self.sticker_preview_at(p, now));
                if let Some(p) = at.filter(|p| Some(*p) != carry.pointer) {
                    crate::win_style::move_carried_sticker(carry.hwnd, p - carry.grab);
                }
                carry.pointer = at;
            }
            d.shift |= crate::system::shift_down();
            d.saw_down = true;
            let interval = if d.carry.is_some() { 16 } else { 30 };
            self.studio.drag = Some(d);
            ctx.request_repaint_after(std::time::Duration::from_millis(interval));
            return;
        }
        if !d.saw_down && now - d.started < 0.4 {
            // the press has not shown up in the key state yet
            self.studio.drag = Some(d);
            ctx.request_repaint_after(std::time::Duration::from_millis(30));
            return;
        }
        let target = at.and_then(|p| self.schedule_target_at(p, now));
        if target.is_none() && !d.always_restore {
            if let (Some(carry), Some(p)) = (&d.carry, at) {
                if Some(p) != carry.pointer {
                    crate::win_style::move_carried_sticker(carry.hwnd, p - carry.grab);
                }
            }
        }
        self.drop_sticker(&d, None);
        if let Some((date, calendar)) = target {
            self.assign_dropped_date(
                d.note,
                date,
                d.shift || crate::system::shift_down(),
                calendar,
                None,
            );
            if let Some(w) = self.studio.windows.get_mut(&d.note) {
                w.mode = d.mode;
                w.place = Some(d.origin);
            }
        }
    }
    /// The sticker was let go: over a calendar date it takes the date, and
    /// it goes back to its place when it was dropped on a date or was only
    /// borrowing the title-only look.
    pub fn drop_sticker(&mut self, d: &StickerDrag, date: Option<NaiveDate>) {
        if let Some(date) = date {
            if d.note < self.data.notes.len() {
                self.assign_dropped_date(d.note, date, false, true, None);
            }
        }
        if date.is_some() || d.always_restore {
            if let Some(w) = self.studio.windows.get_mut(&d.note) {
                w.mode = d.mode;
                w.place = Some(d.origin);
            }
        }
    }
    pub(crate) fn sticker_preview_at(
        &self,
        point: Pos2,
        now: f64,
    ) -> Option<crate::dragging::ScheduleHit> {
        self.schedule_hit_at(point, now).filter(|hit| hit.grid)
    }

    /// The calendar date shown under a screen point (physical pixels), if a calendar
    /// drew its grid there lately.
    #[cfg(test)]
    pub fn calendar_date_at(&self, p: Pos2, now: f64) -> Option<NaiveDate> {
        self.schedule_target_at(p, now)
            .filter(|(_, calendar)| *calendar)
            .map(|(date, _)| date)
    }
    pub fn new_sticker(&mut self) -> usize {
        let i = self.data.notes.len();
        let mut n = Note::new("", None, i);
        n.body.clear();
        n.color = self.desktop.take_new_color();
        n.document.font_name = self.desktop.note_font.clone();
        n.document.font_size = self.desktop.note_font_size * 4. / 3.;
        n.document.opacity = self.desktop.opacity / 100.;
        n.document.image_scale = [0.25, 0.5, 0.75, 1., 1.][self.desktop.image_scale];
        n.document.image_fit = self.desktop.image_scale == 4;
        // The caret starts in the body, so a quick memo can be typed at once.
        n.document.requested_selection = Some(0..0);
        self.data.notes.push(n);
        let mut w = Window::new(Mode::Sticker);
        w.sticker_size = self.desktop.sticker_size;
        self.studio.windows.insert(i, w);
        i
    }
    /// Whether memo `i` holds nothing: no title, no text, no picture or
    /// table in any body tab.
    pub fn is_blank(&self, i: usize) -> bool {
        let n = &self.data.notes[i];
        n.title.trim().is_empty()
            && n.body.trim().is_empty()
            && !n.has_image
            && n.document
                .tabs
                .iter()
                .all(|t| t.body.trim().is_empty() && t.blocks.is_empty())
    }
    /// A new sticker for calendar day `day` (the day's +, '+ 이 날짜에
    /// 스티커', a drag over days). Closed while still blank, it is erased for
    /// good ([`Self::drop_blank_drafts`]).
    pub fn new_sticker_on(&mut self, day: NaiveDate) -> usize {
        let i = self.new_sticker();
        self.data.notes[i].date = Some(day);
        if self.tutorial_range_active() {
            self.studio
                .windows
                .get_mut(&i)
                .unwrap()
                .center_for_tutorial(Vec2::ZERO);
        }
        self.drafts.push(i);
        i
    }
    /// Quitting closes a new memo that is still blank like its close button
    /// would: after a restart it would come back as an empty memo that no
    /// close erases any more.
    pub fn close_blank_drafts(&mut self) {
        for i in self.drafts.clone() {
            if i < self.data.notes.len() && self.is_blank(i) {
                if let Some(w) = self.studio.windows.get_mut(&i) {
                    w.open = false;
                }
            }
        }
        self.drop_blank_drafts();
    }
    /// A memo made from a calendar day or '+ 새 일정' ([`Self::drafts`])
    /// whose window was closed while still blank is erased for good (it
    /// never reaches the trash).
    pub fn drop_blank_drafts(&mut self) {
        let drafts = std::mem::take(&mut self.drafts);
        for i in drafts {
            if i >= self.data.notes.len() {
                continue;
            }
            if self.studio.windows.get(&i).is_some_and(|w| w.open) {
                self.drafts.push(i);
            } else if self.is_blank(i) && !self.data.notes[i].purged {
                self.studio.windows.remove(&i);
                self.purge_note(i);
                // No input comes with this, so the cached lists (calendar,
                // 내 작업실, …) are told to look again; else the erased memo
                // would linger there as '삭제된 메모'.
                crate::lists::CHANGES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                self.toast = "내용 없이 닫은 일정 메모를 지웠어요".into();
            }
        }
    }
    /// Stickers and folded memos (not memo editor windows) on screen.
    fn memo_windows(&self) -> Vec<isize> {
        self.studio
            .windows
            .values()
            .filter(|w| w.open && matches!(w.mode, Mode::Sticker | Mode::Title | Mode::Swatch))
            .filter_map(|w| w.hwnd)
            .collect()
    }
    /// On activation, group the open stickers immediately below the main window.
    pub fn request_main_memo_stack(&mut self, ctx: &egui::Context) {
        let (focused, clicked) = ctx.input(|i| (i.focused, i.pointer.any_pressed()));
        let was_focused = std::mem::replace(&mut self.studio.main_focused, focused);
        if !self.desktop.memo_front || !focused || (was_focused && !clicked) {
            return;
        }
        self.studio.stack_below_main = crate::win_style::hwnd(ctx);
    }

    pub fn stack_memos_for_main(&mut self) {
        if let Some(main) = self.studio.stack_below_main.take() {
            let memos = self.memo_windows();
            if self.main_open && !memos.is_empty() {
                crate::win_style::stack_below(main, &memos);
            }
        }
    }
    #[cfg(not(windows))]
    pub fn keep_open_stickers_on_desktop(&mut self, _: &egui::Context) {}

    #[cfg(windows)]
    pub fn keep_open_stickers_on_desktop(&mut self, ctx: &egui::Context) {
        use crate::win_style::*;

        let windows = self.memo_windows();
        if windows.is_empty() || !self.prefs.keep_stickers_on_desktop {
            self.studio.desktop_watch = None;
            self.studio.desktop_entered = None;
            self.studio.desktop_stickers.clear();
            for hwnd in self.studio.desktop_promoted.drain(..) {
                if windows.contains(&hwnd) {
                    release_desktop_sticker(hwnd);
                }
            }
            return;
        }
        self.studio
            .desktop_watch
            .get_or_insert_with(ForegroundWatch::start);
        if let Some(desktop) = desktop_foreground() {
            let now = ctx.input(|i| i.time);
            let entered = *self.studio.desktop_entered.get_or_insert(now);
            self.studio.desktop_promoted.retain(|h| windows.contains(h));
            self.studio.desktop_stickers.retain(|h| windows.contains(h));
            if desktop_is_raised(desktop, &windows) {
                for &hwnd in &self.studio.desktop_stickers {
                    if keep_desktop_sticker(desktop, hwnd)
                        && !self.studio.desktop_promoted.contains(&hwnd)
                    {
                        self.studio.desktop_promoted.push(hwnd);
                    }
                }
            }
            // Explorer can change the stacking order just after taking focus.
            if now - entered < 0.3 {
                ctx.request_repaint_after(std::time::Duration::from_millis(75));
            }
        } else {
            self.studio.desktop_entered = None;
            for hwnd in self.studio.desktop_promoted.drain(..) {
                if windows.contains(&hwnd) {
                    release_desktop_sticker(hwnd);
                }
            }
            self.studio.desktop_stickers = windows
                .into_iter()
                .filter(|&h| is_desktop_sticker_visible(h))
                .collect();
        }
    }

    pub fn render_note_windows(&mut self, root: &egui::Context) {
        self.drop_blank_drafts();
        crate::magnet::configure(self.desktop.magnet, self.desktop.magnet_pt);
        let mut rested = false;
        let ids: Vec<_> = self
            .studio
            .windows
            .iter()
            .filter(|(_, w)| w.open)
            .map(|(&i, _)| i)
            .collect();
        for i in ids {
            let Some(mut w) = self.studio.windows.remove(&i) else {
                continue;
            };
            if self.data.notes[i].deleted
                || (!self.visible(i) && !(self.extra.search_mode && self.extra.include_hidden))
            {
                w.open = false;
                self.studio.windows.insert(i, w);
                continue;
            }
            w.reconcile(&self.data.notes[i]);
            w.skip_title = self.desktop.skip_title;
            w.title_default = self.desktop.title_width;
            w.skip_swatch = self.desktop.skip_swatch && !self.desktop.skip_title;
            w.swatch_default = self.desktop.swatch_size;
            let locked = self.data.notes[i].locked;
            if locked {
                w.mode = Mode::Swatch;
            }
            if self.data.notes[i].duration > 1 && w.occurrence.is_none() {
                let today = Local::now().date_naive();
                w.occurrence = if self.data.notes[i].occurs(today) {
                    Some(today)
                } else {
                    self.data.notes[i].date
                };
            }
            let previous_active = self.active;
            let previous_occurrence = self.extra.edit_occurrence;
            self.active = i;
            self.extra.edit_occurrence = w.occurrence;
            let mode = w.mode;
            let key = (mode, locked);
            let size = if locked { Vec2::splat(64.) } else { w.size() };
            let min = if locked {
                size
            } else {
                match mode {
                    Mode::Editor => Vec2::new(480., 480.),
                    Mode::Sticker => STICKER_MIN,
                    Mode::Title => Vec2::new(160., 40.),
                    Mode::Swatch => Vec2::splat(16.),
                }
            };
            let max = if locked {
                size
            } else {
                match mode {
                    Mode::Title => Vec2::new(1600., 40.),
                    Mode::Swatch => Vec2::splat(640.),
                    _ => Vec2::splat(8192.),
                }
            };
            let top = self.data.notes[i].topmost
                && !(mode == Mode::Swatch && self.desktop.unpin_minimized);
            let title = if self.data.notes[i].title.is_empty() {
                "새 메모"
            } else {
                &self.data.notes[i].title
            };
            let mut builder = egui::ViewportBuilder::default()
                .with_title(title)
                .with_min_inner_size(min)
                .with_max_inner_size(max)
                .with_resizable(!locked && mode != Mode::Swatch)
                .with_transparent(true)
                .with_decorations(false)
                .with_taskbar(self.desktop.taskbar)
                .with_icon(crate::app_icon::window())
                .with_window_level(if top {
                    egui::WindowLevel::AlwaysOnTop
                } else {
                    egui::WindowLevel::Normal
                });
            // The builder is a creation request, never a live geometry feedback loop.
            if w.applied.is_none() {
                builder = builder.with_inner_size(size).with_active(w.focus);
                // A reopened window is created where it goes, not first at
                // the system's default spot.
                if let Some(p) = w.place {
                    builder = builder.with_position(p);
                }
            }
            root.show_viewport_immediate(viewport(i), builder, |ctx, _| {
                let now = ctx.input(|v| v.time);
                let changes = crate::lists::CHANGES.load(std::sync::atomic::Ordering::Relaxed);
                if may_rest(self.quiet(ctx, i, &w, key, changes, now)) {
                    let rest = egui::Id::new("eframe_rest").with(ctx.viewport_id());
                    ctx.data_mut(|d| d.insert_temp(rest, true));
                    rested = true;
                    return;
                }
                let hover_key = egui::Id::new("eframe_stable_hover_rects").with(ctx.viewport_id());
                ctx.data_mut(|d| d.remove::<std::sync::Arc<Vec<Rect>>>(hover_key));
                w.drawn_at = now;
                w.drawn_changes = changes;
                crate::tutorial::mark(ctx, crate::tutorial::Spot::Note(i), ctx.screen_rect());
                self.theme(ctx);
                let serial = self.schedule_drop_serial;
                self.drag_input(ctx);
                self.studio.raw_date_drop =
                    (serial != self.schedule_drop_serial).then_some((i, ctx.input(|v| v.time)));
                if w.move_caret_to_body {
                    w.move_caret_to_body = false;
                    self.data.notes[i].document.requested_selection = Some(0..0);
                }
                let maximized = ctx.input(|v| v.viewport().maximized.unwrap_or(false));
                let changed = w.applied != Some(key);
                if changed {
                    w.native.cancel();
                    if maximized {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(false));
                    }
                    ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(min));
                    ctx.send_viewport_cmd(egui::ViewportCommand::MaxInnerSize(max));
                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Resizable(
                        !locked && mode != Mode::Swatch,
                    ));
                    w.applied = Some(key);
                } else if !maximized {
                    if let Some(rect) = ctx.input(|v| v.viewport().inner_rect) {
                        if rect.width() >= min.x && rect.height() >= min.y {
                            match mode {
                                Mode::Editor => w.full_size = rect.size(),
                                Mode::Sticker => w.sticker_size = rect.size(),
                                Mode::Title => w.title_width = rect.width(),
                                Mode::Swatch => w.swatch_size = rect.size(),
                            }
                        }
                    }
                }
                let tutorial_center = w.tutorial_center.take();
                let reopen_position = if std::mem::take(&mut w.reopening) && mode == Mode::Sticker {
                    self.prefs.sticker_reopen_position
                } else {
                    0
                };
                if reopen_position != 0 {
                    w.place = None;
                    w.fit_place = false;
                }
                if changed || w.center || reopen_position != 0 {
                    // A new window opens on the monitor chosen in the settings.
                    let chosen = if tutorial_center.is_some() {
                        crate::native_window::work_area_of(ctx, main_viewport())
                    } else if w.center || reopen_position != 0 {
                        self.desktop
                            .new_monitor
                            .and_then(|m| crate::native_window::monitors(ctx).get(m).copied())
                    } else {
                        None
                    };
                    if let Some(area) = chosen.or_else(|| crate::native_window::work_area(ctx)) {
                        let position = if let Some(offset) = tutorial_center {
                            area.center() - size * 0.5 + offset
                        } else if matches!(reopen_position, 1 | 2)
                            || (reopen_position == 0 && w.center && self.desktop.new_right)
                        {
                            let others: Vec<Rect> = self
                                .studio
                                .windows
                                .iter()
                                .filter(|(_, o)| o.open)
                                .filter_map(|(&n, o)| {
                                    let at = o.place.or(o.last_pos)?;
                                    let locked = self.data.notes.get(n).is_some_and(|n| n.locked);
                                    Some(Rect::from_min_size(at, o.outer_size(locked)))
                                })
                                .collect();
                            if reopen_position == 2 {
                                let mirror = |x| area.left() + area.right() - x;
                                let mirrored: Vec<_> = others
                                    .iter()
                                    .map(|r| {
                                        Rect::from_min_size(
                                            Pos2::new(mirror(r.right()), r.top()),
                                            r.size(),
                                        )
                                    })
                                    .collect();
                                let right = right_slot(area, size, &mirrored);
                                Pos2::new(mirror(right.x + size.x), right.y)
                            } else {
                                right_slot(area, size, &others)
                            }
                        } else if reopen_position == 3 {
                            area.center() - size * 0.5
                        } else if w.center {
                            // New windows step down-right so they do not
                            // cover the ones already open.
                            let open = self.studio.windows.values().filter(|o| o.open).count();
                            area.center() - size * 0.5 + Vec2::splat(28. * (open % 8) as f32)
                        } else {
                            ctx.input(|v| v.viewport().outer_rect.map(|r| r.min))
                                .unwrap_or(area.min)
                        };
                        let position = crate::native_window::fit_position(position, size, area);
                        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(position));
                        if w.center || reopen_position != 0 {
                            // Seen by the windows opened after this one.
                            w.last_pos = Some(position);
                        }
                    } else if w.center {
                        if let Some(cmd) = egui::ViewportCommand::center_on_screen(ctx) {
                            ctx.send_viewport_cmd(cmd);
                        }
                    }
                    w.center = false;
                }
                if let Some(mut p) = w.place.take() {
                    // A window saved on a monitor that is gone comes back on
                    // the current one.
                    let monitors = crate::native_window::monitors(ctx);
                    let grip = p + Vec2::new(40., 12.);
                    if !monitors.is_empty() && !monitors.iter().any(|m| m.contains(grip)) {
                        if let Some(area) = crate::native_window::work_area(ctx) {
                            p = crate::native_window::fit_position(p, size, area);
                        }
                    } else if w.fit_place {
                        if let Some(m) = monitors.iter().find(|m| m.contains(grip)) {
                            p = crate::native_window::fit_position(p, size, *m);
                        }
                    }
                    w.fit_place = false;
                    ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(p));
                }
                if w.focus {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                    w.focus = false;
                }
                if ctx.input(|v| v.viewport().close_requested()) {
                    if self.data.notes[i].close_locked {
                        // Alt+F4 is a close button too: only the menu closes it.
                        ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                        self.data.notes[i].document.status = CLOSE_LOCKED.into();
                    } else {
                        w.open = false;
                    }
                }
                if let Some(p) = ctx.input(|v| v.viewport().outer_rect.map(|r| r.min)) {
                    w.last_pos = Some(p);
                }
                self.memo_shortcuts(ctx, i, &mut w);
                // The color box is resized by its own handle: Windows enforces a
                // minimum tracking width on natively resizable windows, which
                // stopped boxes from shrinking below a wide rectangle.
                // While a sticker is being worked in (focused or under the
                // pointer) its resize band is wider, so a press near its edge
                // is the sticker's, not whatever lies behind it.
                let working = ctx.input(|v| {
                    v.viewport().focused.unwrap_or(false) || v.pointer.hover_pos().is_some()
                });
                w.native.input(
                    ctx,
                    mode == Mode::Title,
                    false,
                    !locked && !maximized && mode != Mode::Swatch,
                    min,
                    max,
                    if working { 11. } else { 7. },
                );
                let base = self.data.notes[i].color;
                let opacity = self.data.notes[i].document.opacity.clamp(0.2, 1.);
                // On Windows the OS applies opacity and rounds the corners, so
                // the paper is painted opaque and square (see `win_style`).
                let showing_calendar_bar = self.studio.drag.as_ref().is_some_and(|drag| {
                    drag.note == i
                        && drag
                            .carry
                            .as_ref()
                            .is_some_and(|carry| carry.preview.is_some())
                });
                if std::mem::replace(&mut w.calendar_bar, showing_calendar_bar)
                    != showing_calendar_bar
                {
                    // A change in layering may discard the previous GL frame.
                    let paint_once = egui::Id::new("eframe_paint_once").with(ctx.viewport_id());
                    ctx.data_mut(|data| data.insert_temp(paint_once, true));
                }
                crate::win_style::shape_memo(
                    ctx,
                    if showing_calendar_bar { 0. } else { opacity },
                    mode != Mode::Editor,
                );
                if let Some(h) = crate::win_style::shaped_hwnd(ctx) {
                    crate::magnet::track(h, matches!(mode, Mode::Title | Mode::Swatch));
                    w.hwnd = Some(h);
                }
                let alpha = if crate::win_style::NATIVE {
                    1.
                } else {
                    opacity
                };
                let radius: u8 = if crate::win_style::NATIVE { 0 } else { 12 };
                let paper = translucent(base, alpha);
                if mode == Mode::Title {
                    egui::CentralPanel::default()
                        .frame(
                            egui::Frame::NONE
                                .fill(translucent(mix(base, Color32::WHITE, 0.15), alpha))
                                .corner_radius(radius.min(8))
                                .inner_margin(egui::Margin::symmetric(6, 5)),
                        )
                        .show(ctx, |ui| {
                            paper_style(ui, paper);
                            // Tooltips cannot fit in a 44 px tall window: egui kept
                            // flipping them above and below the pointer (flicker).
                            ui.style_mut().interaction.tooltip_delay = f32::INFINITY;
                            let hover = !self.desktop.auto_toolbar
                                || ui.rect_contains_pointer(ui.max_rect());
                            let button = 28. + ui.spacing().item_spacing.x;
                            ui.horizontal(|ui| {
                                // Done at the left end; the space of the right
                                // end's buttons is always kept, so neither they
                                // nor the title move when they appear on hover.
                                let mut done = self.data.notes[i].done_on(w.occurrence);
                                if crate::ui::round_check(ui, &mut done, None).changed() {
                                    self.done_at(i, w.occurrence, done);
                                    self.extra.scope_owner = ctx.viewport_id();
                                }
                                let starred = self.data.notes[i].starred;
                                if starred {
                                    ui.label(RichText::new("★").color(STAR).size(15.));
                                }
                                // No fold button with the color box left out.
                                let buttons = if w.skip_swatch { 1. } else { 2. };
                                let width = (ui.available_width() - buttons * button).max(40.);
                                let r = ui.add_sized(
                                    [width, 26.],
                                    egui::Label::new({
                                        let n = &self.data.notes[i];
                                        let title = if n.title.is_empty() {
                                            "새 메모"
                                        } else {
                                            &n.title
                                        };
                                        if n.deadline {
                                            format!(
                                                "[{}] {}",
                                                n.dday(Local::now().date_naive()),
                                                title
                                            )
                                        } else {
                                            title.to_owned()
                                        }
                                    })
                                    .truncate()
                                    .sense(Sense::click_and_drag()),
                                );
                                self.drag_source(ui, &r, i, false);
                                if r.clicked() {
                                    w.sticker();
                                } else if (r.drag_started() || r.drag_stopped())
                                    && self.studio.raw_date_drop != Some((i, ctx.input(|v| v.time)))
                                {
                                    if r.drag_started()
                                        && self.begin_sticker_drag(ctx, i, Mode::Title, false)
                                    {
                                        w.native.cancel();
                                    } else {
                                        w.native.start_drag(ctx);
                                    }
                                }
                                if r.secondary_clicked() {
                                    open_menu(ctx, &mut w);
                                }
                                if hover {
                                    if !w.skip_swatch
                                        && icon_button(ui, Icon::Fold, "색상 상자로 접기").clicked()
                                    {
                                        w.fold();
                                    }
                                    pin_button(ui, &mut self.data.notes[i].topmost);
                                }
                            });
                        });
                } else if mode == Mode::Swatch {
                    egui::CentralPanel::default()
                        .frame(
                            egui::Frame::NONE
                                .fill(translucent(base, alpha))
                                .corner_radius(radius.min(8)),
                        )
                        .show(ctx, |ui| {
                            let rect = ui.max_rect();
                            if !locked {
                                swatch_resize(ctx, ui, &mut w);
                            }
                            let r = ui.interact(
                                rect.shrink(7.),
                                ui.id().with("swatch"),
                                Sense::click_and_drag(),
                            );
                            if locked {
                                crate::icons::paint_icon(
                                    ui.painter(),
                                    Rect::from_center_size(rect.center(), Vec2::splat(28.)),
                                    Icon::Lock,
                                    INK,
                                );
                            }
                            if r.drag_started()
                                && self.begin_sticker_drag(ctx, i, Mode::Swatch, false)
                            {
                                w.native.cancel();
                            } else if r.drag_started() || r.drag_stopped() {
                                w.native.start_drag(ctx);
                            }
                            // A click (no movement) unfolds; moving the pointer
                            // first turns the press into a window drag instead.
                            if r.clicked() {
                                if locked {
                                    self.ask_unlock(i, true);
                                } else {
                                    w.sticker();
                                }
                            }
                            if r.on_hover_cursor(egui::CursorIcon::Grab)
                                .secondary_clicked()
                            {
                                open_menu(ctx, &mut w);
                            }
                        });
                } else {
                    let focused = ctx.input(|v| v.viewport().focused.unwrap_or(false));
                    // A footer popup (font, size, color, opacity) is a window of
                    // its own: the toolbar stays while one is open.
                    let menu_open = crate::float::was_open_in(ctx);
                    let engaged = mode == Mode::Editor
                        || !self.desktop.auto_toolbar
                        || focused
                        || menu_open
                        || ctx.input(|v| v.pointer.hover_pos().is_some());
                    egui::TopBottomPanel::top(egui::Id::new(("memo_header", i)))
                        .exact_height(38.)
                        .show_separator_line(false)
                        .frame(
                            egui::Frame::NONE
                                .fill(paper)
                                .corner_radius(egui::CornerRadius {
                                    nw: radius,
                                    ne: radius,
                                    sw: 0,
                                    se: 0,
                                })
                                .inner_margin(egui::Margin::symmetric(10, 4)),
                        )
                        .show(ctx, |ui| {
                            paper_style(ui, paper);
                            self.memo_header(ui, i, &mut w, mode, engaged, maximized);
                        });
                    if self.active != i {
                        self.active = i;
                        self.extra.edit_occurrence = w.occurrence;
                    }
                    egui::TopBottomPanel::top(egui::Id::new(("memo_title", i)))
                        .show_separator_line(false)
                        .frame(egui::Frame::NONE.fill(paper).inner_margin(
                            if mode == Mode::Sticker {
                                egui::Margin {
                                    left: 14,
                                    right: 14,
                                    top: 0,
                                    bottom: 2,
                                }
                            } else {
                                egui::Margin {
                                    left: 18,
                                    right: 18,
                                    top: 4,
                                    bottom: 6,
                                }
                            },
                        ))
                        .show(ctx, |ui| {
                            paper_style(ui, paper);
                            if mode == Mode::Sticker {
                                let font = egui::FontId::new(
                                    18.,
                                    crate::desktop::family(
                                        ui,
                                        &self.data.notes[i].document.font_name,
                                    ),
                                );
                                ui.horizontal(|ui| {
                                    if self.data.notes[i].starred {
                                        ui.label(RichText::new("★").color(STAR).size(17.))
                                            .on_hover_text("중요 메모");
                                    }
                                    let title = ui.add(
                                        egui::TextEdit::singleline(&mut self.data.notes[i].title)
                                            .font(font)
                                            .frame(false)
                                            .desired_width(f32::INFINITY)
                                            .hint_text("제목"),
                                    );
                                    if title.secondary_clicked() {
                                        open_menu(ctx, &mut w);
                                    }
                                    if title.lost_focus()
                                        && ui.input(|v| v.key_pressed(egui::Key::Enter))
                                    {
                                        // Deferred to next frame; see
                                        // `move_caret_to_body`'s doc comment.
                                        w.move_caret_to_body = true;
                                    }
                                });
                            } else {
                                self.note_editor(ui);
                            }
                        });
                    let width = ctx
                        .input(|v| v.viewport().inner_rect.map(|r| r.width()))
                        .unwrap_or(size.x);
                    let status_height = if self.data.notes[i].document.status.is_empty() {
                        0.
                    } else {
                        let small_font = egui::TextStyle::Small.resolve(&ctx.style());
                        ctx.fonts(|f| f.row_height(&small_font)) + 4.
                    };
                    // The tab row (40) and, in the editor window, the format
                    // buttons in as many rows as the width needs.
                    let tools = footer_tools(mode, engaged);
                    let footer_height = (if tools {
                        44. + crate::composer::footer_tools_height(width - 18.)
                    } else {
                        40.
                    }) + status_height;
                    egui::TopBottomPanel::bottom(egui::Id::new(("memo_footer", i)))
                        .exact_height(footer_height)
                        .show_separator_line(false)
                        .frame(
                            egui::Frame::NONE
                                .fill(paper)
                                .corner_radius(egui::CornerRadius {
                                    nw: 0,
                                    ne: 0,
                                    sw: radius,
                                    se: radius,
                                })
                                .inner_margin(egui::Margin {
                                    left: 10,
                                    right: 8,
                                    top: 2,
                                    bottom: 6,
                                }),
                        )
                        .show(ctx, |ui| {
                            paper_style(ui, paper);
                            self.data.notes[i].document.toolbar_visible = tools;
                            let rect = ui.max_rect();
                            crate::composer::footer(
                                ui,
                                &mut self.data.notes[i].document,
                                ("memo_document", i),
                                if engaged {
                                    CORNER_ICONS_W + CORNER_ICONS_GAP
                                } else {
                                    0.
                                },
                                mode == Mode::Editor,
                            );
                            if engaged {
                                // On the tab row, kept clear of the corner where
                                // the window is resized.
                                let arrows = Rect::from_min_size(
                                    Pos2::new(
                                        rect.right() - CORNER_ICONS_W - CORNER_ICONS_GAP,
                                        rect.bottom() - status_height - 28.,
                                    ),
                                    Vec2::new(CORNER_ICONS_W, 28.),
                                );
                                ui.allocate_new_ui(egui::UiBuilder::new().max_rect(arrows), |ui| {
                                    ui.horizontal(|ui| {
                                        let r = icon_button(ui, Icon::ChevronDown, "프로젝트 선택");
                                        if r.clicked() {
                                            toggle_menu(ctx, &mut w, MenuKind::Project, r.rect);
                                        }
                                        let r = icon_button(ui, Icon::ChevronRight, "연결된 메모");
                                        if r.clicked() {
                                            toggle_menu(ctx, &mut w, MenuKind::Linked, r.rect);
                                        }
                                    });
                                });
                            }
                        });
                    egui::CentralPanel::default()
                        .frame(egui::Frame::NONE.fill(paper).inner_margin(
                            if mode == Mode::Sticker {
                                egui::Margin::symmetric(14, 2)
                            } else {
                                egui::Margin::symmetric(18, 4)
                            },
                        ))
                        .show(ctx, |ui| {
                            paper_style(ui, paper);
                            let style = egui::Id::new(crate::composer::HOST_STYLE);
                            ui.data_mut(|d| d.insert_temp(style, true));
                            if mode == Mode::Sticker {
                                let n = &mut self.data.notes[i];
                                let host = egui::Id::new(crate::composer::HOST_MENU);
                                ui.data_mut(|d| d.insert_temp(host, true));
                                crate::composer::content(
                                    ui,
                                    &mut n.body,
                                    &mut n.document,
                                    ("memo_document", i),
                                );
                                ui.data_mut(|d| d.remove::<bool>(host));
                            } else {
                                egui::Frame::NONE
                                    .fill(Color32::from_white_alpha(70))
                                    .stroke(egui::Stroke::new(
                                        1.0_f32,
                                        Color32::from_black_alpha(30),
                                    ))
                                    .corner_radius(8.)
                                    .inner_margin(egui::Margin::symmetric(12, 8))
                                    .show(ui, |ui| {
                                        ui.set_min_size(ui.available_size());
                                        let n = &mut self.data.notes[i];
                                        crate::composer::content(
                                            ui,
                                            &mut n.body,
                                            &mut n.document,
                                            ("memo_document", i),
                                        );
                                    });
                            }
                            ui.data_mut(|d| d.remove::<bool>(style));
                            let action = egui::Id::new(crate::composer::HOST_ACTION);
                            let picked = ui.data_mut(|d| {
                                let p = d.get_temp::<crate::composer::HostAction>(action);
                                d.remove::<crate::composer::HostAction>(action);
                                p
                            });
                            match picked {
                                Some(crate::composer::HostAction::Schedule) => {
                                    self.open_schedule(i)
                                }
                                Some(crate::composer::HostAction::Alarm) => self.open_reminder(i),
                                Some(crate::composer::HostAction::StickerMenu) => {
                                    open_menu(ctx, &mut w)
                                }
                                Some(crate::composer::HostAction::Trash) => {
                                    self.trash_note(i);
                                    w.open = false;
                                }
                                Some(crate::composer::HostAction::Purge) => self.request_purge(i),
                                Some(crate::composer::HostAction::Font) => {
                                    self.open_font_dialog(ctx, i)
                                }
                                None => {}
                            }
                        });
                    self.consume_body_project(i);
                    if mode == Mode::Sticker
                        && self.data.notes[i].document.active == 0
                        && (!focused || self.data.notes[i].body.contains('\n'))
                    {
                        let n = &mut self.data.notes[i];
                        crate::desktop::infer_title(&mut n.title, &n.body);
                    }
                }
                if mode != Mode::Editor {
                    let edge_radius = if crate::win_style::NATIVE || mode != Mode::Sticker {
                        8
                    } else {
                        radius
                    };
                    paint_sticker_edge(ctx, base, edge_radius);
                }
                if w.mode != mode || !w.open {
                    w.native.cancel();
                    w.help = false;
                    w.menu = None;
                    if w.mode == Mode::Swatch && self.desktop.unpin_minimized {
                        self.data.notes[i].topmost = false;
                    }
                    if self
                        .extra
                        .picker
                        .as_ref()
                        .is_some_and(|p| p.owner == ctx.viewport_id())
                    {
                        self.extra.picker = None;
                    }
                    if self.extra.scope_owner == ctx.viewport_id() {
                        self.extra.completion_scope = None;
                    }
                }
                if self.active != i {
                    self.active = i;
                    self.extra.edit_occurrence = w.occurrence;
                }
                self.picker_window(ctx);
                self.scope_dialog(ctx);
                self.sticker_menu_window(ctx, i, &mut w);
                let mut help = w.help;
                crate::ui::dialog(
                    ctx,
                    ("memo_help", i),
                    "메모 도움말",
                    [340., 300.],
                    true,
                    &mut help,
                    |ui| {
                        for line in [
                            "윗부분을 끌어 이동하고 가장자리로 크기를 조절합니다.",
                            "접기: 제목 한 줄 → 색상 상자 · 상자를 누르면 복원",
                            "빈 곳을 우클릭하면 스티커 정렬 메뉴가 열립니다.",
                            "이미지와 표는 Ctrl+V로 본문에 붙여 넣습니다.",
                            "Esc를 두 번 누르면 창을 닫습니다. 메모는 유지됩니다.",
                        ] {
                            ui.label(format!("·  {line}"));
                        }
                        ui.add_space(8.);
                        crate::ui::hint(ui, format!("새 스티커 · {}", self.shortcut_label(2)));
                    },
                );
                w.help = help;
            });
            if self.active == i {
                w.occurrence = self.extra.edit_occurrence;
            }
            self.active = previous_active;
            self.extra.edit_occurrence = previous_occurrence;
            self.studio.windows.insert(i, w);
        }
        if rested {
            // Once the pointer stops, every window is built once more.
            root.request_repaint_after(std::time::Duration::from_secs_f64(REST_MAX + 0.05));
        }
        self.editor = self
            .studio
            .windows
            .values()
            .any(|w| w.open && w.mode == Mode::Editor);
        self.sticky = self.studio.windows.values().any(|w| w.open);
    }
    /// Input, a menu or a drag in the window `ctx` draws: nothing it shows
    /// may be skipped this frame.
    fn window_busy(&self, ctx: &egui::Context) -> bool {
        let here = ctx.viewport_id();
        ctx.input(|v| {
            !v.events.is_empty()
                || !v.raw.dropped_files.is_empty()
                || !v.raw.hovered_files.is_empty()
                || v.pointer.hover_pos().is_some()
                || v.pointer.any_down()
                || v.viewport().close_requested()
        }) || crate::tutorial::active()
            || ctx.memory(|m| m.focused().is_some() || m.any_popup_open())
            || crate::float::shown_in(ctx)
            // A floating area (the picture toolbar, a toast) left out of one
            // frame fades in again: it keeps its window built.
            || floating_area_shown(ctx)
            || self.drag.is_some()
            || self.studio.drag.is_some()
            || self.extra.picker.as_ref().is_some_and(|p| p.owner == here)
            || (self.extra.scope_owner == here && self.extra.completion_scope.is_some())
    }
    /// The pointer is well inside an open memo window (so it is in front
    /// of the main window there; the main window had the pointer otherwise).
    fn pointer_inside_a_memo(&self, ctx: &egui::Context) -> bool {
        let Some((x, y)) = crate::system::cursor_pos() else {
            return false;
        };
        let p = Pos2::new(x as f32, y as f32);
        ctx.input(|v| {
            self.studio
                .windows
                .iter()
                .filter(|(_, w)| w.open)
                .any(|(&i, _)| {
                    v.raw.viewports.get(&viewport(i)).is_some_and(|info| {
                        match (info.outer_rect, info.native_pixels_per_point) {
                            (Some(r), Some(ppp)) => Rect::from_min_max(r.min * ppp, r.max * ppp)
                                .shrink(MAIN_REST_INSET * ppp)
                                .contains(p),
                            _ => false,
                        }
                    })
                })
        })
    }
    /// The facts [`may_rest`] decides on, for the main window this frame.
    pub(crate) fn main_quiet(&self, ctx: &egui::Context, changes: u64, now: f64) -> Quiet {
        let (drawn_at, drawn_changes) = self.studio.main_drawn.unwrap_or((f64::NEG_INFINITY, 0));
        Quiet {
            busy: self.window_busy(ctx),
            pointer_near: !self.pointer_inside_a_memo(ctx),
            changed: changes != drawn_changes,
            since_drawn: now - drawn_at,
        }
    }
    /// The facts [`may_rest`] decides on, for memo window `i` this frame.
    fn quiet(
        &self,
        ctx: &egui::Context,
        i: usize,
        w: &Window,
        key: (Mode, bool),
        changes: u64,
        now: f64,
    ) -> Quiet {
        let doc = &self.data.notes[i].document;
        let busy = self.window_busy(ctx)
            || ctx.input(|v| v.viewport().focused == Some(true))
            || w.menu.is_some()
            || w.help
            || w.focus
            || w.center
            || w.place.is_some()
            || w.move_caret_to_body
            || w.swatch_drag.is_some()
            || w.calendar_bar
            || w.applied != Some(key)
            || !w.native.idle()
            || doc.requested_selection.is_some()
            || doc.requested_tab.is_some();
        let pointer_near = match (
            crate::system::cursor_pos(),
            ctx.input(|v| v.viewport().outer_rect),
        ) {
            (Some((x, y)), Some(rect)) => {
                let p = Pos2::new(x as f32, y as f32) / ctx.pixels_per_point().max(0.5);
                rect.expand(REST_NEAR).contains(p)
            }
            // Not known: never rest.
            _ => true,
        };
        Quiet {
            busy,
            pointer_near,
            changed: changes != w.drawn_changes,
            since_drawn: now - w.drawn_at,
        }
    }
    fn memo_shortcuts(&mut self, ctx: &egui::Context, i: usize, w: &mut Window) {
        if ctx.input_mut(|v| {
            v.consume_key(egui::Modifiers::CTRL | egui::Modifiers::SHIFT, egui::Key::T)
        }) {
            self.data.notes[i].topmost = !self.data.notes[i].topmost;
        }
        if ctx.input_mut(|v| {
            v.consume_key(egui::Modifiers::CTRL | egui::Modifiers::SHIFT, egui::Key::H)
        }) {
            w.open = false;
            for other in self.studio.windows.values_mut() {
                other.open = false;
            }
        }
        if ctx.input_mut(|v| v.consume_key(egui::Modifiers::CTRL, egui::Key::P)) {
            self.desktop.project_popup = true;
            self.desktop.focus_dialog = true;
        }
        if ctx.input_mut(|v| v.consume_key(egui::Modifiers::CTRL, egui::Key::F)) {
            self.catalog.focus_find();
            self.catalog.tab = crate::catalog::ALL_MEMOS_TAB;
            self.page = 4;
            self.main_open = true;
            ctx.send_viewport_cmd_to(main_viewport(), egui::ViewportCommand::Focus);
        }
        if ctx.input_mut(|v| v.consume_key(egui::Modifiers::CTRL, egui::Key::H)) {
            self.catalog.replace_open = true;
            self.catalog.selected = Some(i);
            self.page = 4;
            self.main_open = true;
            ctx.send_viewport_cmd_to(main_viewport(), egui::ViewportCommand::Focus);
        }
        // Leave Escape available for egui menu buttons, which close their own
        // menu state after this header handles the sticker shortcuts.
        if ctx.input(|v| v.key_pressed(egui::Key::Escape) && v.modifiers.is_none()) {
            if self
                .studio
                .drag
                .as_ref()
                .is_some_and(|d| d.note == i && d.carry.is_some())
            {
                let drag = self.studio.drag.take().unwrap();
                w.mode = drag.mode;
                w.place = Some(drag.origin);
                return;
            }
            if ctx.memory(|m| m.any_popup_open()) {
                ctx.memory_mut(|m| m.close_popup());
                return;
            }
            let now = ctx.input(|v| v.time);
            let old = self.desktop.last_escape.insert(ctx.viewport_id(), now);
            if old.is_some_and(|t| now - t < 0.8) {
                if self.data.notes[i].close_locked {
                    self.data.notes[i].document.status = CLOSE_LOCKED.into();
                } else {
                    w.open = false;
                }
            }
            w.help = false;
            w.menu = None;
            self.extra.picker = None;
        }
    }
    fn memo_header(
        &mut self,
        ui: &mut egui::Ui,
        i: usize,
        w: &mut Window,
        mode: Mode,
        engaged: bool,
        maximized: bool,
    ) {
        let context = ui.ctx().clone();
        let ctx = &context;
        ui.spacing_mut().item_spacing.x = 4.;
        ui.spacing_mut().button_padding = Vec2::new(6., 4.);
        ui.horizontal(|ui| {
            if mode == Mode::Sticker {
                if icon_button(ui, Icon::Plus, "새 스티커 메모").clicked() {
                    self.new_sticker();
                }
                if engaged {
                    // In a window of its own, so a short sticker does not cut
                    // off the color picker it can open.
                    let r = icon_button(ui, Icon::Palette, "메모 색상");
                    let popup_id = ui.make_persistent_id(("color_popup", i));
                    if r.clicked() {
                        ui.memory_mut(|m| m.toggle_popup(popup_id));
                    }
                    crate::float::popup(ui, popup_id, &r, 270., |ui| {
                        crate::desktop::color_choices(ui, &mut self.data.notes[i].color)
                    });
                }
            } else {
                if icon_button(ui, Icon::Fold, "메모 접기").clicked() {
                    w.fold();
                }
                if icon_button(ui, Icon::Sticky, "스티커 노트로 전환").clicked() {
                    w.sticker();
                }
            }
            let gap = (ui.available_width() - header_controls(mode, engaged) as f32 * 32.).max(0.);
            let response = ui.add_sized(
                [gap, 28.],
                egui::Label::new(
                    RichText::new(if mode == Mode::Editor {
                        "메모 편집"
                    } else {
                        ""
                    })
                    .size(11.)
                    .color(MUTED),
                )
                .sense(Sense::click_and_drag()),
            );
            if response.drag_started()
                && mode == Mode::Sticker
                && self.studio.raw_date_drop != Some((i, ctx.input(|v| v.time)))
                && self.begin_sticker_drag(ctx, i, Mode::Sticker, false)
            {
                w.native.cancel();
            } else if response.drag_started() || response.drag_stopped() {
                w.native.start_drag(ctx);
            }
            if response.secondary_clicked() {
                open_menu(ctx, w);
            }
            // The header's gap is blank only in Sticker mode (Editor shows
            // "메모 편집" there), so this is the sticker's one unambiguous
            // empty area: double-clicking it toggles fullscreen, mirroring
            // the native title-bar gesture.
            if mode == Mode::Sticker && response.double_clicked() {
                w.native.cancel();
                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
            }
            if engaged {
                let r = icon_button(ui, Icon::More, "더보기");
                if r.clicked() {
                    toggle_menu(ctx, w, MenuKind::More, r.rect);
                }
            }
            if mode == Mode::Editor {
                crate::ui::icon_menu(ui, Icon::Opacity, "메모 투명도", |ui| {
                    crate::ui::caption(ui, "투명도");
                    let mut opacity = self.data.notes[i].document.opacity * 100.;
                    if ui
                        .add(
                            egui::Slider::new(&mut opacity, 20.0..=100.)
                                .integer()
                                .suffix("%"),
                        )
                        .changed()
                    {
                        self.data.notes[i].document.opacity = opacity / 100.;
                    }
                });
            }
            if engaged {
                let n = &self.data.notes[i];
                let tip = match n.date {
                    Some(d) if n.repeat.is_none() => format!("일정 · {}", d.format("%m.%d")),
                    Some(d) => format!("일정 · {} · {}", d.format("%m.%d"), n.repeat.describe(d)),
                    None => "일정 설정".into(),
                };
                let r = crate::icons::icon_button_drag(ui, Icon::Calendar, &tip).on_hover_text(
                    "클릭: 일정 설정 · 끌어서 캘린더의 날짜에 놓으면 그 날짜의 일정이 됩니다",
                );
                self.drag_source(ui, &r, i, false);
                if r.clicked() {
                    self.open_schedule(i);
                } else if r.drag_started()
                    && mode == Mode::Sticker
                    && self.studio.raw_date_drop != Some((i, ctx.input(|v| v.time)))
                {
                    // Shown as the title-only sticker while it is carried; it
                    // comes back as the sticker, where it was.
                    if self.begin_sticker_drag(ctx, i, Mode::Sticker, true) {
                        w.native.cancel();
                        return;
                    }
                    w.mode = Mode::Title;
                    // Fold it right now, before the drag begins, not on the next frame.
                    w.native.cancel();
                    ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(Vec2::new(
                        160., 40.,
                    )));
                    ctx.send_viewport_cmd(egui::ViewportCommand::MaxInnerSize(Vec2::new(
                        1600., 40.,
                    )));
                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(w.size()));
                    w.applied = Some((Mode::Title, false));
                    w.native.start_drag(ctx);
                }
            }
            pin_button(ui, &mut self.data.notes[i].topmost);
            if mode == Mode::Sticker {
                let mut done = self.data.notes[i]
                    .done_on(self.extra.edit_occurrence.or(self.data.notes[i].date));
                let changed = ui
                    .add_sized([28., 28.], |ui: &mut egui::Ui| {
                        crate::ui::round_check(ui, &mut done, Some(INK.gamma_multiply(0.45)))
                    })
                    .on_hover_text("완료")
                    .changed();
                if changed {
                    self.done_at(
                        i,
                        self.extra.edit_occurrence.or(self.data.notes[i].date),
                        done,
                    );
                    self.extra.scope_owner = ctx.viewport_id();
                }
                if icon_button(ui, Icon::Fold, "제목 한 줄로 접기").clicked() {
                    w.fold();
                }
            }
            if engaged
                && mode == Mode::Editor
                && icon_button(
                    ui,
                    if maximized {
                        Icon::Restore
                    } else {
                        Icon::Maximize
                    },
                    if maximized {
                        "원래 크기로 복원"
                    } else {
                        "창 최대화"
                    },
                )
                .clicked()
            {
                w.native.cancel();
                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
            }
            // Pin, done, fold and close stay on the idle sticker too.
            if engaged || mode == Mode::Sticker {
                if self.data.notes[i].close_locked {
                    // Locked from the right-click menu: only the menu closes it.
                    ui.scope(|ui| {
                        ui.disable();
                        icon_button(ui, Icon::Close, "창 닫기")
                            .on_disabled_hover_text(CLOSE_LOCKED);
                    });
                } else if icon_button(ui, Icon::Close, "창 닫기 · 메모는 유지").clicked() {
                    w.open = false;
                }
            }
        });
    }
}
/// Said when a close-locked memo window is asked to close by its X button,
/// Alt+F4 or a double Escape.
const CLOSE_LOCKED: &str = "닫기 버튼이 잠겨 있어요 · 우클릭 메뉴의 ‘이 스티커 닫기’로 닫아요";
/// Whether a memo window's footer shows the format buttons above its tabs:
/// only the editor window does. A sticker's font, styles and opacity are in
/// its right-click menus ("글꼴…", "투명도").
fn footer_tools(mode: Mode, engaged: bool) -> bool {
    engaged && mode == Mode::Editor
}
/// The id of the "글꼴" dialog's window (see [`crate::ui::dialog`]).
const FONT_DIALOG: &str = "font_dialog";
const FONT_DIALOG_SIZE: [f32; 2] = [520., 520.];
/// A checkbox in a menu window. Its buttons have no fill there, which left
/// the box itself invisible: it gets an outline of its own.
fn menu_check(ui: &mut egui::Ui, value: &mut bool, label: &str) -> egui::Response {
    ui.scope(|ui| {
        let ink = ui.visuals().text_color();
        let w = &mut ui.visuals_mut().widgets;
        for v in [&mut w.inactive, &mut w.hovered, &mut w.active] {
            v.bg_stroke = egui::Stroke::new(1.0_f32, ink.gamma_multiply(0.5));
        }
        ui.checkbox(value, label)
    })
    .inner
}
/// How many 28 px buttons (each with 4 px spacing before it) follow the gap
/// of a memo window's top bar, so they end exactly at its right edge. Keep in
/// step with [`App::memo_header`].
fn header_controls(mode: Mode, engaged: bool) -> usize {
    match (mode, engaged) {
        // More, schedule, always on top, done, fold, close.
        (Mode::Sticker, true) => 6,
        // Always on top, done, fold, close.
        (Mode::Sticker, false) => 4,
        // More, opacity, schedule, always on top, maximize, close.
        (_, true) => 6,
        // Opacity, always on top.
        (_, false) => 2,
    }
}
/// The project / linked-memo buttons on a sticker's tab row, and the space
/// between them and the window's right edge (the resize grip).
const CORNER_ICONS_W: f32 = 60.;
const CORNER_ICONS_GAP: f32 = 22.;
/// A shallow inner shadow keeps overlapping stickers readable without
/// reviving the large system shadow or adding transparent window margins.
fn paint_sticker_edge(ctx: &egui::Context, paper: Color32, radius: u8) {
    let rect = ctx.screen_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("sticker_edge"),
    ));
    let dark_paper = egui::Rgba::from(paper).intensity() < 0.18;
    for (inset, radius, opacity) in [(0., radius, 26), (1., radius.saturating_sub(1), 10)] {
        let color = if dark_paper {
            Color32::from_white_alpha(opacity)
        } else {
            Color32::from_black_alpha(opacity)
        };
        painter.rect_stroke(
            rect.shrink(inset),
            radius,
            egui::Stroke::new(1.0_f32, color),
            egui::StrokeKind::Inside,
        );
    }
}

fn paper_style(ui: &mut egui::Ui, paper: Color32) {
    let v = ui.visuals_mut();
    v.dark_mode = false;
    v.override_text_color = Some(INK);
    v.window_fill = Color32::from_rgb(255, 254, 250);
    v.panel_fill = paper;
    v.extreme_bg_color = paper;
    v.selection.bg_fill = Color32::from_rgb(250, 225, 180);
    v.widgets.inactive.bg_fill = paper;
    v.widgets.inactive.weak_bg_fill = Color32::from_white_alpha(65);
    v.widgets.hovered.weak_bg_fill = Color32::from_white_alpha(120);
    v.window_stroke = egui::Stroke::new(1.0_f32, Color32::from_gray(215));
}
fn pin_button(ui: &mut egui::Ui, value: &mut bool) {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(28.), Sense::click());
    if *value || response.hovered() {
        ui.painter().rect_filled(
            rect,
            6.,
            Color32::from_black_alpha(if *value { 20 } else { 8 }),
        );
    }
    crate::icons::paint_icon(
        ui.painter(),
        rect.shrink(5.),
        Icon::Pin,
        if *value {
            INK
        } else {
            INK.gamma_multiply(0.35)
        },
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, true, *value, "항상 위에")
    });
    if response.on_hover_text("항상 위에 · Ctrl+Shift+T").clicked() {
        *value = !*value;
    }
}
fn translucent(c: Color32, alpha: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (alpha * 255.).round() as u8)
}
/// Resize the color box from its right and bottom edges. The window origin
/// stays fixed, so pointer positions stay stable while the size follows.
fn swatch_resize(ctx: &egui::Context, ui: &egui::Ui, w: &mut Window) {
    let Some(inner) = ctx.input(|v| v.viewport().inner_rect) else {
        return;
    };
    let size = inner.size();
    let (pointer, down, pressed, shift) = ctx.input(|v| {
        (
            v.pointer.latest_pos(),
            v.pointer.primary_down(),
            v.pointer.primary_pressed(),
            v.modifiers.shift,
        )
    });
    let band = (size.min_elem() / 3.).clamp(4., 10.);
    let edge = |p: Pos2| (p.x >= size.x - band, p.y >= size.y - band);
    if let Some(p) = pointer {
        let (right, bottom) = edge(p);
        if w.swatch_drag.is_some() || right || bottom {
            ctx.set_cursor_icon(match (right, bottom) {
                (true, true) => egui::CursorIcon::ResizeNwSe,
                (true, false) => egui::CursorIcon::ResizeHorizontal,
                _ => egui::CursorIcon::ResizeVertical,
            });
        }
        if pressed && (right || bottom) {
            let axes = Vec2::new(f32::from(u8::from(right)), f32::from(u8::from(bottom)));
            w.swatch_drag = Some((size, p, axes));
        }
    }
    let Some((start, origin, axes)) = w.swatch_drag else {
        return;
    };
    if let Some(p) = pointer {
        let delta = (p - origin) * axes;
        let mut next = (start + delta).clamp(Vec2::splat(16.), Vec2::splat(640.));
        if shift {
            next = Vec2::splat(next.x.max(next.y));
        }
        if (next - size).length() > 0.5 {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(next));
        }
        w.swatch_size = next;
    }
    if !down {
        w.swatch_drag = None;
    }
    ui.ctx().request_repaint();
}
fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    Color32::from_rgb(
        (a.r() as f32 * (1. - t) + b.r() as f32 * t) as u8,
        (a.g() as f32 * (1. - t) + b.g() as f32 * t) as u8,
        (a.b() as f32 * (1. - t) + b.b() as f32 * t) as u8,
    )
}
pub fn nav_button(
    ui: &mut egui::Ui,
    text: &str,
    selected: bool,
    icon: Icon,
    badge: Option<String>,
    accent: bool,
) -> egui::Response {
    let tone = crate::ui::t(ui);
    let accent_color = crate::ui::accent(ui);
    let (rect, r) = ui.allocate_exact_size(
        Vec2::new(
            ui.available_width(),
            if crate::ui::variant(ui) == 2 {
                36.
            } else {
                30.
            },
        ),
        Sense::click(),
    );
    r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, text));
    if selected || r.hovered() || accent {
        ui.painter().rect_filled(
            rect.shrink2(Vec2::new(0., 1.)),
            8.,
            if selected || accent {
                tone.accent_soft
            } else {
                tone.hover
            },
        );
    }
    if selected {
        let marker = Rect::from_min_size(
            rect.left_top() + Vec2::new(0., 7.),
            Vec2::new(
                3.,
                if crate::ui::variant(ui) == 2 {
                    22.
                } else {
                    16.
                },
            ),
        );
        ui.painter().rect_filled(marker, 2., accent_color);
    }
    let highlight = if ui.visuals().dark_mode {
        Color32::from_rgb(191, 190, 255)
    } else {
        accent_color
    };
    let color = if selected || accent {
        highlight
    } else {
        tone.ink
    };
    crate::icons::paint_icon(
        ui.painter(),
        Rect::from_center_size(
            egui::pos2(rect.left() + 17., rect.center().y),
            Vec2::splat(17.),
        ),
        icon,
        if selected || accent {
            highlight
        } else {
            tone.muted
        },
    );
    ui.painter().text(
        egui::pos2(rect.left() + 34., rect.center().y),
        egui::Align2::LEFT_CENTER,
        text,
        egui::FontId::proportional(13.5),
        color,
    );
    if let Some(b) = badge {
        let font = egui::FontId::proportional(11.);
        let galley = ui.painter().layout_no_wrap(b, font, highlight);
        let badge = Rect::from_center_size(
            egui::pos2(rect.right() - 14., rect.center().y),
            Vec2::new((galley.size().x + 12.).max(22.), 20.),
        );
        ui.painter().rect_filled(badge, 10., tone.accent_soft);
        ui.painter()
            .galley(badge.center() - galley.size() / 2., galley, highlight);
    }
    r.on_hover_cursor(egui::CursorIcon::PointingHand)
}
/// Sidebar project row with a hash mark.
pub fn project_button(ui: &mut egui::Ui, name: &str, selected: bool) -> egui::Response {
    let tone = crate::ui::t(ui);
    let accent_color = crate::ui::accent(ui);
    let (rect, r) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.), Sense::click());
    if selected || r.hovered() {
        ui.painter().rect_filled(
            rect,
            6.,
            if selected {
                tone.accent_soft
            } else {
                tone.hover
            },
        );
    }
    ui.painter().text(
        egui::pos2(rect.left() + 17., rect.center().y),
        egui::Align2::CENTER_CENTER,
        "#",
        egui::FontId::proportional(14.),
        if selected { accent_color } else { tone.faint },
    );
    let galley = ui.painter().layout(
        name.into(),
        egui::FontId::proportional(13.5),
        if selected { accent_color } else { tone.ink },
        rect.width() - 44.,
    );
    ui.painter().galley(
        egui::pos2(rect.left() + 34., rect.center().y - galley.size().y / 2.),
        galley,
        tone.ink,
    );
    r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name));
    r.on_hover_cursor(egui::CursorIcon::PointingHand)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_a_floating_area_keeps_a_window_built() {
        let ctx = egui::Context::default();
        let run = |tools: bool| {
            let _ = ctx.run(Default::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| ui.label("본문"));
                if tools {
                    egui::Area::new(egui::Id::new("picture_tools"))
                        .show(ctx, |ui| ui.label("100%"));
                }
            });
        };
        run(false);
        assert!(!floating_area_shown(&ctx), "panels alone");
        run(true);
        assert!(floating_area_shown(&ctx), "the picture toolbar");
        run(false);
        run(false);
        assert!(!floating_area_shown(&ctx), "toolbar gone");
    }
    #[test]
    fn input_keeps_a_window_built() {
        let a = App::sample(Local::now().date_naive());
        let ctx = egui::Context::default();
        let busy = |events: Vec<egui::Event>| {
            let input = egui::RawInput {
                events,
                ..Default::default()
            };
            let mut busy = false;
            let _ = ctx.run(input, |ctx| busy = a.window_busy(ctx));
            busy
        };
        busy(vec![]);
        assert!(!busy(vec![]), "nothing happening");
        assert!(
            busy(vec![egui::Event::PointerMoved(egui::pos2(5., 5.))]),
            "the pointer moves in it"
        );
        assert!(
            busy(vec![egui::Event::PointerGone]),
            "the pointer leaves it"
        );
        assert!(!busy(vec![]));
        assert!(
            busy(vec![egui::Event::Text("가".into())]),
            "typing reaches it"
        );
    }
    #[test]
    fn a_window_rests_only_when_nothing_concerns_it() {
        let quiet = Quiet {
            since_drawn: 0.1,
            ..Quiet::default()
        };
        assert!(may_rest(quiet), "pointer elsewhere, nothing changed");
        for (what, q) in [
            (
                "its own input, menu or drag",
                Quiet {
                    busy: true,
                    ..quiet
                },
            ),
            (
                "pointer close by",
                Quiet {
                    pointer_near: true,
                    ..quiet
                },
            ),
            (
                "data changed",
                Quiet {
                    changed: true,
                    ..quiet
                },
            ),
            (
                "built too long ago",
                Quiet {
                    since_drawn: REST_MAX,
                    ..quiet
                },
            ),
            (
                "never built",
                Quiet {
                    since_drawn: f64::INFINITY,
                    ..quiet
                },
            ),
            (
                "clock went back",
                Quiet {
                    since_drawn: -1.,
                    ..quiet
                },
            ),
        ] {
            assert!(!may_rest(q), "{what}");
        }
        let w = Window::new(Mode::Sticker);
        assert_eq!(
            w.drawn_at,
            f64::NEG_INFINITY,
            "a new window is built at once"
        );
        assert!(w.native.idle());
    }
    #[test]
    fn normal_fold_and_title_to_sticker_are_distinct() {
        let mut w = Window::new(Mode::Editor);
        w.fold();
        assert_eq!(w.mode, Mode::Title);
        w.fold();
        assert_eq!(w.mode, Mode::Swatch);
        w.fold();
        assert_eq!(w.mode, Mode::Editor);
        w.fold();
        w.sticker();
        assert_eq!(w.mode, Mode::Sticker);
        w.fold();
        assert_eq!(w.mode, Mode::Title);
        w.fold();
        assert_eq!(w.mode, Mode::Swatch);
        w.fold();
        assert_eq!(w.mode, Mode::Sticker);
    }
    #[test]
    fn explicit_editor_retains_sticker_restore_intent() {
        let mut w = Window::new(Mode::Sticker);
        w.skip_title = true;
        w.edit();
        assert_eq!(w.mode, Mode::Editor);
        w.fold();
        assert_eq!(w.mode, Mode::Swatch);
        w.fold();
        assert_eq!(w.mode, Mode::Sticker);
    }
    #[test]
    fn adding_stickers_keeps_existing_windows_and_notes_independent() {
        let mut a = App::sample(Local::now().date_naive());
        let old = a.data.notes.len();
        let first = a.new_sticker();
        let second = a.new_sticker();
        assert_ne!(viewport(first), viewport(second));
        a.data.notes[first].body = "첫 번째".into();
        a.data.notes[second].body = "두 번째".into();
        a.studio.windows.get_mut(&first).unwrap().fold();
        assert_eq!(a.studio.windows[&second].mode, Mode::Sticker);
        assert_eq!(a.data.notes.len(), old + 2);
        a.studio.windows.get_mut(&first).unwrap().open = false;
        assert_eq!(a.data.notes[first].body, "첫 번째");
        assert!(a.studio.windows[&second].open);
    }
    #[test]
    fn reopening_a_closed_viewport_reapplies_saved_size_without_resizing_live_windows() {
        let mut studio = Studio::default();
        studio.open(0, None);
        let w = studio.windows.get_mut(&0).unwrap();
        w.sticker();
        w.sticker_size = Vec2::new(410., 370.);
        w.applied = Some((Mode::Sticker, false));
        w.open = false;
        studio.open(0, None);
        let w = studio.windows.get_mut(&0).unwrap();
        w.sticker();
        assert!(w.applied.is_none());
        assert_eq!(w.size(), Vec2::new(410., 370.));
        w.applied = Some((Mode::Editor, false));
        studio.open(0, None);
        assert_eq!(studio.windows[&0].applied, Some((Mode::Editor, false)));
    }
    #[test]
    fn all_three_shortcuts_reject_duplicate_combinations() {
        let mut a = App::sample(Local::now().date_naive());
        a.shortcuts[2] = a.shortcuts[0].clone();
        a.apply_shortcuts();
        assert!(a.hotkey_status.contains("서로 다른"));
        a.shortcuts[2] = a.shortcuts[1].clone();
        a.apply_shortcuts();
        assert!(a.hotkey_status.contains("서로 다른"));
    }
    #[test]
    fn ctrl_f_in_the_main_window_puts_the_cursor_in_the_search_box() {
        let mut a = App::sample(Local::now().date_naive());
        // Any page, settings open, sidebar folded to its icons.
        a.page = 5;
        a.settings = true;
        a.sidebar = false;
        a.sidebar_width = 64.;
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            events: vec![egui::Event::Key {
                key: egui::Key::F,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::CTRL,
            }],
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| a.main_shortcuts(ctx));
        assert!(a.focus_search && a.sidebar && !a.settings);
        assert!(a.sidebar_width >= 160., "the search box is shown");
        assert_eq!(a.page, 5, "the page stays");
        // There is no global search shortcut any more.
        assert_eq!(a.shortcuts.len(), 3);
    }
    #[test]
    fn a_calendar_drag_memo_closed_blank_is_erased_and_a_written_one_stays() {
        let mut a = App::sample(Local::now().date_naive());
        let blank = a.new_sticker();
        let written = a.new_sticker();
        let plain = a.new_sticker();
        a.drafts = vec![blank, written];
        // Still open: nothing happens.
        a.drop_blank_drafts();
        assert_eq!(a.drafts, vec![blank, written]);
        a.data.notes[written].title = "회의".into();
        for i in [blank, written, plain] {
            a.studio.windows.get_mut(&i).unwrap().open = false;
        }
        let before = crate::lists::CHANGES.load(std::sync::atomic::Ordering::Relaxed);
        a.drop_blank_drafts();
        assert!(a.data.notes[blank].purged && a.data.notes[blank].deleted);
        assert!(
            crate::lists::CHANGES.load(std::sync::atomic::Ordering::Relaxed) > before,
            "cached lists drop the erased memo at once"
        );
        assert!(!a.studio.windows.contains_key(&blank));
        assert!(!a.data.notes[written].deleted, "a memo with a title stays");
        assert!(!a.data.notes[plain].deleted, "only calendar drag memos");
        assert!(a.drafts.is_empty());
        // Body text alone also keeps it.
        let body = a.new_sticker();
        a.drafts = vec![body];
        a.data.notes[body].body = "내용".into();
        a.studio.windows.get_mut(&body).unwrap().open = false;
        a.drop_blank_drafts();
        assert!(!a.data.notes[body].deleted);
    }
    #[test]
    fn a_new_memo_for_a_day_closed_blank_is_gone_from_every_list() {
        let today = Local::now().date_naive();
        let mut a = App::sample(today);
        // '+ 이 날짜에 스티커' (or the day's +) and '+ 새 일정'.
        let sticker = a.new_sticker_on(today);
        let schedule = a.new_schedule(today);
        assert_eq!(a.data.notes[sticker].date, Some(today));
        assert_eq!(a.data.notes[schedule].date, Some(today));
        assert_eq!(a.studio.windows[&schedule].mode, Mode::Editor);
        assert!(a.calendar_matches(&a.data.notes[sticker], today));
        for i in [sticker, schedule] {
            a.studio.windows.get_mut(&i).unwrap().open = false;
        }
        a.drop_blank_drafts();
        for i in [sticker, schedule] {
            let n = &a.data.notes[i];
            assert!(n.purged && n.deleted, "erased for good");
            assert!(!a.visible(i), "out of the calendar and the memo lists");
            assert!(!a.studio.windows.contains_key(&i));
        }
        assert!(
            !a.data.notes.iter().any(|n| n.deleted && !n.purged),
            "nothing goes to the trash"
        );
        assert!(a.drafts.is_empty());
        // Written in, then closed: it stays, and later closes keep it.
        let kept = a.new_schedule(today);
        a.data.notes[kept].title = "회의".into();
        a.studio.windows.get_mut(&kept).unwrap().open = false;
        a.drop_blank_drafts();
        assert!(!a.data.notes[kept].deleted && a.drafts.is_empty());
    }
    #[test]
    fn quitting_erases_a_new_memo_left_blank_and_keeps_its_written_neighbour() {
        let today = Local::now().date_naive();
        let mut a = App::sample(today);
        let blank = a.new_sticker_on(today);
        let written = a.new_sticker_on(today);
        a.data.notes[written].body = "내용".into();
        a.close_blank_drafts();
        assert!(a.data.notes[blank].purged);
        assert!(!a.data.notes[written].deleted);
        assert!(
            a.studio.windows[&written].open,
            "a written sticker still reopens after a restart"
        );
        assert_eq!(a.drafts, vec![written]);
    }
    #[test]
    fn choosing_the_page_behind_the_settings_leaves_the_settings() {
        let mut a = App::sample(Local::now().date_naive());
        a.page = 0;
        a.settings = true;
        a.open_page(0);
        assert!(
            !a.settings && a.page == 0,
            "내 작업실 again shows 내 작업실"
        );
        a.settings = true;
        a.open_page(4);
        assert!(!a.settings && a.page == 4);
    }
    #[test]
    fn unchecked_single_note_cannot_expose_stale_completion_time() {
        let mut a = App::sample(Local::now().date_naive());
        a.data.notes[0].completed = Some(Local::now().naive_local());
        a.data.notes[0].done = false;
        assert!(a.editor_completion().is_none());
        a.data.notes[0].done = true;
        assert!(a.editor_completion().is_some());
    }
}
#[cfg(test)]
mod occurrence_window_tests {
    use super::*;
    #[test]
    fn open_editor_follows_rescheduled_period_occurrence() {
        let start = NaiveDate::from_ymd_opt(2026, 11, 9).unwrap();
        let mut n = Note::new("기간", Some(start), 0);
        n.duration = 3;
        let selected = start + chrono::Duration::days(1);
        let mut w = Window::new(Mode::Editor);
        w.schedule_start = Some(start);
        w.occurrence = Some(selected);
        n.move_start(start + chrono::Duration::days(5));
        w.reconcile(&n);
        assert_eq!(w.occurrence, Some(selected + chrono::Duration::days(5)));
        n.duration = 1;
        w.reconcile(&n);
        assert_eq!(w.schedule_start, n.date);
    }
}
#[cfg(test)]
mod arrangement_tests {
    use super::*;
    #[test]
    fn future_deadlines_are_opened_and_completed_ones_are_excluded() {
        let day = Local::now().date_naive();
        let mut app = App::sample(day);
        app.data.notes[0].date = Some(day + chrono::Duration::days(20));
        app.data.notes[0].deadline = true;
        assert!(app.daily_due(day).contains(&0));
        app.data.notes[0].done = true;
        assert!(!app.daily_due(day).contains(&0));
    }
    #[test]
    fn completion_closes_only_when_enabled_and_rearranges_in_visual_order() {
        let mut app = App::sample(Local::now().date_naive());
        for i in 0..3 {
            app.open_sticker(i, None);
        }
        app.studio.windows.get_mut(&1).unwrap().last_pos = Some(Pos2::new(800., 300.));
        app.studio.windows.get_mut(&2).unwrap().last_pos = Some(Pos2::new(800., 100.));
        let ctx = egui::Context::default();
        app.prefs.close_completed_sticker = false;
        app.completed_stickers.push(0);
        app.finish_completed_stickers(&ctx);
        assert!(app.studio.windows[&0].open);
        app.prefs.close_completed_sticker = true;
        app.prefs.rearrange_completed_stickers = true;
        app.completed_stickers.push(0);
        app.finish_completed_stickers(&ctx);
        assert!(!app.studio.windows[&0].open);
        app.apply_arrangement(&ctx);
        assert_eq!(app.studio.windows[&1].mode, Mode::Title);
        assert_eq!(app.studio.windows[&2].mode, Mode::Title);
        assert!(app.studio.windows[&2].place.unwrap().y < app.studio.windows[&1].place.unwrap().y);
    }
    #[test]
    fn daily_auto_open_keeps_undated_stickers_open() {
        let today = daily_day(daily_now());
        let mut app = App::sample(today);
        app.data.notes[0].date = None;
        app.open_sticker(0, None);
        app.desktop.daily_today = true;
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            app.daily_today(ctx);
            app.apply_arrangement(ctx);
        });
        assert!(app.studio.windows[&0].open);
        assert_eq!(app.data.notes[0].date, None);
        app.desktop.daily_shown = Some(today - chrono::Duration::days(1));
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            app.daily_today(ctx);
            app.apply_arrangement(ctx);
        });
        assert!(app.studio.windows[&0].open);
        assert_eq!(app.data.notes[0].date, None);
    }

    #[test]
    fn daily_memos_roll_over_at_five_and_pick_open_past_and_today() {
        let at = |d: u32, h: u32, m: u32| {
            NaiveDate::from_ymd_opt(2026, 11, d)
                .unwrap()
                .and_hms_opt(h, m, 0)
                .unwrap()
        };
        let d = |day: u32| NaiveDate::from_ymd_opt(2026, 11, day).unwrap();
        assert_eq!(daily_day(at(12, 4, 59)), d(11));
        assert_eq!(daily_day(at(12, 5, 0)), d(12));
        let mut a = App::sample(d(11));
        let all = a.daily_due(d(12));
        // Sample memos of the 11th are still open; the 12th's memo is due.
        assert!(all.contains(&0) && all.contains(&2));
        a.done(0, true);
        assert!(!a.daily_due(d(12)).contains(&0));
        // Memos for later days are not shown early.
        assert!(!a.daily_due(d(11)).contains(&2));
    }
    #[test]
    fn submenus_open_outside_their_menu_right_first_then_left() {
        let area = Rect::from_min_size(Pos2::ZERO, Vec2::new(1000., 800.));
        let size = Vec2::new(200., 150.);
        let parent = Rect::from_min_size(Pos2::new(100., 100.), Vec2::new(220., 300.));
        let p = submenu_pos(parent, size, 140., area);
        assert_eq!(p, Pos2::new(322., 140.));
        // Near the right edge it flips to the left, never overlapping.
        let parent = Rect::from_min_size(Pos2::new(700., 100.), Vec2::new(220., 300.));
        let p = submenu_pos(parent, size, 140., area);
        assert!(p.x + size.x <= parent.left());
        // A menu near the bottom is pulled up to stay visible.
        let p = submenu_pos(parent, size, 760., area);
        assert_eq!(p.y, 650.);
    }
    #[test]
    fn right_layout_stacks_from_top_right_and_wraps_into_a_new_column() {
        let area = Rect::from_min_size(Pos2::ZERO, Vec2::new(1000., 300.));
        let sizes = [
            Vec2::new(200., 120.),
            Vec2::new(150., 120.),
            Vec2::new(180., 120.),
        ];
        let p = plan(Layout::Right, area, &sizes);
        assert_eq!(p[0], Pos2::new(1000. - 14. - 200., 14.));
        assert_eq!(p[1], Pos2::new(1000. - 14. - 150., 14. + 120. + 8.));
        // The third does not fit vertically and starts a column to the left.
        assert_eq!(p[2].y, 14.);
        assert!(p[2].x + 180. <= p[0].x);
    }
    #[test]
    fn left_and_cascade_layouts_stay_inside_the_work_area() {
        let area = Rect::from_min_size(Pos2::new(-1280., 0.), Vec2::new(1280., 700.));
        let sizes = vec![Vec2::splat(48.); 20];
        for layout in [Layout::Left, Layout::Cascade] {
            for (pos, size) in plan(layout, area, &sizes).into_iter().zip(&sizes) {
                assert!(
                    area.contains_rect(Rect::from_min_size(pos, *size)),
                    "{layout:?} {pos:?}"
                );
            }
        }
    }
    #[test]
    fn folding_arrangement_changes_every_open_sticker_and_places_it() {
        let mut a = App::sample(Local::now().date_naive());
        let first = a.new_sticker();
        let second = a.new_sticker();
        a.arrange = Some(Arrange {
            preserve_sticker_order: false,
            layout: Layout::Right,
            fold: Some(Mode::Swatch),
            area: Rect::from_min_size(Pos2::ZERO, Vec2::new(1600., 900.)),
        });
        a.apply_arrangement(&egui::Context::default());
        for i in [first, second] {
            let w = &a.studio.windows[&i];
            assert_eq!(w.mode, Mode::Swatch);
            assert!(w.place.is_some());
        }
        assert!(a.arrange.is_none());
    }
    fn menu_has_text(output: &egui::FullOutput, text: &str) -> bool {
        output
            .shapes
            .iter()
            .any(|shape| matches!(&shape.shape, egui::Shape::Text(t) if t.galley.job.text == text))
    }
    #[test]
    fn the_top_bar_buttons_end_at_the_right_edge_down_to_the_smallest_sticker() {
        let mut a = App::sample(Local::now().date_naive());
        let ctx = egui::Context::default();
        // The editor window is at least 480 wide.
        for (width, mode, engaged) in [STICKER_MIN.x, 360., 600.]
            .into_iter()
            .flat_map(|x| [(x, Mode::Sticker, true), (x, Mode::Sticker, false)])
            .chain(
                [480., 700.]
                    .into_iter()
                    .flat_map(|x| [(x, Mode::Editor, true), (x, Mode::Editor, false)]),
            )
        {
            {
                let mut w = Window::new(mode);
                let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(width, 300.));
                let mut ends = (0., 0.);
                let _ = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(screen),
                        ..Default::default()
                    },
                    |ctx| {
                        // The sticker's top bar: 10 px margins.
                        egui::CentralPanel::default()
                            .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(10, 4)))
                            .show(ctx, |ui| {
                                let right = ui.max_rect().right();
                                let bar = ui
                                    .scope(|ui| a.memo_header(ui, 0, &mut w, mode, engaged, false))
                                    .response
                                    .rect;
                                ends = (bar.right(), right);
                            });
                    },
                );
                assert!(
                    (ends.0 - ends.1).abs() < 1.,
                    "{width} {mode:?} engaged {engaged}: {ends:?}"
                );
            }
        }
    }
    #[test]
    fn a_sticker_footer_has_no_format_buttons_and_the_editor_window_keeps_them() {
        assert!(!footer_tools(Mode::Sticker, true));
        assert!(!footer_tools(Mode::Sticker, false));
        assert!(footer_tools(Mode::Editor, true));
        assert!(!footer_tools(Mode::Editor, false));
        // The editor window is never narrower than this: one row.
        assert_eq!(crate::composer::footer_tools_height(480. - 18.), 28.);
    }
    #[test]
    fn the_sticker_menu_offers_the_font_dialog_opacity_and_the_close_lock() {
        let mut a = App::sample(Local::now().date_naive());
        let mut w = Window::new(Mode::Sticker);
        let ctx = egui::Context::default();
        let input = || egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.))),
            ..Default::default()
        };
        let mut menu = |a: &mut App, own: bool| {
            let mut sub2 = None;
            ctx.run(input(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    a.sticker_menu(ui, 0, &mut w, own, &mut sub2);
                });
            })
        };
        let own = menu(&mut a, true);
        for text in ["글꼴…", "투명도", "닫기 버튼 잠금", "이 스티커 닫기"] {
            assert!(menu_has_text(&own, text), "{text}");
        }
        let arrange = menu(&mut a, false);
        for text in ["글꼴…", "투명도", "닫기 버튼 잠금"] {
            assert!(!menu_has_text(&arrange, text), "{text}");
        }
        // A locked memo is not restyled from its menu.
        a.data.notes[0].locked = true;
        let locked = menu(&mut a, true);
        assert!(!menu_has_text(&locked, "글꼴…"));
        assert!(menu_has_text(&locked, "닫기 버튼 잠금"));
    }
    #[test]
    fn a_close_locked_sticker_ignores_its_x_button() {
        for close_locked in [false, true] {
            let mut a = App::sample(Local::now().date_naive());
            a.data.notes[0].close_locked = close_locked;
            let mut w = Window::new(Mode::Sticker);
            let ctx = egui::Context::default();
            let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(360., 200.));
            // The close button ends the top bar (10 px margins, 28 px buttons).
            let x = Pos2::new(360. - 10. - 14., 4. + 14.);
            let button = |pressed| egui::Event::PointerButton {
                pos: x,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            };
            for events in [
                vec![],
                vec![egui::Event::PointerMoved(x)],
                vec![button(true)],
                vec![button(false)],
            ] {
                let _ = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(screen),
                        events,
                        ..Default::default()
                    },
                    |ctx| {
                        egui::CentralPanel::default()
                            .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(10, 4)))
                            .show(ctx, |ui| {
                                a.memo_header(ui, 0, &mut w, Mode::Sticker, true, false);
                            });
                    },
                );
            }
            assert_eq!(w.open, close_locked, "close_locked {close_locked}");
        }
    }
    #[test]
    fn the_font_dialog_applies_on_enter_and_leaves_the_memo_on_escape() {
        let mut a = App::sample(Local::now().date_naive());
        let ctx = egui::Context::default();
        let key = |key| {
            vec![egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }]
        };
        let run = |a: &mut App, events: Vec<egui::Event>, open: bool| {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200., 800.))),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    if open {
                        a.open_font_dialog(ctx, 0);
                    }
                    a.font_dialog_window(ctx);
                },
            );
        };
        let pick = |a: &mut App| {
            let d = a.studio.font_dialog.as_mut().expect("the dialog is open");
            assert_eq!(d.note, 0);
            d.choice.font_name = "굴림".into();
            d.choice.format.bold = true;
        };
        let before = a.data.notes[0].document.font_name.clone();
        assert_ne!(before, "굴림");
        run(&mut a, vec![], true);
        pick(&mut a);
        run(&mut a, vec![], false);
        assert!(a.studio.font_dialog.is_some(), "stays open until answered");
        run(&mut a, key(egui::Key::Escape), false);
        assert!(a.studio.font_dialog.is_none());
        assert_eq!(a.data.notes[0].document.font_name, before);
        assert!(!a.data.notes[0].document.format.bold);
        run(&mut a, vec![], true);
        pick(&mut a);
        run(&mut a, key(egui::Key::Enter), false);
        assert!(a.studio.font_dialog.is_none());
        assert_eq!(a.data.notes[0].document.font_name, "굴림");
        assert!(a.data.notes[0].document.format.bold);
    }
    #[test]
    fn arrange_submenu_omits_the_close_actions_the_context_menu_still_has() {
        let mut a = App::sample(Local::now().date_naive());
        let mut w = Window::new(Mode::Editor);
        let ctx = egui::Context::default();
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.));
        let input = || egui::RawInput {
            screen_rect: Some(screen),
            ..Default::default()
        };
        // The sticker's own right-click menu (`own = true`) still offers to
        // close it.
        let mut sub2 = None;
        let full = ctx.run(input(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                a.sticker_menu(ui, 0, &mut w, true, &mut sub2);
            });
        });
        assert!(menu_has_text(&full, "이 스티커 닫기"));
        assert!(menu_has_text(&full, "스티커 정렬"));
        // "더보기 › 스티커 정렬" (`own = false`) is specifically about
        // arranging stickers; it must not repeat the close actions the
        // context menu already has.
        let mut sub2 = None;
        let sub = ctx.run(input(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                a.sticker_menu(ui, 0, &mut w, false, &mut sub2);
            });
        });
        assert!(!menu_has_text(&sub, "이 스티커 닫기"));
        assert!(!menu_has_text(&sub, "모든 스티커 닫기  Ctrl+Shift+H"));
        assert!(menu_has_text(&sub, "스티커 정렬"));
        assert!(menu_has_text(&sub, "화면 오른쪽에 정렬"));
    }
    #[test]
    fn arrange_menu_no_longer_offers_the_old_combined_position_and_mode_items() {
        // #58 follow-up: "화면 오른쪽/왼쪽에 정렬" now each open their own
        // side-specific popup instead of a flat list that mixed position and
        // fold-mode into one label; those old combined items must be gone
        // from both the context menu and the 더보기 › 스티커 정렬 submenu.
        let mut a = App::sample(Local::now().date_naive());
        let mut w = Window::new(Mode::Editor);
        let ctx = egui::Context::default();
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.));
        let input = || egui::RawInput {
            screen_rect: Some(screen),
            ..Default::default()
        };
        for own in [true, false] {
            let mut sub2 = None;
            let out = ctx.run(input(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    a.sticker_menu(ui, 0, &mut w, own, &mut sub2);
                });
            });
            for gone in [
                "모두 제목만 보이게 · 오른쪽 정렬",
                "모두 상자로 접기 · 오른쪽 정렬",
                "모두 펼치기 · 계단식 정렬",
            ] {
                assert!(!menu_has_text(&out, gone), "own={own}: still has {gone}");
            }
            // The plain cascade action stays, unchanged.
            assert!(menu_has_text(&out, "계단식으로 정렬"));
            assert!(menu_has_text(&out, "화면 왼쪽에 정렬"));
        }
    }
    #[test]
    fn clicking_the_right_side_row_opens_its_popup_instead_of_arranging_immediately() {
        let mut a = App::sample(Local::now().date_naive());
        let mut w = Window::new(Mode::Editor);
        let ctx = egui::Context::default();
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.));
        let full = Rect::from_min_size(Pos2::ZERO, Vec2::new(300., 300.));
        // First frame: find the "화면 오른쪽에 정렬" row's position.
        let mut sub2 = None;
        let out = ctx.run(
            egui::RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    a.sticker_menu(ui, 0, &mut w, true, &mut sub2);
                });
            },
        );
        let rect = out
            .shapes
            .iter()
            .find_map(|shape| {
                if let egui::Shape::Text(t) = &shape.shape {
                    if t.galley.job.text == "화면 오른쪽에 정렬" {
                        return Some(t.galley.rect.translate(t.pos.to_vec2()));
                    }
                }
                None
            })
            .expect("화면 오른쪽에 정렬 row should be drawn");
        // Second frame: hover that row (the same hover-opens-submenu
        // interaction already used for the top-level "스티커 정렬" row).
        let _ = full;
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(screen),
                events: vec![egui::Event::PointerMoved(rect.center())],
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    a.sticker_menu(ui, 0, &mut w, true, &mut sub2);
                });
            },
        );
        assert_eq!(sub2.map(|(l, _)| l), Some(Layout::Right));
        assert!(a.arrange.is_none(), "hovering must not arrange immediately");
    }
    #[test]
    fn arrange_side_menu_arranges_with_the_clicked_fold_mode() {
        let mut a = App::sample(Local::now().date_naive());
        let ctx = egui::Context::default();
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(300., 200.));
        let input = || egui::RawInput {
            screen_rect: Some(screen),
            ..Default::default()
        };
        // First frame: locate the "모두 상자로 접기" button.
        let out = ctx.run(input(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                a.arrange_side_menu(ui, Layout::Left);
            });
        });
        let rect = out
            .shapes
            .iter()
            .find_map(|shape| {
                if let egui::Shape::Text(t) = &shape.shape {
                    if t.galley.job.text == "모두 상자로 접기" {
                        return Some(t.galley.rect.translate(t.pos.to_vec2()));
                    }
                }
                None
            })
            .expect("모두 상자로 접기 button should be drawn");
        // Second frame: click it.
        let pos = rect.center();
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(screen),
                events: vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: Default::default(),
                    },
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        modifiers: Default::default(),
                    },
                ],
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    a.arrange_side_menu(ui, Layout::Left);
                });
            },
        );
        let arrange = a.arrange.expect("should have arranged");
        assert_eq!(arrange.layout, Layout::Left);
        assert_eq!(arrange.fold, Some(Mode::Swatch));
    }
}

#[cfg(test)]
mod new_memo_placement_tests {
    use super::*;

    fn area() -> Rect {
        Rect::from_min_size(Pos2::new(1920., 0.), Vec2::new(1920., 1040.))
    }

    #[test]
    fn new_memos_stack_down_the_right_edge_then_start_a_column_to_the_left() {
        let size = Vec2::new(360., 360.);
        let mut open: Vec<Rect> = vec![];
        let mut at = vec![];
        for _ in 0..4 {
            let p = right_slot(area(), size, &open);
            at.push(p);
            open.push(Rect::from_min_size(p, size));
        }
        // The right edge of the monitor, below one another with a gap.
        assert_eq!(at[0], Pos2::new(1920. + 1920. - 14. - 360., 14.));
        assert_eq!(at[1].x, at[0].x);
        assert_eq!(at[1].y, at[0].y + 360. + 8.);
        // A third does not fit below (14 + 3 * 368 > 1040): a new column starts.
        assert!(at[2].x < at[0].x && at[2].y == 14.);
        assert_eq!(at[3].x, at[2].x);
        assert_eq!(at[3].y, at[2].y + 360. + 8.);
        for (i, a) in open.iter().enumerate() {
            for b in &open[i + 1..] {
                assert!(!a.intersects(*b), "windows must not overlap");
            }
        }
    }

    #[test]
    fn a_memo_moved_by_the_user_reopens_where_it_was_left() {
        let today = chrono::Local::now().date_naive();
        let mut a = App::sample(today);
        a.desktop.new_right = true;
        let i = a.new_sticker();
        // Shown once in its right-aligned place, dragged away, then closed.
        {
            let w = a.studio.windows.get_mut(&i).unwrap();
            w.center = false;
            w.last_pos = Some(Pos2::new(500., 300.));
            w.open = false;
        }
        a.open_sticker(i, None);
        let w = &a.studio.windows[&i];
        assert!(w.open && !w.center, "not placed again like a new memo");
        assert_eq!(w.place, Some(Pos2::new(500., 300.)));
        assert_eq!(w.mode, Mode::Sticker);
        // Reopening an open window does not move it.
        a.studio.windows.get_mut(&i).unwrap().place = None;
        a.open_sticker(i, None);
        assert_eq!(a.studio.windows[&i].place, None);
        // A memo never shown still takes a new-memo place.
        let j = a.new_sticker();
        let w = &a.studio.windows[&j];
        assert!(w.center && w.place.is_none());
    }

    #[test]
    fn a_memo_closed_in_the_middle_leaves_its_place_for_the_next_one() {
        let size = Vec2::new(360., 300.);
        let first = Rect::from_min_size(right_slot(area(), size, &[]), size);
        let third = Rect::from_min_size(Pos2::new(first.min.x, first.max.y + 8. + 308.), size);
        // The second one was closed: the next goes where it was.
        let p = right_slot(area(), size, &[first, third]);
        assert_eq!(p, Pos2::new(first.min.x, first.max.y + 8.));
    }

    #[test]
    fn windows_on_another_monitor_do_not_count() {
        let size = Vec2::new(360., 360.);
        let elsewhere = Rect::from_min_size(Pos2::new(1500., 14.), size);
        assert_eq!(
            right_slot(area(), size, &[elsewhere]),
            Pos2::new(1920. + 1920. - 14. - 360., 14.)
        );
    }

    #[test]
    fn new_stickers_use_the_size_from_the_settings_and_open_on_the_right_by_default() {
        let mut app = App::sample(Local::now().date_naive());
        assert!(app.desktop.new_right, "right alignment is on by default");
        assert_eq!(app.desktop.new_monitor, None);
        app.desktop.sticker_size = Vec2::new(500., 420.);
        let i = app.new_sticker();
        assert_eq!(app.studio.windows[&i].sticker_size, Vec2::new(500., 420.));
    }

    #[test]
    fn the_new_memo_settings_are_saved_and_restored() {
        let mut app = App::sample(Local::now().date_naive());
        app.desktop.new_monitor = Some(1);
        app.desktop.new_right = false;
        let rec = crate::storage::SettingsRec {
            new_monitor: app.desktop.new_monitor,
            new_right: app.desktop.new_right,
            ..Default::default()
        };
        let json = serde_json::to_string(&rec).unwrap();
        let back: crate::storage::SettingsRec = serde_json::from_str(&json).unwrap();
        assert_eq!((back.new_monitor, back.new_right), (Some(1), false));
        // Settings saved before these options existed: right alignment on.
        let old: crate::storage::SettingsRec = serde_json::from_str("{}").unwrap();
        assert!(old.new_right && old.new_monitor.is_none());
    }
}

#[cfg(test)]
mod size_window_tests {
    use super::*;

    fn frame(app: &mut App, ctx: &egui::Context, events: Vec<egui::Event>) {
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200., 800.))),
                events,
                ..Default::default()
            },
            |ctx| app.size_probe_window(ctx),
        );
    }
    fn escape() -> Vec<egui::Event> {
        vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }]
    }

    #[test]
    fn closing_the_size_window_enters_its_size_in_the_settings() {
        let mut app = App::sample(Local::now().date_naive());
        let ctx = egui::Context::default();
        app.desktop.sticker_size = Vec2::splat(360.);
        app.desktop.size_probe = true;
        // Nothing happens while it is open and untouched.
        frame(&mut app, &ctx, vec![]);
        assert!(app.desktop.size_probe);
        assert_eq!(app.desktop.sticker_size, Vec2::splat(360.));
        // The user dragged it to this size; the last drawn size is kept.
        app.desktop.size_probe_last = Some(Vec2::new(612.4, 333.6));
        frame(&mut app, &ctx, escape());
        assert!(!app.desktop.size_probe, "it closes");
        assert_eq!(app.desktop.sticker_size, Vec2::new(612., 334.));
        // Sizes outside the limits of the sliders are brought inside them.
        app.desktop.size_probe = true;
        app.desktop.size_probe_last = Some(Vec2::new(1500., 100.));
        frame(&mut app, &ctx, escape());
        assert_eq!(app.desktop.sticker_size, Vec2::new(900., 220.));
    }

    #[test]
    fn closing_the_color_box_window_sets_the_size_folds_start_with() {
        let mut app = App::sample(Local::now().date_naive());
        let ctx = egui::Context::default();
        assert_eq!(app.desktop.swatch_size, Vec2::splat(SWATCH_SIZE));
        app.desktop.swatch_probe = true;
        let run = |app: &mut App, events| {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200., 800.))),
                    events,
                    ..Default::default()
                },
                |ctx| app.swatch_probe_window(ctx),
            );
        };
        run(&mut app, vec![]);
        assert!(app.desktop.swatch_probe, "stays open until closed");
        app.desktop.swatch_probe_last = Some(Vec2::new(80.4, 30.));
        run(&mut app, escape());
        assert!(!app.desktop.swatch_probe);
        assert_eq!(app.desktop.swatch_size, Vec2::new(80., 30.));
        // Out of range sizes come inside it.
        app.desktop.swatch_probe = true;
        app.desktop.swatch_probe_last = Some(Vec2::new(4., 900.));
        run(&mut app, escape());
        assert_eq!(app.desktop.swatch_size, Vec2::new(SWATCH_MIN, SWATCH_MAX));
        // A memo folded into the box takes that size, also after its box
        // was resized before.
        let mut w = Window::new(Mode::Title);
        w.swatch_default = Vec2::new(80., 30.);
        w.swatch_size = Vec2::splat(200.);
        w.fold();
        assert_eq!((w.mode, w.size()), (Mode::Swatch, Vec2::new(80., 30.)));
    }
    #[test]
    fn leaving_out_the_color_box_folds_only_to_the_title_line_and_back() {
        let mut w = Window::new(Mode::Sticker);
        w.skip_swatch = true;
        w.fold();
        assert_eq!(w.mode, Mode::Title);
        w.fold();
        assert_eq!(w.mode, Mode::Sticker);
        // Without it: the title line, the color box, then back.
        w.skip_swatch = false;
        w.fold();
        w.fold();
        assert_eq!(w.mode, Mode::Swatch);
        w.fold();
        assert_eq!(w.mode, Mode::Sticker);
    }
    #[test]
    fn the_fold_settings_are_saved_and_older_saves_keep_the_magnets_on() {
        use crate::storage::SettingsRec;
        let load = |rec: &SettingsRec| {
            let json = serde_json::to_string(rec).unwrap();
            let back: SettingsRec = serde_json::from_str(&json).unwrap();
            let mut d = crate::desktop::Desktop::default();
            back.write_desktop(&mut d);
            d
        };
        let old: SettingsRec = serde_json::from_str("{}").unwrap();
        let d = load(&old);
        assert!(d.magnet && !d.skip_swatch);
        assert_eq!(d.magnet_pt, crate::magnet::SNAP);
        assert_eq!(d.swatch_size, Vec2::splat(SWATCH_SIZE));
        let mut app = App::sample(Local::now().date_naive());
        app.desktop.skip_swatch = true;
        app.desktop.swatch_size = Vec2::new(70., 40.);
        app.desktop.magnet = false;
        app.desktop.magnet_pt = 20.;
        let mut rec = SettingsRec::default();
        rec.read_desktop(&app.desktop);
        let d = load(&rec);
        assert!(d.skip_swatch && !d.magnet);
        assert_eq!((d.swatch_size, d.magnet_pt), (Vec2::new(70., 40.), 20.));
        // Both steps skipped (a file edited by hand): the color box stays.
        rec.skip_title = true;
        let d = load(&rec);
        assert!(d.skip_title && !d.skip_swatch);
    }
    #[test]
    fn only_stickers_and_folded_memos_on_screen_come_to_the_front() {
        let mut app = App::sample(Local::now().date_naive());
        for (k, mode, open) in [
            (0, Mode::Sticker, true),
            (1, Mode::Title, true),
            (2, Mode::Swatch, true),
            (3, Mode::Editor, true),
            (4, Mode::Sticker, false),
            (5, Mode::Sticker, true),
        ] {
            let mut w = Window::new(mode);
            w.open = open;
            // The last one was never drawn: no window yet.
            w.hwnd = (k != 5).then_some(100 + k as isize);
            app.studio.windows.insert(k, w);
        }
        assert_eq!(app.memo_windows(), vec![100, 101, 102]);
        // On by default, also for saves made before it; it can be turned off.
        use crate::storage::SettingsRec;
        let mut d = crate::desktop::Desktop::default();
        assert!(d.memo_front);
        let old: SettingsRec = serde_json::from_str("{}").unwrap();
        old.write_desktop(&mut d);
        assert!(d.memo_front);
        d.memo_front = false;
        let mut rec = SettingsRec::default();
        rec.read_desktop(&d);
        let mut back = crate::desktop::Desktop::default();
        rec.write_desktop(&mut back);
        assert!(!back.memo_front);
    }
    #[test]
    fn new_memos_take_the_colors_in_turn_and_the_turn_is_saved() {
        use crate::storage::SettingsRec;
        let mut app = App::sample(Local::now().date_naive());
        let one = app.new_sticker();
        assert_eq!(app.data.notes[one].color, app.desktop.default_color);
        app.desktop.cycle_color = true;
        app.desktop.color_turn = PALETTE.len() - 2;
        let colors: Vec<Color32> = (0..3)
            .map(|_| {
                let i = app.new_sticker();
                app.data.notes[i].color
            })
            .collect();
        let n = PALETTE.len();
        assert_eq!(colors, vec![PALETTE[n - 2], PALETTE[n - 1], PALETTE[0]]);
        assert_eq!(app.desktop.next_new_color(), PALETTE[1]);
        let mut rec = SettingsRec::default();
        rec.read_desktop(&app.desktop);
        let back: SettingsRec =
            serde_json::from_str(&serde_json::to_string(&rec).unwrap()).unwrap();
        let mut d = crate::desktop::Desktop::default();
        back.write_desktop(&mut d);
        assert!(d.cycle_color);
        assert_eq!(d.color_turn, 1);
        // Saves made before it: one color as before.
        let old: SettingsRec = serde_json::from_str("{}").unwrap();
        old.write_desktop(&mut d);
        assert!(!d.cycle_color);
    }
    #[test]
    fn closing_the_title_width_window_sets_the_width_folds_start_with() {
        let mut app = App::sample(Local::now().date_naive());
        let ctx = egui::Context::default();
        assert_eq!(app.desktop.title_width, TITLE_WIDTH);
        app.desktop.title_probe = true;
        let run = |app: &mut App, events| {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200., 800.))),
                    events,
                    ..Default::default()
                },
                |ctx| app.title_probe_window(ctx),
            );
        };
        run(&mut app, vec![]);
        assert!(app.desktop.title_probe, "stays open until closed");
        app.desktop.title_probe_last = Some(241.6);
        run(&mut app, escape());
        assert!(!app.desktop.title_probe);
        assert_eq!(app.desktop.title_width, 242.);
        // Out of range widths come inside it.
        app.desktop.title_probe = true;
        app.desktop.title_probe_last = Some(40.);
        run(&mut app, escape());
        assert_eq!(app.desktop.title_width, TITLE_MIN);
        // A memo folded once takes that width, also after it was resized
        // as a title line before.
        let mut w = Window::new(Mode::Sticker);
        w.title_default = 242.;
        w.title_width = 700.;
        w.fold();
        assert_eq!((w.mode, w.size()), (Mode::Title, Vec2::new(242., 44.)));
        // Saved and restored with the settings; old saves get the default.
        let mut rec = crate::storage::SettingsRec::default();
        app.desktop.title_width = 242.;
        rec.read_desktop(&app.desktop);
        let back: crate::storage::SettingsRec =
            serde_json::from_str(&serde_json::to_string(&rec).unwrap()).unwrap();
        let mut d = crate::desktop::Desktop::default();
        back.write_desktop(&mut d);
        assert_eq!(d.title_width, 242.);
        let old: crate::storage::SettingsRec = serde_json::from_str("{}").unwrap();
        old.write_desktop(&mut d);
        assert_eq!(d.title_width, TITLE_WIDTH);
    }

    #[test]
    fn the_new_memo_group_shows_in_the_settings_with_the_size_button() {
        let mut app = App::sample(Local::now().date_naive());
        app.desktop.settings_tab = 2;
        let ctx = egui::Context::default();
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1000., 1200.))),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| app.ppt_settings(ui));
            },
        );
        let texts: Vec<String> = output
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Text(t) => Some(t.galley.job.text.clone()),
                _ => None,
            })
            .collect();
        for wanted in [
            "새 메모",
            "새 스티커 메모 크기",
            "메모창으로 크기 설정하기",
            "새 메모 기본 색",
            "새 메모 띄울 모니터",
            "새 메모 오른쪽 정렬하기",
        ] {
            assert!(texts.iter().any(|t| t == wanted), "missing: {wanted}");
        }
        // The memo window tab: the title width button under "접기 1단계 건너뛰기".
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1000., 2400.))),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| app.ppt_settings(ui));
            },
        );
        let at = |wanted: &str| {
            output.shapes.iter().find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.job.text == wanted => Some(t.pos.y),
                _ => None,
            })
        };
        let skip = at("접기 1단계 건너뛰기").expect("skip option");
        let button = at("1단 접기 크기 조절하기").expect("title width button");
        assert!(button > skip);
    }

    #[test]
    fn each_settings_option_sits_with_what_it_changes() {
        let mut app = App::sample(Local::now().date_naive());
        let ctx = egui::Context::default();
        let mut texts_of = |tab: usize| -> Vec<String> {
            app.desktop.settings_tab = tab;
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1000., 3000.))),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| app.ppt_settings(ui));
                },
            );
            output
                .shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    egui::Shape::Text(t) => Some(t.galley.job.text.clone()),
                    _ => None,
                })
                .collect()
        };
        let sticker = [
            "메모창을 작업표시줄에 표시",
            "스티커 메모 체크완료 시 스티커 닫기",
            "메모 정렬 모니터 정하기",
            "매일 오늘 메모 자동으로 띄우기",
        ];
        let calendar = "메모 끌어 놓기 시 이날까지 업무 선택창 띄우기";
        let general = texts_of(0);
        assert!(general.iter().any(|t| t == "Windows 시작 시 자동 실행"));
        for moved in sticker.iter().chain([&calendar]) {
            assert!(
                !general.iter().any(|t| t == moved),
                "still in 일반: {moved}"
            );
        }
        let memo = texts_of(2);
        for wanted in sticker
            .iter()
            .chain(&["스티커 동작", "접기", "본문 기본값"])
        {
            assert!(
                memo.iter().any(|t| t == wanted),
                "missing in 메모창: {wanted}"
            );
        }
        assert!(
            texts_of(3).iter().any(|t| t == calendar),
            "missing in 캘린더"
        );
    }
}
