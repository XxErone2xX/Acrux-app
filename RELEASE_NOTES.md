## Acrux macro V1.16.1 업데이트
- 미니게임 중 표시가 물고기 구간 오른쪽으로 넘어가던 문제 수정
  - 원인: 누를 때마다 위로 올라가는 힘이 쌓이는데, 게임에 반영되기까지 몇 화면 걸리는 사이 계속 눌러서 힘이 한꺼번에 쌓여 튀어나감
  - 누른 뒤엔 올라가기 시작하는 게 보일 때까지 다시 안 누름
  - 올라가는 속도 제한: 구간에서 멀리 아래면 계속 눌러 따라가고, 가까우면 이미 올라가는 중일 땐 안 누름
  - 구간 가운데보다 오른쪽이면 절대 안 누름
  - 막대가 겹친 부분 때문에 구간이 줄어든 것처럼 보여도 전에 본 구간 폭을 기억해서 씀
  - 빠르게 움직인 직후 내 위치를 계속 못 보던 문제 수정
  - 물리 모델(누를 때마다 힘이 쌓임 / 정해진 힘 · 입력 지연 0~100ms)로 수백 번 돌려 값을 골랐고, 모든 경우에서 전보다 구간 안에 더 오래 머물고 오른쪽으로 덜 넘어감
  - 목표 위치 기본값 10 → 20% (기본값을 쓰던 사람은 자동으로 바뀜)
- 화면 복구할 때 UI 내비게이션(\ 키)을 쓰지 않고 원래대로 결과창 X 만 누름

## Acrux macro V1.16.1 Update
- Fixed the marker overshooting to the right of the fish zone during the minigame
  - Cause: each click adds upward force, and because the game takes a few frames to show a click, it kept clicking in that gap and the force piled up all at once
  - After a click it now waits until the marker visibly starts rising before clicking again
  - Rise speed limit: far below the zone it keeps clicking to catch up; close to it, it doesn't click while already rising
  - Never clicks when right of the zone's middle
  - Remembers the zone's width even if the fill overlap makes the zone look narrower
  - Fixed losing track of the marker right after a fast move
  - Values chosen by running hundreds of reels on physics models (force adds up per click / fixed force · input lag 0–100 ms); in every case it stays in the zone longer and overshoots less than before
  - Default target point 10 → 20% (applied automatically if you used the default)
- Screen recovery no longer uses UI navigation (\ key); it just clicks the result X like before
