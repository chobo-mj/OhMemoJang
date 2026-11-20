use crate::*;
use egui::{Pos2, Rect, ViewportId};
#[derive(Clone)]
pub struct Source {
    pub layer: egui::LayerId,
    pub viewport: ViewportId,
    pub rect: Rect,
    pub note: usize,
    pub card: bool,
    pub folder: bool,
    pub clear_on_miss: bool,
    pub occurrence: Option<NaiveDate>,
}
#[derive(Clone)]
pub enum Destination {
    Date(NaiveDate),
    DayList(NaiveDate),
    Parent(usize),
    /// A wiki folder, or `None` for the unfiled wiki memos.
    WikiFolder(Option<usize>),
    /// The root of a wiki tab's tree.
    WikiTab(usize),
}
#[derive(Clone)]
pub struct Target {
    pub layer: egui::LayerId,
    pub viewport: ViewportId,
    pub rect: Rect,
    pub kind: Destination,
}
pub struct Drag {
    source: Source,
    origin: Pos2,
    initial: Vec2,
    current: Pos2,
    screen_origin: Vec2,
    shift: bool,
    moved: bool,
}
impl Drag {
    pub fn moving_note_in(&self, viewport: ViewportId, note: usize) -> bool {
        self.moved && self.source.viewport == viewport && self.source.note == note
    }
}

impl App {
    pub fn assign_dropped_date(
        &mut self,
        note: usize,
        date: NaiveDate,
        shift: bool,
        calendar: bool,
        occurrence: Option<NaiveDate>,
    ) {
        if note >= self.data.notes.len() {
            return;
        }
        self.schedule_drop_serial = self.schedule_drop_serial.wrapping_add(1);
        if calendar && self.prefs.drop_schedule_choice && !shift {
            self.pending_date_drop = Some((note, date));
            self.desktop.focus_dialog = true;
            return;
        }
        // Put on a day's list (not a calendar grid), a '이날까지 업무' memo
        // stays one: only Shift on the grid decides that.
        let deadline = shift || (!calendar && self.data.notes[note].deadline);
        self.set_dropped_date(note, date, deadline, occurrence);
    }
    /// Give the memo `date`, as a '이날까지 업무' when `deadline`.
    fn set_dropped_date(
        &mut self,
        note: usize,
        date: NaiveDate,
        deadline: bool,
        occurrence: Option<NaiveDate>,
    ) {
        let n = &mut self.data.notes[note];
        let start = if deadline {
            date
        } else {
            match (n.date, occurrence) {
                (Some(start), Some(from)) if !n.deadline => start + (date - from),
                _ => date,
            }
        };
        if deadline || n.deadline || occurrence.is_none() {
            n.duration = 1;
        }
        n.deadline = deadline;
        n.move_start(start);
        self.day = date;
        self.toast = format!(
            "{} → {}{}",
            n.title,
            date,
            if deadline {
                " · 이날까지 업무"
            } else {
                ""
            }
        );
        // The day's stickers ('매일 오늘 메모') are not opened again here:
        // that would bring out every memo due today, not only this one.
    }
    pub fn date_drop_dialog(&mut self, ctx: &egui::Context) {
        let Some((note, date)) = self.pending_date_drop else {
            return;
        };
        let mut open = true;
        let mut choice = None;
        let mut cancel = false;
        crate::ui::dialog(
            ctx,
            "date_drop_choice",
            "일정으로 추가",
            [360., 190.],
            true,
            &mut open,
            |ui| {
                ui.label(format!("{} · {}", date, self.data.notes[note].title));
                ui.add_space(14.);
                ui.horizontal(|ui| {
                    if ui.button("당일 일정").clicked() {
                        choice = Some(false);
                    }
                    if ui.button("🚩 이날까지 업무").clicked() {
                        choice = Some(true);
                    }
                    if ui.button("취소").clicked() {
                        cancel = true;
                    }
                });
            },
        );
        if let Some(deadline) = choice {
            self.pending_date_drop = None;
            // The choice made here stands, even for a memo that was a
            // '이날까지 업무' before.
            if note < self.data.notes.len() {
                self.set_dropped_date(note, date, deadline, None);
            }
        } else if !open || cancel {
            self.pending_date_drop = None;
        }
    }
    pub fn schedule_target_at(&self, point: Pos2, now: f64) -> Option<(NaiveDate, bool)> {
        let days = self
            .day_cells
            .iter()
            .filter_map(|(id, (time, rect, date))| {
                (now - time < 1. && rect.contains(point)).then_some((*id, *date, false))
            });
        // A calendar standing on the desktop is wallpaper: nothing dropped
        // over it becomes a schedule, unless chosen in settings.
        let standing = (!self.stand_feature_on(crate::calendar_dock::StandFeature::Drop))
            .then(|| ViewportId::from_hash_of("calendar"));
        let days = days.filter(|(id, _, _)| Some(*id) != standing);
        let calendars = self.cal_cells.iter().filter_map(|(id, (time, cells))| {
            if now - time >= 1. || Some(*id) == standing {
                return None;
            }
            cells
                .iter()
                .find(|(rect, _)| rect.contains(point))
                .map(|(_, date)| (*id, *date, true))
        });
        let found: Vec<_> = days.chain(calendars).collect();
        // Only a calendar that is really seen under the pointer takes the
        // memo: not a minimized main window or one hidden behind another
        // program or the desktop, whose grid may still be drawn.
        #[cfg(not(test))]
        if !found.is_empty() {
            let origins: Vec<_> = found
                .iter()
                .map(|(id, _, _)| self.schedule_origins.get(id).map(|o| (o.x, o.y)))
                .collect();
            match crate::system::window_seen_under_pointer(&origins) {
                None => return None,
                Some(Some(k)) => return Some((found[k].1, found[k].2)),
                Some(None) => {}
            }
        }
        // Only on release: resolve overlapping calendars by native window order.
        // HashMap iteration order must never decide which date receives a memo.
        found
            .into_iter()
            .min_by_key(|(id, _, _)| {
                self.schedule_origins
                    .get(id)
                    .and_then(|origin| crate::system::window_rank_at_origin(origin.x, origin.y))
                    .unwrap_or(usize::MAX)
            })
            .map(|(_, date, calendar)| (date, calendar))
    }
    /// Finish a drag outside its original viewport, where egui may not receive release.
    pub fn finish_note_drag(&mut self, ctx: &egui::Context) {
        let Some(d) = self.drag.as_ref() else {
            return;
        };
        if !d.moved || d.source.card || d.source.folder || crate::system::left_button_down() {
            return;
        }
        let Some((x, y)) = crate::system::cursor_pos() else {
            return;
        };
        let p = Pos2::new(x as f32, y as f32) / ctx.pixels_per_point().max(0.5);
        // Local release events are handled by drag_input with their exact modifiers.
        let source_viewport = d.source.viewport;
        if ctx.input(|i| {
            i.raw
                .viewports
                .get(&source_viewport)
                .and_then(|v| v.inner_rect)
                .is_some_and(|r| r.contains(p))
        }) {
            return;
        }
        let d = self.drag.take().unwrap();
        if let Some((date, calendar)) = self.schedule_target_at(p, ctx.input(|i| i.time)) {
            self.assign_dropped_date(
                d.source.note,
                date,
                d.shift || crate::system::shift_down(),
                calendar,
                d.source.occurrence,
            );
        } else if d.source.clear_on_miss {
            self.data.notes[d.source.note].clear_schedule();
        }
    }
    pub fn drag_source(&mut self, ui: &egui::Ui, r: &egui::Response, note: usize, card: bool) {
        self.sources.push(Source {
            viewport: ui.ctx().viewport_id(),
            layer: ui.layer_id(),
            rect: r.rect.intersect(ui.clip_rect()),
            note,
            card,
            folder: false,
            clear_on_miss: false,
            occurrence: None,
        });
    }
    pub fn drag_input(&mut self, ctx: &egui::Context) {
        let viewport = ctx.viewport_id();
        let events = ctx.input(|i| i.events.clone());
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if let Some(d) = self.drag.take() {
                if d.source.card {
                    self.data.notes[d.source.note].pos = d.initial;
                }
            }
        }
        for e in events {
            match e {
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    ..
                } => {
                    if !self.settings {
                        if let Some(source) = self
                            .sources
                            .iter()
                            .rev()
                            .find(|s| {
                                s.viewport == viewport
                                    && s.rect.contains(
                                        ctx.layer_transform_from_global(s.layer)
                                            .unwrap_or(egui::emath::TSTransform::IDENTITY)
                                            * pos,
                                    )
                            })
                            .cloned()
                        {
                            let initial = if source.folder {
                                Vec2::ZERO
                            } else {
                                self.data.notes[source.note].pos
                            };
                            self.drag = Some(Drag {
                                source,
                                origin: pos,
                                initial,
                                current: pos,
                                screen_origin: ctx.input(|i| {
                                    i.viewport()
                                        .inner_rect
                                        .map_or(Vec2::ZERO, |r| r.min.to_vec2())
                                }),
                                shift: ctx.input(|i| i.modifiers.shift),
                                moved: false,
                            });
                        }
                    }
                }
                egui::Event::PointerMoved(pos) => {
                    if let Some(d) = &mut self.drag {
                        if d.source.viewport == viewport {
                            d.current = pos;
                            d.moved |= d.origin.distance(pos) > 4.;
                            if d.source.card && d.origin.distance(pos) > 4. {
                                self.data.notes[d.source.note].pos = (d.initial
                                    + (pos - d.origin)
                                        / ctx
                                            .layer_transform_to_global(d.source.layer)
                                            .map_or(1., |t| t.scaling))
                                .clamp(Vec2::ZERO, Vec2::new(1196., 766.));
                            }
                        }
                    }
                }
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers,
                } => {
                    if let Some(d) = self.drag.take() {
                        if (d.source.viewport == viewport && d.origin.distance(pos) > 4.)
                            || (d.source.viewport != viewport && d.moved)
                        {
                            if d.source.card {
                                self.data.notes[d.source.note].pos = (d.initial
                                    + (pos - d.origin)
                                        / ctx
                                            .layer_transform_to_global(d.source.layer)
                                            .map_or(1., |t| t.scaling))
                                .clamp(Vec2::ZERO, Vec2::new(1196., 766.));
                            } else if let Some(t) = self
                                .targets
                                .iter()
                                .rev()
                                .find(|t| {
                                    t.viewport == viewport
                                        && t.rect.contains(
                                            ctx.layer_transform_from_global(t.layer)
                                                .unwrap_or(egui::emath::TSTransform::IDENTITY)
                                                * pos,
                                        )
                                })
                                .cloned()
                            {
                                match t.kind {
                                    Destination::Date(date) => {
                                        if !d.source.folder {
                                            self.assign_dropped_date(
                                                d.source.note,
                                                date,
                                                modifiers.shift || d.shift,
                                                true,
                                                d.source.occurrence,
                                            );
                                        }
                                    }
                                    Destination::DayList(date) => {
                                        if !d.source.folder {
                                            if d.source.clear_on_miss
                                                && !modifiers.shift
                                                && !d.shift
                                                && d.source.viewport == viewport
                                                && t.rect.contains(d.source.rect.center())
                                                && d.source.occurrence == Some(date)
                                                && !self.sources.iter().any(|s| {
                                                    s.viewport == viewport && s.rect.contains(pos)
                                                })
                                            {
                                                self.data.notes[d.source.note].clear_schedule();
                                            } else {
                                                self.assign_dropped_date(
                                                    d.source.note,
                                                    date,
                                                    modifiers.shift || d.shift,
                                                    false,
                                                    None,
                                                );
                                            }
                                        }
                                    }
                                    Destination::WikiFolder(folder) => {
                                        if d.source.folder {
                                            if !self.move_folder(d.source.note, folder) {
                                                self.toast="폴더의 자기 자신/하위 폴더 안으로는 이동할 수 없습니다".into();
                                            }
                                        } else if self.data.notes[d.source.note].wiki {
                                            let tab = folder.map(|f| self.extra.folders[f].tab);
                                            let n = &mut self.data.notes[d.source.note];
                                            n.folder = folder;
                                            n.wiki_tab = tab;
                                            self.toast = "위키 메모를 분류했습니다".into();
                                        }
                                    }
                                    Destination::WikiTab(tab) => {
                                        if d.source.folder {
                                            self.move_folder_to_tab(d.source.note, tab);
                                        } else if self.data.notes[d.source.note].wiki {
                                            let n = &mut self.data.notes[d.source.note];
                                            n.folder = None;
                                            n.wiki_tab = Some(tab);
                                            self.toast = "위키 메모를 분류했습니다".into();
                                        }
                                    }
                                    Destination::Parent(parent) => {
                                        if !d.source.folder
                                            && !self.data.reparent(d.source.note, parent)
                                        {
                                            self.toast =
                                                "자기 자신 또는 순환 관계는 만들 수 없습니다"
                                                    .into();
                                        }
                                    }
                                }
                            } else if !d.source.folder {
                                let screen = pos
                                    + if d.source.viewport == viewport {
                                        d.screen_origin
                                    } else {
                                        ctx.input(|i| {
                                            i.viewport()
                                                .inner_rect
                                                .map_or(Vec2::ZERO, |r| r.min.to_vec2())
                                        })
                                    };
                                if let Some((date, calendar)) =
                                    self.schedule_target_at(screen, ctx.input(|i| i.time))
                                {
                                    self.assign_dropped_date(
                                        d.source.note,
                                        date,
                                        modifiers.shift || d.shift,
                                        calendar,
                                        d.source.occurrence,
                                    );
                                } else if d.source.clear_on_miss && !d.source.rect.contains(pos) {
                                    self.data.notes[d.source.note].clear_schedule();
                                    self.toast = "일정을 지웠습니다".into();
                                }
                            }
                        }
                    }
                }
                egui::Event::WindowFocused(false) => {}
                _ => {}
            }
        }
        self.sources.retain(|s| s.viewport != viewport);
        self.targets.retain(|t| t.viewport != viewport);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    pub(super) fn simulate(a: &mut App, from: Pos2, to: Pos2) {
        simulate_shift(a, from, to, false);
    }
    fn simulate_shift(a: &mut App, from: Pos2, to: Pos2, shift: bool) {
        let ctx = egui::Context::default();
        let events = vec![
            egui::Event::PointerMoved(from),
            egui::Event::PointerButton {
                pos: from,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers {
                    shift,
                    ..Default::default()
                },
            },
            egui::Event::PointerMoved(to),
            egui::Event::PointerButton {
                pos: to,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers {
                    shift,
                    ..Default::default()
                },
            },
        ];
        let _ = ctx.run(
            egui::RawInput {
                events,
                ..Default::default()
            },
            |ctx| a.drag_input(ctx),
        );
    }
    pub(super) fn source() -> Source {
        Source {
            viewport: ViewportId::ROOT,
            layer: egui::LayerId::background(),
            rect: Rect::from_min_max(Pos2::new(10., 10.), Pos2::new(100., 50.)),
            note: 0,
            card: false,
            folder: false,
            clear_on_miss: false,
            occurrence: None,
        }
    }
    #[test]
    fn calendar_choice_is_opt_in_and_shift_always_bypasses_it() {
        let day = NaiveDate::from_ymd_opt(2026, 11, 17).unwrap();
        for choice in [false, true] {
            for shift in [false, true] {
                for calendar in [false, true] {
                    let mut a = App::sample(day);
                    a.prefs.drop_schedule_choice = choice;
                    a.assign_dropped_date(
                        0,
                        day + chrono::Duration::days(5),
                        shift,
                        calendar,
                        None,
                    );
                    assert_eq!(a.pending_date_drop.is_some(), choice && calendar && !shift);
                    if a.pending_date_drop.is_none() {
                        assert_eq!(a.data.notes[0].deadline, shift);
                        assert_eq!(a.data.notes[0].date, Some(day + chrono::Duration::days(5)));
                    } else {
                        assert_eq!(a.data.notes[0].date, Some(day));
                    }
                }
            }
        }
    }
    #[test]
    fn shift_release_on_date_makes_a_deadline_even_when_choice_is_on() {
        let day = NaiveDate::from_ymd_opt(2026, 11, 17).unwrap();
        let mut a = App::sample(day);
        a.prefs.drop_schedule_choice = true;
        a.sources.push(source());
        a.targets.push(Target {
            viewport: ViewportId::ROOT,
            layer: egui::LayerId::background(),
            rect: Rect::from_min_max(Pos2::new(200., 10.), Pos2::new(300., 100.)),
            kind: Destination::Date(day),
        });
        simulate_shift(&mut a, Pos2::new(30., 30.), Pos2::new(230., 30.), true);
        assert!(a.data.notes[0].deadline);
        assert!(a.pending_date_drop.is_none());
    }
    #[test]
    fn only_schedule_list_drag_out_removes_schedule() {
        let day = NaiveDate::from_ymd_opt(2026, 11, 17).unwrap();
        let mut a = App::sample(day);
        let mut source = source();
        source.clear_on_miss = true;
        a.sources.push(source);
        a.data.notes[0].deadline = true;
        simulate(&mut a, Pos2::new(30., 30.), Pos2::new(500., 500.));
        assert!(a.data.notes[0].date.is_none());
        assert!(!a.data.notes[0].deadline);
        assert!(a.data.notes[0].repeat.is_none());
    }
    #[test]
    fn fast_sticker_drag_uses_screen_space_calendar_cells() {
        let day = NaiveDate::from_ymd_opt(2026, 11, 17).unwrap();
        let mut a = App::sample(day);
        a.prefs.drop_schedule_choice = true;
        a.sources.push(source());
        a.cal_cells.insert(
            ViewportId::from_hash_of("calendar"),
            (
                0.,
                vec![(
                    Rect::from_min_max(Pos2::new(200., 10.), Pos2::new(300., 100.)),
                    day,
                )],
            ),
        );
        simulate(&mut a, Pos2::new(30., 30.), Pos2::new(230., 30.));
        assert_eq!(a.pending_date_drop, Some((0, day)));
        assert_eq!(a.schedule_drop_serial, 1);
    }
    #[test]
    fn a_deadline_drop_brings_out_no_other_sticker() {
        let day = NaiveDate::from_ymd_opt(2026, 11, 17).unwrap();
        let mut a = App::sample(day);
        a.desktop.daily_today = true;
        a.desktop.daily_shown = Some(day);
        let open_before = a.studio.windows.values().filter(|w| w.open).count();
        a.assign_dropped_date(1, day, true, true, None);
        assert!(a.data.notes[1].deadline);
        assert_eq!(
            a.desktop.daily_shown,
            Some(day),
            "today's stickers are not brought out again"
        );
        assert_eq!(
            a.studio.windows.values().filter(|w| w.open).count(),
            open_before
        );
    }
    #[test]
    fn a_deadline_memo_put_on_a_day_list_stays_a_deadline() {
        let day = NaiveDate::from_ymd_opt(2026, 11, 17).unwrap();
        let mut a = App::sample(day);
        a.data.notes[1].date = Some(day);
        a.data.notes[1].deadline = true;
        let later = day + chrono::Duration::days(2);
        // The calendar window's day list (no Shift).
        a.assign_dropped_date(1, later, false, false, None);
        assert!(a.data.notes[1].deadline, "still '이날까지 업무'");
        assert_eq!(a.data.notes[1].deadline_date(), Some(later));
        // On the grid without Shift it becomes a plain schedule, as before.
        a.prefs.drop_schedule_choice = false;
        a.assign_dropped_date(1, day, false, true, None);
        assert!(!a.data.notes[1].deadline);
    }
    #[test]
    fn choosing_a_plain_schedule_in_the_drop_dialog_ends_the_deadline() {
        let day = NaiveDate::from_ymd_opt(2026, 11, 17).unwrap();
        let mut a = App::sample(day);
        a.data.notes[1].date = Some(day);
        a.data.notes[1].deadline = true;
        a.prefs.drop_schedule_choice = true;
        a.assign_dropped_date(1, day, false, true, None);
        assert_eq!(a.pending_date_drop, Some((1, day)), "the dialog asks");
        a.set_dropped_date(1, day, false, None);
        assert!(!a.data.notes[1].deadline, "'당일 일정' was chosen");
    }
    #[test]
    fn fast_drag_in_one_frame_moves_date_and_preserves_original_note() {
        let date = NaiveDate::from_ymd_opt(2026, 11, 8).unwrap();
        let mut a = App::sample(date);
        a.sources.push(source());
        a.targets.push(Target {
            viewport: ViewportId::ROOT,
            layer: egui::LayerId::background(),
            rect: Rect::from_min_max(Pos2::new(200., 10.), Pos2::new(300., 100.)),
            kind: Destination::Date(date + chrono::Duration::days(3)),
        });
        let body = a.data.notes[0].body.clone();
        let time = a.data.notes[0].time;
        simulate(&mut a, Pos2::new(30., 30.), Pos2::new(230., 30.));
        assert_eq!(a.data.notes[0].date, Some(date + chrono::Duration::days(3)));
        assert_eq!(a.data.notes[0].body, body);
        assert_eq!(a.data.notes[0].time, time);
        assert_eq!(a.data.notes.len(), 6);
    }
    #[test]
    fn dropping_outside_dates_keeps_schedule() {
        let date = NaiveDate::from_ymd_opt(2026, 11, 8).unwrap();
        let mut a = App::sample(date);
        a.sources.push(source());
        simulate(&mut a, Pos2::new(30., 30.), Pos2::new(500., 500.));
        assert_eq!(a.data.notes[0].date, Some(date));
    }
    #[test]
    fn card_drag_changes_layout_without_changing_schedule() {
        let date = NaiveDate::from_ymd_opt(2026, 11, 8).unwrap();
        let mut a = App::sample(date);
        let mut s = source();
        s.card = true;
        a.sources.push(s);
        let initial = a.data.notes[0].pos;
        simulate(&mut a, Pos2::new(30., 30.), Pos2::new(80., 60.));
        assert_eq!(a.data.notes[0].pos, initial + Vec2::new(50., 30.));
        assert_eq!(a.data.notes[0].date, Some(date));
    }
}

#[cfg(test)]
mod zoom_tests {
    use super::*;
    #[test]
    fn scaled_card_drag_uses_scene_coordinates() {
        let date = NaiveDate::from_ymd_opt(2026, 11, 9).unwrap();
        let mut a = App::sample(date);
        let layer = egui::LayerId::new(egui::Order::Middle, egui::Id::new("zoom_test"));
        a.sources.push(Source {
            viewport: ViewportId::ROOT,
            layer,
            rect: Rect::from_min_max(Pos2::new(10., 10.), Pos2::new(100., 50.)),
            note: 0,
            card: true,
            folder: false,
            clear_on_miss: false,
            occurrence: None,
        });
        let initial = a.data.notes[0].pos;
        let ctx = egui::Context::default();
        let from = Pos2::new(215., 115.);
        let to = Pos2::new(255., 135.);
        let events = vec![
            egui::Event::PointerButton {
                pos: from,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::PointerMoved(to),
            egui::Event::PointerButton {
                pos: to,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ];
        let _ = ctx.run(
            egui::RawInput {
                events,
                ..Default::default()
            },
            |ctx| {
                ctx.set_transform_layer(
                    layer,
                    egui::emath::TSTransform {
                        scaling: 0.5,
                        translation: Vec2::new(200., 100.),
                    },
                );
                a.drag_input(ctx);
            },
        );
        assert_eq!(a.data.notes[0].pos, initial + Vec2::new(80., 40.));
    }
}
#[cfg(test)]
mod wiki_period_tests {
    use super::*;
    fn target(kind: Destination) -> Target {
        Target {
            viewport: ViewportId::ROOT,
            layer: egui::LayerId::background(),
            rect: Rect::from_min_max(Pos2::new(200., 10.), Pos2::new(300., 100.)),
            kind,
        }
    }
    #[test]
    fn wiki_drop_changes_classification_without_copying_note() {
        let mut a = App::sample(Local::now().date_naive());
        let count = a.data.notes.len();
        a.sources.push(super::tests::source());
        a.targets.push(target(Destination::WikiFolder(None)));
        super::tests::simulate(&mut a, Pos2::new(30., 30.), Pos2::new(230., 30.));
        assert!(a.data.notes[0].folder.is_none());
        a.sources.push(super::tests::source());
        a.targets.push(target(Destination::WikiFolder(Some(0))));
        super::tests::simulate(&mut a, Pos2::new(30., 30.), Pos2::new(230., 30.));
        assert_eq!(a.data.notes[0].folder, Some(0));
        assert_eq!(a.data.notes.len(), count);
    }
    #[test]
    fn folder_drag_keeps_note_data_untouched() {
        let mut a = App::sample(Local::now().date_naive());
        a.extra.folders.push(crate::extras::Folder {
            title: "두번째".into(),
            description: String::new(),
            parent: None,
            tab: 0,
        });
        let mut source = super::tests::source();
        source.folder = true;
        source.note = 1;
        a.sources.push(source);
        a.targets.push(target(Destination::WikiFolder(Some(0))));
        super::tests::simulate(&mut a, Pos2::new(30., 30.), Pos2::new(230., 30.));
        assert_eq!(a.extra.folders[1].parent, Some(0));
        assert_eq!(a.data.notes[1].parents, Vec::<usize>::new());
    }
    #[test]
    fn dragging_middle_day_moves_whole_period_by_offset() {
        let start = NaiveDate::from_ymd_opt(2026, 11, 9).unwrap();
        let mut a = App::sample(start);
        a.data.notes[0].duration = 3;
        let mut s = super::tests::source();
        s.occurrence = Some(start + chrono::Duration::days(1));
        a.sources.push(s);
        a.targets
            .push(target(Destination::Date(start + chrono::Duration::days(4))));
        super::tests::simulate(&mut a, Pos2::new(30., 30.), Pos2::new(230., 30.));
        assert_eq!(
            a.data.notes[0].date,
            Some(start + chrono::Duration::days(3))
        );
        assert_eq!(a.data.notes[0].duration, 3);
    }
}
