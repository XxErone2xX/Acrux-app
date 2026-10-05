## Acrux macro V1.19.1 업데이트
- 미니게임 창이 올라오는 동안 릴링 바 위치를 맞춰 버려서 **미니게임 내내 클릭을 안 하던 버그** 수정
  - 바 위치는 창이 멈춘 뒤(같은 자리가 0.15초 이어질 때)에만 맞추고, 바 높이가 맞지 않는 줄은 무시
  - 맞춘 자리에서 바가 계속 안 보이면 다시 맞춤
- 자동 보정이 미니게임 창을 아래쪽 Roll 버튼 줄과 이어 붙여서 못 찾던 문제 수정
- 자동 보정이 창이 열리는 애니메이션 중의 크기를 재지 않게 함 (두 번 연속 같은 자리일 때만 저장)

## Acrux macro V1.19.1 Update
- Fixed a bug where the reel bar was snapped while the minigame window was still sliding in, so **nothing was clicked for the whole minigame**
  - The bar is snapped only once the window stops (same spot for 0.15 s), and lines with the wrong height are ignored
  - If the bar keeps going missing at the snapped spot, it snaps again
- Fixed auto calibrate joining the minigame window to the Roll button row below it and failing to find the window
- Auto calibrate no longer measures a window mid-opening animation (saved only when two shots in a row agree)
