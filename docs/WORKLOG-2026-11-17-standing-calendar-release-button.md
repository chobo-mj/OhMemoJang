# 스탠딩 캘린더 해제 버튼 점검

## 수정 전 확인한 문제
- 스탠딩 상태의 일반 egui 버튼 클릭을 `!standing` 조건으로 무시한다.
- 바탕화면 마우스 감지 영역이 고정 전 화면 좌표이며 타이틀바 제거, 창 이동, DPI 변경을 반영하지 않는다.
- 마우스 훅은 클릭 플래그만 설정하고 앱 갱신을 요청하지 않는다.
- 마우스 훅 설치 실패를 확인하지 않으며 전역 스레드 핸들을 공유한다.
- SetParent의 null 반환을 무조건 실패로 간주한다. 이전 부모가 null인 성공과 오류를 구별해야 한다.

## 작업 목록
- [x] 일반 버튼 클릭으로 스탠딩 해제, 캘린더 열기 단축키를 통한 복구.
- [x] 실제 클라이언트 원점과 현재 버튼 영역으로 바탕화면 클릭 판정.
- [x] 클릭 시 앱 즉시 갱신, 감지 스레드 수명 분리와 설치 실패 처리.
- [x] 고정 전환 및 부모 창 변경 실패를 올바르게 처리.
- [x] 회귀 테스트와 검사. (실제 창 조작은 미검증, 아래 참고)

0.6.0 태그는 변경하지 않는다. 사용자 PC의 정확한 Windows/DPI 환경은 아직 재현되지 않았다.

## 검증 결과 (2026-11-17)
- `cargo test --release` 236개 통과, 13개 ignored. `cargo clippy --release --all-targets -- -D warnings`와 `cargo fmt --check` 통과.
- 추가한 테스트 3개 통과:
  - `standing_button_accepts_normal_click_and_shortcut_restores_pending_pin`: 고정 상태에서도 일반 클릭이 전달되고, 캘린더 열기 단축키가 대기 중인 고정을 되돌린다.
  - `button_pixels_follow_ui_scale_without_adding_stale_screen_origin`: 버튼 영역이 화면 배율(1.0, 1.5, 2.0)을 따르고 고정 전 화면 좌표를 더하지 않는다.
  - `hit_test_uses_current_client_origin_including_negative_monitors`: 클릭 판정이 클릭 시점의 클라이언트 원점 기준이며 음수 좌표 모니터에서도 맞다.
- 마우스 훅 설치 실패는 `watch`가 `None`을 돌려주고, 앱은 분리 창으로 유지하며 안내 문구를 띄운다. 이 경로는 훅 설치를 실패시킬 수 없어 자동 테스트로 확인하지 못했다.
- 실제 바탕화면에서 고정 → 클릭 해제, 창 이동·DPI 변경 후 해제는 직접 확인하지 않았다.
