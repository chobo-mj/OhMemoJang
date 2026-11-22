use crate::*;
use chrono::Duration;
use egui::{Rect, Sense, Stroke};

struct StandingScheduleDrag {
    note: usize,
    from: NaiveDate,
}

fn dragged_sticker_bar(cell: Rect, pointer: egui::Pos2, height: f32) -> Rect {
    Rect::from_min_size(
        pointer - Vec2::new(12., height * 0.5),
        Vec2::new((cell.width() - 8.).clamp(28., 220.), height),
    )
}
impl App {
    pub fn filters(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 6.;
            let combo = egui::ComboBox::from_id_salt(ui.id().with("projects"))
                .width(150_f32.min(ui.available_width()))
                .selected_text(format!(
                    "# {}",
                    crate::refinements::project_label(self.project_filter, &self.data.projects)
                ))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.project_filter, None, "모든 프로젝트");
                    ui.selectable_value(
                        &mut self.project_filter,
                        Some(NO_PROJECT),
                        "프로젝트 없음",
                    );
                    for (i, p) in self.data.projects.iter().enumerate() {
                        if self.extra.hidden_projects.contains(&i)
                            || self.extra.deleted_projects.contains(&i)
                        {
                            continue;
                        }
                        ui.selectable_value(&mut self.project_filter, Some(i), p);
                    }
                    // The open list too: it must stay usable in the tutorial.
                    crate::tutorial::mark(
                        ui.ctx(),
                        crate::tutorial::Spot::Filter,
                        ui.min_rect().expand(8.),
                    );
                });
            crate::tutorial::mark(ui.ctx(), crate::tutorial::Spot::Filter, combo.response.rect);
            if ui::pill(ui, self.recurring, "정기 일정 포함").clicked() {
                self.recurring = !self.recurring;
            }
            if ui::pill(ui, self.extra.created_basis, "생성일 기준").clicked() {
                self.extra.created_basis = !self.extra.created_basis;
            }
            if ui::pill(ui, self.extra.show_done, "완료 메모 보기").clicked() {
                self.extra.show_done = !self.extra.show_done;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui::primary(ui, "+ 새 일정").clicked() {
                    self.new_schedule(match self.page {
                        2 => Local::now().date_naive(),
                        3 => Local::now().date_naive() + Duration::days(1),
                        _ => self.day,
                    });
                }
            });
        });
    }
    pub fn task_row_on(&mut self, ui: &mut egui::Ui, i: usize, occurrence: Option<NaiveDate>) {
        let tone = ui::t(ui);
        let base = match self.desktop.row_spacing {
            0 => 40.,
            2 => 54.,
            _ => 46.,
        };
        // In search results the row also holds a "달력에서 보기" line.
        let jump = self.extra.search_mode.then(|| {
            if self.extra.created_basis {
                Some(self.data.notes[i].created.date())
            } else {
                self.data.notes[i].date
            }
        });
        let jump = jump.flatten();
        let height = base + if jump.is_some() { 18. } else { 0. };
        // On 오늘의 할일 / 다음에 할일 a click shows the memo in the preview
        // pane; the eye button beside the check opens it as a sticker.
        let preview = matches!(self.page, 2 | 3);
        let lead = if preview { 26. } else { 0. };
        ui.push_id((i, occurrence), |ui| {
            let (rect, row) = ui.allocate_exact_size(
                Vec2::new(ui.available_width(), height),
                Sense::click_and_drag(),
            );
            let hovered = ui.rect_contains_pointer(rect);
            let selected = preview && self.catalog.selected == Some(i);
            if selected {
                ui.painter().rect_filled(rect, 6., tone.accent_soft);
            } else if hovered {
                ui.painter()
                    .rect_filled(rect, 6., tone.hover.gamma_multiply(0.6));
            }
            ui.painter().line_segment(
                [rect.left_bottom() + Vec2::new(40., 0.), rect.right_bottom()],
                Stroke::new(1.0_f32, tone.line),
            );
            row.dnd_set_drag_payload(NoteDrag);
            self.drag_source_on(ui, &row, i, occurrence);
            if let Some(source) = self.sources.last_mut() {
                source.clear_on_miss = occurrence.is_some() || matches!(self.page, 2 | 3);
            }
            if hovered {
                for (dx, dy) in [(0., -4.), (0., 0.), (0., 4.), (4., -4.), (4., 0.), (4., 4.)] {
                    ui.painter().circle_filled(
                        egui::pos2(rect.left() + 8. + dx, rect.center().y + dy),
                        1.1,
                        tone.faint,
                    );
                }
            }
            let mut toggle_sticker = false;
            if preview {
                let open = self.studio.windows.get(&i).is_some_and(|w| w.open);
                let eye = Rect::from_center_size(
                    egui::pos2(rect.left() + 30., rect.top() + 15.),
                    Vec2::splat(24.),
                );
                toggle_sticker = ui
                    .put(eye, |ui: &mut egui::Ui| {
                        icons::toggle_icon(ui, icons::Icon::Eye, "스티커 표시 / 숨기기", open)
                    })
                    .clicked();
            }
            let mut d = self.data.notes[i].done_on(occurrence);
            let check = Rect::from_center_size(
                egui::pos2(rect.left() + 30. + lead, rect.top() + 15.),
                Vec2::splat(20.),
            );
            if ui
                .put(check, |ui: &mut egui::Ui| ui::round_check(ui, &mut d, None))
                .changed()
            {
                self.done_at(i, occurrence, d);
                self.extra.scope_owner = ui.ctx().viewport_id();
            }
            let n = &self.data.notes[i];
            let title = if n.title.trim().is_empty() {
                "제목 없음".to_owned()
            } else {
                n.title.clone()
            };
            let title = if n.deadline {
                format!("🚩 [{}] {}", n.dday(Local::now().date_naive()), title)
            } else {
                title
            };
            // Checklist progress: ☑ done/all.
            let title = match crate::model::check_progress(&n.body) {
                Some((done, all)) => format!("{title}   ☑ {done}/{all}"),
                None => title,
            };
            let text_left = rect.left() + 48. + lead;
            let actions = if hovered || n.starred { 64. } else { 0. };
            let title_font = egui::TextStyle::Body.resolve(ui.style());
            let mut job = egui::text::LayoutJob::single_section(
                title,
                egui::TextFormat {
                    font_id: title_font,
                    color: if n.done_on(occurrence) {
                        tone.faint
                    } else {
                        tone.ink
                    },
                    strikethrough: if n.done_on(occurrence) {
                        Stroke::new(1.0_f32, tone.faint)
                    } else {
                        Stroke::NONE
                    },
                    ..Default::default()
                },
            );
            job.wrap = egui::text::TextWrapping::truncate_at_width(
                (rect.right() - text_left - actions - 8.).max(40.),
            );
            let galley = ui.fonts(|f| f.layout_job(job));
            ui.painter()
                .galley(egui::pos2(text_left, rect.top() + 6.), galley, tone.ink);
            let mut meta = vec![];
            if self.extra.created_basis {
                meta.push(format!("생성 {}", n.created.format("%m.%d %H:%M")));
            } else if let Some(d) = occurrence {
                meta.push(format!(
                    "{} {}",
                    d.format("%-m월 %-d일"),
                    n.time.format("%H:%M")
                ));
            }
            if n.recurring {
                meta.push("↻ 정기".into());
            }
            if let Some(t) = n.reminder {
                meta.push(format!("⏰ {}", t.format("%m/%d %H:%M")));
            }
            if let Some(p) = n.project {
                meta.push(format!("# {}", self.data.projects[p]));
            }
            if self.desktop.show_preview && self.split && !n.locked {
                let body = crate::composer::strip_pictures(&n.body).replace('\n', " ");
                if !body.trim().is_empty() {
                    meta.push(body);
                }
            }
            if !meta.is_empty() {
                let mut job = egui::text::LayoutJob::single_section(
                    meta.join("  ·  "),
                    egui::TextFormat {
                        font_id: egui::FontId::proportional(12.),
                        color: tone.muted,
                        ..Default::default()
                    },
                );
                job.wrap = egui::text::TextWrapping::truncate_at_width(
                    (rect.right() - text_left - actions - 8.).max(40.),
                );
                let galley = ui.fonts(|f| f.layout_job(job));
                ui.painter().galley(
                    egui::pos2(text_left, rect.top() + base - 20.),
                    galley,
                    tone.muted,
                );
            }
            let mut toggled_star = false;
            let mut edit = false;
            if hovered || self.data.notes[i].starred {
                let mut x = rect.right() - 28.;
                if hovered {
                    let r =
                        Rect::from_min_size(egui::pos2(x, rect.center().y - 14.), Vec2::splat(28.));
                    edit = ui
                        .put(r, |ui: &mut egui::Ui| {
                            icons::icon_button(ui, icons::Icon::Edit, "편집창 열기")
                        })
                        .clicked();
                    x -= 30.;
                }
                let starred = self.data.notes[i].starred;
                let r = Rect::from_min_size(egui::pos2(x, rect.center().y - 14.), Vec2::splat(28.));
                toggled_star = ui
                    .put(r, |ui: &mut egui::Ui| {
                        icons::toggle_icon(
                            ui,
                            icons::Icon::Star,
                            if starred {
                                "중요 해제"
                            } else {
                                "중요 표시"
                            },
                            starred,
                        )
                    })
                    .clicked();
            }
            if toggled_star {
                let n = &mut self.data.notes[i];
                n.starred = !n.starred;
            }
            if toggle_sticker {
                self.toggle_sticker(i);
            } else if edit {
                self.open_editor_on(i, occurrence);
            } else if row.clicked() {
                if preview {
                    self.catalog.selected = Some(i);
                    self.catalog.selected_day = occurrence;
                } else {
                    self.open_note_on(i, occurrence);
                }
            }
            crate::float::context_menu(&row, 180., |ui| {
                if ui.button("편집창 열기").clicked() {
                    self.open_editor_on(i, occurrence);
                    ui.close_menu();
                }
                if ui.button("스티커로 꺼내기").clicked() {
                    self.open_sticker(i, occurrence);
                    ui.close_menu();
                }
                if ui.button("알람 설정").clicked() {
                    self.open_reminder(i);
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("휴지통으로 이동").clicked() {
                    self.trash_note(i);
                    ui.close_menu();
                }
            });
            if let Some(d) = jump {
                let at = Rect::from_min_size(
                    egui::pos2(rect.left() + 48. + lead, rect.top() + base - 2.),
                    Vec2::new((rect.width() - 56. - lead).max(40.), 16.),
                );
                let link = ui.put(at, |ui: &mut egui::Ui| {
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.link(RichText::new(format!("{d} · 달력에서 보기")).small())
                    })
                    .inner
                });
                if link.clicked() {
                    self.month = d.with_day(1).unwrap();
                    self.day = d;
                    self.split = true;
                }
            }
        });
    }
    /// 오늘의 할일 / 다음에 할일: the memo list with the preview pane on the
    /// right, where the selected memo is read and edited.
    pub fn agenda_preview(&mut self, ui: &mut egui::Ui) {
        let rect = ui.available_rect_before_wrap();
        let height = rect.height();
        let preview_max = (rect.width() - 360.).max(260.);
        let page = usize::from(self.page == 3);
        let preview_width = self.prefs.main_widths.agenda_preview[page].clamp(260., preview_max);
        let list_rect = Rect::from_min_size(
            rect.min,
            Vec2::new((rect.width() - preview_width - 16.).max(200.), height),
        );
        let preview_rect = Rect::from_min_size(
            egui::pos2(list_rect.right() + 16., rect.top()),
            Vec2::new(preview_width, height),
        );
        let mut list = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("agenda_list_pane")
                .max_rect(list_rect)
                .layout(egui::Layout::top_down(egui::Align::LEFT)),
        );
        list.set_clip_rect(ui.clip_rect().intersect(list_rect));
        egui::ScrollArea::vertical()
            .id_salt(("page_scroll", self.page))
            .auto_shrink([false, false])
            .show(&mut list, |ui| {
                ui.set_min_width((ui.available_width() - 8.).max(200.));
                self.agenda(ui, self.page);
            });
        let handle = Rect::from_min_max(
            egui::pos2(list_rect.right() + 2., rect.top()),
            egui::pos2(list_rect.right() + 14., rect.bottom()),
        );
        let drag = ui
            .interact(handle, ui.id().with("agenda_splitter"), Sense::drag())
            .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
        if drag.hovered() || drag.dragged() {
            ui.painter().line_segment(
                [handle.center_top(), handle.center_bottom()],
                Stroke::new(2.0_f32, ACCENT.gamma_multiply(0.6)),
            );
        }
        if drag.dragged() {
            self.prefs.main_widths.agenda_preview[page] =
                (preview_width - drag.drag_delta().x).clamp(260., preview_max);
        }
        let mut preview = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("agenda_preview_pane")
                .max_rect(preview_rect)
                .layout(egui::Layout::top_down(egui::Align::LEFT)),
        );
        preview.set_clip_rect(ui.clip_rect().intersect(preview_rect));
        self.catalog_preview(&mut preview, height);
        ui.allocate_rect(rect, Sense::hover());
    }
    /// Memo rows of the agenda (each day of a multi-day memo is a row),
    /// filtered and sorted.
    fn agenda_rows(&self, mode: usize, today: NaiveDate) -> Vec<(usize, Option<NaiveDate>)> {
        let mut rows = vec![];
        for (i, n) in self.data.notes.iter().enumerate() {
            if !self.matches_search(i)
                || self.is_guide(i)
                || (self.desktop.hide_locked && n.locked)
                || (self.desktop.hide_completed
                    && n.done
                    && !self.extra.search_mode
                    && !self.extra.show_done)
                || (!self.recurring && n.recurring)
                || !crate::refinements::project_match(self.project_filter, n.project)
            {
                continue;
            }
            let days = if n.date.is_some() {
                n.days().into_iter().map(Some).collect()
            } else {
                vec![None]
            };
            for d in days {
                if match mode {
                    2 => d == Some(today),
                    3 => d.is_some_and(|d| d > today),
                    _ => true,
                } {
                    rows.push((i, d));
                }
            }
        }
        rows.sort_by_key(|&(i, d)| {
            let n = &self.data.notes[i];
            if self.extra.created_basis {
                (n.created.date(), n.created.time(), i, d)
            } else {
                (d.unwrap_or(NaiveDate::MAX), n.time, i, d)
            }
        });
        rows
    }
    pub fn agenda(&mut self, ui: &mut egui::Ui, mode: usize) {
        if !self.split && mode == 0 {
            self.detail_workspace(ui, mode);
            return;
        }
        let today = Local::now().date_naive();
        self.filters(ui);
        ui.add_space(14.);
        if mode == 0 {
            crate::tutorial::mark(
                ui.ctx(),
                crate::tutorial::Spot::List,
                ui.available_rect_before_wrap().intersect(ui.clip_rect()),
            );
        }
        let mut cache = std::mem::take(&mut self.lists);
        let key = crate::lists::key((mode, today, self.data.notes.len()));
        let rows = cache.get("agenda", key, || self.agenda_rows(mode, today));
        self.lists = cache;
        let rows = (*rows).clone();
        if mode == 2 || mode == 3 {
            let mut seen = std::collections::HashSet::new();
            let important: Vec<_> = rows
                .iter()
                .copied()
                .filter(|&(i, _)| self.data.notes[i].starred && seen.insert(i))
                .collect();
            if !important.is_empty() {
                group_header(ui, "중요", important.len());
                ui.push_id("important", |ui| {
                    for (i, d) in important {
                        crate::lists::lazy_row(ui, (i, d), 48., false, |ui| {
                            self.task_row_on(ui, i, d)
                        });
                    }
                });
                ui.add_space(18.);
            }
        }
        if rows.is_empty() {
            ui.add_space(40.);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new("표시할 메모가 없습니다")
                        .size(15.)
                        .color(ui::t(ui).muted),
                );
                ui::hint(ui, "필터를 바꾸거나 새 메모를 추가해 보세요");
            });
        }
        type Row = (usize, Option<NaiveDate>);
        let mut groups: Vec<(Option<NaiveDate>, Vec<Row>)> = vec![];
        for (i, occurrence) in rows {
            let d = if self.extra.created_basis {
                Some(self.data.notes[i].created.date())
            } else {
                occurrence
            };
            match groups.last_mut() {
                Some((g, items)) if *g == d => items.push((i, occurrence)),
                _ => groups.push((d, vec![(i, occurrence)])),
            }
        }
        for (d, items) in groups {
            let label = d
                .map(|d| {
                    let rel = match (d - today).num_days() {
                        0 => " · 오늘",
                        1 => " · 내일",
                        -1 => " · 어제",
                        _ => "",
                    };
                    format!("{} {}{rel}", d.format("%-m월 %-d일"), ui::weekday(d))
                })
                .unwrap_or("일정 없는 메모".into());
            group_header(ui, &label, items.len());
            for (i, occurrence) in items {
                crate::lists::lazy_row(ui, (i, occurrence), 48., false, |ui| {
                    self.task_row_on(ui, i, occurrence)
                });
            }
            ui.add_space(18.);
        }
    }
    pub fn calendar(&mut self, ui: &mut egui::Ui) {
        self.calendar_grid(ui);
        self.calendar_day_detail(ui);
    }
    /// The month/week grid with its navigation header and memo bands.
    pub fn calendar_grid(&mut self, ui: &mut egui::Ui) {
        self.calendar_grid_with(ui, None);
    }
    /// `side` is `Some(on)` where the header should carry the side-by-side
    /// toggle (the main window's calendar); returns true when it was clicked.
    pub fn calendar_grid_with(&mut self, ui: &mut egui::Ui, side: Option<bool>) -> bool {
        let mut toggled = false;
        let today = Local::now().date_naive();
        let (tone, sun, sat, today_mark) = self.calendar_tone(ui);
        ui.horizontal(|ui| {
            ui.set_min_height(32.);
            ui.label(
                RichText::new(self.month.format("%Y년 %-m월").to_string())
                    .size(18.)
                    .strong(),
            );
            ui.add_space(4.);
            let weeks = self.prefs.cal_weeks;
            let (back, forward) = if weeks == 0 {
                ("이전 달", "다음 달")
            } else {
                ("이전 주", "다음 주")
            };
            if icons::icon_button(ui, icons::Icon::ChevronLeft, back).clicked() {
                if weeks == 0 {
                    self.month = (self.month - Duration::days(1)).with_day(1).unwrap();
                } else {
                    self.day -= Duration::days(7 * weeks as i64);
                    self.month = self.day.with_day(1).unwrap();
                }
            }
            if icons::icon_button(ui, icons::Icon::ChevronRight, forward).clicked() {
                if weeks == 0 {
                    self.month = (self.month + Duration::days(32)).with_day(1).unwrap();
                } else {
                    self.day += Duration::days(7 * weeks as i64);
                    self.month = self.day.with_day(1).unwrap();
                }
            }
            if ui.button("오늘").clicked() {
                self.month = today.with_day(1).unwrap();
                self.day = today;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(on) = side {
                    // Top right: schedule and calendar side by side.
                    if icons::toggle_icon(
                        ui,
                        icons::Icon::SplitView,
                        if on {
                            "일정·캘린더 세로 보기로 전환"
                        } else {
                            "일정·캘린더 가로 보기 · 선택한 날짜의 일정을 캘린더 오른쪽에"
                        },
                        on,
                    )
                    .clicked()
                    {
                        toggled = true;
                    }
                }
                ui::segmented(
                    ui,
                    &mut self.desktop.calendar_basis,
                    &["작성일", "일정", "알람"],
                );
                // Will open the linked Excel file of who is on duty; not
                // connected yet.
                let _ = ui
                    .button("근무자 보기")
                    .on_hover_text("연결된 엑셀 파일의 근무자 보기 · 준비 중");
            });
        });
        ui.add_space(10.);
        let width = ui.available_width().max(280.);
        let col = width / 7.;
        let (header, _) = ui.allocate_exact_size(Vec2::new(width, 26.), Sense::hover());
        for (i, label) in ["일", "월", "화", "수", "목", "금", "토"]
            .iter()
            .enumerate()
        {
            ui.painter().text(
                header.min + Vec2::new(i as f32 * col + 10., 13.),
                egui::Align2::LEFT_CENTER,
                label,
                egui::FontId::proportional(12.),
                match i {
                    0 => sun,
                    6 => sat,
                    _ => tone.faint,
                },
            );
        }
        let weeks = self.prefs.cal_weeks;
        // Month view starts on the Sunday before the 1st; week views on the
        // Sunday of the selected day's week.
        let (first, rows) = if weeks == 0 {
            let offset = self.month.weekday().num_days_from_sunday() as i64;
            let next = (self.month + Duration::days(32)).with_day(1).unwrap();
            let days = (next - self.month).num_days() + offset;
            (
                self.month - Duration::days(offset),
                (days as usize).div_ceil(7),
            )
        } else {
            let offset = self.day.weekday().num_days_from_sunday() as i64;
            (self.day - Duration::days(offset), weeks)
        };
        let height = match (weeks, width < 580.) {
            (1, _) => 300.,
            (2, _) => 220.,
            (_, true) => 104.,
            _ => 122.,
        };
        let (all, _) =
            ui.allocate_exact_size(Vec2::new(width, rows as f32 * height), Sense::hover());
        if let Some(theme) = self.cal.active() {
            ui.painter().rect_filled(all, 8., theme.card);
        }
        ui.painter().rect_stroke(
            all,
            8.,
            Stroke::new(1.0_f32, tone.line),
            egui::StrokeKind::Inside,
        );
        for r in 1..rows {
            let y = all.top() + r as f32 * height;
            ui.painter().line_segment(
                [egui::pos2(all.left(), y), egui::pos2(all.right(), y)],
                Stroke::new(1.0_f32, tone.line),
            );
        }
        for c in 1..7 {
            let x = all.left() + c as f32 * col;
            ui.painter().line_segment(
                [egui::pos2(x, all.top()), egui::pos2(x, all.bottom())],
                Stroke::new(1.0_f32, tone.line),
            );
        }
        for slot in 0..rows * 7 {
            let rect = Rect::from_min_size(
                all.min + Vec2::new((slot % 7) as f32 * col, (slot / 7) as f32 * height),
                Vec2::new(col, height),
            );
            let day = first + Duration::days(slot as i64);
            let weekday = day.weekday().num_days_from_sunday();
            // Days of the months before and after: faint, but a memo or a
            // schedule can be added on them like on any other day.
            let fade = if weeks == 0 && day.month() != self.month.month() {
                0.45
            } else {
                1.
            };
            if day == self.day {
                ui.painter().rect_filled(
                    rect.shrink(1.),
                    if slot == 0 { 7. } else { 0. },
                    tone.accent_soft.gamma_multiply(0.55),
                );
            }
            self.targets.push(dragging::Target {
                viewport: ui.ctx().viewport_id(),
                layer: ui.layer_id(),
                rect: rect.intersect(ui.clip_rect()),
                kind: dragging::Destination::Date(day),
            });
            let r = ui.interact(rect, ui.id().with(("day", day)), Sense::click_and_drag());
            if r.clicked() {
                self.day = day;
            }
            // Dragging over days marks a stretch for a several-day schedule.
            if r.drag_started() && self.stand_allows(crate::calendar_dock::StandFeature::Range) {
                self.range_drag = Some((ui.ctx().viewport_id(), day, day));
            }
            let viewport = ui.ctx().viewport_id();
            if let Some((owner, from, _)) = self.range_drag {
                if owner == viewport
                    && ui
                        .input(|i| i.pointer.interact_pos())
                        .is_some_and(|p| rect.intersect(ui.clip_rect()).contains(p))
                {
                    self.range_drag = Some((owner, from, day));
                }
            }
            if let Some((owner, a, b)) = self.range_drag {
                if owner == viewport && a != b && (a.min(b)..=a.max(b)).contains(&day) {
                    ui.painter()
                        .rect_filled(rect.shrink(2.), 4., ACCENT.gamma_multiply(0.18));
                }
            }
            let standing_move = self.cal.painting
                && self.cal.standing_pinned()
                && self.stand_feature_on(crate::calendar_dock::StandFeature::Move);
            if standing_move {
                if let Some(drag) = r.dnd_release_payload::<StandingScheduleDrag>() {
                    let note = &mut self.data.notes[drag.note];
                    if let Some(start) = note.date {
                        if let Some(next) = start.checked_add_signed(day - drag.from) {
                            if next != start {
                                note.move_start(next);
                                self.day = day;
                                self.toast = format!("{} → {day}", note.title);
                            }
                        }
                    }
                }
            }
            if r.dnd_hover_payload::<NoteDrag>().is_some()
                || (standing_move && r.dnd_hover_payload::<StandingScheduleDrag>().is_some())
            {
                ui.painter().rect_stroke(
                    rect.shrink(2.),
                    4.,
                    Stroke::new(2.0_f32, ACCENT),
                    egui::StrokeKind::Inside,
                );
            }
            // Holidays are worked out offline (lunar dates and substitutes).
            let holiday = crate::holidays::name(day);
            let datecolor = if holiday.is_some() || weekday == 0 {
                sun
            } else if weekday == 6 || crate::holidays::is_blue_friday(day) {
                sat
            } else {
                tone.ink
            }
            .gamma_multiply(fade);
            let number = rect.min + Vec2::new(17., 18.);
            if day == today {
                ui.painter().circle_filled(number, 11., today_mark);
            }
            ui.painter().text(
                number,
                egui::Align2::CENTER_CENTER,
                day.day().to_string(),
                egui::FontId::proportional(12.5),
                if day == today {
                    Color32::WHITE
                } else {
                    datecolor
                },
            );
            if let Some(name) = &holiday {
                // The name beside the day number, cut to the cell.
                let left = number.x + 15.;
                let room = rect.right() - left - 6.;
                if room > 14. {
                    let galley = ui.painter().layout(
                        name.clone(),
                        egui::FontId::proportional(10.5),
                        sun.gamma_multiply(fade),
                        f32::INFINITY,
                    );
                    let clip = Rect::from_min_max(
                        egui::pos2(left, rect.top()),
                        egui::pos2(left + room, rect.top() + 30.),
                    );
                    ui.painter()
                        .with_clip_rect(clip.intersect(ui.clip_rect()))
                        .galley(
                            egui::pos2(left, number.y - galley.size().y / 2.),
                            galley,
                            sun,
                        );
                }
                ui.interact(
                    Rect::from_center_size(number, Vec2::splat(22.)),
                    ui.id().with(("holiday", day)),
                    Sense::hover(),
                )
                .on_hover_text(name.as_str());
            }
            // The first day of each lunar month, faint, in the top right.
            if let Some((month, leap)) = crate::holidays::lunar_month_start(day) {
                ui.painter().text(
                    rect.right_top() + Vec2::new(-8., 9.),
                    egui::Align2::RIGHT_TOP,
                    format!("{} {month}.1", if leap { "윤" } else { "음" }),
                    egui::FontId::proportional(10.),
                    tone.faint.gamma_multiply(0.75),
                );
            }
            if ui.rect_contains_pointer(rect)
                && self.stand_allows(crate::calendar_dock::StandFeature::NewMemo)
            {
                let plus =
                    Rect::from_min_size(rect.right_top() + Vec2::new(-26., 5.), Vec2::splat(22.));
                // Solid under the + so it stands out over the lunar date.
                ui.painter().rect(
                    plus,
                    6.,
                    tone.bg,
                    Stroke::new(1.2_f32, ACCENT),
                    egui::StrokeKind::Inside,
                );
                overlay(ui, plus, slot + 25000, |ui| {
                    if icons::small_icon(ui, icons::Icon::Plus, "이 날짜에 새 스티커").clicked()
                    {
                        self.new_sticker_on(day);
                        self.day = day;
                    }
                });
            }
        }
        if self.cal.painting
            && self.cal.standing_pinned()
            && ui.input(|i| i.pointer.primary_released())
        {
            let ctx = ui.ctx();
            let outside = crate::system::cursor_pos()
                .is_some_and(|(x, y)| dragging::outside_app(ctx, egui::pos2(x as f32, y as f32)));
            if outside {
                if let Some(drag) = egui::DragAndDrop::take_payload::<StandingScheduleDrag>(ctx) {
                    self.unschedule(drag.note);
                }
            }
        }
        self.calendar_bands(ui, all, first, rows, col, height);
        // Let go after dragging over several days: a new sticker for them.
        if let Some((owner, a, b)) = self.range_drag {
            if owner == ui.ctx().viewport_id() && !ui.input(|i| i.pointer.primary_down()) {
                self.range_drag = None;
                if a != b {
                    let (start, end) = (a.min(b), a.max(b));
                    let i = self.new_sticker_on(start);
                    let n = &mut self.data.notes[i];
                    n.duration = ((end - start).num_days() + 1).min(u16::MAX as i64) as u16;
                    self.day = start;
                    self.toast = format!(
                        "{} ~ {} 일정을 만들었어요",
                        start.format("%-m월 %-d일"),
                        end.format("%-m월 %-d일")
                    );
                }
            }
        }
        // The main window's grid or the calendar window's: drop targets
        // the tutorial points at.
        let spot = if side.is_some() {
            Some(crate::tutorial::Spot::Grid)
        } else if self.cal.painting {
            Some(crate::tutorial::Spot::DockGrid)
        } else {
            None
        };
        if let Some(spot) = spot {
            crate::tutorial::mark(ui.ctx(), spot, ui.min_rect().intersect(ui.clip_rect()));
        }
        toggled
    }
    /// The main window's calendar. `work` is 내 작업실's panel, else the
    /// 캘린더 page. With the side-by-side toggle on, the selected day's
    /// schedule sits to the right of the grid behind a draggable divider;
    /// off, it sits below the grid as before.
    pub fn calendar_main(&mut self, ui: &mut egui::Ui, work: bool) {
        let side = if work {
            self.prefs.cal_side_work
        } else {
            self.prefs.cal_side_page
        };
        let toggled = if side {
            self.calendar_side_by_side(ui, work)
        } else {
            egui::ScrollArea::vertical()
                .id_salt(("calendar_scroll", work))
                .drag_to_scroll(false)
                .show(ui, |ui| {
                    let t = self.calendar_grid_with(ui, Some(false));
                    self.calendar_day_detail(ui);
                    t
                })
                .inner
        };
        if toggled {
            let flag = if work {
                &mut self.prefs.cal_side_work
            } else {
                &mut self.prefs.cal_side_page
            };
            *flag = !*flag;
        }
    }
    /// Grid on the left, the day's schedule on the right, a splitter between.
    fn calendar_side_by_side(&mut self, ui: &mut egui::Ui, work: bool) -> bool {
        let rect = ui.available_rect_before_wrap();
        let (height, total) = (rect.height(), rect.width());
        let handle_w = 12.;
        let min_right = 220.0_f32.min((total - handle_w) * 0.5);
        let max_right = (total - handle_w - 320.).max(min_right);
        let page = usize::from(work);
        let saved = self.prefs.main_widths.calendar_detail[page];
        let width = if saved > 0. { saved } else { total * 0.38 };
        let right_w = width.clamp(min_right, max_right);
        let left_w = (total - handle_w - right_w).max(240.);
        let left_rect = Rect::from_min_size(rect.min, Vec2::new(left_w, height));
        let handle = Rect::from_min_max(
            egui::pos2(left_rect.right(), rect.top()),
            egui::pos2(left_rect.right() + handle_w, rect.bottom()),
        );
        let right_rect = Rect::from_min_size(
            egui::pos2(handle.right(), rect.top()),
            Vec2::new((total - left_w - handle_w).max(40.), height),
        );
        let mut toggled = false;
        let mut left = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(("main_cal_grid_pane", work))
                .max_rect(left_rect)
                .layout(egui::Layout::top_down(egui::Align::LEFT)),
        );
        left.set_clip_rect(ui.clip_rect().intersect(left_rect));
        egui::ScrollArea::vertical()
            .id_salt(("main_cal_grid_scroll", work))
            .drag_to_scroll(false)
            .show(&mut left, |ui| {
                toggled = self.calendar_grid_with(ui, Some(true))
            });
        let drag = ui
            .interact(
                handle,
                ui.id().with(("main_cal_splitter", work)),
                Sense::drag(),
            )
            .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
        if drag.hovered() || drag.dragged() {
            ui.painter().line_segment(
                [handle.center_top(), handle.center_bottom()],
                egui::Stroke::new(2.0_f32, ui::ACCENT.gamma_multiply(0.6)),
            );
        }
        if drag.dragged() {
            let new_right = (right_w - drag.drag_delta().x).clamp(min_right, max_right);
            self.prefs.main_widths.calendar_detail[page] = new_right;
        }
        let mut right = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(("main_cal_detail_pane", work))
                .max_rect(right_rect)
                .layout(egui::Layout::top_down(egui::Align::LEFT)),
        );
        right.set_clip_rect(ui.clip_rect().intersect(right_rect));
        egui::ScrollArea::vertical()
            .id_salt(("main_cal_detail_scroll", work))
            .show(&mut right, |ui| self.calendar_day_detail(ui));
        ui.allocate_rect(rect, Sense::hover());
        toggled
    }
    /// The selected day's title bar and its list of memos.
    pub fn calendar_day_detail(&mut self, ui: &mut egui::Ui) {
        let drop_area = ui.available_rect_before_wrap().intersect(ui.clip_rect());
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!(
                    "{} {}",
                    self.day.format("%-m월 %-d일"),
                    ui::weekday(self.day)
                ))
                .strong()
                .size(20.),
            );
            let new_memo = self.stand_allows(crate::calendar_dock::StandFeature::NewMemo);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if new_memo && ui.button("+ 이 날짜에 스티커").clicked() {
                    self.new_sticker_on(self.day);
                }
            });
        });
        ui.add_space(4.);
        ui::divider(ui);
        let mut cache = std::mem::take(&mut self.lists);
        let key = crate::lists::key((self.day, self.data.notes.len()));
        let ids = cache.get("calendar_day", key, || {
            let mut ids = self
                .data
                .notes
                .iter()
                .enumerate()
                .filter(|(i, n)| {
                    self.visible(*i)
                        && !self.is_guide(*i)
                        && !n.deadline
                        && self.calendar_matches(n, self.day)
                })
                .map(|(i, _)| i)
                .collect::<Vec<_>>();
            ids.sort_by_key(|&i| (self.data.notes[i].time, i));
            ids
        });
        self.lists = cache;
        let ids = (*ids).clone();
        if ids.is_empty() {
            ui.add_space(10.);
            ui::hint(ui, "이 날짜에 표시할 메모가 없습니다");
        }
        for i in ids {
            self.task_row_on(ui, i, Some(self.day));
        }
        ui.add_space(16.);
        let drop_rect = Rect::from_min_max(
            drop_area.min,
            egui::pos2(drop_area.right(), ui.cursor().top().min(drop_area.bottom())),
        );
        self.targets.push(dragging::Target {
            viewport: ui.ctx().viewport_id(),
            layer: ui.layer_id(),
            rect: drop_rect,
            kind: dragging::Destination::DayList(self.day),
        });
        let origin = ui.ctx().input(|i| {
            i.viewport()
                .inner_rect
                .map_or(Vec2::ZERO, |r| r.min.to_vec2())
        });
        self.day_cells.insert(
            ui.ctx().viewport_id(),
            (
                ui.input(|i| i.time),
                egui::emath::TSTransform::from_scaling(ui.ctx().pixels_per_point())
                    * drop_rect.translate(origin),
                self.day,
            ),
        );
        self.schedule_origins.insert(
            ui.ctx().viewport_id(),
            (origin * ui.ctx().pixels_per_point()).to_pos2(),
        );
        ui.label(RichText::new("🚩 이날까지 업무").strong().size(16.));
        ui::divider(ui);
        let mut cache = std::mem::take(&mut self.lists);
        let deadlines = cache.get("calendar_deadlines", key, || {
            let mut ids = self
                .data
                .notes
                .iter()
                .enumerate()
                .filter(|(i, n)| self.visible(*i) && n.deadline_date().is_some() && !n.done)
                .map(|(i, _)| i)
                .collect::<Vec<_>>();
            ids.sort_by_key(|&i| {
                (
                    self.data.notes[i].deadline_date(),
                    self.data.notes[i].time,
                    i,
                )
            });
            ids
        });
        self.lists = cache;
        for &i in deadlines.iter() {
            self.task_row_on(ui, i, self.data.notes[i].deadline_date());
        }
    }
    /// Memo bands of the month grid. A memo shown on several days in a row
    /// is one band across those days (split only at week rows), placed in
    /// lanes so bands never overlap.
    fn calendar_bands(
        &mut self,
        ui: &mut egui::Ui,
        all: Rect,
        first: NaiveDate,
        rows: usize,
        col: f32,
        height: f32,
    ) {
        let (tone, ..) = self.calendar_tone(ui);
        // Remember where each date is on the screen: a sticker carried here
        // and let go takes the date under the pointer.
        {
            let ctx = ui.ctx();
            let origin = ctx
                .input(|i| i.viewport().inner_rect)
                .map(|r| r.min.to_vec2())
                .unwrap_or_default();
            let clip = ui.clip_rect();
            let scale = egui::emath::TSTransform::from_scaling(ctx.pixels_per_point());
            let mut cells = Vec::with_capacity(rows * 7);
            for row in 0..rows {
                for c in 0..7 {
                    let r = Rect::from_min_size(
                        all.min + Vec2::new(c as f32 * col, row as f32 * height),
                        Vec2::new(col, height),
                    )
                    .intersect(clip);
                    if r.is_positive() {
                        cells.push((
                            scale * r.translate(origin),
                            first + Duration::days((row * 7 + c) as i64),
                        ));
                    }
                }
            }
            let now = ctx.input(|i| i.time);
            self.cal_cells.insert(ctx.viewport_id(), (now, cells));
            self.schedule_origins.insert(
                ctx.viewport_id(),
                (origin * ctx.pixels_per_point()).to_pos2(),
            );
        }
        let hide_done = self.prefs.cal_hide_done;
        let band = (self.prefs.cal_font + 8.5).round();
        let step = band + 2.;
        let capacity = (((height - 36.) / step).floor() as usize).max(1);
        // Bands and lanes of every week row, reused until something changes.
        let mut cache = std::mem::take(&mut self.lists);
        let key = crate::lists::key((first, rows, hide_done, self.data.notes.len()));
        let weeks = cache.get("calendar_bands", key, || {
            (0..rows)
                .map(|row| {
                    let start = first + Duration::days((row * 7) as i64);
                    let days: Vec<NaiveDate> = (0..7).map(|c| start + Duration::days(c)).collect();
                    // Bands run over the days of the next and the last month
                    // too: those days take new memos as well.
                    let mut segments = band_segments(
                        &self.data.notes,
                        |i| self.visible(i) && !self.is_guide(i),
                        |n, d| self.calendar_matches(n, d) && !(hide_done && n.done_on(Some(d))),
                        &days,
                        |_| true,
                    );
                    segments.sort_by_key(|s| {
                        (
                            s.first,
                            std::cmp::Reverse(s.last - s.first),
                            self.data.notes[s.note].time,
                        )
                    });
                    let lanes = assign_lanes(&segments);
                    (segments, lanes)
                })
                .collect::<Vec<_>>()
        });
        self.lists = cache;
        for (row, (segments, lanes)) in weeks.iter().enumerate() {
            let start = first + Duration::days((row * 7) as i64);
            let days: Vec<NaiveDate> = (0..7).map(|c| start + Duration::days(c)).collect();
            let mut covering = [0usize; 7];
            for s in segments {
                for c in &mut covering[s.first..=s.last] {
                    *c += 1;
                }
            }
            let visible_lanes: Vec<usize> = covering
                .iter()
                .map(|&n| if n > capacity { capacity - 1 } else { capacity })
                .collect();
            let row_top = all.top() + row as f32 * height;
            for (s, &lane) in segments.iter().zip(lanes) {
                // Draw the parts of the band whose days have room for its lane.
                let mut c = s.first;
                while c <= s.last {
                    if lane >= visible_lanes[c] {
                        c += 1;
                        continue;
                    }
                    let from = c;
                    while c < s.last && lane < visible_lanes[c + 1] {
                        c += 1;
                    }
                    let open_left = from == s.first && s.starts_here;
                    let open_right = c == s.last && s.ends_here;
                    let rect = Rect::from_min_max(
                        egui::pos2(
                            all.left() + from as f32 * col + if open_left { 5. } else { 0. },
                            row_top + 33. + lane as f32 * step,
                        ),
                        egui::pos2(
                            all.left() + (c + 1) as f32 * col - if open_right { 5. } else { 0. },
                            row_top + 33. + lane as f32 * step + band,
                        ),
                    );
                    self.calendar_band(
                        ui,
                        &tone,
                        s.note,
                        rect,
                        &days[from..=c],
                        open_left,
                        open_right,
                    );
                    c += 1;
                }
            }
            for (c, &n) in covering.iter().enumerate() {
                if n > capacity {
                    let shown = capacity - 1;
                    ui.painter().text(
                        egui::pos2(
                            all.left() + c as f32 * col + 9.,
                            row_top + 33. + shown as f32 * step + band / 2.,
                        ),
                        egui::Align2::LEFT_CENTER,
                        format!("+{}개 더보기", n - shown),
                        egui::FontId::proportional(11.),
                        tone.muted,
                    );
                }
            }
        }
        self.band_resize_step(ui, all, first, rows, col, height);
        self.paint_sticker_schedule_preview(ui);
    }

    fn paint_sticker_schedule_preview(&self, ui: &egui::Ui) {
        let ctx = ui.ctx();
        let native = self.studio.drag.as_ref().and_then(|drag| {
            let carry = drag.carry.as_ref()?;
            let (hit, pointer) = carry.preview.zip(carry.pointer)?;
            Some((drag.note, drag.shift, hit, pointer))
        });
        let list = || {
            let note = self.drag.as_ref()?.list_preview()?;
            let (x, y) = crate::system::cursor_pos()?;
            let pointer = egui::pos2(x as f32, y as f32);
            Some((
                note,
                ui.input(|i| i.modifiers.shift),
                self.sticker_preview_at(pointer, ui.input(|i| i.time))?,
                pointer,
            ))
        };
        let Some((note_index, shift, hit, pointer)) = native.or_else(list) else {
            return;
        };
        if hit.viewport != ctx.viewport_id() {
            return;
        }
        let Some((_, cells)) = self.cal_cells.get(&hit.viewport) else {
            return;
        };
        let Some((cell, _)) = cells
            .iter()
            .find(|(r, date)| *date == hit.date && r.contains(pointer))
        else {
            return;
        };
        let ppp = ctx.pixels_per_point();
        let origin = ui.input(|i| {
            i.viewport()
                .inner_rect
                .map_or(Vec2::ZERO, |r| r.min.to_vec2())
        });
        let local = egui::emath::TSTransform::from_scaling(1. / ppp) * *cell;
        let cell = local.translate(-origin);
        let rect = dragged_sticker_bar(cell, pointer / ppp - origin, self.prefs.cal_font + 8.5);
        let Some(note) = self.data.notes.get(note_index) else {
            return;
        };
        ui.painter().rect_stroke(
            cell.shrink(2.),
            4.,
            Stroke::new(2.0_f32, ACCENT),
            egui::StrokeKind::Inside,
        );
        let title = if note.title.trim().is_empty() {
            "제목 없음"
        } else {
            &note.title
        };
        let title = if shift {
            format!("🚩 {title}")
        } else {
            title.to_owned()
        };
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            ui.id().with("sticker_calendar_preview"),
        ));
        painter.rect_filled(rect, 4., note.color);
        painter.rect_stroke(
            rect,
            4.,
            Stroke::new(1.0_f32, ACCENT),
            egui::StrokeKind::Inside,
        );
        let ink = if egui::Rgba::from(note.color).intensity() < 0.18 {
            Color32::WHITE
        } else {
            INK
        };
        if rect.width() <= 10. || rect.height() < self.prefs.cal_font {
            return;
        }
        let mut job = egui::text::LayoutJob::simple_singleline(
            title,
            egui::FontId::proportional(self.prefs.cal_font),
            ink,
        );
        job.wrap = egui::text::TextWrapping::truncate_at_width(rect.width() - 10.);
        let galley = ui.fonts(|fonts| fonts.layout_job(job));
        painter
            .with_clip_rect(rect.intersect(ctx.screen_rect()))
            .galley(
                egui::pos2(rect.left() + 5., rect.center().y - galley.size().y * 0.5),
                galley,
                ink,
            );
    }
    /// A band's end held and moved: the memo covers the days from the end
    /// that stays put to the day under the pointer, shown as it moves.
    fn band_resize_step(
        &mut self,
        ui: &egui::Ui,
        all: Rect,
        first: NaiveDate,
        rows: usize,
        col: f32,
        height: f32,
    ) {
        let Some((viewport, i, anchor, before)) = self.band_resize else {
            return;
        };
        if viewport != ui.ctx().viewport_id() {
            return;
        }
        // An end is being moved, not the memo.
        self.drag = None;
        if !ui.input(|v| v.pointer.primary_down()) {
            self.band_resize = None;
            let n = &self.data.notes[i];
            if let Some((a, b)) = n.span().filter(|&s| Some(s) != before) {
                self.toast = if a == b {
                    format!("{} 당일 일정으로 바꿨어요", a.format("%-m월 %-d일"))
                } else {
                    format!(
                        "{} ~ {} 기간 일정으로 바꿨어요",
                        a.format("%-m월 %-d일"),
                        b.format("%-m월 %-d일")
                    )
                };
            }
            return;
        }
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        let Some(p) = ui.input(|v| v.pointer.interact_pos()) else {
            return;
        };
        let day = grid_day(p, all, first, rows, col, height);
        if self.data.notes[i].span() != Some((anchor.min(day), anchor.max(day))) {
            self.data.notes[i].set_days(anchor, day);
            // Pointer moves are not counted as input: have the lists redrawn.
            crate::lists::CHANGES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn calendar_band(
        &mut self,
        ui: &mut egui::Ui,
        tone: &ui::Tone,
        i: usize,
        rect: Rect,
        days: &[NaiveDate],
        open_left: bool,
        open_right: bool,
    ) {
        let band_on = self.stand_allows(crate::calendar_dock::StandFeature::Band);
        let standing = self.cal.painting && self.cal.standing_pinned();
        let n = &self.data.notes[i];
        let standing_move = standing
            && self.stand_feature_on(crate::calendar_dock::StandFeature::Move)
            && self.desktop.calendar_basis == 1
            && n.date.is_some();
        let band_drag = if standing { standing_move } else { band_on };
        let done = days.iter().all(|&d| n.done_on(Some(d)));
        let fill = if self.dark {
            Color32::from_rgb(52, 50, 46)
        } else {
            n.color
        };
        let response = ui
            .interact(
                rect,
                ui.id().with(("band", i, days[0])),
                match (band_on, band_drag) {
                    (true, true) => Sense::click_and_drag(),
                    (true, false) => Sense::click(),
                    (false, true) => Sense::drag(),
                    (false, false) => Sense::hover(),
                },
            )
            .on_hover_text(&n.title);
        let response = if band_on {
            response.on_hover_cursor(egui::CursorIcon::PointingHand)
        } else {
            response
        };
        let r = |open: bool| if open { 4 } else { 0 };
        let corners = egui::CornerRadius {
            nw: r(open_left),
            sw: r(open_left),
            ne: r(open_right),
            se: r(open_right),
        };
        let label = if n.title.trim().is_empty() {
            "제목 없음".to_owned()
        } else {
            n.title.clone()
        };
        let label = if n.deadline {
            format!("🚩 {label}")
        } else {
            label
        };
        let label = match crate::model::check_progress(&n.body) {
            Some((done, all)) => format!("{label} {done}/{all}"),
            None => label,
        };
        let mut job = egui::text::LayoutJob::single_section(
            label,
            egui::TextFormat {
                font_id: egui::FontId::proportional(self.prefs.cal_font),
                color: if done {
                    tone.faint
                } else if self.dark {
                    tone.ink
                } else {
                    INK
                },
                strikethrough: if done && self.prefs.cal_strike {
                    Stroke::new(1.0_f32, tone.faint)
                } else {
                    Stroke::NONE
                },
                ..Default::default()
            },
        );
        job.wrap = egui::text::TextWrapping::truncate_at_width(rect.width() - 12.);
        let galley = ui.fonts(|f| f.layout_job(job));
        let day_at = |p: egui::Pos2| {
            let k = ((p.x - rect.left()) / (rect.width() / days.len() as f32)).floor();
            days[(k.max(0.) as usize).min(days.len() - 1)]
        };
        if standing_move && response.drag_started_by(egui::PointerButton::Primary) {
            if let Some(press) = ui.input(|input| input.pointer.press_origin()) {
                egui::DragAndDrop::set_payload(
                    ui.ctx(),
                    StandingScheduleDrag {
                        note: i,
                        from: day_at(press),
                    },
                );
            }
        }
        let moving = if standing {
            egui::DragAndDrop::has_payload_of_type::<StandingScheduleDrag>(ui.ctx())
        } else {
            self.drag
                .as_ref()
                .is_some_and(|drag| drag.moving_note_in(ui.ctx().viewport_id(), i))
        };
        let movement = if moving && response.dragged_by(egui::PointerButton::Primary) {
            ui.input(|input| {
                let from = input.pointer.press_origin()?;
                let at = input.pointer.interact_pos()?;
                ((at - from).length_sq() > 16.).then_some(at - from)
            })
        } else {
            None
        };
        let border = (!self.dark && pale(fill))
            .then(|| Stroke::new(1.0_f32, ui::lerp_color(fill, Color32::BLACK, 0.42)));
        let paint = |painter: &egui::Painter, rect: Rect, fill| {
            painter.rect_filled(rect, corners, fill);
            if let Some(border) = border {
                painter.rect_stroke(rect, corners, border, egui::StrokeKind::Inside);
            }
            painter.galley(
                egui::pos2(rect.left() + 6., rect.center().y - galley.size().y / 2.),
                galley.clone(),
                INK,
            );
        };
        if let Some(delta) = movement {
            let mut source = ui.painter().clone();
            source.multiply_opacity(0.35);
            paint(&source, rect, fill);
            let floating = ui.ctx().layer_painter(egui::LayerId::new(
                egui::Order::Tooltip,
                ui.id().with("moving_calendar_band"),
            ));
            paint(&floating, rect.translate(delta), fill);
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        } else {
            let fill = if response.hovered() {
                ui::lerp_color(fill, Color32::BLACK, 0.06)
            } else {
                fill
            };
            paint(ui.painter(), rect, fill);
        }
        if !band_on {
            return;
        }
        // The day under the pointer is the occurrence a click or drag uses.
        let day = response
            .interact_pointer_pos()
            .or(response.hover_pos())
            .map(day_at)
            .unwrap_or(days[0]);
        let occurrence = if self.desktop.calendar_basis == 1 {
            Some(day)
        } else {
            None
        };
        if !standing {
            response.dnd_set_drag_payload(NoteDrag);
            let grips = self.band_edges(ui, i, rect, open_left, open_right);
            self.drag_source_on(ui, &response, i, occurrence);
            if let Some(s) = self.sources.last_mut() {
                // The grips at the ends resize; only the middle carries the memo.
                s.rect = grips.intersect(ui.clip_rect());
                s.calendar_band = true;
            }
        }
        if response.clicked() {
            self.open_sticker(i, occurrence);
        }
        self.band_menu(&response, i, occurrence);
    }
    /// '일정 제외': the memo stays (with its content and alarm), only its
    /// schedule goes, so it leaves the calendar's schedule view.
    pub(crate) fn unschedule(&mut self, i: usize) {
        self.data.notes[i].clear_schedule();
        if let Some(w) = self.studio.windows.get_mut(&i) {
            w.schedule_start = None;
            w.occurrence = None;
        }
        self.toast = "메모는 두고 일정만 뺐습니다".into();
    }
    /// Right click on a memo in the calendar grid.
    fn band_menu(&mut self, response: &egui::Response, i: usize, occurrence: Option<NaiveDate>) {
        crate::float::context_menu(response, 180., |ui| {
            if ui.button("스티커로 열기").clicked() {
                self.open_sticker(i, occurrence);
                ui.close_menu();
            }
            if ui.button("편집창 열기").clicked() {
                self.open_editor_on(i, occurrence);
                ui.close_menu();
            }
            // Only a memo with a schedule (not one shown by its creation
            // or alarm date) has one to take out.
            if self.data.notes[i].date.is_some() && ui.button("일정 제외").clicked() {
                self.unschedule(i);
                ui.close_menu();
            }
            ui.separator();
            if ui.button("메모 삭제 · 휴지통").clicked() {
                self.trash_note(i);
                ui.close_menu();
            }
        });
    }
    /// The two ends of a band (where the memo's days start and stop): the
    /// pointer turns into a left-right arrow there, and dragging one moves
    /// that end to another day. Only for schedules that do not repeat.
    fn band_edges(
        &mut self,
        ui: &mut egui::Ui,
        i: usize,
        rect: Rect,
        open_left: bool,
        open_right: bool,
    ) -> Rect {
        let n = &self.data.notes[i];
        if self.desktop.calendar_basis != 1 || !n.repeat.is_none() {
            return rect;
        }
        let Some((start, end)) = n.span() else {
            return rect;
        };
        let grip = 7_f32.min(rect.width() / 3.);
        let mut middle = rect;
        if open_left {
            middle.min.x += grip;
        }
        if open_right {
            middle.max.x -= grip;
        }
        for (open, left) in [(open_left, true), (open_right, false)] {
            if !open {
                continue;
            }
            let edge = if left {
                Rect::from_min_max(rect.min, egui::pos2(rect.left() + grip, rect.bottom()))
            } else {
                Rect::from_min_max(egui::pos2(rect.right() - grip, rect.top()), rect.max)
            };
            let r = ui
                .interact(edge, ui.id().with(("band_edge", i, left)), Sense::drag())
                .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
            if r.hovered() || r.dragged() {
                let x = if left {
                    edge.left() + 2.
                } else {
                    edge.right() - 2.
                };
                ui.painter().line_segment(
                    [
                        egui::pos2(x, rect.top() + 3.),
                        egui::pos2(x, rect.bottom() - 3.),
                    ],
                    Stroke::new(2_f32, INK.gamma_multiply(0.45)),
                );
            }
            if r.drag_started() {
                let anchor = if left { end } else { start };
                self.band_resize = Some((ui.ctx().viewport_id(), i, anchor, Some((start, end))));
            }
        }
        middle
    }
    /// Colors of the month grid: the app's own, or the calendar window's
    /// theme. Returns the tone and the Sunday, Saturday and today marks.
    fn calendar_tone(&self, ui: &egui::Ui) -> (ui::Tone, Color32, Color32, Color32) {
        let mut tone = ui::t(ui);
        match self.cal.active() {
            Some(t) => {
                tone.ink = t.ink;
                tone.muted = t.muted;
                tone.faint = t.faint;
                tone.line = t.line;
                tone.accent_soft = t.select;
                (tone, t.sun, t.sat, t.today)
            }
            None => (tone, ACCENT, ui::BLUE, ACCENT),
        }
    }
    pub(crate) fn calendar_matches(&self, n: &Note, day: NaiveDate) -> bool {
        match self.desktop.calendar_basis {
            0 => n.created.date() == day,
            2 => n.reminder.is_some_and(|t| t.date() == day),
            _ => n.occurs(day),
        }
    }
    pub fn note_editor(&mut self, ui: &mut egui::Ui) {
        let i = self.active;
        ui.horizontal(|ui| {
            self.note_options(ui, i);
            let n = &self.data.notes[i];
            let today = Local::now().date_naive();
            let occurrence = self.extra.edit_occurrence.or_else(|| {
                if n.occurs(today) {
                    Some(today)
                } else {
                    n.date
                }
            });
            let mut done = n.done_on(occurrence);
            if ui
                .add_sized([24., 32.], |ui: &mut egui::Ui| {
                    ui::round_check(ui, &mut done, None)
                })
                .on_hover_text("완료")
                .changed()
            {
                self.done_at(i, occurrence, done);
                self.extra.scope_owner = ui.ctx().viewport_id();
            }
            let width = (ui.available_width() - 34.).max(60.);
            ui.add_sized(
                [width, 34.],
                egui::TextEdit::singleline(&mut self.data.notes[i].title)
                    .font(egui::FontId::proportional(20.))
                    .frame(false)
                    .hint_text("제목"),
            );
            let starred = self.data.notes[i].starred;
            if icons::toggle_icon(
                ui,
                icons::Icon::Star,
                if starred {
                    "중요 표시 해제"
                } else {
                    "중요 표시"
                },
                starred,
            )
            .clicked()
            {
                self.data.notes[i].starred = !starred;
            }
        });
        ui.add_space(6.);
        self.date_controls(ui, i);
    }
    pub fn journal(&mut self, ui: &mut egui::Ui) {
        self.journal_v2(ui);
    }
    pub fn collections(&mut self, ui: &mut egui::Ui) {
        match self.page {
            7 => {
                ui.heading("표 모아보기");
                egui::Grid::new("table").striped(true).show(ui, |ui| {
                    for row in [
                        ["작업", "상태", "일정"],
                        ["화면 설계", "진행 중", "오늘"],
                        ["프로토타입 검토", "할 일", "내일"],
                    ] {
                        for c in row {
                            ui.label(c);
                        }
                        ui.end_row();
                    }
                });
            }
            8 => {
                ui.heading("이미지 모아보기");
                ui.horizontal_wrapped(|ui| {
                    for (c, t) in [PALETTE[0], PALETTE[1], PALETTE[2]]
                        .iter()
                        .zip(["영감", "기록", "생각"])
                    {
                        egui::Frame::NONE.fill(*c).inner_margin(35.).show(ui, |ui| {
                            ui.set_min_size(Vec2::new(150., 100.));
                            ui.label(RichText::new(t).color(Color32::WHITE));
                        });
                    }
                });
            }
            _ => {
                ui.heading("링크 · 즐겨찾기");
                for t in ["프로젝트 참고 자료", "사내 업무 포털", "디자인 아이디어"]
                {
                    if ui.button(format!("↗ {t}")).clicked() {
                        self.toast = "링크 실행은 후속 기능입니다".into();
                    }
                }
            }
        }
    }
}
impl App {
    pub fn board(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui::segmented(ui, &mut self.extra.board_tab, &["관계", "트리"]);
            if self.extra.board_tab == 0 {
                ui.add_space(6.);
                ui::segmented(ui, &mut self.board_filter, &["모든 메모", "선택한 메모"]);
                if ui::pill(ui, self.extra.created_basis, "생성시간 표시").clicked() {
                    self.extra.created_basis = !self.extra.created_basis;
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if self.extra.board_tab == 0 {
                    if ui.button("+ 메모").clicked() {
                        self.add_note(None);
                    }
                    if ui.button("자동 정렬").clicked() {
                        self.auto_layout();
                    }
                    if ui
                        .add(
                            egui::Slider::new(&mut self.extra.zoom, 0.4..=1.8)
                                .show_value(false)
                                .text("배율"),
                        )
                        .on_hover_text("Ctrl + 휠로도 확대/축소")
                        .changed()
                    {
                        let center = self.extra.scene.center();
                        self.extra.scene =
                            Rect::from_center_size(center, self.extra.scene_size / self.extra.zoom);
                    }
                }
            });
        });
        if self.extra.board_tab == 1 {
            ui.add_space(10.);
            self.relation_tree(ui);
            return;
        }
        ui.add_space(6.);
        self.picked
            .retain(|&i| self.data.notes.get(i).is_some_and(|n| !n.deleted));
        if self
            .link_source
            .is_some_and(|i| i >= self.data.notes.len() || !self.visible(i))
        {
            self.link_source = None;
        }
        ui.horizontal_wrapped(|ui| {
            let previous = self.board_relation_kind;
            ui::segmented(
                ui,
                &mut self.board_relation_kind,
                &["관련 메모 연결", "상위 → 하위 연결"],
            );
            if previous != self.board_relation_kind {
                self.link_source = None;
            }
            if let Some(source) = self.link_source {
                ui.label(format!(
                    "{} → 대상 카드를 선택하세요",
                    shorten(&self.data.notes[source].title, 20)
                ));
                if ui.button("취소 · Esc").clicked() {
                    self.link_source = None;
                }
            } else {
                ui.label(if self.board_relation_kind == 1 {
                    "상위 카드의 ‘상위로 지정’을 누르세요"
                } else {
                    "카드의 ‘연결 시작’을 누르세요"
                });
            }
        });
        if !self.picked.is_empty() {
            ui.horizontal_wrapped(|ui| {
                let tone = ui::t(ui);
                ui.label(
                    RichText::new(format!("{}개 선택", self.picked.len())).color(
                        if self.picked.is_empty() {
                            tone.faint
                        } else {
                            ACCENT
                        },
                    ),
                );
                let enabled = !self.picked.is_empty();
                ui.add_enabled_ui(enabled, |ui| {
                    if ui
                        .add_enabled(
                            self.picked.len() == 2,
                            egui::Button::new("선택한 두 메모 연결"),
                        )
                        .on_hover_text("방향 없는 관련 메모 관계")
                        .clicked()
                    {
                        if self.picked.len() == 2 {
                            self.data.connect(self.picked[0], self.picked[1]);
                        } else {
                            self.toast = "연결할 메모 2개를 선택하세요".into();
                        }
                    }
                    if ui
                        .add_enabled(self.picked.len() == 2, egui::Button::new("연결 해제"))
                        .clicked()
                        && self.picked.len() == 2
                    {
                        let (a, b) = (self.picked[0], self.picked[1]);
                        self.data.links.retain(|e| *e != (a.min(b), a.max(b)));
                    }
                    if ui
                        .add_enabled(
                            self.picked.len() >= 2,
                            egui::Button::new("선택한 상위 아래로 연결"),
                        )
                        .on_hover_text("처음 선택한 메모 아래로 나머지를 묶습니다")
                        .clicked()
                        && self.picked.len() >= 2
                    {
                        let p = self.picked[0];
                        for c in self.picked[1..].iter().copied() {
                            if !self.data.reparent(c, p) {
                                self.toast = "자기 자신 또는 순환 관계는 만들 수 없습니다".into();
                            }
                        }
                    }
                    if ui.button("종속 해제").clicked() {
                        for &i in &self.picked {
                            self.data.notes[i].parents.clear();
                        }
                    }
                    ui.add(
                        egui::TextEdit::singleline(&mut self.project_name)
                            .desired_width(120.)
                            .hint_text("프로젝트 이름"),
                    );
                    if ui.button("프로젝트로 묶기").clicked() {
                        if self.picked.is_empty() || self.project_name.trim().is_empty() {
                            self.toast = "메모와 프로젝트 이름을 선택하세요".into();
                        } else {
                            let p = self
                                .data
                                .projects
                                .iter()
                                .position(|s| s == self.project_name.trim())
                                .unwrap_or_else(|| {
                                    self.data.projects.push(self.project_name.trim().into());
                                    self.data.projects.len() - 1
                                });
                            for &i in &self.picked {
                                self.data.notes[i].project = Some(p);
                            }
                        }
                    }
                    if ui.button("선택 해제").clicked() {
                        self.picked.clear();
                        self.link_source = None;
                    }
                });
            });
        }
        if let Some(&first) = self.picked.first().filter(|_| self.picked.len() >= 2) {
            ui::hint(
                ui,
                format!("선택한 상위 메모 · {}", self.data.notes[first].title),
            );
        }
        ui::hint(
            ui,
            "제목을 끌어 카드 이동 · ≡ 를 상위 카드로 끌어 하위로 연결 · 화살표는 상위 → 하위",
        );
        ui.add_space(8.);
        self.extra.scene_size = ui.available_size();
        if !self.extra.scene_initialized {
            self.extra.scene = Rect::from_min_size(egui::Pos2::ZERO, self.extra.scene_size);
            self.extra.scene_initialized = true;
        }
        let mut scene = self.extra.scene;
        egui::Scene::new()
            .zoom_range(0.4..=1.8)
            .max_inner_size([5000., 5000.])
            .show(ui, &mut scene, |ui| {
                let (canvas, _) = ui.allocate_exact_size(Vec2::new(1450., 950.), Sense::hover());
                let ids: Vec<_> = (0..self.data.notes.len())
                    .filter(|&i| {
                        self.visible(i)
                            && match self.board_filter {
                                1 => self.picked.contains(&i),
                                _ => true,
                            }
                    })
                    .collect();
                let size = Vec2::new(254., if self.extra.created_basis { 170. } else { 152. });
                let rects: Vec<_> = self
                    .data
                    .notes
                    .iter()
                    .map(|n| Rect::from_min_size(canvas.min + n.pos, size))
                    .collect();
                let mut shown = vec![false; self.data.notes.len()];
                for &i in &ids {
                    shown[i] = true;
                }
                for &(a, b) in &self.data.links {
                    if shown.get(a).copied().unwrap_or(false) && shown.get(b).copied().unwrap_or(false) {
                        edge(
                            ui.painter(),
                            rects[a],
                            rects[b],
                            Color32::from_rgb(161, 167, 153),
                            false,
                            self.extra.zoom,
                        );
                    }
                }
                for &i in &ids {
                    for &p in &self.data.notes[i].parents {
                        if shown.get(p).copied().unwrap_or(false) {
                            edge(ui.painter(), rects[p], rects[i],
                                Color32::from_rgb(133, 121, 177), true, self.extra.zoom);
                        }
                    }
                }
                for i in ids {
                    let rect = rects[i];
                    // Cards outside the view are not drawn.
                    if !ui.is_rect_visible(rect) {
                        continue;
                    }
                    let selected = self.picked.contains(&i) || self.link_source == Some(i);
                    let tone = ui::t(ui);
                    let fill = if self.dark {
                        Color32::from_rgb(39, 40, 38)
                    } else {
                        Color32::WHITE
                    };
                    ui.painter().rect(
                        rect,
                        10.,
                        fill,
                        Stroke::new(
                            if selected { 2.0_f32 } else { 1.0_f32 },
                            if selected { ACCENT } else { tone.line_strong },
                        ),
                        egui::StrokeKind::Inside,
                    );
                    ui.painter().rect_filled(
                        Rect::from_min_size(
                            rect.min + Vec2::new(12., 0.),
                            Vec2::new(size.x - 24., 3.),
                        ),
                        2.,
                        self.data.notes[i].color,
                    );
                    overlay(ui, rect.shrink2(Vec2::new(12., 12.)), i + 2000, |ui| {
                        ui.set_clip_rect(ui.clip_rect().intersect(rect));
                        ui.spacing_mut().item_spacing.y = 5.;
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 4.;
                            let mut s = self.picked.contains(&i);
                            if ui
                                .checkbox(&mut s, "")
                                .on_hover_text("여러 카드 선택")
                                .changed()
                            {
                                if s {
                                    self.picked.push(i)
                                } else {
                                    self.picked.retain(|&j| j != i)
                                }
                            }
                            let (handle, drag) =
                                ui.allocate_exact_size(Vec2::new(16., 22.), Sense::drag());
                            crate::icons::paint_icon(
                                ui.painter(),
                                handle.shrink2(Vec2::new(1., 4.)),
                                crate::icons::Icon::Menu,
                                tone.faint,
                            );
                            let drag = drag
                                .on_hover_text("다른 카드로 끌면 자식 메모가 됩니다")
                                .on_hover_cursor(egui::CursorIcon::Grab);
                            drag.dnd_set_drag_payload(NoteDrag);
                            self.drag_source(ui, &drag, i, false);
                            let title = shorten(&self.data.notes[i].title, 14);
                            let mut text = RichText::new(title).strong().size(13.5);
                            if self.data.notes[i].done {
                                text = text.strikethrough().color(tone.muted);
                            }
                            let grip = ui
                                .allocate_ui_with_layout(
                                    Vec2::new((ui.available_width() - 26.).max(40.), 22.),
                                    egui::Layout::left_to_right(egui::Align::Center),
                                    |ui| {
                                        ui.set_min_width(ui.available_width());
                                        ui.add(
                                            egui::Label::new(text)
                                                .truncate()
                                                .sense(Sense::click_and_drag()),
                                        )
                                    },
                                )
                                .inner;
                            self.drag_source(ui, &grip, i, true);
                            if grip.double_clicked() {
                                self.open_note(i);
                            }
                            let n = &mut self.data.notes[i];
                            let star = crate::icons::small_icon(
                                ui,
                                crate::icons::Icon::Star,
                                if n.starred {
                                    "중요 해제"
                                } else {
                                    "중요 표시"
                                },
                            );
                            if n.starred {
                                crate::icons::paint_icon(
                                    ui.painter(),
                                    star.rect.shrink(5.),
                                    crate::icons::Icon::Star,
                                    ACCENT,
                                );
                            }
                            if star.clicked() {
                                n.starred = !n.starred;
                            }
                        });
                        ui.add(
                            egui::Label::new(
                                RichText::new(if self.data.notes[i].locked {
                                    "잠긴 메모".into()
                                } else {
                                    shorten(&self.data.notes[i].body, 40)
                                })
                                .size(12.)
                                .color(tone.muted),
                            )
                            .wrap(),
                        );
                        let n = &self.data.notes[i];
                        let mut meta = vec![n
                            .project
                            .map(|p| format!("# {}", self.data.projects[p]))
                            .unwrap_or("프로젝트 없음".into())];
                        meta.push(match n.parents.first() {
                            Some(&p) => {
                                format!("상위 · {}", shorten(&self.data.notes[p].title, 10))
                            }
                            None => "독립 메모".into(),
                        });
                        if self.extra.created_basis {
                            meta.push(format!("생성 {}", n.created.format("%m-%d %H:%M")));
                        }
                        ui.label(
                            RichText::new(meta.join("  ·  "))
                                .size(11.)
                                .color(tone.faint),
                        );
                        ui.add_space(2.);
                        ui.horizontal(|ui| {
                            let mut d = self.data.notes[i].done;
                            if ui::round_check(ui, &mut d, None).changed() {
                                self.done(i, d);
                                self.extra.scope_owner = ui.ctx().viewport_id();
                            }
                            ui.label(RichText::new("완료").size(12.).color(tone.muted));
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let linking = self.link_source == Some(i);
                                    let label = if linking { "선택 취소" }
                                        else if self.link_source.is_some() {
                                            if self.board_relation_kind == 1 { "하위로 연결" } else { "이 메모와 연결" }
                                        } else if self.board_relation_kind == 1 { "상위로 지정" }
                                        else { "연결 시작" };
                                    if ui::pill(ui, linking, label).clicked() {
                                        if linking { self.link_source = None; }
                                        else if let Some(source) = self.link_source {
                                            if self.board_relation_kind == 0 {
                                                self.data.connect(source, i);
                                                self.link_source = None;
                                                self.toast = "관련 메모를 연결했습니다".into();
                                            } else if self.data.reparent(i, source) {
                                                self.link_source = None;
                                                self.toast = "상위 → 하위 관계를 연결했습니다".into();
                                            } else {
                                                self.toast = "순환 관계가 됩니다. 다른 하위 메모를 선택하세요".into();
                                            }
                                        } else { self.link_source = Some(i); }
                                    }
                                    if ui::pill(ui, false, "열기").clicked() {
                                        self.open_note(i);
                                    }
                                },
                            );
                        });
                    });
                    self.targets.push(dragging::Target {
                        viewport: ui.ctx().viewport_id(),
                        layer: ui.layer_id(),
                        rect: rect.intersect(ui.clip_rect()),
                        kind: dragging::Destination::Parent(i),
                    });
                    let _drop = ui.interact(rect, ui.id().with(("board_drop", i)), Sense::hover());
                }
            });
        self.extra.scene = scene;
        if let Some(t) = ui.ctx().layer_transform_to_global(egui::LayerId::new(
            ui.layer_id().order,
            ui.id().with("scene_area"),
        )) {
            self.extra.zoom = t.scaling;
        }
    }
}

fn group_header(ui: &mut egui::Ui, label: &str, count: usize) {
    let tone = ui::t(ui);
    ui.horizontal(|ui| {
        ui.add_space(4.);
        ui.label(RichText::new(label).strong().size(13.5));
        ui.label(RichText::new(count.to_string()).size(12.).color(tone.faint));
    });
    ui.add_space(2.);
    ui::divider(ui);
}
fn overlay(ui: &mut egui::Ui, rect: Rect, id: usize, f: impl FnOnce(&mut egui::Ui)) {
    let mut child = ui.new_child(egui::UiBuilder::new().id_salt(id).max_rect(rect));
    f(&mut child);
}
/// Attach to facing card borders. Overlapping cards have no visible gap.
fn edge_ports(a: Rect, b: Rect) -> Option<(egui::Pos2, egui::Pos2, Vec2)> {
    if a.right() < b.left() {
        Some((a.right_center(), b.left_center(), Vec2::X))
    } else if b.right() < a.left() {
        Some((a.left_center(), b.right_center(), -Vec2::X))
    } else if a.bottom() < b.top() {
        Some((a.center_bottom(), b.center_top(), Vec2::Y))
    } else if b.bottom() < a.top() {
        Some((a.center_top(), b.center_bottom(), -Vec2::Y))
    } else {
        None
    }
}
fn edge(p: &egui::Painter, a: Rect, b: Rect, color: Color32, directed: bool, zoom: f32) {
    if !p.clip_rect().intersects(a.union(b).expand(16.)) {
        return;
    }
    let Some((start, end, direction)) = edge_ports(a, b) else {
        return;
    };
    let scale = zoom.clamp(0.4, 1.8);
    let gap = (end - start).dot(direction);
    let inset = (3. / scale).min(gap * 0.15);
    let start = start + direction * inset;
    let end = end - direction * inset;
    let bend = (gap * 0.45).clamp(8., 90.);
    let stroke = Stroke::new(1.5 / scale, color);
    p.add(egui::epaint::CubicBezierShape::from_points_stroke(
        [start, start + direction * bend, end - direction * bend, end],
        false,
        Color32::TRANSPARENT,
        stroke,
    ));
    if directed {
        let length = (7. / scale).min(gap * 0.3);
        let back = end - direction * length;
        let normal = Vec2::new(-direction.y, direction.x) * length * 0.48;
        p.line_segment([back + normal, end], stroke);
        p.line_segment([back - normal, end], stroke);
    }
}

#[cfg(test)]
mod board_edge_tests {
    use super::*;

    #[test]
    fn connections_follow_facing_borders_in_all_directions() {
        let a = Rect::from_min_size(egui::pos2(100., 100.), Vec2::new(254., 152.));
        for direction in [Vec2::X, -Vec2::X, Vec2::Y, -Vec2::Y] {
            let b = a.translate(direction * 500.);
            let (start, end, actual) = edge_ports(a, b).unwrap();
            assert_eq!(actual, direction);
            assert!(a.contains(start));
            assert!(b.contains(end));
            assert!((end - start).dot(direction) > 0.);
            let (reverse_start, reverse_end, reverse_direction) = edge_ports(b, a).unwrap();
            assert_eq!(
                (reverse_start, reverse_end, reverse_direction),
                (end, start, -direction)
            );
        }
    }

    #[test]
    fn overlapping_cards_do_not_draw_backwards_arrows() {
        let a = Rect::from_min_size(egui::Pos2::ZERO, Vec2::splat(100.));
        assert!(edge_ports(a, a).is_none());
        assert!(edge_ports(a, a.translate(Vec2::splat(50.))).is_none());
    }
}
/// A run of consecutive days of one week row on which memo `note` shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BandSegment {
    pub note: usize,
    pub first: usize,
    pub last: usize,
    /// The memo does not show the day before `first` (round the left end).
    pub starts_here: bool,
    /// The memo does not show the day after `last` (round the right end).
    pub ends_here: bool,
}
/// Runs of consecutive `days` (one week row) on which each memo shows.
/// The day of the month grid under `p` (held inside the grid).
/// Gray or white, light enough that a band of it needs an edge.
pub fn pale(c: Color32) -> bool {
    let [r, g, b, _] = c.to_array();
    let (lo, hi) = (r.min(g).min(b), r.max(g).max(b));
    lo >= 215 && hi - lo <= 16
}

pub fn grid_day(
    p: egui::Pos2,
    all: Rect,
    first: NaiveDate,
    rows: usize,
    col: f32,
    height: f32,
) -> NaiveDate {
    let c = ((p.x - all.left()) / col).floor().clamp(0., 6.) as i64;
    let r = ((p.y - all.top()) / height)
        .floor()
        .clamp(0., rows.saturating_sub(1) as f32) as i64;
    first + Duration::days(r * 7 + c)
}

pub fn band_segments(
    notes: &[Note],
    visible: impl Fn(usize) -> bool,
    matches: impl Fn(&Note, NaiveDate) -> bool,
    days: &[NaiveDate],
    shown_day: impl Fn(usize) -> bool,
) -> Vec<BandSegment> {
    let mut out = vec![];
    for (i, n) in notes.iter().enumerate() {
        if !visible(i) {
            continue;
        }
        let on: Vec<bool> = days
            .iter()
            .enumerate()
            .map(|(c, &d)| shown_day(c) && matches(n, d))
            .collect();
        let mut c = 0;
        while c < on.len() {
            if !on[c] {
                c += 1;
                continue;
            }
            let first = c;
            while c + 1 < on.len() && on[c + 1] {
                c += 1;
            }
            out.push(BandSegment {
                note: i,
                first,
                last: c,
                starts_here: !(first == 0 && matches(n, days[0] - Duration::days(1))),
                ends_here: !(c == on.len() - 1 && matches(n, days[c] + Duration::days(1))),
            });
            c += 1;
        }
    }
    out
}
/// The lowest free lane for each segment, in order.
pub fn assign_lanes(segments: &[BandSegment]) -> Vec<usize> {
    let mut used: Vec<[bool; 7]> = vec![];
    segments
        .iter()
        .map(|s| {
            let lane = (0..)
                .find(|&l| {
                    used.get(l)
                        .is_none_or(|u| u[s.first..=s.last].iter().all(|x| !x))
                })
                .unwrap();
            if used.len() <= lane {
                used.resize(lane + 1, [false; 7]);
            }
            for x in &mut used[lane][s.first..=s.last] {
                *x = true;
            }
            lane
        })
        .collect()
}
#[cfg(test)]
mod band_tests {
    use super::*;
    #[test]
    fn a_multi_day_memo_is_one_band_split_only_by_week_rows() {
        let d = |day: u32| NaiveDate::from_ymd_opt(2026, 11, day).unwrap();
        let mut long = Note::new("화면 흐름 검토", Some(d(17)), 0);
        long.duration = 4;
        let mut wrap = Note::new("주말 걸침", Some(d(20)), 1);
        wrap.duration = 3;
        let one = Note::new("하루", Some(d(18)), 2);
        let notes = vec![long, wrap, one];
        // Week row Sun 15 .. Sat 21.
        let days: Vec<_> = (15..=21).map(d).collect();
        let segs = band_segments(&notes, |_| true, |n, day| n.occurs(day), &days, |_| true);
        assert_eq!(
            segs[0],
            BandSegment {
                note: 0,
                first: 2,
                last: 5,
                starts_here: true,
                ends_here: true
            }
        );
        // Runs on into the next week: the right end stays open.
        assert_eq!(
            segs[1],
            BandSegment {
                note: 1,
                first: 5,
                last: 6,
                starts_here: true,
                ends_here: false
            }
        );
        assert_eq!(assign_lanes(&segs), vec![0, 1, 1]);
    }
}
#[cfg(test)]
mod column_picker_tests {
    use super::*;
    pub(super) fn has_text(output: &egui::FullOutput, text: &str) -> bool {
        output
            .shapes
            .iter()
            .any(|shape| matches!(&shape.shape, egui::Shape::Text(t) if t.galley.job.text == text))
    }
    #[test]
    fn filters_has_no_column_dropdown_the_header_menu_replaces_it() {
        // The 열 표시 dropdown was removed from the 내 작업실 / 오늘 / 다음
        // list screens: the columns are chosen from the table header's
        // right-click menu instead (catalog.rs).
        let mut a = App::sample(Local::now().date_naive());
        let ctx = egui::Context::default();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(900., 300.));
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    a.filters(ui);
                });
            },
        );
        assert!(!has_text(&output, "열 표시"));
        assert!(has_text(&output, "+ 새 일정"));
    }
}
#[cfg(test)]
mod schedule_button_tests {
    use super::*;
    #[test]
    fn new_schedule_button_creates_a_dated_note_and_opens_its_editor() {
        let today = Local::now().date_naive();
        let mut a = App::sample(today);
        let before = a.data.notes.len();
        a.add_note(Some(today));
        assert_eq!(a.data.notes.len(), before + 1);
        let i = before;
        assert_eq!(a.data.notes[i].date, Some(today));
        // "+ 새 일정" opens the memo editor window, not just a bare sticker.
        assert!(a.editor);
        assert_eq!(a.active, i);
        assert!(a.studio.windows.contains_key(&i));
        assert!(
            a.desktop.always_sticker,
            "even while memos open as stickers by default"
        );
        assert_eq!(a.studio.windows[&i].mode, crate::studio::Mode::Editor);
        // Starts blank (no "새 할 일" title or placeholder body text to
        // delete by hand) - the title/body fields' own hint text is enough.
        assert_eq!(a.data.notes[i].title, "");
        assert_eq!(a.data.notes[i].body, "");
    }
    #[test]
    fn taking_a_memo_out_of_the_schedule_keeps_the_memo() {
        let today = Local::now().date_naive();
        let mut a = App::sample(today);
        let i = a.new_sticker_on(today);
        let n = &mut a.data.notes[i];
        n.title = "회의".into();
        n.body = "안건".into();
        n.duration = 3;
        a.desktop.calendar_basis = 1;
        assert!(a.calendar_matches(&a.data.notes[i], today));
        a.unschedule(i);
        let n = &a.data.notes[i];
        assert_eq!((n.date, n.duration), (None, 1));
        assert!(!n.deleted, "the memo itself stays");
        assert_eq!((n.title.as_str(), n.body.as_str()), ("회의", "안건"));
        assert!(!a.calendar_matches(n, today), "off the schedule calendar");
        assert_eq!(a.studio.windows[&i].schedule_start, None);
    }
}

#[cfg(test)]
mod calendar_side_tests {
    use super::*;

    /// y of the "+ 이 날짜에 스티커" button in one frame of `calendar_main`.
    fn detail_y(app: &mut App, work: bool) -> Option<f32> {
        let ctx = egui::Context::default();
        let mut found = None;
        for i in 0..3 {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1200., 800.),
                )),
                time: Some(i as f64 * 0.1),
                ..Default::default()
            };
            let out = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| app.calendar_main(ui, work));
            });
            found = None;
            for cs in &out.shapes {
                let mut stack = vec![&cs.shape];
                while let Some(sh) = stack.pop() {
                    match sh {
                        egui::Shape::Vec(v) => stack.extend(v.iter()),
                        egui::Shape::Text(t) if t.galley.job.text.contains("이 날짜에 스티커") => {
                            found = Some(t.pos.y)
                        }
                        _ => {}
                    }
                }
            }
        }
        found
    }

    #[test]
    fn the_work_calendar_starts_side_by_side_and_the_page_calendar_below() {
        let today = Local::now().date_naive();
        let mut app = App::sample(today);
        assert!(app.prefs.cal_side_work && !app.prefs.cal_side_page);
        let side = detail_y(&mut app, true).expect("the day's schedule is shown");
        let below = detail_y(&mut app, false).expect("the day's schedule is shown");
        assert!(side < 120., "beside the grid, at the top: {side}");
        assert!(below > 250., "below the grid: {below}");
        // the toggle flips each page's own setting
        app.prefs.cal_side_work = false;
        assert!(detail_y(&mut app, true).unwrap() > 250.);
        app.prefs.cal_side_page = true;
        assert!(detail_y(&mut app, false).unwrap() < 120.);
    }

    #[test]
    fn the_side_by_side_settings_are_saved_with_the_other_settings() {
        let mut p = crate::prefs::Prefs::default();
        assert!(p.cal_side_work && !p.cal_side_page);
        p.cal_side_work = false;
        p.cal_side_page = true;
        let back = crate::prefs::Prefs::from_text(&p.to_text());
        assert!(!back.cal_side_work && back.cal_side_page);
        // a settings file from before has neither key: the defaults apply
        let old = crate::prefs::Prefs::from_text("start_page=0\n");
        assert!(old.cal_side_work && !old.cal_side_page);
    }
}

#[cfg(test)]
mod sticker_drop_tests {
    use super::*;

    #[test]
    fn sticker_preview_chooses_a_grid_and_follows_the_pointer() {
        let today = Local::now().date_naive();
        let mut app = App::sample(today);
        let main = crate::ui::main_viewport();
        let detached = egui::ViewportId::from_hash_of("calendar");
        let work_cell = Rect::from_min_size(egui::pos2(20., 40.), Vec2::new(90., 95.));
        let dock_cell = Rect::from_min_size(egui::pos2(1700., 60.), Vec2::new(180., 150.));
        app.cal_cells.insert(main, (0., vec![(work_cell, today)]));
        app.cal_cells
            .insert(detached, (0., vec![(dock_cell, today.succ_opt().unwrap())]));
        app.day_cells.insert(
            detached,
            (0., dock_cell.translate(Vec2::new(200., 0.)), today),
        );

        assert_eq!(
            app.sticker_preview_at(work_cell.center(), 0.1)
                .unwrap()
                .viewport,
            main
        );
        assert_eq!(
            app.sticker_preview_at(dock_cell.center(), 0.1)
                .unwrap()
                .viewport,
            detached
        );
        assert!(app
            .sticker_preview_at(work_cell.left_top() - Vec2::splat(1.), 0.1)
            .is_none());
        assert!(app
            .sticker_preview_at(dock_cell.center() + Vec2::new(200., 0.), 0.1)
            .is_none());

        for pointer_y in [work_cell.top(), work_cell.center().y, work_cell.bottom()] {
            let pointer = egui::pos2(work_cell.right() - 1., pointer_y);
            let preview = dragged_sticker_bar(work_cell, pointer, 22.);
            assert_eq!(preview.center().y, pointer_y);
            assert!(preview.right() > work_cell.right());
            assert_eq!(preview.height(), 22.);
        }
    }
    #[test]
    fn the_pointer_picks_the_grid_day_and_stays_inside_the_grid() {
        let first = NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
        let all = Rect::from_min_size(egui::pos2(10., 20.), Vec2::new(700., 500.));
        let day = |x: f32, y: f32| grid_day(egui::pos2(x, y), all, first, 5, 100., 100.);
        assert_eq!(day(15., 25.), first);
        assert_eq!(day(355., 125.), first + Duration::days(10));
        // Outside: the nearest day of the grid.
        assert_eq!(day(-50., -50.), first);
        assert_eq!(day(900., 900.), first + Duration::days(34));
    }
    use crate::studio::{Mode, StickerDrag, Window};

    #[test]
    fn a_sticker_let_go_over_a_date_takes_it_and_returns_to_its_place() {
        let today = Local::now().date_naive();
        let mut a = App::sample(today);
        let i = a.new_sticker();
        a.studio.windows.get_mut(&i).unwrap().mode = Mode::Title; // carried as the title-only sticker
        let date = today + Duration::days(3);
        let d = StickerDrag {
            shift: false,
            note: i,
            origin: egui::pos2(140., 220.),
            mode: Mode::Sticker,
            always_restore: true,
            started: 0.,
            saw_down: true,
            carry: None,
        };
        a.drop_sticker(&d, Some(date));
        assert_eq!(a.data.notes[i].date, Some(date));
        let w = &a.studio.windows[&i];
        assert_eq!(w.mode, Mode::Sticker);
        assert_eq!(w.place, Some(egui::pos2(140., 220.)));
        // pulled out by the calendar icon and let go elsewhere: back as it was, no date
        let j = a.new_sticker();
        a.studio.windows.get_mut(&j).unwrap().mode = Mode::Title;
        let d2 = StickerDrag { note: j, ..d };
        a.drop_sticker(&d2, None);
        assert_eq!(a.data.notes[j].date, None);
        assert_eq!(a.studio.windows[&j].mode, Mode::Sticker);
        // a title-only sticker simply moved (not onto a date) stays where it was put
        let k = a.new_sticker();
        a.studio.windows.insert(k, Window::new(Mode::Title));
        let d3 = StickerDrag {
            note: k,
            always_restore: false,
            mode: Mode::Title,
            ..d2
        };
        a.drop_sticker(&d3, None);
        assert_eq!(a.studio.windows[&k].place, None);
        // ... and one dropped on a date returns to its place
        a.drop_sticker(&d3, Some(date));
        assert_eq!(a.studio.windows[&k].place, Some(egui::pos2(140., 220.)));
    }

    #[test]
    fn the_calendar_grid_tells_which_date_is_under_a_screen_point() {
        let today = Local::now().date_naive();
        let mut app = App::sample(today);
        let ctx = egui::Context::default();
        for i in 0..3 {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000., 800.),
                )),
                time: Some(i as f64 * 0.1),
                ..Default::default()
            };
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| app.calendar_main(ui, false));
            });
        }
        let (_, cells) = app
            .cal_cells
            .values()
            .next()
            .expect("the grid registered its dates");
        assert!(cells.len() >= 28);
        let (rect, date) = cells[10];
        assert_eq!(app.calendar_date_at(rect.center(), 0.2), Some(date));
        assert_eq!(
            app.calendar_date_at(rect.center(), 5.0),
            None,
            "a window that stopped drawing is ignored"
        );
        assert_eq!(app.calendar_date_at(egui::pos2(-50., -50.), 0.2), None);
    }
    #[test]
    fn the_next_months_days_in_the_grid_show_and_take_memos() {
        let date = |s: &str| NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap();
        let mut app = App::sample(date("2026-11-10"));
        app.month = date("2026-11-01");
        app.day = date("2026-11-10");
        app.prefs.cal_weeks = 0;
        // November 2026 ends on a Monday: the last row runs to 5 December.
        let i = app.new_sticker_on(date("2026-12-02"));
        app.data.notes[i].title = "다음 달 첫 주 일정".into();
        let ctx = egui::Context::default();
        let mut output = None;
        for t in 0..2 {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000., 900.),
                )),
                time: Some(t as f64 * 0.1),
                ..Default::default()
            };
            output = Some(ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| app.calendar_grid(ui));
            }));
        }
        assert!(super::column_picker_tests::has_text(
            output.as_ref().unwrap(),
            "다음 달 첫 주 일정"
        ));
        assert!(app
            .targets
            .iter()
            .any(|t| matches!(t.kind, dragging::Destination::Date(d) if d == date("2026-12-02"))));
        assert!(app
            .targets
            .iter()
            .any(|t| matches!(t.kind, dragging::Destination::Date(d) if d == date("2026-11-01"))));
    }
    #[test]
    fn only_gray_and_white_bands_get_an_edge() {
        let pale_ones: Vec<bool> = crate::PALETTE.iter().map(|&c| pale(c)).collect();
        assert_eq!(
            pale_ones,
            vec![false, false, false, false, false, false, false, false, true, true]
        );
        assert!(pale(Color32::WHITE));
        assert!(!pale(Color32::from_rgb(120, 120, 120)));
    }
}
