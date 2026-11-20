//! Example data for the usage guide's screenshots. Run by hand:
//! `OHMEMO_STAGE=<dir> cargo test --release guide_stage -- --ignored`
//! then start the program on that data folder and take the pictures.
use super::*;
use crate::composer::{Format, Span};
use crate::storage::SessionRec;

fn note(a: &mut App, title: &str, body: &str, date: Option<NaiveDate>, project: usize) -> usize {
    let i = a.data.notes.len();
    let mut n = Note::new(title, date, i);
    n.body = body.into();
    n.document.tabs[0].body = body.into();
    n.project = Some(project);
    a.data.notes.push(n);
    i
}

fn strike(a: &mut App, i: usize, range: std::ops::Range<usize>) {
    a.data.notes[i].document.tabs[0].spans.push(Span {
        range,
        format: Format {
            strike: true,
            ..Default::default()
        },
    });
}

fn sticker(note: usize, x: f32, y: f32, mode: u8) -> SessionRec {
    SessionRec {
        note,
        mode,
        sticker_preferred: true,
        pos: Some([x, y]),
        full_size: [340., 330.],
        sticker_size: [340., 330.],
        title_width: 260.,
        swatch_size: [56., 56.],
        closed: false,
    }
}

#[test]
#[ignore]
fn guide_stage() {
    let Some(dir) = std::env::var_os("OHMEMO_STAGE").map(std::path::PathBuf::from) else {
        return;
    };
    let today = Local::now().date_naive();
    let day = |n: i64| Some(today + chrono::Duration::days(n));
    let mut a = App::sample(today);
    a.start_fresh();
    a.data.projects = vec!["개인 업무".into(), "회사".into(), "집안일".into()];
    let shop = note(
        &mut a,
        "장보기 목록",
        "- 우유\n- 계란\n- 식빵\n☑ 세탁소 들르기\n☐ 택배 보내기",
        Some(today),
        2,
    );
    strike(&mut a, shop, 2..4);
    a.data.notes[shop].color = PALETTE[0];
    let meet = note(
        &mut a,
        "주간 회의 준비",
        "1. 지난주 실적 정리\n2. 발표 자료 확인\n  a) 그래프 업데이트\n  b) 일정표 첨부",
        day(1),
        1,
    );
    a.data.notes[meet].time = chrono::NaiveTime::from_hms_opt(10, 0, 0).unwrap();
    a.data.notes[meet].color = PALETTE[3];
    let report = note(
        &mut a,
        "분기 보고서 제출",
        "팀장님께 메일로 제출",
        day(5),
        1,
    );
    a.data.notes[report].deadline = true;
    let trip = note(&mut a, "부산 출장", "KTX 08:00 · 숙소 예약 완료", day(8), 1);
    a.data.notes[trip].duration = 3;
    note(&mut a, "독서 모임", "이번 달 책: 데미안", day(12), 0);
    note(&mut a, "치과 예약", "오후 3시", day(-3), 0);
    let done1 = note(&mut a, "견적서 보내기", "", day(-1), 1);
    a.data.notes[done1].done = true;
    a.data.notes[done1].completed = Some(Local::now().naive_local() - chrono::Duration::days(1));
    let done2 = note(&mut a, "회의록 정리", "", Some(today), 1);
    a.data.notes[done2].done = true;
    a.data.notes[done2].completed = Some(Local::now().naive_local());
    let ideas = note(
        &mut a,
        "아이디어 메모",
        "- 주말에 캠핑 가기\n- 새 노트북 알아보기",
        None,
        0,
    );
    a.data.notes[ideas].color = PALETTE[5];
    let old = note(&mut a, "지난 메모", "휴지통 예시", None, 0);
    a.trash_note(old);
    // A wiki tab with a folder.
    let tab = a.extra.wiki_tabs.len();
    a.extra.wiki_tabs.push("업무 노트".into());
    a.extra.folders.push(crate::extras::Folder {
        title: "회의록".into(),
        description: String::new(),
        parent: None,
        tab,
    });
    let folder = a.extra.folders.len() - 1;
    a.extra.folder_open.insert(folder);
    for (title, body) in [
        ("11월 회의록", "· 다음 분기 목표 확정\n· 담당자 배정"),
        ("10월 회의록", "· 예산 검토"),
    ] {
        let i = note(&mut a, title, body, None, 1);
        a.data.notes[i].wiki = true;
        a.data.notes[i].wiki_tab = Some(tab);
        a.data.notes[i].folder = Some(folder);
    }
    let i = note(
        &mut a,
        "업무 규칙",
        "· 회의는 30분 안에\n· 자료는 전날 공유",
        None,
        1,
    );
    a.data.notes[i].wiki = true;
    a.data.notes[i].wiki_tab = Some(tab);
    a.extra.wiki_tab = tab;
    // An alarm due shortly, to see the Windows notification.
    if let Ok(secs) = std::env::var("OHMEMO_STAGE_ALARM") {
        let secs: i64 = secs.parse().unwrap_or(40);
        a.data.notes[shop].reminder =
            Some(Local::now().naive_local() + chrono::Duration::seconds(secs));
    }
    let mut saved = a.snapshot(&dir);
    saved.session = match std::env::var("OHMEMO_STAGE_MODE").as_deref() {
        // Stickers on the desktop: two open, three folded to titles on the right.
        Ok("stickers") => vec![sticker(shop, 260., 120., 3), sticker(meet, 640., 120., 3)],
        _ => vec![],
    };
    crate::storage::write_all(&dir, &saved).unwrap();
}
