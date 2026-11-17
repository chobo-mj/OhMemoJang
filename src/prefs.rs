//! Settings kept between runs: the start screen, hidden menus and the
//! calendar options. Stored as `key=value` lines in the user's settings
//! folder (`%APPDATA%\OhMemoJang\settings.txt` on Windows).
use std::collections::BTreeSet;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub struct Prefs {
    pub sidebar_width: f32,
    pub drop_schedule_choice: bool,
    pub close_completed_sticker: bool,
    pub rearrange_completed_stickers: bool,

    /// Page shown when the app starts (a page index of the main menu).
    pub start_page: usize,
    /// Menu pages hidden from the sidebar (never page 0, 내 작업실).
    pub hidden_pages: BTreeSet<usize>,
    /// Calendar window theme: 0 follows the app, 1.. picks a theme.
    pub cal_theme: usize,
    /// Text size of memos inside the calendar grid.
    pub cal_font: f32,
    pub cal_hide_done: bool,
    pub cal_strike: bool,
    /// 0: month, 1: one week, 2: two weeks.
    pub cal_weeks: usize,
    /// Data folder chosen in settings (empty: the default folder).
    pub data_path: String,
    /// The usage guide was shown on the first run.
    pub first_run_done: bool,
    /// The "still running in the tray" notice was shown once.
    pub tray_hint_done: bool,
    /// The main window's calendar shows the selected day's schedule beside the
    /// grid (side by side): in 내 작업실 (on by default) and in the 캘린더 page.
    pub cal_side_work: bool,
    pub cal_side_page: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            sidebar_width: 236.,
            drop_schedule_choice: false,
            close_completed_sticker: true,
            rearrange_completed_stickers: false,
            start_page: 0,
            hidden_pages: BTreeSet::new(),
            cal_theme: 0,
            cal_font: 11.5,
            cal_hide_done: false,
            cal_strike: true,
            cal_weeks: 0,
            data_path: String::new(),
            first_run_done: false,
            tray_hint_done: false,
            cal_side_work: true,
            cal_side_page: false,
        }
    }
}

impl Prefs {
    pub fn to_text(&self) -> String {
        let hidden: Vec<String> = self.hidden_pages.iter().map(|p| p.to_string()).collect();
        format!(
            "sidebar_width={}\ndrop_schedule_choice={}\nclose_completed_sticker={}\nrearrange_completed_stickers={}\nstart_page={}\nhidden_pages={}\ncal_theme={}\ncal_font={}\ncal_hide_done={}\ncal_strike={}\ncal_weeks={}\ndata_path={}\nfirst_run_done={}\ntray_hint_done={}\ncal_side_work={}\ncal_side_page={}\n",
            self.sidebar_width,
            self.drop_schedule_choice,
            self.close_completed_sticker,
            self.rearrange_completed_stickers,
            self.start_page,
            hidden.join(","),
            self.cal_theme,
            self.cal_font,
            self.cal_hide_done,
            self.cal_strike,
            self.cal_weeks,
            self.data_path.replace('\n', " "),
            self.first_run_done,
            self.tray_hint_done,
            self.cal_side_work,
            self.cal_side_page,
        )
    }
    /// Unknown keys and bad values fall back to the defaults.
    pub fn from_text(text: &str) -> Self {
        let mut p = Self::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.trim();
            match key.trim() {
                "sidebar_width" => {
                    p.sidebar_width = value.parse().unwrap_or(p.sidebar_width).clamp(64., 360.)
                }
                "drop_schedule_choice" => p.drop_schedule_choice = value == "true",
                "close_completed_sticker" => p.close_completed_sticker = value == "true",
                "rearrange_completed_stickers" => p.rearrange_completed_stickers = value == "true",
                "start_page" => p.start_page = value.parse().unwrap_or(p.start_page),
                "hidden_pages" => {
                    p.hidden_pages = value
                        .split(',')
                        .filter_map(|v| v.trim().parse().ok())
                        .filter(|&v| v != 0)
                        .collect()
                }
                "cal_theme" => p.cal_theme = value.parse().unwrap_or(p.cal_theme),
                "cal_font" => p.cal_font = value.parse().unwrap_or(p.cal_font).clamp(9., 18.),
                "cal_hide_done" => p.cal_hide_done = value == "true",
                "cal_strike" => p.cal_strike = value == "true",
                "cal_weeks" => p.cal_weeks = value.parse().unwrap_or(p.cal_weeks).min(2),
                "data_path" => p.data_path = value.to_owned(),
                "first_run_done" => p.first_run_done = value == "true",
                "tray_hint_done" => p.tray_hint_done = value == "true",
                "cal_side_work" => p.cal_side_work = value == "true",
                "cal_side_page" => p.cal_side_page = value == "true",
                _ => {}
            }
        }
        p
    }
    fn path() -> Option<PathBuf> {
        let base = std::env::var_os("APPDATA")
            .map(|d| PathBuf::from(d).join("OhMemoJang"))
            .or_else(|| {
                std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config/ohmemojang"))
            })?;
        Some(base.join("settings.txt"))
    }
    pub fn load() -> Self {
        Self::path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map(|t| Self::from_text(&t))
            .unwrap_or_default()
    }
    pub fn save(&self) {
        if let Some(path) = Self::path() {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(path, self.to_text());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_survive_a_round_trip_and_bad_lines_fall_back() {
        let p = Prefs {
            start_page: 5,
            hidden_pages: BTreeSet::from([4, 6]),
            cal_theme: 2,
            cal_font: 13.,
            cal_hide_done: true,
            cal_strike: false,
            cal_weeks: 2,
            data_path: "D:\\메모".into(),
            first_run_done: true,
            tray_hint_done: true,
            cal_side_work: false,
            cal_side_page: true,
            ..Prefs::default()
        };
        assert_eq!(Prefs::from_text(&p.to_text()), p);
        let q = Prefs::from_text("start_page=x\nhidden_pages=0,3\ncal_font=99\nnoise");
        assert_eq!(q.start_page, 0);
        assert_eq!(q.hidden_pages, BTreeSet::from([3]));
        assert_eq!(q.cal_font, 18.);
    }
}
