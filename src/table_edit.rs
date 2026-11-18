//! Pure table editing and Excel-format clipboard text. Nothing here touches
//! egui or the clipboard, so every rule is unit-tested.
//!
//! A merged region is one *logical cell*: the owner (top-left) cell carries
//! `row_span`/`col_span`, the grid positions it covers are `covered` cells.
//! Row/column edits go through the logical cells so a merge grows, shrinks or
//! moves instead of being cleared.
use crate::composer::{Cell, CellAlign, Format, Table};
use std::collections::HashMap;

pub const MAX_ROWS: usize = 100;
pub const MAX_COLS: usize = 20;

/// Inclusive rectangle: (row0, col0, row1, col1).
pub type Rng = (usize, usize, usize, usize);

pub fn normalize(a: (usize, usize), b: (usize, usize)) -> Rng {
    (a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1))
}

pub fn rows(t: &Table) -> usize {
    t.cells.len()
}
pub fn cols(t: &Table) -> usize {
    t.cells.first().map_or(0, Vec::len)
}

/// The cell that owns grid position (r, c): itself, or the merge covering it.
pub fn owner_of(t: &Table, r: usize, c: usize) -> (usize, usize) {
    if !t.cells[r][c].covered {
        return (r, c);
    }
    for rr in (0..=r).rev() {
        for cc in (0..=c).rev() {
            let cell = &t.cells[rr][cc];
            if !cell.covered && rr + cell.row_span > r && cc + cell.col_span > c {
                return (rr, cc);
            }
        }
    }
    (r, c)
}

/// The grid area of the logical cell at (r, c), clipped to the table.
pub fn span_rect(t: &Table, r: usize, c: usize) -> Rng {
    let (or, oc) = owner_of(t, r, c);
    let cell = &t.cells[or][oc];
    (
        or,
        oc,
        (or + cell.row_span.max(1) - 1).min(rows(t) - 1),
        (oc + cell.col_span.max(1) - 1).min(cols(t) - 1),
    )
}

/// Grows the rectangle until no merged cell is cut by its edge.
#[allow(clippy::mut_range_bound)]
pub fn expand(t: &Table, rng: Rng) -> Rng {
    let (mut r0, mut c0, mut r1, mut c1) = rng;
    r1 = r1.min(rows(t) - 1);
    c1 = c1.min(cols(t) - 1);
    loop {
        let before = (r0, c0, r1, c1);
        for r in r0..=r1 {
            for c in c0..=c1 {
                let s = span_rect(t, r, c);
                r0 = r0.min(s.0);
                c0 = c0.min(s.1);
                r1 = r1.max(s.2);
                c1 = c1.max(s.3);
            }
        }
        if before == (r0, c0, r1, c1) {
            return before;
        }
    }
}

/// The logical cell next to (r, c): 0 up, 1 down, 2 left, 3 right. Stays
/// put at the table edge; a merge is crossed as one cell.
pub fn step(t: &Table, from: (usize, usize), dir: u8) -> (usize, usize) {
    let (r, c) = owner_of(t, from.0, from.1);
    let cell = &t.cells[r][c];
    match dir {
        0 if r > 0 => owner_of(t, r - 1, c),
        1 if r + cell.row_span.max(1) < rows(t) => owner_of(t, r + cell.row_span.max(1), c),
        2 if c > 0 => owner_of(t, r, c - 1),
        3 if c + cell.col_span.max(1) < cols(t) => owner_of(t, r, c + cell.col_span.max(1)),
        _ => (r, c),
    }
}

/// Tab order: along the row, then the start of the next row.
pub fn next_cell(t: &Table, from: (usize, usize), back: bool) -> (usize, usize) {
    let (r, c) = owner_of(t, from.0, from.1);
    let side = step(t, (r, c), if back { 2 } else { 3 });
    if side != (r, c) {
        return side;
    }
    if back {
        if r == 0 {
            return (r, c);
        }
        owner_of(t, r - 1, cols(t) - 1)
    } else {
        let below = r + t.cells[r][c].row_span.max(1);
        if below >= rows(t) {
            (r, c)
        } else {
            owner_of(t, below, 0)
        }
    }
}

// ------------------------------------------------------------ copy text ----

/// One cell's text for the clipboard: Excel quotes a value holding a tab,
/// line break or quote, and doubles the quotes inside.
pub fn tsv_quote(text: &str) -> String {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    if text.contains(['\t', '\n', '"']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text
    }
}

pub fn grid_tsv(grid: &[Vec<String>]) -> String {
    let mut out = String::new();
    for row in grid {
        out.push_str(
            &row.iter()
                .map(|c| tsv_quote(c))
                .collect::<Vec<_>>()
                .join("\t"),
        );
        out.push_str("\r\n");
    }
    out
}

/// Texts of a range; merge-covered positions are empty, as in Excel.
pub fn range_grid(t: &Table, rng: Rng) -> Vec<Vec<String>> {
    (rng.0..=rng.2)
        .map(|r| {
            (rng.1..=rng.3)
                .map(|c| {
                    let cell = &t.cells[r][c];
                    if cell.covered {
                        String::new()
                    } else {
                        cell.text.clone()
                    }
                })
                .collect()
        })
        .collect()
}

pub fn range_tsv(t: &Table, rng: Rng) -> String {
    grid_tsv(&range_grid(t, rng))
}

pub fn table_tsv(t: &Table) -> String {
    range_tsv(t, (0, 0, rows(t) - 1, cols(t) - 1))
}

/// Text Excel would turn into a number or a formula: marked as text.
fn needs_text_format(s: &str) -> bool {
    let digits = !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
    s.starts_with(['=', '@', '+'])
        || (digits && s.len() > 1 && s.starts_with('0'))
        || (digits && s.len() >= 12)
}

pub fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// A cell's text as Excel writes it: line breaks stay inside the cell
/// (`mso-data-placement:same-cell`), and spaces that HTML would drop (at a
/// line's start or end, or two in a row) are written as Excel does, a
/// `mso-spacerun` span of non-breaking spaces.
fn html_cell(text: &str) -> String {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut out = String::new();
    for (i, line) in text.split('\n').enumerate() {
        if i > 0 {
            out.push_str("<br style='mso-data-placement:same-cell;'>");
        }
        let chars: Vec<char> = line.chars().collect();
        let mut j = 0;
        while j < chars.len() {
            if chars[j] == ' ' {
                let mut k = j;
                while k < chars.len() && chars[k] == ' ' {
                    k += 1;
                }
                let n = k - j;
                if n >= 2 || j == 0 || k == chars.len() {
                    out.push_str("<span style='mso-spacerun:yes'>");
                    for x in 0..n {
                        out.push_str(if x % 2 == 0 { "&nbsp;" } else { " " });
                    }
                    out.push_str("</span>");
                } else {
                    out.push(' ');
                }
                j = k;
            } else {
                out.push_str(&html_escape(&chars[j].to_string()));
                j += 1;
            }
        }
    }
    out
}

fn hex(c: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}

/// The cell's look as an inline style (single-quoted in the tag, so a
/// double quote may appear inside): black border, fill, text color,
/// alignment, emphasis, and "keep as text" for values Excel would convert.
fn cell_css(cell: &Cell) -> String {
    let mut st = String::from("border:.5pt solid black;");
    if let Some(f) = cell.fill {
        st.push_str(&format!("background:{};mso-pattern:black none;", hex(f)));
    }
    if let Some(c) = cell.text_color {
        st.push_str(&format!("color:{};", hex(c)));
    }
    match cell.align {
        CellAlign::Left => st.push_str("text-align:left;"),
        CellAlign::Center => st.push_str("text-align:center;"),
        CellAlign::Right => st.push_str("text-align:right;"),
        CellAlign::Default => {}
    }
    if cell.format.bold {
        st.push_str("font-weight:700;");
    }
    if cell.format.italic {
        st.push_str("font-style:italic;");
    }
    match (cell.format.underline, cell.format.strike) {
        (true, true) => st.push_str("text-decoration:underline line-through;"),
        (true, false) => st.push_str("text-decoration:underline;"),
        (false, true) => st.push_str("text-decoration:line-through;"),
        _ => {}
    }
    if needs_text_format(&cell.text) {
        st.push_str("mso-number-format:\"\\@\";");
    }
    st
}

/// HTML for the range, the way Excel writes a cell: quotes escaped, a line
/// break kept inside its cell with `mso-data-placement:same-cell`, text
/// marked for values that must not become numbers or formulas.
pub fn range_html(t: &Table, rng: Rng) -> String {
    let mut out = String::from("<table border=0 cellpadding=0 cellspacing=0>");
    for r in rng.0..=rng.2 {
        out.push_str("<tr>");
        for c in rng.1..=rng.3 {
            let cell = &t.cells[r][c];
            if cell.covered {
                continue;
            }
            out.push_str("<td");
            let cs = cell.col_span.min(rng.3 - c + 1);
            let rs = cell.row_span.min(rng.2 - r + 1);
            if cs > 1 {
                out.push_str(&format!(" colspan=\"{cs}\""));
            }
            if rs > 1 {
                out.push_str(&format!(" rowspan=\"{rs}\""));
            }
            out.push_str(&format!(" style='{}'", cell_css(cell)));
            out.push('>');
            out.push_str(&html_cell(&cell.text));
            out.push_str("</td>");
        }
        out.push_str("</tr>");
    }
    out.push_str("</table>");
    out
}

pub fn table_html(t: &Table) -> String {
    range_html(t, (0, 0, rows(t) - 1, cols(t) - 1))
}

/// A cell's visible text: runs of spaces and line breaks in the markup
/// collapse to one space (as a browser shows them), `<br>` and block ends
/// are real line breaks, `&nbsp;` counts as a space. Scripts and styles are
/// ignored; nothing is loaded.
/// Stand for non-breaking spaces while a cell's text is collapsed: one
/// Excel wrote on purpose (`mso-spacerun`), and one that is just `&nbsp;`.
const NBSP: char = '\u{e001}';
const NBSP_PLAIN: char = '\u{e002}';

fn html_cell_text(cell: scraper::ElementRef) -> String {
    use scraper::Node;
    fn walk(node: ego_tree::NodeRef<Node>, out: &mut String) {
        match node.value() {
            Node::Text(t) => {
                let mut prev_space = out.ends_with(' ') || out.ends_with('\n') || out.is_empty();
                for ch in t.chars() {
                    if ch == '\u{a0}' {
                        // A non-breaking space is a real, kept space (Excel
                        // writes leading and repeated spaces this way); one
                        // at the end of a line is only padding.
                        out.push(NBSP_PLAIN);
                        prev_space = false;
                    } else if matches!(ch, ' ' | '\n' | '\r') {
                        if !prev_space {
                            out.push(' ');
                        }
                        prev_space = true;
                    } else {
                        out.push(ch);
                        prev_space = false;
                    }
                }
            }
            Node::Element(e) => {
                let name = e.name();
                if matches!(name, "script" | "style" | "head" | "title") {
                    return;
                }
                // Excel's own marker for spaces it wrote on purpose: every
                // character in it is one space.
                if e.attr("style")
                    .is_some_and(|st| st.contains("mso-spacerun:yes"))
                {
                    for child in node.children() {
                        if let Node::Text(t) = child.value() {
                            out.extend(
                                t.chars()
                                    .filter(|c| matches!(c, ' ' | '\u{a0}'))
                                    .map(|_| NBSP),
                            );
                        }
                    }
                    return;
                }
                if name == "br" {
                    out.push('\n');
                    return;
                }
                if matches!(name, "td" | "th") && !out.is_empty() && !out.ends_with(['\n', ' ']) {
                    out.push(' ');
                }
                let block = matches!(
                    name,
                    "p" | "div" | "li" | "tr" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
                );
                if block && !out.is_empty() && !out.ends_with('\n') {
                    out.push('\n');
                }
                for child in node.children() {
                    walk(child, out);
                }
                if block && !out.ends_with('\n') {
                    out.push('\n');
                }
            }
            _ => {
                for child in node.children() {
                    walk(child, out);
                }
            }
        }
    }
    let mut out = String::new();
    for child in cell.children() {
        walk(child, &mut out);
    }
    out.lines()
        .map(|l| {
            l.trim_start_matches(' ')
                .trim_end_matches([' ', NBSP_PLAIN])
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim_matches('\n')
        .replace([NBSP, NBSP_PLAIN], " ")
}

/// `property:value` pairs, names and values lower-cased.
type Decls = Vec<(String, String)>;

fn parse_decls(s: &str) -> Decls {
    s.split(';')
        .filter_map(|d| {
            let (k, v) = d.split_once(':')?;
            let k = k.trim().to_ascii_lowercase();
            (!k.is_empty()).then(|| (k, v.trim().to_ascii_lowercase()))
        })
        .collect()
}

/// The `<style>` rules Excel writes: `.xl65 { ... }` classes and the
/// defaults for `td`. Nothing is loaded and nothing is executed.
struct Css {
    classes: HashMap<String, Decls>,
    td: Decls,
}

fn parse_css(html: &str) -> Css {
    let mut css = Css {
        classes: HashMap::new(),
        td: vec![],
    };
    let lower = html.to_ascii_lowercase();
    let mut from = 0;
    while let Some(a) = lower[from..].find("<style") {
        let a = from + a;
        let Some(gt) = lower[a..].find('>') else {
            break;
        };
        let start = a + gt + 1;
        let Some(end) = lower[start..].find("</style") else {
            break;
        };
        let end = start + end;
        let text = html[start..end].replace("<!--", "").replace("-->", "");
        for rule in text.split('}') {
            let Some((selectors, body)) = rule.split_once('{') else {
                continue;
            };
            let decls = parse_decls(body);
            for sel in selectors.split(',') {
                let sel = sel.trim().to_ascii_lowercase();
                if sel.starts_with('@') {
                    continue;
                }
                match sel.rsplit_once('.') {
                    Some((_, class)) if !class.is_empty() => css
                        .classes
                        .entry(class.to_owned())
                        .or_default()
                        .extend(decls.clone()),
                    _ if sel == "td" || sel == "th" => css.td.extend(decls.clone()),
                    _ => {}
                }
            }
        }
        from = end;
    }
    css
}

fn parse_color(v: &str) -> Option<[u8; 3]> {
    for tok in v.split_whitespace() {
        if let Some(hex) = tok.strip_prefix('#') {
            let h: Vec<u8> = hex
                .bytes()
                .filter_map(|b| (b as char).to_digit(16).map(|d| d as u8))
                .collect();
            if h.len() != hex.len() {
                continue;
            }
            match h.len() {
                6 => return Some([h[0] * 16 + h[1], h[2] * 16 + h[3], h[4] * 16 + h[5]]),
                3 => return Some([h[0] * 17, h[1] * 17, h[2] * 17]),
                _ => continue,
            }
        }
        if let Some(args) = tok.strip_prefix("rgb(") {
            let n: Vec<u8> = args
                .trim_end_matches(')')
                .split(',')
                .filter_map(|x| x.trim().parse().ok())
                .collect();
            if n.len() == 3 {
                return Some([n[0], n[1], n[2]]);
            }
        }
        let named = match tok {
            "black" => [0, 0, 0],
            "white" => [255, 255, 255],
            "red" => [255, 0, 0],
            "lime" => [0, 255, 0],
            "green" => [0, 128, 0],
            "blue" => [0, 0, 255],
            "yellow" => [255, 255, 0],
            "orange" => [255, 165, 0],
            "gray" | "grey" => [128, 128, 128],
            "silver" => [192, 192, 192],
            "maroon" => [128, 0, 0],
            "purple" => [128, 0, 128],
            "fuchsia" | "magenta" => [255, 0, 255],
            "aqua" | "cyan" => [0, 255, 255],
            "navy" => [0, 0, 128],
            "teal" => [0, 128, 128],
            "olive" => [128, 128, 0],
            "brown" => [165, 42, 42],
            "pink" => [255, 192, 203],
            _ => continue,
        };
        return Some(named);
    }
    None
}

/// Fill, text color, alignment and emphasis of one `<td>`: presentation
/// attributes first, then the `td` defaults, then its classes, then its
/// own `style` (the last one wins). Black and "windowtext" text is the
/// default color, so it stays readable on a dark theme.
fn cell_look(
    el: scraper::ElementRef,
    css: &Css,
) -> (Option<[u8; 3]>, Option<[u8; 3]>, CellAlign, Format) {
    let mut decls: Decls = vec![];
    if let Some(b) = el.value().attr("bgcolor") {
        decls.push(("background-color".into(), b.trim().to_ascii_lowercase()));
    }
    if let Some(a) = el.value().attr("align") {
        decls.push(("text-align".into(), a.trim().to_ascii_lowercase()));
    }
    decls.extend(css.td.iter().cloned());
    for class in el.value().attr("class").unwrap_or("").split_whitespace() {
        if let Some(d) = css.classes.get(&class.to_ascii_lowercase()) {
            decls.extend(d.iter().cloned());
        }
    }
    if let Some(st) = el.value().attr("style") {
        decls.extend(parse_decls(st));
    }
    let get = |k: &str| {
        decls
            .iter()
            .rev()
            .find(|(n, _)| n == k)
            .map(|(_, v)| v.as_str())
    };
    let fill = get("background-color")
        .and_then(parse_color)
        .or_else(|| get("background").and_then(parse_color));
    let color = get("color")
        .and_then(parse_color)
        .filter(|c| *c != [0, 0, 0]);
    // `text-align:general` is Excel's own value, not CSS: a browser ignores
    // it, so an earlier (or `align=`) value stays.
    let align_value = decls
        .iter()
        .rev()
        .find(|(n, v)| {
            n == "text-align"
                && matches!(
                    v.as_str(),
                    "left" | "center" | "right" | "justify" | "start" | "end"
                )
        })
        .map(|(_, v)| v.as_str());
    let align = match align_value {
        Some("left") => CellAlign::Left,
        Some("center") => CellAlign::Center,
        Some("right") => CellAlign::Right,
        _ => CellAlign::Default,
    };
    let deco = get("text-decoration").unwrap_or("");
    let format = Format {
        bold: get("font-weight").is_some_and(|w| {
            matches!(w, "bold" | "bolder") || w.parse::<u32>().is_ok_and(|n| n >= 600)
        }),
        italic: get("font-style").is_some_and(|v| v.contains("italic") || v.contains("oblique")),
        underline: deco.contains("underline"),
        strike: deco.contains("line-through"),
        ..Default::default()
    };
    (fill, color, align, format)
}

fn html_number(v: Option<&str>) -> Option<usize> {
    v?.trim()
        .trim_end_matches("px")
        .trim()
        .parse::<usize>()
        .ok()
}

/// Reads the table on the clipboard into a table, the way a browser lays
/// it out: each cell goes to the first free column of its row,
/// `colspan`/`rowspan` become real merges, nothing may overlap or leave the
/// table, a cell's own tabs and line breaks stay inside the cell. Column
/// widths come from `<col width>` when present. Rows and columns beyond the
/// limits are cut.
///
/// Copies from mail programs and web pages are rarely as tidy as Excel's:
/// * rows/cells copied without their `<table>` (or without their `<tr>`)
///   are wrapped so they still read as a table;
/// * with several tables (a layout table around the data, a table inside a
///   cell) the one with the most cells is taken; a table left inside a cell
///   of the chosen one becomes that cell's text;
/// * when the copy started in the middle of the first row, that row is
///   shorter than the rest: its missing leading cells are assumed and left
///   empty, so the cells stay in their own columns;
/// * rows that end early are filled up with empty cells.
pub fn table_from_html(html: &str) -> Option<Table> {
    use scraper::{ElementRef, Html, Selector};
    let lower = html.to_ascii_lowercase();
    let wrapped;
    let html = if lower.contains("<table") {
        html
    } else if lower.contains("<tr") {
        wrapped = format!("<table>{html}</table>");
        &wrapped
    } else if lower.contains("<td") || lower.contains("<th") {
        wrapped = format!("<table><tr>{html}</tr></table>");
        &wrapped
    } else {
        return None;
    };
    let css = parse_css(html);
    let doc = Html::parse_fragment(html);
    let table_sel = Selector::parse("table").ok()?;
    let tr_sel = Selector::parse("tr").ok()?;
    let col_sel = Selector::parse("col").ok()?;
    let cell_sel = Selector::parse("td, th").ok()?;
    let nearest_table = |e: ElementRef| {
        e.ancestors()
            .filter_map(ElementRef::wrap)
            .find(|a| a.value().name() == "table")
            .map(|a| a.id())
    };
    // The table with the most cells of its own (first one on a tie).
    let mut own: HashMap<_, usize> = HashMap::new();
    for cell in doc.select(&cell_sel) {
        if let Some(t) = nearest_table(cell) {
            *own.entry(t).or_default() += 1;
        }
    }
    let mut table_el = None;
    let mut best = 0;
    for t in doc.select(&table_sel) {
        let n = own.get(&t.id()).copied().unwrap_or(0);
        if n > best {
            best = n;
            table_el = Some(t);
        }
    }
    let table_el = table_el?;
    let tid = table_el.id();
    let trs: Vec<ElementRef> = table_el
        .select(&tr_sel)
        .filter(|tr| nearest_table(*tr) == Some(tid))
        .take(MAX_ROWS)
        .collect();
    let (mut lcs, mut n_rows) = layout_rows(&trs, &css, 0);
    // A first row shorter than the later ones: its leading cells were not copied.
    let row0 = lcs
        .iter()
        .filter(|l| l.r == 0)
        .map(|l| l.c + l.cs)
        .max()
        .unwrap_or(0);
    let later = lcs
        .iter()
        .filter(|l| l.r > 0)
        .map(|l| l.c + l.cs)
        .max()
        .unwrap_or(0);
    // How many later rows reach that width on their own, and whether they
    // do it with plain cells (a single wide merged row proves nothing).
    let full_rows = (1..n_rows)
        .filter(|&r| {
            let mine: Vec<&Lc> = lcs.iter().filter(|l| l.r == r).collect();
            mine.iter().map(|l| l.c + l.cs).max() == Some(later)
                && mine.iter().all(|l| l.cs == 1 && l.rs == 1)
        })
        .count();
    if n_rows > 1 && row0 > 0 && row0 < later && full_rows >= 1 && (full_rows >= 2 || n_rows == 2) {
        (lcs, n_rows) = layout_rows(&trs, &css, (later - row0).min(MAX_COLS - 1));
    }
    let n_cols = lcs.iter().map(|l| l.c + l.cs).max().unwrap_or(0);
    if n_rows == 0 || n_cols == 0 {
        return None;
    }
    let mut widths: Vec<f32> = Vec::new();
    for col in table_el.select(&col_sel) {
        if nearest_table(col) != Some(tid) {
            continue;
        }
        let w = html_number(col.value().attr("width")).map(|w| (w as f32).clamp(60., 400.));
        let span = html_number(col.value().attr("span"))
            .unwrap_or(1)
            .clamp(1, MAX_COLS);
        for _ in 0..span {
            widths.push(w.unwrap_or(120.));
        }
    }
    let mut t = Table::new(1, n_cols);
    rebuild(&mut t, n_rows, n_cols, lcs);
    for (i, w) in t.col_widths.iter_mut().enumerate() {
        *w = widths.get(i).copied().unwrap_or(120.);
    }
    Some(t)
}

/// Places the cells of the rows on the grid. `first_offset` is where the
/// first row starts (assumed, uncopied cells before it).
fn layout_rows(trs: &[scraper::ElementRef], css: &Css, first_offset: usize) -> (Vec<Lc>, usize) {
    use scraper::ElementRef;
    let mut occupied: Vec<Vec<bool>> = vec![vec![false; MAX_COLS]; trs.len().max(1)];
    let mut lcs: Vec<Lc> = Vec::new();
    for (r, tr) in trs.iter().enumerate() {
        let mut col = if r == 0 { first_offset } else { 0 };
        for cell in tr
            .children()
            .filter_map(ElementRef::wrap)
            .filter(|e| matches!(e.value().name(), "td" | "th"))
        {
            while col < MAX_COLS && occupied[r][col] {
                col += 1;
            }
            if col >= MAX_COLS {
                break;
            }
            let want_cs = html_number(cell.value().attr("colspan"))
                .unwrap_or(1)
                .clamp(1, MAX_COLS - col);
            let want_rs = html_number(cell.value().attr("rowspan"))
                .unwrap_or(1)
                .clamp(1, MAX_ROWS - r);
            let cs = (col..col + want_cs)
                .take_while(|&c| !occupied[r][c])
                .count()
                .max(1);
            if occupied.len() < r + want_rs {
                occupied.resize(r + want_rs, vec![false; MAX_COLS]);
            }
            let mut rs = 1;
            while rs < want_rs && (col..col + cs).all(|c| !occupied[r + rs][c]) {
                rs += 1;
            }
            for row in &mut occupied[r..r + rs] {
                for slot in &mut row[col..col + cs] {
                    *slot = true;
                }
            }
            let (fill, text_color, align, format) = cell_look(cell, css);
            lcs.push(Lc {
                r,
                c: col,
                rs,
                cs,
                cell: Cell {
                    text: html_cell_text(cell),
                    fill,
                    text_color,
                    align,
                    format,
                    ..Cell::plain()
                },
            });
            col += cs;
        }
    }
    (lcs, trs.len())
}

/// Tab separated text with Excel's quoting. A quoted value may hold tabs,
/// line breaks and doubled quotes. A trailing line break ends the last row.
pub fn parse_tsv(text: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut cell = String::new();
    let mut chars = text.chars().peekable();
    let mut start = true;
    let mut quoted = false;
    while let Some(c) = chars.next() {
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    cell.push('"');
                } else {
                    quoted = false;
                }
            } else {
                cell.push(c);
            }
            continue;
        }
        match c {
            '"' if start => {
                quoted = true;
                start = false;
            }
            '\t' => {
                row.push(std::mem::take(&mut cell));
                start = true;
            }
            '\r' => {}
            '\n' => {
                row.push(std::mem::take(&mut cell));
                rows.push(std::mem::take(&mut row));
                start = true;
            }
            _ => {
                cell.push(c);
                start = false;
            }
        }
    }
    if !cell.is_empty() || !row.is_empty() {
        row.push(cell);
        rows.push(row);
    }
    rows
}

pub fn table_from_grid(grid: Vec<Vec<String>>) -> Table {
    let grid: Vec<Vec<String>> = grid.into_iter().take(MAX_ROWS).collect();
    let n_cols = grid
        .iter()
        .map(Vec::len)
        .max()
        .unwrap_or(1)
        .clamp(1, MAX_COLS);
    let mut t = Table::new(grid.len().max(1), n_cols);
    for (r, row) in grid.into_iter().enumerate() {
        for (c, text) in row.into_iter().take(MAX_COLS).enumerate() {
            t.cells[r][c].text = text;
        }
    }
    t
}

// --------------------------------------------------------------- editing ----

pub fn clear_range(t: &mut Table, rng: Rng) {
    for r in rng.0..=rng.2 {
        for c in rng.1..=rng.3 {
            if !t.cells[r][c].covered {
                t.cells[r][c].text.clear();
            }
        }
    }
}

/// Merge the whole rectangle into its top-left cell. The texts of the other
/// cells are kept, one per line, so nothing is lost. False when the range
/// is a single logical cell.
pub fn merge_range(t: &mut Table, rng: Rng) -> bool {
    let (r0, c0, r1, c1) = expand(t, rng);
    if (r0, c0) == (r1, c1) || span_rect(t, r0, c0) == (r0, c0, r1, c1) {
        return false;
    }
    let mut lines: Vec<String> = Vec::new();
    for r in r0..=r1 {
        for c in c0..=c1 {
            let cell = &mut t.cells[r][c];
            if !cell.covered && !cell.text.is_empty() {
                lines.push(std::mem::take(&mut cell.text));
            }
        }
    }
    let proto = t.cells[r0][c0].clone();
    for r in r0..=r1 {
        for c in c0..=c1 {
            t.cells[r][c] = Cell {
                covered: true,
                ..Cell::plain()
            };
        }
    }
    t.cells[r0][c0] = Cell {
        text: lines.join("\n"),
        col_span: c1 - c0 + 1,
        row_span: r1 - r0 + 1,
        covered: false,
        ..proto
    };
    true
}

/// The merge owned by (r, c) becomes plain cells again.
pub fn unmerge(t: &mut Table, r: usize, c: usize) -> bool {
    let (or, oc) = owner_of(t, r, c);
    let (r0, c0, r1, c1) = span_rect(t, or, oc);
    if (r0, c0) == (r1, c1) {
        return false;
    }
    for rr in r0..=r1 {
        for cc in c0..=c1 {
            if (rr, cc) != (r0, c0) {
                t.cells[rr][cc] = Cell::plain();
            }
        }
    }
    t.cells[r0][c0].row_span = 1;
    t.cells[r0][c0].col_span = 1;
    true
}

struct Lc {
    r: usize,
    c: usize,
    rs: usize,
    cs: usize,
    cell: Cell,
}

fn logical(t: &Table) -> Vec<Lc> {
    let mut out = vec![];
    for r in 0..rows(t) {
        for c in 0..cols(t) {
            let cell = &t.cells[r][c];
            if !cell.covered {
                out.push(Lc {
                    r,
                    c,
                    rs: cell.row_span.max(1),
                    cs: cell.col_span.max(1),
                    cell: cell.clone(),
                });
            }
        }
    }
    out
}

#[allow(clippy::needless_range_loop)]
fn rebuild(t: &mut Table, n_rows: usize, n_cols: usize, lcs: Vec<Lc>) {
    let mut grid = vec![vec![Cell::plain(); n_cols]; n_rows];
    for lc in lcs {
        if lc.r >= n_rows || lc.c >= n_cols {
            continue;
        }
        let rs = lc.rs.min(n_rows - lc.r);
        let cs = lc.cs.min(n_cols - lc.c);
        for r in lc.r..lc.r + rs {
            for c in lc.c..lc.c + cs {
                grid[r][c] = Cell {
                    covered: true,
                    ..Cell::plain()
                };
            }
        }
        grid[lc.r][lc.c] = Cell {
            row_span: rs,
            col_span: cs,
            covered: false,
            ..lc.cell
        };
    }
    t.cells = grid;
}

/// New row before `at` (`at == rows` appends). A merge crossing the new row
/// grows by one row.
pub fn insert_row(t: &mut Table, at: usize) -> bool {
    if rows(t) >= MAX_ROWS || at > rows(t) {
        return false;
    }
    let mut lcs = logical(t);
    for lc in &mut lcs {
        if lc.r >= at {
            lc.r += 1;
        } else if lc.r + lc.rs > at {
            lc.rs += 1;
        }
    }
    let (n_rows, n_cols) = (rows(t) + 1, cols(t));
    rebuild(t, n_rows, n_cols, lcs);
    t.fit_rows();
    t.row_manual.insert(at.min(t.row_manual.len()), 0.);
    t.row_manual.truncate(n_rows);
    true
}

pub fn insert_col(t: &mut Table, at: usize) -> bool {
    if cols(t) >= MAX_COLS || at > cols(t) {
        return false;
    }
    let mut lcs = logical(t);
    for lc in &mut lcs {
        if lc.c >= at {
            lc.c += 1;
        } else if lc.c + lc.cs > at {
            lc.cs += 1;
        }
    }
    let (n_rows, n_cols) = (rows(t), cols(t) + 1);
    rebuild(t, n_rows, n_cols, lcs);
    t.col_widths.insert(at.min(t.col_widths.len()), 120.);
    true
}

/// Remove rows r0..=r1 (never the last row). A merge loses the rows it had
/// there; one that started inside the band continues below it.
pub fn delete_rows(t: &mut Table, r0: usize, r1: usize) -> bool {
    let n = r1 + 1 - r0;
    if r1 >= rows(t) || n >= rows(t) {
        return false;
    }
    let mut lcs = Vec::new();
    for mut lc in logical(t) {
        let overlap = (lc.r + lc.rs).min(r1 + 1).saturating_sub(lc.r.max(r0));
        let left = lc.rs - overlap;
        if left == 0 {
            continue;
        }
        lc.r = if lc.r < r0 {
            lc.r
        } else if lc.r > r1 {
            lc.r - n
        } else {
            r0
        };
        lc.rs = left;
        lcs.push(lc);
    }
    let (n_rows, n_cols) = (rows(t) - n, cols(t));
    t.fit_rows();
    t.row_manual.drain(r0..=r1);
    rebuild(t, n_rows, n_cols, lcs);
    t.fit_rows();
    true
}

pub fn delete_cols(t: &mut Table, c0: usize, c1: usize) -> bool {
    let n = c1 + 1 - c0;
    if c1 >= cols(t) || n >= cols(t) {
        return false;
    }
    let mut lcs = Vec::new();
    for mut lc in logical(t) {
        let overlap = (lc.c + lc.cs).min(c1 + 1).saturating_sub(lc.c.max(c0));
        let left = lc.cs - overlap;
        if left == 0 {
            continue;
        }
        lc.c = if lc.c < c0 {
            lc.c
        } else if lc.c > c1 {
            lc.c - n
        } else {
            c0
        };
        lc.cs = left;
        lcs.push(lc);
    }
    let (n_rows, n_cols) = (rows(t), cols(t) - n);
    rebuild(t, n_rows, n_cols, lcs);
    for _ in c0..=c1 {
        if c0 < t.col_widths.len() {
            t.col_widths.remove(c0);
        }
    }
    true
}

/// Paste a table at (r, c). The table grows (up to the limits) to hold it;
/// merges under the pasted area are dissolved first. Returns the area that
/// was filled.
pub fn paste_table(t: &mut Table, r: usize, c: usize, src: &Table) -> Rng {
    while rows(t) < r + rows(src) && insert_row(t, rows(t)) {}
    while cols(t) < c + cols(src) && insert_col(t, cols(t)) {}
    let sr = rows(src).min(rows(t) - r);
    let sc = cols(src).min(cols(t) - c);
    for rr in r..r + sr {
        for cc in c..c + sc {
            let (or, oc) = owner_of(t, rr, cc);
            unmerge(t, or, oc);
        }
    }
    for i in 0..sr {
        for j in 0..sc {
            let mut cell = src.cells[i][j].clone();
            cell.col_span = cell.col_span.min(sc - j).max(1);
            cell.row_span = cell.row_span.min(sr - i).max(1);
            t.cells[r + i][c + j] = cell;
        }
    }
    (r, c, r + sr - 1, c + sc - 1)
}

/// Checks the invariants: every position belongs to exactly one logical
/// cell, spans stay inside the table.
#[cfg(test)]
#[allow(clippy::needless_range_loop)]
pub fn is_valid(t: &Table) -> bool {
    let (nr, nc) = (rows(t), cols(t));
    if nr == 0 || nc == 0 || t.cells.iter().any(|r| r.len() != nc) || t.col_widths.len() != nc {
        return false;
    }
    let mut owner = vec![vec![0u8; nc]; nr];
    for r in 0..nr {
        for c in 0..nc {
            let cell = &t.cells[r][c];
            if cell.covered {
                continue;
            }
            if cell.row_span < 1
                || cell.col_span < 1
                || r + cell.row_span > nr
                || c + cell.col_span > nc
            {
                return false;
            }
            for rr in r..r + cell.row_span {
                for cc in c..c + cell.col_span {
                    owner[rr][cc] += 1;
                    if (rr, cc) != (r, c) && !t.cells[rr][cc].covered {
                        return false;
                    }
                }
            }
        }
    }
    owner.iter().flatten().all(|&n| n == 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(rows: usize, cols: usize) -> Table {
        let mut t = Table::new(rows, cols);
        for r in 0..rows {
            for c in 0..cols {
                t.cells[r][c].text = format!("{r}{c}");
            }
        }
        t
    }

    #[test]
    fn tsv_quotes_tabs_newlines_and_quotes_like_excel() {
        assert_eq!(tsv_quote("plain"), "plain");
        assert_eq!(tsv_quote("a\tb"), "\"a\tb\"");
        assert_eq!(tsv_quote("줄1\r\n줄2"), "\"줄1\n줄2\"");
        assert_eq!(
            tsv_quote("그가 \"안녕\" 했다"),
            "\"그가 \"\"안녕\"\" 했다\""
        );
        let g = vec![
            vec!["a".to_string(), "x\ty".into(), "".into()],
            vec!["q\"r\ns".into(), "b".into(), "c".into()],
        ];
        assert_eq!(parse_tsv(&grid_tsv(&g)), g);
    }

    #[test]
    fn excel_tsv_parses_blank_cells_and_trailing_newline() {
        let g = parse_tsv("a\t\tc\r\n\t\t\r\n");
        assert_eq!(g, vec![vec!["a", "", "c"], vec!["", "", ""]]);
        assert_eq!(parse_tsv("\"안\n녕\"\r\n"), vec![vec!["안\n녕"]]);
    }

    #[test]
    fn html_escapes_quotes_and_keeps_line_breaks_in_the_cell() {
        let mut t = Table::new(1, 3);
        t.cells[0][0].text = "He said \"hi\" & <b>".into();
        t.cells[0][1].text = "줄1\n줄2".into();
        t.cells[0][2].text = "007".into();
        let h = table_html(&t);
        assert!(h.contains("He said &quot;hi&quot; &amp; &lt;b&gt;"));
        assert!(h.contains("줄1<br style='mso-data-placement:same-cell;'>줄2"));
        assert!(h.contains("mso-number-format:\"\\@\";'>007</td>"));
        assert!(
            !h.contains("\"hi\""),
            "no raw double quote inside cell text"
        );
    }

    #[test]
    fn copied_html_keeps_leading_and_repeated_spaces_when_read_back() {
        let mut t = Table::new(1, 2);
        t.cells[0][0].text = "병합1\n 줄바꿈 앞 1칸\n  앞 2칸  \n끝 ".into();
        t.cells[0][1].text = "그냥 한 칸".into();
        let html = table_html(&t);
        assert!(html.contains("<span style='mso-spacerun:yes'>&nbsp;</span>줄바꿈 앞 1칸"));
        assert!(
            html.contains("그냥 한 칸"),
            "a single inner space stays a plain space"
        );
        let back = table_from_html(&html).unwrap();
        assert_eq!(back.cells[0][0].text, t.cells[0][0].text);
        assert_eq!(back.cells[0][1].text, "그냥 한 칸");
    }

    #[test]
    fn html_keeps_merges_of_the_copied_range() {
        let mut t = grid(3, 3);
        assert!(merge_range(&mut t, (0, 0, 1, 1)));
        let h = range_html(&t, (0, 0, 2, 2));
        assert!(h.contains("colspan=\"2\" rowspan=\"2\""));
        // copying only a part cuts the merge at the edge, never past it
        let part = expand(&t, (0, 0, 0, 0));
        assert_eq!(part, (0, 0, 1, 1));
    }

    #[test]
    fn merge_keeps_text_and_unmerge_restores_cells() {
        let mut t = grid(3, 3);
        assert!(merge_range(&mut t, (0, 0, 1, 1)));
        assert_eq!(t.cells[0][0].text, "00\n01\n10\n11");
        assert_eq!((t.cells[0][0].row_span, t.cells[0][0].col_span), (2, 2));
        assert!(is_valid(&t));
        assert_eq!(owner_of(&t, 1, 1), (0, 0));
        assert!(unmerge(&mut t, 1, 1));
        assert!(is_valid(&t) && !t.cells[1][1].covered);
        assert!(!merge_range(&mut t, (2, 2, 2, 2)));
    }

    #[test]
    fn range_expands_over_merges() {
        let mut t = grid(4, 4);
        merge_range(&mut t, (1, 1, 2, 2));
        assert_eq!(expand(&t, (0, 0, 1, 1)), (0, 0, 2, 2));
        assert_eq!(expand(&t, (2, 2, 3, 3)), (1, 1, 3, 3));
    }

    #[test]
    fn row_insert_inside_a_merge_grows_it_and_outside_keeps_it() {
        let mut t = grid(4, 3);
        merge_range(&mut t, (1, 0, 2, 1));
        assert!(insert_row(&mut t, 2)); // inside the merge
        assert_eq!(t.cells[1][0].row_span, 3);
        assert!(is_valid(&t));
        assert!(insert_row(&mut t, 0)); // above it
        assert_eq!(t.cells[2][0].row_span, 3);
        assert!(is_valid(&t));
        assert_eq!(rows(&t), 6);
    }

    #[test]
    fn column_insert_and_delete_with_merges_stay_valid() {
        let mut t = grid(3, 4);
        merge_range(&mut t, (0, 1, 1, 2));
        assert!(insert_col(&mut t, 2));
        assert_eq!(t.cells[0][1].col_span, 3);
        assert_eq!(t.col_widths.len(), 5);
        assert!(is_valid(&t));
        assert!(delete_cols(&mut t, 1, 1));
        assert_eq!(t.cells[0][1].col_span, 2);
        assert_eq!(t.col_widths.len(), 4);
        assert!(is_valid(&t));
    }

    #[test]
    fn deleting_the_owner_row_moves_the_merge_down() {
        let mut t = grid(4, 2);
        merge_range(&mut t, (1, 0, 2, 0));
        assert!(delete_rows(&mut t, 1, 1));
        assert_eq!(t.cells[1][0].row_span, 1);
        assert_eq!(t.cells[1][0].text, "10\n20");
        assert!(is_valid(&t));
        assert!(delete_rows(&mut t, 0, 1));
        assert!(is_valid(&t));
        assert!(!delete_rows(&mut t, 0, 1), "the last row cannot be removed");
    }

    #[test]
    fn deleting_the_whole_merge_removes_it() {
        let mut t = grid(4, 3);
        merge_range(&mut t, (1, 1, 2, 2));
        assert!(delete_rows(&mut t, 1, 2));
        assert!(is_valid(&t));
        assert_eq!(rows(&t), 2);
        assert!(t.cells.iter().flatten().all(|c| !c.covered));
    }

    #[test]
    fn paste_grows_the_table_and_dissolves_merges_under_it() {
        let mut t = grid(2, 2);
        merge_range(&mut t, (0, 0, 0, 1));
        let mut src = Table::new(3, 3);
        src.cells[0][0].text = "x".into();
        src.cells[2][2].text = "z".into();
        let area = paste_table(&mut t, 0, 1, &src);
        assert_eq!(area, (0, 1, 2, 3));
        assert_eq!((rows(&t), cols(&t)), (3, 4));
        assert_eq!(t.cells[0][1].text, "x");
        assert_eq!(t.cells[2][3].text, "z");
        assert!(is_valid(&t));
        assert_eq!(
            t.cells[0][0].col_span, 1,
            "the merge under the paste was dissolved"
        );
    }

    #[test]
    fn paste_keeps_source_merges_and_respects_limits() {
        let mut t = Table::new(2, 2);
        let mut src = grid(2, 3);
        merge_range(&mut src, (0, 0, 0, 1));
        paste_table(&mut t, 0, 0, &src);
        assert_eq!(t.cells[0][0].col_span, 2);
        assert!(is_valid(&t));
        let big = Table::new(100, 20);
        let mut small = Table::new(1, 1);
        paste_table(&mut small, 50, 0, &big);
        assert!(rows(&small) <= MAX_ROWS && cols(&small) <= MAX_COLS);
    }

    #[test]
    fn stepping_crosses_merges_and_wraps_tab_order() {
        let mut t = grid(3, 3);
        merge_range(&mut t, (0, 0, 1, 1));
        assert_eq!(step(&t, (1, 1), 3), (0, 2));
        assert_eq!(step(&t, (0, 0), 1), (2, 0));
        assert_eq!(step(&t, (0, 2), 2), (0, 0));
        assert_eq!(step(&t, (0, 0), 0), (0, 0));
        assert_eq!(next_cell(&t, (0, 2), false), (0, 0)); // next row starts inside the merge
        assert_eq!(next_cell(&t, (2, 2), false), (2, 2));
        assert_eq!(next_cell(&t, (2, 0), true), (1, 2));
    }

    #[test]
    fn html_layout_follows_the_browser_algorithm() {
        let h = "<table><col width=64><col width=200><tr><td rowspan=2>가</td><td colspan=2>나</td></tr>\
                 <tr><td>다</td><td>라</td></tr></table>";
        let t = table_from_html(h).unwrap();
        assert_eq!((rows(&t), cols(&t)), (2, 3));
        assert_eq!((t.cells[0][0].row_span, t.cells[0][1].col_span), (2, 2));
        assert_eq!(t.cells[1][1].text, "다");
        assert_eq!(t.cells[1][2].text, "라");
        assert!(is_valid(&t));
        assert_eq!(t.col_widths, vec![64.0, 200.0, 120.0]);
    }

    #[test]
    fn html_cells_collapse_markup_whitespace_but_keep_real_breaks() {
        let h = "<table>\n<tr>\n <td>\n   한 줄\n   이어서&nbsp;쓴다<br style='mso-data-placement:same-cell;'>둘째 줄\n </td>\n<td>가\t나</td></tr></table>";
        let t = table_from_html(h).unwrap();
        assert_eq!(t.cells[0][0].text, "한 줄 이어서 쓴다\n둘째 줄");
        assert_eq!(t.cells[0][1].text, "가\t나");
    }

    #[test]
    fn excel_fragment_without_the_table_tag_is_read_and_spaces_are_kept() {
        // What the clipboard gives: the part after Excel's <table> tag.
        let h = "\r\n <col width=265 style='width:198pt'>\r\n <col width=72 span=3>\r\n <tr>\r\n  <td rowspan=3 class=xl81>병합1<br />\r\n    <span style='mso-spacerun:yes'>&nbsp;</span>줄바꿈 뒤 1칸<br />\r\n    <span style='mso-spacerun:yes'>&nbsp; </span>2칸</td>\r\n  <td>가</td><td>나</td><td rowspan=2>병합3</td>\r\n </tr>\r\n <tr><td>다</td><td>라</td></tr>\r\n <tr><td colspan=2>병합2</td><td>마</td></tr>\r\n</table>";
        let t = table_from_html(h).unwrap();
        assert_eq!((rows(&t), cols(&t)), (3, 4));
        assert!(is_valid(&t));
        assert_eq!(t.cells[0][0].text, "병합1\n 줄바꿈 뒤 1칸\n  2칸");
        assert_eq!(t.cells[0][0].row_span, 3);
        assert_eq!(
            (t.cells[2][1].col_span, t.cells[2][1].text.as_str()),
            (2, "병합2")
        );
        assert_eq!(t.cells[2][3].text, "마");
        assert_eq!(t.col_widths, vec![265.0, 72.0, 72.0, 72.0]);
        assert!(table_from_html("<p>x</p>").is_none());
    }

    #[test]
    fn html_overlapping_or_oversized_spans_are_made_valid() {
        let h =
            "<table><tr><td rowspan=5>a</td><td>b</td></tr><tr><td colspan=9>c</td></tr></table>";
        let t = table_from_html(h).unwrap();
        assert!(is_valid(&t));
        assert_eq!(rows(&t), 2);
        assert_eq!(
            t.cells[0][0].row_span, 2,
            "a rowspan past the last row is cut to the table"
        );
        let nested =
            "<table><tr><td>바깥<table><tr><td>안</td><td>쪽</td></tr></table></td></tr></table>";
        let t = table_from_html(nested).unwrap();
        assert_eq!(
            (rows(&t), cols(&t)),
            (1, 2),
            "the table with more cells is the data"
        );
        assert!(table_from_html("<p>표 아님</p>").is_none());
    }

    #[test]
    fn html_ignores_scripts_and_cuts_to_the_limits() {
        let mut h = String::from("<table>");
        for _ in 0..150 {
            h.push_str("<tr>");
            for _ in 0..30 {
                h.push_str("<td>x<script>alert(1)</script></td>");
            }
            h.push_str("</tr>");
        }
        h.push_str("</table>");
        let t = table_from_html(&h).unwrap();
        assert_eq!((rows(&t), cols(&t)), (MAX_ROWS, MAX_COLS));
        assert_eq!(t.cells[0][0].text, "x");
        assert!(is_valid(&t));
    }

    /// A trimmed copy of what Excel puts on the clipboard: a `<style>` block
    /// with classes, and cells that only name their class.
    const EXCEL_STYLED: &str = "<html><head><style>\r\n<!--table\r\n\t{mso-displayed-decimal-separator:\"\\.\";}\r\n@page\r\n\t{margin:.75in .7in .75in .7in;}\r\ntd\r\n\t{padding-top:1px;color:black;font-weight:400;text-align:general;vertical-align:middle;border:none;white-space:nowrap;}\r\n.xl65\r\n\t{color:black;text-align:center;border-top:.5pt solid windowtext;}\r\n.xl68\r\n\t{color:red;text-align:center;}\r\n.xl71\r\n\t{color:black;text-align:center;background:yellow;mso-pattern:black none;}\r\n.xl73\r\n\t{color:black;font-weight:700;text-align:right;}\r\n.xl75\r\n\t{color:#0070C0;text-decoration:underline;text-underline-style:single;text-align:left;background:#C6E0B4;}\r\n.xl77\r\n\t{font-style:italic;}\r\n-->\r\n</style></head><body><table>\r\n <col width=64>\r\n <tr><td class=xl71>노랑</td><td class=xl68>빨강</td><td class=xl73>굵게 오른쪽</td></tr>\r\n <tr><td class=xl75>파랑 밑줄 초록바탕</td><td class=xl77 style='background:#ff00ff;color:rgb(10,20,30)'>인라인</td><td class=xl65>보통</td></tr>\r\n <tr><td>없음</td><td bgcolor=\"#ABCDEF\" align=right>속성</td><td style='background:none'>none</td></tr>\r\n</table></body></html>";

    #[test]
    fn excel_classes_give_fill_text_color_alignment_and_emphasis() {
        let t = table_from_html(EXCEL_STYLED).unwrap();
        assert!(is_valid(&t));
        let c = |r: usize, k: usize| &t.cells[r][k];
        assert_eq!(
            (c(0, 0).fill, c(0, 0).text_color, c(0, 0).align),
            (Some([255, 255, 0]), None, CellAlign::Center)
        );
        assert_eq!(
            (c(0, 1).fill, c(0, 1).text_color),
            (None, Some([255, 0, 0]))
        );
        assert_eq!(
            (c(0, 2).align, c(0, 2).format.bold),
            (CellAlign::Right, true)
        );
        assert_eq!(c(1, 0).text_color, Some([0x00, 0x70, 0xC0]));
        assert_eq!(c(1, 0).fill, Some([0xC6, 0xE0, 0xB4]));
        assert_eq!(
            (c(1, 0).align, c(1, 0).format.underline),
            (CellAlign::Left, true)
        );
        // inline style beats the class; rgb() works; italic comes from the class
        assert_eq!(
            (c(1, 1).fill, c(1, 1).text_color, c(1, 1).format.italic),
            (Some([255, 0, 255]), Some([10, 20, 30]), true)
        );
        assert_eq!(c(1, 2).align, CellAlign::Center);
        // the td default (text-align:general, black) means no explicit look
        assert_eq!(
            (c(2, 0).fill, c(2, 0).text_color, c(2, 0).align),
            (None, None, CellAlign::Default)
        );
        // presentation attributes, and an explicit "none" background
        assert_eq!(
            (c(2, 1).fill, c(2, 1).align),
            (Some([0xAB, 0xCD, 0xEF]), CellAlign::Right)
        );
        assert_eq!(c(2, 2).fill, None);
    }

    #[test]
    fn copied_html_carries_black_border_fill_color_and_alignment_and_reads_back() {
        let mut t = Table::new(1, 2);
        t.cells[0][0].text = "가".into();
        t.cells[0][0].fill = Some([255, 255, 0]);
        t.cells[0][0].text_color = Some([255, 0, 0]);
        t.cells[0][0].align = CellAlign::Center;
        t.cells[0][0].format.bold = true;
        t.cells[0][1].align = CellAlign::Right;
        let h = table_html(&t);
        assert!(h.contains("border:.5pt solid black;"));
        assert!(h.contains("background:#FFFF00;mso-pattern:black none;color:#FF0000;text-align:center;font-weight:700;"));
        assert!(h.contains("text-align:right;"));
        let back = table_from_html(&h).unwrap();
        assert_eq!(
            (
                back.cells[0][0].fill,
                back.cells[0][0].text_color,
                back.cells[0][0].align
            ),
            (Some([255, 255, 0]), Some([255, 0, 0]), CellAlign::Center)
        );
        assert!(back.cells[0][0].format.bold);
        assert_eq!(back.cells[0][1].align, CellAlign::Right);
    }

    #[test]
    fn merge_keeps_the_owner_cells_look_and_paste_copies_it() {
        let mut t = Table::new(2, 2);
        t.cells[0][0].fill = Some([1, 2, 3]);
        t.cells[0][0].align = CellAlign::Right;
        assert!(merge_range(&mut t, (0, 0, 1, 1)));
        assert_eq!(
            (t.cells[0][0].fill, t.cells[0][0].align),
            (Some([1, 2, 3]), CellAlign::Right)
        );
        let mut dest = Table::new(1, 1);
        paste_table(&mut dest, 0, 0, &t);
        assert_eq!(dest.cells[0][0].fill, Some([1, 2, 3]));
    }

    fn texts(t: &Table) -> Vec<Vec<String>> {
        t.cells
            .iter()
            .map(|r| r.iter().map(|c| c.text.clone()).collect())
            .collect()
    }

    #[test]
    fn a_layout_table_around_the_data_table_is_unwrapped() {
        let h = "<table><tr><td>사이드</td><td><table><tr><td>a</td><td>b</td><td>c</td></tr><tr><td>d</td><td>e</td><td>f</td></tr></table></td></tr></table>";
        let t = table_from_html(h).unwrap();
        assert_eq!(texts(&t), vec![vec!["a", "b", "c"], vec!["d", "e", "f"]]);
        assert!(is_valid(&t));
        // a small table inside a cell of the data table stays that cell's text
        let h2 = "<table><tr><td>가</td><td>나</td></tr><tr><td>다</td><td><table><tr><td>안1</td><td>안2</td></tr><tr><td>안3</td><td>안4</td></tr></table></td></tr></table>";
        let t2 = table_from_html(h2).unwrap();
        assert_eq!((rows(&t2), cols(&t2)), (2, 2));
        assert_eq!(t2.cells[1][1].text, "안1 안2\n안3 안4");
        assert!(table_from_html("<table><tr><td>혼자</td></tr></table>").is_some());
    }

    #[test]
    fn a_first_row_missing_its_leading_cells_is_padded_to_its_own_columns() {
        // the copy began in the third column of the first row
        let h = "<table><tr><td>c1</td><td>d1</td></tr><tr><td>a2</td><td>b2</td><td>c2</td><td>d2</td></tr><tr><td>a3</td><td>b3</td><td>c3</td><td>d3</td></tr></table>";
        let t = table_from_html(h).unwrap();
        assert_eq!((rows(&t), cols(&t)), (3, 4));
        assert_eq!(texts(&t)[0], vec!["", "", "c1", "d1"]);
        assert_eq!(texts(&t)[1], vec!["a2", "b2", "c2", "d2"]);
        assert!(is_valid(&t));
        // ragged later rows are filled up at the end, not shifted
        let h2 = "<table><tr><td>a</td><td>b</td><td>c</td></tr><tr><td>d</td></tr></table>";
        assert_eq!(texts(&table_from_html(h2).unwrap())[1], vec!["d", "", ""]);
    }

    #[test]
    fn a_missing_leading_cell_beside_a_row_merge_keeps_the_merge_valid() {
        // first row: only the last two cells, the first of them spans two rows
        let h = "<table><tr><td rowspan=2>c1</td><td>d1</td></tr><tr><td>a2</td><td>b2</td><td>d2</td></tr></table>";
        let t = table_from_html(h).unwrap();
        assert_eq!((rows(&t), cols(&t)), (2, 4));
        assert!(is_valid(&t));
        assert_eq!(
            (t.cells[0][2].text.as_str(), t.cells[0][2].row_span),
            ("c1", 2)
        );
        assert_eq!(t.cells[1][0].text, "a2");
        assert_eq!(t.cells[1][3].text, "d2");
    }

    #[test]
    fn cells_or_rows_without_their_table_are_still_a_table() {
        let t = table_from_html("<tr><td>가</td><td>나</td></tr><tr><td>다</td><td>라</td></tr>")
            .unwrap();
        assert_eq!(texts(&t), vec![vec!["가", "나"], vec!["다", "라"]]);
        let t = table_from_html("<td>가</td><td>나</td>").unwrap();
        assert_eq!(texts(&t), vec![vec!["가", "나"]]);
        assert!(table_from_html("<p>문단</p><div>x</div>").is_none());
    }

    #[test]
    fn mail_style_tables_with_paragraphs_and_header_cells_read_cleanly() {
        let h = "<TABLE class=MsoNormalTable border=1><THEAD><TR><TH><P>이름<o:p></o:p></P></TH><TH>값</TH></TR></THEAD><TBODY><TR><TD><P>가&nbsp;</P><P>나</P></TD><TD><SPAN>1</SPAN></TD></TR></TBODY></TABLE>";
        let t = table_from_html(h).unwrap();
        assert_eq!(texts(&t), vec![vec!["이름", "값"], vec!["가\n나", "1"]]);
    }

    #[test]
    fn clear_keeps_the_merge_and_empties_text() {
        let mut t = grid(2, 2);
        merge_range(&mut t, (0, 0, 0, 1));
        clear_range(&mut t, (0, 0, 1, 1));
        assert_eq!(t.cells[0][0].col_span, 2);
        assert!(t.cells.iter().flatten().all(|c| c.text.is_empty()));
    }
}
