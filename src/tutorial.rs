//! The first-run tutorial: a guided tour on the real windows.
//!
//! Everything but the part being explained is dimmed by a screen-wide dark
//! window with holes cut where the explained windows or buttons are; clicks
//! on the dark part are blocked, the holes work as usual. A small window with
//! the explanation and a looping mini demo follows the highlighted part. Each
//! step watches the real data and moves on by itself once the user did it.
//!
//! `Extra::tutorial` keeps the state across restarts:
//! 0 off, `WELCOME`, `STEP0 + n` while on step n, `FINISH` at the last choice.
//! `Extra::tutorial_notes` lists the example memos, which the user may erase
//! at the end together with everything filed under the project `튜토리얼`.
use crate::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

pub const WELCOME: u32 = 1;
pub const STEP0: u32 = 10;
pub const FINISH: u32 = 100;

const PROJECT: &str = "튜토리얼";

/// Parts of the screen a step can point at.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Spot {
    /// A memo window (sticker, title strip or color box).
    Note(usize),
    /// A menu or dialog window: always left bright and clickable.
    Popup,
    /// The calendar grid of the main window.
    Grid,
    /// The "캘린더 분리창" button of the main window.
    Detach,
    /// The stand button of the calendar window.
    Stand,
    /// The project filter of 내 작업실 (and its open list).
    Filter,
    /// The project buttons in the left menu.
    Projects,
    /// The memo list of 내 작업실.
    List,
    /// The "편의기능" button in the left menu.
    NavUtilities,
    /// The 편의기능 page.
    Utilities,
    /// The calendar window's grid (and its schedule list).
    DockGrid,
    /// The "클립보드 히스토리" button in the left menu.
    NavClipboard,
    /// The 클립보드 히스토리 page.
    Clipboard,
    /// The 클립보이 window.
    Clipboy,
}

static ACTIVE: AtomicBool = AtomicBool::new(false);
/// Screen rectangles (physical pixels: left, top, right, bottom) of the spots
/// drawn this frame.
static MARKS: Mutex<Vec<(Spot, [i32; 4])>> = Mutex::new(Vec::new());

/// Called by the windows that draw a spot: remembers where it is on screen.
/// `rect` is in the points of the window `ctx` draws. Free when no tutorial
/// is running.
pub fn mark(ctx: &egui::Context, spot: Spot, rect: egui::Rect) {
    if !ACTIVE.load(Ordering::Relaxed) || !rect.is_positive() {
        return;
    }
    let Some((origin, ppp)) = ctx.input(|i| {
        let v = i.viewport();
        Some((v.inner_rect?.min, v.native_pixels_per_point?))
    }) else {
        return;
    };
    let px = |v: f32| (v * ppp).round() as i32;
    let r = rect.translate(origin.to_vec2());
    MARKS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push((spot, [px(r.min.x), px(r.min.y), px(r.max.x), px(r.max.y)]));
}

/// `mark` for a widget, handing its response back.
pub fn marked(r: egui::Response, spot: Spot) -> egui::Response {
    mark(&r.ctx, spot, r.rect);
    r
}

/// The tutorial runs (every window must say where its parts are).
pub fn active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

/// Start of a frame: forget last frame's spots.
pub fn begin_frame(active: bool) {
    ACTIVE.store(active, Ordering::Relaxed);
    MARKS.lock().unwrap_or_else(|e| e.into_inner()).clear();
}

fn marks() -> Vec<(Spot, [i32; 4])> {
    MARKS.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// The example memos, by role (their place in `Extra::tutorial_notes`).
#[derive(Clone, Copy, PartialEq)]
enum Role {
    Practice,
    Drag,
    Deadline,
    Arrange(usize),
    Complete,
    /// Added in 2.1: dropped on the calendar window.
    DockDrag,
    DockDeadline,
}

/// How many memos the tutorial adds before 2.1 (roles, then two examples).
const OLD_NOTES: usize = 9;
const DOCK_TITLE: &str = "분리창 연습 · 자료 정리";
const DOCK_DEADLINE_TITLE: &str = "분리창 연습 · 계약서 회신";
const DRAG_TITLE: &str = "드래그 연습 · 회의 준비";
const DEADLINE_TITLE: &str = "드래그 연습 · 보고서 제출";
const ARRANGE_TITLES: [&str; 3] = ["정렬 연습 · 하나", "정렬 연습 · 둘", "정렬 연습 · 셋"];
/// Put in the clipboard history for the 클립보이 step, pasted by the user.
const CLIPBOY_SAMPLE: &str = "클립보이 연습 · 붙여넣기 성공!";

/// What a step is about (also picks its mini demo).
#[derive(Clone, Copy, PartialEq, Debug)]
enum K {
    Project,
    Title,
    MultiList,
    ListUp,
    Strike,
    Check,
    Drag,
    ShiftDrag,
    Arrange,
    Filter,
    Complete,
    Journal,
    Clipboard,
    Clipboy,
    Detach,
    DockDrag,
    DockShift,
    Range,
    Stand,
    End,
}

struct Step {
    key: K,
    title: &'static str,
    text: &'static str,
    /// What to do, in one line, shown bold.
    todo: &'static str,
}

impl Step {
    /// Moves on by itself once done (the rest wait for "다음" so the result
    /// can be looked at).
    fn auto(&self) -> bool {
        !matches!(self.key, K::Journal | K::Clipboard | K::End)
    }
}

const STEPS: &[Step] = &[
    Step {
        key: K::Project,
        title: "#프로젝트로 분류하기",
        text: "밝게 보이는 스티커가 연습용 메모예요. 본문 맨 앞에 ‘#프로젝트이름’과 스페이스를 쓰면 그 프로젝트로 분류되고, 입력한 글자는 사라져요.",
        todo: "본문에 ‘#튜토리얼 ’(스페이스까지)을 입력해 보세요.",
    },
    Step {
        key: K::Title,
        title: "제목은 저절로",
        text: "제목 칸을 비워 두면 본문 첫 줄이 제목이 돼요. 따로 제목을 쓸 필요가 없어요.",
        todo: "본문에 ‘장보기 목록’을 쓰고 Enter를 눌러 보세요.",
    },
    Step {
        key: K::MultiList,
        title: "여러 단계 목록 만들기",
        text: "‘1. ’로 목록을 시작하고 Enter를 누르면 ‘2. ’가 저절로 생겨요. 그 자리에 ‘a) ’를 쓰면 한 단계 아래 목록이 되고, 또 그 아래에 ‘i) ’나 ‘- ’를 쓰면 더 아래로 내려가요.",
        todo: "‘1. 우유’ 쓰고 Enter → 생긴 ‘2. ’ 자리에 ‘a) ’ 입력 → ‘저지방’을 써 보세요.",
    },
    Step {
        key: K::ListUp,
        title: "Enter로 위 단계로 올라가기",
        text: "하위 목록의 빈 항목에서 Enter를 누르면 한 단계 위 목록으로 올라가요. 계속 누르면 한 단계씩 올라가다가 목록이 끝나요.",
        todo: "‘저지방’ 끝에서 Enter를 두 번 눌러 ‘2. ’로 올라가 보세요.",
    },
    Step {
        key: K::Strike,
        title: "물결로 취소선 긋기",
        text: "끝낸 일은 단어 뒤에 물결 두 개(~~)와 스페이스를 붙이면 취소선이 그어져요. 물결 세 개(~~~)면 그 줄 전체에 그어요.",
        todo: "‘우유’ 바로 뒤에 ‘~~’를 쓰고 스페이스를 눌러 보세요.",
    },
    Step {
        key: K::Check,
        title: "[]로 체크박스 만들기",
        text: "대괄호 두 개([])와 스페이스를 쓰면 체크박스 ☐가 생겨요. 체크박스를 누르면 ☑로 바뀌어요.",
        todo: "새 줄에 ‘[] ’를 쓰고, 생긴 ☐를 눌러 체크해 보세요.",
    },
    Step {
        key: K::Drag,
        title: "끌어다 놓으면 일정",
        text: "스티커 위쪽의 달력 아이콘(일정)을 잡고 캘린더 날짜 칸에 끌어다 놓으면 그날 일정이 돼요. 제목만 보이는 스티커는 제목 줄을 잡고 끌어도 돼요.",
        todo: "‘회의 준비’ 스티커의 달력 아이콘을 잡고 날짜 칸에 놓아 보세요.",
    },
    Step {
        key: K::ShiftDrag,
        title: "Shift + 끌어다 놓기 = 이날까지 할 일",
        text: "Shift를 누른 채 놓으면 그 날짜가 마감인 ‘이날까지 업무’가 돼요. 캘린더에 깃발과 D-day가 표시돼요.",
        todo: "‘보고서 제출’ 스티커의 달력 아이콘을 끌다가 Shift를 누른 채 날짜 칸에 놓아 보세요.",
    },
    Step {
        key: K::Arrange,
        title: "스티커 한 번에 정리하기",
        text: "스티커가 많아지면 화면 오른쪽에 제목만 한 줄씩 가지런히 세울 수 있어요. 제목 줄을 누르면 다시 펼쳐져요.",
        todo: "스티커 위쪽 빈 곳을 우클릭 → 화면 오른쪽에 정렬 → 모두 제목만 보이게",
    },
    Step {
        key: K::Filter,
        title: "프로젝트별로 모아 보기",
        text: "내 작업실에서는 프로젝트를 골라 그 프로젝트의 메모만 볼 수 있어요. 위쪽 ‘# 모든 프로젝트’에서 고르거나 왼쪽 프로젝트 목록을 눌러요. 다시 누르면 전체 보기로 돌아가요.",
        todo: "‘튜토리얼’ 프로젝트를 골라 보세요.",
    },
    Step {
        key: K::Complete,
        title: "완료 체크",
        text: "할 일을 끝냈으면 목록의 동그라미를 눌러 완료해요. 완료한 일은 목록에서 빠지고 업무일지에 차곡차곡 쌓여요.",
        todo: "‘완료 연습 · 기획서 검토’의 동그라미를 눌러 보세요.",
    },
    Step {
        key: K::Journal,
        title: "업무일지에서 확인하기",
        text: "편의기능 › 업무일지에는 완료한 일이 날짜별로 모여요. 하루나 한 달을 돌아볼 때 좋아요. 항목마다 댓글도 남길 수 있어요.",
        todo: "왼쪽 메뉴의 ‘편의기능’을 눌러 방금 완료한 일을 찾아보세요.",
    },
    Step {
        key: K::Clipboard,
        title: "클립보드 히스토리",
        text: "Ctrl+C로 복사한 글·링크·표·이미지가 ‘클립보드 히스토리’에 저절로 모여요. 항목을 한 번 누르면 다시 복사되니 Ctrl+V로 붙여 넣으면 돼요. 위쪽 검색과 필터로 찾고, 압정을 누르면 맨 위에 고정돼요. 비밀번호처럼 민감한 걸 복사할 땐 ‘일시 중지’를 눌러요.",
        todo: "왼쪽 메뉴의 ‘클립보드 히스토리’를 눌러 보세요.",
    },
    Step {
        key: K::Clipboy,
        title: "클립보이로 바로 붙여 넣기",
        text: "Alt+V를 누르면 최근에 복사한 100개를 담은 ‘클립보이’가 커서 옆에 떠요. 항목을 누르면 지금 커서 자리에 바로 붙여 넣어지고, ‘복사’는 클립보드에만 담아요. 위쪽 압정을 꽂으면 다른 곳을 눌러도 닫히지 않고, ‘전체비우기’는 압정·코멘트 항목만 남겨요.",
        todo: "연습 스티커 본문 끝을 누르고 Alt+V → ‘클립보이 연습 · 붙여넣기 성공!’을 눌러 보세요.",
    },
    Step {
        key: K::Detach,
        title: "캘린더 분리하기",
        text: "캘린더를 따로 띄워 두면 다른 일을 하면서도 일정을 볼 수 있어요. 단축키는 Ctrl+Alt+C예요.",
        todo: "밝게 보이는 ‘캘린더 분리창’ 버튼을 눌러 보세요.",
    },
    Step {
        key: K::DockDrag,
        title: "분리창에 끌어다 놓기",
        text: "분리창에도 스티커를 끌어다 놓을 수 있어요. 날짜 칸에 놓으면 그날 일정이 되고, 오른쪽 일정 목록에 놓으면 보고 있는 날짜의 일정이 돼요.",
        todo: "‘자료 정리’ 스티커의 달력 아이콘을 잡고 분리창의 날짜 칸에 놓아 보세요.",
    },
    Step {
        key: K::DockShift,
        title: "분리창에서 Shift + 끌어다 놓기",
        text: "분리창에서도 Shift를 누른 채 놓으면 그 날짜가 마감인 ‘이날까지 할 일’이 돼요. 깃발과 D-day가 붙어요.",
        todo: "‘계약서 회신’ 스티커의 달력 아이콘을 끌다가 Shift를 누른 채 분리창의 날짜 칸에 놓아 보세요.",
    },
    Step {
        key: K::Range,
        title: "날짜를 끌어 일정 추가하기",
        text: "빈 날짜 칸을 누른 채 다른 날짜까지 끌었다 놓으면 그 기간의 새 일정 스티커가 생겨요. 하루짜리는 날짜 칸의 +를 눌러요. 메인 창의 캘린더에서도 똑같아요.",
        todo: "분리창에서 한 날짜를 누른 채 이틀 뒤 날짜까지 끌었다 놓아 보세요.",
    },
    Step {
        key: K::Stand,
        title: "스탠딩 캘린더",
        text: "스탠딩 버튼을 누르면 캘린더가 지금 크기 그대로 바탕화면에 세워져요. 바탕화면 아이콘이 그 위에 올라가고, 같은 자리를 다시 누르면 풀려요.\n⚠ Windows 11 24H2에서는 스탠딩 캘린더가 동작하지 않아요. 그럴 땐 분리창으로 쓰세요.",
        todo: "분리창의 받침대 모양 스탠딩 버튼을 눌러 보세요.",
    },
    Step {
        key: K::End,
        title: "다 됐어요! 🎉",
        text: "핵심 기능은 모두 해 봤어요. 일정·알람·잠금, 휴지통, 나만의 위키는 ‘나만의 위키 › 사용법’에 정리돼 있어요. 나머지는 왼쪽 아래 ‘설정’에서 천천히 읽어 보세요.",
        todo: "",
    },
];

/// Progress of the step on screen in this session (not saved).
#[derive(Default)]
pub struct Live {
    /// The state whose set-up already ran.
    entered: Option<u32>,
    /// When the current step was found done.
    done_at: Option<f64>,
    /// When the current step started.
    entered_at: f64,
    /// How many memos there were then (the ones made since are new).
    notes_at: usize,
}

impl App {
    fn tutorial_project(&self) -> Option<usize> {
        self.data
            .projects
            .iter()
            .enumerate()
            .find(|(p, t)| t.as_str() == PROJECT && !self.extra.deleted_projects.contains(p))
            .map(|(p, _)| p)
    }
    /// Example memos still there, plus memos filed under the tutorial project.
    fn tutorial_notes(&self) -> Vec<usize> {
        let project = self.tutorial_project();
        (0..self.data.notes.len())
            .filter(|&i| {
                let n = &self.data.notes[i];
                !n.purged
                    && (self.extra.tutorial_notes.contains(&i)
                        || (project.is_some() && n.project == project))
            })
            .collect()
    }
    fn role(&self, role: Role) -> Option<usize> {
        let k = match role {
            Role::Practice => 0,
            Role::Drag => 1,
            Role::Deadline => 2,
            Role::Arrange(n) => 3 + n,
            Role::Complete => 6,
            Role::DockDrag => OLD_NOTES,
            Role::DockDeadline => OLD_NOTES + 1,
        };
        self.extra
            .tutorial_notes
            .get(k)
            .copied()
            .filter(|&i| i < self.data.notes.len() && !self.data.notes[i].purged)
    }
    /// Ask on a first run whether to take the tutorial.
    pub fn tutorial_offer(&mut self) {
        self.extra.tutorial = WELCOME;
    }
    /// Add the example memos unless they are there.
    fn tutorial_install(&mut self) {
        if self.role(Role::Practice).is_none() {
            self.tutorial_install_first();
        }
        self.tutorial_install_dock();
    }
    /// The practice memos for the calendar window (2.1), also for a tour
    /// whose memos came from an older version.
    fn tutorial_install_dock(&mut self) {
        if self.extra.tutorial_notes.len() != OLD_NOTES {
            return;
        }
        let a = tutorial_add(
            self,
            DOCK_TITLE,
            "위쪽 달력 아이콘을 잡고 캘린더 분리창의 날짜로 끌어다 놓아 보세요.",
            None,
        );
        let b = tutorial_add(
            self,
            DOCK_DEADLINE_TITLE,
            "위쪽 달력 아이콘을 잡고 Shift를 누른 채 분리창의 날짜로 끌어다 놓아 보세요.",
            None,
        );
        self.extra.tutorial_notes.extend([a, b]);
    }
    fn tutorial_install_first(&mut self) {
        if self.tutorial_project().is_none() {
            self.data.projects.push(PROJECT.into());
        }
        let today = Local::now().date_naive();
        let add = tutorial_add;
        let mut ids = vec![
            add(self, "", "", None),
            add(
                self,
                DRAG_TITLE,
                "위쪽 달력 아이콘을 잡고 캘린더 날짜로 끌어다 놓아 보세요.",
                None,
            ),
            add(
                self,
                DEADLINE_TITLE,
                "위쪽 달력 아이콘을 잡고 Shift를 누른 채 날짜로 끌어다 놓아 보세요.",
                None,
            ),
        ];
        for t in ARRANGE_TITLES {
            ids.push(add(self, t, "오른쪽에 제목만 보이게 정렬해 보세요.", None));
        }
        let i = add(
            self,
            "완료 연습 · 기획서 검토",
            "끝냈으면 동그라미를 눌러 완료!",
            Some(today),
        );
        self.data.notes[i].project = self.tutorial_project();
        ids.push(i);
        let day = |n: i64| Some(today + chrono::Duration::days(n));
        let i = add(self, "예시 일정 · 팀 회의", "오전 10시 회의", day(1));
        self.data.notes[i].time = chrono::NaiveTime::from_hms_opt(10, 0, 0).unwrap();
        ids.push(i);
        let i = add(
            self,
            "예시 일정 · 출장",
            "여러 날 일정은 색띠로 보여요.",
            day(4),
        );
        self.data.notes[i].duration = 3;
        ids.push(i);
        self.data.notes[ids[0]].color = PALETTE[0];
        self.extra.tutorial_notes = ids;
    }
    /// Erase the example memos and everything filed under the tutorial project.
    pub fn tutorial_erase(&mut self) {
        let notes = self.tutorial_notes();
        for &i in &notes {
            if let Some(w) = self.studio.windows.get_mut(&i) {
                w.open = false;
            }
            self.purge_note(i);
            let n = &mut self.data.notes[i];
            n.project = None;
            n.wiki = false;
            n.wiki_tab = None;
            n.folder = None;
            n.date = None;
            n.deadline = false;
        }
        let set: std::collections::HashSet<usize> = notes.into_iter().collect();
        self.data
            .links
            .retain(|(a, b)| !set.contains(a) && !set.contains(b));
        if let Some(p) = self.tutorial_project() {
            self.extra.deleted_projects.insert(p);
            self.extra.hidden_projects.remove(&p);
        }
        self.extra.tutorial_notes.clear();
        self.project_filter = None;
        self.picked.retain(|i| !set.contains(i));
        self.catalog.selected = self.catalog.selected.filter(|i| !set.contains(i));
        self.catalog.selection.items.retain(|i| !set.contains(i));
        if set.contains(&self.active) {
            self.editor = false;
            self.sticky = false;
        }
    }
    /// Start (or restart) the tour from the first step.
    pub fn tutorial_start(&mut self) {
        self.tutorial_install();
        self.extra.tutorial = STEP0;
        self.tutorial_live = Live::default();
    }
    fn show_main_page(&mut self, ctx: &egui::Context, page: usize) {
        self.page = page;
        self.settings = false;
        self.main_open = true;
        ctx.send_viewport_cmd_to(main_viewport(), egui::ViewportCommand::Minimized(false));
    }
    /// Open an example memo as a sticker in front of the other windows.
    fn open_role(&mut self, ctx: &egui::Context, role: Role, focus_body: bool) {
        let Some(i) = self.role(role) else {
            return;
        };
        ctx.send_viewport_cmd_to(crate::studio::viewport(i), egui::ViewportCommand::Focus);
        self.open_sticker(i, self.data.notes[i].date);
        if let Some(w) = self.studio.windows.get_mut(&i) {
            w.sticker();
            let offset = match role {
                Role::Arrange(k) => Vec2::splat((k as f32 - 1.) * 52.),
                _ => Vec2::ZERO,
            };
            w.center_for_tutorial(offset);
        }
        if focus_body {
            let len = self.data.notes[i].body.chars().count();
            self.data.notes[i].document.requested_selection = Some(len..len);
        }
    }
    fn close_role(&mut self, role: Role) {
        if let Some(w) = self
            .role(role)
            .and_then(|i| self.studio.windows.get_mut(&i))
        {
            w.open = false;
        }
    }
    /// What a step sets up when it starts (also "다시 보여 주기").
    fn enter_step(&mut self, ctx: &egui::Context, n: usize) {
        match STEPS[n].key {
            K::Project | K::Title | K::MultiList | K::ListUp | K::Strike | K::Check => {
                self.open_role(ctx, Role::Practice, true);
            }
            K::Drag => {
                self.close_role(Role::Practice);
                self.show_main_page(ctx, 5);
                self.open_role(ctx, Role::Drag, false);
            }
            K::ShiftDrag => {
                self.close_role(Role::Drag);
                self.show_main_page(ctx, 5);
                self.open_role(ctx, Role::Deadline, false);
            }
            K::Arrange => {
                self.close_role(Role::Deadline);
                for k in 0..3 {
                    self.open_role(ctx, Role::Arrange(k), false);
                }
            }
            K::Filter => {
                // The tidied title strips stay on top: out of the way.
                for k in 0..3 {
                    self.close_role(Role::Arrange(k));
                }
                self.project_filter = None;
                self.split = true;
                self.show_main_page(ctx, 0);
            }
            K::Complete => self.show_main_page(ctx, 0),
            K::Journal | K::Clipboard => {}
            K::Clipboy => {
                self.clipboard.clipboy_sample(CLIPBOY_SAMPLE);
                self.open_role(ctx, Role::Practice, true);
            }
            K::Detach => {
                self.extra.utilities_tab = 0;
                self.show_main_page(ctx, 5);
            }
            K::DockDrag | K::DockShift | K::Range => {
                // A tour from an older version has no memos for these yet.
                self.tutorial_install_dock();
                // Dropped on and dragged over the window, not the desktop
                // (a standing calendar may not take either), also when
                // it stood at the last quit.
                self.cal.stay_window();
                if self.cal.standing_active() {
                    self.restore_standing();
                }
                if self.cal.state == crate::calendar_dock::DockState::Hidden {
                    self.show_calendar();
                }
                let key = STEPS[n].key;
                if key != K::DockDrag {
                    self.close_role(Role::DockDrag);
                }
                if key != K::DockShift {
                    self.close_role(Role::DockDeadline);
                }
                match key {
                    K::DockDrag => self.open_role(ctx, Role::DockDrag, false),
                    K::DockShift => self.open_role(ctx, Role::DockDeadline, false),
                    _ => {}
                }
            }
            K::Stand if self.cal.state == crate::calendar_dock::DockState::Hidden => {
                self.show_calendar();
            }
            _ => {}
        }
    }
    /// Has the user done what step `n` asks?
    fn step_done(&self, n: usize) -> bool {
        let note = |r| self.role(r).map(|i| &self.data.notes[i]);
        let practice = note(Role::Practice);
        match STEPS[n].key {
            K::Project => practice.is_some_and(|p| p.project.is_some()),
            K::Title => practice.is_some_and(|p| !p.title.trim().is_empty()),
            K::MultiList => practice.is_some_and(|p| list_levels(&p.body).0),
            K::ListUp => practice.is_some_and(|p| list_levels(&p.body).1),
            K::Strike => practice.is_some_and(|p| {
                p.document.tabs.iter().any(|t| {
                    t.spans
                        .iter()
                        .any(|s| s.format.strike && !s.range.is_empty())
                })
            }),
            K::Check => practice.is_some_and(|p| p.body.contains('☑')),
            K::Drag => note(Role::Drag).is_some_and(|d| d.date.is_some() && !d.deadline),
            K::ShiftDrag => note(Role::Deadline).is_some_and(|d| d.date.is_some() && d.deadline),
            K::Arrange => (0..3).all(|k| {
                self.role(Role::Arrange(k))
                    .and_then(|i| self.studio.windows.get(&i))
                    .is_some_and(|w| w.open && w.mode == crate::studio::Mode::Title)
            }),
            K::Filter => self.project_filter.is_some_and(|p| p != NO_PROJECT),
            K::Complete => note(Role::Complete).is_some_and(|c| c.done),
            K::Journal => self.page == 6 && self.extra.utilities_tab == 0 && !self.settings,
            K::Clipboard => self.page == 7 && !self.settings,
            K::Clipboy => practice.is_some_and(|p| p.body.contains(CLIPBOY_SAMPLE)),
            K::Detach => self.cal.state != crate::calendar_dock::DockState::Hidden,
            K::DockDrag => note(Role::DockDrag).is_some_and(|d| d.date.is_some() && !d.deadline),
            K::DockShift => {
                note(Role::DockDeadline).is_some_and(|d| d.date.is_some() && d.deadline)
            }
            K::Range => !self.range_notes().is_empty(),
            K::Stand => self.cal.standing_pinned(),
            K::End => false,
        }
    }
    /// Schedules of several days made since the step started (dragged over
    /// days), the examples aside.
    fn range_notes(&self) -> Vec<usize> {
        (self.tutorial_live.notes_at..self.data.notes.len())
            .filter(|&i| {
                let n = &self.data.notes[i];
                !n.purged
                    && n.date.is_some()
                    && n.duration > 1
                    && !self.extra.tutorial_notes.contains(&i)
            })
            .collect()
    }
    pub(crate) fn tutorial_range_active(&self) -> bool {
        self.extra
            .tutorial
            .checked_sub(STEP0)
            .and_then(|step| STEPS.get(step as usize))
            .is_some_and(|step| step.key == K::Range)
    }

    /// File the practice range under the tutorial project so it can be
    /// cleaned up with the other examples at the end.
    fn file_range_notes(&mut self, n: usize) {
        if STEPS[n].key != K::Range {
            return;
        }
        let project = self.tutorial_project();
        for i in self.range_notes() {
            let note = &mut self.data.notes[i];
            if note.project.is_none() {
                note.project = project;
            }
        }
    }
    /// Which spots step `n` lights up.
    fn step_spots(&self, n: usize) -> Vec<Spot> {
        let note = |r| self.role(r).map(Spot::Note);
        match STEPS[n].key {
            K::Project | K::Title | K::MultiList | K::ListUp | K::Strike | K::Check => {
                note(Role::Practice).into_iter().collect()
            }
            K::Drag => note(Role::Drag).into_iter().chain([Spot::Grid]).collect(),
            K::ShiftDrag => note(Role::Deadline)
                .into_iter()
                .chain([Spot::Grid])
                .collect(),
            K::Arrange => (0..3).filter_map(|k| note(Role::Arrange(k))).collect(),
            K::Filter => vec![Spot::Filter, Spot::Projects],
            K::Complete => vec![Spot::List],
            K::Journal => vec![Spot::NavUtilities, Spot::Utilities],
            K::Clipboard => vec![Spot::NavClipboard, Spot::Clipboard],
            K::Clipboy => note(Role::Practice)
                .into_iter()
                .chain([Spot::Clipboy])
                .collect(),
            K::Detach => vec![Spot::Detach],
            K::DockDrag => note(Role::DockDrag)
                .into_iter()
                .chain([Spot::DockGrid])
                .collect(),
            K::DockShift => note(Role::DockDeadline)
                .into_iter()
                .chain([Spot::DockGrid])
                .collect(),
            K::Range => vec![Spot::DockGrid],
            K::Stand => vec![Spot::Stand],
            K::End => vec![],
        }
    }
    /// The tour: offer, steps and the closing choice. Drawn last in a frame,
    /// after every window marked its spots.
    pub fn tutorial_window(&mut self, ctx: &egui::Context) {
        let state = self.extra.tutorial;
        if state == 0 {
            dim::hide();
            return;
        }
        let now = ctx.input(|i| i.time);
        let step = state
            .checked_sub(STEP0)
            .map(|n| n as usize)
            .filter(|n| *n < STEPS.len());
        if let Some(n) = step {
            if self.tutorial_live.entered != Some(state) {
                self.tutorial_live = Live {
                    entered: Some(state),
                    done_at: None,
                    entered_at: now,
                    notes_at: self.data.notes.len(),
                };
                self.enter_step(ctx, n);
            }
            // Memo windows just opened come up in front of the main window
            // (a window that does not exist yet misses a single request).
            if now - self.tutorial_live.entered_at < 0.8 {
                for spot in self.step_spots(n) {
                    if let Spot::Note(i) = spot {
                        ctx.send_viewport_cmd_to(
                            crate::studio::viewport(i),
                            egui::ViewportCommand::Focus,
                        );
                    }
                }
            }
            if self.tutorial_live.done_at.is_none() && self.step_done(n) {
                self.tutorial_live.done_at = Some(now);
                self.file_range_notes(n);
            }
        }
        // Where the lit parts are this frame.
        let marks = marks();
        let wanted = step.map(|n| self.step_spots(n)).unwrap_or_default();
        let targets: Vec<[i32; 4]> = marks
            .iter()
            .filter(|(s, _)| wanted.contains(s))
            .map(|(_, r)| pad(*r, 6))
            .collect();
        let popups = marks
            .iter()
            .filter(|(s, _)| *s == Spot::Popup)
            .map(|(_, r)| *r)
            .collect::<Vec<_>>();
        // A step that points at something dims the rest once it is on
        // screen; the opening, the last step and the closing choice dim all.
        let dim = wanted.is_empty() || !targets.is_empty();
        let holes: Vec<[i32; 4]> = targets.iter().chain(&popups).copied().collect();
        let screen = virtual_screen();
        if dim {
            dim::show(screen, &holes, &targets);
        } else {
            dim::hide();
        }
        let mut cmd = None;
        // The tip keeps clear of the lit parts and of open menus.
        tip_window(ctx, screen, &holes, |ui| {
            cmd = self.tip_contents(ui, state, step, now);
        });
        match cmd {
            None => {}
            Some(Cmd::Start) => self.tutorial_start(),
            Some(Cmd::Skip) => {
                // Nothing was added yet when the offer is declined.
                self.extra.tutorial = if self.tutorial_notes().is_empty() {
                    0
                } else {
                    FINISH
                };
            }
            Some(Cmd::Prev) => self.extra.tutorial = state.saturating_sub(1).max(STEP0),
            Some(Cmd::Next) => self.extra.tutorial = next_state(state),
            Some(Cmd::Again) => {
                if let Some(n) = step {
                    self.enter_step(ctx, n);
                }
            }
            Some(Cmd::Guide) => {
                self.open_guide();
                self.show_main_page(ctx, 1);
            }
            Some(Cmd::Keep) => {
                self.extra.tutorial = 0;
                self.toast = "연습 메모를 남겨 두었어요".into();
            }
            Some(Cmd::Erase) => {
                self.tutorial_erase();
                self.extra.tutorial = 0;
                self.page = 0;
                self.toast = "연습 메모를 모두 지우고 깨끗하게 시작해요".into();
            }
            Some(Cmd::Back) => self.extra.tutorial = STEP0,
        }
        // A finished step moves on by itself after a moment of praise.
        if let Some(at) = self.tutorial_live.done_at {
            if now - at > 1.6
                && self.extra.tutorial == state
                && step.is_some_and(|n| STEPS[n].auto())
            {
                self.extra.tutorial = next_state(state);
            }
        }
        // The highlight follows moving windows and the demo plays.
        ctx.request_repaint_after(std::time::Duration::from_millis(50));
    }
    fn tip_contents(
        &mut self,
        ui: &mut egui::Ui,
        state: u32,
        step: Option<usize>,
        now: f64,
    ) -> Option<Cmd> {
        let mut cmd = None;
        let tone = crate::ui::t(ui);
        if state == WELCOME {
            ui.label(
                RichText::new("OhMemoJang에 오신 걸 환영해요! 👋")
                    .strong()
                    .size(17.),
            );
            ui.add_space(8.);
            ui.label("실제 화면에서 핵심 기능을 하나씩 직접 해 보는 튜토리얼이 있어요. 설명하는 곳만 밝게 보여 드리고, 해내면 저절로 다음으로 넘어가요. 5분이면 충분해요.");
            ui.add_space(6.);
            crate::ui::hint(
                ui,
                "연습용 메모 몇 개를 만들어요. 언제든 건너뛸 수 있고, 끝나면 지울지 고를 수 있어요.",
            );
            ui.add_space(6.);
            // Why a Start menu shortcut appears: alarms are Windows notifications.
            crate::ui::hint(
                ui,
                "🔔 메모 알람은 Windows 알림으로 알려 드려요. Windows는 시작 메뉴 바로가기가 있는 앱의 알림만 보여 줘요. 그래서 OhMemoJang이 시작 메뉴에 바로가기를 만들어 둬요.",
            );
            ui.add_space(6.);
            crate::ui::hint(
                ui,
                format!(
                    "💾 메모는 이 PC의 {} 에 저장돼요. 다른 곳(예: 클라우드 폴더)으로 옮기려면 설정 › 백업 / 복원의 ‘데이터 위치’에서 바꿀 수 있어요.",
                    crate::storage::data_dir(&self.prefs.data_path).display()
                ),
            );
            ui.add_space(14.);
            ui.horizontal(|ui| {
                if crate::ui::primary(ui, "튜토리얼 시작").clicked() {
                    cmd = Some(Cmd::Start);
                }
                if ui.button("건너뛰기").clicked() {
                    cmd = Some(Cmd::Skip);
                }
            });
            return cmd;
        }
        if state == FINISH {
            let count = self.tutorial_notes().len();
            ui.label(RichText::new("수고하셨어요! 🎉").strong().size(17.));
            ui.add_space(8.);
            ui.label(format!(
                "연습용 메모와 일정이 {count}개 남아 있어요. 어떻게 할까요?"
            ));
            ui.add_space(4.);
            crate::ui::hint(
                ui,
                "지우면 연습 메모와 ‘튜토리얼’ 프로젝트의 메모가 모두 사라져요. 사용법과 내가 만든 다른 메모는 그대로예요.",
            );
            ui.add_space(12.);
            if crate::ui::primary(ui, "연습 메모 지우고 깨끗하게 시작").clicked() {
                cmd = Some(Cmd::Erase);
            }
            ui.add_space(4.);
            ui.horizontal(|ui| {
                if ui.button("남겨 두기").clicked() {
                    cmd = Some(Cmd::Keep);
                }
                if ui.button("처음부터 다시").clicked() {
                    cmd = Some(Cmd::Back);
                }
            });
            return cmd;
        }
        let n = step?;
        let s = &STEPS[n];
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("{} / {}", n + 1, STEPS.len()))
                    .small()
                    .color(tone.muted),
            );
            for k in 0..STEPS.len() {
                let (r, _) = ui.allocate_exact_size(Vec2::new(8., 8.), egui::Sense::hover());
                ui.painter().circle_filled(
                    r.center(),
                    3.,
                    if k <= n {
                        crate::ui::ACCENT
                    } else {
                        tone.line_strong
                    },
                );
            }
        });
        ui.add_space(4.);
        ui.label(RichText::new(s.title).strong().size(17.));
        ui.add_space(4.);
        ui.label(s.text);
        if s.key != K::End {
            ui.add_space(8.);
            draw_demo(ui, s.key, now);
        }
        ui.add_space(8.);
        let done = self.tutorial_live.done_at.is_some();
        if done {
            ui.label(
                RichText::new(if s.auto() {
                    "🎉 잘했어요! 다음으로 넘어가요"
                } else {
                    "🎉 잘했어요! 다 봤으면 ‘다음’을 눌러요"
                })
                .strong()
                .color(Color32::from_rgb(40, 140, 80)),
            );
        } else if !s.todo.is_empty() {
            ui.label(RichText::new(format!("👉 {}", s.todo)).strong());
        }
        ui.add_space(10.);
        ui.horizontal(|ui| {
            if ui.button("건너뛰기").clicked() {
                cmd = Some(Cmd::Skip);
            }
            let last = n + 1 == STEPS.len();
            if last && ui.button("사용법 보기").clicked() {
                cmd = Some(Cmd::Guide);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let next = if last { "마치기" } else { "다음 ▶" };
                let r = if last || done {
                    crate::ui::primary(ui, next)
                } else {
                    ui.button(next)
                };
                if r.clicked() {
                    cmd = Some(Cmd::Next);
                }
                if n > 0 && ui.button("◀").on_hover_text("이전").clicked() {
                    cmd = Some(Cmd::Prev);
                }
                if !last && !done && ui.button("다시 보여 주기").clicked() {
                    cmd = Some(Cmd::Again);
                }
            });
        });
        cmd
    }
}

/// Add an example memo; its index.
fn tutorial_add(a: &mut App, title: &str, body: &str, date: Option<NaiveDate>) -> usize {
    let i = a.data.notes.len();
    let mut n = Note::new(title, date, i);
    n.body = body.into();
    a.data.notes.push(n);
    i
}

fn next_state(state: u32) -> u32 {
    if state + 1 >= STEP0 + STEPS.len() as u32 {
        FINISH
    } else {
        state + 1
    }
}

enum Cmd {
    Start,
    Skip,
    Prev,
    Next,
    Again,
    Guide,
    Keep,
    Erase,
    Back,
}

fn pad(r: [i32; 4], d: i32) -> [i32; 4] {
    [r[0] - d, r[1] - d, r[2] + d, r[3] + d]
}

/// The whole desktop (all monitors), physical pixels.
fn virtual_screen() -> [i32; 4] {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
            SM_YVIRTUALSCREEN,
        };
        let (x, y) = (
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
        );
        let (w, h) = (
            GetSystemMetrics(SM_CXVIRTUALSCREEN),
            GetSystemMetrics(SM_CYVIRTUALSCREEN),
        );
        if w > 0 && h > 0 {
            return [x, y, x + w, y + h];
        }
    }
    [0, 0, 1920, 1080]
}

/// Native state of a tutorial window, kept between frames.
#[derive(Clone, Default)]
struct Native {
    hwnd: isize,
    placed: [i32; 4],
    styled: bool,
}

fn native_state(ctx: &egui::Context) -> Native {
    let key = egui::Id::new(("tutorial_native", ctx.viewport_id()));
    let mut s: Native = ctx.data(|d| d.get_temp(key)).unwrap_or_default();
    if s.hwnd == 0 || !is_window(s.hwnd) {
        s = Native {
            hwnd: find_by_title(ctx).unwrap_or(0),
            ..Default::default()
        };
    }
    s
}

/// The tutorial's windows have titles of their own: found by title alone (a
/// screen-sized window may not end up exactly where it was asked to be).
fn find_by_title(ctx: &egui::Context) -> Option<isize> {
    let title = ctx.input(|i| i.viewport().title.clone())?;
    #[cfg(windows)]
    unsafe {
        let wide: Vec<u16> = title.encode_utf16().chain([0]).collect();
        let h = windows_sys::Win32::UI::WindowsAndMessaging::FindWindowW(
            std::ptr::null(),
            wide.as_ptr(),
        );
        if !h.is_null() {
            return Some(h as isize);
        }
    }
    let _ = title;
    crate::win_style::hwnd(ctx)
}

fn store_native(ctx: &egui::Context, s: Native) {
    let key = egui::Id::new(("tutorial_native", ctx.viewport_id()));
    ctx.data_mut(|d| d.insert_temp(key, s));
}

/// The dark screen: a plain native window (no OpenGL), painted black with a
/// yellow ring around each lit part and made see-through as a whole. The
/// holes are cut out of its region, so they show and take clicks as usual;
/// clicks on the dark part go nowhere.
#[cfg(windows)]
mod dim {
    use super::RAISE_TIP;
    use std::sync::atomic::{AtomicIsize, Ordering};
    use std::sync::Mutex;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
    use windows_sys::Win32::Graphics::Gdi::*;
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    static WINDOW: AtomicIsize = AtomicIsize::new(0);
    /// What is shown: screen, holes, rings (rings relative to the window).
    type Shown = ([i32; 4], Vec<[i32; 4]>, Vec<[i32; 4]>);
    static SHOWN: Mutex<Option<Shown>> = Mutex::new(None);

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain([0]).collect()
    }

    unsafe extern "system" fn proc(h: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
        match msg {
            WM_PAINT => {
                let mut ps: PAINTSTRUCT = std::mem::zeroed();
                let dc = BeginPaint(h, &mut ps);
                let mut rc: RECT = std::mem::zeroed();
                GetClientRect(h, &mut rc);
                FillRect(dc, &rc, GetStockObject(BLACK_BRUSH) as HBRUSH);
                let rings = SHOWN
                    .lock()
                    .ok()
                    .and_then(|s| s.as_ref().map(|s| s.2.clone()))
                    .unwrap_or_default();
                let pen = CreatePen(PS_SOLID, 6, 0x003C_C8FF);
                let old_pen = SelectObject(dc, pen);
                let old_brush = SelectObject(dc, GetStockObject(NULL_BRUSH));
                for r in rings {
                    RoundRect(dc, r[0] - 5, r[1] - 5, r[2] + 5, r[3] + 5, 24, 24);
                }
                SelectObject(dc, old_pen);
                SelectObject(dc, old_brush);
                DeleteObject(pen);
                EndPaint(h, &ps);
                0
            }
            WM_ERASEBKGND => 1,
            WM_MOUSEACTIVATE => MA_NOACTIVATE as LRESULT,
            _ => DefWindowProcW(h, msg, w, l),
        }
    }

    unsafe fn create() -> HWND {
        let class = wide("OhMemoJangTutorialDim");
        let instance = GetModuleHandleW(std::ptr::null());
        let wc = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance,
            hIcon: std::ptr::null_mut(),
            hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
            hbrBackground: std::ptr::null_mut(),
            lpszMenuName: std::ptr::null(),
            lpszClassName: class.as_ptr(),
        };
        RegisterClassW(&wc);
        let title = wide("OhMemoJang 튜토리얼 배경");
        let h = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class.as_ptr(),
            title.as_ptr(),
            WS_POPUP,
            0,
            0,
            1,
            1,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            instance,
            std::ptr::null(),
        );
        if !h.is_null() {
            SetLayeredWindowAttributes(h, 0, 175, LWA_ALPHA);
        }
        h
    }

    pub fn show(screen: [i32; 4], holes: &[[i32; 4]], rings: &[[i32; 4]]) {
        // SAFETY: plain Win32 calls on a window of this (the UI) thread.
        unsafe {
            let mut h = WINDOW.load(Ordering::Relaxed) as HWND;
            if h.is_null() || IsWindow(h) == 0 {
                h = create();
                if h.is_null() {
                    return;
                }
                WINDOW.store(h as isize, Ordering::Relaxed);
            }
            let rel = |r: &[i32; 4]| {
                [
                    r[0] - screen[0],
                    r[1] - screen[1],
                    r[2] - screen[0],
                    r[3] - screen[1],
                ]
            };
            let next = (
                screen,
                holes.iter().map(rel).collect::<Vec<_>>(),
                rings.iter().map(rel).collect::<Vec<_>>(),
            );
            let mut shown = SHOWN.lock().unwrap_or_else(|e| e.into_inner());
            let before = shown.clone();
            if before.as_ref().is_some_and(|b| *b == next) && IsWindowVisible(h) != 0 {
                return;
            }
            *shown = Some(next.clone());
            drop(shown);
            if before.as_ref().is_none_or(|b| b.0 != screen) || IsWindowVisible(h) == 0 {
                SetWindowPos(
                    h,
                    HWND_TOPMOST,
                    screen[0],
                    screen[1],
                    screen[2] - screen[0],
                    screen[3] - screen[1],
                    SWP_NOACTIVATE | SWP_SHOWWINDOW,
                );
                RAISE_TIP.store(true, Ordering::Relaxed);
            }
            if before.as_ref().is_none_or(|b| b.1 != next.1) {
                let region = CreateRectRgn(0, 0, screen[2] - screen[0], screen[3] - screen[1]);
                for r in &next.1 {
                    let hole = CreateRectRgn(r[0], r[1], r[2], r[3]);
                    CombineRgn(region, region, hole, RGN_DIFF);
                    DeleteObject(hole);
                }
                // The system owns the region from here on.
                SetWindowRgn(h, region, 1);
            }
            InvalidateRect(h, std::ptr::null(), 1);
        }
    }

    pub fn hide() {
        let h = WINDOW.load(Ordering::Relaxed) as HWND;
        // SAFETY: as above.
        unsafe {
            if !h.is_null() && IsWindow(h) != 0 && IsWindowVisible(h) != 0 {
                ShowWindow(h, SW_HIDE);
            }
        }
        if let Ok(mut s) = SHOWN.lock() {
            *s = None;
        }
    }
}

#[cfg(not(windows))]
mod dim {
    pub fn show(_: [i32; 4], _: &[[i32; 4]], _: &[[i32; 4]]) {}
    pub fn hide() {}
}

/// The explanation window, next to the lit parts (centered when none).
fn tip_window(
    ctx: &egui::Context,
    screen: [i32; 4],
    targets: &[[i32; 4]],
    add: impl FnOnce(&mut egui::Ui),
) {
    // As tall as its contents (measured last frame).
    let key = egui::Id::new("tutorial_tip_height");
    let height = ctx.data(|d| d.get_temp::<f32>(key)).unwrap_or(320.);
    let size = egui::vec2(400., height);
    let builder = egui::ViewportBuilder::default()
        .with_title("OhMemoJang 튜토리얼")
        .with_inner_size(size)
        .with_decorations(false)
        .with_resizable(false)
        .with_taskbar(false)
        .with_active(false)
        .with_window_level(egui::WindowLevel::AlwaysOnTop);
    let mut add = Some(add);
    ctx.show_viewport_immediate(
        egui::ViewportId::from_hash_of("tutorial_tip"),
        builder,
        |ctx, _| {
            crate::win_style::shape(ctx, 1.);
            let ppp = ctx
                .input(|i| i.viewport().native_pixels_per_point)
                .unwrap_or(1.);
            let mut s = native_state(ctx);
            if s.hwnd != 0 {
                if !s.styled {
                    style_window(s.hwnd, None);
                    s.styled = true;
                }
                // Within one monitor: the one with the lit parts (or the
                // pointer), sized for that monitor's scale. Placed against
                // the whole desktop, the tip could land in the gap between
                // monitors of different sizes, or straddle two of different
                // scales and jump back and forth between them.
                let (area, scale) = monitor_near(targets).unwrap_or((screen, ppp));
                let w = (size.x * scale) as i32;
                let h = (size.y * scale) as i32;
                let at = tip_position(area, targets, w, h);
                // Follows the windows it points at. It goes above the dark
                // screen, but not above menus opened later: those must stay
                // usable.
                let raise = RAISE_TIP.swap(false, Ordering::Relaxed);
                place(s.hwnd, [at.0, at.1, at.0 + w, at.1 + h], false, raise);
                s.placed = [at.0, at.1, at.0 + w, at.1 + h];
            }
            store_native(ctx, s);
            let fill = ctx.style().visuals.panel_fill;
            egui::CentralPanel::default()
                .frame(
                    egui::Frame::NONE
                        .fill(fill)
                        .stroke(egui::Stroke::new(2_f32, crate::ui::ACCENT))
                        .inner_margin(egui::Margin::same(18)),
                )
                .show(ctx, |ui| {
                    if let Some(add) = add.take() {
                        add(ui);
                    }
                    let needed = (ui.min_rect().height() + 38.).ceil();
                    if (needed - height).abs() > 1. {
                        ctx.data_mut(|d| d.insert_temp(key, needed));
                        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                            size.x, needed,
                        )));
                    }
                });
        },
    );
}

/// Right of the lit parts if there is room, else left, below, above; the
/// main screen's center when nothing is lit.
fn tip_position(screen: [i32; 4], targets: &[[i32; 4]], w: i32, h: i32) -> (i32, i32) {
    let margin = 24;
    let (sl, st, sr, sb) = (screen[0], screen[1], screen[2], screen[3] - 60);
    let clamp_y = |y: i32| y.clamp(st + margin, (sb - h - margin).max(st));
    let clamp_x = |x: i32| x.clamp(sl + margin, (sr - w - margin).max(sl));
    if targets.is_empty() {
        let c = ((screen[0] + screen[2]) / 2, (screen[1] + screen[3]) / 2);
        return (clamp_x(c.0 - w / 2), clamp_y(c.1 - h / 2));
    }
    let u = targets.iter().fold(targets[0], |a, r| {
        [
            a[0].min(r[0]),
            a[1].min(r[1]),
            a[2].max(r[2]),
            a[3].max(r[3]),
        ]
    });
    // Beside the lit parts first; where they fill the screen, the spot
    // (corners included) that hides the least of them.
    let candidates = [
        (u[2] + margin, clamp_y(u[1])),
        (u[0] - margin - w, clamp_y(u[1])),
        (clamp_x(u[0]), u[3] + margin),
        (clamp_x(u[0]), u[1] - margin - h),
        (clamp_x(sr - w - margin), clamp_y(sb - h - margin)),
        (clamp_x(sl + margin), clamp_y(sb - h - margin)),
        (clamp_x(sr - w - margin), clamp_y(st + margin)),
        (clamp_x(sl + margin), clamp_y(st + margin)),
    ];
    let hidden = |(x, y): (i32, i32)| -> i64 {
        targets
            .iter()
            .map(|r| {
                let dx = (r[2].min(x + w) - r[0].max(x)).max(0) as i64;
                let dy = (r[3].min(y + h) - r[1].max(y)).max(0) as i64;
                dx * dy
            })
            .sum()
    };
    let on_screen = |(x, y): (i32, i32)| x >= sl && y >= st && x + w <= sr && y + h <= sb;
    candidates
        .into_iter()
        .filter(|c| on_screen(*c))
        .min_by_key(|c| hidden(*c))
        .unwrap_or((clamp_x(sr - w - margin), clamp_y(sb - h - margin)))
}

/// The monitor with most of the lit parts (the one under the pointer when
/// nothing is lit): its bounds in physical pixels and its scale.
fn monitor_near(targets: &[[i32; 4]]) -> Option<([i32; 4], f32)> {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::Foundation::{POINT, RECT};
        use windows_sys::Win32::Graphics::Gdi::{
            GetMonitorInfoW, MonitorFromPoint, MonitorFromRect, MONITORINFO,
            MONITOR_DEFAULTTONEAREST,
        };
        use windows_sys::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
        use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;
        let monitor = if let Some(first) = targets.first() {
            let u = targets.iter().fold(*first, |a, r| {
                [
                    a[0].min(r[0]),
                    a[1].min(r[1]),
                    a[2].max(r[2]),
                    a[3].max(r[3]),
                ]
            });
            let r = RECT {
                left: u[0],
                top: u[1],
                right: u[2],
                bottom: u[3],
            };
            MonitorFromRect(&r, MONITOR_DEFAULTTONEAREST)
        } else {
            let mut p = POINT { x: 0, y: 0 };
            GetCursorPos(&mut p);
            MonitorFromPoint(p, MONITOR_DEFAULTTONEAREST)
        };
        if monitor.is_null() {
            return None;
        }
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(monitor, &mut info) == 0 {
            return None;
        }
        let m = info.rcMonitor;
        let (mut dx, mut dy) = (0u32, 0u32);
        let scale = if GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dx, &mut dy) == 0 && dx > 0
        {
            dx as f32 / 96.
        } else {
            return None;
        };
        Some(([m.left, m.top, m.right, m.bottom], scale))
    }
    #[cfg(not(windows))]
    {
        let _ = targets;
        None
    }
}

#[cfg(windows)]
fn is_window(h: isize) -> bool {
    unsafe { windows_sys::Win32::UI::WindowsAndMessaging::IsWindow(h as _) != 0 }
}
#[cfg(not(windows))]
fn is_window(_: isize) -> bool {
    false
}

/// Never take the focus (typing goes on in the memo), no taskbar button, and
/// for the dark screen a whole-window opacity.
fn style_window(h: isize, alpha: Option<u8>) {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::*;
        let h = h as windows_sys::Win32::Foundation::HWND;
        let ex = GetWindowLongPtrW(h, GWL_EXSTYLE);
        let mut want = ex | (WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW) as isize;
        if alpha.is_some() {
            want |= WS_EX_LAYERED as isize;
        }
        if want != ex {
            SetWindowLongPtrW(h, GWL_EXSTYLE, want);
        }
        if let Some(a) = alpha {
            SetLayeredWindowAttributes(h, 0, a, LWA_ALPHA);
        }
    }
    #[cfg(not(windows))]
    let _ = (h, alpha);
}

/// Set by the dark screen when it went on top of everything again: the tip
/// then has to go above it once more.
static RAISE_TIP: AtomicBool = AtomicBool::new(true);

/// At `r` (physical pixels) without taking the focus; brought to the top of
/// the always-on-top windows only when `raise`.
fn place(h: isize, r: [i32; 4], resize: bool, raise: bool) {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::*;
        let flags = SWP_NOACTIVATE
            | if resize { 0 } else { SWP_NOSIZE }
            | if raise { 0 } else { SWP_NOZORDER };
        SetWindowPos(
            h as _,
            HWND_TOPMOST,
            r[0],
            r[1],
            r[2] - r[0],
            r[3] - r[1],
            flags,
        );
    }
    #[cfg(not(windows))]
    let _ = (h, r, resize, raise);
}

// ---------------------------------------------------------------------------
// Mini demos: a looping cartoon of what the step asks for.

const PAPER: Color32 = Color32::from_rgb(255, 236, 153);
const INK: Color32 = Color32::from_rgb(50, 46, 40);
const SOFT: Color32 = Color32::from_rgb(140, 132, 120);

fn typed(s: &str, t: f64, from: f64, cps: f64) -> String {
    let n = ((t - from) * cps).max(0.) as usize;
    s.chars().take(n).collect()
}

fn ease(x: f64) -> f32 {
    let x = x.clamp(0., 1.);
    (x * x * (3. - 2. * x)) as f32
}

fn font(size: f32) -> egui::FontId {
    egui::FontId::proportional(size)
}

fn draw_demo(ui: &mut egui::Ui, demo: K, now: f64) {
    let (rect, _) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 132.), egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 10., Color32::from_rgb(240, 238, 232));
    p.text(
        rect.right_top() + Vec2::new(-8., 6.),
        egui::Align2::RIGHT_TOP,
        "시연",
        font(10.),
        SOFT,
    );
    let t = now % 5.;
    match demo {
        K::Project | K::Title | K::MultiList | K::ListUp | K::Strike | K::Check => {
            sticker_demo(&p, rect, demo, t)
        }
        K::Drag => drag_demo(&p, rect, false, t, "회의 준비"),
        K::ShiftDrag => drag_demo(&p, rect, true, t, "보고서 제출"),
        K::DockDrag | K::DockShift => {
            let shift = demo == K::DockShift;
            let label = if shift {
                "계약서 회신"
            } else {
                "자료 정리"
            };
            drag_demo(&p, rect, shift, t, label);
            window_caption(&p, rect, "캘린더 분리창");
        }
        K::Range => range_demo(&p, rect, t),
        K::Clipboard => clipboard_demo(&p, rect, t),
        K::Clipboy => clipboy_demo(&p, rect, t),
        K::Filter => filter_demo(&p, rect, t),
        K::Complete => complete_demo(&p, rect, t),
        K::Journal => journal_demo(&p, rect, t),
        K::End => {}
        K::Arrange => arrange_demo(&p, rect, t),
        K::Detach => detach_demo(&p, rect, t),
        K::Stand => stand_demo(&p, rect, t),
    }
}

fn text(p: &egui::Painter, at: egui::Pos2, s: &str, size: f32, color: Color32) -> egui::Rect {
    p.text(at, egui::Align2::LEFT_TOP, s, font(size), color)
}

fn caret(p: &egui::Painter, after: egui::Rect, t: f64) {
    if (t * 2.) as i64 % 2 == 0 {
        p.line_segment(
            [
                after.right_top() + Vec2::new(1., 0.),
                after.right_bottom() + Vec2::new(1., 0.),
            ],
            egui::Stroke::new(1.5_f32, INK),
        );
    }
}

/// A sticker being typed into.
fn sticker_demo(p: &egui::Painter, rect: egui::Rect, demo: K, t: f64) {
    let card = egui::Rect::from_min_size(rect.min + Vec2::new(14., 12.), Vec2::new(200., 108.));
    p.rect_filled(card, 8., PAPER);
    let title_at = card.min + Vec2::new(10., 8.);
    let body_at = card.min + Vec2::new(10., 34.);
    p.line_segment(
        [
            card.min + Vec2::new(8., 28.),
            card.right_top() + Vec2::new(-8., 28.),
        ],
        egui::Stroke::new(1_f32, Color32::from_rgb(220, 200, 120)),
    );
    let line = 17.;
    let mut title = String::new();
    // (text, characters before the struck word)
    let mut lines: Vec<(String, Option<usize>)> = vec![];
    let mut chip = false;
    let mut note = "";
    match demo {
        K::Project => {
            if t > 2.6 {
                chip = true;
                lines.push((String::new(), None));
                note = "스페이스 → 프로젝트로!";
            } else {
                lines.push((typed("#튜토리얼 ", t, 0.4, 4.), None));
            }
        }
        K::Title => {
            lines.push((typed("장보기 목록", t, 0.4, 5.), None));
            if t > 2.4 {
                title = "장보기 목록".into();
                lines.push((String::new(), None));
                note = "첫 줄 → 제목";
            }
        }
        K::MultiList => {
            if t < 1.6 {
                lines.push((typed("1. 우유", t, 0.2, 6.), None));
            } else {
                lines.push(("1. 우유".into(), None));
                if t < 2.4 {
                    lines.push(("2. ".into(), None));
                    note = "Enter → ‘2. ’ 자동";
                } else {
                    lines.push((format!("    a) {}", typed("저지방", t, 3.0, 5.)), None));
                    note = "‘a) ’ → 한 단계 아래로";
                }
            }
        }
        K::ListUp => {
            lines.push(("1. 우유".into(), None));
            lines.push(("    a) 저지방".into(), None));
            if t < 1.4 {
                note = "끝에서 Enter";
            } else if t < 2.8 {
                lines.push(("    b) ".into(), None));
                note = "빈 항목에서 한 번 더 Enter";
            } else {
                lines.push(("2. ".into(), None));
                note = "위 단계 ‘2. ’로!";
            }
        }
        K::Strike => {
            if t < 2.6 {
                lines.push((format!("- 우유{}", typed("~~ ", t, 0.6, 3.)), None));
            } else {
                lines.push(("- 우유".into(), Some(2)));
                note = "~~ + 스페이스 → 취소선";
            }
            lines.push(("- 계란".into(), None));
        }
        K::Check => {
            if t < 1.8 {
                lines.push((typed("[] ", t, 0.4, 3.), None));
            } else {
                let checked = t > 3.2;
                lines.push((format!("{} 빵 사기", if checked { "☑" } else { "☐" }), None));
                note = if checked {
                    "누르면 ☑"
                } else {
                    "[] + 스페이스 → ☐"
                };
                if (2.6..3.4).contains(&t) {
                    // A click on the box.
                    p.circle_stroke(
                        body_at + Vec2::new(6., 8.),
                        9.,
                        egui::Stroke::new(2_f32, crate::ui::ACCENT),
                    );
                }
            }
        }
        _ => {}
    }
    text(
        p,
        title_at,
        if title.is_empty() {
            "제목 없음"
        } else {
            &title
        },
        13.,
        if title.is_empty() { SOFT } else { INK },
    );
    let mut last = egui::Rect::from_min_size(body_at, Vec2::new(0., 14.));
    for (k, (s, strike)) in lines.iter().enumerate() {
        let at = body_at + Vec2::new(0., k as f32 * line);
        last = text(p, at, s, 12., INK);
        if last.width() == 0. {
            last = egui::Rect::from_min_size(at, Vec2::new(0., 14.));
        }
        if let Some(from) = strike {
            let before: String = s.chars().take(*from).collect();
            let skip = p.layout_no_wrap(before, font(12.), INK).rect.width();
            let y = last.center().y;
            p.line_segment(
                [egui::pos2(at.x + skip, y), egui::pos2(last.right(), y)],
                egui::Stroke::new(1.5_f32, INK),
            );
        }
    }
    caret(p, last, t);
    if chip {
        let r = egui::Rect::from_min_size(
            card.left_bottom() + Vec2::new(10., -26.),
            Vec2::new(78., 18.),
        );
        p.rect_filled(r, 9., crate::ui::ACCENT);
        p.text(
            r.center(),
            egui::Align2::CENTER_CENTER,
            "# 튜토리얼",
            font(11.),
            Color32::WHITE,
        );
    }
    if !note.is_empty() {
        p.text(
            egui::pos2(card.right() + 12., rect.center().y),
            egui::Align2::LEFT_CENTER,
            note,
            font(12.),
            crate::ui::ACCENT,
        );
    }
}

fn mini_calendar(p: &egui::Painter, area: egui::Rect) -> Vec<egui::Rect> {
    p.rect_filled(area, 6., Color32::WHITE);
    let (cols, rows) = (7, 3);
    let cw = area.width() / cols as f32;
    let ch = area.height() / rows as f32;
    let mut cells = vec![];
    for r in 0..rows {
        for c in 0..cols {
            let cell = egui::Rect::from_min_size(
                area.min + Vec2::new(c as f32 * cw, r as f32 * ch),
                Vec2::new(cw, ch),
            );
            p.rect_stroke(
                cell,
                0.,
                egui::Stroke::new(0.5_f32, Color32::from_gray(220)),
                egui::StrokeKind::Inside,
            );
            p.text(
                cell.min + Vec2::new(3., 2.),
                egui::Align2::LEFT_TOP,
                format!("{}", r * cols + c + 1),
                font(8.),
                SOFT,
            );
            cells.push(cell);
        }
    }
    cells
}

fn cursor(p: &egui::Painter, at: egui::Pos2) {
    let pts = vec![
        at,
        at + Vec2::new(0., 14.),
        at + Vec2::new(4., 10.),
        at + Vec2::new(9., 10.),
    ];
    p.add(egui::Shape::convex_polygon(
        pts,
        Color32::WHITE,
        egui::Stroke::new(1.2_f32, INK),
    ));
}

fn drag_demo(p: &egui::Painter, rect: egui::Rect, shift: bool, t: f64, label: &str) {
    let cal = egui::Rect::from_min_size(
        rect.min + Vec2::new(150., 18.),
        Vec2::new(rect.width() - 166., 100.),
    );
    let cells = mini_calendar(p, cal);
    let target = cells[10];
    let start = rect.min + Vec2::new(16., 22.);
    let k = ease((t - 0.8) / 1.8);
    let size = Vec2::new(90., 54.) * (1. - 0.45 * k);
    let pos = start + (target.center() - size / 2. - start) * k;
    if t <= 2.7 {
        let r = egui::Rect::from_min_size(pos, size);
        p.rect_filled(r, 6., PAPER);
        // The calendar icon in the sticker's top bar is the handle.
        let icon = egui::Rect::from_min_size(r.right_top() + Vec2::new(-16., 3.), Vec2::splat(11.));
        p.rect_stroke(
            icon,
            2.,
            egui::Stroke::new(1.2_f32, INK),
            egui::StrokeKind::Inside,
        );
        p.line_segment(
            [
                icon.left_top() + Vec2::new(0., 3.),
                icon.right_top() + Vec2::new(0., 3.),
            ],
            egui::Stroke::new(1.2_f32, INK),
        );
        p.text(
            r.min + Vec2::new(5., 16.),
            egui::Align2::LEFT_TOP,
            label,
            font(10.),
            INK,
        );
        if t < 0.8 {
            p.circle_stroke(
                icon.center(),
                9.,
                egui::Stroke::new(2_f32, crate::ui::ACCENT),
            );
        }
        cursor(p, icon.center());
    } else {
        let bar = egui::Rect::from_min_size(
            target.min + Vec2::new(1., 11.),
            Vec2::new(target.width() - 2., 10.),
        );
        p.rect_filled(
            bar,
            2.,
            if shift {
                Color32::from_rgb(230, 90, 70)
            } else {
                Color32::from_rgb(120, 160, 230)
            },
        );
        p.text(
            bar.left_center() + Vec2::new(2., 0.),
            egui::Align2::LEFT_CENTER,
            if shift { "🚩" } else { "회의" },
            font(8.),
            Color32::WHITE,
        );
        p.text(
            egui::pos2(rect.min.x + 16., rect.max.y - 18.),
            egui::Align2::LEFT_CENTER,
            if shift {
                "이날까지 업무 · D-day"
            } else {
                "그날 일정이 됐어요"
            },
            font(12.),
            crate::ui::ACCENT,
        );
    }
    if shift && (0.3..2.8).contains(&t) {
        let key = egui::Rect::from_min_size(
            rect.min + Vec2::new(16., rect.height() - 32.),
            Vec2::new(54., 22.),
        );
        p.rect_filled(key, 4., INK);
        p.text(
            key.center(),
            egui::Align2::CENTER_CENTER,
            "Shift",
            font(11.),
            Color32::WHITE,
        );
    }
}

/// A small window title over the demo's calendar (as in `drag_demo`).
fn window_caption(p: &egui::Painter, rect: egui::Rect, s: &str) {
    p.text(
        rect.min + Vec2::new(150., 4.),
        egui::Align2::LEFT_TOP,
        s,
        font(9.),
        SOFT,
    );
}

/// Pressing a day and dragging to another: the days light up, then a band.
fn range_demo(p: &egui::Painter, rect: egui::Rect, t: f64) {
    let cal = egui::Rect::from_min_size(
        rect.min + Vec2::new(16., 18.),
        Vec2::new(rect.width() - 32., 84.),
    );
    let cells = mini_calendar(p, cal);
    let (from, to) = (8, 10);
    let k = ease((t - 0.8) / 1.4);
    let reach = from + ((to - from) as f32 * k).round() as usize;
    if t < 2.5 {
        if t > 0.6 {
            for c in &cells[from..=reach] {
                p.rect_filled(c.shrink(1.), 3., crate::ui::ACCENT.gamma_multiply(0.22));
            }
        }
        let at = cells[from].center().lerp(cells[to].center(), k);
        cursor(p, at);
    } else {
        let band = egui::Rect::from_min_max(
            cells[from].min + Vec2::new(1., 11.),
            egui::pos2(cells[to].max.x - 1., cells[from].min.y + 21.),
        );
        p.rect_filled(band, 2., Color32::from_rgb(120, 160, 230));
        p.text(
            band.left_center() + Vec2::new(3., 0.),
            egui::Align2::LEFT_CENTER,
            "새 일정",
            font(8.),
            Color32::WHITE,
        );
    }
    p.text(
        egui::pos2(rect.min.x + 16., rect.max.y - 16.),
        egui::Align2::LEFT_CENTER,
        if t < 2.5 {
            "날짜를 누른 채 끌어요"
        } else {
            "9일 ~ 11일 일정이 생겨요"
        },
        font(12.),
        crate::ui::ACCENT,
    );
}

/// Ctrl+C on some text, the copy joins the top of the list; one click on
/// it copies it again.
fn clipboard_demo(p: &egui::Painter, rect: egui::Rect, t: f64) {
    let src = egui::Rect::from_min_size(rect.min + Vec2::new(16., 22.), Vec2::new(120., 50.));
    p.rect_filled(src, 6., Color32::WHITE);
    let word = text(p, src.min + Vec2::new(8., 8.), "회의실 B동 3층", 11., INK);
    if t > 0.3 {
        p.rect_filled(word.expand(1.), 2., crate::ui::ACCENT.gamma_multiply(0.25));
    }
    if (0.5..1.6).contains(&t) {
        let key =
            egui::Rect::from_min_size(src.left_bottom() + Vec2::new(0., 10.), Vec2::new(54., 22.));
        p.rect_filled(key, 4., INK);
        p.text(
            key.center(),
            egui::Align2::CENTER_CENTER,
            "Ctrl+C",
            font(11.),
            Color32::WHITE,
        );
    }
    let list = egui::Rect::from_min_max(
        egui::pos2(src.right() + 16., rect.top() + 18.),
        egui::pos2(rect.right() - 16., rect.bottom() - 14.),
    );
    p.rect_filled(list, 6., Color32::WHITE);
    let row_h = 22.;
    let k = ease((t - 1.4) / 0.5);
    let new = t > 1.4;
    let shift = if new { row_h * k } else { 0. };
    for (r, s) in [
        "https://example.com/plan",
        "3분기 매출 표",
        "주간 보고 초안",
    ]
    .iter()
    .enumerate()
    {
        let at = list.min + Vec2::new(8., 6. + shift + r as f32 * row_h);
        // Clear of the line under the list.
        if at.y + 14. < list.bottom() - 18. {
            text(p, at, s, 10., SOFT);
        }
    }
    if new {
        let row = egui::Rect::from_min_size(
            list.min + Vec2::new(4., 3.),
            Vec2::new(list.width() - 8., row_h - 2.),
        );
        let lit = t > 3.0;
        p.rect_filled(
            row,
            4.,
            crate::ui::ACCENT.gamma_multiply(if lit { 0.3 } else { 0.15 * k }),
        );
        text(
            p,
            row.min + Vec2::new(4., 3.),
            "회의실 B동 3층",
            10.,
            INK.gamma_multiply(k),
        );
        if t > 2.4 {
            cursor(p, row.center() + Vec2::new(30., 2.));
        }
        if lit {
            p.text(
                egui::pos2(list.min.x + 8., list.max.y - 12.),
                egui::Align2::LEFT_CENTER,
                "다시 복사됐어요 · Ctrl+V",
                font(11.),
                crate::ui::ACCENT,
            );
        }
    }
}

/// Alt+V by a memo, 클립보이 opens beside it, a click pastes at the cursor.
fn clipboy_demo(p: &egui::Painter, rect: egui::Rect, t: f64) {
    let memo = egui::Rect::from_min_size(rect.min + Vec2::new(14., 22.), Vec2::new(130., 74.));
    p.rect_filled(memo, 6., PAPER);
    text(p, memo.min + Vec2::new(8., 8.), "장보기 목록", 11., INK);
    let pasted = t > 3.2;
    let line = if pasted {
        "클립보이 연습 · 성공!"
    } else {
        ""
    };
    let typed = text(p, memo.min + Vec2::new(8., 30.), line, 10., INK);
    caret(p, typed, t);
    if (0.4..1.4).contains(&t) {
        let key =
            egui::Rect::from_min_size(memo.left_bottom() + Vec2::new(0., 8.), Vec2::new(46., 22.));
        p.rect_filled(key, 4., INK);
        p.text(
            key.center(),
            egui::Align2::CENTER_CENTER,
            "Alt+V",
            font(11.),
            Color32::WHITE,
        );
    }
    if !(1.2..3.4).contains(&t) {
        return;
    }
    let k = ease((t - 1.2) / 0.4);
    let panel = egui::Rect::from_min_max(
        egui::pos2(memo.right() + 14., rect.top() + 14.),
        egui::pos2(rect.right() - 14., rect.bottom() - 10.),
    );
    p.rect_filled(panel, 6., Color32::WHITE.gamma_multiply(k));
    text(
        p,
        panel.min + Vec2::new(8., 5.),
        "클립보이 (3/100)",
        9.,
        SOFT.gamma_multiply(k),
    );
    for (r, s) in [
        "클립보이 연습 · 붙여…",
        "https://example.com",
        "3분기 매출 표",
    ]
    .iter()
    .enumerate()
    {
        let row = egui::Rect::from_min_size(
            panel.min + Vec2::new(6., 22. + r as f32 * 24.),
            Vec2::new(panel.width() - 12., 22.),
        );
        if r == 0 && t > 2.4 {
            p.rect_filled(row, 4., crate::ui::ACCENT.gamma_multiply(0.25 * k));
        }
        p.rect_filled(
            egui::Rect::from_min_size(row.min + Vec2::new(2., 3.), Vec2::splat(16.)),
            3.,
            Color32::from_gray(230).gamma_multiply(k),
        );
        text(
            p,
            row.min + Vec2::new(24., 4.),
            s,
            10.,
            INK.gamma_multiply(k),
        );
    }
    if t > 1.9 {
        cursor(p, panel.min + Vec2::new(70., 36.));
    }
}

fn arrange_demo(p: &egui::Painter, rect: egui::Rect, t: f64) {
    let screen = rect.shrink2(Vec2::new(14., 10.));
    p.rect_filled(screen, 6., Color32::from_rgb(210, 225, 240));
    let starts = [
        Vec2::new(20., 14.),
        Vec2::new(80., 44.),
        Vec2::new(40., 62.),
    ];
    let colors = [
        PAPER,
        Color32::from_rgb(255, 205, 210),
        Color32::from_rgb(200, 230, 200),
    ];
    let k = ease((t - 1.6) / 1.4);
    for (n, s) in starts.iter().enumerate() {
        let from = egui::Rect::from_min_size(screen.min + *s, Vec2::new(70., 44.));
        let to = egui::Rect::from_min_size(
            egui::pos2(screen.right() - 96., screen.top() + 8. + n as f32 * 20.),
            Vec2::new(90., 16.),
        );
        let r = egui::Rect::from_min_max(
            from.min + (to.min - from.min) * k,
            from.max + (to.max - from.max) * k,
        );
        p.rect_filled(r, 4., colors[n]);
        p.text(
            r.min + Vec2::new(4., 2.),
            egui::Align2::LEFT_TOP,
            ARRANGE_TITLES[n],
            font(9.),
            INK,
        );
    }
    if t < 1.6 {
        let menu =
            egui::Rect::from_min_size(screen.min + Vec2::new(150., 16.), Vec2::new(120., 52.));
        p.rect_filled(menu, 4., Color32::WHITE);
        for (k, s) in [
            "스티커 정렬 ›",
            "화면 오른쪽에 정렬 ›",
            "모두 제목만 보이게",
        ]
        .iter()
        .enumerate()
        {
            p.text(
                menu.min + Vec2::new(5., 3. + k as f32 * 16.),
                egui::Align2::LEFT_TOP,
                *s,
                font(9.),
                INK,
            );
        }
        cursor(p, menu.min + Vec2::new(60., 40.));
    }
}

fn detach_demo(p: &egui::Painter, rect: egui::Rect, t: f64) {
    let main = egui::Rect::from_min_size(rect.min + Vec2::new(14., 12.), Vec2::new(170., 108.));
    p.rect_filled(main, 6., Color32::WHITE);
    p.rect_filled(
        egui::Rect::from_min_size(main.min, Vec2::new(40., main.height())),
        6.,
        Color32::from_gray(235),
    );
    mini_calendar(
        p,
        egui::Rect::from_min_size(main.min + Vec2::new(48., 24.), Vec2::new(116., 78.)),
    );
    let icon =
        egui::Rect::from_center_size(main.right_top() + Vec2::new(-12., 11.), Vec2::splat(14.));
    p.rect_stroke(
        icon,
        3.,
        egui::Stroke::new(1.5_f32, INK),
        egui::StrokeKind::Inside,
    );
    if t < 1.4 {
        cursor(p, icon.center());
        p.circle_stroke(
            icon.center(),
            10. + (t as f32 * 6.) % 6.,
            egui::Stroke::new(2_f32, crate::ui::ACCENT),
        );
    }
    let k = ease((t - 1.4) / 1.0);
    if k > 0. {
        let to = egui::Rect::from_min_size(
            rect.min + Vec2::new(200., 14.),
            Vec2::new(rect.width() - 214., 104.),
        );
        let from = egui::Rect::from_center_size(icon.center(), Vec2::splat(10.));
        let r = egui::Rect::from_min_max(
            from.min + (to.min - from.min) * k,
            from.max + (to.max - from.max) * k,
        );
        p.rect_filled(r, 6., Color32::from_rgb(250, 247, 240));
        if k > 0.95 {
            mini_calendar(p, r.shrink(6.));
        }
    }
}

fn stand_demo(p: &egui::Painter, rect: egui::Rect, t: f64) {
    let desk = rect.shrink2(Vec2::new(14., 10.));
    p.rect_filled(desk, 6., Color32::from_rgb(70, 110, 160));
    let cal = egui::Rect::from_min_size(desk.min + Vec2::new(90., 12.), Vec2::new(150., 90.));
    let pinned = t > 1.8;
    if !pinned {
        // The window with its title bar; a click on the stand button.
        p.rect_filled(cal, 4., Color32::WHITE);
        p.rect_filled(
            egui::Rect::from_min_size(cal.min, Vec2::new(cal.width(), 12.)),
            4.,
            Color32::from_gray(220),
        );
        mini_calendar(p, cal.shrink(8.).translate(Vec2::new(0., 6.)));
        let button = cal.right_top() + Vec2::new(-30., 20.);
        cursor(p, button);
        p.circle_stroke(button, 8., egui::Stroke::new(2_f32, crate::ui::ACCENT));
    } else {
        mini_calendar(p, cal);
    }
    // Desktop icons on the left and, once it stands, one over the calendar.
    let icon = |at: egui::Pos2| {
        p.rect_filled(
            egui::Rect::from_min_size(at, Vec2::splat(16.)),
            3.,
            Color32::from_rgb(250, 210, 90),
        );
    };
    for k in 0..3 {
        icon(desk.min + Vec2::new(14., 10. + k as f32 * 30.));
    }
    if pinned {
        icon(cal.min + Vec2::new(20., 30.));
        p.text(
            egui::pos2(desk.left() + 40., desk.bottom() - 8.),
            egui::Align2::LEFT_BOTTOM,
            "바탕화면에 고정!",
            font(11.),
            Color32::WHITE,
        );
    }
}

/// (a list item one level below another item has content, a list item came
/// after it at a higher level again) in `body`.
fn list_levels(body: &str) -> (bool, bool) {
    let mut seen: Vec<usize> = vec![];
    let (mut nested, mut up) = (false, false);
    let mut deepest_nested: Option<usize> = None;
    for line in body.lines() {
        let indent = line.chars().take_while(|c| *c == ' ').count();
        let rest = line.trim_start();
        let Some((token, text)) = rest.split_once(' ') else {
            continue;
        };
        let marker = matches!(token, "-" | "*")
            || ((token.ends_with('.') || token.ends_with(')'))
                && token.chars().count() <= 5
                && token[..token.len() - 1]
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric()));
        if !marker {
            continue;
        }
        if let Some(d) = deepest_nested {
            if indent < d {
                up = true;
            }
        }
        if seen.iter().any(|&s| s < indent) && !text.trim().is_empty() {
            nested = true;
            deepest_nested = Some(deepest_nested.map_or(indent, |d| d.max(indent)));
        }
        seen.push(indent);
    }
    (nested, up)
}

fn filter_demo(p: &egui::Painter, rect: egui::Rect, t: f64) {
    let area = rect.shrink2(Vec2::new(14., 10.));
    p.rect_filled(area, 6., Color32::WHITE);
    let picked = t > 2.0;
    let combo = egui::Rect::from_min_size(area.min + Vec2::new(10., 8.), Vec2::new(120., 20.));
    p.rect_filled(combo, 4., Color32::from_gray(238));
    p.text(
        combo.left_center() + Vec2::new(6., 0.),
        egui::Align2::LEFT_CENTER,
        if picked {
            "# 튜토리얼  ▾"
        } else {
            "# 모든 프로젝트  ▾"
        },
        font(10.),
        INK,
    );
    let rows: &[(&str, bool)] = &[
        ("완료 연습 · 기획서 검토", true),
        ("장보기", false),
        ("드래그 연습 · 회의 준비", true),
        ("치과 예약", false),
    ];
    let mut y = combo.bottom() + 8.;
    for (title, mine) in rows {
        if picked && !mine {
            continue;
        }
        p.circle_stroke(
            egui::pos2(area.left() + 18., y + 8.),
            5.,
            egui::Stroke::new(1_f32, SOFT),
        );
        p.text(
            egui::pos2(area.left() + 30., y + 8.),
            egui::Align2::LEFT_CENTER,
            *title,
            font(10.),
            INK,
        );
        y += 18.;
    }
    if (0.6..2.0).contains(&t) {
        let menu = egui::Rect::from_min_size(
            combo.left_bottom() + Vec2::new(0., 2.),
            Vec2::new(120., 58.),
        );
        p.rect_filled(menu, 4., Color32::from_gray(250));
        p.rect_stroke(
            menu,
            4.,
            egui::Stroke::new(1_f32, Color32::from_gray(210)),
            egui::StrokeKind::Inside,
        );
        for (k, s) in ["모든 프로젝트", "개인 업무", "튜토리얼"]
            .iter()
            .enumerate()
        {
            let r = egui::Rect::from_min_size(
                menu.min + Vec2::new(2., 2. + k as f32 * 18.),
                Vec2::new(116., 17.),
            );
            if k == 2 && t > 1.3 {
                p.rect_filled(r, 3., crate::ui::ACCENT.gamma_multiply(0.25));
            }
            p.text(
                r.left_center() + Vec2::new(4., 0.),
                egui::Align2::LEFT_CENTER,
                *s,
                font(10.),
                INK,
            );
        }
        cursor(p, menu.min + Vec2::new(50., 46.));
    }
    let side = egui::pos2(area.right() - 120., area.top() + 12.);
    p.text(
        side,
        egui::Align2::LEFT_TOP,
        "왼쪽 목록에서도",
        font(10.),
        SOFT,
    );
    p.text(
        side + Vec2::new(0., 16.),
        egui::Align2::LEFT_TOP,
        "# 튜토리얼",
        font(11.),
        if picked { crate::ui::ACCENT } else { INK },
    );
}

fn complete_demo(p: &egui::Painter, rect: egui::Rect, t: f64) {
    let area = rect.shrink2(Vec2::new(14., 10.));
    p.rect_filled(area, 6., Color32::WHITE);
    let done = t > 1.4;
    let rows = ["완료 연습 · 기획서 검토", "드래그 연습 · 회의 준비"];
    for (k, title) in rows.iter().enumerate() {
        let y = area.top() + 20. + k as f32 * 30.;
        let c = egui::pos2(area.left() + 22., y);
        if k == 0 && done {
            p.circle_filled(c, 7., crate::ui::ACCENT);
            // A check mark drawn in the circle.
            p.line_segment(
                [c + Vec2::new(-3., 0.), c + Vec2::new(-1., 3.)],
                egui::Stroke::new(1.6_f32, Color32::WHITE),
            );
            p.line_segment(
                [c + Vec2::new(-1., 3.), c + Vec2::new(4., -3.)],
                egui::Stroke::new(1.6_f32, Color32::WHITE),
            );
            let r = text(p, egui::pos2(c.x + 14., y - 7.), title, 11., SOFT);
            p.line_segment(
                [r.left_center(), r.right_center()],
                egui::Stroke::new(1_f32, SOFT),
            );
        } else {
            p.circle_stroke(c, 7., egui::Stroke::new(1.2_f32, SOFT));
            text(p, egui::pos2(c.x + 14., y - 7.), title, 11., INK);
        }
        if k == 0 && t < 1.6 {
            cursor(p, c + Vec2::new(2., 2.));
        }
    }
    if done {
        p.text(
            egui::pos2(area.left() + 14., area.bottom() - 12.),
            egui::Align2::LEFT_CENTER,
            "완료! 업무일지로 가요 →",
            font(12.),
            crate::ui::ACCENT,
        );
    }
}

fn journal_demo(p: &egui::Painter, rect: egui::Rect, t: f64) {
    let area = rect.shrink2(Vec2::new(14., 10.));
    p.rect_filled(area, 6., Color32::WHITE);
    let side = egui::Rect::from_min_size(area.min, Vec2::new(86., area.height()));
    p.rect_filled(side, 6., Color32::from_gray(238));
    for (k, s) in ["내 작업실", "캘린더", "편의기능"].iter().enumerate() {
        let r = egui::Rect::from_min_size(
            side.min + Vec2::new(4., 8. + k as f32 * 22.),
            Vec2::new(78., 18.),
        );
        if k == 2 && t > 1.0 {
            p.rect_filled(r, 4., crate::ui::ACCENT.gamma_multiply(0.25));
        }
        p.text(
            r.left_center() + Vec2::new(6., 0.),
            egui::Align2::LEFT_CENTER,
            *s,
            font(10.),
            INK,
        );
    }
    if t < 1.2 {
        cursor(p, side.min + Vec2::new(40., 58.));
    } else {
        let at = egui::pos2(side.right() + 12., area.top() + 10.);
        p.text(
            at,
            egui::Align2::LEFT_TOP,
            "업무일지 · 오늘",
            font(11.),
            INK,
        );
        let k = ease((t - 1.3) / 0.6);
        let row = egui::Rect::from_min_size(at + Vec2::new(0., 22.), Vec2::new(200., 22.));
        p.rect_filled(row, 4., crate::ui::ACCENT.gamma_multiply(0.15 * k));
        p.text(
            row.left_center() + Vec2::new(6., 0.),
            egui::Align2::LEFT_CENTER,
            "☑ 완료 연습 · 기획서 검토",
            font(11.),
            INK.gamma_multiply(k),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_tip_goes_by_one_monitor_at_its_scale() {
        // Whatever this PC has: a real monitor with a sane scale.
        if let Some((r, scale)) = monitor_near(&[[10, 10, 50, 50]]) {
            assert!(r[2] > r[0] && r[3] > r[1]);
            assert!((0.5..=5.).contains(&scale));
            let (x, y) = tip_position(r, &[[r[0] + 10, r[1] + 10, r[0] + 60, r[1] + 60]], 400, 300);
            assert!(x >= r[0] && x + 400 <= r[2] && y >= r[1] && y + 300 <= r[3]);
        }
        assert!(monitor_near(&[]).is_none_or(|(r, s)| r[2] > r[0] && s > 0.));
    }
    fn app() -> App {
        let mut a = App::sample(Local::now().date_naive());
        a.start_fresh();
        a
    }
    #[test]
    fn every_step_says_what_to_do_and_lights_something() {
        let mut a = app();
        a.tutorial_start();
        for (n, s) in STEPS.iter().enumerate() {
            assert!(!s.title.is_empty() && !s.text.is_empty());
            if n + 1 < STEPS.len() {
                assert!(!s.todo.is_empty());
                assert!(!a.step_spots(n).is_empty(), "step {n} lights nothing");
            }
        }
        assert!(a.role(Role::Practice).is_some());
        assert!(a.role(Role::Arrange(2)).is_some());
        assert!(a.role(Role::DockDrag).is_some());
        assert!(a.role(Role::DockDeadline).is_some());
    }
    #[test]
    fn an_example_sticker_leaves_its_saved_position_when_the_tour_reopens_it() {
        let mut a = app();
        a.tutorial_start();
        a.desktop.new_right = true;
        a.prefs.sticker_reopen_position = 2;
        let note = a.role(Role::Practice).unwrap();
        a.studio.open(note, None);
        let w = a.studio.windows.get_mut(&note).unwrap();
        w.last_pos = Some(egui::pos2(24., 36.));
        w.open = false;
        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |ctx| {
            a.open_role(ctx, Role::Practice, true)
        });
        let w = &a.studio.windows[&note];
        assert!(w.open && w.mode == crate::studio::Mode::Sticker);
        assert!(
            w.place.is_none(),
            "the old position cannot override the tutorial center"
        );
    }

    #[test]
    fn a_tour_from_an_older_version_gets_the_calendar_window_memos_once() {
        let mut a = app();
        a.tutorial_start();
        // As 2.0 left it: the first nine memos only.
        a.extra.tutorial_notes.truncate(OLD_NOTES);
        assert!(a.role(Role::DockDrag).is_none());
        let before = a.data.notes.len();
        a.tutorial_start();
        assert_eq!(a.data.notes.len(), before + 2);
        let d = a.role(Role::DockDrag).unwrap();
        assert_eq!(a.data.notes[d].title, DOCK_TITLE);
        let d = a.role(Role::DockDeadline).unwrap();
        assert_eq!(a.data.notes[d].title, DOCK_DEADLINE_TITLE);
        // Already there: nothing more.
        a.tutorial_start();
        a.tutorial_install_dock();
        assert_eq!(a.data.notes.len(), before + 2);
        assert_eq!(a.extra.tutorial_notes.len(), OLD_NOTES + 2);
    }
    #[test]
    fn calendar_window_steps_notice_drops_and_a_dragged_stretch() {
        let mut a = app();
        a.tutorial_start();
        let at = |k: K| STEPS.iter().position(|s| s.key == k).unwrap();
        // In order: after the window opens, before it stands.
        assert!(at(K::Detach) < at(K::DockDrag));
        assert!(at(K::DockDrag) < at(K::DockShift));
        assert!(at(K::DockShift) < at(K::Range));
        assert!(at(K::Range) < at(K::Stand));
        assert!(at(K::Journal) < at(K::Clipboard));
        assert!(at(K::Clipboard) < at(K::Clipboy));
        assert!(at(K::Clipboy) < at(K::Detach));
        let today = Local::now().date_naive();
        a.tutorial_live.notes_at = a.data.notes.len();
        assert!(!a.step_done(at(K::DockDrag)));
        assert!(!a.step_done(at(K::DockShift)));
        // The example trip of several days does not count.
        assert!(!a.step_done(at(K::Range)));
        let d = a.role(Role::DockDrag).unwrap();
        a.assign_dropped_date(d, today, false, true, None);
        assert!(a.step_done(at(K::DockDrag)));
        // A plain drop is not a deadline.
        let d = a.role(Role::DockDeadline).unwrap();
        a.assign_dropped_date(d, today, false, true, None);
        assert!(!a.step_done(at(K::DockShift)));
        a.assign_dropped_date(d, today, true, true, None);
        assert!(a.step_done(at(K::DockShift)));
        // A new one-day memo is not a stretch either.
        let one = a.new_sticker_on(today);
        assert!(!a.step_done(at(K::Range)));
        assert!(a.data.notes[one].project.is_none());
        // What letting go after dragging over three days makes.
        let i = a.new_sticker_on(today);
        a.data.notes[i].duration = 3;
        assert!(a.step_done(at(K::Range)));
        a.file_range_notes(at(K::Range));
        assert_eq!(a.data.notes[i].project, a.tutorial_project());
        assert!(a.tutorial_notes().contains(&i));
        // Opening the clipboard page; it waits for "다음".
        a.page = 7;
        assert!(a.step_done(at(K::Clipboard)));
        a.settings = true;
        assert!(!a.step_done(at(K::Clipboard)));
        assert!(!STEPS[at(K::Clipboard)].auto());
        // Pasted from 클립보이 into the practice memo: done by itself.
        assert!(!a.step_done(at(K::Clipboy)));
        let p = a.role(Role::Practice).unwrap();
        a.data.notes[p].body.push_str(CLIPBOY_SAMPLE);
        assert!(a.step_done(at(K::Clipboy)));
        assert!(STEPS[at(K::Clipboy)].auto());
        assert!(STEPS[at(K::Range)].auto());
    }
    #[test]
    fn steps_notice_what_the_user_did() {
        let mut a = app();
        a.tutorial_start();
        let p = a.role(Role::Practice).unwrap();
        let at = |k: K| STEPS.iter().position(|s| s.key == k).unwrap();
        assert!(!(0..STEPS.len()).any(|n| a.step_done(n)));
        // #튜토리얼 typed at the start of the body.
        a.data.notes[p].body = "#튜토리얼 ".into();
        a.consume_body_project(p);
        assert!(a.step_done(at(K::Project)));
        a.data.notes[p].body = "장보기 목록\n".into();
        let n = &mut a.data.notes[p];
        crate::desktop::infer_title(&mut n.title, &n.body);
        assert!(a.step_done(at(K::Title)));
        // One level only is not yet a multi-level list.
        a.data.notes[p].body = "장보기 목록\n  1. 우유\n  2. 계란".into();
        assert!(!a.step_done(at(K::MultiList)));
        a.data.notes[p].body = "장보기 목록\n  1. 우유\n    a) 저지방".into();
        assert!(a.step_done(at(K::MultiList)));
        assert!(!a.step_done(at(K::ListUp)));
        a.data.notes[p].body = "장보기 목록\n  1. 우유\n    a) 저지방\n  2. ".into();
        assert!(a.step_done(at(K::ListUp)));
        a.data.notes[p].document.tabs[0]
            .spans
            .push(crate::composer::Span {
                range: 12..14,
                format: crate::composer::Format {
                    strike: true,
                    ..Default::default()
                },
            });
        assert!(a.step_done(at(K::Strike)));
        a.data.notes[p].body.push_str("\n☑ 빵");
        assert!(a.step_done(at(K::Check)));
        let today = Local::now().date_naive();
        let d = a.role(Role::Drag).unwrap();
        a.assign_dropped_date(d, today, false, true, None);
        assert!(a.step_done(at(K::Drag)));
        let d = a.role(Role::Deadline).unwrap();
        a.assign_dropped_date(d, today, true, true, None);
        assert!(a.step_done(at(K::ShiftDrag)));
        a.project_filter = a.tutorial_project();
        assert!(a.step_done(at(K::Filter)));
        let c = a.role(Role::Complete).unwrap();
        assert_eq!(a.data.notes[c].project, a.tutorial_project());
        a.done_at(c, Some(today), true);
        assert!(a.step_done(at(K::Complete)));
        a.page = 6;
        a.extra.utilities_tab = 0;
        assert!(a.step_done(at(K::Journal)));
        assert!(!STEPS[at(K::Journal)].auto());
    }
    #[test]
    fn erasing_removes_the_examples_and_what_was_filed_under_the_project_only() {
        let mut a = app();
        let before: Vec<bool> = a.data.notes.iter().map(|n| n.purged).collect();
        let mut mine = Note::new("내 메모", None, 0);
        mine.body = "남아야 해요".into();
        a.data.notes.push(mine);
        a.tutorial_start();
        let p = a.tutorial_project().unwrap();
        let mut tagged = Note::new("연습으로 쓴 메모", None, 1);
        tagged.project = Some(p);
        a.data.notes.push(tagged);
        a.tutorial_erase();
        assert!(a.tutorial_notes().is_empty());
        assert!(a.tutorial_project().is_none());
        let live: Vec<&str> = a
            .data
            .notes
            .iter()
            .filter(|n| !n.purged)
            .map(|n| n.title.as_str())
            .collect();
        assert!(live.contains(&"내 메모"));
        assert!(!live
            .iter()
            .any(|t| t.contains("연습") || t.contains("예시")));
        assert_eq!(
            a.data
                .notes
                .iter()
                .take(before.len())
                .map(|n| n.purged)
                .collect::<Vec<_>>(),
            before
        );
        a.tutorial_start();
        assert!(a.role(Role::Practice).is_some());
    }
    #[test]
    fn the_tip_sits_beside_what_it_explains_and_stays_on_screen() {
        let screen = [0, 0, 2000, 1200];
        let (x, _) = tip_position(screen, &[[100, 100, 500, 500]], 400, 470);
        assert_eq!(x, 524);
        // No room on the right: on the left.
        let (x, _) = tip_position(screen, &[[1500, 100, 1990, 500]], 400, 470);
        assert_eq!(x, 1500 - 24 - 400);
        // Everything lit: still fully on screen.
        let (x, y) = tip_position(screen, &[[0, 0, 2000, 1200]], 400, 470);
        assert!(x >= 0 && y >= 0 && x + 400 <= 2000 && y + 470 <= 1200);
    }
    #[test]
    fn spots_are_only_kept_while_the_tutorial_runs() {
        let ctx = egui::Context::default();
        begin_frame(false);
        mark(
            &ctx,
            Spot::Grid,
            egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::splat(10.)),
        );
        assert!(marks().is_empty());
    }
    #[test]
    fn the_last_step_and_auto_advance_lead_to_the_closing_choice() {
        assert_eq!(next_state(STEP0), STEP0 + 1);
        assert_eq!(next_state(STEP0 + STEPS.len() as u32 - 1), FINISH);
    }
    #[test]
    fn tutorial_state_and_example_memos_survive_saving() {
        let dir = std::env::temp_dir().join(format!("ohmemojang-tut-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut a = app();
        a.tutorial_start();
        a.extra.tutorial = STEP0 + 3;
        a.extra.deleted_wiki_tabs.insert(0);
        let snap = a.snapshot(&dir);
        crate::storage::write_all(&dir, &snap).unwrap();
        let loaded = crate::storage::read(&dir).unwrap().unwrap();
        let mut b = App::sample(Local::now().date_naive());
        b.restore(&loaded);
        assert_eq!(b.extra.tutorial, STEP0 + 3);
        assert_eq!(b.extra.tutorial_notes, a.extra.tutorial_notes);
        assert_eq!(b.extra.guide_version, crate::guide::GUIDE_VERSION);
        assert!(b.extra.deleted_wiki_tabs.contains(&0));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
