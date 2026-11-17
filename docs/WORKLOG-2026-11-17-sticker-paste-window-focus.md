# 스티커 메모 그림 붙여넣기: 창 포커스 확인

## 증상
압정(항상 위) 스티커 메모 2개를 띄우고 한쪽을 편집하다가 다른 쪽으로 넘어가 그림을 붙여넣으면
두 메모 모두에 그림이 붙는다.

## 원인 (코드로 확인)
- 이미지만 클립보드에 있으면 egui는 Ctrl+V에서 `Event::Paste`를 만들지 않는다. 그래서
  `composer.rs`의 `paste_chord`가 `GetAsyncKeyState`로 Ctrl+V를 직접 읽는다.
- 이 값은 **시스템 전체** 키보드 상태라 창과 무관하다. 한편 egui의 텍스트 포커스
  (`has_focus(text_id)`)는 창(뷰포트)마다 따로 기억되어, 다른 창이 활성이어도 마지막으로 편집하던
  커서가 남아 있다.
- 결과: Ctrl+V 한 번에 두 창 모두 "포커스 있음 + 키 눌림"으로 판단해 각각 그림을 넣는다.
  `composer.rs`가 OS 창 포커스를 보지 않는다는 지적이 맞다.

## 수정
- `paste_chord`가 `ui.input(|i| i.viewport().focused != Some(false))`로 OS가 키보드를 준 창인지
  확인하고, 그 창에서만 동작한다. 눌림 상태(`was`)는 포커스와 무관하게 갱신해, 포커스가 넘어간
  순간 이미 눌려 있던 키가 새 붙여넣기로 오인되지 않는다.
- 상태를 알 수 없을 때(`None`)는 기존처럼 동작한다.
- 테스트용 이음새: `chord_down`, 클립보드 그림 읽기(`clipboard_picture_for_paste`)를 테스트에서
  교체할 수 있게 했다. 제품 동작은 그대로다(텍스트가 있으면 그림으로 붙이지 않는 규칙 포함).

## 검증
- 회귀 테스트 `paste_focus_tests::a_pasted_picture_goes_only_to_the_window_with_the_keyboard_focus`:
  한 `egui::Context`에 창 2개, 둘 다 텍스트 커서가 있는 상태에서 Ctrl+V. 수정 전 코드에서
  실패했고(`left: 1, right: 0`, 포커스 없는 창에도 그림이 붙음), 수정 후 통과한다. 키를 뗀 뒤
  포커스를 바꿔 다시 붙이면 반대쪽에만 붙는 것까지 확인한다.
- `cargo fmt`, `cargo clippy --release --all-targets -- -D warnings`, `cargo test --release`
  (242개 통과), 릴리스 빌드 통과.

## 확인하지 못한 것
실제 창 두 개를 띄워 손으로 붙여넣어 보지는 않았다(테스트는 창 포커스를 합성 입력으로 흉내 낸다).
