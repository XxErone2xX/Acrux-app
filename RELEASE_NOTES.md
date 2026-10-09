## Acrux macro V2.0.14 업데이트
- 오토 메모리 매치: 시작한 뒤 카드를 제대로 안 풀던 문제 수정 (스크립트 매크로와 같은 풀이)
  - 이미 아는 두 장이 같으면 그 두 장 → 아니면 안 뒤집은 카드 하나 → 그게 아는 카드와 같으면 그 카드 · 아니면 안 뒤집은 카드 하나 더 · 뒤집은 카드는 전부 기억
  - 카드는 0.1초 꾹 눌러서 누름 · 안 뒤집히면 다시 누름 (최대 3번) · 뒤집혔는지 확인하고 넘어감
  - 카드 구별을 아이템 그림 색 + 개수 글자(3배로 키워 읽음)로 바꿈 — 카드 뒷배경 무늬 때문에 같은 카드를 다른 카드로 보던 문제
  - 남은 기회를 화면 숫자로 확인
- 카드 판 자동 보정 추가 (매크로 기준 위치 설정 → 오토 메모리 매치 → 카드 판): 카드 판이 떠 있을 때 누르면 카드 20장 · 남은 기회 · Close 자리를 저장해서 그 위치를 씀

## Acrux macro V2.0.14 Update
- Auto memory match: fixed the board not really being solved after starting (same approach as the script macro)
  - Two known matching cards first → otherwise one unflipped card → its known match if any · otherwise one more unflipped card · every flipped card is remembered
  - Cards are pressed with a 0.1 s hold · pressed again if they don't flip (up to 3 times) · each flip is confirmed before moving on
  - Cards are identified by the item art's color + the count text (read at 3x) — the card background pattern made identical cards look different
  - Remaining chances are read from the on-screen number
- Added card board auto calibration (Macro base position settings → Auto memory match → Card board): press while the board is open to save the 20 cards, the chances counter and Close, and those positions are used
