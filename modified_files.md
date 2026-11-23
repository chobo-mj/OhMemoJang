# 2.3.0·2.3.1 변경 파일

## 2.3.0 작업분

2.2.3 이후의 [구현 커밋 범위](https://github.com/chobo-mj/OhMemoJang/compare/5dff06f...8c60189)다. GitHub에 v2.3.0 태그와 별도 릴리스 기록이 없어 main에 올라간 작업분을 기준으로 정리했다. 이 변경은 2.3.1에 포함된다.

| 파일 | 주요 변경 |
| --- | --- |
| `docs/manual-verification-v2.3.0.md` | Windows 수동 검증 목록, 확인 결과, 2만 메모 성능 비교를 기록했다. |
| `src/catalog.rs` | 전체메모 선택 모드, 일괄 드래그, 수동 정렬, 프로젝트 보기와 휴지통 목록을 추가했다. |
| `src/catalog_batch.rs` | 일괄 이동·정렬·완료·휴지통 이동과 실행 취소를 구현했다. |
| `src/composer.rs` | 본문 탭의 고유 ID와 표·이미지 블록 제거를 추가했다. |
| `src/content_collections.rs` | 표·이미지 다중 선택, 복사·저장·원본 열기·삭제를 추가했다. |
| `src/desktop.rs` | 휴지통 확인창 Enter·Esc 처리와 영구삭제 저장 완료 알림을 구현했다. |
| `src/dragging.rs` | 일괄 드롭과 기존 일정·관계 드래그의 경로를 조정했다. |
| `src/extras.rs` | 편의기능 탭 전환 시 선택을 정리하고, 프로젝트의 메모 삭제에 휴지통 규칙을 적용했다. |
| `src/lists.rs` | 데이터와 필터가 바뀔 때 목록을 갱신하는 캐시를 추가했다. |
| `src/main.rs` | 선택·확인창 상태와 단축키를 연결하고 데이터 전환 시 임시 상태를 정리했다. |
| `src/media.rs` | 즐겨찾기 HTML 폴더 가져오기, 접기·펼치기, 검색과 원본 위치 이동을 구현했다. |
| `src/model.rs` | 목록 캐시에 메모 변경을 전달하는 번호를 추가했다. |
| `src/perf_bench.rs` | 1,500개·20,000개 메모 벤치마크를 추가했다. |
| `src/refinements.rs` | 단건과 일괄 완료에 같은 기간 일정 기준일을 사용한다. |
| `src/selection.rs` | Ctrl·Shift·Ctrl+A·Esc를 처리하는 공통 선택 상태를 추가했다. |
| `src/storage.rs` | 수동 순서·즐겨찾기 트리·탭 ID를 저장하고 구형 탭 ID를 백업 후 이전한다. |
| `src/trash_groups.rs` | 메모·본문 탭의 일괄 복원·영구삭제와 대상 확인을 구현했다. |
| `src/tutorial.rs` | 예시 메모를 제거할 때 그 메모의 선택 상태를 정리한다. |

## 2.3.1

| 파일 | 주요 변경 |
| --- | --- |
| `src/catalog_projects.rs` | 프로젝트 폴더 트리, 폴더 추가·이름 변경, 완료·본문 체크박스 표시와 편집 영역 너비 조절을 구현했다. |
| `src/catalog.rs` | 프로젝트 보기를 첫 탭으로 옮기고 트리의 메모 선택·드롭·편집을 연결했다. |
| `src/catalog_batch.rs` | 프로젝트 폴더 일괄 이동과 이전 프로젝트·폴더로 실행 취소를 지원한다. |
| `src/desktop.rs` | 메모 영구삭제 시 프로젝트 폴더 배정도 정리한다. |
| `src/extras.rs` | 관계 트리·자동 정렬에 프로젝트 필터를 적용하고 프로젝트 병합·삭제 시 폴더 배정을 정리한다. |
| `src/main.rs` | 전체메모 진입 시 프로젝트 보기를 열고 편집 영역 너비와 관계 보드 필터 상태를 관리한다. |
| `src/perf_bench.rs` | 변경된 탭 순서를 벤치마크에 반영했다. |
| `src/prefs.rs` | 프로젝트 보기의 편집 영역 너비 설정을 저장한다. |
| `src/storage.rs` | 프로젝트 하위 폴더와 메모 배정을 저장하며 이전 데이터에 빈 기본값을 적용한다. |
| `src/studio.rs` | 스티커의 전체메모 바로가기에 변경된 탭 순서를 반영했다. |
| `src/views.rs` | 관계 보드에 프로젝트·미분류 필터를 추가하고 카드, 관계선, 선택 범위에 적용했다. |
| `Cargo.toml`, `Cargo.lock`, `packaging/OhMemoJang.iss` | 앱과 Windows 패키지 버전을 2.3.1로 올렸다. |
| `modified_files.md` | 두 버전의 변경 파일과 내용을 기록했다. |
