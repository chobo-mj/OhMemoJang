//! The detached calendar window, the color box it minimizes to, and the
//! standing calendar that sits on the desktop like wallpaper.
use crate::win_style::PxRect;
use crate::*;
use egui::{Pos2, Rect, Sense, Stroke};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DockState {
    Hidden,
    Window,
    Box,
}

/// An easy-on-the-eyes look for the detached calendar.
pub struct CalTheme {
    pub name: &'static str,
    pub dark: bool,
    pub bg: Color32,
    pub card: Color32,
    pub line: Color32,
    pub ink: Color32,
    pub muted: Color32,
    pub faint: Color32,
    pub select: Color32,
    pub today: Color32,
    pub sun: Color32,
    pub sat: Color32,
}

/// Index 0 follows the app's own look.
pub const THEMES: [CalTheme; 3] = [
    CalTheme {
        name: "종이",
        dark: false,
        bg: Color32::from_rgb(244, 239, 228),
        card: Color32::from_rgb(251, 248, 241),
        line: Color32::from_rgb(226, 217, 199),
        ink: Color32::from_rgb(72, 64, 54),
        muted: Color32::from_rgb(128, 118, 104),
        faint: Color32::from_rgb(170, 160, 145),
        select: Color32::from_rgb(240, 228, 206),
        today: Color32::from_rgb(190, 118, 82),
        sun: Color32::from_rgb(184, 98, 80),
        sat: Color32::from_rgb(86, 116, 156),
    },
    CalTheme {
        name: "세이지",
        dark: false,
        bg: Color32::from_rgb(230, 237, 229),
        card: Color32::from_rgb(242, 246, 241),
        line: Color32::from_rgb(205, 217, 205),
        ink: Color32::from_rgb(50, 68, 56),
        muted: Color32::from_rgb(104, 122, 108),
        faint: Color32::from_rgb(146, 162, 150),
        select: Color32::from_rgb(216, 229, 216),
        today: Color32::from_rgb(86, 136, 104),
        sun: Color32::from_rgb(172, 98, 88),
        sat: Color32::from_rgb(78, 114, 148),
    },
    CalTheme {
        name: "나이트",
        dark: true,
        bg: Color32::from_rgb(33, 37, 45),
        card: Color32::from_rgb(40, 45, 54),
        line: Color32::from_rgb(57, 63, 75),
        ink: Color32::from_rgb(212, 218, 226),
        muted: Color32::from_rgb(150, 158, 172),
        faint: Color32::from_rgb(112, 120, 134),
        select: Color32::from_rgb(52, 60, 74),
        today: Color32::from_rgb(226, 156, 92),
        sun: Color32::from_rgb(222, 128, 116),
        sat: Color32::from_rgb(128, 168, 218),
    },
];

struct Standing {
    hwnd: isize,
    /// Client area on screen, physical pixels.
    rect: PxRect,
    /// The whole window (with title bar) before it was pinned.
    outer: PxRect,
    /// The standing button relative to the client origin, physical pixels.
    button: PxRect,
    pinned: bool,
    /// Frames to wait for the title bar to go before moving to the desktop.
    wait: u8,
    click: Option<crate::win_style::DesktopClick>,
    /// Pinned over the desktop icons, taking input (설정 › 캘린더).
    over_icons: bool,
}

/// What the standing calendar can let through (설정 › 캘린더 › 스탠딩 캘린더).
#[derive(Clone, Copy, Debug)]
pub enum StandFeature {
    /// A date's + button: a new memo on that day.
    NewMemo,
    /// Dragging over dates: a schedule of several days.
    Range,
    /// A sticker dropped on a date becomes a schedule.
    Drop,
    /// Clicking or right-clicking a memo's band.
    Band,
}

pub struct Dock {
    pub state: DockState,
    /// Outer position and inner size a re-created calendar window opens at.
    open_at: Option<(Pos2, Vec2)>,
    window_at: Option<(Pos2, Vec2)>,
    box_spawn: Pos2,
    box_at: Option<Pos2>,
    pub box_size: f32,
    pub box_color: Color32,
    box_resize: Option<(f32, Pos2)>,
    /// Native window moves of the box (also after very fast drags).
    box_native: crate::native_window::State,
    box_menu: Option<(Pos2, bool)>,
    /// 0 follows the app; 1.. picks `THEMES[theme - 1]`.
    pub theme: usize,
    /// True while the calendar grid is drawn inside the calendar window.
    pub painting: bool,
    standing: Option<Standing>,
    /// When the standing calendar last went back to a window: the same
    /// click can reach both the desktop watch and the window, and must not
    /// pin it again.
    unpinned_at: Option<f64>,
    /// Whether the calendar window last drew itself pinned to the desktop.
    drew_pinned: bool,
    focus_requested: bool,
    /// True: the detached window shows the grid and the day's memos side by
    /// side instead of stacked.
    pub horizontal: bool,
    /// Fraction of the horizontal layout's width given to the day-detail
    /// pane on the right. Kept as a fraction (not pixels) so the split stays
    /// proportional as the window is resized.
    split_frac: f32,
    /// Frames until the calendar, standing at the last quit, stands on the
    /// desktop again (0: nothing pending).
    stand_on_start: u8,
    /// The calendar window is left out for one frame, so egui lets its
    /// (dead) native window go and makes a new one.
    recreate: bool,
    /// Times the standing calendar stood again by itself after losing its
    /// place on the desktop, since the button was last pressed.
    restands: u8,
}

impl Dock {
    /// The calendar stands on the desktop.
    pub fn standing_pinned(&self) -> bool {
        self.standing.as_ref().is_some_and(|s| s.pinned)
    }
    /// The calendar is standing on the desktop (or about to).
    pub fn standing_active(&self) -> bool {
        self.standing.is_some()
    }
    /// Stay a window: a stand still waiting from the last run is dropped.
    pub fn stay_window(&mut self) {
        self.stand_on_start = 0;
    }
}

impl Default for Dock {
    fn default() -> Self {
        Self {
            state: DockState::Hidden,
            open_at: None,
            window_at: None,
            box_spawn: Pos2::new(120., 120.),
            box_at: None,
            box_size: 56.,
            box_color: PALETTE[6],
            box_resize: None,
            box_native: Default::default(),
            box_menu: None,
            theme: 0,
            painting: false,
            standing: None,
            unpinned_at: None,
            drew_pinned: false,
            focus_requested: false,
            horizontal: false,
            split_frac: 1. / 3.,
            stand_on_start: 0,
            recreate: false,
            restands: 0,
        }
    }
}

impl Dock {
    /// The theme the calendar grid uses right now, if not the app's own.
    pub fn active(&self) -> Option<&'static CalTheme> {
        if self.painting && self.theme > 0 {
            THEMES.get(self.theme - 1)
        } else {
            None
        }
    }
    /// Where the calendar was, for the next start.
    pub fn write_place(&self, s: &mut crate::storage::SettingsRec) {
        s.cal_state = match self.state {
            DockState::Hidden => 0,
            DockState::Window => 1,
            DockState::Box => 2,
        };
        let at = if self.state == DockState::Box {
            self.open_at.or(self.window_at)
        } else {
            self.window_at.or(self.open_at)
        };
        s.cal_window = at.map_or([0.; 4], |(p, v)| [p.x, p.y, v.x, v.y]);
        s.cal_box_at = self.box_at.map(|p| [p.x, p.y]);
        s.cal_standing =
            self.state == DockState::Window && (self.standing.is_some() || self.stand_on_start > 0);
    }
    /// The calendar comes back the way it was at the last quit.
    pub fn read_place(&mut self, s: &crate::storage::SettingsRec) {
        let [x, y, w, h] = s.cal_window;
        if w >= 200. && h >= 200. && x.is_finite() && y.is_finite() {
            let at = (Pos2::new(x, y), Vec2::new(w, h));
            self.window_at = Some(at);
            self.open_at = Some(at);
        }
        if let Some([x, y]) = s.cal_box_at.filter(|p| p.iter().all(|v| v.is_finite())) {
            self.box_at = Some(Pos2::new(x, y));
        }
        self.state = match s.cal_state {
            1 => DockState::Window,
            2 => {
                self.box_spawn = self
                    .box_at
                    .or(self.window_at.map(|(p, _)| p))
                    .unwrap_or(self.box_spawn);
                DockState::Box
            }
            _ => DockState::Hidden,
        };
        // A few frames after the window is up, as if its button was pressed.
        self.stand_on_start = if s.cal_standing && self.state == DockState::Window {
            10
        } else {
            0
        };
    }
    /// Minimize the calendar window into its color box.
    fn minimize_to_box(&mut self) {
        self.open_at = self.window_at;
        self.box_spawn = self
            .box_at
            .or(self.window_at.map(|(p, _)| p))
            .unwrap_or(self.box_spawn);
        self.state = DockState::Box;
    }
}

fn luminance(c: Color32) -> f32 {
    (0.299 * f32::from(c.r()) + 0.587 * f32::from(c.g()) + 0.114 * f32::from(c.b())) / 255.
}
fn px(r: Rect, ppp: f32) -> PxRect {
    [
        (r.min.x * ppp).round() as i32,
        (r.min.y * ppp).round() as i32,
        (r.max.x * ppp).round() as i32,
        (r.max.y * ppp).round() as i32,
    ]
}

impl App {
    /// Whether `f` works on the standing calendar: only when turned on in
    /// settings, as chosen there. Everywhere else always.
    pub fn stand_feature_on(&self, f: StandFeature) -> bool {
        if !self.cal.standing_active() {
            return true;
        }
        let p = &self.prefs;
        p.stand_input
            && match f {
                StandFeature::NewMemo => p.stand_new_memo,
                StandFeature::Range => p.stand_range,
                StandFeature::Drop => p.stand_drop,
                StandFeature::Band => p.stand_band,
            }
    }
    /// [`Self::stand_feature_on`] for what is being drawn: only the
    /// calendar window's own grid is the standing calendar.
    pub fn stand_allows(&self, f: StandFeature) -> bool {
        !self.cal.painting || self.stand_feature_on(f)
    }
    /// Show the calendar window (from the main window or the shortcut).
    pub fn show_calendar(&mut self) {
        self.restore_standing();
        self.cal.state = DockState::Window;
        self.cal.focus_requested = true;
    }
    pub fn calendar_dock(&mut self, ctx: &egui::Context) {
        // Its window went with the desktop it stood on (Explorer restarted):
        // never painted into, a new one stands in its place.
        if self
            .cal
            .standing
            .as_ref()
            .is_some_and(|s| s.pinned && !crate::win_style::window_alive(s.hwnd))
        {
            self.restand(ctx);
        }
        // The input option changed while standing: it stands again the
        // new way (over or under the desktop icons).
        if self
            .cal
            .standing
            .as_ref()
            .is_some_and(|s| s.pinned && s.over_icons != self.prefs.stand_input)
        {
            self.restore_standing();
            if self.cal.standing.is_none() {
                self.cal.stand_on_start = 3;
            }
        }
        if std::mem::take(&mut self.cal.recreate) {
            // Left out for this frame: egui lets the old window go.
            ctx.request_repaint();
            return;
        }
        match self.cal.state {
            DockState::Window => self.calendar_window(ctx),
            DockState::Box => self.calendar_box(ctx),
            DockState::Hidden => {}
        }
    }
    fn calendar_window(&mut self, ctx: &egui::Context) {
        let standing = self.cal.standing.is_some();
        let mut builder = egui::ViewportBuilder::default()
            .with_title("OhMemoJang · 캘린더")
            // The taskbar shows the program's icon, not eframe's.
            .with_icon(crate::app_icon::window())
            .with_inner_size([860., 760.])
            .with_min_inner_size([480., 420.])
            .with_decorations(!standing);
        if let Some((at, size)) = self.cal.open_at {
            builder = builder.with_position(at).with_inner_size(size);
        }
        let mut minimize = false;
        let mut close = false;
        let mut stand = false;
        let mut stand_rect = Rect::NOTHING;
        ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of("calendar"),
            builder,
            |ctx, _| {
                let theme = self.cal.theme.checked_sub(1).and_then(|t| THEMES.get(t));
                self.theme_with(ctx, theme.map_or(self.dark, |t| t.dark));
                // Painted once right after it was pinned under the desktop
                // icons or let go, even though it looks the same: it moved
                // to another layer (`vendor/eframe` skips same-looking frames).
                let pinned = self.cal.standing_pinned();
                let me = ctx.viewport_id();
                if pinned != self.cal.drew_pinned {
                    ctx.data_mut(|d| {
                        d.insert_temp(egui::Id::new("eframe_paint_once").with(me), true)
                    });
                }
                self.cal.drew_pinned = pinned;
                if !standing && std::mem::take(&mut self.cal.focus_requested) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                self.drag_input(ctx);
                if ctx.input(|i| i.viewport().close_requested()) {
                    // The window's close button closes the calendar; only
                    // the minimize button folds it into the color box.
                    close = true;
                }
                let (outer, inner, ppp) = ctx.input(|i| {
                    let v = i.viewport();
                    (
                        v.outer_rect,
                        v.inner_rect,
                        v.native_pixels_per_point.unwrap_or(1.),
                    )
                });
                if !standing {
                    if let (Some(o), Some(i)) = (outer, inner) {
                        self.cal.window_at = Some((o.min, i.size()));
                    }
                }
                let fill = theme.map_or(ctx.style().visuals.panel_fill, |t| t.bg);
                egui::CentralPanel::default()
                    .frame(
                        egui::Frame::NONE
                            .fill(fill)
                            .inner_margin(egui::Margin::symmetric(20, 14)),
                    )
                    .show(ctx, |ui| {
                        let (pressed, button) = self.calendar_toolbar(ui, &mut minimize);
                        crate::tutorial::mark(ui.ctx(), crate::tutorial::Spot::Stand, button);
                        stand_rect = button;
                        let now = ui.input(|i| i.time);
                        let just_unpinned = self
                            .cal
                            .unpinned_at
                            .is_some_and(|t| (0. ..0.6).contains(&(now - t)));
                        if pressed && !just_unpinned {
                            stand = true;
                            self.cal.restands = 0;
                        }
                        ui.add_space(6.);
                        self.cal.painting = true;
                        if self.cal.horizontal {
                            self.calendar_horizontal(ui);
                        } else {
                            egui::ScrollArea::vertical().show(ui, |ui| self.calendar(ui));
                        }
                        self.cal.painting = false;
                    });
                if !standing && self.calendar_view_menu(ctx) {
                    stand = true;
                    self.cal.restands = 0;
                }
                if self.cal.stand_on_start > 0 && !standing {
                    self.cal.stand_on_start -= 1;
                    if self.cal.stand_on_start == 0 && outer.is_some() && inner.is_some() {
                        stand = true;
                    } else {
                        ctx.request_repaint_after(std::time::Duration::from_millis(50));
                    }
                }
                // The calendar keeps its current place on the desktop, so
                // the stand button's screen rect is known from here. Either
                // the toolbar icon or the right-click "캘린더 분리하기" menu
                // entry can trigger this.
                if stand && !standing {
                    if let (Some(hwnd), Some(i), Some(o)) =
                        (crate::win_style::hwnd(ctx), inner, outer)
                    {
                        self.cal.standing = Some(Standing {
                            hwnd,
                            rect: px(i, ppp),
                            outer: px(o, ppp),
                            button: px(stand_rect, ctx.pixels_per_point()),
                            pinned: false,
                            wait: 3,
                            click: None,
                            over_icons: self.prefs.stand_input,
                        });
                    }
                }
                if let Some(s) = self.cal.standing.as_mut() {
                    s.button = px(stand_rect, ctx.pixels_per_point());
                    if let Some(click) = &s.click {
                        click.update(s.button);
                    }
                }
            },
        );
        if standing && stand {
            self.restore_standing();
            self.cal.unpinned_at = Some(ctx.input(|i| i.time));
            ctx.request_repaint();
        } else {
            self.standing_frame(ctx);
        }
        if minimize && self.cal.standing.is_none() {
            self.cal.minimize_to_box();
        } else if close && self.cal.standing.is_none() {
            self.cal.state = DockState::Hidden;
        }
    }
    /// The grid on the left and the selected day's memos on the right,
    /// separated by a draggable splitter. `self.cal.split_frac` is a
    /// fraction of the available width, not a pixel count, so dragging the
    /// window itself keeps both panes' proportions rather than clipping one.
    fn calendar_horizontal(&mut self, ui: &mut egui::Ui) {
        let rect = ui.available_rect_before_wrap();
        let height = rect.height();
        let total = rect.width();
        let handle_w = 12.;
        let min_right = 220.0_f32.min((total - handle_w) * 0.5);
        let max_right = (total - handle_w - 320.).max(min_right);
        let right_w = (total * self.cal.split_frac).clamp(min_right, max_right);
        let left_w = (total - handle_w - right_w).max(240.);

        let left_rect = Rect::from_min_size(rect.min, Vec2::new(left_w, height));
        let handle = Rect::from_min_max(
            egui::pos2(left_rect.right(), rect.top()),
            egui::pos2(left_rect.right() + handle_w, rect.bottom()),
        );
        let right_rect = Rect::from_min_size(
            egui::pos2(handle.right(), rect.top()),
            Vec2::new(total - left_w - handle_w, height),
        );

        let mut left = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("cal_grid_pane")
                .max_rect(left_rect)
                .layout(egui::Layout::top_down(egui::Align::LEFT)),
        );
        left.set_clip_rect(ui.clip_rect().intersect(left_rect));
        egui::ScrollArea::vertical()
            .id_salt("cal_grid_scroll")
            .show(&mut left, |ui| self.calendar_grid(ui));

        let drag = ui
            .interact(
                handle,
                ui.id().with("cal_horizontal_splitter"),
                Sense::drag(),
            )
            .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
        if drag.hovered() || drag.dragged() {
            ui.painter().line_segment(
                [handle.center_top(), handle.center_bottom()],
                Stroke::new(2.0_f32, ACCENT.gamma_multiply(0.6)),
            );
        }
        if drag.dragged() {
            let new_right = (right_w - drag.drag_delta().x).clamp(min_right, max_right);
            self.cal.split_frac = (new_right / total).clamp(0.15, 0.75);
        }

        let mut right = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("cal_detail_pane")
                .max_rect(right_rect)
                .layout(egui::Layout::top_down(egui::Align::LEFT)),
        );
        right.set_clip_rect(ui.clip_rect().intersect(right_rect));
        egui::ScrollArea::vertical()
            .id_salt("cal_detail_scroll")
            .show(&mut right, |ui| self.calendar_day_detail(ui));

        ui.allocate_rect(rect, Sense::hover());
    }
    /// At quit: lets go of the standing calendar (its desktop click watch
    /// ends, and the window leaves the desktop layer before it is destroyed,
    /// so no picture of it stays on the wallpaper). It is saved as standing,
    /// to stand again at the next start.
    pub fn release_standing(&mut self) {
        if let Some(s) = self.cal.standing.take() {
            if s.pinned {
                crate::win_style::let_go_of_desktop(s.hwnd);
            }
            if self.cal.state == DockState::Window {
                self.cal.stand_on_start = 10;
            }
        }
    }
    /// Pin, poll and unpin the standing calendar.
    pub(crate) fn restore_standing(&mut self) {
        if let Some(s) = self.cal.standing.as_ref() {
            if s.pinned {
                if !crate::win_style::window_alive(s.hwnd) {
                    // Gone with the desktop layer it stood on: what it
                    // showed is cleared, and a new window is made.
                    crate::win_style::refresh_desktop(s.rect);
                    self.cal.recreate = true;
                } else if !crate::win_style::unpin_from_desktop(s.hwnd, s.outer) {
                    self.toast =
                        "분리 창으로 돌아오지 못했습니다. 해제 버튼을 다시 눌러 주세요".into();
                    return;
                }
            }
        }
        self.cal.standing = None;
    }
    /// The standing calendar lost its place on the desktop (Explorer made a
    /// new desktop layer, or the window went with the old one): it comes
    /// back as a window and stands again by itself, a few times at most.
    fn restand(&mut self, ctx: &egui::Context) {
        self.restore_standing();
        if self.cal.standing.is_some() {
            return;
        }
        self.cal.restands = self.cal.restands.saturating_add(1);
        if self.cal.restands <= 3 {
            self.cal.stand_on_start = 10;
        } else {
            self.toast = "바탕화면이 바뀌어 스탠딩 캘린더를 분리 창으로 돌려놓았어요".into();
        }
        ctx.request_repaint();
    }
    fn standing_frame(&mut self, ctx: &egui::Context) {
        let Some(s) = self.cal.standing.as_mut() else {
            return;
        };
        if !s.pinned {
            // Wait until the title bar is gone, then move to the desktop.
            if s.wait > 0 {
                s.wait = s.wait.saturating_sub(1);
                ctx.request_repaint_after(std::time::Duration::from_millis(16));
                return;
            }
            s.pinned = crate::win_style::pin_to_desktop(s.hwnd, s.rect, s.over_icons);
            if !s.pinned {
                self.cal.standing = None;
                return;
            }
            s.click =
                crate::win_style::DesktopClick::watch(s.hwnd, s.button, ctx.clone(), s.over_icons);
            // Paint it once more where it now stands.
            ctx.request_repaint();
            if s.click.is_none() {
                self.restore_standing();
                self.toast = "바탕화면 입력 감지를 시작하지 못해 분리 창으로 유지합니다".into();
                ctx.request_repaint();
            }
            return;
        }
        if s.click.as_ref().is_some_and(|c| c.lost()) {
            self.restand(ctx);
            return;
        }
        if s.click.as_ref().is_some_and(|c| c.take()) {
            // Back where it was, title bar included.
            self.restore_standing();
            self.cal.unpinned_at = Some(ctx.input(|i| i.time));
            ctx.request_repaint();
        }
        // Its place on the desktop is kept by the watch's own thread
        // (`win_style`), without redrawing the program.
    }
    /// Right-click anywhere in the calendar window: month, 1-week or
    /// 2-week view (the same setting as Settings › Calendar), plus
    /// "캘린더 분리하기" to pin the calendar to the desktop (the same action
    /// as the toolbar's stand icon). Returns whether that entry was clicked,
    /// so the caller can pin the window the same way the toolbar does.
    fn calendar_view_menu(&mut self, ctx: &egui::Context) -> bool {
        let id = egui::Id::new("calendar_view_menu");
        let (secondary, pointer) =
            ctx.input(|i| (i.pointer.secondary_clicked(), i.pointer.interact_pos()));
        // A memo (band or row) has its own menu for that click.
        if secondary && !crate::float::right_click_claimed(ctx) {
            if let Some(p) = pointer {
                // Its own window: it may reach past the calendar window.
                crate::float::open_at(
                    ctx,
                    id,
                    crate::float::screen_origin(ctx) + p.to_vec2(),
                    170.,
                );
            }
            return false;
        }
        let mut separate = false;
        crate::float::menu(ctx, id, |ui| {
            ui.label(RichText::new("보기 단위").size(12.).color(ui::t(ui).muted));
            for (k, label) in ["한 달 보기", "1주 보기", "2주 보기"]
                .into_iter()
                .enumerate()
            {
                let on = self.prefs.cal_weeks == k;
                let text = if on {
                    RichText::new(format!("✓  {label}")).color(ACCENT)
                } else {
                    RichText::new(format!("     {label}"))
                };
                if ui.add(egui::Button::new(text).frame(false)).clicked() {
                    self.prefs.cal_weeks = k;
                }
            }
            ui.separator();
            if ui
                .add(egui::Button::new("캘린더 분리하기").frame(false))
                .on_hover_text("바탕화면에 고정 · 도구줄의 스탠딩 아이콘과 같은 동작")
                .clicked()
            {
                separate = true;
            }
        });
        separate
    }
    /// Theme picker, standing toggle and the big minimize button. Returns
    /// whether the standing button was pressed and its rect.
    fn calendar_toolbar(&mut self, ui: &mut egui::Ui, minimize: &mut bool) -> (bool, Rect) {
        let mut stand = false;
        let mut stand_rect = Rect::NOTHING;
        let standing = self.cal.standing.is_some();
        ui.horizontal(|ui| {
            ui.set_min_height(40.);
            let tone = ui::t(ui);
            ui.label(RichText::new("테마").color(tone.muted).size(12.));
            let follow = ui.visuals().panel_fill;
            for k in 0..=THEMES.len() {
                let (color, name) = match k {
                    0 => (follow, "기본"),
                    _ => (THEMES[k - 1].bg, THEMES[k - 1].name),
                };
                let (rect, r) = ui.allocate_exact_size(Vec2::splat(24.), Sense::click());
                let ring = if self.cal.theme == k {
                    ACCENT
                } else {
                    tone.line
                };
                ui.painter().circle_filled(rect.center(), 9., color);
                ui.painter().circle_stroke(
                    rect.center(),
                    10.,
                    Stroke::new(if self.cal.theme == k { 2.0_f32 } else { 1.0 }, ring),
                );
                if r.on_hover_text(name).clicked() {
                    self.cal.theme = k;
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (rect, r) = ui.allocate_exact_size(Vec2::new(96., 38.), Sense::click());
                let fill = if r.hovered() {
                    ui::lerp_color(tone.accent_soft, ACCENT, 0.25)
                } else {
                    tone.accent_soft
                };
                ui.painter().rect_filled(rect, 10., fill);
                icons::paint_icon(
                    ui.painter(),
                    Rect::from_center_size(
                        rect.left_center() + Vec2::new(22., 0.),
                        Vec2::splat(20.),
                    ),
                    icons::Icon::Minus,
                    ACCENT,
                );
                ui.painter().text(
                    rect.left_center() + Vec2::new(38., 0.),
                    egui::Align2::LEFT_CENTER,
                    "최소화",
                    egui::FontId::proportional(14.),
                    ACCENT,
                );
                if r.on_hover_text("작은 캘린더 상자로 접기")
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                    && !standing
                {
                    *minimize = true;
                }
                ui.add_space(4.);
                let horizontal = self.cal.horizontal;
                if icons::toggle_icon_sized(
                    ui,
                    icons::Icon::SplitView,
                    if horizontal {
                        "세로 배치로 전환"
                    } else {
                        "가로 배치로 전환 · 달력과 메모를 나란히"
                    },
                    horizontal,
                    38.,
                )
                .clicked()
                {
                    self.cal.horizontal = !horizontal;
                }
                ui.add_space(4.);
                let r = icons::toggle_icon_sized(
                    ui,
                    icons::Icon::Stand,
                    if standing {
                        "스탠딩 해제 · 이 아이콘을 다시 누르세요"
                    } else {
                        "스탠딩 캘린더 · 바탕화면에 고정"
                    },
                    standing,
                    38.,
                );
                stand_rect = r.rect;
                stand = r.clicked();
                // Standing does not work on Windows 11 24H2 itself (the
                // button stays, to try anyway).
                if !standing && crate::system::standing_unsupported() {
                    ui.label(RichText::new("지원 안 됨").color(tone.muted).size(10.5))
                        .on_hover_text("Windows 11 24H2에서는 스탠딩 캘린더가 동작하지 않아요");
                }
                if standing {
                    ui.label(
                        RichText::new("바탕화면에 고정됨 · 아이콘을 다시 누르면 해제")
                            .color(tone.muted)
                            .size(12.),
                    );
                }
            });
        });
        (stand, stand_rect)
    }
    fn calendar_box(&mut self, ctx: &egui::Context) {
        let size = Vec2::splat(self.cal.box_size);
        let builder = egui::ViewportBuilder::default()
            .with_title("OhMemoJang · 캘린더 상자")
            .with_position(self.cal.box_spawn)
            .with_inner_size(size)
            .with_decorations(false)
            .with_resizable(false)
            .with_taskbar(false)
            .with_transparent(true);
        let mut open = false;
        let mut menu_at = None;
        ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of("calendar_box"),
            builder,
            |ctx, _| {
                self.theme(ctx);
                crate::win_style::shape(ctx, 1.);
                let (outer, inner) =
                    ctx.input(|i| (i.viewport().outer_rect, i.viewport().inner_rect));
                if let Some(o) = outer {
                    self.cal.box_at = Some(o.min);
                }
                self.cal
                    .box_native
                    .input(ctx, false, false, false, size, size, 7.);
                let color = self.cal.box_color;
                let radius = if crate::win_style::NATIVE { 0 } else { 10 };
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE.fill(color).corner_radius(radius))
                    .show(ctx, |ui| {
                        let rect = ui.max_rect();
                        let resizing = self.box_resize(ctx, rect.size());
                        let ink = if luminance(color) > 0.55 {
                            INK
                        } else {
                            Color32::WHITE
                        };
                        let icon = rect.size().min_elem() * 0.56;
                        icons::paint_icon(
                            ui.painter(),
                            Rect::from_center_size(rect.center(), Vec2::splat(icon)),
                            icons::Icon::Today,
                            ink,
                        );
                        let r = ui
                            .interact(
                                rect.shrink(6.),
                                ui.id().with("box"),
                                Sense::click_and_drag(),
                            )
                            .on_hover_text("누르면 캘린더 열기 · 끌어서 이동 · 우클릭: 색/닫기")
                            .on_hover_cursor(egui::CursorIcon::PointingHand);
                        if (r.drag_started() || r.drag_stopped()) && !resizing {
                            self.cal.box_native.start_drag(ctx);
                        }
                        if r.clicked() {
                            open = true;
                        }
                        if r.secondary_clicked() {
                            if let (Some(i), Some(p)) = (inner, r.interact_pointer_pos()) {
                                menu_at = Some(i.min + p.to_vec2());
                            }
                        }
                    });
            },
        );
        if let Some(p) = menu_at {
            self.cal.box_menu = Some((p, false));
        }
        self.calendar_box_menu(ctx);
        if open {
            self.cal.box_menu = None;
            self.cal.state = DockState::Window;
        }
    }
    /// Resize the box from its right or bottom edge. Returns true while resizing.
    fn box_resize(&mut self, ctx: &egui::Context, size: Vec2) -> bool {
        let (pointer, down, pressed) = ctx.input(|v| {
            (
                v.pointer.latest_pos(),
                v.pointer.primary_down(),
                v.pointer.primary_pressed(),
            )
        });
        let band = 8.;
        if let Some(p) = pointer {
            let edge = p.x >= size.x - band || p.y >= size.y - band;
            if edge || self.cal.box_resize.is_some() {
                ctx.set_cursor_icon(egui::CursorIcon::ResizeNwSe);
            }
            if pressed && edge {
                self.cal.box_resize = Some((self.cal.box_size, p));
            }
            if let Some((start, origin)) = self.cal.box_resize {
                let delta = (p - origin).max_elem();
                let next = (start + delta).clamp(32., 200.);
                if (next - self.cal.box_size).abs() > 0.5 {
                    self.cal.box_size = next;
                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(Vec2::splat(next)));
                }
            }
        }
        if !down {
            self.cal.box_resize = None;
        }
        self.cal.box_resize.is_some()
    }
    fn calendar_box_menu(&mut self, ctx: &egui::Context) {
        let Some((at, mut seen)) = self.cal.box_menu else {
            return;
        };
        let mut done = false;
        let mut close_all = false;
        let mut open = false;
        let shown = crate::studio::menu_window(
            ctx,
            egui::ViewportId::from_hash_of("calendar_box_menu"),
            at,
            Vec2::new(220., 120.),
            None,
            |ui| {
                ui.label(RichText::new("상자 색").size(12.).color(ui::t(ui).muted));
                ui.horizontal_wrapped(|ui| {
                    for c in PALETTE {
                        let (rect, r) = ui.allocate_exact_size(Vec2::splat(22.), Sense::click());
                        ui.painter().circle_filled(rect.center(), 9., c);
                        if c == self.cal.box_color {
                            ui.painter().circle_stroke(
                                rect.center(),
                                10.5,
                                Stroke::new(2.0_f32, ACCENT),
                            );
                        }
                        if r.clicked() {
                            self.cal.box_color = c;
                        }
                    }
                });
                ui.separator();
                if ui.button("캘린더 열기").clicked() {
                    open = true;
                    done = true;
                }
                if ui.button("캘린더 상자 닫기").clicked() {
                    close_all = true;
                    done = true;
                }
            },
        );
        seen |= shown.focused;
        if shown.escape || (seen && !shown.focused) {
            done = true;
        }
        self.cal.box_menu = (!done).then_some((at, seen));
        if open {
            self.cal.state = DockState::Window;
        }
        if close_all {
            self.cal.state = DockState::Hidden;
        }
    }
}

#[cfg(test)]
mod standing_tests {
    use super::*;

    #[test]
    fn standing_button_accepts_normal_click_and_shortcut_restores_pending_pin() {
        let mut app = App::sample(chrono::NaiveDate::from_ymd_opt(2026, 11, 17).unwrap());
        app.cal.standing = Some(Standing {
            hwnd: 0,
            rect: [0, 0, 860, 760],
            outer: [0, 0, 860, 800],
            button: [0; 4],
            pinned: false,
            wait: 3,
            click: None,
            over_icons: false,
        });
        let ctx = egui::Context::default();
        let mut button = Rect::NOTHING;
        let mut clicked = false;
        let mut frame = |events: Vec<egui::Event>| {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(860., 760.))),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        (clicked, button) = app.calendar_toolbar(ui, &mut false);
                    });
                },
            );
            (clicked, button)
        };
        let (_, rect) = frame(vec![]);
        let pos = rect.center();
        frame(vec![egui::Event::PointerMoved(pos)]);
        frame(vec![egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        }]);
        let (clicked, _) = frame(vec![egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }]);
        assert!(clicked, "standing mode must not discard the toolbar click");
        app.show_calendar();
        assert!(app.cal.standing.is_none());
        assert_eq!(app.cal.state, DockState::Window);
    }

    #[test]
    fn nothing_dropped_on_a_standing_calendar_becomes_a_schedule() {
        let day = chrono::NaiveDate::from_ymd_opt(2026, 11, 17).unwrap();
        let mut app = App::sample(day);
        let cell = Rect::from_min_max(Pos2::new(200., 10.), Pos2::new(300., 100.));
        app.cal_cells.insert(
            egui::ViewportId::from_hash_of("calendar"),
            (0., vec![(cell, day)]),
        );
        assert_eq!(
            app.schedule_target_at(cell.center(), 0.2),
            Some((day, true)),
            "the detached calendar takes drops"
        );
        app.cal.standing = Some(Standing {
            hwnd: 0,
            rect: [0, 0, 860, 760],
            outer: [0, 0, 860, 800],
            button: [0; 4],
            pinned: true,
            wait: 0,
            click: None,
            over_icons: false,
        });
        assert_eq!(app.schedule_target_at(cell.center(), 0.2), None);
    }

    #[test]
    fn a_standing_calendar_whose_window_is_gone_comes_back_and_stands_again() {
        let mut app = App::sample(chrono::NaiveDate::from_ymd_opt(2026, 11, 20).unwrap());
        app.cal.state = DockState::Window;
        let gone = || Standing {
            // No window has this handle: gone with its desktop layer.
            hwnd: 0,
            rect: [0, 0, 860, 760],
            outer: [0, 0, 860, 800],
            button: [0; 4],
            pinned: true,
            wait: 0,
            click: None,
            over_icons: false,
        };
        let ctx = egui::Context::default();
        for round in 1..=4u8 {
            app.cal.standing = Some(gone());
            let _ = ctx.run(egui::RawInput::default(), |ctx| app.calendar_dock(ctx));
            assert!(
                app.cal.standing.is_none(),
                "never painted into the dead window"
            );
            assert!(!app.cal.recreate, "left out for one frame, then made anew");
            assert_eq!(app.cal.state, DockState::Window);
            assert_eq!(app.cal.restands, round);
            // Stands again by itself three times; then it stays a window.
            assert_eq!(app.cal.stand_on_start > 0, round <= 3);
            app.cal.stand_on_start = 0;
        }
        // The main window's calendar button also gets a dead one back.
        app.cal.standing = Some(gone());
        app.show_calendar();
        assert!(app.cal.standing.is_none());
        assert!(app.cal.recreate);
    }

    #[test]
    fn a_calendar_standing_at_quit_is_saved_as_standing() {
        let mut app = App::sample(chrono::NaiveDate::from_ymd_opt(2026, 11, 20).unwrap());
        app.cal.state = DockState::Window;
        app.cal.standing = Some(Standing {
            hwnd: 0,
            rect: [0, 0, 860, 760],
            outer: [0, 0, 860, 800],
            button: [0; 4],
            pinned: true,
            wait: 0,
            click: None,
            over_icons: false,
        });
        app.release_standing();
        assert!(app.cal.standing.is_none());
        let mut s = crate::storage::SettingsRec::default();
        app.cal.write_place(&mut s);
        assert!(s.cal_standing, "stands again at the next start");
        // A calendar that did not stand is not saved as standing.
        let mut plain = App::sample(chrono::NaiveDate::from_ymd_opt(2026, 11, 20).unwrap());
        plain.cal.state = DockState::Window;
        plain.release_standing();
        let mut s = crate::storage::SettingsRec::default();
        plain.cal.write_place(&mut s);
        assert!(!s.cal_standing);
    }

    #[test]
    fn the_standing_calendar_lets_through_only_what_is_turned_on() {
        let mut app = App::sample(chrono::NaiveDate::from_ymd_opt(2026, 11, 20).unwrap());
        let all = [
            StandFeature::NewMemo,
            StandFeature::Range,
            StandFeature::Drop,
            StandFeature::Band,
        ];
        // Not standing: everything works.
        assert!(all.iter().all(|&f| app.stand_feature_on(f)));
        app.cal.standing = Some(Standing {
            hwnd: 0,
            rect: [0, 0, 860, 760],
            outer: [0, 0, 860, 800],
            button: [0; 4],
            pinned: true,
            wait: 0,
            click: None,
            over_icons: false,
        });
        // Off by default: nothing.
        assert!(!app.prefs.stand_input);
        assert!(all.iter().all(|&f| !app.stand_feature_on(f)));
        // Turned on: all four by default, each one can be left out.
        app.prefs.stand_input = true;
        assert!(all.iter().all(|&f| app.stand_feature_on(f)));
        app.prefs.stand_drop = false;
        assert!(!app.stand_feature_on(StandFeature::Drop));
        assert!(app.stand_feature_on(StandFeature::Band));
        // Only the calendar window's grid is the standing calendar.
        app.prefs.stand_input = false;
        app.cal.painting = false;
        assert!(
            app.stand_allows(StandFeature::NewMemo),
            "the main window's calendar"
        );
        app.cal.painting = true;
        assert!(!app.stand_allows(StandFeature::NewMemo));
        // A sticker dropped on it is a schedule only when chosen.
        let day = chrono::NaiveDate::from_ymd_opt(2026, 11, 20).unwrap();
        let cell = Rect::from_min_max(Pos2::new(200., 10.), Pos2::new(300., 100.));
        app.cal_cells.insert(
            egui::ViewportId::from_hash_of("calendar"),
            (0., vec![(cell, day)]),
        );
        assert_eq!(app.schedule_target_at(cell.center(), 0.2), None);
        app.prefs.stand_input = true;
        app.prefs.stand_drop = true;
        assert_eq!(
            app.schedule_target_at(cell.center(), 0.2),
            Some((day, true))
        );
    }

    #[test]
    fn button_pixels_follow_ui_scale_without_adding_stale_screen_origin() {
        let button = Rect::from_min_max(Pos2::new(120., 14.), Pos2::new(152., 46.));
        assert_eq!(px(button, 1.), [120, 14, 152, 46]);
        assert_eq!(px(button, 1.5), [180, 21, 228, 69]);
        assert_eq!(px(button, 2.), [240, 28, 304, 92]);
    }
}
