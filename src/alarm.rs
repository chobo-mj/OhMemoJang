//! Memo alarms: when a memo's alarm time comes (or was missed while the app
//! was closed), play the Windows notification sound and show an always-on-top
//! popup at the bottom right of the screen. Each alarm is answered there:
//! open the memo, snooze 5 minutes, or confirm (clears the alarm).
use crate::*;
use chrono::NaiveDateTime;
use egui::{Pos2, Rect, Sense, Stroke};

#[derive(Default)]
pub struct Alarms {
    /// Memos whose alarm is showing, in the order they came due.
    pub due: Vec<usize>,
    /// Due alarms shown as Windows notifications (not in the popup).
    pub toasted: Vec<usize>,
    checked: f64,
}

/// Memos whose alarm time has come.
pub fn due_now(notes: &[Note], now: NaiveDateTime) -> Vec<usize> {
    notes
        .iter()
        .enumerate()
        .filter(|(_, n)| !n.deleted && n.reminder.is_some_and(|t| t <= now))
        .map(|(i, _)| i)
        .collect()
}

impl App {
    /// Seconds until the next alarm comes due (`None`: no alarm is waiting).
    pub fn next_alarm_in(&self, now: NaiveDateTime) -> Option<f64> {
        if self.desktop.mute && self.alarms.due.is_empty() {
            return None;
        }
        self.data
            .notes
            .iter()
            .filter(|n| !n.deleted)
            .filter_map(|n| n.reminder)
            .map(|t| (t - now).num_milliseconds() as f64 / 1000. + 0.05)
            .filter(|s| *s > 0.)
            .reduce(f64::min)
            .filter(|_| !self.desktop.mute)
    }
    /// An alarm answered (from a notification or the popup).
    fn answer_alarm(&mut self, i: usize, what: u8) {
        if i >= self.data.notes.len() {
            return;
        }
        self.alarms.due.retain(|&d| d != i);
        self.alarms.toasted.retain(|&d| d != i);
        // Saved right away, like a change made by hand.
        crate::storage::INPUT.store(true, std::sync::atomic::Ordering::Relaxed);
        crate::toast::remove(i);
        let now = Local::now().naive_local();
        match what {
            0 => {
                self.data.notes[i].reminder = None;
                self.open_sticker(i, self.data.notes[i].date);
            }
            1 => self.data.notes[i].reminder = Some(now + chrono::Duration::minutes(5)),
            _ => self.data.notes[i].reminder = None,
        }
    }
    /// Run about once a second from `update`.
    pub fn check_alarms(&mut self, ctx: &egui::Context) {
        for a in crate::toast::take_answers() {
            match a {
                crate::toast::Answer::Open(i) => self.answer_alarm(i, 0),
                crate::toast::Answer::Snooze(i) => self.answer_alarm(i, 1),
                crate::toast::Answer::Done(i) => self.answer_alarm(i, 2),
            }
        }
        let t = ctx.input(|i| i.time);
        if t - self.alarms.checked >= 1. {
            self.alarms.checked = t;
            self.alarms
                .due
                .retain(|&i| self.data.notes.get(i).is_some_and(|n| n.reminder.is_some()));
            let due = self.alarms.due.clone();
            self.alarms.toasted.retain(|i| due.contains(i));
            // While all alarms are off nothing fires; turning them back on
            // shows the alarms that came due meanwhile.
            if !self.desktop.mute {
                let mut new = false;
                for i in due_now(&self.data.notes, Local::now().naive_local()) {
                    if !self.alarms.due.contains(&i) {
                        self.alarms.due.push(i);
                        // As a Windows notification when it can be (it has
                        // its own sound); otherwise the popup below.
                        let n = &self.data.notes[i];
                        let when = n
                            .reminder
                            .map(|w| w.format("%-m월 %-d일 %H:%M").to_string())
                            .unwrap_or_default();
                        if self.desktop.toast_center
                            && crate::toast::show(i, &n.title, &when, self.desktop.sound).is_ok()
                        {
                            self.alarms.toasted.push(i);
                        } else {
                            new = true;
                        }
                    }
                }
                if new && self.desktop.sound {
                    crate::system::alert_sound();
                }
            }
        }
        // Alarms on screen as notifications wait for their answer there.
        if self
            .alarms
            .due
            .iter()
            .all(|i| self.alarms.toasted.contains(i))
        {
            return;
        }
        if self.desktop.popup {
            self.alarm_popup(ctx);
        } else {
            // Without the popup, the alarm is a notice in the main window.
            let titles: Vec<String> = self
                .alarms
                .due
                .iter()
                .map(|&i| self.data.notes[i].title.clone())
                .collect();
            self.toast = format!("알람 · {}", titles.join(", "));
            for i in std::mem::take(&mut self.alarms.due) {
                if !self.alarms.toasted.contains(&i) {
                    self.data.notes[i].reminder = None;
                } else {
                    self.alarms.due.push(i);
                }
            }
        }
    }
    fn alarm_popup(&mut self, ctx: &egui::Context) {
        let shown: Vec<usize> = self
            .alarms
            .due
            .iter()
            .copied()
            .filter(|i| !self.alarms.toasted.contains(i))
            .collect();
        let rows = shown.len().min(5);
        let size = Vec2::new(360., 58. + rows as f32 * 66.);
        let area = crate::native_window::work_area(ctx)
            .unwrap_or(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280., 800.)));
        let at = area.right_bottom() - size - Vec2::splat(16.);
        let builder = egui::ViewportBuilder::default()
            .with_title("OhMemoJang · 알람")
            .with_position(at)
            .with_inner_size(size)
            .with_decorations(false)
            .with_resizable(false)
            .with_taskbar(false)
            .with_transparent(true)
            .with_window_level(egui::WindowLevel::AlwaysOnTop);
        let mut action: Option<(usize, u8)> = None;
        let mut all = false;
        let now = Local::now().naive_local();
        ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of("alarm_popup"),
            builder,
            |ctx, _| {
                self.theme(ctx);
                crate::win_style::shape(ctx, 1.);
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
                let tone = ui::tone(self.dark);
                egui::CentralPanel::default()
                    .frame(
                        egui::Frame::NONE
                            .fill(tone.bg)
                            .stroke(Stroke::new(1.0_f32, tone.line))
                            .inner_margin(egui::Margin::symmetric(14, 10)),
                    )
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            icons::paint_icon(
                                ui.painter(),
                                Rect::from_min_size(ui.cursor().min, Vec2::splat(20.)),
                                icons::Icon::Bell,
                                ACCENT,
                            );
                            ui.add_space(26.);
                            ui.label(
                                RichText::new(format!("알람 {}개", shown.len()))
                                    .strong()
                                    .size(14.),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.button("모두 확인").clicked() {
                                        all = true;
                                    }
                                },
                            );
                        });
                        ui.add_space(6.);
                        for &i in shown.iter().take(5) {
                            let n = &self.data.notes[i];
                            let (rect, _) = ui.allocate_exact_size(
                                Vec2::new(ui.available_width(), 60.),
                                Sense::hover(),
                            );
                            ui.painter().rect_filled(rect, 8., tone.sidebar);
                            ui.painter().rect_filled(
                                Rect::from_min_size(
                                    rect.min + Vec2::new(6., 10.),
                                    Vec2::new(4., 40.),
                                ),
                                2.,
                                n.color,
                            );
                            let when = n.reminder.unwrap_or(now);
                            let late = now - when > chrono::Duration::minutes(1);
                            ui.painter().text(
                                rect.min + Vec2::new(18., 10.),
                                egui::Align2::LEFT_TOP,
                                crate::shorten(
                                    if n.title.trim().is_empty() {
                                        "제목 없음"
                                    } else {
                                        &n.title
                                    },
                                    18,
                                ),
                                egui::FontId::proportional(14.),
                                tone.ink,
                            );
                            ui.painter().text(
                                rect.min + Vec2::new(18., 32.),
                                egui::Align2::LEFT_TOP,
                                format!(
                                    "{}{}",
                                    when.format("%-m월 %-d일 %H:%M"),
                                    if late { " · 놓친 알람" } else { "" }
                                ),
                                egui::FontId::proportional(11.5),
                                if late { ACCENT } else { tone.muted },
                            );
                            let buttons = Rect::from_min_max(
                                rect.right_top() + Vec2::new(-150., 16.),
                                rect.right_bottom() - Vec2::new(6., 16.),
                            );
                            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(buttons), |ui| {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 4.;
                                    if ui.small_button("열기").clicked() {
                                        action = Some((i, 0));
                                    }
                                    if ui.small_button("5분 뒤").clicked() {
                                        action = Some((i, 1));
                                    }
                                    if ui.small_button("확인").clicked() {
                                        action = Some((i, 2));
                                    }
                                });
                            });
                            ui.add_space(6.);
                        }
                    });
            },
        );
        if all {
            for i in shown {
                self.answer_alarm(i, 2);
            }
        }
        if let Some((i, what)) = action {
            self.answer_alarm(i, what);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_program_sleeps_until_the_next_alarm_that_is_still_ahead() {
        let d = NaiveDate::from_ymd_opt(2026, 11, 11).unwrap();
        let mut a = App::sample(d);
        let at = |h: u32, m: u32| d.and_hms_opt(h, m, 0).unwrap();
        for n in a.data.notes.iter_mut() {
            n.reminder = None;
        }
        assert_eq!(a.next_alarm_in(at(9, 0)), None, "no alarm: no wake-up");
        a.data.notes[0].reminder = Some(at(9, 30));
        a.data.notes[1].reminder = Some(at(9, 10));
        a.data.notes[2].reminder = Some(at(8, 0));
        let wait = a.next_alarm_in(at(9, 0)).unwrap();
        assert!((wait - 600.).abs() < 1., "the nearest alarm ahead: {wait}");
        a.data.notes[1].deleted = true;
        let wait = a.next_alarm_in(at(9, 0)).unwrap();
        assert!(
            (wait - 1800.).abs() < 1.,
            "a deleted memo's alarm is ignored"
        );
        a.desktop.mute = true;
        assert_eq!(
            a.next_alarm_in(at(9, 0)),
            None,
            "muted: nothing to wait for"
        );
    }
    #[test]
    fn the_program_sleeps_until_a_memo_has_to_lock_again() {
        let d = NaiveDate::from_ymd_opt(2026, 11, 11).unwrap();
        let mut a = App::sample(d);
        a.desktop.auto_lock = 0;
        assert_eq!(a.next_relock_in(), None);
        a.desktop.unlocked_at.insert(0, std::time::Instant::now());
        let wait = a.next_relock_in().unwrap();
        assert!(wait > 50. && wait <= 61., "one minute: {wait}");
        a.desktop.auto_lock = 3; // never
        assert_eq!(a.next_relock_in(), None);
    }
    #[test]
    fn only_reached_alarms_of_live_memos_are_due() {
        let d = NaiveDate::from_ymd_opt(2026, 11, 11).unwrap();
        let mut a = App::sample(d);
        let at = |h: u32, m: u32| d.and_hms_opt(h, m, 0).unwrap();
        a.data.notes[0].reminder = Some(at(9, 0));
        a.data.notes[1].reminder = Some(at(9, 30));
        a.data.notes[2].reminder = Some(at(8, 0));
        a.data.notes[2].deleted = true;
        assert_eq!(due_now(&a.data.notes, at(9, 10)), vec![0]);
        assert_eq!(due_now(&a.data.notes, at(9, 30)), vec![0, 1]);
    }
}
