//! The "클립보드 히스토리" page: status, search and filter, a virtualized list
//! (click = copy back to the clipboard), a detail pane and the settings.
use super::{ClipboardHistory, Cmd, Confirm, Detail, ItemMeta, Settings};
use crate::icons::Icon;
use crate::ui;
use chrono::{Local, NaiveDate, TimeZone};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, RichText, Sense, Vec2};

const ROW_H: f32 = 56.0;
const ROW_GAP: f32 = 4.0;
const FILTERS: [&str; 5] = ["전체", "텍스트", "링크", "표", "이미지"];

fn kind_label(kind: &str) -> (&'static str, Color32) {
    match kind {
        "link" => ("링크", ui::ACCENT),
        "table" => ("표", Color32::from_rgb(46, 125, 90)),
        "image" => ("이미지", Color32::from_rgb(140, 90, 170)),
        _ => ("텍스트", Color32::from_rgb(120, 119, 115)),
    }
}

fn local_time(ms: i64) -> String {
    let Some(t) = Local.timestamp_millis_opt(ms).single() else {
        return String::new();
    };
    if t.date_naive() == Local::now().date_naive() {
        t.format("오늘 %H:%M:%S").to_string()
    } else {
        t.format("%m-%d %H:%M").to_string()
    }
}

fn megabytes(bytes: i64) -> String {
    if bytes < 1 << 20 {
        format!("{} KB", (bytes + 1023) >> 10)
    } else {
        format!("{:.1} MB", bytes as f64 / (1 << 20) as f64)
    }
}

enum Act {
    Restore(i64),
    Delete(i64),
    Select(usize),
}

impl ClipboardHistory {
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let now = ui.input(|i| i.time);
        if self.live.is_none() {
            ui::empty_state(ui, Icon::Menu, "클립보드 히스토리를 사용할 수 없습니다", "");
            return;
        }
        if self.needs_refresh {
            self.refresh();
        }
        if self.search != self.sent_search {
            if now - self.search_edited > 0.25 {
                self.refresh();
            } else {
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(260));
            }
        }
        self.header(ui, now);
        self.confirm_bar(ui);
        if let Some((msg, at)) = self.status.clone() {
            if now - at < 6.0 {
                ui.label(RichText::new(msg).size(12.5).color(ui::ACCENT));
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(500));
            } else {
                self.status = None;
            }
        }
        if !self.notice.is_empty() {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(&self.notice)
                        .size(12.5)
                        .color(Color32::from_rgb(200, 70, 60)),
                );
                if ui.small_button("닫기").clicked() {
                    self.notice.clear();
                }
            });
        }
        ui.add_space(4.0);
        let mut acts: Vec<Act> = vec![];
        self.detail_panel(ui);
        self.list(ui, &mut acts);
        for a in acts {
            match a {
                Act::Select(i) => {
                    self.sel = Some(i);
                    if let Some(id) = self.items.get(i).map(|m| m.id) {
                        self.open_detail(id);
                    }
                }
                Act::Restore(id) => self.restore(id, now),
                Act::Delete(id) => {
                    self.send(Cmd::Delete(id));
                    self.items.retain(|m| m.id != id);
                    self.textures.remove(&id);
                    if self.detail.as_ref().is_some_and(|d| d.id == id) {
                        self.detail = None;
                        self.full = None;
                    }
                }
            }
        }
    }

    /// Shows the detail of an item; the previous one is released.
    fn open_detail(&mut self, id: i64) {
        if self.detail.as_ref().is_some_and(|d| d.id == id) {
            return;
        }
        self.detail = Some(Detail {
            id,
            text: None,
            table: None,
        });
        self.full = None;
        self.send(Cmd::Detail(id));
        if self.items.iter().any(|m| m.id == id && m.kind == "image") {
            self.send(Cmd::Full(id));
        }
    }

    fn restore(&mut self, id: i64, now: f64) {
        self.send(Cmd::Restore(id));
        self.status = Some(("복사 중…".into(), now));
    }

    fn header(&mut self, ui: &mut egui::Ui, now: f64) {
        let c = ui::t(ui);
        let s = self.settings.clone();
        if !s.enabled {
            ui::card(ui, |ui| {
                ui.label(RichText::new("클립보드 히스토리").size(15.0).strong());
                ui.add_space(4.0);
                ui.label(
                    "메모장이 켜져 있는 동안 다른 프로그램에서 복사한 텍스트·링크·표·이미지를 \
                     이 PC에 저장하고, 항목을 한 번 클릭하면 다시 클립보드로 되돌립니다.",
                );
                ui.add_space(4.0);
                ui::hint(
                    ui,
                    "비밀번호 같은 민감한 내용도 저장될 수 있고 저장 파일은 암호화되지 않습니다. \
                     인터넷으로 전송하지 않으며, 언제든 일시 중지하거나 전체 삭제할 수 있습니다.",
                );
                ui.add_space(8.0);
                if ui::primary(ui, "수집 시작").clicked() {
                    self.set_state(true, false);
                }
            });
            ui.add_space(6.0);
        } else {
            ui.horizontal(|ui| {
                let (text, fill, ink) = if s.paused {
                    (
                        "● 일시 중지",
                        Color32::from_rgb(250, 236, 200),
                        Color32::from_rgb(140, 100, 20),
                    )
                } else {
                    (
                        "● 수집 중",
                        Color32::from_rgb(214, 238, 222),
                        Color32::from_rgb(30, 100, 60),
                    )
                };
                ui::chip(ui, text, fill, ink);
                if ui
                    .button(if s.paused {
                        "다시 시작"
                    } else {
                        "일시 중지"
                    })
                    .clicked()
                {
                    self.set_state(true, !s.paused);
                }
                if ui.button("수집 끄기").clicked() {
                    self.set_state(false, false);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if crate::icons::toggle_icon(
                        ui,
                        Icon::Settings,
                        "설정·삭제",
                        self.settings_open,
                    )
                    .clicked()
                    {
                        self.settings_open = !self.settings_open;
                    }
                    ui::hint(
                        ui,
                        format!("{}건 · {}", self.stats.0, megabytes(self.stats.1)),
                    );
                });
            });
            ui::hint(ui, "민감한 정보를 복사하기 전에는 ‘일시 중지’를 누르세요");
        }
        if self.settings_open {
            self.settings_card(ui);
        }
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let r = ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text("검색")
                    .desired_width(180.0),
            );
            if r.changed() {
                self.search_edited = now;
            }
            self.search_focused = r.has_focus();
            for (i, name) in FILTERS.iter().enumerate() {
                if ui::pill(ui, self.filter == i, name).clicked() && self.filter != i {
                    self.filter = i;
                    self.needs_refresh = true;
                }
            }
        });
        let _ = c;
    }

    pub(super) fn set_state(&mut self, enabled: bool, paused: bool) {
        let was = self.settings.collecting();
        self.settings.enabled = enabled;
        self.settings.paused = paused;
        self.apply_collecting(was);
    }

    fn settings_card(&mut self, ui: &mut egui::Ui) {
        let before = self.settings.clone();
        ui::card(ui, |ui| {
            let s = &mut self.settings;
            ui.label(RichText::new("정리 기준 (체크한 기준만 적용)").strong());
            ui.horizontal(|ui| {
                ui.checkbox(&mut s.count_limit, "개수로 제한");
                ui.add_enabled_ui(s.count_limit, |ui| {
                    egui::ComboBox::from_id_salt("cb_max")
                        .selected_text(format!("최근 {}개", s.max_items))
                        .show_ui(ui, |ui| {
                            for n in Settings::ITEM_CHOICES {
                                ui.selectable_value(&mut s.max_items, n, format!("최근 {n}개"));
                            }
                        });
                });
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut s.period_limit, "기간으로 제한");
                ui.add_enabled_ui(s.period_limit, |ui| {
                    egui::ComboBox::from_id_salt("cb_days")
                        .selected_text(format!("{}일 보관", s.retention_days))
                        .show_ui(ui, |ui| {
                            for n in Settings::DAY_CHOICES {
                                ui.selectable_value(
                                    &mut s.retention_days,
                                    n,
                                    format!("{n}일 보관"),
                                );
                            }
                        });
                });
            });
            ui.add_enabled_ui(s.period_limit, |ui| {
                ui::field(ui, "자동 정리 주기", |ui| {
                    egui::ComboBox::from_id_salt("cb_tidy")
                        .selected_text(tidy_label(s.tidy_hours))
                        .show_ui(ui, |ui| {
                            for n in Settings::TIDY_CHOICES {
                                ui.selectable_value(&mut s.tidy_hours, n, tidy_label(n));
                            }
                        });
                });
            });
            ui::hint(
                ui,
                "개수 기준은 새로 복사할 때마다 적용됩니다. 자동 정리 주기는 기간 기준에만 쓰이며,                  복사가 없는 동안에도 오래된 항목을 지웁니다.",
            );
            ui::hint(ui, "용량은 합계 256 MB까지이며 오래된 항목부터 지웁니다");
            ui.add_space(4.0);
            ui::divider(ui);
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.date_input)
                        .hint_text("YYYY-MM-DD")
                        .desired_width(100.0),
                );
                if ui.button("이 날짜 이전 삭제").clicked() {
                    if let Ok(d) = NaiveDate::parse_from_str(self.date_input.trim(), "%Y-%m-%d") {
                        let local = d
                            .and_hms_opt(0, 0, 0)
                            .and_then(|t| Local.from_local_datetime(&t).earliest());
                        if let Some(t) = local {
                            self.confirm = Some(Confirm::Before(
                                t.timestamp_millis(),
                                format!(
                                    "{} (현지 시간) 이전에 복사한 기록",
                                    t.format("%Y-%m-%d %H:%M")
                                ),
                            ));
                        }
                    } else {
                        self.notice = "날짜는 YYYY-MM-DD 형식으로 입력하세요".into();
                    }
                }
                if ui.button("전체 삭제").clicked() {
                    self.confirm = Some(Confirm::All);
                }
            });
            let c = &self.live.as_ref().map(|l| &l.shared.counters);
            if let Some(c) = c {
                use std::sync::atomic::Ordering::Relaxed;
                ui::hint(
                    ui,
                    format!(
                        "건너뜀: 잠금 {} · 크기 초과 {} · 미지원 {} · 대기 초과 {} · 저장 오류 {}",
                        c.missed_locked.load(Relaxed),
                        c.oversize.load(Relaxed),
                        c.unsupported.load(Relaxed),
                        c.dropped_backlog.load(Relaxed),
                        c.store_errors.load(Relaxed),
                    ),
                );
            }
        });
        if self.settings != before {
            self.policy_changed();
        }
    }

    fn confirm_bar(&mut self, ui: &mut egui::Ui) {
        let Some(c) = self.confirm.clone() else {
            return;
        };
        let text = match &c {
            Confirm::All => "저장된 클립보드 기록 전체를 삭제합니다.".to_owned(),
            Confirm::Before(_, label) => format!("{label}을 삭제합니다."),
        };
        ui::card(ui, |ui| {
            ui.label(RichText::new(text).strong());
            ui::hint(
                ui,
                "되돌릴 수 없습니다. 지금 클립보드와 Windows 클립보드 기록(Win+V)에는 영향이 없습니다.",
            );
            let (yes, no) = ui::dialog_buttons(ui, "삭제", "취소");
            if yes {
                match c {
                    Confirm::All => {
                        let watermark = self.live.as_ref().map_or(0, |l| {
                            l.shared.seq.load(std::sync::atomic::Ordering::Relaxed)
                        });
                        self.send(Cmd::DeleteAll { watermark });
                    }
                    Confirm::Before(ms, _) => self.send(Cmd::DeleteBefore(ms)),
                }
                self.detail = None;
                self.confirm = None;
            } else if no {
                self.confirm = None;
            }
        });
        ui.add_space(4.0);
    }

    fn detail_panel(&mut self, ui: &mut egui::Ui) {
        let Some(d) = self.detail.clone() else { return };
        let meta = self.items.iter().find(|m| m.id == d.id).cloned();
        let mut close = false;
        let big = self.detail_big;
        let height = if big {
            ui.available_height() * 0.75
        } else {
            300.0
        };
        egui::TopBottomPanel::bottom(if big { "clipboard_detail_big" } else { "clipboard_detail" })
            .resizable(true)
            .default_height(height)
            .min_height(90.0)
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    if let Some(m) = &meta {
                        ui.label(RichText::new(format!("상세 · {}", kind_label(&m.kind).0)).strong());
                        ui::hint(ui, format!("{} · {}", local_time(m.captured_ms), megabytes(m.bytes)));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if crate::icons::icon_button(ui, Icon::Close, "상세 닫기").clicked() {
                            close = true;
                        }
                        if ui.small_button(if big { "작게" } else { "크게" }).clicked() {
                            self.detail_big = !big;
                        }
                    });
                });
                egui::ScrollArea::both().id_salt("clip_detail_scroll").auto_shrink([false, false]).show(ui, |ui| {
                    if let Some((rows, total_rows, total_cols)) = &d.table {
                        egui::Grid::new("clip_table").striped(true).spacing([14.0, 4.0]).show(ui, |ui| {
                            for row in rows {
                                for cell in row {
                                    ui.label(cell);
                                }
                                ui.end_row();
                            }
                        });
                        let shown = rows.len();
                        if shown < *total_rows || rows.iter().any(|r| r.len() < *total_cols) {
                            ui::hint(
                                ui,
                                format!("표 {total_rows}×{total_cols} 중 앞부분만 표시합니다 (복사는 전체)"),
                            );
                        }
                    } else if let Some((id, tex)) = &self.full {
                        if *id == d.id {
                            let size = tex.size_vec2();
                            let k = (ui.available_width() / size.x).clamp(0.05, 1.0);
                            ui.image((tex.id(), size * k));
                        }
                    } else if let Some(t) = &d.text {
                        ui.label(t);
                    } else if let Some(tex) = self.textures.get(&d.id) {
                        ui.image((tex.id(), tex.size_vec2()));
                        ui::hint(ui, "큰 미리보기를 불러오는 중…");
                    } else {
                        ui::hint(ui, "불러오는 중…");
                    }
                });
            });
        if close {
            self.detail = None;
            self.full = None;
        }
    }

    fn list(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Act>) {
        if self.items.is_empty() {
            if !self.loading {
                let searching = !self.search.trim().is_empty() || self.filter != 0;
                ui::empty_state(
                    ui,
                    Icon::Menu,
                    if searching {
                        "조건에 맞는 기록이 없습니다"
                    } else {
                        "아직 기록이 없습니다"
                    },
                    if self.settings.collecting() || searching {
                        ""
                    } else {
                        "수집을 시작하면 이후 복사한 내용이 여기에 쌓입니다"
                    },
                );
            }
            return;
        }
        // Arrow keys and Enter, unless the search box is being typed in.
        let typing = self.search_focused || ui.ctx().wants_keyboard_input();
        if !typing {
            let (down, up, enter) = ui.input(|i| {
                (
                    i.key_pressed(egui::Key::ArrowDown),
                    i.key_pressed(egui::Key::ArrowUp),
                    i.key_pressed(egui::Key::Enter),
                )
            });
            let last = self.items.len() - 1;
            let next = match (down, up, self.sel) {
                (true, _, None) => Some(0),
                (true, _, Some(s)) => Some((s + 1).min(last)),
                (_, true, Some(s)) => Some(s.saturating_sub(1)),
                _ => None,
            };
            if let Some(n) = next {
                self.sel = Some(n);
                acts.push(Act::Select(n));
                let (off, view) = self.scroll;
                let top = n as f32 * (ROW_H + ROW_GAP);
                if top < off {
                    self.scroll_to = Some(top);
                } else if top + ROW_H > off + view {
                    self.scroll_to = Some((top + ROW_H - view).max(0.0));
                }
            }
            if enter {
                if let Some(m) = self.sel.and_then(|s| self.items.get(s)) {
                    acts.push(Act::Restore(m.id));
                }
            }
        }
        let mut area = egui::ScrollArea::vertical()
            .id_salt("clipboard_list")
            .auto_shrink([false, false]);
        if let Some(y) = self.scroll_to.take() {
            area = area.vertical_scroll_offset(y);
        }
        ui.spacing_mut().item_spacing.y = ROW_GAP;
        let mut want_more = false;
        let mut thumbs: Vec<i64> = vec![];
        let out = area.show_rows(ui, ROW_H, self.items.len(), |ui, range| {
            if range.end + 5 >= self.items.len() {
                want_more = true;
            }
            for i in range {
                let m = &self.items[i];
                row(
                    ui,
                    i,
                    m,
                    self.sel == Some(i),
                    self.textures.get(&m.id),
                    acts,
                );
                if m.has_thumb
                    && !self.textures.contains_key(&m.id)
                    && !self.thumb_asked.contains(&m.id)
                {
                    thumbs.push(m.id);
                }
            }
        });
        self.scroll = (out.state.offset.y, out.inner_rect.height());
        for id in thumbs {
            self.thumb_asked.insert(id);
            self.send(Cmd::Thumb(id));
        }
        if want_more {
            self.load_more();
        }
    }
}

fn tidy_label(hours: u32) -> String {
    if hours == 0 {
        "시작할 때만".into()
    } else {
        format!("{}일마다", hours / 24)
    }
}

fn row(
    ui: &mut egui::Ui,
    i: usize,
    m: &ItemMeta,
    selected: bool,
    tex: Option<&egui::TextureHandle>,
    acts: &mut Vec<Act>,
) {
    let c = ui::t(ui);
    let (rect, resp) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), ROW_H), Sense::click());
    let painter = ui.painter_at(rect);
    let fill = if selected {
        c.accent_soft
    } else if resp.hovered() {
        c.hover
    } else {
        Color32::TRANSPARENT
    };
    painter.rect_filled(rect, 8.0, fill);
    // left column: thumbnail or the kind label
    let left = Rect::from_min_size(rect.min + Vec2::new(8.0, 6.0), Vec2::splat(ROW_H - 12.0));
    let (label, tint) = kind_label(&m.kind);
    match (m.kind.as_str(), tex) {
        ("image", Some(t)) => {
            let s = t.size_vec2();
            let k = (left.width() / s.x).min(left.height() / s.y);
            let r = Rect::from_center_size(left.center(), s * k);
            painter.image(
                t.id(),
                r,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        _ => {
            painter.rect_filled(left, 6.0, tint.gamma_multiply(0.14));
            painter.text(
                left.center(),
                Align2::CENTER_CENTER,
                label,
                FontId::proportional(12.0),
                tint,
            );
        }
    }
    let text_left = left.right() + 10.0;
    let buttons_w = 92.0;
    let clip = Rect::from_min_max(
        Pos2::new(text_left, rect.top()),
        Pos2::new(
            (rect.right() - buttons_w).max(text_left + 20.0),
            rect.bottom(),
        ),
    );
    let tp = painter.with_clip_rect(clip);
    tp.text(
        Pos2::new(text_left, rect.top() + 9.0),
        Align2::LEFT_TOP,
        &m.summary,
        FontId::proportional(14.0),
        c.ink,
    );
    let mut sub = local_time(m.captured_ms);
    if !m.meta.is_empty() {
        sub.push_str(&format!(" · {}", m.meta));
    }
    sub.push_str(&format!(" · {}", megabytes(m.bytes)));
    if m.reduced {
        sub.push_str(" · 일부 형식만 저장됨");
    }
    tp.text(
        Pos2::new(text_left, rect.top() + 31.0),
        Align2::LEFT_TOP,
        sub,
        FontId::proportional(11.5),
        c.muted,
    );
    // right buttons: detail, delete
    let mut hit_button = false;
    for (n, (name, is_delete)) in [("복사", false), ("삭제", true)].into_iter().enumerate() {
        let r = Rect::from_min_size(
            Pos2::new(
                rect.right() - buttons_w + n as f32 * 44.0,
                rect.center().y - 12.0,
            ),
            Vec2::new(40.0, 24.0),
        );
        let b = ui.interact(r, ui.id().with(("clip_btn", m.id, n)), Sense::click());
        if b.hovered() {
            painter.rect_filled(r, 6.0, c.pressed);
            hit_button = true;
        }
        painter.text(
            r.center(),
            Align2::CENTER_CENTER,
            name,
            FontId::proportional(12.0),
            if is_delete {
                Color32::from_rgb(190, 70, 60)
            } else {
                c.muted
            },
        );
        if b.clicked() {
            acts.push(if is_delete {
                Act::Delete(m.id)
            } else {
                Act::Restore(m.id)
            });
        }
    }
    if resp.clicked() && !hit_button {
        acts.push(Act::Select(i));
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(format!("저장된 형식: {}", m.formats));
}
