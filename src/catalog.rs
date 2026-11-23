//! Folder organization, detailed note lists and the in-memory recycle bin.
use crate::trash_groups::{Group, PurgeKind, TrashItem};
use crate::*;
use egui::{Sense, Stroke};
use std::collections::HashMap;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::ops::Range;

#[path = "catalog_batch.rs"]
mod batch;
pub(crate) use batch::OrderScope;
use batch::{Action as BatchAction, DropTarget, Edit};
#[path = "catalog_projects.rs"]
mod projects;
use projects::ProjectBranch;
pub(crate) use projects::ProjectFolder;

pub(crate) const PROJECT_TAB: usize = 0;
pub(crate) const FOLDER_TAB: usize = 1;
pub(crate) const ALL_MEMOS_TAB: usize = 2;
const BOARD_TAB: usize = 3;

type FindHit = (usize, Option<(usize, Range<usize>)>);

fn catalog_list_focus() -> egui::Id {
    egui::Id::new("catalog_memo_list")
}

/// One memo in the "모두 찾기" results: its title and the first body line
/// containing the text, with the found text's byte range in each.
#[derive(Clone, Debug)]
struct FindResult {
    note: usize,
    title_hit: Option<Range<usize>>,
    line: String,
    line_hit: Option<Range<usize>>,
    count: usize,
}

/// Everything `catalog_replace_targets` depends on.
#[derive(Clone, PartialEq)]
struct ReplaceKey {
    changes: u64,
    notes: usize,
    find: String,
    scope: usize,
    projects: BTreeSet<usize>,
    selected: Option<usize>,
    active: usize,
    project: Option<usize>,
    filter: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct Folder {
    pub title: String,
    pub parent: Option<usize>,
    pub open: bool,
    pub deleted: bool,
}

#[derive(Default)]
pub struct Catalog {
    pub tab: usize,
    pub folders: Vec<Folder>,
    pub assignments: BTreeMap<usize, usize>,
    folder_revision: u64,
    pub selected: Option<usize>,
    pub selected_day: Option<NaiveDate>,
    pub selection: crate::selection::Selection<usize>,
    selection_order: Vec<usize>,
    pub selection_ui: bool,
    pub manual_order: BTreeMap<OrderScope, Vec<usize>>,
    order_revision: u64,
    undo: Vec<Edit>,
    redo: Vec<Edit>,
    batch_completion: Option<(Vec<usize>, bool)>,
    folder_drop_list: bool,
    folder_manual: bool,
    project_branch: Option<ProjectBranch>,
    project_expanded: HashSet<ProjectBranch>,
    project_folder_rename: Option<(usize, String, bool)>,
    project_show_completed: bool,
    pub project_preview_width: f32,
    pub project_folders: Vec<ProjectFolder>,
    pub project_assignments: BTreeMap<usize, usize>,
    batch_targets: Vec<(egui::Rect, DropTarget)>,
    hover_folder: Option<(usize, f64)>,
    pub folder_selected: Option<usize>,
    pub find_selection: Option<(usize, usize, Range<usize>)>,
    pub query: String,
    pub replace_open: bool,
    pub replace_find: String,
    pub replace_with: String,
    pub replace_scope: usize,
    /// Projects searched when the scope is "선택 프로젝트" (`NO_PROJECT`
    /// stands for memos without one).
    pub replace_projects: BTreeSet<usize>,
    /// Index of the current "찾기" match.
    replace_cursor: Option<usize>,
    /// "모두 찾기" results shown in place of the preview.
    pub results_open: bool,
    results: Vec<FindResult>,
    results_selected: BTreeSet<usize>,
    project: Option<usize>,
    sort: usize,
    recurring: usize,
    wiki_only: bool,
    rename: Option<usize>,
    rename_focus: bool,
    find_index: usize,
    find_focus: bool,
    replace_message: String,
    /// Memos the replace dialog works on, kept while nothing it depends on
    /// changed (the dialog is drawn every frame; the search reads every memo).
    replace_cache: Option<(ReplaceKey, Vec<usize>)>,
    /// Groups of the recycle bin that are opened.
    trash_open: std::collections::HashSet<Group>,
    trash_listing: Option<TrashListingCache>,
    drag_sources: Vec<(egui::Rect, FolderDrag)>,
    drag_targets: Vec<(egui::Rect, Option<usize>)>,
    dragging: Option<(FolderDrag, egui::Pos2)>,
    table_offset: f32,
    table_viewport_width: f32,
    table_width: f32,
    pub preview_width: f32,
    /// User-adjusted width of every table column (by table index). `0.`
    /// means "not customized, use the default".
    pub column_widths: [f32; COLUMN_COUNT],
    /// Which of the columns are shown (by table index).
    pub column_shown: ColumnShown,
}

/// Whether each table column is shown. The default is the usual table; 완료일,
/// 위키, 프로젝트 and 중요 are shown only when chosen.
#[derive(Clone, Copy)]
pub struct ColumnShown(pub [bool; COLUMN_COUNT]);

impl Default for ColumnShown {
    fn default() -> Self {
        Self([
            true, true, true, true, true, true, true, false, false, false, false, true,
        ])
    }
}

/// Columns of the table: 표시, 완료, 제목, 내용, 알람, 생성일, 일정, 완료일,
/// 위키, 프로젝트, 중요, 삭제.
pub(crate) const COLUMN_COUNT: usize = 12;
const COLUMN_TITLE: usize = 2;
const COLUMN_BODY: usize = 3;
const COLUMN_DELETE: usize = COLUMN_COUNT - 1;
/// Default widths. Title (2) and body (3) share the rest.
const COLUMN_WIDTHS: [f32; COLUMN_COUNT] =
    [30., 30., 0., 0., 96., 58., 86., 86., 44., 96., 44., 30.];
/// The narrowest table: below this the list scrolls horizontally.
const TABLE_WIDTH: f32 = 560.;
/// Table indices of the columns a user can show or hide, with their labels.
/// 표시, 제목 and 내용 are always shown.
pub(crate) const TOGGLE_COLUMNS: [(usize, &str); 9] = [
    (1, "완료"),
    (4, "알람"),
    (5, "생성일"),
    (6, "일정"),
    (7, "완료일"),
    (8, "위키"),
    (9, "프로젝트"),
    (10, "중요"),
    (COLUMN_DELETE, "삭제"),
];
const COLUMN_MIN_WIDTH: f32 = 24.;
const COLUMN_MAX_WIDTH: f32 = 300.;
const TITLE_MIN_WIDTH: f32 = 80.;
const TITLE_MAX_WIDTH: f32 = 800.;
const BODY_MIN_WIDTH: f32 = 60.;

#[derive(Clone, Copy, Hash)]
enum TableRow {
    Note(usize, Option<NaiveDate>),
    Tab(usize, usize),
    /// A deleted wiki tab or folder with its memos, in the recycle bin.
    Group(Group),
    /// A memo listed below its group.
    Child(usize, Group),
}

struct TrashListing {
    rows: Vec<TableRow>,
    all: Vec<TrashItem>,
    leaves: std::rc::Rc<[TrashItem]>,
    members: std::rc::Rc<HashMap<Group, Vec<usize>>>,
    count: usize,
}

struct TrashListingCache {
    revision: u64,
    open: std::collections::HashSet<Group>,
    listing: std::rc::Rc<TrashListing>,
}

enum TableCommand {
    Toggle(usize),
    Complete(usize, Option<NaiveDate>, bool),
    Reminder(usize),
    Open(usize, Option<NaiveDate>),
    Trash(usize),
    Restore(usize),
    Purge(usize),
    RestoreTab(usize, u64),
    PurgeTab(usize, u64),
    RestoreGroup(Group),
    ToggleGroup(Group),
    RestoreSelected,
    PurgeSelected,
}

fn trash_list_focus() -> egui::Id {
    egui::Id::new("trash_list_focus")
}

fn trash_selection_menu(ui: &mut egui::Ui, pending: &mut Vec<TableCommand>, selected: bool) {
    for (title, command) in [
        ("선택 복원", TableCommand::RestoreSelected),
        ("선택 영구삭제…", TableCommand::PurgeSelected),
    ] {
        if ui.add_enabled(selected, egui::Button::new(title)).clicked() {
            pending.push(command);
            ui.close_menu();
        }
    }
}

impl TableRow {
    fn trash_item(self, app: &App) -> TrashItem {
        match self {
            Self::Note(i, _) | Self::Child(i, _) => TrashItem::Memo(i),
            Self::Group(g) => TrashItem::Group(g),
            Self::Tab(i, t) => TrashItem::Tab(i, app.data.notes[i].document.trash[t].id),
        }
    }
}

fn column_shown(cat: &Catalog, column: usize) -> bool {
    cat.column_shown.0[column]
}

/// Widths of all columns for a table `total` wide: hidden columns are zero, a
/// width the user set overrides the default. The body (내용) takes what the
/// others leave, so it follows the width of the list pane; the title (제목)
/// takes a share of it until the user sets its width. The sum is more than
/// `total` when the columns need more room (the table then scrolls).
fn column_widths(cat: &Catalog, total: f32) -> [f32; COLUMN_COUNT] {
    let mut widths: [f32; COLUMN_COUNT] = std::array::from_fn(|i| {
        if !column_shown(cat, i) {
            0.
        } else if cat.column_widths[i] > 0. {
            cat.column_widths[i]
        } else {
            COLUMN_WIDTHS[i]
        }
    });
    let title_set = cat.column_widths[COLUMN_TITLE] > 0.;
    let mut fixed: f32 = widths.iter().sum::<f32>() - widths[COLUMN_TITLE] - widths[COLUMN_BODY];
    if title_set {
        fixed += widths[COLUMN_TITLE];
    }
    let flex = (total.max(TABLE_WIDTH) - fixed).max(0.);
    if title_set {
        widths[COLUMN_BODY] = flex.max(BODY_MIN_WIDTH);
    } else {
        widths[COLUMN_TITLE] = (flex * 0.38).max(110.);
        widths[COLUMN_BODY] = (flex - widths[COLUMN_TITLE]).max(0.);
    }
    widths
}

/// The width the table needs when the list pane is `pane` wide.
fn table_total(cat: &Catalog, pane: f32) -> f32 {
    column_widths(cat, pane).iter().sum::<f32>().max(pane)
}

/// Column rectangles in a row (or the header) `rect` wide, which is at least
/// as wide as `table_total`.
fn table_cells(rect: egui::Rect, cat: &Catalog) -> [egui::Rect; COLUMN_COUNT] {
    let widths = column_widths(cat, rect.width());
    let mut x = rect.left();
    std::array::from_fn(|i| {
        let r = egui::Rect::from_min_size(
            egui::pos2(x, rect.top()),
            Vec2::new(widths[i], rect.height()),
        );
        x += widths[i];
        r
    })
}

/// Moves the border between column `before` and column `column` (the left
/// edge of `column`) by `dx` and stores the new width. Columns after the body
/// are anchored on the right (the body takes what they leave), so the one
/// whose left edge is dragged changes; for the others the column before the
/// border changes.
fn drag_border(
    cat: &mut Catalog,
    widths: &[f32; COLUMN_COUNT],
    before: usize,
    column: usize,
    dx: f32,
) {
    let (target, width) = if column > COLUMN_BODY {
        (column, widths[column] - dx)
    } else {
        (before, widths[before] + dx)
    };
    let (min, max) = if target == COLUMN_TITLE {
        (TITLE_MIN_WIDTH, TITLE_MAX_WIDTH)
    } else {
        (COLUMN_MIN_WIDTH, COLUMN_MAX_WIDTH)
    };
    cat.column_widths[target] = width.clamp(min, max);
}

fn table_cell(ui: &mut egui::Ui, rect: egui::Rect, salt: impl std::hash::Hash) -> egui::Ui {
    let rect = rect.shrink2(Vec2::new(3., 2.));
    let mut cell = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(salt)
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    cell.set_clip_rect(ui.clip_rect().intersect(rect));
    cell.spacing_mut().item_spacing.x = 4.;
    cell
}

impl Catalog {
    pub fn focus_find(&mut self) {
        self.find_focus = true;
    }

    pub fn create_folder(&mut self) -> usize {
        let parent = self
            .folder_selected
            .filter(|&f| self.folders.get(f).is_some_and(|folder| !folder.deleted));
        let id = self.folders.len();
        self.folders.push(Folder {
            title: "새 폴더".into(),
            parent,
            open: true,
            deleted: false,
        });
        if let Some(p) = parent {
            self.folders[p].open = true;
        }
        // Keep the selected parent, so another click creates a sibling.
        self.rename = Some(id);
        self.rename_focus = true;
        id
    }

    pub fn move_folder(&mut self, folder: usize, parent: Option<usize>) -> bool {
        if self.folders.get(folder).is_none_or(|f| f.deleted) {
            return false;
        }
        let mut cursor = parent;
        let mut visited = BTreeSet::new();
        while let Some(p) = cursor {
            if p == folder || !visited.insert(p) {
                return false;
            }
            let Some(node) = self.folders.get(p).filter(|f| !f.deleted) else {
                return false;
            };
            cursor = node.parent;
        }
        self.folders[folder].parent = parent;
        if let Some(p) = parent {
            self.folders[p].open = true;
        }
        true
    }

    pub fn assign_notes(&mut self, notes: &[usize], folder: Option<usize>) {
        if folder.is_some_and(|f| self.folders.get(f).is_none_or(|f| f.deleted)) {
            return;
        }
        for &note in notes {
            if let Some(f) = folder {
                self.assignments.insert(note, f);
            } else {
                self.assignments.remove(&note);
            }
        }
        self.folder_revision = self.folder_revision.wrapping_add(1);
    }

    pub fn remove_folder(&mut self, f: usize) {
        let parent = self.folders[f].parent;
        self.folders[f].deleted = true;
        for folder in &mut self.folders {
            if folder.parent == Some(f) {
                folder.parent = parent;
            }
        }
        self.assignments.retain(|_, v| *v != f);
        self.folder_revision = self.folder_revision.wrapping_add(1);
        if self.folder_selected == Some(f) {
            self.folder_selected = None;
        }
    }
}

#[derive(Clone)]
enum FolderDrag {
    Notes(Vec<usize>),
    Folder(usize),
}

#[derive(Clone, Copy)]
enum Action {
    Eye,
    Bell,
    Trash,
    Restore,
}

fn action(ui: &mut egui::Ui, kind: Action, tooltip: &str, selected: bool) -> egui::Response {
    let (rect, r) = ui.allocate_exact_size(Vec2::splat(26.), Sense::click());
    r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tooltip));
    if r.hovered() || r.has_focus() || selected {
        ui.painter().rect_filled(
            rect,
            5.,
            if selected {
                Color32::from_rgba_unmultiplied(184, 72, 53, 18)
            } else {
                ui.visuals().widgets.hovered.weak_bg_fill
            },
        );
    }
    let c = if selected { ACCENT } else { MUTED };
    let icon = match kind {
        Action::Eye => crate::icons::Icon::Eye,
        Action::Bell => crate::icons::Icon::Bell,
        Action::Trash => crate::icons::Icon::Trash,
        Action::Restore => crate::icons::Icon::Restore2,
    };
    crate::icons::paint_icon(ui.painter(), rect.shrink(4.), icon, c);
    r.on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(tooltip)
}

impl App {
    pub fn catalog_ui(&mut self, ui: &mut egui::Ui) {
        self.catalog.selection_ui = true;
        self.catalog.drag_sources.clear();
        self.catalog.drag_targets.clear();
        self.catalog.batch_targets.clear();
        let previous_tab = self.catalog.tab;
        ui::tabs(
            ui,
            &mut self.catalog.tab,
            &["프로젝트 보기", "폴더 보기", "전체 메모 보기", "관계 보드"],
        );
        ui.add_space(12.);
        if self.catalog.tab != previous_tab {
            self.catalog.selection.clear();
            self.catalog.dragging = None;
        }
        if self.catalog.tab == BOARD_TAB {
            self.board(ui);
            return;
        }
        if self.catalog.tab == FOLDER_TAB {
            self.catalog_folders(ui);
        } else if self.catalog.tab == PROJECT_TAB {
            self.catalog_projects(ui);
        } else {
            self.catalog_controls(ui);
            let mut cache = std::mem::take(&mut self.lists);
            let key = crate::lists::key((
                self.data.notes.revision(),
                self.catalog.project,
                self.catalog.sort,
                self.catalog.recurring,
                self.catalog.wiki_only,
                self.catalog.order_revision,
                self.catalog_project_visibility_key(),
            ));
            let ids = cache.get_stable("catalog", key, || self.catalog_filtered_ids());
            self.lists = cache;
            let ids = (*ids).clone();
            let hits = self.catalog_find_bar(ui, &ids);
            let ids = if self.catalog.query.is_empty() {
                ids
            } else {
                let matches: HashSet<_> = hits.iter().map(|(id, _)| *id).collect();
                ids.into_iter().filter(|id| matches.contains(id)).collect()
            };
            self.catalog_selection_bar(ui, ids.clone());
            let rows: Vec<_> = ids
                .into_iter()
                .map(|i| (i, self.data.notes[i].date))
                .collect();
            self.catalog_split_list(ui, &rows, false, false);
        }
        self.catalog_drag_input(ui.ctx());
        self.catalog_completion_dialog(ui.ctx());
        self.catalog_replace_window(ui.ctx());
    }

    pub fn detail_workspace(&mut self, ui: &mut egui::Ui, mode: usize) {
        self.catalog.selection_ui = false;
        self.filters(ui);
        ui.add_space(10.);
        let today = Local::now().date_naive();
        let mut cache = std::mem::take(&mut self.lists);
        let key = crate::lists::key((mode, today, self.data.notes.len()));
        let rows = cache.get("detail", key, || self.detail_rows(mode, today));
        self.lists = cache;
        let ids: Vec<_> = rows.iter().map(|&(i, _)| i).collect();
        self.catalog_find_bar(ui, &ids);
        // Title and body clicks only show the memo in the preview; the eye
        // column opens it as a sticker.
        self.catalog_split_list(ui, &rows, false, true);
        self.catalog_replace_window(ui.ctx());
    }

    /// Rows of the detail list (each day of a multi-day memo is a row).
    fn detail_rows(&self, mode: usize, today: NaiveDate) -> Vec<(usize, Option<NaiveDate>)> {
        let mut rows = vec![];
        for (i, n) in self.data.notes.iter().enumerate() {
            if !self.matches_search(i)
                || self.is_guide(i)
                || (!self.recurring && n.recurring)
                || !crate::refinements::project_match(self.project_filter, n.project)
                || (self.desktop.hide_locked && n.locked)
            {
                continue;
            }
            let days: Vec<_> = if n.date.is_some() {
                n.days().into_iter().map(Some).collect()
            } else {
                vec![None]
            };
            for d in days {
                if (self.desktop.hide_completed && !self.extra.show_done && n.done_on(d))
                    || !match mode {
                        2 => d == Some(today),
                        3 => d.is_some_and(|d| d > today),
                        _ => true,
                    }
                {
                    continue;
                }
                rows.push((i, d));
            }
        }
        rows.sort_by_key(|&(i, d)| (d.unwrap_or(NaiveDate::MAX), self.data.notes[i].time, i));
        rows
    }

    fn catalog_filtered_ids(&self) -> Vec<usize> {
        let mut ids: Vec<_> = self
            .data
            .notes
            .iter()
            .enumerate()
            .filter(|(i, n)| {
                self.visible(*i)
                    && crate::refinements::project_match(self.catalog.project, n.project)
                    && (!self.catalog.wiki_only || n.wiki)
                    && match self.catalog.recurring {
                        1 => !n.recurring,
                        2 => n.recurring,
                        _ => true,
                    }
            })
            .map(|(i, _)| i)
            .collect();
        ids.sort_by_key(|&i| {
            let n = &self.data.notes[i];
            if self.catalog.sort == 1 {
                (n.date.unwrap_or(NaiveDate::MAX), n.time, i)
            } else {
                (n.created.date(), n.created.time(), i)
            }
        });
        if self.catalog.sort == 2 {
            self.catalog_ordered(OrderScope::All, &mut ids);
        }
        ids
    }

    fn catalog_controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            egui::ComboBox::from_id_salt("catalog_project")
                .selected_text(crate::refinements::project_label(
                    self.catalog.project,
                    &self.data.projects,
                ))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.catalog.project, None, "모든 프로젝트");
                    ui.selectable_value(
                        &mut self.catalog.project,
                        Some(NO_PROJECT),
                        "프로젝트 없음",
                    );
                    for (p, name) in self.data.projects.iter().enumerate() {
                        if !self.extra.hidden_projects.contains(&p)
                            && !self.extra.deleted_projects.contains(&p)
                        {
                            ui.selectable_value(&mut self.catalog.project, Some(p), name);
                        }
                    }
                });
            egui::ComboBox::from_id_salt("catalog_sort")
                .selected_text(["생성일자 순", "일정 순", "수동 정렬"][self.catalog.sort])
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.catalog.sort, 0, "생성일자 순");
                    ui.selectable_value(&mut self.catalog.sort, 1, "일정 순");
                    ui.selectable_value(&mut self.catalog.sort, 2, "수동 정렬");
                });
            egui::ComboBox::from_id_salt("catalog_recurrence")
                .selected_text(
                    ["정기 일정 포함", "정기 일정 제외", "정기 일정만"][self.catalog.recurring],
                )
                .show_ui(ui, |ui| {
                    for (i, text) in ["정기 일정 포함", "정기 일정 제외", "정기 일정만"]
                        .iter()
                        .enumerate()
                    {
                        ui.selectable_value(&mut self.catalog.recurring, i, *text);
                    }
                });
            ui.checkbox(&mut self.catalog.wiki_only, "위키만 보기");
            self.column_picker(ui);
        });
        ui.add_space(8.);
    }

    /// The "열 표시" column show/hide picker shared by every screen that
    /// renders the adjustable-column table (`catalog_table`): the catalog's
    /// own "전체 메모 보기" controls above, and `views::filters` for the
    /// 할일/일정 list screens that reach the same table via
    /// `detail_workspace`.
    pub(crate) fn column_picker(&mut self, ui: &mut egui::Ui) {
        egui::ComboBox::from_id_salt("catalog_column_picker")
            .selected_text("열 표시")
            .show_ui(ui, |ui| self.column_choices(ui));
    }
    /// The checkboxes that show or hide the adjustable columns: the dropdown's
    /// content, and the menu of a right click on the table header.
    pub(crate) fn column_choices(&mut self, ui: &mut egui::Ui) {
        crate::ui::caption(ui, "표시할 열");
        for &(column, label) in &TOGGLE_COLUMNS {
            ui.checkbox(&mut self.catalog.column_shown.0[column], label);
        }
    }

    fn catalog_find_bar(&mut self, ui: &mut egui::Ui, ids: &[usize]) -> std::rc::Rc<Vec<FindHit>> {
        if ui
            .ctx()
            .input(|i| i.modifiers.command && !i.modifiers.shift && i.key_pressed(egui::Key::H))
        {
            self.catalog.replace_open = true;
        }
        let mut direction = 0;
        let mut reset = false;
        let mut status = None;
        let status_slot = ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.;
            let r = ui::search_field(
                ui,
                &mut self.catalog.query,
                "catalog_find",
                "목록에서 찾기",
                240.,
            );
            if self.catalog.find_focus {
                r.request_focus();
                self.catalog.find_focus = false;
            }
            reset = r.changed();
            if crate::icons::small_icon(ui, crate::icons::Icon::ChevronLeft, "이전 찾기").clicked()
            {
                direction = -1;
            }
            if crate::icons::small_icon(ui, crate::icons::Icon::ChevronRight, "다음 찾기  Enter")
                .clicked()
                || (r.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
            {
                direction = 1;
            }
            let slot = ui.cursor().min;
            ui.add_space(64.);
            if ui.button("찾아 바꾸기").clicked() {
                self.catalog.replace_open = true;
                if !self.catalog.query.is_empty() {
                    self.catalog.replace_find = self.catalog.query.clone();
                }
                ui::raise(ui.ctx(), "catalog_replace");
            }
            slot
        });
        let slot = status_slot.inner;
        let search_ids: &[usize] = if self.catalog.query.is_empty() {
            &[]
        } else {
            ids
        };
        let key = crate::lists::key((search_ids, &self.catalog.query, self.data.notes.revision()));
        let mut cache = std::mem::take(&mut self.lists);
        let hits = cache.get_stable("catalog_find_hits", key, || {
            self.catalog_find_hits(search_ids)
        });
        self.lists = cache;
        if reset {
            self.catalog.find_index = 0;
        }
        if !hits.is_empty() {
            let len = hits.len();
            if direction != 0 {
                self.catalog.find_index = if direction > 0 {
                    (self.catalog.find_index + 1) % len
                } else {
                    (self.catalog.find_index + len - 1) % len
                };
            }
            self.catalog.find_index %= len;
            if reset || direction != 0 {
                let (id, location) = hits[self.catalog.find_index].clone();
                self.catalog.selected = Some(id);
                // Typing a query must keep focus in the search field. Only an
                // explicit next/previous action moves the editor's caret.
                self.catalog.find_selection = if direction != 0 {
                    location.map(|(tab, range)| (id, tab, range))
                } else {
                    None
                };
            }
            status = Some(format!("{} / {}", self.catalog.find_index + 1, len));
        } else if !self.catalog.query.is_empty() {
            status = Some("결과 없음".into());
        }
        if let Some(text) = status {
            ui.painter().text(
                slot + Vec2::new(6., 13.),
                egui::Align2::LEFT_CENTER,
                text,
                egui::FontId::proportional(12.),
                ui::t(ui).muted,
            );
        }
        ui.add_space(8.);
        hits
    }

    fn catalog_find_hits(&self, ids: &[usize]) -> Vec<FindHit> {
        if self.catalog.query.is_empty() {
            return vec![];
        }
        let mut seen = BTreeSet::new();
        let mut hits = vec![];
        for &i in ids {
            if !seen.insert(i) {
                continue;
            }
            let n = &self.data.notes[i];
            if n.title
                .to_lowercase()
                .contains(&self.catalog.query.to_lowercase())
            {
                hits.push((i, None));
            }
            if !n.locked {
                for (tab, body) in std::iter::once((0, n.body.as_str())).chain(
                    n.document
                        .tabs
                        .iter()
                        .enumerate()
                        .skip(1)
                        .map(|(tab, t)| (tab, t.body.as_str())),
                ) {
                    for range in find_ranges(body, &self.catalog.query) {
                        hits.push((i, Some((tab, range))));
                    }
                }
            }
        }
        hits
    }

    fn catalog_split_list(
        &mut self,
        ui: &mut egui::Ui,
        rows: &[(usize, Option<NaiveDate>)],
        open_editor: bool,
        date_groups: bool,
    ) {
        if self
            .catalog
            .selected
            .is_none_or(|s| !rows.iter().any(|r| r.0 == s))
        {
            self.catalog.selected = rows.first().map(|r| r.0);
        }
        if !ui.ctx().wants_keyboard_input() && !rows.is_empty() {
            let direction = ui.input(|i| {
                if i.key_pressed(egui::Key::ArrowDown) {
                    1
                } else if i.key_pressed(egui::Key::ArrowUp) {
                    -1
                } else {
                    0
                }
            });
            if direction != 0 {
                let at = rows
                    .iter()
                    .position(|r| Some(r.0) == self.catalog.selected)
                    .unwrap_or(0);
                let next = (at as isize + direction).clamp(0, rows.len() as isize - 1) as usize;
                self.catalog.selected = Some(rows[next].0);
                self.catalog.selected_day = rows[next].1;
            }
        }
        let height = panel_height(ui);
        let rect =
            egui::Rect::from_min_size(ui.cursor().min, Vec2::new(ui.available_width(), height));
        if self.catalog.preview_width <= 0. {
            self.catalog.preview_width = 300.;
        }
        // The list keeps at least its narrowest table; widening the preview
        // shrinks the list's title and body columns.
        let preview_max = (rect.width() - 30. - TABLE_WIDTH).max(240.);
        let preview_width = self.catalog.preview_width.clamp(240., preview_max);
        let list_width = (rect.width() - preview_width - 16.).max(180.);
        self.catalog.table_width = TABLE_WIDTH.max(list_width - 14.);
        let table_rows: Vec<_> = rows.iter().map(|&(i, d)| TableRow::Note(i, d)).collect();
        ui.allocate_new_ui(
            egui::UiBuilder::new()
                .max_rect(rect)
                .layout(egui::Layout::top_down(egui::Align::LEFT)),
            |ui| {
                ui.set_min_size(rect.size());
                let list_rect = egui::Rect::from_min_size(rect.min, Vec2::new(list_width, height));
                let preview_rect = egui::Rect::from_min_size(
                    egui::pos2(list_rect.right() + 16., rect.top()),
                    Vec2::new(preview_width, height),
                );
                let mut list = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt("detail_table_pane")
                        .max_rect(list_rect)
                        .layout(egui::Layout::top_down(egui::Align::LEFT)),
                );
                list.set_clip_rect(ui.clip_rect().intersect(list_rect));
                self.catalog_table(
                    &mut list,
                    &table_rows,
                    height,
                    open_editor,
                    false,
                    date_groups,
                );
                let handle = egui::Rect::from_min_max(
                    egui::pos2(list_rect.right() + 2., rect.top()),
                    egui::pos2(list_rect.right() + 14., rect.bottom()),
                );
                let drag = ui
                    .interact(handle, ui.id().with("preview_splitter"), Sense::drag())
                    .on_hover_cursor(egui::CursorIcon::ResizeHorizontal)
                    .on_hover_text("드래그해서 미리보기 너비 조절");
                if drag.hovered() || drag.dragged() {
                    ui.painter().line_segment(
                        [handle.center_top(), handle.center_bottom()],
                        Stroke::new(2.0_f32, ACCENT.gamma_multiply(0.6)),
                    );
                }
                if drag.dragged() {
                    self.catalog.preview_width =
                        (preview_width - drag.drag_delta().x).clamp(240., preview_max);
                }
                if drag.double_clicked() {
                    self.catalog.preview_width = 300.;
                }
                let mut preview = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt("detail_preview_pane")
                        .max_rect(preview_rect)
                        .layout(egui::Layout::top_down(egui::Align::LEFT)),
                );
                preview.set_clip_rect(ui.clip_rect().intersect(preview_rect));
                self.catalog_preview(&mut preview, height);
            },
        );
    }

    fn catalog_table(
        &mut self,
        ui: &mut egui::Ui,
        rows: &[TableRow],
        height: f32,
        open_editor: bool,
        trash: bool,
        date_groups: bool,
    ) {
        // The list pane's width, widened when the chosen columns need more.
        let pane = self.catalog.table_width.max(TABLE_WIDTH);
        self.catalog.table_width = table_total(&self.catalog, pane);
        let header_width = if self.catalog.table_viewport_width > 0. {
            self.catalog.table_viewport_width.min(ui.available_width())
        } else {
            ui.available_width()
        };
        let offset = self.catalog.table_offset.clamp(
            0.,
            (self.catalog.table_width.max(TABLE_WIDTH) - header_width).max(0.),
        );
        egui::ScrollArea::horizontal()
            .id_salt(("table_header", trash))
            .horizontal_scroll_offset(offset)
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
            .auto_shrink([false, false])
            .max_width(header_width)
            .max_height(36.)
            .show(ui, |ui| {
                let table_width = self.catalog.table_width.max(TABLE_WIDTH);
                ui.set_min_width(table_width);
                let (rect, _) = ui.allocate_exact_size(Vec2::new(table_width, 34.), Sense::hover());
                ui.painter().line_segment(
                    [rect.left_bottom(), rect.right_bottom()],
                    Stroke::new(1_f32, ui::t(ui).line),
                );
                // Right-click a column title: choose the columns to show. Made
                // before the titles and resize handles, which stay on top of it.
                ui.interact(rect, ui.id().with(("header_menu", trash)), Sense::click())
                    .context_menu(|ui| self.column_choices(ui));
                let cells = table_cells(rect, &self.catalog);
                for (col, title) in [
                    if trash && self.desktop.trash_selection.active {
                        "선택"
                    } else if trash {
                        "복원"
                    } else if self.catalog_selecting() {
                        "선택"
                    } else {
                        "표시"
                    },
                    "완료",
                    "제목",
                    "내용",
                    "알람",
                    "생성일",
                    "일정",
                    "완료일",
                    "위키",
                    "프로젝트",
                    "중요",
                    "삭제",
                ]
                .iter()
                .enumerate()
                {
                    let mut cell = table_cell(ui, cells[col], ("header", col));
                    cell.add(
                        egui::Label::new(RichText::new(*title).size(12.).color(MUTED).strong())
                            .truncate(),
                    );
                }
                // A grip on the left edge of every shown column but the first.
                let widths = column_widths(&self.catalog, rect.width());
                let mut before = 0;
                for idx in 1..COLUMN_COUNT {
                    if widths[idx] <= 0. {
                        continue;
                    }
                    let previous = before;
                    before = idx;
                    let handle = egui::Rect::from_min_max(
                        egui::pos2(cells[idx].left() - 3., rect.top()),
                        egui::pos2(cells[idx].left() + 3., rect.bottom()),
                    );
                    let drag = ui
                        .interact(handle, ui.id().with(("column_resize", idx)), Sense::drag())
                        .on_hover_cursor(egui::CursorIcon::ResizeHorizontal)
                        .on_hover_text("드래그해서 열 너비 조절 · 더블클릭으로 기본값");
                    if drag.hovered() || drag.dragged() {
                        ui.painter().line_segment(
                            [handle.center_top(), handle.center_bottom()],
                            Stroke::new(2.0_f32, ACCENT.gamma_multiply(0.6)),
                        );
                    }
                    if drag.dragged() {
                        drag_border(
                            &mut self.catalog,
                            &widths,
                            previous,
                            idx,
                            drag.drag_delta().x,
                        );
                    }
                    if drag.double_clicked() {
                        let reset = if idx > COLUMN_BODY { idx } else { previous };
                        self.catalog.column_widths[reset] = 0.;
                    }
                }
            });
        let mut pending = vec![];
        let scroll = egui::ScrollArea::both()
            .id_salt(("detail_table_body", trash))
            .horizontal_scroll_offset(offset)
            .auto_shrink([false, false])
            .max_height((height - 44.).max(100.));
        let body = if trash || (!date_groups && !rows.is_empty()) {
            scroll.show_rows(ui, self.catalog_row_height(), rows.len(), |ui, range| {
                if !trash {
                    self.catalog_drag_scroll(ui);
                }
                ui.set_min_width(self.catalog.table_width.max(TABLE_WIDTH));
                for at in range {
                    let row = rows[at];
                    let item = row.trash_item(self);
                    ui.push_id((row, item), |ui| {
                        if let TableRow::Group(g) = row {
                            self.catalog_group_row(ui, g, &mut pending);
                        } else {
                            self.catalog_table_row(ui, row, at, open_editor, trash, &mut pending);
                        }
                    });
                }
            })
        } else {
            scroll.show(ui, |ui| {
                if !date_groups && !trash {
                    self.catalog_drag_scroll(ui);
                }
                ui.set_min_width(self.catalog.table_width.max(TABLE_WIDTH));
                if rows.is_empty() {
                    ui.add_space(24.);
                    ui.label(RichText::new("표시할 메모가 없습니다").color(MUTED));
                }
                let mut previous = Some(NaiveDate::MIN);
                for (at, &row) in rows.iter().enumerate() {
                    if let TableRow::Note(_, day) = row {
                        if date_groups && previous != day {
                            let (rect, _) = ui.allocate_exact_size(
                                Vec2::new(self.catalog.table_width.max(TABLE_WIDTH), 28.),
                                Sense::hover(),
                            );
                            ui.painter().text(
                                rect.left_center() + Vec2::new(7., 0.),
                                egui::Align2::LEFT_CENTER,
                                day.map(|d| d.format("%m월 %d일").to_string())
                                    .unwrap_or("일정 없는 메모".into()),
                                egui::FontId::proportional(12.),
                                MUTED,
                            );
                            previous = day;
                        }
                    }
                    let selected =
                        matches!(row, TableRow::Note(i, _) if self.catalog.selected == Some(i));
                    crate::lists::lazy_row(ui, row, 36., selected, |ui| {
                        self.catalog_table_row(ui, row, at, open_editor, false, &mut pending)
                    });
                }
            })
        };
        self.catalog.table_offset = body.state.offset.x;
        self.catalog.table_viewport_width = body.inner_rect.width();
        for command in pending {
            match command {
                TableCommand::Toggle(i) => self.toggle_sticker(i),
                TableCommand::Complete(i, d, done) => {
                    self.done_at(i, d, done);
                    self.extra.scope_owner = ui.ctx().viewport_id();
                }
                TableCommand::Reminder(i) => self.open_reminder(i),
                TableCommand::Open(i, d) => self.open_note_on(i, d),
                TableCommand::Trash(i) => self.trash_note(i),
                TableCommand::Restore(i) => self.restore_note(i),
                TableCommand::Purge(i) => self.request_purge(i),
                TableCommand::RestoreTab(i, t) => {
                    self.catalog_restore_tab(i, t);
                }
                TableCommand::PurgeTab(i, t) => {
                    self.request_trash_purge(PurgeKind::Tab, Some(TrashItem::Tab(i, t)));
                }
                TableCommand::RestoreGroup(g) => self.restore_group(g),
                TableCommand::ToggleGroup(g) => {
                    if !self.catalog.trash_open.remove(&g) {
                        self.catalog.trash_open.insert(g);
                    }
                }
                TableCommand::RestoreSelected => self.restore_trash_items(),
                TableCommand::PurgeSelected => self.request_trash_purge(PurgeKind::Selection, None),
            }
        }
    }

    fn catalog_row_height(&self) -> f32 {
        match self.desktop.row_spacing {
            0 => 34.,
            2 => 52.,
            _ => 42.,
        }
    }

    fn catalog_table_row(
        &mut self,
        ui: &mut egui::Ui,
        row: TableRow,
        at: usize,
        open_editor: bool,
        trash: bool,
        pending: &mut Vec<TableCommand>,
    ) {
        let (i, day, tab) = match row {
            TableRow::Note(i, d) => (i, d, None),
            TableRow::Tab(i, t) => (i, self.data.notes[i].date, Some(t)),
            TableRow::Child(i, _) => (i, self.data.notes[i].date, None),
            TableRow::Group(_) => return,
        };
        let n = &self.data.notes[i];
        let title = if let Some(t) = tab {
            let Some(t) = n.document.trash.get(t) else {
                return;
            };
            format!("{} - {}", n.title, t.title)
        } else if let TableRow::Child(_, g) = row {
            format!("      {}", self.group_member_title(g, i))
        } else if trash {
            self.bin_prefix(i).unwrap_or_else(|| n.title.clone())
        } else {
            n.title.clone()
        };
        let n = &self.data.notes[i];
        let body = if n.locked {
            "잠긴 메모".into()
        } else if let Some(t) = tab {
            n.document.trash[t].body.replace('\n', " ")
        } else {
            n.body.replace('\n', " ")
        };
        let reminder = n.reminder.map(|t| t.format("%m/%d %H:%M").to_string());
        let created = n.created.format("%y%m%d").to_string();
        let schedule = day
            .map(|d| format!("{} {}", d.format("%m/%d"), n.time.format("%H:%M")))
            .unwrap_or("—".into());
        let completed = day
            .and_then(|d| n.occurrence_completed.get(&d).copied())
            .or(n.completed)
            .filter(|_| n.done_on(day))
            .map(|t| t.format("%m/%d %H:%M").to_string())
            .unwrap_or("—".into());
        let wiki = n.wiki;
        let starred = n.starred;
        let project = n
            .project
            .and_then(|p| self.data.projects.get(p))
            .cloned()
            .unwrap_or("—".into());
        let mut done = n.done_on(day);
        let locked = n.locked;
        let memo_selecting = !trash && self.catalog_selecting();
        let trash_item = trash.then(|| row.trash_item(self));
        let trash_selecting = trash && self.desktop.trash_selection.active;
        let selected = if let Some(item) = trash_item {
            if trash_selecting {
                self.desktop.trash_selection.items.contains(&item)
            } else {
                self.desktop.trash_cursor == Some(item)
            }
        } else if memo_selecting {
            self.catalog.selection.items.contains(&i)
        } else {
            self.catalog.selected == Some(i)
        };
        let height = self.catalog_row_height();
        ui.push_id(("table_row", i, day, trash_item), |ui| {
            let (rect, response) = ui.allocate_exact_size(
                Vec2::new(self.catalog.table_width.max(TABLE_WIDTH), height),
                if trash {
                    Sense::click()
                } else {
                    Sense::hover()
                },
            );
            let mut trash_clicked = trash && response.clicked();
            let fill = if selected {
                ui.visuals().selection.bg_fill
            } else if ui.rect_contains_pointer(rect) {
                ui::t(ui).hover.gamma_multiply(0.6)
            } else {
                Color32::TRANSPARENT
            };
            let _ = at;
            ui.painter().rect_filled(rect, 6., fill);
            ui.painter().line_segment(
                [rect.left_bottom(), rect.right_bottom()],
                Stroke::new(
                    1_f32,
                    ui.visuals()
                        .widgets
                        .noninteractive
                        .bg_stroke
                        .color
                        .gamma_multiply(0.18),
                ),
            );
            if !trash
                && self.catalog.selection_ui
                && self.catalog.tab == ALL_MEMOS_TAB
                && self.catalog.sort == 2
            {
                self.catalog_order_drop(ui, rect, OrderScope::All, i);
            }
            let cells = table_cells(rect, &self.catalog);
            {
                let mut c = table_cell(ui, cells[0], "eye");
                if let Some(item) = trash_item {
                    if trash_selecting {
                        if self.desktop.trash_selection.checkbox(
                            &mut c,
                            item,
                            &self.desktop.trash_leaf_order,
                        ) {
                            c.ctx().memory_mut(|m| m.request_focus(trash_list_focus()));
                            trash_clicked = false;
                        }
                    } else if action(&mut c, Action::Restore, "복원하기", false)
                        .on_hover_text("복원하기")
                        .clicked()
                    {
                        pending.push(if let TrashItem::Tab(_, t) = item {
                            TableCommand::RestoreTab(i, t)
                        } else {
                            TableCommand::Restore(i)
                        });
                    }
                } else {
                    if memo_selecting {
                        if self
                            .catalog
                            .selection
                            .checkbox(&mut c, i, &self.catalog.selection_order)
                        {
                            c.ctx()
                                .memory_mut(|m| m.request_focus(catalog_list_focus()));
                        }
                    } else if action(
                        &mut c,
                        Action::Eye,
                        "스티커 표시 / 숨기기",
                        self.studio.windows.get(&i).is_some_and(|w| w.open),
                    )
                    .clicked()
                    {
                        pending.push(TableCommand::Toggle(i));
                    }
                }
            }
            {
                let mut c = table_cell(ui, cells[1], "done");
                if c.add_enabled(!trash && !memo_selecting, |ui: &mut egui::Ui| {
                    ui::round_check(ui, &mut done, None)
                })
                .on_hover_text("완료 상태")
                .changed()
                {
                    pending.push(TableCommand::Complete(i, day, done));
                }
            }
            {
                let mut c = table_cell(ui, cells[2], "title");
                let mut text = RichText::new(&title);
                if done {
                    text = text.strikethrough().color(MUTED);
                }
                let r = c
                    .add(
                        egui::Label::new(text)
                            .truncate()
                            .sense(Sense::click_and_drag()),
                    )
                    .on_hover_text(&title);
                if tab.is_none() && !trash {
                    self.catalog_note_response(&mut c, &r, i, day, open_editor);
                } else if r.clicked() {
                    trash_clicked = true;
                }
                if trash {
                    r.context_menu(|ui| {
                        if trash_selecting {
                            trash_selection_menu(
                                ui,
                                pending,
                                !self.desktop.trash_selection.items.is_empty(),
                            );
                        } else if ui.button("복구").clicked() {
                            pending.push(if let Some(TrashItem::Tab(_, t)) = trash_item {
                                TableCommand::RestoreTab(i, t)
                            } else {
                                TableCommand::Restore(i)
                            });
                            ui.close_menu();
                        }
                    });
                }
            }
            {
                let mut c = table_cell(ui, cells[3], "body");
                let r = c.add(
                    egui::Label::new(RichText::new(shorten(&body, 100)).color(MUTED))
                        .truncate()
                        .sense(Sense::click_and_drag()),
                );
                let r = if locked {
                    r.on_hover_text("잠금 해제 후 내용을 확인할 수 있습니다")
                } else {
                    r.on_hover_text(&body)
                };
                if tab.is_none() && !trash {
                    self.catalog_note_response(&mut c, &r, i, day, open_editor);
                } else if r.clicked() {
                    trash_clicked = true;
                }
            }
            {
                let mut c = table_cell(ui, cells[4], "reminder");
                if !trash {
                    let has = reminder.is_some();
                    if action(&mut c, Action::Bell, "알람 설정", has).clicked() {
                        pending.push(TableCommand::Reminder(i));
                    }
                    if let Some(text) = reminder {
                        if c.add(
                            egui::Label::new(RichText::new(text).size(12.).color(if has {
                                ACCENT
                            } else {
                                MUTED
                            }))
                            .truncate()
                            .sense(Sense::click()),
                        )
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                        {
                            pending.push(TableCommand::Reminder(i));
                        }
                    }
                } else {
                    c.add(
                        egui::Label::new(
                            RichText::new(reminder.unwrap_or("—".into()))
                                .size(12.)
                                .color(MUTED),
                        )
                        .truncate(),
                    );
                }
            }
            {
                let mut c = table_cell(ui, cells[5], "created");
                c.add(egui::Label::new(RichText::new(created).size(12.).color(MUTED)).truncate());
            }
            {
                let mut c = table_cell(ui, cells[6], "schedule");
                let r = c.add(
                    egui::Label::new(RichText::new(schedule).size(12.).color(MUTED))
                        .truncate()
                        .sense(if trash {
                            Sense::hover()
                        } else {
                            Sense::click()
                        }),
                );
                if !trash
                    && r.on_hover_text("일정 편집")
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                {
                    pending.push(TableCommand::Open(i, day));
                }
            }
            {
                let mut c = table_cell(ui, cells[7], "completed");
                c.add(egui::Label::new(RichText::new(completed).size(12.).color(MUTED)).truncate());
            }
            {
                let mut c = table_cell(ui, cells[8], "wiki");
                if wiki {
                    c.add(
                        egui::Label::new(RichText::new("위키").size(12.).color(ACCENT)).truncate(),
                    )
                    .on_hover_text("위키에 보관된 메모");
                }
            }
            {
                let mut c = table_cell(ui, cells[9], "project");
                c.add(egui::Label::new(RichText::new(project).size(12.).color(MUTED)).truncate());
            }
            {
                let mut c = table_cell(ui, cells[10], "starred");
                if starred {
                    c.add(egui::Label::new(
                        RichText::new("★").color(crate::studio::STAR),
                    ))
                    .on_hover_text("중요 메모");
                }
            }
            {
                let mut c = table_cell(ui, cells[COLUMN_DELETE], "delete");
                if !trash_selecting
                    && action(
                        &mut c,
                        Action::Trash,
                        if trash {
                            "영구 삭제"
                        } else {
                            "휴지통으로 이동"
                        },
                        false,
                    )
                    .clicked()
                {
                    pending.push(if let Some(TrashItem::Tab(_, t)) = trash_item {
                        TableCommand::PurgeTab(i, t)
                    } else if trash {
                        TableCommand::Purge(i)
                    } else {
                        TableCommand::Trash(i)
                    });
                }
            }
            if let Some(item) = trash_item.filter(|_| trash_clicked) {
                self.catalog_trash_click(ui, item);
            }
            if selected
                && !ui.ctx().wants_keyboard_input()
                && ui.input(|i| {
                    i.key_pressed(egui::Key::ArrowDown) || i.key_pressed(egui::Key::ArrowUp)
                })
            {
                let target = egui::Rect::from_min_max(
                    egui::pos2(ui.clip_rect().left() + 2., rect.top()),
                    egui::pos2(ui.clip_rect().left() + 3., rect.bottom()),
                );
                ui.scroll_to_rect(target, Some(egui::Align::Center));
            }
        });
    }

    /// A deleted wiki tab or folder in the bin: "이름/", opened like a folder
    /// to list its memos. Restoring it restores everything in it.
    fn catalog_group_row(&mut self, ui: &mut egui::Ui, g: Group, pending: &mut Vec<TableCommand>) {
        let name = self.group_name(g);
        let count = self.desktop.trash_group_members.get(&g).map_or(0, Vec::len);
        let open = self.catalog.trash_open.contains(&g);
        let item = TrashItem::Group(g);
        let selecting = self.desktop.trash_selection.active;
        let selected = self.desktop.trash_selection.items.contains(&item);
        let height = self.catalog_row_height();
        ui.push_id(("trash_group", g), |ui| {
            let (rect, row) = ui.allocate_exact_size(
                Vec2::new(self.catalog.table_width.max(TABLE_WIDTH), height),
                Sense::click(),
            );
            let fill = if selected {
                ui.visuals().selection.bg_fill
            } else if row.hovered() {
                ui::t(ui).hover.gamma_multiply(0.6)
            } else {
                ui::t(ui).hover.gamma_multiply(0.3)
            };
            ui.painter().rect_filled(rect, 6., fill);
            let cells = table_cells(rect, &self.catalog);
            let mut group_clicked = row.clicked();
            row.context_menu(|ui| {
                if selecting {
                    trash_selection_menu(
                        ui,
                        pending,
                        !self.desktop.trash_selection.items.is_empty(),
                    );
                } else if ui.button("복구").clicked() {
                    pending.push(TableCommand::RestoreGroup(g));
                    ui.close_menu();
                }
            });
            {
                let mut c = table_cell(ui, cells[0], "group_restore");
                if selecting {
                    let mut checked = selected;
                    if c.checkbox(&mut checked, "")
                        .on_hover_text("묶음 전체 선택")
                        .changed()
                    {
                        group_clicked = true;
                    }
                } else if action(&mut c, Action::Restore, "탭·폴더와 메모 전체 복구", false)
                    .on_hover_text("탭·폴더와 메모 전체 복구")
                    .clicked()
                {
                    pending.push(TableCommand::RestoreGroup(g));
                    return;
                }
            }
            {
                let mut c = table_cell(ui, cells[2], "group_title");
                c.horizontal(|ui| {
                    if ui
                        .small_button(if open { "▾" } else { "▸" })
                        .on_hover_text(if open { "접기" } else { "펼치기" })
                        .clicked()
                    {
                        group_clicked = false;
                        ui.memory_mut(|m| m.request_focus(trash_list_focus()));
                        pending.push(TableCommand::ToggleGroup(g));
                    }
                    let title = ui.add(
                        egui::Label::new(RichText::new(format!("{name}/")).strong())
                            .truncate()
                            .sense(Sense::click()),
                    );
                    group_clicked |= title.clicked();
                    title.context_menu(|ui| {
                        if selecting {
                            trash_selection_menu(
                                ui,
                                pending,
                                !self.desktop.trash_selection.items.is_empty(),
                            );
                        } else if ui.button("복구").clicked() {
                            pending.push(TableCommand::RestoreGroup(g));
                            ui.close_menu();
                        }
                    });
                });
            }
            {
                let mut c = table_cell(ui, cells[3], "group_count");
                c.add(
                    egui::Label::new(RichText::new(format!("메모 {count}개")).color(MUTED))
                        .truncate(),
                );
            }
            if group_clicked {
                if self.catalog_trash_click(ui, item) {
                    return;
                }
                pending.push(TableCommand::ToggleGroup(g));
            }
        });
    }

    fn catalog_trash_click(&mut self, ui: &egui::Ui, item: TrashItem) -> bool {
        if self.desktop.purge_target.is_some() {
            return true;
        }
        ui.memory_mut(|m| m.request_focus(trash_list_focus()));
        let modifiers = ui.input(|input| input.modifiers);
        if self.desktop.trash_selection.active
            || modifiers.ctrl
            || modifiers.command
            || modifiers.shift
        {
            if let TrashItem::Group(group) = item {
                // Choosing a group explicitly captures its members; it never
                // becomes a range anchor for the ordinary memo/tab rows.
                if !self.desktop.trash_selection.items.remove(&item) {
                    self.desktop.trash_selection.active = true;
                    self.desktop.trash_selection.items.insert(item);
                    let members = self
                        .desktop
                        .trash_group_members
                        .get(&group)
                        .cloned()
                        .unwrap_or_default();
                    self.desktop.trash_selected_groups.insert(group, members);
                } else {
                    self.desktop.trash_selected_groups.remove(&group);
                }
            } else {
                self.desktop
                    .trash_selection
                    .click(item, &self.desktop.trash_leaf_order, modifiers);
            }
            return true;
        }
        self.desktop.trash_cursor = Some(item);
        false
    }

    fn catalog_note_response(
        &mut self,
        ui: &mut egui::Ui,
        r: &egui::Response,
        i: usize,
        day: Option<NaiveDate>,
        open_editor: bool,
    ) {
        let modifiers = ui.input(|input| input.modifiers);
        let selecting = self.catalog.selection_ui
            && (self.catalog_selecting() || modifiers.command || modifiers.ctrl || modifiers.shift);
        if r.clicked() {
            if self.catalog.selection_ui {
                ui.ctx()
                    .memory_mut(|m| m.request_focus(catalog_list_focus()));
            }
            if selecting {
                self.catalog
                    .selection
                    .click(i, &self.catalog.selection_order, modifiers);
            } else {
                self.catalog_select(ui, i, day);
                if open_editor {
                    self.open_note_on(i, day);
                }
            }
        }
        if !self.catalog_selecting() {
            self.drag_source_on(ui, r, i, day);
        }
        if self.catalog.selection_ui {
            self.catalog
                .drag_sources
                .push((r.rect.intersect(ui.clip_rect()), FolderDrag::Notes(vec![i])));
        }
        if self.catalog_selecting() && self.catalog.selection.items.contains(&i) {
            r.context_menu(|ui| {
                if let Some(action) = self.catalog_batch_menu(ui) {
                    self.catalog_batch_action(self.catalog_batch_ids(), action);
                }
            });
        }
    }

    fn catalog_select(&mut self, _ui: &egui::Ui, i: usize, day: Option<NaiveDate>) {
        self.catalog.selected = Some(i);
        self.catalog.selected_day = day;
    }

    pub fn catalog_selecting(&self) -> bool {
        self.catalog.selection_ui && self.catalog.selection.active
    }

    fn catalog_selection_bar(&mut self, ui: &mut egui::Ui, order: Vec<usize>) {
        if self.catalog.selection_order != order {
            self.catalog.selection_order = order;
            self.catalog
                .selection
                .retain_visible(&self.catalog.selection_order);
        }
        let ctx = ui.ctx().clone();
        let focus = catalog_list_focus();
        let blocked = self.settings
            || self.catalog.replace_open
            || self.catalog.dragging.is_some()
            || self.desktop.lock_target.is_some()
            || self.desktop.reminder_target.is_some()
            || self.desktop.purge_target.is_some()
            || self.extra.picker.is_some()
            || self.extra.completion_scope.is_some()
            || self.catalog.batch_completion.is_some();
        let focused = ctx.memory(|m| m.has_focus(focus));
        self.catalog
            .selection
            .keyboard(&ctx, &self.catalog.selection_order, focused, blocked);
        if focused && !blocked && !ctx.memory(|m| m.any_popup_open()) {
            let redo = ctx.input_mut(|i| {
                i.consume_key(egui::Modifiers::CTRL | egui::Modifiers::SHIFT, egui::Key::Z)
            });
            if redo || ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::Z)) {
                self.catalog_history(redo);
            }
        }
        ui.horizontal(|ui| {
            let rect = egui::Rect::from_min_size(ui.cursor().min, Vec2::ZERO);
            ui.interact(rect, focus, Sense::focusable_noninteractive());
            if self.catalog.selection.toolbar(ui) {
                ui.ctx().memory_mut(|m| m.request_focus(focus));
            }
            if self.catalog.selection.active {
                let action = ui
                    .add_enabled_ui(!self.catalog.selection.items.is_empty(), |ui| {
                        ui.menu_button("선택 작업", |ui| self.catalog_batch_menu(ui))
                            .inner
                            .flatten()
                    })
                    .inner;
                if let Some(action) = action {
                    self.catalog_batch_action(self.catalog_batch_ids(), action);
                }
                if self.catalog.tab != FOLDER_TAB {
                    ui.toggle_value(&mut self.catalog.folder_drop_list, "폴더 이동 대상");
                }
            } else {
                self.catalog.folder_drop_list = false;
            }
            if ui
                .add_enabled(
                    !self.catalog.undo.is_empty(),
                    egui::Button::new("실행 취소"),
                )
                .clicked()
            {
                self.catalog_history(false);
            }
            if ui
                .add_enabled(
                    !self.catalog.redo.is_empty(),
                    egui::Button::new("다시 실행"),
                )
                .clicked()
            {
                self.catalog_history(true);
            }
        });
        if self.catalog.folder_drop_list {
            egui::ScrollArea::vertical()
                .id_salt("catalog_folder_drop_list")
                .max_height(100.)
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        let r = ui.label("분류 해제");
                        self.catalog_folder_drop(ui, &r, None);
                        for id in 0..self.catalog.folders.len() {
                            if !self.catalog.folders[id].deleted {
                                let r = ui.label(self.catalog_folder_path(id));
                                self.catalog_folder_drop(ui, &r, Some(id));
                            }
                        }
                    });
                });
        }
        if let Some((FolderDrag::Notes(ids), origin)) = self.catalog.dragging.as_ref() {
            if let Some(pos) = ctx
                .input(|i| i.pointer.hover_pos())
                .filter(|p| p.distance(*origin) >= 4.)
            {
                egui::Area::new(egui::Id::new("catalog_drag_count"))
                    .order(egui::Order::Tooltip)
                    .fixed_pos(pos + Vec2::new(16., 16.))
                    .interactable(false)
                    .show(&ctx, |ui| {
                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                            ui.label(format!("메모 {}개 이동 중", ids.len()));
                        });
                    });
            }
        }
    }

    fn catalog_batch_drop(&mut self, ui: &egui::Ui, rect: egui::Rect, target: DropTarget) {
        let rect = rect.intersect(ui.clip_rect());
        if !rect.is_positive() {
            return;
        }
        self.catalog.batch_targets.push((rect, target));
        if matches!(self.catalog.dragging, Some((FolderDrag::Notes(_), _)))
            && ui.input(|i| i.pointer.hover_pos().is_some_and(|p| rect.contains(p)))
        {
            if let DropTarget::Order(_, _, after) = target {
                let y = if after { rect.bottom() } else { rect.top() };
                ui.painter()
                    .hline(rect.x_range(), y, Stroke::new(2_f32, ACCENT));
            } else {
                ui.painter().rect_stroke(
                    rect,
                    4.,
                    Stroke::new(2_f32, ACCENT),
                    egui::StrokeKind::Inside,
                );
            }
        }
    }

    fn catalog_order_drop(
        &mut self,
        ui: &egui::Ui,
        rect: egui::Rect,
        scope: OrderScope,
        id: usize,
    ) {
        let middle = rect.center().y;
        self.catalog_batch_drop(
            ui,
            egui::Rect::from_min_max(rect.min, egui::pos2(rect.right(), middle)),
            DropTarget::Order(scope, id, false),
        );
        self.catalog_batch_drop(
            ui,
            egui::Rect::from_min_max(egui::pos2(rect.left(), middle), rect.max),
            DropTarget::Order(scope, id, true),
        );
    }

    fn catalog_drag_scroll(&self, ui: &egui::Ui) {
        let Some((FolderDrag::Notes(_), origin)) = &self.catalog.dragging else {
            return;
        };
        let clip = ui.clip_rect();
        if let Some(pos) = ui
            .input(|i| i.pointer.hover_pos())
            .filter(|p| origin.distance(*p) >= 4. && clip.contains(*p))
        {
            let distance = (clip.bottom() - pos.y).min(pos.y - clip.top());
            if distance < 26. {
                let direction = if pos.y < clip.center().y { 1. } else { -1. };
                ui.scroll_with_delta(Vec2::new(0., direction * (26. - distance) * 0.5));
                ui.ctx().request_repaint();
            }
        }
    }

    pub fn clear_catalog_transients(&mut self) {
        self.catalog.selection.clear();
        self.catalog.dragging = None;
        self.catalog.undo.clear();
        self.catalog.redo.clear();
        self.catalog.batch_completion = None;
        self.catalog.project_branch = None;
        self.catalog.project_expanded.clear();
        self.catalog.project_folder_rename = None;
        self.lists = Default::default();
    }

    pub(crate) fn catalog_preview(&mut self, ui: &mut egui::Ui, height: f32) {
        if self.catalog.results_open {
            self.catalog_results(ui, height);
            return;
        }
        let Some(i) = self
            .catalog
            .selected
            .filter(|&i| i < self.data.notes.len() && !self.data.notes[i].deleted)
        else {
            let tone = ui::t(ui);
            egui::Frame::NONE
                .fill(tone.sidebar)
                .corner_radius(12.)
                .inner_margin(18.)
                .show(ui, |ui| {
                    ui.set_min_height(height - 36.);
                    ui.set_min_width(ui.available_width());
                    ui.add_space(height * 0.28);
                    ui::empty_state(
                        ui,
                        crate::icons::Icon::Notes,
                        "선택한 메모가 없습니다",
                        "목록에서 메모를 고르면 여기에서 바로 읽고 고칠 수 있습니다",
                    );
                });
            return;
        };
        // inline_note already draws the rounded, colored paper. Wrapping it
        // in another Frame would add a second card, duplicate chrome and take
        // useful width away from the title and body.
        ui.set_height(height);
        if self.data.notes[i].locked {
            egui::Frame::NONE
                .fill(ui.visuals().faint_bg_color)
                .corner_radius(18.)
                .inner_margin(20.)
                .show(ui, |ui| {
                    ui.set_min_height((height - 40.).max(120.));
                    ui.heading(&self.data.notes[i].title);
                    ui.add_space(24.);
                    ui.label(RichText::new("잠긴 메모입니다").color(MUTED));
                    if ui.button("잠금 해제하기").clicked() {
                        self.open_note(i);
                    }
                });
        } else if self.catalog_selecting() {
            let note = &self.data.notes[i];
            let mut done = note.done;
            ui.horizontal(|ui| {
                ui.add_enabled(false, |ui: &mut egui::Ui| {
                    ui::round_check(ui, &mut done, None)
                })
                .on_hover_text("완료 상태");
                ui.heading(&note.title);
            });
            ui.label(&note.body);
            if ui.button("독립 편집창 열기").clicked() {
                self.open_note(i);
            }
        } else {
            self.inline_note(ui, i);
        }
    }

    /// Visible memos by folder (`None`: not filed), reused until input.
    fn catalog_folder_index(&mut self) -> std::rc::Rc<HashMap<Option<usize>, Vec<usize>>> {
        let mut cache = std::mem::take(&mut self.lists);
        let key = crate::lists::key((
            self.data.notes.revision(),
            self.catalog.folder_revision,
            self.catalog.folder_manual,
            self.catalog.order_revision,
            self.catalog_project_visibility_key(),
        ));
        let index = cache.get_stable("catalog_folder_index", key, || {
            let mut index: HashMap<Option<usize>, Vec<usize>> = HashMap::new();
            for i in (0..self.data.notes.len()).filter(|&i| self.visible(i)) {
                index
                    .entry(self.catalog.assignments.get(&i).copied())
                    .or_default()
                    .push(i);
            }
            if self.catalog.folder_manual {
                for (folder, ids) in &mut index {
                    self.catalog_ordered(OrderScope::Folder(*folder), ids);
                }
            }
            index
        });
        self.lists = cache;
        index
    }
    fn catalog_folders(&mut self, ui: &mut egui::Ui) {
        let previous_folder = self.catalog.folder_selected;
        ui.horizontal(|ui| {
            ui.toggle_value(&mut self.catalog.folder_manual, "수동 정렬");
            if ui
                .add(egui::Button::new(RichText::new("+  새 폴더").strong()))
                .clicked()
            {
                self.catalog.create_folder();
            }
            if ui::pill(ui, self.catalog.folder_selected.is_none(), "최상위").clicked() {
                self.catalog.folder_selected = None;
            }
            ui::hint(
                ui,
                match self
                    .catalog
                    .folder_selected
                    .and_then(|f| self.catalog.folders.get(f))
                {
                    Some(f) => format!(
                        "새 폴더는 ‘{}’ 아래에 만들어집니다 · 이름은 더블클릭으로 수정",
                        f.title
                    ),
                    None => "폴더를 선택한 뒤 추가하면 하위 폴더가 됩니다".into(),
                },
            );
        });
        if previous_folder != self.catalog.folder_selected {
            self.catalog.selection.clear();
        }
        let index = self.catalog_folder_index();
        let mut children: HashMap<Option<usize>, Vec<usize>> = HashMap::new();
        for (id, folder) in self
            .catalog
            .folders
            .iter()
            .enumerate()
            .filter(|(_, f)| !f.deleted)
        {
            children.entry(folder.parent).or_default().push(id);
        }
        let mut order = Vec::new();
        let mut pending: Vec<_> = children
            .get(&None)
            .into_iter()
            .flatten()
            .rev()
            .copied()
            .collect();
        while let Some(folder) = pending.pop() {
            if self.catalog.folders[folder].open {
                order.extend(index.get(&Some(folder)).into_iter().flatten().copied());
                pending.extend(
                    children
                        .get(&Some(folder))
                        .into_iter()
                        .flatten()
                        .rev()
                        .copied(),
                );
            }
        }
        order.extend(index.get(&None).into_iter().flatten().copied());
        self.catalog_selection_bar(ui, order);
        ui.add_space(9.);
        let height = panel_height(ui);
        let rect =
            egui::Rect::from_min_size(ui.cursor().min, Vec2::new(ui.available_width(), height));
        ui.allocate_new_ui(
            egui::UiBuilder::new()
                .max_rect(rect)
                .layout(egui::Layout::top_down(egui::Align::LEFT)),
            |ui| {
                ui.set_min_size(rect.size());
                ui.set_clip_rect(ui.clip_rect().intersect(rect));
                ui.columns(2, |cols| {
                    for column in cols.iter_mut() {
                        column.set_height(height);
                    }
                    let folder_height = (height * 0.42).max(130.);
                    let folder_count = self.catalog.folders.iter().filter(|f| !f.deleted).count();
                    ui::section_header(
                        &mut cols[0],
                        crate::icons::Icon::Folder,
                        "폴더",
                        Some(folder_count),
                    );
                    ui::card(&mut cols[0], |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("catalog_folders")
                            .max_height((folder_height - 54.).max(70.))
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                self.catalog_drag_scroll(ui);
                                let roots: Vec<_> = self
                                    .catalog
                                    .folders
                                    .iter()
                                    .enumerate()
                                    .filter(|(_, f)| !f.deleted && f.parent.is_none())
                                    .map(|(i, _)| i)
                                    .collect();
                                if roots.is_empty() {
                                    ui::empty_state(
                                        ui,
                                        crate::icons::Icon::Folder,
                                        "아직 폴더가 없습니다",
                                        "‘+ 새 폴더’로 만든 뒤 메모를 끌어 놓으세요",
                                    );
                                }
                                for f in roots {
                                    self.catalog_folder_node(ui, f, 0);
                                }
                                let r = ui.add_sized(
                                    [ui.available_width(), 26.],
                                    egui::Label::new(
                                        RichText::new("여기로 끌면 최상위 폴더가 됩니다")
                                            .size(11.)
                                            .color(ui::t(ui).faint),
                                    )
                                    .sense(Sense::hover()),
                                );
                                self.catalog_folder_drop(ui, &r, None);
                            });
                    });
                    cols[0].add_space(14.);
                    let unfiled = index.get(&None).map(Vec::as_slice).unwrap_or_default();
                    cols[0].horizontal(|ui| {
                        ui::section_header(
                            ui,
                            crate::icons::Icon::Notes,
                            "분류하지 않은 메모",
                            Some(unfiled.len()),
                        );
                    });
                    ui::card(&mut cols[0], |ui| {
                        let scroll = egui::ScrollArea::vertical()
                            .id_salt("catalog_unfiled")
                            .max_height((height - folder_height - 112.).max(70.))
                            .auto_shrink([false, false]);
                        if unfiled.is_empty() {
                            scroll.show(ui, |ui| {
                                ui::empty_state(
                                    ui,
                                    crate::icons::Icon::Notes,
                                    "모든 메모를 분류했습니다",
                                    "",
                                );
                                self.catalog_unfiled_drop(ui);
                            });
                        } else {
                            scroll.show_rows(
                                ui,
                                self.catalog_folder_row_height(),
                                unfiled.len() + 1,
                                |ui, range| {
                                    self.catalog_drag_scroll(ui);
                                    for at in range {
                                        if let Some(&id) = unfiled.get(at) {
                                            ui.push_id(("unfiled", id), |ui| {
                                                self.catalog_folder_note(ui, id)
                                            });
                                        } else {
                                            self.catalog_unfiled_drop(ui);
                                        }
                                    }
                                },
                            );
                        }
                    });
                    cols[0].add_space(4.);
                    ui::hint(
                        &mut cols[0],
                        "Ctrl+클릭으로 여러 메모를 고른 뒤 폴더로 끌어 분류합니다",
                    );
                    self.catalog_preview(&mut cols[1], height);
                });
            },
        );
    }

    fn catalog_folder_node(&mut self, ui: &mut egui::Ui, f: usize, depth: usize) {
        if depth > self.catalog.folders.len() {
            return;
        }
        let selected = self.catalog.folder_selected == Some(f);
        let mut remove = false;
        let mut remove_all = false;
        let inner = if self.catalog.rename == Some(f) {
            ui.horizontal(|ui| {
                let r = ui.add(
                    egui::TextEdit::singleline(&mut self.catalog.folders[f].title)
                        .desired_width((ui.available_width() - 10.).max(80.)),
                );
                if self.catalog.rename_focus {
                    r.request_focus();
                    self.catalog.rename_focus = false;
                }
                if r.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    self.catalog.rename = None;
                }
            })
            .response
        } else {
            let tone = ui::t(ui);
            let (rect, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.), Sense::hover());
            let r = ui.interact(
                rect,
                ui.id().with(("folder_row", f)),
                Sense::click_and_drag(),
            );
            if selected {
                ui.painter().rect_filled(rect, 7., tone.accent_soft);
            } else if r.hovered() {
                ui.painter().rect_filled(rect, 7., tone.hover);
            }
            let chevron = egui::Rect::from_center_size(
                egui::pos2(rect.left() + 13., rect.center().y),
                Vec2::splat(18.),
            );
            crate::icons::paint_icon(
                ui.painter(),
                chevron.shrink(4.),
                if self.catalog.folders[f].open {
                    crate::icons::Icon::ChevronDown
                } else {
                    crate::icons::Icon::ChevronRight
                },
                tone.muted,
            );
            crate::icons::paint_icon(
                ui.painter(),
                egui::Rect::from_center_size(
                    egui::pos2(rect.left() + 34., rect.center().y),
                    Vec2::splat(15.),
                ),
                crate::icons::Icon::Folder,
                if selected { ACCENT } else { tone.muted },
            );
            let count = self
                .catalog_folder_index()
                .get(&Some(f))
                .map_or(0, Vec::len);
            ui.painter().text(
                egui::pos2(rect.left() + 48., rect.center().y),
                egui::Align2::LEFT_CENTER,
                shorten(&self.catalog.folders[f].title, 36),
                egui::FontId::proportional(13.5),
                if selected { ACCENT } else { tone.ink },
            );
            if count > 0 {
                ui.painter().text(
                    egui::pos2(rect.right() - 10., rect.center().y),
                    egui::Align2::RIGHT_CENTER,
                    count.to_string(),
                    egui::FontId::proportional(11.5),
                    tone.faint,
                );
            }
            r.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::WidgetType::Label,
                    true,
                    &self.catalog.folders[f].title,
                )
            });
            if r.clicked() {
                if r.interact_pointer_pos()
                    .is_some_and(|p| chevron.expand(4.).contains(p))
                {
                    self.catalog.folders[f].open = !self.catalog.folders[f].open;
                } else {
                    if self.catalog.folder_selected != Some(f) {
                        self.catalog.selection.clear();
                    }
                    self.catalog.folder_selected = Some(f);
                }
            }
            if r.double_clicked() {
                self.catalog.rename = Some(f);
                self.catalog.rename_focus = true;
            }
            self.catalog
                .drag_sources
                .push((r.rect.intersect(ui.clip_rect()), FolderDrag::Folder(f)));
            r.context_menu(|ui| {
                if ui.button("이름 바꾸기").clicked() {
                    self.catalog.rename = Some(f);
                    self.catalog.rename_focus = true;
                    ui.close_menu();
                }
                if ui.button("폴더 삭제 · 메모 유지").clicked() {
                    remove = true;
                    ui.close_menu();
                }
                if ui.button("폴더 삭제 · 메모 삭제").clicked() {
                    remove_all = true;
                    ui.close_menu();
                }
            });
            r
        };
        let inner = egui::InnerResponse::new((), inner);
        self.catalog_folder_drop(ui, &inner.response, Some(f));
        if matches!(&self.catalog.dragging, Some((FolderDrag::Notes(_), origin))
            if ui.input(|i| i.pointer.hover_pos().is_some_and(|pos| origin.distance(pos) >= 4.
                && inner.response.rect.intersect(ui.clip_rect()).contains(pos))))
            && !self.catalog.folders[f].open
        {
            let now = ui.input(|i| i.time);
            match self.catalog.hover_folder {
                Some((hovered, since)) if hovered == f && now - since >= 0.5 => {
                    self.catalog.folders[f].open = true;
                    self.catalog.hover_folder = None;
                }
                Some((hovered, _)) if hovered == f => {}
                _ => {
                    self.catalog.hover_folder = Some((f, now));
                    ui.ctx()
                        .request_repaint_after(std::time::Duration::from_millis(500));
                }
            }
        } else if self
            .catalog
            .hover_folder
            .is_some_and(|(hovered, _)| hovered == f)
        {
            self.catalog.hover_folder = None;
        }
        if remove {
            self.catalog.remove_folder(f);
            return;
        }
        if remove_all {
            self.trash_catalog_folder_with_notes(f);
            return;
        }
        if self.catalog.folders[f].open {
            ui.indent(("catalog_folder", f), |ui| {
                let notes = self
                    .catalog_folder_index()
                    .get(&Some(f))
                    .cloned()
                    .unwrap_or_default();
                for i in notes {
                    crate::lists::lazy_row(ui, ("folder_note", i), 34., false, |ui| {
                        self.catalog_folder_note(ui, i)
                    });
                }
                let children: Vec<_> = self
                    .catalog
                    .folders
                    .iter()
                    .enumerate()
                    .filter(|(_, child)| !child.deleted && child.parent == Some(f))
                    .map(|(i, _)| i)
                    .collect();
                for c in children {
                    self.catalog_folder_node(ui, c, depth + 1);
                }
            });
        }
    }

    fn catalog_unfiled_drop(&mut self, ui: &mut egui::Ui) {
        let response = ui.add_sized(
            [ui.available_width(), self.catalog_folder_row_height()],
            egui::Label::new(
                RichText::new("여기로 끌면 폴더 분류가 해제됩니다")
                    .size(11.)
                    .color(ui::t(ui).faint),
            )
            .sense(Sense::hover()),
        );
        self.catalog_folder_drop(ui, &response, None);
    }

    fn catalog_folder_row_height(&self) -> f32 {
        match self.desktop.row_spacing {
            0 => 28.,
            2 => 40.,
            _ => 34.,
        }
    }

    fn catalog_folder_note(&mut self, ui: &mut egui::Ui, i: usize) {
        let selected = if self.catalog_selecting() {
            self.catalog.selection.items.contains(&i)
        } else {
            self.catalog.selected == Some(i)
        };
        let height = self.catalog_folder_row_height();
        let n = &self.data.notes[i];
        let meta = n
            .date
            .map(|d| d.format("%m.%d").to_string())
            .unwrap_or_default();
        let color = n.color;
        let title = n.title.clone();
        let day = n.date;
        let mut done = n.done_on(day);
        let complete_enabled = !self.catalog_selecting();
        let r = ui
            .horizontal(|ui| {
                if self
                    .catalog
                    .selection
                    .checkbox(ui, i, &self.catalog.selection_order)
                {
                    ui.ctx()
                        .memory_mut(|m| m.request_focus(catalog_list_focus()));
                }
                if ui
                    .add_enabled(complete_enabled, |ui: &mut egui::Ui| {
                        ui::round_check(ui, &mut done, None)
                    })
                    .on_hover_text("완료 상태")
                    .changed()
                {
                    self.done_at(i, day, done);
                }
                ui::list_row(
                    ui,
                    ui.id().with(("folder_note", i)),
                    selected,
                    Some(color),
                    &title,
                    &meta,
                    height,
                )
            })
            .inner;
        self.catalog_note_response(ui, &r, i, self.data.notes[i].date, false);
        if self.catalog.folder_manual {
            self.catalog_order_drop(
                ui,
                r.rect,
                OrderScope::Folder(self.catalog.assignments.get(&i).copied()),
                i,
            );
        }
        if !self.catalog_selecting() {
            r.context_menu(|ui| {
                if ui.button("독립 편집창 열기").clicked() {
                    self.open_note(i);
                    ui.close_menu();
                }
                if self.catalog.assignments.contains_key(&i)
                    && ui.button("폴더 분류 해제").clicked()
                {
                    self.catalog.assign_notes(&[i], None);
                    ui.close_menu();
                }
                if ui.button("휴지통으로 이동").clicked() {
                    self.trash_note(i);
                    ui.close_menu();
                }
            });
        }
    }

    fn catalog_folder_drop(&mut self, ui: &egui::Ui, r: &egui::Response, folder: Option<usize>) {
        let rect = r.rect.intersect(ui.clip_rect());
        self.catalog.drag_targets.push((rect, folder));
        if self.catalog.dragging.is_some()
            && ui.input(|i| i.pointer.hover_pos().is_some_and(|p| rect.contains(p)))
        {
            ui.painter().rect_stroke(
                rect.expand(2.),
                4.,
                Stroke::new(1.5_f32, ACCENT),
                egui::StrokeKind::Inside,
            );
        }
    }
    pub fn catalog_owns_note_drop(&self, ctx: &egui::Context, pos: egui::Pos2) -> bool {
        self.page == 4
            && !self.settings
            && ctx.viewport_id() == main_viewport()
            && (self
                .catalog
                .drag_targets
                .iter()
                .any(|(rect, _)| rect.contains(pos))
                || self
                    .catalog
                    .batch_targets
                    .iter()
                    .any(|(rect, _)| rect.contains(pos)))
    }

    fn catalog_drag_input(&mut self, ctx: &egui::Context) {
        if self.settings
            || self.desktop.lock_target.is_some()
            || self.desktop.reminder_target.is_some()
            || self.desktop.purge_target.is_some()
            || self.catalog.replace_open
            || self.catalog.batch_completion.is_some()
            || ctx.memory(|m| m.any_popup_open())
        {
            self.catalog.dragging = None;
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.catalog.dragging = None;
            self.catalog.hover_folder = None;
            return;
        }
        for event in ctx.input(|i| i.events.clone()) {
            match event {
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    ..
                } => {
                    self.catalog.dragging = self
                        .catalog
                        .drag_sources
                        .iter()
                        .rev()
                        .find(|(r, _)| r.contains(pos))
                        .map(|(_, source)| {
                            let source = match source {
                                FolderDrag::Notes(ids)
                                    if ids.first().is_some_and(|id| {
                                        self.catalog.selection.items.contains(id)
                                    }) =>
                                {
                                    FolderDrag::Notes(
                                        self.catalog
                                            .selection_order
                                            .iter()
                                            .filter(|id| self.catalog.selection.items.contains(id))
                                            .copied()
                                            .collect(),
                                    )
                                }
                                _ => source.clone(),
                            };
                            (source, pos)
                        });
                }
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    ..
                } => {
                    if let Some((source, origin)) = self.catalog.dragging.take() {
                        if origin.distance(pos) < 4. {
                            continue;
                        }
                        if let FolderDrag::Notes(notes) = &source {
                            if let Some((_, target)) = self
                                .catalog
                                .batch_targets
                                .iter()
                                .rev()
                                .find(|(r, _)| r.contains(pos))
                            {
                                match *target {
                                    DropTarget::Project(location) => {
                                        self.catalog_move_to_project(notes, location)
                                    }
                                    DropTarget::Order(scope, id, after) => {
                                        self.catalog_reorder(scope, id, after, notes)
                                    }
                                }
                                continue;
                            }
                        }
                        let Some(folder) = self
                            .catalog
                            .drag_targets
                            .iter()
                            .rev()
                            .find(|(r, _)| r.contains(pos))
                            .map(|(_, f)| *f)
                        else {
                            continue;
                        };
                        match source {
                            FolderDrag::Notes(notes) => {
                                self.catalog_batch_action(notes, BatchAction::Folder(folder))
                            }
                            FolderDrag::Folder(f) => {
                                if !self.catalog.move_folder(f, folder) {
                                    self.toast =
                                        "자기 자신이나 하위 폴더로 이동할 수 없습니다".into();
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    pub fn trash_ui(&mut self, ui: &mut egui::Ui) {
        let blocked = self.desktop.purge_target.is_some() || self.desktop.purge_save.is_some();
        ui.add_enabled_ui(!blocked, |ui| self.catalog_trash_ui(ui, blocked));
    }

    pub fn clear_trash_listing(&mut self) {
        self.catalog.trash_listing = None;
        self.desktop.trash_cursor = None;
    }

    fn trash_listing(&mut self) -> std::rc::Rc<TrashListing> {
        let revision = self.data.notes.revision();
        if let Some(cached) = &self.catalog.trash_listing {
            if cached.revision == revision && cached.open == self.catalog.trash_open {
                return cached.listing.clone();
            }
        }
        let (groups, members) = self.trash_grouped_members();
        let grouped: std::collections::HashSet<usize> =
            members.values().flatten().copied().collect();
        let mut rows = vec![];
        for g in groups {
            rows.push(TableRow::Group(g));
            if self.catalog.trash_open.contains(&g) {
                rows.extend(members[&g].iter().map(|&i| TableRow::Child(i, g)));
            }
        }
        let mut count = 0;
        let mut tabs = Vec::new();
        for (i, n) in self
            .data
            .notes
            .iter()
            .enumerate()
            .filter(|(_, n)| !n.purged)
        {
            if n.deleted {
                count += 1;
                if !grouped.contains(&i) {
                    rows.push(TableRow::Note(i, n.date));
                }
            }
            tabs.extend((0..n.document.trash.len()).map(|t| TableRow::Tab(i, t)));
        }
        count += tabs.len();
        rows.extend(tabs);
        let all: Vec<_> = rows.iter().map(|row| row.trash_item(self)).collect();
        let mut seen = std::collections::HashSet::new();
        let leaves = all
            .iter()
            .copied()
            .filter(|item| !matches!(item, TrashItem::Group(_)) && seen.insert(*item))
            .collect();
        let listing = std::rc::Rc::new(TrashListing {
            rows,
            all,
            leaves,
            members: std::rc::Rc::new(members),
            count,
        });
        self.catalog.trash_listing = Some(TrashListingCache {
            revision,
            open: self.catalog.trash_open.clone(),
            listing: listing.clone(),
        });
        listing
    }

    fn catalog_trash_ui(&mut self, ui: &mut egui::Ui, blocked: bool) {
        ui::hint(
            ui,
            "삭제한 메모와 본문 탭을 복원하거나 영구 삭제합니다 · 삭제한 메모는 설정의 보관 기간이 지나면 자동으로 지워집니다",
        );
        ui.add_space(10.);
        let listing = self.trash_listing();
        let count = listing.count;
        if !std::rc::Rc::ptr_eq(&self.desktop.trash_group_members, &listing.members) {
            self.desktop.trash_group_members = listing.members.clone();
            self.desktop.trash_leaf_order = listing.leaves.clone();
            self.desktop.trash_selection.retain_visible(&listing.all);
        }
        self.desktop.trash_selection.keyboard(
            ui.ctx(),
            &self.desktop.trash_leaf_order,
            ui.memory(|m| m.has_focus(trash_list_focus())),
            blocked,
        );
        let selected = &self.desktop.trash_selection.items;
        self.desktop
            .trash_selected_groups
            .retain(|group, _| selected.contains(&TrashItem::Group(*group)));
        ui::section_header(ui, crate::icons::Icon::Trash, "휴지통", Some(count));
        let mut restore = false;
        ui.horizontal_wrapped(|ui| {
            if self.desktop.trash_selection.toolbar(ui) {
                ui.memory_mut(|m| m.request_focus(trash_list_focus()));
                self.desktop.trash_selected_groups.clear();
            }
            if self.desktop.trash_selection.active {
                let selected = !self.desktop.trash_selection.items.is_empty();
                if ui
                    .add_enabled(selected, egui::Button::new("선택 복원"))
                    .on_hover_text("삭제된 본문 탭을 복원하면 원본 메모도 복원될 수 있습니다")
                    .clicked()
                {
                    restore = true;
                }
                if ui
                    .add_enabled(selected, egui::Button::new("선택 영구삭제…"))
                    .clicked()
                {
                    self.request_trash_purge(PurgeKind::Selection, None);
                }
            }
            if ui
                .add_enabled(count != 0, egui::Button::new("휴지통 비우기…"))
                .clicked()
            {
                self.request_trash_purge(PurgeKind::Empty, None);
            }
        });
        if self.desktop.trash_selection.active {
            ui::hint(
                ui,
                "묶음을 직접 선택하면 해당 묶음에 있는 메모가 포함됩니다",
            );
        }
        if self.desktop.purge_save.is_some() {
            ui::hint(ui, "영구삭제 결과를 저장하고 있습니다");
        }
        if listing.rows.is_empty() {
            ui.add_space(8.);
            ui::card(ui, |ui| {
                ui.set_width(ui.available_width());
                ui::empty_state(
                    ui,
                    crate::icons::Icon::Trash,
                    "휴지통이 비어 있습니다",
                    "삭제한 메모와 본문 탭이 여기에 모입니다",
                );
            });
        } else {
            let height = panel_height(ui);
            let rect =
                egui::Rect::from_min_size(ui.cursor().min, Vec2::new(ui.available_width(), height));
            ui.allocate_new_ui(
                egui::UiBuilder::new()
                    .max_rect(rect)
                    .layout(egui::Layout::top_down(egui::Align::LEFT)),
                |ui| {
                    ui.set_min_size(rect.size());
                    ui.set_clip_rect(ui.clip_rect().intersect(rect));
                    self.catalog.table_width = TABLE_WIDTH.max(rect.width() - 14.);
                    self.catalog_table(ui, &listing.rows, height, false, true, false);
                },
            );
        }
        if restore {
            self.restore_trash_items();
        }
    }

    fn catalog_restore_tab(&mut self, i: usize, id: u64) -> bool {
        let Some(tab) = self.data.notes[i]
            .document
            .trash
            .iter()
            .find(|tab| tab.id == id)
        else {
            return false;
        };
        let title = tab.title.clone();
        let restored = self.restore_trash_tabs(i, &BTreeSet::from([id]));
        if restored {
            self.toast = format!("{title} 탭을 원래 메모에 복원했습니다");
        }
        restored
    }

    pub fn catalog_replace_window(&mut self, ctx: &egui::Context) {
        if !self.catalog.replace_open {
            return;
        }
        let mut open = true;
        let mut command = None;
        let mut target = vec![];
        ui::dialog(
            ctx,
            "catalog_replace",
            "찾아 바꾸기",
            [460., 420.],
            false,
            &mut open,
            |ui| {
                ui.label(RichText::new("찾아 바꾸기").strong().size(15.));
                ui.add_space(8.);
                ui::field(ui, "찾을 내용", |ui| {
                    let r = ui.add(
                        egui::TextEdit::singleline(&mut self.catalog.replace_find)
                            .desired_width(f32::INFINITY)
                            .margin(egui::Margin::symmetric(8, 5)),
                    );
                    if r.changed() {
                        self.catalog.replace_cursor = None;
                    }
                    if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        command = Some(ReplaceCommand::FindNext);
                    }
                });
                ui::field(ui, "바꿀 내용", |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.catalog.replace_with)
                            .desired_width(f32::INFINITY)
                            .margin(egui::Margin::symmetric(8, 5)),
                    );
                });
                ui.add_space(6.);
                ui::field(ui, "범위", |ui| {
                    if ui::segmented(
                        ui,
                        &mut self.catalog.replace_scope,
                        &["현재 메모", "선택 프로젝트", "전체 메모"],
                    ) {
                        self.catalog.replace_cursor = None;
                    }
                });
                if self.catalog.replace_scope == 1 && self.catalog.replace_projects.is_empty() {
                    let current = self
                        .catalog
                        .selected
                        .and_then(|i| self.data.notes[i].project)
                        .unwrap_or(NO_PROJECT);
                    self.catalog.replace_projects.insert(current);
                }
                ui::field(ui, "프로젝트", |ui| {
                    ui.add_enabled_ui(self.catalog.replace_scope == 1, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            let mut choices = vec![(NO_PROJECT, "프로젝트 없음".to_string())];
                            for (p, name) in self.data.projects.iter().enumerate() {
                                if !self.extra.deleted_projects.contains(&p) {
                                    choices.push((p, format!("# {name}")));
                                }
                            }
                            for (p, name) in choices {
                                let on = self.catalog.replace_projects.contains(&p);
                                if ui::pill(ui, on, &name).clicked() {
                                    if on {
                                        self.catalog.replace_projects.remove(&p);
                                    } else {
                                        self.catalog.replace_projects.insert(p);
                                    }
                                    self.catalog.replace_cursor = None;
                                }
                            }
                        });
                    });
                });
                target = self.replace_targets_cached();
                ui.label(
                    RichText::new(format!(
                        "{}개 메모에서 찾음 · 잠긴 메모와 휴지통 제외",
                        target.len()
                    ))
                    .small()
                    .color(MUTED),
                );
                ui.add_space(4.);
                if !self.catalog.replace_message.is_empty() {
                    ui.label(RichText::new(&self.catalog.replace_message).color(ACCENT));
                }
                ui.add_space(4.);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let any = !target.is_empty();
                    if ui
                        .add_enabled(
                            any,
                            egui::Button::new(
                                RichText::new("모두 바꾸기").color(Color32::WHITE).strong(),
                            )
                            .fill(ACCENT),
                        )
                        .clicked()
                    {
                        command = Some(ReplaceCommand::ReplaceAll);
                    }
                    if ui.add_enabled(any, egui::Button::new("바꾸기")).clicked() {
                        command = Some(ReplaceCommand::Replace);
                    }
                    if ui
                        .add_enabled(any, egui::Button::new("모두 찾기"))
                        .clicked()
                    {
                        command = Some(ReplaceCommand::FindAll);
                    }
                    if ui.add_enabled(any, egui::Button::new("찾기")).clicked() {
                        command = Some(ReplaceCommand::FindNext);
                    }
                });
            },
        );
        self.catalog.replace_open = open;
        match command {
            Some(ReplaceCommand::FindNext) => self.replace_find_next(&target),
            Some(ReplaceCommand::Replace) => {
                self.replace_current(&target);
            }
            Some(ReplaceCommand::FindAll) => self.replace_find_all(&target),
            Some(ReplaceCommand::ReplaceAll) => {
                let count = self.replace_count(&target);
                self.catalog_apply_replacement(&target);
                self.catalog.replace_cursor = None;
                self.catalog.replace_message =
                    format!("{}개 메모에서 {count}곳을 바꿨습니다", target.len());
                if self.catalog.results_open {
                    let target = self.catalog_replace_targets();
                    self.replace_find_all(&target);
                }
            }
            None => {}
        }
    }

    /// Exact (case-sensitive) matches in `targets`, in reading order: title
    /// first, then each tab.
    fn replace_matches(&self, targets: &[usize]) -> Vec<FindHit> {
        let find = &self.catalog.replace_find;
        let mut hits = vec![];
        if find.is_empty() {
            return hits;
        }
        for &i in targets {
            let n = &self.data.notes[i];
            for _ in n.title.match_indices(find.as_str()) {
                hits.push((i, None));
            }
            for (tab, body) in std::iter::once((0, n.body.as_str())).chain(
                n.document
                    .tabs
                    .iter()
                    .enumerate()
                    .skip(1)
                    .map(|(tab, t)| (tab, t.body.as_str())),
            ) {
                for range in exact_ranges(body, find) {
                    hits.push((i, Some((tab, range))));
                }
            }
        }
        hits
    }
    fn replace_count(&self, targets: &[usize]) -> usize {
        self.replace_matches(targets).len()
    }
    /// Select the next match: its memo in the list and the text in the preview.
    fn replace_find_next(&mut self, targets: &[usize]) {
        let hits = self.replace_matches(targets);
        if hits.is_empty() {
            self.catalog.replace_message = "찾는 내용이 없습니다".into();
            self.catalog.replace_cursor = None;
            return;
        }
        let at = self
            .catalog
            .replace_cursor
            .map_or(0, |c| (c + 1) % hits.len());
        self.catalog.replace_cursor = Some(at);
        let (id, location) = hits[at].clone();
        self.catalog.results_open = false;
        self.catalog.selected = Some(id);
        self.catalog.find_selection = location.map(|(tab, range)| (id, tab, range));
        self.catalog.replace_message = format!(
            "{} / {} · {}",
            at + 1,
            hits.len(),
            if self.data.notes[id].title.is_empty() {
                "제목 없음"
            } else {
                &self.data.notes[id].title
            }
        );
    }
    /// Replace the current match, then move on to the next one.
    fn replace_current(&mut self, targets: &[usize]) {
        let hits = self.replace_matches(targets);
        let Some(at) = self.catalog.replace_cursor.filter(|&c| c < hits.len()) else {
            self.replace_find_next(targets);
            return;
        };
        let (i, location) = hits[at].clone();
        let find = self.catalog.replace_find.clone();
        let with = self.catalog.replace_with.clone();
        let n = &mut self.data.notes[i];
        match location {
            None => n.title = n.title.replacen(&find, &with, 1),
            Some((tab, range)) => {
                if let Some(first) = n.document.tabs.first_mut() {
                    sync_tab_body(first, &n.body);
                }
                if let Some(t) = n.document.tabs.get_mut(tab) {
                    replace_tab_range(t, range, &find, &with);
                }
                if let Some(first) = n.document.tabs.first() {
                    n.body = first.body.clone();
                }
            }
        }
        // The replaced text is no longer a match: the next one now sits at
        // the same index.
        self.catalog.replace_cursor = at.checked_sub(1);
        if self.replace_matches(targets).is_empty() {
            self.catalog.replace_cursor = None;
            self.catalog.replace_message = "모두 바꿨습니다".into();
        } else {
            if at == 0 {
                self.catalog.replace_cursor = None;
            }
            self.replace_find_next(targets);
        }
    }
    /// Gather one result per memo and show them in the preview pane.
    fn replace_find_all(&mut self, targets: &[usize]) {
        let find = self.catalog.replace_find.clone();
        let mut results = vec![];
        for &i in targets {
            let n = &self.data.notes[i];
            let count = n.title.matches(find.as_str()).count()
                + std::iter::once(n.body.as_str())
                    .chain(n.document.tabs.iter().skip(1).map(|t| t.body.as_str()))
                    .map(|b| b.matches(find.as_str()).count())
                    .sum::<usize>();
            if count == 0 {
                continue;
            }
            let title_hit = n.title.find(find.as_str()).map(|b| b..b + find.len());
            let line = std::iter::once(n.body.as_str())
                .chain(n.document.tabs.iter().skip(1).map(|t| t.body.as_str()))
                .flat_map(|b| b.lines())
                .find(|l| l.contains(find.as_str()))
                .map(|l| crate::composer::strip_pictures(l).trim().to_string());
            let (line, line_hit) = match line {
                Some(l) => {
                    let hit = l.find(find.as_str()).map(|b| b..b + find.len());
                    (l, hit)
                }
                None => ("제목에서 찾았습니다".into(), None),
            };
            results.push(FindResult {
                note: i,
                title_hit,
                line,
                line_hit,
                count,
            });
        }
        self.catalog
            .results_selected
            .retain(|n| results.iter().any(|r| r.note == *n));
        self.catalog.replace_message = format!("{}개 메모에서 찾았습니다", results.len());
        self.catalog.results = results;
        self.catalog.results_open = true;
    }
    /// The "모두 찾기" results in the preview pane, with the actions on top
    /// so they stay reachable when the dialog falls behind the main window.
    fn catalog_results(&mut self, ui: &mut egui::Ui, height: f32) {
        let tone = ui::t(ui);
        let mut action = None;
        ui.set_height(height);
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("찾기 결과").strong().size(14.));
            ui.label(
                RichText::new(format!(
                    "{}개 · {}개 선택",
                    self.catalog.results.len(),
                    self.catalog.results_selected.len()
                ))
                .size(12.)
                .color(tone.faint),
            );
        });
        ui.add_space(4.);
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    !self.catalog.results_selected.is_empty(),
                    egui::Button::new("바꾸기"),
                )
                .on_hover_text("선택한 결과의 메모에서 바꿉니다")
                .clicked()
            {
                action = Some(0);
            }
            if ui.button("모두 바꾸기").clicked() {
                action = Some(1);
            }
            if ui.button("다른 찾기").clicked() {
                action = Some(2);
            }
            if ui.button("찾아 바꾸기 마침").clicked() {
                action = Some(3);
            }
        });
        ui.add_space(6.);
        let find = self.catalog.replace_find.clone();
        egui::ScrollArea::vertical()
            .id_salt("find_results")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if self.catalog.results.is_empty() {
                    ui::empty_state(
                        ui,
                        crate::icons::Icon::Search,
                        "찾은 내용이 없습니다",
                        "‘다른 찾기’로 검색어를 바꿔 보세요",
                    );
                }
                for k in 0..self.catalog.results.len() {
                    let r = self.catalog.results[k].clone();
                    let selected = self.catalog.results_selected.contains(&r.note);
                    let (rect, resp) = ui
                        .allocate_exact_size(Vec2::new(ui.available_width(), 52.), Sense::click());
                    let color = self.data.notes[r.note].color;
                    ui.painter().rect(
                        rect,
                        8.,
                        if selected {
                            tone.accent_soft
                        } else if resp.hovered() {
                            tone.hover
                        } else {
                            tone.bg
                        },
                        Stroke::new(1.0_f32, if selected { ACCENT } else { tone.line }),
                        egui::StrokeKind::Inside,
                    );
                    ui.painter().circle_filled(
                        egui::pos2(rect.left() + 14., rect.top() + 17.),
                        4.5,
                        color,
                    );
                    let x = rect.left() + 26.;
                    let width = rect.right() - x - 10.;
                    let title = &self.data.notes[r.note].title;
                    let title = if title.is_empty() {
                        "제목 없음"
                    } else {
                        title
                    };
                    let job = highlight_job(
                        title,
                        r.title_hit.clone(),
                        egui::FontId::proportional(13.5),
                        tone.ink,
                        width,
                    );
                    let galley = ui.fonts(|f| f.layout_job(job));
                    ui.painter()
                        .galley(egui::pos2(x, rect.top() + 8.), galley, tone.ink);
                    let job = highlight_job(
                        &r.line,
                        r.line_hit.clone(),
                        egui::FontId::proportional(12.),
                        tone.muted,
                        width,
                    );
                    let galley = ui.fonts(|f| f.layout_job(job));
                    ui.painter()
                        .galley(egui::pos2(x, rect.top() + 29.), galley, tone.muted);
                    if r.count > 1 {
                        ui.painter().text(
                            egui::pos2(rect.right() - 10., rect.top() + 16.),
                            egui::Align2::RIGHT_CENTER,
                            format!("{}곳", r.count),
                            egui::FontId::proportional(11.),
                            tone.faint,
                        );
                    }
                    let resp = resp
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .on_hover_text(format!("‘{find}’ · 클릭해서 선택 / 해제"));
                    if resp.clicked() {
                        if !self.catalog.results_selected.remove(&r.note) {
                            self.catalog.results_selected.insert(r.note);
                        }
                        self.catalog.selected = Some(r.note);
                    }
                    ui.add_space(4.);
                }
            });
        match action {
            Some(0) => {
                let target: Vec<_> = self.catalog.results_selected.iter().copied().collect();
                self.catalog_apply_replacement(&target);
                self.catalog.results_selected.clear();
                let target = self.catalog_replace_targets();
                self.replace_find_all(&target);
            }
            Some(1) => {
                let target: Vec<_> = self.catalog.results.iter().map(|r| r.note).collect();
                self.catalog_apply_replacement(&target);
                let target = self.catalog_replace_targets();
                self.replace_find_all(&target);
            }
            Some(2) => {
                self.catalog.replace_open = true;
                ui::raise(ui.ctx(), "catalog_replace");
            }
            Some(3) => {
                self.catalog.results_open = false;
                self.catalog.replace_open = false;
                self.catalog.results.clear();
                self.catalog.results_selected.clear();
            }
            _ => {}
        }
    }

    /// `catalog_replace_targets`, searched again only when the find text, the
    /// scope, the selection or any memo (input counts as a possible change)
    /// is different from the last time.
    fn replace_targets_cached(&mut self) -> Vec<usize> {
        let key = ReplaceKey {
            changes: crate::lists::CHANGES.load(std::sync::atomic::Ordering::Relaxed),
            notes: self.data.notes.len(),
            find: self.catalog.replace_find.clone(),
            scope: self.catalog.replace_scope,
            projects: self.catalog.replace_projects.clone(),
            selected: self.catalog.selected,
            active: self.active,
            project: self.catalog.project,
            filter: self.project_filter,
        };
        if let Some((k, targets)) = &self.catalog.replace_cache {
            if *k == key {
                return targets.clone();
            }
        }
        let targets = self.catalog_replace_targets();
        self.catalog.replace_cache = Some((key, targets.clone()));
        targets
    }
    fn catalog_replace_targets(&self) -> Vec<usize> {
        let selected = self.catalog.selected.or(Some(self.active));
        let project = self
            .catalog
            .project
            .or(self.project_filter)
            .or_else(|| selected.and_then(|i| self.data.notes[i].project));
        self.data
            .notes
            .iter()
            .enumerate()
            .filter(|(i, n)| {
                self.visible(*i)
                    && !n.locked
                    && !self.catalog.replace_find.is_empty()
                    && match self.catalog.replace_scope {
                        0 => selected == Some(*i),
                        1 if !self.catalog.replace_projects.is_empty() => self
                            .catalog
                            .replace_projects
                            .contains(&n.project.unwrap_or(NO_PROJECT)),
                        1 => crate::refinements::project_match(
                            Some(project.unwrap_or(NO_PROJECT)),
                            n.project,
                        ),
                        _ => true,
                    }
                    && (n.title.contains(&self.catalog.replace_find)
                        || n.body.contains(&self.catalog.replace_find)
                        || n.document
                            .tabs
                            .iter()
                            .skip(1)
                            .any(|t| t.body.contains(&self.catalog.replace_find)))
            })
            .map(|(i, _)| i)
            .collect()
    }

    fn catalog_apply_replacement(&mut self, targets: &[usize]) {
        let find = &self.catalog.replace_find;
        let replacement = &self.catalog.replace_with;
        if find.is_empty() {
            return;
        }
        for &i in targets {
            let n = &mut self.data.notes[i];
            n.title = n.title.replace(find, replacement);
            // The legacy body remains authoritative for the first tab. Bring
            // it into the document before replacing, so the next editor draw
            // cannot overwrite another tab's new text with an old first body.
            if let Some(first) = n.document.tabs.first_mut() {
                sync_tab_body(first, &n.body);
            }
            for tab in &mut n.document.tabs {
                replace_tab_text(tab, find, replacement);
            }
            n.body = n
                .document
                .tabs
                .first()
                .map(|t| t.body.clone())
                .unwrap_or_else(|| n.body.replace(find, replacement));
        }
    }
}

fn remap_spans(spans: &mut Vec<crate::composer::Span>, start: usize, end: usize, added: usize) {
    let removed = end - start;
    let shift = |position: usize| position.saturating_sub(removed) + added;
    for span in spans.iter_mut() {
        span.range.start = if span.range.start >= end {
            shift(span.range.start)
        } else if span.range.start > start {
            start
        } else {
            span.range.start
        };
        span.range.end = if span.range.end >= end {
            shift(span.range.end)
        } else if span.range.end > start {
            start + added
        } else {
            span.range.end
        };
    }
    spans.retain(|s| s.range.start < s.range.end);
}

fn sync_tab_body(tab: &mut crate::composer::Tab, body: &str) {
    if tab.body == body {
        return;
    }
    let old: Vec<_> = tab.body.chars().collect();
    let new: Vec<_> = body.chars().collect();
    let head = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    let tail = old[head..]
        .iter()
        .rev()
        .zip(new[head..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    remap_spans(
        &mut tab.spans,
        head,
        old.len() - tail,
        new.len() - head - tail,
    );
    tab.body = body.into();
}

fn replace_tab_text(tab: &mut crate::composer::Tab, find: &str, replacement: &str) {
    // Apply from the end so byte and character offsets in earlier matches
    // remain valid. Keep rich formatting anchored after length-changing edits.
    let matches: Vec<_> = tab
        .body
        .match_indices(find)
        .map(|(byte, _)| (byte, tab.body[..byte].chars().count()))
        .collect();
    let removed = find.chars().count();
    let added = replacement.chars().count();
    for (byte, character) in matches.into_iter().rev() {
        tab.body.replace_range(byte..byte + find.len(), replacement);
        remap_spans(&mut tab.spans, character, character + removed, added);
    }
}

#[derive(Clone, Copy)]
enum ReplaceCommand {
    FindNext,
    Replace,
    FindAll,
    ReplaceAll,
}

/// Character ranges of exact (case-sensitive) occurrences of `find`.
fn exact_ranges(text: &str, find: &str) -> Vec<Range<usize>> {
    if find.is_empty() {
        return vec![];
    }
    let width = find.chars().count();
    text.match_indices(find)
        .map(|(byte, _)| {
            let start = text[..byte].chars().count();
            start..start + width
        })
        .collect()
}

/// Replace the occurrence of `find` at character range `range`, if it is
/// still there, keeping formatting spans anchored.
fn replace_tab_range(
    tab: &mut crate::composer::Tab,
    range: Range<usize>,
    find: &str,
    replacement: &str,
) -> bool {
    let Some((start, _)) = tab.body.char_indices().nth(range.start) else {
        return false;
    };
    if !tab.body[start..].starts_with(find) {
        return false;
    }
    tab.body
        .replace_range(start..start + find.len(), replacement);
    remap_spans(
        &mut tab.spans,
        range.start,
        range.end,
        replacement.chars().count(),
    );
    true
}

/// One line of text truncated to `width`, with `hit` (a byte range)
/// highlighted.
fn highlight_job(
    text: &str,
    hit: Option<Range<usize>>,
    font: egui::FontId,
    color: Color32,
    width: f32,
) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    let plain = egui::TextFormat {
        font_id: font.clone(),
        color,
        ..Default::default()
    };
    match hit.filter(|h| {
        h.end <= text.len() && text.is_char_boundary(h.start) && text.is_char_boundary(h.end)
    }) {
        Some(h) => {
            // Keep the hit visible on long lines: start a little before it.
            let lead = text[..h.start]
                .char_indices()
                .rev()
                .nth(16)
                .map_or(0, |(b, _)| b);
            if lead > 0 {
                job.append("…", 0., plain.clone());
            }
            job.append(&text[lead..h.start], 0., plain.clone());
            job.append(
                &text[h.clone()],
                0.,
                egui::TextFormat {
                    font_id: font,
                    color: INK,
                    background: Color32::from_rgb(255, 222, 120),
                    ..Default::default()
                },
            );
            job.append(&text[h.end..], 0., plain);
        }
        None => job.append(text, 0., plain),
    }
    job.wrap = egui::text::TextWrapping::truncate_at_width(width.max(40.));
    job
}

/// Character ranges, rather than byte offsets, match TextEdit's cursor contract.
fn panel_height(ui: &egui::Ui) -> f32 {
    // This view can live inside an outer ScrollArea and a horizontal row. Its
    // inherited max_rect may be one row high (or infinite), so derive a finite
    // pane from the visible viewport, rather than inheriting that row height.
    let bottom = ui
        .clip_rect()
        .bottom()
        .min(ui.ctx().screen_rect().bottom() - 18.);
    (bottom - ui.cursor().top() - 8.).clamp(180., 720.)
}

fn find_ranges(text: &str, query: &str) -> Vec<Range<usize>> {
    if query.is_empty() {
        return vec![];
    }
    // Compare folded text while retaining offsets in the original character
    // sequence, so Korean and multi-byte characters select the right range.
    let chars: Vec<_> = text.chars().collect();
    let width = query.chars().count();
    let query = query.to_lowercase();
    let mut ranges = vec![];
    let mut from = 0;
    while from + width <= chars.len() {
        let candidate: String = chars[from..from + width].iter().collect();
        if candidate.to_lowercase() == query {
            ranges.push(from..from + width);
            from += width;
        } else {
            from += 1;
        }
    }
    ranges
}

#[cfg(test)]
mod tests {
    use super::*;
    fn render_table_layout(a: &mut App, ctx: &egui::Context, width: f32) -> egui::FullOutput {
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(width, 760.));
        ctx.run(
            egui::RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            },
            |ctx| {
                egui::SidePanel::left("table_test_sidebar")
                    .exact_width(220.)
                    .show(ctx, |_| {});
                egui::CentralPanel::default().show(ctx, |ui| {
                    a.catalog_split_list(ui, &[(0, a.data.notes[0].date)], false, false);
                });
            },
        )
    }

    fn text_rect(output: &egui::FullOutput, text: &str, before_x: f32) -> egui::Rect {
        output
            .shapes
            .iter()
            .find_map(|shape| {
                if let egui::Shape::Text(t) = &shape.shape {
                    let rect = t.galley.rect.translate(t.pos.to_vec2());
                    if t.galley.job.text == text
                        && rect.left() < before_x
                        && shape.clip_rect.intersects(rect)
                    {
                        return Some(rect);
                    }
                }
                None
            })
            .unwrap_or_else(|| panic!("visible table text missing: {text}"))
    }

    #[test]
    fn replace_targets_are_kept_until_something_changes() {
        let mut a = App::sample(Local::now().date_naive());
        a.data.notes[0].body = "사과".into();
        a.catalog.replace_find = "사과".into();
        a.catalog.replace_scope = 2;
        let first = a.replace_targets_cached();
        assert_eq!(first, a.catalog_replace_targets());
        // Same state (a frame with only mouse movement): the kept list.
        assert_eq!(a.replace_targets_cached(), first);
        // Input counts as a change: searched again.
        a.data.notes[1].body = "사과 둘".into();
        crate::lists::CHANGES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let second = a.replace_targets_cached();
        assert_eq!(second, a.catalog_replace_targets());
        assert!(second.contains(&1) && !first.contains(&1));
        // A different find text is never answered from the kept list.
        a.catalog.replace_find = "없는말".into();
        assert!(a.replace_targets_cached().is_empty());
    }

    #[test]
    fn find_next_and_replace_walk_matches_across_projects() {
        let mut a = App::sample(Local::now().date_naive());
        a.data.notes[0].body = "사과 하나, 사과 둘".into();
        a.data.notes[0].project = Some(0);
        a.data.notes[1].body = "사과 셋".into();
        a.data.notes[1].project = Some(1);
        a.data.notes[2].body = "사과 넷".into();
        a.data.notes[2].project = None;
        a.catalog.replace_find = "사과".into();
        a.catalog.replace_with = "배".into();
        a.catalog.replace_scope = 1;
        a.catalog.replace_projects = BTreeSet::from([0, 1]);
        let targets = a.catalog_replace_targets();
        assert!(targets.contains(&0) && targets.contains(&1) && !targets.contains(&2));
        let total = a.replace_count(&targets);
        a.replace_find_next(&targets);
        assert_eq!(
            a.catalog.find_selection.as_ref().map(|s| s.2.clone()),
            Some(0..2)
        );
        // Replace the first, then the cursor stays on the next match.
        a.replace_current(&targets);
        assert_eq!(a.data.notes[0].body, "배 하나, 사과 둘");
        assert_eq!(a.replace_count(&targets), total - 1);
        assert_eq!(
            a.catalog.find_selection.as_ref().map(|s| s.2.clone()),
            Some(6..8)
        );
        // "모두 찾기" lists one result per memo with the hit highlighted.
        a.replace_find_all(&targets);
        assert!(a.catalog.results_open);
        let r = a.catalog.results.iter().find(|r| r.note == 1).unwrap();
        assert_eq!(&r.line[r.line_hit.clone().unwrap()], "사과");
        assert!(
            a.data.notes[2].body.contains("사과"),
            "other projects are untouched"
        );
    }
    fn representative_table_app() -> App {
        let mut a = App::sample(Local::now().date_naive());
        let n = &mut a.data.notes[0];
        n.title = "COLUMN TITLE".into();
        n.body = "COLUMN BODY".into();
        n.created = NaiveDate::from_ymd_opt(2026, 1, 2)
            .unwrap()
            .and_hms_opt(1, 2, 3)
            .unwrap();
        n.reminder = Some(
            NaiveDate::from_ymd_opt(2026, 2, 3)
                .unwrap()
                .and_hms_opt(4, 5, 0)
                .unwrap(),
        );
        n.date = NaiveDate::from_ymd_opt(2026, 11, 10);
        n.time = chrono::NaiveTime::from_hms_opt(9, 0, 0).unwrap();
        a
    }

    #[test]
    fn detailed_table_keeps_fields_on_one_horizontal_row_with_aligned_headers() {
        let mut a = representative_table_app();
        let ctx = egui::Context::default();
        render_table_layout(&mut a, &ctx, 1440.);
        let out = render_table_layout(&mut a, &ctx, 1440.);
        let values = [
            "COLUMN TITLE",
            "COLUMN BODY",
            "02/03 04:05",
            "260102",
            "11/10 09:00",
        ]
        .map(|s| text_rect(&out, s, 1110.));
        assert!(values.windows(2).all(|p| p[0].left() < p[1].left()));
        assert!(values
            .iter()
            .all(|r| (r.center().y - values[0].center().y).abs() < 4.));
        for (name, index) in [("제목", 0), ("내용", 1), ("생성일", 3), ("일정", 4)] {
            let header = text_rect(&out, name, 1110.);
            assert!((header.left() - values[index].left()).abs() < 1.);
            assert!(header.bottom() < values[index].top());
        }
        assert!(
            a.catalog.table_viewport_width > 800.,
            "300px preview must not take half the workspace"
        );
    }

    #[test]
    fn the_table_shrinks_with_the_list_and_scrolls_only_when_too_narrow() {
        let mut a = representative_table_app();
        let ctx = egui::Context::default();
        // A typical window: every column fits without horizontal scrolling.
        render_table_layout(&mut a, &ctx, 1400.);
        let out = render_table_layout(&mut a, &ctx, 1400.);
        let visible = a.catalog.table_viewport_width;
        assert!(
            (a.catalog.table_width - visible).abs() < 20.,
            "{} vs {visible}",
            a.catalog.table_width
        );
        let header = text_rect(&out, "생성일", 1400.);
        let created = text_rect(&out, "260102", 1400.);
        assert!((header.left() - created.left()).abs() < 1.);
        // Widening the preview narrows the list instead of hiding columns.
        let before = a.catalog.table_width;
        a.catalog.preview_width += 120.;
        render_table_layout(&mut a, &ctx, 1400.);
        assert!(a.catalog.table_width < before);
        // Below the narrowest table the delete column is still reachable.
        a.catalog.table_offset = f32::MAX;
        render_table_layout(&mut a, &ctx, 700.);
        render_table_layout(&mut a, &ctx, 700.);
        let visible = a.catalog.table_viewport_width;
        let last = table_cells(
            egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(TABLE_WIDTH, 48.)),
            &a.catalog,
        )[COLUMN_DELETE];
        assert!(last.center().x - a.catalog.table_offset > 0.);
        assert!(last.right() - a.catalog.table_offset <= visible + 1.);
    }

    #[test]
    fn hidden_columns_collapse_to_zero_and_custom_widths_are_honored() {
        let base = Catalog::default();
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(1400., 48.));
        let default_cells = table_cells(rect, &base);
        // 알람 (column 4) starts at its default width.
        assert_eq!(default_cells[4].width(), COLUMN_WIDTHS[4]);

        let mut hidden = Catalog::default();
        hidden.column_shown.0[4] = false;
        let cells = table_cells(rect, &hidden);
        assert_eq!(cells[4].width(), 0.);
        // The space freed by hiding 알람 goes to 내용 (column 3), not lost.
        assert!(cells[3].width() > default_cells[3].width());

        let mut resized = Catalog::default();
        resized.column_widths[4] = 150.;
        let cells = table_cells(rect, &resized);
        assert_eq!(cells[4].width(), 150.);

        // Out-of-range custom widths are clamped where they're applied in
        // the UI (the drag handler), not inside table_cells itself; a
        // sentinel of exactly 0. still falls back to the default rather
        // than collapsing the column, so "not customized" and "hidden" stay
        // distinct states.
        let mut zeroed = Catalog::default();
        zeroed.column_widths[4] = 0.;
        let cells = table_cells(rect, &zeroed);
        assert_eq!(cells[4].width(), COLUMN_WIDTHS[4]);
    }

    #[test]
    fn dragging_the_left_edge_of_a_header_changes_that_column_with_the_mouse() {
        let mut a = representative_table_app();
        let ctx = egui::Context::default();
        let frame = |a: &mut App, events: Vec<egui::Event>| {
            ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::new(1440., 760.),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::SidePanel::left("table_test_sidebar")
                        .exact_width(220.)
                        .show(ctx, |_| {});
                    egui::CentralPanel::default().show(ctx, |ui| {
                        a.catalog_split_list(ui, &[(0, a.data.notes[0].date)], false, false);
                    });
                },
            )
        };
        frame(&mut a, vec![]);
        let out = frame(&mut a, vec![]);
        let header = text_rect(&out, "일정", 1440.);
        let edge = egui::pos2(header.left() - 3., header.center().y);
        let before = column_widths(&a.catalog, a.catalog.table_width)[6];
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        frame(&mut a, vec![egui::Event::PointerMoved(edge)]);
        frame(&mut a, vec![button(edge, true)]);
        for step in 1..=4 {
            let to = edge - Vec2::new(10. * step as f32, 0.);
            frame(&mut a, vec![egui::Event::PointerMoved(to)]);
        }
        frame(&mut a, vec![button(edge - Vec2::new(40., 0.), false)]);
        let after = a.catalog.column_widths[6];
        assert!(after > before + 20., "{before} -> {after}");
    }

    #[test]
    fn the_extra_columns_start_hidden_and_the_usual_ones_shown() {
        let cat = Catalog::default();
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(1400., 48.));
        let cells = table_cells(rect, &cat);
        for column in [7, 8, 9, 10] {
            assert_eq!(cells[column].width(), 0., "column {column} starts hidden");
        }
        for column in [0, 1, 2, 3, 4, 5, 6, COLUMN_DELETE] {
            assert!(cells[column].width() > 0.);
        }
        // Every column can be shown and hidden except 표시, 제목 and 내용.
        let toggled: Vec<usize> = TOGGLE_COLUMNS.iter().map(|c| c.0).collect();
        assert_eq!(toggled, vec![1, 4, 5, 6, 7, 8, 9, 10, COLUMN_DELETE]);
        let mut all = Catalog::default();
        all.column_shown.0 = [true; COLUMN_COUNT];
        let cells = table_cells(rect, &all);
        assert!(cells.iter().all(|c| c.width() > 0.));
        // The columns still fill the table exactly.
        assert!((cells[COLUMN_DELETE].right() - rect.right()).abs() < 1.);
    }

    #[test]
    fn dragging_a_left_edge_resizes_columns_and_the_body_takes_the_rest() {
        let mut cat = Catalog::default();
        let total = 1400.;
        let widths = column_widths(&cat, total);
        // 일정 (6): its left edge dragged left by 40 makes it 40 wider.
        drag_border(&mut cat, &widths, 5, 6, -40.);
        assert_eq!(cat.column_widths[6], widths[6] + 40.);
        // Dragging right makes it narrower, down to the minimum.
        drag_border(&mut cat, &widths, 5, 6, 10_000.);
        assert_eq!(cat.column_widths[6], COLUMN_MIN_WIDTH);
        // Before the body, the column before the border changes instead: the
        // border between 완료 and 제목 resizes 완료, the one between 제목 and
        // 내용 resizes 제목.
        drag_border(&mut cat, &widths, 1, 2, 20.);
        assert_eq!(cat.column_widths[1], widths[1] + 20.);
        drag_border(&mut cat, &widths, 2, 3, 30.);
        assert_eq!(cat.column_widths[2], widths[2] + 30.);
        // 표시 can be resized from the border after it.
        drag_border(&mut cat, &widths, 0, 1, 10.);
        assert_eq!(cat.column_widths[0], widths[0] + 10.);
        // The body (내용) follows: it takes what the others leave.
        let after = column_widths(&cat, total);
        assert!(after[3] < widths[3] + 40.);
        let right: f32 = after.iter().sum();
        assert!((right - total).abs() < 1., "{right} vs {total}");
    }

    #[test]
    fn many_wide_columns_widen_the_table_instead_of_squeezing_the_text() {
        let mut cat = Catalog::default();
        cat.column_shown.0 = [true; COLUMN_COUNT];
        for column in [0, 1, 4, 5, 6, 7, 8, 9, 10, COLUMN_DELETE] {
            cat.column_widths[column] = 200.;
        }
        cat.column_widths[COLUMN_TITLE] = 300.;
        let needed = table_total(&cat, 900.);
        assert!(needed > 900., "{needed}");
        let widths = column_widths(&cat, 900.);
        assert!(widths[COLUMN_BODY] >= BODY_MIN_WIDTH);
        assert!((widths.iter().sum::<f32>() - needed).abs() < 1.);
    }

    #[test]
    fn the_header_menu_lists_every_column_that_can_be_shown() {
        let mut a = App::sample(Local::now().date_naive());
        let ctx = egui::Context::default();
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(600., 600.),
                )),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| a.column_choices(ui));
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
        for label in [
            "완료",
            "알람",
            "생성일",
            "일정",
            "완료일",
            "위키",
            "프로젝트",
            "중요",
            "삭제",
        ] {
            assert!(texts.iter().any(|t| t == label), "missing {label}");
        }
    }

    #[test]
    fn chosen_extra_columns_show_the_memo_s_values() {
        let mut a = representative_table_app();
        let ctx = egui::Context::default();
        a.data.notes[0].starred = true;
        a.data.notes[0].wiki = true;
        a.data.notes[0].project = Some(0);
        a.catalog.column_shown.0 = [true; COLUMN_COUNT];
        render_table_layout(&mut a, &ctx, 1800.);
        let out = render_table_layout(&mut a, &ctx, 1800.);
        let project = a.data.projects[0].clone();
        for text in ["프로젝트", "완료일", "위키", "중요", "★", project.as_str()] {
            text_rect(&out, text, 1800.);
        }
    }

    #[test]
    fn folder_drop_uses_real_rendered_note_and_folder_bounds() {
        let mut a = App::sample(Local::now().date_naive());
        let folder = a.catalog.create_folder();
        a.catalog.rename = None;
        a.catalog.tab = FOLDER_TAB;
        let ctx = egui::Context::default();
        let draw = |a: &mut App, events| {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::new(1180., 760.),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| a.catalog_ui(ui));
                },
            );
        };
        draw(&mut a, vec![]);
        draw(&mut a, vec![]);
        let from = a
            .catalog
            .drag_sources
            .iter()
            .find_map(|(r, p)| {
                if matches!(p,FolderDrag::Notes(n) if n==&vec![0]) {
                    Some(r.center())
                } else {
                    None
                }
            })
            .unwrap();
        let to = a
            .catalog
            .drag_targets
            .iter()
            .find_map(|(r, f)| {
                if *f == Some(folder) {
                    Some(r.center())
                } else {
                    None
                }
            })
            .unwrap();
        draw(
            &mut a,
            vec![
                egui::Event::PointerMoved(from),
                egui::Event::PointerButton {
                    pos: from,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
                egui::Event::PointerMoved(to),
                egui::Event::PointerButton {
                    pos: to,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: Default::default(),
                },
            ],
        );
        assert_eq!(
            a.catalog.assignments.get(&0),
            Some(&folder),
            "{from:?} -> {to:?}, {}",
            a.toast
        );
    }
    #[test]
    fn rapid_note_drop_uses_press_source_and_keeps_folder_identity() {
        let mut a = App::sample(Local::now().date_naive());
        let folder = a.catalog.create_folder();
        a.catalog.drag_sources = vec![(
            egui::Rect::from_min_size(egui::pos2(0., 100.), Vec2::new(200., 30.)),
            FolderDrag::Notes(vec![0, 1]),
        )];
        a.catalog.drag_targets = vec![(
            egui::Rect::from_min_size(egui::pos2(0., 0.), Vec2::new(200., 30.)),
            Some(folder),
        )];
        a.catalog.tab = FOLDER_TAB;
        let ctx = egui::Context::default();
        let press = egui::pos2(40., 110.);
        let release = egui::pos2(40., 10.);
        let events = vec![
            egui::Event::PointerButton {
                pos: press,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
            egui::Event::PointerMoved(release),
            egui::Event::PointerButton {
                pos: release,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            },
        ];
        let _ = ctx.run(
            egui::RawInput {
                events,
                ..Default::default()
            },
            |ctx| a.catalog_drag_input(ctx),
        );
        assert_eq!(a.catalog.assignments.get(&0), Some(&folder));
        assert_eq!(a.catalog.assignments.get(&1), Some(&folder));
        assert_eq!(a.catalog.folders[folder].parent, None);
    }

    #[test]
    fn project_drop_uses_the_displayed_selection_as_one_command() {
        let mut app = App::sample(NaiveDate::from_ymd_opt(2026, 11, 23).unwrap());
        app.catalog.tab = PROJECT_TAB;
        app.catalog.project_expanded.insert(ProjectBranch::Unfiled);
        app.data.notes[0].project = None;
        app.data.notes[1].project = None;
        let project = app.data.projects.len();
        app.data.projects.push("검증 프로젝트".into());
        app.catalog.selection.active = true;
        app.catalog.selection.items.extend([0, 1]);
        let ctx = egui::Context::default();
        let run = |app: &mut App, events| {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::new(1380., 760.),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| app.catalog_ui(ui));
                },
            );
        };
        run(&mut app, vec![]);
        run(&mut app, vec![]);
        let from = app
            .catalog
            .drag_sources
            .iter()
            .find_map(|(rect, source)| {
                matches!(source, FolderDrag::Notes(ids) if ids.as_slice() == [0])
                    .then_some(rect.center())
            })
            .unwrap();
        let to = app
            .catalog
            .batch_targets
            .iter()
            .find_map(|(rect, target)| {
                matches!(target, DropTarget::Project(location) if location.project == Some(project) && location.folder.is_none())
                    .then_some(rect.center())
            })
            .unwrap();
        run(
            &mut app,
            vec![
                egui::Event::PointerMoved(from),
                egui::Event::PointerButton {
                    pos: from,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
        );
        run(
            &mut app,
            vec![
                egui::Event::PointerMoved(to),
                egui::Event::PointerButton {
                    pos: to,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: Default::default(),
                },
            ],
        );
        assert_eq!(
            app.data.notes[0].project,
            Some(project),
            "{from:?} -> {to:?}"
        );
        assert_eq!(app.data.notes[1].project, Some(project));
        assert_eq!(app.catalog.undo.len(), 1);
    }
    #[test]
    fn creating_siblings_keeps_parent_context_and_roots_remain_possible() {
        let mut c = Catalog::default();
        let a = c.create_folder();
        c.folder_selected = Some(a);
        let b = c.create_folder();
        let d = c.create_folder();
        assert_eq!(c.folders[b].parent, Some(a));
        assert_eq!(c.folders[d].parent, Some(a));
        c.folder_selected = None;
        let root = c.create_folder();
        assert_eq!(c.folders[root].parent, None);
    }
    #[test]
    fn move_rejects_descendants_and_deleting_folder_preserves_other_assignments() {
        let mut c = Catalog::default();
        let root = c.create_folder();
        c.folder_selected = Some(root);
        let child = c.create_folder();
        assert!(!c.move_folder(root, Some(child)));
        c.assign_notes(&[0, 1], Some(child));
        c.assign_notes(&[2], Some(root));
        c.remove_folder(child);
        assert!(!c.assignments.contains_key(&0));
        assert!(!c.assignments.contains_key(&1));
        assert_eq!(c.assignments[&2], root);
    }
    #[test]
    fn batch_assignment_moves_originals_and_search_uses_character_boundaries() {
        let mut c = Catalog::default();
        let a = c.create_folder();
        let b = c.create_folder();
        c.assign_notes(&[4, 7], Some(a));
        c.assign_notes(&[4, 7], Some(b));
        assert_eq!(c.assignments.len(), 2);
        assert_eq!(c.assignments[&4], b);
        assert_eq!(find_ranges("가나다🙂 가나다", "가나다"), vec![0..3, 5..8]);
        assert_eq!(find_ranges("한글 Memo와 MEMO", "memo"), vec![3..7, 9..13]);
    }
    #[test]
    fn find_tracks_all_tabs_and_unicode_locations_without_locked_body_leaks() {
        let mut a = App::sample(Local::now().date_naive());
        let n = &mut a.data.notes[0];
        n.title = "제목".into();
        n.body = "한글🙂 찾기 찾기".into();
        n.document.tabs[0].body = "오래된 첫 탭".into();
        n.document.add_tab();
        n.document.tabs[1].body = "숨긴 찾기".into();
        n.document.tabs[1].hidden = true;
        a.catalog.query = "찾기".into();
        assert_eq!(
            a.catalog_find_hits(&[0, 0]),
            vec![
                (0, Some((0, 4..6))),
                (0, Some((0, 7..9))),
                (0, Some((1, 3..5))),
            ]
        );
        a.data.notes[0].locked = true;
        assert!(a.catalog_find_hits(&[0]).is_empty());
        a.data.notes[0].title = "찾기 제목".into();
        assert_eq!(a.catalog_find_hits(&[0]), vec![(0, None)]);
    }
    #[test]
    fn replacement_scopes_cover_other_tabs_and_keep_excluded_notes_unchanged() {
        let mut a = App::sample(Local::now().date_naive());
        for n in a.data.notes.iter_mut() {
            n.body = "첫 본문".into();
            n.document.add_tab();
            n.document.tabs[1].body = "대상 한글🙂 대상".into();
        }
        a.data.notes[0].body = "대상 첫 본문".into();
        a.data.notes[2].project = Some(1);
        a.data.notes[3].deleted = true;
        a.data.notes[4].locked = true;
        a.extra.hidden_projects.insert(1);
        a.catalog.selected = Some(0);
        a.catalog.replace_find = "대상".into();
        a.catalog.replace_with = "교체된 내용".into();
        assert_eq!(a.catalog_replace_targets(), vec![0]);
        a.catalog.replace_scope = 1;
        assert_eq!(a.catalog_replace_targets(), vec![0, 1]);
        a.catalog.replace_scope = 2;
        let targets = a.catalog_replace_targets();
        assert_eq!(targets, vec![0, 1, 5]);
        a.catalog_apply_replacement(&targets);
        for i in [0, 1, 5] {
            let n = &a.data.notes[i];
            assert_eq!(n.document.tabs[1].body, "교체된 내용 한글🙂 교체된 내용");
            assert_eq!(n.body, n.document.tabs[0].body);
        }
        assert_eq!(a.data.notes[0].body, "교체된 내용 첫 본문");
        for i in [2, 3, 4] {
            assert_eq!(a.data.notes[i].document.tabs[1].body, "대상 한글🙂 대상");
        }
        a.catalog.replace_find.clear();
        assert!(a.catalog_replace_targets().is_empty());
    }
    #[test]
    fn replacement_keeps_formatting_on_multibyte_text_after_length_changes() {
        let mut a = App::sample(Local::now().date_naive());
        let n = &mut a.data.notes[0];
        n.body = "가🙂 끝".into();
        n.document.tabs[0].body = n.body.clone();
        n.document.tabs[0].spans.push(crate::composer::Span {
            range: 3..4,
            format: crate::composer::Format {
                bold: true,
                ..Default::default()
            },
        });
        n.document.add_tab();
        n.document.tabs[1].body = "가🙂 가🙂 끝".into();
        n.document.tabs[1].spans.push(crate::composer::Span {
            range: 6..7,
            format: crate::composer::Format {
                underline: true,
                ..Default::default()
            },
        });
        a.catalog.replace_find = "가🙂".into();
        a.catalog.replace_with = "늘어난글".into();
        a.catalog_apply_replacement(&[0]);
        let n = &a.data.notes[0];
        assert_eq!(n.body, "늘어난글 끝");
        assert_eq!(n.document.tabs[0].spans[0].range, 5..6);
        assert_eq!(n.document.tabs[1].body, "늘어난글 늘어난글 끝");
        assert_eq!(n.document.tabs[1].spans[0].range, 10..11);
        assert!(n.document.tabs[1].spans[0].format.underline);
    }
    #[test]
    fn deleted_tab_restore_keeps_content_and_recovers_its_deleted_owner() {
        let mut a = App::sample(Local::now().date_naive());
        let n = &mut a.data.notes[0];
        n.document.tabs[0].title = "참고 자료".into();
        n.document.tabs[0].body = "이 내용을 복원합니다".into();
        n.document.tabs[0].hidden = true;
        n.document.tabs[0]
            .blocks
            .push(crate::composer::Block::Table(crate::composer::Table::new(
                2, 3,
            )));
        n.document.delete_tab(0);
        let id = n.document.trash[0].id;
        n.body.clear();
        n.deleted = true;
        assert!(a.catalog_restore_tab(0, id));
        let n = &a.data.notes[0];
        let tab = &n.document.tabs[n.document.active];
        assert!(!n.deleted);
        assert_eq!(tab.title, "참고 자료");
        assert_eq!(tab.body, "이 내용을 복원합니다");
        assert!(!tab.hidden);
        assert_eq!(tab.blocks.len(), 1);
        assert!(n.document.trash.is_empty());
    }
    #[test]
    fn deleted_tab_purge_requires_pending_target_and_never_purges_other_tabs() {
        let mut a = App::sample(Local::now().date_naive());
        a.data.notes[0].document.add_tab();
        a.data.notes[0].document.tabs[1].body = "첫 번째 삭제 탭".into();
        a.data.notes[0].document.delete_tab(1);
        a.data.notes[0].document.add_tab();
        a.data.notes[0].document.tabs[1].body = "남겨둘 삭제 탭".into();
        a.data.notes[0].document.delete_tab(1);
        assert_eq!(a.data.notes[0].document.trash.len(), 2);
        let id = a.data.notes[0].document.trash[0].id;
        a.request_trash_purge(PurgeKind::Tab, Some(TrashItem::Tab(0, id)));
        let request = a.desktop.purge_target.take().unwrap();
        a.data.notes[0].document.trash.swap(0, 1);
        assert!(a.confirm_trash_purge(request).is_some());
        assert_eq!(a.data.notes[0].document.trash.len(), 1);
        assert_eq!(a.data.notes[0].document.trash[0].body, "남겨둘 삭제 탭");
        assert_eq!(a.data.notes[0].document.tabs.len(), 1);
    }
    #[test]
    fn detailed_list_inside_outer_scroll_paints_multiple_complete_rows() {
        let mut a = App::sample(Local::now().date_naive());
        a.split = false;
        let ctx = egui::Context::default();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(1180., 760.));
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            },
            |ctx| {
                egui::SidePanel::left("test_sidebar")
                    .exact_width(220.)
                    .show(ctx, |_| {});
                egui::TopBottomPanel::top("test_toolbar")
                    .exact_height(54.)
                    .show(ctx, |_| {});
                egui::CentralPanel::default().show(ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.add_space(20.);
                            ui.vertical(|ui| {
                                ui.set_min_width((ui.available_width() - 25.).max(200.));
                                a.detail_workspace(ui, 0);
                            });
                        });
                    });
                });
            },
        );
        let second_row = output
            .shapes
            .iter()
            .filter_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape {
                    if text.galley.job.text.contains("화면 흐름 검토") {
                        let bounds = text.galley.rect.translate(text.pos.to_vec2());
                        return Some(shape.clip_rect.contains(bounds.center()));
                    }
                }
                None
            })
            .collect::<Vec<_>>();
        assert!(
            !second_row.is_empty(),
            "second row must be painted, rather than clipped to one line"
        );
        assert!(second_row.into_iter().all(|visible| visible));
    }
}
