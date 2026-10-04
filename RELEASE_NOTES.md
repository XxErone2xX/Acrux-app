## Acrux macro V1.15.2 업데이트
- 미니게임 중에 릴링 바를 못 찾아서 낚시가 끊기던 문제 수정
  - 원인: 표시가 맨 왼쪽까지 떨어져 막대가 거의 비었고, 그 막대 색이 평소 청록보다 더 파란색이라 "릴링 바 없음"으로 봄 → 미니게임이 끝난 줄 알고 클릭을 멈춤
  - 이제 릴링 바가 떠 있는지는 막대 색이 아니라 '색 있는 칸(막대 · 물고기 구간)'으로 판단 → 막대가 비어 있어도 구간이 보이면 계속 릴링
  - 막대 색 범위를 청록 ~ 파랑으로 넓힘 (물고기 구간 색과는 겹치지 않게)
  - 구간 위에 남은 시간 숫자가 겹쳐 있으면 구간이 둘로 잘려 보이던 문제도 수정

## Acrux macro V1.15.2 Update
- Fixed fishing stopping mid-minigame because the reel bar wasn't found
  - Cause: the marker had dropped to the far left so the fill was nearly empty, and that fill was a bluer shade than the usual teal, so it read as "no reel bar" → it thought the minigame ended and stopped clicking
  - Whether the reel bar is showing is now judged by colored cells (fill or fish zone), not the fill color → it keeps reeling as long as the zone is visible, even with an empty fill
  - Widened the fill color range from teal to blue (without overlapping fish zone colors)
  - Also fixed the zone looking split in two when the countdown number overlaps it
