//! 설정 › 단축키: a shortcut is changed by pressing the new one. A
//! combination Windows or another program already uses, or one given to
//! another action, is refused with the reason; the user then presses
//! another one or cancels.
use crate::keys::{self, Action, Combo, Recorded, Recorder};
use crate::App;
use eframe::egui::{self, RichText};

/// The global shortcuts, in `App::shortcuts` order.
pub const GLOBAL: [&str; 4] = ["빠른 메모", "캘린더 분리창", "새 스티커 메모", "클립보이"];
pub const CLIPBOY: usize = 3;
/// The 단축키 tab of the settings.
const TAB: usize = 5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Slot {
    Global(usize),
    App(Action),
}

impl Slot {
    fn name(self) -> &'static str {
        match self {
            Slot::Global(k) => GLOBAL[k],
            Slot::App(a) => a.label(),
        }
    }
}

/// A shortcut being changed: the one waiting for a key press, and what
/// came of the last press.
#[derive(Default)]
pub struct Editing {
    slot: Option<Slot>,
    recorder: Option<Recorder>,
    /// (for, text, is a refusal)
    note: Option<(Slot, String, bool)>,
}

impl Editing {
    fn stop(&mut self) {
        self.slot = None;
        self.recorder = None;
    }
}

impl App {
    /// Waiting for a key ends when the 단축키 settings are not shown.
    pub(crate) fn shortcut_edit_frame(&mut self) {
        let shown = self.main_open && self.settings && self.desktop.settings_tab == TAB;
        if self.shortcut_edit.slot.is_some() && !shown {
            self.shortcut_edit.stop();
        }
    }

    fn slot_combo(&self, slot: Slot) -> Option<Combo> {
        match slot {
            Slot::Global(k) => Combo::parse(&self.shortcuts[k]),
            Slot::App(a) => Some(keys::combo(a)),
        }
    }

    /// Why `c` cannot do what `slot` does, found before registering it.
    fn shortcut_problem(&self, slot: Slot, c: &Combo) -> Option<String> {
        let unusable = match slot {
            Slot::Global(_) => c.to_hotkey().is_none(),
            Slot::App(_) if c.win => {
                return Some(
                    "Win 키 조합은 메모장 창 안에서 쓰는 단축키로 지정할 수 없어요.".into(),
                )
            }
            Slot::App(_) => c.to_egui().is_none(),
        };
        if unusable {
            return Some("이 키는 단축키로 쓸 수 없어요.".into());
        }
        if let Some(why) = keys::unusable(c) {
            return Some(why);
        }
        (0..GLOBAL.len())
            .map(Slot::Global)
            .chain(keys::ACTIONS.into_iter().map(Slot::App))
            .filter(|other| *other != slot)
            .find(|other| self.slot_combo(*other).as_ref() == Some(c))
            .map(|other| format!("‘{}’에 이미 쓰고 있어요.", other.name()))
    }

    /// Gives `c` to global shortcut `k`, registered at once. When Windows
    /// refuses it, the old one stays.
    fn set_global(&mut self, k: usize, c: &Combo) -> Result<(), String> {
        let new = c.to_hotkey().ok_or("이 키는 단축키로 쓸 수 없어요.")?;
        if let Some(m) = &self.hotkeys {
            let old = self.registered[k].take();
            if let Some(h) = old {
                let _ = m.unregister(h);
            }
            if m.register(new).is_err() {
                if let Some(h) = old.filter(|h| m.register(*h).is_ok()) {
                    self.registered[k] = Some(h);
                }
                return Err("다른 프로그램이나 Windows가 이미 쓰고 있는 단축키예요.".into());
            }
            if k == CLIPBOY && !self.clipboard.clipboy_enabled() {
                // Free, but 클립보이 is off: kept for when it is turned on.
                let _ = m.unregister(new);
            } else {
                self.registered[k] = Some(new);
            }
        }
        self.shortcuts[k] = c.to_string();
        if k == CLIPBOY {
            self.clipboard.set_clipboy_key(Some(c.clone()));
        }
        let failed: Vec<&str> = (0..GLOBAL.len())
            .filter(|&k| k != CLIPBOY || self.clipboard.clipboy_enabled())
            .filter(|&k| self.registered[k].is_none())
            .map(|k| GLOBAL[k])
            .collect();
        if self.hotkeys.is_some() && failed.is_empty() {
            self.hotkey_status = "전역 단축키 사용 중 · 앱 실행 중에 동작".into();
        }
        Ok(())
    }

    /// Applies the press the recorder took, if any.
    fn shortcut_recorded(&mut self, got: Recorded) {
        let Some(slot) = self.shortcut_edit.slot else {
            return;
        };
        let again = "다른 키를 누르거나 ‘취소’를 누르세요.";
        let c = match got {
            Recorded::Cancel => {
                self.shortcut_edit.stop();
                self.shortcut_edit.note = None;
                return;
            }
            Recorded::Unsupported => {
                self.shortcut_edit.note = Some((
                    slot,
                    format!("이 키는 단축키로 쓸 수 없어요. {again}"),
                    true,
                ));
                return;
            }
            Recorded::Combo(c) => c,
        };
        if self.slot_combo(slot).as_ref() == Some(&c) {
            self.shortcut_edit.stop();
            self.shortcut_edit.note = None;
            return;
        }
        let result = match self.shortcut_problem(slot, &c) {
            Some(why) => Err(why),
            None => match slot {
                Slot::Global(k) => self.set_global(k, &c),
                Slot::App(a) => {
                    keys::set(a, &c);
                    Ok(())
                }
            },
        };
        match result {
            Ok(()) => {
                self.shortcut_edit.stop();
                self.shortcut_edit.note = Some((slot, format!("{c}(으)로 바꿨어요."), false));
                crate::storage::INPUT.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            Err(why) => {
                self.shortcut_edit.note = Some((slot, format!("{c}: {why} {again}"), true));
            }
        }
    }

    fn shortcut_row(&mut self, ui: &mut egui::Ui, slot: Slot) {
        let waiting = self.shortcut_edit.slot == Some(slot);
        ui.horizontal(|ui| {
            let tone = crate::ui::t(ui);
            ui.allocate_ui_with_layout(
                egui::vec2(150., 26.),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.set_min_size(egui::vec2(150., 26.));
                    ui.label(RichText::new(slot.name()).color(tone.ink));
                },
            );
            let text = if waiting {
                "새 단축키를 누르세요…".to_owned()
            } else {
                self.slot_combo(slot)
                    .map_or("사용 안 함".to_owned(), |c| c.to_string())
            };
            let pressed = ui
                .add(
                    egui::Button::new(RichText::new(text))
                        .min_size(egui::vec2(170., 26.))
                        .selected(waiting),
                )
                .on_hover_text("누른 뒤 원하는 키를 함께 누르면 바뀌어요 · Esc는 취소")
                .clicked();
            if waiting {
                if ui.button("취소").clicked() {
                    self.shortcut_edit.stop();
                    self.shortcut_edit.note = None;
                }
            } else if pressed {
                self.shortcut_edit.recorder = None;
                self.shortcut_edit.note = None;
                match Recorder::start(ui.ctx()) {
                    Some(r) => {
                        self.shortcut_edit.slot = Some(slot);
                        self.shortcut_edit.recorder = Some(r);
                    }
                    None => {
                        self.shortcut_edit.note =
                            Some((slot, "지금은 키 입력을 받을 수 없어요.".into(), true))
                    }
                }
            }
            if slot == Slot::Global(CLIPBOY) && !self.clipboard.clipboy_enabled() {
                crate::ui::hint(ui, "꺼져 있음 · 클립보드 히스토리에서 켜요");
            }
        });
        if let Some((s, text, refused)) = &self.shortcut_edit.note {
            if *s == slot {
                let color = if *refused {
                    egui::Color32::from_rgb(200, 70, 60)
                } else {
                    crate::ui::accent(ui)
                };
                ui.label(RichText::new(text).size(12.).color(color));
            }
        }
    }

    /// The 단축키 tab of the settings.
    pub(crate) fn shortcut_settings(&mut self, ui: &mut egui::Ui) {
        if let Some(got) = self
            .shortcut_edit
            .recorder
            .as_ref()
            .and_then(Recorder::take)
        {
            self.shortcut_recorded(got);
        }
        use crate::desktop::settings_group;
        use crate::ui::hint;
        settings_group(ui, "전역 단축키", |ui| {
            hint(
                ui,
                "다른 프로그램을 쓰는 중에도 동작합니다 · 칸을 누른 뒤 원하는 키를 함께 누르면 바뀌어요",
            );
            ui.add_space(4.);
            for k in 0..GLOBAL.len() {
                self.shortcut_row(ui, Slot::Global(k));
            }
            ui.add_space(4.);
            hint(ui, &self.hotkey_status);
        });
        ui.add_space(14.);
        settings_group(ui, "메모 본문영역 서식 단축키", |ui| {
            for a in [
                Action::Bold,
                Action::Italic,
                Action::Underline,
                Action::Date,
                Action::DateTime,
            ] {
                self.shortcut_row(ui, Slot::App(a));
            }
            ui.add_space(6.);
            for (key, label) in [
                ("~~ Space", "직전 단어 취소선"),
                ("~~~ Space", "한 줄 전체 취소선"),
                ("[] Space", "체크박스 넣기"),
                ("#이름 Space", "첫 줄에서 프로젝트 지정"),
            ] {
                fixed_row(ui, label, key);
            }
            hint(ui, "글자로 입력하는 규칙(~~ Space 등)은 바꿀 수 없어요");
        });
        ui.add_space(14.);
        settings_group(ui, "프로그램 사용 중", |ui| {
            for a in [
                Action::CloseStickers,
                Action::Topmost,
                Action::Find,
                Action::Replace,
                Action::Project,
            ] {
                self.shortcut_row(ui, Slot::App(a));
            }
            ui.add_space(6.);
            for (key, label) in [
                ("Shift+드래그", "이날까지 업무 추가"),
                ("Esc Esc", "메모 닫기"),
            ] {
                fixed_row(ui, label, key);
            }
            hint(ui, "Shift+드래그: 날짜에 놓을 때 선택창 없이 이날까지 업무로 지정 · 마우스 동작과 두 번 누르기는 바꿀 수 없어요");
        });
    }
}

/// A shortcut that cannot be changed, in the same columns as the others.
fn fixed_row(ui: &mut egui::Ui, label: &str, key: &str) {
    ui.horizontal(|ui| {
        let tone = crate::ui::t(ui);
        ui.allocate_ui_with_layout(
            egui::vec2(150., 24.),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.set_min_size(egui::vec2(150., 24.));
                ui.label(RichText::new(label).color(tone.muted));
            },
        );
        ui.label(RichText::new(key).color(tone.muted));
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Local;

    #[test]
    fn a_pressed_combination_is_checked_then_kept() {
        let mut a = App::sample(Local::now().date_naive());
        // Another action's combination is refused and nothing changes.
        a.shortcut_edit.slot = Some(Slot::Global(0));
        a.shortcut_recorded(Recorded::Combo(Combo::parse("Ctrl+F").unwrap()));
        let (_, text, refused) = a.shortcut_edit.note.clone().unwrap();
        assert!(refused && text.contains("빠른 검색"), "{text}");
        assert_eq!(a.shortcuts[0], "Ctrl+Alt+N");
        assert_eq!(a.shortcut_edit.slot, Some(Slot::Global(0)), "still waiting");
        // Windows' own.
        a.shortcut_recorded(Recorded::Combo(Combo::parse("Alt+Tab").unwrap()));
        assert!(a.shortcut_edit.note.clone().unwrap().1.contains("Windows"));
        // A free one is kept (no shortcut manager in tests: saved only).
        a.shortcut_recorded(Recorded::Combo(Combo::parse("Ctrl+Alt+M").unwrap()));
        assert_eq!(a.shortcuts[0], "Ctrl+Alt+M");
        assert_eq!(a.shortcut_edit.slot, None);
        // Esc stops without a change.
        a.shortcut_edit.slot = Some(Slot::Global(CLIPBOY));
        a.shortcut_recorded(Recorded::Cancel);
        assert_eq!(a.shortcuts[CLIPBOY], "Alt+V");
        assert_eq!(a.shortcut_edit.slot, None);
        // The program's own: the Windows key does not reach its windows.
        a.shortcut_edit.slot = Some(Slot::App(Action::Project));
        a.shortcut_recorded(Recorded::Combo(Combo::parse("Win+Alt+P").unwrap()));
        assert!(a.shortcut_edit.note.clone().unwrap().2);
        assert_eq!(keys::combo(Action::Project).to_string(), "Ctrl+P");
        // A global shortcut cannot take a program shortcut's keys either.
        a.shortcut_edit.slot = Some(Slot::App(Action::Project));
        a.shortcut_recorded(Recorded::Combo(Combo::parse("Ctrl+Alt+C").unwrap()));
        assert!(a.shortcut_edit.note.clone().unwrap().1.contains("캘린더"));
        a.shortcut_edit.stop();
    }

    #[test]
    fn waiting_for_a_key_ends_when_the_tab_is_left() {
        let mut a = App::sample(Local::now().date_naive());
        a.main_open = true;
        a.settings = true;
        a.desktop.settings_tab = TAB;
        a.shortcut_edit.slot = Some(Slot::Global(1));
        a.shortcut_edit_frame();
        assert!(a.shortcut_edit.slot.is_some());
        a.desktop.settings_tab = 0;
        a.shortcut_edit_frame();
        assert!(a.shortcut_edit.slot.is_none());
    }
}
