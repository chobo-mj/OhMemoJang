//! The clipboard's whole "HTML Format" (CF_HTML) as a string. arboard only
//! returns the fragment, which for Excel has no `<style>` block, and with it
//! no fill, text color or alignment (Excel keeps them in classes there).

/// The HTML document of a CF_HTML buffer: the bytes between `StartHTML` and
/// `EndHTML` (byte offsets from the buffer's start).
pub fn extract(raw: &[u8]) -> Option<String> {
    let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
    let raw = &raw[..end];
    let head = String::from_utf8_lossy(&raw[..raw.len().min(512)]).into_owned();
    let num = |key: &str| {
        head.lines()
            .find_map(|l| l.strip_prefix(key))
            .and_then(|v| v.trim().parse::<usize>().ok())
    };
    let (a, b) = (num("StartHTML:")?, num("EndHTML:")?);
    (a <= b && b <= raw.len()).then(|| String::from_utf8_lossy(&raw[a..b]).into_owned())
}

/// Reads the clipboard's CF_HTML, if there is one.
pub fn read() -> Option<String> {
    #[cfg(windows)]
    {
        imp::read()
    }
    #[cfg(not(windows))]
    {
        None
    }
}

#[cfg(windows)]
mod imp {
    use windows_sys::Win32::Foundation::HGLOBAL;
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
        RegisterClipboardFormatW,
    };
    use windows_sys::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};

    const MAX_BYTES: usize = 8 << 20;

    pub fn read() -> Option<String> {
        let name: Vec<u16> = "HTML Format".encode_utf16().chain(Some(0)).collect();
        // SAFETY: plain clipboard calls; the data is copied out while the
        // clipboard is open and no pointer outlives it.
        let raw = unsafe {
            let fmt = RegisterClipboardFormatW(name.as_ptr());
            if fmt == 0 || IsClipboardFormatAvailable(fmt) == 0 {
                return None;
            }
            let mut opened = false;
            for _ in 0..5 {
                if OpenClipboard(std::ptr::null_mut()) != 0 {
                    opened = true;
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            if !opened {
                return None;
            }
            let h = GetClipboardData(fmt);
            let mut out = None;
            if !h.is_null() {
                let size = GlobalSize(h as HGLOBAL);
                if size > 0 && size <= MAX_BYTES {
                    let p = GlobalLock(h as HGLOBAL) as *const u8;
                    if !p.is_null() {
                        out = Some(std::slice::from_raw_parts(p, size).to_vec());
                        GlobalUnlock(h as HGLOBAL);
                    }
                }
            }
            CloseClipboard();
            out
        }?;
        super::extract(&raw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_the_html_document_by_byte_offsets() {
        let body = "<html><style>.a{}</style><body>한글</body></html>";
        let head = |s: usize, e: usize| {
            format!("Version:1.0\r\nStartHTML:{s:010}\r\nEndHTML:{e:010}\r\nStartFragment:0000000000\r\nEndFragment:0000000000\r\n")
        };
        let h0 = head(0, 0).len();
        let mut raw = head(h0, h0 + body.len()).into_bytes();
        raw.extend(body.as_bytes());
        raw.push(0);
        assert_eq!(extract(&raw).as_deref(), Some(body));
        assert!(extract(b"nothing").is_none());
        assert!(extract(head(5, 1).as_bytes()).is_none());
    }
}
