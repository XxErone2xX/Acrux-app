## Acrux macro V1.16.4 업데이트
- 미니게임이 끝났는데 X 를 바로 안 누르고, X 를 누른 뒤 다시 Fish 를 누르기까지 오래 걸리던 문제 수정
  - 원인: 낚시 창이 사라지면 릴링 바 자리에 뒤의 게임 화면(빨강 · 파랑 바닥 등)이 보이는데, 이걸 릴링 바로 착각해서 미니게임이 계속되는 줄 알고 최대 15초를 기다림
  - 이제 릴링 바 모양(왼쪽 끝이 어두운 바탕 또는 막대 색)인지도 확인
  - ◇ 표시가 미니게임 자리로 옮겨가는 게 한 번 확인되면, ◇ 가 미니게임 자리에서 사라지는 즉시(0.5초) 미니게임 끝으로 봄

## Acrux macro V1.16.4 Update
- Fixed X not being clicked right after the minigame ended, and the delay before clicking Fish again after X
  - Cause: once the fishing window disappears, the game world behind it (red · blue floor, etc.) shows at the reel bar spot, and it was mistaken for the reel bar, so it waited up to 15 s thinking the minigame was still going
  - It now also checks that it looks like a reel bar (left end is the dark track or the fill color)
  - Once the ◇ has been seen moving to its minigame spot, the minigame is treated as over as soon as the ◇ leaves that spot (0.5 s)
