use crate::*;
use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap, HashSet},
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
    rc::Rc,
};
pub struct Asset {
    pub name: String,
    pub date: NaiveDate,
    pub bytes: Option<Vec<u8>>,
    pub thumbnail: Option<Vec<u8>>,
    pub temporary: Option<Vec<u8>>,
    pub path: Option<PathBuf>,
    pub texture: Option<egui::TextureHandle>,
}
impl Asset {
    fn displayed(&self) -> Option<&[u8]> {
        self.temporary.as_deref().or(self.bytes.as_deref())
    }
}
fn decode(bytes: &[u8]) -> Result<image::RgbaImage, String> {
    if bytes.len() > 64 * 1024 * 1024 {
        return Err("64 MB를 넘는 파일은 이 미리보기에서 열지 않습니다".into());
    }
    let mut reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    reader
        .decode()
        .map(|i| i.to_rgba8())
        .map_err(|e| e.to_string())
}
fn png(img: image::RgbaImage) -> Result<Vec<u8>, String> {
    let mut out = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut out, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(out.into_inner())
}
fn draw_text(img: &mut image::RgbaImage, text: &str, x: u32, y: u32, size: f32) {
    static FONT: std::sync::OnceLock<Option<fontdue::Font>> = std::sync::OnceLock::new();
    let Some(font) = FONT.get_or_init(|| {
        std::fs::read("C:/Windows/Fonts/malgun.ttf")
            .ok()
            .and_then(|b| fontdue::Font::from_bytes(b, fontdue::FontSettings::default()).ok())
    }) else {
        return;
    };
    let mut left = x as i32;
    for ch in text.chars() {
        let (m, b) = font.rasterize(ch, size);
        for row in 0..m.height {
            for col in 0..m.width {
                let xx = left + m.xmin + col as i32;
                let yy = y as i32 + (size as i32 - m.height as i32 - m.ymin) + row as i32;
                if xx >= 0 && yy >= 0 && (xx as u32) < img.width() && (yy as u32) < img.height() {
                    let alpha = b[row * m.width + col] as f32 / 255.;
                    let p = img.get_pixel_mut(xx as u32, yy as u32);
                    for c in 0..3 {
                        p[c] = (p[c] as f32 * (1. - alpha) + 48. * alpha) as u8;
                    }
                }
            }
        }
        left += m.advance_width.ceil() as i32;
    }
}
fn table_png(rows: &[[&str; 3]]) -> Result<Vec<u8>, String> {
    let mut img = image::RgbaImage::from_pixel(
        660,
        rows.len() as u32 * 48 + 1,
        image::Rgba([255, 254, 252, 255]),
    );
    for (r, row) in rows.iter().enumerate() {
        for (c, text) in row.iter().enumerate() {
            draw_text(&mut img, text, 15 + c as u32 * 220, 10 + r as u32 * 48, 19.);
        }
    }
    for x in [0, 219, 439, 659] {
        for y in 0..img.height() {
            img.put_pixel(x, y, image::Rgba([210, 210, 205, 255]));
        }
    }
    for r in 0..=rows.len() {
        let y = (r as u32 * 48).min(img.height() - 1);
        for x in 0..img.width() {
            img.put_pixel(x, y, image::Rgba([210, 210, 205, 255]));
        }
    }
    png(img)
}
fn sample_asset(i: usize) -> Asset {
    let color = PALETTE[i];
    let mut img = image::RgbaImage::from_pixel(
        480,
        300,
        image::Rgba([color.r(), color.g(), color.b(), 255]),
    );
    draw_text(&mut img, ["기록", "아이디어", "참고 자료"][i], 35, 115, 38.);
    Asset {
        name: format!("예시 이미지 {}", i + 1),
        date: Local::now().date_naive() - chrono::Duration::days(i as i64),
        thumbnail: png(image::imageops::thumbnail(&img, 120, 75)).ok(),
        bytes: png(img).ok(),
        temporary: None,
        path: None,
        texture: None,
    }
}
const BOOKMARK_FILE_LIMIT: u64 = 10 * 1024 * 1024;
const BOOKMARK_LINK_LIMIT: usize = 20_000;

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
enum BookmarkKind {
    Folder { open: bool },
    Link { url: String },
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
struct BookmarkEntry {
    parent: Option<usize>,
    title: String,
    kind: BookmarkKind,
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct BookmarkSource {
    path: PathBuf,
    title: String,
    open: bool,
    entries: Vec<BookmarkEntry>,
}

impl BookmarkSource {
    pub fn links(&self) -> impl Iterator<Item = (&str, &str)> {
        self.entries.iter().filter_map(|entry| match &entry.kind {
            BookmarkKind::Folder { .. } => None,
            BookmarkKind::Link { url } => Some((entry.title.as_str(), url.as_str())),
        })
    }
}

pub fn legacy_bookmarks(
    bookmarks: &[(String, String)],
    sources: &[BookmarkSource],
) -> Vec<(String, String)> {
    let mut seen = HashSet::new();
    bookmarks
        .iter()
        .map(|(title, url)| (title.as_str(), url.as_str()))
        .chain(sources.iter().flat_map(BookmarkSource::links))
        .filter(|&(_, url)| seen.insert(url))
        .map(|(title, url)| (title.to_owned(), url.to_owned()))
        .collect()
}

pub fn remove_imported_bookmark_copies(
    bookmarks: &mut Vec<(String, String)>,
    sources: &[BookmarkSource],
) {
    let imported: HashSet<_> = sources
        .iter()
        .flat_map(BookmarkSource::links)
        .map(|(_, url)| url)
        .collect();
    bookmarks.retain(|(_, url)| !imported.contains(url.as_str()));
}

fn valid_bookmark_url(url: &str) -> bool {
    (url.starts_with("https://") || url.starts_with("http://"))
        && !url.chars().any(char::is_control)
}

fn parse_bookmarks(html: &str) -> Vec<BookmarkEntry> {
    use ego_tree::iter::Edge;

    let doc = scraper::Html::parse_document(html);
    let mut entries = Vec::new();
    let mut seen = HashSet::new();
    // Each DL inherits its enclosing folder or consumes the preceding H3.
    // HTML parsers can place the DL inside or alongside the H3's DT.
    let mut lists = vec![(None, None)];
    for event in doc.tree.root().traverse() {
        let (opening, node) = match event {
            Edge::Open(node) => (true, node),
            Edge::Close(node) => (false, node),
        };
        let Some(element) = scraper::ElementRef::wrap(node) else {
            continue;
        };
        let (parent, pending) = lists.last_mut().expect("bookmark root list");
        match (opening, element.value().name()) {
            (true, "dl") => {
                let folder = pending.take().or(*parent);
                lists.push((folder, None));
            }
            (false, "dl") => {
                lists.pop();
            }
            (true, "h3")
                if node
                    .parent()
                    .and_then(scraper::ElementRef::wrap)
                    .is_some_and(|e| e.value().name() == "dt") =>
            {
                *pending = Some(entries.len());
                entries.push(BookmarkEntry {
                    parent: *parent,
                    title: element.text().collect::<String>().trim().to_owned(),
                    kind: BookmarkKind::Folder { open: false },
                });
            }
            (true, "a") => {
                *pending = None;
                let Some(url) = element.value().attr("href").map(str::trim) else {
                    continue;
                };
                if seen.len() < BOOKMARK_LINK_LIMIT
                    && valid_bookmark_url(url)
                    && seen.insert((*parent, url.to_owned()))
                {
                    entries.push(BookmarkEntry {
                        parent: *parent,
                        title: element.text().collect(),
                        kind: BookmarkKind::Link {
                            url: url.to_owned(),
                        },
                    });
                }
            }
            _ => {}
        }
    }
    entries
}

#[derive(Default)]
struct BookmarkSiblings<'a> {
    folders: HashMap<(Option<usize>, &'a str), Vec<usize>>,
    links: HashSet<(Option<usize>, &'a str)>,
}

fn merge_bookmark_entries(
    existing: &mut Vec<BookmarkEntry>,
    imported: Vec<BookmarkEntry>,
) -> (usize, usize) {
    let mut siblings = BookmarkSiblings::default();
    for (id, entry) in existing.iter().enumerate() {
        match &entry.kind {
            BookmarkKind::Folder { .. } => siblings
                .folders
                .entry((entry.parent, &entry.title))
                .or_default()
                .push(id),
            BookmarkKind::Link { url } => {
                siblings.links.insert((entry.parent, url));
            }
        }
    }
    let mut occurrence = HashMap::new();
    let mut map = Vec::with_capacity(imported.len());
    let mut folders = 0;
    let mut links = 0;
    let mut additions = Vec::new();
    for entry in &imported {
        let parent = entry.parent.and_then(|id| map[id]);
        let next = existing.len() + additions.len();
        let destination = match &entry.kind {
            BookmarkKind::Folder { .. } => {
                let key = (parent, entry.title.as_str());
                let nth = occurrence.entry(key).or_insert(0);
                let destination = siblings.folders.get(&key).and_then(|ids| ids.get(*nth));
                *nth += 1;
                if let Some(&id) = destination {
                    map.push(Some(id));
                    continue;
                }
                folders += 1;
                next
            }
            BookmarkKind::Link { url } => {
                if !siblings.links.insert((parent, url)) {
                    map.push(None);
                    continue;
                }
                links += 1;
                next
            }
        };
        map.push(Some(destination));
        additions.push(BookmarkEntry {
            parent,
            ..entry.clone()
        });
    }
    existing.extend(additions);
    (folders, links)
}

fn read_bookmark_html(path: &Path) -> Result<String, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(BOOKMARK_FILE_LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > BOOKMARK_FILE_LIMIT {
        return Err("10 MB를 넘는 HTML 파일은 가져오지 않습니다".into());
    }
    String::from_utf8(bytes).map_err(|_| "UTF-8 즐겨찾기 HTML 파일을 선택하세요".into())
}

#[derive(Clone, Copy, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct BookmarkLayout {
    section_open: bool,
    legacy_open: bool,
}

impl Default for BookmarkLayout {
    fn default() -> Self {
        Self {
            section_open: true,
            legacy_open: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum BookmarkRow {
    LegacyRoot,
    LegacyLink(usize),
    Source(usize),
    Entry(usize, usize),
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct BookmarkLine {
    row: BookmarkRow,
    depth: usize,
    open: bool,
}

#[derive(Default)]
pub struct BookmarkUi {
    query: String,
    revealed: Option<BookmarkRow>,
    scroll_target: Option<BookmarkRow>,
    rows: Option<Rc<[BookmarkLine]>>,
    count: Option<usize>,
}

const LEGACY_BOOKMARKS: &str = "기존 즐겨찾기";

fn bookmark_rows(
    legacy: &[(String, String)],
    sources: &[BookmarkSource],
    marks: &BTreeMap<String, (String, bool)>,
    layout: BookmarkLayout,
    search: &str,
    revealed: Option<BookmarkRow>,
) -> Vec<BookmarkLine> {
    let query = search.trim().to_lowercase();
    let searching = !query.is_empty() && revealed.is_none();
    let matches = |text: &str| text.to_lowercase().contains(&query);
    let mut rows = Vec::new();
    if !legacy.is_empty() {
        let root_match = searching && matches(LEGACY_BOOKMARKS);
        let open = layout.legacy_open || matches!(revealed, Some(BookmarkRow::LegacyLink(_)));
        if !searching || root_match {
            rows.push(BookmarkLine {
                row: BookmarkRow::LegacyRoot,
                depth: 0,
                open,
            });
        }
        if open || searching {
            for id in bookmark_order(legacy, marks) {
                let (title, url) = &legacy[id];
                if !searching || root_match || matches(title) || matches(url) {
                    rows.push(BookmarkLine {
                        row: BookmarkRow::LegacyLink(id),
                        depth: 1,
                        open: false,
                    });
                }
            }
        }
    }
    for (source_id, source) in sources.iter().enumerate() {
        let mut reveal_path = HashSet::new();
        if let Some(BookmarkRow::Entry(id, entry)) = revealed {
            if id == source_id {
                let mut next = source.entries.get(entry).and_then(|entry| entry.parent);
                while let Some(parent) = next {
                    if !reveal_path.insert(parent) {
                        break;
                    }
                    next = source.entries.get(parent).and_then(|entry| entry.parent);
                }
            }
        }
        let root_match = searching && matches(&source.title);
        let open =
            source.open || matches!(revealed, Some(BookmarkRow::Entry(id, _)) if id == source_id);
        if !searching || root_match {
            rows.push(BookmarkLine {
                row: BookmarkRow::Source(source_id),
                depth: 0,
                open,
            });
        }
        if !open && !searching {
            continue;
        }
        // Build the children in import order. Only parents that precede their
        // child are accepted so damaged save data cannot make a cyclic tree.
        let mut children = vec![Vec::new(); source.entries.len() + 1];
        for (id, entry) in source.entries.iter().enumerate() {
            let parent = match entry.parent {
                Some(p)
                    if p < id && matches!(source.entries[p].kind, BookmarkKind::Folder { .. }) =>
                {
                    p + 1
                }
                _ => 0,
            };
            children[parent].push(id);
        }
        let mut stack: Vec<_> = children[0]
            .iter()
            .rev()
            .map(|&id| (id, 1, root_match))
            .collect();
        while let Some((id, depth, ancestor_match)) = stack.pop() {
            let entry = &source.entries[id];
            let title_match = searching
                && (matches(&entry.title)
                    || query.rsplit_once(" / ").is_some_and(|(_, last)| {
                        matches!(entry.kind, BookmarkKind::Folder { .. })
                            && entry.title.to_lowercase().contains(last)
                            && matches(&format!(
                                "{} / {}",
                                bookmark_path(source, entry.parent),
                                entry.title
                            ))
                    }));
            let url_match =
                searching && matches!(&entry.kind, BookmarkKind::Link { url } if matches(url));
            let path_match = ancestor_match || title_match;
            let open = matches!(entry.kind, BookmarkKind::Folder { open: true })
                || reveal_path.contains(&id);
            if !searching || path_match || url_match {
                rows.push(BookmarkLine {
                    row: BookmarkRow::Entry(source_id, id),
                    depth,
                    open,
                });
            }
            if open || searching {
                stack.extend(
                    children[id + 1]
                        .iter()
                        .rev()
                        .map(|&child| (child, depth + 1, path_match)),
                );
            }
        }
    }
    rows
}

fn bookmark_path(source: &BookmarkSource, mut parent: Option<usize>) -> String {
    let mut path = Vec::new();
    while let Some(id) = parent {
        let Some(folder) = source.entries.get(id) else {
            break;
        };
        path.push(folder.title.as_str());
        parent = folder.parent.filter(|&next| next < id);
    }
    path.push(&source.title);
    path.reverse();
    path.join(" / ")
}

fn draw_bookmark_link(
    ui: &mut egui::Ui,
    title: &str,
    url: &str,
    selected: bool,
    marks: &mut BTreeMap<String, (String, bool)>,
) -> bool {
    let (mut comment, pinned) = marks.get(url).cloned().unwrap_or_default();
    let host = url
        .split_once("://")
        .and_then(|(_, rest)| rest.split('/').next())
        .unwrap_or(url);
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        let mut changed = false;
        if crate::icons::toggle_icon(
            ui,
            crate::icons::Icon::Pin,
            if pinned { "고정 해제" } else { "고정" },
            pinned,
        )
        .clicked()
        {
            marks.entry(url.to_owned()).or_default().1 = !pinned;
            changed = true;
        }
        let comment_width = (ui.available_width() * 0.30).clamp(60., 180.);
        if ui
            .add(
                egui::TextEdit::singleline(&mut comment)
                    .hint_text("코멘트")
                    .desired_width(comment_width),
            )
            .changed()
        {
            marks.entry(url.to_owned()).or_default().0 = comment;
            changed = true;
        }
        if crate::ui::list_row(
            ui,
            ui.id().with("link"),
            selected,
            Some(crate::ui::BLUE),
            title,
            host,
            32.,
        )
        .on_hover_text(url)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
            && valid_bookmark_url(url)
        {
            ui.ctx().open_url(egui::OpenUrl::new_tab(url));
        }
        changed
    })
    .inner
}
impl App {
    fn copy_image_bytes(&mut self, bytes: &[u8]) -> Result<(), String> {
        let img = decode(bytes)?;
        let mut c = crate::clipboard_copy::Clipboard::new().map_err(|e| e.to_string())?;
        c.set_image(arboard::ImageData {
            width: img.width() as usize,
            height: img.height() as usize,
            bytes: Cow::Owned(img.into_raw()),
        })
        .map_err(|e| e.to_string())
    }
    pub fn real_table_gallery(&mut self, ui: &mut egui::Ui) {
        ui.add(egui::TextEdit::singleline(&mut self.extra.table_search).hint_text("표 내용 검색"));
        let rows = [
            ["작업", "상태", "일정"],
            ["화면 설계", "진행 중", "오늘"],
            ["프로토타입 검토", "할 일", "내일"],
        ];
        if !self.extra.table_search.is_empty()
            && !rows
                .iter()
                .flatten()
                .any(|c| c.contains(&self.extra.table_search))
        {
            ui.label("검색 결과가 없습니다.");
            return;
        }
        egui::Grid::new("actual_table")
            .striped(true)
            .min_col_width(150.)
            .show(ui, |ui| {
                for r in rows {
                    for c in r {
                        ui.label(c);
                    }
                    ui.end_row();
                }
            });
        ui.horizontal(|ui| {
            if ui.button("표로 복사").clicked() {
                let text = rows
                    .iter()
                    .map(|r| r.join("\t"))
                    .collect::<Vec<_>>()
                    .join("\n");
                let html = format!(
                    "<table>{}</table>",
                    rows.iter()
                        .map(|r| format!(
                            "<tr>{}</tr>",
                            r.iter()
                                .map(|c| format!("<td>{c}</td>"))
                                .collect::<String>()
                        ))
                        .collect::<String>()
                );
                self.toast = match crate::clipboard_copy::Clipboard::new()
                    .and_then(|mut c| c.set_html(html, Some(text)))
                {
                    Ok(_) => "표를 HTML / 탭 구분 텍스트로 복사했습니다".into(),
                    Err(e) => format!("복사 실패: {e}"),
                };
            }
            if ui.button("이미지로 복사").clicked() {
                self.toast = match table_png(&rows).and_then(|b| self.copy_image_bytes(&b)) {
                    Ok(_) => "표 이미지를 클립보드에 복사했습니다".into(),
                    Err(e) => format!("복사 실패: {e}"),
                };
            }
        });
        ui.label(
            RichText::new("표 데이터는 예시입니다. 복사는 실제 클립보드에 반영됩니다.")
                .small()
                .color(MUTED),
        );
    }
    pub fn real_image_gallery(&mut self, ui: &mut egui::Ui) {
        if self.extra.media.is_empty() {
            self.extra.media = (0..3).map(sample_asset).collect();
        }
        let resident: usize = self
            .extra
            .media
            .iter()
            .filter_map(|m| m.bytes.as_ref())
            .map(Vec::len)
            .sum();
        let temp: usize = self
            .extra
            .media
            .iter()
            .filter_map(|m| m.temporary.as_ref())
            .map(Vec::len)
            .sum();
        ui.label(format!(
            "{}개 · 본문 이미지 {:.1} KB · 임시 로딩 {:.1} KB",
            self.extra.media.len(),
            resident as f64 / 1024.,
            temp as f64 / 1024.
        ));
        ui.horizontal_wrapped(|ui| {
            self.date_filter_button(ui, 6, "시작 날짜", self.extra.media_from);
            self.date_filter_button(ui, 7, "끝 날짜", self.extra.media_to);
            if ui.button("필터 해제").clicked() {
                self.extra.media_from = None;
                self.extra.media_to = None;
            }
            if ui.button("일괄 저장하기").clicked() {
                if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                    let mut count = 0;
                    let mut errors = vec![];
                    for (i, m) in self.extra.media.iter_mut().enumerate() {
                        if self.extra.media_from.is_some_and(|a| m.date < a)
                            || self.extra.media_to.is_some_and(|b| m.date > b)
                        {
                            continue;
                        }
                        if let Some(bytes) = m.displayed() {
                            let path = dir.join(format!(
                                "ohmemojang-image-{}-{}.png",
                                i + 1,
                                Local::now().format("%Y%m%d-%H%M%S")
                            ));
                            match decode(bytes)
                                .and_then(png)
                                .and_then(|b| write_new(&path, &b))
                            {
                                Ok(()) => {
                                    m.path = Some(path);
                                    count += 1
                                }
                                Err(e) => errors.push(e),
                            }
                        }
                    }
                    self.toast = format!("{count}개 저장 · {}개 실패", errors.len());
                }
            }
        });
        let ids: Vec<_> = self
            .extra
            .media
            .iter()
            .enumerate()
            .filter(|(_, m)| {
                self.extra.media_from.is_none_or(|a| m.date >= a)
                    && self.extra.media_to.is_none_or(|b| m.date <= b)
            })
            .map(|(i, _)| i)
            .collect();
        ui.horizontal_wrapped(|ui| {
            for i in ids {
                ui.push_id(i, |ui| {
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.vertical(|ui| self.image_card(ui, i));
                    });
                });
            }
        });
        ui.label(RichText::new("저장 위치와 비운 상태는 앱 실행 중에만 유지됩니다. 프로그램 종료 후 복원은 저장 기능 연결 단계에서 지원합니다.").small().color(MUTED));
    }
    pub fn real_bookmark_gallery(&mut self, ui: &mut egui::Ui) {
        if ui.button("+ 즐겨찾기 가져오기…").clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("즐겨찾기 HTML", &["html", "htm"])
                .pick_file()
            {
                self.toast = match read_bookmark_html(&path) {
                    Ok(html) => self.import_bookmarks(path, &html),
                    Err(e) => format!("가져오기 실패: {e}"),
                };
            }
        }
        let view = &mut self.extra.bookmark_ui;
        let layout = &mut self.extra.bookmark_layout;
        let sources = &mut self.extra.bookmark_sources;
        let legacy = &self.extra.bookmarks;
        let count = *view.count.get_or_insert_with(|| {
            legacy.len() + sources.iter().map(|s| s.links().count()).sum::<usize>()
        });
        ui.horizontal_wrapped(|ui| {
            let section_label = if layout.section_open {
                format!("▾ 즐겨찾기 ({count})")
            } else {
                format!("▸ 즐겨찾기 ({count})")
            };
            if ui.button(section_label).clicked() {
                layout.section_open = !layout.section_open;
            }
            if ui
                .add(
                    egui::TextEdit::singleline(&mut view.query)
                        .hint_text("즐겨찾기 이름, 주소, 폴더 검색")
                        .desired_width(240.),
                )
                .changed()
            {
                view.revealed = None;
                view.rows = None;
            }
            if ui.button("모두 펼치기").clicked() {
                set_bookmark_folders(sources, layout, true);
                view.rows = None;
            }
            if ui.button("모두 접기").clicked() {
                set_bookmark_folders(sources, layout, false);
                view.rows = None;
            }
        });
        let searching = !view.query.trim().is_empty();
        if !layout.section_open && !searching {
            return;
        }
        if view.revealed.is_some() && ui.button("검색 결과로 돌아가기").clicked() {
            view.revealed = None;
            view.rows = None;
        }
        let marks = &mut self.extra.bookmark_marks;
        let rows = view
            .rows
            .get_or_insert_with(|| {
                bookmark_rows(legacy, sources, marks, *layout, &view.query, view.revealed).into()
            })
            .clone();
        if rows.is_empty() {
            if searching {
                ui.label("검색 결과가 없습니다");
            } else {
                ui.label("브라우저에서 내보낸 즐겨찾기 HTML을 가져오세요");
            }
            return;
        }
        let mut picked = None;
        let mut scroll = egui::ScrollArea::vertical()
            .id_salt(("bookmark_rows", searching && view.revealed.is_none()))
            .auto_shrink([false, true])
            .max_height(ui.clip_rect().height() * 0.74);
        const ROW: f32 = 32.;
        if let Some(target) = view.scroll_target.take() {
            if let Some(at) = rows.iter().position(|line| line.row == target) {
                scroll =
                    scroll.vertical_scroll_offset(at as f32 * (ROW + ui.spacing().item_spacing.y));
            }
        }
        scroll.show_rows(ui, ROW, rows.len(), |ui, range| {
            for line in &rows[range] {
                let result = searching && view.revealed.is_none();
                ui.push_id(line.row, |ui| {
                    if result {
                        let (title, path) = search_bookmark_line(legacy, sources, line.row);
                        if crate::ui::list_row(
                            ui,
                            ui.id().with("result"),
                            false,
                            Some(crate::ui::BLUE),
                            title,
                            &path,
                            ROW,
                        )
                        .on_hover_text(path)
                        .clicked()
                        {
                            picked = Some(line.row);
                        }
                        return;
                    }
                    ui.horizontal(|ui| {
                        ui.set_min_height(ROW);
                        let indent = (line.depth as f32 * 14.).min(ui.available_width() * 0.35);
                        ui.add_space(indent);
                        let selected = view.revealed == Some(line.row);
                        match line.row {
                            BookmarkRow::LegacyRoot => {
                                if draw_bookmark_folder(ui, LEGACY_BOOKMARKS, line.open, selected) {
                                    layout.legacy_open = !layout.legacy_open;
                                    view.rows = None;
                                }
                            }
                            BookmarkRow::LegacyLink(id) => {
                                let (title, url) = &legacy[id];
                                if draw_bookmark_link(ui, title, url, selected, marks) {
                                    view.rows = None;
                                }
                            }
                            BookmarkRow::Source(id) => {
                                let source = &mut sources[id];
                                if draw_bookmark_folder(ui, &source.title, line.open, selected) {
                                    source.open = !source.open;
                                    view.rows = None;
                                }
                            }
                            BookmarkRow::Entry(source, id) => {
                                let entry = &mut sources[source].entries[id];
                                match &mut entry.kind {
                                    BookmarkKind::Folder { open } => {
                                        if draw_bookmark_folder(
                                            ui,
                                            &entry.title,
                                            line.open,
                                            selected,
                                        ) {
                                            *open = !*open;
                                            view.rows = None;
                                        }
                                    }
                                    BookmarkKind::Link { url } => {
                                        if draw_bookmark_link(
                                            ui,
                                            &entry.title,
                                            url,
                                            selected,
                                            marks,
                                        ) && !legacy.is_empty()
                                        {
                                            view.rows = None;
                                        }
                                    }
                                }
                            }
                        }
                    });
                });
            }
        });
        if let Some(row) = picked {
            view.revealed = Some(row);
            view.scroll_target = Some(row);
            view.rows = None;
            ui.ctx().request_repaint();
        }
    }

    pub(crate) fn import_bookmarks(&mut self, path: PathBuf, html: &str) -> String {
        let entries = parse_bookmarks(html);
        if entries.is_empty() {
            return "가져올 수 있는 즐겨찾기나 폴더가 없습니다".into();
        }
        let sources = &mut self.extra.bookmark_sources;
        let source = match sources.iter().position(|source| source.path == path) {
            Some(id) => &mut sources[id],
            None => {
                let title = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("가져온 즐겨찾기")
                    .to_owned();
                sources.push(BookmarkSource {
                    path,
                    title,
                    open: true,
                    entries: Vec::new(),
                });
                sources.last_mut().unwrap()
            }
        };
        let (folders, links) = merge_bookmark_entries(&mut source.entries, entries);
        remove_imported_bookmark_copies(&mut self.extra.bookmarks, sources);
        self.extra.bookmark_layout.section_open = true;
        self.extra.bookmark_ui = BookmarkUi::default();
        format!("즐겨찾기 {links}개 · 폴더 {folders}개 추가")
    }
}

fn set_bookmark_folders(sources: &mut [BookmarkSource], layout: &mut BookmarkLayout, open: bool) {
    layout.section_open = open;
    layout.legacy_open = open;
    for source in sources {
        source.open = open;
        for entry in &mut source.entries {
            if let BookmarkKind::Folder { open: folder_open } = &mut entry.kind {
                *folder_open = open;
            }
        }
    }
}

fn search_bookmark_line<'a>(
    legacy: &'a [(String, String)],
    sources: &'a [BookmarkSource],
    row: BookmarkRow,
) -> (&'a str, String) {
    match row {
        BookmarkRow::LegacyRoot => (LEGACY_BOOKMARKS, String::new()),
        BookmarkRow::LegacyLink(id) => (&legacy[id].0, LEGACY_BOOKMARKS.into()),
        BookmarkRow::Source(id) => (&sources[id].title, String::new()),
        BookmarkRow::Entry(source, id) => {
            let source = &sources[source];
            let entry = &source.entries[id];
            (&entry.title, bookmark_path(source, entry.parent))
        }
    }
}

fn draw_bookmark_folder(ui: &mut egui::Ui, title: &str, open: bool, selected: bool) -> bool {
    let (row, _) = crate::ui::tree_row(
        ui,
        ui.id().with("folder"),
        selected,
        Some(open),
        crate::icons::Icon::Folder,
        title,
        0,
    );
    row.clicked()
}

/// Retain the existing pin and comment order in the legacy flat collection.
fn bookmark_order(
    bookmarks: &[(String, String)],
    marks: &BTreeMap<String, (String, bool)>,
) -> Vec<usize> {
    let rank = |url: &str| match marks.get(url) {
        Some((_, true)) => 0,
        Some((comment, _)) if !comment.trim().is_empty() => 1,
        _ => 2,
    };
    let mut order: Vec<usize> = (0..bookmarks.len()).collect();
    order.sort_by_cached_key(|&id| rank(&bookmarks[id].1));
    order
}
fn read_image(path: &Path) -> Result<Vec<u8>, String> {
    let size = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
    if size > 64 * 1024 * 1024 {
        return Err("64 MB 초과 이미지".into());
    }
    let b = std::fs::read(path).map_err(|e| e.to_string())?;
    decode(&b)?;
    Ok(b)
}
fn write_new(path: &Path, b: &[u8]) -> Result<(), String> {
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    f.write_all(b).map_err(|e| e.to_string())
}
#[cfg(test)]
mod tests {
    use super::*;

    const BROWSER_EXPORT: &str = r#"<!DOCTYPE NETSCAPE-Bookmark-file-1>
        <TITLE>Bookmarks</TITLE><H1>Bookmarks</H1>
        <DL><p>
            <DT><A HREF="https://example.org/">최상위</A>
            <DT><H3 PERSONAL_TOOLBAR_FOLDER="true">작업</H3>
            <DL><p>
                <DT><A HREF="https://example.org/">작업 자료</A>
                <DT><H3>조사</H3></DT>
                <DL><p>
                    <DT><A HREF="https://research.example/">검색어 &amp; 참고</A>
                </DL><p>
                <DT><H3>빈 폴더</H3>
                <DL><p></DL><p>
            </DL><p>
            <DT><H3>작업</H3>
            <DL><p>
                <DT><A HREF="https://second.example/">다른 작업 자료</A>
            </DL><p>
        </DL><p>"#;

    #[test]
    fn bookmarks_skip_script_urls_and_deduplicate() {
        assert_eq!(
            parse_bookmarks(
                "<a href='javascript:alert(1)'>x</a><a href='https://example.com'>가 &amp; 나</a>+                 <a href='https://example.com'>duplicate</a><a href='https://bad.example/&#9;x'>x</a>"
            ),
            vec![BookmarkEntry {
                parent: None,
                title: "가 & 나".into(),
                kind: BookmarkKind::Link {
                    url: "https://example.com".into(),
                },
            }]
        );
    }

    #[test]
    fn bookmark_html_keeps_folders_empty_folders_and_source_order() {
        let entries = parse_bookmarks(BROWSER_EXPORT);
        let titles: Vec<_> = entries
            .iter()
            .map(|entry| (entry.title.as_str(), entry.parent))
            .collect();
        assert_eq!(
            titles,
            vec![
                ("최상위", None),
                ("작업", None),
                ("작업 자료", Some(1)),
                ("조사", Some(1)),
                ("검색어 & 참고", Some(3)),
                ("빈 폴더", Some(1)),
                ("작업", None),
                ("다른 작업 자료", Some(6)),
            ]
        );
    }

    #[test]
    fn importing_same_export_keeps_folder_state_and_adds_only_new_siblings() {
        let mut entries = parse_bookmarks(BROWSER_EXPORT);
        entries[6].kind = BookmarkKind::Folder { open: true };
        assert_eq!(
            merge_bookmark_entries(&mut entries, parse_bookmarks(BROWSER_EXPORT)),
            (0, 0)
        );
        let updated = BROWSER_EXPORT.replace(
            r#"<DT><A HREF="https://second.example/">다른 작업 자료</A>"#,
            r#"<DT><A HREF="https://second.example/">다른 작업 자료</A>
               <DT><A HREF="https://new.example/">더 많은 작업 자료</A>"#,
        );
        assert_eq!(
            merge_bookmark_entries(&mut entries, parse_bookmarks(&updated)),
            (0, 1)
        );
        assert_eq!(entries[8].parent, Some(6));
        assert_eq!(entries[6].kind, BookmarkKind::Folder { open: true });
    }

    #[test]
    fn bookmark_search_reveals_a_nested_result_without_mutating_fold_state() {
        let source = BookmarkSource {
            path: PathBuf::from("Bookmark.html"),
            title: "Bookmark.html".into(),
            open: false,
            entries: parse_bookmarks(BROWSER_EXPORT),
        };
        let layout = BookmarkLayout::default();
        let marks = BTreeMap::new();
        let sources = vec![source];
        let hits = bookmark_rows(&[], &sources, &marks, layout, "작업 / 조사", None);
        assert_eq!(
            hits.iter().map(|line| line.row).collect::<Vec<_>>(),
            vec![BookmarkRow::Entry(0, 3), BookmarkRow::Entry(0, 4)],
        );
        let hit = hits[1].row;
        let revealed = bookmark_rows(&[], &sources, &marks, layout, "작업 / 조사", Some(hit));
        assert!(revealed.iter().any(|line| line.row == hit));
        let reset = bookmark_rows(&[], &sources, &marks, layout, "", None);
        assert_eq!(reset.len(), 1);
        assert_eq!(reset[0].row, BookmarkRow::Source(0));
        assert!(!reset[0].open);
    }

    #[test]
    fn bookmark_import_caps_links_and_preserves_empty_folder_at_end() {
        let mut html = String::from("<DL>");
        for i in 0..=BOOKMARK_LINK_LIMIT {
            html.push_str(&format!(r#"<DT><A HREF="https://test.example/{i}">L</A>"#));
        }
        html.push_str("<DT><H3>폴더</H3><DL></DL></DL>");
        let entries = parse_bookmarks(&html);
        assert_eq!(entries.len(), BOOKMARK_LINK_LIMIT + 1);
        assert_eq!(
            entries.last().unwrap().kind,
            BookmarkKind::Folder { open: false }
        );
    }

    #[test]
    fn bookmark_tree_and_legacy_marks_survive_database_reload() {
        let dir = std::env::temp_dir().join(format!(
            "ohmemojang-bookmarks-db-{}-{}",
            std::process::id(),
            line!()
        ));
        let today = NaiveDate::from_ymd_opt(2026, 11, 23).unwrap();
        let mut app = App::sample(today);
        let migrated = "https://example.org/";
        app.extra.bookmarks = vec![
            ("기존 링크".into(), "https://old.example/".into()),
            ("가져온 링크".into(), migrated.into()),
        ];
        app.extra
            .bookmark_marks
            .insert(migrated.into(), ("공유 코멘트".into(), true));
        app.import_bookmarks(PathBuf::from("Bookmarks.html"), BROWSER_EXPORT);
        set_bookmark_folders(
            &mut app.extra.bookmark_sources,
            &mut app.extra.bookmark_layout,
            false,
        );
        let saved = app.snapshot(&dir);
        let flat = saved.bookmarks.as_ref().unwrap();
        assert_eq!(flat.iter().filter(|(_, url)| url == migrated).count(), 1);
        assert_eq!(flat.len(), 4);
        crate::storage::write_all(&dir, &saved).unwrap();

        let mut restored = App::sample(today);
        restored.restore(&crate::storage::read(&dir).unwrap().unwrap());
        assert_eq!(restored.extra.bookmarks, app.extra.bookmarks);
        assert_eq!(restored.extra.bookmarks.len(), 1);
        assert_eq!(restored.extra.bookmark_sources, app.extra.bookmark_sources);
        assert!(!restored.extra.bookmark_layout.section_open);
        assert!(!restored.extra.bookmark_layout.legacy_open);
        assert_eq!(restored.extra.bookmark_marks, app.extra.bookmark_marks);
        let _ = std::fs::remove_dir_all(&dir);
    }
    #[test]
    fn pinned_bookmarks_lead_then_commented_ones() {
        let b: Vec<(String, String)> = ["a", "b", "c", "d"]
            .iter()
            .map(|u| (u.to_string(), u.to_string()))
            .collect();
        let mut marks = std::collections::BTreeMap::new();
        marks.insert("c".to_string(), ("메모".to_string(), false));
        marks.insert("d".to_string(), (String::new(), true));
        marks.insert("a".to_string(), ("  ".to_string(), false));
        assert_eq!(bookmark_order(&b, &marks), vec![3, 2, 0, 1]);
    }
    #[test]
    fn sample_asset_is_valid_png() {
        let a = sample_asset(0);
        let img = decode(a.bytes.as_ref().unwrap()).unwrap();
        assert_eq!((img.width(), img.height()), (480, 300));
    }
    #[test]
    fn table_image_has_exact_rows() {
        let b = table_png(&[["가", "나", "다"], ["1", "2", "3"]]).unwrap();
        let i = decode(&b).unwrap();
        assert_eq!((i.width(), i.height()), (660, 97));
    }
}

#[cfg(test)]
mod filesystem_tests {
    use super::*;
    #[test]
    fn batch_output_never_overwrites_existing_file() {
        let p = std::env::temp_dir().join(format!(
            "ohmemojang-test-{}-{}.tmp",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        write_new(&p, b"original").unwrap();
        assert!(write_new(&p, b"replacement").is_err());
        assert_eq!(std::fs::read(&p).unwrap(), b"original");
        std::fs::remove_file(p).unwrap();
    }
}
impl App {
    fn image_card(&mut self, ui: &mut egui::Ui, i: usize) {
        ui.set_width(235.);
        let thumb_only = self.extra.media[i].displayed().is_none();
        let m = &mut self.extra.media[i];
        if m.texture.is_none() {
            if let Some(bytes) = m.displayed().or(m.thumbnail.as_deref()) {
                if let Ok(img) = decode(bytes) {
                    let ci = egui::ColorImage::from_rgba_unmultiplied(
                        [img.width() as usize, img.height() as usize],
                        img.as_raw(),
                    );
                    m.texture = Some(ui.ctx().load_texture(
                        format!("image-{i}"),
                        ci,
                        egui::TextureOptions::LINEAR,
                    ));
                }
            }
        }
        let clicked = if let Some(t) = &m.texture {
            ui.add(
                egui::Image::new(t)
                    .fit_to_exact_size(Vec2::new(235., 147.))
                    .sense(egui::Sense::click()),
            )
            .on_hover_text(if thumb_only {
                "썸네일 · 클릭하여 백업 찾기"
            } else {
                "클릭하면 이미지가 복사됩니다"
            })
            .clicked()
        } else {
            ui.add_sized([235., 147.], egui::Button::new("백업 이미지 찾기"))
                .clicked()
        };
        if clicked {
            if thumb_only {
                self.extra.missing_image = Some(i);
            } else {
                let b = self.extra.media[i].displayed().map(ToOwned::to_owned);
                self.toast = match b
                    .ok_or("원본 없음".into())
                    .and_then(|b| self.copy_image_bytes(&b))
                {
                    Ok(()) => "이미지를 복사했습니다".into(),
                    Err(e) => e,
                };
            }
        }
        ui.strong(&self.extra.media[i].name);
        ui.label(format!(
            "{} · {}",
            self.extra.media[i].date,
            if thumb_only {
                "썸네일만 보관"
            } else {
                "원본 이미지"
            }
        ));
        if save_icon(ui).clicked() {
            if thumb_only {
                self.extra.missing_image = Some(i);
            } else if let Some(path) = rfd::FileDialog::new()
                .add_filter("PNG", &["png"])
                .set_file_name(format!("image-{}.png", i + 1))
                .save_file()
            {
                let result = self.extra.media[i]
                    .displayed()
                    .ok_or("이미지 없음".into())
                    .and_then(decode)
                    .and_then(png)
                    .and_then(|b| std::fs::write(&path, b).map_err(|e| e.to_string()));
                match result {
                    Ok(()) => {
                        self.extra.media[i].path = Some(path);
                        self.toast = "이미지를 저장했습니다".into()
                    }
                    Err(e) => self.toast = e,
                }
            }
        }
        if let Some(path) = self.extra.media[i].path.clone() {
            if self.extra.media[i].bytes.is_some()
                && ui.button("백업 확인 후 원본 용량 정리").clicked()
            {
                self.toast = match self.archive_asset(i, &path) {
                    Ok(()) => "원본을 정리하고 작은 썸네일을 남겼습니다".into(),
                    Err(e) => format!("정리 중단·원본 유지: {e}"),
                };
            }
        }
        if self.extra.media[i].temporary.is_some() && ui.button("백업 보기 닫기").clicked() {
            self.extra.media[i].temporary = None;
            self.extra.media[i].texture = None;
        }
    }
    fn archive_asset(&mut self, i: usize, path: &Path) -> Result<(), String> {
        let b = read_image(path)?;
        let external = decode(&b)?;
        let original = decode(self.extra.media[i].bytes.as_deref().ok_or("원본 없음")?)?;
        if original != external {
            return Err("백업 파일이 원본과 다릅니다".into());
        }
        let thumb = png(image::imageops::thumbnail(&original, 120, 75))?;
        let m = &mut self.extra.media[i];
        m.thumbnail = Some(thumb);
        m.bytes = None;
        m.temporary = None;
        m.texture = None;
        Ok(())
    }
    pub fn missing_image_dialog(&mut self, ctx: &egui::Context) {
        if let Some(i) = self.extra.missing_image {
            let mut open = true;
            crate::ui::dialog(
                ctx,
                "missing_image",
                "이미지 백업 찾기",
                [420., 220.],
                true,
                &mut open,
                |ui| {
                    ui.label("용량 관리로 원본 이미지가 정리되었습니다. 백업 파일을 찾아주세요.");
                    if let Some(path) = self.extra.media[i].path.clone() {
                        ui.label(path.display().to_string());
                        if ui.button("기억한 백업에서 열기").clicked() {
                            match read_image(&path) {
                                Ok(b) => {
                                    self.extra.media[i].temporary = Some(b);
                                    self.extra.media[i].texture = None;
                                    self.extra.missing_image = None
                                }
                                Err(e) => self.toast = e,
                            }
                        }
                    }
                    if ui.button("백업 파일 찾기…").clicked() {
                        let mut dialog =
                            rfd::FileDialog::new().add_filter("이미지", &["png", "jpg", "jpeg"]);
                        if let Some(parent) =
                            self.extra.media[i].path.as_ref().and_then(|p| p.parent())
                        {
                            dialog = dialog.set_directory(parent);
                        }
                        if let Some(path) = dialog.pick_file() {
                            match read_image(&path) {
                                Ok(b) => {
                                    self.extra.media[i].temporary = Some(b);
                                    self.extra.media[i].path = Some(path);
                                    self.extra.media[i].texture = None;
                                    self.extra.missing_image = None
                                }
                                Err(e) => self.toast = e,
                            }
                        }
                    }
                    if ui.button("닫기").clicked() {
                        self.extra.missing_image = None;
                    }
                },
            );
            if !open {
                self.extra.missing_image = None;
            }
        }
    }
}
fn save_icon(ui: &mut egui::Ui) -> egui::Response {
    let (rect, r) = ui.allocate_exact_size(Vec2::new(32., 28.), egui::Sense::click());
    r.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "다른 이름으로 저장")
    });
    let c = rect.center();
    let color = ui.visuals().text_color();
    let stroke = egui::Stroke::new(1.5_f32, color);
    ui.painter()
        .line_segment([c - Vec2::new(0., 7.), c + Vec2::new(0., 3.)], stroke);
    ui.painter()
        .line_segment([c + Vec2::new(-4., 0.), c + Vec2::new(0., 4.)], stroke);
    ui.painter()
        .line_segment([c + Vec2::new(4., 0.), c + Vec2::new(0., 4.)], stroke);
    ui.painter()
        .line_segment([c + Vec2::new(-7., 7.), c + Vec2::new(7., 7.)], stroke);
    r.on_hover_text("다른 이름으로 저장")
}
#[cfg(test)]
mod thumbnail_tests {
    use super::*;
    #[test]
    fn archive_keeps_thumbnail_and_rejects_wrong_backup() {
        let mut a = App::sample(Local::now().date_naive());
        a.extra.media = vec![sample_asset(0)];
        let p = std::env::temp_dir().join(format!(
            "ohmemojang-archive-{}-{}.png",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let original = a.extra.media[0].bytes.clone().unwrap();
        std::fs::write(&p, sample_asset(1).bytes.unwrap()).unwrap();
        assert!(a.archive_asset(0, &p).is_err());
        assert_eq!(a.extra.media[0].bytes.as_ref().unwrap(), &original);
        std::fs::write(&p, &original).unwrap();
        a.archive_asset(0, &p).unwrap();
        assert!(a.extra.media[0].bytes.is_none());
        let thumb = decode(a.extra.media[0].thumbnail.as_ref().unwrap()).unwrap();
        assert!(thumb.width() <= 120 && thumb.height() <= 75);
        assert_eq!(std::fs::read(&p).unwrap(), original);
        std::fs::remove_file(p).unwrap();
    }
}
