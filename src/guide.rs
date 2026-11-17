//! The usage guide: a "사용법" wiki tab with one memo per topic, most with a
//! screenshot. It is added to a new workspace and opens on the very first run;
//! a newer guide replaces the old pages at the next start (`refresh_guide`).
use crate::composer::{placeholder, Block, Picture};
use crate::*;

pub const TAB: &str = "사용법";

struct Page {
    title: &'static str,
    image: Option<&'static [u8]>,
    intro: &'static str,
    body: &'static str,
}

/// Raised whenever the guide text changes; guide memos written for an older
/// version are replaced the next time the program starts.
pub const GUIDE_VERSION: u32 = 2;

const PAGES: &[Page] = &[
    Page {
        title: "① 처음 시작하기",
        image: Some(include_bytes!("../assets/guide/home.jpg")),
        intro: "OhMemoJang은 메모, 할 일, 일정, 위키를 한곳에서 다루는 내 작업 공간이에요. 이 사용법은 ‘나만의 위키 › 사용법’ 탭에 있어서 언제든 다시 볼 수 있어요.",
        body: "· 왼쪽 메뉴에서 화면을 골라요: 내 작업실, 오늘의 할일, 다음에 할일, 캘린더, 전체메모, 나만의 위키, 편의기능, 클립보드 히스토리.
· ‘빠른 메모’를 누르면 바로 새 스티커 메모가 열려요. 다른 프로그램을 쓰는 중에도 Ctrl+Alt+N(설정에서 바꿀 수 있어요)으로 열 수 있어요.
· 메모·위키·설정은 자동으로 저장돼요. 저장 버튼은 없어요!
· 메인 창을 X로 닫아도 프로그램은 오른쪽 아래 트레이에서 계속 실행되고, 스티커는 바탕화면에 그대로 있어요. 트레이 아이콘을 누르면 다시 열리고, 우클릭 › ‘종료’로 끝내요.
· 처음이라면 설정 › 정보의 ‘튜토리얼 다시 하기’로 예시 메모와 함께 따라 해 볼 수 있어요.",
    },
    Page {
        title: "② 메모장 영역 · 완료 체크",
        image: Some(include_bytes!("../assets/guide/sticker.jpg")),
        intro: "메모 하나가 바탕화면 위의 작은 메모장 하나예요. 크게 제목, 본문, 툴바로 나뉘어요.",
        body: "· 제목: 맨 위 한 줄이에요. 비워 두면 본문 첫 줄이 자동으로 제목이 돼요.
· 본문: 자유롭게 쓰는 곳이에요. 아래쪽 탭의 +로 본문을 여러 장 만들 수 있어요.
· 위쪽 툴바: + 새 메모, 색, … 더보기(잠금·위키 보관·프로젝트·정렬·복제·삭제), 알람, 일정, 편집창, 항상 위, 완료, 접기, 닫기.
· 아래쪽 툴바: 글자 크기, 굵게·기울임·밑줄·취소선, 투명도, ▾ 프로젝트 고르기, › 연결된 메모.
· 완료 체크박스: 제목 줄 오른쪽 동그라미를 누르면 완료예요. 완료한 일은 업무일지에 모여요.
· Esc를 두 번 누르면 메모가 닫혀요.",
    },
    Page {
        title: "③ 본문 쓰기 요령",
        image: None,
        intro: "본문에는 손을 덜어 주는 자동 기능이 여럿 있어요.",
        body: "· 자동 들여쓰기: 줄 맨 앞에 ‘- ’, ‘1. ’, ‘a) ’ 처럼 쓰고 스페이스를 누르면 목록이 돼요. Enter를 누르면 같은 모양의 다음 항목이 이어지고, 빈 항목에서 Enter를 한 번 더 누르면 한 단계 올라가요. Tab은 들여쓰기, Shift+Tab은 내어쓰기예요.
· 취소선: 끝낸 내용을 드래그로 고른 뒤 아래 툴바의 취소선 버튼을 눌러요.
· 자동 제목 입력: 제목을 비워 두고 본문만 써도 첫 줄이 제목이 돼요. 목록 첫 줄이면 ‘1.’ 같은 기호를 빼고 가져와요.
· 자동 프로젝트 입력: 본문 맨 앞에 ‘#프로젝트이름 ’(뒤에 스페이스)을 쓰면 그 프로젝트로 분류돼요. 없는 프로젝트면 새로 만들어 주고, 입력한 ‘#이름 ’은 본문에서 사라져요.
· 서식 단축키: Ctrl+B 굵게, Ctrl+I 기울임, Ctrl+U 밑줄. Ctrl+; 은 오늘 날짜, Ctrl+Shift+; 은 날짜와 시간을 넣어요.
· 표, 이미지, 링크도 붙여넣을 수 있어요. 모아 보기는 ‘편의기능’에 있어요.",
    },
    Page {
        title: "④ 일정 · 알람 · 잠금",
        image: None,
        intro: "메모에 날짜와 알림을 달고, 남이 못 보게 잠글 수 있어요.",
        body: "· 일정: 스티커 위쪽의 일정 아이콘을 눌러요. 시작·종료 날짜와 시간을 고르고, ‘이날까지 업무’를 켜면 종료일이 마감이 돼요. 반복(매일·매주·격주·매월·매년)도 정할 수 있어요.
· 알람: 알람 아이콘을 눌러 날짜와 시각을 정해요. 시각이 되면 화면 오른쪽 아래에 알림이 뜨고 ‘열기 · 5분 뒤 · 확인’ 중에서 골라요. 소리와 팝업은 설정 › 알람에서 바꿔요.
· 잠금: … 더보기에서 잠금을 골라 비밀번호를 정해요. 잠긴 메모는 제목만 보이고, 비밀번호를 넣어야 내용이 열려요. 다시 잠기는 시간은 설정 › 일반에서 정해요.
· 설정한 일정은 캘린더와 ‘오늘의 할일 · 다음에 할일’에 자동으로 나타나요.",
    },
    Page {
        title: "⑤ 스티커 접기 · 자동 정렬",
        image: Some(include_bytes!("../assets/guide/sticker.jpg")),
        intro: "바탕화면이 스티커로 어수선해지면 접거나 한 번에 정리하세요.",
        body: "· 접기 버튼을 누를 때마다 스티커 → 제목 한 줄 → 색상 상자 순서로 작아져요. 색상 상자는 한 번 누르면 펼쳐지고, 끌면 옮겨져요.
· 자동 정렬: 스티커를 우클릭(또는 … 더보기)하고 ‘스티커 정렬’에서 ‘화면 오른쪽에 정렬’, ‘화면 왼쪽에 정렬’, ‘계단식으로 정렬’을 골라요. 오른쪽·왼쪽은 ‘모두 제목만 보이게 · 모두 상자로 접기 · 모두펼치기’를 같이 정할 수 있어요.
· 모든 스티커 닫기: Ctrl+Shift+H. 항상 위에 두기: Ctrl+Shift+T.
· 매일 아침 자동으로: 설정 › 일반의 ‘매일 오늘 메모 자동으로 띄우기’를 켜면 새벽 5시 기준으로 지난 미완료 메모와 오늘 메모가 오른쪽에 제목 스티커로 정렬돼요.
· Windows를 켤 때 자동 실행하면 열어 둔 스티커가 제자리에 다시 떠요.",
    },
    Page {
        title: "⑥ 캘린더",
        image: Some(include_bytes!("../assets/guide/calendar.jpg")),
        intro: "작성일·일정·알람 기준으로 메모를 달력에 펼쳐 봐요.",
        body: "· 위쪽 버튼으로 ‘작성일 · 일정 · 알람’ 기준을 바꿔요.
· 여러 날 이어지는 일정은 색띠 하나로 보여요. 띠를 누르면 그 날짜의 메모가 열려요.
· 날짜에 마우스를 올리면 나오는 + 버튼으로 그날의 새 스티커를 만들어요.
· 선택한 날짜의 일정 목록이 달력과 함께 보여요.
· 설정 › 캘린더에서 테마, 글자 크기, 완료 일정 숨기기·취소선, 보기 단위(한 달·1주·2주)를 정해요.",
    },
    Page {
        title: "⑦ 캘린더 분리창 · 스탠딩 캘린더",
        image: Some(include_bytes!("../assets/guide/calwindow.jpg")),
        intro: "캘린더를 별도 창으로 띄워 두고, 바탕화면에 세워 둘 수도 있어요.",
        body: "· 분리창: 캘린더 화면 오른쪽 위의 분리 아이콘 또는 Ctrl+Alt+C로 열어요. 위쪽 동그라미로 테마(기본·종이·세이지·나이트)를 고르고, 창 아무 곳이나 우클릭해서 한 달·1주·2주 보기를 바꿔요. 가로 배치 아이콘을 누르면 달력 옆에 그날의 일정이 나란히 보여요.
· 닫기와 최소화: 창의 X(닫기)를 누르면 분리창이 닫혀요. ‘최소화’를 누를 때만 달력 모양의 작은 색상 상자로 접혀요. 상자를 누르면 다시 열리고, 끌어서 옮기고, 모서리로 크기를 바꾸고, 우클릭으로 색을 바꾸거나 닫아요.
· 스탠딩 캘린더: 받침대 모양 스탠딩 아이콘을 누르면 지금 보이는 크기 그대로 바탕화면에 고정돼요. 바탕화면 아이콘을 그 위에 둘 수 있어요. 같은 자리의 스탠딩 아이콘을 다시 누르면 풀려요.
· ⚠ Windows 11 24H2에서는 스탠딩 캘린더가 동작하지 않아요. 그럴 땐 분리창이나 색상 상자로 사용해 주세요.",
    },
    Page {
        title: "⑧ 캘린더로 끌어다 놓기",
        image: None,
        intro: "스티커를 달력 위 날짜로 끌어다 놓으면 바로 일정이 돼요.",
        body: "· 그냥 끌어다 놓기: 놓은 날짜가 그 메모의 일정 시작일이 돼요. 이미 일정이 있는 메모를 다른 날로 끌면 날짜가 옮겨져요.
· Shift를 누른 채 끌어다 놓기: 그 날짜가 마감인 ‘이날까지 업무’가 돼요. D-day도 함께 표시돼요.
· 놓을 때마다 고르고 싶다면 설정 › 일반의 ‘메모 끌어 놓기 시 이날까지 업무 선택창 띄우기’를 켜세요(캘린더에서만 나와요). Shift를 누르고 끌면 선택창 없이 바로 ‘이날까지 업무’가 돼요.
· 메인 창의 캘린더, 캘린더 분리창에 똑같이 끌어다 놓을 수 있어요.
· 전체메모의 폴더로 끌어다 놓으면 폴더 분류가 돼요.",
    },
    Page {
        title: "⑨ 내 작업실 · 오늘의 할일 · 다음에 할일",
        image: Some(include_bytes!("../assets/guide/today.jpg")),
        intro: "오늘 할 일과 앞으로의 일을 모아 보고, 오른쪽 미리보기에서 바로 고쳐요.",
        body: "· 내 작업실은 날짜별로 묶인 메모 목록과 캘린더를 함께 보여 줘요. 위쪽 필터로 프로젝트, 정기 일정, 생성일 기준, 완료 메모를 골라요.
· 메모를 누르면 오른쪽 미리보기에서 제목과 본문을 바로 편집해요. 미리보기 아래에 알람, 일정, 프로젝트, 연결된 메모 도구가 있어요.
· 행의 동그라미는 완료, ☆는 중요 표시, 눈 아이콘은 스티커로 띄우기예요. 여러 날 이어지는 일정은 ‘이 날짜만’이나 ‘전체 일정’ 중에서 골라 완료해요.
· ‘+ 새 메모’로 만들고, 메모를 캘린더 날짜로 끌면 일정이 정해져요.",
    },
    Page {
        title: "⑩ 전체메모 · 찾아 바꾸기",
        image: Some(include_bytes!("../assets/guide/allmemo.jpg")),
        intro: "모든 메모를 폴더와 표로 정리하고, 한꺼번에 찾고 바꿔요.",
        body: "· 폴더 보기와 전체 메모 보기(표)를 오가며 정리해요. 표 머리글 우클릭으로 보일 열을 고르고, 열 사이를 끌어 너비를 바꿔요.
· Ctrl+H 또는 찾기 막대의 버튼으로 찾아 바꾸기를 열어요. 범위(현재 메모·선택 프로젝트·전체 메모)를 고르고 찾기·모두 찾기·바꾸기·모두 바꾸기를 써요.
· ‘모두 찾기’ 결과는 오른쪽에 메모별로 보여요. 잠긴 메모와 휴지통은 찾지 않아요.",
    },
    Page {
        title: "⑪ 편의기능 · 업무일지 · 휴지통",
        image: Some(include_bytes!("../assets/guide/utilities.jpg")),
        intro: "일을 돌아보고, 지운 메모를 되살리는 보조 화면이에요.",
        body: "· 업무일지: 완료한 일을 날짜별·월별로 모아 보고 댓글을 남겨요. 한 달을 돌아볼 때 편해요.
· 표 모아보기, 이미지 모아보기, 링크 · 즐겨찾기: 메모 속 자료를 한눈에 모아 봐요.
· 휴지통: 지운 메모와 본문 탭이 여기로 와요. 복원하거나 영구 삭제할 수 있어요. 위키 탭이나 폴더를 지우면 ‘이름/’으로 묶여 보여요. 묶음을 복구하면 통째로, 펼쳐서 메모 하나만 복구하면 그 메모만 돌아와요.
· 지운 메모는 설정의 보관 기간이 지나면 프로그램을 켤 때 완전히 지워져요.
· 쓰지 않는 메뉴는 설정 › 일반 › 메뉴 숨기기에서 숨길 수 있어요.",
    },
    Page {
        title: "⑫ 클립보드 히스토리",
        image: None,
        intro: "복사한 내용을 모아 두었다가 필요할 때 한 번 클릭으로 다시 꺼내 써요.",
        body: "· 처음에는 꺼져 있어요. 왼쪽 메뉴의 ‘클립보드 히스토리’에서 설명을 읽고 ‘수집 시작’을 눌러야 켜져요.
· 켜 두면 OhMemoJang이 실행 중일 때 복사한 텍스트·링크·표·이미지가 이 PC에 쌓여요. 인터넷으로 보내지 않아요.
· 목록의 항목을 한 번 클릭하면 다시 클립보드로 복사돼요. 그다음 원하는 곳에 Ctrl+V로 붙여 넣으면 돼요.
· 위쪽 검색과 ‘전체·텍스트·링크·표·이미지’ 필터로 찾아요.
· ⚠ 비밀번호처럼 민감한 내용도 저장될 수 있고 저장 파일은 암호화되지 않아요. 그런 걸 복사하기 전에는 ‘일시 중지’를 누르세요.
· 정리: ‘설정·삭제’에서 개수 제한, 기간 제한(기본 30일)을 고르고, 날짜 이전 삭제나 전체 삭제를 할 수 있어요. 용량은 합계 256MB까지이고 오래된 것부터 지워요. 매일 낮 12시에 자동으로 정리해요.
· ‘수집 끄기’를 누르면 더 이상 모으지 않아요.",
    },
    Page {
        title: "⑬ 나만의 위키",
        image: Some(include_bytes!("../assets/guide/wiki.jpg")),
        intro: "오래 남길 지식을 탭과 폴더로 정리하고, 두 영역에 나란히 띄워 비교해요.",
        body: "· 위쪽 탭마다 자기 폴더 트리가 있어요. +로 탭을 더하고 더블클릭으로 이름을 바꿔요.
· 왼쪽 위는 트리, 왼쪽 아래는 아직 분류하지 않은 위키 메모예요. 메모를 탭이나 폴더로 끌어 분류해요.
· 메모 우클릭: 새 폴더, 새 위키 메모, 위키 해제, 메모 삭제. 폴더 우클릭: 새 폴더, 새 하위 폴더, 새 위키 메모, 이름 바꾸기, 폴더 삭제, 폴더·메모 삭제.
· 스티커 메모의 … 더보기 › ‘위키 보관’으로 일반 메모를 위키로 옮겨요. 되돌리려면 ‘위키 해제’를 써요.
· 오른쪽 영역 1·2 중 누른 영역에 메모가 열려요. 영역 2의 ‘빈 메모 열기’로 만든 메모는 영역 2를 닫을 때까지 유지돼요. 탭을 바꿔도 열어 둔 내용은 그대로예요.
· 영역 위쪽 아이콘: 스티커로 열기, 댓글 보기, 닫기.
· 탭이나 폴더를 지우면 안의 메모와 함께 휴지통으로 가서 언제든 복구할 수 있어요.",
    },
    Page {
        title: "⑭ 설정 · 백업 · 더 알아보기",
        image: Some(include_bytes!("../assets/guide/settings.jpg")),
        intro: "여기까지 읽었다면 기본은 다 아는 거예요. 나머지는 왼쪽 아래 ‘설정’에서 천천히 읽어 보세요. 항목마다 설명이 달려 있어요.",
        body: "· 일반: Windows 시작 시 자동 실행, 새 스티커 크기, 매일 오늘 메모, 정렬할 모니터, 첫 화면, 메뉴 숨기기, 휴지통 보관 기간, 잠긴 메모 자동 잠금.
· 메인창·메모창: 테마(라이트·다크), 글꼴, 글자 크기, 투명도.
· 캘린더: 테마, 글자 크기, 완료 일정 표시, 보기 단위.
· 알람: 소리, 팝업 알림, 전체 알림 끄기.
· 단축키: 빠른 메모, 캘린더, 새 스티커. 충돌하면 기존 설정으로 돌아가요.
· 백업 / 복원: 데이터 위치, ‘지금 백업’, ‘백업에서 복원’. 백업은 데이터 폴더의 backups 안에 날짜별로 쌓여요. ‘사용하지 않는 이미지 정리’는 어디서도 쓰지 않는 이미지를 지우지 않고 따로 옮겨 둬요.
· 정보: 버전, 사용법 보기, 튜토리얼 다시 하기.",
    },
];

/// Add the guide memos to the wiki tab `tab`.
fn add_pages(a: &mut App, tab: usize) {
    for (k, page) in PAGES.iter().enumerate() {
        let mut n = Note::new(page.title, None, k);
        n.wiki = true;
        n.wiki_tab = Some(tab);
        n.color = PALETTE[k % PALETTE.len()];
        n.body = format!("{}\n", page.intro);
        if let Some(bytes) = page.image {
            if let Ok(img) = image::load_from_memory(bytes) {
                let img = img.to_rgba8();
                let size = [img.width() as usize, img.height() as usize];
                let picture = Picture::from_rgba(
                    0,
                    format!("{}.jpg", page.title),
                    img.into_raw(),
                    size,
                    1.,
                    true,
                );
                n.document.tabs[0].blocks.push(Block::Picture(picture));
                n.body.push(placeholder(0));
                n.body.push('\n');
                n.has_image = true;
            }
        }
        n.body.push_str(page.body);
        a.data.notes.push(n);
    }
}

/// Add the guide tab and its memos.
pub fn install(a: &mut App) {
    let tab = a.extra.wiki_tabs.len();
    a.extra.wiki_tabs.push(TAB.into());
    add_pages(a, tab);
    a.extra.guide_version = GUIDE_VERSION;
}

/// A guide page title starts with a circled number (① … ⑳).
fn is_guide_title(title: &str) -> bool {
    title
        .chars()
        .next()
        .is_some_and(|c| ('\u{2460}'..='\u{2473}').contains(&c))
}

impl App {
    /// Bring the guide memos up to date when this version came with a newer
    /// guide: the old guide pages (titles that start with a circled number)
    /// are erased and the current ones added. Memos the user added to the
    /// guide tab stay, and nothing happens if the tab was deleted.
    pub fn refresh_guide(&mut self) {
        if self.extra.guide_version >= GUIDE_VERSION {
            return;
        }
        let Some(tab) = self.extra.wiki_tabs.iter().position(|t| t == TAB) else {
            return;
        };
        self.extra.guide_version = GUIDE_VERSION;
        if self.extra.deleted_wiki_tabs.contains(&tab) {
            return;
        }
        let old: Vec<usize> = (0..self.data.notes.len())
            .filter(|&i| {
                let n = &self.data.notes[i];
                n.wiki && !n.purged && n.wiki_tab == Some(tab) && is_guide_title(&n.title)
            })
            .collect();
        for i in old {
            if let Some(w) = self.studio.windows.get_mut(&i) {
                w.open = false;
            }
            self.purge_note(i);
            self.data.notes[i].wiki = false;
            self.data.notes[i].wiki_tab = None;
        }
        add_pages(self, tab);
    }
    /// Memos of the guide tab stay in the wiki: the to-do lists skip them.
    pub fn is_guide(&self, i: usize) -> bool {
        self.data.notes[i]
            .wiki_tab
            .and_then(|t| self.extra.wiki_tabs.get(t))
            .is_some_and(|t| t == TAB)
    }
    /// Show the usage guide: My wiki, the guide tab, first page in area 1.
    pub fn open_guide(&mut self) {
        self.page = 1;
        if let Some(tab) = self.extra.wiki_tabs.iter().position(|t| t == TAB) {
            self.extra.wiki_tab = tab;
            let first = self.data.notes.iter().position(|n| {
                n.wiki && !n.deleted && n.wiki_tab == Some(tab) && n.folder.is_none()
            });
            if first.is_some() {
                self.extra.wiki_panes[0] = first;
                self.extra.wiki_focus = 0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_new_workspace_holds_the_guide_and_opens_it() {
        let mut a = App::sample(Local::now().date_naive());
        a.start_fresh();
        let tab = a.extra.wiki_tabs.iter().position(|t| t == TAB).unwrap();
        let pages: Vec<_> = a
            .data
            .notes
            .iter()
            .filter(|n| n.wiki_tab == Some(tab))
            .collect();
        assert_eq!(pages.len(), PAGES.len());
        // Pages that come with a screenshot carry it.
        for (n, page) in pages.iter().zip(PAGES) {
            assert_eq!(n.has_image, page.image.is_some());
        }
        assert_eq!(a.extra.guide_version, GUIDE_VERSION);
        a.open_guide();
        assert_eq!(a.page, 1);
        assert_eq!(a.extra.wiki_tab, tab);
        assert!(a.extra.wiki_panes[0].is_some());
    }
    #[test]
    fn an_old_guide_is_replaced_and_the_users_own_pages_stay() {
        let mut a = App::sample(Local::now().date_naive());
        a.start_fresh();
        let tab = a.extra.wiki_tabs.iter().position(|t| t == TAB).unwrap();
        // The user's own memo in the guide tab, and an edited old page.
        let mut own = Note::new("내 메모", None, 0);
        own.wiki = true;
        own.wiki_tab = Some(tab);
        a.data.notes.push(own);
        a.data.notes[0].body = "예전 내용".into();
        a.extra.guide_version = 1;
        a.refresh_guide();
        assert_eq!(a.extra.guide_version, GUIDE_VERSION);
        let live: Vec<_> = a
            .data
            .notes
            .iter()
            .filter(|n| !n.purged && n.wiki_tab == Some(tab))
            .collect();
        assert_eq!(live.len(), PAGES.len() + 1);
        assert!(live.iter().any(|n| n.title == "내 메모"));
        assert!(live.iter().all(|n| n.body != "예전 내용"));
        // Up to date: nothing more happens.
        let count = a.data.notes.len();
        a.refresh_guide();
        assert_eq!(a.data.notes.len(), count);
    }
    #[test]
    fn a_deleted_guide_tab_is_not_brought_back() {
        let mut a = App::sample(Local::now().date_naive());
        a.start_fresh();
        let tab = a.extra.wiki_tabs.iter().position(|t| t == TAB).unwrap();
        a.extra.deleted_wiki_tabs.insert(tab);
        a.extra.guide_version = 1;
        let count = a.data.notes.len();
        a.refresh_guide();
        assert_eq!(a.data.notes.len(), count);
    }
}
