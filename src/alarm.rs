//! Memo alarms: when a memo's alarm time comes (or was missed while the app
//! was closed), play the Windows notification sound and show a Windows
//! notification or an always-on-top popup at the bottom right of the screen.
//! Each alarm is answered there: open the memo, snooze 5 minutes, or confirm.
//!
//! A memo has its main alarm (`reminder`), which may repeat (daily, on
//! weekdays, weekly, monthly) and may come with an early reminder some
//! minutes before, plus any number of extra one-time alarms.
use crate::*;
use chrono::{Datelike, NaiveDateTime, Weekday};
use egui::{Pos2, Rect, Sense, Stroke};
use std::collections::{HashMap, HashSet};

/// Which of a memo's alarms is showing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Main,
    /// The early reminder before the main alarm at this time.
    Early(NaiveDateTime),
    /// An extra one-time alarm.
    Extra(NaiveDateTime),
}

impl Kind {
    /// The letter carried by the notification's buttons.
    pub fn letter(self) -> char {
        match self {
            Kind::Main => 'm',
            Kind::Early(_) => 'p',
            Kind::Extra(_) => 'x',
        }
    }
}

/// Repeat choices of the main alarm (`Note::alarm_repeat`).
pub const REPEATS: [&str; 5] = ["안 함", "매일", "평일", "매주", "매월"];
/// Early reminder choices, minutes before (`Note::alarm_lead`).
pub const LEADS: [(u16, &str); 6] = [
    (0, "없음"),
    (5, "5분 전"),
    (10, "10분 전"),
    (30, "30분 전"),
    (60, "1시간 전"),
    (1440, "하루 전"),
];

#[derive(Default)]
pub struct Alarms {
    /// Memos whose alarm is showing, in the order they came due.
    pub due: Vec<usize>,
    /// Which alarm of each memo in `due` it is.
    pub kind: HashMap<usize, Kind>,
    /// Due alarms shown as Windows notifications (not in the popup).
    pub toasted: Vec<usize>,
    /// Early reminders already given: (memo, main alarm time).
    early_given: HashSet<(usize, NaiveDateTime)>,
    checked: f64,
}

/// The next time of a repeating alarm after `now` (`rule` as `REPEATS`).
pub fn next_repeat(at: NaiveDateTime, rule: u8, now: NaiveDateTime) -> Option<NaiveDateTime> {
    if rule == 0 {
        return None;
    }
    let step = |t: NaiveDateTime| -> NaiveDateTime {
        match rule {
            1 => t + chrono::Duration::days(1),
            2 => {
                let mut n = t + chrono::Duration::days(1);
                while matches!(n.weekday(), Weekday::Sat | Weekday::Sun) {
                    n += chrono::Duration::days(1);
                }
                n
            }
            3 => t + chrono::Duration::days(7),
            _ => {
                // Same day next month, or that month's last day.
                let (y, m) = if t.month() == 12 {
                    (t.year() + 1, 1)
                } else {
                    (t.year(), t.month() + 1)
                };
                let mut d = t.day();
                loop {
                    if let Some(date) = NaiveDate::from_ymd_opt(y, m, d) {
                        break date.and_time(t.time());
                    }
                    d -= 1;
                }
            }
        }
    };
    let mut t = step(at);
    while t <= now {
        t = step(t);
    }
    Some(t)
}

/// The alarm of each memo that has come due (the main one first, then an
/// extra one, then the early reminder).
pub fn due_now(
    notes: &[Note],
    now: NaiveDateTime,
    early_given: &HashSet<(usize, NaiveDateTime)>,
) -> Vec<(usize, Kind)> {
    notes
        .iter()
        .enumerate()
        .filter(|(_, n)| !n.deleted)
        .filter_map(|(i, n)| {
            if n.reminder.is_some_and(|t| t <= now) {
                return Some((i, Kind::Main));
            }
            if let Some(t) = n.alarm_more.iter().copied().filter(|t| *t <= now).min() {
                return Some((i, Kind::Extra(t)));
            }
            let at = n.reminder?;
            let lead = chrono::Duration::minutes(i64::from(n.alarm_lead));
            (n.alarm_lead > 0 && at - lead <= now && !early_given.contains(&(i, at)))
                .then_some((i, Kind::Early(at)))
        })
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
            .enumerate()
            .filter(|(_, n)| !n.deleted)
            .flat_map(|(i, n)| {
                let early = n
                    .reminder
                    .filter(|at| n.alarm_lead > 0 && !self.alarms.early_given.contains(&(i, *at)));
                n.reminder
                    .into_iter()
                    .chain(early.map(|at| at - chrono::Duration::minutes(i64::from(n.alarm_lead))))
                    .chain(n.alarm_more.iter().copied())
            })
            .map(|t| (t - now).num_milliseconds() as f64 / 1000. + 0.05)
            .filter(|s| *s > 0.)
            .reduce(f64::min)
            .filter(|_| !self.desktop.mute)
    }
    /// "11월 18일 09:00" for the alarm showing for `i`, and what it is.
    fn alarm_text(&self, i: usize, kind: Kind) -> String {
        let n = &self.data.notes[i];
        match kind {
            Kind::Main => {
                let when = n
                    .reminder
                    .map(|w| w.format("%-m월 %-d일 %H:%M").to_string())
                    .unwrap_or_default();
                if n.alarm_repeat > 0 {
                    format!("{when} · {} 반복", REPEATS[n.alarm_repeat as usize])
                } else {
                    when
                }
            }
            Kind::Early(at) => format!("{} 알람 · 미리 알림", at.format("%-m월 %-d일 %H:%M")),
            Kind::Extra(at) => format!("{} · 추가 알람", at.format("%-m월 %-d일 %H:%M")),
        }
    }
    /// An alarm answered (from a notification or the popup): 0 open, 1 in
    /// five minutes, 2 done. `letter` tells which alarm when the program
    /// was started for the answer and does not know yet.
    pub fn answer_alarm(&mut self, i: usize, what: u8, letter: Option<char>) {
        if i >= self.data.notes.len() {
            return;
        }
        let now = Local::now().naive_local();
        let kind = self.alarms.kind.remove(&i).unwrap_or_else(|| {
            let n = &self.data.notes[i];
            match letter {
                Some('p') => n.reminder.map(Kind::Early).unwrap_or(Kind::Main),
                Some('x') => n
                    .alarm_more
                    .iter()
                    .copied()
                    .filter(|t| *t <= now)
                    .min()
                    .map(Kind::Extra)
                    .unwrap_or(Kind::Main),
                _ => Kind::Main,
            }
        });
        self.alarms.due.retain(|&d| d != i);
        self.alarms.toasted.retain(|&d| d != i);
        // Saved right away, like a change made by hand.
        crate::storage::INPUT.store(true, std::sync::atomic::Ordering::Relaxed);
        crate::toast::remove(i);
        let later = now + chrono::Duration::minutes(5);
        let n = &mut self.data.notes[i];
        match kind {
            Kind::Main => {
                // A repeating alarm moves on to its next time; a snooze is
                // then an extra alarm, so the schedule keeps its hour.
                let next = n
                    .reminder
                    .and_then(|at| next_repeat(at, n.alarm_repeat, now));
                if what == 1 {
                    if next.is_some() {
                        n.alarm_more.push(later);
                    } else {
                        n.reminder = Some(later);
                    }
                }
                if what != 1 || next.is_some() {
                    n.reminder = next;
                }
            }
            Kind::Early(at) => {
                self.alarms.early_given.insert((i, at));
                if what == 1 {
                    n.alarm_more.push(later);
                }
            }
            Kind::Extra(at) => {
                n.alarm_more.retain(|t| *t != at);
                if what == 1 {
                    n.alarm_more.push(later);
                }
            }
        }
        if what == 0 {
            self.open_sticker(i, self.data.notes[i].date);
        }
    }
    /// Run about once a second from `update`.
    pub fn check_alarms(&mut self, ctx: &egui::Context) {
        for a in crate::toast::take_answers() {
            match a {
                crate::toast::Answer::Open(i, k) => self.answer_alarm(i, 0, k),
                crate::toast::Answer::Snooze(i, k) => self.answer_alarm(i, 1, k),
                crate::toast::Answer::Done(i, k) => self.answer_alarm(i, 2, k),
            }
        }
        let t = ctx.input(|i| i.time);
        if t - self.alarms.checked >= 1. {
            self.alarms.checked = t;
            // A showing alarm that was changed meanwhile goes away.
            let notes = &self.data.notes;
            let kinds = &self.alarms.kind;
            self.alarms.due.retain(|&i| {
                notes.get(i).is_some_and(|n| match kinds.get(&i) {
                    Some(Kind::Extra(at)) => n.alarm_more.contains(at),
                    _ => n.reminder.is_some(),
                })
            });
            let due = self.alarms.due.clone();
            self.alarms.toasted.retain(|i| due.contains(i));
            self.alarms.kind.retain(|i, _| due.contains(i));
            // While all alarms are off nothing fires; turning them back on
            // shows the alarms that came due meanwhile.
            if !self.desktop.mute {
                let mut new = false;
                let now = Local::now().naive_local();
                for (i, kind) in due_now(&self.data.notes, now, &self.alarms.early_given) {
                    if self.alarms.due.contains(&i) {
                        continue;
                    }
                    self.alarms.due.push(i);
                    self.alarms.kind.insert(i, kind);
                    // As a Windows notification when it can be (it has its
                    // own sound); otherwise the popup below.
                    let when = self.alarm_text(i, kind);
                    let n = &self.data.notes[i];
                    if self.desktop.toast_center
                        && crate::toast::show(i, kind.letter(), &n.title, &when, self.desktop.sound)
                            .is_ok()
                    {
                        self.alarms.toasted.push(i);
                    } else {
                        new = true;
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
            for i in self.alarms.due.clone() {
                if !self.alarms.toasted.contains(&i) {
                    self.answer_alarm(i, 2, None);
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
                let tone = self.app_tone();
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
                            let kind = self.alarms.kind.get(&i).copied().unwrap_or(Kind::Main);
                            let when = match kind {
                                Kind::Extra(t) => t,
                                Kind::Early(_) => now,
                                Kind::Main => n.reminder.unwrap_or(now),
                            };
                            let late = now - when > chrono::Duration::minutes(1);
                            let text = self.alarm_text(i, kind);
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
                                format!("{text}{}", if late { " · 놓친 알람" } else { "" }),
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
                self.answer_alarm(i, 2, None);
            }
        }
        if let Some((i, what)) = action {
            self.answer_alarm(i, what, None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn at(d: &str, h: u32, m: u32) -> NaiveDateTime {
        NaiveDate::parse_from_str(d, "%Y-%m-%d")
            .unwrap()
            .and_hms_opt(h, m, 0)
            .unwrap()
    }
    #[test]
    fn repeating_alarms_move_to_their_next_time() {
        let t = at("2026-11-20", 9, 0); // a Friday
        let now = at("2026-11-20", 9, 1);
        assert_eq!(next_repeat(t, 0, now), None);
        assert_eq!(next_repeat(t, 1, now), Some(at("2026-11-21", 9, 0)));
        assert_eq!(
            next_repeat(t, 2, now),
            Some(at("2026-11-23", 9, 0)),
            "평일: over the weekend"
        );
        assert_eq!(next_repeat(t, 3, now), Some(at("2026-11-27", 9, 0)));
        assert_eq!(
            next_repeat(at("2026-01-31", 9, 0), 4, at("2026-01-31", 9, 1)),
            Some(at("2026-02-28", 9, 0))
        );
        // Missed for days: the first time after now.
        assert_eq!(
            next_repeat(t, 1, at("2026-11-25", 12, 0)),
            Some(at("2026-11-26", 9, 0))
        );
    }
    #[test]
    fn early_reminders_extra_alarms_and_answers() {
        let d = NaiveDate::from_ymd_opt(2026, 11, 11).unwrap();
        let mut a = App::sample(d);
        for n in a.data.notes.iter_mut() {
            n.reminder = None;
        }
        let main = at("2026-11-11", 10, 0);
        a.data.notes[0].reminder = Some(main);
        a.data.notes[0].alarm_lead = 10;
        a.data.notes[0].alarm_repeat = 1;
        a.data.notes[1].alarm_more = vec![at("2026-11-11", 8, 0)];
        let given = HashSet::new();
        // 9:50: the early reminder of memo 0, the extra alarm of memo 1.
        let due = due_now(&a.data.notes, at("2026-11-11", 9, 51), &given);
        assert_eq!(
            due,
            vec![
                (0, Kind::Early(main)),
                (1, Kind::Extra(at("2026-11-11", 8, 0)))
            ]
        );
        // Confirming the early reminder keeps the main alarm.
        a.alarms.kind.insert(0, Kind::Early(main));
        a.answer_alarm(0, 2, None);
        assert_eq!(a.data.notes[0].reminder, Some(main));
        assert!(due_now(
            &a.data.notes,
            at("2026-11-11", 9, 55),
            &a.alarms.early_given
        )
        .iter()
        .all(|(i, _)| *i != 0));
        // Confirming the extra alarm removes it.
        a.alarms.kind.insert(1, Kind::Extra(at("2026-11-11", 8, 0)));
        a.answer_alarm(1, 2, None);
        assert!(a.data.notes[1].alarm_more.is_empty());
        // The main alarm of a daily alarm: done moves it to tomorrow.
        a.alarms.kind.insert(0, Kind::Main);
        a.answer_alarm(0, 2, None);
        let next = a.data.notes[0].reminder.unwrap();
        assert!(next > main && next.time() == main.time());
        // Snoozing a repeating alarm: an extra alarm in five minutes.
        a.alarms.kind.insert(0, Kind::Main);
        a.answer_alarm(0, 1, None);
        assert_eq!(a.data.notes[0].alarm_more.len(), 1);
        // Started for an answer: the letter tells which alarm.
        a.data.notes[2].reminder = Some(main);
        a.answer_alarm(2, 2, Some('m'));
        assert_eq!(a.data.notes[2].reminder, None);
    }
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
        let g = HashSet::new();
        let ids = |v: Vec<(usize, Kind)>| v.into_iter().map(|(i, _)| i).collect::<Vec<_>>();
        assert_eq!(ids(due_now(&a.data.notes, at(9, 10), &g)), vec![0]);
        assert_eq!(ids(due_now(&a.data.notes, at(9, 30), &g)), vec![0, 1]);
    }
}
