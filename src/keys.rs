//! Shortcuts the user can change: the key combination shared by the global
//! shortcuts and the program's own ones, the program's shortcut table, and
//! (on Windows) a recorder that takes the combination the user presses
//! before Windows or another program can act on it.
use eframe::egui;
use std::sync::Mutex;

/// Marks the key presses the program sends itself (클립보이's Ctrl+V), so
/// its keyboard watches can tell them from the user's.
pub const OWN_INPUT: usize = 0x4F4D_4A43;

/// A key with its modifier keys, written like "Ctrl+Shift+H".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Combo {
    pub win: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    /// "A".."Z", "0".."9", "F1".."F24", ";" and the other symbols, "Space", "Up", ...
    pub key: String,
}

/// The name a key is written with, from any of its usual spellings.
fn key_name(token: &str) -> Option<String> {
    let t = token.trim();
    let up = t.to_uppercase();
    let up = up
        .strip_prefix("KEY")
        .filter(|r| r.len() == 1)
        .or_else(|| up.strip_prefix("DIGIT").filter(|r| r.len() == 1))
        .unwrap_or(&up);
    if up.len() == 1 && up.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Some(up.to_owned());
    }
    if let Some(n) = up.strip_prefix('F').and_then(|n| n.parse::<u8>().ok()) {
        return (1..=24).contains(&n).then(|| format!("F{n}"));
    }
    let name = match up {
        ";" | "SEMICOLON" => ";",
        "," | "COMMA" => ",",
        "-" | "MINUS" => "-",
        "." | "PERIOD" => ".",
        "=" | "EQUAL" | "EQUALS" => "=",
        "/" | "SLASH" => "/",
        "`" | "BACKQUOTE" | "BACKTICK" => "`",
        "[" | "BRACKETLEFT" | "OPENBRACKET" => "[",
        "]" | "BRACKETRIGHT" | "CLOSEBRACKET" => "]",
        "\\" | "BACKSLASH" => "\\",
        "'" | "QUOTE" => "'",
        "SPACE" => "Space",
        "ENTER" | "RETURN" => "Enter",
        "TAB" => "Tab",
        "BACKSPACE" => "Backspace",
        "DELETE" | "DEL" => "Delete",
        "INSERT" | "INS" => "Insert",
        "HOME" => "Home",
        "END" => "End",
        "PAGEUP" | "PGUP" => "PageUp",
        "PAGEDOWN" | "PGDN" => "PageDown",
        "UP" | "ARROWUP" => "Up",
        "DOWN" | "ARROWDOWN" => "Down",
        "LEFT" | "ARROWLEFT" => "Left",
        "RIGHT" | "ARROWRIGHT" => "Right",
        "ESC" | "ESCAPE" => "Esc",
        "PAUSE" => "Pause",
        "PRINTSCREEN" => "PrintScreen",
        _ => return None,
    };
    Some(name.to_owned())
}

impl Combo {
    /// Reads "Ctrl+Alt+N" (also the "control+alt+KeyN" spelling of older
    /// saves). Modifiers first, one key last.
    pub fn parse(text: &str) -> Option<Self> {
        let tokens: Vec<&str> = text.split('+').map(str::trim).collect();
        let (key, mods) = tokens.split_last()?;
        let mut c = Combo {
            win: false,
            ctrl: false,
            alt: false,
            shift: false,
            key: key_name(key)?,
        };
        for m in mods {
            match m.to_uppercase().as_str() {
                "CTRL" | "CONTROL" => c.ctrl = true,
                "ALT" | "OPTION" => c.alt = true,
                "SHIFT" => c.shift = true,
                "WIN" | "SUPER" | "META" | "CMD" | "COMMAND" => c.win = true,
                _ => return None,
            }
        }
        Some(c)
    }

    fn function_key(&self) -> bool {
        self.key.len() > 1 && self.key.starts_with('F') && self.key[1..].parse::<u8>().is_ok()
    }

    /// The global shortcut for this combination, if it can be one.
    pub fn to_hotkey(&self) -> Option<global_hotkey::hotkey::HotKey> {
        let mut text = String::new();
        for (on, name) in [
            (self.win, "super+"),
            (self.ctrl, "control+"),
            (self.alt, "alt+"),
            (self.shift, "shift+"),
        ] {
            if on {
                text.push_str(name);
            }
        }
        text.push_str(match self.key.as_str() {
            "Esc" => "Escape",
            k => k,
        });
        text.parse().ok()
    }

    /// The shortcut inside the program's windows, if it can be one (the
    /// Windows key does not reach them).
    pub fn to_egui(&self) -> Option<egui::KeyboardShortcut> {
        if self.win {
            return None;
        }
        // `command` left out: Windows reports Ctrl as both, a test as `ctrl`.
        let modifiers = egui::Modifiers {
            alt: self.alt,
            ctrl: self.ctrl,
            shift: self.shift,
            mac_cmd: false,
            command: false,
        };
        Some(egui::KeyboardShortcut::new(
            modifiers,
            egui::Key::from_name(&self.key)?,
        ))
    }

    /// From a Windows virtual-key code and the modifiers held with it.
    pub fn from_vk(vk: u32, win: bool, ctrl: bool, alt: bool, shift: bool) -> Option<Self> {
        let key = match vk {
            0x30..=0x39 | 0x41..=0x5A => char::from_u32(vk)?.to_string(),
            0x70..=0x87 => format!("F{}", vk - 0x6F),
            0xBA => ";".into(),
            0xBB => "=".into(),
            0xBC => ",".into(),
            0xBD => "-".into(),
            0xBE => ".".into(),
            0xBF => "/".into(),
            0xC0 => "`".into(),
            0xDB => "[".into(),
            0xDC => "\\".into(),
            0xDD => "]".into(),
            0xDE => "'".into(),
            0x20 => "Space".into(),
            0x0D => "Enter".into(),
            0x09 => "Tab".into(),
            0x08 => "Backspace".into(),
            0x2E => "Delete".into(),
            0x2D => "Insert".into(),
            0x24 => "Home".into(),
            0x23 => "End".into(),
            0x21 => "PageUp".into(),
            0x22 => "PageDown".into(),
            0x25 => "Left".into(),
            0x26 => "Up".into(),
            0x27 => "Right".into(),
            0x28 => "Down".into(),
            0x1B => "Esc".into(),
            0x13 => "Pause".into(),
            0x2C => "PrintScreen".into(),
            _ => return None,
        };
        Some(Combo {
            win,
            ctrl,
            alt,
            shift,
            key,
        })
    }
}

impl std::fmt::Display for Combo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (on, name) in [
            (self.win, "Win+"),
            (self.ctrl, "Ctrl+"),
            (self.alt, "Alt+"),
            (self.shift, "Shift+"),
        ] {
            if on {
                f.write_str(name)?;
            }
        }
        f.write_str(&self.key)
    }
}

/// Why `c` cannot be a shortcut of OhMemoJang, if it cannot: a combination
/// Windows (or every program) already uses, or one that would get in the
/// way of typing.
pub fn unusable(c: &Combo) -> Option<String> {
    if !c.ctrl && !c.alt && !c.win && !c.function_key() {
        return Some("글자 입력과 겹쳐요. Ctrl이나 Alt와 함께 눌러 주세요.".into());
    }
    let k = c.key.as_str();
    let only = |win, ctrl, alt| c.win == win && c.ctrl == ctrl && c.alt == alt;
    let reserved = match k {
        "Tab" if only(false, false, true) => Some("창 전환 Alt+Tab"),
        "F4" if only(false, false, true) => Some("창 닫기 Alt+F4"),
        "Esc" if c.alt && !c.ctrl => Some("창 전환 Alt+Esc"),
        "Space" if only(false, false, true) => Some("창 메뉴 Alt+Space"),
        "Esc" if c.ctrl && !c.alt => Some("시작 메뉴·작업 관리자"),
        "Delete" if c.ctrl && c.alt => Some("보안 화면 Ctrl+Alt+Delete"),
        "PrintScreen" => Some("화면 캡처"),
        "C" | "V" | "X" | "Z" | "Y" | "A" | "S" if only(false, true, false) && !c.shift => {
            Some("복사·붙여넣기·실행 취소 같은 기본 편집 키")
        }
        // The Windows key: Windows itself answers most of these.
        _ if c.win && !c.ctrl && !c.alt => Some("Windows 키 단축키"),
        "D" | "Left" | "Right" | "F4" | "Enter" | "O" | "C" | "S" | "B" if c.win && c.ctrl => {
            Some("Windows 키 단축키")
        }
        "R" | "G" | "B" | "D" | "K" if c.win && c.alt => Some("Windows 게임 바"),
        _ => None,
    };
    reserved.map(|what| format!("Windows에서 쓰는 단축키예요 ({what})."))
}

/// What the program's own shortcuts do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Bold,
    Italic,
    Underline,
    Date,
    DateTime,
    CloseStickers,
    Topmost,
    Find,
    Replace,
    Project,
}

pub const ACTIONS: [Action; 10] = [
    Action::Bold,
    Action::Italic,
    Action::Underline,
    Action::Date,
    Action::DateTime,
    Action::CloseStickers,
    Action::Topmost,
    Action::Find,
    Action::Replace,
    Action::Project,
];

impl Action {
    /// The name it is saved under.
    pub fn id(self) -> &'static str {
        match self {
            Action::Bold => "bold",
            Action::Italic => "italic",
            Action::Underline => "underline",
            Action::Date => "date",
            Action::DateTime => "date_time",
            Action::CloseStickers => "close_stickers",
            Action::Topmost => "topmost",
            Action::Find => "find",
            Action::Replace => "replace",
            Action::Project => "project",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Action::Bold => "굵게",
            Action::Italic => "기울임",
            Action::Underline => "밑줄",
            Action::Date => "날짜 넣기",
            Action::DateTime => "날짜와 시간 넣기",
            Action::CloseStickers => "모든 스티커 닫기",
            Action::Topmost => "항상 위에 켜기/끄기",
            Action::Find => "빠른 검색",
            Action::Replace => "찾아 바꾸기",
            Action::Project => "프로젝트 선택",
        }
    }
    pub fn default_combo(self) -> Combo {
        let text = match self {
            Action::Bold => "Ctrl+B",
            Action::Italic => "Ctrl+I",
            Action::Underline => "Ctrl+U",
            Action::Date => "Ctrl+;",
            Action::DateTime => "Ctrl+Shift+;",
            Action::CloseStickers => "Ctrl+Shift+H",
            Action::Topmost => "Ctrl+Shift+T",
            Action::Find => "Ctrl+F",
            Action::Replace => "Ctrl+H",
            Action::Project => "Ctrl+P",
        };
        Combo::parse(text).expect("built-in shortcut")
    }
}

/// The program's shortcuts in `ACTIONS` order; empty until first used.
static TABLE: Mutex<Vec<(Combo, egui::KeyboardShortcut)>> = Mutex::new(Vec::new());

fn with_table<R>(f: impl FnOnce(&mut Vec<(Combo, egui::KeyboardShortcut)>) -> R) -> R {
    let mut t = TABLE.lock().unwrap_or_else(|e| e.into_inner());
    if t.is_empty() {
        *t = ACTIONS
            .iter()
            .map(|a| {
                let c = a.default_combo();
                let s = c.to_egui().expect("built-in shortcut");
                (c, s)
            })
            .collect();
    }
    f(&mut t)
}

/// The combination doing `a` now.
pub fn combo(a: Action) -> Combo {
    with_table(|t| t[a as usize].0.clone())
}

/// Lets `c` do `a`. False (and nothing changes) when it cannot be used in
/// the program's windows.
pub fn set(a: Action, c: &Combo) -> bool {
    let Some(s) = c.to_egui() else {
        return false;
    };
    with_table(|t| t[a as usize] = (c.clone(), s));
    true
}

fn hit(e: &egui::Event, s: &egui::KeyboardShortcut) -> bool {
    matches!(e, egui::Event::Key { key, pressed: true, modifiers, .. }
        if *key == s.logical_key && modifiers.matches_exact(s.modifiers))
}

/// The shortcut for `a` was pressed this frame: taken out of the input so
/// nothing else acts on it.
pub fn consume(i: &mut egui::InputState, a: Action) -> bool {
    let s = with_table(|t| t[a as usize].1);
    let before = i.events.len();
    i.events.retain(|e| !hit(e, &s));
    i.events.len() != before
}

/// The shortcut for `a` was pressed this frame (left in the input).
pub fn pressed(i: &egui::InputState, a: Action) -> bool {
    let s = with_table(|t| t[a as usize].1);
    i.events.iter().any(|e| hit(e, &s))
}

/// What the recorder saw.
#[derive(Clone, Debug, PartialEq)]
pub enum Recorded {
    Combo(Combo),
    /// Esc on its own: stop without changing anything.
    Cancel,
    /// A key that cannot be part of a shortcut.
    Unsupported,
}

pub use rec::Recorder;

/// Takes key presses while one of the program's windows is in front, before
/// Windows' own shortcuts and other programs' global shortcuts see them, so
/// any combination can be pressed and checked. Nothing pressed reaches a
/// program while it records.
#[cfg(windows)]
mod rec {
    use super::{Combo, Recorded};
    use eframe::egui;
    use std::cell::RefCell;
    use std::collections::HashSet;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::{Arc, Mutex};
    use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::System::Threading::{GetCurrentProcessId, GetCurrentThreadId};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, GetForegroundWindow, GetMessageW, GetWindowThreadProcessId, KillTimer,
        PeekMessageW, PostThreadMessageW, SetTimer, SetWindowsHookExW, UnhookWindowsHookEx,
        KBDLLHOOKSTRUCT, MSG, PM_NOREMOVE, WH_KEYBOARD_LL, WM_APP, WM_KEYDOWN, WM_KEYUP,
        WM_SYSKEYDOWN, WM_SYSKEYUP, WM_TIMER,
    };

    const WM_STOP: u32 = WM_APP + 0x51;
    /// A combination was taken: the program is woken from the thread's
    /// loop, not from the hook.
    const WM_GOT: u32 = WM_APP + 0x52;

    struct Hook {
        out: Arc<Mutex<Option<Recorded>>>,
        /// Modifier keys held: Win, Ctrl, Alt, Shift.
        held: [bool; 4],
        /// Keys whose press was taken: their release is taken too.
        taken: HashSet<u32>,
    }

    thread_local! {
        static HOOK: RefCell<Option<Hook>> = const { RefCell::new(None) };
    }

    fn modifier(vk: u32) -> Option<usize> {
        match vk {
            0x5B | 0x5C => Some(0),
            0x11 | 0xA2 | 0xA3 => Some(1),
            0x12 | 0xA4 | 0xA5 => Some(2),
            0x10 | 0xA0 | 0xA1 => Some(3),
            _ => None,
        }
    }

    unsafe fn ours_in_front() -> bool {
        let mut pid = 0;
        GetWindowThreadProcessId(GetForegroundWindow(), &mut pid);
        pid == GetCurrentProcessId()
    }

    /// Short, and never waiting on the program: Windows drops a low-level
    /// hook that answers too slowly (and lets that key through).
    unsafe extern "system" fn hook(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
        if code >= 0 {
            let k = &*(l as *const KBDLLHOOKSTRUCT);
            let msg = w as u32;
            let down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
            let up = msg == WM_KEYUP || msg == WM_SYSKEYUP;
            // Keys sent by other programs count (an on-screen keyboard),
            // the program's own do not.
            let own = k.dwExtraInfo == super::OWN_INPUT;
            let take = HOOK.with(|h| {
                let Ok(mut h) = h.try_borrow_mut() else {
                    return false;
                };
                let Some(h) = h.as_mut() else { return false };
                let vk = k.vkCode;
                if up {
                    if let Some(m) = modifier(vk) {
                        h.held[m] = false;
                    }
                    return h.taken.remove(&vk);
                }
                if !down || own || !ours_in_front() {
                    return false;
                }
                h.taken.insert(vk);
                if let Some(m) = modifier(vk) {
                    h.held[m] = true;
                    return true;
                }
                let [win, ctrl, alt, shift] = h.held;
                let got = if vk == 0x1B && !(win || ctrl || alt || shift) {
                    Recorded::Cancel
                } else {
                    Combo::from_vk(vk, win, ctrl, alt, shift)
                        .map_or(Recorded::Unsupported, Recorded::Combo)
                };
                *h.out.lock().unwrap_or_else(|e| e.into_inner()) = Some(got);
                PostThreadMessageW(GetCurrentThreadId(), WM_GOT, 0, 0);
                true
            });
            if take {
                return 1;
            }
        }
        CallNextHookEx(std::ptr::null_mut(), code, w, l)
    }

    pub struct Recorder {
        out: Arc<Mutex<Option<Recorded>>>,
        thread: Arc<AtomicU32>,
    }

    impl Recorder {
        pub fn start(ctx: &egui::Context) -> Option<Self> {
            let out = Arc::new(Mutex::new(None));
            let thread = Arc::new(AtomicU32::new(0));
            let (ready, started) = std::sync::mpsc::sync_channel(1);
            let (hook_out, hook_thread, ctx) = (out.clone(), thread.clone(), ctx.clone());
            std::thread::Builder::new()
                .name("shortcut-recorder".into())
                .spawn(move || unsafe {
                    let mut msg: MSG = std::mem::zeroed();
                    PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_NOREMOVE);
                    HOOK.with(|h| {
                        *h.borrow_mut() = Some(Hook {
                            out: hook_out,
                            held: [false; 4],
                            taken: HashSet::new(),
                        })
                    });
                    let hhook = SetWindowsHookExW(
                        WH_KEYBOARD_LL,
                        Some(hook),
                        GetModuleHandleW(std::ptr::null()),
                        0,
                    );
                    if hhook.is_null() {
                        let _ = ready.send(false);
                        return;
                    }
                    hook_thread.store(GetCurrentThreadId(), Ordering::SeqCst);
                    let _ = ready.send(true);
                    // After the stop request, keep taking the releases of the
                    // keys taken (a second at most), so no program sees a
                    // key let go that it never saw pressed.
                    let mut stopping = false;
                    loop {
                        let left = HOOK.with(|h| h.borrow().as_ref().map_or(0, |h| h.taken.len()));
                        if stopping && left == 0 {
                            break;
                        }
                        if GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) <= 0 {
                            break;
                        }
                        match msg.message {
                            WM_GOT => ctx.request_repaint_of(egui::ViewportId::ROOT),
                            WM_STOP if !stopping => {
                                stopping = true;
                                SetTimer(std::ptr::null_mut(), 0, 1000, None);
                            }
                            WM_TIMER if stopping => {
                                KillTimer(std::ptr::null_mut(), msg.wParam);
                                break;
                            }
                            _ => {}
                        }
                    }
                    UnhookWindowsHookEx(hhook);
                    HOOK.with(|h| *h.borrow_mut() = None);
                })
                .ok()?;
            match started.recv_timeout(std::time::Duration::from_secs(1)) {
                Ok(true) => Some(Self { out, thread }),
                _ => None,
            }
        }

        /// The last combination pressed since the previous call.
        pub fn take(&self) -> Option<Recorded> {
            self.out.lock().unwrap_or_else(|e| e.into_inner()).take()
        }
    }

    impl Drop for Recorder {
        fn drop(&mut self) {
            let id = self.thread.load(Ordering::SeqCst);
            if id != 0 {
                // SAFETY: posting to a thread that may already be gone is harmless.
                unsafe {
                    PostThreadMessageW(id, WM_STOP, 0, 0);
                }
            }
        }
    }
}

#[cfg(not(windows))]
mod rec {
    use super::Recorded;
    use eframe::egui;

    pub struct Recorder;

    impl Recorder {
        pub fn start(_: &egui::Context) -> Option<Self> {
            None
        }
        pub fn take(&self) -> Option<Recorded> {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combinations_read_and_write_in_one_form() {
        let c = Combo::parse("ctrl + alt + n").unwrap();
        assert_eq!(c.to_string(), "Ctrl+Alt+N");
        // Older saves and other spellings.
        assert_eq!(
            Combo::parse("shift+control+KeyH").unwrap().to_string(),
            "Ctrl+Shift+H"
        );
        assert_eq!(
            Combo::parse("Super+Shift+Digit5").unwrap().to_string(),
            "Win+Shift+5"
        );
        assert_eq!(Combo::parse("Ctrl+Semicolon").unwrap().key, ";");
        assert_eq!(
            Combo::parse("Alt+V").unwrap(),
            Combo::parse("alt+v").unwrap()
        );
        assert!(Combo::parse("Ctrl+Hyper+N").is_none());
        assert!(Combo::parse("Ctrl+").is_none());
        assert!(Combo::parse("").is_none());
        for a in ACTIONS {
            let c = a.default_combo();
            assert_eq!(Combo::parse(&c.to_string()), Some(c));
        }
    }

    #[test]
    fn a_combination_becomes_a_global_and_an_in_program_shortcut() {
        let h = Combo::parse("Alt+V").unwrap().to_hotkey().unwrap();
        assert_eq!(h, "alt+KeyV".parse().unwrap());
        let h = Combo::parse("Win+Ctrl+;").unwrap().to_hotkey().unwrap();
        assert_eq!(h, "super+control+Semicolon".parse().unwrap());
        assert!(Combo::parse("Ctrl+Esc").unwrap().to_hotkey().is_some());
        let s = Combo::parse("Ctrl+Shift+;").unwrap().to_egui().unwrap();
        assert_eq!(s.logical_key, egui::Key::Semicolon);
        assert!(s.modifiers.ctrl && s.modifiers.shift && !s.modifiers.alt);
        // The Windows key never reaches the program's windows.
        assert!(Combo::parse("Win+B").unwrap().to_egui().is_none());
        // Keys egui has no name for stay global only.
        assert!(Combo::parse("Ctrl+Pause").unwrap().to_egui().is_none());
    }

    #[test]
    fn virtual_key_codes_name_the_keys() {
        let c = |vk| Combo::from_vk(vk, false, true, false, false).map(|c| c.to_string());
        assert_eq!(c(0x56).as_deref(), Some("Ctrl+V"));
        assert_eq!(c(0x35).as_deref(), Some("Ctrl+5"));
        assert_eq!(c(0x74).as_deref(), Some("Ctrl+F5"));
        assert_eq!(c(0xBA).as_deref(), Some("Ctrl+;"));
        assert_eq!(c(0x26).as_deref(), Some("Ctrl+Up"));
        // Number pad keys are not offered (they would act as the row keys).
        assert_eq!(c(0x61), None);
        for vk in (0x30..=0x39).chain(0x41..=0x5A).chain(0x70..=0x87) {
            let combo = Combo::from_vk(vk, false, true, true, false).unwrap();
            assert!(combo.to_hotkey().is_some(), "{combo}");
        }
    }

    #[test]
    fn windows_and_typing_combinations_are_refused() {
        let no = |t: &str| unusable(&Combo::parse(t).unwrap());
        for t in [
            "Alt+Tab",
            "Alt+F4",
            "Ctrl+Shift+Esc",
            "Ctrl+Alt+Delete",
            "Win+V",
            "Win+Shift+S",
            "Win+Ctrl+D",
            "Ctrl+C",
            "Ctrl+V",
            "A",
            "Shift+A",
            "Space",
        ] {
            assert!(no(t).is_some(), "{t} must be refused");
        }
        for t in [
            "Alt+V",
            "Ctrl+Alt+N",
            "Ctrl+Shift+H",
            "F9",
            "Ctrl+Shift+C",
            "Win+Alt+N",
        ] {
            assert_eq!(no(t), None, "{t} is free");
        }
        assert!(no("A").unwrap().contains("Ctrl"));
        assert!(no("Alt+Tab").unwrap().contains("Windows"));
    }

    #[test]
    fn a_changed_shortcut_replaces_the_default_exactly() {
        let ctx = egui::Context::default();
        let key = |key, modifiers| egui::RawInput {
            events: vec![egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            }],
            ..Default::default()
        };
        let ctrl_shift = egui::Modifiers::CTRL | egui::Modifiers::SHIFT;
        let mut hits = vec![];
        let run = |input, hits: &mut Vec<(bool, bool)>| {
            let _ = ctx.run(input, |ctx| {
                hits.push(
                    ctx.input_mut(|i| (consume(i, Action::DateTime), consume(i, Action::Date))),
                );
            });
        };
        // Ctrl+; and Ctrl+Shift+; are told apart, whatever the order.
        run(key(egui::Key::Semicolon, egui::Modifiers::CTRL), &mut hits);
        run(key(egui::Key::Semicolon, ctrl_shift), &mut hits);
        assert_eq!(hits, vec![(false, true), (true, false)]);
        assert!(set(Action::Date, &Combo::parse("Alt+D").unwrap()));
        assert!(!set(Action::Date, &Combo::parse("Win+D").unwrap()));
        hits.clear();
        run(key(egui::Key::Semicolon, egui::Modifiers::CTRL), &mut hits);
        run(key(egui::Key::D, egui::Modifiers::ALT), &mut hits);
        assert_eq!(hits, vec![(false, false), (false, true)]);
        assert_eq!(combo(Action::Date).to_string(), "Alt+D");
        set(Action::Date, &Action::Date.default_combo());
    }
}
