//! Pure validation and classification of clipboard payloads. Nothing in this
//! file touches the Windows API, so every rule here is unit-testable.
use std::io::Cursor;

pub const K_TEXT: &str = "text";
pub const K_HTML: &str = "html";
pub const K_URL: &str = "url";
pub const K_PNG: &str = "png";
pub const K_DIBV5: &str = "dibv5";
pub const K_DIB: &str = "dib";

pub const MAX_TEXT_BYTES: usize = 2 << 20;
pub const MAX_HTML_BYTES: usize = 4 << 20;
pub const MAX_URL_BYTES: usize = 64 << 10;
pub const MAX_ITEM_BYTES: usize = 32 << 20;
pub const MAX_PIXELS: u64 = 20_000_000;
pub const MAX_DECODE_BYTES: u64 = 80 << 20;
pub const THUMB_SIZE: u32 = 256;
const PREVIEW_ROWS: usize = 20;
const PREVIEW_COLS: usize = 10;
/// The detail view reads more of the table, on demand.
pub const DETAIL_ROWS: usize = 300;
pub const DETAIL_COLS: usize = 40;
pub const DETAIL_CELL_CHARS: usize = 200;
/// Longest side of the enlarged image preview.
pub const FULL_PREVIEW_SIDE: u32 = 1600;

/// Size limit of one format, by key.
pub fn limit_of(key: &str) -> usize {
    match key {
        K_TEXT => MAX_TEXT_BYTES,
        K_HTML => MAX_HTML_BYTES,
        K_URL => MAX_URL_BYTES,
        _ => MAX_ITEM_BYTES,
    }
}

// ---------------------------------------------------------------- text ----

/// UTF-16LE up to the first NUL, strictly valid, as a `String`.
pub fn utf16le_to_string(bytes: &[u8]) -> Option<String> {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .take_while(|&u| u != 0)
        .collect();
    String::from_utf16(&units).ok()
}

pub fn string_to_utf16le(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(|u| u.to_le_bytes()).collect()
}

/// A single http(s) URL: no whitespace, reasonable length.
pub fn is_link(s: &str) -> bool {
    let t = s.trim();
    (t.starts_with("http://") || t.starts_with("https://"))
        && t.len() > 8
        && t.len() <= 8192
        && !t.chars().any(char::is_whitespace)
}

/// First non-empty line, shortened, for the list.
pub fn summarize(text: &str, max_chars: usize) -> String {
    let line = text
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("");
    let mut out: String = line.chars().take(max_chars).collect();
    if line.chars().count() > max_chars {
        out.push('…');
    }
    out
}

// ---------------------------------------------------------------- HTML ----

/// Validated CF_HTML: byte ranges are checked, the fragment is extracted.
#[derive(Debug, PartialEq)]
pub struct Html {
    /// The bytes up to the first NUL (what is restored).
    pub bytes: Vec<u8>,
    pub fragment: String,
    pub document: Option<String>,
}

/// Header offsets are UTF-8 byte offsets; `-1` for StartHTML/EndHTML is the
/// only negative value the format allows.
pub fn validate_html(raw: &[u8]) -> Option<Html> {
    let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
    let body = &raw[..end];
    let s = std::str::from_utf8(body).ok()?;
    let mut sh = None;
    let mut eh = None;
    let mut sf = None;
    let mut ef = None;
    for line in s.lines().take(16) {
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        let slot = match k {
            "StartHTML" => &mut sh,
            "EndHTML" => &mut eh,
            "StartFragment" => &mut sf,
            "EndFragment" => &mut ef,
            _ => continue,
        };
        *slot = Some(v.trim().parse::<i64>().ok()?);
    }
    let (sf, ef) = (sf?, ef?);
    let len = body.len() as i64;
    let ok = |v: i64| (0..=len).contains(&v) && s.is_char_boundary(v as usize);
    if !(ok(sf) && ok(ef) && sf <= ef) {
        return None;
    }
    let document = match (sh?, eh?) {
        (-1, -1) => None,
        (a, b) if ok(a) && ok(b) && a <= b && a <= sf && ef <= b => {
            Some(s[a as usize..b as usize].to_owned())
        }
        _ => return None,
    };
    Some(Html {
        bytes: body.to_vec(),
        fragment: s[sf as usize..ef as usize].to_owned(),
        document,
    })
}

fn is_block(name: &str) -> bool {
    matches!(
        name,
        "br" | "p" | "div" | "tr" | "li" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "pre"
    )
}

/// Plain text of an HTML fragment. Scripts and styles are skipped and
/// nothing is loaded or executed.
pub fn html_to_text(html: &str) -> String {
    use scraper::Node;
    let doc = scraper::Html::parse_fragment(html);
    let mut out = String::new();
    for node in doc.tree.root().descendants() {
        match node.value() {
            Node::Element(e) => {
                let name = e.name();
                if is_block(name) && !out.is_empty() && !out.ends_with('\n') {
                    out.push('\n');
                } else if matches!(name, "td" | "th")
                    && !out.is_empty()
                    && !out.ends_with('\n')
                    && !out.ends_with('\t')
                {
                    out.push('\t');
                }
            }
            Node::Text(t) => {
                let hidden = node
                    .parent()
                    .and_then(|p| p.value().as_element())
                    .is_some_and(|e| matches!(e.name(), "script" | "style" | "head" | "title"));
                if !hidden {
                    let collapsed = t.split_whitespace().collect::<Vec<_>>().join(" ");
                    if !collapsed.is_empty() {
                        out.push_str(&collapsed);
                    }
                }
            }
            _ => {}
        }
    }
    out.trim().to_owned()
}

/// Cell grid of the first table in the HTML, trimmed for the preview.
/// Returns the preview cells and the full (rows, cols).
pub fn html_table(html: &str) -> Option<(Vec<Vec<String>>, usize, usize)> {
    html_table_with(html, PREVIEW_ROWS, PREVIEW_COLS, 60)
}

pub fn html_table_with(
    html: &str,
    max_rows: usize,
    max_cols: usize,
    cell_chars: usize,
) -> Option<(Vec<Vec<String>>, usize, usize)> {
    let doc = scraper::Html::parse_fragment(html);
    let table = scraper::Selector::parse("table").ok()?;
    let tr = scraper::Selector::parse("tr").ok()?;
    let cell = scraper::Selector::parse("td, th").ok()?;
    let t = doc.select(&table).next()?;
    let mut rows = 0;
    let mut cols = 0;
    let mut grid = Vec::new();
    for r in t.select(&tr) {
        rows += 1;
        let cells: Vec<String> = r
            .select(&cell)
            .map(|c| c.text().collect::<Vec<_>>().join(" "))
            .map(|s| clip(&s.split_whitespace().collect::<Vec<_>>().join(" "), cell_chars))
            .collect();
        cols = cols.max(cells.len());
        if grid.len() < max_rows {
            grid.push(cells.into_iter().take(max_cols).collect());
        }
    }
    (rows > 0 && cols > 0).then_some((grid, rows, cols))
}

fn clip(s: &str, n: usize) -> String {
    if s.chars().count() > n {
        let mut o: String = s.chars().take(n).collect();
        o.push('…');
        o
    } else {
        s.to_owned()
    }
}

// ------------------------------------------------------------------ TSV ----

/// Tab separated values with quoted cells (Excel style). Used only for the
/// preview: the restored bytes are never rewritten.
pub fn parse_tsv(text: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut cell = String::new();
    let mut chars = text.chars().peekable();
    let mut at_start = true;
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
            '"' if at_start => {
                quoted = true;
                at_start = false;
            }
            '\t' => {
                row.push(std::mem::take(&mut cell));
                at_start = true;
            }
            '\r' => {}
            '\n' => {
                row.push(std::mem::take(&mut cell));
                rows.push(std::mem::take(&mut row));
                at_start = true;
            }
            _ => {
                cell.push(c);
                at_start = false;
            }
        }
    }
    if !cell.is_empty() || !row.is_empty() {
        row.push(cell);
        rows.push(row);
    }
    rows
}

/// Text that looks like a copied cell range: every line has the same number
/// of tabs (at least one).
pub fn looks_like_tsv(text: &str) -> bool {
    let mut tabs = None;
    let mut lines = 0;
    for l in text.lines().filter(|l| !l.is_empty()) {
        let n = l.matches('\t').count();
        if n == 0 || tabs.is_some_and(|t| t != n) {
            return false;
        }
        tabs = Some(n);
        lines += 1;
    }
    lines > 0
}

pub fn trim_grid(rows: Vec<Vec<String>>) -> (Vec<Vec<String>>, usize, usize) {
    trim_grid_with(rows, PREVIEW_ROWS, PREVIEW_COLS, 60)
}

pub fn trim_grid_with(
    rows: Vec<Vec<String>>,
    max_rows: usize,
    max_cols: usize,
    cell_chars: usize,
) -> (Vec<Vec<String>>, usize, usize) {
    let r = rows.len();
    let c = rows.iter().map(Vec::len).max().unwrap_or(0);
    let grid = rows
        .into_iter()
        .take(max_rows)
        .map(|row| {
            row.into_iter()
                .take(max_cols)
                .map(|s| clip(&s.replace('\n', " "), cell_chars))
                .collect()
        })
        .collect();
    (grid, r, c)
}

// ------------------------------------------------------------- images ----

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dib {
    pub width: u32,
    pub height: u32,
    pub top_down: bool,
    pub bits: u16,
    pub pixel_offset: usize,
    pub stride: usize,
    /// The alpha channel is defined by a mask (V4/V5 or bitfields).
    pub alpha_mask: bool,
}

fn u32_at(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(o..o + 4)?.try_into().ok()?))
}
fn i32_at(b: &[u8], o: usize) -> Option<i32> {
    Some(i32::from_le_bytes(b.get(o..o + 4)?.try_into().ok()?))
}
fn u16_at(b: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(o..o + 2)?.try_into().ok()?))
}

/// Supported: uncompressed 24-bit and 32-bit (BI_RGB, or BI_BITFIELDS with
/// the usual BGRA masks). Every size is computed with checked arithmetic.
pub fn validate_dib(b: &[u8]) -> Option<Dib> {
    let header = u32_at(b, 0)? as usize;
    if !matches!(header, 40 | 108 | 124) || b.len() < header {
        return None;
    }
    let width = i32_at(b, 4)?;
    let height = i32_at(b, 8)?;
    if width <= 0 || height == 0 || height == i32::MIN || u16_at(b, 12)? != 1 {
        return None;
    }
    let bits = u16_at(b, 14)?;
    let compression = u32_at(b, 16)?;
    let clr_used = u32_at(b, 32)? as usize;
    let (mut masks, mut mask_bytes) = ([0u32; 4], 0usize);
    match (compression, bits) {
        (0, 24) | (0, 32) => {}
        (3, 32) => {
            if header == 40 {
                for i in 0..3 {
                    masks[i] = u32_at(b, 40 + i * 4)?;
                }
                mask_bytes = 12;
            } else {
                for i in 0..4 {
                    masks[i] = u32_at(b, 40 + i * 4)?;
                }
            }
            if masks[0] != 0x00FF_0000
                || masks[1] != 0x0000_FF00
                || masks[2] != 0x0000_00FF
                || !matches!(masks[3], 0 | 0xFF00_0000)
            {
                return None;
            }
        }
        _ => return None,
    }
    let (w, h) = (width as u64, height.unsigned_abs() as u64);
    if w.checked_mul(h)? > MAX_PIXELS || w * h * 4 > MAX_DECODE_BYTES {
        return None;
    }
    let stride = ((w.checked_mul(bits as u64)?.checked_add(31)?) / 32).checked_mul(4)? as usize;
    let pixel_offset = header
        .checked_add(mask_bytes)?
        .checked_add(clr_used.checked_mul(4)?)?;
    let needed = pixel_offset.checked_add(stride.checked_mul(h as usize)?)?;
    if needed > b.len() {
        return None;
    }
    Some(Dib {
        width: width as u32,
        height: h as u32,
        top_down: height < 0,
        bits,
        pixel_offset,
        stride,
        alpha_mask: masks[3] != 0,
    })
}

/// RGBA pixels of a validated DIB. A 32-bit image whose alpha is entirely 0
/// (the usual BI_RGB case) is treated as opaque.
pub fn dib_to_rgba(b: &[u8], d: &Dib) -> Vec<u8> {
    let (w, h) = (d.width as usize, d.height as usize);
    let mut out = vec![255u8; w * h * 4];
    for y in 0..h {
        let src_y = if d.top_down { y } else { h - 1 - y };
        let row = &b[d.pixel_offset + src_y * d.stride..][..d.stride];
        for x in 0..w {
            let o = (y * w + x) * 4;
            if d.bits == 24 {
                let p = &row[x * 3..x * 3 + 3];
                out[o..o + 3].copy_from_slice(&[p[2], p[1], p[0]]);
            } else {
                let p = &row[x * 4..x * 4 + 4];
                out[o..o + 4].copy_from_slice(&[p[2], p[1], p[0], p[3]]);
            }
        }
    }
    if d.bits == 32 && out.chunks_exact(4).all(|p| p[3] == 0) {
        out.chunks_exact_mut(4).for_each(|p| p[3] = 255);
    }
    out
}

/// Dimensions of a PNG after checking the size budget; the pixels are not
/// decoded here.
pub fn validate_png(b: &[u8]) -> Option<(u32, u32)> {
    use image::ImageDecoder;
    let dec = image::codecs::png::PngDecoder::new(Cursor::new(b)).ok()?;
    let (w, h) = dec.dimensions();
    let px = (w as u64).checked_mul(h as u64)?;
    (w > 0 && h > 0 && px <= MAX_PIXELS && px * 4 <= MAX_DECODE_BYTES).then_some((w, h))
}

pub fn png_to_rgba(b: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    validate_png(b)?;
    let img = image::load_from_memory_with_format(b, image::ImageFormat::Png).ok()?;
    let rgba = img.to_rgba8();
    Some((rgba.width(), rgba.height(), rgba.into_raw()))
}

/// A compatibility CF_DIBV5 (32-bit BGRA, bottom-up) made from RGBA pixels.
pub fn rgba_to_dibv5(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; 124];
    let put = |o: &mut Vec<u8>, at: usize, v: u32| o[at..at + 4].copy_from_slice(&v.to_le_bytes());
    put(&mut out, 0, 124);
    put(&mut out, 4, w);
    put(&mut out, 8, h);
    out[12..14].copy_from_slice(&1u16.to_le_bytes());
    out[14..16].copy_from_slice(&32u16.to_le_bytes());
    put(&mut out, 16, 3);
    put(&mut out, 20, w * h * 4);
    put(&mut out, 40, 0x00FF_0000);
    put(&mut out, 44, 0x0000_FF00);
    put(&mut out, 48, 0x0000_00FF);
    put(&mut out, 52, 0xFF00_0000);
    put(&mut out, 56, 0x7352_4742); // LCS_sRGB
    put(&mut out, 108, 4); // LCS_GM_IMAGES
    out.reserve((w * h * 4) as usize);
    for y in (0..h as usize).rev() {
        for x in 0..w as usize {
            let p = &rgba[(y * w as usize + x) * 4..][..4];
            out.extend_from_slice(&[p[2], p[1], p[0], p[3]]);
        }
    }
    out
}

/// RGBA scaled down to fit `max_side` (never enlarged).
pub fn fit_rgba(w: u32, h: u32, rgba: Vec<u8>, max_side: u32) -> Option<(u32, u32, Vec<u8>)> {
    if w.max(h) <= max_side {
        return Some((w, h, rgba));
    }
    let img = image::RgbaImage::from_raw(w, h, rgba)?;
    let k = max_side as f32 / w.max(h) as f32;
    let (tw, th) = (((w as f32 * k).round() as u32).max(1), ((h as f32 * k).round() as u32).max(1));
    let small = image::imageops::thumbnail(&img, tw, th);
    Some((tw, th, small.into_raw()))
}

/// PNG-encoded thumbnail (at most THUMB_SIZE on the long side).
pub fn thumbnail_png(w: u32, h: u32, rgba: Vec<u8>) -> Option<Vec<u8>> {
    let img = image::RgbaImage::from_raw(w, h, rgba)?;
    let scale = (THUMB_SIZE as f32 / w.max(h) as f32).min(1.0);
    let (tw, th) = (
        ((w as f32 * scale).round() as u32).max(1),
        ((h as f32 * scale).round() as u32).max(1),
    );
    let small = image::imageops::thumbnail(&img, tw, th);
    let mut out = Vec::new();
    small
        .write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
        .ok()?;
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn html(frag: &str) -> Vec<u8> {
        let pre = "<html><body><!--StartFragment-->";
        let post = "<!--EndFragment--></body></html>";
        let head_len = 105; // fixed width header below
        let sh = head_len;
        let sf = sh + pre.len();
        let ef = sf + frag.len();
        let eh = ef + post.len();
        let head = format!(
            "Version:0.9\r\nStartHTML:{sh:010}\r\nEndHTML:{eh:010}\r\nStartFragment:{sf:010}\r\nEndFragment:{ef:010}\r\n"
        );
        assert_eq!(head.len(), head_len);
        format!("{head}{pre}{frag}{post}").into_bytes()
    }

    #[test]
    fn html_ranges_are_checked() {
        let h = validate_html(&html("<b>안녕</b>")).unwrap();
        assert_eq!(h.fragment, "<b>안녕</b>");
        let mut bad = html("x");
        let n = bad.len();
        bad.truncate(n - 10);
        assert!(validate_html(&bad).is_none());
        assert!(validate_html(b"Version:0.9\r\nStartHTML:-1\r\nEndHTML:-1\r\nStartFragment:5\r\nEndFragment:2\r\nabcdef").is_none());
        assert!(validate_html(b"nothing").is_none());
    }

    #[test]
    fn html_offsets_must_hit_char_boundaries() {
        let mut v = b"StartHTML:-1\nEndHTML:-1\nStartFragment:0000000073\nEndFragment:0000000075\n".to_vec();
        v.extend("가나다라마바사".as_bytes());
        assert!(validate_html(&v).is_none());
    }

    #[test]
    fn table_from_html_and_text() {
        let (g, r, c) =
            html_table("<table><tr><td>a</td><td>b c</td></tr><tr><td></td><td>d</td></tr></table>")
                .unwrap();
        assert_eq!((r, c), (2, 2));
        assert_eq!(g[0], vec!["a", "b c"]);
        assert_eq!(html_to_text("<p>one</p><script>x()</script><p>two</p>"), "one\ntwo");
        assert_eq!(html_to_text("<table><tr><td>a</td><td>b</td></tr></table>"), "a\tb");
    }

    #[test]
    fn tsv_handles_quotes_and_blank_cells() {
        let rows = parse_tsv("a\t\"x\ty\"\t\r\n\"q\"\"\nr\"\tb\tc\n");
        assert_eq!(rows[0], vec!["a", "x\ty", ""]);
        assert_eq!(rows[1], vec!["q\"\nr", "b", "c"]);
        assert!(looks_like_tsv("a\tb\nc\td"));
        assert!(!looks_like_tsv("a\tb\nc"));
        assert!(!looks_like_tsv("plain"));
    }

    #[test]
    fn utf16_roundtrip_and_invalid() {
        let s = "한글 😀\t줄\n";
        let mut b = string_to_utf16le(s);
        b.extend([0, 0, 65, 0]);
        assert_eq!(utf16le_to_string(&b).as_deref(), Some(s));
        assert!(utf16le_to_string(&[0x00, 0xD8, 0x41, 0x00]).is_none());
    }

    #[test]
    fn link_detection() {
        assert!(is_link(" https://example.com/a?b=1 "));
        assert!(!is_link("https://a b"));
        assert!(!is_link("see https://a.com"));
    }

    fn dib24(w: i32, h: i32) -> Vec<u8> {
        let stride = ((w as usize * 24 + 31) / 32) * 4;
        let mut v = vec![0u8; 40];
        v[0..4].copy_from_slice(&40u32.to_le_bytes());
        v[4..8].copy_from_slice(&w.to_le_bytes());
        v[8..12].copy_from_slice(&h.to_le_bytes());
        v[12..14].copy_from_slice(&1u16.to_le_bytes());
        v[14..16].copy_from_slice(&24u16.to_le_bytes());
        v.extend(vec![7u8; stride * h.unsigned_abs() as usize]);
        v
    }

    #[test]
    fn dib_validation() {
        let d = validate_dib(&dib24(3, 2)).unwrap();
        assert_eq!((d.width, d.height, d.stride), (3, 2, 12));
        let rgba = dib_to_rgba(&dib24(3, 2), &d);
        assert_eq!(rgba.len(), 24);
        assert!(validate_dib(&dib24(3, -2)).unwrap().top_down);
        let mut short = dib24(3, 2);
        short.pop();
        assert!(validate_dib(&short).is_none());
        assert!(validate_dib(&dib24(0, 2)).is_none());
        let mut huge = dib24(3, 2);
        huge[4..8].copy_from_slice(&i32::MAX.to_le_bytes());
        huge[8..12].copy_from_slice(&i32::MAX.to_le_bytes());
        assert!(validate_dib(&huge).is_none());
        let mut rle = dib24(3, 2);
        rle[16..20].copy_from_slice(&1u32.to_le_bytes());
        assert!(validate_dib(&rle).is_none());
    }

    #[test]
    fn png_to_dibv5_and_back() {
        let rgba = vec![255, 0, 0, 128, 0, 255, 0, 255, 0, 0, 255, 255, 9, 9, 9, 0];
        let dibv5 = rgba_to_dibv5(2, 2, &rgba);
        let d = validate_dib(&dibv5).unwrap();
        assert!(d.alpha_mask);
        assert_eq!(dib_to_rgba(&dibv5, &d), rgba);
        let png = thumbnail_png(2, 2, rgba.clone()).unwrap();
        assert_eq!(validate_png(&png), Some((2, 2)));
        assert_eq!(png_to_rgba(&png).unwrap().2, rgba);
    }
}
