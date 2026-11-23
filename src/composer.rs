//! In-memory memo content editor. Tabs share the parent memo's metadata.
use eframe::egui::{
    self,
    text::{CCursor, CCursorRange, LayoutJob},
    Color32, FontFamily, FontId, Id, Key, Modifiers, Rect, RichText, Sense, Stroke, TextFormat,
    Vec2,
};
use std::{hash::Hash, ops::Range, path::Path};

/// Fonts offered throughout the app (the toolbar's font picker, the
/// settings page's font pills), each mapped from a file under
/// `C:/Windows/Fonts` by `main.rs`'s font loader. `index` selects a face
/// inside a multi-face file (`.ttc`); 0 for an ordinary `.ttf`.
///
/// An entry named "<name> Bold" is not a separate choice — the picker
/// never lists it (see `font_choices`) — it is the face `font_id` looks up
/// on its own for bold text in that family. Most families here have no
/// such entry and fall back to the default font's bold face instead,
/// matching the limitation already accepted for 굴림/돋움/바탕/궁서체.
pub const FONTS: &[(&str, &str, u32)] = &[
    ("맑은 고딕", "malgun.ttf", 0),
    ("맑은 고딕 Bold", "malgunbd.ttf", 0),
    ("굴림", "gulim.ttc", 0),
    ("돋움", "gulim.ttc", 2),
    ("바탕", "batang.ttc", 0),
    ("궁서체", "batang.ttc", 2),
    // "개구쟁이" was never an installed font, so selecting it silently
    // fell back to the default face. 휴먼매직체 is the closest playful
    // hand-lettered face actually present in C:/Windows/Fonts.
    ("휴먼매직체", "HMKMMAG.TTF", 0),
    ("새굴림", "NGULIM.TTF", 0),
    ("HY중고딕", "H2GTRM.TTF", 0),
    ("HY견고딕", "H2GTRE.TTF", 0),
    ("HY신명조", "H2MJSM.TTF", 0),
    ("HY궁서B", "H2GSRB.TTF", 0),
    ("HY헤드라인M", "H2HDRM.TTF", 0),
    ("휴먼둥근헤드라인", "HMKMRHD.TTF", 0),
    ("휴먼아미체", "HMKMAMI.TTF", 0),
    ("휴먼편지체", "HMFMPYUN.TTF", 0),
    ("Noto Sans KR", "NotoSansKR-VF.ttf", 0),
    ("Calibri", "calibri.ttf", 0),
    ("Calibri Bold", "calibrib.ttf", 0),
    ("Georgia", "georgia.ttf", 0),
    ("Georgia Bold", "georgiab.ttf", 0),
    ("Consolas", "consola.ttf", 0),
    ("Consolas Bold", "consolab.ttf", 0),
    ("Verdana", "verdana.ttf", 0),
    ("Verdana Bold", "verdanab.ttf", 0),
    ("Segoe UI", "segoeui.ttf", 0),
    ("Segoe UI Bold", "segoeuib.ttf", 0),
];
/// The names worth showing in a font picker: every `FONTS` entry except
/// the synthetic "<name> Bold" faces, which are not choices of their own.
pub fn font_choices() -> impl Iterator<Item = &'static str> {
    FONTS
        .iter()
        .map(|&(name, _, _)| name)
        .filter(|name| !name.ends_with(" Bold"))
}

#[derive(Clone, Default, Debug, PartialEq)]
pub struct Format {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    /// Text color (`TEXT_COLORS`; 0: the memo's own).
    pub color: u8,
    /// Highlighter (`HIGHLIGHTS`; 0: none).
    pub highlight: u8,
}
/// Text colors offered in the toolbar (the first: the memo's own color).
pub const TEXT_COLORS: [(&str, Color32); 7] = [
    ("기본", Color32::TRANSPARENT),
    ("빨강", Color32::from_rgb(214, 48, 49)),
    ("주황", Color32::from_rgb(225, 112, 0)),
    ("초록", Color32::from_rgb(30, 140, 70)),
    ("파랑", Color32::from_rgb(35, 100, 210)),
    ("보라", Color32::from_rgb(130, 60, 190)),
    ("회색", Color32::from_rgb(128, 128, 128)),
];
/// Highlighter colors (the first: none).
pub const HIGHLIGHTS: [(&str, Color32); 6] = [
    ("없음", Color32::TRANSPARENT),
    ("노랑", Color32::from_rgb(255, 236, 100)),
    ("연두", Color32::from_rgb(178, 240, 150)),
    ("하늘", Color32::from_rgb(160, 215, 255)),
    ("분홍", Color32::from_rgb(255, 185, 215)),
    ("주황", Color32::from_rgb(255, 205, 140)),
];
#[derive(Clone, Debug, PartialEq)]
pub struct Span {
    pub range: Range<usize>,
    pub format: Format,
}
#[derive(Clone)]
pub struct Cell {
    pub text: String,
    pub format: Format,
    /// Columns this cell spans, including itself. 1 for a normal cell.
    pub col_span: usize,
    /// Rows this cell spans, including itself. 1 for a normal cell.
    pub row_span: usize,
    /// True when a span from another cell (above or to the left) covers
    /// this grid position. A covered cell holds no text of its own and is
    /// never drawn or focused — the grid stays rectangular underneath the
    /// merge so row/column indices keep working normally.
    pub covered: bool,
    /// Fill color (RGB); none keeps the theme's background.
    pub fill: Option<[u8; 3]>,
    /// Text color (RGB); none keeps the theme's text color.
    pub text_color: Option<[u8; 3]>,
    pub align: CellAlign,
}
/// Horizontal alignment of a cell's text. `Default` reads as left.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CellAlign {
    #[default]
    Default,
    Left,
    Center,
    Right,
}
impl CellAlign {
    pub fn to_u8(self) -> u8 {
        match self {
            Self::Default => 0,
            Self::Left => 1,
            Self::Center => 2,
            Self::Right => 3,
        }
    }
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Left,
            2 => Self::Center,
            3 => Self::Right,
            _ => Self::Default,
        }
    }
    /// 0 for left, 0.5 for center, 1 for right.
    fn factor(self) -> f32 {
        match self {
            Self::Center => 0.5,
            Self::Right => 1.0,
            _ => 0.0,
        }
    }
    fn egui(self) -> egui::Align {
        match self {
            Self::Center => egui::Align::Center,
            Self::Right => egui::Align::RIGHT,
            _ => egui::Align::LEFT,
        }
    }
}
impl Cell {
    pub(crate) fn plain() -> Self {
        Self {
            text: String::new(),
            format: Format::default(),
            col_span: 1,
            row_span: 1,
            covered: false,
            fill: None,
            text_color: None,
            align: CellAlign::Default,
        }
    }
}
/// The cell's fill and the color its text is drawn in: its own text color,
/// else black or white for contrast against its fill, else the theme's.
fn cell_colors(cell: &Cell, theme_text: Color32) -> (Option<Color32>, Color32) {
    let fill = cell.fill.map(|c| Color32::from_rgb(c[0], c[1], c[2]));
    let text = match (cell.text_color, fill) {
        (Some(c), _) => Color32::from_rgb(c[0], c[1], c[2]),
        (None, Some(f)) => {
            let luma = (299 * f.r() as u32 + 587 * f.g() as u32 + 114 * f.b() as u32) / 1000;
            if luma > 140 {
                Color32::BLACK
            } else {
                Color32::WHITE
            }
        }
        (None, None) => theme_text,
    };
    (fill, text)
}
#[derive(Clone)]
pub struct Table {
    /// Unique within the tab's blocks (pictures included). Only meaningful
    /// while `positioned` is true: it is the placeholder character this
    /// table's body text carries.
    pub id: u32,
    pub(crate) instance: u64,
    pub cells: Vec<Vec<Cell>>,
    pub col_widths: Vec<f32>,
    /// The least a row is high, and the height of one with no content.
    pub row_height: f32,
    /// One entry per row: the height the user dragged the row to (at zoom 1),
    /// or 0 for a row that grows with its content.
    pub row_manual: Vec<f32>,
    /// True once the table has been placed in the body text at a specific
    /// character position (via `Tab::insert_table`), the same way a
    /// picture is: it then renders inline there and only while its
    /// placeholder survives in the text. A table built before this (by
    /// `new`/`from_tsv`/`from_html`, or migrated from a save made before
    /// tables could be positioned) is `false` and keeps the original
    /// behavior of always rendering below the tab's text.
    pub positioned: bool,
}
impl Table {
    pub fn new(rows: usize, cols: usize) -> Self {
        Self {
            id: 0,
            instance: new_block_instance(),
            cells: vec![vec![Cell::plain(); cols.clamp(1, 20)]; rows.clamp(1, 100)],
            col_widths: vec![120.; cols.clamp(1, 20)],
            row_height: 32.,
            row_manual: vec![0.; rows.clamp(1, 100)],
            positioned: false,
        }
    }
    /// `row_manual` always has one entry per row.
    pub fn fit_rows(&mut self) {
        self.row_manual.resize(self.cells.len(), 0.);
    }
    pub fn tsv(&self) -> String {
        self.cells
            .iter()
            .map(|row| {
                row.iter()
                    .map(|c| c.text.replace(['\t', '\n'], " "))
                    .collect::<Vec<_>>()
                    .join("\t")
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
    pub fn from_tsv(s: &str) -> Self {
        crate::table_edit::table_from_grid(crate::table_edit::parse_tsv(s))
    }
    /// Build a table from clipboard HTML (the real `<table>` markup that
    /// Excel/한글/Word/browsers place next to their plain-text rendering).
    /// See `table_edit::table_from_html` for how the structure is read.
    pub fn from_html(html: &str) -> Option<Self> {
        crate::table_edit::table_from_html(html)
    }
}
/// An image stored in a tab. It is shown inline wherever its placeholder
/// character (`placeholder(id)`) appears in the tab's text, so it moves,
/// wraps, copies and deletes exactly like a character.
#[derive(Clone)]
pub struct Picture {
    pub id: u32,
    instance: u64,
    pub name: String,
    pub pixels: Vec<u8>,
    pub width: usize,
    pub height: usize,
    pub scale: f32,
    pub fit: bool,
    /// File name in the data folder's `images` folder once saved. A saved
    /// picture keeps no pixels in memory: they are read from the file only
    /// while it is shown, copied or exported.
    pub file: String,
    texture: Option<egui::TextureHandle>,
    /// When the texture was last drawn (egui time).
    texture_used: f64,
}
impl Picture {
    /// A picture from RGBA pixels (used when loading saved memos).
    pub fn from_rgba(
        id: u32,
        name: String,
        pixels: Vec<u8>,
        size: [usize; 2],
        scale: f32,
        fit: bool,
    ) -> Self {
        Self {
            id,
            instance: new_block_instance(),
            name,
            pixels,
            width: size[0],
            height: size[1],
            scale,
            fit,
            file: String::new(),
            texture: None,
            texture_used: 0.,
        }
    }
    /// A saved picture whose pixels stay in its file until shown.
    pub fn saved(
        id: u32,
        name: String,
        file: String,
        size: [usize; 2],
        scale: f32,
        fit: bool,
    ) -> Self {
        let mut p = Self::from_rgba(id, name, vec![], size, scale, fit);
        p.file = file;
        p
    }
    /// The RGBA pixels, read from the saved file when not in memory. Empty
    /// when neither is available.
    pub fn rgba(&self) -> std::borrow::Cow<'_, [u8]> {
        if self.width > 0 && self.pixels.len() == self.width * self.height * 4 {
            std::borrow::Cow::Borrowed(&self.pixels)
        } else {
            crate::storage::load_picture(&self.file, [self.width, self.height])
                .map(std::borrow::Cow::Owned)
                .unwrap_or(std::borrow::Cow::Borrowed(&[]))
        }
    }
    /// Free the texture of a saved picture not drawn since `before`; it is
    /// read from its file again when next shown.
    pub fn release_texture(&mut self, before: f64) {
        if self.texture_used < before && !self.file.is_empty() {
            self.texture = None;
        }
    }
    fn color_image(&self) -> Option<egui::ColorImage> {
        let px = self.rgba();
        (self.width > 0 && px.len() == self.width * self.height * 4)
            .then(|| egui::ColorImage::from_rgba_unmultiplied([self.width, self.height], &px))
    }
}
#[derive(Clone)]
pub enum Block {
    Table(Table),
    Picture(Picture),
}
pub(crate) fn new_block_instance() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

impl Block {
    pub(crate) fn identity(&self) -> (u32, bool, u64) {
        match self {
            Self::Table(table) => (table.id, false, table.instance),
            Self::Picture(picture) => (picture.id, true, picture.instance),
        }
    }

    fn marker(&self) -> Option<char> {
        match self {
            Self::Picture(picture) => Some(placeholder(picture.id)),
            Self::Table(table) if table.positioned => Some(placeholder(table.id)),
            Self::Table(_) => None,
        }
    }
}
#[derive(Clone)]
pub struct Tab {
    pub id: u64,
    pub title: String,
    pub body: String,
    pub hidden: bool,
    pub spans: Vec<Span>,
    pub blocks: Vec<Block>,
}
/// Set (temporarily) by a host that adds its own entries to the text's
/// right-click menu; the entry picked is left under `HOST_ACTION`.
pub const HOST_MENU: &str = "composer_host_menu";
pub const HOST_ACTION: &str = "composer_host_action";
/// Set (temporarily) by a memo window: the text's right-click menu offers
/// "글꼴…" (left under `HOST_ACTION`) and the window's opacity.
pub const HOST_STYLE: &str = "composer_host_style";
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HostAction {
    Schedule,
    Alarm,
    StickerMenu,
    Trash,
    Purge,
    Font,
}

pub const PICTURE_BASE: u32 = 0xE000;
pub fn placeholder(id: u32) -> char {
    char::from_u32(PICTURE_BASE + id.min(0x18FF)).unwrap_or('\u{E000}')
}
fn picture_id(c: char) -> Option<u32> {
    let v = c as u32;
    (PICTURE_BASE..=PICTURE_BASE + 0x18FF)
        .contains(&v)
        .then(|| v - PICTURE_BASE)
}
impl Tab {
    /// True when the block's placeholder is still present in the text. A
    /// table built before tables could be positioned (`!positioned`, see
    /// `Table::positioned`) has no placeholder at all and always shows,
    /// the same as before that existed.
    pub fn shows(&self, block: &Block) -> bool {
        match block {
            Block::Picture(p) => self.body.contains(placeholder(p.id)),
            Block::Table(t) => !t.positioned || self.body.contains(placeholder(t.id)),
        }
    }
    /// The next id unused by any picture or table in this tab, so a new
    /// placeholder character never collides with one already in the text.
    fn next_block_id(&self) -> u32 {
        self.blocks
            .iter()
            .map(|b| match b {
                Block::Picture(p) => p.id + 1,
                Block::Table(t) => t.id + 1,
            })
            .max()
            .unwrap_or(0)
    }
    /// Insert a picture at a character range, replacing it. Returns the caret.
    pub fn insert_picture(&mut self, range: Range<usize>, mut picture: Picture) -> usize {
        picture.id = self.next_block_id();
        picture.instance = new_block_instance();
        let c = placeholder(picture.id);
        self.blocks.push(Block::Picture(picture));
        replace_chars(&mut self.body, range.clone(), &c.to_string());
        range.start + 1
    }
    /// Insert a table at a character range, replacing it, the same way
    /// `insert_picture` does for an image: the table gets a fresh id, a
    /// placeholder character takes its place in the text, and it renders
    /// inline there from then on. Returns the caret.
    pub fn insert_table(&mut self, range: Range<usize>, mut table: Table) -> usize {
        table.id = self.next_block_id();
        table.instance = new_block_instance();
        table.positioned = true;
        let c = placeholder(table.id);
        self.blocks.push(Block::Table(table));
        replace_chars(&mut self.body, range.clone(), &c.to_string());
        range.start + 1
    }
    fn new(title: String) -> Self {
        Self {
            id: 0,
            title,
            body: String::new(),
            hidden: false,
            spans: vec![],
            blocks: vec![],
        }
    }

    fn remove_blocks(&mut self, indices: &[usize]) -> Option<Vec<usize>> {
        let indices: std::collections::HashSet<_> = indices.iter().copied().collect();
        let mut markers = std::collections::HashSet::new();
        let mut index = 0;
        self.blocks.retain(|block| {
            let remove = indices.contains(&index);
            index += 1;
            if remove {
                markers.extend(block.marker());
            }
            !remove
        });
        if markers.is_empty() {
            return None;
        }
        let mut cursor_map = vec![0];
        let mut after = 0;
        self.body.retain(|c| {
            let keep = !markers.contains(&c);
            after += usize::from(keep);
            cursor_map.push(after);
            keep
        });
        for span in &mut self.spans {
            span.range = map_removed_chars(&cursor_map, &span.range);
        }
        self.spans.retain(|span| !span.range.is_empty());
        Some(cursor_map)
    }
}

fn map_removed_chars(map: &[usize], range: &Range<usize>) -> Range<usize> {
    let last = map.len() - 1;
    map[range.start.min(last)]..map[range.end.min(last)]
}
// One retained body layout per editor. Compare compact source inputs before
// constructing or hashing the per-character LayoutJob.
#[derive(Clone, PartialEq)]
struct BodyLayoutKey {
    text: String,
    spans: Vec<Span>,
    font: FontId,
    bold_font: FontId,
    color: Color32,
    pictures: Vec<PictureSize>,
    tables: Vec<TableSize>,
    spacer: Option<egui::FontFamily>,
    width: f32,
    pixels_per_point: f32,
}
#[derive(Clone)]
struct BodyLayoutCache {
    key: BodyLayoutKey,
    galley: std::sync::Arc<egui::Galley>,
}
/// One reversible step. Body text and every table share one ordered list,
/// so Ctrl+Z walks back through what was done, in the order it was done.
enum UndoKind {
    /// The tab's text and formatting before an edit.
    Body {
        tab: usize,
        text: String,
        spans: Vec<Span>,
    },
    /// Body formatting can change without editing the text.
    Style { tab: usize, spans: Vec<Span> },
    /// A table as it was before a cell edit, paste, row/column change, merge ...
    Table { tab: usize, bi: usize, table: Table },
    /// A table was pasted into the text: undo removes it again.
    TableInserted {
        tab: usize,
        id: u32,
        text: String,
        spans: Vec<Span>,
    },
    /// A table was deleted: undo puts it back.
    TableDeleted {
        tab: usize,
        bi: usize,
        table: Table,
        text: String,
        spans: Vec<Span>,
    },
}
impl UndoKind {
    fn tab(&self) -> usize {
        match self {
            Self::Body { tab, .. }
            | Self::Style { tab, .. }
            | Self::Table { tab, .. }
            | Self::TableInserted { tab, .. }
            | Self::TableDeleted { tab, .. } => *tab,
        }
    }
}
struct UndoEntry {
    seq: u64,
    label: &'static str,
    kind: UndoKind,
    /// When it was last extended (egui time): typing runs are one step.
    at: f64,
}
/// Ordered undo/redo history. A copy of a document starts with no history.
#[derive(Default)]
pub struct UndoStack {
    entries: Vec<UndoEntry>,
    redo: Vec<UndoEntry>,
    next: u64,
}
impl Clone for UndoStack {
    fn clone(&self) -> Self {
        Self::default()
    }
}
impl UndoStack {
    const MAX: usize = 200;
    fn push(&mut self, label: &'static str, kind: UndoKind, now: f64) {
        self.redo.clear();
        if self.entries.len() >= Self::MAX {
            self.entries.remove(0);
        }
        self.next += 1;
        self.entries.push(UndoEntry {
            seq: self.next,
            label,
            kind,
            at: now,
        });
    }
    /// Typing in the same tab within a second of the last change is one step.
    fn push_body(&mut self, tab: usize, text: String, spans: Vec<Span>, now: f64) {
        if self.redo.is_empty() {
            if let Some(last) = self.entries.last_mut() {
                if matches!(&last.kind, UndoKind::Body { tab: t, .. } if *t == tab)
                    && now - last.at < 1.0
                {
                    last.at = now;
                    return;
                }
            }
        }
        self.push("본문 편집", UndoKind::Body { tab, text, spans }, now);
    }
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}
/// Where text `old` and `new` start to differ, and where the changed part
/// of `new` ends: the caret goes there after an undo.
fn change_end(old: &str, new: &str) -> usize {
    let a: Vec<char> = old.chars().collect();
    let b: Vec<char> = new.chars().collect();
    let head = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let tail = a[head..]
        .iter()
        .rev()
        .zip(b[head..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    b.len() - tail
}
#[derive(Clone)]
pub struct Document {
    pub tabs: Vec<Tab>,
    pub active: usize,
    pub trash: Vec<Tab>,
    next_tab_id: u64,
    pub font_size: f32,
    pub font_name: String,
    pub opacity: f32,
    pub toolbar_visible: bool,
    pub zoom: f32,
    pub requested_selection: Option<Range<usize>>,
    pub requested_tab: Option<usize>,
    pub status: String,
    pub image_scale: f32,
    pub image_fit: bool,
    pub format: Format,
    pub editing: bool,
    selection: Range<usize>,
    selected_cell: Option<(usize, usize, usize)>,
    rename: Option<usize>,
    tab_scroll: usize,
    ime: bool,
    zoom_until: f64,
    aliases: Vec<(String, String)>,
    first_tab_changed: bool,
    selected_picture: Option<u32>,
    /// Where a right click on the picture asked for its toolbar.
    picture_tool_at: Option<egui::Pos2>,
    /// The caret as of the last frame: a move that the editor did not scroll
    /// to (list helpers, undo, pastes) is followed here.
    last_caret: usize,
    tsel: Option<TSel>,
    undo: UndoStack,
    undo_cursor: Option<usize>,
    undo_focus: bool,
}
impl Default for Document {
    fn default() -> Self {
        let mut first = Tab::new("본문".into());
        first.id = 1;
        Self {
            tabs: vec![first],
            active: 0,
            trash: vec![],
            next_tab_id: 2,
            font_size: 13.333,
            font_name: "맑은 고딕".into(),
            opacity: 1.,
            toolbar_visible: true,
            zoom: 1.,
            requested_selection: None,
            requested_tab: None,
            status: String::new(),
            image_scale: 1.,
            image_fit: false,
            format: Format::default(),
            editing: false,
            selection: 0..0,
            selected_cell: None,
            rename: None,
            tab_scroll: 0,
            ime: false,
            zoom_until: 0.,
            aliases: vec![],
            first_tab_changed: false,
            selected_picture: None,
            picture_tool_at: None,
            last_caret: 0,
            tsel: None,
            undo: UndoStack::default(),
            undo_cursor: None,
            undo_focus: false,
        }
    }
}
impl Document {
    pub fn normalize_tab_ids(&mut self) {
        let mut reserved: std::collections::HashSet<_> = self
            .tabs
            .iter()
            .chain(&self.trash)
            .map(|tab| tab.id)
            .filter(|&id| id != 0)
            .collect();
        let mut seen = std::collections::HashSet::new();
        let highest = reserved.iter().copied().max().unwrap_or(0);
        let mut next = highest.wrapping_add(1).max(1);
        for tab in self.tabs.iter_mut().chain(&mut self.trash) {
            if tab.id != 0 && seen.insert(tab.id) {
                continue;
            }
            while reserved.contains(&next) {
                next = next.wrapping_add(1).max(1);
            }
            tab.id = next;
            reserved.insert(next);
            next = next.wrapping_add(1).max(1);
        }
        self.next_tab_id = next;
    }
    fn push_tab(&mut self, title: String) {
        let mut tab = Tab::new(title);
        while self
            .tabs
            .iter()
            .chain(&self.trash)
            .any(|existing| existing.id == self.next_tab_id)
        {
            self.next_tab_id = self.next_tab_id.wrapping_add(1).max(1);
        }
        tab.id = self.next_tab_id;
        self.next_tab_id = self.next_tab_id.wrapping_add(1).max(1);
        self.tabs.push(tab);
    }
    pub fn plain_text(&self, first_body: &str) -> String {
        let mut parts = vec![strip_pictures(first_body)];
        for (i, t) in self.tabs.iter().enumerate() {
            if i > 0 {
                parts.push(strip_pictures(&t.body));
            }
            for b in &t.blocks {
                if let Block::Table(t) = b {
                    parts.push(t.tsv());
                }
            }
        }
        parts.join("\n")
    }
    pub fn has_images(&self) -> bool {
        self.tabs.iter().any(|t| {
            t.blocks
                .iter()
                .any(|b| matches!(b, Block::Picture(_)) && t.shows(b))
        })
    }
    pub fn has_tables(&self) -> bool {
        self.tabs
            .iter()
            .any(|t| t.blocks.iter().any(|b| matches!(b, Block::Table(_))))
    }
    /// Number of steps that can still be undone.
    #[cfg(test)]
    pub fn undo_steps(&self) -> usize {
        self.undo.len()
    }
    /// Undoes the latest step of the body or of any table. Returns the text
    /// for the status line, `None` when there is nothing to undo.
    pub fn undo(&mut self) -> Option<String> {
        self.apply_history(false)
    }
    pub fn redo(&mut self) -> Option<String> {
        self.apply_history(true)
    }
    fn apply_history(&mut self, redo: bool) -> Option<String> {
        let e = if redo {
            self.undo.redo.pop()?
        } else {
            self.undo.entries.pop()?
        };
        let tab = e.kind.tab();
        let current = self.tabs.get(tab)?;
        let inverse_kind = match &e.kind {
            UndoKind::Body { .. } => UndoKind::Body {
                tab,
                text: current.body.clone(),
                spans: current.spans.clone(),
            },
            UndoKind::Style { .. } => UndoKind::Style {
                tab,
                spans: current.spans.clone(),
            },
            UndoKind::Table { bi, .. } => {
                let Block::Table(table) = current.blocks.get(*bi)? else {
                    return None;
                };
                UndoKind::Table {
                    tab,
                    bi: *bi,
                    table: table.clone(),
                }
            }
            UndoKind::TableInserted { id, .. } => {
                let bi = current
                    .blocks
                    .iter()
                    .position(|b| matches!(b, Block::Table(t) if t.id == *id))?;
                let Block::Table(table) = &current.blocks[bi] else {
                    return None;
                };
                UndoKind::TableDeleted {
                    tab,
                    bi,
                    table: table.clone(),
                    text: current.body.clone(),
                    spans: current.spans.clone(),
                }
            }
            UndoKind::TableDeleted { table, .. } => UndoKind::TableInserted {
                tab,
                id: table.id,
                text: current.body.clone(),
                spans: current.spans.clone(),
            },
        };
        let inverse = UndoEntry {
            seq: e.seq,
            label: e.label,
            at: f64::NEG_INFINITY,
            kind: inverse_kind,
        };
        let label = format!(
            "{} #{} · {}",
            if redo {
                "다시 실행"
            } else {
                "실행 취소"
            },
            e.seq,
            e.label
        );
        // A table step keeps the table selected (so the next Ctrl+Z still
        // reaches the history); any other step hands focus to the body.
        if matches!(e.kind, UndoKind::Table { .. }) {
            if let Some(s) = &mut self.tsel {
                s.editing = false;
                s.drag = false;
                s.orig.clear();
            }
        } else {
            self.tsel = None;
            self.selected_cell = None;
            self.undo_focus = true;
        }
        match e.kind {
            UndoKind::Body { tab, text, spans } => {
                let t = self.tabs.get_mut(tab)?;
                self.undo_cursor = Some(change_end(&t.body, &text));
                self.undo_focus = true;
                t.body = text;
                t.spans = spans;
                self.active = tab;
            }
            UndoKind::Style { tab, spans } => {
                self.tabs.get_mut(tab)?.spans = spans;
                self.active = tab;
            }
            UndoKind::Table { tab, bi, table } => {
                if let Some(Block::Table(t)) =
                    self.tabs.get_mut(tab).and_then(|t| t.blocks.get_mut(bi))
                {
                    *t = table;
                }
                self.active = tab.min(self.tabs.len() - 1);
            }
            UndoKind::TableInserted {
                tab,
                id,
                text,
                spans,
            } => {
                let t = self.tabs.get_mut(tab)?;
                t.blocks
                    .retain(|b| !matches!(b, Block::Table(x) if x.id == id));
                t.body = text;
                t.spans = spans;
                self.active = tab;
            }
            UndoKind::TableDeleted {
                tab,
                bi,
                table,
                text,
                spans,
            } => {
                let t = self.tabs.get_mut(tab)?;
                let at = bi.min(t.blocks.len());
                t.blocks.insert(at, Block::Table(table));
                t.body = text;
                t.spans = spans;
                self.active = tab;
            }
        }
        if redo {
            self.undo.entries.push(inverse);
        } else {
            self.undo.redo.push(inverse);
        }
        Some(label)
    }
    /// Selects the whole text of the tab shown (from its context menu).
    pub fn select_all(&mut self) {
        // The tab is named: a selection asked for alone opens the first tab.
        self.requested_tab = Some(self.active);
        self.requested_selection = Some(0..self.tabs[self.active].body.chars().count());
    }
    pub fn add_tab(&mut self) {
        let n = self.tabs.len() + 1;
        self.push_tab(format!("탭 {n}"));
        self.active = self.tabs.len() - 1;
        self.selection = 0..0;
        self.selected_cell = None;
        self.tsel = None;
    }
    pub fn delete_tab(&mut self, index: usize) {
        if index >= self.tabs.len() {
            return;
        }
        self.trash.push(self.tabs.remove(index));
        // History points at tab numbers that just changed.
        self.undo = UndoStack::default();
        self.tsel = None;
        self.first_tab_changed |= index == 0;
        if self.tabs.is_empty() {
            self.push_tab("본문".into());
        }
        self.active = self.active.min(self.tabs.len() - 1);
        if self.tabs[self.active].hidden {
            self.active = self.tabs.iter().position(|t| !t.hidden).unwrap_or(0);
        }
        self.selection = 0..0;
        self.selected_cell = None;
    }

    pub(crate) fn remove_collection_blocks(&mut self, tab: usize, indices: &[usize]) {
        let cursor_map = self.tabs[tab].remove_blocks(indices);
        self.first_tab_changed |= tab == 0;
        // History in this tab may refer to removed blocks or old character
        // offsets. Other tabs still have valid histories.
        self.undo.entries.retain(|entry| entry.kind.tab() != tab);
        self.undo.redo.retain(|entry| entry.kind.tab() != tab);
        if self.active == tab {
            self.selected_cell = None;
            self.tsel = None;
            self.selected_picture = None;
            self.picture_tool_at = None;
            self.undo_cursor = None;
            if let Some(map) = cursor_map {
                self.selection = map_removed_chars(&map, &self.selection);
                self.requested_tab = Some(tab);
                self.requested_selection = Some(self.selection.clone());
            }
        }
    }
    /// Change one thing about each selected character (`edit`), keeping the
    /// rest of its formatting.
    fn restyle(&mut self, edit: impl Fn(&mut Format)) {
        let tab = &mut self.tabs[self.active];
        let chars = tab.body.chars().count();
        let range = self.selection.start.min(chars)..self.selection.end.min(chars);
        let winner = winning_spans(&tab.spans, chars);
        let mut runs: Vec<Span> = vec![];
        for i in range {
            let mut f = match winner.get(i) {
                Some(&w) if w != u32::MAX => tab.spans[w as usize].format.clone(),
                _ => Format::default(),
            };
            let old = f.clone();
            edit(&mut f);
            if f == old {
                continue;
            }
            match runs.last_mut() {
                Some(r) if r.range.end == i && r.format == f => r.range.end = i + 1,
                _ => runs.push(Span {
                    range: i..i + 1,
                    format: f,
                }),
            }
        }
        tab.spans.extend(runs);
    }
    /// Color (`highlight`: false) or highlight the selected text. With
    /// nothing selected, the color is for what is typed next.
    pub fn paint_selection(&mut self, highlight: bool, k: u8) {
        let set = |f: &mut Format| {
            if highlight {
                f.highlight = k;
            } else {
                f.color = k;
            }
        };
        if self.selected_cell.is_some() || self.selection.is_empty() {
            set(&mut self.format);
        } else {
            self.restyle(set);
        }
    }
    fn paint_selection_with_history(&mut self, highlight: bool, index: u8, now: f64) {
        if self.selection.is_empty() || self.selected_cell.is_some() {
            self.paint_selection(highlight, index);
            return;
        }
        let tab = self.active;
        let spans = self.tabs[tab].spans.clone();
        self.paint_selection(highlight, index);
        if spans != self.tabs[tab].spans {
            self.undo.push(
                if highlight { "형광펜" } else { "글자색" },
                UndoKind::Style { tab, spans },
                now,
            );
        }
    }

    pub fn toggle_format(&mut self, kind: usize) {
        // On a selection: on unless every selected character has it.
        let on = if self.selected_cell.is_none() && !self.selection.is_empty() {
            let tab = &self.tabs[self.active];
            let chars = tab.body.chars().count();
            let winner = winning_spans(&tab.spans, chars);
            let range = self.selection.start.min(chars)..self.selection.end.min(chars);
            !range.clone().all(|i| match winner[i] {
                u32::MAX => false,
                w => *flag(&mut tab.spans[w as usize].format.clone(), kind),
            }) || range.is_empty()
        } else {
            !*flag(&mut self.format, kind)
        };
        self.set_format(kind, on);
    }
    /// Turn one style (`kind`: 0 bold, 1 italic, 2 underline, 3 strike) on
    /// or off for the selection (or the selected table cell), and for what
    /// is typed next.
    fn set_format(&mut self, kind: usize, on: bool) {
        *flag(&mut self.format, kind) = on;
        if let Some((b, r, c)) = self.selected_cell {
            if let Some(Block::Table(t)) = self.tabs[self.active].blocks.get_mut(b) {
                if let Some(cell) = t.cells.get_mut(r).and_then(|r| r.get_mut(c)) {
                    cell.format = self.format.clone();
                }
            }
        } else if !self.selection.is_empty() {
            // Only this one style changes; colors and the other styles of
            // each selected character stay.
            self.restyle(|f| *flag(f, kind) = on);
        }
    }
    /// What the font dialog starts from: the memo's font and size, and the
    /// styles of the selection (a style only when every selected character
    /// has it, a color only when they share it), or of what is typed next.
    pub fn font_choice(&self) -> FontChoice {
        let tab = self.active.min(self.tabs.len().saturating_sub(1));
        let cell = self.selected_cell.filter(|_| tab == self.active);
        let mut start = self.format.clone();
        if let (None, Some(t)) = (cell, self.tabs.get(tab)) {
            let chars = t.body.chars().count();
            let range = self.selection.start.min(chars)..self.selection.end.min(chars);
            if !range.is_empty() {
                let winner = winning_spans(&t.spans, chars);
                let at = |i: usize| match winner[i] {
                    u32::MAX => Format::default(),
                    w => t.spans[w as usize].format.clone(),
                };
                let first = at(range.start);
                start = first.clone();
                for i in range {
                    let mut f = at(i);
                    for kind in 0..4 {
                        if !*flag(&mut f, kind) {
                            *flag(&mut start, kind) = false;
                        }
                    }
                    if f.color != first.color {
                        start.color = MIXED;
                    }
                    if f.highlight != first.highlight {
                        start.highlight = MIXED;
                    }
                }
            }
        }
        FontChoice {
            font_name: self.font_name.clone(),
            font_size: self.font_size,
            format: start.clone(),
            start,
            tab,
            selection: self.selection.clone(),
            cell,
        }
    }
    /// The font dialog's "확인": the font and size for the whole memo, and
    /// each style or color the dialog changed for the text selected when it
    /// opened (what was not changed keeps each character's own).
    pub fn apply_font_choice(&mut self, c: &FontChoice) {
        if !c.font_name.is_empty() {
            self.font_name.clone_from(&c.font_name);
        }
        if c.font_size.is_finite() && c.font_size > 0. {
            self.font_size = c.font_size.clamp(6. / 0.75, 48. / 0.75);
        }
        if c.format == c.start {
            return;
        }
        // The text may have been clicked elsewhere while the dialog was
        // open: the styles go to what was selected when it opened.
        let (selection, cell) = (self.selection.clone(), self.selected_cell);
        if c.tab == self.active {
            self.selection = c.selection.clone();
            self.selected_cell = c.cell;
        } else {
            let end = self.selection.end;
            self.selection = end..end;
            self.selected_cell = None;
        }
        let (mut now, mut was) = (c.format.clone(), c.start.clone());
        for kind in 0..4 {
            let on = *flag(&mut now, kind);
            if on != *flag(&mut was, kind) {
                self.set_format(kind, on);
            }
        }
        for (highlight, new, old) in [
            (false, c.format.color, c.start.color),
            (true, c.format.highlight, c.start.highlight),
        ] {
            if new != old && new != MIXED {
                self.paint_selection(highlight, new);
            }
        }
        if let Some((b, r, col)) = self.selected_cell {
            // A cell keeps its colors in its own format.
            if let Some(Block::Table(t)) = self.tabs[self.active].blocks.get_mut(b) {
                if let Some(cell) = t.cells.get_mut(r).and_then(|r| r.get_mut(col)) {
                    cell.format = self.format.clone();
                }
            }
        }
        if c.tab == self.active {
            self.selection = selection;
            self.selected_cell = cell;
        }
    }
}
/// A color or highlighter that differs across the selection.
pub const MIXED: u8 = u8::MAX;
fn flag(f: &mut Format, kind: usize) -> &mut bool {
    match kind {
        0 => &mut f.bold,
        1 => &mut f.italic,
        2 => &mut f.underline,
        _ => &mut f.strike,
    }
}
/// What the font dialog edits (see [`Document::font_choice`]).
#[derive(Clone, Debug, PartialEq)]
pub struct FontChoice {
    pub font_name: String,
    /// In the memo's own unit (points are this × 0.75).
    pub font_size: f32,
    pub format: Format,
    /// The styles as they were: only what the dialog changed is applied, so
    /// a selection of mixed styles keeps them unless one is picked.
    start: Format,
    tab: usize,
    selection: Range<usize>,
    cell: Option<(usize, usize, usize)>,
}
impl FontChoice {
    /// The text the preview shows: the selection's first line (up to 24
    /// characters), or a sample.
    pub fn sample(&self, doc: &Document) -> String {
        let picked = doc
            .tabs
            .get(self.tab)
            .filter(|_| self.cell.is_none() && !self.selection.is_empty())
            .map(|t| {
                t.body
                    .chars()
                    .skip(self.selection.start)
                    .take(self.selection.len())
                    .filter(|&c| picture_id(c).is_none())
                    .take_while(|&c| c != '\n')
                    .take(24)
                    .collect::<String>()
            })
            .unwrap_or_default();
        if picked.trim().is_empty() {
            "가나다라 AaBbCc 123".into()
        } else {
            picked
        }
    }
}
/// The font dialog's controls: font, size, styles, colors and a preview of
/// `sample`. Returns whether anything was changed.
pub fn font_panel(ui: &mut egui::Ui, c: &mut FontChoice, sample: &str) -> bool {
    let before = c.clone();
    let list_height = 168.;
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.strong("글꼴");
            ui.set_width(178.);
            egui::Frame::NONE
                .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                .corner_radius(4.)
                .inner_margin(egui::Margin::same(3))
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("font_dialog_fonts")
                        .max_height(list_height)
                        .min_scrolled_height(list_height)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for name in font_choices() {
                                let face = font_id(ui, 14., name, false);
                                let selected = c.font_name == name;
                                let r =
                                    ui.selectable_label(selected, RichText::new(name).font(face));
                                if r.clicked() {
                                    c.font_name = name.into();
                                }
                            }
                        });
                });
        });
        ui.vertical(|ui| {
            ui.strong("크기");
            ui.set_width(84.);
            let mut pt = (c.font_size * 0.75).round();
            if ui
                .add(
                    egui::DragValue::new(&mut pt)
                        .range(6.0..=48.0)
                        .speed(0.2)
                        .fixed_decimals(0)
                        .suffix(" pt"),
                )
                .changed()
            {
                c.font_size = pt.round() / 0.75;
            }
            egui::ScrollArea::vertical()
                .id_salt("font_dialog_sizes")
                .max_height(list_height - 26.)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for v in [
                        8., 9., 10., 11., 12., 14., 16., 18., 20., 24., 28., 32., 36., 48.,
                    ] {
                        if ui
                            .selectable_label((pt - v).abs() < 0.1, format!("{v:.0} pt"))
                            .clicked()
                        {
                            c.font_size = v / 0.75;
                        }
                    }
                });
        });
        ui.vertical(|ui| {
            ui.strong("서식");
            for (kind, label) in [
                (0, "굵게  Ctrl+B"),
                (1, "기울임  Ctrl+I"),
                (2, "밑줄  Ctrl+U"),
                (3, "취소선"),
            ] {
                ui.checkbox(flag(&mut c.format, kind), label);
            }
        });
    });
    ui.add_space(6.);
    for (highlight, title, colors) in [
        (false, "글자 색", &TEXT_COLORS[..]),
        (true, "형광펜", &HIGHLIGHTS[..]),
    ] {
        ui.horizontal(|ui| {
            ui.add_sized([52., 22.], egui::Label::new(title));
            for (k, &(name, col)) in colors.iter().enumerate() {
                let r = color_chip(ui, name, col, highlight);
                let value = if highlight {
                    &mut c.format.highlight
                } else {
                    &mut c.format.color
                };
                if *value == k as u8 {
                    ui.painter().rect_stroke(
                        r.rect,
                        5.,
                        Stroke::new(2.0_f32, ui.visuals().selection.stroke.color),
                        egui::StrokeKind::Inside,
                    );
                }
                if r.clicked() {
                    *value = k as u8;
                }
            }
            let value = if highlight {
                c.format.highlight
            } else {
                c.format.color
            };
            if value == MIXED {
                crate::ui::caption(ui, "여러 색");
            }
        });
    }
    ui.add_space(6.);
    ui.strong("미리보기");
    let size = c.font_size.clamp(6. / 0.75, 48. / 0.75);
    let font = font_id(ui, size, &c.font_name, false);
    let bold_font = font_id(ui, size, &c.font_name, true);
    let mut shown = c.format.clone();
    if shown.color == MIXED {
        shown.color = 0;
    }
    if shown.highlight == MIXED {
        shown.highlight = 0;
    }
    let ink = ui.visuals().text_color();
    let mut job = layout(sample, &[], &shown, font, bold_font, ink);
    job.wrap.max_width = f32::INFINITY;
    job.wrap.max_rows = 1;
    egui::Frame::NONE
        .fill(ui.visuals().extreme_bg_color)
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .corner_radius(4.)
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.set_height(72.);
            ui.centered_and_justified(|ui| {
                ui.add(egui::Label::new(job).truncate());
            });
        });
    crate::ui::caption(
        ui,
        "글꼴과 크기는 메모 전체에, 서식과 색은 선택한 글자(없으면 이어서 쓸 글자)에 적용돼요.",
    );
    *c != before
}

/// Render an inline editor with its footer anchored to the bottom of its available panel.
/// The text widget id is shared with `content`; rendering order does not replace drafts.
pub fn show(
    ui: &mut egui::Ui,
    legacy_body: &mut String,
    doc: &mut Document,
    id: impl Hash,
) -> egui::Response {
    show_with(ui, legacy_body, doc, id, 0.)
}
/// Like [`show`] for panes inside the main window: no window-opacity tool,
/// and `trailing_space` kept free at the right of the tool row for the
/// caller's own buttons.
pub fn show_with(
    ui: &mut egui::Ui,
    legacy_body: &mut String,
    doc: &mut Document,
    id: impl Hash,
    trailing_space: f32,
) -> egui::Response {
    let id = ui.make_persistent_id(id);
    let top = ui.cursor().min;
    let bottom = ui.clip_rect().bottom().min(ui.ctx().screen_rect().bottom());
    let width = ui.available_width().min(ui.clip_rect().width()).max(32.);
    let height = ui
        .available_height()
        .min((bottom - top.y).max(120.))
        .max(120.);
    let (bounds, _) = ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::hover());
    let mut footer_ui = ui.new_child(
        egui::UiBuilder::new()
            .id_salt((id, "footer_region"))
            .max_rect(bounds)
            .layout(egui::Layout::bottom_up(egui::Align::Min)),
    );
    render_footer(&mut footer_ui, doc, id, trailing_space, true, true, false);
    let content_bottom =
        (footer_ui.min_rect().top() - ui.spacing().item_spacing.y).max(bounds.top());
    let content_rect =
        egui::Rect::from_min_max(bounds.min, egui::pos2(bounds.right(), content_bottom));
    let mut content_ui = ui.new_child(
        egui::UiBuilder::new()
            .id_salt((id, "content_region"))
            .max_rect(content_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    content_impl(&mut content_ui, legacy_body, doc, id)
}
/// Render only editable content inside the parent's finite available area.
/// Use `footer` in the parent's bottom panel to keep all controls outside scrolling content.
pub fn content(
    ui: &mut egui::Ui,
    legacy_body: &mut String,
    doc: &mut Document,
    id: impl Hash,
) -> egui::Response {
    let id = ui.make_persistent_id(id);
    content_impl(ui, legacy_body, doc, id)
}
/// Render tabs, a divider, then the existing icon tools. `trailing_space` reserves
/// the right end of the icon row for the parent's related-note control.
pub fn footer(
    ui: &mut egui::Ui,
    doc: &mut Document,
    id: impl Hash,
    trailing_space: f32,
    divider: bool,
) -> egui::Response {
    let id = ui.make_persistent_id(id);
    // Editing toolbar on top, the tabs at the very bottom.
    render_footer(ui, doc, id, trailing_space, true, divider, true)
}
/// Maps a char-index cursor position from `old` text to `new` text after an
/// external edit replaced some span of `old` with different content. A
/// position inside the untouched common prefix or suffix is preserved; a
/// position inside the replaced middle region is clamped to just after the
/// common prefix (i.e. where the replacement starts).
fn remap_cursor(old: &str, new: &str, idx: usize) -> usize {
    let old: Vec<char> = old.chars().collect();
    let new: Vec<char> = new.chars().collect();
    let max_common = old.len().min(new.len());
    let prefix = old
        .iter()
        .zip(new.iter())
        .take_while(|(a, b)| a == b)
        .count();
    let mut suffix = 0;
    while suffix < max_common - prefix && old[old.len() - 1 - suffix] == new[new.len() - 1 - suffix]
    {
        suffix += 1;
    }
    if idx <= prefix {
        idx.min(new.len())
    } else if idx >= old.len() - suffix {
        new.len() - (old.len() - idx)
    } else {
        prefix
    }
}
fn content_impl(
    ui: &mut egui::Ui,
    legacy_body: &mut String,
    doc: &mut Document,
    id: Id,
) -> egui::Response {
    if doc.tabs.is_empty() {
        doc.push_tab("본문".into());
    }
    doc.active = doc.active.min(doc.tabs.len() - 1);
    if doc.first_tab_changed {
        *legacy_body = doc.tabs[0].body.clone();
        doc.first_tab_changed = false;
    }
    if doc.tabs[0].body != *legacy_body {
        let old_body = doc.tabs[0].body.clone();
        let tab = &mut doc.tabs[0];
        transform_spans(&tab.body, legacy_body, &mut tab.spans, &Format::default());
        tab.body = legacy_body.clone();
        // `legacy_body` was changed from outside this widget (e.g. the
        // #project-tag auto-conversion stripping its prefix from the note
        // body). The TextEdit for tab 0 may still have a cursor/IME state
        // persisted against the *old* text; left alone, the next keystroke
        // lands at a stale char index and corrupts composition (notably for
        // Hangul). Remap the persisted cursor onto the new text so typing
        // continues from the right place.
        if doc.active == 0 {
            let text_id = id.with(("text", 0usize));
            if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), text_id) {
                if let Some(range) = state.cursor.char_range() {
                    let new_primary = remap_cursor(&old_body, legacy_body, range.primary.index);
                    let new_secondary = remap_cursor(&old_body, legacy_body, range.secondary.index);
                    state.cursor.set_char_range(Some(CCursorRange {
                        primary: CCursor::new(new_primary),
                        secondary: CCursor::new(new_secondary),
                    }));
                    state.store(ui.ctx(), text_id);
                }
            }
        }
    }
    let requested = doc.requested_selection.take();
    let requested_tab = doc.requested_tab.take();
    let requested_focus = requested.is_some();
    if requested.is_some() || requested_tab.is_some() {
        doc.active = requested_tab.filter(|&i| i < doc.tabs.len()).unwrap_or(0);
    }
    let queued = ui.data_mut(|d| d.remove_temp::<Vec<egui::Event>>(id.with("edit_menu_events")));
    if let Some(events) = queued {
        ui.memory_mut(|m| m.request_focus(id.with(("text", doc.active))));
        ui.input_mut(|i| i.events.extend(events));
    }
    // Body text and tables share undo/redo history. Active cell text editors
    // handle their own typing history until the edit is committed.
    let probe = id.with(("text", doc.active));
    if ui.input_mut(|i| i.consume_key(Modifiers::CTRL, Key::Num0)) {
        doc.zoom = 1.;
    }
    let cell_editing = doc.tsel.as_ref().is_some_and(|s| s.editing);
    let table_focus = doc
        .tsel
        .as_ref()
        .is_some_and(|s| ui.memory(|m| m.has_focus(id.with(("table_grid", s.tab, s.bi)))));
    if !doc.ime
        && !cell_editing
        && (table_focus || ui.memory(|m| m.has_focus(probe)))
        && ui.input_mut(|i| {
            i.consume_key(Modifiers::CTRL | Modifiers::SHIFT, Key::Z)
                || i.consume_key(Modifiers::CTRL, Key::Y)
        })
    {
        doc.status = doc
            .redo()
            .unwrap_or_else(|| "다시 실행할 내용이 없습니다".into());
    }
    if !doc.ime
        && !cell_editing
        && (table_focus || ui.memory(|m| m.has_focus(probe)))
        && ui.input_mut(|i| i.consume_key(Modifiers::CTRL, Key::Z))
    {
        doc.status = doc
            .undo()
            .unwrap_or_else(|| "되돌릴 내용이 없습니다".to_owned());
    }
    let text_id = id.with(("text", doc.active));
    if std::mem::take(&mut doc.undo_focus) {
        ui.memory_mut(|m| m.request_focus(text_id));
    }
    let before = doc.tabs[doc.active].body.clone();
    let spans_before = doc.tabs[doc.active].spans.clone();
    let now = ui.input(|i| i.time);
    let mut table_undo_done = false;
    let hover = ui.rect_contains_pointer(ui.available_rect_before_wrap());
    let zoom_delta = ui.input(|i| {
        if hover && i.modifiers.ctrl {
            i.raw_scroll_delta.y
        } else {
            0.
        }
    });
    if zoom_delta != 0. {
        doc.zoom = (doc.zoom + zoom_delta.signum() * 0.05).clamp(0.3, 2.);
        doc.zoom_until = ui.input(|i| i.time) + 0.9;
        ui.input_mut(|i| {
            i.raw_scroll_delta = Vec2::ZERO;
            i.smooth_scroll_delta = Vec2::ZERO;
        });
    }
    ui.input(|i| {
        for e in &i.events {
            if let egui::Event::Ime(ev) = e {
                match ev {
                    // An empty preedit (Backspace or Esc during composition)
                    // ends it as much as a commit does; the last event wins.
                    egui::ImeEvent::Preedit(s) => doc.ime = !s.is_empty(),
                    egui::ImeEvent::Commit(_) | egui::ImeEvent::Disabled => doc.ime = false,
                    _ => {}
                }
            }
        }
    });
    let focused = ui.memory(|m| m.has_focus(text_id));
    let had_preedit = ui.input(|i| {
        i.events
            .iter()
            .any(|e| matches!(e, egui::Event::Ime(egui::ImeEvent::Preedit(_))))
    });
    if focused {
        hold_paste_during_ime(ui, id.with("held_paste"));
        if ui.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Paste(_)))) {
            // A paste means no composition is open (the IME commits before
            // Ctrl+V), so a composing flag left by a lost commit ends here.
            // Left on, it kept every later paste in this memo from turning
            // a copied table into a table.
            doc.ime = false;
        }
    }
    let editing = focused || hover;
    doc.editing = editing;
    if focused && !doc.ime && !had_preedit {
        for (key, kind) in [(Key::B, 0), (Key::I, 1), (Key::U, 2)] {
            if ui.input_mut(|i| i.consume_key(Modifiers::CTRL, key)) {
                doc.toggle_format(kind);
            }
        }
    }
    let mut pending_cursor = doc.undo_cursor.take();
    let menu_picture = ui.data_mut(|d| {
        let picture = d.get_temp::<Picture>(id.with("menu_picture"));
        d.remove::<Picture>(id.with("menu_picture"));
        picture
    });
    if let Some(picture) = menu_picture {
        pending_cursor = Some(doc.tabs[doc.active].insert_picture(doc.selection.clone(), picture));
        ui.memory_mut(|m| m.request_focus(text_id));
    }
    if let Some(r) = requested {
        let len = doc.tabs[doc.active].body.chars().count();
        let r = r.start.min(len)..r.end.min(len);
        let mut state = egui::TextEdit::load_state(ui.ctx(), text_id).unwrap_or_default();
        state.cursor.set_char_range(Some(CCursorRange::two(
            CCursor::new(r.start),
            CCursor::new(r.end),
        )));
        state.store(ui.ctx(), text_id);
        ui.memory_mut(|m| m.request_focus(text_id));
    }
    if focused && !doc.ime && !had_preedit {
        let old_range = egui::TextEdit::load_state(ui.ctx(), text_id)
            .and_then(|s| s.cursor.char_range())
            .map(|r| {
                let [a, b] = r.sorted();
                a.index..b.index
            })
            .unwrap_or_else(|| doc.selection.clone());
        if old_range.is_empty() && ui.input(|i| i.key_pressed(Key::Backspace)) {
            let strike = doc.tabs[doc.active]
                .spans
                .iter_mut()
                .rev()
                .find(|s| s.range.contains(&old_range.start.saturating_sub(1)));
            if let Some(s) = strike.filter(|s| s.format.strike) {
                s.format.strike = false;
                ui.input_mut(|i| {
                    i.consume_key(Modifiers::NONE, Key::Backspace);
                });
            }
        }
        let outdent = ui.input_mut(|i| i.consume_key(Modifiers::SHIFT, Key::Tab));
        let indent = ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Tab));
        let stamp =
            ui.input_mut(|i| i.consume_key(Modifiers::CTRL | Modifiers::SHIFT, Key::Semicolon));
        let date = !stamp && ui.input_mut(|i| i.consume_key(Modifiers::CTRL, Key::Semicolon));
        if outdent || indent || date || stamp {
            let text = &mut doc.tabs[doc.active].body;
            let insert = if indent {
                "  ".into()
            } else if stamp {
                chrono::Local::now().format("%y-%m-%d %H:%M:%S").to_string()
            } else if date {
                chrono::Local::now().format("%y-%m-%d").to_string()
            } else {
                String::new()
            };
            if outdent {
                pending_cursor = Some(outdent_line(text, old_range.start));
            } else {
                replace_chars(text, old_range.clone(), &insert);
                pending_cursor = Some(old_range.start + insert.chars().count());
            }
        }
        let paste = ui.input(|i| {
            i.events.iter().find_map(|e| {
                if let egui::Event::Paste(t) = e {
                    Some(t.clone())
                } else {
                    None
                }
            })
        });
        // Placeholders only come from real images; never accept stray ones.
        ui.input_mut(|i| {
            for e in &mut i.events {
                if let egui::Event::Paste(t) | egui::Event::Text(t) = e {
                    if t.chars().any(|c| picture_id(c).is_some()) {
                        *t = strip_pictures(t);
                    }
                }
            }
        });
        let mut table_pasted = false;
        let text_pasted = paste.is_some();
        // '표 붙여넣기: 텍스트' (설정 › 일반) leaves a copied table as tab
        // separated text, the way the TextEdit pastes it.
        let paste = paste
            .filter(|t| t.len() < 1024 * 1024)
            .filter(|_| !paste_tables_as_text());
        let html = paste.as_ref().and_then(|_| clipboard_html_source());
        // Text around a table (a web page, a document) keeps its order:
        // text, then the table, then text.
        if let Some(pieces) = html
            .as_deref()
            .and_then(crate::table_edit::pieces_from_html)
        {
            pending_cursor = Some(insert_pieces(
                doc,
                old_range.clone(),
                pieces,
                &before,
                &spans_before,
                now,
            ));
            table_undo_done = true;
            ui.input_mut(|i| i.events.retain(|e| !matches!(e, egui::Event::Paste(_))));
            table_pasted = true;
        } else if let Some(paste) = paste {
            // The clipboard's real `<table>` markup is read first: its rows,
            // columns and merged cells are exact. One cell is just text.
            // Without markup, tab separated text is split with Excel's
            // quoting (a quoted cell may hold tabs and line breaks).
            let html_table = html
                .as_deref()
                .and_then(Table::from_html)
                .filter(|t| t.cells.len() > 1 || t.cells[0].len() > 1);
            let table =
                html_table.or_else(|| paste.contains('\t').then(|| Table::from_tsv(&paste)));
            if let Some(table) = table {
                // Lands right where the pasted text itself would have, not
                // appended after everything else, and moves/deletes with the
                // surrounding text from then on, the same as a pasted image.
                pending_cursor = Some(doc.tabs[doc.active].insert_table(old_range.clone(), table));
                if let Some(Block::Table(t)) = doc.tabs[doc.active].blocks.last() {
                    let entry = UndoKind::TableInserted {
                        tab: doc.active,
                        id: t.id,
                        text: before.clone(),
                        spans: spans_before.clone(),
                    };
                    doc.undo.push("표 붙여넣기", entry, now);
                    table_undo_done = true;
                }
                ui.input_mut(|i| i.events.retain(|e| !matches!(e, egui::Event::Paste(_))));
                table_pasted = true;
            }
        }
        if !table_pasted && !text_pasted && paste_chord(ui, text_id) {
            // egui's own Paste(text) event can arrive a frame or more after
            // this key-down edge, so `text_pasted` above does not yet know
            // whether this same Ctrl+V is about to deliver table/text too.
            // A table clipboard (e.g. from Excel) also carries an image
            // representation; without this check that image would land here
            // first and the text would still turn into a second, separate
            // paste once its event finally showed up. Read the clipboard's
            // own text synchronously instead of waiting on that event.
            if let Some(picture) = clipboard_picture_for_paste(doc.image_scale, doc.image_fit) {
                pending_cursor =
                    Some(doc.tabs[doc.active].insert_picture(old_range.clone(), picture));
                ui.input_mut(|i| {
                    i.events.retain(|e| {
                        !matches!(
                            e,
                            egui::Event::Paste(_) | egui::Event::Key { key: Key::V, .. }
                        )
                    })
                });
            }
        }
        if let Some(id) = doc.selected_picture {
            if ui.input_mut(|i| {
                i.consume_key(Modifiers::NONE, Key::Delete)
                    || i.consume_key(Modifiers::NONE, Key::Backspace)
            }) {
                let c = placeholder(id);
                let tab = &mut doc.tabs[doc.active];
                if let Some(at) = tab.body.chars().position(|x| x == c) {
                    replace_chars(&mut tab.body, at..at + 1, "");
                    pending_cursor = Some(at);
                }
                doc.selected_picture = None;
            }
        }
    }
    let enter_event = ui.input(|i| {
        i.events.iter().any(|e| {
            matches!(e, egui::Event::Key { key: Key::Enter, pressed: true, modifiers, .. }
                if !(modifiers.shift || modifiers.ctrl || modifiers.alt || modifiers.command))
        })
    });
    let font = font_id(ui, doc.font_size * doc.zoom, &doc.font_name, false);
    let bold_font = font_id(ui, doc.font_size * doc.zoom, &doc.font_name, true);
    let spans = doc.tabs[doc.active].spans.clone();
    let base = Format::default();
    let color = ui.visuals().text_color();
    let pictures = picture_sizes(&doc.tabs[doc.active], doc.zoom);
    let tables = {
        let (tab_i, key_id) = (doc.active, id);
        table_sizes_with(&doc.tabs[doc.active], doc.zoom, &|tid| {
            ui.data(|d| d.get_temp::<f32>(key_id.with(("table_h", tab_i, tid))))
        })
    };
    // The invisible space that reserves a picture's or a table's place is
    // set in the glyph-less spacer font when the app registered it (see
    // `spacer_font`); its width is then the font size itself.
    let spacer = spacer_family(ui);
    let space_ratio = ui
        .fonts(|f| {
            let family = spacer.clone().unwrap_or_else(|| font.family.clone());
            f.glyph_width(&FontId::new(100., family), '\u{3000}')
        })
        .max(1.)
        / 100.;
    let cache_id = id.with("body_layout_cache");
    let mut cached = ui.data(|d| d.get_temp::<std::sync::Arc<BodyLayoutCache>>(cache_id));
    let mut layouter = |ui: &egui::Ui, text: &str, width: f32| {
        if let Some(cache) = &cached {
            let k = &cache.key;
            if k.text == text
                && k.spans == spans
                && k.font == font
                && k.bold_font == bold_font
                && k.color == color
                && k.pictures == pictures
                && k.tables == tables
                && k.spacer == spacer
                && k.width == width
                && k.pixels_per_point == ui.ctx().pixels_per_point()
            {
                return cache.galley.clone();
            }
        }
        let mut job = layout(text, &spans, &base, font.clone(), bold_font.clone(), color);
        inline_pictures(&mut job, &pictures, width, space_ratio);
        inline_tables(&mut job, &tables, width, space_ratio);
        if let Some(family) = &spacer {
            set_spacer_family(&mut job, family);
        }
        job.wrap.max_width = width;
        let galley = ui.fonts(|f| f.layout_job(job));
        let cache = std::sync::Arc::new(BodyLayoutCache {
            key: BodyLayoutKey {
                text: text.to_owned(),
                spans: spans.clone(),
                font: font.clone(),
                bold_font: bold_font.clone(),
                color,
                pictures: pictures.clone(),
                tables: tables.clone(),
                spacer: spacer.clone(),
                width,
                pixels_per_point: ui.ctx().pixels_per_point(),
            },
            galley: galley.clone(),
        });
        ui.data_mut(|d| d.insert_temp(cache_id, cache.clone()));
        cached = Some(cache);
        galley
    };
    let bottom = ui.ctx().screen_rect().bottom().min(ui.clip_rect().bottom());
    let body_height = ui
        .available_height()
        .min((bottom - ui.cursor().top()).max(1.))
        .max(1.);
    let rows = (body_height / (doc.font_size * doc.zoom * 1.4)).clamp(1., 18.) as usize;
    let previous_selection = doc.selection.clone();
    let selection_popup = ui.data(|d| d.get_temp::<bool>(Id::new(HOST_MENU))) == Some(true);
    let body_width = ui.available_width().max(32.);
    let oversized_picture = pictures.iter().any(|p| !p.fit && p.width > body_width);
    let mut output = egui::ScrollArea::new([oversized_picture, true])
        .id_salt(id.with(("text_scroll", doc.active)))
        .max_height(body_height)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let text = &mut doc.tabs[doc.active].body;
            let output = egui::TextEdit::multiline(text)
                .id(text_id)
                .font(font.clone())
                .frame(false)
                .desired_width(body_width)
                .desired_rows(rows)
                .lock_focus(true)
                .hint_text("여기에 메모하세요…")
                .layouter(&mut layouter)
                .show(ui);
            if requested_focus {
                if let Some(range) = output.cursor_range {
                    ui.scroll_to_rect(
                        output
                            .galley
                            .pos_from_cursor(&range.primary)
                            .translate(output.galley_pos.to_vec2()),
                        Some(egui::Align::Center),
                    );
                }
            }
            if output.response.has_focus() {
                if let Some(range) = output.cursor_range {
                    #[cfg(test)]
                    LAST_CARET.with(|c| {
                        let r = output
                            .galley
                            .pos_from_cursor(&range.primary)
                            .translate(output.galley_pos.to_vec2());
                        c.set(Some((ui.clip_rect(), r)))
                    });
                    let caret = range.primary.ccursor.index;
                    if caret != doc.last_caret {
                        doc.last_caret = caret;
                        let r = output
                            .galley
                            .pos_from_cursor(&range.primary)
                            .translate(output.galley_pos.to_vec2());
                        // out of the visible body: bring it in
                        if !ui.clip_rect().contains_rect(r) {
                            ui.scroll_to_rect(r.expand2(Vec2::new(0., 8.)), None);
                        }
                    }
                }
            }
            paint_pictures(ui, doc, id, &output, &pictures);
            paint_tables(ui, doc, id, &output, &tables, &font, &bold_font);
            show_links(ui, doc, id);
            show_blocks(ui, doc, id);
            output
        })
        .inner;
    // TextEdit moves its caret on a secondary press. Keep the selected text
    // available to the context menu's Cut and Copy commands.
    if !previous_selection.is_empty()
        && ui.input(|i| {
            i.pointer.secondary_pressed()
                && i.pointer
                    .interact_pos()
                    .is_some_and(|p| output.response.rect.contains(p))
        })
    {
        output.state.cursor.set_char_range(Some(CCursorRange::two(
            CCursor::new(previous_selection.start),
            CCursor::new(previous_selection.end),
        )));
    }
    if let Some(cursor) = pending_cursor {
        output
            .state
            .cursor
            .set_char_range(Some(CCursorRange::one(CCursor::new(cursor))));
    }
    doc.selection = output
        .state
        .cursor
        .char_range()
        .map(|r| {
            let [a, b] = r.sorted();
            a.index..b.index
        })
        .unwrap_or(0..0);
    if output.response.has_focus() {
        doc.selected_cell = None;
        doc.tsel = None;
    }
    let mut new_strike = None;
    // What was typed is read from the text itself (one new character right
    // before the caret), not from event names: a Hangul syllable that is
    // committed in the same frame as Enter or a space, or an IME that sends
    // empty preedits, must not make the list helpers skip their turn.
    if output.response.has_focus() && !doc.ime {
        let cursor = doc.selection.end;
        let text = &mut doc.tabs[doc.active].body;
        let grown = text.chars().count() as isize - before.chars().count() as isize;
        let prev = cursor.checked_sub(1).and_then(|i| text.chars().nth(i));
        if output.response.changed() && doc.selection.is_empty() && grown > 0 {
            let mut caret = None;
            if enter_event && prev == Some('\n') {
                if let Some((range, with, at)) = enter_edit(text, cursor - 1) {
                    replace_chars(text, range, &with);
                    caret = Some(at);
                }
                doc.format.strike = false;
            } else if grown == 1 && prev == Some(' ') {
                if let Some((range, replace, style_range)) = shortcut_substitution(text, cursor) {
                    replace_chars(text, range.clone(), &replace);
                    caret = Some(range.start + replace.chars().count());
                    new_strike = style_range;
                    doc.format.strike = false;
                } else {
                    caret = normalize_outline_line(text, cursor);
                }
            }
            if let Some(at) = caret {
                output
                    .state
                    .cursor
                    .set_char_range(Some(CCursorRange::one(CCursor::new(at))));
            }
        }
    }
    if before != doc.tabs[doc.active].body {
        if !table_undo_done {
            doc.undo
                .push_body(doc.active, before.clone(), spans_before, now);
        }
        let tab = &mut doc.tabs[doc.active];
        transform_spans(&before, &tab.body, &mut tab.spans, &doc.format);
    }
    if let Some(range) = new_strike {
        let mut format = doc.format.clone();
        format.strike = true;
        doc.tabs[doc.active].spans.push(Span { range, format });
    }
    if selection_popup {
        sticker_selection_popup(ui, doc, id, &mut output, &previous_selection);
    }
    output.state.store(ui.ctx(), text_id);
    let marks: Vec<_> = doc.tabs[doc.active]
        .body
        .chars()
        .enumerate()
        .filter(|(_, c)| matches!(c, '☐' | '☑'))
        .collect();
    for (idx, ch) in marks {
        let caret = output
            .galley
            .pos_from_ccursor(CCursor::new(idx))
            .translate(output.galley_pos.to_vec2());
        let rect = egui::Rect::from_min_size(caret.min, Vec2::splat((font.size * 1.1).max(13.)));
        if output.text_clip_rect.contains_rect(rect)
            && ui
                .interact(
                    rect,
                    id.with(("check", doc.active, idx)),
                    egui::Sense::click(),
                )
                .on_hover_text("체크 상태 바꾸기")
                .clicked()
        {
            replace_chars(
                &mut doc.tabs[doc.active].body,
                idx..idx + 1,
                if ch == '☐' { "☑" } else { "☐" },
            );
        }
    }
    // A right click on a web address in the text: the menu offers the link.
    let menu_link = id.with("menu_link");
    if output.response.secondary_clicked() {
        let link = output.response.interact_pointer_pos().and_then(|p| {
            let at = output
                .galley
                .cursor_from_pos(p - output.galley_pos)
                .ccursor
                .index;
            url_at(&doc.tabs[doc.active].body, at)
        });
        ui.data_mut(|d| d.insert_temp(menu_link, link));
    }
    let response = output.response;
    crate::float::context_menu(&response, 220., |ui| {
        if let Some(url) = ui
            .data(|d| d.get_temp::<Option<String>>(menu_link))
            .flatten()
        {
            if ui.button("링크 열기").clicked() {
                ui.ctx().open_url(egui::OpenUrl::new_tab(url.clone()));
                ui.close_menu();
            }
            if ui.button("링크 주소 복사").clicked() {
                ui.ctx().copy_text(url);
                doc.status = "링크 주소를 복사했습니다".into();
                ui.close_menu();
            }
            ui.separator();
        }
        for (label, keys, event) in [
            ("잘라내기", "Ctrl+X", egui::Event::Cut),
            ("복사", "Ctrl+C", egui::Event::Copy),
        ] {
            if ui
                .add_enabled(
                    !doc.selection.is_empty(),
                    egui::Button::new(label).shortcut_text(keys),
                )
                .clicked()
            {
                ui.data_mut(|d| d.insert_temp(id.with("edit_menu_events"), vec![event]));
                ui.ctx().request_repaint();
                ui.close_menu();
            }
        }
        if ui
            .add(egui::Button::new("붙여넣기").shortcut_text("Ctrl+V"))
            .clicked()
        {
            match crate::clipboard_copy::Clipboard::new().and_then(|mut c| c.get_text()) {
                Ok(text) => ui.data_mut(|d| {
                    d.insert_temp(id.with("edit_menu_events"), vec![egui::Event::Paste(text)])
                }),
                Err(_) => match picture_from_clipboard(doc.image_scale, doc.image_fit) {
                    Ok(picture) => ui.data_mut(|d| d.insert_temp(id.with("menu_picture"), picture)),
                    Err(e) => doc.status = format!("클립보드 읽기 실패: {e}"),
                },
            }
            ui.ctx().request_repaint();
            ui.close_menu();
        }
        if ui
            .add(egui::Button::new("모두 선택").shortcut_text("Ctrl+A"))
            .clicked()
        {
            doc.select_all();
            ui.ctx().request_repaint();
            ui.close_menu();
        }
        ui.separator();
        // In a memo window: the font dialog and the window's opacity.
        if ui
            .data(|d| d.get_temp::<bool>(egui::Id::new(HOST_STYLE)))
            .unwrap_or(false)
        {
            if ui.button("글꼴…").clicked() {
                ui.data_mut(|d| d.insert_temp(egui::Id::new(HOST_ACTION), HostAction::Font));
                ui.close_menu();
            }
            if opacity_row(ui, &mut doc.opacity) {
                crate::float::keep_open(ui);
            }
            ui.separator();
        }
        if ui
            .add_enabled(doc.zoom != 1., egui::Button::new("원본 배율 100%"))
            .clicked()
        {
            doc.zoom = 1.;
            ui.close_menu();
        }
        if ui.button("체크박스 삽입").clicked() {
            let pos = doc.selection.end;
            replace_chars(&mut doc.tabs[doc.active].body, pos..pos, "☐ ");
            ui.close_menu();
        }
        // In a sticker, the memo's own actions too (the host acts on them).
        if ui
            .data(|d| d.get_temp::<bool>(egui::Id::new(HOST_MENU)))
            .unwrap_or(false)
        {
            ui.separator();
            for (label, action) in [
                ("일정 설정…", HostAction::Schedule),
                ("알람 설정…", HostAction::Alarm),
                ("스티커 정렬 · 메뉴…", HostAction::StickerMenu),
            ] {
                if ui.button(label).clicked() {
                    ui.data_mut(|d| d.insert_temp(egui::Id::new(HOST_ACTION), action));
                    ui.close_menu();
                }
            }
            ui.separator();
            for (label, action) in [
                (RichText::new("스티커 휴지통으로"), HostAction::Trash),
                (
                    RichText::new("스티커 영구삭제…").color(crate::ACCENT),
                    HostAction::Purge,
                ),
            ] {
                if ui.button(label).clicked() {
                    ui.data_mut(|d| d.insert_temp(egui::Id::new(HOST_ACTION), action));
                    ui.close_menu();
                }
            }
        }
    });
    if hover {
        let paths = ui.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect::<Vec<_>>()
        });
        for path in paths {
            match picture_from_file(&path, doc.image_scale, doc.image_fit) {
                Ok(p) => {
                    let range = doc.selection.clone();
                    let caret = doc.tabs[doc.active].insert_picture(range, p);
                    doc.selection = caret..caret;
                }
                Err(e) => doc.status = e,
            }
        }
    }
    if ui.input(|i| i.time) < doc.zoom_until {
        let rect = response.rect;
        let text = format!("{}%", (doc.zoom * 100.).round() as i32);
        let p = rect.center();
        ui.painter().rect_filled(
            egui::Rect::from_center_size(p, Vec2::new(76., 38.)),
            8.,
            Color32::from_black_alpha(155),
        );
        ui.painter().text(
            p,
            egui::Align2::CENTER_CENTER,
            text,
            FontId::proportional(20.),
            Color32::WHITE,
        );
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(100));
    }
    // Every frame for every open memo: copy only when it differs.
    if *legacy_body != doc.tabs[0].body {
        legacy_body.clone_from(&doc.tabs[0].body);
    }
    response
}

#[derive(Clone)]
struct SelectionPopup {
    text_id: Id,
    range: CCursorRange,
    rect: Option<egui::Rect>,
    lost_focus_at: Option<f64>,
}

fn sticker_selection_popup(
    ui: &egui::Ui,
    doc: &mut Document,
    id: Id,
    output: &mut egui::text_edit::TextEditOutput,
    previous_selection: &Range<usize>,
) {
    let ctx = ui.ctx();
    let key = id.with("selection_popup");
    let text_id = output.response.id;
    let popup_id = egui::ViewportId(key);
    let old = ui.data_mut(|d| d.remove_temp::<Option<SelectionPopup>>(key).flatten());
    let old_native = old
        .as_ref()
        .filter(|p| p.text_id == text_id && p.rect.is_none());
    let (focused, escaped, pressed, clicking, pointer, extend) = ui.input(|i| {
        (
            i.focused,
            i.key_pressed(Key::Escape),
            i.pointer.any_pressed(),
            i.pointer.any_down() || i.pointer.any_released(),
            i.pointer.interact_pos(),
            i.events.iter().any(|e| {
                matches!(e, egui::Event::Key {
                key: Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown,
                pressed: true,
                modifiers,
                ..
            } if modifiers.shift)
            }),
        )
    });
    if (!focused && old_native.is_none())
        || escaped
        || output.response.changed()
        || doc.ime
        || doc.selected_cell.is_some()
    {
        return;
    }
    let using_popup = old.as_ref().is_some_and(|p| {
        clicking
            && p.text_id == text_id
            && p.rect
                .zip(pointer)
                .is_some_and(|(r, point)| r.contains(point))
    });
    if pressed && !using_popup {
        return;
    }
    let selected = output
        .state
        .cursor
        .char_range()
        .filter(|r| r.primary.index != r.secondary.index);
    let opened = output
        .response
        .drag_stopped_by(egui::PointerButton::Primary)
        || (extend && doc.selection != *previous_selection);
    let range = if (using_popup
        || old_native.is_some() && (!focused || !output.response.has_focus()))
        && !opened
    {
        old.as_ref().map(|p| p.range)
    } else if output.response.has_focus()
        && (opened || old.as_ref().is_some_and(|p| p.text_id == text_id))
    {
        selected
    } else {
        None
    };
    let Some(range) = range else { return };

    let [start, end] = range.sorted();
    let caret_rect = |cursor| {
        output
            .galley
            .pos_from_ccursor(cursor)
            .translate(output.galley_pos.to_vec2())
    };
    let caret = caret_rect(range.primary);
    let clip = output.text_clip_rect;
    let (first, last) = (caret_rect(start), caret_rect(end));
    let selected = if first.top() == last.top() {
        first.union(last)
    } else {
        egui::Rect::from_min_max(
            egui::pos2(clip.left(), first.top()),
            egui::pos2(clip.right(), last.bottom()),
        )
    }
    .intersect(clip);
    if selected.height() <= 0. {
        return;
    }
    let native_sticker = ctx
        .input(|i| i.viewport().inner_rect)
        .filter(|_| !ctx.embed_viewports());
    let embedded = native_sticker.is_none();
    let sticker = native_sticker.unwrap_or_else(|| ctx.screen_rect());
    let origin = native_sticker.map_or(Vec2::ZERO, |r| r.min.to_vec2());
    let work = if embedded {
        sticker
    } else {
        crate::native_window::work_area(ctx).unwrap_or(sticker)
    };
    let size = Vec2::new(236., 106.);
    let Some(at) = selection_popup_position(
        sticker,
        clip.translate(origin),
        selected.translate(origin),
        caret.center() + origin,
        work.shrink(4.),
        size,
    ) else {
        return;
    };
    let mut controls = Vec::with_capacity(15);
    let mut clipboard = None;
    let mut color = None;
    let draw = |ui: &mut egui::Ui| {
        let tone = crate::ui::t(ui);
        egui::Frame::NONE
            .fill(tone.bg)
            .stroke(Stroke::new(1.0_f32, tone.line))
            .inner_margin(egui::Margin::same(6))
            .corner_radius(8.)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = Vec2::splat(2.);
                ui.horizontal(|ui| {
                    for (label, event) in
                        [("잘라내기", egui::Event::Cut), ("복사", egui::Event::Copy)]
                    {
                        let button = ui.add_sized([108., 22.], egui::Button::new(label));
                        controls.push(button.rect);
                        if button.clicked() {
                            clipboard = Some(event);
                        }
                    }
                });
                for (highlight, label, colors) in [
                    (false, "글자색", &TEXT_COLORS[..]),
                    (true, "형광펜", &HIGHLIGHTS[..]),
                ] {
                    ui.horizontal(|ui| {
                        ui.add_sized([48., 22.], egui::Label::new(label).extend());
                        for (index, &(_, swatch)) in colors.iter().enumerate() {
                            let button = paint_color_chip(ui, swatch, highlight);
                            controls.push(button.rect);
                            if button.clicked() {
                                color = Some((highlight, index as u8));
                            }
                        }
                    });
                }
            });
    };
    let mut popup_escape = false;
    let mut popup_focused = false;
    let popup_rect = if embedded {
        Some(
            egui::Area::new(key)
                .order(egui::Order::Foreground)
                .fixed_pos(at)
                .default_size(size)
                .fade_in(false)
                .show(ctx, draw)
                .response
                .rect,
        )
    } else {
        let builder = egui::ViewportBuilder::default()
            .with_title("선택한 글 서식")
            .with_position(at)
            .with_inner_size(size)
            .with_active(false)
            .with_decorations(false)
            .with_resizable(false)
            .with_taskbar(false)
            .with_window_level(egui::WindowLevel::AlwaysOnTop);
        let mut draw = Some(draw);
        ctx.show_viewport_immediate(popup_id, builder, |popup_ctx, _| {
            crate::win_style::shape(popup_ctx, 1.);
            popup_escape = popup_ctx.input(|i| i.key_pressed(Key::Escape));
            popup_focused = crate::win_style::is_foreground(popup_ctx)
                .unwrap_or_else(|| popup_ctx.input(|i| i.focused));
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(popup_ctx.style().visuals.panel_fill))
                .show(popup_ctx, |ui| {
                    if let Some(mut draw) = draw.take() {
                        draw(ui);
                    }
                });
        });
        None
    };
    if !embedded && (popup_escape || clipboard.is_some() || color.is_some()) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        ctx.memory_mut(|m| m.request_focus(text_id));
    }
    if popup_escape {
        return;
    }
    if let Some(event) = clipboard {
        output.state.cursor.set_char_range(Some(range));
        ui.data_mut(|d| d.insert_temp(id.with("edit_menu_events"), vec![event]));
        ctx.request_repaint();
    } else if let Some((highlight, index)) = color {
        let [a, b] = range.sorted();
        doc.selection = a.index..b.index;
        output.state.cursor.set_char_range(Some(range));
        ctx.memory_mut(|m| m.request_focus(text_id));
        doc.paint_selection_with_history(highlight, index, ui.input(|i| i.time));
        ctx.request_repaint();
    } else {
        let viewport = if embedded {
            ctx.viewport_id()
        } else {
            egui::ViewportId(key)
        };
        let owner_focused = crate::win_style::is_foreground(ctx).unwrap_or(focused);
        let lost_focus_at = if !embedded && !owner_focused && !popup_focused {
            let now = ui.input(|i| i.time);
            let since = old_native.and_then(|p| p.lost_focus_at).unwrap_or(now);
            if now - since >= 0.05 {
                return;
            }
            // The two native focus notifications can land on different passes.
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(0.05 - (now - since)));
            Some(since)
        } else {
            None
        };
        ui.data_mut(|d| {
            d.insert_temp(
                key,
                Some(SelectionPopup {
                    text_id,
                    range,
                    rect: popup_rect,
                    lost_focus_at,
                }),
            );
            d.insert_temp(
                Id::new("eframe_stable_hover_rects").with(viewport),
                std::sync::Arc::new(controls),
            );
        });
    }
}

fn selection_popup_position(
    sticker: egui::Rect,
    body: egui::Rect,
    selection: egui::Rect,
    caret: egui::Pos2,
    work: egui::Rect,
    size: Vec2,
) -> Option<egui::Pos2> {
    let gap = 5.;
    let x = (caret.x - size.x * 0.5).clamp(work.left(), (work.right() - size.x).max(work.left()));
    let y = (caret.y - size.y * 0.5).clamp(work.top(), (work.bottom() - size.y).max(work.top()));
    let body_work = body.intersect(work);
    let body_x = x.clamp(
        body_work.left(),
        (body_work.right() - size.x).max(body_work.left()),
    );
    [
        egui::pos2(sticker.right() + gap, y),
        egui::pos2(sticker.left() - gap - size.x, y),
        egui::pos2(x, sticker.bottom() + gap),
        egui::pos2(x, sticker.top() - gap - size.y),
        egui::pos2(body_x, selection.bottom() + gap),
        egui::pos2(body_x, selection.top() - gap - size.y),
    ]
    .into_iter()
    .filter(|&pos| {
        let rect = egui::Rect::from_min_size(pos, size);
        work.contains_rect(rect)
            && (!rect.intersects(sticker) || body.contains_rect(rect))
            && !rect.intersects(selection)
    })
    .min_by(|&a, &b| {
        let distance = |pos| egui::Rect::from_min_size(pos, size).distance_sq_to_pos(caret);
        distance(a).total_cmp(&distance(b))
    })
}

/// Read-only rendering for a saved comment. The document and legacy text are
/// never changed. Tab selection and image textures belong to temporary UI state.
pub fn view(ui: &mut egui::Ui, legacy: &str, doc: &Document, id: impl Hash) {
    let id = ui.make_persistent_id(id);
    if doc.tabs.is_empty() {
        ui.label(legacy);
        return;
    }
    let visible: Vec<_> = doc
        .tabs
        .iter()
        .enumerate()
        .filter(|(_, t)| !t.hidden)
        .map(|(i, _)| i)
        .collect();
    let fallback = if visible.contains(&doc.active) {
        doc.active
    } else {
        visible.first().copied().unwrap_or(0)
    };
    let selection_id = id.with("view_tab");
    let mut active = ui
        .ctx()
        .data(|d| d.get_temp::<usize>(selection_id))
        .filter(|&i| i < doc.tabs.len() && !doc.tabs[i].hidden)
        .unwrap_or(fallback);
    if visible.len() > 1 {
        ui.horizontal_wrapped(|ui| {
            for &i in &visible {
                if ui
                    .selectable_label(active == i, &doc.tabs[i].title)
                    .clicked()
                {
                    active = i;
                }
            }
        });
    }
    ui.ctx().data_mut(|d| d.insert_temp(selection_id, active));
    let tab = &doc.tabs[active];
    let body = if active == 0 {
        legacy
    } else {
        tab.body.as_str()
    };
    let font = font_id(ui, doc.font_size, &doc.font_name, false);
    let bold = font_id(ui, doc.font_size, &doc.font_name, true);
    let mut job = layout(
        body,
        &tab.spans,
        &Format::default(),
        font.clone(),
        bold.clone(),
        ui.visuals().text_color(),
    );
    job.wrap.max_width = ui.available_width();
    let label = ui.add(egui::Label::new(job).selectable(true));
    crate::float::context_menu(&label, 160., |ui| {
        if ui.button("본문 복사").clicked() {
            ui.ctx().copy_text(body.to_owned());
        }
    });
    let links = urls(body);
    if !links.is_empty() {
        ui.horizontal_wrapped(|ui| {
            for url in links {
                let name = doc
                    .aliases
                    .iter()
                    .find(|(address, _)| address == &url)
                    .map(|(_, name)| name.as_str())
                    .unwrap_or(&url);
                let r = ui
                    .link(short(name, 36))
                    .on_hover_text(format!("{url}\nCtrl+클릭: 주소 복사"));
                if r.clicked() {
                    if ui.input(|i| i.modifiers.ctrl) {
                        ui.ctx().copy_text(url);
                    } else {
                        ui.ctx().open_url(egui::OpenUrl::new_tab(url));
                    }
                }
            }
        });
    }
    for (bi, block) in tab.blocks.iter().enumerate() {
        ui.add_space(6.);
        match block {
            Block::Table(table) => {
                if table.cells.is_empty() {
                    continue;
                }
                egui::ScrollArea::horizontal()
                    .id_salt(id.with(("view_table", active, bi)))
                    .show(ui, |ui| {
                        egui::Grid::new(id.with(("view_grid", active, bi)))
                            .spacing([1., 1.])
                            .striped(true)
                            .show(ui, |ui| {
                                for row in &table.cells {
                                    for (c, cell) in row.iter().enumerate() {
                                        if cell.covered {
                                            continue;
                                        }
                                        let width: f32 = table
                                            .col_widths
                                            .get(c..(c + cell.col_span).min(table.col_widths.len()))
                                            .map(|ws| ws.iter().sum())
                                            .unwrap_or(120.)
                                            .max(60.);
                                        egui::Frame::NONE
                                            .inner_margin(6.)
                                            .stroke(Stroke::new(
                                                1.0_f32,
                                                ui.visuals().widgets.noninteractive.bg_stroke.color,
                                            ))
                                            .show(ui, |ui| {
                                                ui.set_width(width);
                                                let mut job = layout(
                                                    &cell.text,
                                                    &[],
                                                    &cell.format,
                                                    font.clone(),
                                                    bold.clone(),
                                                    ui.visuals().text_color(),
                                                );
                                                job.wrap.max_width = width;
                                                ui.add(egui::Label::new(job).selectable(true));
                                            });
                                    }
                                    ui.end_row();
                                }
                            });
                    });
            }
            Block::Picture(picture) => {
                if picture.width == 0 || picture.height == 0 {
                    continue;
                }
                let available = ui.available_width().max(16.);
                let width = if picture.fit {
                    available
                } else {
                    picture.width as f32 * picture.scale
                };
                let size = Vec2::new(width, width * picture.height as f32 / picture.width as f32);
                let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
                // Pictures are read from disk only once they scroll into view.
                if !ui.is_rect_visible(rect) {
                    continue;
                }
                let texture_id =
                    id.with(("view_picture", active, bi, picture.width, picture.height));
                let texture = picture.texture.clone().or_else(|| {
                    crate::lists::texture(ui.ctx(), texture_id, || picture.color_image())
                });
                if let Some(texture) = texture {
                    ui.painter().image(
                        texture.id(),
                        rect,
                        egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1., 1.)),
                        Color32::WHITE,
                    );
                }
                response.on_hover_text(&picture.name);
            }
        }
    }
}

fn font_id(ui: &egui::Ui, size: f32, name: &str, bold: bool) -> FontId {
    let wanted = if bold {
        format!("{name} Bold")
    } else {
        name.into()
    };
    let family = FontFamily::Name(wanted.clone().into());
    let found = ui.fonts(|f| f.families().contains(&family));
    FontId::new(
        size,
        if found {
            family
        } else {
            FontFamily::Proportional
        },
    )
}
/// For every character, the index of the last span that covers it (the
/// one that wins), or `u32::MAX`. Each character is written once however
/// many spans there are or however they overlap: spans are taken from the
/// last, and `next[i]` skips the characters already decided.
fn winning_spans(spans: &[Span], chars: usize) -> Vec<u32> {
    let mut winner = vec![u32::MAX; chars];
    // `next[i]`: an undecided character at or after `i` (`chars` if none).
    let mut next: Vec<u32> = (0..=chars as u32).collect();
    for (k, span) in spans.iter().enumerate().rev() {
        let end = span.range.end.min(chars);
        let mut i = span.range.start;
        while i < end {
            // Find the first undecided character, shortening the path to it.
            let mut free = i;
            while next[free] as usize != free {
                free = next[free] as usize;
            }
            while next[i] as usize != i {
                let step = next[i] as usize;
                next[i] = free as u32;
                i = step;
            }
            if free >= end {
                break;
            }
            winner[free] = k as u32;
            next[free] = free as u32 + 1;
            i = free + 1;
        }
    }
    winner
}
/// One section per character (so no kerning or letter spacing is applied
/// between characters, and a picture or table placeholder is a section of
/// its own), in the format of the last span covering it.
fn layout(
    text: &str,
    spans: &[Span],
    base: &Format,
    font: FontId,
    bold_font: FontId,
    color: Color32,
) -> LayoutJob {
    let winner = if spans.is_empty() {
        vec![]
    } else {
        winning_spans(spans, text.chars().count())
    };
    let stroke = Stroke::new(1.0_f32, color);
    let link = Color32::from_rgb(61, 105, 166);
    let mut job = LayoutJob {
        text: text.to_owned(),
        ..Default::default()
    };
    job.sections.reserve(winner.len().max(text.len() / 3));
    // Whether the word being read starts with a URL scheme; asked once per word.
    let mut token_is_link = text.starts_with("https://") || text.starts_with("http://");
    for (i, (byte, ch)) in text.char_indices().enumerate() {
        let f = match winner.get(i) {
            Some(&k) if k != u32::MAX => &spans[k as usize].format,
            _ => base,
        };
        let mut format = TextFormat {
            font_id: if f.bold {
                bold_font.clone()
            } else {
                font.clone()
            },
            color,
            italics: f.italic,
            extra_letter_spacing: if f.bold { 0.25 } else { 0. },
            underline: if f.underline { stroke } else { Stroke::NONE },
            strikethrough: if f.strike { stroke } else { Stroke::NONE },
            ..Default::default()
        };
        if f.color > 0 {
            if let Some(&(_, c)) = TEXT_COLORS.get(f.color as usize) {
                format.color = c;
                format.underline.color = c;
                format.strikethrough.color = c;
            }
        }
        if f.highlight > 0 {
            if let Some(&(_, c)) = HIGHLIGHTS.get(f.highlight as usize) {
                format.background = c;
                // Readable on the highlighter, whatever the theme.
                if f.color == 0 {
                    format.color = Color32::from_rgb(30, 30, 30);
                }
            }
        }
        let end = byte + ch.len_utf8();
        // Automatic link emphasis uses the URL's exact lexical extent.
        if ch.is_whitespace() {
            let rest = &text[end..];
            token_is_link = rest.starts_with("https://") || rest.starts_with("http://");
        } else if token_is_link {
            format.color = link;
            format.underline = Stroke::new(1.0_f32, link);
        }
        job.sections.push(egui::text::LayoutSection {
            leading_space: 0.,
            byte_range: byte..end,
            format,
        });
    }
    job
}
#[cfg(test)]
fn layout_reference(
    text: &str,
    spans: &[Span],
    base: &Format,
    font: FontId,
    bold_font: FontId,
    color: Color32,
) -> LayoutJob {
    let mut job = LayoutJob::default();
    let mut token_start = 0;
    for (i, (byte, ch)) in text.char_indices().enumerate() {
        let f = spans
            .iter()
            .rev()
            .find(|s| s.range.contains(&i))
            .map(|s| &s.format)
            .unwrap_or(base);
        let stroke = Stroke::new(1.0_f32, color);
        let mut style = TextFormat {
            font_id: if f.bold {
                bold_font.clone()
            } else {
                font.clone()
            },
            color,
            italics: f.italic,
            extra_letter_spacing: if f.bold { 0.25 } else { 0. },
            underline: if f.underline { stroke } else { Stroke::NONE },
            strikethrough: if f.strike { stroke } else { Stroke::NONE },
            ..Default::default()
        };
        // Automatic link emphasis uses the URL's exact lexical extent.
        if ch.is_whitespace() {
            token_start = byte + ch.len_utf8();
        }
        if !ch.is_whitespace()
            && (text[token_start..].starts_with("https://")
                || text[token_start..].starts_with("http://"))
        {
            style.color = Color32::from_rgb(61, 105, 166);
            style.underline = Stroke::new(1.0_f32, style.color);
        }
        job.append(&ch.to_string(), 0., style);
    }
    job
}
/// How long a message (undo, copy, an error) stays under the memo.
const STATUS_SECS: f64 = 5.;
/// A message goes away by itself: it used to stay, with the row it takes,
/// until another one replaced it.
fn expire_status(ui: &egui::Ui, doc: &mut Document, id: Id) {
    if doc.status.is_empty() {
        return;
    }
    let key = id.with("status_shown");
    let now = ui.input(|i| i.time);
    let shown = ui.data(|d| d.get_temp::<(String, f64)>(key));
    let since = match shown {
        Some((text, since)) if text == doc.status => since,
        _ => {
            ui.data_mut(|d| d.insert_temp(key, (doc.status.clone(), now)));
            now
        }
    };
    let left = STATUS_SECS - (now - since);
    if left <= 0. {
        doc.status.clear();
        ui.data_mut(|d| d.remove::<(String, f64)>(key));
    } else {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs_f64(left));
    }
}
fn render_footer(
    ui: &mut egui::Ui,
    doc: &mut Document,
    id: Id,
    trailing_space: f32,
    bottom_up: bool,
    divider: bool,
    opacity: bool,
) -> egui::Response {
    if doc.tabs.is_empty() {
        doc.push_tab("본문".into());
    }
    expire_status(ui, doc, id);
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 4.;
        let status = |ui: &mut egui::Ui, doc: &Document| {
            if !doc.status.is_empty() {
                ui.add(
                    egui::Label::new(
                        RichText::new(&doc.status)
                            .small()
                            .color(Color32::from_rgb(155, 75, 45)),
                    )
                    .truncate(),
                )
                .on_hover_text(&doc.status);
            }
        };
        let tools = |ui: &mut egui::Ui, doc: &mut Document| {
            toolbar(ui, doc, id, opacity);
        };
        // The corner buttons (project, linked memos) sit on the tab row.
        let tab_row = |ui: &mut egui::Ui, doc: &mut Document| {
            ui.scope(|ui| {
                ui.set_max_width((ui.available_width() - trailing_space.max(0.)).max(28.));
                tabs(ui, doc, id);
            });
        };
        let line = |ui: &mut egui::Ui| {
            if divider {
                crate::ui::divider(ui);
            } else {
                ui.add_space(1.);
            }
        };
        if bottom_up {
            if doc.toolbar_visible {
                tools(ui, doc);
                line(ui);
            }
            tab_row(ui, doc);
            status(ui, doc);
        } else {
            status(ui, doc);
            tab_row(ui, doc);
            if doc.toolbar_visible {
                line(ui);
                tools(ui, doc);
            }
        }
    })
    .response
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Tool {
    Font,
    Size,
    Bold,
    Italic,
    Underline,
    Strike,
    Color,
    Opacity,
}
// Images and tables arrive from the clipboard; zoom stays on Ctrl+wheel.
const TOOLS: [Tool; 8] = [
    Tool::Font,
    Tool::Size,
    Tool::Bold,
    Tool::Italic,
    Tool::Underline,
    Tool::Strike,
    Tool::Color,
    Tool::Opacity,
];
/// Height of a memo footer's format buttons at `width`.
pub fn footer_tools_height(width: f32) -> f32 {
    toolbar_height(width, TOOLS.len())
}
fn toolbar_height(width: f32, count: usize) -> f32 {
    let columns = (((width.max(28.) + 2.) / 30.).floor() as usize).clamp(1, count);
    let rows = count.div_ceil(columns);
    rows as f32 * 28. + rows.saturating_sub(1) as f32 * 2.
}
fn toolbar(ui: &mut egui::Ui, doc: &mut Document, id: Id, opacity: bool) -> egui::Response {
    let width = ui.available_width().min(ui.clip_rect().width()).max(28.);
    let tools: Vec<Tool> = TOOLS
        .into_iter()
        .filter(|t| opacity || *t != Tool::Opacity)
        .collect();
    let height = toolbar_height(width, tools.len());
    ui.allocate_ui_with_layout(
        Vec2::new(width, height),
        egui::Layout::top_down(egui::Align::Min),
        |ui| {
            ui.spacing_mut().item_spacing = Vec2::splat(2.);
            ui.spacing_mut().button_padding = Vec2::new(6., 4.);
            ui.set_max_width(ui.available_width().min(ui.clip_rect().width()).max(28.));
            ui.horizontal_wrapped(|ui| {
                for tool in tools {
                    let (label, selected) = match tool {
                        Tool::Font => (format!("글꼴 · {}", doc.font_name), false),
                        Tool::Size => {
                            (format!("글자 크기 · {:.0} pt", doc.font_size * 0.75), false)
                        }
                        Tool::Bold => ("굵게 · Ctrl+B".into(), doc.format.bold),
                        Tool::Italic => ("기울임 · Ctrl+I".into(), doc.format.italic),
                        Tool::Underline => ("밑줄 · Ctrl+U".into(), doc.format.underline),
                        Tool::Strike => ("취소선".into(), doc.format.strike),
                        Tool::Color => ("글자 색 · 형광펜".into(), false),
                        Tool::Opacity => {
                            (format!("메모 투명도 · {:.0}%", doc.opacity * 100.), false)
                        }
                    };
                    let size_text =
                        (tool == Tool::Size).then(|| format!("{:.0}", doc.font_size * 0.75));
                    let response = tool_button(ui, tool, &label, selected, size_text);
                    match tool {
                        Tool::Bold | Tool::Italic | Tool::Underline | Tool::Strike => {
                            if response.clicked() {
                                doc.toggle_format(match tool {
                                    Tool::Bold => 0,
                                    Tool::Italic => 1,
                                    Tool::Underline => 2,
                                    _ => 3,
                                });
                            }
                        }
                        _ => {
                            let popup_id = id.with(("tool_popup", tool));
                            if response.clicked() {
                                ui.memory_mut(|m| m.toggle_popup(popup_id));
                            }
                            crate::float::popup(
                                ui,
                                popup_id,
                                &response,
                                match tool {
                                    Tool::Color => 250.,
                                    // The slider with its value, and four presets.
                                    Tool::Opacity => 250.,
                                    _ => 190.,
                                },
                                |ui| {
                                    ui.set_min_width(170.);
                                    match tool {
                                        Tool::Font => {
                                            ui.strong("글꼴");
                                            // Its window grows to fit what it
                                            // shows: without a minimum the list
                                            // kept the first, small window's
                                            // height (three fonts).
                                            egui::ScrollArea::vertical()
                                                .max_height(220.)
                                                .min_scrolled_height(220.)
                                                .show(ui, |ui| {
                                                    for name in font_choices() {
                                                        if ui
                                                            .selectable_label(
                                                                doc.font_name == name,
                                                                name,
                                                            )
                                                            .clicked()
                                                        {
                                                            doc.font_name = name.into();
                                                            ui.memory_mut(|m| m.close_popup());
                                                        }
                                                    }
                                                });
                                        }
                                        Tool::Size => {
                                            ui.strong("글자 크기");
                                            ui.horizontal_wrapped(|ui| {
                                                for pt in [
                                                    8., 10., 11., 12., 14., 16., 18., 20., 24., 32.,
                                                ] {
                                                    if ui
                                                        .selectable_label(
                                                            (doc.font_size * 0.75 - pt).abs() < 0.1,
                                                            format!("{pt:.0} pt"),
                                                        )
                                                        .clicked()
                                                    {
                                                        doc.font_size = pt / 0.75;
                                                        ui.memory_mut(|m| m.close_popup());
                                                    }
                                                }
                                            });
                                            ui.separator();
                                            let mut pt = (doc.font_size * 0.75).round();
                                            if ui
                                                .add(
                                                    egui::DragValue::new(&mut pt)
                                                        .range(6.0..=48.0)
                                                        .speed(1.)
                                                        .fixed_decimals(0)
                                                        .suffix(" pt"),
                                                )
                                                .changed()
                                            {
                                                doc.font_size = pt.round() / 0.75;
                                            }
                                        }
                                        Tool::Color => {
                                            ui.set_min_width(230.);
                                            for (highlight, title, colors) in [
                                                (false, "글자 색", &TEXT_COLORS[..]),
                                                (true, "형광펜", &HIGHLIGHTS[..]),
                                            ] {
                                                ui.strong(title);
                                                ui.horizontal_wrapped(|ui| {
                                                    for (k, &(name, c)) in colors.iter().enumerate()
                                                    {
                                                        if color_chip(ui, name, c, highlight)
                                                            .clicked()
                                                        {
                                                            doc.paint_selection_with_history(
                                                                highlight,
                                                                k as u8,
                                                                ui.input(|i| i.time),
                                                            );
                                                            ui.memory_mut(|m| m.close_popup());
                                                        }
                                                    }
                                                });
                                            }
                                            crate::ui::caption(
                                                ui,
                                                "글자를 먼저 선택한 뒤 색을 고르세요",
                                            );
                                        }
                                        Tool::Opacity => {
                                            crate::ui::caption(ui, "메모 투명도");
                                            let mut pct = (doc.opacity * 100.).round();
                                            if ui
                                                .add(
                                                    egui::Slider::new(&mut pct, 20.0..=100.)
                                                        .integer()
                                                        .suffix("%"),
                                                )
                                                .changed()
                                            {
                                                doc.opacity = pct / 100.;
                                            }
                                            ui.horizontal(|ui| {
                                                for v in [40., 60., 80., 100.] {
                                                    if ui
                                                        .selectable_label(
                                                            (pct - v).abs() < 0.5,
                                                            format!("{v:.0}%"),
                                                        )
                                                        .clicked()
                                                    {
                                                        doc.opacity = v / 100.;
                                                    }
                                                }
                                            });
                                        }
                                        _ => {}
                                    }
                                },
                            );
                        }
                    }
                }
            });
        },
    )
    .response
}
/// "투명도" with its slider and four presets, for a memo window's
/// right-click menus. Returns whether the pointer is on it (a menu that
/// closes on a click stays open for it).
pub fn opacity_row(ui: &mut egui::Ui, opacity: &mut f32) -> bool {
    let r = ui
        .horizontal(|ui| {
            ui.label("투명도");
            let mut pct = (*opacity * 100.).round();
            ui.spacing_mut().slider_width = 96.;
            if ui
                .add(
                    egui::Slider::new(&mut pct, 20.0..=100.)
                        .integer()
                        .suffix("%"),
                )
                .changed()
            {
                *opacity = pct / 100.;
            }
        })
        .response;
    let presets = ui
        .horizontal(|ui| {
            ui.spacing_mut().button_padding = Vec2::new(6., 2.);
            let pct = (*opacity * 100.).round();
            for v in [40., 60., 80., 100.] {
                if ui
                    .selectable_label((pct - v).abs() < 0.5, format!("{v:.0}%"))
                    .clicked()
                {
                    *opacity = v / 100.;
                }
            }
        })
        .response;
    ui.rect_contains_pointer(r.rect.union(presets.rect))
}
/// A swatch in the color popup ("없음"/"기본": a crossed box).
fn color_chip(ui: &mut egui::Ui, name: &str, c: Color32, highlight: bool) -> egui::Response {
    paint_color_chip(ui, c, highlight).on_hover_text(name)
}

fn paint_color_chip(ui: &mut egui::Ui, c: Color32, highlight: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(22.), egui::Sense::click());
    let ink = ui.visuals().text_color();
    let r = rect.shrink(2.);
    let faint = Stroke::new(1.0_f32, ink.gamma_multiply(0.6));
    if c == Color32::TRANSPARENT {
        ui.painter()
            .rect_stroke(r, 4., faint, egui::StrokeKind::Inside);
        ui.painter()
            .line_segment([r.left_bottom(), r.right_top()], faint);
    } else if highlight {
        ui.painter().rect_filled(r, 4., c);
    } else {
        ui.painter().text(
            r.center(),
            egui::Align2::CENTER_CENTER,
            "가",
            FontId::proportional(15.),
            c,
        );
    }
    if response.hovered() {
        ui.painter().rect_stroke(
            rect,
            5.,
            Stroke::new(1.5_f32, ink),
            egui::StrokeKind::Inside,
        );
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}
fn tool_button(
    ui: &mut egui::Ui,
    tool: Tool,
    label: &str,
    selected: bool,
    text: Option<String>,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(28.), egui::Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    if selected || response.hovered() || response.has_focus() {
        ui.painter().rect_filled(
            rect,
            5.,
            if selected {
                ui.visuals().selection.bg_fill
            } else {
                ui.visuals().widgets.hovered.weak_bg_fill
            },
        );
    }
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect.shrink(0.5),
            5.,
            Stroke::new(1.0_f32, ui.visuals().selection.stroke.color),
            egui::StrokeKind::Inside,
        );
    }
    let color = ui
        .visuals()
        .text_color()
        .gamma_multiply(if selected || response.hovered() {
            1.
        } else {
            0.78
        });
    if let Some(text) = text {
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            text,
            FontId::proportional(12.5),
            color,
        );
    } else {
        paint_tool(ui.painter(), rect.shrink(5.), tool, color);
    }
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(label)
}
fn paint_tool(painter: &egui::Painter, rect: egui::Rect, tool: Tool, color: Color32) {
    let unit = rect.width().min(rect.height()) / 24.;
    let origin = rect.center() - Vec2::splat(12. * unit);
    let p = |x: f32, y: f32| origin + Vec2::new(x, y) * unit;
    let stroke = Stroke::new(1.6_f32 * unit, color);
    let line = |a: (f32, f32), b: (f32, f32)| {
        painter.line_segment([p(a.0, a.1), p(b.0, b.1)], stroke);
    };
    let path = |points: &[(f32, f32)]| {
        painter.add(egui::Shape::line(
            points.iter().map(|&(x, y)| p(x, y)).collect(),
            stroke,
        ));
    };
    match tool {
        Tool::Font => {
            path(&[(4., 20.), (11., 4.), (18., 20.)]);
            line((7., 13.), (15., 13.));
            line((17., 5.), (22., 5.));
            line((19.5, 5.), (19.5, 10.));
        }
        Tool::Size => {
            line((3., 5.), (15., 5.));
            line((9., 5.), (9., 20.));
            line((5., 20.), (13., 20.));
            line((17., 10.), (23., 10.));
            line((20., 10.), (20., 20.));
        }
        Tool::Bold => {
            path(&[
                (6., 20.),
                (6., 4.),
                (13., 4.),
                (17., 6.),
                (17., 9.),
                (13., 12.),
                (6., 12.),
            ]);
            path(&[(13., 12.), (18., 14.), (18., 18.), (14., 20.), (6., 20.)]);
        }
        Tool::Italic => {
            line((10., 4.), (20., 4.));
            line((5., 20.), (15., 20.));
            line((15., 4.), (10., 20.));
        }
        Tool::Underline => {
            path(&[
                (6., 4.),
                (6., 13.),
                (8., 17.),
                (12., 18.),
                (16., 17.),
                (18., 13.),
                (18., 4.),
            ]);
            line((4., 22.), (20., 22.));
        }
        Tool::Strike => {
            path(&[
                (18., 6.),
                (15., 3.),
                (9., 3.),
                (6., 6.),
                (7., 9.),
                (16., 15.),
                (18., 18.),
                (15., 21.),
                (9., 21.),
                (6., 19.),
            ]);
            line((3., 12.), (21., 12.));
        }
        Tool::Color => {
            // A pen tip over a color bar.
            path(&[(6., 16.), (14., 4.), (18., 7.), (10., 19.), (6., 16.)]);
            painter.rect_filled(
                egui::Rect::from_min_max(p(3., 20.), p(21., 23.)),
                1.,
                Color32::from_rgb(230, 190, 40),
            );
        }
        Tool::Opacity => {
            painter.circle_stroke(p(12., 12.), 8. * unit, stroke);
            let mut points = vec![p(12., 4.), p(12., 20.)];
            for i in 0..=12 {
                let angle = std::f32::consts::FRAC_PI_2 - i as f32 * std::f32::consts::PI / 12.;
                points.push(p(12. + 8. * angle.cos(), 12. + 8. * angle.sin()));
            }
            painter.add(egui::Shape::convex_polygon(points, color, Stroke::NONE));
        }
    }
}

/// A compact tab chip: the selected one is lifted, the rest are plain text.
/// A document tab: plain text, the selected one carries an underline.
fn tab_chip(ui: &mut egui::Ui, text: &str, selected: bool) -> egui::Response {
    let ink = ui.visuals().text_color();
    let galley = ui.painter().layout_no_wrap(
        text.into(),
        FontId::proportional(12.5),
        if selected {
            ink
        } else {
            ink.gamma_multiply(0.55)
        },
    );
    let (rect, r) =
        ui.allocate_exact_size(Vec2::new(galley.size().x + 16., 24.), egui::Sense::click());
    if r.hovered() && !selected {
        ui.painter().rect_filled(rect, 6., ink.gamma_multiply(0.07));
    }
    let text_rect = egui::Rect::from_center_size(rect.center() - Vec2::new(0., 1.), galley.size());
    ui.painter().galley(text_rect.min, galley, ink);
    if selected {
        let y = rect.bottom() - 1.5;
        ui.painter().line_segment(
            [
                egui::pos2(text_rect.left(), y),
                egui::pos2(text_rect.right(), y),
            ],
            Stroke::new(2.0_f32, ink.gamma_multiply(0.8)),
        );
    }
    r.on_hover_cursor(egui::CursorIcon::PointingHand)
}
/// A small round icon button used beside the tabs.
fn tab_icon(ui: &mut egui::Ui, icon: crate::icons::Icon, tip: &str) -> egui::Response {
    let ink = ui.visuals().text_color();
    let (rect, r) = ui.allocate_exact_size(Vec2::new(22., 24.), egui::Sense::click());
    if r.hovered() {
        ui.painter()
            .circle_filled(rect.center(), 10., ink.gamma_multiply(0.08));
    }
    crate::icons::paint_icon(
        ui.painter(),
        egui::Rect::from_center_size(rect.center(), Vec2::splat(12.)),
        icon,
        ink.gamma_multiply(if r.hovered() { 0.85 } else { 0.5 }),
    );
    r.on_hover_text(tip)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}
fn tabs(ui: &mut egui::Ui, doc: &mut Document, id: Id) {
    ui.add_space(2.);
    let mut delete = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.;
        let visible: Vec<_> = doc
            .tabs
            .iter()
            .enumerate()
            .filter(|(_, t)| !t.hidden)
            .map(|(i, _)| i)
            .collect();
        let limit = ((ui.available_width() - 82.) / 82.).floor().max(1.) as usize;
        let max_scroll = visible.len().saturating_sub(limit);
        doc.tab_scroll = doc.tab_scroll.min(max_scroll);
        if max_scroll > 0 && tab_icon(ui, crate::icons::Icon::ChevronLeft, "이전 탭").clicked() {
            doc.tab_scroll = doc.tab_scroll.saturating_sub(1);
        }
        for &i in visible.iter().skip(doc.tab_scroll).take(limit) {
            let r = tab_chip(ui, &short(&doc.tabs[i].title, 10), doc.active == i);
            if r.clicked() {
                doc.active = i;
                doc.selected_cell = None;
                doc.tsel = None;
                doc.selection = 0..0;
            }
            if r.double_clicked() {
                doc.rename = Some(i);
            }
            crate::float::context_menu(&r, 200., |ui| {
                if ui.button("이름 바꾸기").clicked() {
                    doc.rename = Some(i);
                    ui.close_menu();
                }
                if visible.len() > 1 && ui.button("이 탭 숨기기").clicked() {
                    doc.tabs[i].hidden = true;
                    if doc.active == i {
                        doc.active = doc.tabs.iter().position(|t| !t.hidden).unwrap_or(0);
                    }
                    ui.close_menu();
                }
                if ui.button("탭 삭제 · 휴지통으로").clicked() {
                    delete = Some(i);
                    ui.close_menu();
                }
                if doc.tabs.iter().any(|t| t.hidden) {
                    ui.separator();
                    ui.label(
                        egui::RichText::new("숨긴 탭 보이기")
                            .small()
                            .color(crate::ui::t(ui).muted),
                    );
                    for t in doc.tabs.iter_mut().filter(|t| t.hidden) {
                        if ui.button(format!("  {}", t.title)).clicked() {
                            t.hidden = false;
                        }
                    }
                }
            });
        }
        if max_scroll > 0 && tab_icon(ui, crate::icons::Icon::ChevronRight, "다음 탭").clicked()
        {
            doc.tab_scroll = (doc.tab_scroll + 1).min(max_scroll);
        }
        if tab_icon(
            ui,
            crate::icons::Icon::Plus,
            "본문 탭 추가 · 메모 속성은 함께 사용",
        )
        .clicked()
        {
            doc.add_tab();
            doc.tab_scroll = max_scroll + 1;
        }
    });
    if let Some(i) = delete {
        doc.delete_tab(i);
    }
    if let Some(i) = doc.rename {
        if i < doc.tabs.len() {
            let mut open = true;
            crate::ui::dialog(
                ui.ctx(),
                id.with("rename_popup"),
                "탭 이름 바꾸기",
                [300., 150.],
                true,
                &mut open,
                |ui| {
                    let r = ui.add(
                        egui::TextEdit::singleline(&mut doc.tabs[i].title)
                            .id(id.with("rename"))
                            .desired_width(f32::INFINITY)
                            .margin(egui::Margin::symmetric(10, 6)),
                    );
                    r.request_focus();
                    let enter = ui.input(|i| i.key_pressed(Key::Enter));
                    let (ok, _) = crate::ui::dialog_buttons(ui, "확인", "");
                    if ok || enter {
                        doc.rename = None;
                    }
                },
            );
            if !open {
                doc.rename = None;
            }
        } else {
            doc.rename = None;
        }
    }
}
fn show_links(ui: &mut egui::Ui, doc: &mut Document, id: Id) {
    let urls = urls(&doc.tabs[doc.active].body);
    if urls.is_empty() {
        return;
    }
    ui.horizontal_wrapped(|ui| {
        for url in urls {
            let pos = doc
                .aliases
                .iter()
                .position(|(u, _)| u == &url)
                .unwrap_or_else(|| {
                    doc.aliases.push((url.clone(), url.clone()));
                    doc.aliases.len() - 1
                });
            let r = ui
                .link(short(&doc.aliases[pos].1, 32))
                .on_hover_text(format!("{url}\nCtrl+클릭: 주소 복사 · 우클릭: 이름 변경"));
            if r.clicked() {
                if ui.input(|i| i.modifiers.ctrl) {
                    ui.ctx().copy_text(url.clone());
                    doc.status = "링크 주소를 복사했습니다".into();
                } else {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(url.clone()));
                }
            }
            crate::float::context_menu(&r, 200., |ui| {
                ui.label("하이퍼링크 이름");
                ui.add(
                    egui::TextEdit::singleline(&mut doc.aliases[pos].1)
                        .id(id.with(("alias", pos)))
                        .desired_width(170.),
                );
                if ui.button("링크 열기").clicked() {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(url.clone()));
                    ui.close_menu();
                }
                if ui.button("주소 복사").clicked() {
                    ui.ctx().copy_text(url.clone());
                    ui.close_menu();
                }
            });
        }
    });
}
/// The image a Ctrl+V should insert: the clipboard's picture, but only when
/// it holds no text (a table or text clipboard also carries an image
/// representation, and the text must win).
fn clipboard_picture_for_paste(scale: f32, fit: bool) -> Option<Picture> {
    #[cfg(test)]
    let _ = (scale, fit);
    #[cfg(test)]
    return tests::CLIPBOARD_PICTURE.with(|p| p.borrow().clone());
    #[cfg(not(test))]
    {
        let has_text = crate::clipboard_copy::Clipboard::new()
            .and_then(|mut c| c.get_text())
            .is_ok_and(|t| !t.trim().is_empty());
        if has_text {
            return None;
        }
        picture_from_clipboard(scale, fit).ok()
    }
}
/// True on the frame Ctrl+V goes down. egui turns Ctrl+V into a text paste
/// event and drops the key entirely, so an image-only clipboard produces no
/// event at all; read the key state directly instead.
fn paste_chord(ui: &egui::Ui, id: Id) -> bool {
    let key = id.with("paste_chord");
    let down = chord_down() || ui.input(|i| i.modifiers.command && i.key_pressed(Key::V));
    let was = ui.data(|d| d.get_temp::<bool>(key)).unwrap_or(false);
    ui.data_mut(|d| d.insert_temp(key, down));
    // `chord_down` reads the keyboard of the whole system, and egui keeps a
    // text focus per window even while another window is the active one.
    // Only the window the operating system gave the keyboard to may act.
    let focused = ui.input(|i| i.viewport().focused != Some(false));
    down && !was && focused
}
#[cfg(test)]
fn chord_down() -> bool {
    tests::CHORD.with(|c| c.get())
}
#[cfg(all(windows, not(test)))]
fn chord_down() -> bool {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_CONTROL};
    // Reads the current physical key state only; no pointers are involved.
    unsafe {
        (GetAsyncKeyState(i32::from(VK_CONTROL)) as u16 & 0x8000) != 0
            && (GetAsyncKeyState(0x56) as u16 & 0x8000) != 0
    }
}
#[cfg(all(not(windows), not(test)))]
fn chord_down() -> bool {
    false
}
/// Inline size inputs for each picture of a tab, keyed by placeholder.
#[derive(Clone, Copy, PartialEq)]
struct PictureSize {
    key: char,
    width: f32,
    ratio: f32,
    fit: bool,
}
impl PictureSize {
    fn at(&self, wrap: f32) -> Vec2 {
        let room = (wrap - 4.).max(16.);
        let w = if self.fit { room } else { self.width };
        // Quantized so continuous window resizing reuses cached font sizes.
        let w = ((w / 4.).floor() * 4.).max(8.);
        Vec2::new(w, (w * self.ratio).max(4.))
    }
}
fn picture_sizes(tab: &Tab, zoom: f32) -> Vec<PictureSize> {
    tab.blocks
        .iter()
        .filter_map(|b| match b {
            Block::Picture(p) if p.width > 0 && p.height > 0 => Some(PictureSize {
                key: placeholder(p.id),
                width: p.width as f32 * p.scale * zoom,
                ratio: p.height as f32 / p.width as f32,
                fit: p.fit,
            }),
            _ => None,
        })
        .collect()
}
/// Replace each picture placeholder with an invisible full-width space whose
/// advance and row height equal the picture, so text flows around it.
fn inline_pictures(job: &mut LayoutJob, pictures: &[PictureSize], wrap: f32, ratio: f32) {
    if pictures.is_empty() {
        return;
    }
    let mut changed = false;
    for section in &mut job.sections {
        let Some(c) = job.text[section.byte_range.clone()].chars().next() else {
            continue;
        };
        let Some(size) = pictures.iter().find(|p| p.key == c) else {
            continue;
        };
        let s = size.at(wrap);
        section.format.font_id = FontId::new(s.x / ratio, section.format.font_id.family.clone());
        section.format.line_height = Some(s.y + 4.);
        section.format.color = Color32::TRANSPARENT;
        section.format.underline = Stroke::NONE;
        section.format.strikethrough = Stroke::NONE;
        section.format.italics = false;
        section.format.valign = egui::Align::BOTTOM;
        changed = true;
    }
    if changed {
        // Placeholders and U+3000 are both three UTF-8 bytes, so every
        // section's byte range stays valid after the substitution.
        job.text = job
            .text
            .chars()
            .map(|c| {
                if pictures.iter().any(|p| p.key == c) {
                    '\u{3000}'
                } else {
                    c
                }
            })
            .collect();
    }
}
/// Name of the font family that holds only `spacer_font`.
pub const SPACER_FONT: &str = "ohmemojang-spacer";
/// A font with one glyph: U+3000, one em wide and without an outline.
///
/// A picture or a table reserves its place in the text as an invisible
/// U+3000 whose font size makes it as wide as the block. In a normal font
/// that size (hundreds of pixels, a new one for every width while a window
/// is resized) makes egui rasterize its replacement glyph ('◻' or '?') at
/// that size into the font atlas: the atlas grows to tens of megapixels and
/// is uploaded again as a whole, which is the stutter when a memo with a
/// table is maximized. This font has neither character and no outlines, so
/// nothing is rasterized whatever the size.
pub fn spacer_font() -> Vec<u8> {
    fn be16(v: &mut Vec<u8>, x: u16) {
        v.extend_from_slice(&x.to_be_bytes());
    }
    fn be32(v: &mut Vec<u8>, x: u32) {
        v.extend_from_slice(&x.to_be_bytes());
    }
    let mut cmap = vec![];
    for x in [0, 1, 3, 1] {
        be16(&mut cmap, x); // version, one subtable: Windows, Unicode BMP
    }
    be32(&mut cmap, 12);
    // Format 4, two segments: U+3000 -> glyph 1, and the closing U+FFFF.
    for x in [4, 32, 0, 4, 4, 1, 0] {
        be16(&mut cmap, x);
    }
    let delta = 1u16.wrapping_sub(0x3000);
    for x in [0x3000, 0xFFFF, 0, 0x3000, 0xFFFF, delta, 1, 0, 0] {
        be16(&mut cmap, x);
    }
    one_glyph_font(cmap, 1000)
}
/// Characters that only change how the emoji before them looks (variation
/// selectors, the zero-width joiner, skin tones, tag characters). Drawn with
/// no width, so an emoji from Win+. does not show stray boxes after it.
const INVISIBLE: [(u32, u32); 5] = [
    (0x200D, 0x200D),
    (0xFE00, 0xFE0F),
    (0x1F3FB, 0x1F3FF),
    (0xE0020, 0xE007F),
    (0xE0100, 0xE01EF),
];
pub const INVISIBLE_FONT: &str = "invisible_marks";
/// A font whose one glyph, empty and without width, stands for `INVISIBLE`.
pub fn invisible_font() -> Vec<u8> {
    let mut cmap = vec![];
    cmap.extend_from_slice(&0u16.to_be_bytes());
    cmap.extend_from_slice(&1u16.to_be_bytes());
    cmap.extend_from_slice(&3u16.to_be_bytes()); // Windows
    cmap.extend_from_slice(&10u16.to_be_bytes()); // full Unicode
    cmap.extend_from_slice(&12u32.to_be_bytes());
    // Format 12: groups of (first, last, glyph).
    cmap.extend_from_slice(&12u16.to_be_bytes());
    cmap.extend_from_slice(&0u16.to_be_bytes());
    cmap.extend_from_slice(&(16 + 12 * INVISIBLE.len() as u32).to_be_bytes());
    cmap.extend_from_slice(&0u32.to_be_bytes());
    cmap.extend_from_slice(&(INVISIBLE.len() as u32).to_be_bytes());
    for (first, last) in INVISIBLE {
        for x in [first, last, 1] {
            cmap.extend_from_slice(&x.to_be_bytes());
        }
    }
    one_glyph_font(cmap, 0)
}
/// Every family falls back to the invisible marks last.
pub fn add_invisible_font(fonts: &mut egui::FontDefinitions) {
    fonts.font_data.insert(
        INVISIBLE_FONT.into(),
        egui::FontData::from_owned(invisible_font()).into(),
    );
    for entries in fonts.families.values_mut() {
        entries.push(INVISIBLE_FONT.into());
    }
}
/// A minimal font: glyph 0 (.notdef) and glyph 1, both empty, glyph 1
/// `advance` wide (of 1000), mapped by `cmap`.
fn one_glyph_font(cmap: Vec<u8>, advance: u16) -> Vec<u8> {
    fn be16(v: &mut Vec<u8>, x: u16) {
        v.extend_from_slice(&x.to_be_bytes());
    }
    fn be32(v: &mut Vec<u8>, x: u32) {
        v.extend_from_slice(&x.to_be_bytes());
    }
    let mut head = vec![];
    for x in [0x0001_0000, 0x0001_0000, 0, 0x5F0F_3CF5] {
        be32(&mut head, x);
    }
    for x in [0, 1000] {
        be16(&mut head, x); // flags, units per em
    }
    head.extend_from_slice(&[0; 16]); // created, modified
    for x in [0, 0, 1000, 1000, 0, 8, 2, 0, 0] {
        be16(&mut head, x); // bounds, style, smallest size, direction, loca and glyph formats
    }
    let mut hhea = vec![];
    be32(&mut hhea, 0x0001_0000);
    let descent = (-200i16) as u16;
    for x in [800, descent, 0, 1000, 0, 0, 1000, 1, 0, 0, 0, 0, 0, 0, 0, 2] {
        be16(&mut hhea, x);
    }
    let mut hmtx = vec![];
    for x in [0, 0, advance, 0] {
        be16(&mut hmtx, x); // (advance, left bearing) of .notdef and of glyph 1
    }
    let mut maxp = vec![];
    be32(&mut maxp, 0x0000_5000);
    be16(&mut maxp, 2);
    let tables: [(&[u8; 4], Vec<u8>); 5] = [
        (b"cmap", cmap),
        (b"head", head),
        (b"hhea", hhea),
        (b"hmtx", hmtx),
        (b"maxp", maxp),
    ];
    let mut out = vec![];
    be32(&mut out, 0x0001_0000);
    for x in [5, 64, 2, 16] {
        be16(&mut out, x); // table count and its binary-search fields
    }
    let mut offset = 12 + 16 * tables.len();
    for (tag, data) in &tables {
        out.extend_from_slice(*tag);
        be32(&mut out, 0); // checksum (not verified by the reader)
        be32(&mut out, offset as u32);
        be32(&mut out, data.len() as u32);
        offset += data.len().next_multiple_of(4);
    }
    for (_, data) in &tables {
        out.extend_from_slice(data);
        out.resize(out.len().next_multiple_of(4), 0);
    }
    out
}
/// Registers the spacer font as its own family (nothing falls back into it
/// and it falls back to nothing).
pub fn add_spacer_font(fonts: &mut egui::FontDefinitions) {
    fonts.font_data.insert(
        SPACER_FONT.into(),
        egui::FontData::from_owned(spacer_font()).into(),
    );
    fonts.families.insert(
        FontFamily::Name(SPACER_FONT.into()),
        vec![SPACER_FONT.into()],
    );
}
fn spacer_family(ui: &egui::Ui) -> Option<FontFamily> {
    let family = FontFamily::Name(SPACER_FONT.into());
    ui.fonts(|f| f.families().contains(&family))
        .then_some(family)
}
/// Moves the invisible spaces `inline_pictures` and `inline_tables` made
/// into the spacer font. They are told from a U+3000 the user typed by
/// their style: transparent, with a row height of their own.
fn set_spacer_family(job: &mut LayoutJob, family: &FontFamily) {
    for section in &mut job.sections {
        if section.format.color == Color32::TRANSPARENT
            && section.format.line_height.is_some()
            && job.text[section.byte_range.clone()].starts_with('\u{3000}')
        {
            section.format.font_id.family = family.clone();
        }
    }
}
/// Where the picture whose placeholder is character `idx` is drawn: on the
/// bottom of its own row. A soft-wrapped picture starts a new row; without
/// `prefer_next_row` its position would be the end of the row above, which
/// drew the picture over the preceding text.
fn picture_rect(galley: &egui::Galley, idx: usize, s: Vec2) -> egui::Rect {
    let caret = galley.pos_from_ccursor(CCursor {
        index: idx,
        prefer_next_row: true,
    });
    egui::Rect::from_min_size(egui::pos2(caret.min.x, caret.max.y - s.y - 2.), s)
}
/// Where the selected picture's floating size/copy/delete toolbar goes.
/// Floats above the picture by default; when that would clip above the
/// visible area (the picture sits near the top of the scroll view), it
/// drops below the picture instead so it stays reachable.
fn picture_toolbar_pos(rect: egui::Rect, clip_top: f32) -> egui::Pos2 {
    let above = rect.left_top() - Vec2::new(0., 34.);
    if above.y >= clip_top {
        above
    } else {
        rect.left_bottom() + Vec2::new(0., 2.)
    }
}
fn paint_pictures(
    ui: &mut egui::Ui,
    doc: &mut Document,
    id: Id,
    output: &egui::text_edit::TextEditOutput,
    pictures: &[PictureSize],
) {
    if pictures.is_empty() {
        return;
    }
    let wrap = output.galley.job.wrap.max_width;
    let origin = output.galley_pos.to_vec2();
    let body: Vec<char> = doc.tabs[doc.active].body.chars().collect();
    let painter = ui.painter().with_clip_rect(output.text_clip_rect);
    let mut clicked_any = false;
    for (idx, c) in body.iter().enumerate() {
        let Some(size) = pictures.iter().find(|p| p.key == *c) else {
            continue;
        };
        let pid = picture_id(*c).unwrap_or(0);
        let rect = picture_rect(&output.galley, idx, size.at(wrap)).translate(origin);
        let Some(Block::Picture(p)) = doc.tabs[doc.active]
            .blocks
            .iter_mut()
            .find(|b| matches!(b, Block::Picture(p) if p.id == pid))
        else {
            continue;
        };
        // Pictures are read from disk only once they scroll into view.
        if p.texture.is_none() && ui.is_rect_visible(rect) {
            if let Some(image) = p.color_image() {
                p.texture = Some(
                    ui.ctx().load_texture(
                        id.with(("inline_picture", doc.active, pid))
                            .short_debug_format(),
                        image,
                        egui::TextureOptions::LINEAR,
                    ),
                );
            }
        }
        if let Some(t) = &p.texture {
            p.texture_used = ui.input(|i| i.time);
            painter.image(
                t.id(),
                rect,
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1., 1.)),
                Color32::WHITE,
            );
        }
        if !output.text_clip_rect.intersects(rect) {
            continue;
        }
        let r = ui
            .interact(
                rect.intersect(output.text_clip_rect),
                id.with(("picture_hit", doc.active, pid, idx)),
                egui::Sense::click(),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        if r.clicked() {
            doc.selected_picture = Some(pid);
            doc.picture_tool_at = None;
            clicked_any = true;
        }
        // A right click opens the toolbar right where the mouse is.
        if r.secondary_clicked() {
            doc.selected_picture = Some(pid);
            doc.picture_tool_at = r
                .interact_pointer_pos()
                .or_else(|| ui.input(|i| i.pointer.latest_pos()));
            clicked_any = true;
        }
        if doc.selected_picture != Some(pid) {
            continue;
        }
        painter.rect_stroke(
            rect.expand(1.),
            2.,
            Stroke::new(2.0_f32, crate::ui::ACCENT),
            egui::StrokeKind::Outside,
        );
        let mut close = false;
        let mut delete = false;
        // The text layout may extend beyond the viewport for oversized images.
        // Keep controls in the visible viewport, independently of that layout.
        let toolbar_bounds = output
            .text_clip_rect
            .intersect(ui.ctx().screen_rect())
            .shrink(4.);
        let toolbar_width = toolbar_bounds.width().clamp(120., 360.);
        let anchor = doc
            .picture_tool_at
            .unwrap_or_else(|| picture_toolbar_pos(rect, output.text_clip_rect.top()));
        let toolbar_pos = egui::pos2(
            anchor.x.clamp(
                toolbar_bounds.left(),
                (toolbar_bounds.right() - toolbar_width).max(toolbar_bounds.left()),
            ),
            anchor.y.clamp(
                toolbar_bounds.top(),
                (toolbar_bounds.bottom() - 76.).max(toolbar_bounds.top()),
            ),
        );
        egui::Area::new(id.with(("picture_tools", pid)))
            .order(egui::Order::Foreground)
            .fixed_pos(toolbar_pos)
            .constrain_to(toolbar_bounds)
            .show(ui.ctx(), |ui| {
                ui.set_width(toolbar_width);
                egui::Frame::popup(ui.style())
                    .inner_margin(egui::Margin::same(4))
                    .show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing.x = 2.;
                            for (value, label) in
                                [(0.25, "25%"), (0.5, "50%"), (0.75, "75%"), (1., "100%")]
                            {
                                if ui
                                    .selectable_label(
                                        !p.fit && (p.scale - value).abs() < 0.01,
                                        label,
                                    )
                                    .clicked()
                                {
                                    p.scale = value;
                                    p.fit = false;
                                }
                            }
                            if ui.selectable_label(p.fit, "너비 맞춤").clicked() {
                                p.fit = true;
                            }
                            ui.separator();
                            if ui.button("복사").clicked() {
                                let copied =
                                    crate::clipboard_copy::Clipboard::new().and_then(|mut c| {
                                        c.set_image(arboard::ImageData {
                                            width: p.width,
                                            height: p.height,
                                            bytes: p.rgba(),
                                        })
                                    });
                                // Said either way: nothing else shows it.
                                doc.status = match copied {
                                    Ok(()) => "이미지를 복사했습니다".into(),
                                    Err(e) => format!("이미지를 복사하지 못했습니다: {e}"),
                                };
                                close = true;
                            }
                            if ui
                                .button("삭제")
                                .on_hover_text("Ctrl+Z로 되돌릴 수 있습니다")
                                .clicked()
                            {
                                delete = true;
                                close = true;
                            }
                        });
                    });
            });
        if delete {
            let tab = &mut doc.tabs[doc.active];
            let key = placeholder(pid);
            tab.body = tab.body.chars().filter(|c| *c != key).collect();
        }
        if close {
            doc.selected_picture = None;
            doc.picture_tool_at = None;
            return;
        }
    }
    if !clicked_any
        && doc.selected_picture.is_some()
        && ui.input(|i| i.pointer.any_pressed())
        && output.response.hovered()
    {
        doc.selected_picture = None;
        doc.picture_tool_at = None;
    }
}
/// A positioned table's rough rendered height below its grid: the
/// row/column/merge toolbar plus the surrounding spacing. Not exact (the
/// toolbar's real height depends on the active theme's button metrics),
/// but close enough that reserving this much room in the text rarely
/// leaves a visible gap or overlap.
const TABLE_TOOLBAR_H: f32 = 30.;
#[derive(Clone, Copy, PartialEq)]
struct TableSize {
    key: char,
    /// The table's full, unclamped size: its grid (every column width
    /// summed, every row's height) plus the toolbar. May be wider than
    /// any wrap width `at` is asked for; `table_widget`'s own horizontal
    /// scroll area is what actually keeps it from overflowing.
    content: Vec2,
}
impl TableSize {
    fn at(&self, wrap: f32) -> Vec2 {
        let room = (wrap - 4.).max(16.);
        Vec2::new(self.content.x.min(room), self.content.y)
    }
}
#[cfg(test)]
fn table_sizes(tab: &Tab, zoom: f32) -> Vec<TableSize> {
    table_sizes_with(tab, zoom, &|_| None)
}
/// `measured(table id)` is the height the table was last drawn with.
fn table_sizes_with(tab: &Tab, zoom: f32, measured: &dyn Fn(u32) -> Option<f32>) -> Vec<TableSize> {
    tab.blocks
        .iter()
        .filter_map(|b| match b {
            Block::Table(t) if t.positioned => Some(TableSize {
                key: placeholder(t.id),
                content: Vec2::new(
                    t.col_widths.iter().sum::<f32>() * zoom + t.col_widths.len() as f32 + 2.,
                    measured(t.id).unwrap_or({
                        (t.row_height * zoom + 1.) * t.cells.len() as f32 + TABLE_TOOLBAR_H
                    }),
                ),
            }),
            _ => None,
        })
        .collect()
}
/// Replace each table placeholder with an invisible full-width space sized
/// to the table (clamped to the available wrap width — see `TableSize::at`
/// — the same way a picture is), so text flows around it. Mirrors
/// `inline_pictures`; the two are separate functions because a table and a
/// picture size themselves differently (a table never shrinks below its
/// natural width, it scrolls instead), not because the substitution itself
/// differs.
fn inline_tables(job: &mut LayoutJob, tables: &[TableSize], wrap: f32, ratio: f32) {
    if tables.is_empty() {
        return;
    }
    let mut changed = false;
    for section in &mut job.sections {
        let Some(c) = job.text[section.byte_range.clone()].chars().next() else {
            continue;
        };
        let Some(size) = tables.iter().find(|t| t.key == c) else {
            continue;
        };
        let s = size.at(wrap);
        section.format.font_id = FontId::new(s.x / ratio, section.format.font_id.family.clone());
        section.format.line_height = Some(s.y + 4.);
        section.format.color = Color32::TRANSPARENT;
        section.format.underline = Stroke::NONE;
        section.format.strikethrough = Stroke::NONE;
        section.format.italics = false;
        section.format.valign = egui::Align::BOTTOM;
        changed = true;
    }
    if changed {
        job.text = job
            .text
            .chars()
            .map(|c| {
                if tables.iter().any(|t| t.key == c) {
                    '\u{3000}'
                } else {
                    c
                }
            })
            .collect();
    }
}
/// Table selection and editing state of the one table being worked in.
/// Cells are plain text until one is edited; only that cell is a TextEdit.
#[derive(Clone, Debug, PartialEq)]
pub struct TSel {
    tab: usize,
    bi: usize,
    /// Logical (owner) cells: where the selection started and where it is now.
    anchor: (usize, usize),
    head: (usize, usize),
    editing: bool,
    /// The mouse button is held after pressing a cell.
    drag: bool,
    /// The cell's text when editing began: Esc and undo return to it.
    orig: String,
    /// Frames since editing began (focus loss only counts after the first).
    age: u8,
}
impl TSel {
    fn at(tab: usize, bi: usize, cell: (usize, usize)) -> Self {
        Self {
            tab,
            bi,
            anchor: cell,
            head: cell,
            editing: false,
            drag: false,
            orig: String::new(),
            age: 0,
        }
    }
    fn range(&self, t: &Table) -> crate::table_edit::Rng {
        crate::table_edit::expand(t, crate::table_edit::normalize(self.anchor, self.head))
    }
}
#[cfg(test)]
thread_local! {
    /// (visible body, caret) as of the last caret move, for tests.
    static LAST_CARET: std::cell::Cell<Option<(egui::Rect, egui::Rect)>> = const { std::cell::Cell::new(None) };
}
#[cfg(test)]
thread_local! {
    /// Bottom edge (screen y) of each row of the last drawn table, for tests.
    static ROW_EDGES: std::cell::RefCell<Vec<f32>> = const { std::cell::RefCell::new(Vec::new()) };
}
#[cfg(test)]
thread_local! {
    /// Where the last drawn table was, for tests (which have no window to ask).
    static LAST_TABLE_RECT: std::cell::Cell<Option<egui::Rect>> = const { std::cell::Cell::new(None) };
}
/// What a table needs from its document while it is drawn.
struct TableCtx<'a> {
    tsel: &'a mut Option<TSel>,
    undo: &'a mut UndoStack,
    selected: &'a mut Option<(usize, usize, usize)>,
    status: &'a mut String,
    tab: usize,
    bi: usize,
    ime: bool,
    now: f64,
}
#[derive(Clone, Copy, PartialEq)]
enum TAct {
    Copy,
    Cut,
    Paste,
    Clear,
    RowAbove,
    RowBelow,
    ColLeft,
    ColRight,
    DelRows,
    DelCols,
    Merge,
    Unmerge,
    EqualCols,
    EqualRows,
    AddRowEnd,
    AddColEnd,
    CopyAll,
    Remove,
    Align(CellAlign),
}
/// A table toolbar button. It claims drags as well as clicks: the body's
/// text editor is under the toolbar, and a press that only clicks-through
/// would give it the focus and drop the table's selection.
fn toolbar_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(text)
            .small()
            .sense(Sense::click_and_drag()),
    )
}
/// A small button with four text lines drawn left, centered or right.
fn align_button(ui: &mut egui::Ui, align: CellAlign, active: bool, tip: &str) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(22., 20.), Sense::click_and_drag());
    let visuals = ui.style().interact_selectable(&resp, active);
    ui.painter().rect_filled(
        rect,
        4.,
        if active {
            visuals.bg_fill
        } else if resp.hovered() {
            ui.visuals().widgets.hovered.bg_fill
        } else {
            Color32::TRANSPARENT
        },
    );
    let color = ui.visuals().text_color();
    let inner = rect.shrink2(Vec2::new(5., 4.));
    for (i, frac) in [1.0_f32, 0.6, 1.0, 0.6].into_iter().enumerate() {
        let w = inner.width() * frac;
        let x0 = match align {
            CellAlign::Center => inner.left() + (inner.width() - w) / 2.,
            CellAlign::Right => inner.right() - w,
            _ => inner.left(),
        };
        let y = inner.top() + i as f32 * (inner.height() / 3.);
        ui.painter().line_segment(
            [egui::pos2(x0, y), egui::pos2(x0 + w, y)],
            Stroke::new(1.2_f32, color),
        );
    }
    #[cfg(test)]
    ALIGN_BUTTONS.with(|b| b.borrow_mut().push(rect));
    resp.on_hover_text(tip)
}
#[cfg(test)]
thread_local! {
    /// Rects of the alignment buttons drawn in the last frame, for tests.
    static ALIGN_BUTTONS: std::cell::RefCell<Vec<egui::Rect>> = const { std::cell::RefCell::new(Vec::new()) };
}
/// Paints each positioned table at its placeholder, the same way
/// `paint_pictures` paints an image there, except what's painted is a live
/// `table_widget` rather than a static texture: it is drawn in a floating
/// `egui::Area` pinned to the reserved rect so its cells, buttons and
/// context menus stay interactive.
fn paint_tables(
    ui: &mut egui::Ui,
    doc: &mut Document,
    id: Id,
    output: &egui::text_edit::TextEditOutput,
    tables: &[TableSize],
    font: &FontId,
    bold_font: &FontId,
) {
    if tables.is_empty() {
        return;
    }
    let wrap = output.galley.job.wrap.max_width;
    let origin = output.galley_pos.to_vec2();
    let tab = doc.active;
    let body: Vec<char> = doc.tabs[tab].body.chars().collect();
    let zoom = doc.zoom;
    let (ime, now) = (doc.ime, ui.input(|i| i.time));
    let mut selected = doc.selected_cell;
    let mut tsel = doc.tsel.take();
    let mut undo = std::mem::take(&mut doc.undo);
    let mut status = String::new();
    let mut delete_key = None;
    for (idx, c) in body.iter().enumerate() {
        let Some(size) = tables.iter().find(|t| t.key == *c) else {
            continue;
        };
        let tid = picture_id(*c).unwrap_or(0);
        let Some(bi) = doc.tabs[tab]
            .blocks
            .iter()
            .position(|b| matches!(b, Block::Table(t) if t.id == tid))
        else {
            continue;
        };
        let rect = picture_rect(&output.galley, idx, size.at(wrap)).translate(origin);
        if !output.text_clip_rect.intersects(rect) {
            continue;
        }
        let mut removed = false;
        // A child of the body's scroll content, not a floating window: the
        // scroll area clips it (it cannot cover the title or the tabs) and
        // its clicks stay inside the visible body.
        let room = Vec2::new(rect.width().max(1.), rect.height() + 600.);
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(id.with(("inline_table", tab, tid)))
                .max_rect(egui::Rect::from_min_size(rect.left_top(), room)),
        );
        child.set_width(rect.width().max(1.));
        if let Block::Table(t) = &mut doc.tabs[tab].blocks[bi] {
            let mut cx = TableCtx {
                tsel: &mut tsel,
                undo: &mut undo,
                selected: &mut selected,
                status: &mut status,
                tab,
                bi,
                ime,
                now,
            };
            removed = table_widget(&mut child, id, t, font, bold_font, zoom, &mut cx);
        }
        #[cfg(test)]
        LAST_TABLE_RECT.with(|r| r.set(Some(child.min_rect())));
        // The text reserves the table's real height from the next frame on.
        let actual = child.min_rect().height();
        let key = id.with(("table_h", tab, tid));
        let known = ui.data(|d| d.get_temp::<f32>(key));
        if known.is_none_or(|k| (k - actual).abs() > 0.5) {
            ui.data_mut(|d| d.insert_temp(key, actual));
            ui.ctx().request_repaint();
        }
        if removed {
            delete_key = Some(*c);
        }
    }
    doc.selected_cell = selected;
    doc.tsel = tsel;
    doc.undo = undo;
    if !status.is_empty() {
        doc.status = status;
    }
    if let Some(key) = delete_key {
        let tid = picture_id(key).unwrap_or(0);
        let t = &mut doc.tabs[tab];
        if let Some(i) = t
            .blocks
            .iter()
            .position(|b| matches!(b, Block::Table(x) if x.id == tid))
        {
            if let Block::Table(x) = &t.blocks[i] {
                doc.undo.push(
                    "표 삭제",
                    UndoKind::TableDeleted {
                        tab,
                        bi: i,
                        table: x.clone(),
                        text: t.body.clone(),
                        spans: t.spans.clone(),
                    },
                    now,
                );
            }
        }
        if let Some(index) = t
            .blocks
            .iter()
            .position(|b| matches!(b, Block::Table(x) if x.id == tid))
        {
            t.remove_blocks(&[index]);
        }
        doc.selected_cell = None;
        doc.tsel = None;
    }
}
fn show_blocks(ui: &mut egui::Ui, doc: &mut Document, id: Id) {
    let font = font_id(ui, doc.font_size * doc.zoom, &doc.font_name, false);
    let bold_font = font_id(ui, doc.font_size * doc.zoom, &doc.font_name, true);
    let tab = doc.active;
    let zoom = doc.zoom;
    let (ime, now) = (doc.ime, ui.input(|i| i.time));
    // The common case, a memo without tables, does nothing here.
    if !doc.tabs[tab]
        .blocks
        .iter()
        .any(|b| matches!(b, Block::Table(t) if !t.positioned))
    {
        return;
    }
    let mut remove = None;
    let mut selected = doc.selected_cell;
    let mut tsel = doc.tsel.take();
    let mut undo = std::mem::take(&mut doc.undo);
    let mut status = String::new();
    for (bi, block) in doc.tabs[tab].blocks.iter_mut().enumerate() {
        // A positioned table renders inline at its placeholder (see
        // `paint_tables`); only a legacy, never-positioned one still
        // belongs in this always-below-the-text list.
        if let Block::Table(t) = block {
            if !t.positioned {
                ui.add_space(8.);
                let mut cx = TableCtx {
                    tsel: &mut tsel,
                    undo: &mut undo,
                    selected: &mut selected,
                    status: &mut status,
                    tab,
                    bi,
                    ime,
                    now,
                };
                if table_widget(ui, id, t, &font, &bold_font, zoom, &mut cx) {
                    remove = Some(bi);
                }
            }
        }
    }
    doc.selected_cell = selected;
    doc.tsel = tsel;
    doc.undo = undo;
    if !status.is_empty() {
        doc.status = status;
    }
    if let Some(i) = remove {
        if let Block::Table(x) = doc.tabs[tab].blocks.remove(i) {
            let t = &doc.tabs[tab];
            let entry = UndoKind::TableDeleted {
                tab,
                bi: i,
                table: x,
                text: t.body.clone(),
                spans: t.spans.clone(),
            };
            doc.undo.push("표 삭제", entry, now);
        }
        doc.selected_cell = None;
        doc.tsel = None;
    }
}
/// Where each row of a drawn table starts and how high it is.
struct RowGeo {
    /// Top of each row, from the top of the grid.
    y: Vec<f32>,
    h: Vec<f32>,
    total: f32,
}
const ROW_SPACING: f32 = 1.;
/// Row heights: a row the user resized keeps that height; any other is as
/// high as its tallest cell needs (a cell with line breaks or long wrapped
/// text), never less than the table's row height. A merged cell that needs
/// more than its rows give adds the difference to its last row.
#[allow(clippy::too_many_arguments)]
fn row_geometry(
    ui: &egui::Ui,
    t: &Table,
    font: &FontId,
    bold_font: &FontId,
    widths: &[f32],
    zoom: f32,
) -> RowGeo {
    let nr = t.cells.len();
    let min_h = t.row_height * zoom;
    let mut need = vec![min_h; nr];
    let mut spans: Vec<(usize, usize, f32)> = vec![];
    let color = ui.visuals().text_color();
    for (r, row) in t.cells.iter().enumerate() {
        for (c, cell) in row.iter().enumerate() {
            if cell.covered || cell.text.is_empty() {
                continue;
            }
            let cs = cell.col_span.max(1).min(row.len() - c);
            let w: f32 = widths[c..c + cs].iter().sum::<f32>() + ROW_SPACING * (cs as f32 - 1.);
            let avail = (w - 12.).max(8.);
            // Only a cell that can need a second line is laid out.
            let may_wrap = cell.text.contains('\n')
                || cell.text.chars().count() as f32 * font.size * 0.5 > avail;
            if !may_wrap {
                continue;
            }
            let mut job = layout(
                &cell.text,
                &[],
                &cell.format,
                font.clone(),
                bold_font.clone(),
                color,
            );
            job.wrap.max_width = avail;
            let h = ui.fonts(|f| f.layout_job(job)).size().y + 12.;
            let rs = cell.row_span.max(1).min(nr - r);
            if rs == 1 {
                need[r] = need[r].max(h);
            } else {
                spans.push((r, rs, h));
            }
        }
    }
    let mut h: Vec<f32> = (0..nr)
        .map(|r| match t.row_manual.get(r) {
            Some(m) if *m > 0. => m * zoom,
            _ => need[r],
        })
        .collect();
    for (r, rs, need_h) in spans {
        let have: f32 = h[r..r + rs].iter().sum::<f32>() + ROW_SPACING * (rs as f32 - 1.);
        let last = r + rs - 1;
        if need_h > have && t.row_manual.get(last).is_none_or(|m| *m <= 0.) {
            h[last] += need_h - have;
        }
    }
    let mut y = Vec::with_capacity(nr);
    let mut top = 0.;
    for &hh in &h {
        y.push(top);
        top += hh + ROW_SPACING;
    }
    RowGeo { y, h, total: top }
}
fn cell_box(
    origin: egui::Pos2,
    col_x: &[f32],
    widths: &[f32],
    geo: &RowGeo,
    rg: crate::table_edit::Rng,
) -> Rect {
    Rect::from_min_max(
        origin + Vec2::new(col_x[rg.1], geo.y[rg.0]),
        origin + Vec2::new(col_x[rg.3] + widths[rg.3], geo.y[rg.2] + geo.h[rg.2]),
    )
}
fn grid_cell(
    t: &Table,
    origin: egui::Pos2,
    col_x: &[f32],
    geo: &RowGeo,
    p: egui::Pos2,
) -> Option<(usize, usize)> {
    let (x, y) = (p.x - origin.x, p.y - origin.y);
    if x < 0. || y < 0. {
        return None;
    }
    let r = geo.y.partition_point(|&top| top <= y).checked_sub(1)?;
    let c = col_x.iter().rposition(|&cx| cx <= x)?;
    (r < t.cells.len() && c < t.cells[0].len()).then(|| crate::table_edit::owner_of(t, r, c))
}
/// Ends editing; the change becomes one undo step.
fn tbl_commit(t: &mut Table, s: &mut TSel, cx: &mut TableCtx) {
    if !s.editing {
        return;
    }
    s.editing = false;
    let (r, c) = s.head;
    let orig = std::mem::take(&mut s.orig);
    if t.cells[r][c].text != orig {
        let mut snap = t.clone();
        snap.cells[r][c].text = orig;
        cx.undo.push(
            "셀 편집",
            UndoKind::Table {
                tab: cx.tab,
                bi: cx.bi,
                table: snap,
            },
            cx.now,
        );
    }
}
fn tbl_cancel(t: &mut Table, s: &mut TSel) {
    if s.editing {
        let (r, c) = s.head;
        t.cells[r][c].text = std::mem::take(&mut s.orig);
        s.editing = false;
    }
}
fn tbl_begin_edit(t: &mut Table, s: &mut TSel, replace: bool, ctx: &egui::Context, edit_id: Id) {
    let (r, c) = s.head;
    s.editing = true;
    s.age = 0;
    s.orig = t.cells[r][c].text.clone();
    if replace {
        t.cells[r][c].text.clear();
    }
    let mut st = egui::TextEdit::load_state(ctx, edit_id).unwrap_or_default();
    let end = t.cells[r][c].text.chars().count();
    st.cursor
        .set_char_range(Some(CCursorRange::one(CCursor::new(end))));
    st.store(ctx, edit_id);
    ctx.memory_mut(|m| m.request_focus(edit_id));
}
fn copy_range(t: &Table, rng: crate::table_edit::Rng, status: &mut String) {
    let html = crate::table_edit::range_html(t, rng);
    let tsv = crate::table_edit::range_tsv(t, rng);
    if let Err(e) =
        crate::clipboard_copy::Clipboard::new().and_then(|mut c| c.set_html(html, Some(tsv)))
    {
        *status = format!("복사 실패: {e}");
    } else {
        *status = "표 셀을 복사했습니다 (Excel 형식)".into();
    }
}
/// What a paste puts into a table: the clipboard's `<table>` markup when
/// there is one, else the text split with Excel's quoting.
/// The `<table>` on the clipboard. The whole CF_HTML is read (its style
/// block holds the fill, text color and alignment Excel puts in classes);
/// the fragment arboard hands over has no style block.
fn clipboard_html_table() -> Option<Table> {
    clipboard_html_source().and_then(|h| Table::from_html(&h))
}
/// The clipboard's HTML: the whole CF_HTML when it can be read, else the
/// fragment arboard hands over.
fn clipboard_html_source() -> Option<String> {
    #[cfg(test)]
    return tests::CLIPBOARD_HTML.with(|h| h.borrow().clone());
    #[cfg(not(test))]
    crate::clipboard_html::read().or_else(|| {
        crate::clipboard_copy::Clipboard::new()
            .ok()
            .and_then(|mut c| c.get().html().ok())
    })
}
/// '표 붙여넣기' in 설정 › 일반: on, a copied table is pasted as tab
/// separated text instead of a table.
#[cfg(not(test))]
static PASTE_TABLES_AS_TEXT: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
#[cfg(test)]
thread_local! {
    // Tests run in parallel: each keeps its own choice.
    static PASTE_TABLES_AS_TEXT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
pub fn set_paste_tables_as_text(on: bool) {
    #[cfg(not(test))]
    PASTE_TABLES_AS_TEXT.store(on, std::sync::atomic::Ordering::Relaxed);
    #[cfg(test)]
    PASTE_TABLES_AS_TEXT.with(|c| c.set(on));
}
fn paste_tables_as_text() -> bool {
    #[cfg(not(test))]
    return PASTE_TABLES_AS_TEXT.load(std::sync::atomic::Ordering::Relaxed);
    #[cfg(test)]
    PASTE_TABLES_AS_TEXT.with(|c| c.get())
}
/// Ctrl+V right after Korean typing can arrive in the same frame as the
/// IME's commit. The paste then waits a frame (three at most), so the commit
/// lands first and a copied table still becomes a table.
fn hold_paste_during_ime(ui: &mut egui::Ui, key: Id) {
    let pass = ui.ctx().cumulative_pass_nr();
    let mut tries = 0;
    if let Some((text, n, at)) = ui.data_mut(|d| d.remove_temp::<(String, u8, u64)>(key)) {
        // A paste left waiting while the memo lost focus is dropped.
        if pass.saturating_sub(at) <= 5 {
            ui.input_mut(|i| i.events.push(egui::Event::Paste(text)));
            tries = n;
        }
    }
    let ime = ui.input(|i| {
        i.events.iter().any(|e| {
            matches!(
                e,
                egui::Event::Ime(egui::ImeEvent::Preedit(_) | egui::ImeEvent::Commit(_))
            )
        })
    });
    if !ime || tries >= 3 {
        return;
    }
    let taken = ui.input_mut(|i| {
        let at = i
            .events
            .iter()
            .position(|e| matches!(e, egui::Event::Paste(_)))?;
        match i.events.remove(at) {
            egui::Event::Paste(t) => Some(t),
            _ => None,
        }
    });
    if let Some(text) = taken {
        ui.data_mut(|d| d.insert_temp(key, (text, tries + 1, pass)));
        ui.ctx().request_repaint();
    }
}
/// Puts pasted text and tables in place of `range`, in order, and returns
/// the cursor after them. Each table is one undo step; undoing the first
/// brings back the text as it was before the paste.
fn insert_pieces(
    doc: &mut Document,
    range: Range<usize>,
    pieces: Vec<crate::table_edit::Piece>,
    before: &str,
    spans_before: &[Span],
    now: f64,
) -> usize {
    use crate::table_edit::Piece;
    let active = doc.active;
    let mut at = range.start;
    replace_chars(&mut doc.tabs[active].body, range, "");
    let (mut first, mut after_table) = (true, false);
    for piece in pieces {
        match piece {
            Piece::Text(t) => {
                replace_chars(&mut doc.tabs[active].body, at..at, &t);
                at += t.chars().count();
                after_table = false;
            }
            Piece::Table(table) => {
                if after_table {
                    replace_chars(&mut doc.tabs[active].body, at..at, "\n");
                    at += 1;
                }
                let text = if first {
                    before.to_owned()
                } else {
                    doc.tabs[active].body.clone()
                };
                at = doc.tabs[active].insert_table(at..at, table);
                if let Some(Block::Table(t)) = doc.tabs[active].blocks.last() {
                    let entry = UndoKind::TableInserted {
                        tab: active,
                        id: t.id,
                        text,
                        spans: spans_before.to_vec(),
                    };
                    doc.undo.push("표 붙여넣기", entry, now);
                }
                first = false;
                after_table = true;
            }
        }
    }
    at
}
fn clipboard_table(event_text: Option<&str>) -> Option<Table> {
    let html = clipboard_html_table();
    if html.is_some() {
        return html;
    }
    let text = event_text.map(str::to_owned).or_else(|| {
        crate::clipboard_copy::Clipboard::new()
            .ok()
            .and_then(|mut c| c.get_text().ok())
    })?;
    (!text.is_empty() && text.len() < 4 * 1024 * 1024)
        .then(|| crate::table_edit::table_from_grid(crate::table_edit::parse_tsv(&text)))
}
/// Renders one table. Cells are painted as text; a click selects, dragging
/// selects a range, double-click / typing / F2 edits (only then is a
/// TextEdit created, for that one cell). A merged cell is drawn at the size
/// of every column/row it covers. Returns true when "표 삭제" was chosen.
#[allow(clippy::too_many_arguments, clippy::needless_range_loop)]
fn table_widget(
    ui: &mut egui::Ui,
    id: Id,
    t: &mut Table,
    font: &FontId,
    bold_font: &FontId,
    zoom: f32,
    cx: &mut TableCtx,
) -> bool {
    use crate::table_edit as te;
    let (tab, bi) = (cx.tab, cx.bi);
    let mut sel: Option<TSel> = cx.tsel.take_if(|s| s.tab == tab && s.bi == bi);
    let (nr, nc) = (te::rows(t), te::cols(t));
    if let Some(s) = &mut sel {
        s.anchor = te::owner_of(t, s.anchor.0.min(nr - 1), s.anchor.1.min(nc - 1));
        s.head = te::owner_of(t, s.head.0.min(nr - 1), s.head.1.min(nc - 1));
    }
    let spacing = ROW_SPACING;
    let widths: Vec<f32> = t.col_widths.iter().map(|w| w * zoom).collect();
    let col_x: Vec<f32> = {
        let mut acc = 0.;
        widths
            .iter()
            .map(|w| {
                let x = acc;
                acc += w + spacing;
                x
            })
            .collect()
    };
    t.fit_rows();
    let geo = row_geometry(ui, t, font, bold_font, &widths, zoom);
    let total_w = widths.iter().sum::<f32>() + spacing * widths.len() as f32;
    let total_h = geo.total;
    let gid = id.with(("table_grid", tab, bi));
    let edit_id = |r: usize, c: usize| id.with(("table_cell", tab, bi, r, c));
    let mut act: Option<TAct> = None;
    let accent = crate::ui::ACCENT;
    egui::ScrollArea::horizontal()
        .id_salt(id.with(("table_scroll", tab, bi)))
        .drag_to_scroll(false)
        .show(ui, |ui| {
            let (rect, _) =
                ui.allocate_exact_size(Vec2::new(total_w.max(1.), total_h.max(1.)), Sense::hover());
            let resp = ui.interact(rect, gid, Sense::click_and_drag());
            let origin = rect.min;
            let ctx = ui.ctx().clone();
            let layer = ui.layer_id();
            // Row borders: drag to set a row's height (kept as set, whatever the
            // content), double-click to let it follow its content again.
            let mut edge_pressed = false;
            #[cfg(test)]
            ROW_EDGES.with(|e| {
                *e.borrow_mut() = (0..nr).map(|r| origin.y + geo.y[r] + geo.h[r]).collect()
            });
            let row_key = gid.with("row_resize");
            type RowResize = (usize, f32, f32, std::sync::Arc<Table>);
            let pointer = ui.input(|i| i.pointer.clone());
            for r in 0..nr {
                let edge = origin.y + geo.y[r] + geo.h[r];
                let handle = Rect::from_min_max(
                    egui::pos2(rect.left(), edge - 2.5),
                    egui::pos2(rect.right(), edge + 3.5),
                )
                .intersect(ui.clip_rect());
                let hit_pos = if pointer.primary_pressed() {
                    pointer.press_origin()
                } else {
                    pointer.interact_pos()
                };
                let over = hit_pos
                    .is_some_and(|p| handle.contains(p) && ctx.layer_id_at(p) == Some(layer));
                let response = ui
                    .interact(handle, gid.with(("row_edge", r)), Sense::click())
                    .on_hover_cursor(egui::CursorIcon::ResizeVertical)
                    .on_hover_text("드래그해서 행 높이 조절 · 더블클릭으로 내용에 맞춤");
                if over && pointer.primary_pressed() {
                    edge_pressed = true;
                    if let Some(s) = sel.as_mut() {
                        tbl_commit(t, s, cx);
                    }
                    ctx.data_mut(|d| {
                        d.insert_temp(
                            row_key,
                            (
                                r,
                                pointer.press_origin().unwrap().y,
                                geo.h[r] / zoom,
                                std::sync::Arc::new(t.clone()),
                            ),
                        )
                    });
                    sel = Some(TSel::at(tab, bi, (r, 0)));
                    ctx.memory_mut(|m| m.request_focus(gid));
                }
                if response.double_clicked() && t.row_manual[r] > 0. {
                    ctx.data_mut(|d| d.remove::<RowResize>(row_key));
                    let before = t.clone();
                    t.row_manual[r] = 0.;
                    cx.undo.push(
                        "행 높이",
                        UndoKind::Table {
                            tab,
                            bi,
                            table: before,
                        },
                        cx.now,
                    );
                }
            }
            if let Some((r, start, height, before)) = ctx.data(|d| d.get_temp::<RowResize>(row_key))
            {
                if let Some(pos) = pointer.interact_pos() {
                    if pos.y != start {
                        t.row_manual[r] = (height + (pos.y - start) / zoom).clamp(14., 800.);
                    }
                }
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
                if !pointer.primary_down() {
                    ctx.data_mut(|d| d.remove::<RowResize>(row_key));
                    if before.row_manual != t.row_manual {
                        cx.undo.push(
                            "행 높이",
                            UndoKind::Table {
                                tab,
                                bi,
                                table: (*before).clone(),
                            },
                            cx.now,
                        );
                    }
                }
            }
            let resize_key = gid.with("column_resize");
            type ColumnResize = (usize, f32, std::sync::Arc<Table>);
            let pointer = ui.input(|i| i.pointer.clone());
            for c in 0..nc {
                let edge = origin.x + col_x[c] + widths[c];
                let handle = Rect::from_min_max(
                    egui::pos2(edge - 3., rect.top()),
                    egui::pos2(edge + 3., rect.bottom()),
                )
                .intersect(ui.clip_rect());
                let hit_pos = if pointer.primary_pressed() {
                    pointer.press_origin()
                } else {
                    pointer.interact_pos()
                };
                let over = hit_pos
                    .is_some_and(|p| handle.contains(p) && ctx.layer_id_at(p) == Some(layer));
                if over {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                }
                if over && pointer.primary_pressed() {
                    edge_pressed = true;
                    if let Some(s) = sel.as_mut() {
                        tbl_commit(t, s, cx);
                    }
                    let start = pointer.press_origin().unwrap().x;
                    ctx.data_mut(|d| {
                        d.insert_temp(resize_key, (c, start, std::sync::Arc::new(t.clone())))
                    });
                    sel = Some(TSel::at(tab, bi, (0, c)));
                    ctx.memory_mut(|m| m.request_focus(gid));
                    break;
                }
            }
            if let Some((c, start, before)) = ctx.data(|d| d.get_temp::<ColumnResize>(resize_key)) {
                if let Some(pos) = pointer.interact_pos() {
                    t.col_widths[c] =
                        (before.col_widths[c] + (pos.x - start) / zoom).clamp(24., 2000.);
                }
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                if !pointer.primary_down() {
                    ctx.data_mut(|d| d.remove::<ColumnResize>(resize_key));
                    if before.col_widths != t.col_widths {
                        cx.undo.push(
                            "열 너비",
                            UndoKind::Table {
                                tab,
                                bi,
                                table: (*before).clone(),
                            },
                            cx.now,
                        );
                    }
                }
            }
            let (pressed, down, pos, mods) = ui.input(|i| {
                (
                    i.pointer.primary_pressed(),
                    i.pointer.primary_down(),
                    i.pointer.interact_pos(),
                    i.modifiers,
                )
            });
            // ---- mouse: select, extend, drag a range
            if pressed && !edge_pressed {
                if let Some(p) = pos.filter(|p| {
                    rect.contains(*p)
                        && ui.clip_rect().contains(*p)
                        && ctx.layer_id_at(*p) == Some(layer)
                }) {
                    let editing_box = sel.as_ref().filter(|s| s.editing).map(|s| {
                        cell_box(
                            origin,
                            &col_x,
                            &widths,
                            &geo,
                            te::span_rect(t, s.head.0, s.head.1),
                        )
                    });
                    if !editing_box.is_some_and(|b| b.contains(p)) {
                        if let Some(cell) = grid_cell(t, origin, &col_x, &geo, p) {
                            if let Some(s) = sel.as_mut() {
                                tbl_commit(t, s, cx);
                            }
                            match sel.as_mut() {
                                Some(s) if mods.shift => s.head = cell,
                                _ => sel = Some(TSel::at(tab, bi, cell)),
                            }
                            if let Some(s) = sel.as_mut() {
                                s.drag = true;
                            }
                            ctx.memory_mut(|m| m.request_focus(gid));
                        }
                    }
                }
            }
            if let Some(s) = sel.as_mut().filter(|s| s.drag) {
                if down {
                    if let Some(p) = pos {
                        let p = egui::Pos2::new(
                            p.x.clamp(rect.min.x + 0.5, rect.max.x - 0.5),
                            p.y.clamp(rect.min.y + 0.5, rect.max.y - 0.5),
                        );
                        if let Some(cell) = grid_cell(t, origin, &col_x, &geo, p) {
                            s.head = cell;
                        }
                    }
                } else {
                    s.drag = false;
                }
            }
            if resp.double_clicked() {
                if let Some(s) = sel.as_mut().filter(|s| !s.editing) {
                    s.drag = false;
                    tbl_begin_edit(t, s, false, &ctx, edit_id(s.head.0, s.head.1));
                }
            }
            if resp.secondary_clicked() {
                if let Some(cell) = resp
                    .interact_pointer_pos()
                    .and_then(|p| grid_cell(t, origin, &col_x, &geo, p))
                {
                    let inside = sel.as_ref().is_some_and(|s| {
                        let g = s.range(t);
                        cell.0 >= g.0 && cell.0 <= g.2 && cell.1 >= g.1 && cell.1 <= g.3
                    });
                    if !inside {
                        if let Some(s) = sel.as_mut() {
                            tbl_commit(t, s, cx);
                        }
                        sel = Some(TSel::at(tab, bi, cell));
                    }
                    ctx.memory_mut(|m| m.request_focus(gid));
                }
            }
            // ---- keyboard on the selection (no cell being edited)
            let grid_focus = ctx.memory(|m| m.has_focus(gid));
            if grid_focus {
                ctx.memory_mut(|m| {
                    m.set_focus_lock_filter(
                        gid,
                        egui::EventFilter {
                            tab: true,
                            horizontal_arrows: true,
                            vertical_arrows: true,
                            escape: true,
                        },
                    )
                });
            }
            let keys_on = grid_focus && !cx.ime && sel.as_ref().is_some_and(|s| !s.editing);
            if keys_on {
                let take = |m: Modifiers, k: Key| ui.input_mut(|i| i.consume_key(m, k));
                let s = sel.as_mut().expect("checked above");
                if take(Modifiers::CTRL, Key::A) {
                    s.anchor = (0, 0);
                    s.head = (nr - 1, nc - 1);
                }
                for (key, dir) in [
                    (Key::ArrowUp, 0u8),
                    (Key::ArrowDown, 1),
                    (Key::ArrowLeft, 2),
                    (Key::ArrowRight, 3),
                ] {
                    if take(Modifiers::SHIFT, key) {
                        s.head = te::step(t, s.head, dir);
                    } else if take(Modifiers::NONE, key) {
                        s.head = te::step(t, s.head, dir);
                        s.anchor = s.head;
                    }
                }
                // (`consume_key` also matches the key with extra modifiers held,
                // so the modified shortcuts are asked first.)
                let mut go = None;
                if take(Modifiers::SHIFT, Key::Tab) {
                    go = Some(te::next_cell(t, s.head, true));
                } else if take(Modifiers::NONE, Key::Tab) {
                    go = Some(te::next_cell(t, s.head, false));
                } else if take(Modifiers::SHIFT, Key::Enter) {
                    go = Some(te::step(t, s.head, 0));
                } else if take(Modifiers::NONE, Key::Enter) {
                    go = Some(te::step(t, s.head, 1));
                }
                if let Some(n) = go {
                    s.head = n;
                    s.anchor = n;
                }
                if take(Modifiers::NONE, Key::F2) {
                    tbl_begin_edit(t, s, false, &ctx, edit_id(s.head.0, s.head.1));
                }
                if take(Modifiers::NONE, Key::Delete) || take(Modifiers::NONE, Key::Backspace) {
                    act = Some(TAct::Clear);
                }
                let (copy, cut, paste, typed) = ui.input(|i| {
                    let (mut copy, mut cut, mut paste, mut typed) = (false, false, None, false);
                    for e in &i.events {
                        match e {
                            egui::Event::Copy => copy = true,
                            egui::Event::Cut => cut = true,
                            egui::Event::Paste(s) => paste = Some(s.clone()),
                            egui::Event::Text(s) if !s.chars().all(char::is_control) => {
                                typed = true
                            }
                            egui::Event::Ime(egui::ImeEvent::Preedit(s)) if !s.is_empty() => {
                                typed = true
                            }
                            egui::Event::Ime(egui::ImeEvent::Commit(_)) => typed = true,
                            _ => {}
                        }
                    }
                    (copy, cut, paste, typed)
                });
                if copy || cut || paste.is_some() {
                    ui.input_mut(|i| {
                        i.events.retain(|e| {
                            !matches!(
                                e,
                                egui::Event::Copy | egui::Event::Cut | egui::Event::Paste(_)
                            )
                        })
                    });
                }
                if copy {
                    act = Some(TAct::Copy);
                } else if cut {
                    act = Some(TAct::Cut);
                } else if let Some(text) = paste {
                    ctx.data_mut(|d| d.insert_temp(gid.with("paste_text"), text));
                    act = Some(TAct::Paste);
                } else if typed {
                    tbl_begin_edit(t, s, true, &ctx, edit_id(s.head.0, s.head.1));
                }
                if take(Modifiers::NONE, Key::Escape) {
                    sel = None;
                }
            }
            // ---- paint the cells as text
            let painter = ui.painter().clone();
            let clip = ui.clip_rect();
            // Black cell borders (a light gray on a dark theme).
            let line = if ui.visuals().dark_mode {
                Color32::from_gray(200)
            } else {
                Color32::BLACK
            };
            let stroke = Stroke::new(1.0_f32, line);
            let theme_text = ui.visuals().text_color();
            let editing_cell = sel.as_ref().filter(|s| s.editing).map(|s| s.head);
            for r in 0..nr {
                for c in 0..nc {
                    let cell = &t.cells[r][c];
                    if cell.covered {
                        continue;
                    }
                    let cr = cell_box(origin, &col_x, &widths, &geo, te::span_rect(t, r, c));
                    if !cr.intersects(clip) {
                        continue;
                    }
                    let (fill, text_color) = cell_colors(cell, theme_text);
                    if let Some(f) = fill {
                        painter.rect_filled(cr, 0.0, f);
                    }
                    painter.rect_stroke(cr, 0.0, stroke, egui::StrokeKind::Inside);
                    if editing_cell == Some((r, c)) || cell.text.is_empty() {
                        continue;
                    }
                    let avail = (cr.width() - 12.).max(8.);
                    let mut job = layout(
                        &cell.text,
                        &[],
                        &cell.format,
                        font.clone(),
                        bold_font.clone(),
                        text_color,
                    );
                    job.wrap.max_width = avail;
                    // Rows are aligned around the anchor x, so the anchor
                    // sits at the left, middle or right of the cell.
                    job.halign = cell.align.egui();
                    let galley = ui.fonts(|f| f.layout_job(job));
                    let at = cr.min + Vec2::new(6. + avail * cell.align.factor(), 6.);
                    painter
                        .with_clip_rect(cr.shrink(1.).intersect(clip))
                        .galley(at, galley, text_color);
                }
            }
            // ---- selection highlight
            if let Some(s) = &sel {
                let g = s.range(t);
                let sr = cell_box(origin, &col_x, &widths, &geo, g);
                if !s.editing {
                    painter.rect_filled(
                        sr,
                        0.0,
                        accent.gamma_multiply(if grid_focus { 0.16 } else { 0.08 }),
                    );
                }
                painter.rect_stroke(
                    sr,
                    0.0,
                    Stroke::new(2.0_f32, accent),
                    egui::StrokeKind::Inside,
                );
                if !s.editing && (g.0 != g.2 || g.1 != g.3) {
                    let hb = cell_box(
                        origin,
                        &col_x,
                        &widths,
                        &geo,
                        te::span_rect(t, s.head.0, s.head.1),
                    );
                    painter.rect_stroke(
                        hb,
                        0.0,
                        Stroke::new(1.0_f32, accent),
                        egui::StrokeKind::Inside,
                    );
                }
            }
            // ---- the one cell being edited
            let mut finished_move: Option<(usize, usize)> = None;
            if let Some(s) = sel.as_mut().filter(|s| s.editing) {
                let (r, c) = s.head;
                let eid = edit_id(r, c);
                if !cx.ime {
                    let take = |m: Modifiers, k: Key| ui.input_mut(|i| i.consume_key(m, k));
                    if take(Modifiers::NONE, Key::Escape) {
                        tbl_cancel(t, s);
                    } else if take(Modifiers::SHIFT, Key::Tab) {
                        tbl_commit(t, s, cx);
                        finished_move = Some(te::next_cell(t, (r, c), true));
                    } else if take(Modifiers::NONE, Key::Tab) {
                        tbl_commit(t, s, cx);
                        finished_move = Some(te::next_cell(t, (r, c), false));
                    } else if take(Modifiers::ALT, Key::Enter) || take(Modifiers::SHIFT, Key::Enter)
                    {
                        // A line break inside the cell.
                        let mut st = egui::TextEdit::load_state(&ctx, eid).unwrap_or_default();
                        let (a, b) = st
                            .cursor
                            .char_range()
                            .map(|r| {
                                let [a, b] = r.sorted();
                                (a.index, b.index)
                            })
                            .unwrap_or_default();
                        replace_chars(&mut t.cells[r][c].text, a..b, "\n");
                        st.cursor
                            .set_char_range(Some(CCursorRange::one(CCursor::new(a + 1))));
                        st.store(&ctx, eid);
                    } else if take(Modifiers::NONE, Key::Enter) {
                        tbl_commit(t, s, cx);
                        finished_move = Some(te::step(t, (r, c), 1));
                    }
                }
                // Pasting a table (tabs, or HTML) into the cell fills cells from here.
                let pasted = ui.input(|i| {
                    i.events.iter().find_map(|e| match e {
                        egui::Event::Paste(s) => Some(s.clone()),
                        _ => None,
                    })
                });
                if s.editing {
                    if let Some(text) = pasted {
                        let src = clipboard_table(Some(&text)).filter(|x| {
                            x.cells.len() > 1 || x.cells[0].len() > 1 || text.contains('\t')
                        });
                        if let Some(src) = src {
                            ui.input_mut(|i| {
                                i.events.retain(|e| !matches!(e, egui::Event::Paste(_)))
                            });
                            let at = s.head;
                            tbl_cancel(t, s);
                            let snap = t.clone();
                            let area = te::paste_table(t, at.0, at.1, &src);
                            cx.undo.push(
                                "표에 붙여넣기",
                                UndoKind::Table {
                                    tab,
                                    bi,
                                    table: snap,
                                },
                                cx.now,
                            );
                            s.anchor = (area.0, area.1);
                            s.head = te::owner_of(t, area.2, area.3);
                            *cx.status = "표에 붙여넣었습니다".into();
                        } else if text.starts_with('"') {
                            // A single Excel cell arrives quoted.
                            let g = te::parse_tsv(&text);
                            if g.len() == 1 && g[0].len() == 1 {
                                let one = g[0][0].clone();
                                ui.input_mut(|i| {
                                    for e in &mut i.events {
                                        if let egui::Event::Paste(p) = e {
                                            *p = one.clone();
                                        }
                                    }
                                });
                            }
                        }
                    }
                }
                if s.editing {
                    let (r, c) = s.head;
                    let cr = cell_box(origin, &col_x, &widths, &geo, te::span_rect(t, r, c));
                    let mut cell_ui =
                        ui.new_child(egui::UiBuilder::new().id_salt(eid).max_rect(cr));
                    let cf = t.cells[r][c].format.clone();
                    let (_, col_color) =
                        cell_colors(&t.cells[r][c], cell_ui.visuals().text_color());
                    let h_align = t.cells[r][c].align.egui();
                    let (f, bf) = (font.clone(), bold_font.clone());
                    let mut lay = move |ui: &egui::Ui, txt: &str, wrap_w: f32| {
                        let mut j = layout(txt, &[], &cf, f.clone(), bf.clone(), col_color);
                        j.wrap.max_width = wrap_w;
                        ui.fonts(|f| f.layout_job(j))
                    };
                    egui::Frame::NONE.show(&mut cell_ui, |ui| {
                        ui.add_sized(
                            [cr.width(), cr.height()],
                            egui::TextEdit::multiline(&mut t.cells[r][c].text)
                                .id(eid)
                                .font(font.clone())
                                .margin(Vec2::splat(6.))
                                .frame(false)
                                .horizontal_align(h_align)
                                .layouter(&mut lay),
                        );
                    });
                    s.age = s.age.saturating_add(1);
                    // Clicking elsewhere ends the edit.
                    if s.age > 2 && !ctx.memory(|m| m.has_focus(eid)) {
                        tbl_commit(t, s, cx);
                    }
                }
            }
            if let Some(n) = finished_move {
                if let Some(s) = sel.as_mut() {
                    s.head = n;
                    s.anchor = n;
                }
                ctx.memory_mut(|m| m.request_focus(gid));
            }
            // ---- context menu
            crate::float::context_menu(&resp, 230., |ui| {
                let has = sel.is_some();
                let merged = sel.as_ref().is_some_and(|s| {
                    let g = s.range(t);
                    te::span_rect(t, g.0, g.1) != (g.0, g.1, g.0, g.1)
                });
                let multi = sel.as_ref().is_some_and(|s| {
                    let g = s.range(t);
                    te::span_rect(t, g.0, g.1) != g
                });
                // In the order and with the keys of the body's menu.
                for (label, keys, a) in [
                    ("잘라내기", "Ctrl+X", TAct::Cut),
                    ("복사", "Ctrl+C", TAct::Copy),
                    ("붙여넣기", "Ctrl+V", TAct::Paste),
                    ("선택 영역 내용 삭제", "Delete", TAct::Clear),
                ] {
                    if ui
                        .add_enabled(has, egui::Button::new(label).shortcut_text(keys))
                        .clicked()
                    {
                        act = Some(a);
                        ui.close_menu();
                    }
                }
                ui.separator();
                for (label, a) in [
                    ("위에 행 추가", TAct::RowAbove),
                    ("아래에 행 추가", TAct::RowBelow),
                    ("왼쪽에 열 추가", TAct::ColLeft),
                    ("오른쪽에 열 추가", TAct::ColRight),
                    ("선택한 행 삭제", TAct::DelRows),
                    ("선택한 열 삭제", TAct::DelCols),
                ] {
                    if ui.add_enabled(has, egui::Button::new(label)).clicked() {
                        act = Some(a);
                        ui.close_menu();
                    }
                }
                ui.separator();
                if ui
                    .add_enabled(has && multi, egui::Button::new("선택한 셀 병합"))
                    .clicked()
                {
                    act = Some(TAct::Merge);
                    ui.close_menu();
                }
                if ui
                    .add_enabled(has && merged, egui::Button::new("병합 해제"))
                    .clicked()
                {
                    act = Some(TAct::Unmerge);
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("열 너비 일정하게").clicked() {
                    act = Some(TAct::EqualCols);
                    ui.close_menu();
                }
                if ui.button("행 높이 일정하게").clicked() {
                    act = Some(TAct::EqualRows);
                    ui.close_menu();
                }
            });
        });
    ui.horizontal(|ui| {
        if toolbar_button(ui, "+ 행").clicked() {
            act = Some(TAct::AddRowEnd);
        }
        if toolbar_button(ui, "+ 열").clicked() {
            act = Some(TAct::AddColEnd);
        }
        if toolbar_button(ui, "표 복사").clicked() {
            act = Some(TAct::CopyAll);
        }
        // Alignment of the selected cells (the cell being edited, or the range).
        let head_align = sel.as_ref().map(|s| t.cells[s.head.0][s.head.1].align);
        for (al, tip) in [
            (CellAlign::Left, "왼쪽 정렬"),
            (CellAlign::Center, "가운데 정렬"),
            (CellAlign::Right, "오른쪽 정렬"),
        ] {
            let on = head_align
                .is_some_and(|h| h == al || (al == CellAlign::Left && h == CellAlign::Default));
            if align_button(ui, al, on, tip).clicked() && sel.is_some() {
                act = Some(TAct::Align(al));
            }
        }
        // Deleting asks once more, in the same menu.
        let ask = ui.id().with("table_remove_ask");
        let asking = ui.data(|d| d.get_temp::<bool>(ask).unwrap_or(false));
        if !asking {
            if ui.button("표 삭제…").clicked() {
                ui.data_mut(|d| d.insert_temp(ask, true));
                crate::float::keep_open(ui);
            }
        } else {
            ui.label("이 표를 삭제할까요?");
            if ui
                .button(egui::RichText::new("삭제").color(crate::ACCENT))
                .clicked()
            {
                act = Some(TAct::Remove);
                ui.data_mut(|d| d.remove::<bool>(ask));
            }
        }
    });
    let mut remove = false;
    if let Some(a) = act {
        if let Some(s) = sel.as_mut() {
            tbl_commit(t, s, cx);
        }
        let rng = sel.as_ref().map(|s| s.range(t));
        let (r0, c0, r1, c1) = rng.unwrap_or((0, 0, 0, 0));
        let snap = t.clone();
        let (nr, nc) = (te::rows(t), te::cols(t));
        let mut changed = false;
        let mut label = "";
        match a {
            TAct::Copy => {
                if rng.is_some() {
                    copy_range(t, (r0, c0, r1, c1), cx.status);
                }
            }
            TAct::CopyAll => copy_range(t, (0, 0, nr - 1, nc - 1), cx.status),
            TAct::Cut => {
                copy_range(t, (r0, c0, r1, c1), cx.status);
                te::clear_range(t, (r0, c0, r1, c1));
                changed = true;
                label = "셀 잘라내기";
            }
            TAct::Clear => {
                te::clear_range(t, (r0, c0, r1, c1));
                changed = true;
                label = "셀 내용 삭제";
            }
            TAct::Paste => {
                let text: Option<String> =
                    ui.ctx().data_mut(|d| d.remove_temp(gid.with("paste_text")));
                if let Some(src) = clipboard_table(text.as_deref()) {
                    let one = src.cells.len() == 1 && src.cells[0].len() == 1;
                    let area = te::paste_table(t, r0, c0, &src);
                    if let Some(s) = sel.as_mut() {
                        s.anchor = (area.0, area.1);
                        s.head = te::owner_of(t, area.2, area.3);
                        if one {
                            s.head = s.anchor;
                        }
                    }
                    changed = true;
                    label = "표에 붙여넣기";
                } else {
                    *cx.status = "붙여넣을 표 데이터가 없습니다".into();
                }
            }
            TAct::RowAbove => {
                if te::insert_row(t, r0) {
                    if let Some(s) = sel.as_mut() {
                        s.anchor.0 += 1;
                        s.head.0 += 1;
                    }
                    changed = true;
                    label = "행 추가";
                }
            }
            TAct::RowBelow => {
                changed = te::insert_row(t, r1 + 1);
                label = "행 추가";
            }
            TAct::AddRowEnd => {
                changed = te::insert_row(t, nr);
                label = "행 추가";
            }
            TAct::ColLeft => {
                if te::insert_col(t, c0) {
                    if let Some(s) = sel.as_mut() {
                        s.anchor.1 += 1;
                        s.head.1 += 1;
                    }
                    changed = true;
                    label = "열 추가";
                }
            }
            TAct::ColRight => {
                changed = te::insert_col(t, c1 + 1);
                label = "열 추가";
            }
            TAct::AddColEnd => {
                changed = te::insert_col(t, nc);
                label = "열 추가";
            }
            TAct::DelRows => {
                if te::delete_rows(t, r0, r1) {
                    let n = te::rows(t);
                    sel = Some(TSel::at(tab, bi, te::owner_of(t, r0.min(n - 1), c0)));
                    changed = true;
                    label = "행 삭제";
                } else {
                    *cx.status = "마지막 행은 삭제할 수 없습니다 (표 삭제를 사용하세요)".into();
                }
            }
            TAct::DelCols => {
                if te::delete_cols(t, c0, c1) {
                    let n = te::cols(t);
                    sel = Some(TSel::at(tab, bi, te::owner_of(t, r0, c0.min(n - 1))));
                    changed = true;
                    label = "열 삭제";
                } else {
                    *cx.status = "마지막 열은 삭제할 수 없습니다 (표 삭제를 사용하세요)".into();
                }
            }
            TAct::Merge => {
                if te::merge_range(t, (r0, c0, r1, c1)) {
                    if let Some(s) = sel.as_mut() {
                        s.anchor = (r0, c0);
                        s.head = (r0, c0);
                    }
                    changed = true;
                    label = "셀 병합";
                }
            }
            TAct::Unmerge => {
                changed = te::unmerge(t, r0, c0);
                label = "병합 해제";
            }
            TAct::EqualCols => {
                let average = t.col_widths.iter().sum::<f32>() / nc as f32;
                t.col_widths.fill(average);
                changed = true;
                label = "열 너비";
            }
            TAct::EqualRows => {
                let average = geo.h.iter().sum::<f32>() / (nr as f32 * zoom);
                t.row_height = average;
                t.row_manual.fill(average);
                changed = true;
                label = "행 높이";
            }
            TAct::Remove => remove = true,
            TAct::Align(al) => {
                for r in r0..=r1 {
                    for k in c0..=c1 {
                        if !t.cells[r][k].covered {
                            t.cells[r][k].align = al;
                        }
                    }
                }
                changed = true;
                label = "정렬";
            }
        }
        if changed {
            cx.undo.push(
                label,
                UndoKind::Table {
                    tab,
                    bi,
                    table: snap,
                },
                cx.now,
            );
        }
    }
    match &sel {
        Some(s) => *cx.selected = Some((bi, s.head.0, s.head.1)),
        None if cx.selected.is_some_and(|(b, _, _)| b == bi) => *cx.selected = None,
        None => {}
    }
    if !remove {
        if let Some(s) = sel {
            *cx.tsel = Some(s);
        }
    }
    remove
}
fn picture_from_file(path: &Path, scale: f32, fit: bool) -> Result<Picture, String> {
    let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if meta.len() > 64 * 1024 * 1024 {
        return Err("64 MB 이하 이미지를 선택하세요".into());
    }
    let mut reader = image::ImageReader::open(path)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    let img = reader
        .decode()
        .map_err(|_| "PNG/JPEG 이미지를 읽을 수 없습니다")?
        .to_rgba8();
    Ok(Picture {
        instance: new_block_instance(),
        name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into(),
        width: img.width() as usize,
        height: img.height() as usize,
        pixels: img.into_raw(),
        scale,
        fit,
        file: String::new(),
        texture: None,
        texture_used: 0.,
        id: 0,
    })
}
fn picture_from_clipboard(scale: f32, fit: bool) -> Result<Picture, String> {
    let mut clipboard = crate::clipboard_copy::Clipboard::new().map_err(|e| e.to_string())?;
    let image = clipboard
        .get_image()
        .map_err(|_| "클립보드에 이미지가 없습니다".to_string())?;
    if image.width > 8192 || image.height > 8192 || image.bytes.len() > 256 * 1024 * 1024 {
        return Err("8192×8192 이하 이미지를 사용하세요".into());
    }
    Ok(Picture {
        instance: new_block_instance(),
        name: "붙여 넣은 이미지".into(),
        width: image.width,
        height: image.height,
        pixels: image.bytes.into_owned(),
        scale,
        fit,
        file: String::new(),
        texture: None,
        texture_used: 0.,
        id: 0,
    })
}
#[cfg(test)]
pub fn is_picture(c: char) -> bool {
    picture_id(c).is_some()
}
pub fn strip_pictures(text: &str) -> String {
    text.chars().filter(|c| picture_id(*c).is_none()).collect()
}
fn char_byte(text: &str, index: usize) -> usize {
    text.char_indices()
        .nth(index)
        .map_or(text.len(), |(b, _)| b)
}
fn replace_chars(text: &mut String, range: Range<usize>, replacement: &str) {
    let a = char_byte(text, range.start);
    let b = char_byte(text, range.end);
    text.replace_range(a..b, replacement);
}
fn short(s: &str, n: usize) -> String {
    if s.chars().count() > n {
        format!("{}…", s.chars().take(n).collect::<String>())
    } else {
        s.into()
    }
}
fn transform_spans(before: &str, after: &str, spans: &mut Vec<Span>, typing: &Format) {
    let a: Vec<_> = before.chars().collect();
    let b: Vec<_> = after.chars().collect();
    let head = a.iter().zip(&b).take_while(|(a, b)| a == b).count();
    let tail = a[head..]
        .iter()
        .rev()
        .zip(b[head..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let old_end = a.len() - tail;
    let new_end = b.len() - tail;
    let delta = new_end as isize - old_end as isize;
    for s in spans.iter_mut() {
        s.range.start = if s.range.start >= old_end {
            (s.range.start as isize + delta).max(0) as usize
        } else {
            s.range.start.min(head)
        };
        s.range.end = if s.range.end >= old_end {
            (s.range.end as isize + delta).max(0) as usize
        } else {
            s.range.end.min(head)
        };
    }
    spans.retain(|s| s.range.start < s.range.end && s.range.end <= b.len());
    if new_end > head
        && !spans
            .last()
            .is_some_and(|s| s.format == *typing && s.range.start <= head && s.range.end >= new_end)
    {
        spans.push(Span {
            range: head..new_end,
            format: typing.clone(),
        });
    }
}
/// The web address (as [`urls`] finds them) at character `at`, or ending
/// right before it (a click on an address's last letter can land after it).
fn url_at(text: &str, at: usize) -> Option<String> {
    let chars: Vec<char> = text.chars().collect();
    let inside = |i: usize| chars.get(i).is_some_and(|c| !c.is_whitespace());
    let at = if inside(at) {
        at
    } else if at > 0 && inside(at - 1) {
        at - 1
    } else {
        return None;
    };
    let start = (0..at).rev().find(|&i| !inside(i)).map_or(0, |i| i + 1);
    let end = (at..chars.len())
        .find(|&i| !inside(i))
        .unwrap_or(chars.len());
    let word: String = chars[start..end].iter().collect();
    urls(&word).into_iter().next()
}
fn urls(text: &str) -> Vec<String> {
    let mut out = vec![];
    for t in text.split_whitespace() {
        let t = t.trim_matches(|c: char| matches!(c, ')' | ']' | '>' | ',' | ';' | '"' | '\''));
        if (t.starts_with("http://") || t.starts_with("https://")) && !out.iter().any(|s| s == t) {
            out.push(t.to_owned());
        }
    }
    out
}
#[derive(Clone, Debug, PartialEq)]
struct Marker {
    family: String,
    value: String,
    indent: usize,
}
fn marker(line: &str) -> Option<Marker> {
    let indent = line.chars().take_while(|c| *c == ' ').count();
    let token = line.split_whitespace().next()?;
    if matches!(token, "-" | "*") {
        return Some(Marker {
            family: token.into(),
            value: token.into(),
            indent,
        });
    }
    let (left, inner, right) = if token.starts_with('(') && token.ends_with(')') {
        ("(", &token[1..token.len() - 1], ")")
    } else if let Some(v) = token.strip_suffix(')') {
        ("", v, ")")
    } else {
        ("", token.strip_suffix('.')?, ".")
    };
    let kind = if inner.chars().all(|c| c.is_ascii_digit()) {
        "number"
    } else if inner
        .chars()
        .all(|c| matches!(c, 'i' | 'v' | 'x' | 'l' | 'c' | 'd' | 'm'))
        && (inner.starts_with('i') || inner.len() > 1)
    {
        "roman"
    } else if inner
        .chars()
        .all(|c| matches!(c, 'I' | 'V' | 'X' | 'L' | 'C' | 'D' | 'M'))
        && (inner.starts_with('I') || inner.len() > 1)
    {
        "ROMAN"
    } else if inner.len() == 1 && inner.chars().all(|c| c.is_ascii_lowercase()) {
        "alpha"
    } else if inner.len() == 1 && inner.chars().all(|c| c.is_ascii_uppercase()) {
        "ALPHA"
    } else if inner.chars().count() == 1 && "ㄱㄴㄷㄹㅁㅂㅅㅇㅈㅊㅋㅌㅍㅎ".contains(inner)
    {
        "korean"
    } else if inner.chars().count() == 1 && "가나다라마바사아자차카타파하".contains(inner)
    {
        "hangul"
    } else {
        return None;
    };
    Some(Marker {
        family: format!("{left}{kind}{right}"),
        value: token.into(),
        indent,
    })
}
/// The text of a list item without its number or bullet (`"  1. 내용"` gives
/// `"내용"`, a marker with nothing after it gives `""`); `None` for a line
/// that is not a list item.
pub fn list_item_text(line: &str) -> Option<String> {
    marker(line)?;
    let rest = line
        .trim_start()
        .split_once(char::is_whitespace)
        .map_or("", |(_, r)| r);
    Some(rest.trim().to_owned())
}
fn continuation(text: &str, cursor: usize) -> String {
    let prefix = &text[..char_byte(text, cursor)];
    let line = prefix.rsplit('\n').next().unwrap_or("");
    if marker(line).is_none() {
        return "\n".into();
    }
    let Some(m) = contextual_markers(prefix).last().cloned() else {
        return "\n".into();
    };
    if line.trim() == m.value {
        return "\n".into();
    }
    format!(
        "\n{}{} ",
        " ".repeat(m.indent),
        next_marker(&m.value, &m.family)
    )
}
// Single-letter i/v/x can be either alphabetic or Roman. Keep the established
// sequence at this indentation level; a fresh i/I starts a Roman sequence.
fn contextual_markers(text: &str) -> Vec<Marker> {
    let mut out: Vec<Marker> = vec![];
    for line in text.lines() {
        let Some(mut m) = marker(line) else {
            continue;
        };
        let inner = m.value.trim_matches(['(', ')', '.']);
        if inner.len() == 1 && inner.chars().all(|c| "ivxlcdmIVXLCDM".contains(c)) {
            let upper = inner.chars().next().is_some_and(char::is_uppercase);
            let shape = (m.value.starts_with('('), m.value.ends_with('.'));
            if let Some(previous) = out.iter().rev().find(|p| {
                p.indent == m.indent && (p.value.starts_with('('), p.value.ends_with('.')) == shape
            }) {
                let roman = previous.family.contains("roman") || previous.family.contains("ROMAN");
                let alpha_next = next_marker(&previous.value, &previous.family) == m.value
                    && (previous.family.contains("alpha") || previous.family.contains("ALPHA"));
                let kind = if roman {
                    if upper {
                        "ROMAN"
                    } else {
                        "roman"
                    }
                } else if alpha_next {
                    if upper {
                        "ALPHA"
                    } else {
                        "alpha"
                    }
                } else if inner.eq_ignore_ascii_case("i") {
                    if upper {
                        "ROMAN"
                    } else {
                        "roman"
                    }
                } else {
                    if upper {
                        "ALPHA"
                    } else {
                        "alpha"
                    }
                };
                m.family = format!(
                    "{}{}{}",
                    if shape.0 { "(" } else { "" },
                    kind,
                    if shape.1 { "." } else { ")" }
                );
            }
        }
        out.push(m);
    }
    out
}
fn next_marker(value: &str, family: &str) -> String {
    if matches!(value, "-" | "*") {
        return value.into();
    }
    let start = usize::from(value.starts_with('('));
    let end = value.len() - 1;
    let inner = &value[start..end];
    let next = if family.contains("number") {
        inner
            .parse::<u32>()
            .ok()
            .and_then(|n| n.checked_add(1))
            .map(|n| n.to_string())
            .unwrap_or_else(|| inner.into())
    } else if family.contains("roman") || family.contains("ROMAN") {
        let upper = inner.chars().next().is_some_and(char::is_uppercase);
        let n = roman_number(inner);
        let s = roman(n + 1);
        if upper {
            s.to_uppercase()
        } else {
            s
        }
    } else if family.contains("alpha") || family.contains("ALPHA") {
        inner
            .chars()
            .next()
            .and_then(|c| char::from_u32(c as u32 + 1))
            .unwrap_or('a')
            .to_string()
    } else {
        let seq = if family.contains("korean") {
            "ㄱㄴㄷㄹㅁㅂㅅㅇㅈㅊㅋㅌㅍㅎ"
        } else {
            "가나다라마바사아자차카타파하"
        };
        let v: Vec<_> = seq.chars().collect();
        let n = v.iter().position(|c| c.to_string() == inner).unwrap_or(0);
        v.get(n + 1).unwrap_or(&v[0]).to_string()
    };
    format!(
        "{}{}{}",
        if start == 1 { "(" } else { "" },
        next,
        &value[end..]
    )
}
fn roman_number(s: &str) -> u32 {
    let mut sum = 0;
    let mut prev = 0;
    for c in s.to_ascii_lowercase().chars().rev() {
        let n = match c {
            'i' => 1,
            'v' => 5,
            'x' => 10,
            'l' => 50,
            'c' => 100,
            'd' => 500,
            'm' => 1000,
            _ => 0,
        };
        if n < prev {
            sum -= n
        } else {
            sum += n;
            prev = n;
        }
    }
    sum
}
fn roman(mut n: u32) -> String {
    let mut out = String::new();
    for (v, t) in [
        (1000, "m"),
        (900, "cm"),
        (500, "d"),
        (400, "cd"),
        (100, "c"),
        (90, "xc"),
        (50, "l"),
        (40, "xl"),
        (10, "x"),
        (9, "ix"),
        (5, "v"),
        (4, "iv"),
        (1, "i"),
    ] {
        while n >= v {
            out.push_str(t);
            n -= v;
        }
    }
    out
}
/// The list item that Enter makes. `nl` is the char index of the newline the
/// editor just inserted. Returns the range to replace (it includes that
/// newline when the item is replaced), the new text and the caret.
///
/// * an item with content gets the next marker of its level on the new line;
/// * an empty item (Enter twice) is replaced by the next marker of the level
///   above it, so `    b) ` becomes `  3. `;
/// * an empty item with no level above it is removed (the line stays, empty).
fn enter_edit(text: &str, nl: usize) -> Option<(Range<usize>, String, usize)> {
    let prefix = &text[..char_byte(text, nl)];
    let line = prefix.rsplit('\n').next().unwrap_or("");
    marker(line)?;
    let m = contextual_markers(prefix).last().cloned()?;
    if line.trim() == m.value {
        let line_start = prefix.len() - line.len();
        let start = prefix[..line_start].chars().count();
        let Some(parent) = contextual_markers(&prefix[..line_start])
            .into_iter()
            .rev()
            // a level above is a different kind of marker (`a)` under `2.`),
            // not just an earlier, less indented item of the same list
            .find(|p| p.indent < m.indent && p.family != m.family)
        else {
            // nothing above: the list ends here, the empty marker goes away
            return Some((start..nl + 1, String::new(), start));
        };
        let new = format!(
            "{}{} ",
            " ".repeat(parent.indent),
            next_marker(&parent.value, &parent.family)
        );
        let caret = start + new.chars().count();
        return Some((start..nl + 1, new, caret));
    }
    let c = continuation(text, nl);
    let rest = c.strip_prefix('\n')?;
    (!rest.is_empty()).then(|| {
        (
            nl + 1..nl + 1,
            rest.to_owned(),
            nl + 1 + rest.chars().count(),
        )
    })
}
/// A marker typed at the start of a line takes its place in the list: its
/// indentation is that of the last marker of its kind, else two spaces
/// deeper than the marker before it (two spaces for the first). When it is
/// typed after an item that was left empty (`  3. a) `), that empty item is
/// dropped and the new marker goes one level deeper. Returns the new caret.
fn normalize_outline_line(text: &mut String, cursor: usize) -> Option<usize> {
    let prefix = &text[..char_byte(text, cursor)];
    let byte_start = prefix.rfind('\n').map_or(0, |x| x + 1);
    let line = &prefix[byte_start..];
    if !line.ends_with(' ') {
        return None;
    }
    let tokens: Vec<&str> = line.split_whitespace().collect();
    let (token, replaces_empty_item) = match tokens.as_slice() {
        [one] if marker(line).is_some() => (*one, false),
        [first, second] if marker(first).is_some() && marker(second).is_some() => (*second, true),
        _ => return None,
    };
    let ancestors = &text[..byte_start];
    // the marker's own family, judged in its place in the list
    let m = contextual_markers(&format!("{ancestors}{token} "))
        .last()?
        .clone();
    let prev = contextual_markers(ancestors);
    let indent = prev
        .iter()
        .rev()
        .find(|p| p.family == m.family)
        .map(|p| p.indent)
        .unwrap_or_else(|| prev.last().map_or(2, |p| p.indent + 2));
    let replacement = format!("{}{} ", " ".repeat(indent), m.value);
    if !replaces_empty_item && replacement == line {
        return None;
    }
    let line_start = text[..byte_start].chars().count();
    replace_chars(text, line_start..cursor, &replacement);
    Some(line_start + replacement.chars().count())
}
fn outdent_line(text: &mut String, cursor: usize) -> usize {
    let prefix = &text[..char_byte(text, cursor)];
    let start = prefix.rfind('\n').map_or(0, |b| b + 1);
    let n = text[start..]
        .chars()
        .take_while(|c| *c == ' ')
        .count()
        .min(2);
    text.replace_range(start..start + n, "");
    cursor.saturating_sub(n)
}
fn shortcut_substitution(
    text: &str,
    cursor: usize,
) -> Option<(Range<usize>, String, Option<Range<usize>>)> {
    let prefix = &text[..char_byte(text, cursor)];
    if prefix.ends_with("[] ") {
        return Some((cursor - 3..cursor, "☐ ".into(), None));
    }
    if prefix.ends_with("~~~ ") {
        let end = cursor - 4;
        let start = prefix[..char_byte(prefix, end)]
            .rfind('\n')
            .map_or(0, |b| prefix[..b + 1].chars().count());
        return Some((end..cursor, String::new(), Some(start..end)));
    }
    if prefix.ends_with("~~ ") {
        let end = cursor - 3;
        let chars: Vec<_> = text.chars().collect();
        let mut start = end;
        while start > 0 && !chars[start - 1].is_whitespace() {
            start -= 1;
        }
        return Some((end..cursor, " ".into(), Some(start..end)));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_font_dialog_starts_from_what_every_selected_character_shares() {
        let mut d = Document::default();
        d.tabs[0].body = "abcdef".into();
        let bold_red = Format {
            bold: true,
            color: 1,
            ..Default::default()
        };
        d.tabs[0].spans.push(Span {
            range: 0..2,
            format: bold_red.clone(),
        });
        d.selection = 0..2;
        assert_eq!(d.font_choice().format, bold_red);
        // Half bold, two colors: neither is shown as chosen.
        d.selection = 0..4;
        let c = d.font_choice();
        assert!(!c.format.bold);
        assert_eq!(c.format.color, MIXED);
        assert_eq!(c.format.highlight, 0);
        // Nothing selected: what is typed next.
        d.selection = 3..3;
        d.format.italic = true;
        assert!(d.font_choice().format.italic);
    }
    #[test]
    fn the_font_dialog_changes_only_what_was_picked_on_the_selection_it_opened_with() {
        let mut d = Document::default();
        d.tabs[0].body = "abcdef".into();
        d.tabs[0].spans.push(Span {
            range: 0..2,
            format: Format {
                bold: true,
                color: 1,
                ..Default::default()
            },
        });
        d.selection = 0..4;
        let mut c = d.font_choice();
        c.font_name = "굴림".into();
        c.font_size = 20. / 0.75;
        c.format.underline = true;
        // The text was clicked elsewhere while the dialog was open.
        d.selection = 5..5;
        d.apply_font_choice(&c);
        assert_eq!(d.font_name, "굴림");
        assert!((d.font_size * 0.75 - 20.).abs() < 0.01);
        let w = winning_spans(&d.tabs[0].spans, 6);
        let f = |i: usize| match w[i] {
            u32::MAX => Format::default(),
            k => d.tabs[0].spans[k as usize].format.clone(),
        };
        for i in 0..4 {
            assert!(f(i).underline, "{i}");
        }
        // Bold and the two colors were left as they were (mixed).
        assert!(f(0).bold && f(1).bold && !f(2).bold);
        assert_eq!((f(0).color, f(3).color), (1, 0));
        assert_eq!(f(4), Format::default(), "outside the selection");
        assert_eq!(d.selection, 5..5, "the caret stays where it was put");
        // A color picked over a mixed selection colors all of it.
        d.selection = 0..4;
        let mut c = d.font_choice();
        c.format.color = 4;
        d.apply_font_choice(&c);
        let w = winning_spans(&d.tabs[0].spans, 6);
        let color = |i: usize| d.tabs[0].spans[w[i] as usize].format.color;
        assert!((0..4).all(|i| color(i) == 4));
    }
    #[test]
    fn with_nothing_selected_the_font_dialog_sets_what_is_typed_next() {
        let mut d = Document::default();
        d.tabs[0].body = "abc".into();
        d.selection = 3..3;
        let mut c = d.font_choice();
        c.format.bold = true;
        c.format.highlight = 2;
        d.apply_font_choice(&c);
        assert!(d.format.bold);
        assert_eq!(d.format.highlight, 2);
        assert!(d.tabs[0].spans.is_empty(), "no text was restyled");
        // An unchanged dialog changes nothing at all.
        let before = d.format.clone();
        let c = d.font_choice();
        d.apply_font_choice(&c);
        assert_eq!(d.format, before);
    }
    #[test]
    fn the_font_preview_shows_the_selected_line_or_a_sample() {
        let mut d = Document::default();
        d.tabs[0].body = "첫 줄입니다\n둘째 줄".into();
        d.selection = 2..9;
        assert_eq!(d.font_choice().sample(&d), "줄입니다");
        d.selection = 1..1;
        assert_eq!(d.font_choice().sample(&d), "가나다라 AaBbCc 123");
    }
    #[test]
    fn coloring_a_selection_keeps_its_bold_and_paints_the_layout() {
        let mut d = Document::default();
        d.tabs[0].body = "abcdef".into();
        d.tabs[0].spans.push(Span {
            range: 0..3,
            format: Format {
                bold: true,
                ..Default::default()
            },
        });
        d.selection = 1..5;
        d.paint_selection(false, 1);
        d.paint_selection(true, 2);
        let w = winning_spans(&d.tabs[0].spans, 6);
        let f = |i: usize| d.tabs[0].spans[w[i] as usize].format.clone();
        assert!(f(1).bold && f(1).color == 1 && f(1).highlight == 2);
        assert!(!f(3).bold && f(4).color == 1);
        assert_eq!(w[5], u32::MAX, "outside the selection: untouched");
        assert_eq!(w[0], 0);
        let font = FontId::proportional(14.);
        let job = layout(
            "abcdef",
            &d.tabs[0].spans,
            &Format::default(),
            font.clone(),
            font,
            Color32::BLACK,
        );
        assert_eq!(job.sections[2].format.color, TEXT_COLORS[1].1);
        assert_eq!(job.sections[2].format.background, HIGHLIGHTS[2].1);
        assert_eq!(job.sections[5].format.background, Color32::TRANSPARENT);
    }
    #[test]
    fn bold_after_coloring_keeps_colors_and_does_not_spread_them() {
        let mut d = Document::default();
        d.tabs[0].body = "red bold".into();
        d.selection = 0..3;
        d.paint_selection(false, 1);
        assert_eq!(
            d.format.color, 0,
            "a colored selection leaves the typing color"
        );
        d.selection = 4..8;
        d.toggle_format(0);
        d.selection = 0..3;
        d.toggle_format(0);
        let w = winning_spans(&d.tabs[0].spans, 8);
        let f = |i: usize| d.tabs[0].spans[w[i] as usize].format.clone();
        assert!(f(5).bold && f(5).color == 0, "'bold' is not red");
        assert!(
            f(1).bold && f(1).color == 1,
            "'red' keeps its color when bolded"
        );
    }
    thread_local! {
        /// Stands in for the keyboard state `chord_down` reads.
        pub(super) static CHORD: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
        /// Stands in for the clipboard's picture (`None`: no image, or text).
        pub(super) static CLIPBOARD_PICTURE: std::cell::RefCell<Option<Picture>> =
            const { std::cell::RefCell::new(None) };
        /// Stands in for the clipboard's HTML (`None`: no HTML).
        pub(super) static CLIPBOARD_HTML: std::cell::RefCell<Option<String>> =
            const { std::cell::RefCell::new(None) };
    }
    #[test]
    fn remap_cursor_moves_to_the_cut_point_when_a_leading_prefix_is_removed() {
        // "#프로젝트 " (5 chars incl. the trailing space) is stripped from the
        // front, as consume_body_project does. A cursor that was right after
        // the stripped prefix should land at the very start of what's left.
        let old = "#프로젝트 나머지내용";
        let new = "나머지내용";
        assert_eq!(remap_cursor(old, new, 5), 0);
        // A cursor further into the untouched tail keeps its relative
        // distance from the end.
        assert_eq!(
            remap_cursor(old, new, old.chars().count()),
            new.chars().count()
        );
    }
    #[test]
    fn remap_cursor_keeps_a_position_inside_an_untouched_common_prefix() {
        let old = "안녕하세요 세계";
        let new = "안녕하세요 월드";
        // "안녕하세요 " is common to both; a cursor inside it is unaffected.
        assert_eq!(remap_cursor(old, new, 3), 3);
    }
    #[test]
    fn remap_cursor_clamps_a_position_inside_the_replaced_middle_region() {
        let old = "앞 가운데 뒤";
        let new = "앞 뒤";
        // "가운데" was removed from the middle; a cursor that was inside it
        // clamps to right after the common prefix ("앞 ").
        let inside = old.chars().position(|c| c == '운').unwrap();
        assert_eq!(remap_cursor(old, new, inside), 2);
    }
    #[test]
    fn project_tag_strip_remaps_a_live_cursor_instead_of_leaving_it_stale() {
        // Reproduces #62: typing "#프로젝트 " at the body's start and letting
        // the project-tag conversion strip that prefix (as desktop.rs's
        // consume_body_project does) must not leave the TextEdit's persisted
        // cursor pointing at a now-invalid char index in the shorter text,
        // which is what corrupted subsequent Hangul composition.
        let mut body = "#프로젝트 나머지내용".to_owned();
        let mut doc = Document::default();
        let id = "project_tag_cursor_test";
        let ctx = egui::Context::default();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(400., 300.));
        // First frame: render once so the TextEdit's state exists, then
        // place its cursor right after the "#프로젝트 " prefix (char index 5),
        // mimicking where the user's caret sits right after typing the
        // triggering space.
        let mut text_id = None;
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    // The TextEdit response's id is the editor's real
                    // `text_id` for the active tab, salted by the enclosing
                    // Ui's own id (unlike a bare `Id::new(id)`).
                    text_id = Some(show(ui, &mut body, &mut doc, id).id);
                });
            },
        );
        let text_id = text_id.unwrap();
        let mut state = egui::TextEdit::load_state(&ctx, text_id).unwrap_or_default();
        state
            .cursor
            .set_char_range(Some(CCursorRange::one(CCursor::new(5))));
        state.store(&ctx, text_id);
        // The project-tag conversion mutates the legacy body externally,
        // outside this editor, exactly like consume_body_project does.
        body.replace_range(.."#프로젝트 ".len(), "");
        // Next frame: content_impl must notice the external change and
        // remap the stale cursor rather than leave it at index 5 (now past
        // the end of the shrunk "나머지내용" selection start).
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    show(ui, &mut body, &mut doc, id);
                });
            },
        );
        let state = egui::TextEdit::load_state(&ctx, text_id).unwrap();
        let range = state.cursor.char_range().unwrap();
        assert_eq!(range.primary.index, 0);
        assert_eq!(range.secondary.index, 0);
        assert_eq!(body, "나머지내용");
    }
    #[test]
    fn picture_toolbar_drops_below_when_above_would_clip() {
        let clip_top = 40.;
        // Plenty of room above: the toolbar floats above the picture.
        let roomy = egui::Rect::from_min_size(egui::pos2(10., 100.), Vec2::new(200., 80.));
        let pos = picture_toolbar_pos(roomy, clip_top);
        assert_eq!(pos, roomy.left_top() - Vec2::new(0., 34.));
        // The picture sits right at the top of the scroll view: no room
        // above, so the toolbar drops below the picture instead.
        let at_top = egui::Rect::from_min_size(egui::pos2(10., 42.), Vec2::new(200., 80.));
        let pos = picture_toolbar_pos(at_top, clip_top);
        assert_eq!(pos, at_top.left_bottom() + Vec2::new(0., 2.));
    }
    #[test]
    fn font_choices_excludes_synthetic_bold_faces_and_the_broken_entry() {
        let names: Vec<&str> = font_choices().collect();
        assert!(
            !names.contains(&"개구쟁이"),
            "개구쟁이 was never an installed font; it was replaced, not kept"
        );
        assert!(names.contains(&"맑은 고딕"));
        assert!(names.contains(&"휴먼매직체"));
        assert!(names.contains(&"Noto Sans KR"));
        assert!(names.contains(&"Calibri"));
        for name in &names {
            assert!(
                !name.ends_with(" Bold"),
                "{name} is a bold face variant, not its own picker entry"
            );
        }
        // No duplicate display names, and every name appears exactly once
        // in FONTS (so the picker and the font loader agree on the set).
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), names.len());
    }
    #[test]
    fn tabs_keep_bodies_and_deleted_tab_content() {
        let mut d = Document::default();
        d.tabs[0].body = "처음".into();
        d.add_tab();
        d.tabs[1].body = "둘째".into();
        d.delete_tab(1);
        assert_eq!(d.tabs[0].body, "처음");
        assert_eq!(d.trash[0].body, "둘째");
        assert_eq!(d.active, 0);
    }
    #[test]
    fn html_table_keeps_multiline_cells_and_tracks_merges() {
        let html = "<table>\
            <tr><td colspan=\"2\">제목</td></tr>\
            <tr><td>첫 줄<br>둘째 줄</td><td>가\t나</td></tr>\
            </table>";
        let t = Table::from_html(html).expect("table parses");
        assert_eq!(t.cells.len(), 2);
        assert_eq!(
            t.cells[0].len(),
            2,
            "the grid stays rectangular underneath the merge"
        );
        assert_eq!(t.cells[0][0].text, "제목");
        assert_eq!(
            t.cells[0][0].col_span, 2,
            "colspan becomes a real merge, not two cells padded with blank text"
        );
        assert!(
            t.cells[0][1].covered,
            "the cell colspan covers is marked covered rather than holding its own blank text"
        );
        assert_eq!(
            t.cells[1][0].text, "첫 줄\n둘째 줄",
            "<br> becomes a newline instead of running the lines together"
        );
        assert_eq!(
            t.cells[1][1].text, "가\t나",
            "a tab inside a cell's own text is not mistaken for a column boundary"
        );
    }
    #[test]
    fn html_table_rowspan_covers_cells_in_later_rows() {
        let html = "<table>\
            <tr><td rowspan=\"2\">왼쪽</td><td>위</td></tr>\
            <tr><td>아래</td></tr>\
            </table>";
        let t = Table::from_html(html).expect("table parses");
        assert_eq!(t.cells.len(), 2);
        assert_eq!(t.cells[0][0].text, "왼쪽");
        assert_eq!(t.cells[0][0].row_span, 2);
        assert_eq!(
            t.cells[1].len(),
            2,
            "the row the rowspan reaches into still has both columns"
        );
        assert!(
            t.cells[1][0].covered,
            "the rowspan's covered cell carries no text of its own"
        );
        assert_eq!(t.cells[1][1].text, "아래");
    }
    #[test]
    fn non_table_html_falls_back_to_none() {
        assert!(Table::from_html("<p>표가 아닙니다</p>").is_none());
    }
    #[test]
    fn enter_continues_the_level_and_an_empty_item_goes_up_a_level() {
        let apply = |text: &str, nl: usize| {
            let (range, with, caret) = enter_edit(text, nl)?;
            let mut t = text.to_owned();
            replace_chars(&mut t, range, &with);
            Some((t, caret))
        };
        // content: the next marker of the same level
        let t = "  1. 테스트\n";
        assert_eq!(
            apply(t, t.chars().count() - 1).unwrap().0,
            "  1. 테스트\n  2. "
        );
        let t = "  1. 테스트\n  2. ㅇㅇ\n    a) 내용\n";
        let n = t.chars().count() - 1;
        let (out, caret) = apply(t, n).unwrap();
        assert!(out.ends_with("    a) 내용\n    b) "));
        assert_eq!(caret, out.chars().count());
        // an empty item is replaced by the next number of the level above
        let t = "  1. 테스트\n  2. ㅇㅇ\n    a) 내용\n    b) \n";
        let n = t.chars().count() - 1;
        let (out, caret) = apply(t, n).unwrap();
        assert_eq!(out, "  1. 테스트\n  2. ㅇㅇ\n    a) 내용\n  3. ");
        assert_eq!(caret, out.chars().count());
        // two levels deep goes up one level at a time
        let t = "  1. 가\n    a) 나\n      i) 다\n      ii) \n";
        let n = t.chars().count() - 1;
        assert!(apply(t, n).unwrap().0.ends_with("      i) 다\n    b) "));
        // nothing above: the empty marker is removed; a plain line: nothing
        assert_eq!(apply("  1. \n", 5), Some((String::new(), 0)));
        assert_eq!(apply("끝\n  1. \n", 7).unwrap().0, "끝\n");
        assert!(apply("일반 문장\n", 5).is_none());
        // the whole example, one empty item after another
        let t = "1. dd\n  2. dd\n    a) dd\n      - dd\n      - \n";
        let n = t.chars().count() - 1;
        let (out, _) = apply(t, n).unwrap();
        assert!(out.ends_with("      - dd\n    b) "), "{out:?}");
        let (out, _) = apply(&format!("{out}\n"), out.chars().count()).unwrap();
        assert!(out.ends_with("      - dd\n  3. "), "{out:?}");
        let (out, caret) = apply(&format!("{out}\n"), out.chars().count()).unwrap();
        assert!(out.ends_with("      - dd\n"), "{out:?}");
        assert_eq!(caret, out.chars().count());
    }
    #[test]
    fn a_marker_typed_after_an_empty_item_replaces_it_one_level_deeper() {
        let mut t = "  1. 테스트\n  2. ㅇㅇ\n  3. a) ".to_string();
        let n = t.chars().count();
        let caret = normalize_outline_line(&mut t, n).unwrap();
        assert_eq!(t, "  1. 테스트\n  2. ㅇㅇ\n    a) ");
        assert_eq!(caret, t.chars().count());
        // the first marker of a memo is indented two spaces
        let mut t = "1. ".to_string();
        assert_eq!(normalize_outline_line(&mut t, 3), Some(5));
        assert_eq!(t, "  1. ");
        // already in place: nothing to do
        let mut t = "  1. a\n  2. ".to_string();
        let n = t.chars().count();
        assert!(normalize_outline_line(&mut t, n).is_none());
        // text after the marker is not a second marker
        let mut t = "  1. 가 ".to_string();
        let n = t.chars().count();
        assert!(normalize_outline_line(&mut t, n).is_none());
    }
    #[test]
    fn indentation_continues_same_level_and_returns_to_known_family() {
        assert_eq!(continuation("  1) 한글", 7), "\n  2) ");
        let mut t = "  1. 부모\n    a) 자식\n      2. ".to_string();
        let n = t.chars().count();
        normalize_outline_line(&mut t, n);
        assert!(t.ends_with("\n  2. "));
        let n = t.chars().count();
        outdent_line(&mut t, n);
        assert!(t.ends_with("\n2. "));
    }
    #[test]
    fn roman_and_alphabet_families_are_deterministic() {
        assert_eq!(next_marker("i)", "roman)"), "ii)");
        assert_eq!(next_marker("(iv)", "(roman)"), "(v)");
        assert_eq!(next_marker("h)", "alpha)"), "i)");
        assert_ne!(
            marker("i) x").unwrap().family,
            marker("h) x").unwrap().family
        );
        let roman = "  i) 하나\n  ii) 둘\n  iii) 셋\n  iv) 넷\n  v) 다섯";
        assert_eq!(continuation(roman, roman.chars().count()), "\n  vi) ");
        let alphabet = "  h) 여덟\n  i) 아홉";
        assert_eq!(continuation(alphabet, alphabet.chars().count()), "\n  j) ");
        assert_eq!(continuation("  i) 항목\n일반 문장", 17), "\n");
    }
    #[test]
    fn emoji_modifiers_take_no_room() {
        let ctx = egui::Context::default();
        let mut fonts = egui::FontDefinitions::default();
        add_invisible_font(&mut fonts);
        ctx.set_fonts(fonts);
        let _ = ctx.run(Default::default(), |_| {});
        let width = |c: &egui::Context, s: &str| {
            c.fonts(|f| {
                f.layout_no_wrap(s.into(), FontId::proportional(14.), Color32::BLACK)
                    .size()
                    .x
            })
        };
        assert!((width(&ctx, "A\u{2764}\u{FE0F}B") - width(&ctx, "A\u{2764}B")).abs() < 0.5);
        assert!((width(&ctx, "\u{1F44D}\u{1F3FB}") - width(&ctx, "\u{1F44D}")).abs() < 0.5);
        assert!((width(&ctx, "A\u{200D}B") - width(&ctx, "AB")).abs() < 0.5);
        // Without it, a box is drawn for each.
        let plain = egui::Context::default();
        let _ = plain.run(Default::default(), |_| {});
        assert!(width(&plain, "A\u{FE0F}B") > width(&plain, "AB") + 1.);
    }
    #[test]
    fn unicode_shortcuts_do_not_split_korean_chars() {
        let s = "한글단어~~ ";
        let n = s.chars().count();
        let (r, rep, style) = shortcut_substitution(s, n).unwrap();
        let mut t = s.to_owned();
        replace_chars(&mut t, r, &rep);
        assert_eq!(t, "한글단어 ");
        assert_eq!(style, Some(0..4));
        let mut s = "안녕[] ".to_string();
        let (r, rep, _) = shortcut_substitution(&s, s.chars().count()).unwrap();
        replace_chars(&mut s, r, &rep);
        assert_eq!(s, "안녕☐ ");
    }
    #[test]
    fn table_edits_keep_rectangular_shape() {
        use crate::table_edit as te;
        let mut t = Table::from_tsv("가\t나\n다\t라");
        assert!(te::insert_col(&mut t, 1));
        assert!(te::insert_row(&mut t, 2));
        assert_eq!(t.cells.len(), 3);
        assert!(t.cells.iter().all(|r| r.len() == 3));
        assert!(te::delete_cols(&mut t, 1, 1));
        assert!(te::delete_rows(&mut t, 2, 2));
        assert_eq!(t.tsv(), "가\t나\n다\t라");
    }
    #[test]
    fn redo_restores_body_and_new_typing_discards_redo() {
        let mut d = Document::default();
        d.undo.push_body(0, String::new(), vec![], 0.);
        d.tabs[0].body = "한글 입력".into();
        d.undo().unwrap();
        assert_eq!(d.tabs[0].body, "");
        d.redo().unwrap();
        assert_eq!(d.tabs[0].body, "한글 입력");
        d.undo().unwrap();
        d.undo.push_body(0, String::new(), vec![], 0.5);
        d.tabs[0].body = "다른 입력".into();
        assert!(d.redo().is_none());
    }

    #[test]
    fn undo_walks_back_through_body_and_table_steps_in_order() {
        let mut d = Document::default();
        d.tabs[0].body = "처음".into();
        d.undo.push_body(0, String::new(), vec![], 0.0);
        d.tabs[0]
            .blocks
            .push(Block::Table(Table::from_tsv("가\t나")));
        let snap = Table::from_tsv("가\t나");
        if let Block::Table(t) = &mut d.tabs[0].blocks[0] {
            t.cells[0][0].text = "바뀜".into();
        }
        d.undo.push(
            "셀 편집",
            UndoKind::Table {
                tab: 0,
                bi: 0,
                table: snap,
            },
            5.0,
        );
        d.tabs[0].body = "처음 둘째".into();
        d.undo.push_body(0, "처음".into(), vec![], 9.0);
        assert_eq!(d.undo_steps(), 3);
        assert!(d.undo().unwrap().contains("#3"));
        assert_eq!(d.tabs[0].body, "처음");
        assert!(d.undo().unwrap().contains("셀 편집"));
        assert!(matches!(&d.tabs[0].blocks[0], Block::Table(t) if t.cells[0][0].text == "가"));
        assert!(d.undo().is_some());
        assert_eq!(d.tabs[0].body, "");
        assert!(d.undo().is_none());
    }
    #[test]
    fn typing_within_a_second_is_one_undo_step_and_clones_start_empty() {
        let mut u = UndoStack::default();
        u.push_body(0, "a".into(), vec![], 1.0);
        u.push_body(0, "ab".into(), vec![], 1.4);
        u.push_body(0, "abc".into(), vec![], 3.0);
        u.push_body(1, "x".into(), vec![], 3.1);
        assert_eq!(u.len(), 3);
        assert_eq!(u.clone().len(), 0);
    }
    #[test]
    fn undoing_a_table_paste_and_a_table_delete_restores_text_and_blocks() {
        let mut d = Document::default();
        let caret = d.tabs[0].insert_table(0..0, Table::from_tsv("가\t나"));
        assert_eq!(caret, 1);
        let id = match &d.tabs[0].blocks[0] {
            Block::Table(t) => t.id,
            _ => unreachable!(),
        };
        d.undo.push(
            "표 붙여넣기",
            UndoKind::TableInserted {
                tab: 0,
                id,
                text: String::new(),
                spans: vec![],
            },
            0.0,
        );
        d.undo().unwrap();
        assert!(d.tabs[0].blocks.is_empty() && d.tabs[0].body.is_empty());
        let t = Table::from_tsv("다");
        d.undo.push(
            "표 삭제",
            UndoKind::TableDeleted {
                tab: 0,
                bi: 0,
                table: t,
                text: "본문".into(),
                spans: vec![],
            },
            1.0,
        );
        d.undo().unwrap();
        assert_eq!(d.tabs[0].blocks.len(), 1);
        assert_eq!(d.tabs[0].body, "본문");
    }
    #[test]
    fn spans_follow_unicode_insertions() {
        let mut s = vec![Span {
            range: 2..4,
            format: Format {
                strike: true,
                ..Default::default()
            },
        }];
        transform_spans("가나다라", "한글가나다라", &mut s, &Format::default());
        assert_eq!(s[0].range, 4..6);
    }
    #[test]
    fn deleting_the_first_or_last_tab_never_revives_its_content() {
        let mut d = Document::default();
        d.tabs[0].body = "첫 탭".into();
        d.add_tab();
        d.tabs[1].body = "남은 탭".into();
        d.delete_tab(0);
        assert_eq!(d.tabs[0].body, "남은 탭");
        assert_eq!(d.trash[0].body, "첫 탭");
        let legacy = d.tabs[0].body.clone();
        assert_eq!(legacy, "남은 탭");
        d.delete_tab(0);
        assert_eq!(d.tabs.len(), 1);
        assert!(d.tabs[0].body.is_empty());
        assert_eq!(d.trash[1].body, "남은 탭");
    }
    #[test]
    fn formatting_selection_does_not_change_unselected_text() {
        let mut d = Document::default();
        d.tabs[0].body = "처음 선택 나중".into();
        d.selection = 3..5;
        d.toggle_format(2);
        assert!(d.tabs[0].spans[0].format.underline);
        assert_eq!(d.tabs[0].spans[0].range, 3..5);
        let font = FontId::proportional(14.);
        let job = layout(
            &d.tabs[0].body,
            &d.tabs[0].spans,
            &Format::default(),
            font.clone(),
            font,
            Color32::BLACK,
        );
        assert_eq!(job.sections[0].format.underline, Stroke::NONE);
        assert_ne!(job.sections[3].format.underline, Stroke::NONE);
        assert_eq!(job.sections[6].format.underline, Stroke::NONE);
    }
    #[test]
    fn compact_sticker_keeps_tabs_below_two_tool_rows_inside_viewport() {
        let ctx = egui::Context::default();
        let mut doc = Document::default();
        let mut text = "짧은 메모".to_string();
        let mut bottom = 0.;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                Vec2::splat(360.),
            )),
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.inner_margin(14.))
                .show(ctx, |ui| {
                    ui.add_space(96.);
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        show(ui, &mut text, &mut doc, "sticker_test");
                        bottom = ui.cursor().top();
                    });
                });
        });
        assert!(
            bottom < 350.,
            "footer bottom {bottom} exceeds the compact sticker"
        );
    }
    #[test]
    fn search_selection_targets_the_requested_tab_without_replacing_first_body() {
        let ctx = egui::Context::default();
        let mut doc = Document::default();
        let mut body = "첫 탭 원문".to_owned();
        doc.add_tab();
        doc.tabs[1].body = "둘째 탭의 검색어".into();
        doc.active = 0;
        doc.requested_tab = Some(1);
        doc.requested_selection = Some(2..5);
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                show(ui, &mut body, &mut doc, "requested_tab_test");
            });
        });
        assert_eq!(doc.active, 1);
        assert_eq!(doc.selection, 2..5);
        assert_eq!(body, "첫 탭 원문");
        assert!(doc.requested_tab.is_none());
        assert!(doc.requested_selection.is_none());
    }
    #[test]
    fn a_footer_message_goes_away_by_itself() {
        let ctx = egui::Context::default();
        let mut doc = Document::default();
        let frame = |doc: &mut Document, time: f64| {
            let input = egui::RawInput {
                time: Some(time),
                ..Default::default()
            };
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    footer(ui, doc, "status_test", 0., false);
                });
            });
        };
        doc.status = "되돌릴 내용이 없습니다".into();
        frame(&mut doc, 1.);
        frame(&mut doc, 1. + STATUS_SECS - 0.5);
        assert_eq!(
            doc.status, "되돌릴 내용이 없습니다",
            "still within its time"
        );
        // A new message starts its own time.
        doc.status = "이미지를 복사했습니다".into();
        frame(&mut doc, 1. + STATUS_SECS + 0.5);
        assert_eq!(doc.status, "이미지를 복사했습니다");
        frame(&mut doc, 2. + 2. * STATUS_SECS);
        assert!(doc.status.is_empty(), "gone after its time");
    }
    #[test]
    fn a_right_click_finds_the_web_address_under_it() {
        let text = "참고 https://a.com/x, 끝 http://b.kr";
        let at = |s: &str| text[..text.find(s).unwrap()].chars().count();
        let a = Some("https://a.com/x".to_owned());
        assert_eq!(url_at(text, at("https") + 3), a, "inside");
        assert_eq!(url_at(text, at("https")), a, "first letter");
        assert_eq!(url_at(text, at("x,")), a, "last letter");
        assert_eq!(
            url_at(text, at(", 끝")),
            a,
            "right after it, before the comma"
        );
        assert_eq!(url_at(text, at(" 끝") + 1), None, "next word");
        assert_eq!(url_at(text, at("참고")), None, "plain word");
        let end = text.chars().count();
        assert_eq!(
            url_at(text, end),
            Some("http://b.kr".into()),
            "at the very end"
        );
        assert_eq!(url_at("", 0), None);
        assert_eq!(url_at("a  b", 2), None, "between spaces");
    }
    #[test]
    fn select_all_from_the_menu_stays_on_the_tab_shown() {
        let ctx = egui::Context::default();
        let mut doc = Document::default();
        let mut body = "첫 탭".to_owned();
        doc.add_tab();
        doc.tabs[1].body = "둘째 탭 전체".into();
        assert_eq!(doc.active, 1);
        doc.select_all();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                show(ui, &mut body, &mut doc, "select_all_test");
            });
        });
        assert_eq!(doc.active, 1);
        assert_eq!(doc.selection, 0.."둘째 탭 전체".chars().count());
    }
    #[test]
    fn saved_rich_view_keeps_the_comment_and_formats_immutable() {
        let ctx = egui::Context::default();
        let mut doc = Document::default();
        doc.tabs[0].body = "읽기 전용 댓글".into();
        doc.tabs[0].spans.push(Span {
            range: 0..2,
            format: Format {
                underline: true,
                ..Default::default()
            },
        });
        doc.tabs[0]
            .blocks
            .push(Block::Table(Table::from_tsv("가\t나\n다\t라")));
        let before = doc.tabs[0].body.clone();
        let spans = doc.tabs[0].spans.len();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                view(ui, &before, &doc, "comment_view_test");
            });
        });
        assert_eq!(doc.tabs[0].body, before);
        assert_eq!(doc.tabs[0].spans.len(), spans);
        assert!(doc.tabs[0].spans[0].format.underline);
        assert_eq!(doc.active, 0);
        assert!(matches!(&doc.tabs[0].blocks[0],Block::Table(t) if t.tsv()=="가\t나\n다\t라"));
    }
    #[test]
    fn icon_toolbar_fits_one_row_and_wraps_without_visible_text_controls() {
        for (width, max_height) in [(360., 32.), (200., 64.)] {
            let ctx = egui::Context::default();
            let mut doc = Document::default();
            let mut height = 0.;
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::new(width, 360.),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default()
                        .frame(egui::Frame::NONE.inner_margin(14.))
                        .show(ctx, |ui| {
                            height = toolbar(ui, &mut doc, Id::new("icon_toolbar_test"), true)
                                .rect
                                .height();
                        });
                },
            );
            assert!(
                height <= max_height,
                "toolbar at width {width} used {height} px"
            );
            // Only the font size number is drawn as text, as the design asks.
            assert!(
                !output.shapes.iter().any(|s| matches!(&s.shape,
                    egui::Shape::Text(t) if !t.galley.job.text.chars().all(|c| c.is_ascii_digit()))),
                "closed toolbar contains a text control"
            );
            assert_eq!(
                toolbar_height(width - 28., TOOLS.len()),
                if width == 360. { 28. } else { 58. }
            );
        }
    }
    #[test]
    fn split_footer_stays_at_panel_bottom_without_consuming_body_scroll_height() {
        let ctx = egui::Context::default();
        let mut doc = Document::default();
        let mut text = "본문\n".repeat(40);
        let mut footer_rect = egui::Rect::NOTHING;
        let mut body_rect = egui::Rect::NOTHING;
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::splat(360.),
                )),
                ..Default::default()
            },
            |ctx| {
                egui::TopBottomPanel::bottom("separate_footer")
                    .default_height(80.)
                    .frame(egui::Frame::NONE.inner_margin(8.))
                    .show(ctx, |ui| {
                        footer_rect = footer(ui, &mut doc, "split_memo", 32., false).rect;
                    });
                egui::CentralPanel::default().show(ctx, |ui| {
                    body_rect = ui.available_rect_before_wrap();
                    content(ui, &mut text, &mut doc, "split_memo");
                });
            },
        );
        assert!(
            footer_rect.bottom() >= 345. && footer_rect.bottom() <= 360.,
            "footer {footer_rect:?}, body {body_rect:?}"
        );
        assert!(
            footer_rect.height() < 100.,
            "compact footer became taller than expected: {footer_rect:?}"
        );
        assert!(
            body_rect.bottom() <= footer_rect.top(),
            "body and pinned footer overlap"
        );
        assert_eq!(text.lines().count(), 40);
    }
    #[test]
    fn footer_first_tab_delete_cannot_revive_legacy_content_in_separate_body() {
        let ctx = egui::Context::default();
        let mut doc = Document::default();
        let mut legacy = "지워진 첫 본문".to_owned();
        doc.tabs[0].body = legacy.clone();
        doc.add_tab();
        doc.tabs[1].body = "살아 있는 본문".into();
        doc.delete_tab(0);
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::TopBottomPanel::bottom("delete_footer").show(ctx, |ui| {
                footer(ui, &mut doc, "delete_memo", 0., false);
            });
            egui::CentralPanel::default().show(ctx, |ui| {
                content(ui, &mut legacy, &mut doc, "delete_memo");
            });
        });
        assert_eq!(legacy, "살아 있는 본문");
        assert_eq!(doc.tabs[0].body, "살아 있는 본문");
        assert_eq!(doc.trash[0].body, "지워진 첫 본문");
    }
    #[test]
    fn wrapped_tools_stay_inside_a_bottom_up_inline_preview() {
        for width in [220., 260., 300.] {
            let ctx = egui::Context::default();
            let mut doc = Document::default();
            let mut bounds = egui::Rect::NOTHING;
            let mut tools = egui::Rect::NOTHING;
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::new(width, 240.),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default()
                        .frame(egui::Frame::NONE.inner_margin(20.))
                        .show(ctx, |ui| {
                            ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                                bounds = ui.max_rect();
                                tools = toolbar(ui, &mut doc, Id::new("inline_tools_bounds"), true)
                                    .rect;
                            });
                        });
                },
            );
            assert!(
                bounds.expand(0.5).contains_rect(tools),
                "width={width}: tools={tools:?}, bounds={bounds:?}"
            );
        }
    }
    #[test]
    fn status_line_footer_keeps_tools_tabs_and_body_inside_active_and_passive_bounds() {
        let mut errors = vec![];
        for (active, width, base_height) in
            [(true, 360., 92.), (true, 320., 122.), (false, 360., 48.)]
        {
            let ctx = egui::Context::default();
            if let Ok(bytes) = std::fs::read("C:/Windows/Fonts/malgun.ttf") {
                let mut fonts = egui::FontDefinitions::default();
                fonts.font_data.insert(
                    "actual_windows_malgun".into(),
                    egui::FontData::from_owned(bytes).into(),
                );
                fonts
                    .families
                    .get_mut(&FontFamily::Proportional)
                    .unwrap()
                    .insert(0, "actual_windows_malgun".into());
                ctx.set_fonts(fonts);
            }
            ctx.style_mut(|s| {
                s.spacing.item_spacing = Vec2::splat(8.);
                s.spacing.button_padding = Vec2::new(9., 6.);
                s.text_styles
                    .insert(egui::TextStyle::Button, FontId::proportional(15.));
                s.text_styles
                    .insert(egui::TextStyle::Small, FontId::proportional(12.));
            });
            let mut doc = Document {
                toolbar_visible: active,
                status: "이미지 처리 안내가 긴 한 줄로 표시됩니다".into(),
                ..Default::default()
            };
            let mut legacy = "원본 본문\n".repeat(30);
            let original = legacy.clone();
            let mut bounds = egui::Rect::NOTHING;
            let mut footer_rect = egui::Rect::NOTHING;
            let mut body = egui::Rect::NOTHING;
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::new(width, 360.),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    let small_font = egui::TextStyle::Small.resolve(&ctx.style());
                    let status_height = ctx.fonts(|f| f.row_height(&small_font)) + 4.;
                    egui::TopBottomPanel::bottom("status_footer")
                        .exact_height(base_height + status_height)
                        .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(12, 6)))
                        .show(ctx, |ui| {
                            bounds = ui.max_rect().intersect(ui.clip_rect());
                            footer_rect = footer(
                                ui,
                                &mut doc,
                                "status_document",
                                if active { 32. } else { 0. },
                                false,
                            )
                            .rect;
                        });
                    egui::CentralPanel::default().show(ctx, |ui| {
                        body = ui.available_rect_before_wrap();
                        content(ui, &mut legacy, &mut doc, "status_document");
                    });
                },
            );
            if !bounds.expand(0.5).contains_rect(footer_rect) {
                errors.push(format!(
                    "active={active} width={width}: footer={footer_rect:?}, bounds={bounds:?}"
                ));
            }
            if body.bottom() > footer_rect.top() {
                errors.push(format!(
                    "active={active} width={width}: body overlaps footer"
                ));
            }
            assert_eq!(legacy, original);
        }
        assert!(errors.is_empty(), "{}", errors.join("\n"));
    }
}
#[cfg(test)]
mod inline_picture_tests {
    use super::*;
    #[test]
    fn non_fit_images_can_exceed_body_width_without_changing_fit_behavior() {
        let size = PictureSize {
            key: 'x',
            width: 1000.,
            ratio: 0.5,
            fit: false,
        };
        assert_eq!(size.at(320.), Vec2::new(1000., 500.));
        assert_eq!(
            PictureSize { fit: true, ..size }.at(320.),
            Vec2::new(316., 158.)
        );
    }
    fn picture(w: usize, h: usize) -> Picture {
        Picture {
            id: 0,
            instance: new_block_instance(),
            name: "test".into(),
            pixels: vec![255; w * h * 4],
            width: w,
            height: h,
            scale: 1.,
            fit: false,
            file: String::new(),
            texture: None,
            texture_used: 0.,
        }
    }
    #[test]
    fn pasted_picture_sits_in_the_text_and_follows_deletion() {
        let mut tab = Tab::new("본문".into());
        tab.body = "앞뒤".into();
        let caret = tab.insert_picture(1..1, picture(40, 20));
        assert_eq!(caret, 2);
        assert_eq!(tab.body.chars().count(), 3);
        assert!(is_picture(tab.body.chars().nth(1).unwrap()));
        assert!(tab.shows(&tab.blocks[0]));
        assert_eq!(strip_pictures(&tab.body), "앞뒤");
        let second = tab.insert_picture(3..3, picture(10, 10));
        assert_eq!(second, 4);
        assert_ne!(tab.body.chars().nth(1), tab.body.chars().nth(3));
        // Deleting the placeholder hides the picture from the note and its collections.
        replace_chars(&mut tab.body, 1..2, "");
        assert!(!tab.shows(&tab.blocks[0]));
        assert!(tab.shows(&tab.blocks[1]));
    }
    #[test]
    fn a_wrapped_fit_picture_is_drawn_below_the_text_before_it() {
        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |_| {});
        let mut tab = Tab::new("본문".into());
        tab.body = "가나다라마바사아자차카타파하".into();
        let at = tab.body.chars().count();
        let mut p = picture(400, 200);
        p.fit = true;
        tab.insert_picture(at..at, p);
        let font = FontId::proportional(14.);
        let ratio = ctx.fonts(|f| f.glyph_width(&FontId::proportional(100.), '\u{3000}')) / 100.;
        let mut job = layout(
            &tab.body,
            &[],
            &Format::default(),
            font.clone(),
            font,
            Color32::BLACK,
        );
        let wrap = 200.;
        job.wrap.max_width = wrap;
        let sizes = picture_sizes(&tab, 1.);
        inline_pictures(&mut job, &sizes, wrap, ratio);
        let galley = ctx.fonts(|f| f.layout_job(job));
        assert!(galley.rows.len() >= 2, "the picture wraps onto its own row");
        let text_bottom = galley.rows[0].rect.max.y;
        let rect = picture_rect(&galley, at, sizes[0].at(wrap));
        assert!(
            rect.min.y >= text_bottom - 0.5,
            "{rect:?} overlaps text ending at {text_bottom}"
        );
    }
    #[test]
    fn layout_reserves_the_picture_size_and_keeps_byte_ranges_valid() {
        let mut tab = Tab::new("본문".into());
        tab.body = "가".into();
        tab.insert_picture(1..1, picture(80, 40));
        tab.body.push('나');
        let sizes = picture_sizes(&tab, 1.);
        let font = FontId::proportional(14.);
        let mut job = layout(
            &tab.body,
            &[],
            &Format::default(),
            font.clone(),
            font,
            Color32::BLACK,
        );
        inline_pictures(&mut job, &sizes, 300., 1.);
        assert_eq!(job.text, "가\u{3000}나");
        for s in &job.sections {
            assert!(job.text.get(s.byte_range.clone()).is_some());
        }
        let image = &job.sections[1].format;
        assert_eq!(image.font_id.size, 80.);
        assert_eq!(image.line_height, Some(44.));
        assert_eq!(image.color, Color32::TRANSPARENT);
        // Fit-to-width follows the wrap width.
        if let Block::Picture(p) = &mut tab.blocks[0] {
            p.fit = true;
        }
        let fit = picture_sizes(&tab, 1.)[0].at(201.);
        assert_eq!(fit, Vec2::new(196., 98.));
    }
    #[test]
    fn plain_text_and_title_inference_skip_picture_placeholders() {
        let mut doc = Document::default();
        doc.tabs[0].insert_picture(0..0, picture(4, 4));
        let body = doc.tabs[0].body.clone() + "제목 줄\n본문";
        assert_eq!(doc.plain_text(&body), "제목 줄\n본문");
        let mut title = String::new();
        crate::desktop::infer_title(&mut title, &body);
        assert_eq!(title, "제목 줄");
        assert_eq!(body.chars().filter(|c| is_picture(*c)).count(), 1);
        assert!(body.ends_with("본문"));
    }
}
#[cfg(test)]
mod inline_table_tests {
    use super::tests::CLIPBOARD_HTML;
    use super::*;
    #[test]
    fn pasted_table_sits_in_the_text_and_follows_deletion() {
        let mut tab = Tab::new("본문".into());
        tab.body = "앞뒤".into();
        let table = Table::new(2, 2);
        let caret = tab.insert_table(1..1, table);
        assert_eq!(caret, 2);
        assert_eq!(tab.body.chars().count(), 3);
        assert!(is_picture(tab.body.chars().nth(1).unwrap()));
        assert!(tab.shows(&tab.blocks[0]));
        assert_eq!(strip_pictures(&tab.body), "앞뒤");
        // Deleting the placeholder hides the table from the note, the same
        // way it would a picture — it is removed with the surrounding text,
        // not left behind as an orphaned block.
        replace_chars(&mut tab.body, 1..2, "");
        assert!(!tab.shows(&tab.blocks[0]));
    }
    fn ui_frame(
        ctx: &egui::Context,
        doc: &mut Document,
        body: &mut String,
        events: Vec<egui::Event>,
        time: f64,
    ) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                Vec2::new(700., 500.),
            )),
            events,
            time: Some(time),
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                show(ui, body, doc, "tbl_ui_test");
            });
        });
    }
    /// A memo whose body has the keyboard focus, after two frames.
    fn focused_memo(body: &str) -> (egui::Context, Document, String) {
        let ctx = egui::Context::default();
        let mut doc = Document::default();
        let mut body = body.to_owned();
        let end = body.chars().count();
        doc.requested_selection = Some(end..end);
        ui_frame(&ctx, &mut doc, &mut body, vec![], 0.);
        ui_frame(&ctx, &mut doc, &mut body, vec![], 0.2);
        (ctx, doc, body)
    }
    fn tables_in(doc: &Document) -> usize {
        doc.tabs[0]
            .blocks
            .iter()
            .filter(|b| matches!(b, Block::Table(t) if doc.tabs[0].shows(b) && t.positioned))
            .count()
    }
    #[test]
    fn text_around_a_copied_table_keeps_its_order() {
        let (ctx, mut doc, mut body) = focused_memo("메모 ");
        CLIPBOARD_HTML.with(|h| {
            *h.borrow_mut() = Some(
                "<p>앞 문단</p><table><tr><td>가</td><td>나</td></tr>\
                 <tr><td>다</td><td>라</td></tr></table><p>뒤 문단</p>"
                    .into(),
            )
        });
        let plain = "앞 문단\n가\t나\n다\t라\n뒤 문단".to_owned();
        ui_frame(
            &ctx,
            &mut doc,
            &mut body,
            vec![egui::Event::Paste(plain)],
            0.4,
        );
        CLIPBOARD_HTML.with(|h| *h.borrow_mut() = None);
        assert_eq!(tables_in(&doc), 1, "one table");
        let text = strip_pictures(&doc.tabs[0].body);
        assert_eq!(
            text, "메모 앞 문단\n\n뒤 문단",
            "text before and after, in order"
        );
        let at = doc.tabs[0].body.chars().position(is_picture).unwrap();
        let before: String = doc.tabs[0].body.chars().take(at).collect();
        assert!(before.ends_with("앞 문단\n"));
        doc.undo().unwrap();
        assert_eq!(doc.tabs[0].body, "메모 ");
    }
    #[test]
    fn a_paste_after_a_lost_ime_commit_still_makes_a_table() {
        let (ctx, mut doc, mut body) = focused_memo("");
        // A composition that never got its commit (the memo lost focus).
        doc.ime = true;
        let tsv = "가\t나\n다\t라".to_owned();
        ui_frame(
            &ctx,
            &mut doc,
            &mut body,
            vec![egui::Event::Paste(tsv)],
            0.4,
        );
        assert_eq!(tables_in(&doc), 1);
        assert!(!doc.ime);
    }
    #[test]
    fn a_paste_in_the_same_frame_as_an_ime_commit_waits_a_frame() {
        let (ctx, mut doc, mut body) = focused_memo("");
        let tsv = "가\t나\n다\t라".to_owned();
        ui_frame(
            &ctx,
            &mut doc,
            &mut body,
            vec![
                egui::Event::Ime(egui::ImeEvent::Commit("한".into())),
                egui::Event::Paste(tsv),
            ],
            0.4,
        );
        ui_frame(&ctx, &mut doc, &mut body, vec![], 0.5);
        assert_eq!(tables_in(&doc), 1, "the held paste became a table");
        assert!(
            !strip_pictures(&doc.tabs[0].body).contains('\t'),
            "no tab separated text"
        );
    }
    #[test]
    fn tables_paste_as_tab_separated_text_when_chosen() {
        let (ctx, mut doc, mut body) = focused_memo("");
        set_paste_tables_as_text(true);
        let tsv = "가\t나\n다\t라".to_owned();
        ui_frame(
            &ctx,
            &mut doc,
            &mut body,
            vec![egui::Event::Paste(tsv)],
            0.4,
        );
        set_paste_tables_as_text(false);
        assert_eq!(tables_in(&doc), 0);
        assert!(body.contains("가\t나"), "{body:?}");
    }
    #[test]
    fn body_selection_survives_menu_copy_event() {
        let ctx = egui::Context::default();
        let mut doc = Document::default();
        let mut body = "클립보드 ABC".to_owned();
        doc.requested_selection = Some(0..body.chars().count());
        ui_frame(&ctx, &mut doc, &mut body, vec![], 0.);
        ui_frame(&ctx, &mut doc, &mut body, vec![], 0.2);
        assert_eq!(doc.selection, 0..body.chars().count());
        ui_frame(&ctx, &mut doc, &mut body, vec![egui::Event::Copy], 0.4);
        assert_eq!(doc.selection, 0..body.chars().count());
        assert_eq!(body, "클립보드 ABC");
    }

    #[test]
    fn popup_can_move_outside_when_the_body_has_no_free_space() {
        let rect = |a: (f32, f32), b: (f32, f32)| {
            egui::Rect::from_min_max(egui::pos2(a.0, a.1), egui::pos2(b.0, b.1))
        };
        let sticker = rect((500., 120.), (800., 400.));
        let body = rect((514., 205.), (786., 362.));
        let selection = rect((514., 210.), (786., 358.));
        let work = rect((0., 0.), (802., 450.));
        let size = Vec2::new(232., 86.);
        let at = selection_popup_position(sticker, body, selection, selection.center(), work, size)
            .unwrap();
        let popup = egui::Rect::from_min_size(at, size);
        assert!(work.contains_rect(popup));
        assert!(!sticker.intersects(popup));
        assert!(popup.right() < sticker.left());
    }

    #[test]
    fn undo_and_redo_walk_through_color_and_highlighter_before_typing() {
        let mut doc = Document::default();
        doc.undo.push_body(0, String::new(), vec![], 0.);
        doc.tabs[0].body = "색연습".into();
        doc.selection = 1..3;

        doc.paint_selection_with_history(false, 1, 0.1);
        doc.paint_selection_with_history(true, 2, 0.2);
        let styled = doc.tabs[0].spans.clone();
        assert_eq!(
            (
                styled.last().unwrap().format.color,
                styled.last().unwrap().format.highlight
            ),
            (1, 2)
        );
        doc.undo().unwrap();
        assert_eq!(doc.tabs[0].body, "색연습");
        assert_eq!(doc.tabs[0].spans.last().unwrap().format.highlight, 0);
        doc.undo().unwrap();
        assert_eq!(doc.tabs[0].body, "색연습");
        assert!(doc.tabs[0].spans.is_empty());
        doc.redo().unwrap();
        doc.redo().unwrap();
        assert_eq!(doc.tabs[0].body, "색연습");
        assert_eq!(doc.tabs[0].spans, styled);
    }

    #[test]
    fn sticker_selection_popup_applies_color_and_keeps_copy_and_cut_on_the_selection() {
        let ctx = egui::Context::default();
        let mut doc = Document::default();
        let mut body = "메모 ABC".to_owned();
        let end = body.chars().count();
        doc.requested_selection = Some(end..end);
        let mut time = 0.;
        let mut frame = |events| {
            time += 0.1;
            ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::new(360., 270.),
                    )),
                    time: Some(time),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let host = Id::new(HOST_MENU);
                        ui.data_mut(|d| {
                            d.insert_temp(host, true);
                            d.remove::<std::sync::Arc<Vec<egui::Rect>>>(
                                Id::new("eframe_stable_hover_rects").with(egui::ViewportId::ROOT),
                            );
                        });
                        content(ui, &mut body, &mut doc, "selection_popup_test");
                        ui.data_mut(|d| d.remove::<bool>(host));
                    });
                },
            )
        };
        let controls = || {
            ctx.data(|d| {
                d.get_temp::<std::sync::Arc<Vec<egui::Rect>>>(
                    Id::new("eframe_stable_hover_rects").with(egui::ViewportId::ROOT),
                )
                .unwrap()
            })
        };
        let shift_left = || key(Key::ArrowLeft, Modifiers::SHIFT);
        let _ = frame(vec![]);
        let _ = frame(vec![shift_left()]);
        let _ = frame(vec![]);
        for events in click(controls()[3].center()) {
            let _ = frame(events);
        }

        let _ = frame(vec![shift_left()]);
        let _ = frame(vec![]);
        for events in click(controls()[1].center()) {
            let _ = frame(events);
        }
        let copied = frame(vec![]);
        assert!(copied
            .platform_output
            .commands
            .iter()
            .any(|c| matches!(c, egui::OutputCommand::CopyText(text) if text == "BC")));

        let _ = frame(vec![shift_left()]);
        let _ = frame(vec![]);
        for events in click(controls()[0].center()) {
            let _ = frame(events);
        }
        let cut = frame(vec![]);
        assert!(cut
            .platform_output
            .commands
            .iter()
            .any(|c| matches!(c, egui::OutputCommand::CopyText(text) if text == "ABC")));
        let _ = frame(vec![key(Key::Z, Modifiers::CTRL)]);
        let _ = frame(vec![key(Key::End, Modifiers::NONE)]);
        let end_of_line = LAST_CARET.with(|r| r.get().unwrap().1.center());
        let from = end_of_line - Vec2::new(2., 0.);
        let to = from - Vec2::new(36., 0.);
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            pressed,
            button: egui::PointerButton::Primary,
            modifiers: Modifiers::NONE,
        };
        let _ = frame(vec![egui::Event::PointerMoved(from), button(from, true)]);
        let _ = frame(vec![egui::Event::PointerMoved(to)]);
        assert!(!ctx
            .data(|d| d.get_temp::<std::sync::Arc<Vec<egui::Rect>>>(
                Id::new("eframe_stable_hover_rects").with(egui::ViewportId::ROOT),
            ))
            .is_some());
        let _ = frame(vec![button(to, false)]);
        assert_eq!(controls().len(), 15);
        assert_eq!(body, "메모 ABC");
        assert_eq!(doc.tabs[0].spans.last().unwrap().range, end - 1..end);
        assert_eq!(doc.tabs[0].spans.last().unwrap().format.color, 1);
    }

    fn table_area(_ctx: &egui::Context) -> egui::Rect {
        LAST_TABLE_RECT
            .with(|r| r.get())
            .expect("the inline table was drawn")
    }
    fn key(k: Key, mods: Modifiers) -> egui::Event {
        egui::Event::Key {
            key: k,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: mods,
        }
    }
    fn click(p: egui::Pos2) -> Vec<Vec<egui::Event>> {
        let b = |pressed| egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        vec![
            vec![egui::Event::PointerMoved(p)],
            vec![b(true)],
            vec![b(false)],
        ]
    }
    #[test]
    fn table_cells_select_edit_range_delete_and_undo_through_the_ui() {
        let ctx = egui::Context::default();
        let mut doc = Document::default();
        let mut table = Table::new(3, 3);
        table.cells[1][1].text = "원래".into();
        table.cells[0][0].text = "가".into();
        doc.tabs[0].insert_table(0..0, table);
        let mut body = doc.tabs[0].body.clone();
        let mut t = 0.0;
        let mut run = |doc: &mut Document, body: &mut String, ev: Vec<egui::Event>| {
            t += 0.2;
            ui_frame(&ctx, doc, body, ev, t);
        };
        for _ in 0..3 {
            run(&mut doc, &mut body, vec![]);
        }
        let area = table_area(&ctx);
        let cell =
            |r: usize, c: usize| area.min + Vec2::new(c as f32 * 121. + 60., r as f32 * 33. + 16.);
        // click selects (no edit yet)
        for ev in click(cell(1, 1)) {
            run(&mut doc, &mut body, ev);
        }
        let s = doc.tsel.clone().expect("a click selects a cell");
        assert_eq!((s.head, s.editing), ((1, 1), false));
        // typing replaces the content and edits
        run(&mut doc, &mut body, vec![egui::Event::Text("새".into())]);
        assert!(doc.tsel.as_ref().unwrap().editing);
        run(&mut doc, &mut body, vec![]);
        let text = |doc: &Document, r: usize, c: usize| match &doc.tabs[0].blocks[0] {
            Block::Table(t) => t.cells[r][c].text.clone(),
            _ => unreachable!(),
        };
        assert_eq!(text(&doc, 1, 1), "새");
        // Enter commits and moves down; one undo step was recorded
        run(&mut doc, &mut body, vec![key(Key::Enter, Modifiers::NONE)]);
        run(&mut doc, &mut body, vec![]);
        let s = doc.tsel.clone().unwrap();
        assert_eq!((s.head, s.editing), ((2, 1), false));
        assert_eq!(doc.undo_steps(), 1);
        // Ctrl+Z puts the old text back
        run(&mut doc, &mut body, vec![key(Key::Z, Modifiers::CTRL)]);
        assert_eq!(text(&doc, 1, 1), "원래");
        assert!(doc.status.contains("셀 편집"), "{}", doc.status);
        assert_eq!(doc.undo_steps(), 0);
        // drag a range, then Delete clears it (one undo step)
        let (a, b) = (cell(0, 0), cell(1, 1));
        let press = egui::Event::PointerButton {
            pos: a,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        };
        let release = egui::Event::PointerButton {
            pos: b,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        };
        run(&mut doc, &mut body, vec![egui::Event::PointerMoved(a)]);
        run(&mut doc, &mut body, vec![press]);
        run(&mut doc, &mut body, vec![egui::Event::PointerMoved(b)]);
        run(&mut doc, &mut body, vec![]);
        run(&mut doc, &mut body, vec![release]);
        let s = doc.tsel.clone().unwrap();
        assert_eq!((s.anchor, s.head), ((0, 0), (1, 1)));
        run(&mut doc, &mut body, vec![key(Key::Delete, Modifiers::NONE)]);
        run(&mut doc, &mut body, vec![]);
        assert_eq!(
            (text(&doc, 0, 0), text(&doc, 1, 1)),
            (String::new(), String::new())
        );
        assert_eq!(doc.undo_steps(), 1);
        run(&mut doc, &mut body, vec![key(Key::Z, Modifiers::CTRL)]);
        assert_eq!(
            (text(&doc, 0, 0), text(&doc, 1, 1)),
            ("가".to_owned(), "원래".to_owned())
        );
    }
    #[test]
    fn undo_follows_one_order_across_body_typing_and_table_edits() {
        let ctx = egui::Context::default();
        let mut doc = Document::default();
        doc.tabs[0].insert_table(0..0, Table::new(2, 2));
        let mut body = doc.tabs[0].body.clone();
        let placeholder_only = body.clone();
        let mut t = 0.0;
        let mut run = |doc: &mut Document, body: &mut String, ev: Vec<egui::Event>| {
            t += 0.2;
            ui_frame(&ctx, doc, body, ev, t);
        };
        for _ in 0..3 {
            run(&mut doc, &mut body, vec![]);
        }
        // 1. type in the body (after the table placeholder)
        doc.requested_selection = Some(1..1);
        run(&mut doc, &mut body, vec![]);
        run(&mut doc, &mut body, vec![egui::Event::Text("본문".into())]);
        run(&mut doc, &mut body, vec![]);
        assert!(doc.tabs[0].body.ends_with("본문"));
        let after_body = doc.undo_steps();
        assert!(after_body >= 1);
        // 2. edit a table cell
        let area = table_area(&ctx);
        let c00 = area.min + Vec2::new(60., 16.);
        for ev in click(c00) {
            run(&mut doc, &mut body, ev);
        }
        run(&mut doc, &mut body, vec![egui::Event::Text("셀".into())]);
        run(&mut doc, &mut body, vec![]);
        run(&mut doc, &mut body, vec![key(Key::Enter, Modifiers::NONE)]);
        run(&mut doc, &mut body, vec![]);
        assert_eq!(doc.undo_steps(), after_body + 1);
        // 3. undo: the cell first, then the body text
        run(&mut doc, &mut body, vec![key(Key::Z, Modifiers::CTRL)]);
        assert!(matches!(&doc.tabs[0].blocks[0], Block::Table(t) if t.cells[0][0].text.is_empty()));
        assert!(
            doc.tabs[0].body.ends_with("본문"),
            "the body is still as typed"
        );
        run(&mut doc, &mut body, vec![key(Key::Z, Modifiers::CTRL)]);
        assert_eq!(doc.tabs[0].body, placeholder_only);
        assert!(doc.status.contains("본문 편집"), "{}", doc.status);
    }
    /// `cargo test table_frame_time -- --ignored --nocapture` (use --release for real numbers)
    #[test]
    #[ignore]
    fn table_frame_time() {
        let ctx = egui::Context::default();
        let mut doc = Document::default();
        let mut table = Table::new(100, 20);
        for r in 0..100 {
            for c in 0..20 {
                table.cells[r][c].text = format!("셀 {r}-{c} 한글 text");
            }
        }
        doc.tabs[0].insert_table(0..0, table);
        let mut body = doc.tabs[0].body.clone();
        let mut times = vec![];
        for i in 0..60 {
            let t = std::time::Instant::now();
            ui_frame(&ctx, &mut doc, &mut body, vec![], i as f64 * 0.016);
            times.push(t.elapsed().as_secs_f64() * 1000.);
        }
        let first = times[0];
        let mut rest = times[5..].to_vec();
        rest.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mean = rest.iter().sum::<f64>() / rest.len() as f64;
        println!(
            "100x20 table: first {first:.2} ms, repeat mean {mean:.2} ms, median {:.2} ms, p95 {:.2} ms, max {:.2} ms",
            rest[rest.len() / 2],
            rest[rest.len() * 95 / 100],
            rest[rest.len() - 1]
        );
    }
    /// Reads the real clipboard (an Excel range) and prints how it was read.
    #[test]
    #[ignore]
    fn probe_real_clipboard_table() {
        let html = match crate::clipboard_html::read() {
            Some(h) => Ok(h),
            None => crate::clipboard_copy::Clipboard::new()
                .unwrap()
                .get()
                .html(),
        };
        match &html {
            Ok(h) => println!(
                "ARBOARD HTML LEN {} START: {:?}",
                h.len(),
                &h[..h.len().min(300)]
            ),
            Err(e) => println!("ARBOARD HTML ERR {e}"),
        }
        if let Ok(h) = html {
            match Table::from_html(&h) {
                None => println!("PARSE: None"),
                Some(t) => {
                    println!("PARSE: {}x{}", t.cells.len(), t.cells[0].len());
                    for (r, row) in t.cells.iter().enumerate() {
                        for (c, cell) in row.iter().enumerate() {
                            if cell.covered {
                                println!("  [{r},{c}] covered");
                            } else {
                                println!("  [{r},{c}] {}x{} {:?} fill {:?} color {:?} align {:?} bold {} italic {} underline {}", cell.row_span, cell.col_span, cell.text, cell.fill, cell.text_color, cell.align, cell.format.bold, cell.format.italic, cell.format.underline);
                            }
                        }
                    }
                    println!("widths {:?}", t.col_widths);
                    println!("valid {}", crate::table_edit::is_valid(&t));
                }
            }
        }
        let text = crate::clipboard_copy::Clipboard::new()
            .unwrap()
            .get_text()
            .unwrap_or_default();
        let grid = crate::table_edit::parse_tsv(&text);
        println!(
            "TSV GRID {}x{}: {:?}",
            grid.len(),
            grid.iter().map(Vec::len).max().unwrap_or(0),
            grid
        );
    }
    /// Real clipboard: paste an Excel range into the memo, then copy the
    /// whole memo table back (leaves the memo's Excel-format data on the clipboard).
    #[test]
    #[ignore]
    fn real_clipboard_paste_into_memo_then_copy_back() {
        let pasted_text = crate::clipboard_copy::Clipboard::new()
            .unwrap()
            .get_text()
            .unwrap();
        let ctx = egui::Context::default();
        let mut doc = Document::default();
        let mut body = String::new();
        let mut t = 0.0;
        let mut run = |doc: &mut Document, body: &mut String, ev: Vec<egui::Event>| {
            t += 0.2;
            ui_frame(&ctx, doc, body, ev, t);
        };
        run(&mut doc, &mut body, vec![]);
        doc.requested_selection = Some(0..0);
        run(&mut doc, &mut body, vec![]);
        run(&mut doc, &mut body, vec![egui::Event::Paste(pasted_text)]);
        for _ in 0..3 {
            run(&mut doc, &mut body, vec![]);
        }
        let table = match doc.tabs[0].blocks.first() {
            Some(Block::Table(t)) => t.clone(),
            _ => panic!("no table was pasted into the memo"),
        };
        println!("MEMO TABLE {}x{}", table.cells.len(), table.cells[0].len());
        assert!(crate::table_edit::is_valid(&table));
        assert_eq!(
            (table.cells[0][0].row_span, table.cells[0][0].col_span),
            (3, 1)
        );
        assert_eq!(
            table.cells[0][0].text,
            "병합1\n 줄바꿈 앞에 띄어쓰기 1칸\n  띄어쓰기 2칸"
        );
        assert_eq!(table.cells[2][1].col_span, 2);
        assert_eq!(
            (table.cells[3][2].row_span, table.cells[3][2].col_span),
            (2, 2)
        );
        assert_eq!(table.cells[6][0].col_span, 4);
        // select everything by dragging, then Ctrl+C
        let area = table_area(&ctx);
        let xs = [0., 266., 339., 412.];
        let ws = [265., 72., 72., 72.];
        let cell =
            |r: usize, c: usize| area.min + Vec2::new(xs[c] + ws[c] / 2., r as f32 * 33. + 16.);
        let (a, b) = (cell(0, 0), cell(6, 3));
        let btn = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        run(&mut doc, &mut body, vec![egui::Event::PointerMoved(a)]);
        run(&mut doc, &mut body, vec![btn(a, true)]);
        run(&mut doc, &mut body, vec![egui::Event::PointerMoved(b)]);
        run(&mut doc, &mut body, vec![]);
        run(&mut doc, &mut body, vec![btn(b, false)]);
        let s = doc.tsel.clone().expect("range selected");
        println!("SELECTION {:?} -> {:?}", s.anchor, s.head);
        run(&mut doc, &mut body, vec![egui::Event::Copy]);
        println!("STATUS {}", doc.status);
        assert!(doc.status.contains("복사"), "{}", doc.status);
    }
    /// Puts what Excel really put on the clipboard for the user's merged range
    /// back on it: Excel's HTML fragment (arboard hands over only the part
    /// after `<table>`) and its tab separated text.
    #[test]
    #[ignore]
    fn seed_excel_like_clipboard() {
        let fragment = "\r\n <col width=265 style='mso-width-source:userset;mso-width-alt:8464;width:198pt'>\r\n <col width=72 span=3 style='width:54pt'>\r\n <tr height=23 style='height:16.9pt'>\r\n  <td rowspan=3 height=69 class=xl81 width=265 style='border-bottom:.5pt solid black;\r\n  height:50.7pt;width:198pt'>병합1<br />\r\n    <span style='mso-spacerun:yes'>&nbsp;</span>줄바꿈 앞에 띄어쓰기 1칸<br />\r\n    <span style='mso-spacerun:yes'>&nbsp; </span>띄어쓰기 2칸</td>\r\n  <td class=xl65 width=72 style='width:54pt'>표 복사 테스트</td>\r\n  <td class=xl65 width=72 style='width:54pt'>표 복사 테스트</td>\r\n  <td rowspan=2 class=xl75 width=72 style='border-bottom:.5pt solid black;\r\n  width:54pt'>병합3</td>\r\n </tr>\r\n <tr height=23 style='height:16.9pt'>\r\n  <td height=23 class=xl67 style='height:16.9pt'>표 복사 테스트</td>\r\n  <td class=xl67>표 복사 테스트</td>\r\n </tr>\r\n <tr height=23 style='height:16.9pt'>\r\n  <td colspan=2 height=23 class=xl73 style='border-right:.5pt solid black;\r\n  height:16.9pt;border-left:none'>병합2</td>\r\n  <td class=xl67>표 복사 테스트</td>\r\n </tr>\r\n <tr height=23 style='height:16.9pt'>\r\n  <td height=23 class=xl66 style='height:16.9pt'>표 복사 테스트</td>\r\n  <td class=xl67>표 복사 테스트</td>\r\n  <td colspan=2 rowspan=2 class=xl77 style='border-right:.5pt solid black;\r\n  border-bottom:.5pt solid black'>병합4</td>\r\n </tr>\r\n <tr height=23 style='height:16.9pt'>\r\n  <td height=23 class=xl66 style='height:16.9pt'>표 복사 테스트</td>\r\n  <td class=xl67>표 복사 테스트</td>\r\n </tr>\r\n <tr height=23 style='height:16.9pt'>\r\n  <td height=23 class=xl66 style='height:16.9pt'>표 복사 테스트</td>\r\n  <td class=xl67>표 복사 테스트</td>\r\n  <td class=xl67>표 복사 테스트</td>\r\n  <td class=xl67>표 복사 테스트</td>\r\n </tr>\r\n <tr height=23 style='height:16.9pt'>\r\n  <td colspan=4 height=23 class=xl68 style='border-right:.5pt solid black;\r\n  height:16.9pt'>병합5</td>\r\n </tr>\r\n";
        let tsv = "\"병합1\n 줄바꿈 앞에 띄어쓰기 1칸\n  띄어쓰기 2칸\"\t표 복사 테스트\t표 복사 테스트\t병합3\r\n\t표 복사 테스트\t표 복사 테스트\t\r\n\t병합2\t\t표 복사 테스트\r\n표 복사 테스트\t표 복사 테스트\t병합4\t\r\n표 복사 테스트\t표 복사 테스트\t\t\r\n표 복사 테스트\t표 복사 테스트\t표 복사 테스트\t표 복사 테스트\r\n병합5\t\t\t\r\n";
        crate::clipboard_copy::Clipboard::new()
            .unwrap()
            .set_html(fragment, Some(tsv))
            .unwrap();
    }
    #[test]
    #[ignore]
    fn probe_halign_galley() {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            for a in [egui::Align::LEFT, egui::Align::Center, egui::Align::RIGHT] {
                let mut job = layout(
                    "가나다\n가",
                    &[],
                    &Format::default(),
                    FontId::proportional(14.),
                    FontId::proportional(14.),
                    Color32::BLACK,
                );
                job.wrap.max_width = 200.;
                job.halign = a;
                let g = ctx.fonts(|f| f.layout_job(job));
                println!(
                    "ALIGN {:?} rect {:?} row0 {:?} row1 {:?}",
                    a, g.rect, g.rows[0].rect, g.rows[1].rect
                );
            }
        });
    }
    #[test]
    fn table_draws_black_borders_fill_text_color_and_alignment() {
        let ctx = egui::Context::default();
        ctx.set_visuals(egui::Visuals::light());
        let mut doc = Document::default();
        let mut table = Table::new(1, 3);
        table.col_widths = vec![200., 200., 200.];
        for c in 0..3 {
            table.cells[0][c].text = "가".into();
        }
        table.cells[0][0].fill = Some([255, 255, 0]);
        table.cells[0][0].text_color = Some([255, 0, 0]);
        table.cells[0][1].align = CellAlign::Center;
        table.cells[0][2].align = CellAlign::Right;
        doc.tabs[0].insert_table(0..0, table);
        let mut body = doc.tabs[0].body.clone();
        let mut shapes = vec![];
        for i in 0..3 {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(800., 400.),
                )),
                time: Some(i as f64 * 0.1),
                ..Default::default()
            };
            let out = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    show(ui, &mut body, &mut doc, "tbl_look_test");
                });
            });
            shapes = out.shapes;
        }
        let mut black = 0;
        let mut yellow = false;
        let mut text_x = vec![];
        let mut red_text = false;
        for cs in &shapes {
            let mut stack = vec![&cs.shape];
            while let Some(sh) = stack.pop() {
                match sh {
                    egui::Shape::Vec(v) => stack.extend(v.iter()),
                    egui::Shape::Rect(r) => {
                        if r.stroke.color == Color32::BLACK && r.stroke.width >= 1.0 {
                            black += 1;
                        }
                        if r.fill == Color32::from_rgb(255, 255, 0) {
                            yellow = true;
                        }
                    }
                    egui::Shape::Text(t) if t.galley.job.text == "가" => {
                        text_x.push(t.pos.x);
                        if t.galley.job.sections[0].format.color == Color32::from_rgb(255, 0, 0) {
                            red_text = true;
                        }
                    }
                    _ => {}
                }
            }
        }
        assert!(black >= 3, "one black border per cell, saw {black}");
        assert!(yellow, "the fill is painted");
        assert!(red_text, "the text color is used");
        text_x.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(text_x.len(), 3, "{text_x:?}");
        // left cell text sits at the left, center in the middle, right at the right edge
        let (l, c, r) = (text_x[0], text_x[1], text_x[2]);
        // cells are 200 wide plus 1 of spacing; the anchor of a centered text
        // is half the cell's inner width (188) further, a right one the full width
        assert!((c - l - (201. + 94.)).abs() < 3., "center {c} vs left {l}");
        assert!(
            (r - c - 201.).abs() < 100. && r > c,
            "right {r} vs center {c}"
        );
        assert!((r - l - (402. + 188.)).abs() < 3., "right {r} vs left {l}");
    }
    #[test]
    fn typing_a_numbered_outline_with_hangul_follows_the_levels() {
        let ctx = egui::Context::default();
        let mut doc = Document::default();
        let mut body = String::new();
        let mut t = 0.0;
        let mut run = |doc: &mut Document, body: &mut String, ev: Vec<egui::Event>| {
            t += 0.2;
            ui_frame(&ctx, doc, body, ev, t);
        };
        run(&mut doc, &mut body, vec![]);
        doc.requested_selection = Some(0..0);
        run(&mut doc, &mut body, vec![]);
        let text = |e: &str| egui::Event::Text(e.into());
        let enter = || key(Key::Enter, Modifiers::NONE);
        let pre = |s: &str| egui::Event::Ime(egui::ImeEvent::Preedit(s.into()));
        let commit = |s: &str| egui::Event::Ime(egui::ImeEvent::Commit(s.into()));
        // "1. " is indented at once
        for e in ["1", ".", " "] {
            run(&mut doc, &mut body, vec![text(e)]);
        }
        assert_eq!(doc.tabs[0].body, "  1. ");
        // Hangul with a real IME: composing, committing a syllable at a time
        run(
            &mut doc,
            &mut body,
            vec![egui::Event::Ime(egui::ImeEvent::Enabled)],
        );
        for (p, c) in [("ㅌ", "테"), ("ㅅ", "스")] {
            run(&mut doc, &mut body, vec![pre(p)]);
            run(&mut doc, &mut body, vec![pre(c)]);
            run(&mut doc, &mut body, vec![pre(""), commit(c)]);
        }
        run(&mut doc, &mut body, vec![pre("ㅌ")]);
        run(&mut doc, &mut body, vec![pre("트")]);
        // Enter while the last syllable is still being composed: the IME commits it
        // and the Enter key arrives in the same frame
        run(&mut doc, &mut body, vec![pre(""), commit("트"), enter()]);
        run(&mut doc, &mut body, vec![]);
        assert_eq!(
            doc.tabs[0].body, "  1. 테스트\n  2. ",
            "{:?}",
            doc.tabs[0].body
        );
        run(&mut doc, &mut body, vec![text("ㅇㅇ")]);
        run(&mut doc, &mut body, vec![enter()]);
        assert_eq!(doc.tabs[0].body, "  1. 테스트\n  2. ㅇㅇ\n  3. ");
        // a marker typed on the empty item replaces it, one level deeper
        for e in ["a", ")", " "] {
            run(&mut doc, &mut body, vec![text(e)]);
        }
        assert_eq!(
            doc.tabs[0].body, "  1. 테스트\n  2. ㅇㅇ\n    a) ",
            "{:?}",
            doc.tabs[0].body
        );
        // content, then Enter: the same level
        run(&mut doc, &mut body, vec![text("내용")]);
        run(&mut doc, &mut body, vec![enter()]);
        assert!(
            doc.tabs[0].body.ends_with("    a) 내용\n    b) "),
            "{:?}",
            doc.tabs[0].body
        );
        // Enter on the empty item: the next number of the level above
        run(&mut doc, &mut body, vec![enter()]);
        assert_eq!(
            doc.tabs[0].body,
            "  1. 테스트\n  2. ㅇㅇ\n    a) 내용\n  3. "
        );
        // the caret is at the end, typing continues there
        run(&mut doc, &mut body, vec![text("끝")]);
        assert!(
            doc.tabs[0].body.ends_with("  3. 끝"),
            "{:?}",
            doc.tabs[0].body
        );
    }
    #[test]
    fn a_cancelled_composition_does_not_switch_the_list_helpers_off() {
        let ctx = egui::Context::default();
        let mut doc = Document::default();
        let mut body = String::new();
        let mut t = 0.0;
        let mut run = |doc: &mut Document, body: &mut String, ev: Vec<egui::Event>| {
            t += 0.2;
            ui_frame(&ctx, doc, body, ev, t);
        };
        run(&mut doc, &mut body, vec![]);
        doc.requested_selection = Some(0..0);
        run(&mut doc, &mut body, vec![]);
        let pre = |s: &str| egui::Event::Ime(egui::ImeEvent::Preedit(s.into()));
        // compose a jamo, then Backspace it away: only an empty preedit arrives
        run(
            &mut doc,
            &mut body,
            vec![egui::Event::Ime(egui::ImeEvent::Enabled)],
        );
        run(&mut doc, &mut body, vec![pre("ㅇ")]);
        run(&mut doc, &mut body, vec![pre("")]);
        for e in ["1", ".", " "] {
            run(&mut doc, &mut body, vec![egui::Event::Text(e.into())]);
        }
        assert_eq!(doc.tabs[0].body, "  1. ", "{:?}", doc.tabs[0].body);
    }
    #[test]
    fn a_symbol_or_hanja_picked_from_the_ime_list_replaces_the_composition() {
        // ㅁ + 한자 opens the symbol list; the Enter, arrows or digit that pick
        // from it stay with the IME (vendor winit drops VK_PROCESSKEY presses),
        // so the frame carries only what egui-winit makes of the commit.
        let ctx = egui::Context::default();
        let mut doc = Document::default();
        let mut body = String::new();
        let mut t = 0.0;
        let mut run = |doc: &mut Document, body: &mut String, ev: Vec<egui::Event>| {
            t += 0.2;
            ui_frame(&ctx, doc, body, ev, t);
        };
        run(&mut doc, &mut body, vec![]);
        doc.requested_selection = Some(0..0);
        run(&mut doc, &mut body, vec![]);
        let ime = |e: egui::ImeEvent| egui::Event::Ime(e);
        let pre = |s: &str| ime(egui::ImeEvent::Preedit(s.into()));
        let picked = |s: &str| {
            vec![
                ime(egui::ImeEvent::Disabled),
                ime(egui::ImeEvent::Commit(s.into())),
                ime(egui::ImeEvent::Disabled),
                ime(egui::ImeEvent::Disabled),
            ]
        };
        for e in ["1", ".", " "] {
            run(&mut doc, &mut body, vec![egui::Event::Text(e.into())]);
        }
        run(&mut doc, &mut body, vec![ime(egui::ImeEvent::Enabled)]);
        run(&mut doc, &mut body, vec![pre("ㅁ")]);
        // the list is open: the jamo stays as the composition
        run(&mut doc, &mut body, vec![pre("ㅁ")]);
        assert!(doc.ime);
        run(&mut doc, &mut body, picked("※"));
        assert!(!doc.ime);
        assert_eq!(doc.tabs[0].body, "  1. ※", "{:?}", doc.tabs[0].body);
        // a syllable converted to Hanja, then plain Hangul after it
        run(&mut doc, &mut body, vec![ime(egui::ImeEvent::Enabled)]);
        run(&mut doc, &mut body, vec![pre("ㅎ")]);
        run(&mut doc, &mut body, vec![pre("한")]);
        run(&mut doc, &mut body, picked("韓"));
        run(&mut doc, &mut body, vec![ime(egui::ImeEvent::Enabled)]);
        run(&mut doc, &mut body, vec![pre("ㄱ")]);
        run(&mut doc, &mut body, vec![pre("글")]);
        run(
            &mut doc,
            &mut body,
            vec![
                ime(egui::ImeEvent::Disabled),
                ime(egui::ImeEvent::Commit("글".into())),
                ime(egui::ImeEvent::Disabled),
            ],
        );
        assert_eq!(doc.tabs[0].body, "  1. ※韓글", "{:?}", doc.tabs[0].body);
        // the list helpers still work after a picked symbol
        run(&mut doc, &mut body, vec![key(Key::Enter, Modifiers::NONE)]);
        assert_eq!(
            doc.tabs[0].body, "  1. ※韓글\n  2. ",
            "{:?}",
            doc.tabs[0].body
        );
    }
    #[test]
    fn a_caret_moved_by_undo_scrolls_into_view() {
        let ctx = egui::Context::default();
        let mut doc = Document::default();
        let mut body: String = (0..120).map(|i| format!("줄 {i}\n")).collect();
        doc.tabs[0].body = body.clone();
        let mut t = 0.0;
        let mut run = |doc: &mut Document, body: &mut String, ev: Vec<egui::Event>| {
            t += 0.2;
            ui_frame(&ctx, doc, body, ev, t);
        };
        run(&mut doc, &mut body, vec![]);
        // caret at the very end (scrolls to the bottom), then type there
        doc.requested_selection = Some(body.chars().count()..body.chars().count());
        for _ in 0..3 {
            run(&mut doc, &mut body, vec![]);
        }
        run(&mut doc, &mut body, vec![egui::Event::Text("끝".into())]);
        run(&mut doc, &mut body, vec![]);
        // an undo step far above: the caret jumps to the first line
        let old = doc.tabs[0].body.replacen("줄 0", "줄 영", 1);
        doc.undo.push(
            "본문 편집",
            UndoKind::Body {
                tab: 0,
                text: old,
                spans: vec![],
            },
            99.0,
        );
        run(&mut doc, &mut body, vec![key(Key::Z, Modifiers::CTRL)]);
        for _ in 0..4 {
            run(&mut doc, &mut body, vec![]);
        }
        let (clip, caret) = LAST_CARET.with(|c| c.get()).expect("the caret moved");
        assert!(
            caret.top() < 200.,
            "the caret is near the top of the text: {caret:?}"
        );
        assert!(
            clip.contains_rect(caret),
            "the view follows the caret: view {clip:?} caret {caret:?}"
        );
    }
    #[test]
    fn a_table_scrolled_half_out_of_the_body_is_clipped_to_the_body() {
        let ctx = egui::Context::default();
        ctx.set_visuals(egui::Visuals::light());
        let mut doc = Document::default();
        let mut table = Table::new(8, 2);
        for r in 0..8 {
            table.cells[r][0].text = format!("셀{r}");
        }
        let mut body = "\n".repeat(3);
        doc.tabs[0].body = body.clone();
        doc.tabs[0].insert_table(3..3, table);
        body = doc.tabs[0].body.clone();
        let mut top_of_body = 0.0_f32;
        let mut shapes = vec![];
        for i in 0..6 {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(500., 260.),
                )),
                time: Some(i as f64 * 0.1),
                ..Default::default()
            };
            if i == 2 {
                // put the caret after the table so the view scrolls down past its top
                doc.requested_selection = Some(5..5);
            }
            let out = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.label("제목 영역");
                    ui.separator();
                    top_of_body = ui.cursor().top();
                    show(ui, &mut body, &mut doc, "tbl_clip_test");
                });
            });
            shapes = out.shapes;
        }
        let mut seen = 0;
        for cs in &shapes {
            let mut stack = vec![&cs.shape];
            while let Some(sh) = stack.pop() {
                match sh {
                    egui::Shape::Vec(v) => stack.extend(v.iter()),
                    egui::Shape::Rect(r) if r.stroke.color == Color32::BLACK => {
                        seen += 1;
                        assert!(
                            cs.clip_rect.top() >= top_of_body - 4.0,
                            "a table border is clipped to the body: clip {:?}, body top {top_of_body}",
                            cs.clip_rect
                        );
                    }
                    _ => {}
                }
            }
        }
        assert!(seen > 0, "table borders were drawn");
    }
    #[test]
    fn alt_enter_breaks_a_line_inside_the_edited_cell_and_the_copy_keeps_it_in_one_cell() {
        let ctx = egui::Context::default();
        let mut doc = Document::default();
        doc.tabs[0].insert_table(0..0, Table::new(2, 2));
        let mut body = doc.tabs[0].body.clone();
        let mut t = 0.0;
        let mut run = |doc: &mut Document, body: &mut String, ev: Vec<egui::Event>| {
            t += 0.2;
            ui_frame(&ctx, doc, body, ev, t);
        };
        for _ in 0..3 {
            run(&mut doc, &mut body, vec![]);
        }
        let area = table_area(&ctx);
        for ev in click(area.min + Vec2::new(60., 16.)) {
            run(&mut doc, &mut body, ev);
        }
        run(&mut doc, &mut body, vec![egui::Event::Text("첫줄".into())]);
        run(&mut doc, &mut body, vec![]);
        run(&mut doc, &mut body, vec![key(Key::Enter, Modifiers::ALT)]);
        run(&mut doc, &mut body, vec![]);
        run(&mut doc, &mut body, vec![egui::Event::Text("둘째".into())]);
        run(&mut doc, &mut body, vec![]);
        assert!(
            doc.tsel.as_ref().unwrap().editing,
            "Alt+Enter keeps editing the cell"
        );
        run(&mut doc, &mut body, vec![key(Key::Enter, Modifiers::NONE)]);
        run(&mut doc, &mut body, vec![]);
        let Block::Table(table) = &doc.tabs[0].blocks[0] else {
            unreachable!()
        };
        assert_eq!(table.cells[0][0].text, "첫줄\n둘째");
        // Excel gets the line break inside one cell: quoted text, <br> in the cell
        assert!(crate::table_edit::table_tsv(table).starts_with("\"첫줄\n둘째\"\t"));
        let html = crate::table_edit::table_html(table);
        assert!(html.contains("첫줄<br style='mso-data-placement:same-cell;'>둘째"));
    }
    #[test]
    fn the_alignment_buttons_align_the_selected_cells() {
        let ctx = egui::Context::default();
        let mut doc = Document::default();
        doc.tabs[0].insert_table(0..0, Table::new(2, 3));
        let mut body = doc.tabs[0].body.clone();
        let mut t = 0.0;
        let mut run = |doc: &mut Document, body: &mut String, ev: Vec<egui::Event>| {
            t += 0.2;
            ui_frame(&ctx, doc, body, ev, t);
        };
        for _ in 0..3 {
            run(&mut doc, &mut body, vec![]);
        }
        let area = table_area(&ctx);
        // select the first row (drag across it)
        let (a, b) = (
            area.min + Vec2::new(60., 16.),
            area.min + Vec2::new(300., 16.),
        );
        let btn = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        run(&mut doc, &mut body, vec![egui::Event::PointerMoved(a)]);
        run(&mut doc, &mut body, vec![btn(a, true)]);
        run(&mut doc, &mut body, vec![egui::Event::PointerMoved(b)]);
        run(&mut doc, &mut body, vec![]);
        run(&mut doc, &mut body, vec![btn(b, false)]);
        run(&mut doc, &mut body, vec![]);
        // the three alignment buttons of the last frame: left, center, right
        ALIGN_BUTTONS.with(|b| b.borrow_mut().clear());
        run(&mut doc, &mut body, vec![]);
        let buttons = ALIGN_BUTTONS.with(|b| b.borrow().clone());
        assert_eq!(
            buttons.len(),
            3,
            "the toolbar has the three alignment buttons"
        );
        for ev in click(buttons[1].center()) {
            run(&mut doc, &mut body, ev);
        }
        let Block::Table(table) = &doc.tabs[0].blocks[0] else {
            unreachable!()
        };
        let first = table.cells[0][0].align;
        assert_eq!(first, CellAlign::Center, "the center button was pressed");
        assert!(
            table.cells[0].iter().all(|c| c.align == first),
            "the whole selected row follows"
        );
        assert!(
            table.cells[1].iter().all(|c| c.align == CellAlign::Default),
            "other rows are not touched"
        );
    }
    #[test]
    fn a_right_click_on_a_picture_opens_its_toolbar_at_the_mouse() {
        let ctx = egui::Context::default();
        let mut doc = Document::default();
        let pic = Picture::from_rgba(0, "p".into(), vec![255; 40 * 40 * 4], [40, 40], 1., false);
        doc.tabs[0].insert_picture(0..0, pic);
        let mut body = doc.tabs[0].body.clone();
        let mut t = 0.0;
        let mut run = |doc: &mut Document, body: &mut String, ev: Vec<egui::Event>| {
            t += 0.2;
            ui_frame(&ctx, doc, body, ev, t);
        };
        for _ in 0..3 {
            run(&mut doc, &mut body, vec![]);
        }
        let at = egui::pos2(30., 30.);
        let button = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        run(&mut doc, &mut body, vec![egui::Event::PointerMoved(at)]);
        run(&mut doc, &mut body, vec![button(true)]);
        run(&mut doc, &mut body, vec![button(false)]);
        run(&mut doc, &mut body, vec![]);
        assert!(doc.selected_picture.is_some(), "the picture is selected");
        let tool = ctx
            .memory(|m| {
                m.areas()
                    .visible_layer_ids()
                    .into_iter()
                    .filter(|l| l.order == egui::Order::Foreground)
                    .find_map(|l| m.area_rect(l.id))
            })
            .expect("the picture toolbar is shown");
        assert!(
            (tool.min - at).length() < 40.,
            "the toolbar is at the mouse: toolbar {tool:?}, mouse {at:?}"
        );
    }
    #[test]
    fn a_right_click_on_a_table_cell_opens_the_table_menu() {
        let ctx = egui::Context::default();
        let mut doc = Document::default();
        doc.tabs[0].insert_table(0..0, Table::new(2, 2));
        let mut body = doc.tabs[0].body.clone();
        let mut t = 0.0;
        let mut run = |doc: &mut Document, body: &mut String, ev: Vec<egui::Event>| {
            t += 0.2;
            ui_frame(&ctx, doc, body, ev, t);
        };
        for _ in 0..3 {
            run(&mut doc, &mut body, vec![]);
        }
        let area = table_area(&ctx);
        let at = area.left_top() + egui::vec2(12., 10.);
        let button = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        run(&mut doc, &mut body, vec![egui::Event::PointerMoved(at)]);
        run(&mut doc, &mut body, vec![button(true)]);
        run(&mut doc, &mut body, vec![button(false)]);
        run(&mut doc, &mut body, vec![]);
        assert!(crate::float::shown_in(&ctx), "the table menu is shown");
    }
    #[test]
    fn column_drag_resizes_and_round_trips_history() {
        let ctx = egui::Context::default();
        let mut doc = Document::default();
        doc.tabs[0].insert_table(0..0, Table::new(2, 2));
        let mut body = doc.tabs[0].body.clone();
        let mut time = 0.;
        let mut run = |doc: &mut Document, body: &mut String, events| {
            time += 0.2;
            ui_frame(&ctx, doc, body, events, time);
        };
        for _ in 0..4 {
            run(&mut doc, &mut body, vec![]);
        }
        let area = table_area(&ctx);
        let from = area.min + egui::vec2(120., 15.);
        let to = from + egui::vec2(45., 0.);
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            pressed,
            button: egui::PointerButton::Primary,
            modifiers: Modifiers::NONE,
        };
        run(&mut doc, &mut body, vec![egui::Event::PointerMoved(from)]);
        run(&mut doc, &mut body, vec![button(from, true)]);
        run(&mut doc, &mut body, vec![egui::Event::PointerMoved(to)]);
        run(&mut doc, &mut body, vec![]);
        run(&mut doc, &mut body, vec![button(to, false)]);
        let width = |doc: &Document| match &doc.tabs[0].blocks[0] {
            Block::Table(t) => t.col_widths[0],
            _ => unreachable!(),
        };
        assert!((width(&doc) - 165.).abs() < 1., "width {}", width(&doc));
        assert_eq!(doc.undo_steps(), 1);
        doc.undo().unwrap();
        assert_eq!(width(&doc), 120.);
        doc.redo().unwrap();
        assert!((width(&doc) - 165.).abs() < 1.);
    }

    #[test]
    fn a_row_grows_with_line_breaks_and_keeps_the_height_the_user_drags_it_to() {
        let ctx = egui::Context::default();
        let mut doc = Document::default();
        let mut table = Table::new(2, 2);
        table.cells[0][0].text = "첫째\n둘째\n셋째".into();
        table.cells[1][0].text = "짧음".into();
        doc.tabs[0].insert_table(0..0, table);
        let mut body = doc.tabs[0].body.clone();
        let mut t = 0.0;
        let mut run = |doc: &mut Document, body: &mut String, ev: Vec<egui::Event>| {
            t += 0.2;
            ui_frame(&ctx, doc, body, ev, t);
        };
        for _ in 0..4 {
            run(&mut doc, &mut body, vec![]);
        }
        let edges = ROW_EDGES.with(|e| e.borrow().clone());
        let area = table_area(&ctx);
        let first_h = edges[0] - area.min.y;
        let second_h = edges[1] - edges[0] - 1.;
        assert!(first_h > 55., "three lines make the row taller: {first_h}");
        assert!(
            (second_h - 32.).abs() < 1.,
            "a short row keeps the table's row height: {second_h}"
        );
        // drag the first row's bottom edge up: the new height is kept
        let from = egui::pos2(area.min.x + 100., edges[0] + 0.5);
        let to = egui::pos2(from.x, from.y - 30.);
        let btn = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        run(&mut doc, &mut body, vec![egui::Event::PointerMoved(from)]);
        run(&mut doc, &mut body, vec![btn(from, true)]);
        run(&mut doc, &mut body, vec![egui::Event::PointerMoved(to)]);
        run(&mut doc, &mut body, vec![]);
        run(&mut doc, &mut body, vec![btn(to, false)]);
        run(&mut doc, &mut body, vec![]);
        let Block::Table(tb) = &doc.tabs[0].blocks[0] else {
            unreachable!()
        };
        let manual = tb.row_manual[0];
        assert!(
            manual > 20. && manual < first_h - 20.,
            "the row was set shorter than its content: {manual} (was {first_h})"
        );
        assert_eq!(doc.undo_steps(), 1, "the drag is one undo step");
        // more lines in the cell do not change a height the user set
        if let Block::Table(tb) = &mut doc.tabs[0].blocks[0] {
            tb.cells[0][0].text.push_str("\n넷째\n다섯째");
        }
        run(&mut doc, &mut body, vec![]);
        let edges2 = ROW_EDGES.with(|e| e.borrow().clone());
        assert!(
            ((edges2[0] - table_area(&ctx).min.y) - (manual + 0.)).abs() < 1.5,
            "the dragged height stays"
        );
        // undo gives the automatic height back
        run(
            &mut doc,
            &mut body,
            vec![egui::Event::PointerMoved(egui::pos2(
                area.min.x + 100.,
                area.min.y + 10.,
            ))],
        );
        let steps = doc.undo_steps();
        assert!(steps >= 1);
        let before = doc.undo.entries.len();
        let _ = doc.undo();
        assert_eq!(doc.undo.entries.len(), before - 1);
        let Block::Table(tb) = &doc.tabs[0].blocks[0] else {
            unreachable!()
        };
        assert_eq!(tb.row_manual[0], 0., "back to growing with the content");
    }
    #[test]
    fn row_heights_follow_row_inserts_and_deletes() {
        let mut t = Table::new(3, 2);
        t.row_manual = vec![50., 0., 70.];
        crate::table_edit::insert_row(&mut t, 1);
        assert_eq!(t.row_manual, vec![50., 0., 0., 70.]);
        crate::table_edit::delete_rows(&mut t, 0, 0);
        assert_eq!(t.row_manual, vec![0., 0., 70.]);
        assert_eq!(t.row_manual.len(), t.cells.len());
    }
    #[test]
    fn layout_reserves_the_table_size_and_clamps_to_wrap_width() {
        let mut tab = Tab::new("본문".into());
        tab.body = "가".into();
        let mut table = Table::new(3, 2);
        table.col_widths = vec![120., 120.];
        table.row_height = 32.;
        tab.insert_table(1..1, table);
        tab.body.push('나');
        let sizes = table_sizes(&tab, 1.);
        let font = FontId::proportional(14.);
        let mut job = layout(
            &tab.body,
            &[],
            &Format::default(),
            font.clone(),
            font,
            Color32::BLACK,
        );
        // Wide enough that the table's natural width (120+120+2+2) fits.
        inline_tables(&mut job, &sizes, 300., 1.);
        assert_eq!(job.text, "가\u{3000}나");
        let full = &job.sections[1].format;
        assert_eq!(full.font_id.size, 244.);
        assert_eq!(full.line_height, Some(3. * 33. + TABLE_TOOLBAR_H + 4.));
        assert_eq!(full.color, Color32::TRANSPARENT);
        // A narrower wrap clamps the reserved width but not the height —
        // `table_widget`'s own horizontal scroll area absorbs the rest.
        let clamped = sizes[0].at(100.);
        assert_eq!(clamped.x, 96.);
        assert_eq!(clamped.y, 3. * 33. + TABLE_TOOLBAR_H);
    }
}
#[cfg(test)]
mod spacer_tests {
    use super::*;
    fn context(spacer: bool) -> egui::Context {
        let ctx = egui::Context::default();
        let mut fonts = egui::FontDefinitions::default();
        if let Ok(bytes) = std::fs::read("C:/Windows/Fonts/malgun.ttf") {
            fonts
                .font_data
                .insert("korean".into(), egui::FontData::from_owned(bytes).into());
            fonts
                .families
                .get_mut(&FontFamily::Proportional)
                .unwrap()
                .insert(0, "korean".into());
        }
        if spacer {
            add_spacer_font(&mut fonts);
        }
        ctx.set_fonts(fonts);
        ctx
    }
    fn memo() -> (Document, String) {
        let mut doc = Document::default();
        let mut table = Table::new(30, 14);
        for r in 0..30 {
            for c in 0..14 {
                table.cells[r][c].text = format!("셀 {r}-{c} 한글");
            }
        }
        doc.tabs[0].body = "앞 글\n\n뒤 글".into();
        doc.tabs[0].insert_table(4..4, table);
        let body = doc.tabs[0].body.clone();
        (doc, body)
    }
    /// One frame at a window size: (time in ms, where the table was drawn).
    fn frame(
        ctx: &egui::Context,
        doc: &mut Document,
        body: &mut String,
        size: Vec2,
        i: usize,
    ) -> (f64, egui::Rect) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            time: Some(i as f64 * 0.016),
            max_texture_side: Some(16384),
            ..Default::default()
        };
        let t = std::time::Instant::now();
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                show(ui, body, doc, "spacer_test");
            });
        });
        let ms = t.elapsed().as_secs_f64() * 1000.;
        (ms, LAST_TABLE_RECT.with(|r| r.get()).expect("table drawn"))
    }
    #[test]
    fn the_spacer_font_has_one_em_wide_glyph_and_nothing_to_draw() {
        let ctx = context(true);
        let _ = ctx.run(Default::default(), |ctx| {
            let family = FontFamily::Name(SPACER_FONT.into());
            for size in [10., 100., 1500.] {
                let w =
                    ctx.fonts(|f| f.glyph_width(&FontId::new(size, family.clone()), '\u{3000}'));
                assert!((w - size).abs() < 0.01, "{size}: {w}");
            }
        });
    }
    /// Tables sit exactly where they did with the old (text font) spacer,
    /// narrow, maximized and at widths in between.
    #[test]
    fn a_table_is_laid_out_the_same_with_the_spacer_font() {
        let (old, new) = (context(false), context(true));
        let (mut doc_a, mut body_a) = memo();
        let (mut doc_b, mut body_b) = memo();
        let mut i = 0;
        for size in [
            Vec2::new(420., 360.),
            Vec2::new(1920., 1040.),
            Vec2::new(733., 500.),
            Vec2::new(1111., 640.),
        ] {
            let (mut a, mut b) = (egui::Rect::NOTHING, egui::Rect::NOTHING);
            for _ in 0..3 {
                a = frame(&old, &mut doc_a, &mut body_a, size, i).1;
                b = frame(&new, &mut doc_b, &mut body_b, size, i).1;
                i += 1;
            }
            for (x, y) in [
                (a.left(), b.left()),
                (a.top(), b.top()),
                (a.right(), b.right()),
                (a.bottom(), b.bottom()),
            ] {
                assert!((x - y).abs() <= 1., "{size:?}: {a:?} vs {b:?}");
            }
        }
        assert_eq!(body_a, body_b);
    }
    /// Maximizing or resizing a memo with a table must not put a
    /// table-sized glyph into the font atlas.
    #[test]
    fn resizing_a_memo_with_a_table_does_not_grow_the_font_atlas() {
        let ctx = context(true);
        let (mut doc, mut body) = memo();
        for i in 0..3 {
            frame(&ctx, &mut doc, &mut body, Vec2::new(420., 360.), i);
        }
        // Every glyph the memo uses is in the atlas now.
        frame(&ctx, &mut doc, &mut body, Vec2::new(1920., 1040.), 3);
        let before = ctx.fonts(|f| f.font_image_size());
        for i in 4..40 {
            frame(
                &ctx,
                &mut doc,
                &mut body,
                Vec2::new(500. + i as f32 * 30., 700.),
                i,
            );
        }
        frame(&ctx, &mut doc, &mut body, Vec2::new(1920., 1040.), 40);
        assert_eq!(ctx.fonts(|f| f.font_image_size()), before);
        assert!(before[1] <= 512, "atlas {before:?}");
    }
    /// `cargo test --release maximize_frame_time -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn maximize_frame_time() {
        for spacer in [false, true] {
            let ctx = context(spacer);
            let (mut doc, mut body) = memo();
            for i in 0..4 {
                frame(&ctx, &mut doc, &mut body, Vec2::new(420., 360.), i);
            }
            let (ms, _) = frame(&ctx, &mut doc, &mut body, Vec2::new(1920., 1040.), 4);
            println!(
                "spacer font {spacer}: maximize frame {ms:.2} ms, atlas {:?}",
                ctx.fonts(|f| f.font_image_size())
            );
        }
    }
}
#[cfg(test)]
mod layout_tests {
    use super::*;
    /// A small deterministic generator (no extra crate for tests).
    struct Rng(u64);
    impl Rng {
        fn next(&mut self, below: usize) -> usize {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((self.0 >> 33) as usize) % below.max(1)
        }
    }
    fn fonts() -> (FontId, FontId) {
        (
            FontId::proportional(14.),
            FontId::new(14., FontFamily::Name("bold".into())),
        )
    }
    fn random_spans(rng: &mut Rng, chars: usize, count: usize) -> Vec<Span> {
        (0..count)
            .map(|_| {
                // Past the end, empty and reversed ranges are all possible.
                let a = rng.next(chars + 4);
                let b = match rng.next(6) {
                    0 => a,
                    1 => a.saturating_sub(rng.next(5)),
                    2 => chars + rng.next(9),
                    _ => a + rng.next(12),
                };
                let bits = rng.next(16);
                Span {
                    range: a..b,
                    format: Format {
                        bold: bits & 1 != 0,
                        italic: bits & 2 != 0,
                        underline: bits & 4 != 0,
                        strike: bits & 8 != 0,
                        ..Default::default()
                    },
                }
            })
            .collect()
    }
    /// The quick `layout` gives exactly the job the original one did.
    #[test]
    fn layout_matches_the_reference_for_any_spans_and_links() {
        let (font, bold) = fonts();
        let color = Color32::from_rgb(20, 30, 40);
        let pieces = [
            "한글 메모 ",
            "plain ",
            "https://example.com/a?b=c ",
            "http://x.kr\n",
            "\n",
            "☐ 할 일\t",
            "xhttps://not.a.link ",
            "https://",
            "  ",
            "\u{3000}",
            "끝",
            "http:/no ",
            "https://a\u{3000}b ",
            "\u{1F642} ",
        ];
        let mut rng = Rng(7);
        for round in 0..400 {
            let mut text = String::new();
            for _ in 0..rng.next(14) {
                text.push_str(pieces[rng.next(pieces.len())]);
            }
            if round % 9 == 0 {
                text.push(placeholder(3));
            }
            let chars = text.chars().count();
            let count = rng.next(9);
            let spans = random_spans(&mut rng, chars, count);
            let base = Format {
                bold: round % 5 == 0,
                ..Default::default()
            };
            let got = layout(&text, &spans, &base, font.clone(), bold.clone(), color);
            let want = layout_reference(&text, &spans, &base, font.clone(), bold.clone(), color);
            assert!(got == want, "round {round}: {text:?} {spans:?}");
        }
        for text in [
            "",
            "https://only",
            " https://after.space",
            "\nhttp://after.line",
        ] {
            let base = Format::default();
            let got = layout(text, &[], &base, font.clone(), bold.clone(), color);
            let want = layout_reference(text, &[], &base, font.clone(), bold.clone(), color);
            assert!(got == want, "{text:?}");
        }
    }
    fn long_memo(spans: usize) -> (String, Vec<Span>) {
        let text =
            "긴 메모의 한 줄입니다. plain text and 한글 https://example.com/page\n".repeat(400);
        let chars = text.chars().count();
        let mut rng = Rng(11);
        let spans = (0..spans)
            .map(|_| {
                let a = rng.next(chars);
                Span {
                    range: a..a + 1 + rng.next(30),
                    format: Format {
                        bold: true,
                        ..Default::default()
                    },
                }
            })
            .collect();
        (text, spans)
    }
    /// `cargo test --release layout_build_time -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn layout_build_time() {
        let (font, bold) = fonts();
        let base = Format::default();
        for count in [0, 50, 500, 3000] {
            let (text, spans) = long_memo(count);
            let time = |f: &dyn Fn() -> LayoutJob| {
                let t = std::time::Instant::now();
                for _ in 0..20 {
                    std::hint::black_box(f());
                }
                t.elapsed().as_secs_f64() * 1000. / 20.
            };
            let black = Color32::BLACK;
            let old =
                time(&|| layout_reference(&text, &spans, &base, font.clone(), bold.clone(), black));
            let new = time(&|| layout(&text, &spans, &base, font.clone(), bold.clone(), black));
            println!(
                "{} chars, {count} spans: before {old:.3} ms, after {new:.3} ms per frame",
                text.chars().count()
            );
        }
    }
}

/// Ctrl+V with an image on the clipboard, with two sticker windows open.
#[cfg(test)]
mod paste_focus_tests {
    use super::tests::{CHORD, CLIPBOARD_PICTURE};
    use super::*;

    fn picture() -> Picture {
        Picture {
            id: 0,
            instance: new_block_instance(),
            name: "붙여 넣은 이미지".into(),
            pixels: vec![255; 2 * 2 * 4],
            width: 2,
            height: 2,
            scale: 1.,
            fit: false,
            file: String::new(),
            texture: None,
            texture_used: 0.,
        }
    }
    fn pictures(doc: &Document) -> usize {
        doc.tabs[0]
            .blocks
            .iter()
            .filter(|b| matches!(b, Block::Picture(_)))
            .count()
    }
    /// One frame of one window (viewport) of the shared context; `focused`
    /// is what the operating system says about that window.
    fn frame(
        ctx: &egui::Context,
        window: egui::ViewportId,
        focused: bool,
        doc: &mut Document,
        body: &mut String,
        events: Vec<egui::Event>,
        time: f64,
    ) {
        let mut viewports = egui::ViewportIdMap::default();
        viewports.insert(
            window,
            egui::ViewportInfo {
                focused: Some(focused),
                ..Default::default()
            },
        );
        let input = egui::RawInput {
            viewport_id: window,
            viewports,
            focused,
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                Vec2::new(500., 400.),
            )),
            events,
            time: Some(time),
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                show(ui, body, doc, "sticker_body");
            });
        });
    }
    fn click(pos: egui::Pos2) -> Vec<egui::Event> {
        [true, false]
            .into_iter()
            .map(|pressed| egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            })
            .collect()
    }

    /// Two stickers both keep the text cursor of their last edit; the
    /// keyboard is only in the one the operating system gave the focus to.
    /// Pasting must not put the picture into the other one as well.
    #[test]
    fn a_pasted_picture_goes_only_to_the_window_with_the_keyboard_focus() {
        let ctx = egui::Context::default();
        let (win_a, win_b) = (
            egui::ViewportId::from_hash_of("a"),
            egui::ViewportId::from_hash_of("b"),
        );
        let (mut a, mut b) = (Document::default(), Document::default());
        let (mut body_a, mut body_b) = (a.tabs[0].body.clone(), b.tabs[0].body.clone());
        let mut t = 0.;
        let mut step = || {
            t += 0.05;
            t
        };
        // The user edited in both stickers: both have a text cursor.
        for (win, doc, body) in [(win_a, &mut a, &mut body_a), (win_b, &mut b, &mut body_b)] {
            frame(&ctx, win, true, doc, body, vec![], step());
            frame(
                &ctx,
                win,
                true,
                doc,
                body,
                click(egui::pos2(100., 80.)),
                step(),
            );
            frame(&ctx, win, true, doc, body, vec![], step());
        }
        // Now working in A; B is behind it.
        CLIPBOARD_PICTURE.with(|p| *p.borrow_mut() = Some(picture()));
        CHORD.with(|c| c.set(true));
        frame(&ctx, win_a, true, &mut a, &mut body_a, vec![], step());
        frame(&ctx, win_b, false, &mut b, &mut body_b, vec![], step());
        assert_eq!(
            pictures(&a),
            1,
            "the window being typed in gets the picture"
        );
        assert_eq!(
            pictures(&b),
            0,
            "a window without the keyboard focus must not"
        );
        // Keys up, then the same in the other direction.
        CHORD.with(|c| c.set(false));
        frame(&ctx, win_a, false, &mut a, &mut body_a, vec![], step());
        frame(&ctx, win_b, true, &mut b, &mut body_b, vec![], step());
        CHORD.with(|c| c.set(true));
        frame(&ctx, win_a, false, &mut a, &mut body_a, vec![], step());
        frame(&ctx, win_b, true, &mut b, &mut body_b, vec![], step());
        assert_eq!((pictures(&a), pictures(&b)), (1, 1));
        CHORD.with(|c| c.set(false));
        CLIPBOARD_PICTURE.with(|p| *p.borrow_mut() = None);
    }
}
