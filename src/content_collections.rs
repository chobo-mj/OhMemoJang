//! Collections projected from the live memo documents, rather than sample flags.
use crate::composer::{Block, Picture, Table};
use crate::*;
use std::borrow::Cow;
use std::io::{Cursor, Write};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;

type Source = (usize, usize, usize);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ContentKey {
    note: usize,
    tab: u64,
    block: u32,
    image: bool,
    instance: u64,
}

impl ContentKey {
    fn pin(self) -> (usize, u64, u32, bool) {
        (self.note, self.tab, self.block, self.image)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ContentAction {
    Copy,
    CopyTableImage,
    Export,
    Open,
    Delete,
}

fn content_action_buttons(ui: &mut egui::Ui, image: bool, count: usize) -> Option<ContentAction> {
    if ui
        .add_enabled(!image || count == 1, egui::Button::new("복사"))
        .on_disabled_hover_text("이미지가 여러 개면 PNG 저장을 사용하세요")
        .clicked()
    {
        return Some(ContentAction::Copy);
    }
    let save = if image {
        "선택 이미지 PNG 저장"
    } else {
        "선택 표 HTML 저장"
    };
    if ui.button(save).clicked() {
        return Some(ContentAction::Export);
    }
    if ui.button("원본 메모 열기").clicked() {
        return Some(ContentAction::Open);
    }
    let delete = if image {
        "선택 이미지 삭제"
    } else {
        "선택 표 삭제"
    };
    ui.button(delete).clicked().then_some(ContentAction::Delete)
}

#[derive(Default)]
struct GallerySources {
    tables: Vec<Source>,
    images: Vec<Source>,
    image_bytes: usize,
}

struct GalleryListing {
    revision: u64,
    total: usize,
    image_bytes: usize,
    sources: Vec<Source>,
    keys: Vec<ContentKey>,
}

fn visible_gallery_rows(ui: &egui::Ui, row_count: usize, row_height: f32) -> Range<usize> {
    let top = ui.cursor().min.y;
    let clip = ui.clip_rect();
    let start = ((clip.top() - top) / row_height).floor().max(0.) as usize;
    let end = ((clip.bottom() - top) / row_height).ceil().max(0.) as usize;
    start.min(row_count)..end.min(row_count)
}

impl App {
    fn content_sources(&self) -> GallerySources {
        let mut sources = GallerySources::default();
        for (i, n) in self.data.notes.iter().enumerate() {
            if !self.visible(i) || n.locked || n.purged {
                continue;
            }
            for (t, tab) in n.document.tabs.iter().enumerate() {
                for (b, item) in tab.blocks.iter().enumerate() {
                    if !tab.shows(item) {
                        continue;
                    }
                    match item {
                        Block::Table(_) => sources.tables.push((i, t, b)),
                        Block::Picture(picture) => {
                            sources.images.push((i, t, b));
                            let bytes = picture
                                .width
                                .saturating_mul(picture.height)
                                .saturating_mul(4);
                            sources.image_bytes = sources.image_bytes.saturating_add(bytes);
                        }
                    }
                }
            }
        }
        sources
    }

    fn content_listing(&self, ctx: &egui::Context, image: bool) -> Arc<GalleryListing> {
        let base_revision = crate::lists::key((
            self.data.notes.revision(),
            self.extra
                .hidden_projects
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            self.extra
                .deleted_projects
                .iter()
                .copied()
                .collect::<Vec<_>>(),
        ));
        let listing_revision = crate::lists::key((
            base_revision,
            &self.extra.pinned_content,
            image,
            if image { "" } else { &self.extra.table_search },
            if image {
                (self.extra.media_from, self.extra.media_to)
            } else {
                (None, None)
            },
        ));
        let listing_id = egui::Id::new(("content_listing", image));
        if let Some(ready) = ctx.data(|d| d.get_temp::<Arc<GalleryListing>>(listing_id)) {
            if ready.revision == listing_revision {
                return ready;
            }
        }
        let sources_id = egui::Id::new("gallery_sources");
        let cached = ctx.data(|d| d.get_temp::<(u64, Arc<GallerySources>)>(sources_id));
        let sources =
            if let Some((_, sources)) = cached.filter(|(revision, _)| *revision == base_revision) {
                sources
            } else {
                let sources = Arc::new(self.content_sources());
                ctx.data_mut(|d| d.insert_temp(sources_id, (base_revision, sources.clone())));
                sources
            };
        let rows = if image {
            &sources.images
        } else {
            &sources.tables
        };
        let query = self.extra.table_search.to_lowercase();
        let mut filtered: Vec<_> = rows.iter().copied().filter(|&(i, t, b)| {
            if image {
                let date = self.data.notes[i].created.date();
                return self.extra.media_from.is_none_or(|d| date >= d)
                    && self.extra.media_to.is_none_or(|d| date <= d);
            }
            query.is_empty() || matches!(
                &self.data.notes[i].document.tabs[t].blocks[b],
                Block::Table(table)
                    if table.cells.iter().flatten().any(|cell| cell.text.to_lowercase().contains(&query))
            )
        }).collect();
        self.pinned_first(&mut filtered);
        let keys = filtered
            .iter()
            .map(|&source| self.content_key(source))
            .collect();
        let listing = Arc::new(GalleryListing {
            revision: listing_revision,
            total: rows.len(),
            image_bytes: sources.image_bytes,
            sources: filtered,
            keys,
        });
        ctx.data_mut(|d| d.insert_temp(listing_id, listing.clone()));
        listing
    }

    fn content_key(&self, (i, t, b): Source) -> ContentKey {
        let tab = &self.data.notes[i].document.tabs[t];
        let (block, image, instance) = tab.blocks[b].identity();
        ContentKey {
            note: i,
            tab: tab.id,
            block,
            image,
            instance,
        }
    }

    fn content_source(&self, key: ContentKey) -> Option<Source> {
        let note = self.data.notes.get(key.note)?;
        if !self.visible(key.note) || note.locked || note.purged {
            return None;
        }
        let ti = note
            .document
            .tabs
            .iter()
            .position(|tab| tab.id == key.tab)?;
        let tab = &note.document.tabs[ti];
        let bi = tab.blocks.iter().position(|block| {
            block.identity() == (key.block, key.image, key.instance) && tab.shows(block)
        })?;
        Some((key.note, ti, bi))
    }

    fn pinned_first(&self, sources: &mut [Source]) {
        sources.sort_by_key(|&s| {
            !self
                .extra
                .pinned_content
                .contains(&self.content_key(s).pin())
        });
    }

    fn toggle_content_pin(&mut self, key: ContentKey) {
        let pin = key.pin();
        if !self.extra.pinned_content.remove(&pin) {
            self.extra.pinned_content.insert(pin);
        }
    }

    fn open_content_source(&mut self, note: usize, tab: usize) {
        if tab < self.data.notes[note].document.tabs.len() {
            let document = &mut self.data.notes[note].document;
            document.active = tab;
            document.tabs[tab].hidden = false;
        }
        self.open_note(note);
    }

    fn content_controls(
        &mut self,
        ui: &mut egui::Ui,
        listing: &GalleryListing,
        image: bool,
    ) -> Option<ContentAction> {
        let keys = listing.keys.as_slice();
        let reconcile = egui::Id::new(("content_reconcile", image));
        if ui.data_mut(|d| {
            let changed = d.get_temp::<u64>(reconcile) != Some(listing.revision);
            d.insert_temp(reconcile, listing.revision);
            changed
        }) {
            self.content_selection.retain_visible(keys);
        }
        let focus = egui::Id::new("content_collection_focus");
        ui.memory_mut(|m| m.interested_in_focus(focus, ui.layer_id()));
        let owns_focus = ui.memory(|m| m.focused().is_none_or(|id| id == focus));
        let blocked = !self.content_purge.is_empty() || ui.memory(|m| m.any_popup_open());
        self.content_selection
            .keyboard(ui.ctx(), keys, owns_focus, blocked);
        let mut action = None;
        ui.horizontal_wrapped(|ui| {
            if self.content_selection.toolbar(ui) && self.content_selection.active {
                ui.memory_mut(|m| m.request_focus(focus));
            }
            if !self.content_selection.active {
                return;
            }
            let count = self.content_selection.items.len();
            ui.add_enabled_ui(count > 0, |ui| {
                action = content_action_buttons(ui, image, count);
            });
        });
        action
    }

    fn content_card_click(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        key: ContentKey,
        keys: &[ContentKey],
        single: Option<ContentAction>,
    ) -> Option<ContentAction> {
        if !response.clicked() {
            return None;
        }
        let focus = egui::Id::new("content_collection_focus");
        ui.memory_mut(|m| m.request_focus(focus));
        let modifiers = ui.input(|input| input.modifiers);
        if self.content_selection.active || modifiers.ctrl || modifiers.command || modifiers.shift {
            self.content_selection.click(key, keys, modifiers);
            None
        } else {
            single
        }
    }

    fn content_card_menu(
        &self,
        response: &egui::Response,
        key: ContentKey,
        batch: &mut Option<ContentAction>,
        single: &mut Option<(ContentAction, ContentKey)>,
    ) {
        let use_selection =
            self.content_selection.active && self.content_selection.items.contains(&key);
        let count = if use_selection {
            self.content_selection.items.len()
        } else {
            1
        };
        response.context_menu(|ui| {
            if let Some(action) = content_action_buttons(ui, key.image, count) {
                if use_selection {
                    *batch = Some(action);
                } else {
                    *single = Some((action, key));
                }
                ui.close_menu();
            }
        });
    }

    pub fn content_tables(&mut self, ui: &mut egui::Ui) {
        let listing = self.content_listing(ui.ctx(), false);
        if listing.total == 0 {
            self.content_selection.retain_visible(&[]);
            self.content_delete_dialog(ui.ctx(), false);
            if empty_with_example(
                ui,
                crate::icons::Icon::Grid,
                "메모에 붙여 넣은 표가 없습니다",
                "표를 복사해 메모 본문에 붙여 넣으면 여기에 모입니다",
            ) {
                self.real_table_gallery(ui);
            }
            return;
        }
        ui.horizontal(|ui| {
            crate::ui::section_header(
                ui,
                crate::icons::Icon::Grid,
                "메모 속 표",
                Some(listing.total),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                crate::ui::search_field(
                    ui,
                    &mut self.extra.table_search,
                    "table_search",
                    "표 안의 내용 검색",
                    300.,
                );
            });
        });
        let listing = self.content_listing(ui.ctx(), false);
        let keys = &listing.keys;
        let mut action = self.content_controls(ui, &listing, false);
        let mut single = None;
        let selection_mode = self.content_selection.active;
        for (&source, &key) in listing.sources.iter().zip(keys) {
            let (i, ti, bi) = source;
            let n = &self.data.notes[i];
            let tab = &n.document.tabs[ti];
            let Block::Table(table) = &tab.blocks[bi] else {
                continue;
            };
            let mut response = None;
            let mut pin = false;
            let estimate = table.cells.len() as f32 * 27. + 66.;
            crate::lists::lazy_row(ui, ("content_table", key), estimate, false, |ui| {
                let source_title = format!("{} · {}", n.title, tab.title);
                let pinned = self.extra.pinned_content.contains(&key.pin());
                let selected = self.content_selection.items.contains(&key);
                let tone = ui::t(ui);
                egui::Frame::NONE
                    .fill(if selected { tone.accent_soft } else { tone.bg })
                    .stroke(egui::Stroke::new(1_f32, tone.line))
                    .corner_radius(10)
                    .inner_margin(egui::Margin::symmetric(8, 8))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            if selection_mode {
                                if self.content_selection.checkbox(ui, key, &keys) {
                                    ui.memory_mut(|m| {
                                        m.request_focus(egui::Id::new("content_collection_focus"))
                                    });
                                }
                            }
                            let label = if selected {
                                RichText::new(&source_title).strong().color(ACCENT)
                            } else {
                                RichText::new(&source_title).strong()
                            };
                            response = Some(ui.add(
                                egui::Label::new(label.size(13.5)).sense(egui::Sense::click()),
                            ));
                            if !selection_mode {
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if crate::ui::pill(ui, false, "이미지로 복사").clicked()
                                        {
                                            single = Some((ContentAction::CopyTableImage, key));
                                        }
                                        if crate::ui::pill(ui, false, "표로 복사").clicked() {
                                            single = Some((ContentAction::Copy, key));
                                        }
                                        if crate::ui::pill(ui, false, "원본 메모").clicked() {
                                            single = Some((ContentAction::Open, key));
                                        }
                                        pin = crate::icons::toggle_icon(
                                            ui,
                                            crate::icons::Icon::Pin,
                                            if pinned {
                                                "고정 해제"
                                            } else {
                                                "맨 위에 고정"
                                            },
                                            pinned,
                                        )
                                        .clicked();
                                    },
                                );
                            }
                        });
                        ui.add_space(8.);
                        egui::ScrollArea::horizontal()
                            .id_salt(("content_table_scroll", key))
                            .show(ui, |ui| {
                                egui::Grid::new(("content_table_cells", key))
                                    .striped(true)
                                    .min_col_width(110.)
                                    .max_col_width(260.)
                                    .show(ui, |ui| {
                                        for row in &table.cells {
                                            for cell in row {
                                                let mut text = RichText::new(&cell.text);
                                                if cell.format.bold {
                                                    text = text.strong();
                                                }
                                                if cell.format.italic {
                                                    text = text.italics();
                                                }
                                                if cell.format.underline {
                                                    text = text.underline();
                                                }
                                                if cell.format.strike {
                                                    text = text.strikethrough();
                                                }
                                                ui.label(text);
                                            }
                                            ui.end_row();
                                        }
                                    });
                            });
                    });
                ui.add_space(12.);
            });
            if let Some(response) = response {
                self.content_card_click(ui, &response, key, &keys, None);
                self.content_card_menu(&response, key, &mut action, &mut single);
            }
            if pin {
                self.toggle_content_pin(key);
            }
        }
        if let Some(action) = action {
            self.run_content_action(ui, action, &self.selected_content(&keys));
        } else if let Some((action, key)) = single {
            self.run_content_action(ui, action, &[key]);
        }
        if keys.is_empty() {
            crate::ui::empty_state(
                ui,
                crate::icons::Icon::Search,
                "일치하는 표가 없습니다",
                "다른 검색어로 찾아보세요",
            );
        }
        self.content_delete_dialog(ui.ctx(), false);
    }

    pub fn content_images(&mut self, ui: &mut egui::Ui) {
        let listing = self.content_listing(ui.ctx(), true);
        if listing.total == 0 {
            self.content_selection.retain_visible(&[]);
            self.content_delete_dialog(ui.ctx(), true);
            if empty_with_example(
                ui,
                crate::icons::Icon::Grid,
                "메모에 붙여 넣은 이미지가 없습니다",
                "이미지를 복사해 메모 본문에 붙여 넣으면 여기에 모입니다",
            ) {
                self.real_image_gallery(ui);
            }
            return;
        }
        ui.label(format!(
            "{}개 · 편집 이미지 {:.2} MB · 메모리 기준",
            listing.total,
            listing.image_bytes as f64 / 1024. / 1024.
        ));
        let mut batch = false;
        ui.horizontal_wrapped(|ui| {
            self.date_filter_button(ui, 6, "시작 날짜", self.extra.media_from);
            self.date_filter_button(ui, 7, "끝 날짜", self.extra.media_to);
            if ui.button("필터 해제").clicked() {
                self.extra.media_from = None;
                self.extra.media_to = None;
            }
            batch =
                !self.content_selection.active && ui.button("표시한 이미지 일괄 저장").clicked();
        });
        let listing = self.content_listing(ui.ctx(), true);
        let sources = &listing.sources;
        let keys = &listing.keys;
        let mut action = self.content_controls(ui, &listing, true);
        ui.label(
            RichText::new(format!(
                "{}개 표시 · 날짜 기준은 원본 메모 생성일입니다",
                keys.len()
            ))
            .small()
            .color(MUTED),
        );
        ui.add_space(10.);
        let columns = ((ui.available_width() + 12.) / 245.).floor().clamp(1., 4.) as usize;
        let card_width =
            ((ui.available_width() - 14. * (columns as f32 - 1.)) / columns as f32).max(180.);
        let mut single = None;
        let selection_mode = self.content_selection.active;
        ui.scope(|ui| {
            const ROW_HEIGHT: f32 = 252.;
            let row_count = sources.len().div_ceil(columns);
            let visible = visible_gallery_rows(ui, row_count, ROW_HEIGHT);
            let spacing = ui.spacing().item_spacing.y;
            ui.spacing_mut().item_spacing.y = 0.;
            ui.add_space(visible.start as f32 * ROW_HEIGHT);
            for row in visible.clone() {
                let from = row * columns;
                let to = (from + columns).min(sources.len());
                let row_sources = &sources[from..to];
                let row_keys = &keys[from..to];
                let width = ui.available_width();
                ui.allocate_ui_with_layout(
                    Vec2::new(width, ROW_HEIGHT),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        ui.spacing_mut().item_spacing.y = spacing;
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 14.;
                            for (&(i, t, b), &key) in row_sources.iter().zip(row_keys) {
                                let n = &self.data.notes[i];
                                let Block::Picture(picture) = &n.document.tabs[t].blocks[b] else {
                                    continue;
                                };
                                let pinned = self.extra.pinned_content.contains(&key.pin());
                                let selected = self.content_selection.items.contains(&key);
                                let bg = if selected {
                                    ui::t(ui).accent_soft
                                } else {
                                    ui.visuals().faint_bg_color
                                };
                                let mut image_click = None;
                                let mut title_click = None;
                                let mut pin = false;
                                ui.allocate_ui_with_layout(
                                    Vec2::new(card_width, 230.),
                                    egui::Layout::top_down(egui::Align::LEFT),
                                    |ui| {
                                        egui::Frame::NONE
                                            .fill(bg)
                                            .corner_radius(8.)
                                            .inner_margin(10.)
                                            .show(ui, |ui| {
                                                ui.set_width(card_width - 20.);
                                                let image_width = card_width - 20.;
                                                ui.allocate_ui(
                                                    Vec2::new(image_width, 145.),
                                                    |ui| {
                                                        if let Ok(texture) =
                                                            picture_texture(ui, key, picture)
                                                        {
                                                            let factor = (image_width
                                                                / picture.width as f32)
                                                                .min(145. / picture.height as f32);
                                                            let size = Vec2::new(
                                                                picture.width as f32 * factor,
                                                                picture.height as f32 * factor,
                                                            );
                                                            let response = ui.add(
                                                                egui::Image::new(&texture)
                                                                    .fit_to_exact_size(size)
                                                                    .sense(egui::Sense::click()),
                                                            );
                                                            image_click = Some(if selection_mode {
                                                                response
                                                            } else {
                                                                response.on_hover_text(
                                                                    "클릭하면 이미지가 복사됩니다",
                                                                )
                                                            });
                                                        } else {
                                                            ui.label("이미지를 표시할 수 없습니다");
                                                        }
                                                    },
                                                );
                                                ui.horizontal(|ui| {
                                                    if selection_mode {
                                                        if self
                                                            .content_selection
                                                            .checkbox(ui, key, &keys)
                                                        {
                                                            ui.memory_mut(|m| {
                                                                m.request_focus(egui::Id::new(
                                                                    "content_collection_focus",
                                                                ))
                                                            });
                                                        }
                                                    }
                                                    title_click = Some(
                                                        ui.add(
                                                            egui::Label::new(
                                                                RichText::new(&picture.name)
                                                                    .strong(),
                                                            )
                                                            .truncate()
                                                            .sense(egui::Sense::click()),
                                                        ),
                                                    );
                                                });
                                                ui.label(
                                                    RichText::new(format!(
                                                        "{} · {} × {}",
                                                        n.created.format("%Y.%m.%d"),
                                                        picture.width,
                                                        picture.height
                                                    ))
                                                    .small()
                                                    .color(MUTED),
                                                );
                                                if !selection_mode {
                                                    ui.horizontal(|ui| {
                                                        pin = crate::icons::toggle_icon(
                                                            ui,
                                                            crate::icons::Icon::Pin,
                                                            if pinned {
                                                                "고정 해제"
                                                            } else {
                                                                "맨 위에 고정"
                                                            },
                                                            pinned,
                                                        )
                                                        .clicked();
                                                        if save_icon(ui).clicked() {
                                                            single =
                                                                Some((ContentAction::Export, key));
                                                        }
                                                        if ui
                                                            .small_button("원본 메모")
                                                            .on_hover_text(&n.title)
                                                            .clicked()
                                                        {
                                                            single =
                                                                Some((ContentAction::Open, key));
                                                        }
                                                    });
                                                }
                                            });
                                    },
                                );
                                if let Some(response) = image_click {
                                    if let Some(action) = self.content_card_click(
                                        ui,
                                        &response,
                                        key,
                                        &keys,
                                        Some(ContentAction::Copy),
                                    ) {
                                        single = Some((action, key));
                                    }
                                }
                                if let Some(response) = title_click {
                                    self.content_card_click(ui, &response, key, &keys, None);
                                    self.content_card_menu(
                                        &response,
                                        key,
                                        &mut action,
                                        &mut single,
                                    );
                                }
                                if pin {
                                    self.toggle_content_pin(key);
                                }
                            }
                        });
                    },
                );
            }
            ui.add_space((row_count - visible.end) as f32 * ROW_HEIGHT);
        });
        if let Some(action) = action {
            self.run_content_action(ui, action, &self.selected_content(&keys));
        } else if let Some((action, key)) = single {
            if action == ContentAction::Export {
                self.save_one_image(key);
            } else {
                self.run_content_action(ui, action, &[key]);
            }
        } else if batch {
            self.run_content_action(ui, ContentAction::Export, &keys);
        }
        if sources.is_empty() {
            ui.label("선택한 날짜에 생성된 이미지가 없습니다");
        }
        self.content_delete_dialog(ui.ctx(), true);
    }

    fn selected_content(&self, keys: &[ContentKey]) -> Vec<ContentKey> {
        keys.iter()
            .copied()
            .filter(|key| self.content_selection.items.contains(key))
            .collect()
    }

    fn run_content_action(&mut self, ui: &egui::Ui, action: ContentAction, keys: &[ContentKey]) {
        let Some(first) = keys.first() else {
            return;
        };
        let Some(sources): Option<Vec<_>> =
            keys.iter().map(|&key| self.content_source(key)).collect()
        else {
            self.toast = "항목이 변경되었습니다. 다시 선택하세요".into();
            return;
        };
        match action {
            ContentAction::Delete => {
                self.content_purge = keys.to_vec();
                ui.ctx().request_repaint();
            }
            ContentAction::Open => {
                let mut opened = std::collections::HashSet::new();
                for (i, t, _) in sources {
                    if opened.insert(i) {
                        self.open_content_source(i, t);
                    }
                }
            }
            ContentAction::Copy if first.image => {
                let (i, t, b) = sources[0];
                if let Block::Picture(picture) = &self.data.notes[i].document.tabs[t].blocks[b] {
                    self.toast = validate_picture(picture)
                        .and_then(|_| {
                            copy_rgba(picture.width, picture.height, picture.rgba().into_owned())
                        })
                        .map(|_| "이미지를 복사했습니다".into())
                        .unwrap_or_else(|e| format!("복사 실패: {e}"));
                }
            }
            ContentAction::CopyTableImage => {
                let (i, t, b) = sources[0];
                if let Block::Table(table) = &self.data.notes[i].document.tabs[t].blocks[b] {
                    self.toast = table_image(table)
                        .and_then(|image| {
                            copy_rgba(
                                image.width() as usize,
                                image.height() as usize,
                                image.into_raw(),
                            )
                        })
                        .map(|_| "표 이미지를 복사했습니다".into())
                        .unwrap_or_else(|e| format!("복사 실패: {e}"));
                }
            }
            ContentAction::Export if first.image => {
                if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                    let mut saved = 0;
                    for (i, t, b) in sources {
                        if let Block::Picture(picture) =
                            &self.data.notes[i].document.tabs[t].blocks[b]
                        {
                            let stem = format!(
                                "memo-{}-{}-{}-{}",
                                i + 1,
                                t + 1,
                                b + 1,
                                safe_stem(&picture.name)
                            );
                            if picture_png(picture)
                                .and_then(|bytes| write_unique_png(&folder, &stem, &bytes))
                                .is_ok()
                            {
                                saved += 1;
                            }
                        }
                    }
                    let failed = keys.len() - saved;
                    self.toast = format!(
                        "이미지 {saved}개 저장{}",
                        if failed > 0 {
                            format!(" · {failed}개 실패")
                        } else {
                            String::new()
                        }
                    );
                }
            }
            ContentAction::Copy | ContentAction::Export => {
                let tables = sources.iter().filter_map(|&(i, t, b)| {
                    match &self.data.notes[i].document.tabs[t].blocks[b] {
                        Block::Table(table) => Some(table),
                        Block::Picture(_) => None,
                    }
                });
                let (html, plain) = table_batch_formats(tables);
                if action == ContentAction::Copy {
                    let result = crate::clipboard_copy::Clipboard::new()
                        .and_then(|mut clipboard| clipboard.set_html(html, Some(plain)));
                    self.toast = result
                        .map(|_| format!("표 {}개를 복사했습니다", keys.len()))
                        .unwrap_or_else(|e| format!("복사 실패: {e}"));
                } else if let Some(path) = rfd::FileDialog::new()
                    .add_filter("HTML", &["html"])
                    .set_file_name("tables.html")
                    .save_file()
                {
                    let html = format!(
                        "<!doctype html><meta charset=\"utf-8\"><title>OhMemoJang</title>\n{html}"
                    );
                    self.toast = std::fs::write(path, html)
                        .map(|_| format!("표 {}개를 저장했습니다", keys.len()))
                        .unwrap_or_else(|e| format!("저장 실패: {e}"));
                }
            }
        }
    }

    fn save_one_image(&mut self, key: ContentKey) {
        let Some((i, t, b)) = self.content_source(key) else {
            return;
        };
        let Block::Picture(picture) = &self.data.notes[i].document.tabs[t].blocks[b] else {
            return;
        };
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("PNG 이미지", &["png"])
            .set_file_name(format!("{}.png", safe_stem(&picture.name)))
            .save_file()
        {
            self.toast = picture_png(picture)
                .and_then(|bytes| std::fs::write(path, bytes).map_err(|e| e.to_string()))
                .map(|_| "이미지를 저장했습니다".into())
                .unwrap_or_else(|e| format!("저장 실패: {e}"));
        }
    }

    fn content_delete_dialog(&mut self, ctx: &egui::Context, image: bool) {
        if self.content_purge.is_empty() {
            return;
        }
        let kind = if image { "이미지" } else { "표" };
        let modal = egui::Modal::new(egui::Id::new("delete_collected_content")).show(ctx, |ui| {
            ui.heading(format!("선택 {kind} 삭제"));
            ui.label(format!(
                "선택한 {kind} {}개를 원본 메모에서 삭제합니다.",
                self.content_purge.len()
            ));
            crate::ui::dialog_buttons(ui, "삭제", "취소")
        });
        let (confirm, cancel) = modal.inner;
        if cancel || modal.should_close() {
            self.content_purge.clear();
        } else if confirm {
            let targets = std::mem::take(&mut self.content_purge);
            if self.delete_content(&targets) {
                self.toast = format!("{kind} {}개를 삭제했습니다", targets.len());
            } else {
                self.toast = "선택한 항목이 바뀌었습니다. 다시 선택하세요".into();
            }
        }
    }

    fn delete_content(&mut self, targets: &[ContentKey]) -> bool {
        let Some(sources): Option<Vec<_>> = targets
            .iter()
            .map(|&key| self.content_source(key))
            .collect()
        else {
            return false;
        };
        let mut by_tab = std::collections::BTreeMap::<_, Vec<_>>::new();
        for (i, t, b) in sources {
            by_tab.entry((i, t)).or_default().push(b);
        }
        for ((i, t), indices) in by_tab {
            let note = &mut self.data.notes[i];
            note.document.remove_collection_blocks(t, &indices);
            if t == 0 {
                note.body = note.document.tabs[0].body.clone();
            }
        }
        let removed_pins: std::collections::HashSet<_> =
            targets.iter().map(|key| key.pin()).collect();
        self.extra
            .pinned_content
            .retain(|&(i, tab_id, block_id, image)| {
                !removed_pins.contains(&(i, tab_id, block_id, image))
                    || self.data.notes[i].document.tabs.iter().any(|tab| {
                        tab.id == tab_id
                            && tab.blocks.iter().any(|block| {
                                let (id, is_image, _) = block.identity();
                                id == block_id && is_image == image
                            })
                    })
            });
        for key in targets {
            self.content_selection.items.remove(key);
        }
        true
    }
}

fn table_batch_formats<'a>(tables: impl Iterator<Item = &'a Table>) -> (String, String) {
    let mut html = Vec::new();
    let mut text = Vec::new();
    for table in tables {
        html.push(table_html(table));
        text.push(crate::table_edit::table_tsv(table));
    }
    (html.join("\n"), text.join("\r\n"))
}

/// An empty collection: a placeholder plus a toggle that shows example
/// content. Returns whether the example is shown.
fn empty_with_example(
    ui: &mut egui::Ui,
    icon: crate::icons::Icon,
    title: &str,
    hint: &str,
) -> bool {
    let id = ui.id().with(("show_example", title));
    let mut show = ui.data(|d| d.get_temp::<bool>(id).unwrap_or(false));
    crate::ui::card(ui, |ui| {
        ui.set_width(ui.available_width());
        crate::ui::empty_state(ui, icon, title, hint);
        ui.vertical_centered(|ui| {
            if crate::ui::pill(
                ui,
                show,
                if show {
                    "예시 숨기기"
                } else {
                    "예시 보기"
                },
            )
            .clicked()
            {
                show = !show;
            }
            ui.add_space(10.);
        });
    });
    ui.data_mut(|d| d.insert_temp(id, show));
    if show {
        ui.add_space(12.);
    }
    show
}
fn table_html(table: &Table) -> String {
    crate::table_edit::table_html(table)
}

fn copy_rgba(width: usize, height: usize, pixels: Vec<u8>) -> Result<(), String> {
    if width.checked_mul(height).and_then(|n| n.checked_mul(4)) != Some(pixels.len()) {
        return Err("이미지 픽셀 데이터를 읽을 수 없습니다".into());
    }
    crate::clipboard_copy::Clipboard::new()
        .and_then(|mut c| {
            c.set_image(arboard::ImageData {
                width,
                height,
                bytes: Cow::Owned(pixels),
            })
        })
        .map_err(|e| e.to_string())
}

fn validate_picture(p: &Picture) -> Result<(), String> {
    if p.width == 0 || p.height == 0 || p.width > 8192 || p.height > 8192 {
        return Err("이미지 크기 또는 픽셀 데이터가 올바르지 않습니다".into());
    }
    Ok(())
}

fn picture_png(p: &Picture) -> Result<Vec<u8>, String> {
    validate_picture(p)?;
    let image = image::RgbaImage::from_raw(p.width as u32, p.height as u32, p.rgba().into_owned())
        .ok_or("이미지 변환 실패")?;
    let mut bytes = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut bytes, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(bytes.into_inner())
}

fn picture_texture(
    ui: &egui::Ui,
    source: ContentKey,
    picture: &Picture,
) -> Result<egui::TextureHandle, String> {
    validate_picture(picture)?;
    let id = egui::Id::new((
        "content_picture_texture",
        source,
        picture.width,
        picture.height,
        &picture.file,
    ));
    // A small thumbnail, not the full picture.
    crate::lists::texture(ui.ctx(), id, || crate::storage::thumbnail(picture))
        .ok_or_else(|| "이미지를 읽을 수 없습니다".into())
}

fn safe_stem(name: &str) -> String {
    let stem = Path::new(name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("image");
    let safe: String = stem
        .chars()
        .filter(|&c| !c.is_control() && !"<>:\"/\\|?*".contains(c))
        .take(70)
        .collect();
    let safe = safe.trim_matches([' ', '.']);
    if safe.is_empty() {
        "image".into()
    } else {
        safe.into()
    }
}

fn write_unique_png(folder: &Path, stem: &str, bytes: &[u8]) -> Result<PathBuf, String> {
    for suffix in 0..10000 {
        let name = if suffix == 0 {
            format!("{stem}.png")
        } else {
            format!("{stem}-{suffix}.png")
        };
        let path = folder.join(name);
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => match file.write_all(bytes) {
                Ok(()) => return Ok(path),
                Err(e) => {
                    drop(file);
                    let _ = std::fs::remove_file(&path);
                    return Err(e.to_string());
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    Err("같은 이름의 파일이 너무 많습니다".into())
}

fn save_icon(ui: &mut egui::Ui) -> egui::Response {
    let (rect, r) = ui.allocate_exact_size(Vec2::new(30., 28.), egui::Sense::click());
    r.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "다른 이름으로 저장")
    });
    let c = rect.center();
    let stroke = egui::Stroke::new(1.5_f32, ui.visuals().text_color());
    for (a, b) in [
        (Vec2::new(0., -7.), Vec2::new(0., 4.)),
        (Vec2::new(-4., 0.), Vec2::new(0., 4.)),
        (Vec2::new(4., 0.), Vec2::new(0., 4.)),
        (Vec2::new(-7., 7.), Vec2::new(7., 7.)),
    ] {
        ui.painter().line_segment([c + a, c + b], stroke);
    }
    r.on_hover_text("다른 이름으로 저장")
}

fn table_image(table: &Table) -> Result<image::RgbaImage, String> {
    static FONT: std::sync::OnceLock<Option<fontdue::Font>> = std::sync::OnceLock::new();
    let font = FONT
        .get_or_init(|| {
            std::fs::read("C:/Windows/Fonts/malgun.ttf")
                .ok()
                .and_then(|b| fontdue::Font::from_bytes(b, fontdue::FontSettings::default()).ok())
        })
        .as_ref()
        .ok_or("표 이미지 복사에 필요한 글꼴을 읽지 못했습니다")?;
    let columns = table.cells.iter().map(Vec::len).max().unwrap_or(0);
    if columns == 0 || table.cells.is_empty() || columns > 20 || table.cells.len() > 100 {
        return Err("복사할 표의 크기를 확인해 주세요".into());
    }
    let width = 180u32;
    let size = 17.;
    let rows: Vec<Vec<Vec<String>>> = table
        .cells
        .iter()
        .map(|row| {
            row.iter()
                .map(|c| wrapped_lines(&c.text, width as f32 - 18., size, font))
                .collect()
        })
        .collect();
    let heights: Vec<_> = rows
        .iter()
        .map(|r| (r.iter().map(Vec::len).max().unwrap_or(1) as u32 * 23 + 16).max(40))
        .collect();
    let total: u32 = heights.iter().sum::<u32>() + 1;
    if total > 8192 {
        return Err("표가 너무 길어 이미지로 복사할 수 없습니다. 표로 복사를 사용해 주세요".into());
    }
    let mut image = image::RgbaImage::from_pixel(
        columns as u32 * width + 1,
        total,
        image::Rgba([255, 254, 252, 255]),
    );
    let mut top = 0;
    for (ri, row) in rows.iter().enumerate() {
        for (ci, cell) in row.iter().enumerate() {
            for (li, line) in cell.iter().enumerate() {
                draw_line(
                    &mut image,
                    font,
                    line,
                    ci as u32 * width + 9,
                    top + 8 + li as u32 * 23,
                    size,
                );
            }
        }
        top += heights[ri];
        for x in 0..image.width() {
            image.put_pixel(x, top, image::Rgba([205, 205, 200, 255]));
        }
    }
    for col in 0..=columns {
        let x = col as u32 * width;
        for y in 0..image.height() {
            image.put_pixel(x, y, image::Rgba([205, 205, 200, 255]));
        }
    }
    for x in 0..image.width() {
        image.put_pixel(x, 0, image::Rgba([205, 205, 200, 255]));
    }
    Ok(image)
}

fn wrapped_lines(text: &str, width: f32, size: f32, font: &fontdue::Font) -> Vec<String> {
    let mut lines = vec![];
    let mut line = String::new();
    let mut advance = 0.;
    for ch in text.chars() {
        if ch == '\n' {
            lines.push(std::mem::take(&mut line));
            advance = 0.;
            continue;
        }
        if ch == '\r' {
            continue;
        }
        let step = font.metrics(ch, size).advance_width;
        if advance + step > width && !line.is_empty() {
            lines.push(std::mem::take(&mut line));
            advance = 0.;
        }
        line.push(ch);
        advance += step;
    }
    lines.push(line);
    lines
}

fn draw_line(
    image: &mut image::RgbaImage,
    font: &fontdue::Font,
    text: &str,
    x: u32,
    y: u32,
    size: f32,
) {
    let mut left = x as i32;
    for ch in text.chars() {
        let (m, raster) = font.rasterize(ch, size);
        for row in 0..m.height {
            for col in 0..m.width {
                let xx = left + m.xmin + col as i32;
                let yy = y as i32 + size as i32 - m.height as i32 - m.ymin + row as i32;
                if xx >= 0 && yy >= 0 && xx < image.width() as i32 && yy < image.height() as i32 {
                    let alpha = raster[row * m.width + col] as f32 / 255.;
                    let p = image.get_pixel_mut(xx as u32, yy as u32);
                    for c in 0..3 {
                        p[c] = (p[c] as f32 * (1. - alpha) + 48. * alpha) as u8;
                    }
                }
            }
        }
        left += m.advance_width.ceil() as i32;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::composer::{Format, Span};

    #[test]
    fn deleting_selected_blocks_keeps_unicode_text_formatting_and_other_blocks() {
        let mut app = App::sample(Local::now().date_naive());
        let document = &mut app.data.notes[0].document;
        let tab = &mut document.tabs[0];
        tab.body = "가나다".into();
        tab.insert_table(1..1, Table::new(1, 1));
        tab.insert_picture(
            3..3,
            Picture::from_rgba(0, "이미지".into(), vec![255; 4], [1, 1], 1., false),
        );
        tab.spans = vec![Span {
            range: 2..5,
            format: Format {
                bold: true,
                ..Format::default()
            },
        }];
        tab.blocks.push(Block::Table(Table::new(1, 1)));
        tab.blocks.push(Block::Table(Table::new(1, 1)));
        let inline = app.content_key((0, 0, 0));
        let legacy = app.content_key((0, 0, 2));
        let keep = app.content_key((0, 0, 3));
        app.extra.pinned_content.insert(inline.pin());

        assert!(app.delete_content(&[inline, legacy]));
        let note = &app.data.notes[0];
        let tab = &note.document.tabs[0];
        assert_eq!(
            tab.body,
            format!("가나{}다", crate::composer::placeholder(1))
        );
        assert_eq!(note.body, tab.body);
        assert_eq!(tab.spans[0].range, 1..4);
        assert!(tab.spans[0].format.bold);
        assert_eq!(app.content_source(keep), Some((0, 0, 1)));
        assert!(app.content_source(legacy).is_none());
        assert_eq!(tab.blocks.len(), 2);
    }

    #[test]
    fn stale_block_selection_cannot_delete_a_replacement_with_the_same_block_id() {
        let mut app = App::sample(Local::now().date_naive());
        let tab = &mut app.data.notes[0].document.tabs[0];
        tab.body.clear();
        tab.insert_table(0..0, Table::new(1, 1));
        let old_key = app.content_key((0, 0, 0));
        assert!(app.delete_content(&[old_key]));
        app.data.notes[0].document.tabs[0].insert_table(0..0, Table::new(1, 1));
        let replacement = app.content_key((0, 0, 0));
        assert_eq!(old_key.pin(), replacement.pin());
        assert!(!app.delete_content(&[old_key, replacement]));
        assert_eq!(app.content_source(replacement), Some((0, 0, 0)));
    }

    #[test]
    fn collecting_multiple_tables_preserves_order_and_plain_text_boundaries() {
        let mut first = Table::new(1, 1);
        let mut second = Table::new(1, 1);
        first.cells[0][0].text = "첫 표".into();
        second.cells[0][0].text = "둘째 표".into();
        let (html, plain) = table_batch_formats([&first, &second].into_iter());
        assert!(html.find("첫 표") < html.find("둘째 표"));
        assert_eq!(html.matches("<table").count(), 2);
        assert_eq!(plain, "첫 표\r\n\r\n둘째 표\r\n");
    }

    #[test]
    fn gallery_selection_reuses_the_listing_and_filter_changes_recompute_it() {
        let mut app = App::sample(Local::now().date_naive());
        for (i, text) in ["하나", "둘"].iter().enumerate() {
            let mut table = Table::new(1, 1);
            table.cells[0][0].text = (*text).into();
            app.data.notes[i].document.tabs[0]
                .blocks
                .push(Block::Table(table));
        }
        let ctx = egui::Context::default();
        let all = app.content_listing(&ctx, false);
        assert_eq!(all.keys.len(), 2);
        app.content_selection
            .click(all.keys[0], &all.keys, egui::Modifiers::CTRL);
        assert!(Arc::ptr_eq(&all, &app.content_listing(&ctx, false)));
        app.extra.table_search = "둘".into();
        let filtered = app.content_listing(&ctx, false);
        assert_eq!(filtered.keys, all.keys[1..]);
    }
    #[test]
    fn table_export_preserves_variable_columns_and_escapes_html() {
        let mut table = Table::new(2, 4);
        table.cells[0][0].text = "<script>&\"".into();
        table.cells[1][3].text = "마지막 열".into();
        let html = table_html(&table);
        assert!(html.contains("&lt;script&gt;&amp;&quot;"));
        assert!(!html.contains("<script>"));
        assert!(table.tsv().contains("마지막 열"));
        let image = table_image(&table).unwrap();
        assert_eq!(image.width(), 721);
        assert!(image.height() >= 81);
    }
    #[test]
    fn collection_excludes_locked_deleted_and_hidden_project_sources() {
        let mut a = App::sample(Local::now().date_naive());
        for n in a.data.notes.iter_mut().take(4) {
            n.document.tabs[0]
                .blocks
                .push(Block::Table(Table::new(1, 1)));
        }
        a.data.notes[0].locked = true;
        a.data.notes[1].deleted = true;
        a.data.notes[2].project = Some(1);
        a.extra.hidden_projects.insert(1);
        assert_eq!(a.content_sources().tables, vec![(3, 0, 0)]);
    }
    #[test]
    fn batch_image_export_never_overwrites_an_existing_file() {
        let folder = std::env::temp_dir().join(format!(
            "ohmemojang-gallery-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&folder).unwrap();
        let first = write_unique_png(&folder, "test", b"first").unwrap();
        let second = write_unique_png(&folder, "test", b"second").unwrap();
        assert_ne!(first, second);
        assert_eq!(std::fs::read(&first).unwrap(), b"first");
        assert_eq!(std::fs::read(&second).unwrap(), b"second");
        std::fs::remove_file(first).unwrap();
        std::fs::remove_file(second).unwrap();
        std::fs::remove_dir(folder).unwrap();
    }
}
