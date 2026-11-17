//! The first-run tutorial: example memos and schedules, and a small window
//! that walks through the program one topic at a time. It can be skipped at
//! any moment. When it ends (or is skipped) the user chooses whether the
//! example data is erased.
//!
//! `Extra::tutorial` keeps the state across restarts:
//! 0 off, `WELCOME`, `STEP0 + n` while on step n, `FINISH` at the choice.
//!
//! Everything the tutorial adds belongs to the project `튜토리얼` or to the
//! wiki tab `연습 위키`; that is how it is found again to be erased. A memo
//! the user starts with `#튜토리얼 ` lands in that project and goes with it.
use crate::*;

pub const WELCOME: u32 = 1;
pub const STEP0: u32 = 10;
pub const FINISH: u32 = 100;

const PROJECT: &str = "튜토리얼";
const WIKI_TAB: &str = "연습 위키";

const M_TOUR: &str = "튜토리얼 ① 메모장 구경하기";
const M_TEXT: &str = "튜토리얼 ② 본문 연습장";
const M_PLAN: &str = "튜토리얼 ③ 일정·알람·잠금 연습";
const M_S1: &str = "튜토리얼 ④ 스티커 연습 1";
const M_S2: &str = "튜토리얼 ④ 스티커 연습 2";
const M_S3: &str = "튜토리얼 ④ 스티커 연습 3";
const M_WIKI: &str = "튜토리얼 위키 · 분류 안 한 메모";

#[derive(Clone, Copy, PartialEq)]
enum Act {
    Open(&'static [&'static str]),
    Page(usize),
    Calendar,
    Utilities(usize),
    Wiki,
    Settings,
}

struct Step {
    title: &'static str,
    lines: &'static [&'static str],
    acts: &'static [(&'static str, Act)],
}

const STEPS: &[Step] = &[
    Step {
        title: "메모장 한눈에 보기",
        lines: &[
            "스티커 메모 하나가 바탕화면 위의 작은 메모장이에요. 예시 메모를 열어서 같이 볼까요?",
            "제목: 맨 위 한 줄이에요. 비워 두면 본문 첫 줄이 제목이 돼요.",
            "본문: 자유롭게 쓰는 곳이에요. 아래 탭의 +로 본문을 여러 장 만들 수 있어요.",
            "툴바: 위쪽엔 색·더보기(…)·알람·일정·완료·접기·닫기, 아래쪽엔 글자 서식과 투명도가 있어요.",
            "완료 체크박스: 제목 줄 오른쪽 동그라미예요. 눌러서 완료해 보세요. 완료한 일은 업무일지에 모여요.",
        ],
        acts: &[("예시 메모 열기", Act::Open(&[M_TOUR]))],
    },
    Step {
        title: "본문 쓰기 요령",
        lines: &[
            "연습장을 열고 한 가지씩 해 보세요.",
            "자동 들여쓰기: ‘- ’ 나 ‘1. ’ 을 쓰고 Enter를 누르면 다음 항목이 이어져요. Tab은 들여쓰기, Shift+Tab은 내어쓰기!",
            "취소선: 글자를 드래그로 고르고 아래 툴바의 취소선 버튼을 눌러요.",
            "자동 제목: 새 메모(Ctrl+Alt+N)에 제목 없이 본문만 써 보세요. 첫 줄이 제목이 돼요.",
            "자동 프로젝트: 새 메모 맨 앞에 ‘#튜토리얼 ’ (스페이스까지)을 쓰면 튜토리얼 프로젝트로 분류돼요. 나중에 예시와 함께 지울 수 있어요.",
        ],
        acts: &[("연습장 열기", Act::Open(&[M_TEXT]))],
    },
    Step {
        title: "일정 · 알람 · 잠금",
        lines: &[
            "연습 메모를 열고 위쪽 툴바를 써 보세요.",
            "일정: 일정 아이콘 → 시작·종료 날짜와 시간, ‘이날까지 업무’, 반복을 정해요.",
            "알람: 알람 아이콘 → 시각을 5분 뒤로 맞춰 보세요. 오른쪽 아래에 알림이 떠요.",
            "잠금: … 더보기 → 잠금. 잠긴 메모는 제목만 보이고 비밀번호를 넣어야 열려요. 연습할 땐 쉬운 비밀번호를 쓰세요.",
        ],
        acts: &[("연습 메모 열기", Act::Open(&[M_PLAN]))],
    },
    Step {
        title: "스티커 접기 · 자동 정렬",
        lines: &[
            "스티커 세 개를 펼쳐 둘게요.",
            "접기 버튼을 누를 때마다 스티커 → 제목 한 줄 → 색상 상자로 작아져요. 색상 상자는 누르면 펼쳐지고 끌면 옮겨져요.",
            "자동 정렬: 스티커를 우클릭 → 스티커 정렬 → 화면 오른쪽에 정렬, 화면 왼쪽에 정렬, 계단식으로 정렬.",
            "오른쪽·왼쪽 정렬에서는 ‘모두 제목만 보이게’, ‘모두 상자로 접기’도 고를 수 있어요.",
        ],
        acts: &[("스티커 3개 펼치기", Act::Open(&[M_S1, M_S2, M_S3]))],
    },
    Step {
        title: "캘린더 영역",
        lines: &[
            "캘린더로 이동해서 예시 일정을 구경해 보세요.",
            "여러 날 이어지는 일정은 색띠 하나로 보이고, 띠를 누르면 그날의 메모가 열려요.",
            "날짜 위에 마우스를 올리면 + 버튼이 나와요. 그날의 새 스티커를 만들어요.",
            "위쪽 버튼으로 작성일·일정·알람 기준을 바꿔 보세요.",
        ],
        acts: &[("캘린더 열기", Act::Page(5))],
    },
    Step {
        title: "캘린더 분리창 · 스탠딩 캘린더",
        lines: &[
            "분리창: 캘린더 오른쪽 위의 분리 아이콘이나 Ctrl+Alt+C로 열어요. 우클릭으로 한 달·1주·2주 보기를 바꿔요.",
            "최소화 버튼을 누르면 작은 색상 상자로 접혀요. 창의 X는 분리창을 닫아요.",
            "스탠딩 캘린더: 받침대 모양 아이콘을 누르면 바탕화면에 고정돼요. 같은 아이콘을 다시 누르면 풀려요.",
            "⚠ Windows 11 24H2에서는 스탠딩 캘린더가 동작하지 않아요. 그땐 분리창이나 색상 상자를 쓰세요.",
        ],
        acts: &[("분리창 열기", Act::Calendar)],
    },
    Step {
        title: "캘린더로 끌어다 놓기",
        lines: &[
            "스티커를 달력의 날짜로 끌어다 놓아 보세요. 연습 스티커를 펼쳐 둘게요.",
            "그냥 놓으면 그 날짜가 일정 시작일이 돼요.",
            "Shift를 누른 채 놓으면 그 날짜가 마감인 ‘이날까지 업무’가 돼요. 연습 스티커 2로 해 보세요!",
        ],
        acts: &[("연습 스티커 펼치고 캘린더 열기", Act::Open(&[M_S1, M_S2]))],
    },
    Step {
        title: "편의기능 · 업무일지와 휴지통",
        lines: &[
            "업무일지: 완료한 일을 날짜·월별로 모아 보고 댓글을 남겨요. 아까 완료한 메모가 있는지 보세요.",
            "휴지통: 지운 메모를 복원하거나 영구 삭제해요. 예시 휴지통 메모를 복원해 보세요.",
            "표·이미지·링크 모아보기도 여기 있어요.",
        ],
        acts: &[
            ("업무일지 보기", Act::Utilities(0)),
            ("휴지통 보기", Act::Utilities(4)),
        ],
    },
    Step {
        title: "클립보드 히스토리",
        lines: &[
            "복사한 텍스트·링크·표·이미지를 모아 두었다가 한 번 클릭으로 다시 꺼내 써요.",
            "처음엔 꺼져 있어요. 켜려면 직접 ‘수집 시작’을 눌러야 해요. 튜토리얼은 켜지 않아요.",
            "비밀번호 같은 민감한 내용도 저장될 수 있어요(암호화 안 됨, 인터넷 전송 안 함). 그런 걸 복사하기 전엔 ‘일시 중지’!",
            "검색과 종류 필터, 개수·기간 정리 기준, 날짜 이전 삭제·전체 삭제를 쓸 수 있어요.",
        ],
        acts: &[("클립보드 히스토리 보기", Act::Page(7))],
    },
    Step {
        title: "나만의 위키",
        lines: &[
            "오래 남길 지식을 탭과 폴더로 정리해요. ‘연습 위키’ 탭을 열어 둘게요.",
            "메모 우클릭: 새 폴더, 새 위키 메모, 위키 해제, 메모 삭제.",
            "폴더 우클릭: 새 폴더, 새 하위 폴더, 새 위키 메모, 이름 바꾸기, 폴더 삭제, 폴더·메모 삭제.",
            "왼쪽 아래 ‘분류 안 한 메모’를 폴더로 끌어 보세요. 오른쪽 두 영역에는 메모를 나란히 열 수 있어요.",
        ],
        acts: &[("연습 위키 열기", Act::Wiki)],
    },
    Step {
        title: "더 알고 싶다면 옵션에서",
        lines: &[
            "여기까지 오셨다면 기본은 끝났어요! 🎉",
            "나머지는 설정에서 천천히 읽어 보세요. 항목마다 설명이 달려 있어요.",
            "일반 · 메인창/메모창 · 캘린더 · 알람 · 단축키 · 백업/복원이 있어요.",
            "사용법은 나만의 위키의 ‘사용법’ 탭에, 이 튜토리얼은 설정 › 정보에서 다시 볼 수 있어요.",
        ],
        acts: &[("설정 열기", Act::Settings)],
    },
];

impl App {
    /// Index of the live project/tab with that name.
    fn tutorial_project(&self) -> Option<usize> {
        self.data
            .projects
            .iter()
            .enumerate()
            .find(|(p, t)| t.as_str() == PROJECT && !self.extra.deleted_projects.contains(p))
            .map(|(p, _)| p)
    }
    fn tutorial_tab(&self) -> Option<usize> {
        self.extra
            .wiki_tabs
            .iter()
            .enumerate()
            .find(|(t, n)| n.as_str() == WIKI_TAB && !self.extra.deleted_wiki_tabs.contains(t))
            .map(|(t, _)| t)
    }
    /// Memos the tutorial made (or the user filed under it), erased or not.
    fn tutorial_notes(&self) -> Vec<usize> {
        let (project, tab) = (self.tutorial_project(), self.tutorial_tab());
        (0..self.data.notes.len())
            .filter(|&i| {
                let n = &self.data.notes[i];
                !n.purged
                    && ((project.is_some() && n.project == project)
                        || (tab.is_some() && n.wiki && n.wiki_tab == tab))
            })
            .collect()
    }
    fn example(&self, title: &str) -> Option<usize> {
        self.tutorial_notes()
            .into_iter()
            .find(|&i| self.data.notes[i].title == title && !self.data.notes[i].deleted)
    }
    /// Ask on a first run whether to take the tutorial.
    pub fn tutorial_offer(&mut self) {
        self.extra.tutorial = WELCOME;
    }
    /// Add the example memos, schedules and wiki, unless they are there.
    fn tutorial_install(&mut self) {
        if !self.tutorial_notes().is_empty() {
            return;
        }
        let today = Local::now().date_naive();
        let project = self.tutorial_project().unwrap_or_else(|| {
            self.data.projects.push(PROJECT.into());
            self.data.projects.len() - 1
        });
        let tab = self.tutorial_tab().unwrap_or_else(|| {
            self.extra.wiki_tabs.push(WIKI_TAB.into());
            self.extra.wiki_tabs.len() - 1
        });
        let add = |a: &mut App, title: &str, body: &str, date: Option<NaiveDate>| {
            let i = a.data.notes.len();
            let mut n = Note::new(title, date, i);
            n.body = body.into();
            n.project = Some(project);
            a.data.notes.push(n);
            i
        };
        add(
            self,
            M_TOUR,
            "여기가 본문이에요. 자유롭게 써 보세요.\n\n· 맨 위 한 줄은 제목이에요.\n· 위쪽 툴바: 색, 더보기(…), 알람, 일정, 완료, 접기, 닫기\n· 아래쪽 툴바: 글자 서식, 투명도\n· 제목 줄 오른쪽 동그라미가 완료 체크박스예요. 눌러 볼까요?",
            None,
        );
        add(
            self,
            M_TEXT,
            "[자동 들여쓰기]\n아래 항목 끝에서 Enter를 눌러 보세요.\n- 첫 번째 할 일\n\n[취소선]\n이 줄을 드래그로 고른 뒤 아래 툴바의 취소선 버튼을 눌러 보세요.\n\n[날짜 넣기]\n이 줄 끝에서 Ctrl+; 를 눌러 보세요: ",
            None,
        );
        add(
            self,
            M_PLAN,
            "이 메모에 일정과 알람을 달아 보세요.\n1) 위쪽의 일정 아이콘 → 날짜 고르기\n2) 알람 아이콘 → 5분 뒤로 맞추기\n3) … 더보기 → 잠금 (연습용 비밀번호를 쓰세요)",
            None,
        );
        for title in [M_S1, M_S2, M_S3] {
            add(
                self,
                title,
                "접기 버튼을 눌러 보세요.\n스티커 → 제목 한 줄 → 색상 상자\n\n그리고 캘린더 날짜로 끌어다 놓아 보세요.",
                None,
            );
        }
        let day = |n: i64| Some(today + chrono::Duration::days(n));
        let i = add(
            self,
            "튜토리얼 예시 일정 · 팀 회의",
            "오전 회의 준비물 확인",
            day(1),
        );
        self.data.notes[i].time = chrono::NaiveTime::from_hms_opt(10, 0, 0).unwrap();
        add(
            self,
            "튜토리얼 예시 일정 · 점심 약속",
            "맛집 예약하기",
            day(3),
        );
        let i = add(
            self,
            "튜토리얼 예시 일정 · 출장 (3일)",
            "여러 날 이어지는 일정은 색띠로 보여요.",
            day(6),
        );
        self.data.notes[i].duration = 3;
        let i = add(
            self,
            "튜토리얼 예시 일정 · 보고서 마감",
            "‘이날까지 업무’ 예시예요.",
            day(9),
        );
        self.data.notes[i].deadline = true;
        let i = add(
            self,
            "튜토리얼 예시 · 완료한 일",
            "완료한 메모는 업무일지에 모여요.",
            Some(today),
        );
        self.data.notes[i].done = true;
        self.data.notes[i].completed = Some(Local::now().naive_local());
        let i = add(
            self,
            "튜토리얼 예시 · 휴지통 연습",
            "휴지통에서 복원해 보세요.",
            None,
        );
        self.data.notes[i].deleted = true;
        self.desktop
            .deleted_at
            .insert(i, Local::now().naive_local());
        // The practice wiki: a folder with a memo, and one memo to file.
        self.extra.folders.push(crate::extras::Folder {
            title: "연습 폴더".into(),
            description: String::new(),
            parent: None,
            tab,
        });
        let folder = self.extra.folders.len() - 1;
        let i = add(
            self,
            "튜토리얼 위키 · 폴더 안 메모",
            "폴더 안에 있는 위키 메모예요. 우클릭 메뉴를 써 보세요.",
            None,
        );
        self.data.notes[i].wiki = true;
        self.data.notes[i].wiki_tab = Some(tab);
        self.data.notes[i].folder = Some(folder);
        let i = add(
            self,
            M_WIKI,
            "왼쪽 아래 목록에 있는 메모예요. 폴더로 끌어 보세요.",
            None,
        );
        // A wiki memo filed nowhere: it waits in the list below the tree.
        self.data.notes[i].wiki = true;
        self.extra.folder_open.insert(folder);
    }
    /// Erase every example memo and the project and wiki tab that held them.
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
        }
        let set: std::collections::HashSet<usize> = notes.into_iter().collect();
        self.data
            .links
            .retain(|(a, b)| !set.contains(a) && !set.contains(b));
        if let Some(p) = self.tutorial_project() {
            self.extra.deleted_projects.insert(p);
            self.extra.hidden_projects.remove(&p);
        }
        if let Some(t) = self.tutorial_tab() {
            self.extra.deleted_wiki_tabs.insert(t);
            self.extra.wiki_panes = [None; 2];
            self.extra.wiki_blank = false;
        }
        self.project_filter = None;
        self.picked.retain(|i| !set.contains(i));
        self.catalog.selected = self.catalog.selected.filter(|i| !set.contains(i));
        self.catalog.selected_many.retain(|i| !set.contains(i));
        if set.contains(&self.active) {
            self.editor = false;
            self.sticky = false;
        }
    }
    /// Make the main window visible on `page`.
    fn tutorial_show(&mut self, ctx: &egui::Context, page: usize) {
        self.page = page;
        self.settings = false;
        self.main_open = true;
        ctx.send_viewport_cmd_to(main_viewport(), egui::ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd_to(main_viewport(), egui::ViewportCommand::Focus);
    }
    fn tutorial_do(&mut self, ctx: &egui::Context, act: Act) {
        match act {
            Act::Open(titles) => {
                for t in titles {
                    if let Some(i) = self.example(t) {
                        self.open_sticker(i, self.data.notes[i].date);
                    }
                }
                // The drag step also wants the calendar to drop on.
                if titles == [M_S1, M_S2] {
                    self.tutorial_show(ctx, 5);
                }
            }
            Act::Page(p) => self.tutorial_show(ctx, p),
            Act::Calendar => {
                self.show_calendar();
                ctx.send_viewport_cmd_to(
                    egui::ViewportId::from_hash_of("calendar"),
                    egui::ViewportCommand::Focus,
                );
            }
            Act::Utilities(tab) => {
                self.tutorial_show(ctx, 6);
                self.extra.utilities_tab = tab;
            }
            Act::Wiki => {
                self.tutorial_show(ctx, 1);
                if let Some(tab) = self.tutorial_tab() {
                    self.extra.wiki_tab = tab;
                    let first = self.data.notes.iter().position(|n| {
                        n.wiki
                            && !n.deleted
                            && !n.purged
                            && n.folder.is_some()
                            && n.wiki_tab == Some(tab)
                    });
                    if first.is_some() {
                        self.extra.wiki_panes[0] = first;
                        self.extra.wiki_focus = 0;
                    }
                }
            }
            Act::Settings => {
                self.settings = true;
                self.main_open = true;
                ctx.send_viewport_cmd_to(main_viewport(), egui::ViewportCommand::Focus);
            }
        }
    }
    /// Start (or restart) the tutorial from the first step.
    pub fn tutorial_start(&mut self) {
        self.tutorial_install();
        self.extra.tutorial = STEP0;
    }
    /// The tutorial window: the offer, the steps, and the final choice.
    pub fn tutorial_window(&mut self, ctx: &egui::Context) {
        let state = self.extra.tutorial;
        if state == 0 {
            return;
        }
        enum Cmd {
            Start,
            Skip,
            Prev,
            Next,
            Act(Act),
            Keep,
            Erase,
            Back,
        }
        let mut cmd = None;
        let mut open = true;
        let step = state
            .checked_sub(STEP0)
            .map(|n| n as usize)
            .filter(|n| *n < STEPS.len());
        let samples = self.tutorial_notes().len();
        let title = match (state, step) {
            (WELCOME, _) => "환영합니다".to_string(),
            (FINISH, _) => "튜토리얼 마무리".to_string(),
            (_, Some(n)) => format!("튜토리얼 {}/{}", n + 1, STEPS.len()),
            _ => "튜토리얼".to_string(),
        };
        let size = if state == FINISH || state == WELCOME {
            [400., 300.]
        } else {
            [400., 470.]
        };
        crate::ui::dialog(ctx, "tutorial", &title, size, true, &mut open, |ui| {
            if state == WELCOME {
                ui.label(
                    RichText::new("OhMemoJang에 오신 걸 환영해요! 👋")
                        .strong()
                        .size(16.),
                );
                ui.add_space(8.);
                ui.label("예시 메모와 일정으로 주요 기능을 한 번씩 따라 해 보는 튜토리얼이 있어요. 10분이면 충분해요.");
                ui.add_space(4.);
                crate::ui::hint(
                    ui,
                    "언제든 건너뛸 수 있고, 끝나면 예시 데이터를 지울지 고를 수 있어요. 건너뛰어도 사용법은 나만의 위키에 있어요.",
                );
                ui.add_space(12.);
                ui.horizontal(|ui| {
                    if crate::ui::primary(ui, "튜토리얼 시작").clicked() {
                        cmd = Some(Cmd::Start);
                    }
                    if ui.button("건너뛰기").clicked() {
                        cmd = Some(Cmd::Skip);
                    }
                });
            } else if state == FINISH {
                ui.label(RichText::new("수고하셨어요! 🎉").strong().size(16.));
                ui.add_space(8.);
                ui.label(format!(
                    "연습용 예시 메모와 일정이 {samples}개 남아 있어요. 어떻게 할까요?"
                ));
                ui.add_space(4.);
                crate::ui::hint(
                    ui,
                    "지우면 ‘튜토리얼’ 프로젝트와 ‘연습 위키’ 탭의 메모가 모두 사라져요. 연습하며 ‘#튜토리얼 ’로 만든 메모도 함께요. 사용법과 내가 만든 다른 메모는 그대로예요.",
                );
                ui.add_space(12.);
                if crate::ui::primary(ui, "예시 모두 지우고 깨끗하게 시작").clicked() {
                    cmd = Some(Cmd::Erase);
                }
                ui.add_space(4.);
                ui.horizontal(|ui| {
                    if ui.button("예시 남겨 두기").clicked() {
                        cmd = Some(Cmd::Keep);
                    }
                    if ui.button("튜토리얼로 돌아가기").clicked() {
                        cmd = Some(Cmd::Back);
                    }
                });
            } else if let Some(n) = step {
                let s = &STEPS[n];
                ui.label(RichText::new(s.title).strong().size(16.));
                ui.add_space(6.);
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .max_height(size[1] - 150.)
                    .show(ui, |ui| {
                        for (k, line) in s.lines.iter().enumerate() {
                            if k == 0 {
                                ui.label(*line);
                            } else {
                                ui.label(format!("· {line}"));
                            }
                            ui.add_space(3.);
                        }
                    });
                ui.add_space(6.);
                ui.horizontal_wrapped(|ui| {
                    for (label, act) in s.acts {
                        if crate::ui::primary(ui, label).clicked() {
                            cmd = Some(Cmd::Act(*act));
                        }
                    }
                });
                ui.add_space(8.);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let last = n + 1 == STEPS.len();
                    if ui.button(if last { "마치기" } else { "다음 ▶" }).clicked() {
                        cmd = Some(Cmd::Next);
                    }
                    if n > 0 && ui.button("◀ 이전").clicked() {
                        cmd = Some(Cmd::Prev);
                    }
                    if ui.button("건너뛰기").clicked() {
                        cmd = Some(Cmd::Skip);
                    }
                });
            }
        });
        if !open && cmd.is_none() {
            // The window's own close button counts as skipping.
            cmd = Some(Cmd::Skip);
        }
        match cmd {
            None => {}
            Some(Cmd::Start) => {
                self.tutorial_start();
            }
            Some(Cmd::Skip) => {
                // Nothing was added yet when the offer is declined.
                self.extra.tutorial = if self.tutorial_notes().is_empty() {
                    0
                } else {
                    FINISH
                };
            }
            Some(Cmd::Prev) => self.extra.tutorial = state.saturating_sub(1).max(STEP0),
            Some(Cmd::Next) => {
                self.extra.tutorial = if step.is_some_and(|n| n + 1 == STEPS.len()) {
                    FINISH
                } else {
                    state + 1
                };
            }
            Some(Cmd::Act(act)) => self.tutorial_do(ctx, act),
            Some(Cmd::Keep) => {
                self.extra.tutorial = 0;
                self.toast = "예시를 남겨 두었어요".into();
            }
            Some(Cmd::Erase) => {
                self.tutorial_erase();
                self.extra.tutorial = 0;
                self.page = 0;
                self.toast = "예시를 모두 지우고 깨끗하게 시작해요".into();
            }
            Some(Cmd::Back) => self.extra.tutorial = STEP0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn app() -> App {
        let mut a = App::sample(Local::now().date_naive());
        a.start_fresh();
        a
    }
    #[test]
    fn every_step_has_text_and_every_sample_it_opens_exists() {
        let mut a = app();
        a.tutorial_start();
        for s in STEPS {
            assert!(!s.title.is_empty() && s.lines.len() >= 2);
            for (_, act) in s.acts {
                if let Act::Open(titles) = act {
                    for t in *titles {
                        assert!(a.example(t).is_some(), "{t}");
                    }
                }
            }
        }
        assert_eq!(a.extra.tutorial, STEP0);
    }
    #[test]
    fn the_examples_cover_schedules_a_done_memo_a_trashed_memo_and_a_wiki() {
        let mut a = app();
        a.tutorial_start();
        let notes = a.tutorial_notes();
        let n = |f: &dyn Fn(&Note) -> bool| notes.iter().any(|&i| f(&a.data.notes[i]));
        assert!(n(&|x| x.date.is_some() && x.duration == 3));
        assert!(n(&|x| x.deadline));
        assert!(n(&|x| x.done));
        assert!(n(&|x| x.deleted && !x.purged));
        assert!(n(&|x| x.wiki && x.folder.is_some()));
        assert!(n(&|x| x.wiki && x.folder.is_none()));
        // Starting again adds nothing more.
        let count = a.data.notes.len();
        a.tutorial_start();
        assert_eq!(a.data.notes.len(), count);
    }
    #[test]
    fn erasing_removes_the_examples_and_what_the_user_filed_under_the_project_only() {
        let mut a = app();
        let guide: Vec<bool> = a.data.notes.iter().map(|n| n.purged).collect();
        let mut mine = Note::new("내 메모", None, 0);
        mine.body = "남아야 해요".into();
        a.data.notes.push(mine);
        a.tutorial_start();
        // A memo typed with #튜토리얼 goes with the examples.
        let p = a.tutorial_project().unwrap();
        let mut tagged = Note::new("연습으로 쓴 메모", None, 1);
        tagged.project = Some(p);
        a.data.notes.push(tagged);
        a.tutorial_erase();
        assert!(a.tutorial_notes().is_empty());
        assert!(a.tutorial_project().is_none() && a.tutorial_tab().is_none());
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
            .any(|t| t.contains("튜토리얼") || t.contains("연습")));
        // The guide is untouched.
        assert_eq!(
            a.data
                .notes
                .iter()
                .take(guide.len())
                .map(|n| n.purged)
                .collect::<Vec<_>>(),
            guide
        );
        // Doing the tutorial again starts clean.
        a.tutorial_start();
        assert!(!a.tutorial_notes().is_empty());
    }
    #[test]
    fn tutorial_state_guide_version_and_deleted_wiki_tabs_survive_saving() {
        let dir = std::env::temp_dir().join(format!("ohmemojang-tut-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut a = app();
        a.tutorial_start();
        a.extra.tutorial = STEP0 + 3;
        let tab = a.tutorial_tab().unwrap();
        a.extra.deleted_wiki_tabs.insert(0);
        let snap = a.snapshot(&dir);
        crate::storage::write_all(&dir, &snap).unwrap();
        let loaded = crate::storage::read(&dir).unwrap().unwrap();
        let mut b = App::sample(Local::now().date_naive());
        b.restore(&loaded);
        assert_eq!(b.extra.tutorial, STEP0 + 3);
        assert_eq!(b.extra.guide_version, crate::guide::GUIDE_VERSION);
        assert!(b.extra.deleted_wiki_tabs.contains(&0));
        assert_eq!(b.tutorial_tab(), Some(tab));
        let _ = std::fs::remove_dir_all(&dir);
    }
    #[test]
    fn skipping_the_offer_adds_nothing_and_skipping_later_asks_what_to_do() {
        let mut a = app();
        a.tutorial_offer();
        assert_eq!(a.extra.tutorial, WELCOME);
        let count = a.data.notes.len();
        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |ctx| a.tutorial_window(ctx));
        assert_eq!(a.data.notes.len(), count);
        a.tutorial_start();
        assert!(a.tutorial_notes().len() > 5);
    }
}
